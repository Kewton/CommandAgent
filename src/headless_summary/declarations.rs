//! The run budget and the structured stages in the run summary (Issue #1015).
//!
//! The end of a run records what its wallet and its declared stages stood at, each as one event
//! taken by its owner module (`structured_budget::summary`, `structured_stages::summary`). The
//! summary and `summary.md` read those events from the run's own stream and nothing else: no
//! ledger is read here, and no value is judged again. A run that declared neither has neither
//! event, so its summary has neither key and its `summary.md` no section.
//!
//! Only the current run counts. An events file can hold several runs when
//! `COMMANDAGENT_EVAL_EVENTS` is reused, and every run begins with one `run_start`, so the three
//! lookups read only the events from the last `run_start` on (the whole stream when it has none,
//! as a stream written without the run lifecycle does). A run that recorded no snapshot of its
//! own gets no key and no section, whatever an earlier run in the same file recorded. This needs
//! no new field: `run_start` is how every reader already tells a run's beginning.
//!
//! These are recorded only by the end of a direct command (`--prompt`, `--plan-run`, and the
//! others); a REPL session records neither.

use std::time::Instant;

use serde_json::Value;

use crate::config::Config;
use crate::planner::structured_stages::summary::{
    self as stages, FailedChecks, ObservedStages, StagesSummary,
};
use crate::structured_budget::summary::{self as budget, ObservedBudget, RunBudgetSummary};

/// Records the wallet and the stages of a run that declared them, once, at its end. One deadline,
/// [`budget::END_OF_RUN_WAIT`] from now, bounds every lock both snapshots wait for, so an
/// interrupted run whose send still holds a lock records `unavailable` and goes on.
pub(crate) fn emit_end_of_run(config: &Config) {
    let deadline = Instant::now() + budget::END_OF_RUN_WAIT;
    let events = config.eval_events_path.as_deref();
    if let Some(handle) = config.field_sources.run_budget.as_ref() {
        let summary = budget::snapshot(handle, deadline);
        crate::eval_events::emit(events, budget::event(&summary));
    }
    if let Some(summary) = stages::snapshot(config, deadline) {
        crate::eval_events::emit(events, stages::event(&summary));
    }
}

/// The events of the current run: from the last `run_start` on.
pub(super) fn current_run(events: &[Value]) -> &[Value] {
    let start = events
        .iter()
        .rposition(|event| event.get("event").and_then(Value::as_str) == Some("run_start"))
        .unwrap_or(0);
    &events[start..]
}

fn latest<'a>(events: &'a [Value], name: &str) -> Option<&'a Value> {
    events
        .iter()
        .rev()
        .find(|event| event.get("event").and_then(Value::as_str) == Some(name))
}

/// The latest `run_budget_summary` of the current run.
pub(super) fn run_budget(events: &[Value]) -> Option<RunBudgetSummary> {
    latest(current_run(events), budget::EVENT).map(RunBudgetSummary::read_back)
}

/// The latest `structured_stages_summary` of the current run, each target with the checks of its
/// latest failed attempt in the same run.
pub(super) fn structured_stages(events: &[Value]) -> Option<StagesSummary> {
    let events = current_run(events);
    let mut summary = StagesSummary::read_back(latest(events, stages::EVENT)?);
    if let Some(observed) = summary.observed.as_mut() {
        let run_nonce = observed.run_nonce.clone();
        for entry in &mut observed.stages {
            entry.failed_checks = failed_checks(events, &run_nonce, &entry.stage, &entry.target);
        }
    }
    Some(summary)
}

fn failed_checks(
    events: &[Value],
    run_nonce: &str,
    stage: &str,
    target: &str,
) -> Option<FailedChecks> {
    let text =
        |event: &Value, key: &str| event.get(key).and_then(Value::as_str).map(str::to_string);
    let event = events.iter().rev().find(|event| {
        text(event, "event").as_deref() == Some(stages::CHECKS_EVENT)
            && text(event, "run_nonce").as_deref() == Some(run_nonce)
            && text(event, "stage").as_deref() == Some(stage)
            && text(event, "target").as_deref() == Some(target)
    })?;
    serde_json::from_value(event.clone()).ok()
}

/// The `summary.md` sections, only for what the run declared.
pub(crate) fn markdown_sections(events: &[Value]) -> Option<String> {
    let sections = [
        run_budget(events).map(|summary| budget_section(&summary)),
        structured_stages(events).map(|summary| stages_section(&summary)),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    (!sections.is_empty()).then(|| sections.join("\n\n"))
}

fn basis_word(summary: &RunBudgetSummary) -> String {
    summary
        .accounting_basis
        .and_then(|basis| serde_json::to_value(basis).ok())
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown basis".to_string())
}

fn stop_line(summary: &RunBudgetSummary) -> Option<String> {
    let stop = summary.stop.as_ref()?;
    Some(match &stop.limit {
        Some(limit) => format!("Run budget stop: limit {limit}"),
        None => format!("Run budget stop: refused ({})", stop.refusal),
    })
}

fn cause_text(cause: &Value) -> String {
    match cause {
        Value::String(word) => word.clone(),
        Value::Object(object) => object
            .values()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(":"),
        other => other.to_string(),
    }
}

fn budget_section(summary: &RunBudgetSummary) -> String {
    let basis = basis_word(summary);
    let mut lines = match &summary.observed {
        Some(observed) => observed_budget_lines(&basis, observed),
        None => vec![format!(
            "Run budget ({basis}): record unavailable, no amount recorded"
        )],
    };
    lines.extend(stop_line(summary));
    lines.join("\n")
}

fn observed_budget_lines(basis: &str, observed: &ObservedBudget) -> Vec<String> {
    let mut lines = vec![format!(
        "Run budget ({basis}, {}): spent ${} of ${}, held ${}",
        observed.budget_id, observed.spent_usd, observed.limit_usd, observed.held_usd
    )];
    if let Some(total) = &observed.declared_tariff_charge_usd_total {
        lines.push(format!(
            "Declared tariff charge: ${total} (your declared tariff on verified usage, not a provider bill)"
        ));
    }
    let sends = observed.sends;
    lines.push(format!(
        "Run budget sends: settled {}, reserved {}, started {}, unknown {}",
        sends.settled, sends.reserved, sends.started, sends.unknown
    ));
    if !observed.unknown.is_empty() {
        let unknown = observed
            .unknown
            .iter()
            .map(|send| format!("#{} {}", send.physical, cause_text(&send.cause)))
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("Run budget unknown sends: {unknown}"));
    }
    if let Some(latch) = &observed.latch {
        let cause = serde_json::to_value(latch.cause).unwrap_or(Value::Null);
        lines.push(format!("Run budget latch: {}", cause_text(&cause)));
    }
    lines
}

fn stages_section(summary: &StagesSummary) -> String {
    let Some(observed) = &summary.observed else {
        return "Structured stages: record unavailable".to_string();
    };
    observed_stages_lines(observed).join("\n")
}

fn observed_stages_lines(observed: &ObservedStages) -> Vec<String> {
    let sealed = observed.stages.iter().filter(|entry| entry.sealed).count();
    let mut head = format!(
        "Structured stages: {} ({sealed}/{} sealed)",
        if observed.complete {
            "complete"
        } else {
            "incomplete"
        },
        observed.stages.len()
    );
    if !observed.ledger_intact {
        head.push_str(", ledger not intact");
    }
    let mut lines = vec![
        head,
        "| Stage | Target | Phase | Attempts | Failed checks |".to_string(),
        "| --- | --- | --- | ---: | --- |".to_string(),
    ];
    for entry in &observed.stages {
        let phase = serde_json::to_value(entry.phase)
            .ok()
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_default();
        lines.push(format!(
            "| {} | {} | {phase} | {}/{} | {} |",
            entry.stage,
            entry.target,
            entry.attempts.used,
            entry.attempts.limit,
            entry
                .failed_checks
                .as_ref()
                .map_or_else(|| "-".to_string(), checks_cell)
        ));
    }
    lines
}

fn checks_cell(checks: &FailedChecks) -> String {
    let mut parts = checks
        .checks
        .iter()
        .map(|check| {
            let status = serde_json::to_value(check.status)
                .ok()
                .and_then(|value| value.as_str().map(str::to_string))
                .unwrap_or_default();
            format!("{}={status}", check.id)
        })
        .collect::<Vec<_>>();
    if !checks.missing.is_empty() {
        parts.push(format!("missing {}", checks.missing.join(" ")));
    }
    if checks.unrequested > 0 {
        parts.push(format!("{} unrequested", checks.unrequested));
    }
    format!("attempt {}: {}", checks.attempt, parts.join("; "))
}

#[cfg(test)]
#[path = "declarations_tests.rs"]
mod tests;
