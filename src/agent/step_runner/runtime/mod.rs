use crate::agent::events::{
    NoopRuntimeObserver, PlanKind, RuntimeEvent, RuntimeObserver, bounded_event_text,
};
use crate::agent::minimal_loop::loop_run::{ChatClient, MinimalLoopConfig, RunResult};
use crate::agent::minimal_loop::result::MinimalLoopError;
use crate::agent::slash_command::{SlashCommand, SlashCommandKind};
use crate::agent::step_runner::correction_evidence::PlanCorrectionEvidence;
use crate::agent::step_runner::plan_prompt::plan_generation_prompt_with_task_contract;
use crate::agent::step_runner::ultra_plan::{
    UltraPlan, save_ultra_plan, ultra_plan_generation_prompt,
};
use crate::agent::step_runner::verify::VerificationFailure;
use crate::agent::step_runner::{StepPlan, WorkIntent, detect_work_intent, save_step_plan};
use crate::providers::ToolCallMode;
use std::path::Path;

mod dev_server;
mod execution;
mod paths;
pub(crate) mod phase_contract;
mod planning;
mod prompts;
mod repair_loop;
mod setup;

use execution::{load_step_plan, load_ultra_plan};
use paths::display_path;
use planning::{
    GeneratedStepPlanContext, StepPlanCorrectionContext, parse_generated_step_plan,
    parse_generated_ultra_plan, planner_text, save_invalid_generated_plan,
};
use prompts::plan_correction_prompt;
use repair_loop::{RepairStepRequest, RepairStepState};

const MAX_INVALID_PLAN_CORRECTIONS: usize = 2;

fn attach_plan_correction_ledger(
    evidence: &mut Option<Box<PlanCorrectionEvidence>>,
    attempt: usize,
) {
    let Some(evidence) = evidence.as_mut() else {
        return;
    };
    let active_job = evidence.active_job.as_deref().unwrap_or("unknown");
    let target = evidence
        .repair_target
        .as_deref()
        .or(evidence.target_path.as_deref())
        .or(evidence.target_field.as_deref())
        .unwrap_or("unknown");
    let missing = if evidence.missing_literals.is_empty() {
        "none".to_string()
    } else {
        evidence.missing_literals.join(", ")
    };
    let attempt_number = attempt + 1;
    evidence.repair_attempt_ledger.push(format!(
        "plan correction attempt {attempt_number}: active_job={active_job}; target={target}; missing_literals={missing}; result=lint_rejected"
    ));
    if attempt == MAX_INVALID_PLAN_CORRECTIONS {
        evidence.reason_code = Some("plan_correction_no_progress_or_exhausted".to_string());
        evidence.failure_kind = Some("bounded_plan_correction_exhausted".to_string());
    }
}

fn final_plan_correction_error(
    message: String,
    evidence: &Option<Box<PlanCorrectionEvidence>>,
) -> String {
    let Some(evidence) = evidence.as_ref() else {
        return message;
    };
    let Some(rendered) = evidence.render() else {
        return message;
    };
    format!("{message}\n{rendered}")
}

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
        let mut observer = NoopRuntimeObserver;
        self.run_with_observer(command, &mut observer)
    }

    pub fn run_with_observer(
        &mut self,
        command: SlashCommand,
        observer: &mut dyn RuntimeObserver,
    ) -> Result<String, String> {
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
                    observer,
                )?;
                let path = save_step_plan(self.cwd, &plan).map_err(|err| err.to_string())?;
                observer.on_event(RuntimeEvent::PlanSaved {
                    kind: PlanKind::StepPlan,
                    path: display_path(self.cwd, &path),
                    item_ids: step_ids(&plan),
                });
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
                    observer,
                )?;
                let path = save_step_plan(self.cwd, &plan).map_err(|err| err.to_string())?;
                observer.on_event(RuntimeEvent::PlanSaved {
                    kind: PlanKind::StepPlan,
                    path: display_path(self.cwd, &path),
                    item_ids: step_ids(&plan),
                });
                let report = self.execute_step_plan(&plan, observer)?;
                Ok(format!(
                    "created step plan: {}\n{}",
                    display_path(self.cwd, &path),
                    report
                ))
            }
            SlashCommandKind::RunPlan => {
                let plan = load_step_plan(self.cwd, &command.argument)?;
                self.execute_step_plan(&plan, observer)
            }
            SlashCommandKind::UltraPlan => {
                let plan = self.generate_ultra_plan(
                    &command.argument,
                    &profile,
                    &style,
                    intent,
                    &artifacts,
                    observer,
                )?;
                let path = save_ultra_plan(self.cwd, &plan).map_err(|err| err.to_string())?;
                observer.on_event(RuntimeEvent::PlanSaved {
                    kind: PlanKind::UltraPlan,
                    path: display_path(self.cwd, &path),
                    item_ids: phase_ids(&plan),
                });
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
                    observer,
                )?;
                let path = save_ultra_plan(self.cwd, &plan).map_err(|err| err.to_string())?;
                observer.on_event(RuntimeEvent::PlanSaved {
                    kind: PlanKind::UltraPlan,
                    path: display_path(self.cwd, &path),
                    item_ids: phase_ids(&plan),
                });
                let report = self.execute_ultra_plan(&plan, observer)?;
                Ok(format!(
                    "created ultra plan: {}\n{}",
                    display_path(self.cwd, &path),
                    report
                ))
            }
            SlashCommandKind::RunUltraPlan => {
                let plan = load_ultra_plan(self.cwd, &command.argument)?;
                self.execute_ultra_plan(&plan, observer)
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
        observer: &mut dyn RuntimeObserver,
    ) -> Result<StepPlan, String> {
        observer.on_event(RuntimeEvent::PlanGenerationStarted {
            kind: PlanKind::StepPlan,
            goal: bounded_event_text(goal),
            profile: bounded_event_text(profile),
        });
        let phase_contract = phase_contract::PhaseWorkspaceContract::collect_with_goal(
            self.cwd,
            profile,
            required_artifacts,
            goal,
        );
        let prompt = plan_generation_prompt_with_task_contract(
            goal,
            profile,
            style,
            intent,
            required_artifacts,
            Some(&phase_contract.task_contract),
        );
        let text = planner_text(self.planner, &self.planner_config, &prompt)?;
        let correction_context = StepPlanCorrectionContext {
            goal,
            profile,
            style,
            intent,
            required_artifacts,
            profile_obligations: &phase_contract.profile_obligations,
            task_contract: Some(&phase_contract.task_contract),
            save_kind: "step-plan",
            prompt_kind: "step plan",
        };
        let plan = self.parse_generated_step_plan_with_corrections(text, correction_context)?;
        observer.on_event(RuntimeEvent::PlanGenerationFinished {
            kind: PlanKind::StepPlan,
            item_count: plan.steps.len(),
        });
        Ok(plan)
    }

    fn parse_generated_step_plan_with_corrections(
        &mut self,
        initial_text: String,
        context: StepPlanCorrectionContext<'_>,
    ) -> Result<StepPlan, String> {
        let mut text = initial_text;
        for attempt in 0..=MAX_INVALID_PLAN_CORRECTIONS {
            let generated_context = GeneratedStepPlanContext {
                goal: context.goal,
                profile: context.profile,
                style: context.style,
                intent: context.intent,
                required_artifacts: context.required_artifacts,
                profile_obligations: context.profile_obligations,
                task_contract: context.task_contract,
            };
            match parse_generated_step_plan(self.cwd, &text, &generated_context) {
                Ok(plan) => return Ok(plan),
                Err(mut err) => {
                    attach_plan_correction_ledger(&mut err.correction_evidence, attempt);
                    let _ = save_invalid_generated_plan(self.cwd, context.save_kind, &text);
                    if attempt == MAX_INVALID_PLAN_CORRECTIONS {
                        return Err(final_plan_correction_error(
                            err.message,
                            &err.correction_evidence,
                        ));
                    }
                    let correction = plan_correction_prompt(
                        context.goal,
                        &text,
                        &err.message,
                        context.prompt_kind,
                        err.correction_evidence.as_deref(),
                    );
                    text = planner_text(self.planner, &self.planner_config, &correction)?;
                }
            }
        }
        unreachable!("bounded invalid plan correction loop must return");
    }

    fn generate_ultra_plan(
        &mut self,
        goal: &str,
        profile: &str,
        style: &str,
        intent: WorkIntent,
        required_artifacts: &[String],
        observer: &mut dyn RuntimeObserver,
    ) -> Result<UltraPlan, String> {
        observer.on_event(RuntimeEvent::PlanGenerationStarted {
            kind: PlanKind::UltraPlan,
            goal: bounded_event_text(goal),
            profile: bounded_event_text(profile),
        });
        let prompt = ultra_plan_generation_prompt(goal, profile, style, intent.as_str());
        let text = planner_text(self.planner, &self.planner_config, &prompt)?;
        let plan = match parse_generated_ultra_plan(
            &text,
            goal,
            profile,
            style,
            intent,
            required_artifacts,
        ) {
            Ok(plan) => Ok(plan),
            Err(err) => {
                let _ = save_invalid_generated_plan(self.cwd, "ultra-plan", &text);
                let correction =
                    plan_correction_prompt(goal, &text, &err.to_string(), "ultra plan", None);
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
        }?;
        observer.on_event(RuntimeEvent::PlanGenerationFinished {
            kind: PlanKind::UltraPlan,
            item_count: plan.phases.len(),
        });
        Ok(plan)
    }

    fn execute_ultra_plan(
        &mut self,
        plan: &UltraPlan,
        observer: &mut dyn RuntimeObserver,
    ) -> Result<String, String> {
        execution::execute_ultra_plan(self, plan, observer)
    }

    fn execute_step_plan(
        &mut self,
        plan: &StepPlan,
        observer: &mut dyn RuntimeObserver,
    ) -> Result<String, String> {
        let phase_contract = phase_contract::PhaseWorkspaceContract::collect_with_goal(
            self.cwd,
            &plan.profile,
            &plan.required_artifacts,
            &plan.goal,
        );
        let active_contract_seed = phase_contract::ActiveStepContract::from_phase_contract(
            &plan.profile,
            &phase_contract,
            Vec::new(),
        );
        let mut report = execution::execute_step_plan(self, plan, &active_contract_seed, observer)?;
        if let Some(smoke_report) = execution::verify_requested_dev_server_contract(
            self.cwd,
            &plan.profile,
            &plan.goal,
            execution::step_plan_has_nextjs_build_verifier(plan),
        )? {
            report.push('\n');
            report.push_str(&smoke_report);
        }
        Ok(report)
    }

    fn repair_step_after_turn_error(
        &mut self,
        request: RepairStepRequest<'_>,
        turn_error: MinimalLoopError,
        failures: Vec<VerificationFailure>,
        observer: &mut dyn RuntimeObserver,
    ) -> Result<(), String> {
        repair_loop::repair_step_after_turn_error(self, request, turn_error, failures, observer)
    }

    fn repair_step(
        &mut self,
        request: RepairStepRequest<'_>,
        first_result: RunResult,
        failures: Vec<VerificationFailure>,
        observer: &mut dyn RuntimeObserver,
    ) -> Result<(), String> {
        repair_loop::repair_step(self, request, first_result, failures, observer)
    }

    fn repair_step_with_state(
        &mut self,
        request: RepairStepRequest<'_>,
        state: RepairStepState,
        observer: &mut dyn RuntimeObserver,
    ) -> Result<(), String> {
        repair_loop::repair_step_with_state(self, request, state, observer)
    }
}

fn step_ids(plan: &StepPlan) -> Vec<String> {
    plan.steps.iter().map(|step| step.id.clone()).collect()
}

fn phase_ids(plan: &UltraPlan) -> Vec<String> {
    plan.phases.iter().map(|phase| phase.id.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::repair_loop::turn_error_failure;
    use super::*;
    use crate::agent::events::{ArtifactScope, ArtifactStatus, CaptureObserver, RuntimeEvent};
    use crate::agent::minimal_loop::result::ToolArgError;
    use crate::agent::step_runner::profiles::ProfileObligation;
    use crate::agent::step_runner::{ExpectedResult, StepKind, StepPlanStep};
    use crate::providers::{ChatRequest, ChatResponse, ToolCall};
    use std::collections::VecDeque;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn plan_steps_generates_and_saves_plan() {
        let root = temp_workspace("plan-steps");
        let plan_yaml = "goal: \"Create docs\"\nprofile: \"docs\"\nstyle: \"default\"\nsteps:\n  - id: \"write-readme\"\n    kind: \"create\"\n    instruction: \"Create README.md.\"\n    expected_paths:\n      - \"README.md\"\n    verify:\n      - \"cat README.md\"\n";
        let mut planner = MockClient::new(vec![ChatResponse {
            content: plan_yaml.to_string(),
            tool_calls: Vec::new(),

            usage: Default::default(),
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
        let plan_yaml = "goal: \"Create docs\"\nprofile: \"docs\"\nstyle: \"default\"\nsteps:\n  - id: \"write-readme\"\n    kind: \"create\"\n    instruction: \"Create README.md.\"\n    expected_paths:\n      - \"README.md\"\n    verify:\n      - \"cat README.md\"\n";
        let mut planner = MockClient::new(vec![ChatResponse {
            content: plan_yaml.to_string(),
            tool_calls: Vec::new(),

            usage: Default::default(),
        }]);
        let mut executor = MockClient::new(vec![
            ChatResponse {
                content: String::new(),
                tool_calls: vec![ToolCall {
                    id: None,
                    thought_signature: None,
                    name: "Write".to_string(),
                    args_json: r#"{"path":"README.md","content":"ok"}"#.to_string(),
                }],

                usage: Default::default(),
            },
            ChatResponse {
                content: "Created README.md.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
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
    fn plan_run_accepts_completed_step_after_blocked_bash() {
        let root = temp_workspace("plan-run-blocked-bash-completed");
        let plan_yaml = "goal: \"Create docs\"\nprofile: \"docs\"\nstyle: \"default\"\nsteps:\n  - id: \"write-readme\"\n    kind: \"create\"\n    instruction: \"Create README.md with a Usage section.\"\n    expected_paths:\n      - \"README.md\"\n    verify:\n      - \"grep -q Usage README.md\"\n";
        let mut planner = MockClient::new(vec![ChatResponse {
            content: plan_yaml.to_string(),
            tool_calls: Vec::new(),
            usage: Default::default(),
        }]);
        let mut executor = MockClient::new(vec![ChatResponse {
            content: String::new(),
            tool_calls: vec![
                ToolCall {
                    id: None,
                    thought_signature: None,
                    name: "Write".to_string(),
                    args_json:
                        r##"{"path":"README.md","content":"# Demo\n\n## Usage\nRun it.\n"}"##
                            .to_string(),
                },
                ToolCall {
                    id: None,
                    thought_signature: None,
                    name: "Bash".to_string(),
                    args_json: r#"{"command":"cat README.md && true"}"#.to_string(),
                },
            ],
            usage: Default::default(),
        }]);
        let command = SlashCommand {
            kind: SlashCommandKind::PlanRun,
            profile: Some("docs".to_string()),
            style: None,
            intent: Some("document".to_string()),
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

        assert!(output.contains("step write-readme: ok"), "{output}");
        assert!(
            fs::read_to_string(root.join("README.md"))
                .unwrap()
                .contains("## Usage")
        );
    }

    #[test]
    fn plan_run_emits_step_runner_events() {
        let root = temp_workspace("plan-run-events");
        let plan_yaml = "goal: \"Create docs\"\nprofile: \"docs\"\nstyle: \"default\"\nsteps:\n  - id: \"write-readme\"\n    kind: \"create\"\n    instruction: \"Create README.md.\"\n    expected_paths:\n      - \"README.md\"\n    verify:\n      - \"cat README.md\"\n";
        let mut planner = MockClient::new(vec![ChatResponse {
            content: plan_yaml.to_string(),
            tool_calls: Vec::new(),

            usage: Default::default(),
        }]);
        let mut executor = MockClient::new(vec![
            ChatResponse {
                content: String::new(),
                tool_calls: vec![ToolCall {
                    id: None,
                    thought_signature: None,
                    name: "Write".to_string(),
                    args_json: r#"{"path":"README.md","content":"ok"}"#.to_string(),
                }],

                usage: Default::default(),
            },
            ChatResponse {
                content: "Created README.md.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
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
        let mut observer = CaptureObserver::default();

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
        .run_with_observer(command, &mut observer)
        .unwrap();

        assert!(output.contains("step write-readme: ok"));
        assert!(observer
            .events()
            .iter()
            .any(|event| matches!(event, RuntimeEvent::PlanSaved { item_ids, .. } if item_ids == &vec!["write-readme".to_string()])));
        assert!(observer.events().iter().any(|event| {
            matches!(
                event,
                RuntimeEvent::StepStarted {
                    step_id,
                    index: 1,
                    total: 1
                } if step_id == "write-readme"
            )
        }));
        assert!(observer.events().iter().any(|event| {
            matches!(
                event,
                RuntimeEvent::VerifierStarted { step_id, command }
                    if step_id == "write-readme" && command == "cat README.md"
            )
        }));
        assert!(observer.events().iter().any(|event| {
            matches!(
                event,
                RuntimeEvent::StepFinished { step_id, .. } if step_id == "write-readme"
            )
        }));
    }

    #[test]
    fn plan_run_executes_verify_step_without_llm_turn_when_verifier_passes() {
        let root = temp_workspace("verify-step-direct-pass");
        fs::write(root.join("README.md"), "ok").unwrap();
        let plan_yaml = "goal: \"Verify docs\"\nprofile: \"docs\"\nstyle: \"default\"\nsteps:\n  - id: \"verify-readme\"\n    kind: \"verify\"\n    instruction: \"Verify README exists.\"\n    expected_paths: []\n    verify:\n      - \"cat README.md\"\n";
        let mut planner = MockClient::new(vec![ChatResponse {
            content: plan_yaml.to_string(),
            tool_calls: Vec::new(),

            usage: Default::default(),
        }]);
        let mut executor = MockClient::new(vec![]);
        let command = SlashCommand {
            kind: SlashCommandKind::PlanRun,
            profile: Some("docs".to_string()),
            style: None,
            intent: None,
            artifacts: Vec::new(),
            argument: "Verify docs".to_string(),
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

        assert!(output.contains("step verify-readme: ok"), "{output}");
    }

    #[test]
    fn plan_run_repairs_verify_step_only_after_verifier_fails() {
        let root = temp_workspace("verify-step-direct-repair");
        let plan_yaml = "goal: \"Verify docs\"\nprofile: \"docs\"\nstyle: \"default\"\nsteps:\n  - id: \"verify-readme\"\n    kind: \"verify\"\n    instruction: \"Verify README exists.\"\n    expected_paths: []\n    verify:\n      - \"cat README.md\"\n";
        let mut planner = MockClient::new(vec![ChatResponse {
            content: plan_yaml.to_string(),
            tool_calls: Vec::new(),

            usage: Default::default(),
        }]);
        let mut executor = MockClient::new(vec![
            ChatResponse {
                content: String::new(),
                tool_calls: vec![ToolCall {
                    id: None,
                    thought_signature: None,
                    name: "Write".to_string(),
                    args_json: r#"{"path":"README.md","content":"repaired"}"#.to_string(),
                }],

                usage: Default::default(),
            },
            ChatResponse {
                content: "Created README.md.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
        ]);
        let command = SlashCommand {
            kind: SlashCommandKind::PlanRun,
            profile: Some("docs".to_string()),
            style: None,
            intent: None,
            artifacts: Vec::new(),
            argument: "Verify docs".to_string(),
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

        assert!(output.contains("step verify-readme: ok"), "{output}");
        assert_eq!(
            fs::read_to_string(root.join("README.md")).unwrap(),
            "repaired"
        );
    }

    #[test]
    fn repair_retries_recoverable_final_answer_turn_error() {
        let root = temp_workspace("repair-retry-final-answer-error");
        let plan_yaml = "goal: \"Verify docs\"\nprofile: \"docs\"\nstyle: \"default\"\nsteps:\n  - id: \"verify-readme\"\n    kind: \"verify\"\n    instruction: \"Verify README exists.\"\n    expected_paths: []\n    verify:\n      - \"cat README.md\"\n";
        let mut planner = MockClient::new(vec![ChatResponse {
            content: plan_yaml.to_string(),
            tool_calls: Vec::new(),

            usage: Default::default(),
        }]);
        let mut executor = MockClient::new(vec![
            ChatResponse {
                content: "Let me create README.md now.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
            ChatResponse {
                content: "Now I'll create README.md.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
            ChatResponse {
                content: String::new(),
                tool_calls: vec![ToolCall {
                    id: None,
                    thought_signature: None,
                    name: "Write".to_string(),
                    args_json: r#"{"path":"README.md","content":"recovered"}"#.to_string(),
                }],

                usage: Default::default(),
            },
            ChatResponse {
                content: "Created README.md.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
        ]);
        let command = SlashCommand {
            kind: SlashCommandKind::PlanRun,
            profile: Some("docs".to_string()),
            style: None,
            intent: None,
            artifacts: Vec::new(),
            argument: "Verify docs".to_string(),
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

        assert!(output.contains("step verify-readme: ok"), "{output}");
        assert_eq!(
            fs::read_to_string(root.join("README.md")).unwrap(),
            "recovered"
        );
    }

    #[test]
    fn dependency_missing_stops_without_repair_prompt() {
        let root = temp_workspace("dependency-missing-terminal");
        fs::write(
            root.join("package.json"),
            r#"{"scripts":{"build":"next build"},"dependencies":{"next":"14.0.0","react":"18.0.0","react-dom":"18.0.0"}}"#,
        )
        .unwrap();
        let plan_yaml = "goal: \"Verify Next.js build\"\nprofile: \"nextjs\"\nstyle: \"default\"\nsteps:\n  - id: \"verify-build\"\n    kind: \"verify\"\n    instruction: \"Run npm run build.\"\n    expected_paths: []\n    verify:\n      - \"npm run build\"\n";
        let mut planner = MockClient::new(vec![ChatResponse {
            content: plan_yaml.to_string(),
            tool_calls: Vec::new(),

            usage: Default::default(),
        }]);
        let mut executor = MockClient::new(Vec::new());
        let command = SlashCommand {
            kind: SlashCommandKind::PlanRun,
            profile: Some("nextjs".to_string()),
            style: None,
            intent: None,
            artifacts: Vec::new(),
            argument: "Verify Next.js build".to_string(),
        };

        let mut observer = CaptureObserver::default();
        let err = SlashRuntime {
            executor: &mut executor,
            planner: &mut planner,
            cwd: &root,
            loop_config: MinimalLoopConfig::default(),
            planner_config: PlannerRuntimeConfig {
                model: "planner".to_string(),
                tool_call_mode: ToolCallMode::XmlFallback,
            },
        }
        .run_with_observer(command, &mut observer)
        .unwrap_err();

        assert!(err.contains("dependency_missing"), "{err}");
        assert!(err.contains("environment/setup blocker"), "{err}");
        assert!(err.contains("npm install"), "{err}");
        assert!(err.contains("npm run build"), "{err}");
        assert!(!err.contains("repair prompt saved"), "{err}");
        assert!(!err.contains("suggested command"), "{err}");
        assert!(executor.prompts.is_empty());
        assert!(!root.join(".commandagent/repairs").exists());
        assert!(!observer.events().iter().any(|event| matches!(
            event,
            RuntimeEvent::RepairAttemptStarted { .. } | RuntimeEvent::RepairExhausted { .. }
        )));
    }

    #[test]
    fn repair_adds_missing_artifact_no_tool_guard_once() {
        let root = temp_workspace("repair-missing-artifact-no-tool-guard");
        let step = StepPlanStep {
            id: "write-readme".to_string(),
            kind: StepKind::Create,
            instruction: "Create README.md.".to_string(),
            expected_result: ExpectedResult::Pass,
            expected_paths: vec!["README.md".to_string()],
            verify: vec!["cat README.md".to_string()],
        };
        let plan = StepPlan {
            goal: "Create docs".to_string(),
            profile: "docs".to_string(),
            style: "default".to_string(),
            intent: WorkIntent::Document,
            required_artifacts: Vec::new(),
            steps: vec![step.clone()],
        };
        let mut executor = MockClient::new(vec![
            ChatResponse {
                content: "Let me read README.md first.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
            ChatResponse {
                content: "I'll create README.md now.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
            ChatResponse {
                content: String::new(),
                tool_calls: vec![ToolCall {
                    id: None,
                    thought_signature: None,
                    name: "Write".to_string(),
                    args_json: r#"{"path":"README.md","content":"guarded"}"#.to_string(),
                }],

                usage: Default::default(),
            },
            ChatResponse {
                content: "Created README.md.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
        ]);
        let mut planner = MockClient::new(Vec::new());
        let config = MinimalLoopConfig {
            expected_artifacts: step.expected_paths.clone(),
            ..MinimalLoopConfig::default()
        };
        let failures = vec![VerificationFailure {
            command: "cat README.md".to_string(),
            reason: "command_failed:1".to_string(),
            stdout_excerpt: String::new(),
            stderr_excerpt: "cat: README.md: No such file or directory".to_string(),
            diagnostic_excerpt: String::new(),
            source_excerpt: None,
        }];

        let mut observer = NoopRuntimeObserver;
        let contract_seed = phase_contract::ActiveStepContract::empty("docs");
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
        .repair_step_with_state(
            RepairStepRequest {
                plan: &plan,
                step: &step,
                config,
                contract_seed: &contract_seed,
            },
            RepairStepState {
                failures,
                changed_files: Vec::new(),
                file_changing_attempts: 0,
                initial_turn_error: None,
                dependency_setup_attempt_keys: Vec::new(),
                dependency_setup_note: None,
                setup_job_state: Vec::new(),
                tool_records: Vec::new(),
                contract_evidence: Vec::new(),
                repair_attempt_ledger: Vec::new(),
                repair_job_state: crate::agent::step_runner::repair_job::RepairJobState::new(
                    "unknown",
                )
                .with_step_id(step.id.clone()),
                tool_arg_schema_correction_spent: false,
                pending_tool_arg_error: None,
                pending_tool_arg_error_source: None,
            },
            &mut observer,
        )
        .unwrap();

        assert_eq!(
            fs::read_to_string(root.join("README.md")).unwrap(),
            "guarded"
        );
        let guard_prompt_count = executor
            .prompts
            .iter()
            .filter(|prompt| prompt.contains("The required path is still missing: README.md"))
            .count();
        assert_eq!(guard_prompt_count, 1);
    }

    #[test]
    fn turn_error_failure_classifies_edit_target_not_found() {
        let failure = turn_error_failure(
            "repair turn",
            &MinimalLoopError::Tool("edit target was not found".to_string()),
        );

        assert_eq!(failure.reason, "edit_target_not_found");
        assert!(failure.diagnostic_excerpt.contains("file state is stale"));
        assert!(failure.diagnostic_excerpt.contains("Read or Glob"));
        assert!(
            failure
                .diagnostic_excerpt
                .contains("Original error: tool error: edit target was not found")
        );
    }

    #[test]
    fn turn_error_failure_classifies_missing_required_tool_field() {
        let failure = turn_error_failure(
            "initial turn",
            &MinimalLoopError::ToolArgs(ToolArgError::MissingRequiredStringField {
                tool: "Write".to_string(),
                field: "path".to_string(),
                required_fields: vec!["path".to_string(), "content".to_string()],
            }),
        );

        assert_eq!(failure.reason, "tool_args_missing_required_field");
        assert!(failure.diagnostic_excerpt.contains("Write"));
        assert!(
            failure
                .diagnostic_excerpt
                .contains("required string field `path` was missing")
        );
        assert!(
            failure
                .diagnostic_excerpt
                .contains("Write requires: path, content")
        );
        assert!(
            failure
                .diagnostic_excerpt
                .contains("Emit exactly one valid tool call")
        );
    }

    #[test]
    fn turn_error_failure_classifies_invalid_tool_json() {
        let failure = turn_error_failure(
            "initial turn",
            &MinimalLoopError::ToolArgs(ToolArgError::InvalidJson {
                tool: "Write".to_string(),
                message: "expected value".to_string(),
            }),
        );

        assert_eq!(failure.reason, "tool_args_invalid_json");
        assert!(
            failure
                .diagnostic_excerpt
                .contains("invalid JSON arguments")
        );
        assert!(
            failure
                .diagnostic_excerpt
                .contains("Emit exactly one valid tool call")
        );
    }

    #[test]
    fn plan_run_corrects_unowned_required_artifact_before_execution() {
        let root = temp_workspace("plan-run-required-artifact-owner");
        let invalid_plan = "goal: \"Create docs\"\nprofile: \"docs\"\nstyle: \"default\"\nsteps:\n  - id: \"write-readme\"\n    kind: \"create\"\n    instruction: \"Create README.md.\"\n    expected_paths:\n      - \"README.md\"\n    verify:\n      - \"cat README.md\"\n";
        let corrected_plan = "goal: \"Create docs\"\nprofile: \"docs\"\nstyle: \"default\"\nsteps:\n  - id: \"write-final\"\n    kind: \"create\"\n    instruction: \"Create FINAL.md.\"\n    expected_paths:\n      - \"FINAL.md\"\n    verify:\n      - \"cat FINAL.md\"\n";
        let mut planner = MockClient::new(vec![
            ChatResponse {
                content: invalid_plan.to_string(),
                tool_calls: Vec::new(),
                usage: Default::default(),
            },
            ChatResponse {
                content: corrected_plan.to_string(),
                tool_calls: Vec::new(),
                usage: Default::default(),
            },
        ]);
        let mut executor = MockClient::new(vec![
            ChatResponse {
                content: String::new(),
                tool_calls: vec![ToolCall {
                    id: None,
                    thought_signature: None,
                    name: "Write".to_string(),
                    args_json: r#"{"path":"FINAL.md","content":"ok"}"#.to_string(),
                }],

                usage: Default::default(),
            },
            ChatResponse {
                content: "Created FINAL.md.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
        ]);
        let command = SlashCommand {
            kind: SlashCommandKind::PlanRun,
            profile: Some("docs".to_string()),
            style: None,
            intent: None,
            artifacts: vec!["FINAL.md".to_string()],
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

        assert!(output.contains("step write-final: ok"), "{output}");
        assert!(root.join("FINAL.md").exists());
    }

    #[test]
    fn plan_run_does_not_hide_max_iterations_behind_successful_verifier() {
        let root = temp_workspace("plan-run-max-is-fatal");
        let plan_yaml = "goal: \"Create docs\"\nprofile: \"docs\"\nstyle: \"default\"\nsteps:\n  - id: \"write-readme\"\n    kind: \"create\"\n    instruction: \"Create README.md.\"\n    expected_paths:\n      - \"README.md\"\n    verify:\n      - \"cat README.md\"\n";
        let mut planner = MockClient::new(vec![ChatResponse {
            content: plan_yaml.to_string(),
            tool_calls: Vec::new(),

            usage: Default::default(),
        }]);
        let mut executor = MockClient::new(vec![ChatResponse {
            content: String::new(),
            tool_calls: vec![ToolCall {
                id: None,
                thought_signature: None,
                name: "Write".to_string(),
                args_json: r#"{"path":"README.md","content":"ok"}"#.to_string(),
            }],

            usage: Default::default(),
        }]);
        let command = SlashCommand {
            kind: SlashCommandKind::PlanRun,
            profile: Some("docs".to_string()),
            style: None,
            intent: None,
            artifacts: Vec::new(),
            argument: "Create docs".to_string(),
        };
        let loop_config = MinimalLoopConfig {
            max_iterations: 1,
            ..MinimalLoopConfig::default()
        };

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

        assert!(err.contains("initial turn error"), "{err}");
        assert!(err.contains("minimal loop reached max iterations"), "{err}");
        assert_eq!(fs::read_to_string(root.join("README.md")).unwrap(), "ok");
    }

    #[test]
    fn plan_run_does_not_hide_invalid_tool_args_behind_empty_verifier() {
        let root = temp_workspace("plan-run-invalid-tool-args");
        let plan_yaml = "goal: \"Inspect workspace\"\nprofile: \"docs\"\nstyle: \"default\"\nsteps:\n  - id: \"inspect\"\n    kind: \"inspect\"\n    instruction: \"Inspect current workspace.\"\n    expected_paths: []\n    verify: []\n";
        let mut planner = MockClient::new(vec![ChatResponse {
            content: plan_yaml.to_string(),
            tool_calls: Vec::new(),

            usage: Default::default(),
        }]);
        let mut executor = MockClient::new(vec![ChatResponse {
            content: String::new(),
            tool_calls: vec![ToolCall {
                id: None,
                thought_signature: None,
                name: "Glob".to_string(),
                args_json: "{}".to_string(),
            }],

            usage: Default::default(),
        }]);
        let command = SlashCommand {
            kind: SlashCommandKind::PlanRun,
            profile: Some("docs".to_string()),
            style: None,
            intent: None,
            artifacts: Vec::new(),
            argument: "Inspect workspace".to_string(),
        };

        let err = SlashRuntime {
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
        .unwrap_err();

        assert!(err.contains("initial turn error"), "{err}");
        assert!(err.contains("invalid tool arguments"), "{err}");
        assert!(!err.contains("step inspect: ok"), "{err}");
        assert_eq!(executor.prompts.len(), 1);
    }

    #[test]
    fn initial_tool_args_gets_one_protocol_correction() {
        let root = temp_workspace("plan-run-initial-tool-args-correction");
        let plan_yaml = "goal: \"Create docs\"\nprofile: \"docs\"\nstyle: \"default\"\nsteps:\n  - id: \"write-readme\"\n    kind: \"create\"\n    instruction: \"Create README.md.\"\n    expected_paths:\n      - \"README.md\"\n    verify:\n      - \"cat README.md\"\n";
        let mut planner = MockClient::new(vec![ChatResponse {
            content: plan_yaml.to_string(),
            tool_calls: Vec::new(),

            usage: Default::default(),
        }]);
        let mut executor = MockClient::new(vec![
            ChatResponse {
                content: String::new(),
                tool_calls: vec![ToolCall {
                    id: None,
                    thought_signature: None,
                    name: "Write".to_string(),
                    args_json: r#"{"content":"missing path"}"#.to_string(),
                }],

                usage: Default::default(),
            },
            ChatResponse {
                content: String::new(),
                tool_calls: vec![ToolCall {
                    id: None,
                    thought_signature: None,
                    name: "Write".to_string(),
                    args_json: r#"{"path":"README.md","content":"corrected"}"#.to_string(),
                }],

                usage: Default::default(),
            },
            ChatResponse {
                content: "Created README.md.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
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

        assert!(output.contains("step write-readme: ok"), "{output}");
        assert_eq!(
            fs::read_to_string(root.join("README.md")).unwrap(),
            "corrected"
        );
        assert!(
            executor
                .prompts
                .iter()
                .any(|prompt| prompt.contains("Tool protocol correction"))
        );
        assert!(
            executor
                .prompts
                .iter()
                .any(|prompt| prompt.contains("target_path_json=\"README.md\""))
        );
    }

    #[test]
    fn repeated_tool_args_after_initial_schema_failure_stops_bounded() {
        let root = temp_workspace("plan-run-repeated-tool-args");
        let plan_yaml = "goal: \"Create docs\"\nprofile: \"docs\"\nstyle: \"default\"\nsteps:\n  - id: \"write-readme\"\n    kind: \"create\"\n    instruction: \"Create README.md.\"\n    expected_paths:\n      - \"README.md\"\n    verify:\n      - \"cat README.md\"\n";
        let mut planner = MockClient::new(vec![ChatResponse {
            content: plan_yaml.to_string(),
            tool_calls: Vec::new(),

            usage: Default::default(),
        }]);
        let mut executor = MockClient::new(vec![
            ChatResponse {
                content: String::new(),
                tool_calls: vec![ToolCall {
                    id: None,
                    thought_signature: None,
                    name: "Write".to_string(),
                    args_json: r#"{"content":"missing path"}"#.to_string(),
                }],

                usage: Default::default(),
            },
            ChatResponse {
                content: String::new(),
                tool_calls: vec![ToolCall {
                    id: None,
                    thought_signature: None,
                    name: "Write".to_string(),
                    args_json: r#"{"content":"still missing path"}"#.to_string(),
                }],

                usage: Default::default(),
            },
            ChatResponse {
                content: String::new(),
                tool_calls: vec![ToolCall {
                    id: None,
                    thought_signature: None,
                    name: "Write".to_string(),
                    args_json: r#"{"path":"README.md","content":"should not run"}"#.to_string(),
                }],

                usage: Default::default(),
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

        let err = SlashRuntime {
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
        .unwrap_err();

        assert!(err.contains("initial turn error"), "{err}");
        assert!(err.contains("invalid tool arguments"), "{err}");
        assert_eq!(executor.prompts.len(), 2);
        assert!(executor.prompts[1].contains("Tool protocol correction"));
        assert!(executor.prompts[1].contains("Missing required field: path"));
        assert!(executor.prompts[1].contains("target_path_json=\"README.md\""));
        assert!(!root.join("README.md").exists());
        let repair_packet = fs::read_dir(root.join(".commandagent/repairs"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let packet = fs::read_to_string(repair_packet).unwrap();
        assert!(packet.contains("tool_args_missing_required_field"));
        assert!(packet.contains("Write requires: path, content"));
        assert!(packet.contains("Tool protocol correction was attempted once"));
    }

    #[test]
    fn repair_turn_tool_args_gets_one_schema_correction_when_not_spent() {
        let root = temp_workspace("plan-run-repair-tool-args-one-correction");
        let plan_yaml = "goal: \"Create docs\"\nprofile: \"docs\"\nstyle: \"default\"\nsteps:\n  - id: \"write-readme\"\n    kind: \"create\"\n    instruction: \"Create README.md.\"\n    expected_paths:\n      - \"README.md\"\n    verify:\n      - \"grep -q fixed README.md\"\n";
        let mut planner = MockClient::new(vec![ChatResponse {
            content: plan_yaml.to_string(),
            tool_calls: Vec::new(),

            usage: Default::default(),
        }]);
        let mut executor = MockClient::new(vec![
            ChatResponse {
                content: String::new(),
                tool_calls: vec![ToolCall {
                    id: None,
                    thought_signature: None,
                    name: "Write".to_string(),
                    args_json: r#"{"path":"README.md","content":"broken"}"#.to_string(),
                }],

                usage: Default::default(),
            },
            ChatResponse {
                content: "Wrote README.md.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
            ChatResponse {
                content: String::new(),
                tool_calls: vec![ToolCall {
                    id: None,
                    thought_signature: None,
                    name: "Write".to_string(),
                    args_json: r#"{"content":"missing path"}"#.to_string(),
                }],

                usage: Default::default(),
            },
            ChatResponse {
                content: String::new(),
                tool_calls: vec![ToolCall {
                    id: None,
                    thought_signature: None,
                    name: "Write".to_string(),
                    args_json: r#"{"path":"README.md","content":"fixed"}"#.to_string(),
                }],

                usage: Default::default(),
            },
            ChatResponse {
                content: "Fixed README.md.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
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

        assert!(output.contains("step write-readme: ok"), "{output}");
        assert_eq!(fs::read_to_string(root.join("README.md")).unwrap(), "fixed");
        assert!(
            executor
                .prompts
                .iter()
                .any(|prompt| prompt.contains("Tool protocol correction")
                    && prompt.contains("tool_args_missing_required_field"))
        );
    }

    #[test]
    fn plan_run_saves_repair_prompt_after_initial_turn_error() {
        let root = temp_workspace("plan-run-repair-after-error");
        fs::write(root.join("README.md"), "fixture").unwrap();
        let plan_yaml = "goal: \"Verify docs\"\nprofile: \"docs\"\nstyle: \"default\"\nsteps:\n  - id: \"verify-readme\"\n    kind: \"inspect\"\n    instruction: \"Inspect README.md.\"\n    expected_paths:\n      - \"README.md\"\n    verify:\n      - \"grep -q __missing_marker__ /dev/null\"\n";
        let mut planner = MockClient::new(vec![ChatResponse {
            content: plan_yaml.to_string(),
            tool_calls: Vec::new(),

            usage: Default::default(),
        }]);
        let mut executor = MockClient::new(vec![ChatResponse {
            content: "Let me verify README.md.".to_string(),
            tool_calls: Vec::new(),

            usage: Default::default(),
        }]);
        let command = SlashCommand {
            kind: SlashCommandKind::PlanRun,
            profile: Some("docs".to_string()),
            style: None,
            intent: None,
            artifacts: Vec::new(),
            argument: "Verify docs".to_string(),
        };
        let loop_config = MinimalLoopConfig {
            max_iterations: 1,
            ..MinimalLoopConfig::default()
        };

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
        let repair_dir = root.join(".commandagent/repairs");
        assert!(repair_dir.exists());
        let repair_text = fs::read_to_string(
            fs::read_dir(repair_dir)
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path(),
        )
        .unwrap();
        assert!(repair_text.contains("initial turn"));
        assert!(repair_text.contains("turn_error"));
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

                usage: Default::default(),
            },
            ChatResponse {
                content: corrected_yaml.to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
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
    fn ultra_phase_step_plan_uses_profile_obligations_during_correction() {
        let root = temp_workspace("ultra-profile-obligation-correction");
        let ultra_yaml = "goal: \"Create Next.js app on port 3011\"\nprofile: \"nextjs\"\nstyle: \"default\"\nintent: \"new\"\nphases:\n  - id: \"scaffold\"\n    goal: \"Create app files.\"\n";
        let invalid_plan = "goal: \"Create app files.\"\nprofile: \"nextjs\"\nstyle: \"default\"\nsteps:\n  - id: \"create-app\"\n    kind: \"create\"\n    instruction: \"Create package.json with scripts.build as next build, dependencies next, react, and react-dom with React 18.2 compatibility plus typescript 5.x compatibility and @types/react 18.x compatibility, plus app/page.tsx.\"\n    expected_paths:\n      - \"package.json\"\n      - \"app/page.tsx\"\n    verify:\n      - \"test -f package.json\"\n      - \"test -f app/page.tsx\"\n";
        let corrected_plan = "goal: \"Create app files.\"\nprofile: \"nextjs\"\nstyle: \"default\"\nsteps:\n  - id: \"create-app\"\n    kind: \"create\"\n    instruction: \"Create package.json with scripts.dev as next dev -p 3011, scripts.build as next build, dependencies next, react, and react-dom with React 18.2 compatibility plus typescript 5.x compatibility and @types/react 18.x compatibility, plus app/page.tsx.\"\n    expected_paths:\n      - \"package.json\"\n      - \"app/page.tsx\"\n    verify:\n      - \"test -f package.json\"\n      - \"test -f app/page.tsx\"\n";
        let mut planner = MockClient::new(vec![
            ChatResponse {
                content: ultra_yaml.to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
            ChatResponse {
                content: invalid_plan.to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
            ChatResponse {
                content: corrected_plan.to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
        ]);
        let mut executor = MockClient::new(vec![
            ChatResponse {
                content: String::new(),
                tool_calls: vec![
                    ToolCall {
                        id: None,
                        thought_signature: None,
                        name: "Write".to_string(),
                        args_json: r#"{"path":"package.json","content":"{\"scripts\":{\"dev\":\"next dev -p 3011\",\"build\":\"next build\"},\"dependencies\":{\"next\":\"latest\",\"react\":\"latest\",\"react-dom\":\"latest\"}}"}"#.to_string(),
                    },
                    ToolCall {
                        id: None,
                        thought_signature: None,
                        name: "Write".to_string(),
                        args_json: r#"{"path":"app/page.tsx","content":"export default function Page() { return null }"}"#.to_string(),
                    },
                ],

                usage: Default::default(),},
            ChatResponse {
                content: "Created app files.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),},
            ChatResponse {
                content: String::new(),
                tool_calls: vec![ToolCall {
                    id: None,
                    thought_signature: None,
                    name: "Write".to_string(),
                    args_json: r#"{"path":"app/layout.tsx","content":"import type { ReactNode } from \"react\";\n\nexport default function RootLayout({ children }: { children: ReactNode }) {\n  return <html lang=\"en\"><body>{children}</body></html>;\n}\n"}"#.to_string(),
                }],

                usage: Default::default(),},
            ChatResponse {
                content: "Created root layout.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),},
        ]);
        let command = SlashCommand {
            kind: SlashCommandKind::UltraPlanRun,
            profile: Some("nextjs".to_string()),
            style: None,
            intent: Some("new".to_string()),
            artifacts: Vec::new(),
            argument: "Create Next.js app on port 3011".to_string(),
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

        assert!(output.contains("phase scaffold: ok"), "{output}");
        assert!(root.join(".commandagent/plans").exists());
    }

    #[test]
    fn ultra_plan_does_not_enforce_required_artifacts_between_phases() {
        let root = temp_workspace("ultra-final-artifact-between-phases");
        let ultra_yaml = "goal: \"Create final\"\nprofile: \"docs\"\nstyle: \"default\"\nintent: \"new\"\nphases:\n  - id: \"inspect\"\n    goal: \"Inspect existing files.\"\n  - id: \"create-final\"\n    goal: \"Create FINAL.md.\"\n";
        let inspect_plan = "goal: \"Inspect existing files.\"\nprofile: \"docs\"\nstyle: \"default\"\nsteps:\n  - id: \"inspect\"\n    kind: \"inspect\"\n    instruction: \"Inspect current workspace.\"\n    expected_paths: []\n    verify: []\n";
        let create_plan = "goal: \"Create FINAL.md.\"\nprofile: \"docs\"\nstyle: \"default\"\nsteps:\n  - id: \"write-final\"\n    kind: \"create\"\n    instruction: \"Create FINAL.md.\"\n    expected_paths:\n      - \"FINAL.md\"\n    verify:\n      - \"cat FINAL.md\"\n";
        let mut planner = MockClient::new(vec![
            ChatResponse {
                content: ultra_yaml.to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
            ChatResponse {
                content: inspect_plan.to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
            ChatResponse {
                content: create_plan.to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
        ]);
        let mut executor = MockClient::new(vec![
            ChatResponse {
                content: "Inspection complete.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
            ChatResponse {
                content: String::new(),
                tool_calls: vec![ToolCall {
                    id: None,
                    thought_signature: None,
                    name: "Write".to_string(),
                    args_json: r#"{"path":"FINAL.md","content":"done"}"#.to_string(),
                }],

                usage: Default::default(),
            },
            ChatResponse {
                content: "Created FINAL.md.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
        ]);
        let command = SlashCommand {
            kind: SlashCommandKind::UltraPlanRun,
            profile: Some("docs".to_string()),
            style: None,
            intent: Some("new".to_string()),
            artifacts: vec!["FINAL.md".to_string()],
            argument: "Create final".to_string(),
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

        assert!(output.contains("phase inspect: ok"), "{output}");
        assert!(output.contains("phase create-final: ok"), "{output}");
        assert_eq!(fs::read_to_string(root.join("FINAL.md")).unwrap(), "done");
    }

    #[test]
    fn ultra_plan_enforces_required_artifacts_at_final_boundary() {
        let root = temp_workspace("ultra-final-artifact-final-boundary");
        let ultra_yaml = "goal: \"Create final\"\nprofile: \"docs\"\nstyle: \"default\"\nintent: \"new\"\nphases:\n  - id: \"inspect\"\n    goal: \"Inspect existing files.\"\n";
        let inspect_plan = "goal: \"Inspect existing files.\"\nprofile: \"docs\"\nstyle: \"default\"\nsteps:\n  - id: \"inspect\"\n    kind: \"inspect\"\n    instruction: \"Inspect current workspace.\"\n    expected_paths: []\n    verify: []\n";
        let mut planner = MockClient::new(vec![
            ChatResponse {
                content: ultra_yaml.to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
            ChatResponse {
                content: inspect_plan.to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
        ]);
        let mut executor = MockClient::new(vec![ChatResponse {
            content: "Inspection complete.".to_string(),
            tool_calls: Vec::new(),

            usage: Default::default(),
        }]);
        let command = SlashCommand {
            kind: SlashCommandKind::UltraPlanRun,
            profile: Some("docs".to_string()),
            style: None,
            intent: Some("new".to_string()),
            artifacts: vec!["FINAL.md".to_string()],
            argument: "Create final".to_string(),
        };

        let mut observer = CaptureObserver::default();
        let err = SlashRuntime {
            executor: &mut executor,
            planner: &mut planner,
            cwd: &root,
            loop_config: MinimalLoopConfig::default(),
            planner_config: PlannerRuntimeConfig {
                model: "planner".to_string(),
                tool_call_mode: ToolCallMode::XmlFallback,
            },
        }
        .run_with_observer(command, &mut observer)
        .unwrap_err();

        assert!(
            err.contains("missing required final artifacts: FINAL.md"),
            "{err}"
        );
        assert!(observer.events().iter().any(|event| matches!(
            event,
            RuntimeEvent::ArtifactStatus {
                scope: ArtifactScope::FinalRequiredArtifact,
                path,
                status: ArtifactStatus::Missing,
            } if path == "FINAL.md"
        )));
    }

    #[test]
    fn ultra_plan_fails_phase_on_nextjs_profile_verification() {
        let root = temp_workspace("ultra-nextjs-profile-failure");
        let ultra_yaml = "goal: \"Create Next.js app on port 3011\"\nprofile: \"nextjs\"\nstyle: \"default\"\nintent: \"new\"\nphases:\n  - id: \"scaffold\"\n    goal: \"Create app files.\"\n";
        let scaffold_plan = "goal: \"Create app files.\"\nprofile: \"nextjs\"\nstyle: \"default\"\nsteps:\n  - id: \"create-app\"\n    kind: \"create\"\n    instruction: \"Create package.json with scripts.dev as next dev -p 3011, scripts.build as next build, dependencies next, react, and react-dom with React 18.2 compatibility plus typescript 5.x compatibility and @types/react 18.x compatibility, plus app/page.tsx.\"\n    expected_paths:\n      - \"package.json\"\n      - \"app/page.tsx\"\n    verify:\n      - \"test -f package.json\"\n      - \"test -f app/page.tsx\"\n";
        let mut planner = MockClient::new(vec![
            ChatResponse {
                content: ultra_yaml.to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
            ChatResponse {
                content: scaffold_plan.to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
        ]);
        let mut executor = MockClient::new(vec![
            ChatResponse {
                content: String::new(),
                tool_calls: vec![
                    ToolCall {
                        id: None,
                        thought_signature: None,
                        name: "Write".to_string(),
                        args_json: r#"{"path":"package.json","content":"{\"scripts\":{\"dev\":\"next dev\",\"build\":\"next build\"},\"dependencies\":{\"next\":\"latest\",\"react\":\"latest\",\"react-dom\":\"latest\"}}"}"#.to_string(),
                    },
                    ToolCall {
                        id: None,
                        thought_signature: None,
                        name: "Write".to_string(),
                        args_json: r#"{"path":"app/page.tsx","content":"export default function Page() { return null }"}"#.to_string(),
                    },
                ],

                usage: Default::default(),},
            ChatResponse {
                content: "Created app files.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),},
            ChatResponse {
                content: String::new(),
                tool_calls: vec![ToolCall {
                    id: None,
                    thought_signature: None,
                    name: "Write".to_string(),
                    args_json: r#"{"path":"app/layout.tsx","content":"import type { ReactNode } from \"react\";\n\nexport default function RootLayout({ children }: { children: ReactNode }) {\n  return <html lang=\"en\"><body>{children}</body></html>;\n}\n"}"#.to_string(),
                }],

                usage: Default::default(),},
            ChatResponse {
                content: "Created root layout.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),},
        ]);
        let command = SlashCommand {
            kind: SlashCommandKind::UltraPlanRun,
            profile: Some("nextjs".to_string()),
            style: None,
            intent: Some("new".to_string()),
            artifacts: Vec::new(),
            argument: "Create Next.js app on port 3011".to_string(),
        };

        let mut observer = CaptureObserver::default();
        let err = SlashRuntime {
            executor: &mut executor,
            planner: &mut planner,
            cwd: &root,
            loop_config: MinimalLoopConfig::default(),
            planner_config: PlannerRuntimeConfig {
                model: "planner".to_string(),
                tool_call_mode: ToolCallMode::XmlFallback,
            },
        }
        .run_with_observer(command, &mut observer)
        .unwrap_err();

        assert!(err.contains("profile verification failed"), "{err}");
        assert!(err.contains("nextjs_dev_port_drift"), "{err}");
        assert!(err.contains("profile repair prompt saved"), "{err}");
        assert!(err.contains("suggested command"), "{err}");
        let repair_dir = root.join(".commandagent/repairs");
        assert!(repair_dir.exists());
        let repair_text = fs::read_to_string(
            fs::read_dir(repair_dir)
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path(),
        )
        .unwrap();
        assert!(repair_text.contains("nextjs_dev_port_drift"));
        assert!(repair_text.contains("profile.obligation.nextjs_dev_port_required"));
        assert!(observer.events().iter().any(|event| matches!(
            event,
            RuntimeEvent::ProfileVerificationFailed { profile, failures }
                if profile == "nextjs" && failures.iter().any(|failure| failure.contains("nextjs_dev_port_drift"))
        )));
    }

    #[test]
    fn generated_step_plan_can_omit_known_header_fields() {
        let text = "steps:\n  - id: \"write-readme\"\n    instruction: \"Create README.md.\"\n    expected_paths:\n      - \"README.md\"\n    verify: []\n";

        let root = temp_workspace("parse-generated-header");
        let plan = parse_generated_step_plan(
            &root,
            text,
            &generated_step_context("Create docs", "docs", WorkIntent::New, &[], &[]),
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
            &generated_step_context("Create Rust CLI", "rust", WorkIntent::New, &[], &[]),
        )
        .unwrap();

        assert_eq!(plan.goal, "Create Rust CLI");
        assert_eq!(plan.profile, "rust");
        assert_eq!(plan.style, "default");
    }

    #[test]
    fn generated_step_plan_uses_requested_artifacts_over_model_artifacts() {
        let root = temp_workspace("parse-generated-artifacts");
        let text = "required_artifacts:\n  - wrong/path.txt\n  - README.md\nsteps:\n  - id: \"write-readme\"\n    instruction: \"Create README.md.\"\n    expected_paths:\n      - \"README.md\"\n    verify: []\n";

        let plan = parse_generated_step_plan(
            &root,
            text,
            &generated_step_context(
                "Create docs",
                "docs",
                WorkIntent::Document,
                &["README.md".to_string(), "README.md".to_string()],
                &[],
            ),
        )
        .unwrap();

        assert_eq!(plan.intent, WorkIntent::Document);
        assert_eq!(plan.required_artifacts, vec!["README.md"]);
    }

    #[test]
    fn generated_step_plan_lint_error_carries_contract_evidence() {
        let root = temp_workspace("parse-generated-contract-evidence");
        let text = "steps:\n  - id: \"create-package-json\"\n    kind: \"create\"\n    instruction: \"Create package.json with next and react dependencies.\"\n    expected_paths:\n      - \"package.json\"\n    verify: []\n  - id: \"update-package-json\"\n    kind: \"edit\"\n    instruction: \"Update package.json scripts.\"\n    expected_paths:\n      - \"package.json\"\n    verify: []\n";
        let err = parse_generated_step_plan(
            &root,
            text,
            &generated_step_context(
                "Create Next.js app",
                "nextjs",
                WorkIntent::New,
                &[],
                &[ProfileObligation {
                    code: "nextjs_dependencies_required".to_string(),
                    message: "dependencies required".to_string(),
                    paths: vec!["package.json".to_string()],
                    expected: None,
                }],
            ),
        )
        .unwrap_err();

        assert!(err.message.contains("plan lint failed"), "{}", err.message);
        let evidence = err.correction_evidence.unwrap();
        assert_eq!(evidence.failed_step.as_deref(), Some("create-package-json"));
        assert_eq!(
            evidence.violated_contract.as_deref(),
            Some("nextjs_dependencies_required")
        );
        assert_eq!(
            evidence.required_literals,
            vec!["next", "react", "react-dom", "18.2"]
        );
        assert_eq!(evidence.missing_literals, vec!["react-dom", "18.2"]);
        assert_eq!(evidence.active_job.as_deref(), Some("manifest_repair"));
        assert!(
            evidence
                .repair_attempt_ledger
                .iter()
                .any(|entry| entry.contains("could not select one package.json step"))
        );
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

                usage: Default::default(),
            },
            ChatResponse {
                content: corrected_yaml.to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
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

    #[test]
    fn invalid_source_grep_step_plan_gets_one_correction_attempt() {
        let root = temp_workspace("source-grep-plan-correction");
        let invalid_yaml = "goal: \"Create Rust CLI\"\nprofile: \"rust\"\nstyle: \"default\"\nsteps:\n  - id: \"write-main\"\n    kind: \"create\"\n    instruction: \"Create src/main.rs.\"\n    expected_paths:\n      - \"src/main.rs\"\n    verify:\n      - \"grep -q clap src/main.rs\"\n";
        let corrected_yaml = "goal: \"Create Rust CLI\"\nprofile: \"rust\"\nstyle: \"default\"\nsteps:\n  - id: \"write-main\"\n    kind: \"create\"\n    instruction: \"Create src/main.rs.\"\n    expected_paths:\n      - \"src/main.rs\"\n    verify:\n      - \"test -f src/main.rs\"\n  - id: \"verify-build\"\n    kind: \"verify\"\n    instruction: \"Run cargo check.\"\n    expected_paths: []\n    verify:\n      - \"cargo check\"\n";
        let mut planner = MockClient::new(vec![
            ChatResponse {
                content: invalid_yaml.to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
            ChatResponse {
                content: corrected_yaml.to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
        ]);
        let mut executor = MockClient::new(vec![]);
        let command = SlashCommand {
            kind: SlashCommandKind::PlanSteps,
            profile: Some("rust".to_string()),
            style: None,
            intent: Some("new".to_string()),
            artifacts: Vec::new(),
            argument: "Create Rust CLI".to_string(),
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

        assert!(output.contains("created step plan"), "{output}");
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

    #[test]
    fn invalid_source_grep_step_plan_gets_second_bounded_correction_attempt() {
        let root = temp_workspace("source-grep-plan-second-correction");
        let invalid_yaml = "goal: \"Create Rust CLI\"
profile: \"rust\"
style: \"default\"
steps:
  - id: \"write-main\"
    kind: \"create\"
    instruction: \"Create src/main.rs.\"
    expected_paths:
      - \"src/main.rs\"
    verify:
      - \"grep -q clap src/main.rs\"
";
        let still_invalid_yaml = "goal: \"Create Rust CLI\"
profile: \"rust\"
style: \"default\"
steps:
  - id: \"write-main\"
    kind: \"create\"
    instruction: \"Create src/main.rs.\"
    expected_paths:
      - \"src/main.rs\"
    verify:
      - \"test -f src/main.rs\"
      - \"grep -q fn src/main.rs\"
";
        let corrected_yaml = "goal: \"Create Rust CLI\"
profile: \"rust\"
style: \"default\"
steps:
  - id: \"write-main\"
    kind: \"create\"
    instruction: \"Create src/main.rs.\"
    expected_paths:
      - \"src/main.rs\"
    verify:
      - \"test -f src/main.rs\"
  - id: \"verify-build\"
    kind: \"verify\"
    instruction: \"Run cargo check.\"
    expected_paths: []
    verify:
      - \"cargo check\"
";
        let mut planner = MockClient::new(vec![
            ChatResponse {
                content: invalid_yaml.to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
            ChatResponse {
                content: still_invalid_yaml.to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
            ChatResponse {
                content: corrected_yaml.to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
        ]);
        let mut executor = MockClient::new(vec![]);
        let command = SlashCommand {
            kind: SlashCommandKind::PlanSteps,
            profile: Some("rust".to_string()),
            style: None,
            intent: Some("new".to_string()),
            artifacts: Vec::new(),
            argument: "Create Rust CLI".to_string(),
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

        assert!(output.contains("created step plan"), "{output}");
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
        assert_eq!(invalid_count, 2);
    }

    #[test]
    fn invalid_phase_step_plan_gets_one_correction_attempt() {
        let root = temp_workspace("phase-step-plan-correction");
        let ultra_yaml = "goal: \"Create Rust CLI\"\nprofile: \"rust\"\nstyle: \"default\"\nintent: \"new\"\nphases:\n  - id: \"main\"\n    goal: \"Create main file.\"\n";
        let invalid_step_yaml = "steps:\n  - id: \"write-main\"\n    kind: \"create\"\n    instruction: \"Create src/main.rs.\"\n    expected_paths:\n      - \"src/main.rs\"\n    verify:\n      - \"grep -q clap src/main.rs\"\n";
        let corrected_step_yaml = "steps:\n  - id: \"write-main\"\n    kind: \"create\"\n    instruction: |\n      Create src/main.rs.\n      Keep the implementation minimal.\n    expected_paths:\n      - \"src/main.rs\"\n    verify: []\n";
        let mut planner = MockClient::new(vec![
            ChatResponse {
                content: ultra_yaml.to_string(),
                tool_calls: Vec::new(),
                usage: Default::default(),
            },
            ChatResponse {
                content: invalid_step_yaml.to_string(),
                tool_calls: Vec::new(),
                usage: Default::default(),
            },
            ChatResponse {
                content: corrected_step_yaml.to_string(),
                tool_calls: Vec::new(),
                usage: Default::default(),
            },
        ]);
        let mut executor = MockClient::new(vec![
            ChatResponse {
                content: String::new(),
                tool_calls: vec![ToolCall {
                    id: None,
                    thought_signature: None,
                    name: "Write".to_string(),
                    args_json: r#"{"path":"src/main.rs","content":"fn main() {}\n"}"#.to_string(),
                }],
                usage: Default::default(),
            },
            ChatResponse {
                content: "Created src/main.rs.".to_string(),
                tool_calls: Vec::new(),
                usage: Default::default(),
            },
        ]);
        let command = SlashCommand {
            kind: SlashCommandKind::UltraPlanRun,
            profile: Some("rust".to_string()),
            style: None,
            intent: Some("new".to_string()),
            artifacts: vec!["src/main.rs".to_string()],
            argument: "Create Rust CLI".to_string(),
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

        assert!(output.contains("phase main: ok"), "{output}");
        assert_eq!(
            fs::read_to_string(root.join("src/main.rs")).unwrap(),
            "fn main() {}\n"
        );
    }

    struct MockClient {
        responses: VecDeque<ChatResponse>,
        prompts: Vec<String>,
    }

    impl MockClient {
        fn new(responses: Vec<ChatResponse>) -> Self {
            Self {
                responses: VecDeque::from(responses),
                prompts: Vec::new(),
            }
        }
    }

    impl ChatClient for MockClient {
        fn chat(&mut self, request: &ChatRequest) -> Result<ChatResponse, String> {
            self.prompts.push(
                request
                    .messages
                    .last()
                    .map(|message| message.content.clone())
                    .unwrap_or_default(),
            );
            self.responses
                .pop_front()
                .ok_or_else(|| "missing mock response".to_string())
        }
    }

    fn generated_step_context<'a>(
        goal: &'a str,
        profile: &'a str,
        intent: WorkIntent,
        required_artifacts: &'a [String],
        profile_obligations: &'a [ProfileObligation],
    ) -> GeneratedStepPlanContext<'a> {
        GeneratedStepPlanContext {
            goal,
            profile,
            style: "default",
            intent,
            required_artifacts,
            profile_obligations,
            task_contract: None,
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
