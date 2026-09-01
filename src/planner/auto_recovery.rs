//! Bounded, opt-in execution of typed Recovery Plan candidates.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde_json::json;

use crate::config::Config;
use crate::planner::recovery_validation::RecoveryPlanValidationError;
use crate::planner::ultra_plan::UltraPlan;
use crate::providers::ChatClient;
use crate::tui::InteractionUi;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecoveryCandidate {
    path: PathBuf,
    plan: UltraPlan,
    handoff: crate::planner::repair::RecoveryHandoff,
    verify_command_source: String,
}

#[derive(Debug)]
enum AttemptFailure {
    Interrupted,
    Recoverable(Box<RecoveryCandidate>),
    NonRecoverable,
}

#[derive(Debug)]
struct AttemptOutcome {
    result: anyhow::Result<String>,
    failure: Option<AttemptFailure>,
}

#[derive(Debug, Default)]
struct AttemptCapture {
    active: bool,
    candidate: Option<RecoveryCandidate>,
}

thread_local! {
    static ATTEMPT_CAPTURE: RefCell<AttemptCapture> = RefCell::new(AttemptCapture::default());
}

struct AttemptCaptureGuard;

impl AttemptCaptureGuard {
    fn begin() -> Self {
        ATTEMPT_CAPTURE.with(|capture| {
            *capture.borrow_mut() = AttemptCapture {
                active: true,
                candidate: None,
            };
        });
        Self
    }

    fn finish(self) -> Option<RecoveryCandidate> {
        let candidate = ATTEMPT_CAPTURE.with(|capture| {
            let mut capture = capture.borrow_mut();
            capture.active = false;
            capture.candidate.take()
        });
        std::mem::forget(self);
        candidate
    }
}

impl Drop for AttemptCaptureGuard {
    fn drop(&mut self) {
        ATTEMPT_CAPTURE.with(|capture| *capture.borrow_mut() = AttemptCapture::default());
    }
}

#[cfg(test)]
fn record_candidate(
    path: PathBuf,
    plan: UltraPlan,
    failure_kind: String,
    failed_step: Option<String>,
    verify_commands: Vec<String>,
) {
    let handoff = crate::planner::repair::RecoveryHandoff {
        profile: plan.profile.clone(),
        original_goal: plan.goal.clone(),
        failed_step,
        failure_kind,
        verify_commands,
        ..crate::planner::repair::RecoveryHandoff::default()
    };
    record_typed_candidate(RecoveryCandidate {
        path,
        plan,
        handoff,
        verify_command_source: "failure_handoff".to_string(),
    });
}

fn record_typed_candidate(candidate: RecoveryCandidate) {
    ATTEMPT_CAPTURE.with(|capture| {
        let mut capture = capture.borrow_mut();
        if capture.active {
            let replace = capture.candidate.as_ref().is_none_or(|current| {
                candidate.handoff.failed_step.is_some() || current.handoff.failed_step.is_none()
            });
            if replace {
                capture.candidate = Some(candidate);
            }
        }
    });
}

pub(crate) fn record_handoff_candidate(
    path: PathBuf,
    plan: UltraPlan,
    handoff: &crate::planner::repair::RecoveryHandoff,
) {
    record_typed_candidate(RecoveryCandidate {
        path,
        plan,
        handoff: handoff.clone(),
        verify_command_source: "failure_handoff".to_string(),
    });
}

fn capture_attempt(
    ui: &dyn InteractionUi,
    run: impl FnOnce() -> anyhow::Result<String>,
) -> AttemptOutcome {
    let capture = AttemptCaptureGuard::begin();
    let result = run();
    let candidate = capture.finish();
    let failure = if result.is_ok() {
        None
    } else if ui.interrupted() {
        Some(AttemptFailure::Interrupted)
    } else if let Some(candidate) = candidate {
        Some(AttemptFailure::Recoverable(Box::new(candidate)))
    } else {
        Some(AttemptFailure::NonRecoverable)
    };
    AttemptOutcome { result, failure }
}

enum InitialExecution<'a> {
    Generate(&'a str),
    File(&'a Path),
    Plan(&'a UltraPlan),
}

trait RecoveryDriver {
    type Prepared;

    fn preflight(&mut self, _candidate: &RecoveryCandidate) -> RecoveryPreflight {
        RecoveryPreflight::NotConfigured
    }
    fn prepare(&mut self, candidate: &RecoveryCandidate) -> Result<Self::Prepared, CandidateStop>;
    fn normalized(&self, prepared: &Self::Prepared) -> anyhow::Result<Vec<u8>>;
    fn start(
        &mut self,
        used: u8,
        candidate: &RecoveryCandidate,
        prepared: &Self::Prepared,
    ) -> Result<(), CandidateStop>;
    fn execute(&mut self, prepared: Self::Prepared) -> AttemptOutcome;
    fn finish(
        &mut self,
        _used: u8,
        _candidate: &RecoveryCandidate,
        outcome: AttemptOutcome,
    ) -> AttemptOutcome {
        outcome
    }
}

struct RunnerRecoveryDriver<'a> {
    planner: &'a mut dyn ChatClient,
    execution: &'a mut dyn ChatClient,
    config: &'a Config,
    ui: &'a dyn InteractionUi,
    transaction_snapshot: Option<crate::planner::recovery_snapshot::RecoveryBoundarySnapshot>,
    transaction_treatment: Option<PathBuf>,
    transaction_config: Option<Config>,
}

impl RecoveryDriver for RunnerRecoveryDriver<'_> {
    type Prepared = crate::runs::ResumePlan;

    fn preflight(&mut self, candidate: &RecoveryCandidate) -> RecoveryPreflight {
        recovery_preflight(self.config, candidate, 0)
    }

    fn prepare(&mut self, candidate: &RecoveryCandidate) -> Result<Self::Prepared, CandidateStop> {
        prepare_candidate(self.config, candidate)
    }

    fn normalized(&self, prepared: &Self::Prepared) -> anyhow::Result<Vec<u8>> {
        normalized_plan(&prepared.plan)
    }

    fn start(
        &mut self,
        used: u8,
        candidate: &RecoveryCandidate,
        prepared: &Self::Prepared,
    ) -> Result<(), CandidateStop> {
        let snapshot = crate::planner::recovery_snapshot::capture_for_transaction(
            &self.config.workspace_root,
            used,
        );
        emit_boundary_snapshot(self.config, used, &snapshot);
        let snapshot = snapshot.map_err(|_| CandidateStop::BoundaryCaptureFailed)?;
        let treatment = crate::planner::recovery_snapshot::prepare_treatment(
            &self.config.workspace_root,
            &snapshot,
            used,
        )
        .map_err(|_| CandidateStop::TreatmentPrepareFailed)?;
        let treatment_config =
            crate::planner::recovery_contract_binding::bind_config(self.config, &treatment)
                .map_err(|_| CandidateStop::TreatmentContractBindFailed)?;
        emit_with_candidate(
            self.config,
            "recovery_plan_auto_run_start",
            used,
            candidate,
            Some(&snapshot),
            Some(&treatment),
        );
        self.transaction_snapshot = Some(snapshot);
        self.transaction_treatment = Some(treatment);
        self.transaction_config = Some(treatment_config);
        crate::runs::emit_resume_start(self.config, prepared);
        Ok(())
    }

    fn execute(&mut self, prepared: Self::Prepared) -> AttemptOutcome {
        let Some(config) = self.transaction_config.as_ref() else {
            return rejected_without_restore(
                AttemptOutcome {
                    result: Err(anyhow::anyhow!("Recovery treatment config unavailable")),
                    failure: Some(AttemptFailure::NonRecoverable),
                },
                "Recovery treatment config unavailable",
            );
        };
        capture_attempt(self.ui, || {
            super::run_ultra_plan_with_ui(
                self.planner,
                self.execution,
                &prepared.plan,
                config,
                self.ui,
            )
        })
    }

    fn finish(
        &mut self,
        used: u8,
        candidate: &RecoveryCandidate,
        outcome: AttemptOutcome,
    ) -> AttemptOutcome {
        let Some(snapshot) = self.transaction_snapshot.take() else {
            return rejected_without_restore(
                outcome,
                "Recovery transaction snapshot was not available",
            );
        };
        let Some(treatment) = self.transaction_treatment.take() else {
            return retain_control(
                self.config,
                used,
                snapshot,
                outcome,
                "Recovery treatment workspace was not available",
                false,
            );
        };
        let Some(treatment_config) = self.transaction_config.take() else {
            return retain_control(
                self.config,
                used,
                snapshot,
                outcome,
                "Recovery treatment config was not available",
                false,
            );
        };
        emit_treatment_delta(self.config, used, &snapshot, &treatment);
        if outcome.result.is_err() {
            return retain_control(
                self.config,
                used,
                snapshot,
                outcome,
                "recovery_execution_failed",
                false,
            );
        }
        match recovery_preflight(&treatment_config, candidate, used.saturating_add(128)) {
            RecoveryPreflight::CurrentSuccess { reason } => {
                emit_preflight(self.config, candidate, "post_recovery", "pass", &reason);
                promote_treatment(
                    self.config,
                    used,
                    snapshot,
                    &treatment,
                    outcome,
                    "registered_final_success_passed",
                )
            }
            RecoveryPreflight::NotConfigured => {
                emit_preflight(
                    self.config,
                    candidate,
                    "post_recovery",
                    "not_configured",
                    "no registered command",
                );
                retain_control(
                    self.config,
                    used,
                    snapshot,
                    outcome,
                    "no_registered_post_recovery_observation",
                    false,
                )
            }
            RecoveryPreflight::Failed { reason } => {
                emit_preflight(self.config, candidate, "post_recovery", "fail", &reason);
                retain_control(self.config, used, snapshot, outcome, &reason, true)
            }
            RecoveryPreflight::VerificationInconsistency { reason } => {
                emit_preflight(
                    self.config,
                    candidate,
                    "post_recovery",
                    "verification_inconsistency",
                    &reason,
                );
                retain_control(self.config, used, snapshot, outcome, &reason, false)
            }
            RecoveryPreflight::Unavailable { reason } => {
                emit_preflight(
                    self.config,
                    candidate,
                    "post_recovery",
                    "unavailable",
                    &reason,
                );
                retain_control(self.config, used, snapshot, outcome, &reason, false)
            }
        }
    }
}

fn emit_treatment_delta(
    config: &Config,
    used: u8,
    snapshot: &crate::planner::recovery_snapshot::RecoveryBoundarySnapshot,
    treatment: &Path,
) {
    match crate::planner::recovery_snapshot::treatment_delta(
        &config.workspace_root,
        snapshot,
        treatment,
    ) {
        Ok(delta) => crate::eval_events::emit(
            config.eval_events_path.as_deref(),
            json!({
                "event": "recovery_treatment_delta",
                "recovery_plan_auto_run_current": used,
                "status": "observed",
                "attempted_product_delta": delta.product,
                "treatment_runtime_evidence_delta": delta.runtime_evidence,
            }),
        ),
        Err(error) => crate::eval_events::emit(
            config.eval_events_path.as_deref(),
            json!({
                "event": "recovery_treatment_delta",
                "recovery_plan_auto_run_current": used,
                "status": "unavailable",
                "reason": crate::eval_events::body_snippet(&error.to_string()),
            }),
        ),
    }
}

pub fn generate_and_run_with_ui(
    planner: &mut dyn ChatClient,
    execution: &mut dyn ChatClient,
    goal: &str,
    config: &Config,
    ui: &dyn InteractionUi,
) -> anyhow::Result<String> {
    run_with_ui(
        InitialExecution::Generate(goal),
        planner,
        execution,
        config,
        ui,
    )
}

pub fn run_file_with_ui(
    planner: &mut dyn ChatClient,
    execution: &mut dyn ChatClient,
    path: &Path,
    config: &Config,
    ui: &dyn InteractionUi,
) -> anyhow::Result<String> {
    run_with_ui(InitialExecution::File(path), planner, execution, config, ui)
}

pub fn run_plan_with_ui(
    planner: &mut dyn ChatClient,
    execution: &mut dyn ChatClient,
    plan: &UltraPlan,
    config: &Config,
    ui: &dyn InteractionUi,
) -> anyhow::Result<String> {
    run_with_ui(InitialExecution::Plan(plan), planner, execution, config, ui)
}

fn run_with_ui(
    initial: InitialExecution<'_>,
    planner: &mut dyn ChatClient,
    execution: &mut dyn ChatClient,
    config: &Config,
    ui: &dyn InteractionUi,
) -> anyhow::Result<String> {
    if config.recovery_plan_auto_runs == 0 {
        return execute_initial(initial, planner, execution, config, ui);
    }

    emit(
        config,
        "recovery_plan_auto_run_configured",
        0,
        "initial_run",
    );
    let outcome = capture_attempt(ui, || {
        execute_initial(initial, planner, execution, config, ui)
    });
    drive(
        config,
        outcome,
        &mut RunnerRecoveryDriver {
            planner,
            execution,
            config,
            ui,
            transaction_snapshot: None,
            transaction_treatment: None,
            transaction_config: None,
        },
    )
}

fn drive(
    config: &Config,
    mut outcome: AttemptOutcome,
    driver: &mut impl RecoveryDriver,
) -> anyhow::Result<String> {
    if outcome.result.is_ok() {
        emit(
            config,
            "recovery_plan_auto_run_complete",
            0,
            "initial_success",
        );
        return outcome.result;
    }
    let mut controller = AutoRecoveryController::new(config.recovery_plan_auto_runs);
    loop {
        let error = outcome.result.expect_err("failed result checked above");
        let candidate = match outcome
            .failure
            .expect("failed attempt always has a typed failure")
        {
            AttemptFailure::Interrupted => {
                emit(
                    config,
                    "recovery_plan_auto_run_stopped",
                    controller.used,
                    "interrupted",
                );
                return Err(error);
            }
            AttemptFailure::NonRecoverable => {
                emit(
                    config,
                    "recovery_plan_auto_run_stopped",
                    controller.used,
                    "not_recoverable",
                );
                return Err(error);
            }
            AttemptFailure::Recoverable(candidate) => *candidate,
        };
        let preflight_failure_reason = match driver.preflight(&candidate) {
            RecoveryPreflight::NotConfigured => {
                emit_preflight(
                    config,
                    &candidate,
                    "pre_recovery",
                    "not_configured",
                    "no registered command",
                );
                None
            }
            RecoveryPreflight::Failed { reason } => {
                emit_preflight(config, &candidate, "pre_recovery", "fail", &reason);
                Some(reason)
            }
            RecoveryPreflight::CurrentSuccess { reason } => {
                emit_preflight(config, &candidate, "pre_recovery", "pass", &reason);
                emit(
                    config,
                    "recovery_suppressed_current_success",
                    controller.used,
                    "current_success_protected",
                );
                emit(
                    config,
                    "recovery_plan_auto_run_stopped",
                    controller.used,
                    "current_success_protected",
                );
                return Err(error.context(
                    "automatic Recovery Plan suppressed: current final-success observations pass",
                ));
            }
            RecoveryPreflight::VerificationInconsistency { reason } => {
                emit_preflight(
                    config,
                    &candidate,
                    "pre_recovery",
                    "verification_inconsistency",
                    &reason,
                );
                emit(
                    config,
                    "recovery_suppressed_verification_inconsistency",
                    controller.used,
                    "verification_inconsistency",
                );
                emit(
                    config,
                    "recovery_plan_auto_run_stopped",
                    controller.used,
                    "verification_inconsistency",
                );
                return Err(error.context(format!(
                    "automatic Recovery Plan suppressed: registered observations pass but completion evidence is inconsistent: {reason}"
                )));
            }
            RecoveryPreflight::Unavailable { reason } => {
                emit_preflight(config, &candidate, "pre_recovery", "unavailable", &reason);
                emit(
                    config,
                    "recovery_plan_auto_run_stopped",
                    controller.used,
                    "preflight_unavailable",
                );
                return Err(error.context(format!(
                    "automatic Recovery Plan stopped: preflight unavailable: {reason}"
                )));
            }
        };
        let Some(used) = controller.next_run() else {
            emit(
                config,
                "recovery_plan_auto_run_stopped",
                controller.used,
                "limit_reached",
            );
            return Err(error);
        };
        let candidate = if let Some(reason) = preflight_failure_reason.as_deref() {
            match bind_candidate_verify_commands(config, candidate, reason) {
                Ok(candidate) => candidate,
                Err(reason) => {
                    emit(
                        config,
                        "recovery_plan_auto_run_stopped",
                        used - 1,
                        reason.code(),
                    );
                    return Err(error.context(format!(
                        "automatic Recovery Plan stopped: {}",
                        reason.code()
                    )));
                }
            }
        } else {
            candidate
        };
        let prepared = match driver.prepare(&candidate) {
            Ok(prepared) => prepared,
            Err(reason) => {
                emit(
                    config,
                    "recovery_plan_auto_run_stopped",
                    used - 1,
                    reason.code(),
                );
                return Err(error.context(format!(
                    "automatic Recovery Plan stopped: {}",
                    reason.code()
                )));
            }
        };
        let normalized = driver.normalized(&prepared)?;
        if !controller.observe_plan(normalized) {
            emit(
                config,
                "recovery_plan_auto_run_stopped",
                used - 1,
                "cycle_detected",
            );
            return Err(error.context("automatic Recovery Plan stopped: cycle detected"));
        }

        if let Err(reason) = driver.start(used, &candidate, &prepared) {
            emit(
                config,
                "recovery_plan_auto_run_stopped",
                used - 1,
                reason.code(),
            );
            return Err(error.context(format!(
                "automatic Recovery Plan stopped: {}",
                reason.code()
            )));
        }
        outcome = driver.execute(prepared);
        outcome = driver.finish(used, &candidate, outcome);
        if outcome.result.is_ok() {
            emit(
                config,
                "recovery_plan_auto_run_complete",
                used,
                "recovery_succeeded",
            );
            return outcome.result;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RecoveryPreflight {
    NotConfigured,
    CurrentSuccess { reason: String },
    Failed { reason: String },
    VerificationInconsistency { reason: String },
    Unavailable { reason: String },
}

fn recovery_preflight(
    config: &Config,
    candidate: &RecoveryCandidate,
    checkpoint_attempt: u8,
) -> RecoveryPreflight {
    let contract =
        match crate::minimal_loop::completion::CompletionContract::load_for_config(config) {
            Ok(Some(contract)) => contract,
            Ok(None) => return RecoveryPreflight::NotConfigured,
            Err(error) => {
                return RecoveryPreflight::Unavailable {
                    reason: format!("completion_contract_invalid:{error}"),
                };
            }
        };
    let (capability_commands, unsupported_capabilities) =
        recovery_preflight_capability_commands(&contract);
    if !unsupported_capabilities.is_empty() {
        return RecoveryPreflight::Unavailable {
            reason: format!(
                "required_capability_has_no_product_visible_read_only_observation:{}",
                unsupported_capabilities.join(",")
            ),
        };
    }
    if contract.verify_commands.is_empty() {
        return RecoveryPreflight::NotConfigured;
    }
    let checkpoint = match crate::planner::recovery_snapshot::capture_for_transaction(
        &config.workspace_root,
        checkpoint_attempt,
    ) {
        Ok(checkpoint) => checkpoint,
        Err(error) => {
            return RecoveryPreflight::Unavailable {
                reason: format!("preflight_checkpoint_unavailable:{error}"),
            };
        }
    };
    let observation = match crate::planner::recovery_snapshot::prepare_preflight_observation(
        &config.workspace_root,
        &checkpoint,
        checkpoint_attempt,
    ) {
        Ok(observation) => observation,
        Err(error) => {
            return RecoveryPreflight::Unavailable {
                reason: format!("preflight_observation_prepare_failed:{error}"),
            };
        }
    };
    let mut observation_config =
        match crate::planner::recovery_contract_binding::bind_config(config, &observation) {
            Ok(config) => config,
            Err(error) => {
                return RecoveryPreflight::Unavailable {
                    reason: format!("preflight_observation_contract_bind_failed:{error}"),
                };
            }
        };
    observation_config.eval_events_path = None;
    let effect_policy =
        crate::planner::recovery_observation_policy::RecoveryObservationPolicy::for_contract(
            &contract,
        );
    crate::eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "recovery_observation_effect_policy_bound",
            "source": "product_visible_completion_contract",
            "allowed_generated_paths": &effect_policy.allowed_generated_paths,
            "protected_change_disposition": "reject_and_retain_control",
            "registered_data_input_fixture": crate::planner::recovery_observation_policy::registered_data_input_fixture(&contract),
            "external_oracle_used": false,
        }),
    );
    let observation_before =
        match crate::planner::recovery_snapshot::current_preflight_source_sha256(
            &observation,
            &effect_policy.allowed_generated_paths,
        ) {
            Ok(hash) => hash,
            Err(error) => {
                return RecoveryPreflight::Unavailable {
                    reason: format!("preflight_source_observation_failed:{error}"),
                };
            }
        };
    let mut verify_commands = contract.verify_commands.clone();
    for command in capability_commands {
        push_unique(&mut verify_commands, command);
    }
    let step = crate::planner::step_plan::PlanStep {
        id: "recovery-boundary-final-success".to_string(),
        kind: "verify".to_string(),
        expected_result: "pass".to_string(),
        instruction: "Read-only Recovery boundary final-success observation.".to_string(),
        expected_paths: contract.required_paths.clone(),
        verify: verify_commands,
    };
    let (report, _) = crate::planner::verify::verify_step_with_context(
        &observation,
        &step,
        contract.profile.as_deref(),
        contract
            .goal
            .as_deref()
            .or(Some(candidate.plan.goal.as_str())),
        crate::minimal_loop::dependency_setup::NodeDependencySetupAuthority::None,
        config.offline,
        None,
    );
    let completion_acceptance = if report.is_pass() {
        Some(
            crate::planner::runner::recovery_acceptance::runtime_acceptance_report(
                &candidate.plan,
                &observation_config,
            ),
        )
    } else {
        None
    };
    let observed_sha256 = crate::planner::recovery_snapshot::current_preflight_source_sha256(
        &observation,
        &effect_policy.allowed_generated_paths,
    );
    let source_mutated = observed_sha256
        .as_ref()
        .is_ok_and(|value| value != &observation_before);
    if source_mutated {
        return RecoveryPreflight::Unavailable {
            reason: "preflight_source_mutation_rejected_and_restored".to_string(),
        };
    }
    if let Err(error) = observed_sha256 {
        return RecoveryPreflight::Unavailable {
            reason: format!("preflight_source_observation_failed:{error}"),
        };
    }
    match crate::planner::recovery_snapshot::current_source_sha256(&config.workspace_root) {
        Ok(hash) if hash == checkpoint.snapshot_sha256 => {}
        Ok(_) => {
            let reason = match crate::planner::recovery_snapshot::restore_transaction(
                &config.workspace_root,
                &checkpoint,
            ) {
                Ok(_) => "preflight_control_source_mutation_rejected_and_restored".to_string(),
                Err(error) => format!("preflight_control_source_restore_failed:{error}"),
            };
            return RecoveryPreflight::Unavailable { reason };
        }
        Err(error) => {
            return RecoveryPreflight::Unavailable {
                reason: format!("preflight_control_source_observation_failed:{error}"),
            };
        }
    }
    if report.is_pass() {
        return match completion_acceptance.expect("pass report records acceptance") {
            Ok(acceptance)
                if completion_acceptance_passes_after_registered_observation(
                    &acceptance,
                    &contract,
                ) =>
            {
                RecoveryPreflight::CurrentSuccess {
                    reason: format!(
                        "registered_final_success_and_completion_contract_passed:{} commands",
                        candidate
                            .handoff
                            .verify_commands
                            .len()
                            .max(contract.verify_commands.len())
                    ),
                }
            }
            Ok(acceptance) => RecoveryPreflight::VerificationInconsistency {
                reason: format!(
                    "registered_observations_passed_but_completion_contract_acceptance_failed:{}",
                    acceptance.primary_reason
                ),
            },
            Err(error) => RecoveryPreflight::Unavailable {
                reason: format!("completion_contract_acceptance_unavailable:{error}"),
            },
        };
    }
    if !report.dependency_missing.is_empty() || !report.verifier_command_false_negatives.is_empty()
    {
        RecoveryPreflight::Unavailable {
            reason: report.primary_reason(),
        }
    } else {
        RecoveryPreflight::Failed {
            reason: report.primary_reason(),
        }
    }
}

fn recovery_preflight_capability_commands(
    contract: &crate::minimal_loop::completion::CompletionContract,
) -> (Vec<String>, Vec<String>) {
    let mut commands = Vec::new();
    let mut unsupported = Vec::new();
    let input_output_bound = has_registered_fix_reproducer(contract);
    for capability in &contract.required_capabilities {
        if capability == "input_output_contract" && input_output_bound {
            continue;
        }
        if contract.profile.as_deref() == Some("data")
            && crate::planner::profiles::data::manifest::check_ids()
                .iter()
                .any(|id| id == capability)
        {
            if matches!(
                capability.as_str(),
                "pipeline_probe" | "data_rerun_consistency"
            ) {
                let Some(input) =
                    crate::planner::recovery_observation_policy::registered_data_input_fixture(
                        contract,
                    )
                else {
                    unsupported.push(format!("{capability}:registered_input_not_bound"));
                    continue;
                };
                commands.push(
                    crate::planner::profiles::data::step_policy::catalog_check_command_with_input(
                        capability, &input,
                    ),
                );
            } else {
                commands.push(
                    crate::planner::profiles::data::step_policy::catalog_check_command(capability),
                );
            }
            continue;
        }
        unsupported.push(capability.clone());
    }
    (commands, unsupported)
}

fn bind_candidate_verify_commands(
    config: &Config,
    mut candidate: RecoveryCandidate,
    preflight_reason: &str,
) -> Result<RecoveryCandidate, CandidateStop> {
    let contract = crate::minimal_loop::completion::CompletionContract::load_for_config(config)
        .map_err(|_| CandidateStop::ContractCommandBindFailed)?
        .ok_or(CandidateStop::ContractCommandBindFailed)?;
    if contract.verify_commands.is_empty() {
        return Err(CandidateStop::ContractCommandBindFailed);
    }

    let contract_goal = contract
        .goal
        .as_deref()
        .map(str::trim)
        .filter(|goal| !goal.is_empty())
        .ok_or(CandidateStop::ContractHandoffBindFailed)?;

    let original_commands = candidate.handoff.verify_commands.clone();
    let original_commands_all_registered = original_commands
        .iter()
        .all(|command| contract.verify_commands.contains(command));
    let commands_rewritten = original_commands != contract.verify_commands;
    candidate.handoff.verify_commands = contract.verify_commands.clone();
    candidate.verify_command_source = "completion_contract".to_string();
    let goal_rewritten = candidate.handoff.original_goal != contract_goal;
    candidate.handoff.original_goal = contract_goal.to_string();
    let original_target_count = candidate.handoff.repair_targets.len();
    for path in candidate
        .handoff
        .missing_paths
        .iter()
        .chain(candidate.handoff.changed_paths.iter())
        .chain(contract.required_paths.iter())
    {
        if !is_protected_repair_path(path, &contract.protected_paths) {
            push_unique(&mut candidate.handoff.repair_targets, path.clone());
        }
    }
    if candidate.handoff.repair_targets.is_empty() {
        return Err(CandidateStop::ContractHandoffBindFailed);
    }
    let targets_rewritten = candidate.handoff.repair_targets.len() != original_target_count;
    if commands_rewritten {
        candidate.handoff.failure_evidence = vec![format!(
            "Registered final-success observation failed before automatic Recovery: {preflight_reason}"
        )];
    }
    let completion_requirements_added =
        bind_candidate_completion_requirements(config, &contract, &mut candidate)?;
    let plan_rewritten =
        commands_rewritten || completion_requirements_added || goal_rewritten || targets_rewritten;

    if plan_rewritten {
        if !commands_rewritten {
            push_unique(
                &mut candidate.handoff.failure_evidence,
                format!(
                    "Registered final-success observation failed before automatic Recovery: {preflight_reason}"
                ),
            );
        }
        let plan = crate::planner::repair::build_recovery_ultra_plan(&candidate.handoff);
        let scope = contract_bound_scope(candidate.handoff.failed_step.as_deref());
        let path = crate::planner::repair::save_recovery_ultra_plan(
            &config.workspace_root,
            &scope,
            &candidate.handoff,
        )
        .map_err(|_| CandidateStop::ContractCommandBindFailed)?;
        candidate.path = path;
        candidate.plan = plan;
    }

    crate::eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "recovery_candidate_verify_commands_bound",
            "source": "product_visible_completion_contract",
            "external_oracle_used": false,
            "original_verify_command_count": original_commands.len(),
            "registered_verify_command_count": candidate.handoff.verify_commands.len(),
            "original_commands_all_registered": original_commands_all_registered,
            "recovery_plan_rewritten": plan_rewritten,
            "recovery_verify_command_source": candidate.verify_command_source,
        }),
    );
    crate::eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "recovery_handoff_fidelity_bound",
            "source": "product_visible_completion_contract",
            "external_oracle_used": false,
            "goal_source": "completion_contract",
            "contract_bound": true,
            "verify_command_count": candidate.handoff.verify_commands.len(),
            "repair_target_count": candidate.handoff.repair_targets.len(),
            "protected_path_count": contract.protected_paths.len(),
            "fidelity_ok": true,
        }),
    );
    Ok(candidate)
}

fn is_protected_repair_path(path: &str, protected_paths: &[String]) -> bool {
    let path = path.trim_matches('/');
    protected_paths.iter().any(|protected| {
        let protected = protected.trim_matches('/');
        path == protected || path.starts_with(&format!("{protected}/"))
    })
}

fn bind_candidate_completion_requirements(
    config: &Config,
    contract: &crate::minimal_loop::completion::CompletionContract,
    candidate: &mut RecoveryCandidate,
) -> Result<bool, CandidateStop> {
    let acceptance = crate::planner::runner::recovery_acceptance::runtime_acceptance_report(
        &candidate.plan,
        config,
    )
    .map_err(|_| CandidateStop::ContractAcceptanceBindFailed)?;
    let mut added = false;
    for target in &acceptance.obligation_repair_targets {
        added |= push_unique(
            &mut candidate.handoff.missing_paths,
            target.target_path.clone(),
        );
        added |= push_unique(
            &mut candidate.handoff.repair_targets,
            format!(
                "completion obligation {} at {}",
                target.obligation, target.target_path
            ),
        );
    }
    for evidence in &acceptance.missing_evidence {
        if evidence == "bound_verify_command" && has_registered_fix_reproducer(contract) {
            continue;
        }
        added |= push_unique(
            &mut candidate.handoff.failure_evidence,
            format!("missing completion evidence: {evidence}"),
        );
    }
    for obligation in &acceptance.missing_obligations {
        added |= push_unique(
            &mut candidate.handoff.failure_evidence,
            format!("missing completion obligation: {obligation}"),
        );
    }
    if !acceptance.passed && !acceptance.primary_reason.is_empty() {
        added |= push_unique(
            &mut candidate.handoff.failure_evidence,
            format!(
                "completion contract acceptance failed: {}",
                acceptance.primary_reason
            ),
        );
    }
    crate::eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "recovery_candidate_completion_requirements_bound",
            "source": "product_visible_completion_contract",
            "external_oracle_used": false,
            "completion_acceptance_passed": acceptance.passed,
            "missing_evidence_count": acceptance.missing_evidence.len(),
            "missing_obligation_count": acceptance.missing_obligations.len(),
            "obligation_repair_target_count": acceptance.obligation_repair_targets.len(),
            "recovery_plan_rewrite_required": added,
        }),
    );
    Ok(added)
}

fn completion_acceptance_passes_after_registered_observation(
    acceptance: &crate::minimal_loop::evidence::RuntimeAcceptanceReport,
    contract: &crate::minimal_loop::completion::CompletionContract,
) -> bool {
    acceptance.passed
        || (has_registered_fix_reproducer(contract)
            && acceptance.missing_capabilities.is_empty()
            && acceptance.missing_obligations.is_empty()
            && acceptance.missing_evidence.len() == 1
            && acceptance.missing_evidence[0] == "bound_verify_command"
            && acceptance.weak_evidence.is_empty()
            && !acceptance.inconclusive)
}

fn has_registered_fix_reproducer(
    contract: &crate::minimal_loop::completion::CompletionContract,
) -> bool {
    contract
        .fix_reproducer_command
        .as_ref()
        .is_some_and(|command| {
            contract
                .verify_commands
                .iter()
                .any(|verify| verify == command)
        })
}

fn push_unique(values: &mut Vec<String>, value: String) -> bool {
    if values.contains(&value) {
        false
    } else {
        values.push(value);
        true
    }
}

fn contract_bound_scope(failed_step: Option<&str>) -> String {
    let token = failed_step
        .unwrap_or("phase")
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    let token = token.trim_matches('-');
    format!(
        "contract-bound-{}",
        if token.is_empty() { "phase" } else { token }
    )
}

fn execute_initial(
    initial: InitialExecution<'_>,
    planner: &mut dyn ChatClient,
    execution: &mut dyn ChatClient,
    config: &Config,
    ui: &dyn InteractionUi,
) -> anyhow::Result<String> {
    match initial {
        InitialExecution::Generate(goal) => {
            super::generate_and_run_ultra_plan_with_ui(planner, execution, goal, config, ui)
        }
        InitialExecution::File(path) => {
            super::run_ultra_plan_file_with_ui(planner, execution, path, config, ui)
        }
        InitialExecution::Plan(plan) => {
            super::run_ultra_plan_with_ui(planner, execution, plan, config, ui)
        }
    }
}

#[derive(Debug)]
struct AutoRecoveryController {
    limit: u8,
    used: u8,
    seen_plans: BTreeSet<Vec<u8>>,
}

impl AutoRecoveryController {
    fn new(limit: u8) -> Self {
        Self {
            limit,
            used: 0,
            seen_plans: BTreeSet::new(),
        }
    }

    fn next_run(&mut self) -> Option<u8> {
        if self.used == self.limit {
            return None;
        }
        self.used += 1;
        Some(self.used)
    }

    fn observe_plan(&mut self, normalized: Vec<u8>) -> bool {
        self.seen_plans.insert(normalized)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CandidateStop {
    PathEscape,
    RecoveryYamlMissing,
    RecoveryYamlInvalid,
    RecoveryNeedsReview,
    BoundaryCaptureFailed,
    TreatmentPrepareFailed,
    TreatmentContractBindFailed,
    ContractCommandBindFailed,
    ContractHandoffBindFailed,
    ContractAcceptanceBindFailed,
    ResumeSafetyRejected,
    WorkspaceDrift,
}

impl CandidateStop {
    const fn code(self) -> &'static str {
        match self {
            Self::PathEscape => "path_escape",
            Self::RecoveryYamlMissing => "recovery_yaml_missing",
            Self::RecoveryYamlInvalid => "recovery_yaml_invalid",
            Self::RecoveryNeedsReview => "recovery_needs_review",
            Self::BoundaryCaptureFailed => "boundary_capture_failed",
            Self::TreatmentPrepareFailed => "treatment_prepare_failed",
            Self::TreatmentContractBindFailed => "treatment_contract_bind_failed",
            Self::ContractCommandBindFailed => "contract_command_bind_failed",
            Self::ContractHandoffBindFailed => "contract_handoff_bind_failed",
            Self::ContractAcceptanceBindFailed => "contract_acceptance_bind_failed",
            Self::ResumeSafetyRejected => "resume_safety_rejected",
            Self::WorkspaceDrift => "workspace_drift",
        }
    }
}

fn prepare_candidate(
    config: &Config,
    candidate: &RecoveryCandidate,
) -> Result<crate::runs::ResumePlan, CandidateStop> {
    let root = config
        .workspace_root
        .canonicalize()
        .map_err(|_| CandidateStop::ResumeSafetyRejected)?;
    let path = candidate.path.canonicalize().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            CandidateStop::RecoveryYamlMissing
        } else {
            CandidateStop::RecoveryYamlInvalid
        }
    })?;
    if !path.starts_with(&root) {
        return Err(CandidateStop::PathEscape);
    }
    let parsed = super::recovery_validation::validate(&path).map_err(|error| match error {
        RecoveryPlanValidationError::Missing => CandidateStop::RecoveryYamlMissing,
        RecoveryPlanValidationError::NeedsReview => CandidateStop::RecoveryNeedsReview,
        RecoveryPlanValidationError::Unreadable
        | RecoveryPlanValidationError::Parse
        | RecoveryPlanValidationError::Roundtrip => CandidateStop::RecoveryYamlInvalid,
    })?;
    if parsed != candidate.plan {
        return Err(CandidateStop::RecoveryYamlInvalid);
    }
    let resume = crate::runs::prepare_resume(&root, path.to_string_lossy().as_ref())
        .map_err(|_| CandidateStop::ResumeSafetyRejected)?;
    if resume.workspace_drift_error().is_some() {
        return Err(CandidateStop::WorkspaceDrift);
    }
    Ok(resume)
}

fn normalized_plan(plan: &UltraPlan) -> anyhow::Result<Vec<u8>> {
    serde_json::to_vec(plan).context("normalize Recovery Plan content")
}

fn emit(config: &Config, event: &str, used: u8, stop_reason: &str) {
    crate::eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": event,
            "recovery_plan_auto_runs": config.recovery_plan_auto_runs,
            "recovery_plan_auto_runs_used": used,
            "recovery_plan_auto_run_current": used,
            "recovery_plan_auto_run_stop_reason": stop_reason,
        }),
    );
}

fn emit_preflight(
    config: &Config,
    candidate: &RecoveryCandidate,
    observation_phase: &str,
    status: &str,
    reason: &str,
) {
    crate::eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "recovery_preflight_observation",
            "status": status,
            "observation_phase": observation_phase,
            "reason": crate::eval_events::body_snippet(reason),
            "source": "product_visible_completion_contract",
            "read_only": true,
            "observation_isolated": true,
            "external_oracle_used": false,
            "recovery_handoff_kind": candidate.handoff.failure_kind,
            "recovery_failed_step": candidate.handoff.failed_step,
            "verify_command_count": candidate.handoff.verify_commands.len(),
        }),
    );
}

fn rejected_without_restore(mut outcome: AttemptOutcome, reason: &str) -> AttemptOutcome {
    outcome.result = Err(anyhow::anyhow!(reason.to_string()));
    outcome.failure = Some(AttemptFailure::NonRecoverable);
    outcome
}

fn retain_control(
    config: &Config,
    used: u8,
    snapshot: crate::planner::recovery_snapshot::RecoveryBoundarySnapshot,
    mut outcome: AttemptOutcome,
    reason: &str,
    regression: bool,
) -> AttemptOutcome {
    let restore =
        crate::planner::recovery_snapshot::retain_control(&config.workspace_root, &snapshot);
    match restore {
        Ok(report) => {
            if regression {
                emit_transaction_event(
                    config,
                    "recovery_treatment_rejected_regression",
                    used,
                    reason,
                    Some(&report),
                );
            }
            emit_transaction_event(
                config,
                "recovery_control_retained",
                used,
                reason,
                Some(&report),
            );
            emit_promotion_decision(config, used, "rejected", reason);
            if outcome.result.is_ok() {
                outcome.result = Err(anyhow::anyhow!(
                    "automatic Recovery treatment rejected: {reason}"
                ));
            }
        }
        Err(error) => {
            emit_transaction_event(
                config,
                "recovery_control_restore_failed",
                used,
                &error.to_string(),
                None,
            );
            outcome.result = Err(anyhow::anyhow!(
                "automatic Recovery treatment failed and control restore failed: {error}"
            ));
        }
    }
    outcome.failure = Some(AttemptFailure::NonRecoverable);
    outcome
}

fn promote_treatment(
    config: &Config,
    used: u8,
    snapshot: crate::planner::recovery_snapshot::RecoveryBoundarySnapshot,
    treatment: &Path,
    mut outcome: AttemptOutcome,
    reason: &str,
) -> AttemptOutcome {
    match crate::planner::recovery_snapshot::promote_treatment(&config.workspace_root, treatment) {
        Ok(report) => {
            emit_transaction_event(
                config,
                "recovery_treatment_promoted",
                used,
                reason,
                Some(&report),
            );
            emit_promotion_decision(config, used, "promoted", reason);
            outcome
        }
        Err(error) => {
            outcome.result = Err(anyhow::anyhow!(
                "automatic Recovery treatment promotion failed: {error}"
            ));
            retain_control(
                config,
                used,
                snapshot,
                outcome,
                &format!("treatment_promotion_failed:{error}"),
                false,
            )
        }
    }
}

fn emit_transaction_event(
    config: &Config,
    event: &str,
    used: u8,
    reason: &str,
    report: Option<&crate::planner::recovery_snapshot::RecoveryRestoreReport>,
) {
    crate::eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": event,
            "recovery_plan_auto_run_current": used,
            "reason": crate::eval_events::body_snippet(reason),
            "restored_file_count": report.map(|value| value.restored_file_count),
            "removed_file_count": report.map(|value| value.removed_file_count),
            "control_snapshot_sha256": report.map(|value| value.snapshot_sha256.as_str()),
        }),
    );
}

fn emit_promotion_decision(config: &Config, used: u8, decision: &str, reason: &str) {
    crate::eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "recovery_promotion_decision",
            "recovery_plan_auto_run_current": used,
            "decision": decision,
            "reason": crate::eval_events::body_snippet(reason),
            "external_oracle_used": false,
        }),
    );
}

fn emit_boundary_snapshot(
    config: &Config,
    used: u8,
    snapshot: &anyhow::Result<crate::planner::recovery_snapshot::RecoveryBoundarySnapshot>,
) {
    let (status, path, file_count, total_bytes, snapshot_sha256, reason) = match snapshot {
        Ok(snapshot) => (
            "captured",
            snapshot.workspace_relative_path.as_str(),
            Some(snapshot.file_count),
            Some(snapshot.total_bytes),
            Some(snapshot.snapshot_sha256.as_str()),
            None,
        ),
        Err(error) => ("failed", "", None, None, None, Some(error.to_string())),
    };
    crate::eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "recovery_boundary_snapshot",
            "recovery_plan_auto_run_current": used,
            "status": status,
            "workspace_relative_path": path,
            "file_count": file_count,
            "total_bytes": total_bytes,
            "snapshot_sha256": snapshot_sha256,
            "reason": reason,
        }),
    );
}

fn emit_with_candidate(
    config: &Config,
    event: &str,
    used: u8,
    candidate: &RecoveryCandidate,
    snapshot: Option<&crate::planner::recovery_snapshot::RecoveryBoundarySnapshot>,
    treatment: Option<&Path>,
) {
    crate::eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": event,
            "recovery_plan_auto_runs": config.recovery_plan_auto_runs,
            "recovery_plan_auto_runs_used": used,
            "recovery_plan_auto_run_current": used,
            "recovery_plan_auto_run_stop_reason": "running",
            "recovery_handoff_kind": candidate.handoff.failure_kind,
            "recovery_candidate_scope": if candidate.handoff.failed_step.is_some() { "step" } else { "phase" },
            "recovery_failed_step": candidate.handoff.failed_step,
            "recovery_verify_command_count": candidate.handoff.verify_commands.len(),
            "recovery_verify_command_source": candidate.verify_command_source,
            "recovery_ultra_plan_path": crate::planner::repair::workspace_relative_handoff_path(
                &candidate.path
            ),
            "pre_recovery_snapshot_path": snapshot
                .map(|snapshot| snapshot.workspace_relative_path.as_str())
                .unwrap_or_default(),
            "recovery_treatment_path": treatment
                .and_then(|path| path.strip_prefix(&config.workspace_root).ok())
                .map(|path| path.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default(),
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::ultra_plan::UltraPhase;
    use crate::providers::AssistantReply;
    use crate::state::ConversationMessage;
    use crate::tools::registry::ToolSpec;
    use clap::Parser;
    use std::collections::VecDeque;

    #[derive(Clone)]
    struct UnusedClient;

    impl ChatClient for UnusedClient {
        fn label(&self) -> &str {
            "unused"
        }

        fn boxed_clone(&self) -> Box<dyn ChatClient> {
            Box::new(self.clone())
        }

        fn chat(
            &mut self,
            _model: &str,
            _messages: &[ConversationMessage],
            _tools: &[ToolSpec],
            _native_tools_enabled: bool,
        ) -> anyhow::Result<AssistantReply> {
            panic!("missing file must fail before a model call")
        }
    }

    fn plan(goal: &str) -> UltraPlan {
        UltraPlan {
            goal: goal.to_string(),
            profile: "generic".to_string(),
            style: "recovery".to_string(),
            intent: "fix".to_string(),
            phases: vec![UltraPhase {
                id: "repair".to_string(),
                prompt: "repair".to_string(),
            }],
        }
    }

    fn config(root: &Path, limit: u8) -> Config {
        let mut config =
            Config::from_cli(crate::cli::Cli::parse_from(["commandagent", "--ux-demo"])).unwrap();
        config.workspace_root = root.to_path_buf();
        let events_dir = root.join(".commandagent");
        std::fs::create_dir_all(&events_dir).unwrap();
        config.eval_events_path = Some(events_dir.join("events.jsonl"));
        config.recovery_plan_auto_runs = limit;
        config
    }

    fn candidate(goal: &str) -> RecoveryCandidate {
        RecoveryCandidate {
            path: PathBuf::from(format!("{goal}.yaml")),
            plan: plan(goal),
            handoff: crate::planner::repair::RecoveryHandoff {
                profile: "generic".to_string(),
                original_goal: goal.to_string(),
                failure_kind: "verification_failed".to_string(),
                failed_step: Some(goal.to_string()),
                verify_commands: vec![format!("verify-{goal}")],
                ..crate::planner::repair::RecoveryHandoff::default()
            },
            verify_command_source: "failure_handoff".to_string(),
        }
    }

    fn copy_fixture_tree(source: &Path, target: &Path) {
        std::fs::create_dir_all(target).unwrap();
        for entry in std::fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let destination = target.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_fixture_tree(&entry.path(), &destination);
            } else {
                std::fs::copy(entry.path(), destination).unwrap();
            }
        }
    }

    fn success(report: &str) -> AttemptOutcome {
        AttemptOutcome {
            result: Ok(report.to_string()),
            failure: None,
        }
    }

    fn failed(failure: AttemptFailure) -> AttemptOutcome {
        AttemptOutcome {
            result: Err(anyhow::anyhow!("scripted honest failure")),
            failure: Some(failure),
        }
    }

    fn recoverable(candidate: RecoveryCandidate) -> AttemptFailure {
        AttemptFailure::Recoverable(Box::new(candidate))
    }

    struct ScriptedDriver {
        prepare_error: Option<CandidateStop>,
        outcomes: VecDeque<AttemptOutcome>,
        starts: Vec<u8>,
    }

    impl RecoveryDriver for ScriptedDriver {
        type Prepared = UltraPlan;

        fn prepare(
            &mut self,
            candidate: &RecoveryCandidate,
        ) -> Result<Self::Prepared, CandidateStop> {
            if let Some(error) = self.prepare_error {
                return Err(error);
            }
            Ok(candidate.plan.clone())
        }

        fn normalized(&self, prepared: &Self::Prepared) -> anyhow::Result<Vec<u8>> {
            normalized_plan(prepared)
        }

        fn start(
            &mut self,
            used: u8,
            _candidate: &RecoveryCandidate,
            _prepared: &Self::Prepared,
        ) -> Result<(), CandidateStop> {
            self.starts.push(used);
            Ok(())
        }

        fn execute(&mut self, _prepared: Self::Prepared) -> AttemptOutcome {
            self.outcomes.pop_front().expect("scripted outcome")
        }
    }

    fn driver(outcomes: Vec<AttemptOutcome>) -> ScriptedDriver {
        ScriptedDriver {
            prepare_error: None,
            outcomes: outcomes.into(),
            starts: Vec::new(),
        }
    }

    #[test]
    fn initial_success_runs_no_recovery() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path(), 3);
        let mut driver = driver(Vec::new());
        assert_eq!(
            drive(&config, success("initial"), &mut driver).unwrap(),
            "initial"
        );
        assert!(driver.starts.is_empty());
    }

    #[test]
    fn zero_uses_the_exact_legacy_path_without_auto_events() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path(), 0);
        let mut planner = UnusedClient;
        let mut execution = UnusedClient;
        assert!(
            run_file_with_ui(
                &mut planner,
                &mut execution,
                Path::new("missing.yaml"),
                &config,
                &crate::tui::NOOP_UI,
            )
            .is_err()
        );
        assert!(!config.eval_events_path.unwrap().exists());
    }

    #[test]
    fn failure_then_recovery_success_stops_after_one() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path(), 3);
        let mut driver = driver(vec![success("recovered")]);
        let initial = failed(recoverable(candidate("first")));
        assert_eq!(drive(&config, initial, &mut driver).unwrap(), "recovered");
        assert_eq!(driver.starts, vec![1]);
    }

    #[test]
    fn repeated_failures_stop_at_exact_configured_count() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path(), 2);
        let mut driver = driver(vec![
            failed(recoverable(candidate("second"))),
            failed(recoverable(candidate("third"))),
        ]);
        let initial = failed(recoverable(candidate("first")));
        assert!(drive(&config, initial, &mut driver).is_err());
        assert_eq!(driver.starts, vec![1, 2]);
        let events = std::fs::read_to_string(config.eval_events_path.unwrap()).unwrap();
        assert!(events.contains("\"recovery_plan_auto_run_stop_reason\":\"limit_reached\""));
    }

    #[test]
    fn non_recoverable_and_invalid_candidates_stop_without_execution() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path(), 2);
        let mut non_recoverable = driver(Vec::new());
        assert!(
            drive(
                &config,
                failed(AttemptFailure::NonRecoverable),
                &mut non_recoverable,
            )
            .is_err()
        );
        assert!(non_recoverable.starts.is_empty());

        let mut invalid = driver(Vec::new());
        invalid.prepare_error = Some(CandidateStop::RecoveryYamlInvalid);
        assert!(
            drive(
                &config,
                failed(recoverable(candidate("invalid"))),
                &mut invalid,
            )
            .is_err()
        );
        assert!(invalid.starts.is_empty());
    }

    #[test]
    fn candidate_preparation_returns_typed_safety_stops() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path(), 1);
        let expected_plan = plan("candidate");

        let missing = RecoveryCandidate {
            path: root.path().join("missing.yaml"),
            plan: expected_plan.clone(),
            handoff: crate::planner::repair::RecoveryHandoff {
                failure_kind: "test".to_string(),
                ..crate::planner::repair::RecoveryHandoff::default()
            },
            verify_command_source: "failure_handoff".to_string(),
        };
        assert_eq!(
            prepare_candidate(&config, &missing).unwrap_err(),
            CandidateStop::RecoveryYamlMissing
        );

        let invalid_path = root.path().join("invalid.yaml");
        std::fs::write(&invalid_path, "not a plan").unwrap();
        let invalid = RecoveryCandidate {
            path: invalid_path,
            plan: expected_plan.clone(),
            handoff: crate::planner::repair::RecoveryHandoff {
                failure_kind: "test".to_string(),
                ..crate::planner::repair::RecoveryHandoff::default()
            },
            verify_command_source: "failure_handoff".to_string(),
        };
        assert_eq!(
            prepare_candidate(&config, &invalid).unwrap_err(),
            CandidateStop::RecoveryYamlInvalid
        );

        let review_path = root.path().join("review.yaml");
        std::fs::write(
            &review_path,
            format!(
                "recovery_needs_review: true\n{}",
                crate::planner::ultra_plan::render_ultra_plan(&expected_plan)
            ),
        )
        .unwrap();
        let review = RecoveryCandidate {
            path: review_path,
            plan: expected_plan.clone(),
            handoff: crate::planner::repair::RecoveryHandoff {
                failure_kind: "test".to_string(),
                ..crate::planner::repair::RecoveryHandoff::default()
            },
            verify_command_source: "failure_handoff".to_string(),
        };
        assert_eq!(
            prepare_candidate(&config, &review).unwrap_err(),
            CandidateStop::RecoveryNeedsReview
        );

        let outside = tempfile::tempdir().unwrap();
        let outside_path = outside.path().join("outside.yaml");
        std::fs::write(
            &outside_path,
            crate::planner::ultra_plan::render_ultra_plan(&expected_plan),
        )
        .unwrap();
        let escaped = RecoveryCandidate {
            path: outside_path,
            plan: expected_plan,
            handoff: crate::planner::repair::RecoveryHandoff {
                failure_kind: "test".to_string(),
                ..crate::planner::repair::RecoveryHandoff::default()
            },
            verify_command_source: "failure_handoff".to_string(),
        };
        assert_eq!(
            prepare_candidate(&config, &escaped).unwrap_err(),
            CandidateStop::PathEscape
        );
    }

    #[test]
    fn controller_caps_recovery_runs_at_exact_limit() {
        let mut controller = AutoRecoveryController::new(2);
        assert_eq!(controller.next_run(), Some(1));
        assert_eq!(controller.next_run(), Some(2));
        assert_eq!(controller.next_run(), None);
        assert_eq!(controller.used, 2);
    }

    #[test]
    fn normalized_cycle_ignores_yaml_metadata_paths_and_formatting() {
        let original = plan("recover");
        let rendered = crate::planner::ultra_plan::render_ultra_plan(&original);
        let decorated =
            format!("# volatile path: /tmp/first.yaml\nrecovery_failure_kind: first\n{rendered}");
        let parsed = crate::planner::ultra_plan::parse_ultra_plan(&decorated).unwrap();
        let mut controller = AutoRecoveryController::new(2);
        assert!(controller.observe_plan(normalized_plan(&original).unwrap()));
        assert!(!controller.observe_plan(normalized_plan(&parsed).unwrap()));
    }

    #[test]
    fn attempt_outcome_is_typed_without_error_text_classification() {
        struct InterruptedUi;
        impl InteractionUi for InterruptedUi {
            fn before_model_call(&self, _label: &str) -> crate::tui::UiGuard {
                crate::tui::UiGuard::noop()
            }

            fn before_tool_call(&self, _name: &str) -> crate::tui::UiGuard {
                crate::tui::UiGuard::noop()
            }

            fn publish_status(&self, _status: crate::tui::status::UiStatus) {}

            fn interrupted(&self) -> bool {
                true
            }
        }
        let outcome = capture_attempt(&InterruptedUi, || anyhow::bail!("arbitrary wording"));
        assert!(matches!(outcome.failure, Some(AttemptFailure::Interrupted)));

        let outcome = capture_attempt(&crate::tui::NOOP_UI, || {
            anyhow::bail!("interrupted by user")
        });
        assert!(matches!(
            outcome.failure,
            Some(AttemptFailure::NonRecoverable)
        ));

        let expected = candidate("typed");
        let recorded = expected.clone();
        let outcome = capture_attempt(&crate::tui::NOOP_UI, || {
            record_candidate(
                recorded.path,
                recorded.plan,
                recorded.handoff.failure_kind,
                recorded.handoff.failed_step,
                recorded.handoff.verify_commands,
            );
            anyhow::bail!("unclassified failure")
        });
        assert!(matches!(
            outcome.failure,
            Some(AttemptFailure::Recoverable(candidate)) if *candidate == expected
        ));
    }

    #[test]
    fn step_candidate_is_not_overwritten_by_later_phase_candidate() {
        let capture = AttemptCaptureGuard::begin();
        record_candidate(
            PathBuf::from("phase.yaml"),
            plan("phase"),
            "phase_execute_error".to_string(),
            None,
            Vec::new(),
        );
        record_candidate(
            PathBuf::from("step.yaml"),
            plan("step"),
            "verify_repair_progress_unchanged".to_string(),
            Some("verify-output".to_string()),
            vec!["test -f app.py".to_string()],
        );
        record_candidate(
            PathBuf::from("later-phase.yaml"),
            plan("later-phase"),
            "phase_execute_error".to_string(),
            None,
            Vec::new(),
        );

        let selected = capture.finish().expect("selected candidate");
        assert_eq!(selected.path, PathBuf::from("step.yaml"));
        assert_eq!(
            selected.handoff.failed_step.as_deref(),
            Some("verify-output")
        );
        assert_eq!(selected.handoff.verify_commands, vec!["test -f app.py"]);
    }

    #[test]
    fn recovery_candidate_rebinds_unregistered_step_commands_to_contract() {
        let root = tempfile::tempdir().unwrap();
        let contract_path = root.path().join("completion-contract.json");
        std::fs::write(
            &contract_path,
            r#"{"goal":"Fix the registered CLI behavior","required_paths":["cli.py"],"verify_commands":["python3 cli.py 16","python3 -m pytest -q tests"],"fix_reproducer_command":"python3 cli.py 16","required_capabilities":["input_output_contract"],"profile":"cli"}"#,
        )
        .unwrap();
        let mut config = config(root.path(), 1);
        config.completion_contract_path = Some(contract_path);

        let mut original = candidate("reproduce-failure");
        original.handoff.profile = "cli".to_string();
        original.handoff.original_goal = "repair cli input 16".to_string();
        original.handoff.failure_evidence =
            vec!["command failed: [ $exit_code -eq 2 ]: unary operator expected".to_string()];
        original.handoff.repair_targets = vec!["implementation".to_string()];
        original.handoff.verify_commands = vec![
            "python3 cli.py 16".to_string(),
            "exit_code=$?".to_string(),
            "[ $exit_code -eq 2 ]".to_string(),
        ];
        original.plan = crate::planner::repair::build_recovery_ultra_plan(&original.handoff);
        let original_path = original.path.clone();

        let rebound =
            bind_candidate_verify_commands(&config, original, "command failed: python3 cli.py 16")
                .unwrap();

        assert_ne!(rebound.path, original_path);
        assert_eq!(rebound.verify_command_source, "completion_contract");
        assert_eq!(
            rebound.handoff.verify_commands,
            vec!["python3 cli.py 16", "python3 -m pytest -q tests"]
        );
        assert_eq!(
            rebound.handoff.original_goal,
            "Fix the registered CLI behavior"
        );
        assert!(
            rebound
                .handoff
                .repair_targets
                .contains(&"cli.py".to_string())
        );
        let rendered = std::fs::read_to_string(&rebound.path).unwrap();
        assert!(rendered.contains("Preferred product-visible final-success check"));
        assert!(rendered.contains("python3 cli.py 16"));
        assert!(rendered.contains("python3 -m pytest -q tests"));
        assert!(!rendered.contains("exit_code=$?"));
        assert!(!rendered.contains("[ $exit_code -eq 2 ]"));
        let events = std::fs::read_to_string(config.eval_events_path.unwrap()).unwrap();
        assert!(events.contains("recovery_candidate_verify_commands_bound"));
        assert!(events.contains("recovery_handoff_fidelity_bound"));
        assert!(events.contains("\"original_commands_all_registered\":false"));
        assert!(events.contains("\"recovery_plan_rewritten\":true"));
    }

    #[test]
    fn recovery_candidate_binds_missing_completion_obligation_targets() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("fixture")).unwrap();
        std::fs::write(root.path().join("app.py"), "print('ready')\n").unwrap();
        std::fs::write(root.path().join("fixture/task-01.json"), "{}\n").unwrap();
        std::fs::write(root.path().join("fixture/control.json"), "{}\n").unwrap();
        let contract_path = root.path().join("completion-contract.json");
        std::fs::write(
            &contract_path,
            r#"{"goal":"Fix app.py without changing frozen fixtures","required_paths":["app.py","fixture/task-01.json","fixture/control.json"],"protected_paths":["fixture"],"verify_commands":["python3 app.py fixture/task-01.json"],"fix_reproducer_command":"python3 app.py fixture/task-01.json","required_evidence":["implementation_artifact","bound_verify_command"],"required_obligations":["implementation","verification","acceptance_evidence"],"profile":"generic"}"#,
        )
        .unwrap();
        let mut config = config(root.path(), 1);
        config.completion_contract_path = Some(contract_path);
        let mut original = candidate("completion-obligations");
        original.handoff.original_goal =
            "Fix app.py and verify fixture/task-01.json without substitution".to_string();
        original.handoff.verify_commands = vec!["python3 app.py fixture/task-01.json".to_string()];

        let rebound = bind_candidate_verify_commands(&config, original, "completion incomplete")
            .expect("bind completion requirements");

        assert!(
            rebound
                .handoff
                .missing_paths
                .contains(&"tests/test_app.py".to_string())
        );
        assert!(
            rebound
                .handoff
                .missing_paths
                .contains(&"README.md".to_string())
        );
        assert!(
            rebound
                .handoff
                .repair_targets
                .contains(&"app.py".to_string())
        );
        assert!(
            !rebound
                .handoff
                .repair_targets
                .contains(&"fixture/task-01.json".to_string())
        );
        let rendered = std::fs::read_to_string(&rebound.path).unwrap();
        assert!(rendered.contains("tests/test_app.py"));
        assert!(rendered.contains("README.md"));
        let events = std::fs::read_to_string(config.eval_events_path.unwrap()).unwrap();
        assert!(events.contains("recovery_candidate_completion_requirements_bound"));
        assert!(events.contains("\"obligation_repair_target_count\":2"));
        assert!(events.contains("\"recovery_plan_rewrite_required\":true"));
    }

    #[test]
    fn preflight_requires_full_completion_contract_before_success() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("fixture")).unwrap();
        std::fs::write(root.path().join("app.py"), "print('ready')\n").unwrap();
        std::fs::write(root.path().join("fixture/task-01.json"), "{}\n").unwrap();
        std::fs::write(root.path().join("fixture/control.json"), "{}\n").unwrap();
        let contract_path = root.path().join("completion-contract.json");
        std::fs::write(
            &contract_path,
            r#"{"required_paths":["app.py","fixture/task-01.json","fixture/control.json"],"verify_commands":["python3 app.py fixture/task-01.json"],"fix_reproducer_command":"python3 app.py fixture/task-01.json","required_evidence":["implementation_artifact","bound_verify_command"],"required_obligations":["implementation","verification","acceptance_evidence"],"profile":"generic"}"#,
        )
        .unwrap();
        let mut config = config(root.path(), 1);
        config.completion_contract_path = Some(contract_path);
        let recovery = candidate("completion-gate");

        let incomplete = recovery_preflight(&config, &recovery, 0);
        assert!(
            matches!(
                &incomplete,
                RecoveryPreflight::VerificationInconsistency { reason }
                    if reason.starts_with("registered_observations_passed_but_completion_contract_acceptance_failed:")
            ),
            "{incomplete:?}"
        );

        std::fs::create_dir_all(root.path().join("tests")).unwrap();
        std::fs::write(
            root.path().join("tests/test_app.py"),
            "def test_app():\n    assert True\n",
        )
        .unwrap();
        std::fs::write(root.path().join("README.md"), "# Acceptance\nready\n").unwrap();
        assert!(matches!(
            recovery_preflight(&config, &recovery, 1),
            RecoveryPreflight::CurrentSuccess { .. }
        ));
    }

    #[test]
    fn recovery_candidate_rewrites_an_incomplete_handoff_even_when_commands_match() {
        let root = tempfile::tempdir().unwrap();
        let contract_path = root.path().join("completion-contract.json");
        std::fs::write(
            &contract_path,
            r#"{"goal":"Create ready.txt","required_paths":["ready.txt"],"verify_commands":["test -f ready.txt"],"profile":"generic"}"#,
        )
        .unwrap();
        let mut config = config(root.path(), 1);
        config.completion_contract_path = Some(contract_path);
        let mut original = candidate("exact");
        original.handoff.verify_commands = vec!["test -f ready.txt".to_string()];
        let original_path = original.path.clone();

        let rebound =
            bind_candidate_verify_commands(&config, original, "missing ready.txt").unwrap();

        assert_ne!(rebound.path, original_path);
        assert_eq!(rebound.verify_command_source, "completion_contract");
        assert_eq!(rebound.handoff.verify_commands, vec!["test -f ready.txt"]);
        assert_eq!(rebound.handoff.original_goal, "Create ready.txt");
        assert_eq!(rebound.handoff.repair_targets, vec!["ready.txt"]);
        assert!(root.path().join(".commandagent/plans").exists());
    }

    #[test]
    fn preflight_protects_registered_current_success_without_external_oracle() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("ready.txt"), "ready").unwrap();
        let contract_path = root.path().join("completion-contract.json");
        std::fs::write(
            &contract_path,
            r#"{"required_paths":["ready.txt"],"verify_commands":["test -f ready.txt"],"profile":"generic"}"#,
        )
        .unwrap();
        let mut config = config(root.path(), 1);
        config.completion_contract_path = Some(contract_path);

        let protected = recovery_preflight(&config, &candidate("protected"), 0);
        assert!(
            matches!(&protected, RecoveryPreflight::CurrentSuccess { .. }),
            "{protected:?}"
        );
    }

    #[test]
    fn preflight_allows_recovery_for_registered_artifact_failure() {
        let root = tempfile::tempdir().unwrap();
        let contract_path = root.path().join("completion-contract.json");
        std::fs::write(
            &contract_path,
            r#"{"required_paths":["missing.txt"],"verify_commands":["test -f missing.txt"],"profile":"generic"}"#,
        )
        .unwrap();
        let mut config = config(root.path(), 1);
        config.completion_contract_path = Some(contract_path);

        assert!(matches!(
            recovery_preflight(&config, &candidate("repairable"), 0),
            RecoveryPreflight::Failed { .. }
        ));
    }

    #[test]
    fn preflight_observes_input_output_contract_with_bound_fix_reproducer() {
        let root = tempfile::tempdir().unwrap();
        let app = root.path().join("app.py");
        std::fs::write(&app, "print('ready')\n").unwrap();
        let contract_path = root.path().join("completion-contract.json");
        std::fs::write(
            &contract_path,
            r#"{"required_paths":["app.py"],"verify_commands":["python3 app.py"],"fix_reproducer_command":"python3 app.py","required_capabilities":["input_output_contract"],"profile":"generic"}"#,
        )
        .unwrap();
        let mut config = config(root.path(), 1);
        config.completion_contract_path = Some(contract_path);

        let protected = recovery_preflight(&config, &candidate("protected-input-output"), 0);
        assert!(
            matches!(&protected, RecoveryPreflight::CurrentSuccess { .. }),
            "{protected:?}"
        );

        std::fs::write(app, "raise SystemExit(1)\n").unwrap();
        let missing = recovery_preflight(&config, &candidate("repairable-input-output"), 1);
        assert!(
            matches!(&missing, RecoveryPreflight::Failed { .. }),
            "{missing:?}"
        );
    }

    #[test]
    fn preflight_observes_typed_data_capabilities_in_isolation() {
        let root = tempfile::tempdir().unwrap();
        copy_fixture_tree(
            Path::new("tests/fixtures/goal_verify_v4/a15/fix-data-reconciliation/after"),
            root.path(),
        );
        let contract_path = root.path().join("completion-contract.json");
        std::fs::write(
            &contract_path,
            r#"{
              "required_paths":["pipeline/main.py","data/task-02.csv","output/inspection.json","output/results.json","output/report.md"],
              "verify_commands":["python3 pipeline/main.py data/task-02.csv"],
              "required_capabilities":["pipeline_probe","data_reconciliation","data_claims_binding","data_rerun_consistency","data_results_schema"],
              "profile":"data",
              "goal":"Summarize data/task-02.csv and reconcile every input row."
            }"#,
        )
        .unwrap();
        let mut config = config(root.path(), 1);
        config.completion_contract_path = Some(contract_path);
        let mut recovery = candidate("data-capability-preflight");
        recovery.plan.profile = "data".to_string();
        recovery.handoff.profile = "data".to_string();
        let source_before =
            crate::planner::recovery_snapshot::current_source_sha256(root.path()).unwrap();

        let observed = recovery_preflight(&config, &recovery, 0);

        assert!(
            matches!(&observed, RecoveryPreflight::CurrentSuccess { .. }),
            "{observed:?}"
        );
        assert_eq!(
            crate::planner::recovery_snapshot::current_source_sha256(root.path()).unwrap(),
            source_before
        );
        assert!(!root.path().join("evidence").exists());
        let contract =
            crate::minimal_loop::completion::CompletionContract::load_for_config(&config)
                .unwrap()
                .unwrap();
        let (commands, unsupported) = recovery_preflight_capability_commands(&contract);
        assert!(unsupported.is_empty(), "{unsupported:?}");
        assert!(commands.contains(
            &crate::planner::profiles::data::step_policy::catalog_check_command_with_input(
                "pipeline_probe",
                "data/task-02.csv",
            )
        ));
        assert!(!commands.contains(
            &crate::planner::profiles::data::step_policy::catalog_check_command("pipeline_probe",)
        ));
        let probe: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                root.path().join(
                    ".commandagent/recovery-observations/attempt-0/workspace/evidence/pipeline-run.json",
                ),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            probe["command"],
            serde_json::json!(["python3", "-B", "pipeline/main.py", "data/task-02.csv"])
        );
    }

    #[test]
    fn preflight_rejects_unbound_input_output_contract_capability() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("ready.txt"), "ready").unwrap();
        let contract_path = root.path().join("completion-contract.json");
        let mut config = config(root.path(), 1);
        config.completion_contract_path = Some(contract_path.clone());

        for contract in [
            r#"{"required_paths":["ready.txt"],"verify_commands":["test -f ready.txt"],"required_capabilities":["input_output_contract"],"profile":"generic"}"#,
            r#"{"required_paths":["ready.txt"],"verify_commands":["test -f ready.txt"],"fix_reproducer_command":"test -f other.txt","required_capabilities":["input_output_contract"],"profile":"generic"}"#,
        ] {
            std::fs::write(&contract_path, contract).unwrap();
            assert!(matches!(
                recovery_preflight(&config, &candidate("unbound-input-output"), 0),
                RecoveryPreflight::Unavailable { reason }
                    if reason.ends_with(":input_output_contract")
            ));
        }
    }

    #[test]
    fn preflight_rejects_and_restores_source_mutation() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("writer.py"),
            "from pathlib import Path\nPath('injected.txt').write_text('x')\n",
        )
        .unwrap();
        let contract_path = root.path().join("completion-contract.json");
        std::fs::write(
            &contract_path,
            r#"{"required_paths":["writer.py"],"verify_commands":["python3 writer.py"],"profile":"generic"}"#,
        )
        .unwrap();
        let mut config = config(root.path(), 1);
        config.completion_contract_path = Some(contract_path);

        assert!(matches!(
            recovery_preflight(&config, &candidate("mutating"), 0),
            RecoveryPreflight::Unavailable { reason }
                if reason == "preflight_source_mutation_rejected_and_restored"
        ));
        assert!(!root.path().join("injected.txt").exists());
    }

    #[test]
    fn preflight_does_not_treat_build_as_browser_capability_success() {
        let root = tempfile::tempdir().unwrap();
        let contract_path = root.path().join("completion-contract.json");
        std::fs::write(
            &contract_path,
            r#"{"required_paths":[],"verify_commands":["true"],"fix_reproducer_command":"true","required_capabilities":["input_output_contract","browser_readiness"],"profile":"nextjs"}"#,
        )
        .unwrap();
        let mut config = config(root.path(), 1);
        config.completion_contract_path = Some(contract_path);

        assert!(matches!(
            recovery_preflight(&config, &candidate("browser"), 0),
            RecoveryPreflight::Unavailable { reason }
                if reason.contains("browser_readiness")
        ));
        assert!(
            !root
                .path()
                .join(".commandagent/recovery-boundaries")
                .exists()
        );
    }
}
