use super::PlannerRuntimeConfig;
use crate::agent::minimal_loop::loop_run::ChatClient;
use crate::agent::step_runner::correction_evidence::PlanCorrectionEvidence;
use crate::agent::step_runner::plan_lint::PlanLintError;
use crate::agent::step_runner::plan_lint::lint_step_plan_with_workspace_and_obligations;
use crate::agent::step_runner::profiles::ProfileObligation;
use crate::agent::step_runner::ultra_plan::{UltraPlan, parse_ultra_plan_yaml};
use crate::agent::step_runner::{StepPlan, StepPlanStep, WorkIntent, extract_plan_from_response};
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
    pub(super) correction_evidence: Option<Box<PlanCorrectionEvidence>>,
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
            correction_evidence: error.correction_evidence().cloned().map(Box::new),
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
            ChatMessage::new(
                ChatRole::System,
                "You generate CommandAgent plan YAML. Return only the requested YAML.",
            ),
            ChatMessage::new(ChatRole::User, prompt),
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
    match lint_step_plan_with_workspace_and_obligations(
        &plan,
        Some(cwd),
        context.profile_obligations,
    ) {
        Ok(()) => Ok(plan),
        Err(error) => {
            if let Some(materialized) = materialize_plan_from_lint_error(&plan, &error) {
                lint_step_plan_with_workspace_and_obligations(
                    &materialized,
                    Some(cwd),
                    context.profile_obligations,
                )
                .map_err(GeneratedPlanError::from_lint)?;
                Ok(materialized)
            } else {
                Err(GeneratedPlanError::from_lint(error))
            }
        }
    }
}

fn materialize_plan_from_lint_error(plan: &StepPlan, error: &PlanLintError) -> Option<StepPlan> {
    let evidence = error.correction_evidence()?;
    if evidence.active_job.as_deref() != Some("manifest_repair") {
        return None;
    }
    if evidence.target_path.as_deref() != Some("package.json") {
        return None;
    }
    let target_step_id = single_package_step_id_from_evidence(plan, evidence)?;
    let mut materialized = plan.clone();
    let step = materialized
        .steps
        .iter_mut()
        .find(|step| step.id == target_step_id)?;
    materialize_nextjs_manifest_obligation(step, evidence);
    Some(materialized)
}

fn single_package_step_id_from_evidence(
    plan: &StepPlan,
    evidence: &PlanCorrectionEvidence,
) -> Option<String> {
    if let Some(target) = evidence.repair_target.as_deref()
        && let Some(step_id) = target
            .strip_prefix("step:")
            .and_then(|rest| rest.split(':').next())
        && plan
            .steps
            .iter()
            .filter(|step| step_mentions_package_json(step))
            .any(|step| step.id == step_id)
    {
        return Some(step_id.to_string());
    }
    let package_steps = plan
        .steps
        .iter()
        .filter(|step| step_mentions_package_json(step))
        .collect::<Vec<_>>();
    match package_steps.as_slice() {
        [step] => Some(step.id.clone()),
        _ => None,
    }
}

fn step_mentions_package_json(step: &StepPlanStep) -> bool {
    step.expected_paths
        .iter()
        .any(|path| path == "package.json")
        || step
            .instruction
            .to_ascii_lowercase()
            .contains("package.json")
}

fn materialize_nextjs_manifest_obligation(
    step: &mut StepPlanStep,
    evidence: &PlanCorrectionEvidence,
) {
    const MARKER: &str = "CommandAgent deterministic manifest obligation";
    if !step.instruction.contains(MARKER) {
        let required = evidence.required_literals.join(" ").to_ascii_lowercase();
        let missing = evidence.missing_literals.join(" ").to_ascii_lowercase();
        let contracts = evidence
            .violated_contract
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let mentions = |needle: &str| {
            required.contains(needle) || missing.contains(needle) || contracts.contains(needle)
        };
        let mut clauses = vec![
            "next".to_string(),
            "react".to_string(),
            "react-dom".to_string(),
            "React 18.2+ compatibility".to_string(),
        ];
        if mentions("typescript") || mentions("@types/react") {
            clauses.push("typescript 5.x".to_string());
            clauses.push("@types/react 18.x".to_string());
        }
        let tailwind_required = mentions("tailwind")
            || mentions("tailwindcss")
            || mentions("postcss")
            || mentions("autoprefixer");
        if tailwind_required {
            clauses.push("tailwindcss".to_string());
            clauses.push("postcss".to_string());
            clauses.push("autoprefixer".to_string());
        }
        let mut script_clauses = Vec::new();
        if mentions("next build") {
            script_clauses.push("preserve scripts.build=next build".to_string());
        }
        if mentions("3011") {
            script_clauses.push("preserve scripts.dev port 3011".to_string());
        }
        let block = format!(
            "{MARKER}: keep the profile manifest setup explicit in this package/setup step. \
Literally include package.json dependencies or devDependencies {};{}{}",
            clauses.join(", "),
            if script_clauses.is_empty() {
                String::new()
            } else {
                format!(" {};", script_clauses.join("; "))
            },
            if tailwind_required {
                " also create setup/config outputs tailwind.config.js and postcss.config.js."
                    .to_string()
            } else {
                String::new()
            }
        );
        if step.instruction.trim().is_empty() {
            step.instruction = block;
        } else {
            step.instruction = format!("{}\n\n{}", step.instruction.trim_end(), block);
        }
    }
    let tailwind_required = evidence
        .required_literals
        .iter()
        .chain(evidence.missing_literals.iter())
        .any(|literal| {
            matches!(
                literal.as_str(),
                "tailwindcss" | "postcss" | "autoprefixer" | "tailwind.config" | "postcss.config"
            )
        });
    if tailwind_required {
        for path in ["tailwind.config.js", "postcss.config.js"] {
            if !step.expected_paths.iter().any(|existing| existing == path) {
                step.expected_paths.push(path.to_string());
            }
        }
    }
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
    let lines = body.lines().collect::<Vec<_>>();
    let mut index = 0;
    let mut skip_model_required_artifacts = false;
    while index < lines.len() {
        let line = lines[index];
        index += 1;
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
        if let Some(value) = overridden_header_value(line) {
            if is_block_scalar_start(value) {
                skip_block_scalar_continuation(&lines, &mut index, line);
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn overridden_header_value(line: &str) -> Option<&str> {
    line.strip_prefix("goal:")
        .or_else(|| line.strip_prefix("profile:"))
        .or_else(|| line.strip_prefix("style:"))
        .or_else(|| line.strip_prefix("intent:"))
        .or_else(|| line.strip_prefix("required_artifacts:"))
}

fn is_block_scalar_start(value: &str) -> bool {
    let value = value.trim();
    value == "|" || value == ">" || value.starts_with('|') || value.starts_with('>')
}

fn skip_block_scalar_continuation(lines: &[&str], index: &mut usize, field_line: &str) {
    let field_indent = leading_spaces(field_line);
    while *index < lines.len() {
        let line = lines[*index].trim_end();
        if line.trim().is_empty() {
            *index += 1;
            continue;
        }
        if leading_spaces(line) <= field_indent {
            break;
        }
        *index += 1;
    }
}

fn leading_spaces(line: &str) -> usize {
    line.as_bytes()
        .iter()
        .take_while(|byte| **byte == b' ')
        .count()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::step_runner::{parse_step_plan_yaml, render_step_plan_yaml};

    #[test]
    fn generated_header_normalization_skips_block_scalar_goal_body() {
        let generated = r#"
goal: |
  Model supplied goal line.
  This header should be replaced.
profile: docs
style: default
intent: document
steps:
  - id: write-readme
    kind: create
    instruction: |
      Create README.md.
      Keep it short.
    expected_paths:
      - README.md
    verify:
      - test -f README.md
"#;

        let normalized = ensure_generated_plan_header(
            generated,
            "Context goal",
            "docs",
            "default",
            WorkIntent::Document,
            &[],
        );
        let plan = parse_step_plan_yaml(&normalized).unwrap();

        assert_eq!(plan.goal, "Context goal");
        assert_eq!(
            plan.steps[0].instruction,
            "Create README.md.\nKeep it short."
        );
        assert!(!normalized.contains("Model supplied goal line."));
    }

    #[test]
    fn generated_tailwind_plan_materializes_manifest_obligation_for_single_package_step() {
        let root = std::env::temp_dir().join(format!(
            "commandagent-tailwind-materialization-{}",
            now_ms()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let generated = r#"
steps:
  - id: setup-package-json
    kind: setup
    instruction: Create package.json with next, react, react-dom, TypeScript 5.x, @types/react 18, and Tailwind CSS dependencies.
    expected_paths:
      - package.json
    verify:
      - test -f package.json
  - id: create-global-css
    kind: create
    instruction: Create app/globals.css with Tailwind CSS directives.
    expected_paths:
      - app/globals.css
    verify:
      - test -f app/globals.css
"#;
        let context = GeneratedStepPlanContext {
            goal: "Create a Next.js app.",
            profile: "nextjs",
            style: "default",
            intent: WorkIntent::New,
            required_artifacts: &[],
            profile_obligations: &[],
        };

        let plan = parse_generated_step_plan(&root, generated, &context).unwrap();
        let package_step = plan
            .steps
            .iter()
            .find(|step| step.id == "setup-package-json")
            .unwrap();
        let rendered = render_step_plan_yaml(&plan);

        assert!(
            package_step
                .instruction
                .contains("CommandAgent deterministic manifest obligation")
        );
        assert!(package_step.instruction.contains("tailwindcss"));
        assert!(package_step.instruction.contains("postcss"));
        assert!(package_step.instruction.contains("autoprefixer"));
        assert!(
            package_step
                .expected_paths
                .contains(&"tailwind.config.js".to_string())
        );
        assert!(
            package_step
                .expected_paths
                .contains(&"postcss.config.js".to_string())
        );
        assert!(rendered.contains("tailwindcss"));
        let _ = std::fs::remove_dir_all(root);
    }
}
