use super::*;
use crate::mode::ExecutionMode;
use crate::tools::workspace_policy::WorkspacePolicy;

fn context(root: &Path) -> ToolContext {
    ToolContext {
        root: root.to_path_buf(),
        mode: ExecutionMode::Act,
        auto_approve: true,
        interactive_approval: false,
        offline: true,
        workspace_policy: WorkspacePolicy::NormalTask,
        eval_events_path: None,
        expected_paths: vec![],
        protected_paths: vec![],
    }
}

#[test]
fn read_failure_after_lookup_is_classified_after_a_plain_leaf_deletion() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("file.txt");
    std::fs::write(&path, "before").unwrap();
    let captured = ReadFailureContext::capture(&context(root.path()), "file.txt");
    let resolved = super::super::path_guard::resolve_existing(root.path(), "file.txt").unwrap();
    std::fs::remove_file(path).unwrap();
    let error = super::super::read::run(
        root.path(),
        &resolved,
        None,
        None,
        WorkspacePolicy::NormalTask,
    )
    .unwrap_err();
    assert!(from_error(&captured.classify(error, Some(&resolved))).is_some());
}

#[cfg(unix)]
#[test]
fn parent_symlink_swap_after_lookup_does_not_become_ordinary_missing() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("src")).unwrap();
    std::fs::write(root.path().join("src/file"), "before").unwrap();
    let captured = ReadFailureContext::capture(&context(root.path()), "src/file");
    let resolved = super::super::path_guard::resolve_existing(root.path(), "src/file").unwrap();
    std::fs::rename(root.path().join("src"), root.path().join("old-src")).unwrap();
    symlink(outside.path(), root.path().join("src")).unwrap();
    let error = super::super::read::run(
        root.path(),
        &resolved,
        None,
        None,
        WorkspacePolicy::NormalTask,
    )
    .unwrap_err();
    assert!(from_error(&captured.classify(error, Some(&resolved))).is_none());
    assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
}

#[test]
fn root_replacement_and_root_disappearance_do_not_become_ordinary_missing() {
    for replace in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("workspace");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("file"), "before").unwrap();
        let captured = ReadFailureContext::capture(&context(&root), "file");
        let resolved = super::super::path_guard::resolve_existing(&root, "file").unwrap();
        std::fs::rename(&root, temp.path().join("old")).unwrap();
        if replace {
            std::fs::create_dir(&root).unwrap();
        }
        let error =
            super::super::read::run(&root, &resolved, None, None, WorkspacePolicy::NormalTask)
                .unwrap_err();
        assert!(from_error(&captured.classify(error, Some(&resolved))).is_none());
    }
}

#[test]
fn a_disappearing_selected_path_is_not_misattributed_to_the_original_request() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("src")).unwrap();
    std::fs::write(root.path().join("src/file"), "existing fallback").unwrap();
    let captured = ReadFailureContext::capture(&context(root.path()), "file");
    std::fs::remove_file(root.path().join("src/file")).unwrap();
    let error = super::super::path_guard::resolve_existing(root.path(), "src/file").unwrap_err();
    assert!(from_error(&captured.classify_lookup(error, "src/file")).is_none());
}

#[test]
fn registry_preserves_suffix_fallback_and_rejects_its_disappearance_after_selection() {
    use crate::tools::registry::{ToolRegistry, recoverable_tool_error, tool_error_kind};
    use serde_json::{Value, json};
    for protected in [false, true] {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("src")).unwrap();
        std::fs::write(root.path().join("src/file"), "selected source").unwrap();
        std::fs::write(root.path().join("frozen-check"), "original check").unwrap();
        let events = root.path().join("events.jsonl");
        let mut context = context(root.path());
        context.expected_paths = vec!["src/file".into()];
        context.protected_paths = if protected {
            vec!["src/file".into()]
        } else {
            vec![]
        };
        context.eval_events_path = Some(events.clone());
        let arguments = json!({"path":"redundant/src/file"});
        let registry = ToolRegistry::default();
        assert!(
            registry
                .execute("Read", &arguments, &context)
                .unwrap()
                .contains("selected source")
        );
        let _reset = super::test_hook::install(|context, selected| {
            assert_eq!(selected, "src/file");
            // Environmental deletion between selection and lookup, not a tool
            // mutation or a production shortcut for the error classification.
            std::fs::remove_file(context.root.join(selected)).unwrap();
        });
        let error = registry.execute("Read", &arguments, &context).unwrap_err();
        assert_eq!(tool_error_kind(&error), "tool_execution_error", "{error:#}");
        assert!(!recoverable_tool_error(&error));
        assert!(error.to_string().contains("src/file"));
        assert_eq!(arguments, json!({"path":"redundant/src/file"}));
        assert_eq!(context.protected_paths.len(), usize::from(protected));
        assert!(!root.path().join("redundant").exists());
        assert!(!root.path().join("src/file").exists());
        assert_eq!(
            std::fs::read_to_string(root.path().join("frozen-check")).unwrap(),
            "original check"
        );
        let events: Vec<Value> = std::fs::read_to_string(events)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(
            events
                .iter()
                .filter(|event| event["event"] == "path_fallback_evaluated"
                    && event["accepted"] == true
                    && event["normalized"] == "src/file")
                .count(),
            2
        );
    }
}
