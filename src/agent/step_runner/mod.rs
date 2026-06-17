pub mod plan_lint;
pub mod profiles;
pub mod repair;
pub mod runtime;
pub mod ultra_plan;
pub mod ultra_run;
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
    pub intent: WorkIntent,
    pub required_artifacts: Vec<String>,
    pub steps: Vec<StepPlanStep>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepPlanStep {
    pub id: String,
    pub kind: StepKind,
    pub instruction: String,
    pub expected_result: ExpectedResult,
    pub expected_paths: Vec<String>,
    pub verify: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkIntent {
    New,
    Modify,
    Investigate,
    Document,
    Data,
    Unknown,
}

impl WorkIntent {
    pub fn parse(value: &str) -> Result<Self, PlanError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "new" | "create" => Ok(Self::New),
            "modify" | "fix" | "enhance" | "refactor" => Ok(Self::Modify),
            "investigate" | "triage" | "debug" => Ok(Self::Investigate),
            "document" | "docs" => Ok(Self::Document),
            "data" | "data-analysis" | "data-pipeline" => Ok(Self::Data),
            "unknown" | "" => Ok(Self::Unknown),
            other => Err(PlanError::InvalidEnum {
                field: "intent".to_string(),
                value: other.to_string(),
            }),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Modify => "modify",
            Self::Investigate => "investigate",
            Self::Document => "document",
            Self::Data => "data",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    Inspect,
    Create,
    Edit,
    Setup,
    Verify,
    Repair,
    Report,
}

impl StepKind {
    pub fn parse(value: &str) -> Result<Self, PlanError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "inspect" | "read" | "analyze" | "analyse" => Ok(Self::Inspect),
            "create" => Ok(Self::Create),
            "edit" | "modify" | "update" => Ok(Self::Edit),
            "setup" | "install" | "configure" => Ok(Self::Setup),
            "verify" | "check" | "test" | "shell" | "command" | "run" => Ok(Self::Verify),
            "repair" | "fix" => Ok(Self::Repair),
            "report" | "summarize" | "summarise" => Ok(Self::Report),
            other => Err(PlanError::InvalidEnum {
                field: "step.kind".to_string(),
                value: other.to_string(),
            }),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Inspect => "inspect",
            Self::Create => "create",
            Self::Edit => "edit",
            Self::Setup => "setup",
            Self::Verify => "verify",
            Self::Repair => "repair",
            Self::Report => "report",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpectedResult {
    Pass,
    Fail,
    Unavailable,
}

impl ExpectedResult {
    pub fn parse(value: &str) -> Result<Self, PlanError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "pass" => Ok(Self::Pass),
            "fail" => Ok(Self::Fail),
            "unavailable" => Ok(Self::Unavailable),
            other => Err(PlanError::InvalidEnum {
                field: "step.expected_result".to_string(),
                value: other.to_string(),
            }),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Unavailable => "unavailable",
        }
    }
}

pub fn detect_work_intent(goal: &str) -> WorkIntent {
    let lower = goal.to_ascii_lowercase();
    if contains_any(&lower, &["investigate", "triage", "debug", "原因", "調査"]) {
        WorkIntent::Investigate
    } else if contains_any(&lower, &["document", "docs", "readme", "ドキュメント"]) {
        WorkIntent::Document
    } else if contains_any(
        &lower,
        &["fix", "modify", "update", "repair", "修正", "改修"],
    ) {
        WorkIntent::Modify
    } else if contains_any(&lower, &["data", "csv", "report", "分析", "整形"]) {
        WorkIntent::Data
    } else if contains_any(
        &lower,
        &["create", "build", "implement", "new", "作成", "開発"],
    ) {
        WorkIntent::New
    } else {
        WorkIntent::Unknown
    }
}

pub fn plan_generation_prompt(
    goal: &str,
    profile: &str,
    style: &str,
    intent: WorkIntent,
    required_artifacts: &[String],
) -> String {
    format!(
        "Create a small step plan for CommandAgent.\n\
Return only YAML in this schema:\n\
goal: <string>\n\
profile: <string>\n\
style: <string>\n\
intent: <new|modify|investigate|document|data|unknown>\n\
required_artifacts:\n\
  - <repository-relative final artifact path>\n\
steps:\n\
  - id: <short-slug>\n\
    kind: <inspect|create|edit|setup|verify|repair|report>\n\
    instruction: <concrete action for one minimal-loop turn>\n\
    expected_result: <pass|fail|unavailable>\n\
    expected_paths:\n\
      - <repository-relative file path>\n\
    verify:\n\
      - <local verification command>\n\
\n\
Rules:\n\
- Keep steps small and executable.\n\
- Use only canonical kind values in output: inspect, create, edit, setup, verify, repair, report.\n\
- Use kind inspect instead of read/analyze, and kind verify instead of shell/run.\n\
- Do not mix setup and final verification in the same step.\n\
- File creation or modification steps must be executable with Write/Edit, not shell scaffolding.\n\
- Do not create directory-only steps; Write creates parent directories automatically.\n\
- Do not plan dependency installation as a required success step; dependency installs may be unavailable offline.\n\
- expected_paths must be actual file paths, not package names, concepts, directories, or dependency caches.\n\
- Verifier commands must be one simple local check each; split shell chaining into separate list items and avoid &&, ||, or ;.\n\
- If no file path is expected for a step, use an empty list.\n\
- required_artifacts are final user-requested outputs and must be preserved exactly.\n\
- setup prepares local dependencies or configuration; verify runs deterministic checks and must not change files.\n\
- report steps explicitly report blockers such as dependency_missing or verifier_unavailable.\n\
- Do not include tool-call fields such as action, path, content, old, or new in the plan.\n\
\n\
Goal: {goal}\n\
Profile: {profile}\n\
Style: {style}\n\
Intent: {intent}\n\
Required final artifacts:\n{artifacts}\n\
Profile guidance:\n{profile_guidance}",
        intent = intent.as_str(),
        artifacts = bullet_list(required_artifacts),
        profile_guidance = plan_profile_guidance(profile)
    )
}

fn plan_profile_guidance(profile: &str) -> &'static str {
    match profile {
        "rust" => {
            "For new Rust projects, plan explicit file creation for Cargo.toml and src/main.rs. Do not plan cargo init or cargo new shell scaffolding."
        }
        _ => "No additional profile-specific plan guidance.",
    }
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
If the invalid plan includes tool-call fields such as action, path, content, old, or new, rewrite them into instruction and expected_paths fields.\n\
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
    out.push_str(&format!("intent: {}\n", yaml_string(plan.intent.as_str())));
    out.push_str("required_artifacts:\n");
    for path in &plan.required_artifacts {
        out.push_str(&format!("  - {}\n", yaml_string(path)));
    }
    out.push_str("steps:\n");
    for step in &plan.steps {
        out.push_str(&format!("  - id: {}\n", yaml_string(&step.id)));
        out.push_str(&format!("    kind: {}\n", yaml_string(step.kind.as_str())));
        out.push_str(&format!(
            "    instruction: {}\n",
            yaml_string(&step.instruction)
        ));
        out.push_str(&format!(
            "    expected_result: {}\n",
            yaml_string(step.expected_result.as_str())
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
    let mut intent = None;
    let mut required_artifacts = Vec::new();
    let mut steps = Vec::new();
    let mut current_step: Option<(StepPlanStep, bool)> = None;
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
        } else if let Some(value) = line.strip_prefix("intent:") {
            intent = Some(WorkIntent::parse(&parse_yaml_string(value.trim())?)?);
            current_list = None;
        } else if line == "required_artifacts:" {
            current_list = Some(ListField::RequiredArtifacts);
        } else if let Some(value) = line.strip_prefix("required_artifacts:") {
            if parse_inline_empty_list(value.trim())? {
                current_list = None;
            }
        } else if line == "steps:" {
            current_list = None;
        } else if let Some(value) = step_id_value(line) {
            if let Some((step, kind_explicit)) = current_step.take() {
                steps.push(finalize_step(step, kind_explicit));
            }
            current_step = Some((
                StepPlanStep {
                    id: parse_yaml_string(value.trim())?,
                    kind: StepKind::Inspect,
                    instruction: String::new(),
                    expected_result: ExpectedResult::Pass,
                    expected_paths: Vec::new(),
                    verify: Vec::new(),
                },
                false,
            ));
            current_list = None;
        } else if let Some(value) = step_field_value(line, "kind") {
            let Some((step, kind_explicit)) = current_step.as_mut() else {
                return Err(PlanError::InvalidYaml(
                    "kind appears before step id".to_string(),
                ));
            };
            step.kind = StepKind::parse(&parse_yaml_string(value.trim())?)?;
            *kind_explicit = true;
            current_list = None;
        } else if let Some(value) = step_field_value(line, "instruction") {
            let Some((step, _kind_explicit)) = current_step.as_mut() else {
                return Err(PlanError::InvalidYaml(
                    "instruction appears before step id".to_string(),
                ));
            };
            step.instruction = parse_yaml_string(value.trim())?;
            current_list = None;
        } else if let Some(value) = step_field_value(line, "expected_result") {
            let Some((step, _kind_explicit)) = current_step.as_mut() else {
                return Err(PlanError::InvalidYaml(
                    "expected_result appears before step id".to_string(),
                ));
            };
            step.expected_result = ExpectedResult::parse(&parse_yaml_string(value.trim())?)?;
            current_list = None;
        } else if is_step_field(line, "expected_paths") {
            current_list = Some(ListField::ExpectedPaths);
        } else if let Some(value) = step_field_value(line, "expected_paths") {
            if parse_inline_empty_list(value.trim())? {
                let Some((_step, _kind_explicit)) = current_step.as_mut() else {
                    return Err(PlanError::InvalidYaml(
                        "expected_paths appears before step id".to_string(),
                    ));
                };
                current_list = None;
            }
        } else if is_step_field(line, "verify") {
            current_list = Some(ListField::Verify);
        } else if let Some(value) = step_field_value(line, "verify") {
            if parse_inline_empty_list(value.trim())? {
                let Some((_step, _kind_explicit)) = current_step.as_mut() else {
                    return Err(PlanError::InvalidYaml(
                        "verify appears before step id".to_string(),
                    ));
                };
                current_list = None;
            }
        } else if let Some(value) = list_item_value(line) {
            if matches!(current_list, Some(ListField::RequiredArtifacts)) {
                required_artifacts.push(parse_yaml_string(value.trim())?);
                continue;
            }
            let Some((step, _kind_explicit)) = current_step.as_mut() else {
                return Err(PlanError::InvalidYaml(
                    "list item appears before step id".to_string(),
                ));
            };
            match current_list {
                Some(ListField::RequiredArtifacts) => unreachable!("handled before step list item"),
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
        } else if is_ignored_step_annotation(line) {
            let Some((_step, _kind_explicit)) = current_step.as_mut() else {
                return Err(PlanError::InvalidYaml(
                    "step annotation appears before step id".to_string(),
                ));
            };
            current_list = None;
        } else {
            return Err(PlanError::InvalidYaml(format!("unexpected line: {line}")));
        }
    }

    if let Some((step, kind_explicit)) = current_step.take() {
        steps.push(finalize_step(step, kind_explicit));
    }

    let plan = StepPlan {
        goal: goal.ok_or_else(|| PlanError::MissingField("goal".to_string()))?,
        profile: profile.ok_or_else(|| PlanError::MissingField("profile".to_string()))?,
        style: style.unwrap_or_else(|| "default".to_string()),
        intent: intent.unwrap_or(WorkIntent::Unknown),
        required_artifacts: dedupe_required_artifacts(required_artifacts),
        steps,
    };
    validate_step_plan(&plan)?;
    Ok(plan)
}

fn dedupe_required_artifacts(paths: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for path in paths {
        if seen.insert(path.clone()) {
            out.push(path);
        }
    }
    out
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

fn step_id_value(line: &str) -> Option<&str> {
    line.strip_prefix("  - id:")
        .or_else(|| line.strip_prefix("- id:"))
}

fn step_field_value<'a>(line: &'a str, field: &str) -> Option<&'a str> {
    line.strip_prefix(&format!("    {field}:"))
        .or_else(|| line.strip_prefix(&format!("  {field}:")))
        .or_else(|| line.strip_prefix(&format!("{field}:")))
}

fn is_step_field(line: &str, field: &str) -> bool {
    line == format!("    {field}:") || line == format!("  {field}:") || line == format!("{field}:")
}

fn list_item_value(line: &str) -> Option<&str> {
    line.trim_start().strip_prefix("- ")
}

fn is_ignored_step_annotation(line: &str) -> bool {
    matches!(
        line.trim_start().split_once(':').map(|(key, _)| key),
        Some("action")
    ) && (line.starts_with("  ") || line.starts_with("    "))
}

fn parse_inline_empty_list(value: &str) -> Result<bool, PlanError> {
    if value == "[]" {
        Ok(true)
    } else {
        Err(PlanError::InvalidYaml(format!(
            "unsupported inline list value: {value}"
        )))
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
    RequiredArtifacts,
    ExpectedPaths,
    Verify,
}

fn finalize_step(mut step: StepPlanStep, kind_explicit: bool) -> StepPlanStep {
    if !kind_explicit {
        step.kind = infer_step_kind(&step.instruction, &step.expected_paths, &step.verify);
    }
    step
}

fn infer_step_kind(instruction: &str, expected_paths: &[String], verify: &[String]) -> StepKind {
    let lower = instruction.to_ascii_lowercase();
    if contains_any(&lower, &["verify", "validate", "test", "build", "check"]) || !verify.is_empty()
    {
        StepKind::Verify
    } else if contains_any(&lower, &["install", "configure", "setup"]) {
        StepKind::Setup
    } else if contains_any(&lower, &["fix", "repair"]) {
        StepKind::Repair
    } else if contains_any(&lower, &["report", "summarize"]) {
        StepKind::Report
    } else if contains_any(&lower, &["edit", "update", "modify"]) {
        StepKind::Edit
    } else if !expected_paths.is_empty()
        || contains_any(&lower, &["create", "write", "add", "implement"])
    {
        StepKind::Create
    } else {
        StepKind::Inspect
    }
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn bullet_list(values: &[String]) -> String {
    if values.is_empty() {
        "- none".to_string()
    } else {
        values
            .iter()
            .map(|value| format!("- {value}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    MissingField(String),
    EmptyField(String),
    NoSteps,
    InvalidStepId(String),
    DuplicateStepId(String),
    InvalidYaml(String),
    InvalidEnum { field: String, value: String },
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
            Self::InvalidEnum { field, value } => write!(f, "invalid {field}: {value}"),
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
    fn accepts_inline_empty_step_lists() {
        let yaml = "goal: \"Run check\"\nprofile: \"python\"\nstyle: \"default\"\nsteps:\n  - id: \"inspect\"\n    instruction: \"Inspect workspace.\"\n    expected_paths: []\n    verify: []\n";

        let plan = parse_step_plan_yaml(yaml).unwrap();

        assert!(plan.steps[0].expected_paths.is_empty());
        assert!(plan.steps[0].verify.is_empty());
    }

    #[test]
    fn accepts_common_model_step_indentation_drift() {
        let yaml = "goal: \"Create Rust CLI\"\nprofile: \"rust\"\nstyle: \"default\"\nsteps:\n- id: create-cargo-toml\n  instruction: \"Create Cargo.toml.\"\n  expected_paths:\n    - Cargo.toml\n  verify:\n    - test -f Cargo.toml\n";

        let plan = parse_step_plan_yaml(yaml).unwrap();

        assert_eq!(plan.steps[0].id, "create-cargo-toml");
        assert_eq!(plan.steps[0].expected_paths, vec!["Cargo.toml"]);
        assert_eq!(plan.steps[0].verify, vec!["test -f Cargo.toml"]);
    }

    #[test]
    fn accepts_unindented_step_fields() {
        let yaml = "goal: \"Create schemas\"\nprofile: \"python\"\nstyle: \"default\"\nintent: modify\nrequired_artifacts:\n- app/main.py\nsteps:\n- id: create-schemas\nkind: create\ninstruction: Create app/schemas.py.\nexpected_result: pass\nexpected_paths:\n- app/schemas.py\nverify:\n- test -f app/schemas.py\n";

        let plan = parse_step_plan_yaml(yaml).unwrap();

        assert_eq!(plan.required_artifacts, vec!["app/main.py"]);
        assert_eq!(plan.steps[0].kind, StepKind::Create);
        assert_eq!(plan.steps[0].expected_paths, vec!["app/schemas.py"]);
    }

    #[test]
    fn accepts_common_step_kind_aliases_but_renders_canonical_values() {
        let yaml = "goal: \"Inspect and verify\"\nprofile: \"rust\"\nstyle: \"default\"\nsteps:\n- id: inspect-source\nkind: read\ninstruction: Read src/main.rs.\nexpected_paths:\n- src/main.rs\nverify:\n- test -f src/main.rs\n- id: analyze-source\nkind: analyze\ninstruction: Analyze source layout.\nexpected_paths: []\nverify: []\n- id: run-tests\nkind: shell\ninstruction: Run cargo test.\nexpected_paths: []\nverify:\n- cargo test\n";

        let plan = parse_step_plan_yaml(yaml).unwrap();
        let rendered = render_step_plan_yaml(&plan);

        assert_eq!(plan.steps[0].kind, StepKind::Inspect);
        assert_eq!(plan.steps[1].kind, StepKind::Inspect);
        assert_eq!(plan.steps[2].kind, StepKind::Verify);
        assert!(rendered.contains("kind: \"inspect\""));
        assert!(rendered.contains("kind: \"verify\""));
        assert!(!rendered.contains("kind: \"shell\""));
    }

    #[test]
    fn accepts_common_model_list_item_indentation_drift() {
        let yaml = "goal: \"Create Next app\"\nprofile: \"nextjs\"\nstyle: \"default\"\nsteps:\n  - id: create-files\n    instruction: \"Create Next.js files.\"\n    expected_paths:\n  - package.json\n  - app/page.tsx\n    verify:\n  - cat package.json\n";

        let plan = parse_step_plan_yaml(yaml).unwrap();

        assert_eq!(
            plan.steps[0].expected_paths,
            vec!["package.json", "app/page.tsx"]
        );
        assert_eq!(plan.steps[0].verify, vec!["cat package.json"]);
    }

    #[test]
    fn accepts_arbitrary_list_item_indentation_drift() {
        let yaml = "goal: \"Create Python app\"\nprofile: \"python\"\nstyle: \"default\"\nsteps:\n- id: create-init\n  instruction: Create init files.\n  expected_paths:\n     - app/__init__.py\n     - tests/__init__.py\n  verify:\n     - test -f app/__init__.py\n";

        let plan = parse_step_plan_yaml(yaml).unwrap();

        assert_eq!(
            plan.steps[0].expected_paths,
            vec!["app/__init__.py", "tests/__init__.py"]
        );
    }

    #[test]
    fn accepts_three_space_list_item_indentation_drift() {
        let yaml = r#"
goal: Build app
profile: nextjs
style: default
steps:
  - id: create-files
    instruction: Create files.
    expected_paths:
   - package.json
   - app/page.tsx
    verify: []
"#;

        let plan = parse_step_plan_yaml(yaml).unwrap();

        assert_eq!(
            plan.steps[0].expected_paths,
            vec!["package.json", "app/page.tsx"]
        );
    }

    #[test]
    fn ignores_common_model_action_annotation() {
        let yaml = "goal: \"Create docs\"\nprofile: \"docs\"\nstyle: \"default\"\nsteps:\n  - id: create-readme\n    action: write\n    instruction: \"Create README.md.\"\n    expected_paths:\n      - README.md\n    verify:\n      - cat README.md\n";

        let plan = parse_step_plan_yaml(yaml).unwrap();

        assert_eq!(plan.steps[0].id, "create-readme");
        assert_eq!(plan.steps[0].instruction, "Create README.md.");
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
        let prompt =
            plan_generation_prompt("Build docs", "docs", "default", WorkIntent::Document, &[]);

        assert!(prompt.contains("Return only YAML"));
        assert!(prompt.contains("Goal: Build docs"));
        assert!(prompt.contains("Profile: docs"));
        assert!(prompt.contains("Intent: document"));
        assert!(prompt.contains("Do not include tool-call fields"));
    }

    #[test]
    fn generation_prompt_warns_rust_against_shell_scaffolding() {
        let prompt =
            plan_generation_prompt("Build Rust CLI", "rust", "default", WorkIntent::New, &[]);

        assert!(prompt.contains("Cargo.toml"));
        assert!(prompt.contains("src/main.rs"));
        assert!(prompt.contains("Do not plan cargo init or cargo new"));
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
            intent: WorkIntent::Document,
            required_artifacts: vec!["README.md".to_string()],
            steps: vec![StepPlanStep {
                id: "write-readme".to_string(),
                kind: StepKind::Create,
                instruction: "Create README.md with usage notes.".to_string(),
                expected_result: ExpectedResult::Pass,
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
