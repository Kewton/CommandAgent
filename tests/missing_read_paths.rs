use std::path::Path;

use commandagent::mode::ExecutionMode;
use commandagent::tools::registry::{
    ToolContext, ToolRegistry, recoverable_tool_error, tool_error_kind,
};
use commandagent::tools::workspace_policy::WorkspacePolicy;
use serde_json::json;

const MISSING: &str = "read_path_missing";

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

fn read_error(context: &ToolContext, path: &str) -> anyhow::Error {
    ToolRegistry::default()
        .execute("Read", &json!({"path": path}), context)
        .unwrap_err()
}

#[test]
fn ordinary_missing_leaf_and_parents_are_errors_without_filesystem_changes() {
    let root = tempfile::tempdir().unwrap();
    let context = context(root.path());
    for path in ["new.txt", "src/lib/new.txt", "./src/./lib/new.txt"] {
        let error = read_error(&context, path);
        assert_eq!(tool_error_kind(&error), MISSING, "{error:#}");
        assert!(recoverable_tool_error(&error));
    }
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}

#[test]
fn filename_text_cannot_spoof_missing_error_classification() {
    let root = tempfile::tempdir().unwrap();
    let context = context(root.path());
    for path in [
        "verify_command_policy_error.txt",
        "path_not_found_recoverable.txt",
        "workspace_policy_blocked.txt",
        "missing string argument `path`",
    ] {
        assert_eq!(tool_error_kind(&read_error(&context, path)), MISSING);
    }
    let forged = anyhow::anyhow!("read_path_missing: path does not exist: x");
    assert!(!recoverable_tool_error(&forged));
}

#[test]
fn hidden_and_policy_blocked_missing_paths_keep_their_rejections() {
    let root = tempfile::tempdir().unwrap();
    let context = context(root.path());
    for path in [
        ".commandagent/missing.json",
        ".anvil/missing.json",
        ".git/config",
        ".next/output.json",
        "node_modules/pkg/index.js",
        "src/../outside",
    ] {
        let error = read_error(&context, path);
        assert_ne!(tool_error_kind(&error), MISSING, "{path}: {error:#}");
    }
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}

#[test]
fn root_failures_and_non_directory_parents_are_not_ordinary_missing() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("file"), "content").unwrap();
    for (target_root, path) in [
        (root.path().join("absent-root"), "x"),
        (root.path().join("file"), "x"),
        (root.path().to_path_buf(), "file/child"),
    ] {
        let error = read_error(&context(&target_root), path);
        assert_ne!(tool_error_kind(&error), MISSING, "{error:#}");
        assert!(!recoverable_tool_error(&error), "{error:#}");
    }
}

#[test]
fn protected_missing_inputs_are_not_repairable_but_existing_reads_remain_allowed() {
    let root = tempfile::tempdir().unwrap();
    let mut context = context(root.path());
    context.protected_paths = vec!["checks".into()];
    assert_ne!(
        tool_error_kind(&read_error(&context, "checks/verify.cjs")),
        MISSING
    );
    std::fs::create_dir(root.path().join("checks")).unwrap();
    std::fs::write(root.path().join("checks/verify.cjs"), "frozen check").unwrap();
    let registry = ToolRegistry::default();
    assert!(
        registry
            .execute("Read", &json!({"path": "checks/verify.cjs"}), &context)
            .unwrap()
            .contains("frozen check")
    );
    let error = registry
        .execute(
            "Write",
            &json!({"path": "checks/verify.cjs", "content": "weakened"}),
            &context,
        )
        .unwrap_err();
    assert_eq!(tool_error_kind(&error), "protected_path_mutation_rejected");
    assert_eq!(
        std::fs::read_to_string(root.path().join("checks/verify.cjs")).unwrap(),
        "frozen check"
    );
}

#[test]
fn missing_edit_does_not_inherit_read_recovery() {
    let root = tempfile::tempdir().unwrap();
    let error = ToolRegistry::default()
        .execute(
            "Edit",
            &json!({"path": "absent.txt", "old_string": "old", "new_string": "new"}),
            &context(root.path()),
        )
        .unwrap_err();
    assert_ne!(tool_error_kind(&error), MISSING);
    assert!(!root.path().join("absent.txt").exists());
}

#[test]
fn raw_protected_path_is_not_erased_by_redundant_root_prefix_normalization() {
    let root = tempfile::tempdir().unwrap();
    let mut context = context(root.path());
    let raw = format!(
        "{}/checks/verify.cjs",
        root.path().file_name().unwrap().to_str().unwrap()
    );
    context.protected_paths = vec![raw.clone()];
    assert_ne!(tool_error_kind(&read_error(&context, &raw)), MISSING);
}

#[test]
fn deleted_file_does_not_return_cached_content_and_recreated_file_can_be_read() {
    let root = tempfile::tempdir().unwrap();
    let context = context(root.path());
    let registry = ToolRegistry::default();
    let path = root.path().join("file.txt");
    std::fs::write(&path, "before").unwrap();
    assert!(
        registry
            .execute("Read", &json!({"path": "file.txt"}), &context)
            .unwrap()
            .contains("before")
    );
    std::fs::remove_file(&path).unwrap();
    let error = registry
        .execute("Read", &json!({"path": "file.txt"}), &context)
        .unwrap_err();
    assert_eq!(tool_error_kind(&error), MISSING);
    std::fs::write(&path, "after").unwrap();
    let output = registry
        .execute("Read", &json!({"path": "file.txt"}), &context)
        .unwrap();
    assert!(output.contains("after"));
    assert!(!output.contains("before"));
}

#[cfg(unix)]
#[test]
fn missing_through_symlinks_is_excluded_including_absolute_normalization() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("real")).unwrap();
    symlink("real", root.path().join("inside")).unwrap();
    symlink("absent", root.path().join("broken")).unwrap();
    symlink(outside.path(), root.path().join("outside")).unwrap();
    symlink(
        outside.path().join("absent"),
        root.path().join("broken-outside"),
    )
    .unwrap();
    symlink("cycle", root.path().join("cycle")).unwrap();
    let context = context(root.path());
    for path in [
        "inside/missing",
        "broken",
        "broken/child",
        "outside/missing",
        "broken-outside",
        "cycle",
    ] {
        for request in [
            path.to_string(),
            root.path().join(path).to_string_lossy().into_owned(),
        ] {
            let error = read_error(&context, &request);
            assert_ne!(tool_error_kind(&error), MISSING, "{request}: {error:#}");
        }
    }
    std::fs::write(root.path().join("real/existing"), "allowed").unwrap();
    let output = ToolRegistry::default()
        .execute("Read", &json!({"path": "inside/existing"}), &context)
        .unwrap();
    assert!(output.contains("allowed"));
    assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn actual_permission_denial_is_not_ordinary_missing() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let private = root.path().join("private");
    std::fs::create_dir(&private).unwrap();
    std::fs::set_permissions(&private, std::fs::Permissions::from_mode(0o000)).unwrap();
    let probe = std::fs::symlink_metadata(private.join("missing"));
    let result = read_error(&context(root.path()), "private/missing");
    std::fs::set_permissions(&private, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        probe.unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied,
        "this test must exercise actual EACCES; run under an unprivileged account"
    );
    assert_ne!(tool_error_kind(&result), MISSING);
    assert!(!recoverable_tool_error(&result));
}
