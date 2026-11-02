//! Binds a session to the tool set its selected profile and pack admitted
//! (Issue #972). Off keeps the default registry; a refusal stops the session
//! before its first provider call.

use std::path::Path;

use crate::config::Config;
use crate::tools::profile_tools::admit_selected;
use crate::tools::registry::ToolRegistry;

pub(super) fn session_registry(config: &Config) -> anyhow::Result<ToolRegistry> {
    let run_id = config.eval_events_path.as_deref().and_then(run_directory);
    match admit_selected(&config.workspace_root, &config.profile, run_id)? {
        Some(admitted) => Ok(admitted.registry()?),
        None => Ok(ToolRegistry::default()),
    }
}

/// The run's evidence directory name, when the run records one. A run without
/// evidence has no identity to bind, and keeps none rather than inventing one.
fn run_directory(events_path: &Path) -> Option<String> {
    events_path
        .parent()?
        .file_name()?
        .to_str()
        .map(str::to_string)
}
