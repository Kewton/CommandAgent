use crate::config::Config;
use crate::eval_events::{CompletionProjection, CompletionSnapshot};
use crate::planner::profile::canonical_profile_name;

pub(super) fn apply_snapshot(config: &Config, snapshot: &mut CompletionSnapshot) -> bool {
    if canonical_profile_name(&snapshot.effective_profile) == "data" {
        super::super::data::apply_snapshot(&config.workspace_root, snapshot);
        return true;
    }
    if super::super::ingest::apply_snapshot(&config.workspace_root, snapshot) {
        return true;
    }
    super::super::cli::apply_snapshot(&config.workspace_root, snapshot)
}

pub(super) fn apply_terminal_projection(config: &Config, projection: &mut CompletionProjection) {
    super::super::cli::apply_terminal_projection(&config.workspace_root, projection);
    super::super::data::apply_terminal_projection(&config.workspace_root, projection);
    super::super::ingest::apply_terminal_projection(&config.workspace_root, projection);
}
