mod cli;
mod data;
mod ingest;
mod intent;

use crate::config::Config;
use crate::eval_events::{
    CompletionProjection, CompletionSnapshot, GENERIC_REDUCED_ASSURANCE_REASON,
    GENERIC_STATIC_ASSURANCE_REASON,
};
use crate::planner::profile::canonical_profile_name;
use crate::planner::profile_admission::cap_assurance;

pub(crate) fn apply_config_completion_metadata(config: &Config, snapshot: &mut CompletionSnapshot) {
    if let Some(inference) = config.profile_inference {
        snapshot.profile_inferred = inference.profile.to_string();
        snapshot.profile_inference_source = inference.source.as_str().to_string();
    }
    if snapshot.profile.trim().is_empty() {
        snapshot.profile = config.profile.clone();
    }
    if snapshot.effective_profile.trim().is_empty() {
        snapshot.effective_profile = snapshot.profile.clone();
    }
    if snapshot.prompt_layout.trim().is_empty() {
        snapshot.prompt_layout = config.prompt_layout.as_str().to_string();
    }
    if intent::apply_snapshot(config, snapshot) {
        return;
    }
    if canonical_profile_name(&snapshot.profile) == "generic" {
        if snapshot.assurance_level == "static" {
            snapshot.assurance_reason = GENERIC_STATIC_ASSURANCE_REASON.to_string();
        } else {
            snapshot.assurance_level = "reduced".to_string();
            snapshot.assurance_reason = GENERIC_REDUCED_ASSURANCE_REASON.to_string();
        }
    } else {
        snapshot.assurance_level = "full".to_string();
        snapshot.assurance_reason.clear();
    }
}

pub(crate) fn apply_config_completion_projection(
    config: &Config,
    projection: &mut CompletionProjection,
) {
    intent::apply_terminal_projection(config, projection);
    let (level, reason) = (
        &mut projection.assurance_level,
        &mut projection.assurance_reason,
    );
    cap_assurance(&projection.effective_profile, level, reason);
}
