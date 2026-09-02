use super::{
    Config, NodeDependencySetupAuthority, PlanStep, RunSessionOptions, StepKind, StepPromptContext,
    StepRunError, StepRunOutcome, VerificationReport, merge_unique_strings,
    merge_verification_report,
};

use crate::minimal_loop::loop_run::CompletionContractVerification;
use crate::planner::profile_behavior::ProfileRuntime;
use crate::{eval_events, planner::recovery_contract_binding::load_fix_origin};
use serde_json::json;
use std::path::Path;

pub(super) struct BoundRecoveryFix<'a> {
    config: &'a Config,
    goal: String,
    setup_authority: NodeDependencySetupAuthority,
    requires_write: bool,
    suppress_profile_post_step_repair: bool,
    step_id: String,
    safety: crate::planner::recovery_fix_safety::RecoveryFixSafety,
}

pub(super) fn bind<'a>(
    step_config: &'a Config,
    step: &PlanStep,
    prompt_context: &StepPromptContext,
    setup_authority: NodeDependencySetupAuthority,
) -> Result<BoundRecoveryFix<'a>, Box<StepRunError>> {
    let requires_write = requires_write(step_config, step).map_err(|err| {
        Box::new(StepRunError {
            message: format!("Recovery fix origin validation failed: {err}"),
            outcome: StepRunOutcome::default(),
        })
    })?;
    let step_kind = step.step_kind();
    let inspection_fix_origin_bound = if step_kind == StepKind::Inspect {
        load_fix_origin(step_config)
            .map_err(|err| {
                Box::new(StepRunError {
                    message: format!("Recovery fix origin validation failed: {err}"),
                    outcome: StepRunOutcome::default(),
                })
            })?
            .is_some()
    } else {
        false
    };
    let suppress_profile_post_step_repair =
        suppress_profile_post_step_repair(&step_kind, inspection_fix_origin_bound);
    let safety = crate::planner::recovery_fix_safety::RecoveryFixSafety::capture(
        step_config,
        requires_write,
    )
    .map_err(|err| {
        Box::new(StepRunError {
            message: format!("Recovery fix safety binding failed: {err}"),
            outcome: StepRunOutcome::default(),
        })
    })?;
    Ok(BoundRecoveryFix {
        config: step_config,
        goal: prompt_context.overall_goal.clone(),
        setup_authority,
        requires_write,
        suppress_profile_post_step_repair,
        step_id: step.id.clone(),
        safety,
    })
}

pub(in crate::planner::runner) fn requires_write(
    config: &Config,
    step: &PlanStep,
) -> anyhow::Result<bool> {
    Ok(step.step_kind() == StepKind::Implement && load_fix_origin(config)?.is_some())
}

fn suppress_profile_post_step_repair(step_kind: &StepKind, fix_origin_bound: bool) -> bool {
    fix_origin_bound && *step_kind == StepKind::Inspect
}

pub(super) fn defer_contract_verification(
    options: &mut RunSessionOptions,
    recovery_write_required: bool,
) {
    if recovery_write_required {
        options.completion_contract_verification =
            CompletionContractVerification::DisabledDuringStep;
    }
}

impl BoundRecoveryFix<'_> {
    pub(super) fn requires_write(&self) -> bool {
        self.requires_write
    }

    pub(super) fn local_repair_turns(&self, default: usize) -> usize {
        if self.requires_write { 1 } else { default }
    }

    pub(super) fn post_repair(
        &self,
        runtime: &dyn ProfileRuntime,
        root: &Path,
        goal: &str,
    ) -> anyhow::Result<bool> {
        if !self.suppress_profile_post_step_repair {
            return runtime.post_step_repair(root, goal);
        }
        eval_events::emit(
            self.config.eval_events_path.as_deref(),
            json!({
                "event": "recovery_read_only_post_step_repair_suppressed",
                "step_id": self.step_id,
                "step_kind": "inspect",
                "reason": "recovery_read_only_inspection",
            }),
        );
        Ok(false)
    }

    pub(super) fn merge_verification(
        &self,
        report: &mut VerificationReport,
        changed_paths: &[String],
    ) {
        merge_verification_report(
            report,
            self.safety
                .verify(self.config, &self.goal, self.setup_authority, changed_paths),
        );
    }

    pub(super) fn merge_verify_commands(&self, commands: &mut Vec<String>) {
        merge_unique_strings(commands, &self.safety.verify_commands());
    }

    pub(super) fn mutation_checkpoint(
        &self,
    ) -> crate::planner::recovery_fix_safety::ArtifactSnapshot {
        self.safety.artifact_checkpoint(&self.config.workspace_root)
    }

    pub(super) fn observed_changes_from_start(&self, reported_paths: &[String]) -> Vec<String> {
        self.safety.observed_changes_from_start(
            &self.config.workspace_root,
            reported_paths,
            self.config.eval_events_path.as_deref(),
        )
    }

    pub(super) fn observed_changes_since(
        &self,
        checkpoint: &crate::planner::recovery_fix_safety::ArtifactSnapshot,
        reported_paths: &[String],
    ) -> Vec<String> {
        self.safety.observed_artifact_changes(
            &self.config.workspace_root,
            checkpoint,
            reported_paths,
            "bounded_local_repair",
            self.config.eval_events_path.as_deref(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::minimal_loop::loop_run::{RunSessionOptions, RunSessionStepKind};

    #[test]
    fn recovery_implement_defers_contract_verification_to_host_observation() {
        let mut recovery = RunSessionOptions::plan_step(RunSessionStepKind::Implement);
        defer_contract_verification(&mut recovery, true);
        assert_eq!(
            recovery.completion_contract_verification,
            CompletionContractVerification::DisabledDuringStep
        );

        let mut ordinary = RunSessionOptions::plan_step(RunSessionStepKind::Implement);
        defer_contract_verification(&mut ordinary, false);
        assert_eq!(
            ordinary.completion_contract_verification,
            CompletionContractVerification::Enabled
        );
    }

    #[test]
    fn recovery_inspection_suppresses_profile_post_step_repair() {
        assert!(suppress_profile_post_step_repair(&StepKind::Inspect, true));
        assert!(!suppress_profile_post_step_repair(
            &StepKind::Implement,
            true
        ));
        assert!(!suppress_profile_post_step_repair(
            &StepKind::Inspect,
            false
        ));
    }
}
