use super::plan_yaml::parse_step_plan_yaml;
use super::{ExpectedResult, PlanError, StepKind, StepPlan, StepPlanStep, WorkIntent};
use serde_json::Value;

#[derive(Debug, Clone, Copy)]
pub(crate) struct StepPlanInputDefaults<'a> {
    pub(crate) goal: &'a str,
    pub(crate) profile: &'a str,
    pub(crate) style: &'a str,
    pub(crate) intent: WorkIntent,
    pub(crate) required_artifacts: &'a [String],
}

pub(crate) fn looks_like_json_plan_input(text: &str) -> bool {
    let text = strip_code_fence(text).trim_start();
    text.starts_with('{') || text.starts_with('[')
}

pub(crate) fn parse_step_plan_input(text: &str) -> Result<StepPlan, PlanError> {
    parse_step_plan_input_with_defaults(text, None)
}

pub(crate) fn parse_step_plan_input_with_defaults(
    text: &str,
    defaults: Option<StepPlanInputDefaults<'_>>,
) -> Result<StepPlan, PlanError> {
    let text = strip_code_fence(text).trim();
    if text.starts_with('{') || text.starts_with('[') {
        parse_json_step_plan(text, defaults)
    } else {
        parse_step_plan_yaml(text)
    }
}

fn parse_json_step_plan(
    text: &str,
    defaults: Option<StepPlanInputDefaults<'_>>,
) -> Result<StepPlan, PlanError> {
    let value = serde_json::from_str::<Value>(text)
        .map_err(|err| PlanError::InvalidPlanInput(format!("json parse error: {err}")))?;
    let object = match &value {
        Value::Object(object) => object,
        Value::Array(_) => {
            let Some(defaults) = defaults else {
                return Err(PlanError::MissingField("goal".to_string()));
            };
            return parse_json_steps_only(value, defaults);
        }
        _ => {
            return Err(PlanError::InvalidPlanInput(
                "top-level JSON plan must be an object or steps array".to_string(),
            ));
        }
    };

    let goal = string_field(object, "goal")
        .or_else(|| defaults.map(|value| value.goal.to_string()))
        .ok_or_else(|| PlanError::MissingField("goal".to_string()))?;
    let profile = string_field(object, "profile")
        .or_else(|| defaults.map(|value| value.profile.to_string()))
        .ok_or_else(|| PlanError::MissingField("profile".to_string()))?;
    let style = string_field(object, "style")
        .or_else(|| defaults.map(|value| value.style.to_string()))
        .unwrap_or_else(|| "default".to_string());
    let intent = string_field(object, "intent")
        .map(|value| WorkIntent::parse(&value))
        .transpose()?
        .or_else(|| defaults.map(|value| value.intent))
        .unwrap_or(WorkIntent::Unknown);
    let mut required_artifacts = string_list_field(object, "required_artifacts")?;
    if required_artifacts.is_empty()
        && let Some(defaults) = defaults
    {
        required_artifacts = defaults.required_artifacts.to_vec();
    }
    let steps_value = object
        .get("steps")
        .or_else(|| object.get("plan"))
        .ok_or_else(|| PlanError::MissingField("steps".to_string()))?;
    let steps = parse_json_steps(steps_value)?;
    let plan = StepPlan {
        goal,
        profile,
        style,
        intent,
        required_artifacts,
        steps,
    };
    super::plan_yaml::validate_step_plan(&plan)?;
    Ok(plan)
}

fn parse_json_steps_only(
    value: Value,
    defaults: StepPlanInputDefaults<'_>,
) -> Result<StepPlan, PlanError> {
    let steps = parse_json_steps(&value)?;
    let plan = StepPlan {
        goal: defaults.goal.to_string(),
        profile: defaults.profile.to_string(),
        style: defaults.style.to_string(),
        intent: defaults.intent,
        required_artifacts: defaults.required_artifacts.to_vec(),
        steps,
    };
    super::plan_yaml::validate_step_plan(&plan)?;
    Ok(plan)
}

fn parse_json_steps(value: &Value) -> Result<Vec<StepPlanStep>, PlanError> {
    let steps = value
        .as_array()
        .ok_or_else(|| PlanError::InvalidPlanInput("steps must be an array".to_string()))?;
    steps
        .iter()
        .enumerate()
        .map(|(index, value)| parse_json_step(index, value))
        .collect()
}

fn parse_json_step(index: usize, value: &Value) -> Result<StepPlanStep, PlanError> {
    let object = value
        .as_object()
        .ok_or_else(|| PlanError::InvalidPlanInput("each step must be an object".to_string()))?;
    let id = string_field(object, "id").unwrap_or_else(|| format!("step-{}", index + 1));
    let kind = string_field(object, "kind")
        .or_else(|| string_field(object, "type"))
        .map(|value| StepKind::parse(&value))
        .transpose()?;
    let instruction = string_field(object, "instruction")
        .or_else(|| string_field(object, "step"))
        .or_else(|| string_field(object, "task"))
        .ok_or_else(|| PlanError::MissingField(format!("step.{id}.instruction")))?;
    let expected_result = string_field(object, "expected_result")
        .map(|value| ExpectedResult::parse(&value))
        .transpose()?
        .unwrap_or(ExpectedResult::Pass);
    let expected_paths = string_list_field(object, "expected_paths")
        .or_else(|_| string_list_field(object, "output_paths"))
        .or_else(|_| string_list_field(object, "artifacts"))?;
    let verify = string_list_field(object, "verify")
        .or_else(|_| string_list_field(object, "verification"))
        .or_else(|_| string_list_field(object, "checks"))
        .or_else(|_| string_list_field(object, "commands"))?;
    Ok(StepPlanStep {
        id,
        kind: kind.unwrap_or_else(|| infer_json_step_kind(&instruction, &expected_paths, &verify)),
        instruction,
        expected_result,
        expected_paths,
        verify,
    })
}

fn string_field(map: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    map.get(key).and_then(|value| match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    })
}

fn string_list_field(
    map: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<Vec<String>, PlanError> {
    let Some(value) = map.get(key) else {
        return Ok(Vec::new());
    };
    match value {
        Value::Array(values) => values
            .iter()
            .map(|value| match value {
                Value::String(value) => Ok(value.clone()),
                Value::Number(value) => Ok(value.to_string()),
                Value::Bool(value) => Ok(value.to_string()),
                _ => Err(PlanError::InvalidPlanInput(format!(
                    "{key} entries must be strings"
                ))),
            })
            .collect(),
        Value::String(value) if value.trim().is_empty() => Ok(Vec::new()),
        Value::String(value) => Ok(vec![value.clone()]),
        _ => Err(PlanError::InvalidPlanInput(format!(
            "{key} must be a string or array"
        ))),
    }
}

fn infer_json_step_kind(
    instruction: &str,
    expected_paths: &[String],
    verify: &[String],
) -> StepKind {
    let lower = instruction.to_ascii_lowercase();
    if !verify.is_empty() || contains_any(&lower, &["verify", "validate", "test", "build"]) {
        StepKind::Verify
    } else if contains_any(&lower, &["install", "setup", "configure"]) {
        StepKind::Setup
    } else if contains_any(&lower, &["fix", "repair"]) {
        StepKind::Repair
    } else if contains_any(&lower, &["report", "summarize", "summarise"]) {
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

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn strip_code_fence(text: &str) -> &str {
    let text = text.trim();
    if let Some(rest) = text.strip_prefix("```json") {
        return rest.trim().strip_suffix("```").unwrap_or(rest).trim();
    }
    if let Some(rest) = text.strip_prefix("```yaml") {
        return rest.trim().strip_suffix("```").unwrap_or(rest).trim();
    }
    if let Some(rest) = text.strip_prefix("```") {
        return rest.trim().strip_suffix("```").unwrap_or(rest).trim();
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json_plan_object() {
        let plan = parse_step_plan_input(
            r#"{
              "goal": "Create docs",
              "profile": "docs",
              "steps": [
                {
                  "id": "write-readme",
                  "kind": "create",
                  "instruction": "Create README.md with usage.",
                  "expected_paths": ["README.md"],
                  "verify": ["test -f README.md"]
                }
              ]
            }"#,
        )
        .unwrap();

        assert_eq!(plan.goal, "Create docs");
        assert_eq!(plan.steps[0].id, "write-readme");
        assert_eq!(plan.steps[0].kind, StepKind::Create);
        assert_eq!(plan.steps[0].expected_paths, vec!["README.md"]);
    }

    #[test]
    fn parses_json_steps_array_with_defaults() {
        let defaults = StepPlanInputDefaults {
            goal: "Create app",
            profile: "nextjs",
            style: "default",
            intent: WorkIntent::New,
            required_artifacts: &["package.json".to_string()],
        };

        let plan = parse_step_plan_input_with_defaults(
            r#"[{"step": "Create package.json", "expected_paths": ["package.json"]}]"#,
            Some(defaults),
        )
        .unwrap();

        assert_eq!(plan.goal, "Create app");
        assert_eq!(plan.profile, "nextjs");
        assert_eq!(plan.steps[0].id, "step-1");
        assert_eq!(plan.steps[0].kind, StepKind::Create);
    }

    #[test]
    fn rejects_unknown_json_shape() {
        let err =
            parse_step_plan_input(r#"{"goal":"x","profile":"generic","steps":42}"#).unwrap_err();

        assert!(err.to_string().contains("invalid plan input"));
    }
}
