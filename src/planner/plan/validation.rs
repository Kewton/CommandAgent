use std::fmt;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::guidance::{PlanFileKind, next_command};
use crate::planner::lint::{PlanLintError, lint_ultra_plan_report};
use crate::planner::step_plan::{PlanStep, StepPlan};
use crate::planner::ultra_plan::UltraPlan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanValidation {
    pub kind: PlanFileKind,
    pub recovery: bool,
    pub recovery_failure_kind: Option<String>,
    pub recovery_failed_scope: Option<String>,
    pub recovery_retained_artifacts: Vec<String>,
}

impl PlanValidation {
    pub fn render_success(&self, path: &Path) -> String {
        let label = match (self.kind, self.recovery) {
            (PlanFileKind::Step, _) => "step plan",
            (PlanFileKind::Ultra, false) => "UltraPlan",
            (PlanFileKind::Ultra, true) => "recovery UltraPlan",
        };
        let mut lines = vec![format!("Valid {label}: {}", path.display())];
        if self.recovery {
            lines.push(format!(
                "Recovery diff: failed scope {}; failure {}; retained artifacts {}",
                self.recovery_failed_scope
                    .as_deref()
                    .unwrap_or("not recorded"),
                self.recovery_failure_kind
                    .as_deref()
                    .unwrap_or("not recorded"),
                display_list(&self.recovery_retained_artifacts),
            ));
        }
        lines.push(format!("Next: {}", next_command(path, self.kind)));
        lines.join("\n")
    }
}

#[derive(Debug)]
struct PlanValidationFailure {
    diagnostics: Vec<Diagnostic>,
}

#[derive(Debug)]
struct Diagnostic {
    path: PathBuf,
    line: usize,
    column: usize,
    message: String,
}

impl fmt::Display for PlanValidationFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(formatter, "invalid plan YAML:")?;
        for (index, diagnostic) in self.diagnostics.iter().enumerate() {
            if index > 0 {
                writeln!(formatter)?;
            }
            write!(
                formatter,
                "{}:{}:{}: {}",
                diagnostic.path.display(),
                diagnostic.line,
                diagnostic.column,
                diagnostic.message
            )?;
        }
        Ok(())
    }
}

impl std::error::Error for PlanValidationFailure {}

#[derive(Debug, Deserialize)]
struct EditableStepPlan {
    goal: String,
    steps: Vec<EditablePlanStep>,
}

#[derive(Debug, Deserialize)]
struct EditablePlanStep {
    id: String,
    #[serde(default = "legacy_step_kind")]
    kind: String,
    #[serde(default = "default_expected_result")]
    expected_result: String,
    instruction: String,
    #[serde(default)]
    expected_paths: Vec<String>,
    #[serde(default)]
    verify: Vec<String>,
}

pub fn validate_plan_file(path: &Path, workspace_root: &Path) -> anyhow::Result<PlanValidation> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| anyhow::anyhow!("failed to read plan `{}`: {error}", path.display()))?;
    validate_plan_text(path, &text, workspace_root).map_err(anyhow::Error::new)
}

fn validate_plan_text(
    path: &Path,
    text: &str,
    workspace_root: &Path,
) -> Result<PlanValidation, PlanValidationFailure> {
    let value = serde_yaml::from_str::<serde_yaml::Value>(text)
        .map_err(|error| failure(path, yaml_error_location(&error), error.to_string()))?;
    let Some(mapping) = value.as_mapping() else {
        return Err(failure(
            path,
            (1, 1),
            "plan document must be a top-level YAML mapping".to_string(),
        ));
    };
    let has_steps = mapping.contains_key(serde_yaml::Value::String("steps".to_string()));
    let has_phases = mapping.contains_key(serde_yaml::Value::String("phases".to_string()));
    match (has_steps, has_phases) {
        (true, true) => Err(failure(
            path,
            locate_key(text, "phases"),
            "plan cannot contain both `steps` and `phases`".to_string(),
        )),
        (false, false) => Err(failure(
            path,
            (1, 1),
            "plan must contain either a `steps` list or a `phases` list".to_string(),
        )),
        (true, false) => validate_step_plan(path, text, workspace_root),
        (false, true) => validate_ultra_plan(path, text, mapping),
    }
}

fn validate_step_plan(
    path: &Path,
    text: &str,
    workspace_root: &Path,
) -> Result<PlanValidation, PlanValidationFailure> {
    let editable = serde_yaml::from_str::<EditableStepPlan>(text)
        .map_err(|error| failure(path, yaml_error_location(&error), error.to_string()))?;
    let plan = StepPlan {
        goal: editable.goal,
        steps: editable
            .steps
            .into_iter()
            .map(|step| PlanStep {
                id: step.id,
                kind: step.kind,
                expected_result: step.expected_result,
                instruction: step.instruction,
                expected_paths: step.expected_paths,
                verify: step.verify,
            })
            .collect(),
    };
    if plan.goal.trim().is_empty() {
        return Err(failure(
            path,
            locate_key(text, "goal"),
            "StepPlan goal must not be empty".to_string(),
        ));
    }
    if plan.steps.is_empty() {
        return Err(failure(
            path,
            locate_key(text, "steps"),
            "StepPlan must contain at least one step".to_string(),
        ));
    }
    let report = crate::planner::step_plan_finalize::validate_step_plan_contract(
        &plan,
        Some(workspace_root),
    );
    if !report.is_pass() {
        return Err(PlanValidationFailure {
            diagnostics: report
                .errors
                .iter()
                .map(|error| lint_diagnostic(path, text, error))
                .collect(),
        });
    }
    Ok(PlanValidation {
        kind: PlanFileKind::Step,
        recovery: false,
        recovery_failure_kind: None,
        recovery_failed_scope: None,
        recovery_retained_artifacts: Vec::new(),
    })
}

fn validate_ultra_plan(
    path: &Path,
    text: &str,
    mapping: &serde_yaml::Mapping,
) -> Result<PlanValidation, PlanValidationFailure> {
    let plan = serde_yaml::from_str::<UltraPlan>(text)
        .map_err(|error| failure(path, yaml_error_location(&error), error.to_string()))?;
    if plan.goal.trim().is_empty() {
        return Err(failure(
            path,
            locate_key(text, "goal"),
            "UltraPlan goal must not be empty".to_string(),
        ));
    }
    if plan.phases.is_empty() {
        return Err(failure(
            path,
            locate_key(text, "phases"),
            "UltraPlan must contain at least one phase".to_string(),
        ));
    }
    let report = lint_ultra_plan_report(&plan);
    if !report.is_pass() {
        return Err(PlanValidationFailure {
            diagnostics: report
                .errors
                .iter()
                .map(|error| lint_diagnostic(path, text, error))
                .collect(),
        });
    }
    let recovery = mapping.contains_key(serde_yaml::Value::String(
        "recovery_schema_version".to_string(),
    ));
    Ok(PlanValidation {
        kind: PlanFileKind::Ultra,
        recovery,
        recovery_failure_kind: mapping_string(mapping, "recovery_failure_kind"),
        recovery_failed_scope: mapping_string(mapping, "recovery_failed_phase")
            .or_else(|| mapping_string(mapping, "recovery_failed_step")),
        recovery_retained_artifacts: mapping_strings(
            mapping,
            "recovery_expected_completed_artifacts",
        ),
    })
}

fn failure(path: &Path, location: (usize, usize), message: String) -> PlanValidationFailure {
    PlanValidationFailure {
        diagnostics: vec![Diagnostic {
            path: path.to_path_buf(),
            line: location.0,
            column: location.1,
            message,
        }],
    }
}

fn yaml_error_location(error: &serde_yaml::Error) -> (usize, usize) {
    error
        .location()
        .map(|location| (location.line(), location.column()))
        .unwrap_or((1, 1))
}

fn lint_diagnostic(path: &Path, text: &str, error: &PlanLintError) -> Diagnostic {
    let location = error
        .verify_rejection
        .as_ref()
        .and_then(|rejection| locate_value(text, &rejection.original_command))
        .or_else(|| locate_message_subject(text, &error.message))
        .unwrap_or((1, 1));
    Diagnostic {
        path: path.to_path_buf(),
        line: location.0,
        column: location.1,
        message: format!("[{}] {}", error.category, error.message),
    }
}

fn locate_message_subject(text: &str, message: &str) -> Option<(usize, usize)> {
    text.lines().enumerate().find_map(|(index, line)| {
        let trimmed = line.trim();
        let value = trimmed
            .strip_prefix("- id:")
            .or_else(|| trimmed.strip_prefix("id:"))?
            .trim()
            .trim_matches(['\'', '"']);
        message
            .contains(value)
            .then(|| (index + 1, line.len() - line.trim_start().len() + 1))
    })
}

fn locate_key(text: &str, key: &str) -> (usize, usize) {
    text.lines()
        .enumerate()
        .find_map(|(index, line)| {
            let indentation = line.len() - line.trim_start().len();
            line.trim_start()
                .starts_with(&format!("{key}:"))
                .then_some((index + 1, indentation + 1))
        })
        .unwrap_or((1, 1))
}

fn locate_value(text: &str, value: &str) -> Option<(usize, usize)> {
    text.lines().enumerate().find_map(|(index, line)| {
        line.find(value)
            .map(|column| (index + 1, line[..column].chars().count() + 1))
    })
}

fn mapping_string(mapping: &serde_yaml::Mapping, key: &str) -> Option<String> {
    mapping
        .get(serde_yaml::Value::String(key.to_string()))
        .and_then(serde_yaml::Value::as_str)
        .map(str::to_string)
}

fn mapping_strings(mapping: &serde_yaml::Mapping, key: &str) -> Vec<String> {
    mapping
        .get(serde_yaml::Value::String(key.to_string()))
        .and_then(serde_yaml::Value::as_sequence)
        .into_iter()
        .flatten()
        .filter_map(serde_yaml::Value::as_str)
        .map(str::to_string)
        .collect()
}

fn legacy_step_kind() -> String {
    "work".to_string()
}

fn default_expected_result() -> String {
    "pass".to_string()
}

fn display_list(values: &[String]) -> String {
    if values.is_empty() {
        "none recorded".to_string()
    } else {
        values.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn syntax_error_keeps_yaml_line_and_column() {
        let error = validate_plan_text(
            Path::new("broken.yaml"),
            "goal: x\nsteps:\n  - id: bad\n    instruction: [\n",
            Path::new("."),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("broken.yaml:5:"), "{error}");
    }

    #[test]
    fn lint_error_points_to_the_rejected_command() {
        let text = "goal: test\nsteps:\n  - id: verify\n    kind: verify\n    expected_result: pass\n    instruction: Run tests.\n    verify:\n      - \"cargo test | cargo clippy\"\n";
        let error = validate_plan_text(Path::new("plan.yaml"), text, Path::new("."))
            .unwrap_err()
            .to_string();
        assert!(error.contains("plan.yaml:8:10:"), "{error}");
        assert!(error.contains("verify_policy"), "{error}");
    }

    #[test]
    fn recovery_success_describes_diff_and_next_command() {
        let text = "recovery_schema_version: '1'\nrecovery_failure_kind: build_failed\nrecovery_failed_phase: verify\nrecovery_expected_completed_artifacts:\n  - src/lib.rs\ngoal: recover\nprofile: generic\nstyle: recovery\nintent: recover\nphases:\n  - id: repair\n    prompt: Repair the implementation.\n  - id: verify\n    prompt: Verify the repaired implementation.\n";
        let validation =
            validate_plan_text(Path::new("recovery.yaml"), text, Path::new(".")).unwrap();
        let output = validation.render_success(Path::new("recovery.yaml"));
        assert!(output.contains("Valid recovery UltraPlan"), "{output}");
        assert!(output.contains("failed scope verify"), "{output}");
        assert!(output.contains("retained artifacts src/lib.rs"), "{output}");
        assert!(
            output.contains("--run-ultra-plan recovery.yaml"),
            "{output}"
        );
    }
}
