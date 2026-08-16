use std::collections::BTreeMap;
use std::path::{Path as FilePath, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, UNIX_EPOCH};

use anyhow::Context;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
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

use super::trial_access::AccessError;
use super::{AppState, trial_options, workspace_policy::TrialWorkspace};

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
    price: BandPrice,
}

#[derive(Debug, Serialize)]
pub struct BandPrice {
    duration_n: usize,
    average_duration_seconds: Option<f64>,
    cost_n: usize,
    average_cost_usd: Option<f64>,
    source: String,
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
    headers: HeaderMap,
    Json(spec): Json<SessionSpec>,
) -> Result<Json<SessionProposal>, SessionError> {
    let workspace = require_trial(&state, &headers, true)?;
    let (_, identity, card_markdown) = gate_one(
        &state,
        &spec,
        &workspace,
        proposal_confirmation_root(&workspace),
    )?;
    let price = band_price(&state.repository_root, &identity).await?;
    Ok(Json(SessionProposal {
        confirmation_required: true,
        card_hash: identity.card_hash().map_err(internal)?,
        card_markdown,
        identity,
        price,
    }))
}

pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateSessionRequest>,
) -> Result<impl IntoResponse, SessionError> {
    let workspace = require_trial(&state, &headers, true)?;
    let Some(confirmation_hash) = request.confirmation_hash.as_deref() else {
        return Err(SessionError {
            status: StatusCode::PRECONDITION_REQUIRED,
            message: "Gate 1 confirmation_hash is required before dispatch".to_string(),
        });
    };
    let id = Uuid::now_v7().to_string();
    let paths = SessionPaths::new(&workspace, &id);
    let (mut shell, identity, _) =
        gate_one(&state, &request.spec, &workspace, paths.confirmation_root())?;
    let expected_hash = identity.card_hash().map_err(internal)?;
    if confirmation_hash != expected_hash {
        return Err(SessionError {
            status: StatusCode::PRECONDITION_FAILED,
            message: "Gate 1 card changed; request and confirm the current card".to_string(),
        });
    }
    state.trial_workspace.acquire(&id).map_err(conflict)?;
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
    Ok((
        StatusCode::ACCEPTED,
        Json(CreatedSession {
            id,
            gate: "gate_2",
            status: "starting",
            events_path: relative_path(&workspace, &events_path),
        }),
    ))
}

pub async fn workspace_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<super::workspace_policy::LeaseSnapshot>, SessionError> {
    require_trial(&state, &headers, false)?;
    state
        .trial_workspace
        .lease_snapshot()
        .map(Json)
        .map_err(internal)
}

pub async fn status(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, SessionError> {
    let workspace = require_trial(&state, &headers, false)?;
    require_session_id(&id)?;
    let paths = SessionPaths::new(&workspace, &id);
    let confirmed = load_latest_confirmation(&paths.confirmation_root())
        .map_err(internal)?
        .ok_or_else(|| not_found("session confirmation was not found"))?;
    let events_path = paths.events_path();
    let etag = status_etag(confirmed.card_hash(), &events_path).await?;
    if if_none_match(&headers, &etag) {
        return Ok(status_response(StatusCode::NOT_MODIFIED, &etag));
    }
    let text = read_events(&events_path).await?;
    let events = parse_events(&text)?;
    let terminal_index = latest_event_index(&events, "tui_command_stop");
    let continuation_index = latest_event_index(&events, "human_directive_continuation_started");
    let terminal_is_current = match (terminal_index, continuation_index) {
        (Some(terminal), Some(continuation)) => terminal > continuation,
        (Some(_), None) => true,
        _ => false,
    };
    let terminal = terminal_index.map(|index| &events[index]);
    let run_stop = latest_event(&events, "run_stop");
    let terminal_seen = terminal_is_current && (terminal.is_some() || run_stop.is_some());
    let command_succeeded = terminal
        .and_then(|event| event.get("ok"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let generated = terminal
        .filter(|_| terminal_is_current)
        .map(|_| sheet::generate(confirmed.identity(), Some(&events_path), command_succeeded))
        .transpose()
        .map_err(internal)?;
    let verdict = terminal_is_current
        .then(|| latest_event(&events, "ultra_final_acceptance"))
        .flatten()
        .and_then(|event| string(event, "verdict"))
        .or_else(|| terminal.and_then(|event| string(event, "assurance_level")))
        .map(str::to_string);
    let assurance = terminal
        .filter(|_| terminal_is_current)
        .and_then(|event| string(event, "assurance_level"))
        .map(str::to_string);
    let gate = match generated.as_ref() {
        Some(sheet) if sheet.full => "gate_3",
        Some(_) => "gate_4",
        None if terminal_seen => "gate_4",
        None => "gate_2",
    };
    let status = if terminal_is_current {
        terminal
            .and_then(|event| string(event, "status"))
            .or_else(|| run_stop.and_then(|event| string(event, "status")))
            .unwrap_or("running")
    } else if events.is_empty() {
        "starting"
    } else {
        "running"
    };
    let session = PolledSession {
        id,
        gate: gate.to_string(),
        status: status.to_string(),
        verdict,
        assurance,
        phases: phase_statuses(&events),
        event_count: events.len(),
        acceptance_sheet: generated.as_ref().map(|sheet| sheet.markdown.clone()),
        section5: generated.and_then(|sheet| sheet.section5),
        events_path: relative_path(&workspace, &events_path),
    };
    let mut response = Json(session).into_response();
    insert_status_headers(&mut response, &etag);
    Ok(response)
}

pub async fn propose_directive(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<DirectiveRequest>,
) -> Result<Json<DirectiveProposal>, SessionError> {
    let workspace = require_trial(&state, &headers, true)?;
    require_session_id(&id)?;
    let paths = SessionPaths::new(&workspace, &id);
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

pub async fn confirm_directive(
    State(state): State<AppState>,
    Path((id, hash)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, SessionError> {
    let workspace = require_trial(&state, &headers, true)?;
    require_session_id(&id)?;
    let paths = SessionPaths::new(&workspace, &id);
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
    state.trial_workspace.acquire(&id).map_err(conflict)?;
    if let Err(error) = shell.confirm_directive(&hash) {
        state.trial_workspace.cancel_start(&id);
        return Err(bad_request(error));
    }
    let continuation =
        shell.prepare_confirmed_continuation(&workspace, &events_path, &identity, &directive);
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

fn gate_one(
    state: &AppState,
    spec: &SessionSpec,
    workspace: &FilePath,
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
        workspace,
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
            workspace,
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
) -> anyhow::Result<Child> {
    let workspace = state
        .trial_workspace
        .require_current()
        .map_err(anyhow::Error::msg)?;
    if identity.workspace != workspace.to_string_lossy() {
        anyhow::bail!("Gate 1 workspace changed before CLI delegation");
    }
    let mut command = Command::new(&state.commandagent_bin);
    command
        .current_dir(&workspace)
        .env("COMMANDAGENT_EVAL_EVENTS", paths.events_path())
        .args(["--yes", "--quiet", "--footer", "off", "--stream", "off"])
        .arg("--cwd")
        .arg(&workspace)
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
        .spawn()
        .map_err(|cause| {
            anyhow::anyhow!(
                "failed to spawn delegated CLI binary {}: {cause}",
                state.commandagent_bin.display()
            )
        })
}

fn monitor_cli(lease: TrialWorkspace, session_id: String, events_path: PathBuf, mut child: Child) {
    std::thread::spawn(move || {
        if let Err(error) = child.wait() {
            eprintln!("GUI delegated CLI wait failed: {error:#}");
        }
        lease.complete_from_events(&session_id, &events_path);
    });
}

fn run_cli_continuation(
    state: &AppState,
    paths: &SessionPaths,
    identity: &ConfirmationIdentity,
    continuation: &commandagent::tui::boundary_shell::directive::DirectiveContinuation,
) -> anyhow::Result<String> {
    let workspace = state
        .trial_workspace
        .require_current()
        .map_err(anyhow::Error::msg)?;
    if identity.workspace != workspace.to_string_lossy() {
        anyhow::bail!("Gate 1 workspace changed before CLI continuation");
    }
    let status = Command::new(&state.commandagent_bin)
        .current_dir(&workspace)
        .env("COMMANDAGENT_EVAL_EVENTS", paths.events_path())
        .args(["--yes", "--quiet", "--footer", "off", "--stream", "off"])
        .arg("--cwd")
        .arg(&workspace)
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
        .arg("--run-ultra-plan")
        .arg(&continuation.plan_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if !status.success() {
        anyhow::bail!("delegated CLI exited with {status}");
    }
    Ok("delegated directive completed".to_string())
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
        if !trial_options::is_admitted_provider(provider) {
            return Err(unprocessable(format!(
                "provider `{provider}` is not admitted"
            )));
        }
    }
    Ok(())
}

fn phase_statuses(events: &[Value]) -> Vec<PhaseStatus> {
    let mut phases = BTreeMap::<(u64, String), PhaseStatus>::new();
    let mut terminal_seen = false;
    for event in events {
        let event_name = string(event, "event").unwrap_or("unknown");
        if matches!(event_name, "tui_command_stop" | "run_stop") {
            terminal_seen = true;
            for phase in phases.values_mut() {
                if matches!(phase.status.as_str(), "pending" | "running") {
                    phase.status = "interrupted".to_string();
                }
            }
            continue;
        }

        let Some(id) = string(event, "phase_id") else {
            continue;
        };
        let Some(effect) = phase_event_effect(event_name) else {
            continue;
        };
        if terminal_seen && matches!(effect, PhaseEventEffect::Status(status) if status != "failed")
        {
            continue;
        }
        let index = event.get("phase_index").and_then(Value::as_u64);
        let key = match index {
            Some(index) => (index, id.to_string()),
            None => {
                let Some(key) = phases
                    .keys()
                    .rev()
                    .find(|(_, phase_id)| phase_id == id)
                    .cloned()
                else {
                    continue;
                };
                key
            }
        };
        if matches!(effect, PhaseEventEffect::StageOnly) && !phases.contains_key(&key) {
            continue;
        }
        let total = event
            .get("total_phases")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let phase_index = key.0;
        let phase = phases.entry(key).or_insert_with(|| PhaseStatus {
            id: id.to_string(),
            index: phase_index,
            total,
            stage: "queued".to_string(),
            status: "pending".to_string(),
        });
        match effect {
            PhaseEventEffect::StageOnly => {
                phase.stage = string(event, "stage").unwrap_or(event_name).to_string();
            }
            PhaseEventEffect::Status(status) => {
                let current_status = phase.status.as_str();
                if current_status == "failed"
                    || (current_status == "completed" && status != "failed")
                    || (current_status == "interrupted" && status != "failed")
                {
                    continue;
                }
                phase.stage = string(event, "stage").unwrap_or(event_name).to_string();
                phase.status = status.to_string();
            }
        }
    }
    phases.into_values().collect()
}

#[derive(Clone, Copy)]
enum PhaseEventEffect {
    Status(&'static str),
    StageOnly,
}

fn phase_event_effect(event_name: &str) -> Option<PhaseEventEffect> {
    match event_name {
        "ultra_phase_complete" => Some(PhaseEventEffect::Status("completed")),
        "ultra_phase_failed" => Some(PhaseEventEffect::Status("failed")),
        "ultra_phase_start"
        | "ultra_phase_scaffold_complete"
        | "ultra_phase_plan_validated"
        | "ultra_phase_execute_complete"
        | "phase_verification_result"
        | "ultra_phase_profile_check" => Some(PhaseEventEffect::Status("running")),
        "ultra_phase_context_attached"
        | "ultra_phase_context_updated"
        | "recovery_prompt_saved" => Some(PhaseEventEffect::StageOnly),
        _ => None,
    }
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

async fn status_etag(card_hash: &str, events_path: &FilePath) -> Result<String, SessionError> {
    let revision = match tokio::fs::metadata(events_path).await {
        Ok(metadata) => {
            let modified = metadata
                .modified()
                .map_err(internal)?
                .duration_since(UNIX_EPOCH)
                .map_err(internal)?
                .as_nanos();
            format!("{}-{modified}", metadata.len())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => "missing".to_string(),
        Err(error) => return Err(internal(error)),
    };
    Ok(format!(
        "W/\"{}-{revision}\"",
        card_hash.trim_start_matches("sha256:")
    ))
}

fn if_none_match(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get_all(header::IF_NONE_MATCH)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .any(|candidate| candidate == "*" || weak_etag_eq(candidate, etag))
}

fn weak_etag_eq(left: &str, right: &str) -> bool {
    left.strip_prefix("W/").unwrap_or(left) == right.strip_prefix("W/").unwrap_or(right)
}

fn status_response(status: StatusCode, etag: &str) -> Response {
    let mut response = status.into_response();
    insert_status_headers(&mut response, etag);
    response
}

fn insert_status_headers(response: &mut Response, etag: &str) {
    response.headers_mut().insert(
        header::ETAG,
        HeaderValue::from_str(etag).expect("generated status ETags are valid header values"),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-cache"),
    );
}

async fn require_current_terminal(path: &FilePath) -> Result<(), SessionError> {
    let events = parse_events(&read_events(path).await?)?;
    let terminal = latest_event_index(&events, "tui_command_stop")
        .ok_or_else(|| conflict("session has not reached Gate 3 or Gate 4"))?;
    if latest_event_index(&events, "human_directive_continuation_started")
        .is_some_and(|continuation| continuation > terminal)
    {
        return Err(conflict(
            "a confirmed directive continuation is still running",
        ));
    }
    Ok(())
}

async fn require_no_pending_directive(path: &FilePath) -> Result<(), SessionError> {
    let events = parse_events(&read_events(path).await?)?;
    let terminal = latest_event_index(&events, "tui_command_stop")
        .ok_or_else(|| conflict("session has not reached Gate 3 or Gate 4"))?;
    if latest_event_index(&events, "human_directive_proposed")
        .is_some_and(|proposal| proposal > terminal)
    {
        return Err(conflict(
            "the current terminal already has a pending directive proposal",
        ));
    }
    Ok(())
}

async fn band_price(
    repository_root: &FilePath,
    identity: &ConfirmationIdentity,
) -> Result<BandPrice, SessionError> {
    let path = repository_root.join(&identity.band_source);
    let text = tokio::fs::read_to_string(&path).await.map_err(internal)?;
    let mut durations = Vec::new();
    let mut costs = Vec::new();
    let mut headers: Option<Vec<String>> = None;
    for line in text.lines() {
        if !line.trim_start().starts_with('|') {
            headers = None;
            continue;
        }
        let cells = markdown_cells(line);
        if cells.iter().all(|cell| {
            let trimmed = cell.trim_matches(['-', ':', ' ']);
            trimmed.is_empty()
        }) {
            continue;
        }
        if cells.iter().any(|cell| cell == "Family") && cells.iter().any(|cell| cell == "Seconds") {
            headers = Some(cells);
            continue;
        }
        let Some(header) = headers.as_ref() else {
            continue;
        };
        if cells.len() != header.len() {
            continue;
        }
        let field = |name: &str| {
            header
                .iter()
                .position(|value| value == name)
                .and_then(|index| cells.get(index))
                .map(String::as_str)
        };
        if field("Family") != Some(identity.task_family.as_str()) {
            continue;
        }
        if let Some(status) = field("Band status")
            && !status.contains(&identity.band_arm)
        {
            continue;
        }
        if let Some(seconds) = field("Seconds").and_then(parse_number) {
            durations.push(seconds);
        }
        if let Some(cost) = field("Cost USD").and_then(parse_number) {
            costs.push(cost);
        }
    }
    Ok(BandPrice {
        duration_n: durations.len(),
        average_duration_seconds: mean(&durations),
        cost_n: costs.len(),
        average_cost_usd: mean(&costs),
        source: identity.band_source.clone(),
    })
}

fn markdown_cells(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect()
}

fn parse_number(value: &str) -> Option<f64> {
    value
        .trim()
        .trim_start_matches('$')
        .replace(',', "")
        .parse()
        .ok()
}

fn mean(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
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

fn latest_event_index(events: &[Value], name: &str) -> Option<usize> {
    events
        .iter()
        .rposition(|event| string(event, "event") == Some(name))
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

fn require_trial(
    state: &AppState,
    headers: &HeaderMap,
    require_origin: bool,
) -> Result<PathBuf, SessionError> {
    if !state.trial_workspace.is_enabled() {
        return Err(SessionError {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: "trial execution is disabled; configure --execution-root".to_string(),
        });
    }
    let workspace = state.trial_workspace.require_current().map_err(conflict)?;
    state
        .trial_access
        .authorize(headers, require_origin)
        .map_err(|error| match error {
            AccessError::Disabled => SessionError {
                status: StatusCode::SERVICE_UNAVAILABLE,
                message: "trial execution authentication is disabled".to_string(),
            },
            AccessError::Unauthorized => SessionError {
                status: StatusCode::UNAUTHORIZED,
                message: "a valid GUI trial bearer token is required".to_string(),
            },
            AccessError::ForbiddenOrigin => SessionError {
                status: StatusCode::FORBIDDEN,
                message: "trial request origin is not allowed".to_string(),
            },
        })?;
    Ok(workspace)
}

fn relative_path(root: &FilePath, path: &FilePath) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn proposal_confirmation_root(workspace: &FilePath) -> PathBuf {
    workspace.join(".anvil/gui-proposal-preview")
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

    fn rollback_unstarted(&self) -> anyhow::Result<()> {
        match std::fs::remove_dir_all(&self.run_root) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error).with_context(|| {
                format!(
                    "remove unstarted session directory {}",
                    self.run_root.display()
                )
            }),
        }
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

fn conflict(message: impl ToString) -> SessionError {
    SessionError {
        status: StatusCode::CONFLICT,
        message: message.to_string(),
    }
}

fn internal(error: impl ToString) -> SessionError {
    SessionError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lm_studio_is_admitted_for_both_session_roles() {
        let spec = SessionSpec {
            goal: "Inspect the workspace".to_string(),
            profile: "generic".to_string(),
            provider: "lm-studio".to_string(),
            model: "qwen/test".to_string(),
            planner_provider: "lm-studio".to_string(),
            planner_model: "qwen/test".to_string(),
        };

        validate_spec(&spec).unwrap();
    }

    #[test]
    fn phase_statuses_attach_unindexed_events_without_creating_ghost_rows() {
        let events = vec![
            serde_json::json!({
                "event": "ultra_phase_start",
                "phase_id": "setup",
                "phase_index": 1,
                "total_phases": 1
            }),
            serde_json::json!({
                "event": "recovery_prompt_saved",
                "phase_id": "setup"
            }),
            serde_json::json!({
                "event": "ultra_phase_start",
                "phase_id": "ghost",
                "total_phases": 1
            }),
        ];

        let phases = phase_statuses(&events);

        assert_eq!(phases.len(), 1);
        assert_eq!(phases[0].index, 1);
        assert_eq!(phases[0].stage, "recovery_prompt_saved");
        assert_eq!(phases[0].status, "running");
    }

    #[test]
    fn phase_statuses_do_not_restore_running_after_completion() {
        let events = vec![
            serde_json::json!({
                "event": "ultra_phase_start",
                "phase_id": "setup",
                "phase_index": 1,
                "total_phases": 1
            }),
            serde_json::json!({
                "event": "ultra_phase_complete",
                "phase_id": "setup",
                "phase_index": 1,
                "total_phases": 1,
                "stage": "complete"
            }),
            serde_json::json!({
                "event": "ultra_phase_context_updated",
                "phase_id": "setup",
                "phase_index": 1,
                "total_phases": 1
            }),
        ];

        let phases = phase_statuses(&events);

        assert_eq!(phases.len(), 1);
        assert!(phases.iter().all(|phase| phase.status != "running"));
        assert_eq!(phases[0].stage, "ultra_phase_context_updated");
        assert_eq!(phases[0].status, "completed");
    }

    #[test]
    fn phase_statuses_keep_failures_terminal() {
        let events = vec![
            serde_json::json!({
                "event": "ultra_phase_start",
                "phase_id": "setup",
                "phase_index": 1,
                "total_phases": 1
            }),
            serde_json::json!({
                "event": "ultra_phase_failed",
                "phase_id": "setup",
                "phase_index": 1,
                "total_phases": 1,
                "stage": "execute"
            }),
            serde_json::json!({
                "event": "recovery_prompt_saved",
                "phase_id": "setup"
            }),
            serde_json::json!({
                "event": "future_phase_annotation",
                "phase_id": "setup",
                "phase_index": 1,
                "total_phases": 1
            }),
        ];

        let phases = phase_statuses(&events);

        assert_eq!(phases.len(), 1);
        assert_eq!(phases[0].stage, "recovery_prompt_saved");
        assert_eq!(phases[0].status, "failed");
    }

    #[test]
    fn phase_statuses_interrupt_running_phases_at_terminal() {
        let events = vec![
            serde_json::json!({
                "event": "ultra_phase_start",
                "phase_id": "setup",
                "phase_index": 1,
                "total_phases": 2
            }),
            serde_json::json!({
                "event": "ultra_phase_start",
                "phase_id": "build",
                "phase_index": 2,
                "total_phases": 2
            }),
            serde_json::json!({
                "event": "ultra_phase_complete",
                "phase_id": "build",
                "phase_index": 2,
                "total_phases": 2,
                "stage": "complete"
            }),
            serde_json::json!({"event": "tui_command_stop", "status": "failed"}),
            serde_json::json!({"event": "run_stop", "status": "failed"}),
            serde_json::json!({
                "event": "ultra_phase_context_updated",
                "phase_id": "setup",
                "phase_index": 1,
                "total_phases": 2
            }),
        ];

        let phases = phase_statuses(&events);

        assert_eq!(phases.len(), 2);
        assert_eq!(phases[0].status, "interrupted");
        assert_eq!(phases[1].status, "completed");
        assert!(phases.iter().all(|phase| phase.status != "running"));
    }

    #[test]
    fn gui_smoke_events_project_one_failed_phase() {
        let events = parse_events(include_str!(
            "../../../workspace/management/runs/g1-gui-smoke/root-events.jsonl"
        ))
        .unwrap();

        let phases = phase_statuses(&events);

        assert_eq!(phases.len(), 1);
        assert_eq!(phases[0].id, "setup-project");
        assert_eq!(phases[0].index, 1);
        assert_eq!(phases[0].total, 5);
        assert_eq!(phases[0].stage, "recovery_prompt_saved");
        assert_eq!(phases[0].status, "failed");
    }
}
