#![cfg(unix)]

//! Issue #635: a `cd`/`pushd` destination that holds a glob or brace
//! metacharacter (`* ? [ ] { }`) added no working-directory candidate and did
//! not mark the walk "cannot be determined", so a following relative path was
//! judged against the workspace root alone and an outward symlink inside the
//! destination escaped. The walk now flags such a destination, and the second
//! stage refuses the `/`-less relative words under that flag too. A destination
//! whose brackets are quoted or escaped (`cd "s[2]"`) is flagged the same way,
//! because the lexical read cannot tell it apart from an executed glob.
//!
//! The write rows are pinned to the write-side operation and the existing
//! "working directory change that cannot be determined" reason, so they are
//! refused by `W` (the write side) and not by a protected-path match `P`.

use std::path::PathBuf;

use commandagent::tools::bash::path_confinement_rejection;

/// The fixed operation a read-side refusal carries.
const READ_OPERATION: &str = "path reference";
/// The existing write-side working-directory reason. The refusal must carry
/// this reason and not a protected-path match.
const CWD_REASON: &str = "follows a working directory change that cannot be determined";

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

/// A tempdir outside the prefix `is_system_prefix_allowed` accepts (`/usr`,
/// `/bin`, `/opt`, `/etc`, `/tmp`), so an escaping symlink target is never
/// admitted as a system path. `s2` is an ordinary directory holding the outward
/// symlink `lf4` and the protected-looking `tests/spec.rs`; `s[2]` is a literal
/// directory name holding the outward symlink `lf5`; `sub/deep` holds the
/// outward symlink `lf3`. The workspace root itself holds no symlink, so a
/// root-level `ls *` stays inside.
fn fixture() -> Fixture {
    let dir = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).expect("create tempdir");
    let root = dir.path().join("ws");
    for directory in ["s2/tests", "s[2]", "sub", "sub/deep", "src"] {
        std::fs::create_dir_all(root.join(directory)).unwrap();
    }
    std::fs::write(root.join("f"), "x").unwrap();
    std::fs::write(root.join("sub/f"), "x").unwrap();
    std::fs::write(root.join("src/f"), "x").unwrap();
    std::fs::write(root.join("s2/tests/spec.rs"), "x").unwrap();
    let outside = dir.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("secret"), "outside-secret").unwrap();
    std::os::unix::fs::symlink(&outside, root.join("s2/lf4")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("s[2]/lf5")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("sub/deep/lf3")).unwrap();
    let root = root.canonicalize().unwrap();
    Fixture { _dir: dir, root }
}

fn assert_glob_destination_read_rejected(root: &std::path::Path, commands: &[String]) {
    for command in commands {
        let rejection = path_confinement_rejection(command, root)
            .unwrap_or_else(|| panic!("expected rejection: {command}"));
        assert_eq!(
            rejection.operation, READ_OPERATION,
            "command: {command} ({})",
            rejection.reason
        );
        assert!(
            rejection.reason.contains(CWD_REASON),
            "command: {command}: {}",
            rejection.reason
        );
    }
}

/// The issue table, reads: every shape whose `cd`/`pushd` destination holds a
/// glob or brace metacharacter must be refused even though only the workspace
/// root is a working-directory candidate.
#[test]
fn glob_cd_destination_rejects_relative_reads() {
    let fixture = fixture();
    let root = &fixture.root;
    let root_absolute = root.display();
    let cases = [
        // Row 1: `*`, `?`, and an executed `[2]`.
        "cd s2* && cat lf4".to_string(),
        "cd s? && cat lf4".to_string(),
        "cd s[2] && cat lf4".to_string(),
        // Row 2: brace lists, with `&&`, `;`, and a trailing space.
        "cd {s2,x} && cat lf4".to_string(),
        "cd {s2,x}; cat lf4".to_string(),
        "cd s{2,} ; cat lf4".to_string(),
        // Row 3: `-P`, `-L`, and `--` before the destination.
        "cd -P s2* && cat lf4".to_string(),
        "cd -L s2* && cat lf4".to_string(),
        "cd -- s2* && cat lf4".to_string(),
        // Row 4: a second `cd` after the glob destination.
        "cd s2* && cd . && cat lf4".to_string(),
        "cd sub && cd de* && cat lf3".to_string(),
        "cd su? && cd deep && cat lf3".to_string(),
        // Row 5: pushd.
        "pushd s2* && cat lf4".to_string(),
        "pushd {s2,x} && cat lf4".to_string(),
        // Row 6: subshell, brace group, and `if`.
        "(cd s2* && cat lf4)".to_string(),
        "{ cd s2*; cat lf4; }".to_string(),
        "if cd s2*; then cat lf4; fi".to_string(),
        // Row 7: control operators and a newline between the two commands.
        "cd s2*\ncat lf4".to_string(),
        "cd s2*; cat lf4".to_string(),
        "cd s2* || cat lf4".to_string(),
        // Row 8: quoted or escaped literal brackets.
        "cd \"s[2]\" && cat lf5".to_string(),
        "cd 's[2]' && cat lf5".to_string(),
        r"cd s\[2\] && cat lf5".to_string(),
        // A prefixed `cd` and an absolute destination keep their handling.
        "env cd s2* && cat lf4".to_string(),
        format!("cd {root_absolute}/s2* && cat lf4"),
    ];
    assert_glob_destination_read_rejected(root, &cases);
}

/// The issue table, writes: a relative write after a glob destination is refused
/// by the write side (`W=1`), pinned to the write operation and the existing
/// working-directory reason.
#[test]
fn glob_cd_destination_rejects_relative_writes() {
    let fixture = fixture();
    let root = &fixture.root;
    let cases = [
        ("cd s2* && echo x > lf4", "output redirection"),
        ("cd s2* && tee lf4", "tee"),
        ("cd s2* && cp f lf4", "cp"),
        ("cd s2* && touch lf4", "touch"),
        ("cd {s2,x} && printf x > lf4", "output redirection"),
        ("cd \"s[2]\" && touch lf5", "touch"),
        ("pushd s2* && tee lf4", "tee"),
    ];
    for (command, operation) in cases {
        let rejection = path_confinement_rejection(command, root)
            .unwrap_or_else(|| panic!("expected write rejection: {command}"));
        assert_eq!(rejection.operation, operation, "command: {command}");
        assert!(
            rejection.reason.contains(CWD_REASON),
            "command: {command}: {}",
            rejection.reason
        );
    }
}

/// The issue table, protected file: a glob `cd` into a directory that holds a
/// protected-looking path, then `rm`/`tee`/`>` that path. The write is refused
/// by the write side with the existing working-directory reason (`W=1`), so it
/// is not a protected-path match and `P` stays 0.
#[test]
fn glob_cd_destination_rejects_protected_file_writes() {
    let fixture = fixture();
    let root = &fixture.root;
    let cases = [
        ("cd s2* && rm tests/spec.rs", "rm"),
        ("cd s2* && tee tests/spec.rs", "tee"),
        ("cd s2* && echo x > tests/spec.rs", "output redirection"),
    ];
    for (command, operation) in cases {
        let rejection = path_confinement_rejection(command, root)
            .unwrap_or_else(|| panic!("expected write rejection: {command}"));
        assert_eq!(rejection.operation, operation, "command: {command}");
        assert!(
            rejection.reason.contains(CWD_REASON),
            "command: {command}: {}",
            rejection.reason
        );
    }
}

/// A glob or brace destination that only one of the two carriage-return
/// readings sees (`cd \rs2*`): the historical reading splits on `\r` and finds
/// no operand, the shell reading keeps `\rs2*` as a glob destination. The
/// command must be refused. The `merge` unit test in `working_directory.rs`
/// pins that the mark itself survives the union; here the R=1 value is fixed.
#[test]
fn glob_cd_destination_mark_survives_the_carriage_return_union() {
    let fixture = fixture();
    let root = &fixture.root;
    for command in [
        // The historical reading splits on `\r` (`cd` has no operand); the shell
        // reading keeps it in the word (`\rs[2]`).
        "cd \r\"s[2]\" && cat lf5",
        // The issue's shape: `\r` then a `*` destination.
        "cd \rs2* && cat lf4",
    ] {
        assert!(
            path_confinement_rejection(command, root).is_some(),
            "expected rejection: {command}"
        );
    }
}

/// The candidate cap must count a jump that passes [`MAX_CANDIDATES`] without
/// landing on it: forty repeated `cd sub` add one candidate each, then `cd r`
/// doubles the set and skips 64. Past the cap the relative write must be refused
/// (`undecidable`), for the `cd` and the `pushd` branch alike; a `>` read as `==`
/// leaves the mark clear and allows the write.
#[test]
fn glob_cd_destination_cap_rejects_relative_writes() {
    let fixture = fixture();
    let root = &fixture.root;
    for command in [
        format!("{}cd r; tee f", "cd sub; ".repeat(40)),
        format!("{}pushd r; tee f", "pushd sub; ".repeat(40)),
    ] {
        let rejection = path_confinement_rejection(&command, root)
            .unwrap_or_else(|| panic!("expected a reject past the cap: {command}"));
        assert!(
            rejection.reason.contains(CWD_REASON),
            "the cap must refuse via the working-directory reason: {}",
            rejection.reason
        );
    }
}

/// Destinations that do not hold a glob or brace metacharacter, and globs used
/// for reading rather than as the `cd` destination, keep their existing
/// handling. A `CDPATH` word without a `cd` must not become newly refused.
#[test]
fn non_glob_cd_and_glob_reads_are_unchanged() {
    let fixture = fixture();
    let root = &fixture.root;
    for command in [
        "cd sub && cat f",
        "cd sub && cargo test",
        "cd sub && echo x > out",
        "cd sub && cd deep && cat f",
        "pushd sub",
        "cd src && ls *",
        "cat s2*/x",
        "env | grep CDPATH",
        "echo $CDPATH",
    ] {
        assert!(
            path_confinement_rejection(command, root).is_none(),
            "expected allow: {command}"
        );
    }
}

/// A glob destination that is only the working directory (no relative read or
/// write after it) is refused as well, because the destination itself cannot be
/// read from its spelling: `cd s2*` alone.
#[test]
fn glob_cd_destination_alone_is_rejected() {
    let fixture = fixture();
    let root = &fixture.root;
    let cases = [
        "cd s2*".to_string(),
        "pushd s2*".to_string(),
        "cd {s2,x}".to_string(),
    ];
    assert_glob_destination_read_rejected(root, &cases);
}
