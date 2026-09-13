//! Apply profile additions while recording their independent origin.
use super::formation_scope::ProfileAddition;
use super::*;
use crate::planner::profile::resolve_profile_runtime;

pub(crate) fn strengthen_step_plan_for_profile(
    plan: &mut StepPlan,
    config: &Config,
) -> Option<ProfileAddition> {
    let runtime = resolve_profile_runtime(&config.profile);
    let is_scaffold = plan.goal.to_ascii_lowercase().contains("scaffold");
    let target_index = plan
        .steps
        .iter()
        .rposition(|step| matches!(step.step_kind(), StepKind::Setup | StepKind::Implement))
        .or_else(|| {
            is_scaffold.then(|| {
                plan.steps
                    .iter()
                    .rposition(|step| step.step_kind() == StepKind::Report)
            })?
        })?;
    let target = &mut plan.steps[target_index];
    let model = target.clone();
    if is_scaffold {
        for path in runtime.expected_scaffold_paths(&config.workspace_root, &plan.goal) {
            if path.ends_with("package.json") && !target.expected_paths.contains(&path) {
                target.expected_paths.push(path);
            }
        }
        if !target.expected_paths.is_empty() && target.kind == "report" {
            target.kind = "implement".to_string();
        }
    }
    let guidance = runtime.guidance(&plan.goal).unwrap_or_default();
    if !guidance.is_empty() && !target.instruction.contains(&guidance) {
        if guidance.starts_with(&target.instruction) {
            // A model may have moved the host guidance verbatim to a new owner.
            // Extend an exact prefix without duplicating it past the text limit.
            target.instruction = guidance.clone();
        } else {
            target.instruction =
                format!("{}\n\nProfile contract:\n{}", target.instruction, guidance);
        }
    }
    if *target == model && guidance.is_empty() {
        return None;
    }
    Some(ProfileAddition::new(model, target, guidance))
}
