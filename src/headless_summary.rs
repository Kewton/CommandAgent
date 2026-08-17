use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use crate::config::Config;

const SCHEMA_VERSION: &str = "commandagent.headless-summary/v1";

#[derive(Debug, Clone)]
pub(crate) struct Source {
    events_path: Option<PathBuf>,
}

impl Source {
    pub(crate) fn from_config(config: &Config) -> Self {
        Self {
            events_path: config.eval_events_path.clone(),
        }
    }

    #[cfg(test)]
    fn from_events_path(path: impl Into<PathBuf>) -> Self {
        Self {
            events_path: Some(path.into()),
        }
    }
}

#[derive(Debug, Serialize)]
struct HeadlessSummary {
    schema_version: &'static str,
    run_id: Option<String>,
    verdict: Option<String>,
    assurance: Option<String>,
    score: Option<f64>,
    acceptance_sheet_path: Option<String>,
    artifacts_dir: Option<String>,
    events_path: Option<String>,
    duration_secs: Option<f64>,
    provider_cost_usd: Option<f64>,
    stop_class: Option<String>,
    directive_round: u64,
}

pub(crate) fn render(source: &Source) -> String {
    serde_json::to_string(&project(source)).expect("headless summary serialization is infallible")
}

fn project(source: &Source) -> HeadlessSummary {
    let events_path = source.events_path.as_deref();
    let events = events_path.map(read_events).unwrap_or_default();
    let terminal = latest_terminal(&events);
    let failed = terminal
        .and_then(|event| event.get("ok"))
        .and_then(Value::as_bool)
        == Some(false);
    let assurance = terminal
        .and_then(|event| text(event, "assurance_level"))
        .or_else(|| latest_event_text(&events, "community_profile_verification", "assurance"));
    let verdict = latest_event_text(&events, "ultra_final_acceptance", "verdict")
        .or_else(|| latest_event_text(&events, "workflow_adjudicated", "verdict"))
        .or_else(|| latest_event_text(&events, "community_profile_verification", "verdict"))
        .or_else(|| assurance.clone());
    let duration_secs = terminal
        .and_then(|event| number(event, "time_profile_total_ms"))
        .or_else(|| latest_nested_number(&events, "time_profile", "profile", "total_ms"))
        .map(|milliseconds| milliseconds / 1_000.0);
    let summary_path = events_path
        .and_then(Path::parent)
        .map(|parent| parent.join("summary.md"))
        .filter(|path| path.is_file());

    HeadlessSummary {
        schema_version: SCHEMA_VERSION,
        run_id: events_path
            .and_then(Path::parent)
            .and_then(Path::file_name)
            .map(|value| value.to_string_lossy().into_owned()),
        verdict,
        assurance,
        score: latest_number(&events, &["score"])
            .or_else(|| latest_nested_number(&events, "score_checkpoint", "vector", "score")),
        acceptance_sheet_path: summary_path.map(display_path),
        artifacts_dir: latest_event_text(&events, "run_start", "workspace_root"),
        events_path: events_path.map(display_path),
        duration_secs,
        provider_cost_usd: latest_number(&events, &["provider_cost_usd", "cost_usd"]),
        stop_class: failed
            .then(|| {
                latest_event_text(&events, "planner_quality_retry_exhausted", "stop_class")
                    .or_else(|| {
                        latest_event_text(&events, "community_profile_verification", "violation")
                            .filter(|value| !value.is_empty())
                            .map(|_| "community_profile_violation".to_string())
                    })
                    .or_else(|| terminal.and_then(|event| text(event, "failure_kind")))
            })
            .flatten(),
        directive_round: latest_integer(&events, "directive_round").unwrap_or(0),
    }
}

fn read_events(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

fn latest_terminal(events: &[Value]) -> Option<&Value> {
    events
        .iter()
        .rev()
        .find(|event| event.get("event").and_then(Value::as_str) == Some("tui_command_stop"))
        .or_else(|| {
            events
                .iter()
                .rev()
                .find(|event| event.get("event").and_then(Value::as_str) == Some("run_stop"))
        })
}

fn latest_event_text(events: &[Value], event_name: &str, key: &str) -> Option<String> {
    events.iter().rev().find_map(|event| {
        (event.get("event").and_then(Value::as_str) == Some(event_name))
            .then(|| text(event, key))
            .flatten()
    })
}

fn latest_number(events: &[Value], keys: &[&str]) -> Option<f64> {
    events
        .iter()
        .rev()
        .find_map(|event| keys.iter().find_map(|key| number(event, key)))
}

fn latest_nested_number(
    events: &[Value],
    event_name: &str,
    object_key: &str,
    value_key: &str,
) -> Option<f64> {
    events.iter().rev().find_map(|event| {
        (event.get("event").and_then(Value::as_str) == Some(event_name))
            .then(|| {
                event
                    .get(object_key)
                    .and_then(|object| number(object, value_key))
            })
            .flatten()
    })
}

fn latest_integer(events: &[Value], key: &str) -> Option<u64> {
    events
        .iter()
        .rev()
        .find_map(|event| event.get(key).and_then(Value::as_u64))
}

fn text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn number(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(Value::as_f64)
}

fn display_path(path: impl AsRef<Path>) -> String {
    path.as_ref().display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(case: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/corpus/apps/cm0-headless-summary/fixtures")
            .join(case)
            .join("events.jsonl")
    }

    fn summary(case: &str) -> Value {
        serde_json::from_str(&render(&Source::from_events_path(fixture(case)))).unwrap()
    }

    #[test]
    fn full_fixture_projects_existing_terminal_values() {
        let value = summary("full");
        assert_eq!(value["verdict"], "full");
        assert_eq!(value["assurance"], "full");
        assert_eq!(value["score"], 92.5);
        assert_eq!(value["duration_secs"], 12.5);
        assert_eq!(value["provider_cost_usd"], 0.0123);
        assert!(value["stop_class"].is_null());
        assert_eq!(value["directive_round"], 0);
    }

    #[test]
    fn failed_fixture_keeps_failure_class_without_upgrading_verdict() {
        let value = summary("failed");
        assert_eq!(value["verdict"], "failed");
        assert_eq!(value["assurance"], "failed");
        assert_eq!(value["stop_class"], "direct_cli_command_failed");
        assert_eq!(value["directive_round"], 1);
    }

    #[test]
    fn static_fixture_keeps_static_assurance_and_absent_measurements() {
        let value = summary("static");
        assert_eq!(value["verdict"], "static");
        assert_eq!(value["assurance"], "static");
        assert!(value["score"].is_null());
        assert!(value["provider_cost_usd"].is_null());
        assert!(value["stop_class"].is_null());
    }
}
