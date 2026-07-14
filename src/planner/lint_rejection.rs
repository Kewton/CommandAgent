use std::fmt;
use std::path::Path;

use serde_json::json;

use crate::eval_events;
use crate::planner::lint::{PlanLintError, PlanLintReport};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyLintRejection {
    pub step_id: String,
    pub command_index: usize,
    pub original_command: String,
    pub normalized_commands: Vec<String>,
    pub violation_kind: String,
}

#[derive(Debug)]
pub struct VerifyLintFailure {
    pub message: String,
    pub rejection: VerifyLintRejection,
}

#[derive(Debug, Clone)]
pub struct PlanLintExhausted {
    pub report: PlanLintReport,
}

impl fmt::Display for PlanLintExhausted {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid StepPlan after corrective retries: {}",
            self.report.primary_message()
        )
    }
}

impl std::error::Error for PlanLintExhausted {}

pub fn lint_verify_command(
    step_id: &str,
    command_index: usize,
    command: &str,
) -> Result<(), VerifyLintFailure> {
    let diagnosis = crate::planner::verify::diagnose_verify_command(command);
    let violation_kind = diagnosis
        .violation
        .map(|kind| kind.as_str())
        .unwrap_or("verify_policy")
        .to_string();
    let normalized_commands =
        match crate::planner::verify::normalize_planner_verify_command(command) {
            Ok(commands) => commands,
            Err(error) => {
                return Err(VerifyLintFailure {
                    message: error.to_string(),
                    rejection: VerifyLintRejection {
                        step_id: step_id.to_string(),
                        command_index,
                        original_command: command.to_string(),
                        normalized_commands: Vec::new(),
                        violation_kind,
                    },
                });
            }
        };
    if let Some((message, normalized_kind)) = normalized_commands.iter().find_map(|normalized| {
        crate::planner::verify::validate_verify_command(normalized)
            .err()
            .map(|error| {
                let kind = crate::planner::verify::diagnose_verify_command(normalized)
                    .violation
                    .map(|kind| kind.as_str().to_string())
                    .unwrap_or_else(|| violation_kind.clone());
                (error.to_string(), kind)
            })
    }) {
        return Err(VerifyLintFailure {
            message,
            rejection: VerifyLintRejection {
                step_id: step_id.to_string(),
                command_index,
                original_command: command.to_string(),
                normalized_commands,
                violation_kind: normalized_kind,
            },
        });
    }
    Ok(())
}

pub fn rejected_commands(report: &PlanLintReport) -> Vec<String> {
    report
        .errors
        .iter()
        .filter_map(|error| error.verify_rejection.as_ref())
        .map(|rejection| rejection.original_command.clone())
        .fold(Vec::new(), |mut commands, command| {
            if !commands.contains(&command) {
                commands.push(command);
            }
            commands
        })
}

pub fn rejected_commands_from_error(error: &anyhow::Error) -> Vec<String> {
    error
        .downcast_ref::<PlanLintExhausted>()
        .map(|exhausted| rejected_commands(&exhausted.report))
        .unwrap_or_default()
}

pub fn retry_error_line(error: &PlanLintError) -> String {
    let original = error
        .verify_rejection
        .as_ref()
        .map(|rejection| {
            format!(
                "; rejected original_command: {}",
                serde_json::to_string(&rejection.original_command)
                    .expect("a command string always serializes")
            )
        })
        .unwrap_or_default();
    format!("- [{}] {}{original}", error.category, error.message)
}

pub fn emit_planner_error(
    events_path: Option<&Path>,
    provider: &str,
    model: &str,
    stage: &str,
    kind: &str,
    report: &PlanLintReport,
    attempt: usize,
) {
    let mut event = json!({
        "event": "planner_error",
        "planner_stage": stage,
        "planner_error_kind": kind,
        "planner_error_message": eval_events::body_snippet(&report.primary_message()),
        "planner_provider": provider,
        "planner_model": model,
        "repair_attempt": attempt,
    });
    if let Some(rejection) = report
        .errors
        .iter()
        .find_map(|error| error.verify_rejection.as_ref())
        && let Some(fields) = event.as_object_mut()
    {
        fields.insert("step_id".to_string(), json!(rejection.step_id));
        fields.insert("command_index".to_string(), json!(rejection.command_index));
        fields.insert(
            "original_command".to_string(),
            json!(rejection.original_command),
        );
        fields.insert(
            "normalized_commands".to_string(),
            json!(rejection.normalized_commands),
        );
        fields.insert(
            "violation_kind".to_string(),
            json!(rejection.violation_kind),
        );
    }
    eval_events::emit(events_path, event);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn shell_control_rejection_retains_original_command_and_location() {
        let original = "python3 pipeline/main.py > output/run.log";
        let failure = lint_verify_command("verify-results", 2, original).unwrap_err();

        assert_eq!(failure.rejection.step_id, "verify-results");
        assert_eq!(failure.rejection.command_index, 2);
        assert_eq!(failure.rejection.original_command, original);
        assert!(failure.rejection.normalized_commands.is_empty());
        assert_eq!(failure.rejection.violation_kind, "shell_control_syntax");
    }

    #[test]
    fn retry_line_quotes_the_rejected_original_command() {
        let failure =
            lint_verify_command("verify-results", 0, "cargo test | cargo metadata").unwrap_err();
        let error = PlanLintError {
            category: "verify_policy".to_string(),
            message: failure.message,
            verify_rejection: Some(failure.rejection),
        };

        let line = retry_error_line(&error);

        assert!(line.contains(r#"rejected original_command: "cargo test | cargo metadata""#));
    }

    #[test]
    fn lint_event_keeps_existing_fields_and_adds_rejection_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let failure =
            lint_verify_command("verify-results", 1, "cargo test | cargo metadata").unwrap_err();
        let report = PlanLintReport {
            errors: vec![PlanLintError {
                category: "verify_policy".to_string(),
                message: failure.message,
                verify_rejection: Some(failure.rejection),
            }],
        };

        emit_planner_error(
            Some(&path),
            "ollama",
            "planner-model",
            "verify_policy",
            "verify_command_policy_error",
            &report,
            3,
        );

        let event: Value =
            serde_json::from_str(std::fs::read_to_string(path).unwrap().trim()).unwrap();
        assert_eq!(event["event"], "planner_error");
        assert_eq!(event["planner_stage"], "verify_policy");
        assert_eq!(event["planner_error_kind"], "verify_command_policy_error");
        assert_eq!(event["planner_provider"], "ollama");
        assert_eq!(event["planner_model"], "planner-model");
        assert_eq!(event["repair_attempt"], 3);
        assert_eq!(event["step_id"], "verify-results");
        assert_eq!(event["command_index"], 1);
        assert_eq!(event["original_command"], "cargo test | cargo metadata");
        assert_eq!(event["normalized_commands"], json!([]));
        assert_eq!(event["violation_kind"], "shell_control_syntax");
    }
}
