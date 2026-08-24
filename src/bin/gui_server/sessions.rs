use std::collections::BTreeMap;
use std::path::{Path as FilePath, PathBuf};
use std::time::UNIX_EPOCH;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use commandagent::tui::boundary_shell::confirmation::{
    ConfirmationIdentity, load_latest_confirmation,
};
use commandagent::tui::boundary_shell::sheet;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use super::AppState;
use super::error_response::GuiError;
use super::session_diagnostics::{FailureDiagnostics, project as project_diagnostics};
use super::session_paths::{SessionPaths, relative_path};
use super::trial_access::AccessError;

const MAX_EVENTS_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct PhaseStatus {
    id: String,
    index: u64,
    total: u64,
    stage: String,
    status: String,
}

#[derive(Debug, Serialize)]
pub struct PolledSession {
    id: String,
    started_epoch_seconds: u64,
    average_duration_seconds: Option<f64>,
    gate: String,
    status: String,
    verdict: Option<String>,
    assurance: Option<String>,
    assurance_reason: Option<String>,
    stop_reason: Option<String>,
    failure_diagnostics: FailureDiagnostics,
    next_action: Option<String>,
    phases: Vec<PhaseStatus>,
    event_count: usize,
    acceptance_sheet: Option<String>,
    section5: Option<String>,
    events_path: String,
    identity: ConfirmationIdentity,
}

pub type SessionError = GuiError;

pub async fn workspace_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<super::workspace_policy::LeaseSnapshot>, SessionError> {
    require_trial(&state, &headers, false)?;
    state
        .trial_workspace
        .lease_snapshot()
        .map(Json)
        .map_err(internal)
}

pub async fn status(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, SessionError> {
    let workspace = require_trial(&state, &headers, false)?;
    require_session_id(&id)?;
    let paths = SessionPaths::existing(&workspace, &id)
        .map_err(internal)?
        .ok_or_else(|| not_found("session run was not found"))?;
    let confirmed = load_latest_confirmation(&paths.confirmation_root())
        .map_err(internal)?
        .ok_or_else(|| not_found("session confirmation was not found"))?;
    let events_path = paths.events_path();
    let etag = status_etag(confirmed.card_hash(), &events_path).await?;
    if if_none_match(&headers, &etag) {
        return Ok(status_response(StatusCode::NOT_MODIFIED, &etag));
    }
    let text = read_events(&events_path).await?;
    let events = parse_events(&text)?;
    let continuation_index = latest_event_index(&events, "human_directive_continuation_started");
    let current_event_start = continuation_index.map_or(0, |index| index + 1);
    let current_events = &events[current_event_start..];
    let terminal = latest_event(current_events, "tui_command_stop");
    let run_stop = latest_event(current_events, "run_stop");
    let terminal_is_current = terminal.is_some();
    let terminal_seen = terminal_is_current;
    let mut terminal_details = current_terminal_details(&events, continuation_index, terminal_seen);
    redact_terminal_details(&mut terminal_details, &workspace);
    let mut failure_diagnostics = if terminal_seen {
        project_diagnostics(current_events)
    } else {
        FailureDiagnostics::default()
    };
    failure_diagnostics.redact_execution_root(&workspace);
    let stop_reason = failure_diagnostics
        .stop_reason
        .as_ref()
        .filter(|reason| reason.as_str() != "completed")
        .cloned()
        .or(terminal_details.stop_reason);
    let command_succeeded = terminal
        .and_then(|event| event.get("ok"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut generated = terminal
        .filter(|_| terminal_is_current)
        .map(|_| sheet::generate(confirmed.identity(), Some(&events_path), command_succeeded))
        .transpose()
        .map_err(internal)?;
    if let Some(sheet) = &mut generated {
        sheet.markdown = super::public_projection::text(&sheet.markdown, &workspace);
        sheet.section5 = sheet
            .section5
            .take()
            .map(|section| super::public_projection::text(section, &workspace));
    }
    let verdict = terminal_is_current
        .then(|| project_verdict(current_events))
        .flatten()
        .or_else(|| terminal.and_then(|event| string(event, "assurance_level")))
        .map(str::to_string);
    let assurance = terminal
        .filter(|_| terminal_is_current)
        .and_then(|event| string(event, "assurance_level"))
        .map(str::to_string);
    let gate = match generated.as_ref() {
        Some(sheet) if sheet.full => "gate_3",
        Some(_) => "gate_4",
        None if terminal_seen => "gate_4",
        None => "gate_2",
    };
    let status = if terminal_is_current {
        terminal
            .and_then(|event| string(event, "status"))
            .or_else(|| run_stop.and_then(|event| string(event, "status")))
            .unwrap_or("running")
    } else if events.is_empty() {
        "starting"
    } else {
        "running"
    };
    let started_epoch_seconds = started_epoch_seconds(&id, paths.run_root(), &events_path).await;
    let average_duration_seconds =
        super::gate_one::average_duration_seconds(&state.repository_root, confirmed.identity())
            .await?;
    let session = PolledSession {
        id,
        started_epoch_seconds,
        average_duration_seconds,
        gate: gate.to_string(),
        status: status.to_string(),
        verdict,
        assurance,
        assurance_reason: terminal_details.assurance_reason,
        stop_reason,
        failure_diagnostics,
        next_action: terminal_details.next_action,
        phases: phase_statuses(&events),
        event_count: events.len(),
        acceptance_sheet: generated.as_ref().map(|sheet| sheet.markdown.clone()),
        section5: generated.and_then(|sheet| sheet.section5),
        events_path: relative_path(&workspace, &events_path),
        identity: super::public_projection::identity(confirmed.identity()),
    };
    let mut response = Json(session).into_response();
    insert_status_headers(&mut response, &etag);
    Ok(response)
}

#[derive(Debug, Default, PartialEq, Eq)]
struct TerminalDetails {
    assurance_reason: Option<String>,
    stop_reason: Option<String>,
    next_action: Option<String>,
}

fn current_terminal_details(
    events: &[Value],
    continuation_index: Option<usize>,
    terminal_seen: bool,
) -> TerminalDetails {
    if !terminal_seen {
        return TerminalDetails::default();
    }
    let current_event_start = continuation_index.map_or(0, |index| index + 1);
    project_terminal_details(&events[current_event_start..])
}

fn redact_terminal_details(details: &mut TerminalDetails, execution_root: &FilePath) {
    details.assurance_reason = details
        .assurance_reason
        .take()
        .map(|value| super::public_projection::text(value, execution_root));
    details.stop_reason = details
        .stop_reason
        .take()
        .map(|value| super::public_projection::text(value, execution_root));
    details.next_action = details
        .next_action
        .take()
        .map(|value| super::public_projection::text(value, execution_root));
}

fn project_verdict(events: &[Value]) -> Option<&str> {
    events
        .iter()
        .rev()
        .find_map(|event| non_neutral_string(event, "verdict"))
        .or_else(|| {
            events
                .iter()
                .rev()
                .find_map(|event| non_neutral_string(event, "final_acceptance_status"))
        })
}

fn project_terminal_details(events: &[Value]) -> TerminalDetails {
    let terminal =
        latest_event(events, "run_stop").or_else(|| latest_event(events, "tui_command_stop"));
    TerminalDetails {
        assurance_reason: terminal
            .and_then(|event| non_empty_string(event, "assurance_reason"))
            .or_else(|| {
                latest_event(events, "ultra_final_acceptance")
                    .and_then(|event| non_empty_string(event, "assurance_reason"))
            })
            .map(str::to_string),
        stop_reason: terminal
            .and_then(|event| {
                non_empty_string(event, "stop_reason")
                    .or_else(|| non_empty_string(event, "primary_reason"))
                    .or_else(|| non_empty_string(event, "failure_kind"))
            })
            .map(str::to_string),
        next_action: terminal
            .and_then(|event| {
                non_empty_string(event, "next_action")
                    .or_else(|| non_empty_string(event, "recovery_next_action"))
            })
            .or_else(|| {
                latest_event(events, "ultra_final_acceptance")
                    .and_then(|event| non_empty_string(event, "next_action"))
            })
            .or_else(|| {
                latest_event(events, "plan_final_contract")
                    .and_then(|event| non_empty_string(event, "next_action"))
            })
            .map(str::to_string),
    }
}

fn phase_statuses(events: &[Value]) -> Vec<PhaseStatus> {
    let mut phases = BTreeMap::<(u64, String), PhaseStatus>::new();
    let mut terminal_seen = false;
    for event in events {
        let event_name = string(event, "event").unwrap_or("unknown");
        if matches!(event_name, "tui_command_stop" | "run_stop") {
            terminal_seen = true;
            for phase in phases.values_mut() {
                if matches!(phase.status.as_str(), "pending" | "running") {
                    phase.status = "interrupted".to_string();
                }
            }
            continue;
        }

        let Some(id) = string(event, "phase_id") else {
            continue;
        };
        let Some(effect) = phase_event_effect(event_name) else {
            continue;
        };
        if terminal_seen && matches!(effect, PhaseEventEffect::Status(status) if status != "failed")
        {
            continue;
        }
        let index = event.get("phase_index").and_then(Value::as_u64);
        let key = match index {
            Some(index) => (index, id.to_string()),
            None => {
                let Some(key) = phases
                    .keys()
                    .rev()
                    .find(|(_, phase_id)| phase_id == id)
                    .cloned()
                else {
                    continue;
                };
                key
            }
        };
        if matches!(effect, PhaseEventEffect::StageOnly) && !phases.contains_key(&key) {
            continue;
        }
        let total = event
            .get("total_phases")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let phase_index = key.0;
        let phase = phases.entry(key).or_insert_with(|| PhaseStatus {
            id: id.to_string(),
            index: phase_index,
            total,
            stage: "queued".to_string(),
            status: "pending".to_string(),
        });
        match effect {
            PhaseEventEffect::StageOnly => {
                phase.stage = string(event, "stage").unwrap_or(event_name).to_string();
            }
            PhaseEventEffect::Status(status) => {
                let current_status = phase.status.as_str();
                if current_status == "failed"
                    || (current_status == "completed" && status != "failed")
                    || (current_status == "interrupted" && status != "failed")
                {
                    continue;
                }
                phase.stage = string(event, "stage").unwrap_or(event_name).to_string();
                phase.status = status.to_string();
            }
        }
    }
    let mut projected = phases.into_values().collect::<Vec<_>>();
    if let Some(planning) = plan_generation_phase(events) {
        projected.insert(0, planning);
    }
    projected
}

fn plan_generation_phase(events: &[Value]) -> Option<PhaseStatus> {
    let mut status = None;
    for event in events {
        match string(event, "event").unwrap_or("unknown") {
            "ultra_plan_generation_attempt"
            | "ultra_plan_generation_metadata_normalized"
            | "ultra_plan_generation_retry"
            | "ultra_plan_generation_tool_call_rejected" => status = Some("running"),
            "ultra_plan_generation_succeeded" => status = Some("completed"),
            "ultra_plan_generation_failed" => status = Some("failed"),
            "tui_command_stop" | "run_stop" if status == Some("running") => {
                status = Some("interrupted");
            }
            _ => {}
        }
    }
    status.map(|status| PhaseStatus {
        id: "plan_generation".to_string(),
        index: 0,
        total: 0,
        stage: if status == "completed" {
            "complete".to_string()
        } else {
            "scaffold".to_string()
        },
        status: status.to_string(),
    })
}

#[derive(Clone, Copy)]
enum PhaseEventEffect {
    Status(&'static str),
    StageOnly,
}

fn phase_event_effect(event_name: &str) -> Option<PhaseEventEffect> {
    match event_name {
        "ultra_phase_complete" => Some(PhaseEventEffect::Status("completed")),
        "ultra_phase_failed" => Some(PhaseEventEffect::Status("failed")),
        "ultra_phase_start"
        | "ultra_phase_scaffold_complete"
        | "ultra_phase_plan_validated"
        | "ultra_phase_execute_complete"
        | "phase_verification_result"
        | "ultra_phase_profile_check" => Some(PhaseEventEffect::Status("running")),
        "ultra_phase_context_attached"
        | "ultra_phase_context_updated"
        | "recovery_prompt_saved" => Some(PhaseEventEffect::StageOnly),
        _ => None,
    }
}

async fn read_events(path: &FilePath) -> Result<String, SessionError> {
    let metadata = match tokio::fs::metadata(path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(error) => return Err(internal(error)),
    };
    if metadata.len() > MAX_EVENTS_BYTES {
        return Err(GuiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "trial_events_too_large",
            "session event stream exceeds the 4 MiB polling limit",
        ));
    }
    tokio::fs::read_to_string(path).await.map_err(internal)
}

async fn status_etag(card_hash: &str, events_path: &FilePath) -> Result<String, SessionError> {
    let revision = match tokio::fs::metadata(events_path).await {
        Ok(metadata) => {
            let modified = metadata
                .modified()
                .map_err(internal)?
                .duration_since(UNIX_EPOCH)
                .map_err(internal)?
                .as_nanos();
            format!("{}-{modified}", metadata.len())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => "missing".to_string(),
        Err(error) => return Err(internal(error)),
    };
    Ok(format!(
        "W/\"{}-{revision}\"",
        card_hash.trim_start_matches("sha256:")
    ))
}

fn if_none_match(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get_all(header::IF_NONE_MATCH)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .any(|candidate| candidate == "*" || weak_etag_eq(candidate, etag))
}

fn weak_etag_eq(left: &str, right: &str) -> bool {
    left.strip_prefix("W/").unwrap_or(left) == right.strip_prefix("W/").unwrap_or(right)
}

fn status_response(status: StatusCode, etag: &str) -> Response {
    let mut response = status.into_response();
    insert_status_headers(&mut response, etag);
    response
}

fn insert_status_headers(response: &mut Response, etag: &str) {
    response.headers_mut().insert(
        header::ETAG,
        HeaderValue::from_str(etag).expect("generated status ETags are valid header values"),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-cache"),
    );
}

pub(super) async fn started_epoch_seconds(
    id: &str,
    run_root: &FilePath,
    events_path: &FilePath,
) -> u64 {
    let uuid_epoch = Uuid::parse_str(id)
        .ok()
        .filter(|id| id.get_version_num() == 7)
        .and_then(|id| id.get_timestamp())
        .map(|timestamp| timestamp.to_unix().0);
    if let Some(epoch) = uuid_epoch {
        return epoch;
    }
    let events_created = metadata_created(events_path).await;
    if events_created > 0 {
        events_created
    } else {
        metadata_created(run_root).await
    }
}

async fn metadata_created(path: &FilePath) -> u64 {
    tokio::fs::symlink_metadata(path)
        .await
        .ok()
        .and_then(|metadata| metadata.created().or_else(|_| metadata.modified()).ok())
        .and_then(|created| created.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

pub(super) async fn require_current_terminal(path: &FilePath) -> Result<(), SessionError> {
    let events = parse_events(&read_events(path).await?)?;
    let terminal = latest_event_index(&events, "tui_command_stop")
        .ok_or_else(|| conflict("session has not reached Gate 3 or Gate 4"))?;
    if latest_event_index(&events, "human_directive_continuation_started")
        .is_some_and(|continuation| continuation > terminal)
    {
        return Err(conflict(
            "a confirmed directive continuation is still running",
        ));
    }
    Ok(())
}

pub(super) async fn require_no_pending_directive(path: &FilePath) -> Result<(), SessionError> {
    let events = parse_events(&read_events(path).await?)?;
    let terminal = latest_event_index(&events, "tui_command_stop")
        .ok_or_else(|| conflict("session has not reached Gate 3 or Gate 4"))?;
    if latest_event_index(&events, "human_directive_proposed")
        .is_some_and(|proposal| proposal > terminal)
    {
        return Err(conflict(
            "the current terminal already has a pending directive proposal",
        ));
    }
    Ok(())
}

fn parse_events(text: &str) -> Result<Vec<Value>, SessionError> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).map_err(invalid_events))
        .collect()
}

fn latest_event<'a>(events: &'a [Value], name: &str) -> Option<&'a Value> {
    events
        .iter()
        .rev()
        .find(|event| string(event, "event") == Some(name))
}

fn latest_event_index(events: &[Value], name: &str) -> Option<usize> {
    events
        .iter()
        .rposition(|event| string(event, "event") == Some(name))
}

fn string<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn non_empty_string<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    string(value, key).filter(|value| !value.trim().is_empty())
}

fn non_neutral_string<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    non_empty_string(value, key).filter(|value| {
        !matches!(
            value.trim(),
            "not_applicable" | "not_required" | "not_checked"
        )
    })
}

pub(super) fn require_session_id(id: &str) -> Result<(), SessionError> {
    let parsed = Uuid::parse_str(id).map_err(|_| not_found("invalid session id"))?;
    if parsed.to_string() != id {
        return Err(not_found("invalid session id"));
    }
    Ok(())
}

pub(super) fn require_trial(
    state: &AppState,
    headers: &HeaderMap,
    require_origin: bool,
) -> Result<PathBuf, SessionError> {
    if !state.trial_workspace.is_enabled() {
        return Err(GuiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "trial_execution_disabled",
            "trial execution is disabled; configure --execution-root",
        ));
    }
    let workspace = state
        .trial_workspace
        .require_current()
        .map_err(workspace_conflict)?;
    state
        .trial_access
        .authorize(headers, require_origin)
        .map_err(|error| match error {
            AccessError::Unauthorized => GuiError::new(
                StatusCode::UNAUTHORIZED,
                "trial_token_invalid",
                "a valid GUI trial bearer token is required",
            ),
            AccessError::ForbiddenOrigin => GuiError::new(
                StatusCode::FORBIDDEN,
                "trial_origin_not_allowed",
                "trial request origin is not allowed",
            ),
        })?;
    Ok(workspace)
}

pub(super) fn unprocessable(message: impl ToString) -> SessionError {
    GuiError::new(
        StatusCode::UNPROCESSABLE_ENTITY,
        "trial_request_invalid",
        message.to_string(),
    )
}

pub(super) fn bad_request(error: impl ToString) -> SessionError {
    GuiError::new(
        StatusCode::BAD_REQUEST,
        "trial_request_invalid",
        error.to_string(),
    )
}

pub(super) fn not_found(message: impl ToString) -> SessionError {
    GuiError::new(
        StatusCode::NOT_FOUND,
        "trial_session_not_found",
        message.to_string(),
    )
}

fn conflict(message: impl ToString) -> SessionError {
    GuiError::new(
        StatusCode::CONFLICT,
        "trial_session_conflict",
        message.to_string(),
    )
}

pub(super) fn workspace_conflict(message: impl ToString) -> SessionError {
    let message = message.to_string();
    let code = if message.contains("already running session") {
        "trial_workspace_running"
    } else if message.contains("requires recovery for non-terminal session") {
        "trial_workspace_recovery_required"
    } else {
        "trial_workspace_conflict"
    };
    let session_id = (code == "trial_workspace_recovery_required")
        .then(|| message.rsplit_once(' ').map(|(_, id)| id))
        .flatten()
        .filter(|id| Uuid::parse_str(id).is_ok_and(|parsed| parsed.to_string() == *id))
        .map(str::to_string);
    let public_message = if code == "trial_workspace_conflict" {
        "trial workspace changed or became unavailable; verify --execution-root and restart"
            .to_string()
    } else {
        message
    };
    let error = GuiError::new(StatusCode::CONFLICT, code, public_message);
    match session_id {
        Some(session_id) => error.with_session_id(session_id),
        None => error,
    }
}

fn invalid_events(error: impl ToString) -> SessionError {
    GuiError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "trial_session_events_invalid",
        error.to_string(),
    )
}

pub(super) fn internal(error: impl ToString) -> SessionError {
    GuiError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "trial_internal_error",
        error.to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_statuses_attach_unindexed_events_without_creating_ghost_rows() {
        let events = vec![
            serde_json::json!({
                "event": "ultra_phase_start",
                "phase_id": "setup",
                "phase_index": 1,
                "total_phases": 1
            }),
            serde_json::json!({
                "event": "recovery_prompt_saved",
                "phase_id": "setup"
            }),
            serde_json::json!({
                "event": "ultra_phase_start",
                "phase_id": "ghost",
                "total_phases": 1
            }),
        ];

        let phases = phase_statuses(&events);

        assert_eq!(phases.len(), 1);
        assert_eq!(phases[0].index, 1);
        assert_eq!(phases[0].stage, "recovery_prompt_saved");
        assert_eq!(phases[0].status, "running");
    }

    #[test]
    fn phase_statuses_project_plan_generation_before_numbered_phases() {
        let events = vec![
            serde_json::json!({"event": "tui_command_start"}),
            serde_json::json!({"event": "ultra_plan_generation_attempt", "attempt": 1}),
            serde_json::json!({
                "event": "ultra_plan_generation_metadata_normalized",
                "fields": ["goal"]
            }),
        ];

        let phases = phase_statuses(&events);

        assert_eq!(phases.len(), 1);
        assert_eq!(phases[0].id, "plan_generation");
        assert_eq!(phases[0].index, 0);
        assert_eq!(phases[0].total, 0);
        assert_eq!(phases[0].stage, "scaffold");
        assert_eq!(phases[0].status, "running");
    }

    #[test]
    fn phase_statuses_keep_plan_generation_outcomes_honest() {
        let completed = phase_statuses(&[
            serde_json::json!({"event": "ultra_plan_generation_attempt", "attempt": 1}),
            serde_json::json!({"event": "ultra_plan_generation_succeeded", "phase_count": 2}),
        ]);
        let failed = phase_statuses(&[
            serde_json::json!({"event": "ultra_plan_generation_attempt", "attempt": 1}),
            serde_json::json!({"event": "ultra_plan_generation_failed", "attempt": 1}),
        ]);
        let absent = phase_statuses(&[serde_json::json!({"event": "tui_command_start"})]);

        assert_eq!(completed[0].stage, "complete");
        assert_eq!(completed[0].status, "completed");
        assert_eq!(failed[0].stage, "scaffold");
        assert_eq!(failed[0].status, "failed");
        assert!(absent.is_empty());
    }

    #[test]
    fn phase_statuses_do_not_restore_running_after_completion() {
        let events = vec![
            serde_json::json!({
                "event": "ultra_phase_start",
                "phase_id": "setup",
                "phase_index": 1,
                "total_phases": 1
            }),
            serde_json::json!({
                "event": "ultra_phase_complete",
                "phase_id": "setup",
                "phase_index": 1,
                "total_phases": 1,
                "stage": "complete"
            }),
            serde_json::json!({
                "event": "ultra_phase_context_updated",
                "phase_id": "setup",
                "phase_index": 1,
                "total_phases": 1
            }),
        ];

        let phases = phase_statuses(&events);

        assert_eq!(phases.len(), 1);
        assert!(phases.iter().all(|phase| phase.status != "running"));
        assert_eq!(phases[0].stage, "ultra_phase_context_updated");
        assert_eq!(phases[0].status, "completed");
    }

    #[test]
    fn phase_statuses_keep_failures_terminal() {
        let events = vec![
            serde_json::json!({
                "event": "ultra_phase_start",
                "phase_id": "setup",
                "phase_index": 1,
                "total_phases": 1
            }),
            serde_json::json!({
                "event": "ultra_phase_failed",
                "phase_id": "setup",
                "phase_index": 1,
                "total_phases": 1,
                "stage": "execute"
            }),
            serde_json::json!({
                "event": "recovery_prompt_saved",
                "phase_id": "setup"
            }),
            serde_json::json!({
                "event": "future_phase_annotation",
                "phase_id": "setup",
                "phase_index": 1,
                "total_phases": 1
            }),
        ];

        let phases = phase_statuses(&events);

        assert_eq!(phases.len(), 1);
        assert_eq!(phases[0].stage, "recovery_prompt_saved");
        assert_eq!(phases[0].status, "failed");
    }

    #[test]
    fn phase_statuses_interrupt_running_phases_at_terminal() {
        let events = vec![
            serde_json::json!({
                "event": "ultra_phase_start",
                "phase_id": "setup",
                "phase_index": 1,
                "total_phases": 2
            }),
            serde_json::json!({
                "event": "ultra_phase_start",
                "phase_id": "build",
                "phase_index": 2,
                "total_phases": 2
            }),
            serde_json::json!({
                "event": "ultra_phase_complete",
                "phase_id": "build",
                "phase_index": 2,
                "total_phases": 2,
                "stage": "complete"
            }),
            serde_json::json!({"event": "tui_command_stop", "status": "failed"}),
            serde_json::json!({"event": "run_stop", "status": "failed"}),
            serde_json::json!({
                "event": "ultra_phase_context_updated",
                "phase_id": "setup",
                "phase_index": 1,
                "total_phases": 2
            }),
        ];

        let phases = phase_statuses(&events);

        assert_eq!(phases.len(), 2);
        assert_eq!(phases[0].status, "interrupted");
        assert_eq!(phases[1].status, "completed");
        assert!(phases.iter().all(|phase| phase.status != "running"));
    }

    #[test]
    fn gui_smoke_events_project_one_failed_phase() {
        let events = parse_events(include_str!(
            "../../../workspace/management/runs/g1-gui-smoke/root-events.jsonl"
        ))
        .unwrap();

        let phases = phase_statuses(&events);

        assert_eq!(phases.len(), 2);
        assert_eq!(phases[0].id, "plan_generation");
        assert_eq!(phases[0].index, 0);
        assert_eq!(phases[0].status, "completed");
        assert_eq!(phases[1].id, "setup-project");
        assert_eq!(phases[1].index, 1);
        assert_eq!(phases[1].total, 5);
        assert_eq!(phases[1].stage, "recovery_prompt_saved");
        assert_eq!(phases[1].status, "failed");
    }

    #[test]
    fn terminal_details_project_recorded_result_without_changing_it() {
        let events = vec![
            serde_json::json!({
                "event": "ultra_final_acceptance",
                "assurance_reason": "cli_probe_not_run",
                "next_action": "repair_release_gate_failure"
            }),
            serde_json::json!({
                "event": "tui_command_stop",
                "assurance_reason": "cli_probe_not_run",
                "stop_reason": "completed",
                "next_action": "fix_command_failure"
            }),
            serde_json::json!({
                "event": "run_stop",
                "assurance_reason": "cli_probe_not_run",
                "stop_reason": "verification failed",
                "recovery_next_action": "resume_or_rerun_command"
            }),
        ];

        assert_eq!(
            project_terminal_details(&events),
            TerminalDetails {
                assurance_reason: Some("cli_probe_not_run".to_string()),
                stop_reason: Some("verification failed".to_string()),
                next_action: Some("resume_or_rerun_command".to_string()),
            }
        );
    }

    #[test]
    fn terminal_details_use_current_acceptance_fallbacks_only() {
        let events = vec![
            serde_json::json!({
                "event": "ultra_final_acceptance",
                "assurance_reason": "acceptance_not_full_success",
                "next_action": "repair_release_gate_failure"
            }),
            serde_json::json!({
                "event": "tui_command_stop",
                "primary_reason": "release gate failed"
            }),
        ];

        assert_eq!(
            project_terminal_details(&events),
            TerminalDetails {
                assurance_reason: Some("acceptance_not_full_success".to_string()),
                stop_reason: Some("release gate failed".to_string()),
                next_action: Some("repair_release_gate_failure".to_string()),
            }
        );
        assert_eq!(project_terminal_details(&[]), TerminalDetails::default());
    }

    #[test]
    fn terminal_details_do_not_leak_a_prior_directive_round() {
        let events = vec![
            serde_json::json!({
                "event": "tui_command_stop",
                "assurance_reason": "stale_reason",
                "stop_reason": "stale stop",
                "next_action": "stale_action"
            }),
            serde_json::json!({"event": "human_directive_continuation_started"}),
            serde_json::json!({
                "event": "tui_command_stop",
                "assurance_reason": "cli_probe_not_run",
                "stop_reason": "current stop",
                "next_action": "fix_command_failure"
            }),
        ];

        assert_eq!(
            current_terminal_details(&events, Some(1), true),
            TerminalDetails {
                assurance_reason: Some("cli_probe_not_run".to_string()),
                stop_reason: Some("current stop".to_string()),
                next_action: Some("fix_command_failure".to_string()),
            }
        );
        assert_eq!(
            current_terminal_details(&events, Some(1), false),
            TerminalDetails::default()
        );
    }

    #[test]
    fn verdict_projection_ignores_trailing_neutral_status_and_prior_round() {
        let fixture = include_str!(
            "../../../tests/corpus/apps/issue364-gui-terminal-outcomes/fixtures/events.jsonl"
        );
        let events = fixture
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect::<Vec<_>>();

        assert_eq!(project_verdict(&events[..3]), Some("full"));
        assert_eq!(project_verdict(&events[13..]), Some("full"));
        assert_eq!(project_verdict(&events[12..13]), None);
    }
}
