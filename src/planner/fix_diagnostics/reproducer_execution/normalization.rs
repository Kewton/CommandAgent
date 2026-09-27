use crate::planner::adjudication::fix::ProbeOutcome;
use crate::planner::verify::{NormalizedVerifyCommand, normalize_verify_command};

use super::ReproducerExecution;

/// Re-normalizes a stored reproducer command without relying on the
/// unproven idempotence of normalization: a command that no longer
/// normalizes is reported as an unavailable probe instead of panicking.
pub(super) fn normalize_stored_reproducer(
    command: &str,
) -> Result<NormalizedVerifyCommand, Box<ReproducerExecution>> {
    normalize_verify_command(command).map_err(|error| {
        Box::new(ReproducerExecution {
            outcome: ProbeOutcome::Unavailable,
            reason: format!("reproducer_command_not_normalizable:{error}"),
            shell_observation: None,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_reproducer_is_returned_unchanged() {
        let normalized = normalize_stored_reproducer("npm test")
            .ok()
            .expect("valid command");
        assert_eq!(normalized.as_str(), "npm test");
    }

    #[test]
    fn non_normalizable_reproducer_becomes_unavailable_without_panicking() {
        let failure = normalize_stored_reproducer("   ").expect_err("empty command is invalid");
        assert_eq!(failure.outcome, ProbeOutcome::Unavailable);
        assert!(
            failure
                .reason
                .starts_with("reproducer_command_not_normalizable:"),
            "{}",
            failure.reason
        );
        assert!(failure.shell_observation.is_none());
    }
}
