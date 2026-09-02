use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use commandagent::planner::pack::Actor;
use commandagent::planner::profile_manifest::supply::{
    ProfileCatalogEntry, ProfilePreview, ProfileRegistrationReport, ProfileSupplyError,
    ProfileSupplyRoot,
};
use serde::Deserialize;

use super::AppState;
use super::error_response::GuiError;
use super::trial_access::AccessError;

pub const MAX_BODY_BYTES: usize =
    commandagent::planner::profile_manifest::supply::MAX_PROFILE_BODY_BYTES;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewRequest {
    path: String,
    content: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisterRequest {
    path: String,
    content: String,
    expected_hash: String,
}

pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ProfileCatalogEntry>>, GuiError> {
    require_access(&state, &headers, false)?;
    let root = supply_root(&state)?;
    run_supply(move || root.catalog()).await.map(Json)
}

pub async fn preview(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Result<Json<PreviewRequest>, JsonRejection>,
) -> Result<Json<ProfilePreview>, GuiError> {
    require_access(&state, &headers, true)?;
    let root = supply_root(&state)?;
    let Json(request) = body.map_err(invalid_json)?;
    run_supply(move || root.preview(&request.path, request.content.as_bytes()))
        .await
        .map(Json)
}

pub async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Result<Json<RegisterRequest>, JsonRejection>,
) -> Result<Json<ProfileRegistrationReport>, GuiError> {
    require_access(&state, &headers, true)?;
    let root = supply_root(&state)?;
    let Json(request) = body.map_err(invalid_json)?;
    run_supply(move || {
        root.register(
            &request.path,
            request.content.as_bytes(),
            &request.expected_hash,
            Actor::Gui,
        )
    })
    .await
    .map(Json)
}

fn require_access(
    state: &AppState,
    headers: &HeaderMap,
    require_origin: bool,
) -> Result<(), GuiError> {
    state
        .trial_access
        .authorize(headers, require_origin)
        .map_err(|error| match error {
            AccessError::Unauthorized => GuiError::new(
                StatusCode::UNAUTHORIZED,
                "profile_auth_failed",
                "認証に失敗しました。GUI の Trial トークンを確認して再試行してください。",
            ),
            AccessError::ForbiddenOrigin => GuiError::new(
                StatusCode::FORBIDDEN,
                "profile_origin_not_allowed",
                "この Origin から profile を登録できません。GUI_TRIAL_ALLOWED_ORIGINS と現在の URL を確認してください。",
            ),
        })
}

fn supply_root(state: &AppState) -> Result<ProfileSupplyRoot, GuiError> {
    let root = state.extension_root.as_deref().ok_or_else(|| {
        GuiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "extensions_disabled",
            "profile 供給は無効です。GUI サーバーを --extension-root 付きで再起動してください。",
        )
    })?;
    ProfileSupplyRoot::open(root).map_err(profile_error)
}

async fn run_supply<T, F>(operation: F) -> Result<T, GuiError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, ProfileSupplyError> + Send + 'static,
{
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|error| {
            GuiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "profile_io_failed",
                format!(
                    "profile 供給処理を完了できませんでした。GUI サーバーのログを確認してください: {error}"
                ),
            )
        })?
        .map_err(profile_error)
}

fn invalid_json(error: JsonRejection) -> GuiError {
    let response = error.into_response();
    if response.status() == StatusCode::PAYLOAD_TOO_LARGE {
        GuiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "profile_body_too_large",
            "profile 文書が上限を超えています。manifest / overlay を 256 KiB 以下にしてください。",
        )
    } else {
        GuiError::new(
            StatusCode::BAD_REQUEST,
            "profile_invalid_request",
            "profile 要求の JSON が不正です。path、content、確認済み hash を確認してください。",
        )
    }
}

fn profile_error(error: ProfileSupplyError) -> GuiError {
    match error {
        ProfileSupplyError::Root(reason) => GuiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "extensions_disabled",
            format!(
                "extension root を使用できません。所有権、0700 権限、symlink でないことを確認してください。詳細: {reason}"
            ),
        ),
        ProfileSupplyError::InvalidPath(reason) | ProfileSupplyError::Validation(reason) => {
            GuiError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "profile_validation_failed",
                format!(
                    "profile を検証できません。相対 path、closed schema、capability、additive overlay を確認してください。詳細: {reason}"
                ),
            )
        }
        ProfileSupplyError::TooLarge { .. } => GuiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "profile_body_too_large",
            "profile 文書が上限を超えています。manifest / overlay を 256 KiB 以下にしてください。",
        ),
        ProfileSupplyError::Conflict(reason) if reason.contains("preview の exact hash") => {
            GuiError::new(
                StatusCode::CONFLICT,
                "profile_confirmation_stale",
                format!(
                    "確認済み hash が現在の本文と一致しません。preview をやり直してください。詳細: {reason}"
                ),
            )
        }
        ProfileSupplyError::Conflict(reason) => GuiError::new(
            StatusCode::CONFLICT,
            "profile_conflict",
            format!(
                "profile を保存できません。既存の built-in / 外部 ID またはファイル内容と競合しています。詳細: {reason}"
            ),
        ),
        ProfileSupplyError::Io(reason) => GuiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "profile_io_failed",
            format!(
                "profile の保存または journal 記録に失敗しました。権限、空き容量、managed path を確認してください。詳細: {reason}"
            ),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::response::IntoResponse;

    #[tokio::test]
    async fn profile_supply_errors_keep_stable_http_mappings() {
        let cases = [
            (
                ProfileSupplyError::Root("private root".to_string()),
                StatusCode::SERVICE_UNAVAILABLE,
                "extensions_disabled",
            ),
            (
                ProfileSupplyError::InvalidPath("escape".to_string()),
                StatusCode::UNPROCESSABLE_ENTITY,
                "profile_validation_failed",
            ),
            (
                ProfileSupplyError::Validation("unknown capability".to_string()),
                StatusCode::UNPROCESSABLE_ENTITY,
                "profile_validation_failed",
            ),
            (
                ProfileSupplyError::TooLarge { limit: 1 },
                StatusCode::PAYLOAD_TOO_LARGE,
                "profile_body_too_large",
            ),
            (
                ProfileSupplyError::Conflict("existing bytes".to_string()),
                StatusCode::CONFLICT,
                "profile_conflict",
            ),
            (
                ProfileSupplyError::Conflict(
                    "preview の exact hash と保存要求が一致しません。".to_string(),
                ),
                StatusCode::CONFLICT,
                "profile_confirmation_stale",
            ),
            (
                ProfileSupplyError::Io("disk full".to_string()),
                StatusCode::INTERNAL_SERVER_ERROR,
                "profile_io_failed",
            ),
        ];
        for (error, expected_status, expected_code) in cases {
            let response = profile_error(error).into_response();
            assert_eq!(response.status(), expected_status, "{expected_code}");
            let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(body["code"], expected_code);
            assert!(
                body["error"]
                    .as_str()
                    .is_some_and(|message| !message.is_empty())
            );
        }
    }
}
