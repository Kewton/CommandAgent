use std::path::{Path, PathBuf};

use anyhow::Context;

pub(super) struct SessionPaths {
    run_root: PathBuf,
}

impl SessionPaths {
    pub(super) fn new(workspace: &Path, id: &str) -> Self {
        Self {
            run_root: workspace.join(".anvil/runs").join(id),
        }
    }

    pub(super) fn state_root(&self) -> PathBuf {
        self.run_root.join("state")
    }

    pub(super) fn run_root(&self) -> &Path {
        &self.run_root
    }

    pub(super) fn confirmation_root(&self) -> PathBuf {
        self.state_root().join("boundary-confirmations")
    }

    pub(super) fn events_path(&self) -> PathBuf {
        self.run_root.join("events.jsonl")
    }

    pub(super) fn rollback_unstarted(&self) -> anyhow::Result<()> {
        match std::fs::remove_dir_all(&self.run_root) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error).with_context(|| {
                format!(
                    "remove unstarted session directory {}",
                    self.run_root.display()
                )
            }),
        }
    }
}

pub(super) fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub(super) fn proposal_confirmation_root(workspace: &Path) -> PathBuf {
    workspace.join(".anvil/gui-proposal-preview")
}
