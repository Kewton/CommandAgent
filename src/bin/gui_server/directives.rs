use std::sync::mpsc;
use std::time::Duration;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use commandagent::tui::boundary_shell::BoundaryShell;
use serde::{Deserialize, Serialize};

use super::AppState;
use super::delegate::run_cli_continuation;
use super::session_paths::SessionPaths;
use super::sessions::{
    SessionError, bad_request, internal, not_found, require_current_terminal,
    require_no_pending_directive, require_session_id, require_trial, unprocessable,
    workspace_conflict,
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectiveRequest {
    directive: String,
}

#[derive(Debug, Serialize)]
pub struct DirectiveProposal {
    directive_hash: String,
    directive_round: u32,
    issued_gate: String,
    scrubbed_directive: String,
    confirmation_required: bool,
}

#[derive(Debug, Serialize)]
pub struct ConfirmedContinuation {
    directive_hash: String,
    directive_round: u32,
    target_run_id: String,
    continuation_plan: String,
    status: &'static str,
}

pub async fn propose(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<DirectiveRequest>,
) -> Result<Json<DirectiveProposal>, SessionError> {
    let workspace = require_trial(&state, &headers, true)?;
    require_session_id(&id)?;
    let paths = SessionPaths::existing(&workspace, &id)
        .map_err(|_| not_found("session run path is not safely readable"))?
        .ok_or_else(|| not_found("session run was not found"))?;
    paths
        .require_execution_workspace()
        .map_err(|_| not_found("session working directory is not safely available"))?;
    let events_path = paths.events_path();
    require_current_terminal(&events_path).await?;
    require_no_pending_directive(&events_path).await?;
    let mut shell = BoundaryShell::new(paths.confirmation_root(), Some(events_path));
    shell.restore_latest_terminal().map_err(bad_request)?;
    let round = shell.next_directive_round(&id).map_err(internal)?;
    let directive = shell
        .begin_directive(&request.directive, &id, round)
        .map_err(unprocessable)?;
    Ok(Json(DirectiveProposal {
        directive_hash: directive.hash().to_string(),
        directive_round: directive.artifact().round,
        issued_gate: directive.artifact().issued_gate.clone(),
        scrubbed_directive: directive.artifact().raw.clone(),
        confirmation_required: true,
    }))
}

pub async fn confirm(
    State(state): State<AppState>,
    Path((id, hash)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, SessionError> {
    let workspace = require_trial(&state, &headers, true)?;
    require_session_id(&id)?;
    let paths = SessionPaths::existing(&workspace, &id)
        .map_err(|_| not_found("session run path is not safely readable"))?
        .ok_or_else(|| not_found("session run was not found"))?;
    let events_path = paths.events_path();
    require_current_terminal(&events_path).await?;
    let mut shell = BoundaryShell::new(paths.confirmation_root(), Some(events_path.clone()));
    let identity = shell
        .restore_latest_terminal()
        .map_err(bad_request)?
        .ok_or_else(|| not_found("confirmed terminal identity was not found"))?;
    let directive = shell
        .restore_directive_proposal(&hash)
        .map_err(bad_request)?
        .clone();
    state
        .trial_workspace
        .acquire(&id)
        .map_err(workspace_conflict)?;
    if let Err(error) = shell.confirm_directive(&hash) {
        state.trial_workspace.cancel_start(&id);
        return Err(bad_request(error));
    }
    let continuation = shell.prepare_confirmed_continuation(
        paths.execution_workspace(),
        &events_path,
        &identity,
        &directive,
    );
    let continuation = match continuation {
        Ok(continuation) => continuation,
        Err(error) => {
            state.trial_workspace.cancel_start(&id);
            return Err(bad_request(error));
        }
    };
    let response = ConfirmedContinuation {
        directive_hash: continuation.directive_hash.clone(),
        directive_round: continuation.directive_round,
        target_run_id: continuation.target_run_id.clone(),
        continuation_plan: continuation.plan_workspace_path.clone(),
        status: "starting",
    };
    let (started_tx, started_rx) = mpsc::channel();
    let lease = state.trial_workspace.clone();
    let lease_id = id.clone();
    let lease_events = events_path.clone();
    std::thread::spawn(move || {
        let running_tx = started_tx.clone();
        let result = shell.dispatch_directive(&continuation, || {
            let _ = running_tx.send(Ok(()));
            run_cli_continuation(&state, &paths, &identity, &continuation)
        });
        lease.complete_from_events(&lease_id, &lease_events);
        if let Err(error) = result {
            let _ = started_tx.send(Err(error.to_string()));
            eprintln!("GUI directive continuation failed: {error:#}");
        }
    });
    match started_rx.recv_timeout(Duration::from_secs(2)) {
        Ok(Ok(())) => {}
        Ok(Err(error)) => return Err(internal(error)),
        Err(error) => {
            return Err(internal(format!(
                "directive start acknowledgement: {error}"
            )));
        }
    }
    Ok((StatusCode::ACCEPTED, Json(response)))
}
