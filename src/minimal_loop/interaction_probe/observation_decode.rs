//! Pure JSON-to-observation decision for the browser interaction probe.
//!
//! The probe script reports raw evidence JSON. This leaf turns it into a
//! `BrowserInteractionObservation` without touching processes, files, the
//! network, or the clock: everything the decision needs arrives through
//! `DecodeContext` and the JSON value. A missing, null, false, or non-boolean
//! `ok`, or an unobserved start or input transition, never decodes as success.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

use super::{
    BrowserInteractionObservation, CanvasSnapshotEvidence, HttpMutationObservation,
    InteractionProbeCandidateEvidence, PlaywrightResolution, SurfaceFitEvidence,
    interaction_failure_remediation, normalized_interaction_failure_kind,
};
use crate::eval_events;

/// Everything besides the evidence JSON that the decision reads.
pub(super) struct DecodeContext<'a> {
    pub(super) evidence_path: &'a Path,
    pub(super) script_path: &'a Path,
    pub(super) url: &'a str,
    /// Wall-clock fallback used when the evidence carries no `duration_ms`.
    pub(super) elapsed_ms: u128,
    pub(super) output_excerpt: String,
    pub(super) stdout_excerpt: String,
    pub(super) stderr_excerpt: String,
    pub(super) raw_stdout_excerpt: String,
    pub(super) child_spawned: bool,
    pub(super) child_reaped: bool,
    pub(super) playwright_resolution: Option<PlaywrightResolution>,
}

/// Which transitions and state changes the probe actually observed.
struct Flow {
    recovery_transition_observed: bool,
    recovery_transition_not_observed: bool,
    start_transition_observed: bool,
    input_state_evaluated_after_start: bool,
    input_state_changed: bool,
    input_contract_state_changed: bool,
    start_control_found: bool,
    text_entry: String,
    text_input_state_change: bool,
    startless_input_observed: bool,
}

/// Fields that feed failure classification.
struct Diagnostics {
    stage: String,
    error: String,
    server_http_status: Option<i64>,
    server_http_error: String,
}

struct Verdict {
    ok: bool,
    failure_kind: String,
    remediation: String,
}

struct Persistence {
    after_reload: String,
    reason: String,
    changed_dimensions: Vec<String>,
    before_marker: String,
    after_marker: String,
}

pub(super) fn decode_observation(
    ctx: DecodeContext<'_>,
    value: &Value,
) -> BrowserInteractionObservation {
    let steps = string_array_field(value, "steps");
    let flow = decode_flow(value, &steps);
    let diagnostics = decode_diagnostics(value);
    let verdict = decide(value, &flow, &diagnostics, &ctx.output_excerpt);
    let probe_mode = str_field(value, "probe_mode", "heuristic");
    let contract_hook_status = value
        .get("contract_hook_status")
        .and_then(Value::as_str)
        .or_else(|| {
            value
                .get("contract_hooks")
                .and_then(|hooks| hooks.get("status"))
                .and_then(Value::as_str)
        })
        .unwrap_or("unknown")
        .to_string();
    let canvas_snapshots = canvas_snapshots_from_value(value);
    let state_dimensions_changed = string_array_field(value, "state_dimensions_changed");
    let persistence = decode_persistence(value, &steps, &probe_mode, &state_dimensions_changed);
    let primary_transition_observed = value
        .get("primary_start_transition")
        .or_else(|| value.get("primary_transition_observed"))
        .and_then(Value::as_bool)
        .unwrap_or_else(|| {
            flow.start_transition_observed && !has_step(&steps, "primary_start_transition_missing")
        });
    BrowserInteractionObservation {
        ok: verdict.ok,
        status: if verdict.ok { "passed" } else { "failed" }.to_string(),
        url: ctx.url.to_string(),
        evidence_path: ctx.evidence_path.to_path_buf(),
        script_path: ctx.script_path.to_path_buf(),
        before_marker: str_field(value, "before_marker", ""),
        after_marker: str_field(value, "after_marker", ""),
        input_before_marker: str_field(value, "input_before_marker", ""),
        input_after_marker: str_field(value, "input_after_marker", ""),
        recovery_before_marker: str_field(value, "recovery_before_marker", ""),
        recovery_after_marker: str_field(value, "recovery_after_marker", ""),
        input_state_changed: flow.input_state_changed,
        input_contract_state_changed: flow.input_contract_state_changed,
        input_state_evaluated_after_start: flow.input_state_evaluated_after_start,
        probe_mode,
        contract_hook_status,
        candidate_table: interaction_candidate_table(value),
        input_dispatches: string_array_field(value, "input_dispatches"),
        canvas_blank_before_start: canvas_blank_snapshot_value(
            value,
            &canvas_snapshots,
            "before_start",
        ),
        canvas_blank_after_start: canvas_blank_snapshot_value(
            value,
            &canvas_snapshots,
            "after_start",
        ),
        canvas_blank_after_inputs: canvas_blank_snapshot_value(
            value,
            &canvas_snapshots,
            "after_inputs",
        ),
        canvas_snapshots,
        canvas_not_redrawn_after_start: value
            .get("canvas_not_redrawn_after_start")
            .and_then(Value::as_bool)
            .unwrap_or_else(|| has_step(&steps, "canvas_not_redrawn_after_start")),
        state_dimensions_changed,
        surface_fit: surface_fit_from_value(value),
        restart_hook_reachable_after_start: bool_field(value, "restart_hook_reachable_after_start")
            || has_step(&steps, "restart_hook_reachable_after_start"),
        restart_hook_count_after_start: value
            .get("restart_hook_count_after_start")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or(0),
        persistence_after_reload: persistence.after_reload,
        persistence_after_reload_reason: persistence.reason,
        persistence_changed_dimensions: persistence.changed_dimensions,
        persistence_before_reload_marker: persistence.before_marker,
        persistence_after_reload_marker: persistence.after_marker,
        action_hooks: string_array_field(value, "action_hooks")
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        text_entry: flow.text_entry,
        text_entry_target: str_field(value, "text_entry_target", ""),
        typed_token: str_field(value, "typed_token", ""),
        token_echoed: bool_field(value, "token_echoed"),
        echo_latency_ms: value.get("echo_latency_ms").and_then(Value::as_u64),
        token_echoed_after_reload: bool_field(value, "token_echoed_after_reload")
            || has_step(&steps, "token_echoed_after_reload"),
        token_echo_after_reload_latency_ms: value
            .get("token_echo_after_reload_latency_ms")
            .and_then(Value::as_u64),
        text_input_state_change: flow.text_input_state_change,
        primary_transition_observed,
        start_control_found: flow.start_control_found,
        informational_failure_kinds: string_array_field(value, "informational_failure_kinds"),
        http_mutation_observations: http_mutation_observations_from_value(value),
        recovery_transition_observed: flow.recovery_transition_observed,
        recovery_transition_not_observed: flow.recovery_transition_not_observed,
        failure_kind: verdict.failure_kind,
        stage: diagnostics.stage,
        error: diagnostics.error,
        remediation: verdict.remediation,
        duration_ms: value
            .get("duration_ms")
            .and_then(Value::as_u64)
            .map(u128::from)
            .unwrap_or(ctx.elapsed_ms),
        output_excerpt: eval_events::body_snippet(&ctx.output_excerpt),
        stdout_excerpt: eval_events::body_snippet(&ctx.stdout_excerpt),
        stderr_excerpt: eval_events::body_snippet(&ctx.stderr_excerpt),
        raw_stdout_excerpt: eval_events::body_snippet(
            value
                .get("raw_stdout_excerpt")
                .and_then(Value::as_str)
                .unwrap_or(&ctx.raw_stdout_excerpt),
        ),
        child_spawned: ctx.child_spawned,
        child_reaped: ctx.child_reaped,
        playwright_resolution: ctx.playwright_resolution,
        server_http_status: diagnostics.server_http_status,
        server_http_error: diagnostics.server_http_error,
        navigation_failure_kind: str_field(value, "navigation_failure_kind", ""),
        cold_start_ms: value.get("cold_start_ms").and_then(Value::as_u64),
        measured_navigation_ms: value.get("measured_navigation_ms").and_then(Value::as_u64),
        has_canvas: bool_value(
            value,
            &[
                "post_js_has_canvas",
                "has_canvas",
                "canvas_found",
                "canvas_available",
            ],
        ),
        interactive_control_count: usize_value(
            value,
            &[
                "post_js_interactive_control_count",
                "interactive_control_count",
                "interactive_controls",
            ],
        ),
        steps,
    }
}

fn has_step(steps: &[String], name: &str) -> bool {
    steps.iter().any(|step| step == name)
}

fn bool_field(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn str_field(value: &Value, key: &str, default: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_string()
}

fn non_empty_str_field(value: &Value, key: &str) -> bool {
    value
        .get(key)
        .and_then(Value::as_str)
        .is_some_and(|marker| !marker.is_empty())
}

fn decode_flow(value: &Value, steps: &[String]) -> Flow {
    let recovery_transition_observed =
        has_step(steps, "recovery_transition") || bool_field(value, "recovery_transition");
    let start_transition_observed = has_step(steps, "start_transition")
        || bool_field(value, "start_transition")
        || recovery_transition_observed;
    let input_state_evaluated_after_start = value
        .get("input_state_evaluated_after_start")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| {
            has_step(steps, "input_state_evaluated_after_start")
                || (start_transition_observed
                    && non_empty_str_field(value, "input_before_marker")
                    && non_empty_str_field(value, "input_after_marker"))
        });
    let explicit_state_changed = value
        .get("input_state_change")
        .or_else(|| value.get("state_changed"))
        .or_else(|| value.get("visible_state_changed"))
        .and_then(Value::as_bool);
    let text_entry = str_field(value, "text_entry", "not_evaluated");
    let text_input_state_change =
        bool_field(value, "text_input_state_change") || has_step(steps, "text_input_state_change");
    let input_state_changed = has_step(steps, "input_state_change")
        || explicit_state_changed == Some(true)
        || text_input_state_change;
    let input_contract_state_changed = value
        .get("input_contract_state_change")
        .and_then(Value::as_bool)
        .unwrap_or(input_state_changed);
    let recovery_transition_not_observed = has_step(steps, "recovery_transition:not_observed")
        || value.get("recovery_transition").and_then(Value::as_bool) == Some(false)
        || value
            .get("recovery_transition_status")
            .and_then(Value::as_str)
            == Some("not_observed");
    let start_control_found = value
        .get("start_control_found")
        .or_else(|| value.get("start_control_present"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let surface_visible = has_step(steps, "surface_visible")
        || value.get("surface_visible").and_then(Value::as_bool) == Some(true)
        || value.get("interactive_surface").and_then(Value::as_bool) == Some(true);
    let text_input_observed = text_entry == "entered" && text_input_state_change;
    let startless_input_observed =
        (!start_control_found && surface_visible && input_state_changed) || text_input_observed;
    Flow {
        recovery_transition_observed,
        recovery_transition_not_observed,
        start_transition_observed,
        input_state_evaluated_after_start,
        input_state_changed,
        input_contract_state_changed,
        start_control_found,
        text_entry,
        text_input_state_change,
        startless_input_observed,
    }
}

fn decode_diagnostics(value: &Value) -> Diagnostics {
    Diagnostics {
        stage: value
            .get("stage")
            .and_then(Value::as_str)
            .or_else(|| {
                value
                    .get("probe")
                    .and_then(|probe| probe.get("stage"))
                    .and_then(Value::as_str)
            })
            .unwrap_or("")
            .to_string(),
        error: str_field(value, "error", ""),
        server_http_status: value
            .get("server_http_status")
            .or_else(|| {
                value
                    .get("server_check")
                    .and_then(|server_check| server_check.get("status"))
            })
            .and_then(Value::as_i64),
        server_http_error: value
            .get("server_http_error")
            .or_else(|| {
                value
                    .get("server_check")
                    .and_then(|server_check| server_check.get("error"))
            })
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    }
}

/// Success needs a literal `ok: true` and every taxonomy check satisfied.
fn decide(value: &Value, flow: &Flow, diagnostics: &Diagnostics, output_excerpt: &str) -> Verdict {
    let raw_ok = bool_field(value, "ok");
    let raw_failure_kind = value
        .get("failure_kind")
        .or_else(|| value.get("browser_failure_kind"))
        .and_then(Value::as_str)
        .unwrap_or(if raw_ok {
            ""
        } else {
            "browser_interaction_failed"
        })
        .to_string();
    let taxonomy_failure_kind = interaction_taxonomy_failure_kind(
        flow.start_transition_observed || flow.startless_input_observed,
        flow.input_state_evaluated_after_start || flow.startless_input_observed,
        flow.input_state_changed,
    );
    let ok = raw_ok && taxonomy_failure_kind.is_empty();
    let effective_raw_failure_kind =
        effective_interaction_failure_kind(&raw_failure_kind, taxonomy_failure_kind);
    let failure_kind = normalized_interaction_failure_kind(
        ok,
        &diagnostics.stage,
        effective_raw_failure_kind,
        &diagnostics.error,
        output_excerpt,
        diagnostics.server_http_status,
        &diagnostics.server_http_error,
    );
    let remediation = interaction_failure_remediation(&failure_kind);
    Verdict {
        ok,
        failure_kind,
        remediation,
    }
}

fn decode_persistence(
    value: &Value,
    steps: &[String],
    probe_mode: &str,
    state_dimensions_changed: &[String],
) -> Persistence {
    let mut after_reload = str_field(value, "persistence_after_reload", "not_evaluated");
    let mut reason = persistence_after_reload_reason_from_value(value, steps, &after_reload);
    let mut changed_dimensions = string_array_field(value, "persistence_changed_dimensions");
    let before_marker = str_field(value, "persistence_before_reload_marker", "");
    let after_marker = str_field(value, "persistence_after_reload_marker", "");
    reconcile_observed_mutation_persistence(
        probe_mode,
        state_dimensions_changed,
        &before_marker,
        &after_marker,
        &mut after_reload,
        &mut reason,
        &mut changed_dimensions,
    );
    Persistence {
        after_reload,
        reason,
        changed_dimensions,
        before_marker,
        after_marker,
    }
}

fn bool_value(value: &Value, keys: &[&str]) -> Option<bool> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_bool))
}

fn usize_value(value: &Value, keys: &[&str]) -> Option<usize> {
    for key in keys {
        let Some(raw) = value.get(*key) else {
            continue;
        };
        if let Some(number) = raw.as_u64()
            && let Ok(value) = usize::try_from(number)
        {
            return Some(value);
        }
        if let Some(text) = raw.as_str()
            && let Ok(value) = text.parse::<usize>()
        {
            return Some(value);
        }
    }
    None
}

fn string_array_field(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

fn persistence_after_reload_reason_from_value(
    value: &Value,
    steps: &[String],
    persistence_after_reload: &str,
) -> String {
    if persistence_after_reload != "not_evaluated"
        && !steps
            .iter()
            .any(|step| step == "persistence_reload:not_evaluated")
    {
        return String::new();
    }
    if let Some(reason) = value
        .get("persistence_after_reload_reason")
        .and_then(Value::as_str)
        .and_then(normalized_persistence_not_evaluated_reason)
    {
        return reason.to_string();
    }
    let failure_kind = value
        .get("failure_kind")
        .and_then(Value::as_str)
        .unwrap_or("");
    if failure_kind.contains("reload") {
        return "reload_failed".to_string();
    }
    let text_entry = value
        .get("text_entry")
        .and_then(Value::as_str)
        .unwrap_or("");
    let text_entry_target = value
        .get("text_entry_target")
        .and_then(Value::as_str)
        .unwrap_or("");
    if text_entry != "entered" && text_entry_target.trim().is_empty() {
        return "no_text_entry_surface".to_string();
    }
    "no_mutation_observed".to_string()
}

fn reconcile_observed_mutation_persistence(
    probe_mode: &str,
    state_dimensions_changed: &[String],
    before_reload_marker: &str,
    after_reload_marker: &str,
    persistence_after_reload: &mut String,
    persistence_after_reload_reason: &mut String,
    persistence_changed_dimensions: &mut Vec<String>,
) {
    if probe_mode == "contract"
        || persistence_after_reload != "not_evaluated"
        || persistence_after_reload_reason != "no_mutation_observed"
        || state_dimensions_changed.is_empty()
        || before_reload_marker.trim().is_empty()
        || after_reload_marker.trim().is_empty()
    {
        return;
    }
    *persistence_changed_dimensions = state_dimensions_changed
        .iter()
        .filter(|dimension| !dimension.trim().is_empty())
        .cloned()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    if persistence_changed_dimensions.is_empty() {
        return;
    }
    *persistence_after_reload = if before_reload_marker == after_reload_marker {
        "preserved".to_string()
    } else {
        "reset".to_string()
    };
    persistence_after_reload_reason.clear();
}

fn normalized_persistence_not_evaluated_reason(reason: &str) -> Option<&'static str> {
    match reason {
        "no_mutation_observed" => Some("no_mutation_observed"),
        "no_text_entry_surface" => Some("no_text_entry_surface"),
        "reload_failed" => Some("reload_failed"),
        _ => None,
    }
}

fn interaction_candidate_table(value: &Value) -> Vec<InteractionProbeCandidateEvidence> {
    value
        .get("candidate_table")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|candidate| InteractionProbeCandidateEvidence {
            rank: candidate
                .get("rank")
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .unwrap_or(0),
            index: candidate
                .get("index")
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .unwrap_or(0),
            text_excerpt: candidate
                .get("text_excerpt")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            changed: candidate
                .get("changed")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        })
        .collect()
}

fn http_mutation_observations_from_value(value: &Value) -> Vec<HttpMutationObservation> {
    value
        .get("http_mutation_observations")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(20)
        .filter_map(|observation| {
            Some(HttpMutationObservation {
                method: observation.get("method")?.as_str()?.to_string(),
                status: observation.get("status")?.as_i64()?,
                ok: observation.get("ok")?.as_bool()?,
                url: observation.get("url")?.as_str()?.to_string(),
            })
        })
        .collect()
}

fn canvas_snapshots_from_value(value: &Value) -> Vec<CanvasSnapshotEvidence> {
    value
        .get("canvas_snapshots")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|snapshot| {
            let name = snapshot.get("name").and_then(Value::as_str)?.to_string();
            Some(CanvasSnapshotEvidence {
                name,
                canvas_count: snapshot
                    .get("canvas_count")
                    .and_then(Value::as_u64)
                    .and_then(|value| usize::try_from(value).ok())
                    .unwrap_or(0),
                canvas_blank: snapshot.get("canvas_blank").and_then(Value::as_bool),
                blank_canvas_count: snapshot
                    .get("blank_canvas_count")
                    .and_then(Value::as_u64)
                    .and_then(|value| usize::try_from(value).ok())
                    .unwrap_or(0),
                pixel_hashes: snapshot
                    .get("pixel_hashes")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect(),
            })
        })
        .collect()
}

fn canvas_blank_snapshot_value(
    value: &Value,
    snapshots: &[CanvasSnapshotEvidence],
    name: &str,
) -> Option<bool> {
    let field = match name {
        "before_start" => "canvas_blank_before_start",
        "after_start" => "canvas_blank_after_start",
        "after_inputs" => "canvas_blank_after_inputs",
        _ => "",
    };
    if !field.is_empty()
        && let Some(value) = value.get(field).and_then(Value::as_bool)
    {
        return Some(value);
    }
    snapshots
        .iter()
        .find(|snapshot| snapshot.name == name)
        .and_then(|snapshot| snapshot.canvas_blank)
}

pub(super) fn surface_fit_from_value(value: &Value) -> Option<SurfaceFitEvidence> {
    let fit = value.get("surface_fit").filter(|fit| fit.is_object())?;
    let number = |name: &str| finite_surface_fit_i64(fit, name).unwrap_or(0);
    Some(SurfaceFitEvidence {
        surface: fit
            .get("surface")
            .and_then(Value::as_str)
            .unwrap_or("surface")
            .to_string(),
        fits_viewport: fit
            .get("fits_viewport")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        overflow_top_px: number("overflow_top_px"),
        overflow_right_px: number("overflow_right_px"),
        overflow_bottom_px: number("overflow_bottom_px"),
        overflow_left_px: number("overflow_left_px"),
        viewport_width_px: number("viewport_width_px"),
        viewport_height_px: number("viewport_height_px"),
        rect_width_px: number("rect_width_px"),
        rect_height_px: number("rect_height_px"),
    })
}

fn finite_surface_fit_i64(fit: &Value, name: &str) -> Option<i64> {
    let value = fit.get(name)?;
    if let Some(number) = value.as_i64() {
        return Some(number);
    }
    let number = value.as_f64()?;
    if !number.is_finite() || number < i64::MIN as f64 || number >= 9_223_372_036_854_775_808.0 {
        return None;
    }
    Some(number.round() as i64)
}

fn interaction_taxonomy_failure_kind(
    transition_observed: bool,
    input_evaluated_after_start: bool,
    input_state_changed: bool,
) -> &'static str {
    if !transition_observed {
        "start_transition_missing"
    } else if !input_evaluated_after_start {
        "input_state_change_not_evaluated_after_start"
    } else if !input_state_changed {
        "input_state_change_missing_after_start"
    } else {
        ""
    }
}

fn effective_interaction_failure_kind<'a>(
    raw_failure_kind: &'a str,
    taxonomy_failure_kind: &'static str,
) -> &'a str {
    if taxonomy_failure_kind.is_empty() {
        return match raw_failure_kind {
            ""
            | "browser_interaction_failed"
            | "start_transition_missing"
            | "interaction_state_change_missing"
            | "input_state_change_missing" => "",
            _ => raw_failure_kind,
        };
    }
    match raw_failure_kind {
        ""
        | "browser_interaction_failed"
        | "start_transition_missing"
        | "interaction_state_change_missing"
        | "input_state_change_missing" => taxonomy_failure_kind,
        _ => raw_failure_kind,
    }
}
