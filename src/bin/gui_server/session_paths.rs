use std::path::{Path as FilePath, PathBuf};

use anyhow::Context;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use super::AppState;
use super::error_response::GuiError;
use super::sessions::{not_found, require_session_id, require_trial};

pub(super) const SESSION_WORKSPACES_DIRECTORY: &str = "sessions";

#[derive(Debug, Serialize)]
pub(super) struct SessionPathProjection {
    id: String,
    working_directory: WorkingDirectoryProjection,
    run_records: RunRecordPaths,
}

#[derive(Debug, Serialize)]
pub(super) struct WorkingDirectoryProjection {
    path: String,
    state: WorkingDirectoryState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum WorkingDirectoryState {
    Available,
    Missing,
}

#[derive(Debug, Serialize)]
pub(super) struct RunRecordPaths {
    directory: String,
    events: String,
    summary: String,
}

pub(super) struct SessionPaths {
    run_root: PathBuf,
    execution_workspace: PathBuf,
}

impl SessionPaths {
    pub(super) fn new(workspace: &FilePath, id: &str) -> Self {
        Self {
            run_root: commandagent::runtime_paths::runs_dir(workspace).join(id),
            execution_workspace: workspace.join(SESSION_WORKSPACES_DIRECTORY).join(id),
        }
    }

    pub(super) fn existing(workspace: &FilePath, id: &str) -> anyhow::Result<Option<Self>> {
        for runs_root in commandagent::runtime_paths::run_read_dirs(workspace) {
            let run_root = runs_root.join(id);
            let metadata = match std::fs::symlink_metadata(&run_root) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("inspect session run {}", run_root.display()));
                }
            };
            require_real_directory(&run_root, &metadata)?;
            let runtime_root = runs_root
                .parent()
                .expect("runtime runs directories always have a parent");
            require_canonical_real_directory(runtime_root)?;
            require_canonical_real_directory(&runs_root)?;
            require_canonical_real_directory(&run_root)?;
            return Ok(Some(Self {
                run_root,
                execution_workspace: workspace.join(SESSION_WORKSPACES_DIRECTORY).join(id),
            }));
        }
        Ok(None)
    }

    pub(super) fn state_root(&self) -> PathBuf {
        self.run_root.join("state")
    }

    pub(super) fn run_root(&self) -> &FilePath {
        &self.run_root
    }

    pub(super) fn confirmation_root(&self) -> PathBuf {
        self.state_root().join("boundary-confirmations")
    }

    pub(super) fn events_path(&self) -> PathBuf {
        self.run_root.join("events.jsonl")
    }

    pub(super) fn execution_workspace(&self) -> &FilePath {
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
        if self.execution_workspace_state()? != WorkingDirectoryState::Available {
            anyhow::bail!(
                "session execution workspace is missing: {}",
                self.execution_workspace.display()
            );
        }
        require_canonical_real_directory(&self.execution_workspace)
    }

    pub(super) fn execution_workspace_state(&self) -> anyhow::Result<WorkingDirectoryState> {
        let sessions_root = self
            .execution_workspace
            .parent()
            .expect("session workspaces always have a parent");
        match std::fs::symlink_metadata(sessions_root) {
            Ok(metadata) => require_real_directory(sessions_root, &metadata)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(WorkingDirectoryState::Missing);
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("inspect session workspace root {}", sessions_root.display())
                });
            }
        }
        require_canonical_real_directory(sessions_root)?;
        let metadata = match std::fs::symlink_metadata(&self.execution_workspace) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(WorkingDirectoryState::Missing);
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "inspect session execution workspace {}",
                        self.execution_workspace.display()
                    )
                });
            }
        };
        require_real_directory(&self.execution_workspace, &metadata)?;
        require_canonical_real_directory(&self.execution_workspace)?;
        Ok(WorkingDirectoryState::Available)
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

pub(super) async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, GuiError> {
    let workspace = require_trial(&state, &headers, false)?;
    require_session_id(&id)?;
    let paths = SessionPaths::existing(&workspace, &id)
        .map_err(|_| not_found("session run path is not safely readable"))?
        .ok_or_else(|| not_found("session run was not found"))?;
    let working_directory_state = paths
        .execution_workspace_state()
        .map_err(|_| not_found("session working directory is not safely readable"))?;
    let projection = SessionPathProjection {
        id,
        working_directory: WorkingDirectoryProjection {
            path: absolute_path(paths.execution_workspace()),
            state: working_directory_state,
        },
        run_records: RunRecordPaths {
            directory: absolute_path(paths.run_root()),
            events: absolute_path(&paths.events_path()),
            summary: absolute_path(&paths.run_root().join("summary.md")),
        },
    };
    let mut response = Json(projection).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    Ok(response)
}

fn require_real_directory(path: &FilePath, metadata: &std::fs::Metadata) -> anyhow::Result<()> {
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        anyhow::bail!(
            "session workspace path must be a real directory: {}",
            path.display()
        );
    }
    Ok(())
}

fn require_canonical_real_directory(path: &FilePath) -> anyhow::Result<PathBuf> {
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

fn absolute_path(path: &FilePath) -> String {
    path.to_string_lossy().to_string()
}

pub(super) fn relative_path(root: &FilePath, path: &FilePath) -> String {
    path.strip_prefix(root)
        .unwrap_or_else(|_| FilePath::new("<outside-execution-root>"))
        .to_string_lossy()
        .replace('\\', "/")
}

pub(super) fn proposal_confirmation_root(workspace: &FilePath) -> PathBuf {
    commandagent::runtime_paths::workspace_dir(workspace).join("gui-proposal-preview")
}
