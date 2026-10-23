//! A Verify step that rewrote a model-written verifier passes only when the
//! host's deterministic oracle also passes.
//!
//! The host cannot tell whether a rewrite weakened the verifier, so any rewrite
//! of the step's own registered verify script needs an oracle verdict the model
//! cannot influence. `PlanStep::verify` commands stay immutable; this module only
//! decides whether the retry's pass may stand.
use std::collections::BTreeSet;

use super::model_verifier_disagreement::{detect, model_verifier_failure, names_command, produces};
use super::{
    Config, RepairContext, RepairTarget, StepFailureHandoff, StepRunError, StepRunOutcome,
    VerificationReport, eval_events, json,
};
use crate::minimal_loop::completion::CompletionContract;
use crate::planner::profile::{build_oracle_for_command, resolve_profile_runtime};
use crate::planner::recovery_contract_authority::{
    local_script_path,
    verifier_obligations::{checked_paths, generated, normalized},
};
use crate::planner::step_plan::{PlanStep, StepKind, StepPlan};

pub(super) const WITHOUT_ORACLE_KIND: &str = "model_verifier_edited_without_host_oracle";
pub(super) const ORACLE_FAILED_KIND: &str = "model_verifier_edited_host_oracle_failed";
const ORACLE_PASSED_KIND: &str = "model_verifier_edited_host_oracle_passed";

/// Scripts of this Verify step's verify commands that changed in the step.
#[derive(Default)]
struct EditedScripts {
    /// Created by an earlier Implement step or registered by the model.
    eligible: Vec<String>,
    /// Host oracle, protected, or private scripts: no authority to rewrite.
    denied: Vec<String>,
}

fn relative(config: &Config, path: &str) -> Option<String> {
    let path = std::path::Path::new(path);
    let path = path.strip_prefix(&config.workspace_root).unwrap_or(path);
    normalized(&path.to_string_lossy()).ok()
}

fn edited_scripts(
    config: &Config,
    plan: &StepPlan,
    step: &PlanStep,
    changed_paths: &[String],
) -> EditedScripts {
    let mut edited = EditedScripts::default();
    let Some(position) = plan.steps.iter().position(|s| s.id == step.id) else {
        return edited;
    };
    let changed: BTreeSet<String> = changed_paths
        .iter()
        .filter_map(|path| relative(config, path))
        .collect();
    // An unreadable contract cannot vouch for any verifier: fail closed.
    let contract = CompletionContract::load_for_config(config);
    let unreadable = contract.is_err();
    let contract = contract.ok().flatten();
    let oracles = contract
        .as_ref()
        .map(|contract| contract.deterministic_oracles.clone())
        .unwrap_or_default();
    let registered = generated(config);
    for command in &step.verify {
        let normalized_commands =
            crate::planner::verify::normalize_planner_verify_command(command).unwrap_or_default();
        for command in normalized_commands {
            let Some(script) = local_script_path(&command).filter(|s| changed.contains(s)) else {
                continue;
            };
            let owned = PlanStep {
                expected_paths: vec![script.clone()],
                ..step.clone()
            };
            let authorized = !unreadable
                && build_oracle_for_command(Some(&config.profile), &command).is_none()
                && !names_command(&oracles, &command)
                && contract
                    .as_ref()
                    .is_none_or(|contract| checked_paths(config, contract, &owned).is_ok());
            let produced = plan.steps[..position]
                .iter()
                .chain(registered.iter())
                .any(|earlier| produces(earlier, &script));
            let target = if authorized && produced {
                &mut edited.eligible
            } else if authorized {
                continue;
            } else {
                &mut edited.denied
            };
            if !target.contains(&script) {
                target.push(script);
            }
        }
    }
    edited
}

/// Who may fix the verifier: set only when #820's disagreement conditions hold
/// on the first failing verification.
pub(super) fn authority(
    config: &Config,
    plan: &StepPlan,
    step: &PlanStep,
    goal: &str,
    report: &VerificationReport,
) -> Option<String> {
    detect(config, plan, step, goal, report)?;
    let (_, script, _) = model_verifier_failure(config, plan, step, report)?;
    let edited = edited_scripts(config, plan, step, std::slice::from_ref(&script));
    edited.eligible.contains(&script).then_some(script)
}

enum Verdict {
    Passed(Vec<String>),
    Failed(&'static str, Vec<String>),
}

fn judge(
    config: &Config,
    plan: &StepPlan,
    step: &PlanStep,
    goal: &str,
    changed_paths: &[String],
) -> Option<Verdict> {
    if step.step_kind() != StepKind::Verify {
        return None;
    }
    let edited = edited_scripts(config, plan, step, changed_paths);
    if edited.eligible.is_empty() && edited.denied.is_empty() {
        return None;
    }
    let mut evidence = vec![format!(
        "model_verifier_edited: {}",
        [edited.eligible.as_slice(), edited.denied.as_slice()]
            .concat()
            .join(", ")
    )];
    let runtime = resolve_profile_runtime(&config.profile);
    if !edited.denied.is_empty() || !runtime.has_behavior_oracle() {
        evidence.push(format!(
            "{WITHOUT_ORACLE_KIND}: no host oracle may vouch for a rewritten verifier here \
             (host-oracle or protected script: {}; behavior oracle: {})",
            !edited.denied.is_empty(),
            runtime.has_behavior_oracle()
        ));
        return Some(Verdict::Failed(WITHOUT_ORACLE_KIND, evidence));
    }
    let final_report = runtime.verify_final(&config.workspace_root, goal);
    evidence.push(format!("verify_final={}", final_report.primary_reason()));
    let probe = runtime.run_behavior_probe(
        &runtime.profile_id(),
        &config.workspace_root,
        goal,
        &runtime.required_capabilities(goal),
        config.offline,
    );
    let probe_status = match &probe {
        Ok(probe) => probe.status.to_string(),
        Err(err) => format!("error: {err}"),
    };
    evidence.push(format!("behavior_probe={probe_status}"));
    if final_report.is_pass() && probe_status == "pass" {
        Some(Verdict::Passed(evidence))
    } else {
        Some(Verdict::Failed(ORACLE_FAILED_KIND, evidence))
    }
}

/// Gate a passing Verify step; `None` lets the pass stand.
pub(super) fn reject(
    config: &Config,
    plan: &StepPlan,
    step: &PlanStep,
    goal: &str,
    outcome: &StepRunOutcome,
) -> Option<StepRunError> {
    let verdict = judge(config, plan, step, goal, &outcome.changed_paths)?;
    let (kind, passed, evidence) = match verdict {
        Verdict::Passed(evidence) => (ORACLE_PASSED_KIND, true, evidence),
        Verdict::Failed(kind, evidence) => (kind, false, evidence),
    };
    let reason = format!("{kind}: {}", evidence.join("; "));
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "step_verify_repair",
            "step_id": step.id,
            "attempt": outcome.repair_attempts,
            "ok": passed,
            "repair_target": RepairTarget::ModelVerifierEdit.as_str(),
            "failure_kind": kind,
            "primary_reason": eval_events::body_snippet(&reason),
            "changed_paths": outcome.changed_paths.clone(),
            "allowed_action": RepairTarget::ModelVerifierEdit.allowed_action(),
        }),
    );
    if passed {
        return None;
    }
    let report = VerificationReport::profile_failed(reason);
    let context = RepairContext {
        profile: Some(config.profile.clone()),
        overall_goal: Some(goal.to_string()),
        step_instruction: Some(step.instruction.clone()),
        expected_paths: step.expected_paths.clone(),
        verify_commands: step.verify.clone(),
        changed_files: outcome.changed_paths.clone(),
        repair_stop_reason: Some(kind.to_string()),
        workspace_root: Some(config.workspace_root.clone()),
        eval_events_path: config.eval_events_path.clone(),
        prompt_layout: config.prompt_layout,
        ..RepairContext::default()
    };
    Some(
        StepFailureHandoff {
            config,
            plan_goal: &plan.goal,
            step_id: &step.id,
            report: &report,
            context: &context,
            failure_kind: kind,
            repair_target: RepairTarget::ModelVerifierEdit,
            repair_stop_reason: outcome.stop_reason.as_deref(),
            blocked_requirements: &[],
            extra_evidence: &evidence,
        }
        .into_error(outcome.clone()),
    )
}

#[cfg(test)]
#[path = "model_verifier_edit/tests.rs"]
mod tests;
