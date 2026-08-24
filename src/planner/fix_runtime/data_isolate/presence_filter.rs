use std::path::Path;

use crate::planner::step_plan::{PlanStep, StepKind, StepPlan};
use crate::planner::ultra_plan::UltraPhase;

use super::super::FixRuntime;
use super::canonical_artifact_presence;

pub(in crate::planner::fix_runtime) fn bind_step_plan(
    runtime: Option<&FixRuntime>,
    phase: &UltraPhase,
    plan: &mut StepPlan,
) {
    let Some(runtime) = runtime.filter(|runtime| applies(&runtime.profile, phase)) else {
        return;
    };
    bind_for_workspace(
        &runtime.terminal_config.workspace_root,
        &runtime.profile,
        &runtime.goal,
        phase,
        plan,
    );
}

pub(in crate::planner::fix_runtime) fn bind_for_workspace(
    root: &Path,
    profile: &str,
    goal: &str,
    phase: &UltraPhase,
    plan: &mut StepPlan,
) {
    if !applies(profile, phase) {
        return;
    }
    let (_, absent) = canonical_artifact_presence(root, profile, goal);
    if absent.is_empty() {
        return;
    }
    plan.steps
        .retain(|step| write_capable(step) || !references_any_path(step, &absent));
    if plan.steps.is_empty() {
        plan.steps.push(PlanStep {
            id: "inspect-f1-existing-subject".to_string(),
            kind: "inspect".to_string(),
            expected_result: "pass".to_string(),
            instruction:
                "Read only the executed F1 failure evidence and its existing subject files."
                    .to_string(),
            expected_paths: Vec::new(),
            verify: Vec::new(),
        });
    }
}

fn applies(profile: &str, phase: &UltraPhase) -> bool {
    // FIX-9b: recovery/replan repair phases can reintroduce read references to
    // absent canonical artifacts (observed in dfix-003 schema/qwen A). Apply
    // the same presence filter there; write-capable steps remain untouched so
    // repair may create the artifact.
    crate::planner::profile::resolve_profile_runtime(profile).synthesizes_fix_plan()
        && (phase.id == "isolate-cause" || phase.id.starts_with("repair"))
}

fn write_capable(step: &PlanStep) -> bool {
    matches!(step.step_kind(), StepKind::Setup | StepKind::Implement)
}

fn references_any_path(step: &PlanStep, paths: &[String]) -> bool {
    paths.iter().any(|path| {
        step.expected_paths.iter().any(|expected| expected == path)
            || text_references_path(&step.instruction, path)
            || step
                .verify
                .iter()
                .any(|command| text_references_path(command, path))
    })
}

fn text_references_path(text: &str, path: &str) -> bool {
    let normalized = text.replace('\\', "/").to_ascii_lowercase();
    let path = path.to_ascii_lowercase();
    normalized.contains(&path)
        || Path::new(&path)
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| normalized.contains(name))
}

#[cfg(test)]
mod tests {
    use super::super::attach_for_workspace;
    use super::*;

    const RUN4_PLAN: &str = include_str!(
        "../../../../tests/corpus/apps/test0718_fix7a_existing_isolate_artifacts/fixtures/run4-isolate-plan.json"
    );

    fn isolate_phase() -> UltraPhase {
        UltraPhase {
            id: "isolate-cause".to_string(),
            prompt: "Isolate the schema failure without modifying the workspace.".to_string(),
        }
    }

    fn run4_workspace(with_inspection: bool) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("pipeline")).unwrap();
        std::fs::create_dir_all(root.path().join("output")).unwrap();
        std::fs::write(root.path().join("pipeline/main.py"), "raise ValueError()\n").unwrap();
        std::fs::write(root.path().join("output/results.json"), "{}\n").unwrap();
        if with_inspection {
            std::fs::write(root.path().join("output/inspection.json"), "{}\n").unwrap();
        }
        root
    }

    fn measured_plan() -> StepPlan {
        serde_json::from_str(RUN4_PLAN).unwrap()
    }

    #[test]
    fn absent_inspection_is_removed_from_run4_isolate_plan() {
        let root = run4_workspace(false);
        let phase = isolate_phase();
        let mut plan = measured_plan();

        bind_for_workspace(root.path(), "data", "fix results schema", &phase, &mut plan);

        assert_eq!(plan.steps.len(), 3);
        assert!(
            plan.steps
                .iter()
                .all(|step| !step.instruction.contains("inspection.json"))
        );
        assert!(
            plan.steps
                .iter()
                .any(|step| step.id == "inspect-results-json")
        );
        assert!(
            plan.steps
                .iter()
                .any(|step| step.id == "inspect-pipeline-main")
        );
        let lint = crate::planner::lint::lint_step_plan_report(&plan);
        assert!(lint.is_pass(), "{}", lint.primary_message());
    }

    #[test]
    fn existing_inspection_remains_available_to_isolate_plan() {
        let root = run4_workspace(true);
        let phase = isolate_phase();
        let mut plan = measured_plan();

        bind_for_workspace(root.path(), "data", "fix results schema", &phase, &mut plan);

        assert_eq!(plan.steps.len(), 4);
        assert!(
            plan.steps
                .iter()
                .any(|step| step.id == "inspect-inspection-json")
        );
    }

    #[test]
    fn absent_inspection_is_filtered_from_repair_replan_inspect_step() {
        let root = run4_workspace(false);
        let phase = UltraPhase {
            id: "repair".to_string(),
            prompt: "Repair the schema failure.".to_string(),
        };
        let mut plan = StepPlan {
            goal: "Repair results schema".to_string(),
            steps: vec![PlanStep {
                id: "inspect-workspace".to_string(),
                kind: "inspect".to_string(),
                expected_result: "pass".to_string(),
                instruction: "Read output/inspection.json and pipeline/main.py.".to_string(),
                expected_paths: Vec::new(),
                verify: Vec::new(),
            }],
        };

        bind_for_workspace(root.path(), "data", "fix results schema", &phase, &mut plan);

        assert_eq!(plan.steps.len(), 1);
        assert!(!plan.steps[0].instruction.contains("inspection.json"));
        let lint = crate::planner::lint::lint_step_plan_report(&plan);
        assert!(lint.is_pass(), "{}", lint.primary_message());
    }

    #[test]
    fn prompt_declares_f1_and_presence_bound_inputs() {
        let root = run4_workspace(false);
        let prompt = attach_for_workspace(
            root.path(),
            "data",
            "fix results schema",
            &isolate_phase(),
            "Fix F1 profile catalog failure: missing reconciliation".to_string(),
        );

        assert!(prompt.contains("Data fix cause-isolation artifact policy (runtime-bound):"));
        assert!(prompt.contains("executed F1 evidence"));
        assert!(prompt.contains("- pipeline/main.py"));
        assert!(prompt.contains("- output/results.json"));
        assert!(prompt.contains("Absent canonical artifacts"));
        assert!(prompt.contains("- output/inspection.json"));
        assert!(prompt.contains("Defer creation or regeneration"));
    }

    #[test]
    fn nextjs_fix_and_non_isolate_data_paths_are_byte_stable() {
        let root = run4_workspace(false);
        let isolate = isolate_phase();
        let repair = UltraPhase {
            id: "repair".to_string(),
            prompt: "Repair the schema failure.".to_string(),
        };
        let original = measured_plan();
        let mut nextjs = original.clone();
        bind_for_workspace(root.path(), "nextjs", "fix build", &isolate, &mut nextjs);
        assert_eq!(nextjs, original);
        assert_eq!(
            attach_for_workspace(
                root.path(),
                "nextjs",
                "fix build",
                &isolate,
                "base".to_string(),
            ),
            "base"
        );

        let mut data_repair = original.clone();
        bind_for_workspace(
            root.path(),
            "data",
            "fix results schema",
            &repair,
            &mut data_repair,
        );
        assert_ne!(data_repair, original);
        assert!(
            data_repair
                .steps
                .iter()
                .all(|step| !references_any_path(step, &["output/inspection.json".to_string()]))
        );
    }
}
