use crate::agent::step_runner::ultra_plan::{UltraPhase, UltraPlan};
use crate::agent::step_runner::{StepPlan, plan_generation_prompt};
use std::fs;
use std::path::Path;

pub trait PhasePlanner {
    fn generate_step_plan(&mut self, prompt: &str) -> Result<StepPlan, String>;
}

pub trait StepPlanExecutor {
    fn execute_step_plan(&mut self, plan: &StepPlan) -> Result<(), String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UltraRunReport {
    pub completed_phases: usize,
    pub phases: Vec<PhaseRunReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseRunReport {
    pub phase_id: String,
    pub status: PhaseStatus,
    pub step_count: usize,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseStatus {
    Completed,
    Failed,
}

pub fn run_ultra_plan<P, E>(
    cwd: impl AsRef<Path>,
    plan: &UltraPlan,
    profile_contract: &str,
    planner: &mut P,
    executor: &mut E,
) -> UltraRunReport
where
    P: PhasePlanner,
    E: StepPlanExecutor,
{
    let snapshot = workspace_snapshot(cwd.as_ref());
    let mut report = UltraRunReport {
        completed_phases: 0,
        phases: Vec::new(),
    };

    for phase in &plan.phases {
        let prompt = phase_step_plan_prompt(plan, phase, &snapshot, profile_contract);
        let step_plan = match planner.generate_step_plan(&prompt) {
            Ok(step_plan) => step_plan,
            Err(err) => {
                report.phases.push(PhaseRunReport {
                    phase_id: phase.id.clone(),
                    status: PhaseStatus::Failed,
                    step_count: 0,
                    message: format!("planning failed: {err}"),
                });
                break;
            }
        };

        let step_count = step_plan.steps.len();
        match executor.execute_step_plan(&step_plan) {
            Ok(()) => {
                report.completed_phases += 1;
                report.phases.push(PhaseRunReport {
                    phase_id: phase.id.clone(),
                    status: PhaseStatus::Completed,
                    step_count,
                    message: "ok".to_string(),
                });
            }
            Err(err) => {
                report.phases.push(PhaseRunReport {
                    phase_id: phase.id.clone(),
                    status: PhaseStatus::Failed,
                    step_count,
                    message: format!("execution failed: {err}"),
                });
                break;
            }
        }
    }

    report
}

pub fn phase_step_plan_prompt(
    plan: &UltraPlan,
    phase: &UltraPhase,
    workspace_snapshot: &str,
    profile_contract: &str,
) -> String {
    format!(
        "{}\n\n\
Ultra phase context:\n\
- original goal: {original_goal}\n\
- current phase id: {phase_id}\n\
- current phase goal: {phase_goal}\n\n\
Workspace snapshot:\n{workspace_snapshot}\n\n\
Profile contract:\n{profile_contract}\n\n\
Create a step plan only for this phase. Do not redo completed phases.",
        plan_generation_prompt(&phase.goal, &plan.profile, &plan.style),
        original_goal = plan.goal,
        phase_id = phase.id,
        phase_goal = phase.goal,
    )
}

pub fn workspace_snapshot(cwd: &Path) -> String {
    let Ok(entries) = fs::read_dir(cwd) else {
        return "- snapshot unavailable".to_string();
    };
    let mut names = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                None
            } else if entry.path().is_dir() {
                Some(format!("{name}/"))
            } else {
                Some(name)
            }
        })
        .collect::<Vec<_>>();
    names.sort();
    names.truncate(20);
    if names.is_empty() {
        "- none detected".to_string()
    } else {
        names
            .into_iter()
            .map(|name| format!("- {name}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::step_runner::StepPlanStep;
    use std::collections::VecDeque;
    use std::path::PathBuf;

    #[test]
    fn runs_two_phase_fixture_to_completion() {
        let root = temp_workspace("complete");
        let plan = sample_ultra_plan();
        let mut planner =
            MockPlanner::new(vec![Ok(sample_step_plan("p1")), Ok(sample_step_plan("p2"))]);
        let mut executor = MockExecutor::new(vec![Ok(()), Ok(())]);

        let report = run_ultra_plan(&root, &plan, "profile rules", &mut planner, &mut executor);

        assert_eq!(report.completed_phases, 2);
        assert_eq!(report.phases.len(), 2);
        assert!(
            report
                .phases
                .iter()
                .all(|phase| phase.status == PhaseStatus::Completed)
        );
        assert_eq!(planner.prompts.len(), 2);
        assert_eq!(executor.executed_ids, vec!["p1", "p2"]);
    }

    #[test]
    fn stops_on_phase_failure_and_reports_message() {
        let root = temp_workspace("failure");
        let plan = sample_ultra_plan();
        let mut planner =
            MockPlanner::new(vec![Ok(sample_step_plan("p1")), Ok(sample_step_plan("p2"))]);
        let mut executor = MockExecutor::new(vec![Ok(()), Err("build failed".to_string())]);

        let report = run_ultra_plan(&root, &plan, "profile rules", &mut planner, &mut executor);

        assert_eq!(report.completed_phases, 1);
        assert_eq!(report.phases.len(), 2);
        assert_eq!(report.phases[1].status, PhaseStatus::Failed);
        assert!(report.phases[1].message.contains("build failed"));
    }

    #[test]
    fn phase_prompt_contains_snapshot_and_profile_contract() {
        let root = temp_workspace("prompt");
        fs::write(root.join("README.md"), "readme").unwrap();
        let plan = sample_ultra_plan();
        let snapshot = workspace_snapshot(&root);

        let prompt = phase_step_plan_prompt(&plan, &plan.phases[0], &snapshot, "Use Next.js.");

        assert!(prompt.contains("current phase id: scaffold"));
        assert!(prompt.contains("- README.md"));
        assert!(prompt.contains("Use Next.js."));
        assert!(prompt.contains("Do not redo completed phases"));
    }

    struct MockPlanner {
        responses: VecDeque<Result<StepPlan, String>>,
        prompts: Vec<String>,
    }

    impl MockPlanner {
        fn new(responses: Vec<Result<StepPlan, String>>) -> Self {
            Self {
                responses: VecDeque::from(responses),
                prompts: Vec::new(),
            }
        }
    }

    impl PhasePlanner for MockPlanner {
        fn generate_step_plan(&mut self, prompt: &str) -> Result<StepPlan, String> {
            self.prompts.push(prompt.to_string());
            self.responses
                .pop_front()
                .unwrap_or_else(|| Err("missing mock plan".to_string()))
        }
    }

    struct MockExecutor {
        responses: VecDeque<Result<(), String>>,
        executed_ids: Vec<String>,
    }

    impl MockExecutor {
        fn new(responses: Vec<Result<(), String>>) -> Self {
            Self {
                responses: VecDeque::from(responses),
                executed_ids: Vec::new(),
            }
        }
    }

    impl StepPlanExecutor for MockExecutor {
        fn execute_step_plan(&mut self, plan: &StepPlan) -> Result<(), String> {
            self.executed_ids.push(plan.steps[0].id.clone());
            self.responses
                .pop_front()
                .unwrap_or_else(|| Err("missing mock execution".to_string()))
        }
    }

    fn sample_ultra_plan() -> UltraPlan {
        UltraPlan {
            goal: "Build app".to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            intent: "new".to_string(),
            phases: vec![
                UltraPhase {
                    id: "scaffold".to_string(),
                    goal: "Create skeleton.".to_string(),
                },
                UltraPhase {
                    id: "build".to_string(),
                    goal: "Verify build.".to_string(),
                },
            ],
        }
    }

    fn sample_step_plan(id: &str) -> StepPlan {
        StepPlan {
            goal: "phase".to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            steps: vec![StepPlanStep {
                id: id.to_string(),
                instruction: "Do work.".to_string(),
                expected_paths: Vec::new(),
                verify: Vec::new(),
            }],
        }
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "commandagent-ultra-run-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
}
