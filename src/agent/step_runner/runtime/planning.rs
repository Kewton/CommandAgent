use super::PlannerRuntimeConfig;
use crate::agent::minimal_loop::loop_run::ChatClient;
use crate::agent::step_runner::correction_evidence::PlanCorrectionEvidence;
use crate::agent::step_runner::plan_lint::PlanLintError;
use crate::agent::step_runner::plan_lint::lint_step_plan_with_workspace_and_obligations;
use crate::agent::step_runner::profiles::ProfileObligation;
use crate::agent::step_runner::ultra_plan::{UltraPlan, parse_ultra_plan_yaml};
use crate::agent::step_runner::{StepPlan, WorkIntent, extract_plan_from_response};
use crate::providers::{ChatMessage, ChatRequest, ChatRole};
use std::fs;
use std::path::Path;

pub(super) struct StepPlanCorrectionContext<'a> {
    pub(super) goal: &'a str,
    pub(super) profile: &'a str,
    pub(super) style: &'a str,
    pub(super) intent: WorkIntent,
    pub(super) required_artifacts: &'a [String],
    pub(super) profile_obligations: &'a [ProfileObligation],
    pub(super) save_kind: &'a str,
    pub(super) prompt_kind: &'a str,
}

pub(super) struct GeneratedStepPlanContext<'a> {
    pub(super) goal: &'a str,
    pub(super) profile: &'a str,
    pub(super) style: &'a str,
    pub(super) intent: WorkIntent,
    pub(super) required_artifacts: &'a [String],
    pub(super) profile_obligations: &'a [ProfileObligation],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GeneratedPlanError {
    pub(super) message: String,
    pub(super) correction_evidence: Option<PlanCorrectionEvidence>,
}

impl GeneratedPlanError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            correction_evidence: None,
        }
    }

    fn from_lint(error: PlanLintError) -> Self {
        Self {
            message: format!("plan lint failed: {error}"),
            correction_evidence: error.correction_evidence().cloned(),
        }
    }
}

impl std::fmt::Display for GeneratedPlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

pub(super) fn planner_text<C>(
    client: &mut C,
    config: &PlannerRuntimeConfig,
    prompt: &str,
) -> Result<String, String>
where
    C: ChatClient,
{
    let response = client.chat(&ChatRequest {
        model: config.model.clone(),
        messages: vec![
            ChatMessage {
                role: ChatRole::System,
                content: "You generate CommandAgent plan YAML. Return only the requested YAML."
                    .to_string(),
            },
            ChatMessage {
                role: ChatRole::User,
                content: prompt.to_string(),
            },
        ],
        tools: Vec::new(),
        tool_call_mode: config.tool_call_mode,
    })?;
    Ok(response.content)
}

pub(super) fn parse_generated_step_plan(
    cwd: &Path,
    text: &str,
    context: &GeneratedStepPlanContext<'_>,
) -> Result<StepPlan, GeneratedPlanError> {
    let normalized = ensure_generated_plan_header(
        text,
        context.goal,
        context.profile,
        context.style,
        context.intent,
        context.required_artifacts,
    );
    let plan = extract_plan_from_response(&normalized)
        .map_err(|err| GeneratedPlanError::new(err.to_string()))?;
    lint_step_plan_with_workspace_and_obligations(&plan, Some(cwd), context.profile_obligations)
        .map_err(GeneratedPlanError::from_lint)?;
    Ok(plan)
}

pub(super) fn parse_generated_ultra_plan(
    text: &str,
    goal: &str,
    profile: &str,
    style: &str,
    intent: WorkIntent,
    required_artifacts: &[String],
) -> Result<UltraPlan, String> {
    let mut plan = parse_ultra_plan_yaml(text).map_err(|err| err.to_string())?;
    plan.goal = goal.to_string();
    plan.profile = profile.to_string();
    plan.style = style.to_string();
    plan.intent = intent.as_str().to_string();
    plan.required_artifacts = dedupe_required_artifacts(required_artifacts.iter().cloned());
    Ok(plan)
}

fn ensure_generated_plan_header(
    text: &str,
    goal: &str,
    profile: &str,
    style: &str,
    intent: WorkIntent,
    required_artifacts: &[String],
) -> String {
    let body = strip_markdown_fence(text.trim());
    let mut out = String::new();
    out.push_str(&format!("goal: {}\n", yaml_quote(goal)));
    out.push_str(&format!("profile: {}\n", yaml_quote(profile)));
    out.push_str(&format!("style: {}\n", yaml_quote(style)));
    out.push_str(&format!("intent: {}\n", yaml_quote(intent.as_str())));
    out.push_str("required_artifacts:\n");
    for path in required_artifacts {
        out.push_str(&format!("  - {}\n", yaml_quote(path)));
    }
    let mut skip_model_required_artifacts = false;
    for line in body.lines() {
        if line.starts_with("required_artifacts:") {
            skip_model_required_artifacts = true;
            continue;
        }
        if skip_model_required_artifacts {
            if line.starts_with("  - ")
                || line.starts_with("    - ")
                || line.starts_with("      - ")
            {
                continue;
            }
            skip_model_required_artifacts = false;
        }
        if line.starts_with("goal:")
            || line.starts_with("profile:")
            || line.starts_with("style:")
            || line.starts_with("intent:")
            || line.starts_with("required_artifacts:")
        {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

pub(super) fn save_invalid_generated_plan(
    cwd: &Path,
    kind: &str,
    text: &str,
) -> Result<(), String> {
    let dir = crate::util::workspace_paths::plans_dir(cwd);
    fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    let stamp = now_ms();
    let mut path = dir.join(format!("invalid-{kind}-{stamp}.yaml"));
    let mut suffix = 1;
    while path.exists() {
        path = dir.join(format!("invalid-{kind}-{stamp}-{suffix}.yaml"));
        suffix += 1;
    }
    fs::write(path, text).map_err(|err| err.to_string())
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn strip_markdown_fence(text: &str) -> &str {
    if let Some(rest) = text.strip_prefix("```yaml") {
        return rest.trim().strip_suffix("```").unwrap_or(rest).trim();
    }
    if let Some(rest) = text.strip_prefix("```") {
        return rest.trim().strip_suffix("```").unwrap_or(rest).trim();
    }
    text
}

fn yaml_quote(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

fn dedupe_required_artifacts(paths: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for path in paths {
        if seen.insert(path.clone()) {
            out.push(path);
        }
    }
    out
}
