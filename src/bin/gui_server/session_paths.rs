use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Component, Path as FilePath, PathBuf};

use anyhow::Context;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use super::AppState;
use super::error_response::GuiError;
use super::sessions::{not_found, require_session_id, require_trial};

pub(super) const SESSION_WORKSPACES_DIRECTORY: &str = "sessions";
const WORKING_DIRECTORY_BINDING: &str = "gui-working-directory.json";
const WORKING_DIRECTORY_BINDING_SCHEMA: &str = "commandagent.gui-working-directory/v1";

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
    selected_binding: Option<WorkingDirectoryBinding>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WorkingDirectoryBinding {
    schema_version: String,
    relative_path: String,
    canonical_path: PathBuf,
    device: Option<u64>,
    inode: Option<u64>,
}

impl SessionPaths {
    pub(super) fn new(
        workspace: &FilePath,
        id: &str,
        selected: Option<&str>,
    ) -> anyhow::Result<Self> {
        let selected_binding = selected
            .map(|selected| WorkingDirectoryBinding::resolve(workspace, selected))
            .transpose()?;
        let execution_workspace = selected_binding.as_ref().map_or_else(
            || workspace.join(SESSION_WORKSPACES_DIRECTORY).join(id),
            |binding| binding.canonical_path.clone(),
        );
        Ok(Self {
            run_root: commandagent::runtime_paths::runs_dir(workspace).join(id),
            execution_workspace,
            selected_binding,
        })
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
            let selected_binding = load_working_directory_binding(&run_root, workspace)?;
            let execution_workspace = selected_binding.as_ref().map_or_else(
                || workspace.join(SESSION_WORKSPACES_DIRECTORY).join(id),
                |binding| binding.canonical_path.clone(),
            );
            return Ok(Some(Self {
                run_root,
                execution_workspace,
                selected_binding,
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

    pub(super) fn gate_workspace<'a>(&'a self, execution_root: &'a FilePath) -> &'a FilePath {
        if self.selected_binding.is_some() {
            &self.execution_workspace
        } else {
            execution_root
        }
    }

    pub(super) fn persist_working_directory(&self) -> anyhow::Result<()> {
        let Some(binding) = self.selected_binding.as_ref() else {
            return Ok(());
        };
        binding.require_current()?;
        let path = self.state_root().join(WORKING_DIRECTORY_BINDING);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .with_context(|| format!("create working directory binding {}", path.display()))?;
        let mut bytes = serde_json::to_vec_pretty(binding)?;
        bytes.push(b'\n');
        file.write_all(&bytes)
            .with_context(|| format!("write working directory binding {}", path.display()))?;
        file.sync_all()
            .with_context(|| format!("sync working directory binding {}", path.display()))
    }

    pub(super) fn create_execution_workspace(&self) -> anyhow::Result<()> {
        if self.selected_binding.is_some() {
            self.require_execution_workspace()?;
            return Ok(());
        }
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
        if let Some(binding) = self.selected_binding.as_ref() {
            return binding.require_current();
        }
        if self.execution_workspace_state()? != WorkingDirectoryState::Available {
            anyhow::bail!(
                "session execution workspace is missing: {}",
                self.execution_workspace.display()
            );
        }
        require_canonical_real_directory(&self.execution_workspace)
    }

    pub(super) fn execution_workspace_state(&self) -> anyhow::Result<WorkingDirectoryState> {
        if let Some(binding) = self.selected_binding.as_ref() {
            return match std::fs::symlink_metadata(&binding.canonical_path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    Ok(WorkingDirectoryState::Missing)
                }
                Err(error) => Err(error).with_context(|| {
                    format!(
                        "inspect selected working directory {}",
                        binding.canonical_path.display()
                    )
                }),
                Ok(_) => binding
                    .require_current()
                    .map(|_| WorkingDirectoryState::Available),
            };
        }
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
        if self.selected_binding.is_some() {
            return Ok(());
        }
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

impl WorkingDirectoryBinding {
    fn resolve(execution_root: &FilePath, selected: &str) -> anyhow::Result<Self> {
        let relative = validate_relative_selection(selected)?;
        let canonical_path = require_selected_directory(execution_root, &relative)?;
        let metadata = std::fs::metadata(&canonical_path).with_context(|| {
            format!(
                "inspect selected working directory {}",
                canonical_path.display()
            )
        })?;
        let (device, inode) = filesystem_identity(&metadata);
        Ok(Self {
            schema_version: WORKING_DIRECTORY_BINDING_SCHEMA.to_string(),
            relative_path: relative.to_string_lossy().replace('\\', "/"),
            canonical_path,
            device,
            inode,
        })
    }

    fn require_current(&self) -> anyhow::Result<PathBuf> {
        let metadata = std::fs::symlink_metadata(&self.canonical_path).with_context(|| {
            format!(
                "inspect selected working directory {}",
                self.canonical_path.display()
            )
        })?;
        require_real_directory(&self.canonical_path, &metadata)?;
        let canonical = self.canonical_path.canonicalize().with_context(|| {
            format!(
                "canonicalize selected working directory {}",
                self.canonical_path.display()
            )
        })?;
        if canonical != self.canonical_path {
            anyhow::bail!(
                "selected working directory changed after confirmation: expected {}, found {}",
                self.canonical_path.display(),
                canonical.display()
            );
        }
        let metadata = std::fs::metadata(&canonical)?;
        let (device, inode) = filesystem_identity(&metadata);
        if self.device.is_some_and(|expected| Some(expected) != device)
            || self.inode.is_some_and(|expected| Some(expected) != inode)
        {
            anyhow::bail!(
                "selected working directory was replaced after confirmation: {}",
                self.canonical_path.display()
            );
        }
        Ok(canonical)
    }
}

fn validate_relative_selection(selected: &str) -> anyhow::Result<PathBuf> {
    let selected = selected.trim();
    if selected.is_empty() {
        anyhow::bail!("working_directory must be omitted or name an existing relative directory");
    }
    let relative = FilePath::new(selected);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        anyhow::bail!("working_directory must be a traversal-free relative path");
    }
    let first = relative
        .components()
        .next()
        .and_then(|component| match component {
            Component::Normal(value) => value.to_str(),
            _ => None,
        });
    if matches!(
        first,
        Some(
            SESSION_WORKSPACES_DIRECTORY
                | commandagent::runtime_paths::WORKSPACE_DIR
                | commandagent::runtime_paths::LEGACY_WORKSPACE_DIR
        )
    ) {
        anyhow::bail!("working_directory cannot select GUI runtime directories");
    }
    Ok(relative.to_path_buf())
}

fn require_selected_directory(
    execution_root: &FilePath,
    relative: &FilePath,
) -> anyhow::Result<PathBuf> {
    let requested = execution_root.join(relative);
    let metadata = std::fs::symlink_metadata(&requested)
        .with_context(|| format!("inspect selected working directory {}", requested.display()))?;
    require_real_directory(&requested, &metadata)?;
    let canonical = requested.canonicalize().with_context(|| {
        format!(
            "canonicalize selected working directory {}",
            requested.display()
        )
    })?;
    if canonical != requested || !canonical.starts_with(execution_root) {
        anyhow::bail!("working_directory must resolve without symlinks below --execution-root");
    }
    Ok(canonical)
}

fn load_working_directory_binding(
    run_root: &FilePath,
    execution_root: &FilePath,
) -> anyhow::Result<Option<WorkingDirectoryBinding>> {
    let path = run_root.join("state").join(WORKING_DIRECTORY_BINDING);
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("inspect working directory binding {}", path.display()));
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > 16 * 1024 {
        anyhow::bail!("working directory binding is not a safe regular file");
    }
    let binding: WorkingDirectoryBinding = serde_json::from_slice(
        &std::fs::read(&path)
            .with_context(|| format!("read working directory binding {}", path.display()))?,
    )
    .with_context(|| format!("parse working directory binding {}", path.display()))?;
    if binding.schema_version != WORKING_DIRECTORY_BINDING_SCHEMA {
        anyhow::bail!("unsupported working directory binding schema");
    }
    let relative = validate_relative_selection(&binding.relative_path)?;
    let expected = execution_root.join(relative);
    if expected != binding.canonical_path || !expected.starts_with(execution_root) {
        anyhow::bail!("working directory binding escaped or changed execution root");
    }
    Ok(Some(binding))
}

#[cfg(unix)]
fn filesystem_identity(metadata: &std::fs::Metadata) -> (Option<u64>, Option<u64>) {
    use std::os::unix::fs::MetadataExt;

    (Some(metadata.dev()), Some(metadata.ino()))
}

#[cfg(not(unix))]
fn filesystem_identity(_: &std::fs::Metadata) -> (Option<u64>, Option<u64>) {
    (None, None)
}

pub(super) fn selected_gate_workspace(
    execution_root: &FilePath,
    selected: Option<&str>,
) -> anyhow::Result<PathBuf> {
    selected.map_or_else(
        || Ok(execution_root.to_path_buf()),
        |selected| {
            WorkingDirectoryBinding::resolve(execution_root, selected)
                .map(|value| value.canonical_path)
        },
    )
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
