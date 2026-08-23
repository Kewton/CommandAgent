use std::path::{Path, PathBuf};

use anyhow::Context;

pub(super) const SESSION_WORKSPACES_DIRECTORY: &str = "sessions";

pub(super) struct SessionPaths {
    run_root: PathBuf,
    execution_workspace: PathBuf,
}

impl SessionPaths {
    pub(super) fn new(workspace: &Path, id: &str) -> Self {
        Self {
            run_root: commandagent::runtime_paths::runs_dir(workspace).join(id),
            execution_workspace: workspace.join(SESSION_WORKSPACES_DIRECTORY).join(id),
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

    pub(super) fn execution_workspace(&self) -> &Path {
        &self.execution_workspace
    }

    pub(super) fn create_execution_workspace(&self) -> anyhow::Result<()> {
        let sessions_root = self
            .execution_workspace
            .parent()
            .expect("session workspaces always have a parent");
        match std::fs::symlink_metadata(sessions_root) {
            Ok(metadata) => require_real_directory(sessions_root, &metadata)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match std::fs::create_dir(sessions_root) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                        let metadata =
                            std::fs::symlink_metadata(sessions_root).with_context(|| {
                                format!(
                                    "inspect session workspace root {}",
                                    sessions_root.display()
                                )
                            })?;
                        require_real_directory(sessions_root, &metadata)?;
                    }
                    Err(error) => {
                        return Err(error).with_context(|| {
                            format!("create session workspace root {}", sessions_root.display())
                        });
                    }
                }
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("inspect session workspace root {}", sessions_root.display())
                });
            }
        }
        require_canonical_real_directory(sessions_root)?;
        std::fs::create_dir(&self.execution_workspace).with_context(|| {
            format!(
                "create session execution workspace {}",
                self.execution_workspace.display()
            )
        })?;
        if let Err(error) = self.require_execution_workspace() {
            return match self.rollback_execution_workspace() {
                Ok(()) => Err(error),
                Err(rollback_error) => Err(anyhow::anyhow!(
                    "{error:#}; failed to roll back invalid session execution workspace: {rollback_error:#}"
                )),
            };
        }
        Ok(())
    }

    pub(super) fn require_execution_workspace(&self) -> anyhow::Result<PathBuf> {
        let sessions_root = self
            .execution_workspace
            .parent()
            .expect("session workspaces always have a parent");
        require_canonical_real_directory(sessions_root)?;
        require_canonical_real_directory(&self.execution_workspace)
    }

    pub(super) fn rollback_execution_workspace(&self) -> anyhow::Result<()> {
        let sessions_root = self
            .execution_workspace
            .parent()
            .expect("session workspaces always have a parent");
        require_canonical_real_directory(sessions_root)?;
        std::fs::remove_dir(&self.execution_workspace).with_context(|| {
            format!(
                "remove unstarted session execution workspace {}",
                self.execution_workspace.display()
            )
        })
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

fn require_real_directory(path: &Path, metadata: &std::fs::Metadata) -> anyhow::Result<()> {
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        anyhow::bail!(
            "session workspace path must be a real directory: {}",
            path.display()
        );
    }
    Ok(())
}

fn require_canonical_real_directory(path: &Path) -> anyhow::Result<PathBuf> {
    let metadata = std::fs::symlink_metadata(path)
        .with_context(|| format!("inspect session workspace path {}", path.display()))?;
    require_real_directory(path, &metadata)?;
    let canonical = path
        .canonicalize()
        .with_context(|| format!("canonicalize session workspace path {}", path.display()))?;
    if canonical != path {
        anyhow::bail!(
            "session workspace path changed after configuration: expected {}, found {}",
            path.display(),
            canonical.display()
        );
    }
    Ok(canonical)
}

pub(super) fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub(super) fn proposal_confirmation_root(workspace: &Path) -> PathBuf {
    commandagent::runtime_paths::workspace_dir(workspace).join("gui-proposal-preview")
}
