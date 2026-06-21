use super::PlannerRuntimeConfig;
use crate::agent::minimal_loop::loop_run::ChatClient;
use crate::agent::step_runner::correction_evidence::PlanCorrectionEvidence;
use crate::agent::step_runner::plan_input::{
    StepPlanInputDefaults, looks_like_json_plan_input, parse_step_plan_input_with_defaults,
};
use crate::agent::step_runner::plan_lint::PlanLintError;
use crate::agent::step_runner::plan_lint::lint_step_plan_with_workspace_and_obligations;
use crate::agent::step_runner::profiles::ProfileObligation;
use crate::agent::step_runner::ultra_plan::{UltraPlan, parse_ultra_plan_yaml};
use crate::agent::step_runner::{
    ExpectedResult, StepKind, StepPlan, StepPlanStep, WorkIntent, extract_plan_from_response,
};
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
    fn from_parse_error(error: crate::agent::step_runner::PlanError) -> Self {
        let message = error.to_string();
        let mut evidence = PlanCorrectionEvidence::new("plan_lint.plan_input")
            .with_violated_contract("plan_input_contract")
            .with_reason_code(plan_error_reason_code(&error))
            .with_diagnostic(message.clone());
        match &error {
            crate::agent::step_runner::PlanError::MissingField(field)
            | crate::agent::step_runner::PlanError::EmptyField(field) => {
                evidence = evidence
                    .with_target_field(field.clone())
                    .with_missing_literals(vec![field.clone()])
                    .with_required_action(format!(
                        "include the required `{field}` field in the corrected plan"
                    ));
            }
            crate::agent::step_runner::PlanError::InvalidEnum { field, value } => {
                evidence = evidence
                    .with_target_field(field.clone())
                    .with_rejected_value(value.clone())
                    .with_required_action(format!(
                        "replace `{field}` with a supported canonical value"
                    ));
            }
            crate::agent::step_runner::PlanError::InvalidPlanInput(detail)
            | crate::agent::step_runner::PlanError::InvalidYaml(detail) => {
                evidence = evidence
                    .with_rejected_value(detail.clone())
                    .with_required_action(
                        "return a valid CommandAgent plan as YAML or JSON matching the public plan input schema",
                    );
            }
            crate::agent::step_runner::PlanError::NoSteps => {
                evidence = evidence
                    .with_target_field("steps")
                    .with_missing_literals(vec!["steps"])
                    .with_required_action("include at least one executable step");
            }
            crate::agent::step_runner::PlanError::DuplicateStepId(id)
            | crate::agent::step_runner::PlanError::InvalidStepId(id) => {
                evidence = evidence
                    .with_target_field("step.id")
                    .with_rejected_value(id.clone())
                    .with_required_action(
                        "use unique step ids containing only letters, numbers, '-' or '_'",
                    );
            }
            crate::agent::step_runner::PlanError::Io { .. } => {}
        }
        Self {
            message,
            correction_evidence: Some(Box::new(evidence)),
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
    let plan = if looks_like_json_plan_input(text) {
        parse_step_plan_input_with_defaults(
            text,
            Some(StepPlanInputDefaults {
                goal: context.goal,
                profile: context.profile,
                style: context.style,
                intent: context.intent,
                required_artifacts: context.required_artifacts,
            }),
        )
        .map_err(GeneratedPlanError::from_parse_error)?
    } else {
        let normalized = ensure_generated_plan_header(
            text,
            context.goal,
            context.profile,
            context.style,
            context.intent,
            context.required_artifacts,
        );
        extract_plan_from_response(&normalized).map_err(GeneratedPlanError::from_parse_error)?
    };
    let mut plan = plan;
    for _ in 0..4 {
        match lint_step_plan_with_workspace_and_obligations(
            &plan,
            Some(cwd),
            context.profile_obligations,
        ) {
            Ok(()) => return Ok(plan),
            Err(error) => {
                if let Some(materialized) = materialize_plan_from_lint_error(&plan, &error) {
                    plan = materialized;
                    continue;
                }
                return Err(GeneratedPlanError::from_lint(error));
            }
        }
    }
    lint_step_plan_with_workspace_and_obligations(&plan, Some(cwd), context.profile_obligations)
        .map(|()| plan)
        .map_err(GeneratedPlanError::from_lint)
}

fn plan_error_reason_code(error: &crate::agent::step_runner::PlanError) -> &'static str {
    match error {
        crate::agent::step_runner::PlanError::MissingField(_) => "plan_input_missing_field",
        crate::agent::step_runner::PlanError::EmptyField(_) => "plan_input_empty_field",
        crate::agent::step_runner::PlanError::NoSteps => "plan_input_no_steps",
        crate::agent::step_runner::PlanError::InvalidStepId(_) => "plan_input_invalid_step_id",
        crate::agent::step_runner::PlanError::DuplicateStepId(_) => "plan_input_duplicate_step_id",
        crate::agent::step_runner::PlanError::InvalidPlanInput(_) => "plan_input_invalid_json",
        crate::agent::step_runner::PlanError::InvalidYaml(_) => "plan_input_invalid_yaml",
        crate::agent::step_runner::PlanError::InvalidEnum { .. } => "plan_input_invalid_enum",
        crate::agent::step_runner::PlanError::Io { .. } => "plan_input_io_error",
    }
}

fn materialize_plan_from_lint_error(plan: &StepPlan, error: &PlanLintError) -> Option<StepPlan> {
    if let Some(materialized) = materialize_plan_without_directory_only_step(plan, error) {
        return Some(materialized);
    }
    let evidence = error.correction_evidence()?;
    if evidence.violated_contract.as_deref() == Some("nextjs_app_layout_plan_contract") {
        return materialize_nextjs_app_layout_step(plan, evidence);
    }
    if evidence.violated_contract.as_deref() == Some("nextjs_alias_plan_contract") {
        return materialize_nextjs_alias_imports(plan, evidence);
    }
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

fn materialize_nextjs_app_layout_step(
    plan: &StepPlan,
    evidence: &PlanCorrectionEvidence,
) -> Option<StepPlan> {
    let layout_path = evidence.target_path.as_deref()?;
    if !matches!(layout_path, "app/layout.tsx" | "src/app/layout.tsx") {
        return None;
    }
    if plan.steps.iter().any(|step| {
        step.expected_paths.iter().any(|path| path == layout_path)
            || step.instruction.contains(layout_path)
    }) {
        return None;
    }
    let mut materialized = plan.clone();
    let insert_at = materialized
        .steps
        .iter()
        .position(|step| matches!(step.kind, StepKind::Verify))
        .unwrap_or(materialized.steps.len());
    materialized.steps.insert(
        insert_at,
        StepPlanStep {
            id: unique_step_id(&materialized, "create-root-layout"),
            kind: StepKind::Create,
            instruction: format!(
                "Create {layout_path} with a minimal Next.js root layout. Use `import type {{ ReactNode }} from \"react\";` and export default function RootLayout({{ children }}: {{ children: ReactNode }}) that renders `<html lang=\"en\"><body>{{children}}</body></html>`."
            ),
            expected_result: ExpectedResult::Pass,
            expected_paths: vec![layout_path.to_string()],
            verify: vec![format!("test -f {layout_path}")],
        },
    );
    Some(materialized)
}

fn materialize_nextjs_alias_imports(
    plan: &StepPlan,
    evidence: &PlanCorrectionEvidence,
) -> Option<StepPlan> {
    if !evidence
        .missing_literals
        .iter()
        .any(|literal| literal == "@components/*")
    {
        return None;
    }
    let mut materialized = plan.clone();
    let mut changed = false;
    for step in &mut materialized.steps {
        if step.instruction.contains("@components/") {
            step.instruction = step.instruction.replace("@components/", "../components/");
            if !step
                .instruction
                .contains("CommandAgent deterministic alias obligation")
            {
                step.instruction.push_str(
                    "\n\nCommandAgent deterministic alias obligation: use relative imports for components from app routes unless tsconfig.json defines the exact alias used by the import.",
                );
            }
            changed = true;
        }
    }
    changed.then_some(materialized)
}

fn unique_step_id(plan: &StepPlan, base: &str) -> String {
    if !plan.steps.iter().any(|step| step.id == base) {
        return base.to_string();
    }
    for index in 2.. {
        let candidate = format!("{base}-{index}");
        if !plan.steps.iter().any(|step| step.id == candidate) {
            return candidate;
        }
    }
    unreachable!("unbounded unique step id search must return")
}

fn materialize_plan_without_directory_only_step(
    plan: &StepPlan,
    error: &PlanLintError,
) -> Option<StepPlan> {
    let PlanLintError::InvalidStepInstruction { step_id, reason } = error else {
        return None;
    };
    if !reason.contains("directory-only steps are unnecessary") {
        return None;
    }
    let mut materialized = plan.clone();
    let original_len = materialized.steps.len();
    materialized.steps.retain(|step| step.id != *step_id);
    if materialized.steps.len() == original_len || materialized.steps.is_empty() {
        return None;
    }
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
    fn generated_step_plan_accepts_folded_strip_block_scalar_instruction() {
        let root =
            std::env::temp_dir().join(format!("commandagent-folded-strip-plan-{}", now_ms()));
        std::fs::create_dir_all(&root).unwrap();
        let generated = r#"
steps:
  - id: write-readme
    kind: create
    instruction: >-
      Create README.md.
      Include usage notes.
    expected_paths:
      - README.md
    verify:
      - test -f README.md
"#;
        let context = GeneratedStepPlanContext {
            goal: "Create docs.",
            profile: "docs",
            style: "default",
            intent: WorkIntent::Document,
            required_artifacts: &[],
            profile_obligations: &[],
        };

        let plan = parse_generated_step_plan(&root, generated, &context).unwrap();

        assert_eq!(
            plan.steps[0].instruction,
            "Create README.md. Include usage notes."
        );
        let _ = std::fs::remove_dir_all(root);
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

    #[test]
    fn generated_typescript_plan_materializes_manifest_toolchain_versions() {
        let root = std::env::temp_dir().join(format!(
            "commandagent-typescript-materialization-{}",
            now_ms()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let generated = r#"
steps:
  - id: update-scripts
    kind: edit
    instruction: Edit package.json to ensure scripts.dev is next dev -p 3011 and scripts.build is next build. Ensure dependencies include next, react, react-dom, typescript, @types/react, @types/react-dom, and @types/node.
    expected_paths:
      - package.json
    verify:
      - test -f package.json
  - id: verify-build
    kind: verify
    instruction: Run npm run build.
    expected_paths: []
    verify:
      - npm run build
"#;
        let context = GeneratedStepPlanContext {
            goal: "Create a minimal Next.js app.",
            profile: "nextjs",
            style: "default",
            intent: WorkIntent::Modify,
            required_artifacts: &[],
            profile_obligations: &[],
        };

        let plan = parse_generated_step_plan(&root, generated, &context).unwrap();
        let package_step = plan
            .steps
            .iter()
            .find(|step| step.id == "update-scripts")
            .unwrap();

        assert!(
            package_step
                .instruction
                .contains("CommandAgent deterministic manifest obligation")
        );
        assert!(package_step.instruction.contains("typescript 5.x"));
        assert!(package_step.instruction.contains("@types/react 18.x"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn generated_nextjs_plan_materializes_app_layout_step() {
        let root =
            std::env::temp_dir().join(format!("commandagent-layout-materialization-{}", now_ms()));
        std::fs::create_dir_all(&root).unwrap();
        let generated = r#"
steps:
  - id: create-page
    kind: create
    instruction: Create app/page.tsx with a default export.
    expected_paths:
      - app/page.tsx
    verify:
      - test -f app/page.tsx
  - id: verify-build
    kind: verify
    instruction: Run npm run build.
    expected_paths: []
    verify:
      - npm run build
"#;
        let context = GeneratedStepPlanContext {
            goal: "Create a minimal Next.js app.",
            profile: "nextjs",
            style: "default",
            intent: WorkIntent::New,
            required_artifacts: &[],
            profile_obligations: &[],
        };

        let plan = parse_generated_step_plan(&root, generated, &context).unwrap();

        assert!(plan.steps.iter().any(|step| {
            step.id == "create-root-layout"
                && step.expected_paths == vec!["app/layout.tsx".to_string()]
        }));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn generated_nextjs_plan_materializes_components_alias_to_relative_import() {
        let root =
            std::env::temp_dir().join(format!("commandagent-alias-materialization-{}", now_ms()));
        std::fs::create_dir_all(&root).unwrap();
        let generated = r#"
steps:
  - id: create-package-json
    kind: create
    instruction: Create package.json with next, react, react-dom, typescript ^5.4.0, @types/react 18, scripts.dev=next dev -p 3011, and scripts.build=next build.
    expected_paths:
      - package.json
    verify:
      - test -f package.json
  - id: create-tsconfig
    kind: create
    instruction: Create tsconfig.json with compilerOptions.paths mapping @/* to ./*.
    expected_paths:
      - tsconfig.json
    verify:
      - test -f tsconfig.json
  - id: create-page
    kind: create
    instruction: Create app/page.tsx importing Game from "@components/Game".
    expected_paths:
      - app/page.tsx
    verify:
      - test -f app/page.tsx
"#;
        let context = GeneratedStepPlanContext {
            goal: "Create a minimal Next.js app.",
            profile: "nextjs",
            style: "default",
            intent: WorkIntent::New,
            required_artifacts: &[],
            profile_obligations: &[],
        };

        let plan = parse_generated_step_plan(&root, generated, &context).unwrap();
        let page_step = plan
            .steps
            .iter()
            .find(|step| step.id == "create-page")
            .unwrap();

        assert!(page_step.instruction.contains("../components/Game"));
        assert!(!page_step.instruction.contains("@components/Game"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn generated_plan_materializes_away_directory_only_step() {
        let root = std::env::temp_dir().join(format!(
            "commandagent-directory-step-materialization-{}",
            now_ms()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let generated = r#"
steps:
  - id: init-test-dir
    kind: create
    instruction: Create the tests directory structure.
    expected_paths: []
    verify:
      - test -d tests
  - id: write-test
    kind: create
    instruction: Create tests/test_math_tools.py with assertions for add.
    expected_paths:
      - tests/test_math_tools.py
    verify:
      - python -m py_compile tests/test_math_tools.py
  - id: write-source
    kind: create
    instruction: Create math_tools.py with add(a, b).
    expected_paths:
      - math_tools.py
    verify:
      - python -m py_compile math_tools.py
"#;
        let required = vec![
            "math_tools.py".to_string(),
            "tests/test_math_tools.py".to_string(),
        ];
        let context = GeneratedStepPlanContext {
            goal: "Create Python source and tests.",
            profile: "python",
            style: "tdd",
            intent: WorkIntent::New,
            required_artifacts: &required,
            profile_obligations: &[],
        };

        let plan = parse_generated_step_plan(&root, generated, &context).unwrap();

        assert!(!plan.steps.iter().any(|step| step.id == "init-test-dir"));
        assert!(plan.steps.iter().any(|step| step.id == "write-test"));
        assert!(plan.steps.iter().any(|step| step.id == "write-source"));
        let _ = std::fs::remove_dir_all(root);
    }
}
