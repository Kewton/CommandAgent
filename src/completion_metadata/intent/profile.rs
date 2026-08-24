use crate::config::Config;
use crate::eval_events::{CompletionProjection, CompletionSnapshot};
use crate::planner::profile::{ProfileId, resolve_profile_runtime};

pub(super) fn apply_snapshot(config: &Config, snapshot: &mut CompletionSnapshot) -> bool {
    let profile_id = ProfileId::parse(&snapshot.effective_profile);
    resolve_profile_runtime(&snapshot.effective_profile).apply_completion_snapshot(
        &profile_id,
        &config.workspace_root,
        snapshot,
    );
    true
}

pub(super) fn apply_terminal_projection(config: &Config, projection: &mut CompletionProjection) {
    let profile_id = ProfileId::parse(&projection.effective_profile);
    resolve_profile_runtime(&projection.effective_profile).apply_completion_projection(
        &profile_id,
        &config.workspace_root,
        projection,
    );
}
