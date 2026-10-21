//! Validate the restoration source before the copying API can change control.
use std::path::Path;

use crate::planner::recovery_snapshot::{RecoveryBoundarySnapshot, current_source_sha256};

pub(super) fn validate_source(
    root: &Path,
    checkpoint: &RecoveryBoundarySnapshot,
) -> anyhow::Result<()> {
    let root = root.canonicalize()?;
    let source = root
        .join(&checkpoint.workspace_relative_path)
        .canonicalize()?;
    anyhow::ensure!(
        source.starts_with(root.join(".commandagent/recovery-boundaries")),
        "Recovery restoration source escaped the boundary namespace"
    );
    anyhow::ensure!(
        current_source_sha256(&source)? == checkpoint.snapshot_sha256,
        "Recovery restoration source differs from original checkpoint"
    );
    Ok(())
}
