use std::path::Path;
use std::time::UNIX_EPOCH;

use axum::Json;
use axum::extract::State;
use axum::http::HeaderMap;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use super::AppState;
use super::sessions::{SessionError, internal, require_trial};
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
    modified_epoch_seconds: u64,
    status: String,
}

pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SessionIndex>, SessionError> {
    let workspace = require_trial(&state, &headers, false)?;
    let lease = state.trial_workspace.lease_snapshot().map_err(internal)?;
    let runs_root = workspace.join(".anvil/runs");
    let mut entries = match tokio::fs::read_dir(&runs_root).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Json(SessionIndex {
                sessions: Vec::new(),
                lease,
            }));
        }
        Err(error) => return Err(internal(format!("read {}: {error}", runs_root.display()))),
    };

    let mut sessions = Vec::new();
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
        if !has_confirmation_record(&path.join("state/boundary-confirmations")).await {
            continue;
        }
        let events_path = path.join("events.jsonl");
        sessions.push(SessionSummary {
            id,
            modified_epoch_seconds: modified_epoch_seconds(&path, &events_path).await,
            status: session_status(&events_path).await,
        });
    }
    sessions.sort_by(|left, right| {
        right
            .modified_epoch_seconds
            .cmp(&left.modified_epoch_seconds)
            .then_with(|| right.id.cmp(&left.id))
    });
    sessions.truncate(MAX_SESSIONS);
    Ok(Json(SessionIndex { sessions, lease }))
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

async fn session_status(events_path: &Path) -> String {
    let metadata = match tokio::fs::symlink_metadata(events_path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return "starting".to_string();
        }
        Err(_) => return "unreadable".to_string(),
    };
    if !metadata.file_type().is_file() || metadata.len() > MAX_EVENTS_BYTES {
        return "unreadable".to_string();
    }
    let Ok(text) = tokio::fs::read_to_string(events_path).await else {
        return "unreadable".to_string();
    };
    let mut saw_event = false;
    let mut terminal = None;
    let mut run_stop_status = None;
    let mut continuation_index = None;
    for (index, line) in text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .enumerate()
    {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            return "unreadable".to_string();
        };
        saw_event = true;
        match event.get("event").and_then(Value::as_str) {
            Some("tui_command_stop") => terminal = Some((index, recorded_status(&event))),
            Some("run_stop") => run_stop_status = recorded_status(&event),
            Some("human_directive_continuation_started") => continuation_index = Some(index),
            _ => {}
        }
    }
    if !saw_event {
        return "starting".to_string();
    }
    let terminal_is_current = terminal.as_ref().is_some_and(|(terminal_index, _)| {
        continuation_index.is_none_or(|index| *terminal_index > index)
    });
    if !terminal_is_current {
        return "running".to_string();
    }
    terminal
        .and_then(|(_, status)| status)
        .or(run_stop_status)
        .unwrap_or_else(|| "running".to_string())
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
