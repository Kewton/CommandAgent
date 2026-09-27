use std::path::Path;

use super::{NormalizedVerifyCommand, VerifyCommandRunResult};

/// Outcome recorded at the boundary where a plan verify command ran.
///
/// A command that was only declared and never executed is never recorded here;
/// the projection reports it as not-recorded instead of upgrading it to success.
#[derive(Clone, Copy)]
enum VerifyCommandObservationStatus {
    Passed,
    Failed,
}

impl VerifyCommandObservationStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
        }
    }
}

/// Records one plan verify execution as an additive observation event.
///
/// Existing events, keys, and schema versions are unchanged. The declared
/// command is the one prepared for execution and the effective command is the
/// one that actually ran after runtime normalization, so a reader can see that
/// they diverged instead of assuming the declared form ran.
pub(super) fn record(
    path: Option<&Path>,
    step: &str,
    declared: &NormalizedVerifyCommand,
    outcome: &VerifyCommandRunResult,
) {
    let (status, effective_command) = match outcome {
        VerifyCommandRunResult::Passed {
            normalization: Some(normalization),
            ..
        } => (
            VerifyCommandObservationStatus::Passed,
            normalization.repaired.clone(),
        ),
        VerifyCommandRunResult::Passed { .. } => (
            VerifyCommandObservationStatus::Passed,
            declared.as_str().to_string(),
        ),
        VerifyCommandRunResult::Failed { command, .. }
        | VerifyCommandRunResult::FalseNegative { command, .. } => {
            (VerifyCommandObservationStatus::Failed, command.clone())
        }
    };
    crate::eval_events::emit(
        path,
        serde_json::json!({
            "event": "verify_command_observed",
            "step": step,
            "declared_command": crate::eval_events::body_snippet(declared.as_str()),
            "effective_command": crate::eval_events::body_snippet(&effective_command),
            "status": status.as_str(),
        }),
    );
}
