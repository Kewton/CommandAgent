use super::runs_plan_script;
use crate::planner::lint::{PlanQualityContext, step_plan_quality_report};
use crate::planner::step_plan::{PlanStep, StepPlan};

const WEAK: &str = "weak_code_verify";

fn step(id: &str, kind: &str, paths: &[&str], verify: &[&str]) -> PlanStep {
    PlanStep {
        id: id.into(),
        kind: kind.into(),
        expected_result: "pass".into(),
        instruction: format!("Create {}.", paths.join(", ")),
        expected_paths: paths.iter().map(|p| (*p).into()).collect(),
        verify: verify.iter().map(|v| (*v).into()).collect(),
    }
}

fn plan(command: &str) -> StepPlan {
    StepPlan {
        goal: "Create the Python word counter topwords.py.".into(),
        steps: vec![
            step("implement-topwords", "implement", &["topwords.py"], &[]),
            step(
                "implement-smoke-check",
                "implement",
                &["smoke_check.py", "tests/check_cli.py"],
                &[],
            ),
            step("verify-topwords", "verify", &[], &[command]),
        ],
    }
}

fn weak(command: &str) -> bool {
    step_plan_quality_report(&plan(command), &PlanQualityContext::default())
        .issues
        .iter()
        .any(|issue| issue.category == WEAK)
}

#[test]
fn plan_created_scripts_are_strong_in_the_quality_report() {
    for command in [
        "python3 smoke_check.py sample.txt",
        "python3 -B tests/check_cli.py",
        "python smoke_check.py",
        "python3 ./smoke_check.py < sample.txt",
        "python3 -W ignore smoke_check.py",
        "python3 -Werror -X utf8 smoke_check.py",
        "python3 topwords.py 3",
    ] {
        assert!(runs_plan_script(&plan(command), command), "{command}");
        assert!(!weak(command), "{command}");
    }
}

#[test]
fn other_python_commands_stay_weak() {
    for command in [
        "python3 -c \"import topwords\"",
        "python3 -Bc 'import smoke_check'",
        "python3 other.py",
        "python3 ../smoke_check.py",
        "python3 /abs/smoke_check.py",
        "python3",
        "python3 -m http.server",
        "python3 -- smoke_check.py",
        "python3 --check-hash-based-pycs never smoke_check.py",
        "python3 -V smoke_check.py",
        "python3 -i smoke_check.py",
        "python3 - smoke_check.py",
        "python3 -W",
        "python3 smoke_check.py | tee out.txt",
        "python3 smoke_check.py || true",
        "python3 smoke_check.py; echo done",
        "python3 smoke_check.py && echo done",
        "python3 $(echo smoke_check.py)",
        "python3 \"smoke_check.py\"",
        "python3.12 smoke_check.py",
        "env python3 smoke_check.py",
        "cd tests && python3 check_cli.py",
        "python3 Smoke_check.py",
        "python3 smoke_check",
    ] {
        assert!(!runs_plan_script(&plan(command), command), "{command}");
        assert!(weak(command), "{command}");
    }
}

fn plan_with_owned(owned: &str, command: &str) -> StepPlan {
    StepPlan {
        goal: "Create the Python word counter topwords.py.".into(),
        steps: vec![
            step("implement-topwords", "implement", &["topwords.py"], &[]),
            step("implement-check", "implement", &[owned], &[]),
            step("verify-topwords", "verify", &[], &[command]),
        ],
    }
}

#[test]
fn plan_created_files_that_are_not_py_scripts_stay_weak() {
    for owned in ["check.sh", "data.txt", "checker"] {
        let command = format!("python3 {owned}");
        let plan = plan_with_owned(owned, &command);
        assert!(!runs_plan_script(&plan, &command), "{command}");
        let report = step_plan_quality_report(&plan, &PlanQualityContext::default());
        let issue = report
            .issues
            .iter()
            .find(|issue| issue.category == WEAK)
            .unwrap_or_else(|| panic!("{command}: {report:?}"));
        assert_eq!(
            issue.message, "code task lacks test, build, smoke, or compile verification",
            "{command}"
        );
    }
}

#[test]
fn plan_paths_outside_the_workspace_stay_weak() {
    for owned in ["../smoke_check.py", "/abs/smoke_check.py"] {
        let command = format!("python3 {owned}");
        let plan = plan_with_owned(owned, &command);
        assert!(!runs_plan_script(&plan, &command), "{command}");
    }
}

#[test]
fn only_implement_and_setup_paths_count() {
    let command = "python3 smoke_check.py";
    for (kind, strong) in [
        ("implement", true),
        ("create", true),
        ("setup", true),
        ("verify", false),
        ("inspect", false),
        ("report", false),
    ] {
        let mut plan = plan(command);
        plan.steps[1].kind = kind.into();
        assert_eq!(runs_plan_script(&plan, command), strong, "{kind}");
    }
}

#[test]
fn existing_strong_commands_are_unchanged() {
    for command in [
        "node smoke-check.js",
        "python3 -m unittest",
        "pytest",
        "python3 -m py_compile topwords.py",
        "python3 -m compileall .",
    ] {
        assert!(!runs_plan_script(&plan(command), command), "{command}");
        assert!(!weak(command), "{command}");
    }
}
