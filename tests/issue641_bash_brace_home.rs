#![cfg(unix)]

//! Issue #641: the brace-expansion proof never re-checked a `~` that a brace
//! produced, so `{~,x}` was proven as the workspace-relative names `~` and `x`
//! instead of the home directory and `x`.
//!
//! The shell expands a brace first and the tilde second
//! (`HOME=/HOMEX sh -c 'echo {~/f,y} {~,x}/f'` prints `/HOMEX/f y /HOMEX/f x/f`),
//! so the word that actually runs names home. The same gap reached writes
//! (`rm {~/f,x}`, `echo x > {~/f,y}`).
//!
//! This file drives the public second-stage predicate `R`
//! (`path_confinement_rejection`). A write-shaped command is refused by the
//! write guard before the read path runs, so its rejection carries the write
//! operation rather than `path reference`. Both the read `Expand` candidate and
//! the write proof close in the one place
//! (`path_guard::ensure_expanded_write_target`) this issue changes.
//!
//! ## False-rejection estimate (from the Issue body)
//!
//! The 436 distinct agent Bash commands recorded under `dev-reports/` and
//! `workspace/management/` contain no `~` in a brace, and the prototype changed
//! none of them. The reference set (~59k distinct Claude Code Bash commands)
//! showed 0 brace expansions and 2 `${...}` parameter expansions, neither a
//! brace. Newly refused forms: `echo {~,x}` and `ls {"~",x}` (both already
//! refused in their `..` spelling).
//!
//! `ls x{~,y}` and `ls {x~,y}` produce `x~`, which the shell does not
//! tilde-expand, so both stay allowed. A glob result that starts with `~` is
//! likewise not tilde-expanded and stays allowed.

use std::path::PathBuf;

use commandagent::tools::bash::path_confinement_rejection;

/// A fake canary written into the outside file; a refused command never reads
/// it, so it must not appear in any error message.
const CANARY: &str = "OUTER641_FAKE_CANARY_9f3c";

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
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

/// A workspace with the directories and files the workspace-inside brace forms
/// name, plus a file whose name starts with `~` for the glob case, and an
/// outside file holding the canary.
fn fixture() -> Fixture {
    let dir = tempdir();
    let root = dir.path().join("ws");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(root.join("tests")).unwrap();
    std::fs::write(root.join("a.txt"), "x").unwrap();
    std::fs::write(root.join("b.txt"), "x").unwrap();
    std::fs::write(root.join("~canary.txt"), "x").unwrap();
    let outside = dir.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("secret"), CANARY).unwrap();
    let root = root.canonicalize().unwrap();
    Fixture { _dir: dir, root }
}

/// Acceptance row 1: a brace that expands to a `~`-prefixed word is refused as
/// a read (`R=1`), reported as the fixed `path reference` operation that carries
/// no file contents.
#[test]
fn issue641_brace_tilde_read_forms_are_refused() {
    let fixture = fixture();
    let root = &fixture.root;
    for command in [
        "ls {~,x}",
        "ls {x,~}",
        "ls {~root,x}",
        "cat {~,~}",
        "ls {~/,x}",
        "ls {~+,x}",
        "cat {~/.bashrc,x}",
        "ls {~,x}/f",
        "ls {{~,x},y}",
        "du -sh {~,.}",
        "find {~,.} -name x",
        "grep -r k {~,src}",
    ] {
        let rejection = path_confinement_rejection(command, root)
            .unwrap_or_else(|| panic!("expected rejection: {command:?}"));
        assert_eq!(rejection.operation, "path reference", "{command:?}");
        assert!(
            !rejection.reason.contains(CANARY) && !rejection.message.contains(CANARY),
            "the canary leaked into the reason: {command:?}"
        );
    }
}

/// Acceptance row 2: a brace that expands to a `~`-prefixed word is refused as a
/// write (`W=1`). The write guard runs before the read path, so the rejection
/// carries the write operation instead of `path reference`.
#[test]
fn issue641_brace_tilde_write_forms_are_refused() {
    let fixture = fixture();
    let root = &fixture.root;
    for command in [
        "tee {~/f,y}",
        "cp a.txt {~/f,y}",
        "touch {~/f,x}",
        "rm {~/f,x}",
        "dd if=in of={~/f,y}",
        "echo x > {~/f,y}",
        "cp a.txt {x,~}",
    ] {
        let rejection = path_confinement_rejection(command, root)
            .unwrap_or_else(|| panic!("expected rejection: {command:?}"));
        assert_ne!(
            rejection.operation, "path reference",
            "the write guard must refuse it first: {command:?}"
        );
        assert!(
            !rejection.reason.contains(CANARY) && !rejection.message.contains(CANARY),
            "the canary leaked into the reason: {command:?}"
        );
    }
}

/// Acceptance row 2 (allow side): a workspace-inside brace write/read, a `~`
/// that is not the whole word, and a glob that merely *finds* a name starting
/// with `~` all stay allowed.
#[test]
fn issue641_workspace_braces_and_glob_matches_stay_allowed() {
    let fixture = fixture();
    let root = &fixture.root;
    for command in [
        "ls {src,tests}",
        "cat {a,b}.txt",
        "touch {a,b}.txt",
        "cp a.txt{,.bak}",
        // `x~` is not tilde-expanded by the shell.
        "ls x{~,y}",
        "ls {x~,y}",
        // A glob result starting with `~` is not tilde-expanded.
        "ls *",
        "cat *",
    ] {
        assert!(
            path_confinement_rejection(command, root).is_none(),
            "expected allow: {command:?}"
        );
    }
}

/// The same forms that the `..` spelling already refused keep their value: the
/// brace result still carries `..` or an empty word, so `path_confinement_rejection`
/// stays `Some` either way.
#[test]
fn issue641_already_refused_brace_shapes_stay_refused() {
    let fixture = fixture();
    let root = &fixture.root;
    for command in ["ls {..,x}", "ls {,~}", "echo x > {~/f,}", "mv a.txt {~/f,}"] {
        assert!(
            path_confinement_rejection(command, root).is_some(),
            "expected rejection: {command:?}"
        );
    }
}
