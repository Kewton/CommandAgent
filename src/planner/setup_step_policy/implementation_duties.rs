//! Keep broad implementation duties executable while retaining setup checks.
use super::{
    domain_profile, is_nextjs_profile, merge_unique_paths, profile_owns_declared_paths,
    profile_setup_checks, references_template_owned_artifacts,
};
use crate::minimal_loop::evidence::package_script_check;
use crate::planner::step_plan::{PlanStep, StepKind};
use std::path::Path;

// Broad profile implementation instructions cannot be discharged by the
// package/scaffold precheck subset. This only disables an optimization; the
// admission authority comes from the recorded augmentation, never this text.
pub(super) fn carries_profile_implementation(profile: &str, step: &PlanStep) -> bool {
    is_nextjs_profile(profile)
        && (step
            .instruction
            .starts_with("Update package.json scripts so that ")
            || has_package_observation(step)
            || step.instruction.contains("\n\nProfile contract:")
            || domain_profile(profile)
                .guidance("")
                .is_some_and(|guidance| {
                    guidance.split(". ").next().is_some_and(|opening| {
                        !opening.is_empty() && step.instruction.contains(opening)
                    })
                }))
}

fn has_package_observation(step: &PlanStep) -> bool {
    step.verify.iter().any(|c| {
        package_script_check::comparison(c).is_some()
            || step.step_kind() == StepKind::Verify
                && package_script_check::printed_script(c).is_some()
    })
}

pub(super) fn retain_implementation_checks(
    root: &Path,
    profile: &str,
    goal: &str,
    step: &mut PlanStep,
    phase_id: Option<&str>,
) -> bool {
    // Explicit checks are executable duties, not evidence that a profile
    // setup subset can replace this step. Preserve their scope and order;
    // sanitation, Admission and verification still enforce command policy.
    // Only the existing exact profile predicates for this goal may continue
    // through setup conversion; this does not broaden command equivalence.
    if !carries_profile_implementation(profile, step) {
        return is_nextjs_profile(profile)
            && ((!step.verify.is_empty() && step.expected_result != "pass")
                || step.verify.iter().any(|command| {
                    !crate::planner::profiles::nextjs::recovery_authority::final_verifier_covers_command(
                        goal, command,
                    )
                }));
    }
    // A declared observation already has its own target and output boundary.
    // Preserve it verbatim instead of replacing it with the profile subset.
    if step.step_kind() == StepKind::Verify && has_package_observation(step) {
        return true;
    }
    if references_template_owned_artifacts(profile, step)
        && profile_owns_declared_paths(root, profile, step)
        && let Some(checks) = profile_setup_checks(root, profile, goal, step, phase_id)
    {
        // Preserve the package checks as post-implementation requirements,
        // without treating their subset as proof of the entire Profile duty.
        merge_unique_paths(&mut step.expected_paths, checks.expected_paths);
        for command in checks.verify_commands {
            if !step.verify.contains(&command) {
                step.verify.push(command);
            }
        }
    }
    true
}
