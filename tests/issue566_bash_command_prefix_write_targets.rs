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
    ];
    for command in cases {
        assert!(
            path_confinement_rejection(command, root).is_none(),
            "expected allow: {command}"
        );
    }
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
