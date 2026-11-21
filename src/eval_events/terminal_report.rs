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

/// Scrub a rendered summary with the run scope registered for its events path,
/// just before it is saved. `human_summary` renders free text the terminal
/// report projects from the events file (`Stop reason`, `Next action`) ahead of
/// the caller's (already scrubbed) body, so that text never passes through
/// `summary_body`'s scrub. The renderer's own fixed text — the headings and
/// separators that delimit the document, every field label, and a fixed status
/// under a closed-value label — is replaced by a placeholder first and restored
/// after, so a registered value that equals one of them cannot rewrite it or
/// break the boundary `append_run_summary` splits on. The scrub still sees the
/// whole document at once, so a value that spans lines is matched as one
/// string. A missing scope leaves it unchanged.
pub(crate) fn scrub_summary_document(path: &Path, document: &str) -> String {
    let Some(context) = crate::sensitive_data::active_for(Some(path)) else {
        return document.to_string();
    };
    let (protected, fixed) = protect_summary_fixed_text(document);
    let mut scrubbed = context.scrub_text(&protected);
    for (placeholder, value) in &fixed {
        scrubbed = scrubbed.replace(placeholder.as_str(), value);
    }
    scrubbed
}

/// The labels `human_summary` renders, each marked closed when its value is a
/// product status (so a fixed identifier there is preserved). The localized
/// header uses the `SummaryLanguage` labels; the machine-details body keeps the
/// caller's English labels (`Status:`, `Result:`), which `summary_body` already
/// protected the same way.
fn summary_labels(language: super::summary_language::SummaryLanguage) -> Vec<(String, bool)> {
    vec![
        (format!("{}:", language.result_label()), true),
        (format!("{}:", language.assurance_label()), true),
        (format!("{}:", language.gate_label()), true),
        (format!("{}:", language.stop_reason_label()), false),
        (format!("{}:", language.next_action_label()), false),
        (format!("{}:", language.changed_files_label()), false),
        (format!("{}:", language.verification_label()), false),
        (format!("{}:", language.exit_code_label()), false),
        ("Status:".to_string(), true),
        ("Result:".to_string(), true),
        ("Assurance:".to_string(), true),
        ("Gate:".to_string(), true),
        ("Stop reason:".to_string(), false),
        ("Next action:".to_string(), false),
        ("Changed files:".to_string(), false),
        ("Verification:".to_string(), false),
        ("Exit code:".to_string(), false),
    ]
}

/// Replace the summary's fixed structure with placeholders so the save-point
/// scrub cannot rewrite it. Every other byte, including the projected free
/// text, is left in place for the scrub.
fn protect_summary_fixed_text(document: &str) -> (String, Vec<(String, String)>) {
    let language = super::summary_language::SummaryLanguage::from_process_locale();
    let structural = [
        format!("# {}", language.heading()),
        "## Machine details".to_string(),
        format!("## {}", language.changed_files_label()),
        format!("## {}", language.verification_commands_heading()),
        format!("- {}", language.none()),
        "---".to_string(),
    ];
    let labels = summary_labels(language);
    let mut text = String::new();
    let mut fixed = Vec::new();
    for (index, line) in document.split('\n').enumerate() {
        if index > 0 {
            text.push('\n');
        }
        if structural.iter().any(|structural| structural == line) {
            push_summary_placeholder(&mut text, &mut fixed, line);
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let (prefix_end, body) = match line[indent..].strip_prefix("- ") {
            Some(body) => (indent + 2, body),
            None => (indent, &line[indent..]),
        };
        let Some((label, closed, rest)) = labels.iter().find_map(|(label, closed)| {
            body.strip_prefix(label.as_str())
                .map(|rest| (label.as_str(), *closed, rest))
        }) else {
            text.push_str(line);
            continue;
        };
        push_summary_placeholder(&mut text, &mut fixed, &line[..prefix_end + label.len()]);
        let (separator, value) = match rest.strip_prefix(' ') {
            Some(value) => (" ", value),
            None => ("", rest),
        };
        text.push_str(separator);
        if closed && crate::sensitive_data::is_fixed_identifier(value) {
            push_summary_placeholder(&mut text, &mut fixed, value);
        } else {
            text.push_str(value);
        }
    }
    (text, fixed)
}

fn push_summary_placeholder(text: &mut String, fixed: &mut Vec<(String, String)>, value: &str) {
    // The sentinel keeps one placeholder from matching inside another.
    let placeholder = format!("\u{1}commandagentsummaryfixed{}\u{1}", fixed.len());
    text.push_str(&placeholder);
    fixed.push((placeholder, value.to_string()));
}

/// Every line that is one JSON value, in file order. A line cut inside a
/// multi-byte character or otherwise malformed is skipped on its own; the whole
/// lines around it are still returned.
pub(crate) fn read_events(path: Option<&Path>) -> Vec<Value> {
    super::event_reader::read_lenient_opt(path)
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
        && matches!(state_dir.file_name()?.to_str()?, ".commandagent" | ".anvil")
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
        collect_verify_command_observation(event, &mut observations);
        collect_declarative_command_check(event, &mut observations);
        collect_build_verifier_observations(event, &mut observations);
        collect_timeout_observation(event, &mut observations);
    }
    observations.into_values().collect()
}

fn collect_verify_command_observation(
    event: &Value,
    observations: &mut BTreeMap<String, VerificationObservation>,
) {
    if event_name(event) != Some("verify_command_observed") {
        return;
    }
    let Some(command) = text(event, "effective_command") else {
        return;
    };
    let status = text(event, "status")
        .as_deref()
        .map(VerificationStatus::from_value)
        .unwrap_or(VerificationStatus::NotRecorded);
    record_observation(observations, command, status, None);
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

    /// The review of Issue #1059: once a broken line no longer hides the whole
    /// line before it, the terminal report projects that line's free text
    /// (`Next action`, `Stop reason`) ahead of the caller's body, past
    /// `summary_body`'s scrub. Both summary writers must scrub the rendered
    /// document with the events path's scope, so a registered value that only
    /// the kept line holds never reaches `summary.md`, while the fixed status
    /// and the ordinary event fields still render.
    #[test]
    fn a_projected_next_action_is_scrubbed_before_the_summary_is_saved() {
        crate::eval_events::summary_language::with_language_for_test(
            crate::eval_events::summary_language::SummaryLanguage::English,
            projected_next_action_summary,
        );
    }

    fn projected_next_action_summary() {
        let secret = "SYNTHETIC_NEXT_ACTION_SECRET_42";
        let root = tempfile::tempdir().unwrap();
        let path = root
            .path()
            .join(".commandagent/runs/review-run/events.jsonl");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let failed = json!({
            "event": "run_stop", "ok": false, "status": "failed",
            "stop_reason": format!("verification failed ({secret})"),
            "next_action": format!("resume_with {secret}"),
        });
        let mut bytes = serde_json::to_vec(&failed).unwrap();
        bytes.push(b'\n');
        // Line 2 is cut inside a multi-byte character and holds no secret.
        bytes.extend_from_slice(b"{\"event\":\"fixture\",\"text\":\"\xE8\xA9\"}\n");
        std::fs::write(&path, &bytes).unwrap();

        let mut catalog = crate::sensitive_data::SecretCatalog::new();
        catalog.register_credential("review_token", secret).unwrap();
        crate::sensitive_data::install_scope(catalog, Some(root.path()), Some(&path));

        crate::eval_events::write_run_summary(
            Some(&path),
            "Status: failed\nCompleted phases:\n- scaffold",
        );
        let summary = std::fs::read_to_string(path.parent().unwrap().join("summary.md")).unwrap();
        assert!(!summary.contains(secret), "{summary}");
        assert!(summary.contains("<redacted>"), "{summary}");
        assert!(summary.contains("- Result: failed"), "{summary}");
        assert!(
            summary.contains("- Stop reason: verification failed"),
            "{summary}"
        );

        crate::eval_events::append_run_summary(Some(&path), "TUI command failed: phase failed");
        let summary = std::fs::read_to_string(path.parent().unwrap().join("summary.md")).unwrap();
        assert!(!summary.contains(secret), "{summary}");
        assert!(summary.contains("<redacted>"), "{summary}");
        assert!(summary.contains("- Result: failed"), "{summary}");
        assert!(
            summary.contains("---\n\nTUI command failed: phase failed"),
            "{summary}"
        );
        assert!(summary.contains("- scaffold"), "{summary}");

        // Only this test's scope is restored: drop its thread-local scope and
        // leave unregistered keys untouched, because `reset_scopes_for_tests`
        // would clear scopes other lib tests registered in this process.
        crate::sensitive_data::set_current(None);
    }

    /// The reviewer's second reproduction: scrubbing the whole rendered
    /// summary also rewrote the renderer's own fixed strings when a registered
    /// value equalled one of them. With `Machine details` registered (and the
    /// value kept out of the events file and the body), the `## Machine details`
    /// boundary must survive both writers, so `append_run_summary` still finds
    /// it and re-renders exactly one `# Run result` heading.
    #[test]
    fn fixed_summary_boundaries_survive_a_registered_heading_value() {
        crate::eval_events::summary_language::with_language_for_test(
            crate::eval_events::summary_language::SummaryLanguage::English,
            fixed_summary_boundaries,
        );
    }

    fn fixed_summary_boundaries() {
        let root = tempfile::tempdir().unwrap();
        let path = root
            .path()
            .join(".commandagent/runs/heading-run/events.jsonl");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut catalog = crate::sensitive_data::SecretCatalog::new();
        catalog.register("Machine details");
        crate::sensitive_data::install_scope(catalog, Some(root.path()), Some(&path));

        crate::eval_events::write_run_summary(
            Some(&path),
            "Status: failed\nCompleted phases:\n- scaffold",
        );
        let summary = std::fs::read_to_string(path.parent().unwrap().join("summary.md")).unwrap();
        assert!(summary.contains("## Machine details"), "{summary}");

        crate::eval_events::append_run_summary(Some(&path), "TUI command failed: phase failed");
        let summary = std::fs::read_to_string(path.parent().unwrap().join("summary.md")).unwrap();
        assert!(summary.contains("## Machine details"), "{summary}");
        assert_eq!(
            summary.matches("# Run result").count(),
            1,
            "the boundary broke and the previous header was appended twice: {summary}"
        );
        assert!(
            summary.contains("---\n\nTUI command failed: phase failed"),
            "{summary}"
        );

        crate::sensitive_data::set_current(None);
    }

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
    fn plan_verify_execution_observations_populate_verify_commands() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "tests/corpus/apps/issue513-plan-verify-projection/fixtures/plan-verify-events.jsonl",
        );
        let events = read_events(Some(&path));

        let report = project(&events, Some(&path), None, None, None);

        assert!(
            !report.verifications.is_empty(),
            "a run that executed plan verify must not project empty verify_commands"
        );
        let by_command = report
            .verifications
            .iter()
            .map(|observation| (observation.command.as_str(), observation.status))
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(
            by_command.get("cargo test --test smoke"),
            Some(&VerificationStatus::Passed)
        );
        assert_eq!(
            by_command.get("python3 -m pytest tests"),
            Some(&VerificationStatus::Passed)
        );
        assert_eq!(
            by_command.get("npm run build"),
            Some(&VerificationStatus::Failed)
        );
        assert_eq!(
            by_command.get("test -f artifacts/report.json"),
            Some(&VerificationStatus::NotRecorded),
            "a declared-only command must not be upgraded to success"
        );
        assert_eq!(report.status, Some(TerminalStatus::Failed));
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
