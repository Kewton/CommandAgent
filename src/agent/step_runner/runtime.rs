use crate::agent::minimal_loop::guards::is_file_change_tool;
use crate::agent::minimal_loop::loop_run::{ChatClient, MinimalLoopConfig, RunResult, run_session};
use crate::agent::slash_command::{SlashCommand, SlashCommandKind};
use crate::agent::step_runner::plan_lint::lint_step_plan_with_workspace;
use crate::agent::step_runner::profiles::profile_contract_text;
use crate::agent::step_runner::repair::{
    RepairBudget, RepairContext, build_repair_prompt, save_repair_prompt,
};
use crate::agent::step_runner::ultra_plan::{
    UltraPlan, parse_ultra_plan_yaml, save_ultra_plan, ultra_plan_generation_prompt,
};
use crate::agent::step_runner::ultra_run::phase_step_plan_prompt;
use crate::agent::step_runner::verify::{VerificationFailure, run_verifiers};
use crate::agent::step_runner::{
    ExpectedResult, StepPlan, StepPlanStep, WorkIntent, detect_work_intent,
    extract_plan_from_response, parse_step_plan_yaml, plan_generation_prompt, save_step_plan,
};
use crate::providers::{ChatMessage, ChatRequest, ChatRole, ToolCallMode};
use crate::safety::path_guard::PathGuard;
use std::fs;
use std::path::Path;

const MAX_REPAIR_TURNS: usize = 3;

#[derive(Debug, Clone)]
pub struct PlannerRuntimeConfig {
    pub model: String,
    pub tool_call_mode: ToolCallMode,
}

pub struct SlashRuntime<'a, E, P> {
    pub executor: &'a mut E,
    pub planner: &'a mut P,
    pub cwd: &'a Path,
    pub loop_config: MinimalLoopConfig,
    pub planner_config: PlannerRuntimeConfig,
}

impl<E, P> SlashRuntime<'_, E, P>
where
    E: ChatClient,
    P: ChatClient,
{
    pub fn run(&mut self, command: SlashCommand) -> Result<String, String> {
        let profile = command.profile.unwrap_or_else(|| "generic".to_string());
        let style = command.style.unwrap_or_else(|| "default".to_string());
        let intent = command
            .intent
            .as_deref()
            .map(WorkIntent::parse)
            .transpose()
            .map_err(|err| err.to_string())?
            .unwrap_or_else(|| detect_work_intent(&command.argument));
        let artifacts = command.artifacts;
        match command.kind {
            SlashCommandKind::PlanSteps => {
                let plan = self.generate_step_plan(
                    &command.argument,
                    &profile,
                    &style,
                    intent,
                    &artifacts,
                )?;
                let path = save_step_plan(self.cwd, &plan).map_err(|err| err.to_string())?;
                Ok(format!(
                    "created step plan: {}",
                    display_path(self.cwd, &path)
                ))
            }
            SlashCommandKind::PlanRun => {
                let plan = self.generate_step_plan(
                    &command.argument,
                    &profile,
                    &style,
                    intent,
                    &artifacts,
                )?;
                let path = save_step_plan(self.cwd, &plan).map_err(|err| err.to_string())?;
                let report = self.execute_step_plan(&plan)?;
                Ok(format!(
                    "created step plan: {}\n{}",
                    display_path(self.cwd, &path),
                    report
                ))
            }
            SlashCommandKind::RunPlan => {
                let plan = load_step_plan(self.cwd, &command.argument)?;
                self.execute_step_plan(&plan)
            }
            SlashCommandKind::UltraPlan => {
                let plan = self.generate_ultra_plan(
                    &command.argument,
                    &profile,
                    &style,
                    intent,
                    &artifacts,
                )?;
                let path = save_ultra_plan(self.cwd, &plan).map_err(|err| err.to_string())?;
                Ok(format!(
                    "created ultra plan: {}",
                    display_path(self.cwd, &path)
                ))
            }
            SlashCommandKind::UltraPlanRun => {
                let plan = self.generate_ultra_plan(
                    &command.argument,
                    &profile,
                    &style,
                    intent,
                    &artifacts,
                )?;
                let path = save_ultra_plan(self.cwd, &plan).map_err(|err| err.to_string())?;
                let report = self.execute_ultra_plan(&plan)?;
                Ok(format!(
                    "created ultra plan: {}\n{}",
                    display_path(self.cwd, &path),
                    report
                ))
            }
            SlashCommandKind::RunUltraPlan => {
                let plan = load_ultra_plan(self.cwd, &command.argument)?;
                self.execute_ultra_plan(&plan)
            }
        }
    }

    fn generate_step_plan(
        &mut self,
        goal: &str,
        profile: &str,
        style: &str,
        intent: WorkIntent,
        required_artifacts: &[String],
    ) -> Result<StepPlan, String> {
        let prompt = plan_generation_prompt(goal, profile, style, intent, required_artifacts);
        let text = planner_text(self.planner, &self.planner_config, &prompt)?;
        match parse_generated_step_plan(
            self.cwd,
            &text,
            goal,
            profile,
            style,
            intent,
            required_artifacts,
        ) {
            Ok(plan) => Ok(plan),
            Err(err) => {
                let _ = save_invalid_generated_plan(self.cwd, "step-plan", &text);
                let correction = plan_correction_prompt(goal, &text, &err, "step plan");
                let corrected = planner_text(self.planner, &self.planner_config, &correction)?;
                parse_generated_step_plan(
                    self.cwd,
                    &corrected,
                    goal,
                    profile,
                    style,
                    intent,
                    required_artifacts,
                )
            }
        }
    }

    fn generate_ultra_plan(
        &mut self,
        goal: &str,
        profile: &str,
        style: &str,
        intent: WorkIntent,
        required_artifacts: &[String],
    ) -> Result<UltraPlan, String> {
        let prompt = ultra_plan_generation_prompt(goal, profile, style, intent.as_str());
        let text = planner_text(self.planner, &self.planner_config, &prompt)?;
        match parse_generated_ultra_plan(&text, goal, profile, style, intent, required_artifacts) {
            Ok(plan) => Ok(plan),
            Err(err) => {
                let _ = save_invalid_generated_plan(self.cwd, "ultra-plan", &text);
                let correction =
                    plan_correction_prompt(goal, &text, &err.to_string(), "ultra plan");
                let corrected = planner_text(self.planner, &self.planner_config, &correction)?;
                parse_generated_ultra_plan(
                    &corrected,
                    goal,
                    profile,
                    style,
                    intent,
                    required_artifacts,
                )
            }
        }
    }

    fn execute_ultra_plan(&mut self, plan: &UltraPlan) -> Result<String, String> {
        let profile_contract =
            profile_contract_text(&plan.profile).map_err(|err| err.to_string())?;
        let snapshot = crate::agent::step_runner::ultra_run::workspace_snapshot(self.cwd);
        let mut lines = Vec::new();
        lines.push(format!("ultra plan: {} phases", plan.phases.len()));

        for (idx, phase) in plan.phases.iter().enumerate() {
            lines.push(format!(
                "phase {}/{} {}: planning",
                idx + 1,
                plan.phases.len(),
                phase.id
            ));
            let prompt = phase_step_plan_prompt(plan, phase, &snapshot, &profile_contract);
            let text = planner_text(self.planner, &self.planner_config, &prompt)?;
            let step_plan = match parse_generated_step_plan(
                self.cwd,
                &text,
                &phase.goal,
                &plan.profile,
                &plan.style,
                WorkIntent::parse(&plan.intent).unwrap_or(WorkIntent::Unknown),
                &plan.required_artifacts,
            ) {
                Ok(plan) => plan,
                Err(err) => {
                    let _ = save_invalid_generated_plan(self.cwd, "phase-step-plan", &text);
                    let correction =
                        plan_correction_prompt(&phase.goal, &text, &err, "phase step plan");
                    let corrected = planner_text(self.planner, &self.planner_config, &correction)?;
                    parse_generated_step_plan(
                        self.cwd,
                        &corrected,
                        &phase.goal,
                        &plan.profile,
                        &plan.style,
                        WorkIntent::parse(&plan.intent).unwrap_or(WorkIntent::Unknown),
                        &plan.required_artifacts,
                    )?
                }
            };
            let path = save_step_plan(self.cwd, &step_plan).map_err(|err| err.to_string())?;
            lines.push(format!(
                "phase {}: step plan {}",
                phase.id,
                display_path(self.cwd, &path)
            ));
            let report = self.execute_step_plan(&step_plan)?;
            lines.push(format!("phase {}: ok\n{}", phase.id, report));
        }

        let missing = missing_paths(self.cwd, &plan.required_artifacts);
        if !missing.is_empty() {
            return Err(format!(
                "missing required final artifacts: {}",
                missing.join(", ")
            ));
        }

        Ok(lines.join("\n"))
    }

    fn execute_step_plan(&mut self, plan: &StepPlan) -> Result<String, String> {
        let mut lines = Vec::new();
        lines.push(format!("step plan: {} steps", plan.steps.len()));
        for (idx, step) in plan.steps.iter().enumerate() {
            lines.push(format!(
                "step {}/{} {}: running",
                idx + 1,
                plan.steps.len(),
                step.id
            ));
            self.execute_step(plan, step)?;
            lines.push(format!("step {}: ok", step.id));
        }
        let missing = missing_paths(self.cwd, &plan.required_artifacts);
        if !missing.is_empty() {
            return Err(format!(
                "missing required final artifacts: {}",
                missing.join(", ")
            ));
        }
        Ok(lines.join("\n"))
    }

    fn execute_step(&mut self, plan: &StepPlan, step: &StepPlanStep) -> Result<(), String> {
        let mut config = self.loop_config.clone();
        if step.expected_result == ExpectedResult::Pass {
            config.expected_artifacts = step.expected_paths.clone();
        }
        let prompt = step_prompt(plan, step)?;
        let result = match run_session(self.executor, self.cwd, &prompt, config.clone()) {
            Ok(result) => result,
            Err(err) => {
                let failures = verify_step(self.cwd, step)?;
                if failures.is_empty() || step_accepts_verifier_failure(step) {
                    return Ok(());
                }
                return self.repair_step_after_turn_error(
                    plan,
                    step,
                    config,
                    err.to_string(),
                    failures,
                );
            }
        };
        let failures = verify_step(self.cwd, step)?;
        if failures.is_empty() || step_accepts_verifier_failure(step) {
            return Ok(());
        }

        self.repair_step(plan, step, config, result, failures)
    }

    fn repair_step_after_turn_error(
        &mut self,
        plan: &StepPlan,
        step: &StepPlanStep,
        config: MinimalLoopConfig,
        turn_error: String,
        failures: Vec<VerificationFailure>,
    ) -> Result<(), String> {
        self.repair_step_with_state(
            plan,
            step,
            config,
            failures,
            Vec::new(),
            0,
            Some(turn_error),
        )
    }

    fn repair_step(
        &mut self,
        plan: &StepPlan,
        step: &StepPlanStep,
        config: MinimalLoopConfig,
        first_result: RunResult,
        failures: Vec<VerificationFailure>,
    ) -> Result<(), String> {
        self.repair_step_with_state(
            plan,
            step,
            config,
            failures,
            changed_file_markers(&first_result),
            usize::from(result_changed_files(&first_result)),
            None,
        )
    }

    fn repair_step_with_state(
        &mut self,
        plan: &StepPlan,
        step: &StepPlanStep,
        config: MinimalLoopConfig,
        mut failures: Vec<VerificationFailure>,
        mut changed_files: Vec<String>,
        mut file_changing_attempts: usize,
        initial_turn_error: Option<String>,
    ) -> Result<(), String> {
        let budget = RepairBudget::default();
        let mut repair_turns = 0usize;

        while budget.allows_next_attempt(file_changing_attempts) && repair_turns < MAX_REPAIR_TURNS
        {
            repair_turns += 1;
            let context = RepairContext {
                step_id: step.id.clone(),
                original_goal: plan.goal.clone(),
                profile: plan.profile.clone(),
                style: plan.style.clone(),
                step_instruction: step.instruction.clone(),
                verification_failures: failures.clone(),
                missing_expected_paths: missing_paths(self.cwd, &step.expected_paths),
                changed_files: changed_files.clone(),
            };
            let prompt = build_repair_prompt(&context);
            let result = match run_session(self.executor, self.cwd, &prompt, config.clone()) {
                Ok(result) => result,
                Err(err) => {
                    failures.push(turn_error_failure("repair turn", err.to_string()));
                    break;
                }
            };
            if result_changed_files(&result) {
                file_changing_attempts += 1;
            }
            changed_files.extend(changed_file_markers(&result));
            failures = verify_step(self.cwd, step)?;
            if failures.is_empty() {
                return Ok(());
            }
        }

        let context = RepairContext {
            step_id: step.id.clone(),
            original_goal: plan.goal.clone(),
            profile: plan.profile.clone(),
            style: plan.style.clone(),
            step_instruction: step.instruction.clone(),
            verification_failures: failures,
            missing_expected_paths: missing_paths(self.cwd, &step.expected_paths),
            changed_files,
        };
        let saved = save_repair_prompt(self.cwd, &context).map_err(|err| err.to_string())?;
        let initial = initial_turn_error
            .map(|err| format!("initial turn error: {err}\n"))
            .unwrap_or_default();
        Err(format!(
            "{initial}step {} failed verification; repair prompt saved: {}\nsuggested command: {}",
            step.id, saved.relative_path, saved.suggested_command
        ))
    }
}

fn step_accepts_verifier_failure(step: &StepPlanStep) -> bool {
    matches!(
        step.expected_result,
        ExpectedResult::Fail | ExpectedResult::Unavailable
    )
}

fn turn_error_failure(command: &str, error: String) -> VerificationFailure {
    VerificationFailure {
        command: command.to_string(),
        reason: "turn_error".to_string(),
        stdout_excerpt: String::new(),
        stderr_excerpt: String::new(),
        diagnostic_excerpt: error,
        source_excerpt: None,
    }
}

fn planner_text<C>(
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

fn parse_generated_step_plan(
    cwd: &Path,
    text: &str,
    goal: &str,
    profile: &str,
    style: &str,
    intent: WorkIntent,
    required_artifacts: &[String],
) -> Result<StepPlan, String> {
    let normalized =
        ensure_generated_plan_header(text, goal, profile, style, intent, required_artifacts);
    let plan = extract_plan_from_response(&normalized).map_err(|err| err.to_string())?;
    lint_step_plan_with_workspace(&plan, Some(cwd))
        .map_err(|err| format!("plan lint failed: {err}"))?;
    Ok(plan)
}

fn parse_generated_ultra_plan(
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
    plan.required_artifacts = required_artifacts.to_vec();
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

fn save_invalid_generated_plan(cwd: &Path, kind: &str, text: &str) -> Result<(), String> {
    let dir = crate::util::workspace_paths::plans_dir(cwd);
    fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    let path = dir.join(format!("invalid-{kind}-{}.yaml", now_ms()));
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

fn plan_correction_prompt(
    original_goal: &str,
    invalid_plan: &str,
    error: &str,
    plan_kind: &str,
) -> String {
    format!(
        "The generated CommandAgent {plan_kind} is invalid and must be corrected.\n\
Original goal:\n{original_goal}\n\n\
Validation error:\n{error}\n\n\
Invalid plan:\n{invalid_plan}\n\n\
If the error mentions shell scaffolding, replace that step with explicit file creation or editing instructions that can be completed with Write/Edit.\n\
If the error mentions action/path/content/old/new fields, rewrite those tool-call fields into step instruction and expected_paths fields.\n\
Return only corrected YAML using the required CommandAgent schema."
    )
}

fn step_prompt(plan: &StepPlan, step: &StepPlanStep) -> Result<String, String> {
    let profile_contract = profile_contract_text(&plan.profile).map_err(|err| err.to_string())?;
    Ok(format!(
        "Run one CommandAgent step.\n\
Overall goal: {goal}\n\
Profile: {profile}\n\
Style: {style}\n\
Intent: {intent}\n\
Required final artifacts:\n{artifacts}\n\
Profile contract:\n{profile_contract}\n\n\
Step id: {step_id}\n\
Step kind: {kind}\n\
Step instruction: {instruction}\n\
Expected result: {expected_result}\n\
Expected paths:\n{expected}\n\
Verifier commands:\n{verify}\n\n\
Do only this step. Use Write/Edit for file changes; Write creates parent directories automatically.\n\
The runtime executes verifier commands after your response. Do not run listed verifier commands yourself unless the step kind is verify and the command is a single allowed local check.\n\
Do not use compound Bash commands with &&, ||, or ;.\n\
Do not install network dependencies unless the step explicitly asks for dependency setup and the environment allows it.",
        goal = plan.goal,
        profile = plan.profile,
        style = plan.style,
        intent = plan.intent.as_str(),
        artifacts = bullet_list(&plan.required_artifacts),
        step_id = step.id,
        kind = step.kind.as_str(),
        instruction = step.instruction,
        expected_result = step.expected_result.as_str(),
        expected = bullet_list(&step.expected_paths),
        verify = bullet_list(&step.verify),
    ))
}

fn verify_step(cwd: &Path, step: &StepPlanStep) -> Result<Vec<VerificationFailure>, String> {
    let commands = if step.verify.is_empty() {
        Vec::new()
    } else {
        step.verify.clone()
    };
    let report = run_verifiers(cwd, &commands).map_err(|err| err.to_string())?;
    Ok(report.failures)
}

fn load_step_plan(cwd: &Path, path: &str) -> Result<StepPlan, String> {
    let guard = PathGuard::new(cwd).map_err(|err| err.to_string())?;
    let path = guard.resolve(path).map_err(|err| err.to_string())?;
    let text = fs::read_to_string(&path).map_err(|err| format!("{}: {err}", path.display()))?;
    let plan = parse_step_plan_yaml(&text).map_err(|err| err.to_string())?;
    lint_step_plan_with_workspace(&plan, Some(cwd))
        .map_err(|err| format!("plan lint failed: {err}"))?;
    Ok(plan)
}

fn load_ultra_plan(cwd: &Path, path: &str) -> Result<UltraPlan, String> {
    let guard = PathGuard::new(cwd).map_err(|err| err.to_string())?;
    let path = guard.resolve(path).map_err(|err| err.to_string())?;
    let text = fs::read_to_string(&path).map_err(|err| format!("{}: {err}", path.display()))?;
    parse_ultra_plan_yaml(&text).map_err(|err| err.to_string())
}

fn missing_paths(cwd: &Path, paths: &[String]) -> Vec<String> {
    paths
        .iter()
        .filter(|path| !cwd.join(path).exists())
        .cloned()
        .collect()
}

fn result_changed_files(result: &RunResult) -> bool {
    result
        .tool_results
        .iter()
        .any(|record| record.ok && is_file_change_tool(&record.name))
}

fn changed_file_markers(result: &RunResult) -> Vec<String> {
    result
        .tool_results
        .iter()
        .filter(|record| record.ok && is_file_change_tool(&record.name))
        .map(|record| record.name.clone())
        .collect()
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

fn display_path(cwd: &Path, path: &Path) -> String {
    path.strip_prefix(cwd).unwrap_or(path).display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::{ChatResponse, ToolCall};
    use std::collections::VecDeque;
    use std::path::PathBuf;

    #[test]
    fn plan_steps_generates_and_saves_plan() {
        let root = temp_workspace("plan-steps");
        let plan_yaml = "goal: \"Create docs\"\nprofile: \"docs\"\nstyle: \"default\"\nsteps:\n  - id: \"write-readme\"\n    instruction: \"Create README.md.\"\n    expected_paths:\n      - \"README.md\"\n    verify:\n      - \"cat README.md\"\n";
        let mut planner = MockClient::new(vec![ChatResponse {
            content: plan_yaml.to_string(),
            tool_calls: Vec::new(),
        }]);
        let mut executor = MockClient::new(vec![]);
        let command = SlashCommand {
            kind: SlashCommandKind::PlanSteps,
            profile: Some("docs".to_string()),
            style: None,
            intent: None,
            artifacts: Vec::new(),
            argument: "Create docs".to_string(),
        };

        let output = SlashRuntime {
            executor: &mut executor,
            planner: &mut planner,
            cwd: &root,
            loop_config: MinimalLoopConfig::default(),
            planner_config: PlannerRuntimeConfig {
                model: "planner".to_string(),
                tool_call_mode: ToolCallMode::XmlFallback,
            },
        }
        .run(command)
        .unwrap();

        assert!(output.contains("created step plan"));
        assert!(root.join(".commandagent/plans").exists());
    }

    #[test]
    fn plan_run_executes_step_and_verifier() {
        let root = temp_workspace("plan-run");
        let plan_yaml = "goal: \"Create docs\"\nprofile: \"docs\"\nstyle: \"default\"\nsteps:\n  - id: \"write-readme\"\n    instruction: \"Create README.md.\"\n    expected_paths:\n      - \"README.md\"\n    verify:\n      - \"cat README.md\"\n";
        let mut planner = MockClient::new(vec![ChatResponse {
            content: plan_yaml.to_string(),
            tool_calls: Vec::new(),
        }]);
        let mut executor = MockClient::new(vec![
            ChatResponse {
                content: String::new(),
                tool_calls: vec![ToolCall {
                    name: "Write".to_string(),
                    args_json: r#"{"path":"README.md","content":"ok"}"#.to_string(),
                }],
            },
            ChatResponse {
                content: "Created README.md.".to_string(),
                tool_calls: Vec::new(),
            },
        ]);
        let command = SlashCommand {
            kind: SlashCommandKind::PlanRun,
            profile: Some("docs".to_string()),
            style: None,
            intent: None,
            artifacts: Vec::new(),
            argument: "Create docs".to_string(),
        };

        let output = SlashRuntime {
            executor: &mut executor,
            planner: &mut planner,
            cwd: &root,
            loop_config: MinimalLoopConfig::default(),
            planner_config: PlannerRuntimeConfig {
                model: "planner".to_string(),
                tool_call_mode: ToolCallMode::XmlFallback,
            },
        }
        .run(command)
        .unwrap();

        assert!(output.contains("step write-readme: ok"));
        assert_eq!(fs::read_to_string(root.join("README.md")).unwrap(), "ok");
    }

    #[test]
    fn plan_run_accepts_verified_step_after_max_iterations() {
        let root = temp_workspace("plan-run-verified-max");
        let plan_yaml = "goal: \"Create docs\"\nprofile: \"docs\"\nstyle: \"default\"\nsteps:\n  - id: \"write-readme\"\n    instruction: \"Create README.md.\"\n    expected_paths:\n      - \"README.md\"\n    verify:\n      - \"cat README.md\"\n";
        let mut planner = MockClient::new(vec![ChatResponse {
            content: plan_yaml.to_string(),
            tool_calls: Vec::new(),
        }]);
        let mut executor = MockClient::new(vec![ChatResponse {
            content: String::new(),
            tool_calls: vec![ToolCall {
                name: "Write".to_string(),
                args_json: r#"{"path":"README.md","content":"ok"}"#.to_string(),
            }],
        }]);
        let command = SlashCommand {
            kind: SlashCommandKind::PlanRun,
            profile: Some("docs".to_string()),
            style: None,
            intent: None,
            artifacts: Vec::new(),
            argument: "Create docs".to_string(),
        };
        let mut loop_config = MinimalLoopConfig::default();
        loop_config.max_iterations = 1;

        let output = SlashRuntime {
            executor: &mut executor,
            planner: &mut planner,
            cwd: &root,
            loop_config,
            planner_config: PlannerRuntimeConfig {
                model: "planner".to_string(),
                tool_call_mode: ToolCallMode::XmlFallback,
            },
        }
        .run(command)
        .unwrap();

        assert!(output.contains("step write-readme: ok"));
        assert_eq!(fs::read_to_string(root.join("README.md")).unwrap(), "ok");
    }

    #[test]
    fn plan_run_saves_repair_prompt_after_initial_turn_error() {
        let root = temp_workspace("plan-run-repair-after-error");
        fs::write(root.join("README.md"), "fixture").unwrap();
        let plan_yaml = "goal: \"Verify docs\"\nprofile: \"docs\"\nstyle: \"default\"\nsteps:\n  - id: \"verify-readme\"\n    instruction: \"Inspect README.md.\"\n    expected_paths:\n      - \"README.md\"\n    verify:\n      - \"grep -q __missing_marker__ /dev/null\"\n";
        let mut planner = MockClient::new(vec![ChatResponse {
            content: plan_yaml.to_string(),
            tool_calls: Vec::new(),
        }]);
        let mut executor = MockClient::new(vec![ChatResponse {
            content: "Let me verify README.md.".to_string(),
            tool_calls: Vec::new(),
        }]);
        let command = SlashCommand {
            kind: SlashCommandKind::PlanRun,
            profile: Some("docs".to_string()),
            style: None,
            intent: None,
            artifacts: Vec::new(),
            argument: "Verify docs".to_string(),
        };
        let mut loop_config = MinimalLoopConfig::default();
        loop_config.max_iterations = 1;

        let err = SlashRuntime {
            executor: &mut executor,
            planner: &mut planner,
            cwd: &root,
            loop_config,
            planner_config: PlannerRuntimeConfig {
                model: "planner".to_string(),
                tool_call_mode: ToolCallMode::XmlFallback,
            },
        }
        .run(command)
        .unwrap_err();

        assert!(err.contains("initial turn error"));
        assert!(err.contains("repair prompt saved"));
        assert!(root.join(".commandagent/repairs").exists());
    }

    #[test]
    fn invalid_step_plan_gets_one_correction_attempt() {
        let root = temp_workspace("plan-correction");
        let invalid_yaml = "goal: \"Create docs\"\nprofile: \"docs\"\nstyle: \"default\"\nsteps:\n- id: \"write-readme\"\n  instruction: \"Create README.md.\"\n  expected_paths:\n    - \"README.md\"\n  verify:\n    - \"cat README.md\"\n";
        let corrected_yaml = "goal: \"Create docs\"\nprofile: \"docs\"\nstyle: \"default\"\nsteps:\n  - id: \"write-readme\"\n    instruction: \"Create README.md.\"\n    expected_paths:\n      - \"README.md\"\n    verify:\n      - \"cat README.md\"\n";
        let mut planner = MockClient::new(vec![
            ChatResponse {
                content: invalid_yaml.to_string(),
                tool_calls: Vec::new(),
            },
            ChatResponse {
                content: corrected_yaml.to_string(),
                tool_calls: Vec::new(),
            },
        ]);
        let mut executor = MockClient::new(vec![]);
        let command = SlashCommand {
            kind: SlashCommandKind::PlanSteps,
            profile: Some("docs".to_string()),
            style: None,
            intent: None,
            artifacts: Vec::new(),
            argument: "Create docs".to_string(),
        };

        let output = SlashRuntime {
            executor: &mut executor,
            planner: &mut planner,
            cwd: &root,
            loop_config: MinimalLoopConfig::default(),
            planner_config: PlannerRuntimeConfig {
                model: "planner".to_string(),
                tool_call_mode: ToolCallMode::XmlFallback,
            },
        }
        .run(command)
        .unwrap();

        assert!(output.contains("created step plan"));
    }

    #[test]
    fn generated_step_plan_can_omit_known_header_fields() {
        let text = "steps:\n  - id: \"write-readme\"\n    instruction: \"Create README.md.\"\n    expected_paths:\n      - \"README.md\"\n    verify: []\n";

        let root = temp_workspace("parse-generated-header");
        let plan = parse_generated_step_plan(
            &root,
            text,
            "Create docs",
            "docs",
            "default",
            WorkIntent::New,
            &[],
        )
        .unwrap();

        assert_eq!(plan.goal, "Create docs");
        assert_eq!(plan.profile, "docs");
        assert_eq!(plan.style, "default");
        assert_eq!(plan.intent, WorkIntent::New);
        assert!(plan.required_artifacts.is_empty());
        assert_eq!(plan.steps[0].id, "write-readme");
    }

    #[test]
    fn generated_step_plan_uses_requested_profile_over_model_profile() {
        let text = "goal: \"Create Rust CLI\"\nprofile: \"rust cli developer\"\nstyle: \"careful\"\nsteps:\n  - id: \"write-main\"\n    instruction: \"Create src/main.rs.\"\n    expected_paths:\n      - \"src/main.rs\"\n    verify: []\n";

        let root = temp_workspace("parse-generated-profile");
        let plan = parse_generated_step_plan(
            &root,
            text,
            "Create Rust CLI",
            "rust",
            "default",
            WorkIntent::New,
            &[],
        )
        .unwrap();

        assert_eq!(plan.goal, "Create Rust CLI");
        assert_eq!(plan.profile, "rust");
        assert_eq!(plan.style, "default");
    }

    #[test]
    fn generated_step_plan_uses_requested_artifacts_over_model_artifacts() {
        let root = temp_workspace("parse-generated-artifacts");
        let text = "required_artifacts:\n  - wrong/path.txt\nsteps:\n  - id: \"write-readme\"\n    instruction: \"Create README.md.\"\n    expected_paths:\n      - \"README.md\"\n    verify: []\n";

        let plan = parse_generated_step_plan(
            &root,
            text,
            "Create docs",
            "docs",
            "default",
            WorkIntent::Document,
            &["README.md".to_string()],
        )
        .unwrap();

        assert_eq!(plan.intent, WorkIntent::Document);
        assert_eq!(plan.required_artifacts, vec!["README.md"]);
    }

    #[test]
    fn invalid_step_plan_is_saved_before_correction() {
        let root = temp_workspace("invalid-plan-saved");
        let invalid_yaml = "goal: \"Create docs\"\nprofile: \"docs\"\nstyle: \"default\"\nsteps:\n  - id: \"bad\"\n    instruction: \"Create README.md.\"\n    expected_paths:\n      - \"README.md or README.txt\"\n    verify:\n      - \"cat README.md\"\n";
        let corrected_yaml = "goal: \"Create docs\"\nprofile: \"docs\"\nstyle: \"default\"\nsteps:\n  - id: \"write-readme\"\n    instruction: \"Create README.md.\"\n    expected_paths:\n      - \"README.md\"\n    verify:\n      - \"cat README.md\"\n";
        let mut planner = MockClient::new(vec![
            ChatResponse {
                content: invalid_yaml.to_string(),
                tool_calls: Vec::new(),
            },
            ChatResponse {
                content: corrected_yaml.to_string(),
                tool_calls: Vec::new(),
            },
        ]);
        let mut executor = MockClient::new(vec![]);
        let command = SlashCommand {
            kind: SlashCommandKind::PlanSteps,
            profile: Some("docs".to_string()),
            style: None,
            intent: None,
            artifacts: Vec::new(),
            argument: "Create docs".to_string(),
        };

        SlashRuntime {
            executor: &mut executor,
            planner: &mut planner,
            cwd: &root,
            loop_config: MinimalLoopConfig::default(),
            planner_config: PlannerRuntimeConfig {
                model: "planner".to_string(),
                tool_call_mode: ToolCallMode::XmlFallback,
            },
        }
        .run(command)
        .unwrap();

        let invalid_count = fs::read_dir(root.join(".commandagent/plans"))
            .unwrap()
            .flatten()
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("invalid-step-plan-")
            })
            .count();
        assert_eq!(invalid_count, 1);
    }

    struct MockClient {
        responses: VecDeque<ChatResponse>,
    }

    impl MockClient {
        fn new(responses: Vec<ChatResponse>) -> Self {
            Self {
                responses: VecDeque::from(responses),
            }
        }
    }

    impl ChatClient for MockClient {
        fn chat(&mut self, _request: &ChatRequest) -> Result<ChatResponse, String> {
            self.responses
                .pop_front()
                .ok_or_else(|| "missing mock response".to_string())
        }
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "commandagent-slash-runtime-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
}
