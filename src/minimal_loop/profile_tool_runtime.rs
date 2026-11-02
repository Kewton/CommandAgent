//! Binds a session to the tool set its selected profile and pack admitted
//! (Issue #972). Off keeps the default registry; a refusal stops the session
//! before its first provider call. A set with a G2 schema tool (Issue #973) also
//! returns the scope the receiving parsers read; without one there is no scope.

use std::path::Path;

use crate::config::Config;
use crate::minimal_loop::requirement_plan::RequirementPlan;
use crate::providers::{AssistantReply, ChatClient};
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

/// The requirement plan takes its block out of the reply before the XML parse. In a
/// session with a schema tool that is noted, because a string in the arguments can
/// hold the block. Otherwise this is only the capture.
pub(super) fn capture_requirement_plan(
    plan: &mut RequirementPlan,
    reply: &mut AssistantReply,
) -> bool {
    let before = invocation::watches_reply_text().then(|| reply.content.clone());
    let plan_only = plan.capture_reply(reply);
    if before.is_some_and(|before| before != reply.content) {
        invocation::note_reply_cut_before_parse();
    }
    plan_only
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
