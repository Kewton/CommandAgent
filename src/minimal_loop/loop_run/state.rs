pub(super) use super::recoverable_tool_errors::RecoverableToolErrorState;
use super::*;

#[derive(Debug, Default)]
pub(super) struct VerifyRepairState {
    pub(super) pending_signature: Option<VerificationSignature>,
    pub(super) pending_target: Option<RepairTarget>,
    pub(super) pending_error_context: RunSessionErrorContext,
    pub(super) changed_paths_at_failure: Vec<String>,
    pub(super) no_edit_turns: usize,
}

#[derive(Debug)]
pub(super) struct VerifyFailureFeedback {
    pub(super) feedback: String,
    pub(super) signature: VerificationSignature,
    pub(super) target: RepairTarget,
    pub(super) error_context: RunSessionErrorContext,
}

#[derive(Debug, Clone, Default)]
pub(super) struct ContractObservation {
    pub(super) missing_paths: Vec<String>,
    pub(super) missing_capabilities: Vec<String>,
    pub(super) missing_evidence: Vec<String>,
    pub(super) missing_obligations: Vec<String>,
    pub(super) primary_reason: String,
}

impl ContractObservation {
    pub(super) fn from_report(
        report: &crate::planner::verify::VerificationReport,
        runtime_acceptance: &RuntimeAcceptanceReport,
    ) -> Self {
        Self {
            missing_paths: report.missing_paths.clone(),
            missing_capabilities: runtime_acceptance.missing_capabilities.clone(),
            missing_evidence: runtime_acceptance.missing_evidence.clone(),
            missing_obligations: runtime_acceptance.missing_obligations.clone(),
            primary_reason: report.primary_reason(),
        }
    }
}

#[derive(Debug)]
pub(super) enum ContractVerificationOutcome {
    Satisfied,
    NeedsRepair(VerifyFailureFeedback),
    ObservationIncomplete(ContractObservation),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StepShortCircuitAt {
    Start,
    Iteration,
}

impl StepShortCircuitAt {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Iteration => "iteration",
        }
    }
}

#[derive(Debug)]
pub(super) enum VerifyRepairNoEditOutcome {
    NoPendingFailure,
    Feedback(String),
    ObservationIncomplete(ContractObservation),
}

#[derive(Debug, Default)]
pub(super) struct ArtifactRecoveryState {
    pub(super) target_path: Option<String>,
    pub(super) target_attempts: usize,
    pub(super) last_model_action: Option<String>,
}

impl ArtifactRecoveryState {
    pub(super) fn sync_target(
        &mut self,
        required_paths: &[String],
        missing: &[String],
    ) -> Option<String> {
        let next = required_paths
            .iter()
            .find(|path| missing.contains(path))
            .cloned()
            .or_else(|| missing.first().cloned());
        if self.target_path != next {
            self.target_path = next.clone();
            self.target_attempts = 0;
        }
        next
    }

    pub(super) fn record_action(&mut self, action: &str) {
        self.last_model_action = Some(action.to_string());
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub(super) struct TimeSink {
    pub(super) kind: &'static str,
    pub(super) label: String,
    pub(super) duration_ms: u128,
}
