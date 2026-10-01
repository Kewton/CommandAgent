#![cfg(unix)]

//! Issue #567: Bash write targets that use glob (`* ? [`) or brace (`{a,b}`,
//! `{a..z}`) syntax must be judged after expanding the word the way the shell
//! would, so that an intermediate symlink or a brace-produced `..` cannot carry
//! the write outside the workspace root.

use std::path::PathBuf;
use std::time::Instant;

use commandagent::mode::ExecutionMode;
use commandagent::tools::bash::path_confinement_rejection;
use commandagent::tools::registry::tool_error_kind;
use commandagent::tools::registry::{ToolContext, ToolRegistry};
use commandagent::tools::workspace_policy::WorkspacePolicy;
use serde_json::json;

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

/// A workspace that contains an escaping symlink (visible and hidden), a
/// symlink loop, and an outside directory holding an existing `secret`.
fn escaping_fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("ws");
    std::fs::create_dir_all(root.join("sub")).unwrap();
    let outside = dir.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("secret"), "outside-secret").unwrap();
    std::os::unix::fs::symlink(&outside, root.join("linked-outside")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join(".hidden-out")).unwrap();
    std::os::unix::fs::symlink(root.join("sub"), root.join("sub/loop")).unwrap();
    // Literal-brace and POSIX-class spellings the shell and globset disagree on.
    std::os::unix::fs::symlink(&outside, root.join("{x}out")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("sub/esc")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("01")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("2")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("3")).unwrap();
    let root = root.canonicalize().unwrap();
    Fixture { _dir: dir, root }
}

/// A workspace whose root contains only inside directories, so `*` matches
/// only real workspace entries.
fn inside_fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("ws");
    std::fs::create_dir_all(root.join("sub")).unwrap();
    std::fs::create_dir_all(root.join("src/[id]")).unwrap();
    std::fs::write(root.join("sub/f"), "x").unwrap();
    std::fs::write(root.join("a.txt"), "x").unwrap();
    std::os::unix::fs::symlink(root.join("sub"), root.join("sub/loop")).unwrap();
    let root = root.canonicalize().unwrap();
    Fixture { _dir: dir, root }
}

#[test]
fn rejects_glob_and_brace_write_targets_that_escape() {
    let fixture = escaping_fixture();
    let root = &fixture.root;
    let root_absolute = root.display();
    let cases = [
        "tee lin*/f".to_string(),
        "tee linked-outsid?/f".to_string(),
        "tee linked-outsid[e]/f".to_string(),
        "tee lin*/secret".to_string(),
        "cp a.txt lin*/".to_string(),
        "chmod 777 lin*/secret".to_string(),
        "tee .hid*/f".to_string(),
        "tee */secret".to_string(),
        "tee **/secret".to_string(),
        format!("tee {root_absolute}/lin*/f"),
        "tee .*/f".to_string(),
        "tee .?/f".to_string(),
        "tee {.,}./f".to_string(),
        "tee {.,.}./f".to_string(),
        "tee {linked-outside,x}/f".to_string(),
        "touch {linked-outside,x}/f".to_string(),
        "tee {k..m}inked-outside/f".to_string(),
        "tee {linked-outside/f,x}".to_string(),
        "printf x > {linked-outside,x}/f".to_string(),
    ];
    for command in cases {
        assert!(
            path_confinement_rejection(&command, root).is_some(),
            "expected rejection: {command}"
        );
    }
}

#[test]
fn blocks_brace_and_absolute_glob_reads_that_escape() {
    let fixture = escaping_fixture();
    let root = &fixture.root;
    let cases = [
        "cat {linked-outside,x}/secret".to_string(),
        format!("cat {}/lin*/secret", root.display()),
        "cat {.,}./secret".to_string(),
    ];
    for command in cases {
        assert!(
            path_confinement_rejection(&command, root).is_some(),
            "expected rejection: {command}"
        );
    }
}

#[test]
fn rejects_targets_over_the_expansion_limits() {
    let fixture = inside_fixture();
    let root = &fixture.root;

    // 2^12 brace expansions exceed the 256-expansion limit.
    let brace_word = "{a,b}".repeat(12);
    assert!(
        path_confinement_rejection(&format!("tee {brace_word}"), root).is_some(),
        "brace expansion over the limit must be rejected"
    );

    // A directory with more than 4096 matching names exceeds the glob limit.
    let crowded = root.join("crowded");
    std::fs::create_dir_all(&crowded).unwrap();
    for index in 0..=4096 {
        std::fs::write(crowded.join(format!("n{index}")), "x").unwrap();
    }
    assert!(
        path_confinement_rejection("tee crowded/*", root).is_some(),
        "glob match count over the limit must be rejected"
    );
}

#[test]
fn glob_expansion_terminates_on_a_symlink_loop() {
    let fixture = inside_fixture();
    assert!(
        path_confinement_rejection("tee sub/loop/loop/*/f", &fixture.root).is_none(),
        "a per-component glob must terminate on a symlink loop"
    );
}

#[test]
fn keeps_normal_workspace_write_targets() {
    let fixture = inside_fixture();
    let root = &fixture.root;
    let cases = [
        "tee out.txt",
        "tee src/new.rs",
        "tee sub/f",
        "tee nomatch*/f",
        "tee */f",
        r#"tee "src/app/[id]/page.tsx""#,
        "mkdir -p 'src/app/[id]'",
        "printf x > 'src/[id]/route.ts'",
        "printf x > \"src/{a,b}.txt\"",
        "printf x > /dev/null",
        // A bracket without `{ } , \`, and braces without a bracket, stay usable.
        "tee src/*.rs",
        r#"cp a.txt "src/[id]/x""#,
        "printf x > 'src/[id]/page.tsx'",
    ];
    for command in cases {
        assert!(
            path_confinement_rejection(command, root).is_none(),
            "expected allow: {command}"
        );
    }
}

#[test]
fn keeps_normal_workspace_reads() {
    let fixture = inside_fixture();
    let root = &fixture.root;
    let cases = [
        r#"cat "src/[id]/route.ts""#.to_string(),
        "cat src/{main,lib}.rs".to_string(),
        "cat *.txt".to_string(),
        "ls src/*.rs".to_string(),
        "rg foo src/**/*.rs".to_string(),
        format!("ls {}/src/*.rs", root.display()),
    ];
    for command in cases {
        assert!(
            path_confinement_rejection(&command, root).is_none(),
            "expected allow: {command}"
        );
    }
}

#[test]
fn escaping_glob_and_brace_targets_are_rejected_before_execution() {
    let fixture = escaping_fixture();
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
    for command in ["tee lin*/secret", "printf x > {linked-outside,x}/f"] {
        let error = registry
            .execute("Bash", &json!({ "command": command }), &context)
            .unwrap_err();
        assert_eq!(
            tool_error_kind(&error),
            "bash_path_confinement_error",
            "{command}"
        );
    }
    assert!(
        !fixture.root.join("linked-outside").join("secret").exists()
            || std::fs::read_to_string(fixture.root.join("linked-outside/secret")).unwrap()
                == "outside-secret",
        "the escaping target must not have been overwritten"
    );
}

#[test]
fn rejects_posix_bracket_expression_targets() {
    let fixture = escaping_fixture();
    let root = &fixture.root;
    for command in [
        "cp a.txt [[:alpha:]]inked-outside/",
        "tee [[:lower:]]*/secret",
        "tee [[=x=]]inked-outside/secret",
        "tee [[.ch.]]inked-outside/secret",
    ] {
        assert!(
            path_confinement_rejection(command, root).is_some(),
            "expected rejection: {command}"
        );
    }
}

#[test]
fn rejects_literal_brace_glob_through_symlink() {
    // `{x}` is a literal in the shell but an alternation in globset, so the
    // guard must escape it to keep matching the same names.
    let fixture = escaping_fixture();
    assert!(
        path_confinement_rejection("tee {x}*/secret", &fixture.root).is_some(),
        "a literal-brace glob must still reach the escaping symlink"
    );
}

#[test]
fn rejects_glob_with_escaping_symlink_component() {
    let fixture = escaping_fixture();
    assert!(
        path_confinement_rejection("tee s*/esc/secret", &fixture.root).is_some(),
        "a glob component that expands to an escaping symlink must be rejected"
    );
}

#[test]
fn rejects_bracket_mixed_with_brace_comma_or_backslash() {
    // Rewriting `{ } , \` into class literals would nest the brackets and let
    // globset read a different set, silently matching nothing and falling back
    // to the literal spelling. These words are refused instead.
    let fixture = escaping_fixture();
    let root = &fixture.root;
    for command in [
        "cp a.txt [,l]inked-outside/",
        "tee [{l]inked-outside/secret",
        "tee [!,]inked-outside/secret",
        "tee [l}]inked-outside/secret",
        "tee [^{]inked-outside/secret",
        "tee [!{]*/secret",
        "tee [k-m,]inked-outside/secret",
        "rm -rf [,l]inked-outside/secret",
        "chmod 777 [,l]inked-outside/secret",
    ] {
        assert!(
            path_confinement_rejection(command, root).is_some(),
            "expected rejection: {command}"
        );
    }
}

#[test]
fn rejects_nested_brace_escape() {
    let fixture = escaping_fixture();
    for command in [
        "tee {{linked-outside,q},y}/secret",
        "tee {x,{y,linked-outside}}/secret",
    ] {
        assert!(
            path_confinement_rejection(command, &fixture.root).is_some(),
            "expected rejection: {command}"
        );
    }
}

#[test]
fn rejects_zero_padded_range_escape() {
    // A range is checked both zero-padded (`01`, bash 4+) and stripped (`1`),
    // including when only one endpoint carries the leading zero.
    let fixture = escaping_fixture();
    for command in [
        "tee {01..02}/secret",
        "tee {01..2}/secret",
        "tee {1..02}/secret",
    ] {
        assert!(
            path_confinement_rejection(command, &fixture.root).is_some(),
            "a zero-padded range must still reach the escaping symlink: {command}"
        );
    }
}

#[test]
fn rejects_numeric_range_escapes() {
    // Each word reaches outside only through the range expansion: `{k..m}` to
    // `linked-outside`, `{1..3}` to the `2` symlink, `{1..5..2}` to the `3`
    // symlink. Were a range expanded to nothing these words would be allowed,
    // so this pins the expansion itself (`{k..m}` is also in the table above).
    let fixture = escaping_fixture();
    let root = &fixture.root;
    for command in [
        "tee {k..m}inked-outside/f",
        "tee {1..3}/secret",
        "tee {1..5..2}/secret",
    ] {
        assert!(
            path_confinement_rejection(command, root).is_some(),
            "a numeric range must expand before the proof: {command}"
        );
    }
}

#[test]
fn allows_exactly_the_expansion_limits() {
    let fixture = inside_fixture();
    let root = &fixture.root;

    let crowded = root.join("crowded");
    std::fs::create_dir_all(&crowded).unwrap();
    for index in 0..4096 {
        std::fs::write(crowded.join(format!("n{index}")), "x").unwrap();
    }
    assert!(
        path_confinement_rejection("tee crowded/*", root).is_none(),
        "exactly the glob match limit must be allowed"
    );
    std::fs::write(crowded.join("overflow"), "x").unwrap();
    assert!(
        path_confinement_rejection("tee crowded/*", root).is_some(),
        "one match over the glob limit must be rejected"
    );

    let brace_word = "{a,b}".repeat(8);
    assert!(
        path_confinement_rejection(&format!("tee {brace_word}"), root).is_none(),
        "exactly the brace expansion limit must be allowed"
    );
}

#[test]
fn over_limit_brace_targets_fail_fast_without_panicking() {
    let fixture = inside_fixture();
    let root = &fixture.root;
    let start = Instant::now();

    for command in [
        "tee {1..100000000}/f".to_string(),
        format!("tee {}", "{a,b}".repeat(30)),
    ] {
        assert!(
            path_confinement_rejection(&command, root).is_some(),
            "expected rejection: {command}"
        );
    }

    // A two-value range at the i64 boundary is decidable quickly and must not
    // panic; an overflow-sized span is rejected, not panicked.
    let _ = path_confinement_rejection("tee {9223372036854775806..9223372036854775807}/f", root);
    assert!(
        path_confinement_rejection("tee {-9223372036854775808..9223372036854775807}/f", root)
            .is_some(),
        "an overflowing range must be rejected"
    );
    assert!(
        start.elapsed().as_secs() < 10,
        "over-limit brace targets must fail fast"
    );
}
