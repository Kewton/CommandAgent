//! A model-written verifier fails while the host's deterministic oracles pass.
//!
//! The host cannot tell which side is wrong, so the terminal failure only
//! records the disagreement. `PlanStep::verify` commands stay immutable and the
//! phase still fails; nothing here changes `VerificationReport::is_pass`.
use std::collections::BTreeSet;

use crate::config::Config;
use crate::eval_events::body_snippet;
use crate::planner::profile::{build_oracle_for_command, resolve_profile_runtime};
use crate::planner::recovery_contract_authority::{
    local_script_path, verifier_obligations::normalized,
};
use crate::planner::step_plan::{ExpectedResult, PlanStep, StepKind, StepPlan};
use crate::planner::verify::VerificationReport;

pub(super) const FAILURE_KIND: &str = "model_verifier_disagrees_with_host_oracle";
const OUTPUT_EXCERPT_CHARS: usize = 400;

/// Handoff evidence for an applied disagreement.
pub(super) struct Disagreement {
    pub(super) evidence: Vec<String>,
}

/// Apply only when every condition holds; otherwise the caller keeps the
/// ordinary `verify_repair_progress_unchanged` classification.
pub(super) fn detect(
    config: &Config,
    plan: &StepPlan,
    step: &PlanStep,
    goal: &str,
    report: &VerificationReport,
) -> Option<Disagreement> {
    if step.step_kind() != StepKind::Verify || step.expected_result_kind() != ExpectedResult::Pass {
        return None;
    }
    let runtime = resolve_profile_runtime(&config.profile);
    if !only_command_failures(report) || !runtime.has_behavior_oracle() {
        return None;
    }
    let (failure, script, producer) = model_verifier_failure(config, plan, step, report)?;
    if !runtime.verify_final(&config.workspace_root, goal).is_pass() {
        return None;
    }
    let probe = runtime
        .run_behavior_probe(
            &runtime.profile_id(),
            &config.workspace_root,
            goal,
            &runtime.required_capabilities(goal),
            config.offline,
        )
        .ok()
        .filter(|probe| probe.status == "pass")?;
    let mut evidence = vec![
        format!(
            "{FAILURE_KIND}: host oracles pass while the model-written verifier fails; \
             which side is wrong is not decided"
        ),
        format!("model_verifier: {script}"),
        format!("model_verifier_producer_step: {producer}"),
        format!("model_verifier_command: {}", failure.command),
        format!("model_verifier_output: {}", output_excerpt(&failure.reason)),
        "verify_final=pass".to_string(),
        "behavior_probe=pass".to_string(),
    ];
    if let Some(path) = probe.evidence_path {
        evidence.push(format!("behavior_probe_evidence: {path}"));
    }
    Some(Disagreement { evidence })
}

fn only_command_failures(report: &VerificationReport) -> bool {
    !report.command_failures.is_empty()
        && report.missing_paths.is_empty()
        && report.dependency_missing.is_empty()
        && report.profile_failures.is_empty()
        && report.compile_errors.is_empty()
        && report.verifier_command_false_negatives.is_empty()
}

/// Every failed command must run a script that an earlier passing Implement
/// step of the same plan produced, and none may be a host oracle command.
/// Returns the first failure with its script path and producing step id.
fn model_verifier_failure<'a>(
    config: &Config,
    plan: &StepPlan,
    step: &PlanStep,
    report: &'a VerificationReport,
) -> Option<(&'a crate::planner::verify::CommandFailure, String, String)> {
    let position = plan.steps.iter().position(|s| s.id == step.id)?;
    let mut first = None;
    for failure in &report.command_failures {
        if build_oracle_for_command(Some(&config.profile), &failure.command).is_some() {
            return None;
        }
        let script = local_script_path(&failure.command)?;
        let producer = plan.steps[..position]
            .iter()
            .find(|earlier| produces(earlier, &script))?;
        first.get_or_insert((failure, script, producer.id.clone()));
    }
    first
}

fn produces(step: &PlanStep, script: &str) -> bool {
    step.step_kind() == StepKind::Implement
        && step.expected_result_kind() == ExpectedResult::Pass
        && step
            .expected_paths
            .iter()
            .filter_map(|path| normalized(path).ok())
            .collect::<BTreeSet<_>>()
            .contains(script)
}

fn output_excerpt(reason: &str) -> String {
    let from = reason.find("FAIL").unwrap_or(0);
    body_snippet(&reason[from..])
        .chars()
        .take(OUTPUT_EXCERPT_CHARS)
        .collect()
}

#[cfg(test)]
#[path = "model_verifier_disagreement/tests.rs"]
mod tests;
