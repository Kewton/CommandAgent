#![cfg(unix)]

//! Issue #578: a Bash rejection that refuses the *form* of a command (ANSI-C /
//! locale quoting, `env -S` / `--split-string`, an unresolved command prefix, a
//! mixed glob/brace write target, an unreadable command text, or a backtick
//! command substitution) must not ask the caller to rewrite a workspace-relative
//! path. For those rejections the `path` field holds a spelling, not a path, and
//! `reason` already names the alternative spelling, so only the reason is shown.
//! A rejection whose path leaves the workspace keeps its message one byte for
//! one byte, together with the `nearest_relative` and `guidance` field values.
//!
//! The operation lists below freeze the distinction: an operation in
//! `FORM_OPERATIONS` is a form rejection, every other operation is a path
//! rejection. `is_command_form_operation` in `src/tools/bash.rs` mirrors the
//! form list, so a renamed or newly added fail-closed guard fails here.

use std::path::PathBuf;

use commandagent::mode::ExecutionMode;
use commandagent::tools::bash::{path_confinement_rejection, workspace_relative_retry_guidance};
use commandagent::tools::registry::tool_error_kind;
use commandagent::tools::registry::{ToolContext, ToolRegistry};
use commandagent::tools::workspace_policy::WorkspacePolicy;
use serde_json::json;

/// The operations that refuse the command's form rather than a path. Mirrors
/// `is_command_form_operation` in `src/tools/bash.rs`.
const FORM_OPERATIONS: &[&str] = &[
    "ANSI-C / locale quoting",
    "env -S / --split-string",
    "unresolved command prefix",
    "unreadable shell text",
    "unverifiable backtick command substitution",
    "unverifiable glob write target",
];

/// Representative path-rejection operations, one per message branch: the
/// write-target branch (`tee`, `working directory`) and the read-path branch
/// (`path reference`). Every operation outside `FORM_OPERATIONS` is a path
/// rejection; these pin the branch shapes.
const PATH_OPERATIONS: &[&str] = &["tee", "working directory", "path reference"];

const ERROR_PREFIX: &str = "bash_path_confinement_error: ";
/// The path-rewrite fragments that must never appear in a form rejection.
const PATH_RETRY_FRAGMENT: &str = "use workspace-relative path `";
const PATH_GUIDANCE_FRAGMENT: &str = "workspace相対で再実行せよ";
const BOUNDARY_FRAGMENT: &str =
    "Bash may create, modify, or delete only within current workspace root";

/// A tempdir outside the prefix `is_system_prefix_allowed` accepts, so a
/// fixture path outside the workspace is not admitted as a system path and the
/// test does not depend on the platform's temp root.
fn tempdir() -> tempfile::TempDir {
    let dir = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).expect("create tempdir");
    assert!(
        !["/usr", "/bin", "/opt", "/etc", "/tmp"]
            .iter()
            .any(|prefix| dir.path().starts_with(prefix)),
        "the fixture tempdir must not sit under a system prefix: {}",
        dir.path().display()
    );
    dir
}

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
    outside: PathBuf,
}

/// A workspace with an inside `sub` directory and `a.txt`, and an outside
/// directory holding a `secret`.
fn fixture() -> Fixture {
    let dir = tempdir();
    let root = dir.path().join("ws");
    std::fs::create_dir_all(root.join("sub")).unwrap();
    std::fs::write(root.join("a.txt"), "x").unwrap();
    let outside = dir.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("secret"), "outside-secret").unwrap();
    let root = root.canonicalize().unwrap();
    Fixture {
        _dir: dir,
        root,
        outside,
    }
}

fn context(root: PathBuf, events: PathBuf) -> ToolContext {
    ToolContext {
        root,
        mode: ExecutionMode::Act,
        auto_approve: true,
        interactive_approval: false,
        offline: false,
        workspace_policy: WorkspacePolicy::NormalTask,
        eval_events_path: Some(events),
        expected_paths: Vec::new(),
        protected_paths: Vec::new(),
    }
}

#[test]
fn form_and_path_operation_lists_are_disjoint() {
    for operation in FORM_OPERATIONS {
        assert!(
            !PATH_OPERATIONS.contains(operation),
            "the operation lists must not overlap: {operation}"
        );
    }
}

/// One command per form operation, with the operation it must carry and the
/// `nearest_relative` value the guard already recorded. The message is exactly
/// the reason: no path-rewrite retry, and no field value moved.
#[test]
fn form_rejection_message_is_only_the_reason() {
    let fixture = fixture();
    let root = &fixture.root;
    let cases = [
        ("$'tee' /tmp/f", "ANSI-C / locale quoting", "$'"),
        ("env -S'tee /tmp/f'", "env -S / --split-string", "f"),
        ("sudo -X u tee /tmp/f", "unresolved command prefix", "sudo"),
        (
            r#"echo "$(echo "x")""#,
            "unreadable shell text",
            "<shell text>",
        ),
        (
            "echo `tee /tmp/f`",
            "unverifiable backtick command substitution",
            "<backtick command substitution>",
        ),
        (
            r"echo x > sub/li[n\]]k/f",
            "unverifiable glob write target",
            "f",
        ),
    ];
    assert_eq!(
        cases.len(),
        FORM_OPERATIONS.len(),
        "one command per frozen form operation"
    );
    for (command, operation, nearest_relative) in cases {
        assert!(
            FORM_OPERATIONS.contains(&operation),
            "the frozen form list must name this operation: {operation}"
        );
        let rejection = path_confinement_rejection(command, root)
            .unwrap_or_else(|| panic!("expected rejection: {command:?}"));
        assert_eq!(rejection.operation, operation, "command: {command:?}");
        // Only the sentence changes: the message is the prefix and the reason.
        assert_eq!(
            rejection.message,
            format!("{ERROR_PREFIX}{}", rejection.reason),
            "command: {command:?}"
        );
        assert!(
            !rejection.message.contains(PATH_RETRY_FRAGMENT),
            "{command:?} must not ask for a workspace-relative path rewrite: {}",
            rejection.message
        );
        assert!(
            !rejection.message.contains(PATH_GUIDANCE_FRAGMENT),
            "{command:?} must not carry the path retry guidance: {}",
            rejection.message
        );
        assert!(
            !rejection.message.contains(BOUNDARY_FRAGMENT),
            "{command:?} must not carry the write-boundary retry: {}",
            rejection.message
        );
        // Field values are unchanged for the event and its schema.
        assert_eq!(rejection.nearest_relative, nearest_relative, "{command:?}");
        assert_eq!(
            rejection.guidance,
            workspace_relative_retry_guidance(nearest_relative),
            "{command:?}"
        );
    }
}

/// A rejection whose path leaves the workspace keeps its message byte for byte.
#[test]
fn path_rejection_message_keeps_the_workspace_relative_retry() {
    let fixture = fixture();
    let root = &fixture.root;
    let outside = fixture.outside.display().to_string();

    // The write-target branch: reason, path retry, and the boundary clause.
    for (command, operation) in [
        (format!("tee {outside}/f"), "tee"),
        (format!("cd '{outside}' && printf x"), "working directory"),
    ] {
        assert!(PATH_OPERATIONS.contains(&operation));
        let rejection = path_confinement_rejection(&command, root)
            .unwrap_or_else(|| panic!("expected rejection: {command:?}"));
        assert_eq!(rejection.operation, operation, "command: {command:?}");
        assert!(
            !FORM_OPERATIONS.contains(&rejection.operation.as_str()),
            "a path rejection must not be classified as a command form: {command:?}"
        );
        let expected = format!(
            "{ERROR_PREFIX}{}; use workspace-relative path `{}`; {}; {BOUNDARY_FRAGMENT} `{}`",
            rejection.reason, rejection.nearest_relative, rejection.guidance, rejection.root,
        );
        assert_eq!(rejection.message, expected, "command: {command:?}");
    }

    // The read-path branch: the same retry, without the boundary clause.
    let candidate = format!("{outside}/secret");
    let rejection = path_confinement_rejection(&format!("cat {candidate}"), root)
        .expect("expected read-path rejection");
    assert_eq!(rejection.operation, "path reference");
    assert!(PATH_OPERATIONS.contains(&"path reference"));
    let expected = format!(
        "{ERROR_PREFIX}rejected absolute path `{candidate}` outside current workspace root `{}`; use workspace-relative path `{}`; {}",
        rejection.root, rejection.nearest_relative, rejection.guidance,
    );
    assert_eq!(rejection.message, expected);
}

/// The rejection event keeps its name and schema for a form rejection: the
/// `nearest_relative` and `guidance` fields are still recorded with their
/// values, so only the human-readable sentence changed.
#[test]
fn form_rejection_event_keeps_the_existing_schema() {
    let fixture = fixture();
    let events = fixture._dir.path().join("events.jsonl");
    let command = "env -S'tee /tmp/f'";

    let error = ToolRegistry::default()
        .execute(
            "Bash",
            &json!({ "command": command }),
            &context(fixture.root.clone(), events.clone()),
        )
        .unwrap_err();

    assert_eq!(tool_error_kind(&error), "bash_path_confinement_error");
    assert!(!error.to_string().contains(PATH_RETRY_FRAGMENT), "{error}");

    let event_text = std::fs::read_to_string(&events).unwrap();
    let event = event_text
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .find(|event| event["event"] == "bash_path_confinement_rejected")
        .unwrap_or_else(|| panic!("no rejection event: {event_text}"));
    // The event keeps its name and schema: the same fields, with values.
    assert_eq!(event["schema_version"], "1", "{event_text}");
    assert_eq!(event["blocked"], true, "{event_text}");
    assert_eq!(
        event["operation"], "env -S / --split-string",
        "{event_text}"
    );
    let nearest_relative = event["nearest_relative"]
        .as_str()
        .unwrap_or_else(|| panic!("nearest_relative missing: {event_text}"));
    assert_eq!(nearest_relative, "f", "{event_text}");
    assert_eq!(
        event["guidance"],
        workspace_relative_retry_guidance(nearest_relative),
        "{event_text}"
    );
}
