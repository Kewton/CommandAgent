use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use serde_json::Value;

use super::{EventsScope, current_path};
use crate::tools::os_sandbox::{
    SandboxAvailability, SandboxMode, SandboxPolicy, SandboxTestState, install_for_test,
    is_applied, mark_backend_unavailable, status, wrap_command,
};

const CASES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/corpus/apps/issue697-sandbox-status-scope/fixtures/cases.json"
));

/// Every `announce` call: the destination it was given and the event.
static ANNOUNCED: Mutex<Vec<(Option<PathBuf>, Value)>> = Mutex::new(Vec::new());

pub(super) fn record(path: Option<&Path>, event: &Value) {
    ANNOUNCED
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push((path.map(Path::to_path_buf), event.clone()));
}

fn announced() -> Vec<(Option<PathBuf>, Value)> {
    ANNOUNCED
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

fn clear_announced() {
    ANNOUNCED
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
}

fn install(root: &Path, mode: SandboxMode) {
    let policy = SandboxPolicy::with_home(&root.join("work"), &root.join("tmp"), root.join("home"));
    install_for_test(mode, policy, SandboxAvailability::Available);
}

fn lines(path: &Path) -> Option<usize> {
    std::fs::read_to_string(path)
        .ok()
        .map(|text| text.lines().count())
}

fn scope_path(root: &Path, kind: &str, name: &str) -> Option<PathBuf> {
    match kind {
        "file" => Some(root.join(name)),
        "none" => None,
        other => panic!("unknown scope kind {other}"),
    }
}

fn expected_lines(value: &Value) -> Option<usize> {
    value.as_u64().map(|lines| lines as usize)
}

#[test]
fn routing_cases_follow_the_corpus_table() {
    let corpus: Value = serde_json::from_str(CASES).expect("cases.json parses");
    let cases = corpus["cases"].as_array().expect("cases array");
    assert!(!cases.is_empty());
    for case in cases {
        let id = case["id"].as_str().unwrap();
        let _state = SandboxTestState::acquire();
        clear_announced();
        let dir = tempfile::tempdir().unwrap();
        install(dir.path(), SandboxMode::Auto);

        let parent_path = scope_path(dir.path(), case["parent"].as_str().unwrap(), "parent.jsonl");
        let children = case["children"].as_array().unwrap();
        let child_paths: Vec<Option<PathBuf>> = children
            .iter()
            .enumerate()
            .map(|(index, kind)| {
                scope_path(
                    dir.path(),
                    kind.as_str().unwrap(),
                    &format!("child{index}.jsonl"),
                )
            })
            .collect();
        let downgrade = case["downgrade"].as_str().unwrap();

        let parent = EventsScope::enter(parent_path.clone());
        for (index, child_path) in child_paths.iter().enumerate() {
            let child = EventsScope::enter(child_path.clone());
            assert_eq!(
                current_path(),
                *child_path,
                "{id}: child {index} destination"
            );
            if downgrade == format!("child:{index}") {
                mark_backend_unavailable("fake backend failure".to_string());
            }
            drop(child);
            assert_eq!(
                current_path(),
                parent_path,
                "{id}: restored after child {index}"
            );
        }
        if downgrade == "parent" {
            mark_backend_unavailable("fake backend failure".to_string());
        }
        // A second failure of the same backend is not announced again.
        mark_backend_unavailable("a second failure".to_string());
        drop(parent);
        assert_eq!(current_path(), None, "{id}: top level restored");

        let expect = &case["expect"];
        let parent_lines = parent_path.as_deref().and_then(lines);
        let expected_parent = expected_lines(&expect["parent_lines"]);
        assert_eq!(
            parent_lines.or(parent_path.as_ref().map(|_| 0)),
            expected_parent,
            "{id}: parent file lines"
        );
        for (index, child_path) in child_paths.iter().enumerate() {
            let got = child_path.as_deref().and_then(lines);
            let want = expected_lines(&expect["children_lines"][index]);
            assert_eq!(
                got.or(child_path.as_ref().map(|_| 0)),
                want,
                "{id}: child {index} file lines"
            );
        }
        let calls = announced();
        assert_eq!(
            calls.len(),
            1,
            "{id}: one downgrade announcement: {calls:?}"
        );
        let path_less = calls.iter().filter(|(path, _)| path.is_none()).count();
        assert_eq!(
            path_less as u64,
            expect["path_less_emits"].as_u64().unwrap(),
            "{id}: path-less emit"
        );

        let event = &calls[0].1;
        assert_eq!(event["event"], corpus["contract"]["event"], "{id}");
        assert_eq!(event["applied"], false, "{id}");
        assert_eq!(event["mode"], "auto", "{id}");
        assert_eq!(event["reason"], "fake backend failure", "{id}");
        assert!(
            !is_applied(),
            "{id}: restoring must not re-apply the backend"
        );
    }
}

#[test]
fn a_recorded_event_carries_exactly_the_contract_fields() {
    let corpus: Value = serde_json::from_str(CASES).unwrap();
    let _state = SandboxTestState::acquire();
    let dir = tempfile::tempdir().unwrap();
    install(dir.path(), SandboxMode::Auto);
    let events = dir.path().join("events.jsonl");
    let scope = EventsScope::enter(Some(events.clone()));
    mark_backend_unavailable("fake backend failure".to_string());
    drop(scope);

    let text = std::fs::read_to_string(&events).unwrap();
    let event: Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    let mut keys: Vec<&str> = event
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    let want: Vec<&str> = corpus["contract"]["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| field.as_str().unwrap())
        .collect();
    assert_eq!(keys, want);
}

#[test]
fn an_err_in_the_child_gives_the_parent_destination_back() {
    let _state = SandboxTestState::acquire();
    clear_announced();
    let dir = tempfile::tempdir().unwrap();
    install(dir.path(), SandboxMode::Auto);
    let parent_path = dir.path().join("parent.jsonl");
    let child_path = dir.path().join("child.jsonl");
    let _parent = EventsScope::enter(Some(parent_path.clone()));

    let result: anyhow::Result<()> = (|| {
        let _child = EventsScope::enter(Some(child_path.clone()));
        mark_backend_unavailable("fake backend failure".to_string());
        anyhow::bail!("child run failed")
    })();

    assert!(result.is_err());
    assert_eq!(current_path(), Some(parent_path.clone()));
    assert_eq!(lines(&child_path), Some(1));
    assert_eq!(lines(&parent_path), None, "the parent file got nothing");
    assert!(!is_applied());
}

#[test]
fn a_panic_in_the_child_gives_the_parent_destination_back() {
    let _state = SandboxTestState::acquire();
    clear_announced();
    let dir = tempfile::tempdir().unwrap();
    install(dir.path(), SandboxMode::Auto);
    let parent_path = dir.path().join("parent.jsonl");
    let child_path = dir.path().join("child.jsonl");
    let _parent = EventsScope::enter(Some(parent_path.clone()));

    let caught = std::panic::catch_unwind(|| {
        let _child = EventsScope::enter(Some(child_path.clone()));
        mark_backend_unavailable("fake backend failure".to_string());
        panic!("child run unwound");
    });

    assert!(caught.is_err());
    assert_eq!(current_path(), Some(parent_path.clone()));
    assert_eq!(lines(&child_path), Some(1));
    assert_eq!(lines(&parent_path), None);
    let status = status().expect("the sandbox stays installed");
    assert!(!status.applied, "{status:?}");
}

#[test]
fn restoring_the_destination_does_not_let_required_pass_again() {
    let _state = SandboxTestState::acquire();
    clear_announced();
    let dir = tempfile::tempdir().unwrap();
    install(dir.path(), SandboxMode::Required);
    let parent_path = dir.path().join("parent.jsonl");
    let _parent = EventsScope::enter(Some(parent_path.clone()));

    {
        let _child = EventsScope::enter(Some(dir.path().join("child.jsonl")));
        mark_backend_unavailable("fake backend failure".to_string());
    }

    assert_eq!(current_path(), Some(parent_path));
    assert!(!is_applied());
    assert!(
        wrap_command(Command::new("sh")).is_err(),
        "required must keep refusing after the destination is restored"
    );
    assert_eq!(announced().len(), 1, "the downgrade is announced once");
}

#[cfg(target_os = "linux")]
#[test]
fn a_failing_landlock_wrap_is_announced_to_the_child_scope_only() {
    use crate::tools::os_sandbox::landlock::test_hooks::force_wrap_failure;

    struct HookReset;
    impl Drop for HookReset {
        fn drop(&mut self) {
            force_wrap_failure(false);
        }
    }

    let _state = SandboxTestState::acquire();
    let _reset = HookReset;
    clear_announced();
    let dir = tempfile::tempdir().unwrap();
    install(dir.path(), SandboxMode::Auto);
    let parent_path = dir.path().join("parent.jsonl");
    let child_path = dir.path().join("child.jsonl");
    force_wrap_failure(true);

    let _parent = EventsScope::enter(Some(parent_path.clone()));
    {
        let _child = EventsScope::enter(Some(child_path.clone()));
        assert!(wrap_command(Command::new("sh")).is_ok());
    }

    assert_eq!(lines(&child_path), Some(1));
    assert_eq!(lines(&parent_path), None);
}

/// Issue #803: the one announced event is both written to the file and projected
/// as exactly one TUI line; without a path it is projected and not written.
#[test]
fn a_downgrade_is_projected_once_with_or_without_a_path() {
    for with_path in [true, false] {
        let _state = SandboxTestState::acquire();
        clear_announced();
        let dir = tempfile::tempdir().unwrap();
        install(dir.path(), SandboxMode::Auto);
        let events = dir.path().join("events.jsonl");
        let scope = EventsScope::enter(with_path.then(|| events.clone()));
        mark_backend_unavailable("failed at /home/someone/.ssh".to_string());
        mark_backend_unavailable("a second failure".to_string());
        drop(scope);

        let calls = announced();
        assert_eq!(calls.len(), 1, "with_path={with_path}: {calls:?}");
        let line = crate::tui::presentation::render_activity_line(&calls[0].1)
            .expect("the downgrade is projected");
        assert!(!line.contains('\n') && !line.contains("/home"), "{line}");
        assert_eq!(
            lines(&events),
            with_path.then_some(1),
            "with_path={with_path}"
        );
    }
}
