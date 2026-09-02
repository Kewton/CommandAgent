use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Context, bail};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct TrialWorkspace {
    configured: Option<ConfiguredWorkspace>,
    lease: Arc<Mutex<LeaseState>>,
}

#[derive(Debug, Clone)]
struct ConfiguredWorkspace {
    requested: PathBuf,
    canonical: PathBuf,
    repository: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum LeaseSnapshot {
    Idle,
    Running { session_id: String },
    RecoveryRequired { session_id: String },
}

#[derive(Debug)]
enum LeaseState {
    Idle,
    Running(String),
    RecoveryRequired(String),
}

#[derive(Debug, Serialize)]
pub struct RuntimeStatus {
    pub trial_available: bool,
    pub trial_token_auth_enabled: bool,
    pub session: Option<RuntimeSession>,
}

#[derive(Debug, Serialize)]
pub struct RuntimeSession {
    pub id: String,
    pub state: &'static str,
}

impl TrialWorkspace {
    pub fn configure(repository: &Path, requested: Option<&Path>) -> anyhow::Result<Self> {
        let repository = repository
            .canonicalize()
            .with_context(|| format!("canonicalize repository root {}", repository.display()))?;
        let configured = requested
            .map(|requested| configure_workspace(&repository, requested))
            .transpose()?;
        let lease = configured
            .as_ref()
            .and_then(|workspace| unfinished_session(&workspace.canonical))
            .map(LeaseState::RecoveryRequired)
            .unwrap_or(LeaseState::Idle);
        Ok(Self {
            configured,
            lease: Arc::new(Mutex::new(lease)),
        })
    }

    pub fn is_enabled(&self) -> bool {
        self.configured.is_some()
    }

    pub fn configured_path(&self) -> Option<&Path> {
        self.configured
            .as_ref()
            .map(|workspace| workspace.canonical.as_path())
    }

    pub fn runtime_status(&self, trial_token_auth_enabled: bool) -> RuntimeStatus {
        let trial_available = self.require_current().is_ok();
        let session = self.lease.lock().ok().and_then(|lease| match &*lease {
            LeaseState::Idle => None,
            LeaseState::Running(id) => Some(RuntimeSession {
                id: id.clone(),
                state: "running",
            }),
            LeaseState::RecoveryRequired(id) => Some(RuntimeSession {
                id: id.clone(),
                state: "recovery_required",
            }),
        });
        RuntimeStatus {
            trial_available,
            trial_token_auth_enabled,
            session,
        }
    }

    pub fn require_current(&self) -> Result<PathBuf, String> {
        let configured = self
            .configured
            .as_ref()
            .ok_or_else(|| "trial execution is disabled; configure --execution-root".to_string())?;
        let current = configured.requested.canonicalize().map_err(|error| {
            format!(
                "trial workspace is unavailable at {}: {error}",
                configured.requested.display()
            )
        })?;
        require_directory(&current).map_err(|error| error.to_string())?;
        ensure_disjoint(&configured.repository, &current).map_err(|error| error.to_string())?;
        if current != configured.canonical {
            return Err(format!(
                "trial workspace changed after startup: expected {}, found {}",
                configured.canonical.display(),
                current.display()
            ));
        }
        Ok(current)
    }

    pub fn acquire(&self, session_id: &str) -> Result<(), String> {
        let mut lease = self
            .lease
            .lock()
            .map_err(|_| "trial workspace lease is poisoned".to_string())?;
        match &*lease {
            LeaseState::Idle => {
                *lease = LeaseState::Running(session_id.to_string());
                Ok(())
            }
            LeaseState::Running(active) => Err(format!(
                "trial workspace is already running session {active}"
            )),
            LeaseState::RecoveryRequired(active) => Err(format!(
                "trial workspace requires recovery for non-terminal session {active}"
            )),
        }
    }

    pub fn lease_snapshot(&self) -> Result<LeaseSnapshot, String> {
        let lease = self
            .lease
            .lock()
            .map_err(|_| "trial workspace lease is poisoned".to_string())?;
        Ok(match &*lease {
            LeaseState::Idle => LeaseSnapshot::Idle,
            LeaseState::Running(session_id) => LeaseSnapshot::Running {
                session_id: session_id.clone(),
            },
            LeaseState::RecoveryRequired(session_id) => LeaseSnapshot::RecoveryRequired {
                session_id: session_id.clone(),
            },
        })
    }

    pub fn require_running(&self, session_id: &str) -> Result<(), String> {
        let lease = self
            .lease
            .lock()
            .map_err(|_| "trial workspace lease is poisoned".to_string())?;
        match &*lease {
            LeaseState::Running(active) if active == session_id => Ok(()),
            LeaseState::Running(active) => Err(format!(
                "trial workspace is already running session {active}"
            )),
            LeaseState::RecoveryRequired(active) => Err(format!(
                "trial workspace requires recovery for non-terminal session {active}"
            )),
            LeaseState::Idle => Err("trial workspace has no running session".to_string()),
        }
    }

    pub fn cancel_start(&self, session_id: &str) {
        if let Ok(mut lease) = self.lease.lock()
            && matches!(&*lease, LeaseState::Running(active) if active == session_id)
        {
            *lease = LeaseState::Idle;
        }
    }

    pub fn complete_from_events(&self, session_id: &str, events_path: &Path) {
        self.complete_after_process(session_id, events_path, true);
    }

    pub fn complete_after_process(
        &self,
        session_id: &str,
        events_path: &Path,
        process_tree_gone: bool,
    ) {
        let terminal = current_terminal(events_path);
        if let Ok(mut lease) = self.lease.lock()
            && matches!(&*lease, LeaseState::Running(active) if active == session_id)
        {
            *lease = if terminal && process_tree_gone {
                LeaseState::Idle
            } else {
                LeaseState::RecoveryRequired(session_id.to_string())
            };
        }
    }
}

fn configure_workspace(repository: &Path, requested: &Path) -> anyhow::Result<ConfiguredWorkspace> {
    let requested = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        std::env::current_dir()
            .context("read current directory")?
            .join(requested)
    };
    let canonical = requested
        .canonicalize()
        .with_context(|| format!("canonicalize trial workspace {}", requested.display()))?;
    require_directory(&canonical)?;
    ensure_disjoint(repository, &canonical)?;
    Ok(ConfiguredWorkspace {
        requested,
        canonical,
        repository: repository.to_path_buf(),
    })
}

fn require_directory(path: &Path) -> anyhow::Result<()> {
    if !path.is_dir() {
        bail!(
            "trial workspace must be an existing directory: {}",
            path.display()
        );
    }
    Ok(())
}

pub(super) fn ensure_disjoint(repository: &Path, workspace: &Path) -> anyhow::Result<()> {
    if repository.starts_with(workspace) || workspace.starts_with(repository) {
        bail!(
            "trial workspace must be disjoint from repository root: repository={}, workspace={}",
            repository.display(),
            workspace.display()
        );
    }
    Ok(())
}

fn unfinished_session(workspace: &Path) -> Option<String> {
    for runs_dir in commandagent::runtime_paths::run_read_dirs(workspace) {
        let Ok(runs) = std::fs::read_dir(runs_dir) else {
            continue;
        };
        for entry in runs.flatten() {
            let path = entry.path();
            if !path.is_dir() || !path.join("state/boundary-confirmations").is_dir() {
                continue;
            }
            if !current_terminal(&path.join("events.jsonl")) {
                return Some(entry.file_name().to_string_lossy().into_owned());
            }
        }
    }
    None
}

fn current_terminal(events_path: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(events_path) else {
        return false;
    };
    let mut terminal = false;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            return false;
        };
        match event.get("event").and_then(Value::as_str) {
            Some("tui_command_stop" | "run_stop" | "gui_trial_stop_completed") => terminal = true,
            Some("human_directive_continuation_started") => terminal = false,
            _ => {}
        }
    }
    terminal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unverified_process_tree_keeps_recovery_required_even_with_terminal_evidence() {
        let temp = tempfile::tempdir().unwrap();
        let events = temp.path().join("events.jsonl");
        std::fs::write(
            &events,
            "{\"event\":\"gui_trial_stop_completed\",\"ok\":false,\"status\":\"recovery_required\"}\n",
        )
        .unwrap();
        let workspace = TrialWorkspace {
            configured: None,
            lease: Arc::new(Mutex::new(LeaseState::Running("session-408".to_string()))),
        };

        workspace.complete_after_process("session-408", &events, false);

        assert_eq!(
            workspace.lease_snapshot().unwrap(),
            LeaseSnapshot::RecoveryRequired {
                session_id: "session-408".to_string(),
            }
        );
    }
}
