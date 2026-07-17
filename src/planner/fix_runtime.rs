use std::path::Path;

use anyhow::Context;
use serde::Serialize;
use serde_json::json;

use crate::config::Config;
use crate::eval_events;
use crate::planner::adjudication::contract::{
    EvidenceStage, ExpectedOutcome, FIX_CONTRACT_REF, FIX_CONTRACT_VERSION, is_fix_intent,
};
use crate::planner::adjudication::fix::{
    AFTER_PASSES_ID, BEFORE_FAILS_ID, FixAdjudication, FixAssurance, FixEvidenceBundle,
    FixEvidenceObservation, NO_REGRESSION_ID, ProbeOutcome, evaluate_fix_evidence,
    evidence_lineage, reproducer_lineage,
};
use crate::planner::profile::{
    ProfileFixRegressionAdapter, ProfileFixRegressionBinding, profile_fix_regression_bindings,
    run_profile_fix_regressions,
};
use crate::planner::step_plan::{ExpectedResult, StepKind, StepPlan};
use crate::planner::ultra_plan::{UltraPhase, UltraPlan};

mod evidence;
use evidence::*;
#[cfg(test)]
mod fix6_tests;

pub(crate) const FIX_CONTRACT_ORIGIN: &str = "fix_intent_v0";

#[derive(Debug, Clone)]
struct ReproducerBinding {
    command: String,
    lineage: String,
}

#[derive(Debug)]
pub(crate) struct FixRuntime {
    terminal_config: Config,
    run_id: String,
    profile: String,
    goal: String,
    regression_bindings: Vec<ProfileFixRegressionBinding>,
    reproducer: Option<ReproducerBinding>,
    before: Option<FixEvidenceObservation>,
    after: Option<FixEvidenceObservation>,
    regressions: Vec<FixEvidenceObservation>,
    contract_predicate: Option<crate::planner::fix_contract_predicate::FixContractPredicateContext>,
    diagnostic: Option<crate::planner::fix_diagnostics::FixFailureDiagnostic>,
    epoch: u64,
    fix_written: bool,
    terminalized: bool,
}

#[derive(Serialize)]
struct PersistedFixAdjudication<'a> {
    schema_version: &'static str,
    intent: &'static str,
    contract_version: &'static str,
    contract_ref: &'static str,
    run_id: &'a str,
    adjudication: &'a FixAdjudication,
    evidence: &'a FixEvidenceBundle,
}

pub(crate) fn generation_rules(intent: &str) -> &'static str {
    if !is_fix_intent(intent) {
        return "";
    }
    "- Fix intent: phase 1 must only identify one deterministic reproducer R; it must not request edits, setup, dependency changes, or repairs.\n\
- Phase 1 must ask /plan-run for exactly one verify step whose expected_result is fail and whose verify list contains exactly one bounded command.\n\
- Put repair work after phase 1. End with focused verification and profile-bound regression verification.\n\
- Do not claim design quality, minimality, or elegance as acceptance evidence.\n"
}

pub(crate) fn applies(plan: &UltraPlan) -> bool {
    is_fix_intent(&plan.intent)
}

pub(crate) fn phase_prompt(
    plan: &UltraPlan,
    phase: &UltraPhase,
    base: String,
    explicit_fix: bool,
) -> String {
    if !applies(plan) {
        return base;
    }
    let guidance = if plan
        .phases
        .first()
        .is_some_and(|first| first.id == phase.id)
    {
        "Fix contract phase role: reproducer_before. Return exactly one verify step, expected_result=\"fail\", with exactly one deterministic verify command. Declare no expected_paths and do not inspect, write, edit, set up, install, or repair anything. The runtime executes this command directly before any repair."
    } else if explicit_fix && phase.id == "isolate-cause" {
        "Fix contract phase role: cause_isolation. Inspect and narrow the cause of the observed failure without writing, editing, setting up, installing, or repairing."
    } else if explicit_fix && phase.id == "verify-regressions" {
        "Fix contract phase role: regression_verification. Use verify-only steps and do not modify the workspace. The runtime independently reruns the original reproducer and frozen profile regression set after this phase."
    } else {
        "Fix contract phase role: repair. Do not use expected_result=\"fail\" after the baseline phase. Repair the observed defect; the runtime will independently rerun the original reproducer and the frozen profile regression set."
    };
    format!("{base}\n\n{guidance}")
}

pub(crate) fn is_before_prompt(prompt: &str) -> bool {
    prompt.lines().any(|line| {
        line.strip_prefix("Intent:")
            .is_some_and(|intent| is_fix_intent(intent.trim()))
    }) && prompt.contains("Fix contract phase role: reproducer_before")
}

pub(crate) fn bind_step_plan(
    runtime: Option<&FixRuntime>,
    phase: &UltraPhase,
    plan: &mut StepPlan,
) {
    crate::planner::fix_diagnostics::bind_step_plan(
        phase,
        runtime.and_then(FixRuntime::repair_diagnostic),
        plan,
    );
    crate::planner::fix_contract_predicate::bind_step_plan(
        phase,
        runtime.and_then(FixRuntime::contract_predicate),
        plan,
    );
}

impl FixRuntime {
    pub(crate) fn for_plan(plan: &UltraPlan, config: &Config) -> Option<Self> {
        if !applies(plan) {
            return None;
        }
        Some(Self {
            terminal_config: config.clone(),
            run_id: uuid::Uuid::now_v7().to_string(),
            profile: plan.profile.clone(),
            goal: plan.goal.clone(),
            regression_bindings: profile_fix_regression_bindings(
                &config.workspace_root,
                &plan.profile,
                &plan.goal,
            ),
            reproducer: None,
            before: None,
            after: None,
            regressions: Vec::new(),
            contract_predicate: None,
            diagnostic: None,
            epoch: 0,
            fix_written: false,
            terminalized: false,
        })
    }

    pub(crate) const fn is_before_phase(&self, index: usize) -> bool {
        index == 0
    }

    pub(crate) fn repair_diagnostic(
        &self,
    ) -> Option<&crate::planner::fix_diagnostics::FixFailureDiagnostic> {
        self.diagnostic.as_ref()
    }

    pub(crate) fn contract_predicate(
        &self,
    ) -> Option<&crate::planner::fix_contract_predicate::FixContractPredicateContext> {
        self.contract_predicate.as_ref()
    }

    pub(crate) fn run_before_phase(
        &mut self,
        step_plan: &StepPlan,
        config: &Config,
        plan: &UltraPlan,
        phase: &UltraPhase,
        index: usize,
    ) -> anyhow::Result<crate::planner::fix_reproducer_defect::BeforePhaseOutcome> {
        if self.before.is_some() || self.reproducer.is_some() {
            anyhow::bail!("fix reproducer is already bound; F1 lineage cannot change");
        }
        let binding = match extract_reproducer(step_plan) {
            Ok(binding) => binding,
            Err(error) => {
                let reason = format!("reproducer_not_identified:{error}");
                emit_unbound_failure(
                    config,
                    &self.profile,
                    &self.run_id,
                    &self.regression_bindings,
                    &reason,
                )?;
                self.terminalized = true;
                anyhow::bail!(reason);
            }
        };
        self.epoch += 1;
        let run = crate::planner::fix_diagnostics::run_reproducer(
            config,
            &self.run_id,
            BEFORE_FAILS_ID,
            EvidenceStage::Before,
            ExpectedOutcome::Failure,
            self.epoch,
            &binding.command,
            &binding.lineage,
            &self.profile,
            &plan.goal,
        );
        let observation = run.evidence;
        self.diagnostic = run.diagnostic;
        let path = if run.reproducer_defect.is_some() {
            before_attempt_evidence_path(&self.run_id, observation.epoch)
        } else {
            before_evidence_path(&self.run_id)
        };
        persist_json(&config.workspace_root, &path, &observation)?;
        emit_probe_observation(config, &observation, &path);
        if let Some(error_kind) = run.reproducer_defect {
            return Ok(
                crate::planner::fix_reproducer_defect::BeforePhaseOutcome::RebuildRequired {
                    feedback: crate::planner::fix_reproducer_defect::rebuild_feedback(&error_kind),
                },
            );
        }
        self.reproducer = Some(binding);
        self.before = Some(observation.clone());

        if observation.outcome != ProbeOutcome::Failure {
            let bundle = self.bundle();
            let adjudication = evaluate_fix_evidence(&bundle);
            persist_adjudication(&config.workspace_root, &self.run_id, &adjudication, &bundle)?;
            emit_final_adjudication(config, &self.profile, &adjudication, &bundle);
            self.terminalized = true;
            anyhow::bail!("fix baseline gate failed: {}", adjudication.reason);
        }

        self.contract_predicate = self.reproducer.as_ref().and_then(|binding| {
            crate::planner::fix_contract_predicate::FixContractPredicateContext::from_failed_reproducer(
                &config.workspace_root,
                &self.profile,
                &binding.command,
                config.eval_events_path.as_deref(),
            )
        });

        emit_direct_phase_complete(config, plan, phase, index);
        crate::bounded_process::reap_registered_server_children_for_workspace(
            config.eval_events_path.as_deref(),
            "phase_transition",
            &config.workspace_root,
        );
        Ok(crate::planner::fix_reproducer_defect::BeforePhaseOutcome::Confirmed)
    }

    pub(crate) fn finish(mut self, config: &Config, plan: &UltraPlan) -> anyhow::Result<String> {
        let binding = self
            .reproducer
            .as_ref()
            .context("fix reproducer was not bound before repair")?
            .clone();
        self.fix_written = true;
        let mut epoch = self.epoch + 1;
        let after = crate::planner::fix_diagnostics::run_reproducer(
            config,
            &self.run_id,
            AFTER_PASSES_ID,
            EvidenceStage::After,
            ExpectedOutcome::Success,
            epoch,
            &binding.command,
            &binding.lineage,
            &self.profile,
            &plan.goal,
        )
        .evidence;
        self.after = Some(after.clone());
        let path = after_evidence_path(&self.run_id);
        persist_json(&config.workspace_root, &path, &after)?;
        emit_probe_observation(config, &after, &path);

        if after.outcome == ProbeOutcome::Success {
            for regression in run_profile_fix_regressions(
                &config.workspace_root,
                &self.profile,
                &self.goal,
                &self.regression_bindings,
                config.offline,
            ) {
                epoch += 1;
                let lineage = self
                    .regression_bindings
                    .iter()
                    .find(|binding| binding.id == regression.id)
                    .map(regression_binding_lineage)
                    .unwrap_or_default();
                let observation = FixEvidenceObservation::new(
                    NO_REGRESSION_ID,
                    &regression.id,
                    EvidenceStage::After,
                    ExpectedOutcome::Success,
                    &lineage,
                    epoch,
                    &self.run_id,
                    regression.outcome,
                    &regression.reason,
                );
                let path = regression_evidence_path(&self.run_id, &regression.id);
                persist_json(&config.workspace_root, &path, &observation)?;
                emit_probe_observation(config, &observation, &path);
                self.regressions.push(observation);
            }
        }

        let bundle = self.bundle();
        let adjudication = evaluate_fix_evidence(&bundle);
        persist_adjudication(&config.workspace_root, &self.run_id, &adjudication, &bundle)?;
        let (assurance_level, assurance_reason) =
            emit_final_adjudication(config, &self.profile, &adjudication, &bundle);
        self.terminalized = true;
        if adjudication.assurance == FixAssurance::Failed {
            anyhow::bail!("fix final acceptance failed: {}", adjudication.reason);
        }
        eval_events::emit(
            config.eval_events_path.as_deref(),
            json!({
                "event": "ultra_plan_complete",
                "total_phases": plan.phases.len(),
                "profile": plan.profile,
                "intent": "fix",
                "assurance_level": assurance_level,
                "assurance_reason": assurance_reason,
                "ok": true,
            }),
        );
        Ok(format!(
            "ultra-plan-run complete: {} phases",
            plan.phases.len()
        ))
    }

    fn bundle(&self) -> FixEvidenceBundle {
        FixEvidenceBundle {
            run_id: self.run_id.clone(),
            fix_written: self.fix_written,
            bound_regression_ids: self
                .regression_bindings
                .iter()
                .map(|binding| binding.id.clone())
                .collect(),
            bound_regression_lineages: self
                .regression_bindings
                .iter()
                .map(|binding| (binding.id.clone(), regression_binding_lineage(binding)))
                .collect(),
            before: self.before.clone(),
            after: self.after.clone(),
            regressions: self.regressions.clone(),
        }
    }
}

impl Drop for FixRuntime {
    fn drop(&mut self) {
        if self.terminalized {
            return;
        }
        let bundle = self.bundle();
        let adjudication = evaluate_fix_evidence(&bundle);
        let _ = persist_adjudication(
            &self.terminal_config.workspace_root,
            &self.run_id,
            &adjudication,
            &bundle,
        );
        emit_final_adjudication(&self.terminal_config, &self.profile, &adjudication, &bundle);
        self.terminalized = true;
    }
}

fn regression_binding_lineage(binding: &ProfileFixRegressionBinding) -> String {
    let adapter = match &binding.adapter {
        ProfileFixRegressionAdapter::VerifyCommand(command) => format!("verify:{command}"),
        ProfileFixRegressionAdapter::ProfileContract => "profile_contract".to_string(),
        ProfileFixRegressionAdapter::DataManifestCheck => "data_manifest_check".to_string(),
    };
    evidence_lineage("regression", &format!("{}\0{adapter}", binding.id))
}

fn extract_reproducer(plan: &StepPlan) -> anyhow::Result<ReproducerBinding> {
    if plan.steps.len() != 1 {
        anyhow::bail!("before phase must contain exactly one step");
    }
    let step = &plan.steps[0];
    if step.step_kind() != StepKind::Verify
        || step.expected_result_kind() != ExpectedResult::Fail
        || !step.expected_paths.is_empty()
        || step.verify.len() != 1
    {
        anyhow::bail!(
            "before phase requires one verify step with expected_result=fail, one command, and no expected paths (kind={}, expected_result={}, verify_count={}, expected_path_count={})",
            step.kind,
            step.expected_result,
            step.verify.len(),
            step.expected_paths.len(),
        );
    }
    let normalized = crate::planner::verify::normalize_verify_command(&step.verify[0])?;
    let command = normalized.into_string();
    Ok(ReproducerBinding {
        lineage: reproducer_lineage(&command),
        command,
    })
}

fn emit_direct_phase_complete(config: &Config, plan: &UltraPlan, phase: &UltraPhase, index: usize) {
    for (event, stage) in [
        ("ultra_phase_execute_complete", "fix_before_probe"),
        ("ultra_phase_profile_check", "fix_before_observed"),
        ("ultra_phase_complete", "complete"),
    ] {
        eval_events::emit(
            config.eval_events_path.as_deref(),
            json!({
                "event": event,
                "phase_id": phase.id,
                "phase_index": index + 1,
                "total_phases": plan.phases.len(),
                "final_phase": index + 1 == plan.phases.len(),
                "stage": stage,
                "ok": true,
                "reason": "",
                "step_count": 1,
            }),
        );
    }
}

fn emit_unbound_failure(
    config: &Config,
    profile: &str,
    run_id: &str,
    regression_bindings: &[ProfileFixRegressionBinding],
    reason: &str,
) -> anyhow::Result<()> {
    let adjudication = FixAdjudication {
        assurance: FixAssurance::Failed,
        reason: reason.to_string(),
        requirement_statuses: std::collections::BTreeMap::from([
            (BEFORE_FAILS_ID.to_string(), "unverified".to_string()),
            (AFTER_PASSES_ID.to_string(), "not_executed".to_string()),
            (NO_REGRESSION_ID.to_string(), "not_executed".to_string()),
        ]),
    };
    let bundle = FixEvidenceBundle {
        run_id: run_id.to_string(),
        fix_written: false,
        bound_regression_ids: regression_bindings
            .iter()
            .map(|binding| binding.id.clone())
            .collect(),
        bound_regression_lineages: regression_bindings
            .iter()
            .map(|binding| (binding.id.clone(), regression_binding_lineage(binding)))
            .collect(),
        before: None,
        after: None,
        regressions: Vec::new(),
    };
    persist_adjudication(&config.workspace_root, run_id, &adjudication, &bundle)?;
    emit_final_adjudication(config, profile, &adjudication, &bundle);
    Ok(())
}

fn emit_final_adjudication(
    config: &Config,
    profile: &str,
    adjudication: &FixAdjudication,
    bundle: &FixEvidenceBundle,
) -> (String, String) {
    let assurance = adjudication.assurance;
    let mut assurance_level = assurance.as_str().to_string();
    let mut assurance_reason = adjudication.reason.clone();
    crate::planner::profile_admission::cap_assurance(
        profile,
        &mut assurance_level,
        &mut assurance_reason,
    );
    let ok = matches!(assurance, FixAssurance::Full | FixAssurance::Partial);
    let final_status = match assurance {
        FixAssurance::Full => "full_success",
        FixAssurance::Partial => "partial",
        FixAssurance::Static => "incomplete",
        FixAssurance::Failed => "failed",
    };
    let runtime_status = match assurance {
        FixAssurance::Full => "pass",
        FixAssurance::Partial => "partial",
        FixAssurance::Static => "static",
        FixAssurance::Failed => "failed",
    };
    let release_quality = match assurance {
        FixAssurance::Full => "release_ready",
        FixAssurance::Partial => "partial",
        FixAssurance::Static => "not_checked",
        FixAssurance::Failed => "failed",
    };
    let unverified = adjudication
        .requirement_statuses
        .iter()
        .filter(|(_, status)| !matches!(status.as_str(), "passed"))
        .map(|(id, status)| format!("{id}:{status}"))
        .collect::<Vec<_>>();
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "ultra_final_acceptance",
            "cycle_index": 0,
            "intent": "fix",
            "profile": profile,
            "effective_profile": profile,
            "prompt_layout": config.prompt_layout.as_str(),
            "contract_origin": FIX_CONTRACT_ORIGIN,
            "contract_version": FIX_CONTRACT_VERSION,
            "contract_ref": FIX_CONTRACT_REF,
            "verdict": assurance.as_str(),
            "assurance_level": assurance_level,
            "assurance_reason": assurance_reason,
            "runtime_acceptance_passed": ok,
            "runtime_acceptance_status": runtime_status,
            "final_acceptance_status": final_status,
            "release_gate_status": "not_applicable",
            "release_quality_completion": release_quality,
            "completion_contract_verification_enabled": true,
            "completion_contract_path_merge_enabled": false,
            "completion_contract_path": FIX_CONTRACT_REF,
            "completion_contract_generated": false,
            "external_contract_checked": true,
            "external_contract_ok": assurance == FixAssurance::Full,
            "required_evidence": [BEFORE_FAILS_ID, AFTER_PASSES_ID, NO_REGRESSION_ID],
            "requirement_statuses": adjudication.requirement_statuses,
            "unverified_evidence": unverified,
            "fix_run_id": bundle.run_id,
            "before_epoch": bundle.before.as_ref().map(|item| item.epoch),
            "after_epoch": bundle.after.as_ref().map(|item| item.epoch),
            "reproducer_lineage": bundle.before.as_ref().map(|item| item.lineage.clone()).unwrap_or_default(),
            "bound_regression_ids": bundle.bound_regression_ids,
            "bound_regression_lineages": bundle.bound_regression_lineages,
            "fix_evidence_paths": fix_evidence_paths(bundle),
            "ok": ok,
        }),
    );
    (assurance_level, assurance_reason)
}

fn fix_evidence_paths(bundle: &FixEvidenceBundle) -> Vec<String> {
    let mut paths = vec![adjudication_evidence_path(&bundle.run_id)];
    if bundle.before.is_some() {
        paths.push(before_evidence_path(&bundle.run_id));
    }
    if bundle.after.is_some() {
        paths.push(after_evidence_path(&bundle.run_id));
    }
    paths.extend(
        bundle
            .regressions
            .iter()
            .map(|item| regression_evidence_path(&bundle.run_id, &item.binding_id)),
    );
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use crate::planner::step_plan::PlanStep;
    use clap::Parser;

    fn reproducer_plan(command: &str) -> StepPlan {
        StepPlan {
            goal: "reproduce".to_string(),
            steps: vec![PlanStep {
                id: "reproduce-before".to_string(),
                kind: "verify".to_string(),
                expected_result: "fail".to_string(),
                instruction: "Run the deterministic reproducer before repair".to_string(),
                expected_paths: Vec::new(),
                verify: vec![command.to_string()],
            }],
        }
    }

    fn fix_plan() -> UltraPlan {
        UltraPlan {
            goal: "fix missing marker".to_string(),
            profile: "generic".to_string(),
            style: "default".to_string(),
            intent: "fix".to_string(),
            phases: vec![
                UltraPhase {
                    id: "reproducer-before".to_string(),
                    prompt: "Bind the deterministic reproducer without editing files".to_string(),
                },
                UltraPhase {
                    id: "repair".to_string(),
                    prompt: "Repair the missing marker".to_string(),
                },
            ],
        }
    }

    fn config(root: &Path) -> Config {
        let cwd = root.to_string_lossy().to_string();
        let mut config = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--offline",
            "--profile",
            "generic",
            "--ultra-plan",
            "fix missing marker",
        ]))
        .unwrap();
        config.eval_events_path = Some(root.join("events.jsonl"));
        config
    }

    fn bind_python_cli_profile(root: &Path, config: &mut Config, plan: &mut UltraPlan) {
        let profile = "python-cli";
        let paths = crate::planner::profile::profile_setup_scaffold_paths(root, profile);
        crate::planner::profile::domain_profile(profile)
            .complete_scaffold(root, &paths)
            .unwrap();
        config.profile = profile.to_string();
        plan.profile = profile.to_string();
    }

    #[test]
    fn extracts_one_normalized_expected_failure_reproducer() {
        let binding = extract_reproducer(&reproducer_plan("cargo test -q")).unwrap();
        assert_eq!(binding.command, "cargo test -q");
        assert_eq!(binding.lineage, reproducer_lineage("cargo test -q"));
    }

    #[test]
    fn rejects_mutating_or_multiple_before_steps() {
        let mut plan = reproducer_plan("cargo test -q");
        plan.steps[0].kind = "implement".to_string();
        assert!(extract_reproducer(&plan).is_err());

        let mut plan = reproducer_plan("cargo test -q");
        plan.steps.push(plan.steps[0].clone());
        assert!(extract_reproducer(&plan).is_err());
    }

    #[test]
    fn create_prompts_stay_identical_and_explicit_fix_roles_stay_bounded() {
        assert_eq!(generation_rules("create"), "");
        let plan = UltraPlan::deterministic("goal", "generic", "default", "create");
        assert_eq!(
            phase_prompt(&plan, &plan.phases[0], "base".to_string(), false),
            "base"
        );
        assert!(!is_before_prompt("Intent: create"));

        let plan = crate::planner::intent::explicit_fix_plan("fix", "generic", "default");
        let cause = phase_prompt(&plan, &plan.phases[1], "base".to_string(), true);
        let regression = phase_prompt(&plan, &plan.phases[3], "base".to_string(), true);
        assert!(cause.contains("cause_isolation"));
        assert!(regression.contains("regression_verification"));
    }

    #[test]
    fn evidence_names_cannot_escape_workspace() {
        assert_eq!(safe_evidence_name("../cargo test"), "---cargo-test");
    }

    #[test]
    fn runtime_full_verdict_requires_failed_before_and_newer_passing_after() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = config(dir.path());
        let mut plan = fix_plan();
        bind_python_cli_profile(dir.path(), &mut config, &mut plan);
        let mut runtime = FixRuntime::for_plan(&plan, &config).unwrap();

        runtime
            .run_before_phase(
                &reproducer_plan("test -f fixed.marker"),
                &config,
                &plan,
                &plan.phases[0],
                0,
            )
            .unwrap();
        std::fs::write(dir.path().join("fixed.marker"), "fixed\n").unwrap();
        runtime.finish(&config, &plan).unwrap();

        let mut snapshot =
            eval_events::latest_completion_snapshot(config.eval_events_path.as_deref());
        crate::completion_metadata::apply_config_completion_metadata(&config, &mut snapshot);
        assert_eq!(snapshot.contract_origin, FIX_CONTRACT_ORIGIN);
        assert_eq!(snapshot.assurance_level, "static");
        assert_eq!(
            snapshot.assurance_reason,
            crate::planner::adjudication::PROFILE_NOT_ADMITTED_REASON
        );
        assert_eq!(snapshot.final_acceptance_status, "full_success");
        let events = std::fs::read_to_string(config.eval_events_path.unwrap()).unwrap();
        assert!(events.contains("\"stage\":\"before\""), "{events}");
        assert!(events.contains("\"stage\":\"after\""), "{events}");
        assert!(events.contains("\"verdict\":\"full\""), "{events}");
        assert_eq!(
            events
                .matches("\"event\":\"ultra_final_acceptance\"")
                .count(),
            1
        );
    }

    #[test]
    fn dropped_runtime_projects_an_honest_failed_terminal_after_before() {
        let dir = tempfile::tempdir().unwrap();
        let config = config(dir.path());
        let plan = fix_plan();
        let mut runtime = FixRuntime::for_plan(&plan, &config).unwrap();

        runtime
            .run_before_phase(
                &reproducer_plan("test -f fixed.marker"),
                &config,
                &plan,
                &plan.phases[0],
                0,
            )
            .unwrap();
        drop(runtime);

        let snapshot = eval_events::latest_completion_snapshot(config.eval_events_path.as_deref());
        assert_eq!(snapshot.contract_origin, FIX_CONTRACT_ORIGIN);
        assert_eq!(snapshot.assurance_level, "failed");
        assert_eq!(snapshot.assurance_reason, "after_not_executed");
        let events = std::fs::read_to_string(config.eval_events_path.unwrap()).unwrap();
        assert_eq!(
            events
                .matches("\"event\":\"ultra_final_acceptance\"")
                .count(),
            1
        );
    }

    #[test]
    fn draft_profile_caps_fix_assurance_without_changing_the_contract_verdict() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = config(dir.path());
        config.profile = "unregistered-profile".to_string();
        let adjudication = FixAdjudication {
            assurance: FixAssurance::Full,
            reason: String::new(),
            requirement_statuses: std::collections::BTreeMap::from([
                (BEFORE_FAILS_ID.to_string(), "passed".to_string()),
                (AFTER_PASSES_ID.to_string(), "passed".to_string()),
                (NO_REGRESSION_ID.to_string(), "passed".to_string()),
            ]),
        };
        let bundle = FixEvidenceBundle {
            run_id: "admission-test".to_string(),
            fix_written: true,
            bound_regression_ids: Vec::new(),
            bound_regression_lineages: std::collections::BTreeMap::new(),
            before: None,
            after: None,
            regressions: Vec::new(),
        };
        emit_final_adjudication(&config, &config.profile, &adjudication, &bundle);

        let snapshot = eval_events::latest_completion_snapshot(config.eval_events_path.as_deref());
        assert_eq!(snapshot.assurance_level, "static");
        assert_eq!(
            snapshot.assurance_reason,
            crate::planner::adjudication::PROFILE_NOT_ADMITTED_REASON
        );
        let events = std::fs::read_to_string(config.eval_events_path.unwrap()).unwrap();
        assert!(events.contains("\"verdict\":\"full\""), "{events}");
    }

    #[test]
    fn runtime_stops_before_repair_when_baseline_already_passes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("fixed.marker"), "already fixed\n").unwrap();
        let config = config(dir.path());
        let plan = fix_plan();
        let mut runtime = FixRuntime::for_plan(&plan, &config).unwrap();

        let error = runtime
            .run_before_phase(
                &reproducer_plan("test -f fixed.marker"),
                &config,
                &plan,
                &plan.phases[0],
                0,
            )
            .unwrap_err();

        assert!(error.to_string().contains("baseline_not_reproduced"));
        let events = std::fs::read_to_string(config.eval_events_path.unwrap()).unwrap();
        assert!(events.contains("baseline_not_reproduced"), "{events}");
        assert!(events.contains("\"verdict\":\"failed\""), "{events}");
        assert_eq!(
            events
                .matches("\"event\":\"ultra_final_acceptance\"")
                .count(),
            1
        );
    }

    #[test]
    fn data_completion_dispatch_preserves_fix_partial_assurance() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = config(dir.path());
        config.profile = "data".to_string();
        let mut snapshot = crate::eval_events::CompletionSnapshot::empty();
        snapshot.profile = "data".to_string();
        snapshot.effective_profile = "data".to_string();
        snapshot.contract_origin = FIX_CONTRACT_ORIGIN.to_string();
        snapshot.assurance_level = "partial".to_string();
        snapshot.assurance_reason = "regression_inconclusive:pipeline_probe".to_string();
        snapshot.runtime_acceptance_status = "partial".to_string();
        snapshot.final_acceptance_status = "partial".to_string();
        snapshot.completion_contract_verification_enabled = true;
        snapshot.external_contract_checked = true;

        crate::completion_metadata::apply_config_completion_metadata(&config, &mut snapshot);
        let mut projection = crate::eval_events::project_completion(true, &snapshot);
        crate::completion_metadata::apply_config_completion_projection(&config, &mut projection);

        assert_eq!(snapshot.assurance_level, "partial");
        assert_eq!(projection.assurance_level, "partial");
        assert_eq!(
            projection.assurance_reason,
            "regression_inconclusive:pipeline_probe"
        );
    }
}
