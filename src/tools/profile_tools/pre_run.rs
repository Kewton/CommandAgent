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

fn preflight(
    admitted: Option<AdmittedProfileTools>,
    config: &Config,
) -> Result<(), ProfileToolRefusal> {
    match admitted {
        Some(admitted) => admitted.preflight_schema_tools(config, super::configured_wire(config)),
        None => Ok(()),
    }
}
