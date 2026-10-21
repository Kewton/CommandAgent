use crate::config::Config;
use crate::planner::adjudication::fix::ProbeOutcome;
use crate::planner::verify::NormalizedVerifyCommand;
use crate::tools::bash::BashOutcomeKind;

use super::ReproducerExecution;

pub(super) fn execute(
    normalized: &NormalizedVerifyCommand,
    config: &Config,
    profile: &str,
) -> ReproducerExecution {
    match crate::minimal_loop::verifier_env::run_structured_for_verify_with_profile(
        normalized,
        &config.workspace_root,
        Some(profile),
        config.offline,
    ) {
        Ok(observation) => {
            let (outcome, reason) = match observation.kind {
                BashOutcomeKind::Success => {
                    (ProbeOutcome::Success, "command_succeeded".to_string())
                }
                BashOutcomeKind::CommandFailed => (
                    ProbeOutcome::Failure,
                    crate::eval_events::body_snippet(
                        &crate::minimal_loop::verifier_env::format_verify_outcome(&observation),
                    ),
                ),
                BashOutcomeKind::Blocked
                | BashOutcomeKind::Timeout
                | BashOutcomeKind::Cancelled => (
                    ProbeOutcome::Unavailable,
                    crate::eval_events::body_snippet(
                        &crate::minimal_loop::verifier_env::format_verify_outcome(&observation),
                    ),
                ),
            };
            ReproducerExecution {
                outcome,
                reason,
                shell_observation: Some(observation),
            }
        }
        Err(error) => ReproducerExecution {
            outcome: ProbeOutcome::Unavailable,
            reason: format!("reproducer_probe_error:{error}"),
            shell_observation: None,
        },
    }
}
