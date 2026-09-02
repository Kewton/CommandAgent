use std::collections::HashSet;
use std::path::Path;
use std::time::UNIX_EPOCH;

use axum::Json;
use axum::extract::State;
use axum::http::HeaderMap;
use commandagent::planner::pack::catalog::PackSource;
use commandagent::tui::boundary_shell::confirmation::{
    ConfirmationIdentity, PackSelection, load_latest_confirmation,
};
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use super::AppState;
use super::session_diagnostics::{FailureDiagnostics, project as project_diagnostics};
use super::sessions::{SessionError, internal, require_trial, started_epoch_seconds};
use super::workspace_policy::LeaseSnapshot;

const MAX_SESSIONS: usize = 100;
const MAX_EVENTS_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Serialize)]
pub struct SessionIndex {
    sessions: Vec<SessionSummary>,
    lease: LeaseSnapshot,
}

#[derive(Debug, Serialize)]
struct SessionSummary {
    id: String,
    started_epoch_seconds: u64,
    modified_epoch_seconds: u64,
    gate: Option<&'static str>,
    status: String,
    profile: Option<String>,
    intent: Option<String>,
    failure_diagnostics: FailureDiagnostics,
    pack: Option<SessionPack>,
}

#[derive(Debug, Serialize)]
struct SessionPack {
    id: String,
    version: String,
    hash: String,
    source: PackSource,
    source_label: &'static str,
}

#[derive(Debug)]
struct SessionProjection {
    gate: Option<&'static str>,
    status: String,
    failure_diagnostics: FailureDiagnostics,
}

pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SessionIndex>, SessionError> {
    let workspace = require_trial(&state, &headers, false)?;
    let lease = state.trial_workspace.lease_snapshot().map_err(internal)?;
    let mut sessions = Vec::new();
    let mut seen = HashSet::new();
    for runs_root in commandagent::runtime_paths::run_read_dirs(&workspace) {
        let mut entries = match tokio::fs::read_dir(&runs_root).await {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(internal(format!("read {}: {error}", runs_root.display()))),
        };
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|error| internal(format!("read {}: {error}", runs_root.display())))?
        {
            let path = entry.path();
            let file_type = entry
                .file_type()
                .await
                .map_err(|error| internal(format!("inspect {}: {error}", path.display())))?;
            if !file_type.is_dir() || file_type.is_symlink() {
                continue;
            }
            let Some(id) = canonical_session_id(&entry.file_name().to_string_lossy()) else {
                continue;
            };
            let confirmation_root = path.join("state/boundary-confirmations");
            if !has_confirmation_record(&confirmation_root).await {
                continue;
            }
            if !seen.insert(id.clone()) {
                continue;
            }
            let events_path = path.join("events.jsonl");
            let projection = session_projection(&events_path, &workspace).await;
            let started_epoch_seconds = started_epoch_seconds(&id, &path, &events_path).await;
            let confirmed = load_latest_confirmation(&confirmation_root).ok().flatten();
            sessions.push(SessionSummary {
                id,
                started_epoch_seconds,
                modified_epoch_seconds: modified_epoch_seconds(&path, &events_path).await,
                gate: projection.gate,
                status: projection.status,
                profile: confirmed
                    .as_ref()
                    .map(|record| record.identity().profile.clone()),
                intent: confirmed
                    .as_ref()
                    .map(|record| record.identity().intent.clone()),
                failure_diagnostics: projection.failure_diagnostics,
                pack: confirmed
                    .as_ref()
                    .and_then(|record| confirmed_pack(record.identity())),
            });
        }
    }
    let active_session = match &lease {
        LeaseSnapshot::Idle => None,
        LeaseSnapshot::Running { session_id } | LeaseSnapshot::RecoveryRequired { session_id } => {
            Some(session_id.as_str())
        }
    };
    sessions.sort_by(|left, right| {
        let left_is_active = active_session == Some(left.id.as_str());
        let right_is_active = active_session == Some(right.id.as_str());
        right_is_active
            .cmp(&left_is_active)
            .then_with(|| {
                right
                    .modified_epoch_seconds
                    .cmp(&left.modified_epoch_seconds)
            })
            .then_with(|| right.id.cmp(&left.id))
    });
    sessions.truncate(MAX_SESSIONS);
    Ok(Json(SessionIndex { sessions, lease }))
}

fn confirmed_pack(identity: &ConfirmationIdentity) -> Option<SessionPack> {
    let PackSelection::Pinned {
        id,
        version,
        hash,
        source,
        ..
    } = &identity.pack
    else {
        return None;
    };
    Some(SessionPack {
        id: id.clone(),
        version: version.clone(),
        hash: hash.clone(),
        source: *source,
        source_label: source.japanese_label(),
    })
}

fn canonical_session_id(value: &str) -> Option<String> {
    let id = Uuid::parse_str(value).ok()?;
    (id.to_string() == value).then(|| value.to_string())
}

async fn has_confirmation_record(root: &Path) -> bool {
    if !tokio::fs::symlink_metadata(root)
        .await
        .is_ok_and(|metadata| metadata.file_type().is_dir())
    {
        return false;
    }
    let Ok(mut entries) = tokio::fs::read_dir(root).await else {
        return false;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) == Some("json")
            && entry
                .file_type()
                .await
                .is_ok_and(|file_type| file_type.is_file() && !file_type.is_symlink())
        {
            return true;
        }
    }
    false
}

async fn modified_epoch_seconds(run_root: &Path, events_path: &Path) -> u64 {
    let mut modified = metadata_modified(run_root).await;
    modified = modified.max(metadata_modified(events_path).await);
    modified
}

async fn metadata_modified(path: &Path) -> u64 {
    tokio::fs::symlink_metadata(path)
        .await
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

async fn session_projection(events_path: &Path, execution_root: &Path) -> SessionProjection {
    let metadata = match tokio::fs::symlink_metadata(events_path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return SessionProjection {
                gate: Some("gate_2"),
                status: "starting".to_string(),
                failure_diagnostics: FailureDiagnostics::default(),
            };
        }
        Err(_) => return unreadable_projection(),
    };
    if !metadata.file_type().is_file() || metadata.len() > MAX_EVENTS_BYTES {
        return unreadable_projection();
    }
    let Ok(text) = tokio::fs::read_to_string(events_path).await else {
        return unreadable_projection();
    };
    let mut saw_event = false;
    let mut terminal = None;
    let mut continuation_index = None;
    let mut events = Vec::new();
    for (index, line) in text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .enumerate()
    {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            return unreadable_projection();
        };
        saw_event = true;
        match event.get("event").and_then(Value::as_str) {
            Some("tui_command_stop" | "gui_trial_stop_completed") => {
                let status = recorded_status(&event);
                terminal = Some((
                    index,
                    status.clone(),
                    full_terminal_without_sheet(&event, status.as_deref()),
                ));
            }
            Some("human_directive_continuation_started") => continuation_index = Some(index),
            _ => {}
        }
        events.push(event);
    }
    if !saw_event {
        return SessionProjection {
            gate: Some("gate_2"),
            status: "starting".to_string(),
            failure_diagnostics: FailureDiagnostics::default(),
        };
    }
    let terminal_is_current = terminal.as_ref().is_some_and(|(terminal_index, _, _)| {
        continuation_index.is_none_or(|index| *terminal_index > index)
    });
    if !terminal_is_current {
        return SessionProjection {
            gate: Some("gate_2"),
            status: "running".to_string(),
            failure_diagnostics: FailureDiagnostics::default(),
        };
    }
    let full = terminal.as_ref().is_some_and(|(_, _, full)| *full);
    let current_event_start = continuation_index.map_or(0, |index| index + 1);
    let current_events = &events[current_event_start..];
    let run_stop_status = current_events
        .iter()
        .rev()
        .find(|event| event.get("event").and_then(Value::as_str) == Some("run_stop"))
        .and_then(recorded_status);
    let status = terminal
        .and_then(|(_, status, _)| status)
        .or(run_stop_status)
        .unwrap_or_else(|| "running".to_string());
    let mut failure_diagnostics = project_diagnostics(current_events);
    failure_diagnostics.redact_execution_root(execution_root);
    SessionProjection {
        gate: Some(if full { "gate_3" } else { "gate_4" }),
        status,
        failure_diagnostics,
    }
}

fn unreadable_projection() -> SessionProjection {
    SessionProjection {
        gate: None,
        status: "unreadable".to_string(),
        failure_diagnostics: FailureDiagnostics::default(),
    }
}

fn full_terminal_without_sheet(event: &Value, status: Option<&str>) -> bool {
    event.get("ok").and_then(Value::as_bool) == Some(true)
        && status == Some("completed")
        && event.get("assurance_level").and_then(Value::as_str) == Some("full")
        && matches!(
            event.get("final_acceptance_status").and_then(Value::as_str),
            Some("full" | "full_success" | "completed")
        )
}

fn recorded_status(event: &Value) -> Option<String> {
    event
        .get("status")
        .and_then(Value::as_str)
        .filter(|status| {
            !status.is_empty()
                && status.len() <= 64
                && status
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        })
        .map(str::to_string)
}
