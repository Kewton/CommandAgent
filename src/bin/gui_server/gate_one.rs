use std::path::{Path, PathBuf};

use axum::Json;
use axum::extract::State;
use axum::http::HeaderMap;
use commandagent::planner::pack::catalog::PackLocator;
use commandagent::planner::profile_descriptor::descriptor_for_name;
use commandagent::tui::boundary_shell::BoundaryShell;
use commandagent::tui::boundary_shell::ambiguity::{
    ClassifierProvenance, ProposalStatus, RouteProposal,
};
use commandagent::tui::boundary_shell::confirmation::{
    ConfirmationIdentity, ExecutionPins, PackSelection,
};
use commandagent::tui::boundary_shell::pack_catalog;
use commandagent::tui::boundary_shell::presentation::render_gate_one_for_gui;
use commandagent::tui::boundary_shell::route::{
    DeterministicResolution, ExplicitRouteBinding, RouteRequest,
    deterministic_route_excluding_top_level,
};
use serde::{Deserialize, Serialize};

use super::session_paths::{SESSION_WORKSPACES_DIRECTORY, proposal_confirmation_root};
use super::sessions::{SessionError, internal, require_trial, unprocessable};
use super::{AppState, trial_options};

const MAX_GOAL_BYTES: usize = 16 * 1024;
const MAX_FIELD_BYTES: usize = 256;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SessionSpec {
    goal: String,
    profile: String,
    provider: String,
    model: String,
    planner_provider: String,
    planner_model: String,
    pack: Option<String>,
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

pub(super) fn gate_one(
    state: &AppState,
    spec: &SessionSpec,
    workspace: &Path,
    confirmation_root: PathBuf,
) -> Result<(BoundaryShell, ConfirmationIdentity, String), SessionError> {
    validate_spec(spec)?;
    let descriptor = descriptor_for_name(&spec.profile)
        .ok_or_else(|| unprocessable(format!("profile `{}` is not registered", spec.profile)))?;
    let profile = descriptor.id.clone();
    let deterministic = deterministic_route_excluding_top_level(
        RouteRequest {
            request: &spec.goal,
            workspace,
            explicit: ExplicitRouteBinding {
                profile: Some(profile),
                ..ExplicitRouteBinding::default()
            },
        },
        &[SESSION_WORKSPACES_DIRECTORY],
    );
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
    let selected = deterministic
        .candidates
        .first()
        .expect("unique deterministic routes have one selected candidate");
    let locator =
        PackLocator::with_extension_root(&state.repository_root, state.extension_root.clone());
    let pack = match spec.pack.as_deref() {
        Some(selector) => pack_catalog::select_with_locator(
            selected.profile.as_str(),
            selected.intent.as_str(),
            selector,
            &locator,
        )
        .map_err(unprocessable)?,
        None => PackSelection::None,
    };
    let proposal = RouteProposal {
        selected: Some(selected.clone()),
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
        .begin_gate_one_with_locator(proposal, spec.goal.clone(), workspace, pins, pack, &locator)
        .map_err(unprocessable)?
        .clone();
    let card = render_gate_one_for_gui(&identity, &locator).map_err(internal)?;
    Ok((shell, identity, card))
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
    if spec
        .pack
        .as_deref()
        .is_some_and(|pack| pack.trim().is_empty() || pack.len() > MAX_FIELD_BYTES)
    {
        return Err(unprocessable(format!(
            "pack must contain 1..={MAX_FIELD_BYTES} UTF-8 bytes when selected"
        )));
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

async fn band_price(
    repository_root: &Path,
    identity: &ConfirmationIdentity,
) -> Result<BandPrice, SessionError> {
    if identity.draft_manifest.is_some() {
        return Ok(BandPrice {
            duration_n: 0,
            average_duration_seconds: None,
            cost_n: 0,
            average_cost_usd: None,
            source: "未計測".to_string(),
        });
    }
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

pub(super) async fn average_duration_seconds(
    repository_root: &Path,
    identity: &ConfirmationIdentity,
) -> Result<Option<f64>, SessionError> {
    Ok(band_price(repository_root, identity)
        .await?
        .average_duration_seconds)
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
            pack: None,
        };

        validate_spec(&spec).unwrap();
    }
}
