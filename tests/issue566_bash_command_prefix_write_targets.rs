#![cfg(unix)]

//! Issue #566: the Bash write-target guard must decide the program after
//! peeling command prefixes (`env`, `sudo`, `timeout`, `nice`, reserved words,
//! ...) rather than from the first word of the segment, so a write launched
//! through a prefix is judged against the workspace boundary. Every test name
//! contains `command_prefix` so the mutation-test filter selects these tests
//! together with the unit tests in `bash_write_guard/command_prefix.rs`.

use std::path::{Path, PathBuf};

use commandagent::tools::bash::path_confinement_rejection;

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

/// A workspace with an escaping `sub/link` symlink and an inside `a.txt`.
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("ws");
    std::fs::create_dir_all(root.join("sub")).unwrap();
    std::fs::write(root.join("a.txt"), "x").unwrap();
    let outside = dir.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("secret"), "outside-secret").unwrap();
    std::os::unix::fs::symlink(&outside, root.join("sub/link")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("linked-outside")).unwrap();
    let root = root.canonicalize().unwrap();
    Fixture { _dir: dir, root }
}

#[test]
fn command_prefix_rejects_writes_through_every_prefix() {
    let fixture = fixture();
    let root = &fixture.root;
    let cases = [
        // The problem table from the issue body.
        "env tee /tmp/f",
        "env FOO=1 cp a.txt /tmp/f",
        "sudo cp a.txt /tmp/f",
        "command cp a.txt /tmp/f",
        "nohup tee /tmp/f",
        "timeout 5 cp a.txt /tmp/f",
        "{ tee /tmp/f; }",
        "! tee /tmp/f",
        "if true; then tee /tmp/f; fi",
        "while tee /tmp/f; do :; done",
        // env options.
        "env -i tee /tmp/f",
        "env -u HOME tee /tmp/f",
        "env -u HOME FOO=1 tee /tmp/f",
        "env -- tee /tmp/f",
        "/usr/bin/env tee /tmp/f",
        "env -S \"tee /tmp/f\"",
        // command / exec.
        "command -p tee /tmp/f",
        "exec tee /tmp/f",
        "exec -a x tee /tmp/f",
        // sudo / doas.
        "sudo -u root tee /tmp/f",
        "sudo -E tee /tmp/f",
        "sudo -- tee /tmp/f",
        "sudo -s tee /tmp/f",
        "sudo -e /tmp/f",
        "doas -u root tee /tmp/f",
        // timeout / nice / ionice / stdbuf / time.
        "timeout -s KILL 5 tee /tmp/f",
        "timeout -k 1 5 tee /tmp/f",
        "timeout --preserve-status 5s tee /tmp/f",
        "nice -n 5 tee /tmp/f",
        "nice -5 tee /tmp/f",
        "ionice -c 3 tee /tmp/f",
        "stdbuf -oL tee /tmp/f",
        "time -p tee /tmp/f",
        "/usr/bin/time -o /tmp/f true",
        // reserved words / grouping.
        "for x in a; do tee /tmp/f; done",
        "true && { tee /tmp/f; }",
        "f() { tee /tmp/f; }",
        "coproc tee /tmp/f",
        // nested.
        "env sudo timeout 5 tee /tmp/f",
        // undecidable option.
        "sudo -X u tee /tmp/f",
        // Review round-1 holes: cwd-changing and end-of-options prefixes.
        "env -C sub tee link/f",
        "env --chdir=sub tee link/f",
        "sudo -D sub tee link/f",
        "env -i -- tee /tmp/f",
        "exec -- tee /tmp/f",
        "exec -X tee /tmp/f",
        // B1: the env -S string operand is scanned.
        "env -S'tee /tmp/f'",
        "env -iS'tee /tmp/f'",
        "env --split-string='tee /tmp/f'",
        "env -S\"cp a.txt /tmp/f\"",
        // B2/B3: nohup and builtin skip a leading `--`.
        "nohup -- tee /tmp/f",
        "builtin -- cd linked-outside && tee f",
        // B4: sudo -k still runs the command.
        "sudo -k tee /tmp/f",
        "sudo -k -u root cp a.txt /tmp/f",
        "sudo -n -k tee /tmp/f",
        // sudoedit is a write program for the unresolved scan.
        "env -C sub sudoedit link/f",
        "sudo -X u sudoedit /tmp/f",
        // Unknown options on the value-taking prefixes stay fail-closed.
        "command -X tee /tmp/f",
        "nice -X tee /tmp/f",
        "stdbuf -X tee /tmp/f",
        "/usr/bin/time -X tee /tmp/f",
        "sudo -i tee /tmp/f",
        "sudo -R sub tee /tmp/f",
        "timeout -f 5 tee /tmp/f",
        // R2: a quoted, backslashed, or expanded -S value is unverifiable.
        "env -S'\"tee\" /tmp/f'",
        "env -S\"'tee' /tmp/f\"",
        "env -S'te\"\"e /tmp/f'",
        "env -S'tee\\_/tmp/f'",
        "env -S'cp\\_a.txt\\_/tmp/f'",
        "X=tee env -S'${X} /tmp/f'",
        "env -S'\"cd\" linked-outside'",
        // R1: the env -S value is extracted in the unresolved scan too.
        "env -uX -S'tee /tmp/f'",
        "env -iuX -S'cp a.txt /tmp/f'",
        "env -C sub -S'tee /tmp/f'",
        "env --chdir=sub -S'tee link/f'",
        "env --unknown -S'tee /tmp/f'",
        // The same writes through the escaping `sub/link` symlink.
        "env tee sub/link/f",
        "timeout 5 cp a.txt sub/link/",
        "{ tee sub/link/f; }",
    ];
    for command in cases {
        assert!(
            path_confinement_rejection(command, root).is_some(),
            "expected rejection: {command}"
        );
    }
}

#[test]
fn command_prefix_rejection_names_the_resolved_write_operation() {
    let fixture = fixture();
    let root = &fixture.root;
    for (command, operation) in [
        ("env tee /tmp/f", "tee"),
        ("timeout 5 cp a.txt /tmp/f", "cp"),
        ("sudo -u root tee /tmp/f", "tee"),
        ("command cp a.txt /tmp/f", "cp"),
        ("timeout 5 cp a.txt sub/link/", "cp"),
    ] {
        let rejection = path_confinement_rejection(command, root)
            .unwrap_or_else(|| panic!("expected rejection: {command}"));
        assert_eq!(rejection.operation, operation, "command: {command}");
    }
}

#[test]
fn command_prefix_keeps_inside_and_verification_writes_allowed() {
    let fixture = fixture();
    let root = &fixture.root;
    let cases = [
        "env FOO=1 tee out.txt",
        "printf x | env tee out.txt",
        "timeout 5 cp a.txt b.txt",
        "env tee /dev/null",
        "env FOO=1 cargo test",
        "env RUST_LOG=debug cargo test",
        "timeout 600 cargo test",
        "timeout --foreground 600 cargo test",
        "timeout --unknown 600 cargo test",
        "nice cargo build",
        "nohup cargo build",
        "time cargo test",
        "{ cargo test; }",
        "command -v cargo",
        "command -v tee /tmp/f",
        "sudo -l tee /tmp/f",
        // `command -` runs nothing in sh, so `-` is not a program.
        "command - tee /tmp/f",
        // B1: an env -S string with no write program stays allowed.
        "env -S'cargo test'",
        "env -S",
        "env --split-string",
        // B4: `sudo -k` without a command, and the terminal timestamp options.
        "sudo -k",
        "sudo -K",
        "sudo -v tee /tmp/f",
        // A program reached after an option value must still be seen; the
        // write is inside the workspace, so confinement stays allowed.
        "env -S x tee out.txt",
        "env --split-string x tee out.txt",
        "env --split-string=x tee out.txt",
        "env -i -- tee out.txt",
        "env -u V tee out.txt",
        "env - tee out.txt",
        "command -p tee out.txt",
        "command - tee out.txt",
        "command -v tee out.txt",
        "exec -a n tee out.txt",
        "exec -c tee out.txt",
        "exec -l tee out.txt",
        "exec -- tee out.txt",
        "exec - tee out.txt",
        "sudo -u root tee out.txt",
        "sudo -g g tee out.txt",
        "sudo -n tee out.txt",
        "sudo -- tee out.txt",
        "timeout 5 tee out.txt",
        "timeout -s TERM 5 tee out.txt",
        "timeout -k 1 5 tee out.txt",
        "timeout --signal=TERM 5 tee out.txt",
        "timeout - tee out.txt",
        "nice -n 5 tee out.txt",
        "nice -5 tee out.txt",
        "nice --adjustment=5 tee out.txt",
        "nice -n -5 tee out.txt",
        "nice -n 5 -- tee out.txt",
        "nice - tee out.txt",
        "stdbuf -o L tee out.txt",
        "stdbuf -oL tee out.txt",
        "stdbuf -i0 -o0 tee out.txt",
        "stdbuf --output=L tee out.txt",
        "stdbuf - tee out.txt",
        "time -p tee out.txt",
        "/usr/bin/time -f F tee out.txt",
        "/usr/bin/time -o log tee out.txt",
        "/usr/bin/time -v tee out.txt",
        "/usr/bin/time -a -o log tee out.txt",
        "/usr/bin/time --output=log tee out.txt",
        "/usr/bin/time -fF tee out.txt",
        "/usr/bin/time -olog tee out.txt",
        "/usr/bin/time - tee out.txt",
    ];
    for command in cases {
        assert!(
            path_confinement_rejection(command, root).is_none(),
            "expected allow: {command}"
        );
    }
}

#[test]
fn command_prefix_depth_limit_boundary() {
    let fixture = fixture();
    let root = &fixture.root;

    let past_limit_write = format!("{}tee /tmp/f", "env ".repeat(17));
    assert!(
        path_confinement_rejection(&past_limit_write, root).is_some(),
        "17 nested prefixes before a write must still reject"
    );

    let past_limit_cargo = format!("{}cargo test", "env ".repeat(17));
    assert!(
        path_confinement_rejection(&past_limit_cargo, root).is_none(),
        "17 nested prefixes before a verification command must stay allowed"
    );

    let within_limit_write = format!("{}tee out.txt", "env ".repeat(16));
    assert!(
        path_confinement_rejection(&within_limit_write, root).is_none(),
        "16 nested prefixes still peel to the write program"
    );
}

#[test]
fn command_prefix_rejection_reports_the_workspace_relative_retry() {
    let fixture = fixture();
    let root = &fixture.root;
    let rejection = path_confinement_rejection("env tee /tmp/f", root).expect("rejected");
    assert!(
        rejection.operation == "tee" || !rejection.nearest_relative.is_empty(),
        "a rejection must keep the existing report shape"
    );
    assert!(Path::new(&rejection.root).is_absolute());
}
