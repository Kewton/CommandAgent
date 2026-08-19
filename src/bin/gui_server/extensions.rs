use std::collections::BTreeMap;

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use commandagent::planner::pack::{
    Actor, StageReport, StagedFile, SuppliedPack, SupplyError, SupplyRoot,
};
use serde::{Deserialize, Serialize};

use super::AppState;
use super::error_response::GuiError;
use super::sessions::require_trial;

pub const MAX_BODY_BYTES: usize = 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StageRequest {
    id: String,
    version: String,
    files: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinRequest {
    hash: String,
}

#[derive(Debug, Serialize)]
pub struct PackDetail {
    id: String,
    version: String,
    files: BTreeMap<String, String>,
    report: serde_json::Value,
}

pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<SuppliedPack>>, GuiError> {
    let root = supply_root(&state)?;
    require_trial(&state, &headers, false)?;
    run_supply(move || root.list()).await.map(Json)
}

pub async fn detail(
    State(state): State<AppState>,
    Path((id, version)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<PackDetail>, GuiError> {
    let root = supply_root(&state)?;
    require_trial(&state, &headers, false)?;
    run_supply(move || {
        let files = root
            .bundle(&id, &version)?
            .into_iter()
            .map(|file| {
                let text = String::from_utf8(file.bytes).map_err(|_| {
                    SupplyError::Invalid(format!("member `{}` is not valid UTF-8", file.name))
                })?;
                Ok((file.name, text))
            })
            .collect::<Result<BTreeMap<_, _>, SupplyError>>()?;
        let report = match root.verify(&id, &version) {
            Ok(report) => serde_json::to_value(report).map_err(|error| {
                SupplyError::Invalid(format!("serialize extension verification report: {error}"))
            })?,
            Err(error @ SupplyError::Verification { .. }) => verification_report(&error),
            Err(error) => return Err(error),
        };
        Ok(PackDetail {
            id,
            version,
            files,
            report,
        })
    })
    .await
    .map(Json)
}

pub async fn stage(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Result<Json<StageRequest>, JsonRejection>,
) -> Result<Json<StageReport>, GuiError> {
    let root = supply_root(&state)?;
    require_trial(&state, &headers, true)?;
    let Json(request) = body.map_err(invalid_json)?;
    let files = request
        .files
        .into_iter()
        .map(|(name, text)| StagedFile {
            name,
            bytes: text.into_bytes(),
        })
        .collect::<Vec<_>>();
    run_supply(move || root.stage(&request.id, &request.version, &files, Actor::Gui))
        .await
        .map(Json)
}

pub async fn verify(
    State(state): State<AppState>,
    Path((id, version)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<StageReport>, GuiError> {
    let root = supply_root(&state)?;
    require_trial(&state, &headers, true)?;
    run_supply(move || root.verify_recorded(&id, &version, Actor::Gui))
        .await
        .map(Json)
}

pub async fn pin(
    State(state): State<AppState>,
    Path((id, version)): Path<(String, String)>,
    headers: HeaderMap,
    body: Result<Json<PinRequest>, JsonRejection>,
) -> Result<StatusCode, GuiError> {
    let root = supply_root(&state)?;
    require_trial(&state, &headers, true)?;
    let Json(request) = body.map_err(invalid_json)?;
    run_supply(move || root.pin(&id, &version, &request.hash, Actor::Gui)).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn retire(
    State(state): State<AppState>,
    Path((id, version)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<StatusCode, GuiError> {
    let root = supply_root(&state)?;
    require_trial(&state, &headers, true)?;
    run_supply(move || root.retire(&id, &version, Actor::Gui)).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn supply_root(state: &AppState) -> Result<SupplyRoot, GuiError> {
    let root = state.extension_root.as_deref().ok_or_else(|| {
        GuiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "extensions_disabled",
            "extension supply is disabled; configure --extension-root",
        )
    })?;
    SupplyRoot::open(root).map_err(supply_error)
}

async fn run_supply<T, F>(operation: F) -> Result<T, GuiError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, SupplyError> + Send + 'static,
{
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|error| {
            GuiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "extension_supply_failed",
                format!("join extension supply task: {error}"),
            )
        })?
        .map_err(supply_error)
}

fn invalid_json(error: JsonRejection) -> GuiError {
    let message = error.body_text();
    let response = error.into_response();
    GuiError::new(
        if response.status() == StatusCode::PAYLOAD_TOO_LARGE {
            StatusCode::PAYLOAD_TOO_LARGE
        } else {
            StatusCode::BAD_REQUEST
        },
        "extension_invalid_request",
        message,
    )
}

fn supply_error(error: SupplyError) -> GuiError {
    match error {
        error @ SupplyError::Verification { .. } => GuiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "extension_verification_failed",
            error.to_string(),
        )
        .with_report(verification_report(&error)),
        SupplyError::Conflict(message) => {
            GuiError::new(StatusCode::CONFLICT, "extension_conflict", message)
        }
        SupplyError::Invalid(message) => GuiError::new(
            StatusCode::BAD_REQUEST,
            "extension_invalid_request",
            message,
        ),
        SupplyError::NotFound { id, version } => GuiError::new(
            StatusCode::NOT_FOUND,
            "extension_invalid_request",
            format!("pack `{id}@{version}` is not supplied by the extension root"),
        ),
        SupplyError::Root { .. } => GuiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "extensions_disabled",
            error.to_string(),
        ),
        SupplyError::Io { .. } => GuiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "extension_supply_failed",
            error.to_string(),
        ),
    }
}

fn verification_report(error: &SupplyError) -> serde_json::Value {
    let SupplyError::Verification {
        id,
        version,
        hash,
        reason,
    } = error
    else {
        return serde_json::Value::Null;
    };
    serde_json::json!({
        "status": "failed",
        "id": id,
        "version": version,
        "hash": hash,
        "reason": reason,
    })
}
