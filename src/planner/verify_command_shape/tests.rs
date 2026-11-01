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

fn assert_negatives_still_rejected(root: &Path) {
    for command in [
        "スクリプトを指定仕様に沿って実装できたことを確認する。",
        "確認する",
        "[ -f ]",
        "[ x -f ]",
        "[ -f x",
    ] {
        let found = offenses(&verify_plan(&["x"], command), root, None);
        assert_eq!(found.len(), 1, "{command}: {found:?}");
    }
}

#[test]
fn shell_comment_lines_before_a_command_are_not_rejected() {
    let dir = tempfile::tempdir().unwrap();
    for comment in ["# 出力を確認する", "# check output"] {
        let command = format!("{comment}\npython3 csv2jsonl.py products.csv out.jsonl");
        let plan = verify_plan(&["csv2jsonl.py"], &command);
        let found = offenses(&plan, dir.path(), None);
        assert!(found.is_empty(), "{command}: {found:?}");
        let report = lint_plan_for_execution(&plan, Some(dir.path()));
        assert!(!report.has_category(CATEGORY), "{command}: {report:?}");
    }
    assert_negatives_still_rejected(dir.path());
}

#[test]
fn unquoted_subshell_is_not_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let plan = verify_plan(&["package.json"], "( npm test )");
    let found = offenses(&plan, dir.path(), None);
    assert!(found.is_empty(), "{found:?}");
    let report = lint_plan_for_execution(&plan, Some(dir.path()));
    assert!(!report.has_category(CATEGORY), "{report:?}");
    assert_negatives_still_rejected(dir.path());
}

// Issue #952 positive cases. They were written and run on the base before the
// prose check changed; none of them may become a lint error.

fn ordered_plan(steps: Vec<PlanStep>) -> StepPlan {
    StepPlan {
        goal: "Create the artifact.".into(),
        steps,
    }
}

#[cfg(unix)]
fn marker_executable(dir: &Path, name: &str, marker: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join(name);
    std::fs::write(
        &path,
        format!("#!/bin/sh\necho ran > '{}'\n", marker.display()),
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn issue952_builtins_are_accepted_without_a_path() {
    let dir = tempfile::tempdir().unwrap();
    for command in [
        "test -s out.txt",
        "[ -f out.txt ]",
        "printf done",
        "command -v sh",
        ": placeholder",
        "cd sub",
    ] {
        let found = offenses(&verify_plan(&["out.txt"], command), dir.path(), None);
        assert!(found.is_empty(), "{command}: {found:?}");
    }
}

#[cfg(unix)]
#[test]
fn issue952_pinned_path_executables_are_accepted() {
    let dir = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    fake_executable(bin.path(), "checker");
    fake_executable(bin.path(), "Postpone");
    let path_var = Some(bin.path().as_os_str());
    for command in [
        "checker --strict out.txt",
        "checker",
        // The same first word as a synthetic sentence, but it names a command
        // on the pinned PATH, so it is left to the runtime gate.
        "Postpone the format review until the final checker runs.",
        "Postpone",
    ] {
        let found = offenses(&verify_plan(&["out.txt"], command), dir.path(), path_var);
        assert!(found.is_empty(), "{command}: {found:?}");
    }
}

#[test]
fn issue952_relative_absolute_and_non_ascii_paths_are_accepted() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("run_checks.sh"), "#!/bin/sh\n").unwrap();
    std::fs::create_dir_all(dir.path().join("検査")).unwrap();
    std::fs::write(dir.path().join("検査/確かめる"), "#!/bin/sh\n").unwrap();
    for command in [
        "./run_checks.sh",
        "./run_checks.sh --all",
        "/opt/tools/verify-output --strict out.txt",
        "./検査/確かめる",
        "検査/確かめる --all",
        "tools/not-yet-built --check",
    ] {
        let found = offenses(&verify_plan(&["out.txt"], command), dir.path(), None);
        assert!(found.is_empty(), "{command}: {found:?}");
    }
}

#[test]
fn issue952_same_step_and_earlier_producers_are_accepted() {
    let dir = tempfile::tempdir().unwrap();
    let same = ordered_plan(vec![step(
        "make",
        "implement",
        &["Postpone", "tools/check_output.sh"],
        &["Postpone the format review.", "sh tools/check_output.sh"],
    )]);
    assert!(offenses(&same, dir.path(), None).is_empty());
    let earlier = ordered_plan(vec![
        step("make", "implement", &["Postpone", "確認"], &[]),
        step(
            "check",
            "verify",
            &[],
            &["Postpone the format review.", "確認"],
        ),
    ]);
    assert!(offenses(&earlier, dir.path(), None).is_empty());
}

#[test]
fn issue952_earlier_setup_tool_is_accepted_and_later_setup_is_an_order_error() {
    let dir = tempfile::tempdir().unwrap();
    let command = "npx --no-install eslint .";
    let setup = || step("setup", "setup", &["package.json"], &["npm install"]);
    let check = || step("check", "verify", &[], &[command]);
    let before = ordered_plan(vec![setup(), check()]);
    assert!(offenses(&before, dir.path(), None).is_empty());
    let report = lint_plan_for_execution(&before, Some(dir.path()));
    assert!(!report.has_category(CATEGORY), "{report:?}");
    assert!(!report.has_category("dependency_order"), "{report:?}");
    let after = ordered_plan(vec![check(), setup()]);
    let report = lint_plan_for_execution(&after, Some(dir.path()));
    assert!(report.has_category("dependency_order"), "{report:?}");
}

#[test]
fn issue952_later_producer_is_an_order_error_and_earlier_is_not() {
    let dir = tempfile::tempdir().unwrap();
    let make = || step("make", "implement", &["tools/check_output.sh"], &[]);
    let check = || step("check", "verify", &[], &["sh tools/check_output.sh"]);
    let report = lint_plan_for_execution(&ordered_plan(vec![make(), check()]), Some(dir.path()));
    assert!(report.is_pass(), "{report:?}");
    let report = lint_plan_for_execution(&ordered_plan(vec![check(), make()]), Some(dir.path()));
    assert!(report.has_category("verify_semantics"), "{report:?}");
    assert!(
        report
            .errors
            .iter()
            .any(|e| e.message.contains("only produced by a later step")),
        "{report:?}"
    );
}

#[test]
fn issue952_catalog_check_id_is_accepted() {
    let dir = tempfile::tempdir().unwrap();
    let id = crate::planner::profiles::data::manifest::check_ids()
        .into_iter()
        .next()
        .expect("the data catalog has a bound check id");
    let command = crate::planner::profiles::data::step_policy::catalog_check_command(&id);
    assert!(
        crate::planner::profiles::data::step_policy::catalog_check_id(&command).is_some(),
        "{command}"
    );
    assert!(offenses(&verify_plan(&["out.txt"], &command), dir.path(), None).is_empty());
}

#[test]
fn issue952_commands_absent_from_path_and_workspace_are_left_to_runtime() {
    let dir = tempfile::tempdir().unwrap();
    let empty = tempfile::tempdir().unwrap();
    for path_var in [None, Some(empty.path().as_os_str())] {
        for command in [
            "lint-output --strict report.json",
            "postpone report.txt",
            "mytool run fast",
            "Rscript analysis.R",
            "eslint .",
            "pytest -q",
            "python3 check.py",
            "README",
        ] {
            let found = offenses(&verify_plan(&["out.txt"], command), dir.path(), path_var);
            assert!(found.is_empty(), "{command}: {found:?}");
        }
    }
}

#[cfg(unix)]
#[test]
fn issue952_lint_never_runs_the_command() {
    let dir = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let markers = tempfile::tempdir().unwrap();
    let path_marker = markers.path().join("path-ran");
    let script_marker = markers.path().join("script-ran");
    marker_executable(bin.path(), "Postpone", &path_marker);
    marker_executable(dir.path(), "check.sh", &script_marker);
    let path_var = Some(bin.path().as_os_str());
    for command in [
        "Postpone the format review until the final checker runs.",
        "./check.sh",
        "sh check.sh",
    ] {
        let plan = verify_plan(&["out.txt"], command);
        assert!(
            offenses(&plan, dir.path(), path_var).is_empty(),
            "{command}"
        );
        let _ = lint_plan_for_execution(&plan, Some(dir.path()));
    }
    assert!(!path_marker.exists());
    assert!(!script_marker.exists());
}

// Issue #952 negative cases: synthetic sentences and deferrals in English,
// Japanese, and mixed scripts are rejected with an empty pinned PATH and no
// workspace file, whatever their first word.
const ISSUE952_PROSE: &[&str] = &[
    "Postpone the format review until the final checker runs.",
    "Later steps will confirm the report format",
    "Review manually, then record the outcome",
    "Confirmed by the host review!",
    "出力の形式は最終工程でまとめて確かめる",
    "後続の検査で確認予定。",
    "Report の形式は後で確かめる。",
    "JSONの整形は最後に確かめる",
];

#[test]
fn issue952_sentences_in_any_script_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let empty = tempfile::tempdir().unwrap();
    for path_var in [None, Some(empty.path().as_os_str())] {
        for command in ISSUE952_PROSE {
            let found = offenses(&verify_plan(&["out.txt"], command), dir.path(), path_var);
            assert_eq!(found.len(), 1, "{command}: {found:?}");
            assert!(found[0].contains("step check"), "{found:?}");
            assert!(found[0].contains(command), "{found:?}");
            assert!(found[0].contains("python3 <script> <args>"), "{found:?}");
        }
    }
}

#[test]
fn issue952_ascii_first_word_sentences_are_rejected() {
    // The first word of each has an ASCII letter, which used to exempt it.
    let dir = tempfile::tempdir().unwrap();
    for command in ISSUE952_PROSE
        .iter()
        .filter(|c| c.chars().next().is_some_and(|ch| ch.is_ascii()))
    {
        let report = lint_plan_for_execution(&verify_plan(&["out.txt"], command), Some(dir.path()));
        assert!(report.has_category(CATEGORY), "{command}: {report:?}");
    }
}

#[test]
fn issue952_sentence_shapes_are_judged_by_shape_not_by_words() {
    let dir = tempfile::tempdir().unwrap();
    // The same words with a flag, a path, a digit, or no sentence mark are
    // not sentences; they may name a tool and are left to the runtime gate.
    for command in [
        "postpone the format review",
        "Postpone --until final",
        "Postpone the review in out/report.txt.",
        "Later steps 2 confirm the format",
        "Later steps",
        "Review",
        "review manually",
        "Run it",
        "mytool 確認",
    ] {
        let found = offenses(&verify_plan(&["out.txt"], command), dir.path(), None);
        assert!(found.is_empty(), "{command}: {found:?}");
    }
    for command in [
        "postpone the format review.",
        "review manually, then record it",
        "Postpone the format review now",
        "Postpone the format review now!",
        "確認するReport",
    ] {
        let found = offenses(&verify_plan(&["out.txt"], command), dir.path(), None);
        assert_eq!(found.len(), 1, "{command}: {found:?}");
    }
}

#[cfg(unix)]
#[test]
fn issue952_resolved_first_word_wins_over_sentence_shape() {
    let dir = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    for command in ISSUE952_PROSE {
        let first = command.split_whitespace().next().unwrap();
        fake_executable(bin.path(), first);
        let found = offenses(
            &verify_plan(&["out.txt"], command),
            dir.path(),
            Some(bin.path().as_os_str()),
        );
        assert!(found.is_empty(), "{command}: {found:?}");
        let declared = offenses(&verify_plan(&["out.txt", first], command), dir.path(), None);
        assert!(declared.is_empty(), "{command}: {declared:?}");
    }
}

#[test]
fn issue952_host_declared_sentences_stay_with_their_own_gate() {
    let dir = tempfile::tempdir().unwrap();
    for command in ISSUE952_PROSE {
        let found = diagnose_with_path(
            &verify_plan(&["out.txt"], command),
            Some(dir.path()),
            &[command.to_string()],
            None,
        );
        assert!(found.is_empty(), "{command}: {found:?}");
    }
}
