//! Apply profile additions while recording their independent origin.
use super::formation_scope::ProfileAddition;
use super::profile_contract_reference::PROFILE_CONTRACT_REFERENCE;
use super::*;
use crate::planner::profile::resolve_profile_runtime;
use crate::planner::recovery_contract_authority::verifier_obligations;

pub(crate) fn strengthen_step_plan_for_profile(
    plan: &mut StepPlan,
    config: &Config,
) -> Option<ProfileAddition> {
    let runtime = resolve_profile_runtime(&config.profile);
    let is_scaffold = plan.goal.to_ascii_lowercase().contains("scaffold");
    // A step that creates a verifier script is a producer for admission, which
    // rejects any application/configuration path beside the script.
    let scripts = verifier_obligations::scripts(&verifier_obligations::commands(plan));
    let owns_scope =
        |step: &PlanStep| matches!(step.step_kind(), StepKind::Setup | StepKind::Implement);
    // Prefer an owner that is not a producer; a plan with only producers keeps
    // the last Setup/Implement step so admission can still ask for a rebuild.
    let target_index = plan
        .steps
        .iter()
        .rposition(|step| {
            owns_scope(step) && !(skip_producers() && produces_verifier(step, &scripts))
        })
        .or_else(|| plan.steps.iter().rposition(owns_scope))
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
    // The host duty is the exact text a step must retain: the short reference
    // by default, or the full guidance when the model already carried it.
    let mut duty = String::new();
    if !guidance.is_empty() {
        if target.instruction.contains(&guidance) {
            duty = guidance;
        } else if guidance.starts_with(&target.instruction) {
            // A model may have moved the host guidance verbatim to a new owner.
            // Extend an exact prefix without duplicating it past the text limit.
            target.instruction = guidance.clone();
            duty = guidance;
        } else {
            duty = PROFILE_CONTRACT_REFERENCE.to_string();
            if !target.instruction.contains(&duty) {
                target.instruction =
                    format!("{}\n\nProfile contract:\n{}", target.instruction, duty);
            }
        }
    }
    if *target == model && duty.is_empty() {
        return None;
    }
    Some(ProfileAddition::new(model, target, duty))
}

fn produces_verifier(step: &PlanStep, scripts: &std::collections::BTreeSet<String>) -> bool {
    step.expected_paths
        .iter()
        .any(|p| verifier_obligations::normalized(p).is_ok_and(|p| scripts.contains(&p)))
}

#[cfg(not(test))]
fn skip_producers() -> bool {
    true
}

#[cfg(test)]
fn skip_producers() -> bool {
    !LEGACY_PRE_831_OWNER.with(std::cell::Cell::get)
}

#[cfg(test)]
thread_local! {
    static LEGACY_PRE_831_OWNER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Restores the owner selection of #831 when dropped, even on panic.
#[cfg(test)]
pub(crate) struct LegacyPre831OwnerGuard(bool);

#[cfg(test)]
impl Drop for LegacyPre831OwnerGuard {
    fn drop(&mut self) {
        LEGACY_PRE_831_OWNER.with(|flag| flag.set(self.0));
    }
}

/// Test-only: reproduce the pre-#831 owner choice (the last Setup/Implement
/// step, even a verifier producer) that the recorded #478 fixtures were
/// written against. It is thread-local, so it cannot leak into other tests.
#[cfg(test)]
pub(crate) fn legacy_pre_831_owner_for_issue478_fixtures() -> LegacyPre831OwnerGuard {
    LegacyPre831OwnerGuard(LEGACY_PRE_831_OWNER.with(|flag| flag.replace(true)))
}
