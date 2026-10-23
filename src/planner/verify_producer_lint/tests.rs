use std::path::Path;

use crate::planner::lint::lint_plan_for_execution;
use crate::planner::step_plan::{PlanStep, StepPlan, parse_step_plan};
use crate::planner::verify_producer_lint::diagnose_script_without_producer;

const FIXTURE: &str = "tests/corpus/apps/issue836-verify-script-without-producer";

fn fixture_plan(name: &str) -> StepPlan {
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(FIXTURE)
            .join(name),
    )
    .unwrap();
    parse_step_plan(&text).unwrap()
}

fn step(id: &str, kind: &str, paths: &[&str], verify: &[&str]) -> PlanStep {
    PlanStep {
        id: id.into(),
        kind: kind.into(),
        expected_result: "pass".into(),
        instruction: format!("Carry out {id}."),
        expected_paths: paths.iter().map(|p| (*p).into()).collect(),
        verify: verify.iter().map(|v| (*v).into()).collect(),
    }
}

fn plan(goal: &str, steps: Vec<PlanStep>) -> StepPlan {
    StepPlan {
        goal: goal.into(),
        steps,
    }
}

fn offenses(plan: &StepPlan, root: &Path) -> Vec<String> {
    diagnose_script_without_producer(plan, Some(root), &[])
}

#[test]
fn issue836_content_q_plan_is_a_lint_error_in_the_planner_lint() {
    let dir = tempfile::tempdir().unwrap();
    let report = lint_plan_for_execution(&fixture_plan("content-q-plan.yaml"), Some(dir.path()));
    assert!(report.has_category("verify_semantics"), "{report:?}");
    assert!(
        report
            .errors
            .iter()
            .any(|err| err.category == "verify_semantics" && err.message.contains("smoke-check.js")),
        "{report:?}"
    );
}

#[test]
fn issue836_ambig_plan_with_an_earlier_producer_passes() {
    let dir = tempfile::tempdir().unwrap();
    let found = offenses(&fixture_plan("ambig-plan.yaml"), dir.path());
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn issue836_script_already_in_the_workspace_passes() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("smoke-check.js"), "process.exit(0)\n").unwrap();
    let found = offenses(&fixture_plan("content-q-plan.yaml"), dir.path());
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn issue836_step_that_creates_the_script_may_verify_it() {
    let dir = tempfile::tempdir().unwrap();
    let plan = plan(
        "goal",
        vec![step(
            "make-check",
            "implement",
            &["./check.py"],
            &["python3 check.py"],
        )],
    );
    assert!(offenses(&plan, dir.path()).is_empty());
}

#[test]
fn issue836_producer_after_the_verify_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let plan = plan(
        "goal",
        vec![
            step("verify-first", "verify", &[], &["node smoke-check.js"]),
            step("make-check", "implement", &["smoke-check.js"], &[]),
        ],
    );
    let found = offenses(&plan, dir.path());
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("later step"), "{found:?}");
}

#[test]
fn issue836_a_verify_step_expected_path_is_not_a_producer() {
    let dir = tempfile::tempdir().unwrap();
    let plan = plan(
        "goal",
        vec![step("verify-it", "verify", &["run.sh"], &["bash run.sh"])],
    );
    assert_eq!(offenses(&plan, dir.path()).len(), 1);
}

#[test]
fn issue836_rule_is_independent_of_the_goal_profile_and_language() {
    let dir = tempfile::tempdir().unwrap();
    for (goal, command) in [
        (
            "Build a Python CLI that converts csv files",
            "python3 smoke.py",
        ),
        ("Create a small generic memo page", "node smoke.mjs"),
        (
            "Build a Next.js notes app with route handlers",
            "bash smoke.sh",
        ),
    ] {
        let plan = plan(goal, vec![step("verify-it", "verify", &[], &[command])]);
        let report = lint_plan_for_execution(&plan, Some(dir.path()));
        assert!(
            report.has_category("verify_semantics"),
            "{goal}: {report:?}"
        );
    }
}

#[test]
fn issue836_non_script_commands_and_unknown_workspace_are_not_flagged() {
    let dir = tempfile::tempdir().unwrap();
    let plan = plan(
        "goal",
        vec![step(
            "verify-it",
            "verify",
            &[],
            &["npm run build", "ls out.csv", "node -v"],
        )],
    );
    assert!(offenses(&plan, dir.path()).is_empty());
    let missing = fixture_plan("content-q-plan.yaml");
    assert!(diagnose_script_without_producer(&missing, None, &[]).is_empty());
}

fn contract_config(root: &Path) -> crate::config::Config {
    use clap::Parser;
    let cwd = root.to_string_lossy().to_string();
    let mut config = crate::config::Config::from_cli(crate::cli::Cli::parse_from([
        "commandagent",
        "--cwd",
        &cwd,
        "--offline",
        "goal",
    ]))
    .unwrap();
    let path = root.join("completion-contract.json");
    let contract = serde_json::json!({
        "required_paths": [],
        "verify_commands": ["python3 scripts/contract_check.py"],
        "fix_reproducer_command": "python3 scripts/repro.py data/task-02.csv",
        "deterministic_oracles": ["python3 scripts/oracle.py"],
    });
    std::fs::write(&path, serde_json::to_vec(&contract).unwrap()).unwrap();
    config.completion_contract_path = Some(path);
    config
}

fn host_fix_plan() -> StepPlan {
    plan(
        "Fix the registered failure.",
        vec![
            step(
                "reproduce-before",
                "verify",
                &[],
                &["python3 scripts/repro.py data/task-02.csv"],
            ),
            step(
                "verify-after",
                "verify",
                &[],
                &["python3 scripts/repro.py data/task-02.csv"],
            ),
            step(
                "verify-regressions",
                "verify",
                &[],
                &[
                    "python3 scripts/contract_check.py",
                    "python3 scripts/oracle.py",
                ],
            ),
        ],
    )
}

#[test]
fn issue836_host_plan_may_run_scripts_the_completion_contract_names() {
    let dir = tempfile::tempdir().unwrap();
    let config = contract_config(dir.path());
    let mut host = host_fix_plan();
    let report =
        crate::planner::step_plan_finalize::finalize_step_plan_for_execution(&mut host, &config);
    assert!(report.is_pass(), "{report:?}");
}

#[test]
fn issue836_host_plan_still_rejects_scripts_the_contract_does_not_name() {
    let dir = tempfile::tempdir().unwrap();
    let config = contract_config(dir.path());
    let mut host = plan(
        "Fix the registered failure.",
        vec![step(
            "verify-after",
            "verify",
            &[],
            &["node smoke-check.js"],
        )],
    );
    let report =
        crate::planner::step_plan_finalize::finalize_step_plan_for_execution(&mut host, &config);
    assert!(report.has_category("verify_semantics"), "{report:?}");
}

#[test]
fn issue836_planner_plan_with_a_missing_contract_script_is_a_lint_error() {
    let dir = tempfile::tempdir().unwrap();
    let report = lint_plan_for_execution(&host_fix_plan(), Some(dir.path()));
    assert!(report.has_category("verify_semantics"), "{report:?}");
    assert_eq!(report.errors.len(), 4, "{report:?}");
}

/// A temporary workspace that already holds the `check.js` and `check2.js` scripts.
pub(crate) fn tempdir_with_check_scripts() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for name in ["check.js", "check2.js"] {
        std::fs::write(dir.path().join(name), "").unwrap();
    }
    dir
}
