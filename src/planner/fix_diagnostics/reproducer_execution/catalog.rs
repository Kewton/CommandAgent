use crate::config::Config;

use super::ReproducerExecution;

pub(super) fn catalog_check(
    config: &Config,
    command: &str,
    profile: &str,
    goal: &str,
) -> Option<ReproducerExecution> {
    crate::planner::profile::resolve_profile_runtime(profile)
        .run_fix_reproducer_catalog_check(
            &config.workspace_root,
            goal,
            command,
            config.eval_events_path.as_deref(),
        )
        .map(|observation| ReproducerExecution {
            outcome: observation.outcome,
            reason: observation.reason,
            shell_observation: None,
        })
}
