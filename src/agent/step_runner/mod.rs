pub mod plan_lint;
pub mod repair;
pub mod ultra_plan;
pub mod verify;

use crate::util::workspace_paths::plans_dir;
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepPlan {
    pub goal: String,
    pub profile: String,
    pub style: String,
    pub steps: Vec<StepPlanStep>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepPlanStep {
    pub id: String,
    pub instruction: String,
    pub expected_paths: Vec<String>,
    pub verify: Vec<String>,
}

pub fn plan_generation_prompt(goal: &str, profile: &str, style: &str) -> String {
    format!(
        "Create a small step plan for CommandAgent.\n\
Return only YAML in this schema:\n\
goal: <string>\n\
profile: <string>\n\
style: <string>\n\
steps:\n\
  - id: <short-slug>\n\
    instruction: <concrete action for one minimal-loop turn>\n\
    expected_paths:\n\
      - <repository-relative file path>\n\
    verify:\n\
      - <local verification command>\n\
\n\
Rules:\n\
- Keep steps small and executable.\n\
- Do not mix setup and final verification in the same step.\n\
- expected_paths must be actual file paths, not package names or concepts.\n\
- If no file path is expected for a step, use an empty list.\n\
\n\
Goal: {goal}\n\
Profile: {profile}\n\
Style: {style}"
    )
}

pub fn invalid_plan_correction_prompt(
    original_goal: &str,
    invalid_plan: &str,
    error: &PlanError,
) -> String {
    format!(
        "The generated step plan is invalid and must be corrected.\n\
Original goal:\n{original_goal}\n\n\
Validation error:\n{error}\n\n\
Invalid plan:\n{invalid_plan}\n\n\
Return only corrected YAML using the required CommandAgent step plan schema."
    )
}

pub fn extract_plan_from_response(response: &str) -> Result<StepPlan, PlanError> {
    let yaml = strip_yaml_fence(response.trim());
    parse_step_plan_yaml(yaml)
}

pub fn render_step_plan_yaml(plan: &StepPlan) -> String {
    let mut out = String::new();
    out.push_str(&format!("goal: {}\n", yaml_string(&plan.goal)));
    out.push_str(&format!("profile: {}\n", yaml_string(&plan.profile)));
    out.push_str(&format!("style: {}\n", yaml_string(&plan.style)));
    out.push_str("steps:\n");
    for step in &plan.steps {
        out.push_str(&format!("  - id: {}\n", yaml_string(&step.id)));
        out.push_str(&format!(
            "    instruction: {}\n",
            yaml_string(&step.instruction)
        ));
        out.push_str("    expected_paths:\n");
        for path in &step.expected_paths {
            out.push_str(&format!("      - {}\n", yaml_string(path)));
        }
        out.push_str("    verify:\n");
        for command in &step.verify {
            out.push_str(&format!("      - {}\n", yaml_string(command)));
        }
    }
    out
}

pub fn parse_step_plan_yaml(yaml: &str) -> Result<StepPlan, PlanError> {
    let mut goal = None;
    let mut profile = None;
    let mut style = None;
    let mut steps = Vec::new();
    let mut current_step: Option<StepPlanStep> = None;
    let mut current_list = None;

    for raw in yaml.lines() {
        let line = raw.trim_end();
        if line.trim().is_empty() {
            continue;
        }
        if let Some(value) = line.strip_prefix("goal:") {
            goal = Some(parse_yaml_string(value.trim())?);
            current_list = None;
        } else if let Some(value) = line.strip_prefix("profile:") {
            profile = Some(parse_yaml_string(value.trim())?);
            current_list = None;
        } else if let Some(value) = line.strip_prefix("style:") {
            style = Some(parse_yaml_string(value.trim())?);
            current_list = None;
        } else if line == "steps:" {
            current_list = None;
        } else if let Some(value) = line.strip_prefix("  - id:") {
            if let Some(step) = current_step.take() {
                steps.push(step);
            }
            current_step = Some(StepPlanStep {
                id: parse_yaml_string(value.trim())?,
                instruction: String::new(),
                expected_paths: Vec::new(),
                verify: Vec::new(),
            });
            current_list = None;
        } else if let Some(value) = line.strip_prefix("    instruction:") {
            let Some(step) = current_step.as_mut() else {
                return Err(PlanError::InvalidYaml(
                    "instruction appears before step id".to_string(),
                ));
            };
            step.instruction = parse_yaml_string(value.trim())?;
            current_list = None;
        } else if line == "    expected_paths:" {
            current_list = Some(ListField::ExpectedPaths);
        } else if line == "    verify:" {
            current_list = Some(ListField::Verify);
        } else if let Some(value) = line.strip_prefix("      - ") {
            let Some(step) = current_step.as_mut() else {
                return Err(PlanError::InvalidYaml(
                    "list item appears before step id".to_string(),
                ));
            };
            match current_list {
                Some(ListField::ExpectedPaths) => {
                    step.expected_paths.push(parse_yaml_string(value.trim())?)
                }
                Some(ListField::Verify) => step.verify.push(parse_yaml_string(value.trim())?),
                None => {
                    return Err(PlanError::InvalidYaml(
                        "list item appears outside a known list".to_string(),
                    ));
                }
            }
        } else {
            return Err(PlanError::InvalidYaml(format!("unexpected line: {line}")));
        }
    }

    if let Some(step) = current_step.take() {
        steps.push(step);
    }

    let plan = StepPlan {
        goal: goal.ok_or_else(|| PlanError::MissingField("goal".to_string()))?,
        profile: profile.ok_or_else(|| PlanError::MissingField("profile".to_string()))?,
        style: style.unwrap_or_else(|| "default".to_string()),
        steps,
    };
    validate_step_plan(&plan)?;
    Ok(plan)
}

pub fn validate_step_plan(plan: &StepPlan) -> Result<(), PlanError> {
    if plan.goal.trim().is_empty() {
        return Err(PlanError::EmptyField("goal".to_string()));
    }
    if plan.profile.trim().is_empty() {
        return Err(PlanError::EmptyField("profile".to_string()));
    }
    if plan.style.trim().is_empty() {
        return Err(PlanError::EmptyField("style".to_string()));
    }
    if plan.steps.is_empty() {
        return Err(PlanError::NoSteps);
    }

    let mut ids = HashSet::new();
    for step in &plan.steps {
        if step.id.trim().is_empty() {
            return Err(PlanError::EmptyField("step.id".to_string()));
        }
        if !step
            .id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        {
            return Err(PlanError::InvalidStepId(step.id.clone()));
        }
        if !ids.insert(step.id.clone()) {
            return Err(PlanError::DuplicateStepId(step.id.clone()));
        }
        if step.instruction.trim().is_empty() {
            return Err(PlanError::EmptyField(format!(
                "step.{}.instruction",
                step.id
            )));
        }
    }
    Ok(())
}

pub fn save_step_plan(cwd: impl AsRef<Path>, plan: &StepPlan) -> Result<PathBuf, PlanError> {
    validate_step_plan(plan)?;
    let dir = plans_dir(cwd.as_ref());
    fs::create_dir_all(&dir).map_err(|err| PlanError::Io {
        path: dir.clone(),
        message: err.to_string(),
    })?;
    let path = dir.join(format!(
        "plan-{}-{}.yaml",
        now_ms(),
        slug(&plan.goal).unwrap_or_else(|| "step-plan".to_string())
    ));
    fs::write(&path, render_step_plan_yaml(plan)).map_err(|err| PlanError::Io {
        path: path.clone(),
        message: err.to_string(),
    })?;
    Ok(path)
}

fn strip_yaml_fence(text: &str) -> &str {
    let text = text.trim();
    if let Some(rest) = text.strip_prefix("```yaml") {
        return rest.trim().strip_suffix("```").unwrap_or(rest).trim();
    }
    if let Some(rest) = text.strip_prefix("```") {
        return rest.trim().strip_suffix("```").unwrap_or(rest).trim();
    }
    text
}

fn yaml_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

fn parse_yaml_string(value: &str) -> Result<String, PlanError> {
    if value.starts_with('"') {
        serde_json::from_str::<Value>(value)
            .ok()
            .and_then(|value| value.as_str().map(ToString::to_string))
            .ok_or_else(|| PlanError::InvalidYaml(format!("invalid string: {value}")))
    } else {
        Ok(value.to_string())
    }
}

fn slug(value: &str) -> Option<String> {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if (ch.is_whitespace() || ch == '-' || ch == '_') && !out.ends_with('-') {
            out.push('-');
        }
        if out.len() >= 48 {
            break;
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() { None } else { Some(out) }
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ListField {
    ExpectedPaths,
    Verify,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    MissingField(String),
    EmptyField(String),
    NoSteps,
    InvalidStepId(String),
    DuplicateStepId(String),
    InvalidYaml(String),
    Io { path: PathBuf, message: String },
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingField(field) => write!(f, "missing required field: {field}"),
            Self::EmptyField(field) => write!(f, "field must not be empty: {field}"),
            Self::NoSteps => write!(f, "step plan must contain at least one step"),
            Self::InvalidStepId(id) => write!(f, "invalid step id: {id}"),
            Self::DuplicateStepId(id) => write!(f, "duplicate step id: {id}"),
            Self::InvalidYaml(message) => write!(f, "invalid plan YAML: {message}"),
            Self::Io { path, message } => write!(f, "{}: {}", path.display(), message),
        }
    }
}

impl std::error::Error for PlanError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_and_loads_step_plan_yaml() {
        let plan = sample_plan();

        let yaml = render_step_plan_yaml(&plan);
        let parsed = parse_step_plan_yaml(&yaml).unwrap();

        assert_eq!(parsed, plan);
    }

    #[test]
    fn extracts_plan_from_yaml_code_fence() {
        let yaml = render_step_plan_yaml(&sample_plan());
        let response = format!("```yaml\n{yaml}```");

        let parsed = extract_plan_from_response(&response).unwrap();

        assert_eq!(parsed.goal, "Build docs");
        assert_eq!(parsed.steps.len(), 1);
    }

    #[test]
    fn validates_duplicate_step_ids() {
        let mut plan = sample_plan();
        plan.steps.push(plan.steps[0].clone());

        let err = validate_step_plan(&plan).unwrap_err();

        assert_eq!(err, PlanError::DuplicateStepId("write-readme".to_string()));
    }

    #[test]
    fn saves_plan_under_commandagent_plans() {
        let root = temp_workspace("save");
        let plan = sample_plan();

        let path = save_step_plan(&root, &plan).unwrap();

        assert!(path.starts_with(root.join(".commandagent/plans")));
        assert!(path.exists());
        let loaded = parse_step_plan_yaml(&fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(loaded, plan);
    }

    #[test]
    fn generation_prompt_demands_yaml_only() {
        let prompt = plan_generation_prompt("Build docs", "docs", "default");

        assert!(prompt.contains("Return only YAML"));
        assert!(prompt.contains("Goal: Build docs"));
        assert!(prompt.contains("Profile: docs"));
    }

    #[test]
    fn correction_prompt_contains_error_and_invalid_plan() {
        let err = PlanError::NoSteps;
        let prompt = invalid_plan_correction_prompt("goal", "goal: x", &err);

        assert!(prompt.contains("Validation error"));
        assert!(prompt.contains("step plan must contain at least one step"));
        assert!(prompt.contains("goal: x"));
    }

    fn sample_plan() -> StepPlan {
        StepPlan {
            goal: "Build docs".to_string(),
            profile: "docs".to_string(),
            style: "default".to_string(),
            steps: vec![StepPlanStep {
                id: "write-readme".to_string(),
                instruction: "Create README.md with usage notes.".to_string(),
                expected_paths: vec!["README.md".to_string()],
                verify: vec!["cat README.md".to_string()],
            }],
        }
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "commandagent-step-plan-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
}
