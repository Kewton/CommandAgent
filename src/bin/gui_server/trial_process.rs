use std::path::Path;
use std::process::{Child, Command, ExitStatus};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use axum::Json;
use axum::extract::{Path as RoutePath, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use super::AppState;
use super::session_paths::SessionPaths;
use super::sessions::{
    SessionError, internal, require_current_active, require_session_id, require_trial,
    session_conflict, workspace_conflict,
};

const INTERRUPT_GRACE: Duration = Duration::from_secs(2);
const PROCESS_TREE_VERIFY: Duration = Duration::from_secs(2);
const PROCESS_TREE_POLL: Duration = Duration::from_millis(20);

#[derive(Clone, Debug, Default)]
pub struct TrialProcesses {
    active: Arc<Mutex<Option<ActiveProcess>>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessIdentity {
    session_id: String,
    generation: String,
    pid: u32,
    process_group: i32,
}

#[derive(Clone, Debug)]
struct ActiveProcess {
    identity: ProcessIdentity,
    events_path: std::path::PathBuf,
    stop: StopState,
}

#[derive(Clone, Debug)]
enum StopState {
    Running,
    Requested { requested_at: Instant, forced: bool },
}

#[derive(Debug)]
pub struct ProcessCompletion {
    pub status: ExitStatus,
    pub process_tree_gone: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StopRequest {
    generation: String,
}

#[derive(Debug, Serialize)]
pub struct StopResponse {
    session_id: String,
    process_generation: String,
    status: &'static str,
}

enum StopAcceptance {
    Requested(ProcessIdentity),
    AlreadyRequested(ProcessIdentity),
}

impl TrialProcesses {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_generation() -> String {
        Uuid::now_v7().to_string()
    }

    pub fn prepare_command(command: &mut Command) {
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
    }

    pub fn register(
        &self,
        session_id: &str,
        generation: &str,
        mut child: Child,
        events_path: &Path,
    ) -> anyhow::Result<(ProcessIdentity, Child)> {
        require_generation(generation)?;
        let process_group = i32::try_from(child.id()).context("delegated CLI PID exceeds i32")?;
        let identity = ProcessIdentity {
            session_id: session_id.to_string(),
            generation: generation.to_string(),
            pid: child.id(),
            process_group,
        };
        let mut active = match self.active.lock() {
            Ok(active) => active,
            Err(_) => {
                terminate_unregistered(&mut child, process_group);
                bail!("GUI Trial process registry is poisoned");
            }
        };
        if let Some(existing) = active.as_ref() {
            let message = format!(
                "GUI Trial process registry already owns session {} generation {}",
                existing.identity.session_id, existing.identity.generation
            );
            terminate_unregistered(&mut child, process_group);
            bail!(message);
        }
        *active = Some(ActiveProcess {
            identity: identity.clone(),
            events_path: events_path.to_path_buf(),
            stop: StopState::Running,
        });
        Ok((identity, child))
    }

    pub fn generation_for(&self, session_id: &str) -> Option<String> {
        self.active.lock().ok().and_then(|active| {
            active
                .as_ref()
                .filter(|active| active.identity.session_id == session_id)
                .map(|active| active.identity.generation.clone())
        })
    }

    pub fn wait(
        &self,
        identity: &ProcessIdentity,
        mut child: Child,
    ) -> anyhow::Result<ProcessCompletion> {
        let status = match child.wait().context("wait for delegated CLI") {
            Ok(status) => status,
            Err(error) => {
                self.finish(identity);
                return Err(error);
            }
        };
        let stop = self.stop_snapshot(identity);
        let process_tree_gone = match stop.as_ref() {
            Some(StopState::Requested { requested_at, .. }) => {
                let remaining_grace = requested_at
                    .checked_add(INTERRUPT_GRACE)
                    .and_then(|deadline| deadline.checked_duration_since(Instant::now()))
                    .unwrap_or_default();
                wait_for_process_group_exit(
                    identity.process_group,
                    remaining_grace + PROCESS_TREE_VERIFY,
                )
            }
            _ => wait_for_process_group_exit(identity.process_group, PROCESS_TREE_VERIFY),
        };
        if matches!(stop, Some(StopState::Requested { .. })) {
            let forced = matches!(
                self.stop_snapshot(identity),
                Some(StopState::Requested { forced: true, .. })
            );
            let cli_terminal_observed = self
                .events_path(identity)
                .is_some_and(|path| current_cli_terminal(&path));
            emit_stop_completed(
                self.events_path(identity),
                identity,
                forced,
                process_tree_gone,
                cli_terminal_observed,
                &status,
            );
        }
        self.finish(identity);
        Ok(ProcessCompletion {
            status,
            process_tree_gone,
        })
    }

    fn request_stop(
        &self,
        session_id: &str,
        generation: &str,
    ) -> Result<StopAcceptance, SessionError> {
        require_generation(generation).map_err(|_| {
            session_conflict("the requested process generation is invalid or stale")
        })?;
        let acceptance = {
            let mut active = self
                .active
                .lock()
                .map_err(|_| internal("GUI Trial process registry is unavailable"))?;
            let process = active.as_mut().ok_or_else(|| {
                session_conflict("the running process is not owned by this GUI server instance")
            })?;
            if process.identity.session_id != session_id
                || process.identity.generation != generation
            {
                return Err(session_conflict(
                    "the requested session or process generation is not active",
                ));
            }
            match process.stop {
                StopState::Running => {
                    process.stop = StopState::Requested {
                        requested_at: Instant::now(),
                        forced: false,
                    };
                    StopAcceptance::Requested(process.identity.clone())
                }
                StopState::Requested { .. } => {
                    StopAcceptance::AlreadyRequested(process.identity.clone())
                }
            }
        };
        if let StopAcceptance::Requested(identity) = &acceptance {
            emit_stop_requested(self.events_path(identity), identity);
            send_group_signal(identity.process_group, Signal::Interrupt).map_err(internal)?;
            self.start_force_watchdog(identity.clone());
        }
        Ok(acceptance)
    }

    fn start_force_watchdog(&self, identity: ProcessIdentity) {
        let processes = self.clone();
        std::thread::spawn(move || {
            std::thread::sleep(INTERRUPT_GRACE);
            if !processes.is_stop_requested(&identity)
                || !process_group_exists(identity.process_group)
            {
                return;
            }
            processes.mark_forced(&identity);
            if let Err(error) = send_group_signal(identity.process_group, Signal::Kill) {
                eprintln!(
                    "GUI Trial forced stop failed for session {} generation {}: {error:#}",
                    identity.session_id, identity.generation
                );
            }
        });
    }

    fn is_stop_requested(&self, identity: &ProcessIdentity) -> bool {
        self.active.lock().ok().is_some_and(|active| {
            active.as_ref().is_some_and(|active| {
                active.identity == *identity && matches!(active.stop, StopState::Requested { .. })
            })
        })
    }

    fn mark_forced(&self, identity: &ProcessIdentity) {
        if let Ok(mut active) = self.active.lock()
            && let Some(active) = active.as_mut()
            && active.identity == *identity
            && let StopState::Requested { forced, .. } = &mut active.stop
        {
            *forced = true;
        }
    }

    fn stop_snapshot(&self, identity: &ProcessIdentity) -> Option<StopState> {
        self.active.lock().ok().and_then(|active| {
            active
                .as_ref()
                .filter(|active| active.identity == *identity)
                .map(|active| active.stop.clone())
        })
    }

    fn events_path(&self, identity: &ProcessIdentity) -> Option<std::path::PathBuf> {
        self.active.lock().ok().and_then(|active| {
            active
                .as_ref()
                .filter(|active| active.identity == *identity)
                .map(|active| active.events_path.clone())
        })
    }

    fn finish(&self, identity: &ProcessIdentity) {
        if let Ok(mut active) = self.active.lock()
            && active
                .as_ref()
                .is_some_and(|active| active.identity == *identity)
        {
            *active = None;
        }
    }
}

pub async fn stop(
    State(state): State<AppState>,
    RoutePath(id): RoutePath<String>,
    headers: HeaderMap,
    Json(request): Json<StopRequest>,
) -> Result<impl IntoResponse, SessionError> {
    let workspace = require_trial(&state, &headers, true)?;
    require_session_id(&id)?;
    let paths = SessionPaths::existing(&workspace, &id)
        .map_err(|_| super::sessions::not_found("session run path is not safely readable"))?
        .ok_or_else(|| super::sessions::not_found("session run was not found"))?;
    if state.trial_processes.generation_for(&id).as_deref() != Some(request.generation.as_str()) {
        require_current_active(&paths.events_path()).await?;
    }
    state
        .trial_workspace
        .require_running(&id)
        .map_err(workspace_conflict)?;
    let acceptance = state
        .trial_processes
        .request_stop(&id, &request.generation)?;
    let (identity, status) = match acceptance {
        StopAcceptance::Requested(identity) => (identity, "stopping"),
        StopAcceptance::AlreadyRequested(identity) => (identity, "already_stopping"),
    };
    Ok((
        StatusCode::ACCEPTED,
        Json(StopResponse {
            session_id: identity.session_id,
            process_generation: identity.generation,
            status,
        }),
    ))
}

fn require_generation(generation: &str) -> anyhow::Result<()> {
    let parsed = Uuid::parse_str(generation).context("invalid process generation")?;
    if parsed.to_string() != generation {
        bail!("invalid process generation");
    }
    Ok(())
}

fn current_cli_terminal(events_path: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(events_path) else {
        return false;
    };
    let mut terminal = false;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            return false;
        };
        match event.get("event").and_then(serde_json::Value::as_str) {
            Some("tui_command_stop") => terminal = true,
            Some("human_directive_continuation_started") => terminal = false,
            Some("tui_command_start") if terminal => terminal = false,
            _ => {}
        }
    }
    terminal
}

fn emit_stop_requested(events_path: Option<std::path::PathBuf>, identity: &ProcessIdentity) {
    commandagent::eval_events::emit(
        events_path.as_deref(),
        json!({
            "event": "gui_trial_stop_requested",
            "lifecycle_stage": "gui_trial_process",
            "session_id": identity.session_id,
            "process_generation": identity.generation,
            "pid": identity.pid,
            "process_group": identity.process_group,
            "signal": "SIGINT",
            "ok": false,
        }),
    );
}

fn emit_stop_completed(
    events_path: Option<std::path::PathBuf>,
    identity: &ProcessIdentity,
    forced: bool,
    process_tree_gone: bool,
    cli_terminal_observed: bool,
    status: &ExitStatus,
) {
    commandagent::eval_events::emit(
        events_path.as_deref(),
        json!({
            "event": "gui_trial_stop_completed",
            "lifecycle_stage": "gui_trial_process",
            "session_id": identity.session_id,
            "process_generation": identity.generation,
            "pid": identity.pid,
            "process_group": identity.process_group,
            "signal": if forced { "SIGKILL" } else { "SIGINT" },
            "forced": forced,
            "process_tree_gone": process_tree_gone,
            "cli_terminal_observed": cli_terminal_observed,
            "exit_code": status.code(),
            "ok": false,
            "status": if process_tree_gone { "interrupted" } else { "recovery_required" },
            "failure_kind": if process_tree_gone {
                if forced { "gui_trial_stop_forced" } else { "gui_trial_stop_interrupted" }
            } else {
                "gui_trial_stop_process_tree_unverified"
            },
            "stop_reason": if process_tree_gone {
                if forced {
                    "GUI stop grace period expired; the delegated process group was forcibly terminated"
                } else {
                    "interrupted by GUI operator"
                }
            } else {
                "GUI stop could not verify that the delegated process group exited"
            },
            "next_action": if process_tree_gone { "resume_or_rerun_command" } else { "inspect_process_tree" },
        }),
    );
}

#[derive(Clone, Copy)]
enum Signal {
    Interrupt,
    Kill,
}

#[cfg(unix)]
fn send_group_signal(process_group: i32, signal: Signal) -> anyhow::Result<()> {
    let signal = match signal {
        Signal::Interrupt => libc::SIGINT,
        Signal::Kill => libc::SIGKILL,
    };
    let result = unsafe { libc::kill(-process_group, signal) };
    if result == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        return Ok(());
    }
    Err(error).context("signal delegated CLI process group")
}

#[cfg(not(unix))]
fn send_group_signal(_process_group: i32, _signal: Signal) -> anyhow::Result<()> {
    bail!("GUI Trial stop requires Unix process-group signals")
}

#[cfg(unix)]
fn process_group_exists(process_group: i32) -> bool {
    let result = unsafe { libc::kill(-process_group, 0) };
    if result == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
}

#[cfg(not(unix))]
fn process_group_exists(_process_group: i32) -> bool {
    false
}

fn wait_for_process_group_exit(process_group: i32, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if !process_group_exists(process_group) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(PROCESS_TREE_POLL);
    }
}

fn terminate_unregistered(child: &mut Child, process_group: i32) {
    let _ = send_group_signal(process_group, Signal::Kill);
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_must_be_canonical_uuid() {
        let generation = TrialProcesses::new_generation();
        assert!(require_generation(&generation).is_ok());
        assert!(require_generation(&generation.to_uppercase()).is_err());
        assert!(require_generation("stale-pid-42").is_err());
    }
}
