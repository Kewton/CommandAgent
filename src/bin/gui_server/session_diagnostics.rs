use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

const MAX_RELEASE_REASONS: usize = 16;
const MAX_PROBES: usize = 16;
const MAX_PROBE_REASONS: usize = 8;
const MAX_VALUE_CHARS: usize = 512;

#[derive(Debug, Clone, Default, Serialize)]
pub(super) struct FailureDiagnostics {
    pub(super) stop_reason: Option<String>,
    pub(super) release_gate_reasons: Vec<String>,
    pub(super) probe_findings: Vec<ProbeFinding>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct ProbeFinding {
    name: String,
    status: Option<String>,
    reasons: Vec<String>,
    evidence_path: Option<String>,
}

pub(super) fn project(events: &[Value]) -> FailureDiagnostics {
    let mut diagnostics = FailureDiagnostics::default();
    let mut probes = BTreeMap::<String, ProbeFinding>::new();
    for event in events {
        let event_name = string(event, "event").unwrap_or("unknown");
        if matches!(event_name, "tui_command_stop" | "run_stop")
            && let Some(reason) = ["stop_reason", "primary_reason", "failure_kind"]
                .iter()
                .find_map(|key| string(event, key))
                .and_then(capped)
                .filter(|reason| reason != "completed" || diagnostics.stop_reason.is_none())
        {
            diagnostics.stop_reason = Some(reason);
        }
        append_values(
            &mut diagnostics.release_gate_reasons,
            event.get("release_gate_reasons"),
            MAX_RELEASE_REASONS,
        );

        let Some(object) = event.as_object() else {
            continue;
        };
        for (key, value) in object {
            let Some(prefix) = key.strip_suffix("_status") else {
                continue;
            };
            if !is_probe_prefix(prefix) {
                continue;
            }
            let finding = probes
                .entry(prefix.to_string())
                .or_insert_with(|| ProbeFinding {
                    name: prefix.to_string(),
                    status: None,
                    reasons: Vec::new(),
                    evidence_path: None,
                });
            finding.status = value.as_str().and_then(capped);
            append_values(
                &mut finding.reasons,
                event.get(format!("{prefix}_reasons")),
                MAX_PROBE_REASONS,
            );
            if let Some(reason) = event
                .get(format!("{prefix}_reason"))
                .and_then(Value::as_str)
                .and_then(capped)
            {
                push_unique(&mut finding.reasons, reason, MAX_PROBE_REASONS);
            }
            finding.evidence_path = event
                .get(format!("{prefix}_evidence_path"))
                .and_then(Value::as_str)
                .and_then(capped)
                .or_else(|| {
                    event
                        .get(format!("{prefix}_path"))
                        .and_then(Value::as_str)
                        .and_then(capped)
                })
                .or_else(|| finding.evidence_path.clone());
        }
        if event_name.contains("probe") {
            let finding = probes
                .entry(event_name.to_string())
                .or_insert_with(|| ProbeFinding {
                    name: event_name.to_string(),
                    status: None,
                    reasons: Vec::new(),
                    evidence_path: None,
                });
            finding.status = string(event, "status")
                .and_then(capped)
                .or_else(|| finding.status.clone());
            append_values(
                &mut finding.reasons,
                event.get("reasons"),
                MAX_PROBE_REASONS,
            );
            if let Some(reason) = string(event, "reason").and_then(capped) {
                push_unique(&mut finding.reasons, reason, MAX_PROBE_REASONS);
            }
            finding.evidence_path = string(event, "evidence_path")
                .and_then(capped)
                .or_else(|| finding.evidence_path.clone());
        }
    }
    diagnostics.probe_findings = probes
        .into_values()
        .filter(|finding| {
            finding.status.is_some()
                || !finding.reasons.is_empty()
                || finding.evidence_path.is_some()
        })
        .take(MAX_PROBES)
        .collect();
    diagnostics
}

fn is_probe_prefix(prefix: &str) -> bool {
    prefix.contains("probe") || matches!(prefix, "browser_readiness" | "interaction_evidence")
}

fn append_values(target: &mut Vec<String>, value: Option<&Value>, limit: usize) {
    match value {
        Some(Value::Array(values)) => {
            for value in values {
                if let Some(value) = value.as_str().and_then(capped) {
                    push_unique(target, value, limit);
                }
            }
        }
        Some(Value::String(value)) => {
            if let Some(value) = capped(value) {
                push_unique(target, value, limit);
            }
        }
        _ => {}
    }
}

fn push_unique(target: &mut Vec<String>, value: String, limit: usize) {
    if target.len() < limit && !target.contains(&value) {
        target.push(value);
    }
}

fn capped(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    Some(value.chars().take(MAX_VALUE_CHARS).collect())
}

fn string<'a>(event: &'a Value, key: &str) -> Option<&'a str> {
    event.get(key).and_then(Value::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projects_terminal_release_gate_and_probe_findings() {
        let events = vec![
            serde_json::json!({
                "event": "ultra_final_acceptance",
                "release_gate_reasons": ["browser_not_ready", "interaction_unverified"],
                "browser_readiness_status": "failed",
                "browser_readiness_reasons": ["route returned 500"],
                "browser_readiness_evidence_path": ".commandagent/evidence/browser-readiness.json",
                "profile_behavior_probe_status": "failed",
                "profile_behavior_probe_reasons": ["state did not change"]
            }),
            serde_json::json!({
                "event": "tui_command_stop",
                "status": "failed",
                "stop_reason": "release_gate_failed"
            }),
            serde_json::json!({
                "event": "run_stop",
                "status": "completed",
                "stop_reason": "completed"
            }),
        ];

        let diagnostics = project(&events);

        assert_eq!(
            diagnostics.stop_reason.as_deref(),
            Some("release_gate_failed")
        );
        assert_eq!(
            diagnostics.release_gate_reasons,
            ["browser_not_ready", "interaction_unverified"]
        );
        assert_eq!(diagnostics.probe_findings.len(), 2);
    }
}
