//! The run budget and the structured stages in the run summary (Issue #1015).
//!
//! The end of a run records what its wallet and its declared stages stood at, each as one event
//! taken by its owner module (`structured_budget::summary`, `structured_stages::summary`). The
//! summary and `summary.md` read those events from the run's own stream and nothing else: no
//! ledger is read here, and no value is judged again. A run that declared neither has neither
//! event, so its summary has neither key and its `summary.md` no section.

use serde_json::Value;

use crate::config::Config;
use crate::planner::structured_stages::summary::{
    self as stages, FailedChecks, ObservedStages, StagesSummary,
};
use crate::structured_budget::summary::{self as budget, ObservedBudget, RunBudgetSummary};

/// Records the wallet and the stages of a run that declared them, once, at its end. The waits are
/// bounded, so an interrupted run whose send still holds a lock records `unavailable` and goes on.
pub(crate) fn emit_end_of_run(config: &Config) {
    let events = config.eval_events_path.as_deref();
    if let Some(handle) = config.field_sources.run_budget.as_ref() {
        let summary = budget::snapshot(handle, budget::END_OF_RUN_WAIT);
        crate::eval_events::emit(events, budget::event(&summary));
    }
    if let Some(summary) = stages::snapshot(config, budget::END_OF_RUN_WAIT) {
        crate::eval_events::emit(events, stages::event(&summary));
    }
}

fn latest<'a>(events: &'a [Value], name: &str) -> Option<&'a Value> {
    events
        .iter()
        .rev()
        .find(|event| event.get("event").and_then(Value::as_str) == Some(name))
}

/// The latest `run_budget_summary` of the run.
pub(super) fn run_budget(events: &[Value]) -> Option<RunBudgetSummary> {
    serde_json::from_value(latest(events, budget::EVENT)?.clone()).ok()
}

/// The latest `structured_stages_summary` of the run, each target with the checks of its latest
/// failed attempt in the same run.
pub(super) fn structured_stages(events: &[Value]) -> Option<StagesSummary> {
    let mut summary: StagesSummary =
        serde_json::from_value(latest(events, stages::EVENT)?.clone()).ok()?;
    if let Some(observed) = summary.observed.as_mut() {
        let run_nonce = observed.run_nonce.clone();
        for entry in &mut observed.stages {
            entry.failed_checks = run_nonce
                .as_deref()
                .and_then(|nonce| failed_checks(events, nonce, &entry.stage, &entry.target));
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
    serde_json::to_value(summary.accounting_basis)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_default()
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
    let Some(observed) = &summary.observed else {
        return format!("Run budget ({basis}): record unavailable, no amount recorded");
    };
    observed_budget_lines(&basis, observed).join("\n")
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
    if let Some(stop) = &observed.stop {
        lines.push(match &stop.limit {
            Some(limit) => format!("Run budget stop: limit {limit}"),
            None => format!("Run budget stop: refused ({})", stop.refusal),
        });
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
