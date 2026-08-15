use std::collections::BTreeMap;
use std::path::{Path as FilePath, PathBuf};
use std::process::{Command, Stdio};

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use commandagent::planner::profile::ProfileId;
use commandagent::tui::boundary_shell::ambiguity::{
    ClassifierProvenance, ProposalStatus, RouteProposal,
};
use commandagent::tui::boundary_shell::confirmation::{
    ConfirmationIdentity, ExecutionPins, PackSelection, load_latest_confirmation,
};
use commandagent::tui::boundary_shell::presentation::render_gate_one;
use commandagent::tui::boundary_shell::route::{
    DeterministicResolution, ExplicitRouteBinding, RouteRequest, admitted_profiles,
    deterministic_route,
};
use commandagent::tui::boundary_shell::{BoundaryShell, sheet};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::AppState;

const MAX_GOAL_BYTES: usize = 16 * 1024;
const MAX_FIELD_BYTES: usize = 256;
const MAX_EVENTS_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SessionSpec {
    goal: String,
    profile: String,
    provider: String,
    model: String,
    planner_provider: String,
    planner_model: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateSessionRequest {
    #[serde(flatten)]
    spec: SessionSpec,
    confirmation_hash: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SessionProposal {
    confirmation_required: bool,
    card_hash: String,
    card_markdown: String,
    identity: ConfirmationIdentity,
}

#[derive(Debug, Serialize)]
pub struct CreatedSession {
    id: String,
    gate: &'static str,
    status: &'static str,
    events_path: String,
}

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
    gate: String,
    status: String,
    verdict: Option<String>,
    assurance: Option<String>,
    phases: Vec<PhaseStatus>,
    event_count: usize,
    acceptance_sheet: Option<String>,
    section5: Option<String>,
    events_path: String,
}

#[derive(Debug)]
pub struct SessionError {
    status: StatusCode,
    message: String,
}

impl IntoResponse for SessionError {
    fn into_response(self) -> Response {
        (
            self.status,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            Json(serde_json::json!({ "error": self.message })),
        )
            .into_response()
    }
}

pub async fn proposal(
    State(state): State<AppState>,
    Json(spec): Json<SessionSpec>,
) -> Result<Json<SessionProposal>, SessionError> {
    let (_, identity, card_markdown) = gate_one(&state, &spec, proposal_confirmation_root(&state))?;
    Ok(Json(SessionProposal {
        confirmation_required: true,
        card_hash: identity.card_hash().map_err(internal)?,
        card_markdown,
        identity,
    }))
}

pub async fn create(
    State(state): State<AppState>,
    Json(request): Json<CreateSessionRequest>,
) -> Result<impl IntoResponse, SessionError> {
    let Some(confirmation_hash) = request.confirmation_hash.as_deref() else {
        return Err(SessionError {
            status: StatusCode::PRECONDITION_REQUIRED,
            message: "Gate 1 confirmation_hash is required before dispatch".to_string(),
        });
    };
    let id = Uuid::now_v7().to_string();
    let paths = SessionPaths::new(&state.execution_root, &id);
    let (mut shell, identity, _) = gate_one(&state, &request.spec, paths.confirmation_root())?;
    let expected_hash = identity.card_hash().map_err(internal)?;
    if confirmation_hash != expected_hash {
        return Err(SessionError {
            status: StatusCode::PRECONDITION_FAILED,
            message: "Gate 1 card changed; request and confirm the current card".to_string(),
        });
    }
    shell.confirm(confirmation_hash).map_err(bad_request)?;
    let events_path = paths.events_path();
    shell
        .dispatch(|confirmed| {
            spawn_cli(&state, &paths, confirmed)?;
            Ok("delegated".to_string())
        })
        .map_err(internal)?;
    Ok((
        StatusCode::ACCEPTED,
        Json(CreatedSession {
            id,
            gate: "gate_2",
            status: "starting",
            events_path: relative_path(&state.execution_root, &events_path),
        }),
    ))
}

pub async fn status(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<PolledSession>, SessionError> {
    require_session_id(&id)?;
    let paths = SessionPaths::new(&state.execution_root, &id);
    let confirmed = load_latest_confirmation(&paths.confirmation_root())
        .map_err(internal)?
        .ok_or_else(|| not_found("session confirmation was not found"))?;
    let events_path = paths.events_path();
    let text = read_events(&events_path).await?;
    let events = parse_events(&text)?;
    let terminal = latest_event(&events, "tui_command_stop");
    let run_stop = latest_event(&events, "run_stop");
    let terminal_seen = terminal.is_some() || run_stop.is_some();
    let command_succeeded = terminal
        .and_then(|event| event.get("ok"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let generated = terminal
        .map(|_| sheet::generate(confirmed.identity(), Some(&events_path), command_succeeded))
        .transpose()
        .map_err(internal)?;
    let verdict = latest_event(&events, "ultra_final_acceptance")
        .and_then(|event| string(event, "verdict"))
        .or_else(|| terminal.and_then(|event| string(event, "assurance_level")))
        .map(str::to_string);
    let assurance = terminal
        .and_then(|event| string(event, "assurance_level"))
        .map(str::to_string);
    let gate = match generated.as_ref() {
        Some(sheet) if sheet.full => "gate_3",
        Some(_) => "gate_4",
        None if terminal_seen => "gate_4",
        None => "gate_2",
    };
    let status = terminal
        .and_then(|event| string(event, "status"))
        .or_else(|| run_stop.and_then(|event| string(event, "status")))
        .unwrap_or(if events.is_empty() {
            "starting"
        } else {
            "running"
        });
    Ok(Json(PolledSession {
        id,
        gate: gate.to_string(),
        status: status.to_string(),
        verdict,
        assurance,
        phases: phase_statuses(&events),
        event_count: events.len(),
        acceptance_sheet: generated.as_ref().map(|sheet| sheet.markdown.clone()),
        section5: generated.and_then(|sheet| sheet.section5),
        events_path: relative_path(&state.execution_root, &events_path),
    }))
}

fn gate_one(
    state: &AppState,
    spec: &SessionSpec,
    confirmation_root: PathBuf,
) -> Result<(BoundaryShell, ConfirmationIdentity, String), SessionError> {
    validate_spec(spec)?;
    let profile = ProfileId::parse(&spec.profile);
    if !admitted_profiles().contains(&profile) {
        return Err(unprocessable(format!(
            "profile `{}` is not admitted for Gate 1",
            spec.profile
        )));
    }
    let deterministic = deterministic_route(RouteRequest {
        request: &spec.goal,
        workspace: &state.execution_root,
        explicit: ExplicitRouteBinding {
            profile: Some(profile),
            ..ExplicitRouteBinding::default()
        },
    });
    if deterministic.resolution != DeterministicResolution::Unique {
        let candidates = deterministic
            .candidates
            .iter()
            .map(|candidate| {
                format!(
                    "{} × {} × {}",
                    candidate.profile,
                    candidate.intent.as_str(),
                    candidate.family
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        return Err(unprocessable(format!(
            "Gate 1 requires one deterministic registered route; candidates: {candidates}"
        )));
    }
    let proposal = RouteProposal {
        selected: deterministic.candidates.first().cloned(),
        alternatives: deterministic.candidates,
        classifier: ClassifierProvenance {
            used: false,
            provider: spec.planner_provider.clone(),
            model: spec.planner_model.clone(),
            prompt_version: "g1-gui-deterministic-v1",
            candidate_keys: Vec::new(),
            raw_response_hash: None,
            parse_reason: "deterministic_unique".to_string(),
        },
        status: ProposalStatus::AwaitingConfirmation,
        confirmation_required: true,
    };
    let pins = ExecutionPins {
        planner_provider: spec.planner_provider.clone(),
        planner_model: spec.planner_model.clone(),
        executor_provider: spec.provider.clone(),
        executor_model: spec.model.clone(),
        preset: "profile".to_string(),
    };
    let mut shell = BoundaryShell::new(confirmation_root, None);
    let identity = shell
        .begin_gate_one(
            proposal,
            spec.goal.clone(),
            &state.execution_root,
            pins,
            PackSelection::None,
        )
        .map_err(unprocessable)?
        .clone();
    let card = render_gate_one(&identity, &state.repository_root).map_err(internal)?;
    Ok((shell, identity, card))
}

fn spawn_cli(
    state: &AppState,
    paths: &SessionPaths,
    identity: &ConfirmationIdentity,
) -> anyhow::Result<()> {
    let mut command = Command::new(&state.commandagent_bin);
    command
        .current_dir(&state.execution_root)
        .env("COMMANDAGENT_EVAL_EVENTS", paths.events_path())
        .args(["--yes", "--quiet", "--footer", "off", "--stream", "off"])
        .arg("--cwd")
        .arg(&state.execution_root)
        .arg("--state-dir")
        .arg(paths.state_root())
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
        .arg("--ultra-plan-run")
        .arg("--")
        .arg(&identity.request)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}

fn validate_spec(spec: &SessionSpec) -> Result<(), SessionError> {
    if spec.goal.trim().is_empty() || spec.goal.len() > MAX_GOAL_BYTES {
        return Err(unprocessable(format!(
            "goal must contain 1..={MAX_GOAL_BYTES} UTF-8 bytes"
        )));
    }
    for (name, value) in [
        ("profile", &spec.profile),
        ("provider", &spec.provider),
        ("model", &spec.model),
        ("planner_provider", &spec.planner_provider),
        ("planner_model", &spec.planner_model),
    ] {
        if value.trim().is_empty() || value.len() > MAX_FIELD_BYTES {
            return Err(unprocessable(format!(
                "{name} must contain 1..={MAX_FIELD_BYTES} UTF-8 bytes"
            )));
        }
    }
    for provider in [&spec.provider, &spec.planner_provider] {
        if !matches!(provider.as_str(), "ollama" | "openai" | "gemini") {
            return Err(unprocessable(format!(
                "provider `{provider}` is not admitted"
            )));
        }
    }
    Ok(())
}

fn phase_statuses(events: &[Value]) -> Vec<PhaseStatus> {
    let mut phases = BTreeMap::<(u64, String), PhaseStatus>::new();
    for event in events {
        let Some(id) = string(event, "phase_id") else {
            continue;
        };
        let index = event
            .get("phase_index")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let total = event
            .get("total_phases")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let event_name = string(event, "event").unwrap_or("unknown");
        let phase = phases
            .entry((index, id.to_string()))
            .or_insert_with(|| PhaseStatus {
                id: id.to_string(),
                index,
                total,
                stage: "queued".to_string(),
                status: "pending".to_string(),
            });
        phase.stage = string(event, "stage").unwrap_or(event_name).to_string();
        phase.status = match event_name {
            "ultra_phase_complete" => "completed",
            "ultra_phase_failed" => "failed",
            _ => "running",
        }
        .to_string();
    }
    phases.into_values().collect()
}

async fn read_events(path: &FilePath) -> Result<String, SessionError> {
    let metadata = match tokio::fs::metadata(path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(error) => return Err(internal(error)),
    };
    if metadata.len() > MAX_EVENTS_BYTES {
        return Err(SessionError {
            status: StatusCode::PAYLOAD_TOO_LARGE,
            message: "session event stream exceeds the 4 MiB polling limit".to_string(),
        });
    }
    tokio::fs::read_to_string(path).await.map_err(internal)
}

fn parse_events(text: &str) -> Result<Vec<Value>, SessionError> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).map_err(internal))
        .collect()
}

fn latest_event<'a>(events: &'a [Value], name: &str) -> Option<&'a Value> {
    events
        .iter()
        .rev()
        .find(|event| string(event, "event") == Some(name))
}

fn string<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn require_session_id(id: &str) -> Result<(), SessionError> {
    let parsed = Uuid::parse_str(id).map_err(|_| not_found("invalid session id"))?;
    if parsed.to_string() != id {
        return Err(not_found("invalid session id"));
    }
    Ok(())
}

fn relative_path(root: &FilePath, path: &FilePath) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn proposal_confirmation_root(state: &AppState) -> PathBuf {
    state.execution_root.join(".anvil/gui-proposal-preview")
}

struct SessionPaths {
    run_root: PathBuf,
}

impl SessionPaths {
    fn new(workspace: &FilePath, id: &str) -> Self {
        Self {
            run_root: workspace.join(".anvil/runs").join(id),
        }
    }

    fn state_root(&self) -> PathBuf {
        self.run_root.join("state")
    }

    fn confirmation_root(&self) -> PathBuf {
        self.state_root().join("boundary-confirmations")
    }

    fn events_path(&self) -> PathBuf {
        self.run_root.join("events.jsonl")
    }
}

fn unprocessable(message: impl ToString) -> SessionError {
    SessionError {
        status: StatusCode::UNPROCESSABLE_ENTITY,
        message: message.to_string(),
    }
}

fn bad_request(error: impl ToString) -> SessionError {
    SessionError {
        status: StatusCode::BAD_REQUEST,
        message: error.to_string(),
    }
}

fn not_found(message: impl ToString) -> SessionError {
    SessionError {
        status: StatusCode::NOT_FOUND,
        message: message.to_string(),
    }
}

fn internal(error: impl ToString) -> SessionError {
    SessionError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: error.to_string(),
    }
}
