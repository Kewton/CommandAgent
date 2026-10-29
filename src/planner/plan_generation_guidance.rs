//! Planner guidance toward the verifier/app owner split admission already
//! accepts (Issue #906). Wording only: the separation rule, `preserve`, the
//! 3-attempt budget and fallback stay in `recovery_step_plan_binding`.

use crate::planner::step_plan::{PlanStep, StepKind, StepPlan};

pub(crate) const ORIGINAL_CONTEXT: &str = "Original requirement context:";

pub(crate) fn system_prompt() -> String {
    [
        "You are a deterministic planning component for a local coding agent.",
        "Return only one JSON object. Do not include markdown fences, prose, comments, or XML.",
        "The JSON object must have top-level keys: goal, steps.",
        "Each step must include id, kind, instruction, expected_paths, verify, and expected_result.",
        "Step id must be a quoted string. Use kebab-case such as inspect-workspace.",
        "expected_result must be exactly \"pass\" or \"fail\". Do not put prose in expected_result.",
        "Use 2 to 8 steps for normal implementation tasks, with a maximum of 12 steps.",
        "Allowed step kinds: inspect, setup, implement, verify, report.",
        "Use inspect only for reading or assessing state; it must not declare expected_paths or verify commands.",
        "Use setup for dependency manifests or setup files; do not run build/test verification in setup.",
        "Use implement/create/edit/work/repair semantics as implement steps, with concrete expected_paths.",
        "Use verify only for deterministic checks. Do not request file changes in verify steps.",
        "Use report only for explicit blockers such as dependency_missing, unavailable external service, required user input, or unfixable local blocker. Report is not success.",
        "Normal success plans must end with a verify step or an artifact-owning setup/implement step, not a final summary report.",
        "Expected paths must be workspace-relative, exact, and owned by one implement/setup step.",
        "Do not put dependency/build side-effect directories such as node_modules, .next, __pycache__, .venv, venv, dist, build, target, coverage, or out in expected_paths unless the user goal explicitly asks for that artifact path.",
        "Prefer tests, builds, smoke checks, or content assertions over file existence checks.",
        "Use file existence checks only as a fallback when no stronger deterministic verification fits.",
        "Implement/setup instructions should say what each expected artifact will contain.",
        "Verify commands must not use shell control syntax such as &&, ||, |, or ;.",
        "For multi-statement Node checks, create a smoke-check.js artifact and verify with node smoke-check.js.",
        "When the plan creates a verifier, smoke, or test script, give that script a dedicated implement step whose expected_paths contain only that script; keep application and configuration files in a separate implement step.",
        "For multi-statement checks in Node or Python, create the script first and verify by running it, such as node smoke-check.js or python3 smoke_check.py <args>; do not put multiple statements inline in python3 -c.",
        "Do not put dev server startup or network setup in verify.",
        "For Next.js, create package.json and app entrypoints before npm run build.",
        "Do not use node --check for .ts or .tsx files.",
    ]
    .join("\n")
}

/// Renders the owner split that `admission::preserve` already accepts for the
/// frozen original scope: a role-limiting sentence followed by the complete
/// original instruction, with outputs, expected results and checks in order.
pub(crate) fn owner_split_template(original: Option<&StepPlan>) -> String {
    let mut out = format!(
        "Accepted owner split: when an original step is split, every new owner of its expected_paths keeps the original kind and expected_result, and its instruction is one sentence limiting that step to its own expected_paths followed by \"{ORIGINAL_CONTEXT} <the complete original instruction, unchanged>\". Carrying the complete original instruction as context does not make that owner write another owner's files: each owner writes only its own expected_paths. Keep every original expected_path, expected_result and check in the original step and check order; an original implement step's checks stay unchanged on one Verify step after all owners of its outputs. Paraphrased or partial instructions, and the original instruction moved to an unrelated step, are rejected."
    );
    for step in original
        .map(|plan| plan.steps.as_slice())
        .unwrap_or_default()
    {
        out.push('\n');
        out.push_str(&original_step_template(step));
    }
    out
}

fn original_step_template(step: &PlanStep) -> String {
    let checks = serde_json::to_string(&step.verify).unwrap_or_default();
    if step.expected_paths.is_empty()
        || !matches!(step.step_kind(), StepKind::Implement | StepKind::Setup)
    {
        return format!(
            "- Original step {:?} ({}, expected_result {}): keep it with the same instruction, expected_result and checks {checks}.",
            step.id, step.kind, step.expected_result
        );
    }
    let paths = serde_json::to_string(&step.expected_paths).unwrap_or_default();
    format!(
        "- Original step {:?} ({}, expected_result {}) owns {paths} with checks {checks}. Each owner instruction: \"Only create <this owner's expected_paths> in this step. {ORIGINAL_CONTEXT} {}\"",
        step.id, step.kind, step.expected_result, step.instruction
    )
}

#[cfg(test)]
mod tests;
