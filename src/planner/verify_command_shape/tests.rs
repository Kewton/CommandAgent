use std::ffi::OsStr;
use std::path::Path;

use crate::planner::lint::lint_plan_for_execution;
use crate::planner::step_plan::{PlanStep, StepPlan};
use crate::planner::verify_command_shape::diagnose_with_path;

const CATEGORY: &str = "verify_command_shape";

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

fn verify_plan(producer_paths: &[&str], command: &str) -> StepPlan {
    StepPlan {
        goal: "Create the artifact.".into(),
        steps: vec![
            step("make", "implement", producer_paths, &[]),
            step("check", "verify", &[], &[command]),
        ],
    }
}

// PATH is pinned to a tempdir (or to nothing), never the terminal's PATH.
fn offenses(plan: &StepPlan, root: &Path, path_var: Option<&OsStr>) -> Vec<String> {
    diagnose_with_path(plan, Some(root), &[], path_var)
}

#[cfg(unix)]
fn fake_executable(dir: &Path, name: &str) {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join(name);
    std::fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn prose_first_word_is_rejected_with_step_command_and_example() {
    let dir = tempfile::tempdir().unwrap();
    let command = "スクリプトを指定仕様に沿って実装できたことを確認する。";
    let found = offenses(&verify_plan(&["csv2jsonl.py"], command), dir.path(), None);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("step check"), "{found:?}");
    assert!(found[0].contains(command), "{found:?}");
    assert!(found[0].contains("python3 <script> <args>"), "{found:?}");
    assert!(found[0].contains("test -f <path>"), "{found:?}");
}

#[test]
fn prose_verify_is_a_lint_error_not_a_quality_issue() {
    let dir = tempfile::tempdir().unwrap();
    let plan = verify_plan(&["notes.md"], "確認する");
    let report = lint_plan_for_execution(&plan, Some(dir.path()));
    assert!(report.has_category(CATEGORY), "{report:?}");
}

#[cfg(unix)]
#[test]
fn non_ascii_command_on_the_pinned_path_is_accepted() {
    let dir = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    fake_executable(bin.path(), "確認");
    let plan = verify_plan(&["notes.md"], "確認");
    assert!(offenses(&plan, dir.path(), Some(bin.path().as_os_str())).is_empty());
    let empty = tempfile::tempdir().unwrap();
    assert_eq!(
        offenses(&plan, dir.path(), Some(empty.path().as_os_str())).len(),
        1
    );
}

#[cfg(unix)]
#[test]
fn non_executable_file_on_path_does_not_resolve() {
    let dir = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    std::fs::write(bin.path().join("確認"), "not executable").unwrap();
    let plan = verify_plan(&["notes.md"], "確認");
    assert_eq!(
        offenses(&plan, dir.path(), Some(bin.path().as_os_str())).len(),
        1
    );
}

#[test]
fn non_ascii_workspace_path_is_accepted_when_existing_or_declared() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("確認"), "#!/bin/sh\n").unwrap();
    assert!(offenses(&verify_plan(&["notes.md"], "確認"), dir.path(), None).is_empty());
    let fresh = tempfile::tempdir().unwrap();
    assert!(offenses(&verify_plan(&["検査"], "検査"), fresh.path(), None).is_empty());
}

#[test]
fn later_expected_path_does_not_count_as_declared() {
    let dir = tempfile::tempdir().unwrap();
    let plan = StepPlan {
        goal: "Create the artifact.".into(),
        steps: vec![
            step("check", "verify", &[], &["検査"]),
            step("make", "implement", &["検査"], &[]),
        ],
    };
    assert_eq!(offenses(&plan, dir.path(), None).len(), 1);
}

#[cfg(unix)]
#[test]
fn d3_commands_pass_with_a_pinned_path() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("run_tests.sh"), "#!/bin/sh\n").unwrap();
    let bin = tempfile::tempdir().unwrap();
    fake_executable(bin.path(), "pytest");
    let path_var = Some(bin.path().as_os_str());
    for (paths, command) in [
        (&["check.py"][..], "python3 check.py"),
        (&["smoke-check.js"][..], "node smoke-check.js"),
        (&["out.txt"][..], "./run_tests.sh"),
        (&["package.json"][..], "npm run build"),
        (&["docs/日本語.py"][..], "python3 docs/日本語.py"),
        (&["tests/test_app.py"][..], "pytest -q"),
        (&["out.json"][..], "[ -s out.json ]"),
        (&["index.html"][..], "[ -f index.html ]"),
        (&["index.html"][..], "test -f index.html"),
        (&["bin/check"][..], "bin/check --strict"),
    ] {
        let found = offenses(&verify_plan(paths, command), dir.path(), path_var);
        assert!(found.is_empty(), "{command}: {found:?}");
    }
}

#[test]
fn test_operand_shapes_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    for (command, reason) in [
        (
            "[ \"file_index.html\" -f ]",
            "comes before file operator `-f`",
        ),
        ("[ -f ]", "file operator `-f` has no operand"),
        ("[ x -f ]", "comes before file operator `-f`"),
        ("[ -f x", "has no closing `]`"),
        ("test -d", "file operator `-d` has no operand"),
        ("[ ! -e ]", "file operator `-e` has no operand"),
        ("[ -f a -a -s ]", "file operator `-s` has no operand"),
    ] {
        let found = offenses(&verify_plan(&["x"], command), dir.path(), None);
        assert_eq!(found.len(), 1, "{command}: {found:?}");
        assert!(found[0].contains(reason), "{command}: {found:?}");
    }
}

#[test]
fn valid_test_expressions_are_not_rejected() {
    let dir = tempfile::tempdir().unwrap();
    for command in [
        "[ -f a -a -d b ]",
        "[ ! -f stale.lock ]",
        "test -x bin/run -o -r README.md",
        "[ x = -f ]",
        "[ -f = x ]",
        "[ -n x ]",
        "[ -w out ]",
        "test",
        "[ ]",
    ] {
        let found = offenses(&verify_plan(&["x"], command), dir.path(), None);
        assert!(found.is_empty(), "{command}: {found:?}");
    }
}

#[test]
fn host_declared_commands_are_left_to_their_own_gate() {
    let dir = tempfile::tempdir().unwrap();
    let command = "確認する";
    let found = diagnose_with_path(
        &verify_plan(&["x"], command),
        Some(dir.path()),
        &[command.to_string()],
        None,
    );
    assert!(found.is_empty(), "{found:?}");
}
