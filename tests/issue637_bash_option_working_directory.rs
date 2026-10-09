#![cfg(unix)]

//! Issue #637: a program's option can change the working directory
//! (`tar -C DIR`, `git -C DIR`, `make -C DIR`, `env -C DIR`, `sudo -D DIR`, ...)
//! and a `pushd` after `--` can name a `-`-prefixed directory. Neither was a
//! working-directory candidate, so a following relative `/`-less path was judged
//! against the workspace root alone and an outward symlink inside the option's
//! directory escaped. This file fixes the second-stage decision (`R`), the
//! write-side decision (`W`) for the `pushd --` shape, and the refusal reason
//! path, which must name the destination word (including an option value) rather
//! than the following command word.
//!
//! False-rejection estimate (from the issue, recorded here as evidence): of the
//! 436 distinct agent Bash commands in `dev-reports` and `workspace/management`,
//! zero use a table option or `pushd --`, so no recorded command changes. Of the
//! ~59k distinct commands in this repository, the added candidates are `git -C`
//! 1,745, `tar -C`/`--directory` 108, `env -C` 28, `sudo -D` 11, npm-family 54,
//! `cargo -C` 8, `make -C` 1, and `pushd --` 0; a candidate is added only, so a
//! command is refused only when the added destination holds an outward symlink
//! or when the value holds a glob/`$`. The one newly refused command kept inside
//! the workspace was `git -C sub status; cat lf2` (the candidate stays for the
//! later segment), and the inside commands listed in
//! `inside_workspace_option_directories_are_allowed` are unchanged.

use std::path::{Path, PathBuf};

use commandagent::tools::bash::path_confinement_rejection;

/// The fixed operation a read-side refusal carries.
const READ_OPERATION: &str = "path reference";
/// The existing write-side working-directory reason.
const CWD_REASON: &str = "follows a working directory change that cannot be determined";

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

/// A tempdir outside the prefix `is_system_prefix_allowed` accepts (`/usr`,
/// `/bin`, `/opt`, `/etc`, `/tmp`), so an escaping symlink target is never
/// admitted as a system path. `sub` is an ordinary directory holding the
/// outward symlink `lf2` and the sub-directory `deep/lf3` (also outward);
/// `s2` holds `lf4`, `s[2]` holds `lf5`, and the `-`-named directory `-s2` holds
/// `lf7`. The workspace root holds the regular files `f` and `a.tar`.
fn fixture() -> Fixture {
    let dir = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).expect("create tempdir");
    let root = dir.path().join("ws");
    for directory in ["sub", "sub/deep", "s2", "s[2]", "-s2"] {
        std::fs::create_dir_all(root.join(directory)).unwrap();
    }
    std::fs::write(root.join("f"), "x").unwrap();
    std::fs::write(root.join("a.tar"), "x").unwrap();
    std::fs::write(root.join("sub/f"), "x").unwrap();
    let outside = dir.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("secret"), "outside-secret").unwrap();
    for link in ["sub/lf2", "sub/deep/lf3", "s2/lf4", "s[2]/lf5", "-s2/lf7"] {
        std::os::unix::fs::symlink(&outside, root.join(link)).unwrap();
    }
    let root = root.canonicalize().unwrap();
    Fixture { _dir: dir, root }
}

/// Asserts every command is refused by a read-side `path reference` that names
/// the working-directory reason where the destination cannot be read.
fn assert_option_read_rejected(root: &Path, commands: &[String]) {
    for command in commands {
        let rejection = path_confinement_rejection(command, root)
            .unwrap_or_else(|| panic!("expected rejection: {command}"));
        assert_eq!(
            rejection.operation, READ_OPERATION,
            "command: {command} ({})",
            rejection.reason
        );
    }
}

/// The issue table: every shape that changes the working directory through an
/// option must make the following relative read escape, so `R=1`. The value
/// forms (`-C dir`, `-Cdir`, `--directory=dir`, `--directory dir`), the table
/// programs, the glob/brace values, and the `cd`-then-option shapes are all
/// covered.
#[test]
fn option_directory_rejects_relative_reads() {
    let fixture = fixture();
    let root = &fixture.root;
    let cases: Vec<String> = [
        // tar value forms.
        "tar -C sub -cf - lf2",
        "tar -cf - -C sub lf2",
        "tar -Csub -cf - lf2",
        "tar --directory=sub -cf - lf2",
        "tar --directory sub -cf - lf2",
        "tar -xf a.tar -C sub lf2",
        // git / make.
        "git -C sub log lf2",
        "git -C sub -C deep log lf3",
        "make -C sub lf2",
        "make --directory=sub lf2",
        // prefixes.
        "env -C sub cat lf2",
        "env --chdir=sub cat lf2",
        "nice env -C sub cat lf2",
        "sudo -D sub cat lf2",
        "sudo --chdir=sub cat lf2",
        // the wider table.
        "ninja -C sub lf2",
        "pnpm -C sub exec cat lf2",
        "yarn --cwd sub cat lf2",
        "npm --prefix sub exec cat lf2",
        "go -C sub run lf2",
        "uv --directory sub run cat lf2",
        "poetry -C sub run cat lf2",
        "just -d sub cat lf2",
        "cargo -C sub run lf2",
        "patch -d sub lf2 < x.patch",
        // glob and brace values add no candidate but still refuse.
        "tar -C s2* -cf - lf4",
        "git -C s? log lf4",
        "make -C {s2,x} lf4",
        // a `cd` candidate combined with an option candidate.
        "cd sub && tar -C deep -cf - lf3",
        "tar -C sub -cf - deep/lf3",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    assert_option_read_rejected(root, &cases);
}

/// The issue problem 2, reads: `pushd --` reads a `-`-prefixed word as a
/// destination, so the relative read after it escapes the `-s2` directory.
#[test]
fn pushd_double_dash_rejects_relative_reads() {
    let fixture = fixture();
    let root = &fixture.root;
    let cases = [
        "pushd -- -s2 && cat lf7".to_string(),
        "pushd -- -s* && cat lf7".to_string(),
        "pushd -n -- -s2 && cat lf7".to_string(),
    ];
    assert_option_read_rejected(root, &cases);
}

/// The issue problem 2, writes: `pushd -- -s2` then a relative write is refused
/// by the write side (`W=1`) with the write operation, before the read stage.
#[test]
fn pushd_double_dash_rejects_relative_writes() {
    let fixture = fixture();
    let root = &fixture.root;
    let rejection = path_confinement_rejection("pushd -- -s2 && echo x > lf7", root)
        .expect("expected a write rejection");
    assert_eq!(rejection.operation, "output redirection");

    // Without `--`, a `-`-prefixed operand is still a stack rotation, not a
    // directory, so `lf7` is judged at the root alone and stays allowed.
    for command in ["pushd -n +1 && cat lf7", "pushd -s2 && cat lf7"] {
        assert!(
            path_confinement_rejection(command, root).is_none(),
            "expected allow: {command}"
        );
    }
}

/// The issue problem 3: the refusal reason names the destination word (the
/// option value included), not the command word that follows it.
#[test]
fn refusal_reason_names_the_destination_word() {
    let fixture = fixture();
    let root = &fixture.root;
    for (command, word) in [
        ("cd 's2*' && cat lf4", "s2*"),
        ("cd \"s[2]\" && ls", "s[2]"),
        ("tar -C 's2*' -cf - lf4", "s2*"),
        ("make -C {s2,x} lf4", "{s2,x}"),
        ("git -C s? log lf4", "s?"),
    ] {
        let rejection = path_confinement_rejection(command, root)
            .unwrap_or_else(|| panic!("expected rejection: {command}"));
        assert_eq!(rejection.path, word, "command: {command}");
        assert_eq!(rejection.operation, READ_OPERATION, "command: {command}");
        assert!(
            rejection.reason.contains(CWD_REASON),
            "command: {command}: {}",
            rejection.reason
        );
    }
}

/// The issue acceptance: workspace-internal reads and writes through a table
/// option are not falsely refused. `git commit -C HEAD` must stay allowed because
/// `git -C` is a global option only before the subcommand.
#[test]
fn inside_workspace_option_directories_are_allowed() {
    let fixture = fixture();
    let root = &fixture.root;
    for command in [
        "git -C sub status",
        "make -C sub",
        "tar -C sub -cf - f",
        "tar -C sub -xf a.tar",
        "env -C sub cat f",
        "git commit -C HEAD",
        "git -C sub -C deep status",
        "npm --prefix sub test",
        "git -C . status",
    ] {
        assert!(
            path_confinement_rejection(command, root).is_none(),
            "expected allow: {command}"
        );
    }
}

/// A program that is not in the table keeps its existing handling: its option
/// value is not a working-directory candidate, so a `/`-less path reachable only
/// through it is not newly refused (the `find -execdir`-style gap stays out of
/// this issue's scope).
#[test]
fn programs_not_in_the_table_are_unchanged() {
    let fixture = fixture();
    let root = &fixture.root;
    assert!(
        path_confinement_rejection("fltr -C sub lf2", root).is_none(),
        "an unlisted program must keep its existing handling"
    );
}
