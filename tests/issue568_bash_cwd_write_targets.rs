#![cfg(unix)]

//! Issue #568: the Bash write-target guard judged a relative path against the
//! workspace root only, so a `cd`/`pushd` to a directory whose child is an
//! escaping symlink let the write (or read) leave the workspace, and
//! `pushd`/`popd` were not recognized at all. The guard now tracks the union of
//! the working directories the command could run in and refuses a relative
//! write (or read) that escapes from any of them.
//!
//! Every test name contains `working_directory` so the mutation-test filter
//! selects these tests together with the unit tests in
//! `bash_write_guard/working_directory.rs`.

use std::path::PathBuf;

use commandagent::mode::ExecutionMode;
use commandagent::tools::bash::path_confinement_rejection;
use commandagent::tools::registry::{ToolContext, ToolRegistry, tool_error_kind};
use commandagent::tools::workspace_policy::WorkspacePolicy;
use serde_json::json;

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

/// The issue fixture: `sub/link`, `linked-outside`, and `a/b/link` are escaping
/// symlinks, the root has no `link`, the root `elink` escapes while `sub/elink`
/// is an inside directory.
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("ws");
    let outside = dir.path().join("outside");
    for directory in [
        "sub/elink",
        "sub/tests",
        "tests",
        "frontend",
        "crates/x",
        "src",
        "a/b",
    ] {
        std::fs::create_dir_all(root.join(directory)).unwrap();
    }
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("secret"), "outside-secret").unwrap();
    std::fs::write(root.join("a.txt"), "x").unwrap();
    std::fs::write(root.join("Cargo.toml"), "[package]\nname=\"x\"\n").unwrap();
    std::os::unix::fs::symlink(&outside, root.join("sub/link")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("linked-outside")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("a/b/link")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("elink")).unwrap();
    let root = root.canonicalize().unwrap();
    Fixture { _dir: dir, root }
}

fn context(root: &std::path::Path, events: &std::path::Path) -> ToolContext {
    ToolContext {
        root: root.to_path_buf(),
        mode: ExecutionMode::Act,
        auto_approve: true,
        interactive_approval: false,
        offline: false,
        workspace_policy: WorkspacePolicy::NormalTask,
        eval_events_path: Some(events.to_path_buf()),
        expected_paths: Vec::new(),
        protected_paths: Vec::new(),
    }
}

#[test]
fn working_directory_rejects_writes_after_cd_and_pushd() {
    let fixture = fixture();
    let root = &fixture.root;
    let root_absolute = root.display();
    let cases = [
        // The issue table.
        "cd sub && tee link/f".to_string(),
        "cd sub && printf x > link/f".to_string(),
        "pushd linked-outside && printf x > f".to_string(),
        // Separators must not hide the cwd change.
        "cd sub; tee link/f".to_string(),
        "cd sub\ntee link/f".to_string(),
        "cd sub || tee link/f".to_string(),
        "cd sub | tee link/f".to_string(),
        "(cd sub); tee link/f".to_string(),
        "{ cd sub; tee link/f; }".to_string(),
        "if cd sub; then tee link/f; fi".to_string(),
        // cd spellings and repetition.
        "cd -P sub && touch link/f".to_string(),
        "cd -- sub && touch link/f".to_string(),
        "cd \"sub\" && touch link/f".to_string(),
        format!("cd {root_absolute}/sub && tee link/f"),
        "cd a; cd b; tee link/f".to_string(),
        "cd sub && cd link && touch f".to_string(),
        "cd sub && cd link".to_string(),
        // pushd forms.
        "pushd sub && tee link/f".to_string(),
        "pushd -n sub && tee link/f".to_string(),
        // CDPATH.
        "CDPATH=sub cd link && tee f".to_string(),
        "export CDPATH=sub; cd link; tee f".to_string(),
        // Loops and functions.
        "for i in 1; do cd sub; done; tee link/f".to_string(),
        "f() { cd sub; }; f; tee link/f".to_string(),
        // Prefixed cd.
        "builtin cd sub && tee link/f".to_string(),
        "env cd sub && tee link/f".to_string(),
        "env cd sub && tee elink/f".to_string(),
        // ANSI-C quoting is refused first.
        "cd sub && $'tee' link/f".to_string(),
    ];
    for command in cases {
        assert!(
            path_confinement_rejection(&command, root).is_some(),
            "expected rejection: {command}"
        );
    }
}

#[test]
fn working_directory_rejects_reads_after_cd() {
    let fixture = fixture();
    let root = &fixture.root;
    for command in [
        "cd sub && cat link/secret",
        "cd sub && grep -r x link",
        "cd sub && ls link",
    ] {
        assert!(
            path_confinement_rejection(command, root).is_some(),
            "expected rejection: {command}"
        );
    }
}

#[test]
fn working_directory_rejects_dirstack_and_cdpath_reads() {
    let fixture = fixture();
    let root = &fixture.root;
    let cases = [
        "pushd . ; DIRSTACK[1]=sub/link; popd; tee f",
        "pushd . ; DIRSTACK[1]=sub/link; popd; printf x > f",
        "pushd . ; DIRSTACK[1]=sub/link; pushd +1; tee f",
        "pushd . ; DIRSTACK[1]=linked-outside; popd; tee f",
        "pushd . ; DIRSTACK[1]=sub/link; popd; cat secret",
        "CDPATH=sub cd link && cat secret",
        "CDPATH=sub cd link && grep x secret",
    ];
    for command in cases {
        assert!(
            path_confinement_rejection(command, root).is_some(),
            "expected rejection: {command}"
        );
    }
}

#[test]
fn working_directory_caps_candidate_growth_within_a_second() {
    let fixture = fixture();
    let root = &fixture.root;
    let cds = (0..40)
        .map(|index| format!("cd d{index}"))
        .collect::<Vec<_>>()
        .join("; ");

    assert!(
        path_confinement_rejection(&cds, root).is_none(),
        "a long cd chain without a write must stay allowed"
    );

    let start = std::time::Instant::now();
    let rejection = path_confinement_rejection(&format!("{cds}; tee f"), root);
    assert!(
        start.elapsed().as_secs() < 1,
        "the capped candidate walk must return within a second"
    );
    assert!(
        rejection.is_some(),
        "a relative write after the cap must be rejected"
    );
}

#[test]
fn working_directory_names_the_resolved_operation() {
    let fixture = fixture();
    let root = &fixture.root;
    for (command, operation) in [
        ("cd sub && tee link/f", "tee"),
        ("cd sub && printf x > link/f", "output redirection"),
        ("cd a; cd b; tee link/f", "tee"),
        ("pushd linked-outside", "working directory"),
    ] {
        let rejection = path_confinement_rejection(command, root)
            .unwrap_or_else(|| panic!("expected rejection: {command}"));
        assert_eq!(rejection.operation, operation, "command: {command}");
    }
}

#[test]
fn working_directory_keeps_inside_writes_and_verification_allowed() {
    let fixture = fixture();
    let root = &fixture.root;
    let root_absolute = root.display();
    let cases = [
        "cd src && touch a".to_string(),
        "cd src && echo x > a.txt".to_string(),
        "cd frontend && npm test > out.log".to_string(),
        "cd crates/x && cargo test 2>&1 | tee test.log".to_string(),
        format!("cd {root_absolute} && mkdir -p out"),
        "cd sub && tee /dev/null".to_string(),
        // Verify auto-approval is unaffected.
        "cd frontend && npm test".to_string(),
        "cd crates/x && cargo test".to_string(),
        // No write, so the working-directory target alone stays allowed.
        "pushd sub".to_string(),
        "pushd sub; tee f".to_string(),
        "popd".to_string(),
        "dirs".to_string(),
        "cd link".to_string(),
    ];
    for command in cases {
        assert!(
            path_confinement_rejection(&command, root).is_none(),
            "expected allow: {command}"
        );
    }
}

#[test]
fn working_directory_blocks_escapes_before_execution() {
    let fixture = fixture();
    let events = fixture._dir.path().join("events.jsonl");
    let registry = ToolRegistry::default();
    for command in [
        "cd sub && tee link/f",
        "pushd linked-outside && printf x > f",
        "cd sub && cat link/secret",
    ] {
        let error = registry
            .execute(
                "Bash",
                &json!({ "command": command }),
                &context(&fixture.root, &events),
            )
            .unwrap_err();
        assert_eq!(
            tool_error_kind(&error),
            "bash_path_confinement_error",
            "{command}"
        );
    }
    assert!(
        !fixture.root.join("sub/link").join("f").exists(),
        "the escaping write must not run"
    );
}
