use crate::agent::step_runner::runtime::phase_contract::PhaseWorkspaceContract;
use crate::agent::step_runner::ultra_plan::{UltraPhase, UltraPlan};
use crate::agent::step_runner::{StepPlan, WorkIntent, plan_generation_prompt};
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
    let cwd = cwd.as_ref();
    let mut report = UltraRunReport {
        completed_phases: 0,
        phases: Vec::new(),
    };

    for phase in &plan.phases {
        let snapshot = workspace_snapshot(cwd);
        let phase_artifacts = phase_owned_artifacts(plan, phase);
        let phase_contract = PhaseWorkspaceContract::collect_with_scope(
            cwd,
            &plan.profile,
            &plan.required_artifacts,
            &phase_artifacts,
            &phase.preserve_artifacts,
            &phase.verify_only_artifacts,
            &format!("{} {}", plan.goal, phase.goal),
        )
        .render();
        let prompt =
            phase_step_plan_prompt(plan, phase, &snapshot, &phase_contract, profile_contract);
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
    phase_contract: &str,
    profile_contract: &str,
) -> String {
    let phase_artifacts = phase_owned_artifacts(plan, phase);
    let phase_artifact_list = bullet_list(&phase_artifacts);
    let preserve_artifact_list = bullet_list(&phase.preserve_artifacts);
    let verify_only_artifact_list = bullet_list(&phase.verify_only_artifacts);
    let global_artifact_list = bullet_list(&plan.required_artifacts);
    format!(
        "{}\n\n\
Ultra phase context:\n\
- current phase owned artifacts:\n{phase_artifact_list}\n\
- current phase preserve artifacts:\n{preserve_artifact_list}\n\
- current phase verify-only artifacts:\n{verify_only_artifact_list}\n\
- global final artifacts:\n{global_artifact_list}\n\
- Only put current phase owned artifacts in this phase step plan's required_artifacts and mutation steps.\n\
- Preserve artifacts may be inspected but must not be listed as mutation targets unless a dedicated repair/setup reason grants authority.\n\
- Verify-only artifacts may appear in verification context, not as create/edit targets for this phase.\n\
- Treat global final artifacts outside this phase as final conditions or existing context, not as create/edit targets.\n\
- original goal: {original_goal}\n\
- current phase id: {phase_id}\n\
- current phase goal: {phase_goal}\n\n\
Workspace snapshot:\n{workspace_snapshot}\n\n\
Phase workspace contract:\n{phase_contract}\n\n\
Profile contract:\n{profile_contract}\n\n\
Create a step plan only for this phase. Do not redo completed phases.",
        plan_generation_prompt(
            &phase.goal,
            &plan.profile,
            &plan.style,
            WorkIntent::parse(&plan.intent).unwrap_or(WorkIntent::Unknown),
            &phase_artifacts,
        ),
        original_goal = plan.goal,
        phase_id = phase.id,
        phase_goal = phase.goal,
    )
}

pub fn phase_owned_artifacts(plan: &UltraPlan, phase: &UltraPhase) -> Vec<String> {
    let mut owned = phase.owned_artifacts.clone();
    for artifact in plan
        .required_artifacts
        .iter()
        .filter(|artifact| artifact_matches_phase_goal(artifact, &phase.goal))
    {
        if !owned.iter().any(|owned_path| owned_path == artifact) {
            owned.push(artifact.clone());
        }
    }
    owned
}

fn artifact_matches_phase_goal(artifact: &str, goal: &str) -> bool {
    let goal = goal.to_ascii_lowercase();
    let artifact = artifact.to_ascii_lowercase();
    let basename = artifact.rsplit('/').next().unwrap_or(artifact.as_str());
    if goal.contains(&artifact) || goal.contains(basename) {
        return true;
    }
    artifact == "package.json"
        && [
            "package",
            "dependencies",
            "dependency",
            "project structure",
            "initialize",
        ]
        .iter()
        .any(|needle| goal.contains(needle))
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
    use crate::agent::step_runner::{ExpectedResult, StepKind};
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

        let prompt = phase_step_plan_prompt(
            &plan,
            &plan.phases[0],
            &snapshot,
            "- required_artifacts=app/page.tsx",
            "Use Next.js.",
        );

        assert!(prompt.contains("current phase id: scaffold"));
        assert!(prompt.contains("- README.md"));
        assert!(prompt.contains("current phase owned artifacts"));
        assert!(prompt.contains("current phase preserve artifacts"));
        assert!(prompt.contains("current phase verify-only artifacts"));
        assert!(prompt.contains("global final artifacts"));
        assert!(prompt.contains("Phase workspace contract"));
        assert!(prompt.contains("required_artifacts=app/page.tsx"));
        assert!(prompt.contains("Use Next.js."));
        assert!(prompt.contains("Do not redo completed phases"));
    }

    #[test]
    fn phase_owned_artifacts_are_inferred_from_phase_goal() {
        let plan = UltraPlan {
            goal: "Create a Next.js app".to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            intent: "new".to_string(),
            required_artifacts: vec![
                "package.json".to_string(),
                "postcss.config.js".to_string(),
                "tailwind.config.ts".to_string(),
                "app/globals.css".to_string(),
                "app/layout.tsx".to_string(),
                "app/page.tsx".to_string(),
            ],
            phases: vec![UltraPhase {
                id: "layout".to_string(),
                goal: "Create app/layout.tsx and app/globals.css for base layout.".to_string(),
                owned_artifacts: Vec::new(),
                preserve_artifacts: Vec::new(),
                verify_only_artifacts: Vec::new(),
            }],
        };

        let owned = phase_owned_artifacts(&plan, &plan.phases[0]);

        assert_eq!(
            owned,
            vec!["app/globals.css".to_string(), "app/layout.tsx".to_string()]
        );
    }

    #[test]
    fn phase_owned_artifacts_prefer_explicit_scope() {
        let plan = UltraPlan {
            goal: "Create a Next.js app".to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            intent: "new".to_string(),
            required_artifacts: vec!["app/page.tsx".to_string()],
            phases: vec![UltraPhase {
                id: "ui".to_string(),
                goal: "Work on the page.".to_string(),
                owned_artifacts: vec!["app/components/Card.tsx".to_string()],
                preserve_artifacts: vec!["package.json".to_string()],
                verify_only_artifacts: vec!["tests/ui.test.ts".to_string()],
            }],
        };

        let owned = phase_owned_artifacts(&plan, &plan.phases[0]);

        assert_eq!(owned, vec!["app/components/Card.tsx"]);
    }

    #[test]
    fn phase_owned_artifacts_merge_explicit_scope_with_goal_matched_required_artifacts() {
        let plan = UltraPlan {
            goal: "Create a Next.js app".to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            intent: "new".to_string(),
            required_artifacts: vec!["package.json".to_string(), "app/page.tsx".to_string()],
            phases: vec![UltraPhase {
                id: "setup".to_string(),
                goal: "Initialize project structure and dependencies.".to_string(),
                owned_artifacts: vec!["app/page.tsx".to_string()],
                preserve_artifacts: Vec::new(),
                verify_only_artifacts: Vec::new(),
            }],
        };

        let owned = phase_owned_artifacts(&plan, &plan.phases[0]);

        assert_eq!(
            owned,
            vec!["app/page.tsx".to_string(), "package.json".to_string()]
        );
    }

    #[test]
    fn refreshes_workspace_snapshot_before_each_phase() {
        let root = temp_workspace("refresh");
        let plan = sample_ultra_plan();
        let mut planner =
            MockPlanner::new(vec![Ok(sample_step_plan("p1")), Ok(sample_step_plan("p2"))]);
        let mut executor = CreatingExecutor {
            root: root.clone(),
            inner: MockExecutor::new(vec![Ok(()), Ok(())]),
        };

        let report = run_ultra_plan(&root, &plan, "profile rules", &mut planner, &mut executor);

        assert_eq!(report.completed_phases, 2);
        assert_eq!(planner.prompts.len(), 2);
        assert!(!planner.prompts[0].contains("phase1.txt"));
        assert!(planner.prompts[1].contains("phase1.txt"));
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

    struct CreatingExecutor {
        root: PathBuf,
        inner: MockExecutor,
    }

    impl StepPlanExecutor for CreatingExecutor {
        fn execute_step_plan(&mut self, plan: &StepPlan) -> Result<(), String> {
            let result = self.inner.execute_step_plan(plan);
            if plan.steps[0].id == "p1" {
                fs::write(self.root.join("phase1.txt"), "done").unwrap();
            }
            result
        }
    }

    fn sample_ultra_plan() -> UltraPlan {
        UltraPlan {
            goal: "Build app".to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            intent: "new".to_string(),
            required_artifacts: vec!["app/page.tsx".to_string()],
            phases: vec![
                UltraPhase {
                    id: "scaffold".to_string(),
                    goal: "Create skeleton.".to_string(),
                    owned_artifacts: Vec::new(),
                    preserve_artifacts: Vec::new(),
                    verify_only_artifacts: Vec::new(),
                },
                UltraPhase {
                    id: "build".to_string(),
                    goal: "Verify build.".to_string(),
                    owned_artifacts: Vec::new(),
                    preserve_artifacts: Vec::new(),
                    verify_only_artifacts: Vec::new(),
                },
            ],
        }
    }

    fn sample_step_plan(id: &str) -> StepPlan {
        StepPlan {
            goal: "phase".to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            intent: WorkIntent::New,
            required_artifacts: Vec::new(),
            steps: vec![StepPlanStep {
                id: id.to_string(),
                kind: StepKind::Create,
                instruction: "Do work.".to_string(),
                expected_result: ExpectedResult::Pass,
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
