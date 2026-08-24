use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::eval_events;
use crate::planner::repair_targeting::{RepairTargetSelection, RepairTargetSelectionReason};
use crate::planner::verify::VerificationReport;

const TRACEBACK_HEADER: &str = "Traceback (most recent call last):";
const LEADING_FRAME_LIMIT: usize = 3;
const MESSAGE_LIMIT: usize = 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PythonTracebackFrame {
    pub file: String,
    pub line: usize,
    pub function: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PythonTraceback {
    pub final_frame: PythonTracebackFrame,
    pub leading_frames: Vec<PythonTracebackFrame>,
    pub exception_type: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub target_path: Option<String>,
}

impl PythonTraceback {
    pub(crate) fn signature(&self) -> String {
        format!(
            "traceback:{}:{}:{}:{}",
            self.target_path
                .as_deref()
                .unwrap_or(self.final_frame.file.as_str()),
            self.final_frame.line,
            self.exception_type,
            self.message
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineErrorExtractionStatus {
    Extracted,
    NoTraceback,
    MalformedTraceback,
}

impl PipelineErrorExtractionStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Extracted => "extracted",
            Self::NoTraceback => "no_traceback",
            Self::MalformedTraceback => "malformed_traceback",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PythonTracebackExtraction {
    pub status: PipelineErrorExtractionStatus,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub traceback: Option<PythonTraceback>,
}

pub fn extract(stderr: &str, root: &Path) -> PythonTracebackExtraction {
    if !stderr.contains(TRACEBACK_HEADER) {
        return PythonTracebackExtraction {
            status: PipelineErrorExtractionStatus::NoTraceback,
            traceback: None,
        };
    }
    let Some((frames, exception_type, message)) = parse_traceback(stderr) else {
        return PythonTracebackExtraction {
            status: PipelineErrorExtractionStatus::MalformedTraceback,
            traceback: None,
        };
    };
    let final_frame = frames.last().cloned().expect("parser requires a frame");
    let target_path = workspace_python_target(root, &final_frame.file);
    PythonTracebackExtraction {
        status: PipelineErrorExtractionStatus::Extracted,
        traceback: Some(PythonTraceback {
            final_frame,
            leading_frames: frames.into_iter().take(LEADING_FRAME_LIMIT).collect(),
            exception_type,
            message: bounded_chars(&message, MESSAGE_LIMIT),
            target_path,
        }),
    }
}

pub(crate) fn extract_failed_command(
    command: &str,
    stderr: &str,
    root: &Path,
    eval_events_path: Option<&Path>,
) -> Option<PythonTraceback> {
    if !command_may_run_python(command) && !stderr.contains(TRACEBACK_HEADER) {
        return None;
    }
    let extraction = extract(stderr, root);
    emit_extraction(&extraction, eval_events_path, "verify", command, stderr);
    extraction.traceback
}

pub(crate) fn attach_pipeline_report(
    evidence: &super::pipeline_probe::PipelineProbeReport,
    eval_events_path: Option<&Path>,
    report: &mut VerificationReport,
) {
    attach_pipeline_extraction(
        evidence.python_error_extraction.clone(),
        eval_events_path,
        &evidence.command.join(" "),
        &evidence.stderr.text,
        report,
    );
}

fn attach_pipeline_extraction(
    extraction: Option<PythonTracebackExtraction>,
    eval_events_path: Option<&Path>,
    command: &str,
    stderr: &str,
    report: &mut VerificationReport,
) {
    let Some(extraction) = extraction else {
        return;
    };
    emit_extraction(
        &extraction,
        eval_events_path,
        "pipeline_probe",
        command,
        stderr,
    );
    if let Some(traceback) = extraction.traceback {
        report.push_python_traceback(traceback);
    }
}

fn emit_extraction(
    extraction: &PythonTracebackExtraction,
    eval_events_path: Option<&Path>,
    source: &str,
    command: &str,
    stderr: &str,
) {
    let traceback = extraction.traceback.as_ref();
    eval_events::emit(
        eval_events_path,
        json!({
            "event": "pipeline_error_extraction",
            "source": source,
            "status": extraction.status.as_str(),
            "command": eval_events::body_snippet(command),
            "final_frame": traceback.map(|value| &value.final_frame),
            "leading_frames": traceback.map(|value| &value.leading_frames),
            "exception_type": traceback.map(|value| value.exception_type.as_str()).unwrap_or(""),
            "message": traceback.map(|value| value.message.as_str()).unwrap_or(""),
            "repair_target": traceback.and_then(|value| value.target_path.as_deref()).unwrap_or(""),
            "selection_reason": traceback.and_then(|value| value.target_path.as_ref()).map(|_| "traceback_mapped").unwrap_or("fallback"),
            "fallback_stderr": traceback.is_none().then(|| eval_events::body_snippet(stderr)),
        }),
    );
}

pub(crate) fn resolve_repair_target(report: &VerificationReport) -> Option<RepairTargetSelection> {
    let target = report
        .python_tracebacks
        .iter()
        .rev()
        .find_map(|traceback| traceback.target_path.clone())?;
    Some(RepairTargetSelection {
        selected_targets: vec![target],
        selection_reason: RepairTargetSelectionReason::TracebackMapped,
    })
}

pub(crate) fn append_repair_guidance(prompt: &mut String, report: &VerificationReport) {
    let Some(traceback) = report.python_tracebacks.last() else {
        return;
    };
    prompt.push_str("\n\nPython traceback repair guidance:\n");
    if let Some(target) = &traceback.target_path {
        prompt.push_str(&format!("- repair target: {target}\n"));
    }
    prompt.push_str(&format!(
        "- final frame: {}:{} in {}\n- exception: {}: {}\n",
        traceback.final_frame.file,
        traceback.final_frame.line,
        traceback.final_frame.function,
        traceback.exception_type,
        traceback.message
    ));
    if !traceback.leading_frames.is_empty() {
        prompt.push_str("- leading frames:\n");
        for frame in &traceback.leading_frames {
            prompt.push_str(&format!(
                "  - {}:{} in {}\n",
                frame.file, frame.line, frame.function
            ));
        }
    }
    prompt.push_str(
        "Edit the mapped Python script at the final frame; do not continue with read-only inspection.",
    );
}

fn parse_traceback(stderr: &str) -> Option<(Vec<PythonTracebackFrame>, String, String)> {
    let lines = stderr.lines().collect::<Vec<_>>();
    let header = lines
        .iter()
        .rposition(|line| line.trim() == TRACEBACK_HEADER)?;
    let mut frames = Vec::new();
    let mut last_frame_line = None;
    for (index, line) in lines.iter().enumerate().skip(header + 1) {
        if let Some(frame) = parse_frame(line) {
            frames.push(frame);
            last_frame_line = Some(index);
        }
    }
    let last_frame_line = last_frame_line?;
    let (exception_type, message) = lines
        .iter()
        .skip(last_frame_line + 1)
        .rev()
        .find_map(|line| parse_exception(line))?;
    Some((frames, exception_type, message))
}

fn parse_frame(line: &str) -> Option<PythonTracebackFrame> {
    let rest = line.trim_start().strip_prefix("File \"")?;
    let (file, rest) = rest.split_once("\", line ")?;
    let (line, function) = rest.split_once(", in ")?;
    Some(PythonTracebackFrame {
        file: file.to_string(),
        line: line.parse().ok()?,
        function: function.trim().to_string(),
    })
}

fn parse_exception(line: &str) -> Option<(String, String)> {
    if line.starts_with(char::is_whitespace) || line.trim().is_empty() {
        return None;
    }
    let trimmed = line.trim();
    let (kind, message) = trimmed.split_once(':').unwrap_or((trimmed, ""));
    let valid_kind = !kind.is_empty()
        && kind
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.'));
    valid_kind.then(|| (kind.to_string(), message.trim_start().to_string()))
}

fn workspace_python_target(root: &Path, file: &str) -> Option<String> {
    let raw = Path::new(file);
    let relative = if raw.is_absolute() {
        raw.strip_prefix(root).map(PathBuf::from).ok().or_else(|| {
            let canonical_root = std::fs::canonicalize(root).ok()?;
            let canonical_file = std::fs::canonicalize(raw).ok()?;
            canonical_file
                .strip_prefix(canonical_root)
                .ok()
                .map(PathBuf::from)
        })?
    } else {
        raw.to_path_buf()
    };
    if relative.extension().and_then(|value| value.to_str()) != Some("py")
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        return None;
    }
    let normalized = relative
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/");
    (!normalized.is_empty()
        && !normalized
            .split('/')
            .any(|component| matches!(component, ".commandagent" | ".anvil" | ".git")))
    .then_some(normalized)
}

fn command_may_run_python(command: &str) -> bool {
    command.split_whitespace().any(|token| {
        matches!(
            token.trim_matches(['\'', '"']),
            "python" | "python3" | "pytest" | "py.test"
        )
    })
}

fn bounded_chars(value: &str, limit: usize) -> String {
    value.chars().take(limit).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PromptLayout;
    use crate::minimal_loop::dependency_setup::NodeDependencySetupAuthority;
    use crate::minimal_loop::pipeline_probe::{self, PipelineProbeConfig};
    use crate::planner::repair::{RepairContext, build_repair_prompt_with_context};
    use crate::planner::step_plan::PlanStep;
    use crate::planner::verify::verify_step_with_profile_setup_observed_with_offline_and_events;

    fn traceback(root: &Path, exception: &str) -> PythonTracebackExtraction {
        extract(
            &format!(
                "Traceback (most recent call last):\n  File \"{}/pipeline/main.py\", line 12, in run\n    parse()\n  File \"{}/pipeline/main.py\", line 7, in parse\n    raise\n{exception}\n",
                root.display(),
                root.display()
            ),
            root,
        )
    }

    #[test]
    fn extracts_value_error_final_frame_and_bounded_leading_frames() {
        let dir = tempfile::tempdir().unwrap();
        let stderr = format!(
            "Traceback (most recent call last):\n  File \"{0}/pipeline/main.py\", line 169, in <module>\n    run()\n  File \"{0}/pipeline/main.py\", line 89, in run\n    amount = parse_amount(row[\"amount\"])\n  File \"{0}/pipeline/helpers.py\", line 11, in dispatch\n    parse_amount(value)\n  File \"{0}/pipeline/main.py\", line 53, in parse_amount\n    return int(val.strip())\nValueError: invalid literal for int() with base 10: ''\n",
            dir.path().display()
        );

        let extraction = extract(&stderr, dir.path());
        let parsed = extraction.traceback.unwrap();

        assert_eq!(extraction.status, PipelineErrorExtractionStatus::Extracted);
        assert_eq!(parsed.final_frame.line, 53);
        assert_eq!(parsed.exception_type, "ValueError");
        assert_eq!(parsed.target_path.as_deref(), Some("pipeline/main.py"));
        assert_eq!(parsed.leading_frames.len(), LEADING_FRAME_LIMIT);
        assert_eq!(parsed.leading_frames[0].line, 169);
    }

    #[test]
    fn extracts_key_error_without_losing_quoted_message() {
        let dir = tempfile::tempdir().unwrap();
        let parsed = traceback(dir.path(), "KeyError: 'amount'")
            .traceback
            .unwrap();

        assert_eq!(parsed.exception_type, "KeyError");
        assert_eq!(parsed.message, "'amount'");
        assert_eq!(parsed.final_frame.function, "parse");
    }

    #[test]
    fn extracts_file_not_found_error_message() {
        let dir = tempfile::tempdir().unwrap();
        let parsed = traceback(
            dir.path(),
            "FileNotFoundError: [Errno 2] No such file or directory: 'data/sales.csv'",
        )
        .traceback
        .unwrap();

        assert_eq!(parsed.exception_type, "FileNotFoundError");
        assert!(parsed.message.contains("data/sales.csv"));
    }

    #[test]
    fn records_deterministic_fallback_status() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            extract("python: syntax error", dir.path()).status,
            PipelineErrorExtractionStatus::NoTraceback
        );
        assert_eq!(
            extract(
                "Traceback (most recent call last):\nValueError: missing frame",
                dir.path()
            )
            .status,
            PipelineErrorExtractionStatus::MalformedTraceback
        );
    }

    #[test]
    fn failed_python_without_traceback_emits_fallback_event() {
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join("events.jsonl");

        let parsed = extract_failed_command(
            "python3 verify.py",
            "verification failed without frames",
            dir.path(),
            Some(&events),
        );

        assert!(parsed.is_none());
        let event_text = std::fs::read_to_string(events).unwrap();
        assert!(event_text.contains("\"status\":\"no_traceback\""));
        assert!(event_text.contains("\"selection_reason\":\"fallback\""));
        assert!(event_text.contains("verification failed without frames"));
    }

    #[test]
    fn repair_prompt_and_unified_targeting_use_traceback_mapping() {
        let dir = tempfile::tempdir().unwrap();
        let mut report = VerificationReport::profile_failed("pipeline_probe:pipeline_exit_nonzero");
        report.push_python_traceback(
            traceback(dir.path(), "ValueError: invalid amount")
                .traceback
                .unwrap(),
        );
        let context = RepairContext {
            workspace_root: Some(dir.path().to_path_buf()),
            prompt_layout: PromptLayout::Stable,
            ..RepairContext::default()
        };

        let prompt = build_repair_prompt_with_context("pipeline", &report, &context);
        let selection =
            crate::planner::repair_targeting::resolve_traceback_repair_target(&report).unwrap();

        assert!(prompt.contains("Python traceback repair guidance:"));
        assert!(prompt.contains("repair target: pipeline/main.py"));
        assert!(prompt.contains("ValueError: invalid amount"));
        assert_eq!(selection.primary_target(), Some("pipeline/main.py"));
        assert_eq!(selection.selection_reason.as_str(), "traceback_mapped");
    }

    #[test]
    fn non_python_nextjs_guidance_path_is_byte_noop() {
        let report = VerificationReport::command_failed("npm run build", "build failed");
        let mut prompt = "nextjs repair bytes".to_string();

        append_repair_guidance(&mut prompt, &report);

        assert_eq!(prompt.as_bytes(), b"nextjs repair bytes");
    }

    #[test]
    fn pipeline_probe_retains_runtime_traceback_extraction() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("pipeline")).unwrap();
        std::fs::write(
            dir.path().join("pipeline/main.py"),
            "raise ValueError('invalid amount')\n",
        )
        .unwrap();

        let report =
            pipeline_probe::run(dir.path(), PipelineProbeConfig::new("pipeline/main.py")).unwrap();

        let extraction = report.python_error_extraction.unwrap();
        assert_eq!(extraction.status, PipelineErrorExtractionStatus::Extracted);
        assert_eq!(
            extraction.traceback.unwrap().target_path.as_deref(),
            Some("pipeline/main.py")
        );
    }

    #[test]
    fn data_catalog_probe_attaches_traceback_to_repair_report() {
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join("events.jsonl");
        std::fs::create_dir_all(dir.path().join("pipeline")).unwrap();
        std::fs::write(
            dir.path().join("pipeline/main.py"),
            "raise FileNotFoundError('data/sales.csv')\n",
        )
        .unwrap();
        let step = PlanStep {
            id: "probe-pipeline".to_string(),
            kind: "verify".to_string(),
            instruction: "run pipeline".to_string(),
            expected_paths: Vec::new(),
            verify: vec![
                crate::planner::profiles::data::step_policy::catalog_check_command(
                    "pipeline_probe",
                ),
            ],
            expected_result: "pass".to_string(),
        };

        let (report, _) = verify_step_with_profile_setup_observed_with_offline_and_events(
            dir.path(),
            &step,
            Some("data"),
            NodeDependencySetupAuthority::None,
            false,
            Some(&events),
        );

        assert_eq!(
            report.python_tracebacks[0].exception_type,
            "FileNotFoundError"
        );
        assert_eq!(
            resolve_repair_target(&report).unwrap().primary_target(),
            Some("pipeline/main.py")
        );
        let event_text = std::fs::read_to_string(events).unwrap();
        assert!(event_text.contains("\"source\":\"pipeline_probe\""));
    }

    #[test]
    fn verify_failure_emits_extraction_and_attaches_traceback() {
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join("events.jsonl");
        std::fs::write(dir.path().join("verify.py"), "raise KeyError('amount')\n").unwrap();
        let step = PlanStep {
            id: "verify-python".to_string(),
            kind: "verify".to_string(),
            instruction: "run verifier".to_string(),
            expected_paths: Vec::new(),
            verify: vec!["python3 verify.py".to_string()],
            expected_result: "pass".to_string(),
        };

        let (report, _) = verify_step_with_profile_setup_observed_with_offline_and_events(
            dir.path(),
            &step,
            None,
            NodeDependencySetupAuthority::None,
            false,
            Some(&events),
        );

        assert_eq!(report.python_tracebacks[0].exception_type, "KeyError");
        let event_text = std::fs::read_to_string(events).unwrap();
        assert!(event_text.contains("\"event\":\"pipeline_error_extraction\""));
        assert!(event_text.contains("\"source\":\"verify\""));
        assert!(event_text.contains("\"selection_reason\":\"traceback_mapped\""));
    }
}
