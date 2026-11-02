//! Binds a session to the tool set its selected profile and pack admitted
//! (Issue #972). Off keeps the default registry; a refusal stops the session
//! before its first provider call. A set with a G2 schema tool (Issue #973) also
//! returns the scope the receiving parser reads; without one there is no scope.

use std::path::Path;

use crate::config::Config;
use crate::providers::ChatClient;
use crate::tools::profile_tools::admit_selected;
use crate::tools::profile_tools::schema::invocation::{self, SessionScope};
use crate::tools::registry::ToolRegistry;

pub(super) use invocation::{
    blocks_xml_fallback, recover_unless_schema_tool as recover_tool_arguments,
};

pub(super) fn session_registry(
    config: &Config,
    client: &dyn ChatClient,
) -> anyhow::Result<(ToolRegistry, Option<SessionScope>)> {
    let run_id = config.eval_events_path.as_deref().and_then(run_directory);
    match admit_selected(&config.workspace_root, &config.profile, run_id)? {
        Some(admitted) => {
            admitted.preflight_schema_tools(config, client.profile_tool_schema_wire())?;
            Ok((admitted.registry()?, admitted.enter_session()))
        }
        None => Ok((ToolRegistry::default(), None)),
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
