use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TerminalStatus {
    Completed,
    Failed,
    Interrupted,
}

impl TerminalStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Interrupted => "interrupted",
        }
    }

    fn default_exit_code(self) -> i32 {
        match self {
            Self::Completed => 0,
            Self::Failed => 1,
            Self::Interrupted => 130,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VerificationStatus {
    Passed,
    Failed,
    TimedOut,
    NotRun,
    NotRecorded,
}

impl VerificationStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::TimedOut => "timed_out",
            Self::NotRun => "not_run",
            Self::NotRecorded => "not_recorded",
        }
    }

    fn from_value(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "pass" | "passed" | "success" | "succeeded" | "ok" => Self::Passed,
            "fail" | "failed" | "error" => Self::Failed,
            "timeout" | "timed_out" => Self::TimedOut,
            "not_run" | "not_attempted" | "skipped" | "not_applicable" => Self::NotRun,
            _ => Self::NotRecorded,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VerificationObservation {
    pub(crate) command: String,
    pub(crate) status: VerificationStatus,
    pub(crate) exit_code: Option<i32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TerminalReport {
    pub(crate) status: Option<TerminalStatus>,
    pub(crate) assurance: Option<String>,
    pub(crate) gate: Option<String>,
    pub(crate) stop_reason: Option<String>,
    pub(crate) next_action: Option<String>,
    pub(crate) changed_files: Vec<String>,
    pub(crate) verifications: Vec<VerificationObservation>,
    pub(crate) exit_code: Option<i32>,
}

pub(crate) fn project(
    events: &[Value],
    events_path: Option<&Path>,
    workspace_root: Option<&Path>,
    status_override: Option<TerminalStatus>,
    exit_code_override: Option<i32>,
) -> TerminalReport {
    let terminal = latest_terminal(events);
    let status = status_override.or_else(|| terminal.and_then(status_from_event));
    let workspace = workspace_root
        .map(Path::to_path_buf)
        .or_else(|| workspace_from_events(events))
        .or_else(|| events_path.and_then(workspace_from_standard_events_path));
    let assurance = terminal
        .and_then(|event| text(event, "assurance_level"))
        .or_else(|| latest_event_text(events, "ultra_final_acceptance", "assurance_level"));
    let gate = terminal
        .and_then(|event| text(event, "release_gate_status"))
        .or_else(|| latest_event_text(events, "ultra_final_acceptance", "release_gate_status"))
        .or_else(|| latest_event_text(events, "plan_final_contract", "release_gate_status"));
    let stop_reason = terminal
        .and_then(|event| {
            text(event, "stop_reason")
                .or_else(|| text(event, "primary_reason"))
                .or_else(|| text(event, "failure_kind"))
        })
        .map(|reason| super::render_stop_reason_text(&reason))
        .or_else(|| (status == Some(TerminalStatus::Completed)).then(|| "completed".to_string()));
    let next_action = terminal
        .and_then(|event| {
            text(event, "next_action").or_else(|| text(event, "recovery_next_action"))
        })
        .or_else(|| latest_event_text(events, "ultra_final_acceptance", "next_action"))
        .or_else(|| latest_event_text(events, "plan_final_contract", "next_action"));
    let exit_code = exit_code_override.or_else(|| status.map(TerminalStatus::default_exit_code));

    TerminalReport {
        status,
        assurance,
        gate,
        stop_reason,
        next_action,
        changed_files: workspace.as_deref().map(changed_files).unwrap_or_default(),
        verifications: verification_observations(events),
        exit_code,
    }
}

pub(crate) fn read_events(path: Option<&Path>) -> Vec<Value> {
    path.and_then(|path| std::fs::read_to_string(path).ok())
        .map(|text| {
            text.lines()
                .filter_map(|line| serde_json::from_str(line).ok())
                .collect()
        })
        .unwrap_or_default()
}

fn latest_terminal(events: &[Value]) -> Option<&Value> {
    events
        .iter()
        .rev()
        .find(|event| event_name(event) == Some("run_stop"))
        .or_else(|| {
            events
                .iter()
                .rev()
                .find(|event| event_name(event) == Some("tui_command_stop"))
        })
}

fn status_from_event(event: &Value) -> Option<TerminalStatus> {
    let recorded = event.get("status").and_then(Value::as_str).unwrap_or("");
    if matches!(recorded, "interrupted" | "aborted") {
        return Some(TerminalStatus::Interrupted);
    }
    event
        .get("ok")
        .and_then(Value::as_bool)
        .map(|ok| {
            if ok {
                TerminalStatus::Completed
            } else {
                TerminalStatus::Failed
            }
        })
        .or(match recorded {
            "completed" | "complete" | "partial" => Some(TerminalStatus::Completed),
            "failed" | "incomplete" => Some(TerminalStatus::Failed),
            _ => None,
        })
}

fn latest_event_text(events: &[Value], name: &str, key: &str) -> Option<String> {
    events.iter().rev().find_map(|event| {
        (event_name(event) == Some(name))
            .then(|| text(event, key))
            .flatten()
    })
}

fn event_name(event: &Value) -> Option<&str> {
    event.get("event").and_then(Value::as_str)
}

fn text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn workspace_from_events(events: &[Value]) -> Option<PathBuf> {
    let value = events.iter().rev().find_map(|event| {
        (event_name(event) == Some("run_start"))
            .then(|| text(event, "workspace_root"))
            .flatten()
    })?;
    (!value.contains("<user>"))
        .then(|| PathBuf::from(value))
        .filter(|path| path.is_dir())
}

fn workspace_from_standard_events_path(path: &Path) -> Option<PathBuf> {
    let run_dir = path.parent()?;
    let runs_dir = run_dir.parent()?;
    let state_dir = runs_dir.parent()?;
    if path.file_name()?.to_str()? == "events.jsonl"
        && runs_dir.file_name()?.to_str()? == "runs"
        && state_dir.file_name()?.to_str()? == ".anvil"
    {
        state_dir.parent().map(Path::to_path_buf)
    } else {
        None
    }
}

fn changed_files(workspace: &Path) -> Vec<String> {
    let mut command = Command::new("git");
    command
        .arg("--no-optional-locks")
        .arg("-C")
        .arg(workspace)
        .args([
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--",
            ".",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let Ok(output) = crate::bounded_process::run_with_timeout(&mut command, Duration::from_secs(2))
    else {
        return Vec::new();
    };
    if !output.success() {
        return Vec::new();
    }
    parse_porcelain_v1_z(&output.stdout)
}

fn parse_porcelain_v1_z(bytes: &[u8]) -> Vec<String> {
    let records = bytes.split(|byte| *byte == 0).collect::<Vec<_>>();
    let mut paths = Vec::new();
    let mut index = 0usize;
    while index < records.len() {
        let record = records[index];
        index += 1;
        if record.len() < 4 || record[2] != b' ' {
            continue;
        }
        let renamed_or_copied =
            matches!(record[0], b'R' | b'C') || matches!(record[1], b'R' | b'C');
        let path = String::from_utf8_lossy(&record[3..]).trim().to_string();
        if !path.is_empty() {
            paths.push(path);
        }
        if renamed_or_copied && index < records.len() {
            index += 1;
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

fn verification_observations(events: &[Value]) -> Vec<VerificationObservation> {
    let mut observations = BTreeMap::<String, VerificationObservation>::new();
    for event in events {
        collect_declared_commands(event, &mut observations);
        collect_declarative_command_check(event, &mut observations);
        collect_build_verifier_observations(event, &mut observations);
        collect_timeout_observation(event, &mut observations);
    }
    observations.into_values().collect()
}

fn collect_declared_commands(
    event: &Value,
    observations: &mut BTreeMap<String, VerificationObservation>,
) {
    let recorded_status = event
        .get("verification_summary")
        .and_then(|summary| summary.get("status"))
        .and_then(Value::as_str)
        .map(VerificationStatus::from_value)
        .unwrap_or(VerificationStatus::NotRecorded);
    for key in ["verify", "verify_commands"] {
        for command in string_array(event, key) {
            record_observation(observations, command, recorded_status, None);
        }
    }
}

fn collect_declarative_command_check(
    event: &Value,
    observations: &mut BTreeMap<String, VerificationObservation>,
) {
    if event_name(event) != Some("declarative_command_check_result") {
        return;
    }
    let command = event
        .get("argv")
        .and_then(Value::as_array)
        .map(|argv| {
            argv.iter()
                .filter_map(Value::as_str)
                .map(shell_display_arg)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|command| !command.is_empty());
    let Some(command) = command else {
        return;
    };
    let status = text(event, "status")
        .as_deref()
        .map(VerificationStatus::from_value)
        .unwrap_or(VerificationStatus::NotRecorded);
    let exit_code = event
        .get("observed_exit_code")
        .and_then(Value::as_i64)
        .and_then(|code| i32::try_from(code).ok());
    record_observation(observations, command, status, exit_code);
}

fn collect_build_verifier_observations(
    event: &Value,
    observations: &mut BTreeMap<String, VerificationObservation>,
) {
    if let Some(values) = event
        .get("build_verifier_observations")
        .and_then(Value::as_array)
    {
        for value in values {
            collect_build_observation(value, observations);
        }
    }
    if let Some(values) = event
        .get("build_verifier_lifecycle")
        .and_then(Value::as_array)
    {
        for lifecycle in values {
            if let Some(observation) = lifecycle
                .get("after_setup")
                .filter(|value| !value.is_null())
                .or_else(|| lifecycle.get("before_setup"))
            {
                collect_build_observation(observation, observations);
            }
        }
    }
}

fn collect_build_observation(
    value: &Value,
    observations: &mut BTreeMap<String, VerificationObservation>,
) {
    let Some(command) = text(value, "command") else {
        return;
    };
    let status = text(value, "status")
        .as_deref()
        .map(VerificationStatus::from_value)
        .unwrap_or(VerificationStatus::NotRecorded);
    record_observation(observations, command, status, None);
}

fn collect_timeout_observation(
    event: &Value,
    observations: &mut BTreeMap<String, VerificationObservation>,
) {
    match event_name(event) {
        Some("verify_command_timeout") => {
            if let Some(command) = text(event, "command") {
                record_observation(observations, command, VerificationStatus::TimedOut, None);
            }
        }
        Some("verify_command_timeout_substitution") => {
            if let Some(command) = text(event, "substitution") {
                let status = text(event, "status")
                    .as_deref()
                    .map(VerificationStatus::from_value)
                    .unwrap_or(VerificationStatus::NotRecorded);
                record_observation(observations, command, status, None);
            }
        }
        _ => {}
    }
}

fn record_observation(
    observations: &mut BTreeMap<String, VerificationObservation>,
    command: String,
    status: VerificationStatus,
    exit_code: Option<i32>,
) {
    let command = super::body_snippet(command.trim());
    if command.is_empty() {
        return;
    }
    let current = observations.get(&command);
    if status == VerificationStatus::NotRecorded
        && current.is_some_and(|observation| observation.status != VerificationStatus::NotRecorded)
    {
        return;
    }
    observations.insert(
        command.clone(),
        VerificationObservation {
            command,
            status,
            exit_code,
        },
    );
}

fn string_array(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn shell_display_arg(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | ':'))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn porcelain_parser_returns_destination_paths_and_handles_spaces() {
        let parsed = parse_porcelain_v1_z(
            b" M src/main.rs\0?? notes with spaces.md\0R  src/new.rs\0src/old.rs\0",
        );
        assert_eq!(
            parsed,
            ["notes with spaces.md", "src/main.rs", "src/new.rs"]
        );
    }

    #[test]
    fn git_changed_files_are_sorted_and_deduplicated() {
        let root = tempfile::tempdir().unwrap();
        assert!(
            Command::new("git")
                .args(["init", "--quiet"])
                .current_dir(root.path())
                .status()
                .unwrap()
                .success()
        );
        std::fs::write(root.path().join("tracked.txt"), "before\n").unwrap();
        assert!(
            Command::new("git")
                .args(["add", "tracked.txt"])
                .current_dir(root.path())
                .status()
                .unwrap()
                .success()
        );
        std::fs::write(root.path().join("tracked.txt"), "after\n").unwrap();
        std::fs::write(root.path().join("new file.txt"), "new\n").unwrap();

        assert_eq!(changed_files(root.path()), ["new file.txt", "tracked.txt"]);
    }

    #[test]
    fn projection_keeps_unproven_verification_not_recorded() {
        let events = vec![
            json!({
                "event": "planner_fallback_plan",
                "verify": ["cargo test --test focused"]
            }),
            json!({
                "event": "step_short_circuited",
                "verify_commands": ["test -f output.txt"],
                "verification_summary": {"status": "pass"}
            }),
            json!({
                "event": "declarative_command_check_result",
                "argv": ["python", "-m", "pytest", "tests/smoke test.py"],
                "observed_exit_code": 1,
                "status": "failed"
            }),
            json!({
                "event": "tui_command_stop",
                "ok": false,
                "status": "failed",
                "assurance_level": "failed",
                "release_gate_status": "failed",
                "stop_reason": "verification failed",
                "next_action": "fix_command_failure"
            }),
        ];

        let report = project(&events, None, None, None, None);
        assert_eq!(report.status, Some(TerminalStatus::Failed));
        assert_eq!(report.exit_code, Some(1));
        assert_eq!(report.gate.as_deref(), Some("failed"));
        assert_eq!(report.stop_reason.as_deref(), Some("verification failed"));
        assert_eq!(report.verifications.len(), 3);
        assert_eq!(
            report.verifications[0],
            VerificationObservation {
                command: "cargo test --test focused".to_string(),
                status: VerificationStatus::NotRecorded,
                exit_code: None,
            }
        );
        assert_eq!(report.verifications[2].status, VerificationStatus::Passed);
    }

    #[test]
    fn explicit_terminal_override_supports_interrupt_consumer() {
        let report = project(
            &[],
            None,
            None,
            Some(TerminalStatus::Interrupted),
            Some(130),
        );
        assert_eq!(report.status, Some(TerminalStatus::Interrupted));
        assert_eq!(report.exit_code, Some(130));
    }

    #[test]
    fn process_failure_is_authoritative_over_an_earlier_tui_success() {
        let events = vec![
            json!({"event": "tui_command_stop", "ok": true, "status": "completed"}),
            json!({"event": "run_stop", "ok": false, "status": "complete"}),
        ];

        let report = project(&events, None, None, None, None);

        assert_eq!(report.status, Some(TerminalStatus::Failed));
        assert_eq!(report.exit_code, Some(1));
    }
}
