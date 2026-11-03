//! The CLI's pre-run admission: before the request pattern settings, the OS
//! sandbox, the artifact preflight, `run_start`, and any provider or classifier
//! call. It reads nothing when no pack is selected and no profile declares tools.
//! A pack the CLI could not resolve stays that resolution's refusal, at its
//! existing place.

use super::ProfileToolRefusal;
use super::admission::{AdmittedProfileTools, admit_with};
use super::selection::PackSource;
use crate::cli_pack::ResolvedPack;
use crate::config::Config;

pub(crate) fn refuse_before_cli_run(
    config: &Config,
    resolved_pack: &anyhow::Result<Option<ResolvedPack>>,
) -> Result<(), ProfileToolRefusal> {
    let pack = match resolved_pack {
        Ok(Some(pack)) => PackSource::Selected {
            selection: pack.selection(),
            declared: pack.declared.clone(),
        },
        Ok(None) => PackSource::InheritedBeforeGates,
        Err(_) => PackSource::None,
    };
    let admitted = admit_with(&config.workspace_root, &config.profile, None, pack)?;
    preflight(admitted, config)
}

/// A workflow child runs under the sandbox and the pack selection its outer run
/// installed, already reconciled with the child's workspace.
pub(crate) fn refuse_before_workflow_node(config: &Config) -> Result<(), ProfileToolRefusal> {
    let admitted = super::admit_selected(&config.workspace_root, &config.profile, None)?;
    preflight(admitted, config)
}

/// A REPL's free text, before its route is proposed. The route classifier is a provider
/// call, and the REPL's provider, profile, and inputs can all change after it started.
/// Only a schema tool's and a proposal tool's reasons stop here. Every other refusal keeps
/// its place at the confirmed command, so a set without one proposes a route as it always
/// did. Returns the block a refused command shows; the REPL goes on.
pub(crate) fn refused_before_gate_one(
    config: &Config,
    execution: &dyn crate::providers::ChatClient,
) -> Option<String> {
    let refused = match super::admit_selected(&config.workspace_root, &config.profile, None) {
        Ok(None) => return None,
        Ok(Some(admitted)) => admitted
            .preflight_schema_tools(config, execution.profile_tool_schema_wire())
            .err()?,
        Err(refusal)
            if refusal.kind.is_schema_tool_reason() || refusal.kind.is_effect_tool_reason() =>
        {
            refusal
        }
        Err(_) => return None,
    };
    Some(crate::tui::status::render_task_failure_block(
        &crate::tui::status::TaskFailureBlock {
            task_status: "failed".to_string(),
            failed_phase: String::new(),
            completed_phases: None,
            total_phases: None,
            primary_stop_reason: refused.to_string(),
            recovery_prompt_command: String::new(),
            recovery_ultra_plan_command: String::new(),
            summary_path: String::new(),
        },
    ))
}

fn preflight(
    admitted: Option<AdmittedProfileTools>,
    config: &Config,
) -> Result<(), ProfileToolRefusal> {
    match admitted {
        Some(admitted) => admitted.preflight_schema_tools(config, super::configured_wire(config)),
        None => Ok(()),
    }
}
