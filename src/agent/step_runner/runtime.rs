use crate::agent::minimal_loop::guards::is_file_change_tool;
use crate::agent::minimal_loop::loop_run::{ChatClient, MinimalLoopConfig, RunResult, run_session};
use crate::agent::slash_command::{SlashCommand, SlashCommandKind};
use crate::agent::step_runner::plan_lint::lint_step_plan;
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
    StepPlan, StepPlanStep, extract_plan_from_response, parse_step_plan_yaml,
    plan_generation_prompt, save_step_plan,
};
use crate::providers::{ChatMessage, ChatRequest, ChatRole, ToolCallMode};
use crate::safety::path_guard::PathGuard;
use std::fs;
use std::path::Path;

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
        match command.kind {
            SlashCommandKind::PlanSteps => {
                let plan = self.generate_step_plan(&command.argument, &profile, &style)?;
                let path = save_step_plan(self.cwd, &plan).map_err(|err| err.to_string())?;
                Ok(format!(
                    "created step plan: {}",
                    display_path(self.cwd, &path)
                ))
            }
            SlashCommandKind::PlanRun => {
                let plan = self.generate_step_plan(&command.argument, &profile, &style)?;
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
                let plan = self.generate_ultra_plan(&command.argument, &profile, &style)?;
                let path = save_ultra_plan(self.cwd, &plan).map_err(|err| err.to_string())?;
                Ok(format!(
                    "created ultra plan: {}",
                    display_path(self.cwd, &path)
                ))
            }
            SlashCommandKind::UltraPlanRun => {
                let plan = self.generate_ultra_plan(&command.argument, &profile, &style)?;
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
    ) -> Result<StepPlan, String> {
        let prompt = plan_generation_prompt(goal, profile, style);
        let text = planner_text(self.planner, &self.planner_config, &prompt)?;
        match parse_and_lint_step_plan(&text) {
            Ok(plan) => Ok(plan),
            Err(err) => {
                let correction = plan_correction_prompt(goal, &text, &err, "step plan");
                let corrected = planner_text(self.planner, &self.planner_config, &correction)?;
                parse_and_lint_step_plan(&corrected)
            }
        }
    }

    fn generate_ultra_plan(
        &mut self,
        goal: &str,
        profile: &str,
        style: &str,
    ) -> Result<UltraPlan, String> {
        let prompt = ultra_plan_generation_prompt(goal, profile, style, "new");
        let text = planner_text(self.planner, &self.planner_config, &prompt)?;
        match parse_ultra_plan_yaml(&text) {
            Ok(plan) => Ok(plan),
            Err(err) => {
                let correction =
                    plan_correction_prompt(goal, &text, &err.to_string(), "ultra plan");
                let corrected = planner_text(self.planner, &self.planner_config, &correction)?;
                parse_ultra_plan_yaml(&corrected).map_err(|err| err.to_string())
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
            let step_plan = match parse_and_lint_step_plan(&text) {
                Ok(plan) => plan,
                Err(err) => {
                    let correction =
                        plan_correction_prompt(&phase.goal, &text, &err, "phase step plan");
                    let corrected = planner_text(self.planner, &self.planner_config, &correction)?;
                    parse_and_lint_step_plan(&corrected)?
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
        Ok(lines.join("\n"))
    }

    fn execute_step(&mut self, plan: &StepPlan, step: &StepPlanStep) -> Result<(), String> {
        let mut config = self.loop_config.clone();
        config.expected_artifacts = step.expected_paths.clone();
        let prompt = step_prompt(plan, step)?;
        let result = run_session(self.executor, self.cwd, &prompt, config.clone())
            .map_err(|err| err.to_string())?;
        let failures = verify_step(self.cwd, step)?;
        if failures.is_empty() {
            return Ok(());
        }

        self.repair_step(plan, step, config, result, failures)
    }

    fn repair_step(
        &mut self,
        plan: &StepPlan,
        step: &StepPlanStep,
        config: MinimalLoopConfig,
        first_result: RunResult,
        mut failures: Vec<VerificationFailure>,
    ) -> Result<(), String> {
        let budget = RepairBudget::default();
        let mut file_changing_attempts = usize::from(result_changed_files(&first_result));
        let mut changed_files = changed_file_markers(&first_result);

        while budget.allows_next_attempt(file_changing_attempts) {
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
            let result = run_session(self.executor, self.cwd, &prompt, config.clone())
                .map_err(|err| err.to_string())?;
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
        Err(format!(
            "step {} failed verification; repair prompt saved: {}\nsuggested command: {}",
            step.id, saved.relative_path, saved.suggested_command
        ))
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

fn parse_and_lint_step_plan(text: &str) -> Result<StepPlan, String> {
    let plan = extract_plan_from_response(text).map_err(|err| err.to_string())?;
    lint_step_plan(&plan).map_err(|err| format!("plan lint failed: {err}"))?;
    Ok(plan)
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
Profile contract:\n{profile_contract}\n\n\
Step id: {step_id}\n\
Step instruction: {instruction}\n\
Expected paths:\n{expected}\n\
Verifier commands:\n{verify}\n\n\
Do only this step. Use tools for file changes and local verification.",
        goal = plan.goal,
        profile = plan.profile,
        style = plan.style,
        step_id = step.id,
        instruction = step.instruction,
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
    lint_step_plan(&plan).map_err(|err| format!("plan lint failed: {err}"))?;
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
