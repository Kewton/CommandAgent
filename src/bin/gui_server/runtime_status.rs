use axum::{Json, extract::State};

use super::{AppState, workspace_policy::RuntimeStatus};

pub async fn get(State(state): State<AppState>) -> Json<RuntimeStatus> {
    Json(
        state
            .trial_workspace
            .runtime_status(state.trial_access.authentication_enabled()),
    )
}
