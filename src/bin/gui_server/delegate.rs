use std::path::Path;
use std::process::{Child, Command, Stdio};

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use commandagent::planner::pack::catalog::PackLocator;
use commandagent::tui::boundary_shell::confirmation::{ConfirmationIdentity, PackSelection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::AppState;
use super::gate_one::{SessionSpec, gate_one};
use super::session_paths::{SessionPaths, relative_path};
use super::sessions::{SessionError, bad_request, internal, require_trial, workspace_conflict};
use super::workspace_policy::TrialWorkspace;

const DELEGATE_PARENT_ENV_ALLOWLIST: &[&str] = &[
    "PATH",
    "HOME",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "TERM",
    "TMPDIR",
    "TZ",
    "USER",
    "LOGNAME",
    "OPENAI_API_KEY",
    "GEMINI_API_KEY",
    "LM_STUDIO_API_TOKEN",
];
const DELEGATION_WORKSPACE_CHANGED: &str = "Gate 1 workspace changed before CLI delegation";
const CONTINUATION_WORKSPACE_CHANGED: &str = "Gate 1 workspace changed before CLI continuation";
const PACK_DIRECTORY_ENV: &str = "COMMANDAGENT_PACK_DIRECTORY";
const PACK_ID_ENV: &str = "COMMANDAGENT_PACK_ID";
const PACK_VERSION_ENV: &str = "COMMANDAGENT_PACK_VERSION";
const PACK_HASH_ENV: &str = "COMMANDAGENT_PACK_HASH";

pub(super) fn check_binary(path: &Path) -> anyhow::Result<String> {
    let mut command = Command::new(path);
    command.env_clear();
    restore_allowed_environment(&mut command);
    let output = command
        .arg("--version")
        .stdin(Stdio::null())
        .output()
        .map_err(|cause| anyhow::anyhow!("run {} --version: {cause}", path.display()))?;
    if !output.status.success() {
        anyhow::bail!("{} --version exited with {}", path.display(), output.status);
    }
    let version = String::from_utf8(output.stdout)
        .map_err(|_| anyhow::anyhow!("{} --version returned non-UTF-8 output", path.display()))?;
    let version = version.trim();
    if version.is_empty() {
        anyhow::bail!("{} --version returned no output", path.display());
    }
    Ok(version.to_string())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateSessionRequest {
    #[serde(flatten)]
    spec: SessionSpec,
    confirmation_hash: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreatedSession {
    id: String,
    started_epoch_seconds: u64,
    gate: &'static str,
    status: &'static str,
    events_path: String,
}

pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateSessionRequest>,
) -> Result<impl IntoResponse, SessionError> {
    let workspace = require_trial(&state, &headers, true)?;
    let Some(confirmation_hash) = request.confirmation_hash.as_deref() else {
        return Err(super::error_response::GuiError::new(
            StatusCode::PRECONDITION_REQUIRED,
            "trial_confirmation_required",
            "Gate 1 confirmation_hash is required before dispatch",
        ));
    };
    let id = Uuid::now_v7().to_string();
    let paths = SessionPaths::new(&workspace, &id);
    let (mut shell, identity, _) =
        gate_one(&state, &request.spec, &workspace, paths.confirmation_root())?;
    let expected_hash = identity.card_hash().map_err(internal)?;
    if confirmation_hash != expected_hash {
        return Err(super::error_response::GuiError::new(
            StatusCode::PRECONDITION_FAILED,
            "trial_confirmation_stale",
            "Gate 1 card changed; request and confirm the current card",
        ));
    }
    state
        .trial_workspace
        .acquire(&id)
        .map_err(workspace_conflict)?;
    if let Err(error) = shell.confirm(confirmation_hash) {
        state.trial_workspace.cancel_start(&id);
        return Err(bad_request(error));
    }
    let events_path = paths.events_path();
    let dispatch = shell.dispatch(|confirmed| {
        let child = spawn_cli(&state, &paths, confirmed)?;
        monitor_cli(
            state.trial_workspace.clone(),
            id.clone(),
            events_path.clone(),
            child,
        );
        Ok("delegated".to_string())
    });
    if let Err(error) = dispatch {
        return match paths.rollback_unstarted() {
            Ok(()) => {
                state.trial_workspace.cancel_start(&id);
                Err(internal(format!("{error:#}")))
            }
            Err(rollback_error) => {
                state
                    .trial_workspace
                    .complete_from_events(&id, &events_path);
                Err(internal(format!(
                    "{error:#}; failed to roll back unstarted session {id}: {rollback_error:#}"
                )))
            }
        };
    }
    let started_epoch_seconds =
        super::sessions::started_epoch_seconds(&id, paths.run_root(), &events_path).await;
    Ok((
        StatusCode::ACCEPTED,
        Json(CreatedSession {
            id,
            started_epoch_seconds,
            gate: "gate_2",
            status: "starting",
            events_path: relative_path(&workspace, &events_path),
        }),
    ))
}

fn spawn_cli(
    state: &AppState,
    paths: &SessionPaths,
    identity: &ConfirmationIdentity,
) -> anyhow::Result<Child> {
    paths.create_execution_workspace()?;
    let result = delegated_cli_command(state, paths, identity, DELEGATION_WORKSPACE_CHANGED)
        .and_then(|mut command| {
            command
                .arg("--ultra-plan-run")
                .arg("--")
                .arg(&identity.request)
                .spawn()
                .map_err(|cause| {
                    anyhow::anyhow!(
                        "failed to spawn delegated CLI binary {}: {cause}",
                        state.commandagent_bin.display()
                    )
                })
        });
    match result {
        Ok(child) => Ok(child),
        Err(error) => match paths.rollback_execution_workspace() {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(anyhow::anyhow!(
                "{error:#}; failed to roll back session execution workspace: {rollback_error:#}"
            )),
        },
    }
}

fn monitor_cli(
    lease: TrialWorkspace,
    session_id: String,
    events_path: std::path::PathBuf,
    mut child: Child,
) {
    std::thread::spawn(move || {
        if let Err(error) = child.wait() {
            eprintln!("GUI delegated CLI wait failed: {error:#}");
        }
        lease.complete_from_events(&session_id, &events_path);
    });
}

pub(super) fn run_cli_continuation(
    state: &AppState,
    paths: &SessionPaths,
    identity: &ConfirmationIdentity,
    continuation: &commandagent::tui::boundary_shell::directive::DirectiveContinuation,
) -> anyhow::Result<String> {
    let mut command =
        delegated_cli_command(state, paths, identity, CONTINUATION_WORKSPACE_CHANGED)?;
    let status = command
        .arg("--run-ultra-plan")
        .arg(&continuation.plan_path)
        .status()?;
    if !status.success() {
        anyhow::bail!("delegated CLI exited with {status}");
    }
    Ok("delegated directive completed".to_string())
}

fn delegated_cli_command(
    state: &AppState,
    paths: &SessionPaths,
    identity: &ConfirmationIdentity,
    workspace_changed_message: &'static str,
) -> anyhow::Result<Command> {
    let workspace = state
        .trial_workspace
        .require_current()
        .map_err(anyhow::Error::msg)?;
    if identity.workspace != workspace.to_string_lossy() {
        anyhow::bail!(workspace_changed_message);
    }
    let execution_workspace = paths.require_execution_workspace()?;
    let mut command = Command::new(&state.commandagent_bin);
    command.env_clear();
    restore_allowed_environment(&mut command);
    let confirmed_pack_applied = apply_confirmed_pack(&mut command, state, identity)?;
    command
        .current_dir(&execution_workspace)
        .env("COMMANDAGENT_EVAL_EVENTS", paths.events_path())
        .args([
            "--allow",
            "read,write,bash:verify",
            "--quiet",
            "--footer",
            "off",
            "--stream",
            "off",
        ])
        .arg("--cwd")
        .arg(&execution_workspace)
        .arg("--state-dir")
        .arg(paths.state_root())
        .args(["--ollama-host", &state.ollama_host])
        .args(["--lm-studio-host", &state.lm_studio_host])
        .args(["--profile", identity.profile.as_str()])
        .args(["--intent", identity.intent.as_str()])
        .args(["--provider", identity.pins.executor_provider.as_str()])
        .args(["--model", identity.pins.executor_model.as_str()])
        .args([
            "--planner-provider",
            identity.pins.planner_provider.as_str(),
        ])
        .args(["--planner-model", identity.pins.planner_model.as_str()])
        .args(["--plan-preset", identity.pins.preset.as_str()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(think) = identity.pins.think {
        command.arg(format!("--think={}", think.as_str()));
    }
    if !confirmed_pack_applied && let Some(extension_root) = state.extension_root.as_deref() {
        command.arg("--extension-root").arg(extension_root);
    }
    Ok(command)
}

fn restore_allowed_environment(command: &mut Command) {
    for name in DELEGATE_PARENT_ENV_ALLOWLIST {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
}

fn apply_confirmed_pack(
    command: &mut Command,
    state: &AppState,
    identity: &ConfirmationIdentity,
) -> anyhow::Result<bool> {
    let PackSelection::Pinned {
        id,
        version,
        hash,
        source,
        ..
    } = &identity.pack
    else {
        return Ok(false);
    };
    let locator =
        PackLocator::with_extension_root(&state.repository_root, state.extension_root.clone());
    let located = locator.locate_pinned_from(*source, id, version, Some(hash))?;
    let directory = located.directory;
    let cli_supply_root = match source {
        commandagent::planner::pack::catalog::PackSource::Local => state
            .extension_root
            .clone()
            .ok_or_else(|| anyhow::anyhow!("confirmed local pack requires an extension root"))?,
        commandagent::planner::pack::catalog::PackSource::Admitted
        | commandagent::planner::pack::catalog::PackSource::Repository => {
            state.repository_root.join("packs")
        }
    };
    command
        .args(["--extension-root"])
        .arg(cli_supply_root)
        .args(["--pack", &format!("{id}@{version}")])
        .args(["--pack-hash", hash])
        .env(PACK_DIRECTORY_ENV, directory)
        .env(PACK_ID_ENV, id)
        .env(PACK_VERSION_ENV, version)
        .env(PACK_HASH_ENV, hash);
    Ok(true)
}
