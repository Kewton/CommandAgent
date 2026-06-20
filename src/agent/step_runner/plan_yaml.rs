use super::yaml_scalar::parse_block_scalar_value;
use super::{ExpectedResult, PlanError, StepKind, StepPlan, StepPlanStep, WorkIntent};
use serde_json::Value;
use std::collections::HashSet;

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

    let lines = strip_yaml_fence(yaml).lines().collect::<Vec<_>>();
    let mut index = 0;
    while index < lines.len() {
        let raw = lines[index];
        index += 1;
        let line = raw.trim_end();
        if line.trim().is_empty() {
            continue;
        }
        if let Some(value) = line.strip_prefix("goal:") {
            goal = Some(parse_yaml_scalar(&lines, &mut index, line, value, "goal")?);
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
            step.instruction = parse_yaml_scalar(&lines, &mut index, line, value, "instruction")?;
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

fn parse_yaml_scalar(
    lines: &[&str],
    index: &mut usize,
    field_line: &str,
    value: &str,
    field_name: &str,
) -> Result<String, PlanError> {
    if let Some(block) = parse_block_scalar_value(lines, index, field_line, value, field_name)
        .map_err(PlanError::InvalidYaml)?
    {
        Ok(block)
    } else {
        parse_yaml_string(value.trim())
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
