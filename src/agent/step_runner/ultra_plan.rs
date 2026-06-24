use super::yaml_scalar::parse_block_scalar_value;
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
    pub required_artifacts: Vec<String>,
    pub phases: Vec<UltraPhase>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UltraPhase {
    pub id: String,
    pub goal: String,
    pub owned_artifacts: Vec<String>,
    pub preserve_artifacts: Vec<String>,
    pub verify_only_artifacts: Vec<String>,
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
intent: <new|modify|investigate|document|data|unknown>\n\
required_artifacts:\n\
  - <repository-relative final artifact path>\n\
phases:\n\
  - id: <short-slug>\n\
    goal: <phase goal to pass to /plan-run>\n\
    owned_artifacts:\n\
      - <artifact this phase may create or mutate>\n\
    preserve_artifacts:\n\
      - <artifact this phase may inspect but must not mutate>\n\
    verify_only_artifacts:\n\
      - <artifact this phase verifies but does not create or mutate>\n\
\n\
Rules:\n\
- Use 2 to {max} phases unless the task is truly tiny.\n\
- Each phase must be independently useful and small enough for a step plan.\n\
- Preserve the requested profile, style, and intent.\n\
- Preserve required_artifacts exactly; they are final user-requested outputs.\n\
- Use phase artifact fields to separate current-phase ownership from preserve and verify-only context. If there is no artifact for a field, omit the field or use an empty list.\n\
- Every required_artifacts path must appear in owned_artifacts for at least one phase that creates or mutates that artifact.\n\
- Do not include implementation details that belong inside a step plan.\n\
- Long text fields such as goal and phase goal may use quoted strings or YAML block scalars with markers |, |-, |+, >, >-, or >+; do not use anchors, aliases, merge keys, custom tags, or extra nested maps.\n\
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
    out.push_str("required_artifacts:\n");
    for path in &plan.required_artifacts {
        out.push_str(&format!("  - {}\n", yaml_string(path)));
    }
    out.push_str("phases:\n");
    for phase in &plan.phases {
        out.push_str(&format!("  - id: {}\n", yaml_string(&phase.id)));
        out.push_str(&format!("    goal: {}\n", yaml_string(&phase.goal)));
        render_phase_list(&mut out, "owned_artifacts", &phase.owned_artifacts);
        render_phase_list(&mut out, "preserve_artifacts", &phase.preserve_artifacts);
        render_phase_list(
            &mut out,
            "verify_only_artifacts",
            &phase.verify_only_artifacts,
        );
    }
    out
}

pub fn parse_ultra_plan_yaml(yaml: &str) -> Result<UltraPlan, UltraPlanError> {
    let mut goal = None;
    let mut profile = None;
    let mut style = None;
    let mut intent = None;
    let mut required_artifacts = Vec::new();
    let mut phases = Vec::new();
    let mut current_phase: Option<UltraPhase> = None;
    let mut current_list = None;
    let mut seen_phases = false;

    let lines = strip_yaml_fence(yaml).lines().collect::<Vec<_>>();
    let mut index = 0;
    while index < lines.len() {
        let raw = lines[index];
        index += 1;
        let line = raw.trim_end();
        if line.trim().is_empty() {
            continue;
        }
        if seen_phases && line.starts_with("goal:") && current_phase.is_some() {
            let value = line.strip_prefix("goal:").unwrap();
            let Some(phase) = current_phase.as_mut() else {
                unreachable!("checked current_phase above")
            };
            phase.goal = parse_yaml_scalar(&lines, &mut index, line, value, "phase.goal")?;
            current_list = None;
        } else if let Some(value) = line.strip_prefix("goal:") {
            goal = Some(parse_yaml_scalar(&lines, &mut index, line, value, "goal")?);
        } else if let Some(value) = line.strip_prefix("profile:") {
            profile = Some(parse_yaml_string(value.trim())?);
        } else if let Some(value) = line.strip_prefix("style:") {
            style = Some(parse_yaml_string(value.trim())?);
        } else if let Some(value) = line.strip_prefix("intent:") {
            intent = Some(parse_yaml_string(value.trim())?);
        } else if line == "required_artifacts:" {
            current_list = Some(ListField::Required);
        } else if let Some(value) = line.strip_prefix("required_artifacts:") {
            if parse_inline_empty_list(value.trim())? {
                current_list = None;
            }
        } else if line == "phases:" {
            current_list = None;
            seen_phases = true;
        } else if let Some(value) = phase_id_value(line) {
            if let Some(phase) = current_phase.take() {
                phases.push(phase);
            }
            current_phase = Some(UltraPhase {
                id: parse_yaml_string(value.trim())?,
                goal: String::new(),
                owned_artifacts: Vec::new(),
                preserve_artifacts: Vec::new(),
                verify_only_artifacts: Vec::new(),
            });
            current_list = None;
        } else if let Some(value) = phase_field_value(line, "goal") {
            let Some(phase) = current_phase.as_mut() else {
                return Err(UltraPlanError::InvalidYaml(
                    "phase goal appears before phase id".to_string(),
                ));
            };
            phase.goal = parse_yaml_scalar(&lines, &mut index, line, value, "phase.goal")?;
            current_list = None;
        } else if let Some(value) = phase_field_value(line, "owned_artifacts") {
            ensure_phase(&current_phase, "owned_artifacts")?;
            current_list = parse_phase_list_start(value, ListField::PhaseOwned)?;
        } else if let Some(value) = phase_field_value(line, "preserve_artifacts") {
            ensure_phase(&current_phase, "preserve_artifacts")?;
            current_list = parse_phase_list_start(value, ListField::PhasePreserve)?;
        } else if let Some(value) = phase_field_value(line, "verify_only_artifacts") {
            ensure_phase(&current_phase, "verify_only_artifacts")?;
            current_list = parse_phase_list_start(value, ListField::PhaseVerifyOnly)?;
        } else if let Some(value) = list_item_value(line) {
            match current_list {
                Some(ListField::Required) => {
                    required_artifacts.push(parse_yaml_string(value.trim())?)
                }
                Some(ListField::PhaseOwned) => current_phase
                    .as_mut()
                    .ok_or_else(|| {
                        UltraPlanError::InvalidYaml(
                            "owned_artifacts item appears before phase id".to_string(),
                        )
                    })?
                    .owned_artifacts
                    .push(parse_yaml_string(value.trim())?),
                Some(ListField::PhasePreserve) => current_phase
                    .as_mut()
                    .ok_or_else(|| {
                        UltraPlanError::InvalidYaml(
                            "preserve_artifacts item appears before phase id".to_string(),
                        )
                    })?
                    .preserve_artifacts
                    .push(parse_yaml_string(value.trim())?),
                Some(ListField::PhaseVerifyOnly) => current_phase
                    .as_mut()
                    .ok_or_else(|| {
                        UltraPlanError::InvalidYaml(
                            "verify_only_artifacts item appears before phase id".to_string(),
                        )
                    })?
                    .verify_only_artifacts
                    .push(parse_yaml_string(value.trim())?),
                None if current_phase.is_some() => {}
                None => {
                    return Err(UltraPlanError::InvalidYaml(
                        "list item appears outside a known list".to_string(),
                    ));
                }
            }
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
        intent: intent.unwrap_or_else(|| "unknown".to_string()),
        required_artifacts: dedupe_required_artifacts(required_artifacts),
        phases: dedupe_phase_artifacts(phases),
    };
    validate_ultra_plan(&plan)?;
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

fn dedupe_phase_artifacts(phases: Vec<UltraPhase>) -> Vec<UltraPhase> {
    phases
        .into_iter()
        .map(|phase| UltraPhase {
            owned_artifacts: dedupe_required_artifacts(phase.owned_artifacts),
            preserve_artifacts: dedupe_required_artifacts(phase.preserve_artifacts),
            verify_only_artifacts: dedupe_required_artifacts(phase.verify_only_artifacts),
            ..phase
        })
        .collect()
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
        for (field, values) in [
            ("owned_artifacts", &phase.owned_artifacts),
            ("preserve_artifacts", &phase.preserve_artifacts),
            ("verify_only_artifacts", &phase.verify_only_artifacts),
        ] {
            if values.iter().any(|value| value.trim().is_empty()) {
                return Err(UltraPlanError::EmptyField(format!(
                    "phase.{}.{}",
                    phase.id, field
                )));
            }
        }
    }
    Ok(())
}

fn render_phase_list(out: &mut String, key: &str, values: &[String]) {
    if values.is_empty() {
        return;
    }
    out.push_str(&format!("    {key}:\n"));
    for value in values {
        out.push_str(&format!("      - {}\n", yaml_string(value)));
    }
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

fn parse_yaml_scalar(
    lines: &[&str],
    index: &mut usize,
    field_line: &str,
    value: &str,
    field_name: &str,
) -> Result<String, UltraPlanError> {
    if let Some(block) = parse_block_scalar_value(lines, index, field_line, value, field_name)
        .map_err(UltraPlanError::InvalidYaml)?
    {
        Ok(block)
    } else {
        parse_yaml_string(value.trim())
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ListField {
    Required,
    PhaseOwned,
    PhasePreserve,
    PhaseVerifyOnly,
}

fn list_item_value(line: &str) -> Option<&str> {
    line.strip_prefix("- ")
        .or_else(|| line.strip_prefix("  - "))
        .or_else(|| line.strip_prefix("    - "))
        .or_else(|| line.strip_prefix("      - "))
}

fn parse_inline_empty_list(value: &str) -> Result<bool, UltraPlanError> {
    if value == "[]" {
        Ok(true)
    } else {
        Err(UltraPlanError::InvalidYaml(format!(
            "unsupported inline list value: {value}"
        )))
    }
}

fn parse_phase_list_start(
    value: &str,
    field: ListField,
) -> Result<Option<ListField>, UltraPlanError> {
    let value = value.trim();
    if value.is_empty() {
        Ok(Some(field))
    } else if parse_inline_empty_list(value)? {
        Ok(None)
    } else {
        unreachable!("parse_inline_empty_list returns Err for unsupported values")
    }
}

fn ensure_phase(phase: &Option<UltraPhase>, field: &str) -> Result<(), UltraPlanError> {
    if phase.is_some() {
        Ok(())
    } else {
        Err(UltraPlanError::InvalidYaml(format!(
            "phase {field} appears before phase id"
        )))
    }
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
        assert!(prompt.contains("YAML block scalars"));
        assert!(prompt.contains("do not use anchors"));
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
    fn accepts_literal_block_scalar_ultra_goal() {
        let yaml = r#"
goal: |
  Build a Next.js app.
  Keep it runnable on port 3011.
profile: nextjs
style: default
intent: new
phases:
  - id: scaffold
    goal: Create files.
"#;

        let plan = parse_ultra_plan_yaml(yaml).unwrap();
        let rendered = render_ultra_plan_yaml(&plan);
        let reparsed = parse_ultra_plan_yaml(&rendered).unwrap();

        assert_eq!(
            plan.goal,
            "Build a Next.js app.\nKeep it runnable on port 3011."
        );
        assert_eq!(reparsed, plan);
    }

    #[test]
    fn accepts_folded_strip_block_scalar_ultra_goal() {
        let yaml = r#"
goal: >-
  Build a Next.js app.
  Keep it runnable on port 3011.
profile: nextjs
style: default
intent: new
phases:
  - id: scaffold
    goal: Create files.
"#;

        let plan = parse_ultra_plan_yaml(yaml).unwrap();
        let rendered = render_ultra_plan_yaml(&plan);
        let reparsed = parse_ultra_plan_yaml(&rendered).unwrap();

        assert_eq!(
            plan.goal,
            "Build a Next.js app. Keep it runnable on port 3011."
        );
        assert_eq!(reparsed, plan);
    }

    #[test]
    fn accepts_literal_block_scalar_phase_goal() {
        let yaml = r#"
goal: Build app
profile: nextjs
style: default
intent: new
phases:
  - id: scaffold
    goal: |
      Create package.json.
      Create the app route.
  - id: verify
    goal: Verify build.
"#;

        let plan = parse_ultra_plan_yaml(yaml).unwrap();

        assert_eq!(
            plan.phases[0].goal,
            "Create package.json.\nCreate the app route."
        );
    }

    #[test]
    fn parses_and_renders_phase_artifact_scope() {
        let yaml = r#"
goal: Build app
profile: nextjs
style: default
intent: modify
required_artifacts:
  - app/page.tsx
phases:
  - id: ui
    goal: Update the UI only.
    owned_artifacts:
      - app/page.tsx
      - app/page.tsx
    preserve_artifacts:
      - package.json
    verify_only_artifacts:
      - tests/ui.test.ts
"#;

        let plan = parse_ultra_plan_yaml(yaml).unwrap();
        let rendered = render_ultra_plan_yaml(&plan);
        let reparsed = parse_ultra_plan_yaml(&rendered).unwrap();

        assert_eq!(plan.phases[0].owned_artifacts, vec!["app/page.tsx"]);
        assert_eq!(plan.phases[0].preserve_artifacts, vec!["package.json"]);
        assert_eq!(
            plan.phases[0].verify_only_artifacts,
            vec!["tests/ui.test.ts"]
        );
        assert!(rendered.contains("owned_artifacts:"));
        assert_eq!(reparsed, plan);
    }

    #[test]
    fn accepts_literal_strip_block_scalar_phase_goal() {
        let yaml = r#"
goal: Build app
profile: nextjs
style: default
intent: new
phases:
  - id: scaffold
    goal: |-
      Create package.json.
      Create the app route.
  - id: verify
    goal: Verify build.
"#;

        let plan = parse_ultra_plan_yaml(yaml).unwrap();
        let rendered = render_ultra_plan_yaml(&plan);
        let reparsed = parse_ultra_plan_yaml(&rendered).unwrap();

        assert_eq!(
            plan.phases[0].goal,
            "Create package.json.\nCreate the app route."
        );
        assert_eq!(reparsed, plan);
    }

    #[test]
    fn accepts_folded_block_scalar_phase_goal() {
        let yaml = r#"
goal: Build app
profile: nextjs
style: default
intent: new
phases:
  - id: scaffold
    goal: >
      Create package.json.
      Create the app route.
"#;

        let plan = parse_ultra_plan_yaml(yaml).unwrap();

        assert_eq!(
            plan.phases[0].goal,
            "Create package.json. Create the app route."
        );
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
    fn accepts_unindented_required_artifacts_and_phase_goals() {
        let yaml = r#"
goal: Build app
profile: nextjs
style: default
intent: new
required_artifacts:
- package.json
- app/page.tsx
phases:
- id: scaffold
goal: Create project files.
- id: verify
goal: Verify the build.
"#;

        let plan = parse_ultra_plan_yaml(yaml).unwrap();

        assert_eq!(
            plan.required_artifacts,
            vec!["package.json", "app/page.tsx"]
        );
        assert_eq!(plan.phases.len(), 2);
        assert_eq!(plan.phases[0].goal, "Create project files.");
        assert_eq!(plan.phases[1].goal, "Verify the build.");
    }

    #[test]
    fn dedupes_required_artifacts_stably() {
        let yaml = r#"
goal: Build app
profile: nextjs
style: default
intent: new
required_artifacts:
- package.json
- app/page.tsx
- package.json
- app/page.tsx
phases:
- id: scaffold
goal: Create project files.
- id: verify
goal: Verify the build.
"#;

        let plan = parse_ultra_plan_yaml(yaml).unwrap();

        assert_eq!(
            plan.required_artifacts,
            vec!["package.json", "app/page.tsx"]
        );
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
            required_artifacts: vec!["app/page.tsx".to_string()],
            phases: vec![
                UltraPhase {
                    id: "scaffold".to_string(),
                    goal: "Create the app skeleton.".to_string(),
                    owned_artifacts: Vec::new(),
                    preserve_artifacts: Vec::new(),
                    verify_only_artifacts: Vec::new(),
                },
                UltraPhase {
                    id: "verify".to_string(),
                    goal: "Run the build and fix failures.".to_string(),
                    owned_artifacts: Vec::new(),
                    preserve_artifacts: Vec::new(),
                    verify_only_artifacts: Vec::new(),
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
