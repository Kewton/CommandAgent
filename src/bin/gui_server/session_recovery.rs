use std::path::Path as FilePath;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use commandagent::eval_events::failure_explanation::{
    ProjectionContext, WorkspaceState, project as project_failure,
};
use serde::Deserialize;

use super::AppState;
use super::error_response::GuiError;
use super::session_paths::{SessionPaths, WorkingDirectoryState};
use super::sessions::{
    current_event_interval, not_found, parse_events, read_events, require_session_id, require_trial,
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RecoveryDocumentQuery {
    path: String,
}

pub(super) async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Query(query): Query<RecoveryDocumentQuery>,
) -> Result<Response, GuiError> {
    if !state.trial_access.authentication_enabled() {
        return Err(GuiError::new(
            StatusCode::FORBIDDEN,
            "trial_recovery_authentication_required",
            "recovery documents require Trial token authentication",
        ));
    }
    let execution_root = require_trial(&state, &headers, false)?;
    require_session_id(&id)?;
    let paths = SessionPaths::existing(&execution_root, &id)
        .map_err(|_| not_found("session run path is not safely readable"))?
        .ok_or_else(|| not_found("session run was not found"))?;
    let workspace_state = paths
        .execution_workspace_state()
        .map_err(|_| not_found("session working directory is not safely readable"))?;
    if workspace_state != WorkingDirectoryState::Available {
        return Err(not_found("session working directory is missing"));
    }

    let events = parse_events(&read_events(&paths.events_path()).await?)?;
    let (start, interval_index) = current_event_interval(&events);
    let explanation = project_failure(
        &events[start..],
        ProjectionContext::new(interval_index, WorkspaceState::Available),
    )
    .ok_or_else(|| not_found("current execution interval has no recovery documents"))?;
    let requested = query.path.trim();
    let allowed = [
        explanation.recovery.repair_prompt_path.as_ref(),
        explanation.recovery.recovery_plan_path.as_ref(),
    ]
    .into_iter()
    .flatten()
    .any(|path| !path.truncated && path.value == requested);
    if !allowed {
        return Err(not_found(
            "path is not a current projected recovery document",
        ));
    }

    let workspace = paths
        .require_execution_workspace()
        .map_err(|_| not_found("session working directory is not safely readable"))?;
    let document_path =
        super::api::checked_existing_path_without_symlinks(&workspace, FilePath::new(requested))
            .await?;
    let mut document = super::api::document(&workspace, &document_path).await?;
    document.redact_execution_root(&execution_root);
    let mut response = Json(document).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    Ok(response)
}
