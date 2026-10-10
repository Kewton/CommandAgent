#![cfg(unix)]

//! Issue #645: the working-directory option check (#637) missed four spellings,
//! so a relative `/`-less read after them was judged against the workspace root
//! alone and an outward symlink inside the option's directory escaped (`R=0`).
//! This file drives the public second-stage predicate `R`
//! (`path_confinement_rejection`) against a tempdir fixture:
//!
//! 1. a long-option abbreviation (`tar --dir=sub`, `env --ch sub`), which GNU
//!    and bsdtar accept but the exact-name match missed;
//! 2. a cluster whose working-directory character is eaten as another option's
//!    value (`tar -cfC -C sub`), which the cluster scan skipped;
//! 3. a `git` value-taking global option before `-C` (`git -c k=v -C sub`),
//!    whose value the scan read as the subcommand and stopped;
//! 4. `npm -C`, the short spelling of `--prefix`, which the table lacked.
//!
//! ## False-rejection estimate (from the Issue body)
//!
//! Of the 436 distinct agent Bash commands recorded under `dev-reports/` and
//! `workspace/management/`, the H-17 prototype changed none. Of the ~59k
//! distinct Claude Code Bash commands in this repository it changed none: the
//! long-name prefix form appeared 0 times, a `git` command starting with a
//! value-taking global option 151 times, and `npm -C` 0 times. The change only
//! *adds* working-directory candidates (it never drops one), so a command is
//! newly refused only when the added destination holds an outward symlink or
//! when the value cannot be read from its spelling (a glob, `$`, `..`, empty).
//!
//! That monotonicity is also why no acceptance mark can loosen: this walk only
//! adds candidates, so `R`/`A`/`W`/`P`/`S` can only go `0 -> 1` and `V` can only
//! stay or tighten. The named issues' own test files plus
//! `cargo test --all-targets` pin that direction; this file pins the four new
//! shapes and the unchanged values of `meson -C`, `unzip -d`, and
//! `git --git-dir=x`.

use std::path::{Path, PathBuf};

use commandagent::tools::bash::path_confinement_rejection;

/// The fixed operation a read-side refusal carries.
const READ_OPERATION: &str = "path reference";

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

/// A tempdir outside the prefix `is_system_prefix_allowed` accepts (`/usr`,
/// `/bin`, `/opt`, `/etc`, `/tmp`), so an escaping symlink target is never
/// admitted as a system path. `sub` holds the outward symlink `lf2` and the
/// regular file `sub/f`; the workspace root holds the regular files `f` and
/// `a.tar`.
fn fixture() -> Fixture {
    let dir = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).expect("create tempdir");
    assert!(
        !["/usr", "/bin", "/opt", "/etc", "/tmp"]
            .iter()
            .any(|prefix| dir.path().starts_with(prefix)),
        "the fixture tempdir must not sit under a system prefix: {}",
        dir.path().display()
    );
    let root = dir.path().join("ws");
    std::fs::create_dir_all(root.join("sub")).unwrap();
    std::fs::write(root.join("f"), "x").unwrap();
    std::fs::write(root.join("a.tar"), "x").unwrap();
    std::fs::write(root.join("sub/f"), "x").unwrap();
    let outside = dir.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("secret"), "outside-secret").unwrap();
    std::os::unix::fs::symlink(&outside, root.join("sub/lf2")).unwrap();
    let root = root.canonicalize().unwrap();
    Fixture { _dir: dir, root }
}

/// A workspace whose every `escape` directory (a nested path such as
/// `--git-dir/sub`) holds the outward symlink `lf2`, plus a root file `f`. Used
/// where the option value itself is a directory name that looks like an option.
fn fixture_with_escape_dirs(escape: &[&str]) -> Fixture {
    let dir = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).expect("create tempdir");
    assert!(
        !["/usr", "/bin", "/opt", "/etc", "/tmp"]
            .iter()
            .any(|prefix| dir.path().starts_with(prefix)),
        "the fixture tempdir must not sit under a system prefix: {}",
        dir.path().display()
    );
    let root = dir.path().join("ws");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("f"), "x").unwrap();
    let outside = dir.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("secret"), "outside-secret").unwrap();
    for directory in escape {
        std::fs::create_dir_all(root.join(directory)).unwrap();
        std::os::unix::fs::symlink(&outside, root.join(directory).join("lf2")).unwrap();
    }
    let root = root.canonicalize().unwrap();
    Fixture { _dir: dir, root }
}

/// Asserts every command is refused by a read-side `path reference`, so the
/// relative read after the option escapes the workspace (`R=1`).
fn assert_read_rejected(root: &Path, commands: &[&str]) {
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

/// Table 1 rows 1 and 4: a long option that is a prefix of a table name
/// (`tar --dir`, `just --working-dir`, `npm --pref`) and `npm -C` now name
/// `sub`, so `lf2` under it escapes.
#[test]
fn abbreviated_and_short_npm_options_reject_relative_reads() {
    let fixture = fixture();
    let root = &fixture.root;
    assert_read_rejected(
        root,
        &[
            "tar --dir=sub -cf - lf2",
            "tar --direc sub -cf - lf2",
            "tar --dir sub -cf - lf2",
            "make --dir=sub lf2",
            "env --ch=sub cat lf2",
            "env --chd sub cat lf2",
            "sudo --chd=sub cat lf2",
            "patch --dir=sub -i lf2",
            "just --working-dir sub cat lf2",
            "pnpm --di sub cat lf2",
            "npm --pref sub x lf2",
            "npm -C sub run x lf2",
            "npm -C sub exec cat lf2",
        ],
    );
}

/// Table 1 row 2: a cluster whose working-directory character is read as another
/// option's value (`tar -cfC -C sub`). The `-`-prefixed value is recorded and
/// scanned again, so the real `-C sub` is found.
#[test]
fn clustered_values_that_hide_the_option_reject_relative_reads() {
    let fixture = fixture();
    let root = &fixture.root;
    assert_read_rejected(
        root,
        &[
            "tar -cfC -C sub lf2",
            "tar -cfC --directory=sub lf2",
            "tar -cfC -Csub lf2",
            "tar -fC -Csub lf2",
            "make -fC -C sub lf2",
            "sudo -uD -D sub cat lf2",
        ],
    );
}

/// Table 1 row 3: a `git` value-taking global option before `-C` consumes its
/// value word, so the scan no longer mistakes it for the subcommand and stops.
#[test]
fn git_value_taking_global_options_reject_relative_reads() {
    let fixture = fixture();
    let root = &fixture.root;
    assert_read_rejected(
        root,
        &[
            "git -c k=v -C sub log lf2",
            "git -pc k=v -C sub log lf2",
            "git --git-dir x -C sub log lf2",
            "git --work-tree sub -C sub log lf2",
            "git --namespace n -C sub log lf2",
        ],
    );
}

/// A `-C` value that is itself spelled like a value-taking global option
/// (`--git-dir`, `--namespace`, `-pc`, `-c`) must still be read as one literal
/// path word: `git -C --git-dir -C sub log lf2` chdirs to `--git-dir`, then to
/// `sub`, then reads `lf2` under `sub`. `git -C` never rescans its value, so the
/// following `-C sub` is not swallowed. The pre-fix code misread the value as a
/// global option and skipped `-C sub`, letting `sub/lf2` escape (`R=0`).
#[test]
fn git_c_value_named_like_a_global_option_still_reads_the_next_c() {
    let fixture = fixture_with_escape_dirs(&["--git-dir/sub", "--namespace/sub", "-pc/sub"]);
    let root = &fixture.root;
    assert_read_rejected(
        root,
        &[
            "git -C --git-dir -C sub log lf2",
            "git -C --namespace -C sub log lf2",
            "git -C -pc -C sub log lf2",
        ],
    );

    // The fourth form uses its own fixture that places `-c/sub/lf2` only and
    // never `-c/lf2`, so the pre-fix code (which stops at `-c`) cannot reject it.
    let fixture = fixture_with_escape_dirs(&["-c/sub"]);
    let root = &fixture.root;
    assert_read_rejected(root, &["git -C -c -C sub log lf2"]);
}

/// Table 2: the workspace-internal reads and writes through these options are
/// not falsely refused; their value is unchanged. `git commit -C HEAD` stays
/// allowed because `git -C` is global only before the subcommand.
#[test]
fn inside_workspace_option_variants_stay_allowed() {
    let fixture = fixture();
    let root = &fixture.root;
    for command in [
        "git -C sub status",
        "make -C sub",
        "tar -C sub -cf - f",
        "grep -C 3 x f",
        "git log -C sub",
        "git -c color.ui=never status",
        "git -c k=v -C sub status",
        "git --no-pager log -C",
        "git commit -C HEAD",
        "tar --exclude=x -cf - f",
        "make --no-print-directory -C sub",
        "make -f Makefile test",
        "cargo test --no-fail-fast",
        "cargo --manifest-path x/Cargo.toml test",
        "npm --prefix sub run build",
        "sudo -u root -D sub cat f",
        "env --ignore-environment cat f",
        "tar -xzf a.tar -C sub",
    ] {
        assert!(
            path_confinement_rejection(command, root).is_none(),
            "expected allow: {command}"
        );
    }
}

/// Table 3, unchanged-and-allowed: a program the design deliberately does not
/// add (`meson`, `gradle`, `tmux`, `docker`, `unzip`) names no candidate, and a
/// `git` object name after `-C sub` is not a file path, so `R` stays `0`.
#[test]
fn unlisted_programs_and_git_objects_keep_their_value() {
    let fixture = fixture();
    let root = &fixture.root;
    for command in [
        "git -c core.pager=cat -C sub show HEAD:lf2",
        "meson -C sub lf2",
        "gradle -p sub lf2",
        "tmux -c sub cat lf2",
        "docker -w sub cat lf2",
        "unzip -d sub a.zip lf2",
    ] {
        assert!(
            path_confinement_rejection(command, root).is_none(),
            "expected allow: {command}"
        );
    }
}

/// Table 3, unchanged-and-refused: the `=`-attached global option and the
/// value-less `--no-pager`/`-P` keep the `-C` after them read, so `R` stays `1`.
#[test]
fn already_refused_git_shapes_stay_refused() {
    let fixture = fixture();
    let root = &fixture.root;
    assert_read_rejected(
        root,
        &[
            "git --no-pager -C sub log lf2",
            "git -P -C sub log lf2",
            "git --git-dir=x -C sub log lf2",
        ],
    );
}
