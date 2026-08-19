use std::path::Path;

use axum::{Json, extract::State};
use serde::Serialize;

use super::{AppState, workspace_policy::RuntimeStatus};

#[derive(Debug, Serialize)]
pub struct Response {
    #[serde(flatten)]
    runtime: RuntimeStatus,
    prerequisites: Prerequisites,
}

#[derive(Debug, Serialize)]
struct Prerequisites {
    execution_root: Prerequisite,
    commandagent_binary: Prerequisite,
    trial_authentication: Prerequisite,
}

#[derive(Debug, Serialize)]
struct Prerequisite {
    status: &'static str,
    detail: String,
}

pub async fn get(State(state): State<AppState>) -> Json<Response> {
    let authentication_enabled = state.trial_access.authentication_enabled();
    Json(Response {
        runtime: state.trial_workspace.runtime_status(authentication_enabled),
        prerequisites: Prerequisites {
            execution_root: execution_root(&state),
            commandagent_binary: commandagent_binary(&state.commandagent_bin),
            trial_authentication: if authentication_enabled {
                Prerequisite {
                    status: "action_required",
                    detail: "Trial フォームで、このタブ用のアクセストークン入力が必要です。"
                        .to_string(),
                }
            } else {
                Prerequisite {
                    status: "ready",
                    detail: "ローカルのトークン認証なしモードです。".to_string(),
                }
            },
        },
    })
}

fn execution_root(state: &AppState) -> Prerequisite {
    if state.trial_workspace.configured_path().is_none() {
        return Prerequisite {
            status: "unconfigured",
            detail: "--execution-root が未設定です。GUI を起動し直して設定してください。"
                .to_string(),
        };
    }
    match state.trial_workspace.require_current() {
        Ok(path) => Prerequisite {
            status: "ready",
            detail: path.display().to_string(),
        },
        Err(detail) => Prerequisite {
            status: "action_required",
            detail,
        },
    }
}

fn commandagent_binary(path: &Path) -> Prerequisite {
    let ready = std::fs::metadata(path)
        .map(|metadata| metadata.is_file() && executable(&metadata))
        .unwrap_or(false);
    Prerequisite {
        status: if ready { "ready" } else { "action_required" },
        detail: if ready {
            path.display().to_string()
        } else {
            format!(
                "{} を実行できません。--commandagent-bin を確認してください。",
                path.display()
            )
        },
    }
}

#[cfg(unix)]
fn executable(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;

    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn executable(_: &std::fs::Metadata) -> bool {
    true
}
