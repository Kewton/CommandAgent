//! The saved package owner plus full host guidance exceeds one instruction's
//! existing limit. Admit an explicit, ordered split without truncating either
//! duty or asking unrelated owners to discharge it.
use super::*;
use crate::minimal_loop::evidence::package_script_check;
use crate::planner::recovery_contract_authority::verifier_obligations as scope;

pub(super) fn views(
    model: &StepPlan,
    host: &StepPlan,
    original: &StepPlan,
    proposed: &StepPlan,
) -> Option<(StepPlan, StepPlan)> {
    let (model_duty, host_duty) = split_duties(model, host)?;
    split_views(model, original, proposed, model_duty, host_duty)
}

/// The registered split's source gate is shared with bounded-repair proofs.
/// Eligibility alone does not prove that a proposed split is admissible.
pub(super) fn split_duties<'a>(
    model: &'a StepPlan,
    host: &'a StepPlan,
) -> Option<(&'a PlanStep, &'a PlanStep)> {
    let [host_duty] = host.steps.as_slice() else {
        return None;
    };
    let model_duty = model.steps.iter().find(|s| s.id == host_duty.id)?;
    if model_duty.expected_paths != ["package.json"]
        || host_duty.expected_paths != ["package.json"]
        || !host_duty.verify.is_empty()
        || [model_duty, host_duty]
            .iter()
            .any(|s| s.step_kind() != StepKind::Implement || s.expected_result != "pass")
        || !model_duty
            .verify
            .iter()
            .any(|c| package_script_check::printed_script(c).is_some())
    {
        return None;
    }
    Some((model_duty, host_duty))
}

fn split_views(
    model: &StepPlan,
    original: &StepPlan,
    proposed: &StepPlan,
    model_duty: &PlanStep,
    host_duty: &PlanStep,
) -> Option<(StepPlan, StepPlan)> {
    let owners = writers(proposed)?;
    let [(model_index, model_owner), (host_index, host_owner)] = owners.as_slice() else {
        return None;
    };
    if model_owner.id != model_duty.id
        || model_owner.instruction != model_duty.instruction
        || host_owner.instruction != host_duty.instruction
        || model.steps.iter().any(|s| s.id == host_owner.id)
        || host_owner.id.is_empty()
        || [*model_owner, *host_owner].iter().any(|s| {
            s.step_kind() != StepKind::Implement
                || s.expected_result != "pass"
                || s.expected_paths != ["package.json"]
                || s.verify.iter().any(|c| {
                    !crate::planner::profiles::nextjs::recovery_authority::is_package_check(c)
                })
        })
    {
        return None;
    }
    let original_owner = original.steps.iter().find(|s| s.id == model_duty.id)?;
    // Preserve the complete check group, including host-added checks, after
    // both executable writers. Views below do not erase this ordering proof.
    if !proposed.steps.iter().enumerate().any(|(index, step)| {
        index > *host_index
            && step.step_kind() == StepKind::Verify
            && step.expected_result == "pass"
            && original_owner
                .verify
                .iter()
                .all(|c| step.verify.contains(c))
    }) {
        return None;
    }
    let mut model_view = proposed.clone();
    model_view.steps.remove(*host_index);
    let mut host_view = proposed.clone();
    host_view.steps.remove(*model_index);
    Some((model_view, host_view))
}

fn writers(plan: &StepPlan) -> Option<Vec<(usize, &PlanStep)>> {
    if plan
        .steps
        .iter()
        .flat_map(|s| &s.expected_paths)
        .any(|p| scope::normalized(p).is_err())
    {
        return None;
    }
    Some(
        plan.steps
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                s.step_kind() != StepKind::Verify
                    && s.expected_paths
                        .iter()
                        .any(|p| scope::normalized(p).is_ok_and(|p| p == "package.json"))
            })
            .collect(),
    )
}

/// Lint can recognize only this concrete ordered configuration split. Admission
/// separately proves identity against the immutable model/host source snapshots.
/// This never authorizes a general duplicate output or an arbitrary second writer.
pub(crate) fn ordered_split(plan: &StepPlan, path: &str, first: &str, second: &str) -> bool {
    let Some(owners) = writers(plan) else {
        return false;
    };
    let [(model_index, model), (host_index, host)] = owners.as_slice() else {
        return false;
    };
    if path != "package.json"
        || model.id != first
        || host.id != second
        || first == second
        || *model_index >= *host_index
        || host.verify.is_empty()
        || [*model, *host].iter().any(|s| {
            s.step_kind() != StepKind::Implement
                || s.expected_result != "pass"
                || s.expected_paths != ["package.json"]
                || s.verify.iter().any(|c| {
                    !crate::planner::profiles::nextjs::recovery_authority::is_package_check(c)
                })
        })
    {
        return false;
    }
    let runtime = crate::planner::profile::resolve_profile_runtime(
        crate::planner::profile_descriptor::NEXTJS_PROFILE_ID,
    );
    if runtime.guidance(&host.instruction).as_deref() != Some(host.instruction.as_str()) {
        return false;
    }
    plan.steps.iter().enumerate().any(|(index, step)| {
        index > *host_index
            && step.step_kind() == StepKind::Verify
            && step.expected_result == "pass"
            && model
                .verify
                .iter()
                .chain(&host.verify)
                .all(|c| step.verify.contains(c))
            && step.verify.iter().any(|c| {
                package_script_check::comparison(c).is_some_and(|(script, value)| {
                    super::package_script_formation::fixed_values(&model.instruction, &script)
                        .is_ok_and(|values| {
                            !values.is_empty() && values.iter().all(|v| *v == value)
                        })
                })
            })
    })
}
