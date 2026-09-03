use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use commandagent::tui::boundary_shell::recovery_run::{
    self, PersistedRecoveryRun, RecoveryRunBinding, RecoveryRunError, RecoveryRunProposal,
};
use commandagent::tui::boundary_shell::{BoundaryShell, BoundaryState};
use serde::{Deserialize, Serialize};

use super::AppState;
use super::delegate::{DELEGATE_PERMISSION_POLICY, spawn_cli_recovery};
use super::error_response::GuiError;
use super::session_paths::SessionPaths;
use super::sessions::{
    SessionError, internal, not_found, read_events, require_current_terminal,
    require_no_pending_directive, require_session_id, require_trial, session_conflict,
    workspace_conflict,
};

#[derive(Debug, Serialize)]
pub struct RecoveryRunProposalResponse {
    confirmation_hash: String,
    confirmation_required: bool,
    #[serde(flatten)]
    proposal: RecoveryRunProposal,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryRunConfirmationRequest {
    #[serde(default)]
    acknowledged: bool,
}

#[derive(Debug, Serialize)]
pub struct ConfirmedRecoveryRun {
    confirmation_hash: String,
    plan_hash: String,
    source_plan_path: String,
    frozen_plan_path: String,
    process_generation: String,
    status: &'static str,
}

pub async fn propose(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<RecoveryRunProposalResponse>, SessionError> {
    let execution_root = require_trial(&state, &headers, true)?;
    require_session_id(&id)?;
    let paths = session_paths(&execution_root, &id)?;
    let execution_workspace = paths
        .require_execution_workspace()
        .map_err(|_| not_found("session working directory is not safely available"))?;
    let events_path = paths.events_path();
    require_current_terminal(&events_path).await?;
    require_no_pending_directive(&events_path).await?;
    let identity = gate_four_identity(&paths, &events_path)?;
    let recovery = recovery_run::propose(
        &paths.state_root(),
        &execution_workspace,
        &events_path,
        &id,
        &identity.card_hash().map_err(internal)?,
        DELEGATE_PERMISSION_POLICY,
        identity.recovery_plan_auto_runs,
    )
    .map_err(recovery_error)?;
    Ok(Json(RecoveryRunProposalResponse {
        confirmation_hash: recovery.confirmation_hash().to_string(),
        confirmation_required: true,
        proposal: recovery.proposal().clone(),
    }))
}

pub async fn confirm(
    State(state): State<AppState>,
    Path((id, hash)): Path<(String, String)>,
    headers: HeaderMap,
    Json(request): Json<RecoveryRunConfirmationRequest>,
) -> Result<impl IntoResponse, SessionError> {
    if !request.acknowledged {
        return Err(GuiError::new(
            StatusCode::PRECONDITION_REQUIRED,
            "recovery_run_confirmation_required",
            "Recovery Run acknowledgement is required before dispatch",
        ));
    }
    let execution_root = require_trial(&state, &headers, true)?;
    require_session_id(&id)?;
    let paths = session_paths(&execution_root, &id)?;
    let execution_workspace = paths
        .require_execution_workspace()
        .map_err(|_| not_found("session working directory is not safely available"))?;
    let events_path = paths.events_path();
    require_current_terminal(&events_path).await?;
    require_no_pending_directive(&events_path).await?;
    let identity = gate_four_identity(&paths, &events_path)?;
    let identity_hash = identity.card_hash().map_err(internal)?;
    let mut recovery = load(
        &paths,
        &execution_workspace,
        &events_path,
        &hash,
        &id,
        &identity_hash,
        identity.recovery_plan_auto_runs,
    )?;
    state
        .trial_workspace
        .acquire(&id)
        .map_err(workspace_conflict)?;
    let start = async {
        require_no_pending_directive(&events_path).await?;
        recovery = load(
            &paths,
            &execution_workspace,
            &events_path,
            &hash,
            &id,
            &identity_hash,
            identity.recovery_plan_auto_runs,
        )?;
        recovery_run::confirm(&paths.state_root(), &recovery).map_err(recovery_error)?;
        let prior_event_count = read_events(&events_path)
            .await?
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count();
        let process_generation = super::trial_process::TrialProcesses::new_generation();
        let (process, child) = spawn_cli_recovery(
            &state,
            &paths,
            &identity,
            &recovery,
            &id,
            &process_generation,
        )
        .map_err(internal)?;
        let response = ConfirmedRecoveryRun {
            confirmation_hash: hash,
            plan_hash: recovery.proposal().plan_hash.clone(),
            source_plan_path: recovery.proposal().source_plan_path.clone(),
            frozen_plan_path: recovery.proposal().frozen_plan_path.clone(),
            process_generation,
            status: "starting",
        };
        let lease = state.trial_workspace.clone();
        let processes = state.trial_processes.clone();
        let lease_id = id.clone();
        std::thread::spawn(move || {
            let process_tree_gone = match processes.wait(&process, child) {
                Ok(completion) => completion.process_tree_gone,
                Err(error) => {
                    eprintln!("GUI Recovery Run wait failed: {error:#}");
                    false
                }
            };
            lease.complete_after_process_since(
                &lease_id,
                &events_path,
                process_tree_gone,
                prior_event_count,
            );
        });
        Ok((StatusCode::ACCEPTED, Json(response)))
    }
    .await;
    if start.is_err() {
        state.trial_workspace.cancel_start(&id);
    }
    start
}

fn session_paths(execution_root: &std::path::Path, id: &str) -> Result<SessionPaths, SessionError> {
    SessionPaths::existing(execution_root, id)
        .map_err(|_| not_found("session run path is not safely readable"))?
        .ok_or_else(|| not_found("session run was not found"))
}

fn gate_four_identity(
    paths: &SessionPaths,
    events_path: &std::path::Path,
) -> Result<commandagent::tui::boundary_shell::confirmation::ConfirmationIdentity, SessionError> {
    let mut shell = BoundaryShell::new(paths.confirmation_root(), Some(events_path.to_path_buf()));
    let identity = shell
        .restore_latest_terminal()
        .map_err(|error| session_conflict(error.to_string()))?
        .ok_or_else(|| not_found("confirmed terminal identity was not found"))?;
    if !matches!(shell.state(), BoundaryState::FailureReady(_)) {
        return Err(session_conflict(
            "Recovery Run is available only from a failed Gate 4 terminal",
        ));
    }
    Ok(identity)
}

fn load(
    paths: &SessionPaths,
    workspace: &std::path::Path,
    events_path: &std::path::Path,
    hash: &str,
    id: &str,
    identity_hash: &str,
    automatic_run_budget: u8,
) -> Result<PersistedRecoveryRun, SessionError> {
    recovery_run::load_current(
        &paths.state_root(),
        workspace,
        events_path,
        hash,
        RecoveryRunBinding {
            target_run_id: id,
            identity_hash,
            permission_policy: DELEGATE_PERMISSION_POLICY,
            automatic_run_budget,
        },
    )
    .map_err(recovery_error)
}

fn recovery_error(error: RecoveryRunError) -> SessionError {
    let (status, code) = match &error {
        RecoveryRunError::Drift(_) => (StatusCode::CONFLICT, "recovery_run_drift"),
        RecoveryRunError::TreatmentRejected(_) => {
            (StatusCode::CONFLICT, "recovery_treatment_rejected")
        }
        RecoveryRunError::TreatmentPending => (StatusCode::CONFLICT, "recovery_treatment_pending"),
        RecoveryRunError::Stale(_) | RecoveryRunError::AlreadyConfirmed => {
            (StatusCode::PRECONDITION_FAILED, "recovery_run_stale")
        }
        RecoveryRunError::Invalid(_) => (StatusCode::UNPROCESSABLE_ENTITY, "recovery_run_invalid"),
        RecoveryRunError::Storage(_) => return internal(error),
    };
    GuiError::new(status, code, error.to_string())
}
