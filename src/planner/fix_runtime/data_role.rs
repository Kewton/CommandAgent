use std::collections::BTreeSet;
use std::path::Path;

use crate::planner::adjudication::contract::is_fix_intent;
use crate::planner::profile::profile_expected_paths;
use crate::planner::repair_targeting::{
    RepairTargetPriority, RepairTargetResolutionInput, RepairTargetSelection,
    resolve_repair_targets,
};
use crate::planner::step_plan::{PlanStep, StepKind, StepPlan};
use crate::planner::ultra_plan::{UltraPhase, UltraPlan};

use super::FixRuntime;

const ROLE_HEADING: &str = "Data fix phase-role boundary (runtime-bound):";

#[derive(Debug, Default)]
pub(super) struct DataRolePolicy {
    enabled: bool,
    repair_phase_id: Option<String>,
    deferred_steps: Vec<PlanStep>,
}

impl DataRolePolicy {
    pub(super) fn for_plan(plan: &UltraPlan) -> Self {
        let enabled = crate::planner::profile::resolve_profile_runtime(&plan.profile)
            .synthesizes_fix_plan()
            && is_fix_intent(&plan.intent);
        let repair_phase_id = enabled.then(|| {
            plan.phases
                .iter()
                .skip_while(|phase| phase.id != "isolate-cause")
                .skip(1)
                .find(|phase| matches!(phase.id.as_str(), "repair" | "implement-fix"))
                .map(|phase| phase.id.clone())
        });
        Self {
            enabled,
            repair_phase_id: repair_phase_id.flatten(),
            deferred_steps: Vec::new(),
        }
    }

    fn bind(
        &mut self,
        root: &Path,
        profile: &str,
        goal: &str,
        phase: &UltraPhase,
        mapped_selection: Option<&RepairTargetSelection>,
        plan: &mut StepPlan,
    ) {
        if !self.enabled {
            return;
        }
        if phase.id == "isolate-cause" {
            self.normalize_isolate(root, profile, goal, mapped_selection, plan);
        } else if self.repair_phase_id.as_deref() == Some(phase.id.as_str()) {
            self.prepend_deferred(plan);
        }
    }

    fn normalize_isolate(
        &mut self,
        root: &Path,
        profile: &str,
        goal: &str,
        mapped_selection: Option<&RepairTargetSelection>,
        plan: &mut StepPlan,
    ) {
        let mut retained = Vec::new();
        for mut step in std::mem::take(&mut plan.steps) {
            if !write_role_leak(&step) {
                retained.push(step);
                continue;
            }
            normalize_write_step(root, profile, goal, mapped_selection, &mut step);
            // Prefer transfer so cause isolation remains read-only. Reclassify in place only
            // for a non-standard fix plan that has no later repair/implement-fix phase.
            if self.repair_phase_id.is_some() {
                self.deferred_steps.push(step);
            } else {
                retained.push(step);
            }
        }
        if retained.is_empty() {
            retained.push(fallback_isolate_step());
        }
        plan.steps = retained;
    }

    fn prepend_deferred(&mut self, plan: &mut StepPlan) {
        if self.deferred_steps.is_empty() {
            return;
        }
        let mut used = plan
            .steps
            .iter()
            .map(|step| step.id.clone())
            .collect::<BTreeSet<_>>();
        let mut deferred = std::mem::take(&mut self.deferred_steps);
        for step in &mut deferred {
            step.id = unique_deferred_id(&step.id, &mut used);
        }
        deferred.append(&mut plan.steps);
        plan.steps = deferred;
    }
}

pub(super) fn attach_to_phase_prompt(
    runtime: Option<&FixRuntime>,
    phase: &UltraPhase,
    prompt: String,
) -> String {
    let Some(runtime) = runtime.filter(|runtime| {
        runtime.profile == "data" && runtime.data_role_policy.enabled && phase.id == "isolate-cause"
    }) else {
        return prompt;
    };
    attach_for_profile(
        &runtime.profile,
        runtime.data_role_policy.enabled,
        phase,
        prompt,
    )
}

fn attach_for_profile(
    profile: &str,
    enabled: bool,
    phase: &UltraPhase,
    mut prompt: String,
) -> String {
    if profile != "data" || !enabled || phase.id != "isolate-cause" {
        return prompt;
    }
    prompt.push_str("\n\n");
    prompt.push_str(ROLE_HEADING);
    prompt.push_str(
        "\n- Return read-only cause-isolation steps only; do not place implementation, file edits, artifact generation, or repair verification in this phase.\n- Put every workspace-changing action in the later repair phase, where implement steps receive write authority.",
    );
    prompt
}

pub(super) fn bind_step_plan(runtime: &mut FixRuntime, phase: &UltraPhase, plan: &mut StepPlan) {
    let mapped_selection = runtime
        .diagnostic
        .as_ref()
        .map(|diagnostic| RepairTargetSelection {
            selected_targets: vec![diagnostic.target_path.clone()],
            selection_reason: diagnostic.selection_reason,
        });
    runtime.data_role_policy.bind(
        &runtime.terminal_config.workspace_root,
        &runtime.profile,
        &runtime.goal,
        phase,
        mapped_selection.as_ref(),
        plan,
    );
}

fn write_role_leak(step: &PlanStep) -> bool {
    matches!(step.step_kind(), StepKind::Setup | StepKind::Implement)
        || crate::planner::lint::looks_like_file_change_instruction(&step.instruction)
}

fn normalize_write_step(
    root: &Path,
    profile: &str,
    goal: &str,
    mapped_selection: Option<&RepairTargetSelection>,
    step: &mut PlanStep,
) {
    step.kind = "implement".to_string();
    step.expected_result = "pass".to_string();
    let fallback_paths = profile_expected_paths(root, profile, goal)
        .into_iter()
        .filter(|path| root.join(path).is_file())
        .collect::<Vec<_>>();
    if let Some(selection) = resolve_repair_targets(RepairTargetResolutionInput {
        root,
        profile,
        pending_evidence: &[],
        missing_capabilities: &[],
        contract_attribute_paths: &[],
        repair_changed_paths: &[],
        required_paths: &step.expected_paths,
        fallback_paths: &fallback_paths,
        mapped_selection,
        priority: RepairTargetPriority::FixIntent,
    }) {
        for target in selection.selected_targets.into_iter().rev() {
            if !step.expected_paths.contains(&target) {
                step.expected_paths.insert(0, target);
            }
        }
    }
}

fn unique_deferred_id(id: &str, used: &mut BTreeSet<String>) -> String {
    if used.insert(id.to_string()) {
        return id.to_string();
    }
    for suffix in 1.. {
        let candidate = format!("{id}-deferred-{suffix}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
    }
    unreachable!()
}

fn fallback_isolate_step() -> PlanStep {
    PlanStep {
        id: "inspect-f1-existing-subject".to_string(),
        kind: "inspect".to_string(),
        expected_result: "pass".to_string(),
        instruction: "Read only the executed F1 failure evidence and its existing subject files."
            .to_string(),
        expected_paths: Vec::new(),
        verify: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::repair_target_selection::RepairTargetSelectionReason;

    const RUN2_PLAN: &str = include_str!(
        "../../../tests/corpus/apps/test0718_fix7b_data_isolate_roles/fixtures/run2-isolate-plan.json"
    );

    fn explicit_data_fix_plan() -> UltraPlan {
        crate::planner::intent::explicit_fix_plan("fix pipeline failure", "data", "default")
    }

    fn workspace() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("pipeline")).unwrap();
        std::fs::create_dir_all(root.path().join("output")).unwrap();
        std::fs::write(root.path().join("pipeline/main.py"), "raise TypeError()\n").unwrap();
        std::fs::write(root.path().join("output/inspection.json"), "{}\n").unwrap();
        std::fs::write(root.path().join("output/results.json"), "{}\n").unwrap();
        root
    }

    fn measured_isolate_plan() -> StepPlan {
        serde_json::from_str(RUN2_PLAN).unwrap()
    }

    fn traceback_selection() -> RepairTargetSelection {
        RepairTargetSelection {
            selected_targets: vec!["pipeline/main.py".to_string()],
            selection_reason: RepairTargetSelectionReason::TracebackMapped,
        }
    }

    #[test]
    fn run2_implement_step_moves_to_write_authorized_repair_phase() {
        let root = workspace();
        let ultra = explicit_data_fix_plan();
        let isolate = &ultra.phases[1];
        let repair = &ultra.phases[2];
        let mut policy = DataRolePolicy::for_plan(&ultra);
        let mut isolate_plan = measured_isolate_plan();
        super::super::data_isolate::bind_for_workspace(
            root.path(),
            "data",
            &ultra.goal,
            isolate,
            &mut isolate_plan,
        );

        policy.bind(
            root.path(),
            "data",
            &ultra.goal,
            isolate,
            Some(&traceback_selection()),
            &mut isolate_plan,
        );

        assert_eq!(
            isolate_plan
                .steps
                .iter()
                .map(|step| step.id.as_str())
                .collect::<Vec<_>>(),
            ["inspect-source"]
        );
        assert!(isolate_plan.steps.iter().all(|step| !write_role_leak(step)));
        assert_eq!(policy.deferred_steps.len(), 1);
        let isolate_lint = crate::planner::lint::lint_step_plan_report(&isolate_plan);
        assert!(isolate_lint.is_pass(), "{}", isolate_lint.primary_message());

        let mut repair_plan = StepPlan {
            goal: "Repair the reproduced TypeError.".to_string(),
            steps: vec![PlanStep {
                id: "verify-repair-target".to_string(),
                kind: "verify".to_string(),
                expected_result: "pass".to_string(),
                instruction: "Run the deterministic pipeline check.".to_string(),
                expected_paths: Vec::new(),
                verify: vec!["python3 -B pipeline/main.py".to_string()],
            }],
        };
        policy.bind(
            root.path(),
            "data",
            &ultra.goal,
            repair,
            Some(&traceback_selection()),
            &mut repair_plan,
        );

        let moved = &repair_plan.steps[0];
        assert_eq!(moved.id, "fix-append-error");
        assert_eq!(moved.step_kind(), StepKind::Implement);
        assert!(
            moved
                .expected_paths
                .contains(&"pipeline/main.py".to_string())
        );
        assert!(policy.deferred_steps.is_empty());
        let repair_lint = crate::planner::lint::lint_step_plan_report(&repair_plan);
        assert!(repair_lint.is_pass(), "{}", repair_lint.primary_message());
    }

    #[test]
    fn nonstandard_plan_without_repair_reclassifies_in_place() {
        let root = workspace();
        let ultra = UltraPlan {
            goal: "fix pipeline failure".to_string(),
            profile: "data".to_string(),
            style: "default".to_string(),
            intent: "fix".to_string(),
            phases: vec![
                UltraPhase {
                    id: "reproduce-before".to_string(),
                    prompt: "Reproduce.".to_string(),
                },
                UltraPhase {
                    id: "isolate-cause".to_string(),
                    prompt: "Isolate.".to_string(),
                },
            ],
        };
        let mut policy = DataRolePolicy::for_plan(&ultra);
        let mut plan = StepPlan {
            goal: "isolate".to_string(),
            steps: vec![PlanStep {
                id: "misclassified-write".to_string(),
                kind: "inspect".to_string(),
                expected_result: "pass".to_string(),
                instruction: "Modify pipeline/main.py to repair the TypeError.".to_string(),
                expected_paths: Vec::new(),
                verify: Vec::new(),
            }],
        };

        policy.bind(
            root.path(),
            "data",
            &ultra.goal,
            &ultra.phases[1],
            Some(&traceback_selection()),
            &mut plan,
        );

        assert_eq!(plan.steps[0].step_kind(), StepKind::Implement);
        assert_eq!(plan.steps[0].expected_paths, ["pipeline/main.py"]);
        assert!(policy.deferred_steps.is_empty());
    }

    #[test]
    fn role_prompt_and_nextjs_non_application_are_stable() {
        let phase = UltraPhase {
            id: "isolate-cause".to_string(),
            prompt: "Isolate.".to_string(),
        };
        let prompt = attach_for_profile("data", true, &phase, "base".to_string());
        assert!(prompt.contains(ROLE_HEADING));
        assert!(prompt.contains("later repair phase"));
        assert!(prompt.contains("write authority"));
        assert_eq!(
            attach_for_profile("nextjs", false, &phase, "base".to_string()),
            "base"
        );

        let ultra = crate::planner::intent::explicit_fix_plan("fix build", "nextjs", "default");
        let policy = DataRolePolicy::for_plan(&ultra);
        assert!(!policy.enabled);
        assert!(policy.repair_phase_id.is_none());
    }
}
