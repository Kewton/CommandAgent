use crate::tools::bash::BashOutcome;
use crate::{config::Config, planner::adjudication::fix::ProbeOutcome};

mod catalog;
mod normalization;
mod shell;

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
    if let Some(execution) = catalog::catalog_check(config, command, profile, goal) {
        return execution;
    }
    let normalized: crate::planner::verify::NormalizedVerifyCommand =
        match normalization::normalize_stored_reproducer(command) {
            Ok(normalized) => normalized,
            Err(failure) => return *failure,
        };
    shell::execute(&normalized, config, profile)
}
