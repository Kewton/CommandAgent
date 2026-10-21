#![cfg(unix)]

//! Issue #628: the second-stage Bash confinement missed two shapes found while
//! scoping #613.
//!
//! 1. A `..`-only word and a `~`-prefixed `/`-less word were never literal read
//!    candidates: `is_literal_path` requires a `/`, the `/`-less symlink check
//!    deliberately excludes `..` and `~`, and so `ls ..` and `ls ~` read the
//!    workspace parent or the home directory with `R=0`. The `=` right-hand side
//!    (`make PREFIX=~`, `cat --file=..`) and the value of an option that changes
//!    the directory (`tar -C ..`, `git -C ..`) had the same gap.
//! 2. `dd`'s `of=` was not a write target, so `dd if=in of=out` was not a
//!    recognized mutation (`A=0`), was auto-approved by `bash:verify` (`V=1`),
//!    and no protected-path write was detected (`P=0`).
//!
//! This file drives the public second-stage predicate `R`
//! (`path_confinement_rejection`) and the verify auto-approval `V`. The
//! crate-private `A`/`W`/`P` (`has_recognized_mutation`, `confinement_rejection`,
//! `protected_path_mutation`) are pinned by the unit tests in
//! `src/tools/bash/read_guard.rs` and `src/tools/bash_write_guard.rs`, and by the
//! corpus table `tests/corpus/apps/issue582-bash-read-candidates/fixtures/decision-cases.jsonl`.
//!
//! ## False-rejection estimate
//!
//! The 436 distinct agent Bash commands recorded under `dev-reports/` and
//! `workspace/management/` contain no `..`-only word, no `~`-only word, no `dd`,
//! and no `-C` option, so this change moves 0 of them. (Reproduced from the
//! pre-work investigation recorded for Issue #628.)
//!
//! ## Newly refused forms, and how to rewrite them
//!
//! * `ls ..` — list the workspace root instead (`ls`).
//! * `cd sub && ls ..` — the parent component is refused regardless of the
//!   working directory; list a workspace-relative path instead.
//! * `ls "~"` (a file named `~`) — the lexical reading cannot tell the quotes
//!   apart, so prefix the name: `ls ./~`.
//! * `make PREFIX=~` — the value points outside the workspace; pass a
//!   workspace-relative value instead.
//! * `dd if=in of=out` — the write is now a recognized mutation, so it is no
//!   longer auto-approved by `bash:verify`; run it through the normal write
//!   authorization.

use std::path::PathBuf;

use commandagent::mode::ExecutionMode;
use commandagent::tools::allow_policy::bash_verify_command_is_auto_approvable;
use commandagent::tools::bash::path_confinement_rejection;
use commandagent::tools::registry::tool_error_kind;
use commandagent::tools::registry::{ToolContext, ToolRegistry};
use commandagent::tools::workspace_policy::WorkspacePolicy;
use serde_json::json;

/// A fake canary written into the outside file; a refused command never reads
/// it, so it must not appear in any error, event, or report output.
const CANARY: &str = "OUTER628_FAKE_CANARY_4d1a";

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
    outside: PathBuf,
}

/// A tempdir outside the prefix `is_system_prefix_allowed` accepts (`/usr`,
/// `/bin`, `/opt`, `/etc`, `/tmp`). `tempfile::tempdir()` lives under `/tmp` on
/// Linux, which would let a fixture path outside the workspace be admitted as a
/// system path and fail only on Linux; `CARGO_TARGET_TMPDIR` is under the target
/// directory instead.
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

/// A workspace with a regular file, an inside directory, and three outward
/// symlinks (`lf`, `sub/link`, `linked-outside`) an absolute or relative `of=`
/// can name.
fn fixture() -> Fixture {
    let dir = tempdir();
    let root = dir.path().join("ws");
    std::fs::create_dir_all(root.join("sub")).unwrap();
    std::fs::write(root.join("a.txt"), "x").unwrap();
    let outside = dir.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("secret"), CANARY).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("lf")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("sub/link")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("linked-outside")).unwrap();
    let root = root.canonicalize().unwrap();
    Fixture {
        _dir: dir,
        root,
        outside,
    }
}

/// Issue #628 problem 1: a `..`-only word, a `~`-prefixed `/`-less word, the `=`
/// right-hand side, and an option value that names the parent are all refused,
/// and the read refusal is the fixed `path reference` operation that carries no
/// file contents. Every one of them keeps `V=1`, so the second-stage refusal
/// rejects them before the shell starts.
#[test]
fn issue628_parent_and_home_words_are_refused() {
    let fixture = fixture();
    let root = &fixture.root;
    for command in [
        "ls ..",
        "ls -la ..",
        "du -sh ..",
        "find .. -name x",
        "grep -r foo ..",
        "cat ..",
        "ls '..'",
        "ls ~",
        "cat ~",
        "ls ~root",
        "ls ~+",
        "ls ~-",
        "make PREFIX=~",
        "cat --file=..",
        "tar -C .. -cf - x",
        "git -C .. status",
    ] {
        let rejection = path_confinement_rejection(command, root)
            .unwrap_or_else(|| panic!("expected rejection: {command:?}"));
        assert_eq!(rejection.operation, "path reference", "{command:?}");
        assert!(
            !rejection.reason.contains(CANARY) && !rejection.message.contains(CANARY),
            "the canary leaked into the reason: {command:?}"
        );
        assert!(
            bash_verify_command_is_auto_approvable(command, root),
            "the verify allow-list stays unchanged for {command:?}"
        );
    }
    // A `cd` before the parent word does not change the decision: the parent
    // component is refused regardless of the working directory.
    assert!(path_confinement_rejection("cd sub && ls ..", root).is_some());
    // A `/`-containing spelling already reached the literal proof before this
    // change (`../x`, `~/x`, `sub/..`); its value is unchanged.
    for command in ["ls ../", "cat ../x", "ls ~/", "cat ~/x", "ls sub/.."] {
        assert!(
            path_confinement_rejection(command, root).is_some(),
            "expected rejection: {command:?}"
        );
    }
}

/// Issue #628 problem 2: `dd`'s `of=` is a write target. An inside write stays
/// allowed (`R=0`) but is a recognized mutation, so it loses `V`. An outward
/// `of=` is refused with the `dd` operation, and an absolute outward `of=` is
/// refused too.
#[test]
fn issue628_dd_of_is_a_write_and_loses_verify_auto_approval() {
    let fixture = fixture();
    let root = &fixture.root;
    for command in [
        "dd if=in.bin of=out.bin",
        "dd if=in of=/dev/null",
        "dd if=in",
    ] {
        assert!(
            path_confinement_rejection(command, root).is_none(),
            "R must stay 0: {command:?}"
        );
    }
    for command in [
        "dd if=in.bin of=out.bin".to_string(),
        "dd if=in of=/dev/null".to_string(),
        "dd if=in of=../outside/x".to_string(),
        "dd if=in of=~/f".to_string(),
        format!("dd if=in of={}", fixture.outside.join("out").display()),
    ] {
        assert!(
            !bash_verify_command_is_auto_approvable(&command, root),
            "V must be 0: {command:?}"
        );
    }
    for command in [
        "dd if=in of=lf".to_string(),
        "dd if=in of=../outside/x".to_string(),
        "dd if=in of=~/f".to_string(),
        format!("dd if=in of={}", fixture.outside.join("out").display()),
    ] {
        let rejection = path_confinement_rejection(&command, root)
            .unwrap_or_else(|| panic!("expected rejection: {command:?}"));
        assert_eq!(rejection.operation, "dd", "{command:?}");
    }
    // `dd if=in` names no write, so it keeps its existing auto-approval.
    assert!(bash_verify_command_is_auto_approvable("dd if=in", root));
}

/// Acceptance criterion: workspace-inside reads and writes, the `echo` display
/// forms, a leading assignment, git ranges, and a `~` that is not a whole word
/// keep their value. `cd ..` keeps its working-directory refusal.
#[test]
fn issue628_inside_and_display_forms_are_unchanged() {
    let fixture = fixture();
    let root = &fixture.root;
    for command in [
        "ls",
        "ls src",
        "ls -la",
        "ls .",
        "cat a.txt",
        "echo ~",
        "echo ..",
        "git diff main..feature",
        "git log HEAD~1",
        "X=~ cargo test",
        "dd if=in of=out.bin",
        "dd if=in of=a of=b",
        "dd if=in of=",
    ] {
        assert!(
            path_confinement_rejection(command, root).is_none(),
            "expected allow: {command:?}"
        );
    }
    // `cd ..` is refused by the working-directory write side, unchanged.
    let rejection = path_confinement_rejection("cd ..", root).expect("cd .. is refused");
    assert_eq!(rejection.operation, "working directory");
    assert!(path_confinement_rejection("cd ~", root).is_some());
}

/// A rejected parent/home read and a rejected `dd` write stop before the shell
/// starts: the registry returns `bash_path_confinement_error` and the fake
/// canary never reaches stdout, stderr, or the event log.
#[test]
fn issue628_refusals_stop_before_the_shell_and_leak_no_canary() {
    let fixture = fixture();
    let events = fixture._dir.path().join("events.jsonl");
    let context = ToolContext {
        root: fixture.root.clone(),
        mode: ExecutionMode::Act,
        auto_approve: true,
        interactive_approval: false,
        offline: true,
        workspace_policy: WorkspacePolicy::NormalTask,
        eval_events_path: Some(events.clone()),
        expected_paths: Vec::new(),
        protected_paths: Vec::new(),
    };
    let registry = ToolRegistry::default();
    for command in ["ls ..", "cat ~", "dd if=in of=lf"] {
        let error = registry
            .execute("Bash", &json!({ "command": command }), &context)
            .unwrap_err();
        assert_eq!(
            tool_error_kind(&error),
            "bash_path_confinement_error",
            "{command}"
        );
        assert!(!error.to_string().contains(CANARY), "{command}");
    }
    let log = std::fs::read_to_string(&events).unwrap_or_default();
    assert!(
        !log.contains(CANARY),
        "the canary must not appear in the event log"
    );
}
