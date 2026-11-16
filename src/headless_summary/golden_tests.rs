//! Byte goldens for the `cm0-headless-summary` fixtures (Issue #1015, acceptance 4).
//!
//! The goldens were captured before #1015 changed any code. They hold the whole
//! `--summary-json` stdout line and the whole `summary.md`, so a run without a
//! run-budget or structured-stage declaration must keep producing these bytes.
//! They are never regenerated: a mismatch means the change is wrong.

use std::path::{Path, PathBuf};

use super::{Source, render};
use crate::eval_events;

const CASES: [&str; 6] = [
    "full",
    "failed",
    "static",
    "billed",
    "billed-incomplete",
    "billed-none",
];

/// Stands in for the per-test temporary directory in the goldens.
const ROOT: &str = "<ROOT>";

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/apps/cm0-headless-summary")
}

fn golden(name: &str) -> String {
    let path = corpus().join("golden").join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
}

fn copy_fixture(case: &str, root: &Path, with_summary: bool) -> PathBuf {
    let source = corpus().join("fixtures").join(case);
    let run = root.join(case);
    std::fs::create_dir_all(&run).unwrap();
    std::fs::copy(source.join("events.jsonl"), run.join("events.jsonl")).unwrap();
    if with_summary {
        std::fs::copy(source.join("summary.md"), run.join("summary.md")).unwrap();
    }
    run.join("events.jsonl")
}

fn placeholder(text: &str, root: &Path) -> String {
    text.replace(&root.display().to_string(), ROOT)
}

/// The `--summary-json` stdout of one fixture: `println!` of `render`.
pub(super) fn summary_json_stdout(case: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let events = copy_fixture(case, dir.path(), true);
    placeholder(
        &format!("{}\n", render(&Source::from_events_path(events))),
        dir.path(),
    )
}

/// The `summary.md` the direct-command stop writes for one fixture.
pub(super) fn summary_md(case: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let events = copy_fixture(case, dir.path(), false);
    let failed = case == "failed";
    let snapshot = eval_events::latest_completion_snapshot(Some(&events));
    let projection = eval_events::project_completion(!failed, &snapshot);
    let (status, stop_reason, failure_kind) = if failed {
        ("failed", "verification failed", "direct_cli_command_failed")
    } else {
        ("completed", "completed", "")
    };
    eval_events::write_tui_command_completion_summary(
        Some(&events),
        "--prompt",
        stop_reason,
        failure_kind,
        status,
        &projection,
    );
    let written = std::fs::read_to_string(events.parent().unwrap().join("summary.md")).unwrap();
    placeholder(&host_independent(&written), dir.path())
}

/// Drops the two lines that depend on the build and the host, not on the run:
/// the `Build:` stamp is replaced and the `Host env:` note is removed.
fn host_independent(text: &str) -> String {
    text.split_inclusive('\n')
        .filter(|line| !line.starts_with("Host env: "))
        .map(|line| {
            if line.starts_with("Build: ") {
                "Build: <BUILD>\n"
            } else {
                line
            }
        })
        .collect()
}

#[test]
fn summary_json_stdout_bytes_match_the_pre_1015_golden() {
    for case in CASES {
        assert_eq!(
            summary_json_stdout(case),
            golden(&format!("{case}.summary.json")),
            "{case}"
        );
    }
}

#[test]
fn summary_md_bytes_match_the_pre_1015_golden() {
    for case in CASES {
        assert_eq!(
            summary_md(case),
            golden(&format!("{case}.summary.md")),
            "{case}"
        );
    }
}
