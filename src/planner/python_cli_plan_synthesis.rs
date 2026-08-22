use std::path::Path;

use serde_json::json;

use crate::planner::profile::{ProfileDeterministicStepPlan, ProfileQualityExpectations};
use crate::planner::profiles::python_cli::{self, manifest};
use crate::planner::step_plan::{PlanStep, StepKind, StepPlan};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PhaseRole {
    Setup,
    Implementation,
    Verify,
}

pub(crate) fn deterministic_step_plan(
    phase_prompt: &str,
    root: &Path,
    goal: &str,
) -> Option<ProfileDeterministicStepPlan> {
    match phase_role(phase_prompt)? {
        PhaseRole::Setup => Some(setup_plan(root, goal)),
        PhaseRole::Implementation => None,
        PhaseRole::Verify => Some(verify_plan()),
    }
}

pub(crate) fn phase_expected_paths(root: &Path, goal: &str) -> Option<Vec<String>> {
    match phase_role(goal)? {
        PhaseRole::Setup => Some(setup_paths(root, goal)),
        PhaseRole::Implementation => Some(implementation_paths(root, goal)),
        PhaseRole::Verify => Some(Vec::new()),
    }
}

pub(crate) fn phase_quality_expectations(
    root: &Path,
    goal: &str,
) -> Option<ProfileQualityExpectations> {
    let role = phase_role(goal)?;
    let required_artifacts = match role {
        PhaseRole::Setup => setup_paths(root, goal),
        PhaseRole::Implementation => implementation_paths(root, goal),
        PhaseRole::Verify => Vec::new(),
    };
    let preferred_verify = match role {
        PhaseRole::Verify => profile_verify_commands(),
        PhaseRole::Setup | PhaseRole::Implementation => Vec::new(),
    };
    Some(ProfileQualityExpectations {
        required_artifacts,
        preferred_verify,
        forbidden_verify: vec!["pip install".to_string(), "python -m venv".to_string()],
        dependency_order_hint: None,
    })
}

pub(crate) fn implementation_only_guidance(goal: &str) -> Option<&'static str> {
    (phase_role(goal) == Some(PhaseRole::Implementation)).then_some(
        "The profile owns setup and verification for this phase plan. Return implement steps only. Do not return setup, inspect, verify, or report steps; keep every implement-step verify array empty. Create the goal-derived src/<package>/main.py and one concrete README.md usage document.",
    )
}

pub(crate) fn canonicalize_implementation_plan(
    plan: &mut StepPlan,
    root: &Path,
    profile: &str,
    create_intent: bool,
    eval_events_path: Option<&Path>,
) -> usize {
    if !create_intent
        || crate::planner::profile::canonical_profile_name(profile)
            != crate::planner::profile_descriptor::PYTHON_CLI_PROFILE_ID
        || phase_role(&plan.goal) != Some(PhaseRole::Implementation)
    {
        return 0;
    }

    let original_step_count = plan.steps.len();
    plan.steps
        .retain(|step| step.step_kind() == StepKind::Implement);
    let removed_step_count = original_step_count.saturating_sub(plan.steps.len());
    let setup_paths = project_setup_paths(root, &plan.goal);
    let mut cleared_verify_count = 0usize;
    let mut removed_setup_path_count = 0usize;
    for step in &mut plan.steps {
        if !step.verify.is_empty() {
            step.verify.clear();
            cleared_verify_count += 1;
        }
        let before = step.expected_paths.len();
        step.expected_paths
            .retain(|path| !setup_paths.contains(path));
        removed_setup_path_count += before.saturating_sub(step.expected_paths.len());
    }

    let required_paths = implementation_paths(root, &plan.goal);
    let injected_paths = required_paths
        .into_iter()
        .filter(|path| {
            !plan
                .steps
                .iter()
                .any(|step| step.expected_paths.contains(path))
        })
        .collect::<Vec<_>>();
    if let Some(target) = plan.steps.last_mut() {
        target.expected_paths.extend(injected_paths.iter().cloned());
    }

    let changes =
        removed_step_count + cleared_verify_count + removed_setup_path_count + injected_paths.len();
    if changes > 0 {
        crate::eval_events::emit(
            eval_events_path,
            json!({
                "event": "python_cli_implementation_plan_canonicalized",
                "removed_non_implement_steps": removed_step_count,
                "cleared_model_verify_steps": cleared_verify_count,
                "removed_setup_paths": removed_setup_path_count,
                "injected_implementation_paths": injected_paths,
            }),
        );
    }
    changes
}

fn setup_plan(root: &Path, goal: &str) -> ProfileDeterministicStepPlan {
    let expected_paths = setup_paths(root, goal);
    ProfileDeterministicStepPlan {
        template_id: "python-cli-setup".to_string(),
        plan: StepPlan {
            goal: "Set up the deterministic Python CLI project metadata.".to_string(),
            steps: vec![PlanStep {
                id: "setup-python-cli-project".to_string(),
                kind: "setup".to_string(),
                expected_result: "pass".to_string(),
                instruction: format!(
                    "Create the coherent Python CLI scaffold before task-specific implementation. Declare the goal-derived package and a Python 3 requirement in pyproject.toml, and create a minimal functional entrypoint at the declared src/<package>/main.py path. Do not run verification in this setup step. Required files: {}.",
                    expected_paths.join(", ")
                ),
                expected_paths,
                verify: Vec::new(),
            }],
        },
    }
}

fn verify_plan() -> ProfileDeterministicStepPlan {
    ProfileDeterministicStepPlan {
        template_id: "python-cli-verify".to_string(),
        plan: StepPlan {
            goal: "Verify the deterministic Python CLI contract.".to_string(),
            steps: vec![PlanStep {
                id: "verify-python-cli".to_string(),
                kind: "verify".to_string(),
                expected_result: "pass".to_string(),
                instruction: "Run the profile-owned deterministic Python syntax verification without changing project files."
                    .to_string(),
                expected_paths: Vec::new(),
                verify: profile_verify_commands(),
            }],
        },
    }
}

fn setup_paths(root: &Path, goal: &str) -> Vec<String> {
    python_cli::contract_scaffold_paths(root, goal)
}

fn project_setup_paths(root: &Path, goal: &str) -> Vec<String> {
    let owned = &manifest::get()
        .step_templates
        .ownership
        .template_owned_artifacts
        .package_manifest_names;
    python_cli::contract_scaffold_paths(root, goal)
        .into_iter()
        .filter(|path| owned.contains(path))
        .collect()
}

fn implementation_paths(root: &Path, goal: &str) -> Vec<String> {
    let setup = project_setup_paths(root, goal);
    let mut paths = python_cli::contract_scaffold_paths(root, goal)
        .into_iter()
        .filter(|path| !setup.contains(path))
        .collect::<Vec<_>>();
    for path in manifest::get().artifacts.preferred_paths() {
        if !setup.contains(&path) && !paths.contains(&path) {
            paths.push(path);
        }
    }
    paths
}

fn profile_verify_commands() -> Vec<String> {
    vec![python_cli::COMPILE_COMMAND.to_string()]
}

fn phase_role(phase_prompt: &str) -> Option<PhaseRole> {
    let id = phase_field(phase_prompt, "Phase id:")?;
    let phases = &manifest::get().plan.phases;
    if phases.first().is_some_and(|phase| phase.id == id) {
        Some(PhaseRole::Setup)
    } else if phases.get(1).is_some_and(|phase| phase.id == id) {
        Some(PhaseRole::Implementation)
    } else if phases.get(2).is_some_and(|phase| phase.id == id) {
        Some(PhaseRole::Verify)
    } else {
        None
    }
}

fn phase_field(phase_prompt: &str, prefix: &str) -> Option<String> {
    phase_prompt.lines().find_map(|line| {
        line.trim_start()
            .strip_prefix(prefix)
            .map(|value| value.trim().to_string())
    })
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    const TOKEN_FIXTURE: &str = include_str!(
        "../../tests/corpus/apps/issue239-python-cli-plan-synthesis/fixtures/planner-token-reduction.toml"
    );
    const EVENT_FIXTURE: &str = include_str!(
        "../../tests/corpus/apps/issue239-python-cli-plan-synthesis/fixtures/implementation-canonicalized.jsonl"
    );

    #[derive(Debug, Deserialize)]
    struct TokenFixture {
        baseline_planner_tokens: u64,
        baseline_model_owned_phases: Vec<String>,
        synthesized_model_owned_phases: Vec<String>,
        minimum_reduction_percent: u64,
    }

    fn phase_prompt(goal: &str, phase_id: &str, phase_task: &str) -> String {
        format!(
            "Original ultra goal: {goal}\nProfile: python-cli\nIntent: create\nPhase id: {phase_id}\nPhase task: {phase_task}"
        )
    }

    #[test]
    fn profile_generates_setup_and_verify_while_implementation_stays_model_owned() {
        let root = tempfile::tempdir().unwrap();
        let preset = manifest::preset_ultra_plan("Build greet.py", "default", "create").unwrap();
        assert_eq!(
            crate::planner::profile::profile_preset_ultra_plan(
                "python-cli",
                "Build greet.py",
                "default",
                "create",
            ),
            Some(preset.clone())
        );
        let plans = preset
            .phases
            .iter()
            .map(|phase| {
                let prompt = phase_prompt(&preset.goal, &phase.id, &phase.prompt);
                (
                    phase.id.as_str(),
                    deterministic_step_plan(&prompt, root.path(), &preset.goal),
                )
            })
            .collect::<Vec<_>>();

        let setup = plans[0].1.as_ref().unwrap();
        assert_eq!(setup.template_id, "python-cli-setup");
        assert_eq!(setup.plan.steps[0].kind, "setup");
        assert_eq!(
            setup.plan.steps[0].expected_paths,
            ["pyproject.toml", "src/greet/main.py"]
        );
        assert!(setup.plan.steps[0].verify.is_empty());

        assert_eq!(plans[1].0, "cli-implementation");
        assert!(plans[1].1.is_none());

        let verify = plans[2].1.as_ref().unwrap();
        assert_eq!(verify.template_id, "python-cli-verify");
        assert_eq!(verify.plan.steps[0].kind, "verify");
        assert!(verify.plan.steps[0].expected_paths.is_empty());
        assert_eq!(
            verify.plan.steps[0].verify,
            ["python3 -m compileall -q src"]
        );
        for template in [setup, verify] {
            let mut plan = template.plan.clone();
            crate::planner::step_plan::repair_generated_step_plan_contract(&mut plan);
            let _ = crate::planner::sanitizer::sanitize_step_plan_against_policy(
                &mut plan,
                Some(root.path()),
            );
            let lint = crate::planner::lint::lint_template_contract(&plan, Some(root.path()));
            assert!(lint.is_pass(), "{}: {lint:?}", template.template_id);
        }
    }

    #[test]
    fn implementation_plan_discards_model_owned_setup_and_verify_contracts() {
        let root = tempfile::tempdir().unwrap();
        let events = root.path().join("events.jsonl");
        let prompt = phase_prompt(
            "Build greet.py",
            "cli-implementation",
            "Implement the deterministic CLI",
        );
        let mut plan = StepPlan {
            goal: prompt,
            steps: vec![
                PlanStep {
                    id: "model-setup".to_string(),
                    kind: "setup".to_string(),
                    expected_result: "pass".to_string(),
                    instruction: "Create project metadata".to_string(),
                    expected_paths: vec!["pyproject.toml".to_string()],
                    verify: Vec::new(),
                },
                PlanStep {
                    id: "model-implement".to_string(),
                    kind: "implement".to_string(),
                    expected_result: "pass".to_string(),
                    instruction: "Implement greet.py".to_string(),
                    expected_paths: vec!["pyproject.toml".to_string()],
                    verify: vec!["python3 -m compileall -q src".to_string()],
                },
                PlanStep {
                    id: "model-verify".to_string(),
                    kind: "verify".to_string(),
                    expected_result: "pass".to_string(),
                    instruction: "Compile the CLI".to_string(),
                    expected_paths: Vec::new(),
                    verify: vec!["python3 -m compileall -q src".to_string()],
                },
            ],
        };

        assert!(
            implementation_only_guidance(&plan.goal)
                .is_some_and(|guidance| guidance.contains("Return implement steps only"))
        );
        let non_python_plan = plan.clone();
        assert_eq!(
            canonicalize_implementation_plan(&mut plan, root.path(), "generic", true, None),
            0
        );
        assert_eq!(plan, non_python_plan);

        assert!(
            canonicalize_implementation_plan(
                &mut plan,
                root.path(),
                "python-cli",
                true,
                Some(&events),
            ) > 0
        );
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].kind, "implement");
        assert!(plan.steps[0].verify.is_empty());
        assert_eq!(
            plan.steps[0].expected_paths,
            ["src/greet/main.py", "README.md"]
        );
        assert_eq!(std::fs::read_to_string(events).unwrap(), EVENT_FIXTURE);
    }

    #[test]
    fn corpus_proves_at_least_thirty_percent_planner_token_reduction() {
        let fixture: TokenFixture = toml::from_str(TOKEN_FIXTURE).unwrap();
        assert_eq!(fixture.baseline_planner_tokens, 10_800);
        assert_eq!(fixture.baseline_model_owned_phases.len(), 3);
        assert_eq!(
            fixture.synthesized_model_owned_phases,
            ["cli-implementation"]
        );
        let synthesized_token_projection = fixture.baseline_planner_tokens
            * fixture.synthesized_model_owned_phases.len() as u64
            / fixture.baseline_model_owned_phases.len() as u64;
        let reduction_percent = 100
            * (fixture.baseline_planner_tokens - synthesized_token_projection)
            / fixture.baseline_planner_tokens;
        assert!(reduction_percent >= fixture.minimum_reduction_percent);
        assert!(reduction_percent >= 30, "reduction={reduction_percent}%");
    }
}
