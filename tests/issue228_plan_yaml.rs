use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use commandagent::planner::plan::{
    PlanFileKind, render_editable_step_plan, saved_plan_guidance, validate_plan_file,
};
use commandagent::planner::step_plan::{PlanStep, StepPlan};

fn commandagent(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .args(args)
        .output()
        .unwrap()
}

fn valid_step_plan() -> StepPlan {
    StepPlan {
        goal: "Update the plan YAML guide".to_string(),
        steps: vec![PlanStep {
            id: "update-guide".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Update docs/guide/en/plan-yaml.md with editing guidance.".to_string(),
            expected_paths: vec!["docs/guide/en/plan-yaml.md".to_string()],
            verify: Vec::new(),
        }],
    }
}

#[test]
fn validate_plan_cli_accepts_commented_step_yaml_and_prints_next_command() {
    let workspace = tempfile::tempdir().unwrap();
    let path = workspace.path().join("step-plan.yaml");
    fs::write(&path, render_editable_step_plan(&valid_step_plan())).unwrap();

    let output = commandagent(&[
        "--cwd",
        workspace.path().to_str().unwrap(),
        "--validate-plan",
        "step-plan.yaml",
    ]);
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(output.status.success(), "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
    assert!(stdout.contains("Valid step plan:"), "{stdout}");
    assert!(stdout.contains("Next: commandagent --run-plan"), "{stdout}");
    assert!(stdout.contains(path.to_str().unwrap()), "{stdout}");
}

#[test]
fn validate_plan_cli_reports_the_source_line_for_schema_and_lint_errors() {
    let workspace = tempfile::tempdir().unwrap();
    let schema_path = workspace.path().join("schema-error.yaml");
    fs::write(
        &schema_path,
        "goal: docs\nsteps:\n  - id: update-guide\n    kind: implement\n    expected_result: pass\n    instruction: [not, a, string]\n",
    )
    .unwrap();

    let schema = commandagent(&[
        "--cwd",
        workspace.path().to_str().unwrap(),
        "--validate-plan",
        "schema-error.yaml",
    ]);
    let schema_stderr = String::from_utf8(schema.stderr).unwrap();
    assert!(!schema.status.success(), "{schema_stderr}");
    assert!(
        schema_stderr.contains(&format!("{}:6:", schema_path.display())),
        "{schema_stderr}"
    );

    let lint_path = workspace.path().join("lint-error.yaml");
    fs::write(
        &lint_path,
        "goal: verify docs\nsteps:\n  - id: verify-docs\n    kind: verify\n    expected_result: pass\n    instruction: Run the documentation checks.\n    verify:\n      - \"cargo test | cargo clippy\"\n",
    )
    .unwrap();
    let lint = commandagent(&[
        "--cwd",
        workspace.path().to_str().unwrap(),
        "--validate-plan",
        "lint-error.yaml",
    ]);
    let lint_stderr = String::from_utf8(lint.stderr).unwrap();
    assert!(!lint.status.success(), "{lint_stderr}");
    assert!(
        lint_stderr.contains(&format!("{}:8:10:", lint_path.display())),
        "{lint_stderr}"
    );
    assert!(lint_stderr.contains("verify_policy"), "{lint_stderr}");
}

#[test]
fn validate_plan_cli_summarizes_recovery_diff_and_ultra_next_command() {
    let workspace = tempfile::tempdir().unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/corpus/apps/issue228-plan-yaml/fixtures/recovery-ultra-plan.yaml");
    let path = workspace.path().join("recovery.yaml");
    fs::copy(source, &path).unwrap();

    let output = commandagent(&["--validate-plan", path.to_str().unwrap()]);
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(output.status.success(), "{stderr}");
    assert!(stdout.contains("Valid recovery UltraPlan"), "{stdout}");
    assert!(stdout.contains("failed scope verify-release"), "{stdout}");
    assert!(stdout.contains("failure build_failed"), "{stdout}");
    assert!(
        stdout.contains("retained artifacts src/lib.rs, tests/cli.rs"),
        "{stdout}"
    );
    assert!(stdout.contains("--run-ultra-plan"), "{stdout}");
}

#[test]
fn saved_template_and_guidance_form_an_edit_validate_run_workflow() {
    let workspace = tempfile::tempdir().unwrap();
    let path = commandagent::planner::save_step_plan(workspace.path(), &valid_step_plan()).unwrap();
    let body = fs::read_to_string(&path).unwrap();
    assert!(
        body.starts_with("# CommandAgent editable plan YAML"),
        "{body}"
    );
    assert!(body.contains("# kind: inspect, setup, implement, verify, or report."));
    validate_plan_file(&path, workspace.path()).unwrap();

    let guidance = saved_plan_guidance(&path, PlanFileKind::Step);
    assert!(guidance.contains("--validate-plan"), "{guidance}");
    assert!(guidance.contains("--run-plan"), "{guidance}");
}

#[test]
fn validate_plan_help_is_public_and_action_conflicts_are_enforced() {
    let help = commandagent(&["--help"]);
    let stdout = String::from_utf8(help.stdout).unwrap();
    assert!(help.status.success());
    assert!(stdout.contains("--validate-plan <PATH>"), "{stdout}");

    let conflict = commandagent(&["--validate-plan", "plan.yaml", "--run-plan", "plan.yaml"]);
    assert_eq!(conflict.status.code(), Some(2));
    assert!(
        String::from_utf8(conflict.stderr)
            .unwrap()
            .contains("cannot be used with")
    );
}
