use crate::util::workspace_paths::plans_dir;
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const MAX_PHASES: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UltraPlan {
    pub goal: String,
    pub profile: String,
    pub style: String,
    pub intent: String,
    pub phases: Vec<UltraPhase>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UltraPhase {
    pub id: String,
    pub goal: String,
}

pub fn ultra_plan_generation_prompt(
    goal: &str,
    profile: &str,
    style: &str,
    intent: &str,
) -> String {
    format!(
        "Create a phased ultra plan for CommandAgent.\n\
Return only YAML in this schema:\n\
goal: <string>\n\
profile: <string>\n\
style: <string>\n\
intent: <new|modify|investigate|docs|data>\n\
phases:\n\
  - id: <short-slug>\n\
    goal: <phase goal to pass to /plan-run>\n\
\n\
Rules:\n\
- Use 2 to {max} phases unless the task is truly tiny.\n\
- Each phase must be independently useful and small enough for a step plan.\n\
- Preserve the requested profile, style, and intent.\n\
- Do not include implementation details that belong inside a step plan.\n\
\n\
Goal: {goal}\n\
Profile: {profile}\n\
Style: {style}\n\
Intent: {intent}",
        max = MAX_PHASES
    )
}

pub fn render_ultra_plan_yaml(plan: &UltraPlan) -> String {
    let mut out = String::new();
    out.push_str(&format!("goal: {}\n", yaml_string(&plan.goal)));
    out.push_str(&format!("profile: {}\n", yaml_string(&plan.profile)));
    out.push_str(&format!("style: {}\n", yaml_string(&plan.style)));
    out.push_str(&format!("intent: {}\n", yaml_string(&plan.intent)));
    out.push_str("phases:\n");
    for phase in &plan.phases {
        out.push_str(&format!("  - id: {}\n", yaml_string(&phase.id)));
        out.push_str(&format!("    goal: {}\n", yaml_string(&phase.goal)));
    }
    out
}

pub fn parse_ultra_plan_yaml(yaml: &str) -> Result<UltraPlan, UltraPlanError> {
    let mut goal = None;
    let mut profile = None;
    let mut style = None;
    let mut intent = None;
    let mut phases = Vec::new();
    let mut current_phase: Option<UltraPhase> = None;

    for raw in strip_yaml_fence(yaml).lines() {
        let line = raw.trim_end();
        if line.trim().is_empty() {
            continue;
        }
        if let Some(value) = line.strip_prefix("goal:") {
            goal = Some(parse_yaml_string(value.trim())?);
        } else if let Some(value) = line.strip_prefix("profile:") {
            profile = Some(parse_yaml_string(value.trim())?);
        } else if let Some(value) = line.strip_prefix("style:") {
            style = Some(parse_yaml_string(value.trim())?);
        } else if let Some(value) = line.strip_prefix("intent:") {
            intent = Some(parse_yaml_string(value.trim())?);
        } else if line == "phases:" {
        } else if let Some(value) = phase_id_value(line) {
            if let Some(phase) = current_phase.take() {
                phases.push(phase);
            }
            current_phase = Some(UltraPhase {
                id: parse_yaml_string(value.trim())?,
                goal: String::new(),
            });
        } else if let Some(value) = phase_field_value(line, "goal") {
            let Some(phase) = current_phase.as_mut() else {
                return Err(UltraPlanError::InvalidYaml(
                    "phase goal appears before phase id".to_string(),
                ));
            };
            phase.goal = parse_yaml_string(value.trim())?;
        } else if is_ignored_phase_annotation(line) {
            let Some(_phase) = current_phase.as_mut() else {
                return Err(UltraPlanError::InvalidYaml(
                    "phase annotation appears before phase id".to_string(),
                ));
            };
        } else {
            return Err(UltraPlanError::InvalidYaml(format!(
                "unexpected line: {line}"
            )));
        }
    }

    if let Some(phase) = current_phase.take() {
        phases.push(phase);
    }

    let plan = UltraPlan {
        goal: goal.ok_or_else(|| UltraPlanError::MissingField("goal".to_string()))?,
        profile: profile.ok_or_else(|| UltraPlanError::MissingField("profile".to_string()))?,
        style: style.unwrap_or_else(|| "default".to_string()),
        intent: intent.unwrap_or_else(|| "new".to_string()),
        phases,
    };
    validate_ultra_plan(&plan)?;
    Ok(plan)
}

pub fn validate_ultra_plan(plan: &UltraPlan) -> Result<(), UltraPlanError> {
    if plan.goal.trim().is_empty() {
        return Err(UltraPlanError::EmptyField("goal".to_string()));
    }
    if plan.profile.trim().is_empty() {
        return Err(UltraPlanError::EmptyField("profile".to_string()));
    }
    if plan.style.trim().is_empty() {
        return Err(UltraPlanError::EmptyField("style".to_string()));
    }
    if plan.intent.trim().is_empty() {
        return Err(UltraPlanError::EmptyField("intent".to_string()));
    }
    if plan.phases.is_empty() || plan.phases.len() > MAX_PHASES {
        return Err(UltraPlanError::InvalidPhaseCount(plan.phases.len()));
    }

    let mut ids = HashSet::new();
    for phase in &plan.phases {
        if phase.id.trim().is_empty() {
            return Err(UltraPlanError::EmptyField("phase.id".to_string()));
        }
        if !phase
            .id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        {
            return Err(UltraPlanError::InvalidPhaseId(phase.id.clone()));
        }
        if !ids.insert(phase.id.clone()) {
            return Err(UltraPlanError::DuplicatePhaseId(phase.id.clone()));
        }
        if phase.goal.trim().is_empty() {
            return Err(UltraPlanError::EmptyField(format!(
                "phase.{}.goal",
                phase.id
            )));
        }
    }
    Ok(())
}

pub fn save_ultra_plan(cwd: impl AsRef<Path>, plan: &UltraPlan) -> Result<PathBuf, UltraPlanError> {
    validate_ultra_plan(plan)?;
    let dir = plans_dir(cwd.as_ref());
    fs::create_dir_all(&dir).map_err(|err| UltraPlanError::Io {
        path: dir.clone(),
        message: err.to_string(),
    })?;
    let path = dir.join(format!(
        "ultra-plan-{}-{}.yaml",
        now_ms(),
        slug(&plan.goal).unwrap_or_else(|| "ultra-plan".to_string())
    ));
    fs::write(&path, render_ultra_plan_yaml(plan)).map_err(|err| UltraPlanError::Io {
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

fn parse_yaml_string(value: &str) -> Result<String, UltraPlanError> {
    if value.starts_with('"') {
        serde_json::from_str::<Value>(value)
            .ok()
            .and_then(|value| value.as_str().map(ToString::to_string))
            .ok_or_else(|| UltraPlanError::InvalidYaml(format!("invalid string: {value}")))
    } else {
        Ok(value.to_string())
    }
}

fn phase_id_value(line: &str) -> Option<&str> {
    line.strip_prefix("  - id:")
        .or_else(|| line.strip_prefix("- id:"))
}

fn phase_field_value<'a>(line: &'a str, field: &str) -> Option<&'a str> {
    line.strip_prefix(&format!("    {field}:"))
        .or_else(|| line.strip_prefix(&format!("  {field}:")))
}

fn is_ignored_phase_annotation(line: &str) -> bool {
    let trimmed = line.trim_start();
    matches!(
        trimmed.split_once(':').map(|(key, _)| key),
        Some("steps" | "expected_paths" | "verify" | "instruction")
    ) || trimmed.starts_with("- ")
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UltraPlanError {
    MissingField(String),
    EmptyField(String),
    InvalidPhaseCount(usize),
    InvalidPhaseId(String),
    DuplicatePhaseId(String),
    InvalidYaml(String),
    Io { path: PathBuf, message: String },
}

impl std::fmt::Display for UltraPlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingField(field) => write!(f, "missing required field: {field}"),
            Self::EmptyField(field) => write!(f, "field must not be empty: {field}"),
            Self::InvalidPhaseCount(count) => write!(
                f,
                "ultra plan must contain 1 to {} phases, got {}",
                MAX_PHASES, count
            ),
            Self::InvalidPhaseId(id) => write!(f, "invalid phase id: {id}"),
            Self::DuplicatePhaseId(id) => write!(f, "duplicate phase id: {id}"),
            Self::InvalidYaml(message) => write!(f, "invalid ultra plan YAML: {message}"),
            Self::Io { path, message } => write!(f, "{}: {}", path.display(), message),
        }
    }
}

impl std::error::Error for UltraPlanError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_and_loads_ultra_plan_yaml() {
        let plan = sample_plan();

        let parsed = parse_ultra_plan_yaml(&render_ultra_plan_yaml(&plan)).unwrap();

        assert_eq!(parsed, plan);
    }

    #[test]
    fn generation_prompt_contains_phase_contract() {
        let prompt = ultra_plan_generation_prompt("Build app", "nextjs", "tdd", "new");

        assert!(prompt.contains("Return only YAML"));
        assert!(prompt.contains("Use 2 to"));
        assert!(prompt.contains("Profile: nextjs"));
        assert!(prompt.contains("Intent: new"));
    }

    #[test]
    fn validates_phase_count() {
        let mut plan = sample_plan();
        plan.phases.clear();

        let err = validate_ultra_plan(&plan).unwrap_err();

        assert_eq!(err, UltraPlanError::InvalidPhaseCount(0));
    }

    #[test]
    fn validates_duplicate_phase_id() {
        let mut plan = sample_plan();
        plan.phases.push(plan.phases[0].clone());

        let err = validate_ultra_plan(&plan).unwrap_err();

        assert_eq!(
            err,
            UltraPlanError::DuplicatePhaseId("scaffold".to_string())
        );
    }

    #[test]
    fn accepts_common_model_phase_indentation_drift() {
        let yaml = r#"
goal: Build app
profile: nextjs
style: default
intent: new
phases:
- id: scaffold
  goal: Create files.
- id: verify
  goal: Verify build.
"#;

        let plan = parse_ultra_plan_yaml(yaml).unwrap();

        assert_eq!(plan.phases.len(), 2);
        assert_eq!(plan.phases[0].id, "scaffold");
        assert_eq!(plan.phases[1].goal, "Verify build.");
    }

    #[test]
    fn ignores_common_nested_step_annotations_in_phase_plan() {
        let yaml = r#"
goal: Build app
profile: nextjs
style: default
intent: new
phases:
  - id: scaffold
    goal: Create files.
    steps:
      - id: create-package
        instruction: Create package.json.
  - id: verify
    goal: Verify build.
"#;

        let plan = parse_ultra_plan_yaml(yaml).unwrap();

        assert_eq!(plan.phases.len(), 2);
        assert_eq!(plan.phases[0].goal, "Create files.");
    }

    #[test]
    fn saves_ultra_plan_under_plans_dir() {
        let root = temp_workspace("save");
        let plan = sample_plan();

        let path = save_ultra_plan(&root, &plan).unwrap();

        assert!(path.starts_with(root.join(".commandagent/plans")));
        assert!(path.exists());
        assert!(
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("ultra-plan-")
        );
        let loaded = parse_ultra_plan_yaml(&fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(loaded, plan);
    }

    fn sample_plan() -> UltraPlan {
        UltraPlan {
            goal: "Build app".to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            intent: "new".to_string(),
            phases: vec![
                UltraPhase {
                    id: "scaffold".to_string(),
                    goal: "Create the app skeleton.".to_string(),
                },
                UltraPhase {
                    id: "verify".to_string(),
                    goal: "Run the build and fix failures.".to_string(),
                },
            ],
        }
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "commandagent-ultra-plan-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
}
