use std::collections::BTreeMap;
use std::path::Path;

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

impl FailureDiagnostics {
    pub(super) fn redact_execution_root(&mut self, execution_root: &Path) {
        self.stop_reason = self
            .stop_reason
            .take()
            .map(|value| super::public_projection::text(value, execution_root));
        for reason in &mut self.release_gate_reasons {
            *reason = super::public_projection::text(&*reason, execution_root);
        }
        for finding in &mut self.probe_findings {
            for reason in &mut finding.reasons {
                *reason = super::public_projection::text(&*reason, execution_root);
            }
            finding.evidence_path = finding
                .evidence_path
                .take()
                .map(|value| super::public_projection::text(value, execution_root));
        }
    }
}

pub(super) fn project(events: &[Value]) -> FailureDiagnostics {
    let mut diagnostics = FailureDiagnostics::default();
    let mut probes = BTreeMap::<String, ProbeFinding>::new();
    for event in events {
        let event_name = string(event, "event").unwrap_or("unknown");
        project_terminal_reason(&mut diagnostics, event, event_name);
        project_release_gate(&mut diagnostics, event);

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
            project_named_probe(&mut probes, prefix, value.as_str(), event, prefix);
        }
        if event_name.contains("probe") {
            project_named_probe(&mut probes, event_name, string(event, "status"), event, "");
        }
    }
    diagnostics.probe_findings = probes
        .into_values()
        .filter(|finding| {
            finding
                .status
                .as_deref()
                .is_some_and(|status| !is_not_applicable(status))
                || !finding.reasons.is_empty()
                || finding.evidence_path.is_some()
        })
        .take(MAX_PROBES)
        .collect();
    diagnostics
}

fn project_terminal_reason(diagnostics: &mut FailureDiagnostics, event: &Value, event_name: &str) {
    if !matches!(event_name, "tui_command_stop" | "run_stop") {
        return;
    }
    let reason = ["stop_reason", "primary_reason", "failure_kind"]
        .iter()
        .find_map(|key| string(event, key))
        .and_then(capped);
    let actionable = reason.filter(|reason| !is_terminal_success(reason));
    if event_name == "tui_command_stop" || actionable.is_some() {
        diagnostics.stop_reason = actionable;
    }
}

fn project_release_gate(diagnostics: &mut FailureDiagnostics, event: &Value) {
    let status = string(event, "release_gate_status").and_then(capped);
    if status.as_deref().is_some_and(is_not_applicable) {
        return;
    }
    if status.is_some() || event.get("release_gate_reasons").is_some() {
        diagnostics.release_gate_reasons.clear();
        if !status.as_deref().is_some_and(is_success) {
            append_values(
                &mut diagnostics.release_gate_reasons,
                event.get("release_gate_reasons"),
                MAX_RELEASE_REASONS,
            );
        }
    }
}

fn project_named_probe(
    probes: &mut BTreeMap<String, ProbeFinding>,
    name: &str,
    raw_status: Option<&str>,
    event: &Value,
    field_prefix: &str,
) {
    let status = raw_status.and_then(capped);
    let finding = probes
        .entry(name.to_string())
        .or_insert_with(|| ProbeFinding {
            name: name.to_string(),
            status: None,
            reasons: Vec::new(),
            evidence_path: None,
        });
    if status.as_deref().is_some_and(is_not_applicable)
        && finding
            .status
            .as_deref()
            .is_some_and(|current| !is_not_applicable(current))
    {
        return;
    }
    if status.is_some() {
        finding.status = status;
        finding.reasons.clear();
        finding.evidence_path = None;
    }
    let reasons_key = prefixed(field_prefix, "reasons");
    append_values(
        &mut finding.reasons,
        event.get(&reasons_key),
        MAX_PROBE_REASONS,
    );
    let reason_key = prefixed(field_prefix, "reason");
    if let Some(reason) = event
        .get(&reason_key)
        .and_then(Value::as_str)
        .and_then(capped)
    {
        push_unique(&mut finding.reasons, reason, MAX_PROBE_REASONS);
    }
    let evidence_path_key = prefixed(field_prefix, "evidence_path");
    let path_key = prefixed(field_prefix, "path");
    finding.evidence_path = event
        .get(&evidence_path_key)
        .and_then(Value::as_str)
        .and_then(capped)
        .or_else(|| {
            event
                .get(&path_key)
                .and_then(Value::as_str)
                .and_then(capped)
        })
        .or_else(|| finding.evidence_path.clone());
}

fn prefixed(prefix: &str, suffix: &str) -> String {
    if prefix.is_empty() {
        suffix.to_string()
    } else {
        format!("{prefix}_{suffix}")
    }
}

fn is_probe_prefix(prefix: &str) -> bool {
    prefix.contains("probe") || matches!(prefix, "browser_readiness" | "interaction_evidence")
}

fn is_success(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "ok" | "pass" | "passed" | "completed" | "full" | "full_success"
    )
}

fn is_not_applicable(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "not_applicable" | "not_required"
    )
}

fn is_terminal_success(reason: &str) -> bool {
    matches!(reason.trim(), "completed" | "none" | "not_applicable")
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
    fn projects_current_terminal_diagnostics_table() {
        struct Case {
            name: &'static str,
            start: usize,
            end: usize,
            stop_reason: Option<&'static str>,
            release_reasons: &'static [&'static str],
            probe_statuses: &'static [(&'static str, &'static str, usize)],
        }
        let fixture = include_str!(
            "../../../tests/corpus/apps/issue364-gui-terminal-outcomes/fixtures/events.jsonl"
        );
        let events = fixture
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect::<Vec<_>>();
        let cases = [
            Case {
                name: "gate 3 keeps passed probes neutral",
                start: 0,
                end: 3,
                stop_reason: None,
                release_reasons: &[],
                probe_statuses: &[
                    ("browser_readiness", "passed", 0),
                    ("interaction_evidence", "passed", 0),
                ],
            },
            Case {
                name: "gate 4 keeps actionable evidence",
                start: 3,
                end: 6,
                stop_reason: Some("release_gate_failed"),
                release_reasons: &["browser_route_unavailable"],
                probe_statuses: &[("browser_readiness", "failed", 1)],
            },
            Case {
                name: "bounded repair replaces stale failure reasons",
                start: 6,
                end: 11,
                stop_reason: None,
                release_reasons: &[],
                probe_statuses: &[("profile_behavior_probe", "passed", 0)],
            },
            Case {
                name: "directive round excludes prior failure",
                start: 13,
                end: 16,
                stop_reason: None,
                release_reasons: &[],
                probe_statuses: &[("profile_behavior_probe", "passed", 0)],
            },
        ];

        for case in cases {
            let diagnostics = project(&events[case.start..case.end]);
            assert_eq!(
                diagnostics.stop_reason.as_deref(),
                case.stop_reason,
                "{}",
                case.name
            );
            assert_eq!(
                diagnostics.release_gate_reasons, case.release_reasons,
                "{}",
                case.name
            );
            let actual = diagnostics
                .probe_findings
                .iter()
                .map(|finding| {
                    (
                        finding.name.as_str(),
                        finding.status.as_deref().unwrap_or(""),
                        finding.reasons.len(),
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(actual, case.probe_statuses, "{}", case.name);
        }
    }

    #[test]
    fn redacts_execution_root_from_all_diagnostic_text() {
        let root = Path::new("/private/tmp/trial-root");
        let mut diagnostics = project(&[serde_json::json!({
            "event": "tui_command_stop",
            "status": "failed",
            "stop_reason": "failed in /private/tmp/trial-root/sessions/one",
            "release_gate_status": "failed",
            "release_gate_reasons": ["inspect /private/tmp/trial-root/summary.md"],
            "browser_readiness_status": "failed",
            "browser_readiness_reasons": ["/private/tmp/trial-root returned 500"],
            "browser_readiness_evidence_path": "/private/tmp/trial-root/evidence.json"
        })]);

        diagnostics.redact_execution_root(root);
        let serialized = serde_json::to_string(&diagnostics).unwrap();

        assert!(!serialized.contains(root.to_string_lossy().as_ref()));
        assert!(serialized.contains("<execution-root>"));
    }
}
