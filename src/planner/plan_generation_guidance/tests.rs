use super::{ORIGINAL_CONTEXT, owner_split_template, system_prompt};
use crate::planner::step_plan::{PlanStep, StepPlan};

fn step(id: &str, kind: &str, paths: &[&str], instruction: &str, verify: &[&str]) -> PlanStep {
    PlanStep {
        id: id.into(),
        kind: kind.into(),
        expected_result: "pass".into(),
        instruction: instruction.into(),
        expected_paths: paths.iter().map(|p| (*p).into()).collect(),
        verify: verify.iter().map(|c| (*c).into()).collect(),
    }
}

#[test]
fn system_prompt_separates_verifier_scripts_for_node_and_python() {
    let prompt = system_prompt();
    assert!(prompt.contains("give that script a dedicated implement step whose expected_paths contain only that script; keep application and configuration files in a separate implement step"));
    assert!(prompt.contains("node smoke-check.js"));
    assert!(prompt.contains("python3 smoke_check.py sample.txt"));
    assert!(prompt.contains("do not put multiple statements inline in python3 -c"));
    // The moved lines are unchanged.
    assert!(prompt.contains("Report is not success"));
    assert!(prompt.contains("For multi-statement Node checks, create a smoke-check.js artifact"));
}

#[test]
fn template_without_frozen_scope_states_the_accepted_form_only() {
    let text = owner_split_template(None);
    assert!(text.contains(&format!(
        "\"{ORIGINAL_CONTEXT} <the complete original instruction, unchanged>\""
    )));
    assert!(text.contains("does not make that owner write another owner's files"));
    assert!(!text.contains("do not duplicate the combined instruction"));
    assert!(!text.contains("\n- Original step"));
}

#[test]
fn template_renders_each_original_step_with_full_context_and_checks() {
    let mixed = "Create index.html and smoke-check.js that checks it.";
    let original = StepPlan {
        goal: "memo".into(),
        steps: vec![
            step("inspect", "inspect", &[], "Read the workspace.", &[]),
            step(
                "app-and-check",
                "implement",
                &["index.html", "smoke-check.js"],
                mixed,
                &["node smoke-check.js", "test -f index.html"],
            ),
            step(
                "verify",
                "verify",
                &[],
                "Run checks.",
                &["node smoke-check.js"],
            ),
        ],
    };
    let text = owner_split_template(Some(&original));
    assert!(text.contains(&format!(
        "- Original step \"app-and-check\" (implement, expected_result pass) owns [\"index.html\",\"smoke-check.js\"] with checks [\"node smoke-check.js\",\"test -f index.html\"]. Each owner instruction: \"Only create <this owner's expected_paths> in this step. {ORIGINAL_CONTEXT} {mixed}\""
    )));
    assert!(text.contains("- Original step \"inspect\" (inspect, expected_result pass): keep it with the same instruction, expected_result and checks []."));
    assert!(text.contains("- Original step \"verify\" (verify, expected_result pass): keep it with the same instruction, expected_result and checks [\"node smoke-check.js\"]."));
    let order = ["\"inspect\"", "\"app-and-check\"", "\"verify\""].map(|id| text.find(id).unwrap());
    assert!(order[0] < order[1] && order[1] < order[2]);
}
