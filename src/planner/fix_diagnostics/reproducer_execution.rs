use crate::config::Config;
use crate::planner::adjudication::fix::ProbeOutcome;
use crate::tools::bash::{BashOutcome, BashOutcomeKind};

pub(super) struct ReproducerExecution {
    pub(super) outcome: ProbeOutcome,
    pub(super) reason: String,
    pub(super) shell_observation: Option<BashOutcome>,
}

pub(super) fn run(
    config: &Config,
    command: &str,
    profile: &str,
    goal: &str,
) -> ReproducerExecution {
    if let Some(observation) = crate::planner::profile::resolve_profile_runtime(profile)
        .run_fix_reproducer_catalog_check(
            &config.workspace_root,
            goal,
            command,
            config.eval_events_path.as_deref(),
        )
    {
        return ReproducerExecution {
            outcome: observation.outcome,
            reason: observation.reason,
            shell_observation: None,
        };
    }
    let normalized: crate::planner::verify::NormalizedVerifyCommand =
        crate::planner::verify::normalize_verify_command(command)
            .expect("stored reproducer is normalized");
    match crate::minimal_loop::verifier_env::run_structured_for_verify_with_profile(
        &normalized,
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
