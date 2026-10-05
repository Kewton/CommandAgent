#![cfg(unix)]

//! Issue #581: the Bash write-target guard's lexical analysis did not know the
//! backtick command substitution (`` `...` ``). It kept the backquotes as
//! ordinary characters, so the command inside was never inspected and a write
//! (or a read) through it escaped the workspace — for example
//! ``echo `tee sub/link/f` `` or ``echo `tee /tmp/f` `` was allowed. The guard
//! now refuses every executed backtick substitution as an unverifiable form,
//! because the word it expands to cannot be proven to remain in the workspace.
//!
//! Every test name contains `backtick_substitution` so the mutation-test filter
//! selects these tests together with the unit tests in
//! `shell_lexical/backticks.rs`.

use std::path::PathBuf;

use commandagent::tools::allow_policy::bash_verify_command_is_auto_approvable;
use commandagent::tools::bash::path_confinement_rejection;
use commandagent::tools::sensitive_path::command_references_secret;

/// The operation carried by the new rejection. The unit tests in
/// `bash_write_guard.rs` and `backticks.rs` pin the same literal.
const BACKTICK_OPERATION: &str = "unverifiable backtick command substitution";

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

/// A workspace with an escaping `sub/link` symlink, an inside `a.txt`, an inside
/// credential file, and directories a second-stage inspection needs.
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("ws");
    for directory in ["sub", "frontend", "crates/x", "src", "tests"] {
        std::fs::create_dir_all(root.join(directory)).unwrap();
    }
    std::fs::write(root.join("a.txt"), "x").unwrap();
    std::fs::write(root.join(".env"), "workspace-secret").unwrap();
    let outside = dir.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("secret"), "outside-secret").unwrap();
    std::os::unix::fs::symlink(&outside, root.join("sub/link")).unwrap();
    let root = root.canonicalize().unwrap();
    Fixture { _dir: dir, root }
}

/// Every executed backtick substitution must be rejected and must not be
/// auto-approved (`R/A=1`, `V=0`). `M=1` is pinned by the unit tests in
/// `bash_write_guard.rs`, because that predicate is crate-private.
#[test]
fn backtick_substitution_rejects_executed_forms_and_clears_auto_approval() {
    let fixture = fixture();
    let root = &fixture.root;
    let cases = [
        // The issue body's table rows 1 and 2.
        "echo `tee sub/link/f`",
        "echo `tee /tmp/f`",
        // Inside double quotes.
        r#"echo "`tee sub/link/f`""#,
        r#"echo "`tee /tmp/f`""#,
        // Nested / a backtick behind `$(...)`.
        r"echo $(echo `tee /tmp/f`)",
        r#"echo "$(echo '`data`')""#,
        r#"echo "$(echo `tee /tmp/f`)""#,
        // Inside a here-string.
        r#"cat <<< "`tee /tmp/f`""#,
        r"cat <<< `tee sub/link/f`",
        // A read named through the substitution.
        r"cat `cat sub/link/secret`",
        // An executed form on an adjacent line.
        "echo `x`\ntee /tmp/f",
        "tee /tmp/f\necho `x`",
        // An unclosed substitution.
        "echo `tee /tmp/f",
        "cat `cat sub/link/secret",
    ];
    for command in cases {
        assert!(
            path_confinement_rejection(command, root).is_some(),
            "expected rejection: {command:?}"
        );
        assert!(
            !bash_verify_command_is_auto_approvable(command, root),
            "must not be auto-approved: {command:?}"
        );
    }
}

/// A form whose only write is the substitution itself gets the new operation.
#[test]
fn backtick_substitution_records_the_operation() {
    let fixture = fixture();
    let root = &fixture.root;
    let cases = [
        r"echo `tee /tmp/f`",
        r"echo `tee sub/link/f`",
        r#"echo "`tee /tmp/f`""#,
        r"echo $(echo `tee /tmp/f`)",
        r#"echo "$(echo '`data`')""#,
        r"cat <<< `tee sub/link/f`",
        r"cat `cat sub/link/secret`",
    ];
    for command in cases {
        let rejection = path_confinement_rejection(command, root)
            .unwrap_or_else(|| panic!("expected rejection: {command:?}"));
        assert_eq!(
            rejection.operation, BACKTICK_OPERATION,
            "command: {command:?}"
        );
    }
}

/// The rejections that existed before this change must all remain.
#[test]
fn backtick_substitution_rejects_existing_rejections() {
    let fixture = fixture();
    let root = &fixture.root;
    let cases = [
        // Dynamic write targets: the current refusal must stay.
        r"tee `echo /tmp/f`",
        r"printf x > `echo /tmp/f`",
        r"cp a `echo /tmp/f`",
        // An unquoted-delimiter heredoc.
        "cat <<EOF\ntee /tmp/f\nEOF",
        // A protected path named through the substitution.
        r"tee `echo tests/spec.ts`",
    ];
    for command in cases {
        assert!(
            path_confinement_rejection(command, root).is_some(),
            "expected rejection: {command:?}"
        );
    }
}

/// The allowed forms from the issue body must not change.
#[test]
fn backtick_substitution_keeps_allowed_forms() {
    let fixture = fixture();
    let root = &fixture.root;
    let cases = [
        // Single quotes keep the backquotes literal.
        r"echo '`tee /tmp/f`'",
        // A correctly escaped backquote is a literal character.
        r"echo \`tee /tmp/f\`",
        r#"echo "\`tee /tmp/f\`""#,
        "git commit -m 'Fix `x` bug'",
        // Only a comment names the form.
        "echo x # `tee /tmp/f`",
        "# `tee /tmp/f`\ncat a.txt",
        // A quoted-delimiter heredoc body is data.
        "cat <<'EOF'\n`tee /tmp/f`\nEOF",
        // Everyday verification commands.
        "cargo test",
        "npm test",
        "cd sub && npm test",
    ];
    for command in cases {
        assert!(
            path_confinement_rejection(command, root).is_none(),
            "expected allow: {command:?}"
        );
    }
}

/// The verify-auto-approval allow-list must not change (`V=1`).
#[test]
fn backtick_substitution_keeps_verify_auto_approval() {
    let fixture = fixture();
    let root = &fixture.root;
    for command in [
        "cargo test",
        "npm test",
        "cd sub && npm test",
        "git commit -m 'Fix `x` bug'",
    ] {
        assert!(
            bash_verify_command_is_auto_approvable(command, root),
            "must stay auto-approved: {command:?}"
        );
    }
}

/// Secret detection (`S`) is unchanged: the backquote still splits the tokens.
#[test]
fn backtick_substitution_detects_secrets() {
    assert!(
        command_references_secret(r"cat `cat .env`").is_some(),
        "expected secret detection"
    );
}

/// An escaped `\"` / `\\` inside a double quote must not hide the command
/// substitution behind it (`R/A=1`, `V=0`). The first row is the mutation
/// report's example.
#[test]
fn backtick_substitution_rejects_double_quote_escapes() {
    let fixture = fixture();
    let root = &fixture.root;
    let cases = [
        r#"echo "a\"'" $(x)`y`"#,
        r#"echo "a\"$(x)`y`""#,
        r#"echo "a\\$(x)`y`""#,
        r#"echo "a\"'$(x)`y`""#,
        r#"echo "a\"${x}`y`""#,
        r#"echo "a\\${x}`y`""#,
    ];
    for command in cases {
        assert!(
            path_confinement_rejection(command, root).is_some(),
            "expected rejection: {command:?}"
        );
        assert!(
            !bash_verify_command_is_auto_approvable(command, root),
            "must not be auto-approved: {command:?}"
        );
    }
}

/// A `$(`/`${` expansion with no backtick substitution must stay allowed.
#[test]
fn backtick_substitution_keeps_forms_without_a_substitution() {
    let fixture = fixture();
    let root = &fixture.root;
    let cases = [
        "$(echo 'x')",
        "${x}",
        r#"echo "$(echo "a")""#,
        "echo $(x)",
        "echo ${x}",
        r#"echo "$(x)""#,
    ];
    for command in cases {
        assert!(
            path_confinement_rejection(command, root).is_none(),
            "expected allow: {command:?}"
        );
    }
}
