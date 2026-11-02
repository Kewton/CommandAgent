//! The CLI's pre-run admission: before the request pattern settings, the OS
//! sandbox, the artifact preflight, `run_start`, and any provider or classifier
//! call. It reads nothing when no selected source declares tools. A pack the CLI
//! could not resolve stays that resolution's refusal, at its existing place.

use super::ProfileToolRefusal;
use super::admission::{SandboxView, admit_with};
use super::selection::{PackSelection, PackSource};
use crate::cli_pack::ResolvedPack;
use crate::config::Config;
use crate::tools::os_sandbox::SandboxMode;

pub(crate) fn refuse_before_cli_run(
    config: &Config,
    resolved_pack: &anyhow::Result<Option<ResolvedPack>>,
    sandbox_mode: SandboxMode,
    action_uses_sandbox: bool,
) -> Result<(), ProfileToolRefusal> {
    let pack = match resolved_pack {
        Ok(Some(pack)) => PackSource::Selected(PackSelection {
            directory: pack.directory.clone(),
            id: pack.id.clone(),
            version: pack.version.clone(),
            hash: pack.hash.clone(),
        }),
        Ok(None) => PackSource::Environment,
        Err(_) => PackSource::None,
    };
    let will_apply = action_uses_sandbox && !matches!(sandbox_mode, SandboxMode::Off);
    admit_with(
        &config.workspace_root,
        &config.profile,
        None,
        pack,
        SandboxView::WillApply(will_apply),
    )
    .map(drop)
}

/// A workflow child runs under the sandbox and the pack selection its outer run
/// installed, already reconciled with the child's workspace.
pub(crate) fn refuse_before_workflow_node(config: &Config) -> Result<(), ProfileToolRefusal> {
    super::admit_selected(&config.workspace_root, &config.profile, None).map(drop)
}
