use std::path::Path;

use crate::planner::profile::profile_expected_paths;
use crate::planner::ultra_plan::UltraPhase;

use super::FixRuntime;

mod presence_filter;
#[cfg(test)]
pub(super) use presence_filter::bind_for_workspace;
pub(super) use presence_filter::bind_step_plan;

const POLICY_HEADING: &str = "Data fix cause-isolation artifact policy (runtime-bound):";

pub(super) fn attach_to_phase_prompt(
    runtime: Option<&FixRuntime>,
    phase: &UltraPhase,
    prompt: String,
) -> String {
    let Some(runtime) = runtime.filter(|runtime| applies_prompt(&runtime.profile, phase)) else {
        return prompt;
    };
    attach_for_workspace(
        &runtime.terminal_config.workspace_root,
        &runtime.profile,
        &runtime.goal,
        phase,
        prompt,
    )
}

pub(super) fn attach_for_workspace(
    root: &Path,
    profile: &str,
    goal: &str,
    phase: &UltraPhase,
    mut prompt: String,
) -> String {
    if !applies_prompt(profile, phase) {
        return prompt;
    }
    let (present, absent) = canonical_artifact_presence(root, profile, goal);
    prompt.push_str("\n\n");
    prompt.push_str(POLICY_HEADING);
    prompt.push_str(
        "\n- Use the executed F1 evidence above and files present in the workspace snapshot as the primary diagnostic inputs.",
    );
    prompt.push_str("\n- Existing canonical artifacts allowed for read-only inspection:\n");
    prompt.push_str(&render_paths(&present));
    prompt.push_str(
        "\n- Absent canonical artifacts: do not request, inspect, verify, create, or expect these during isolate-cause:\n",
    );
    prompt.push_str(&render_paths(&absent));
    prompt.push_str(
        "\n- Defer creation or regeneration of absent final artifacts to the repair phase.",
    );
    prompt
}

fn applies_prompt(profile: &str, phase: &UltraPhase) -> bool {
    profile == "data" && phase.id == "isolate-cause"
}

fn canonical_artifact_presence(
    root: &Path,
    profile: &str,
    goal: &str,
) -> (Vec<String>, Vec<String>) {
    profile_expected_paths(root, profile, goal)
        .into_iter()
        .partition(|path| root.join(path).is_file())
}

fn render_paths(paths: &[String]) -> String {
    if paths.is_empty() {
        return "- none".to_string();
    }
    paths
        .iter()
        .map(|path| format!("- {path}"))
        .collect::<Vec<_>>()
        .join("\n")
}
