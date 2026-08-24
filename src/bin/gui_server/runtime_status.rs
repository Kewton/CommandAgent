use std::path::Path;

use axum::{Json, extract::State};
use serde::Serialize;

use super::{AppState, workspace_policy::RuntimeStatus};

#[derive(Debug, Serialize)]
pub struct Response {
    #[serde(flatten)]
    runtime: RuntimeStatus,
    gui_contract_version: &'static str,
    prerequisites: Prerequisites,
}

#[derive(Debug, Serialize)]
struct Prerequisites {
    execution_root: Prerequisite,
    extension_root: Prerequisite,
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
        gui_contract_version: super::gui_contract::server_contract_version(),
        prerequisites: Prerequisites {
            execution_root: execution_root(&state),
            extension_root: extension_root(&state),
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

fn extension_root(state: &AppState) -> Prerequisite {
    let Some(path) = state.extension_root.as_deref() else {
        return Prerequisite {
            status: "unconfigured",
            detail: "--extension-root が未設定です。GUI を起動し直して設定してください。"
                .to_string(),
        };
    };
    match commandagent::planner::pack::SupplyRoot::open(path) {
        Ok(_) => Prerequisite {
            status: "ready",
            detail: "設定済みの private extension root を利用できます。".to_string(),
        },
        Err(_) => Prerequisite {
            status: "action_required",
            detail: "extension root を利用できません。場所、権限、root 分離を確認して GUI を起動し直してください。"
                .to_string(),
        },
    }
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
        Ok(_) => Prerequisite {
            status: "ready",
            detail: "設定済みの execution root を利用できます。".to_string(),
        },
        Err(_) => Prerequisite {
            status: "action_required",
            detail: "execution root を確認し、GUI を起動し直してください。".to_string(),
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
