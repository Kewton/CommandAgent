//! Binds a session to the tool set its selected profile and pack admitted
//! (Issue #972). Off keeps the default registry; a refusal stops the session
//! before its first provider call. A set with a G2 schema tool (Issue #973) also
//! returns the scope the receiving parser reads; without one there is no scope. A
//! proposal tool (Issue #974) is also judged against what only a session knows.

use std::path::Path;

use serde_json::Value;

use super::context::{RunSessionOptions, RunSessionStepKind};
use super::implementation_completion::structured_stage_completion::enter_stage_run;
use crate::config::Config;
use crate::planner::structured_stages::ledger::LedgerEntry;
use crate::providers::ChatClient;
use crate::state::ToolCall;
use crate::tools::profile_tools::admit_selected;
use crate::tools::profile_tools::effects::SessionAccess;
use crate::tools::profile_tools::schema::invocation::{self, SessionScope};
use crate::tools::registry::ToolRegistry;

pub(super) use invocation::{
    blocks_xml_fallback, recover_unless_schema_tool as recover_tool_arguments,
};

pub(super) fn session_registry(
    config: &Config,
    client: &dyn ChatClient,
    options: &RunSessionOptions,
) -> anyhow::Result<(ToolRegistry, Option<SessionScope>)> {
    let run_id = config.eval_events_path.as_deref().and_then(run_directory);
    match admit_selected(&config.workspace_root, &config.profile, run_id)? {
        Some(admitted) => {
            admitted.preflight_schema_tools(config, client.profile_tool_schema_wire())?;
            admitted.preflight_session(&|name| session_access(config, options, name))?;
            admitted.bind_stage_session(config, LedgerEntry::of_config(config))?;
            if let Some(run) = admitted.stage_run() {
                run.remember_bound();
                enter_stage_run(run);
            }
            Ok((admitted.registry()?, admitted.enter_session()))
        }
        None => Ok((ToolRegistry::default(), None)),
    }
}

/// `ToolContext.mode` is `Act` in every session, so it says nothing here. The step's kind
/// and the recovery inspection's own rule do.
fn session_access(config: &Config, options: &RunSessionOptions, tool: &str) -> SessionAccess {
    if options.step_kind == Some(RunSessionStepKind::Inspect) {
        return SessionAccess::InspectStep;
    }
    let call = ToolCall {
        id: String::new(),
        name: tool.to_string(),
        arguments: Value::Null,
    };
    if crate::planner::recovery_inspection::tool_rejection(
        config,
        options.phase_scope.as_deref(),
        &call,
    )
    .is_some()
    {
        return SessionAccess::ScopedRecoveryReads;
    }
    SessionAccess::Normal
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

#[cfg(test)]
mod tests {
    use super::{RunSessionOptions, RunSessionStepKind, SessionAccess, session_access};
    use crate::config::Config;

    fn config() -> Config {
        crate::tools::profile_tools::effects::config_for_test(std::path::Path::new("/nonexistent"))
    }

    #[test]
    fn an_inspect_step_reads_only_and_another_step_is_not_held_back() {
        let inspect = RunSessionOptions {
            step_kind: Some(RunSessionStepKind::Inspect),
            ..RunSessionOptions::default()
        };
        assert_eq!(
            session_access(&config(), &inspect, "Propose"),
            SessionAccess::InspectStep
        );
        for step_kind in [None, Some(RunSessionStepKind::Implement)] {
            let options = RunSessionOptions {
                step_kind,
                ..RunSessionOptions::default()
            };
            assert_eq!(
                session_access(&config(), &options, "Propose"),
                SessionAccess::Normal
            );
        }
    }
}
