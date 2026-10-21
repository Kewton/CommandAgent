#![cfg(unix)]

//! Issue #575: the Bash write-target guard's lexical analysis does not know
//! ANSI-C quoting (`$'...'`) or locale quoting (`$"..."`). It leaves the `$` as
//! an ordinary character and strips the following quotes as ordinary quotes, so
//! `$'tee'` becomes the word `$tee` and the program, its prefixes, and its
//! options are read wrong. The shell expands those forms before running, so
//! every command that spells `$'` or `$"` outside a shell quote is refused as an
//! unverifiable form. Every test name contains `ansi_c_quoting` so the
//! mutation-test filter selects these tests together with the unit tests in
//! `bash_write_guard/ansi_c_quoting.rs`.

use std::path::PathBuf;

use commandagent::tools::bash::path_confinement_rejection;

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

/// A workspace with an escaping `sub/link` symlink, an inside `a.txt`, and an
/// inside secret file.
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("ws");
    std::fs::create_dir_all(root.join("sub")).unwrap();
    std::fs::write(root.join("a.txt"), "x").unwrap();
    std::fs::write(root.join(".env"), "workspace-secret").unwrap();
    let outside = dir.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("secret"), "outside-secret").unwrap();
    std::os::unix::fs::symlink(&outside, root.join("sub/link")).unwrap();
    let root = root.canonicalize().unwrap();
    Fixture { _dir: dir, root }
}

#[test]
fn ansi_c_quoting_rejects_program_and_option_spellings() {
    let fixture = fixture();
    let root = &fixture.root;
    let cases = [
        // The problem table from the issue body: the program, a prefix, and an
        // option are spelled with ANSI-C / locale quoting.
        "$'tee' /tmp/f",
        "env $'tee' /tmp/f",
        "env $'-S' 'tee /tmp/f'",
        "$'\\x74ee' /tmp/f",
        "$'t\\145e' /tmp/f",
        "$\"tee\" /tmp/f",
        "x$'tee' /tmp/f",
        "cat $'.env'",
        "cat $'sub/link/secret'",
        // The same spellings through an escaping symlink.
        "$'tee' sub/link/f",
        "env $'-C' sub tee link/f",
        "cp $'-t' sub/link a.txt",
        // A mis-read quote that would hide everything after it.
        "echo $'\\'' ; tee /tmp/f",
        // More program, option, and assignment spellings.
        "$'\\cX' /tmp/f",
        "$''tee /tmp/f",
        "$\"\"tee /tmp/f",
        "$'sudo' tee /tmp/f",
        "$'rm' -rf sub/link/x",
        "echo $'\\''\ntee sub/link/f",
        "printf $'a\\'b' > sub/link/f",
        "X=$'tee'",
    ];
    for command in cases {
        assert!(
            path_confinement_rejection(command, root).is_some(),
            "expected rejection: {command}"
        );
    }
}

#[test]
fn ansi_c_quoting_keeps_quoted_escaped_and_plain_forms_allowed() {
    let fixture = fixture();
    let root = &fixture.root;
    let cases = [
        // The `$'` / `$"` introducer is not expanded inside a quote, after a
        // backslash, or as a lone `$`.
        "echo \"$'x'\"",
        "echo '$'",
        "echo \\$'x'",
        "echo \"$\"",
        // Ordinary single-quoted escapes stay usable.
        "printf '%s\\n' x > out.txt",
        "printf 'a\\tb\\n' > out.txt",
        // Everyday verification commands.
        "cargo test",
        "cargo fmt --all -- --check",
        "cargo clippy --all-targets -- -D warnings",
        "npm test",
        "npm run build",
        "python3 -m pytest",
        "timeout 600 cargo test",
    ];
    for command in cases {
        assert!(
            path_confinement_rejection(command, root).is_none(),
            "expected allow: {command}"
        );
    }
}

#[test]
fn ansi_c_quoting_rejection_names_the_ansi_c_operation() {
    let fixture = fixture();
    let rejection =
        path_confinement_rejection("$'tee' /tmp/f", &fixture.root).expect("expected rejection");
    assert_eq!(rejection.operation, "ANSI-C / locale quoting");
}
