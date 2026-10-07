#![cfg(unix)]

//! Issue #582 (integrating #571): the second-stage Bash path inspection judged
//! only static path words. When any word in a command carried an expansion
//! character (`$`, a backtick, or an unquoted glob/brace), the old word splitter
//! returned `None` and only embedded *absolute* paths were inspected, so an
//! unrelated static relative path next to the expansion (`head sub/link/secret`)
//! escaped the workspace, and unquoted glob, brace, dynamic-rooted, and absolute
//! glob reads were never resolved. The guard now classifies every word, refuses
//! the words whose value cannot be determined statically, and expands the
//! supported glob/brace words through the existing #567 write-target proof.
//!
//! Every test name contains `read_candidates` so the mutation-test filter
//! selects these tests together with the unit tests in `read_guard.rs`.

use std::path::{Path, PathBuf};

use commandagent::mode::ExecutionMode;
use commandagent::tools::allow_policy::bash_verify_command_is_auto_approvable;
use commandagent::tools::bash::path_confinement_rejection;
use commandagent::tools::registry::tool_error_kind;
use commandagent::tools::registry::{ToolContext, ToolRegistry};
use commandagent::tools::sensitive_path::command_references_secret;
use commandagent::tools::workspace_policy::WorkspacePolicy;
use serde::Deserialize;
use serde_json::json;

/// The fixed operation the read-side refusal carries.
const READ_OPERATION: &str = "path reference";

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

/// A workspace that contains escaping symlinks (visible and hidden), a dangling
/// symlink, a symlink loop, a quoted-literal route directory, a package under
/// `node_modules`, and an outside directory holding an existing `secret`. A
/// second `root` is canonicalized so symlink checks compare real paths.
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("ws");
    for directory in [
        "sub",
        "src/app/[id]",
        "node_modules/pkg",
        "frontend",
        "dir1",
        "dir2",
    ] {
        std::fs::create_dir_all(root.join(directory)).unwrap();
    }
    for file in [
        "a.txt",
        "sub/f",
        "src/main.rs",
        "src/app/[id]/route.ts",
        "node_modules/pkg/package.json",
    ] {
        std::fs::write(root.join(file), "x").unwrap();
    }
    let outside = dir.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("secret"), CANARY).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("sub/link")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("sub/esc")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("linked-outside")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join(".hidden-out")).unwrap();
    std::os::unix::fs::symlink(dir.path().join("missing-target"), root.join("dangling")).unwrap();
    std::os::unix::fs::symlink(root.join("sub"), root.join("sub/loop")).unwrap();
    let root = root.canonicalize().unwrap();
    Fixture { _dir: dir, root }
}

/// A fake canary written into the outside file; a rejected command must never
/// read it, so it must not appear in any error, event, or report output.
const CANARY: &str = "OUTER582_FAKE_CANARY_9f3c";

fn assert_rejected(root: &Path, commands: &[&str]) {
    for command in commands {
        assert!(
            path_confinement_rejection(command, root).is_some(),
            "expected rejection: {command:?}"
        );
    }
}

fn assert_allowed(root: &Path, commands: &[&str]) {
    for command in commands {
        assert!(
            path_confinement_rejection(command, root).is_none(),
            "expected allow: {command:?}"
        );
    }
}

/// The #582/#571 table: an unrelated static relative path beside a `$`, a
/// backtick, or a glob is still confined; unquoted glob, brace, dynamic-rooted,
/// and absolute-glob reads are refused. The first three rows are the shell's
/// three spellings of the same outward path (unquoted, quoted, and joined), and
/// a variable PATH prefix is compared with a static one.
#[test]
fn read_candidates_rejects_static_relative_paths_beside_expansions() {
    let fixture = fixture();
    let root = &fixture.root;
    assert_rejected(
        root,
        &[
            // #582's four lines.
            "X=$HOME; head sub/link/secret",
            r#"echo $HOME && head sub/li"nk/secret""#,
            "head 'sub/link/secret'",
            "head sub/link/secret",
            // A static outward path on either side of a dynamic word or glob.
            "echo $HOME && head sub/link/secret",
            "head sub/link/secret && echo $HOME",
            "cat lin*/secret; echo $HOME",
            "echo $HOME; cat lin*/secret",
            // Newlines and separators do not hide it.
            "echo $HOME\nhead sub/link/secret",
            "echo $HOME;head sub/link/secret",
        ],
    );
}

#[test]
fn read_candidates_rejects_unquoted_glob_brace_and_dynamic_reads() {
    let fixture = fixture();
    let root = &fixture.root;
    let outside = fixture._dir.path().join("outside");
    assert_rejected(
        root,
        &[
            // #571 table rows.
            "cat lin*/secret",
            "cat linked-outsid?/secret",
            "cat linked-outside/*",
            "cat .hid*/secret",
            "cat .*/secret",
            "cat {linked-outside,x}/secret",
            "cat {.,}./secret",
            "cat $ROOT/lin*/secret",
            "cat $HOME/secret",
            "cat $FILE",
            // Brace, dynamic, and absolute-glob reads.
            "cat {linked-outside/f,x}",
            "cat {k..m}inked-outside/f",
            "cat [[:alpha:]]inked-outside/secret",
            "cat s*/esc/secret",
            // Only the last of several matches is outward.
            "cat {dir1,linked-outside}/secret",
            "cat {dir1,dir2,linked-outside}/secret",
            // An absolute root glob, and `/usr/..` joined to a fixture-external
            // absolute path and glob.
            &format!("cat {}/lin*/secret", root.display()),
            &format!("cat {}/linked-outside/*", root.display()),
            &format!("cat /usr/../{}/secret", outside.display()),
            &format!("cat /usr/../{}/secre*", outside.display()),
            "cat /usr/../etc/*",
        ],
    );
}

/// The heredoc read form of the #582 table: a `PATH` prefix runs `sh`, `bash`,
/// or `bash -s` with a quoted `OUTER` delimiter holding a `cat` + `INNER`
/// heredoc whose body reads an outward relative path.
#[test]
fn read_candidates_rejects_reads_inside_nested_heredocs() {
    let fixture = fixture();
    let root = &fixture.root;
    let bodies = [
        "cat(){ sh; }\nexport -f cat\nsh <<'OUTER'\ncat <<'INNER'\nhead sub/link/secret\nINNER\nOUTER",
        "cat(){ sh; }\nexport -f cat\nbash <<'OUTER'\ncat <<'INNER'\nhead sub/link/secret\nINNER\nOUTER",
        "cat(){ sh; }\nexport -f cat\nbash -s <<'OUTER'\ncat <<'INNER'\nhead sub/link/secret\nINNER\nOUTER",
        "mkdir -p bin && printf 'sh\\n' > bin/cat && chmod +x bin/cat && PATH=$PWD/bin:$PATH sh <<'OUTER'\ncat <<'INNER'\nhead sub/li\"nk/secret\"\nINNER\nOUTER",
    ];
    assert_rejected(root, &bodies);
}

/// The nine normal forms from the reference section must stay allowed, together
/// with the other inside reads the issue names.
#[test]
fn read_candidates_allows_the_nine_normal_forms() {
    let fixture = fixture();
    let root = &fixture.root;
    assert_allowed(
        root,
        &[
            "cargo test",
            "cd frontend && npm test",
            "echo $HOME && ls src",
            "PATH=$PWD/bin:$PATH cargo test",
            "ls src/*.rs",
            "cat *.txt",
            "rg foo src/**/*.rs",
            "ls node_modules/*/package.json",
            r#"cat "src/app/[id]/route.ts""#,
        ],
    );
}

/// Quoted symbols stay literal; a quoted literal is not expanded even when a
/// same-spelling glob would reach an outward symlink.
#[test]
fn read_candidates_keeps_quoted_literals_unexpanded() {
    let fixture = fixture();
    let root = &fixture.root;
    assert_allowed(
        root,
        &[
            r#"cat "src/app/[id]/route.ts""#,
            "cat 'src/app/[id]/route.ts'",
            r"cat src/app/[id]'/route.ts'",
            "echo '$HOME/secret'",
            "echo \"\\$HOME/secret\"",
            "cat nomatch*/secret",
        ],
    );
}

/// A glob that expands a directory symlink loop must terminate. With an
/// escaping `sub/link` sibling the `*` reaches it and the read is refused; an
/// inside-only tree is allowed, matching the #567 write-side behaviour.
#[test]
fn read_candidates_terminates_a_glob_symlink_loop() {
    let fixture = fixture();
    let root = &fixture.root;
    assert_rejected(root, &["cat sub/loop/loop/*/f"]);

    let inside = tempfile::tempdir().unwrap();
    let inside_root = inside.path().join("ws");
    std::fs::create_dir_all(inside_root.join("sub")).unwrap();
    std::fs::write(inside_root.join("sub/f"), "x").unwrap();
    std::os::unix::fs::symlink(inside_root.join("sub"), inside_root.join("sub/loop")).unwrap();
    let inside_root = inside_root.canonicalize().unwrap();
    assert_allowed(&inside_root, &["cat sub/loop/loop/*/f"]);
}

/// A glob that matches only inside names is allowed; a dangling symlink and an
/// unresolvable component are refused rather than enumerated.
#[test]
fn read_candidates_pins_mismatch_dangling_and_loop() {
    let fixture = fixture();
    let root = &fixture.root;
    assert_allowed(root, &["cat dir1/*/x", "cat nomatch*/secret"]);
    assert_rejected(root, &["cat dangling/*", "cat dangling/secret"]);
}

/// Reads after a `cd` are judged against every working-directory candidate.
#[test]
fn read_candidates_reuses_working_directory_candidates() {
    let fixture = fixture();
    let root = &fixture.root;
    assert_rejected(
        root,
        &[
            "cd sub && cat link/secret",
            "cd sub && cat link/*",
            "cd sub && grep -r x link",
            "cd sub && ls link",
            "cd frontend && cd sub && head link/secret",
            "cd frontend && cat ../sub/f",
        ],
    );
    assert_allowed(root, &["cd sub && cat f"]);
}

/// Exactly the brace (256) and glob (4096) limits are allowed; one over is
/// refused. Over the limit the guard must not check only the leading candidates.
#[test]
fn read_candidates_pins_the_expansion_limits() {
    let fixture = fixture();
    let root = &fixture.root;

    let brace_word = "{a,b}".repeat(8);
    assert_allowed(root, &[&format!("cat {brace_word}/f")]);
    let over = "{a,b}".repeat(12);
    assert_rejected(root, &[&format!("cat {over}/f")]);

    let crowded = root.join("crowded");
    std::fs::create_dir_all(&crowded).unwrap();
    for index in 0..4096 {
        std::fs::write(crowded.join(format!("n{index}")), "x").unwrap();
    }
    assert_allowed(root, &["cat crowded/*"]);
    std::fs::write(crowded.join("overflow"), "x").unwrap();
    assert_rejected(root, &["cat crowded/*"]);
}

/// A shell glob setting the guard does not model makes a glob word unverifiable.
#[test]
fn read_candidates_refuses_glob_under_an_unmodeled_shell_setting() {
    let fixture = fixture();
    let root = &fixture.root;
    assert_rejected(
        root,
        &[
            "shopt -s dotglob; cat *",
            "GLOBIGNORE=.; cat *",
            "set -f; cat *",
        ],
    );
    // The same commands without a glob word keep their existing handling.
    assert_allowed(root, &["shopt -s dotglob; cargo test"]);
}

/// The read rejections are the fixed `path reference` operation and name no
/// file contents. Every one of them is still approvable by the verify
/// allow-list (`V=1`), yet the second-stage refusal (`R`) rejects it before the
/// shell starts — the ordering the issue requires.
#[test]
fn read_candidates_records_a_fixed_operation() {
    let fixture = fixture();
    let root = &fixture.root;
    for command in [
        "cat $FILE",
        "cat $HOME/secret",
        "cat lin*/secret",
        "cat {linked-outside,x}/secret",
        "X=$HOME; head sub/link/secret",
    ] {
        let rejection = path_confinement_rejection(command, root)
            .unwrap_or_else(|| panic!("expected rejection: {command:?}"));
        assert_eq!(rejection.operation, READ_OPERATION, "{command:?}");
        assert!(
            !rejection.reason.contains(CANARY) && !rejection.message.contains(CANARY),
            "the canary leaked into the reason: {command:?}"
        );
        assert!(
            bash_verify_command_is_auto_approvable(command, root),
            "the verify allow-list stays unchanged for {command:?}"
        );
    }
}

/// `V=1` does not erase the read refusal: `${x}` is still auto-approvable yet
/// the second-stage refusal (`R`) rejects it before the shell starts.
#[test]
fn read_candidates_refuses_before_execution_even_when_verify_approves() {
    let fixture = fixture();
    let root = &fixture.root;
    let command = "${x}";
    assert!(path_confinement_rejection(command, root).is_some());
    assert!(
        bash_verify_command_is_auto_approvable(command, root),
        "the verify allow-list is unchanged for {command:?}"
    );
}

/// A rejected read stops before the shell starts: the registry returns
/// `bash_path_confinement_error` and the fake canary never reaches stdout,
/// stderr, or the event log.
#[test]
fn read_candidates_stop_before_the_shell_and_leak_no_canary() {
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
    for command in [
        "cat lin*/secret",
        "cat {linked-outside,x}/secret",
        "X=$HOME; head sub/link/secret",
    ] {
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

#[derive(Deserialize)]
struct DecisionCase {
    id: String,
    command: String,
    #[serde(rename = "R")]
    r: bool,
    #[serde(default)]
    operation: Option<String>,
    #[serde(rename = "S")]
    s: bool,
    #[serde(rename = "V")]
    v: bool,
}

fn corpus_dir() -> PathBuf {
    Path::new("tests/corpus/apps/issue582-bash-read-candidates").to_path_buf()
}

/// The representative table committed under the corpus directory is driven by
/// the public second-stage predicate (`R`), the credential scan (`S`), and the
/// verify allow-list (`V`). The crate-private `A`/`W`/`P` are pinned by the
/// unit test in `read_guard.rs` against the same file.
#[test]
fn read_candidates_decision_corpus_matches_the_predicate() {
    let fixture = fixture();
    let root = &fixture.root;
    let path = corpus_dir().join("fixtures/decision-cases.jsonl");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    let mut seen = 0usize;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let case: DecisionCase = serde_json::from_str(line)
            .unwrap_or_else(|error| panic!("invalid decision case {line:?}: {error}"));
        seen += 1;
        let rejection = path_confinement_rejection(&case.command, root);
        assert_eq!(
            rejection.is_some(),
            case.r,
            "{}: R mismatch for {:?}",
            case.id,
            case.command
        );
        if let Some(operation) = &case.operation {
            let rejection = rejection
                .unwrap_or_else(|| panic!("{}: expected rejection: {:?}", case.id, case.command));
            assert_eq!(&rejection.operation, operation, "{}: operation", case.id);
        }
        assert_eq!(
            command_references_secret(&case.command).is_some(),
            case.s,
            "{}: S mismatch",
            case.id
        );
        assert_eq!(
            bash_verify_command_is_auto_approvable(&case.command, root),
            case.v,
            "{}: V mismatch",
            case.id
        );
    }
    assert!(seen >= 20, "the decision table is too small: {seen}");
}

/// Mutation-killing coverage for the word splitter. Each command is a shape
/// whose read refusal depends on one piece of `path_tokens::read_words`; if
/// that piece is removed or mis-advanced the word is misread and the command
/// would be allowed, so these rows move in the "allow more" direction when the
/// splitter is mutated.
#[test]
fn read_candidates_reject_dynamic_words_inside_double_quotes() {
    let fixture = fixture();
    let root = &fixture.root;
    assert_rejected(
        root,
        &[
            // A plain variable inside double quotes is still a dynamic read.
            r#"cat "$FILE""#,
            r#"cat "$HOME/secret""#,
            // A simple command substitution inside double quotes.
            r#"cat "a$(x)b""#,
            // A parameter operator inside double quotes.
            r#"cat "${x:-y}""#,
        ],
    );
}

/// A backslash inside double quotes (`\$`, `\"`, `\\`, or `\` + newline) must
/// not swallow the outward static path placed after the quoted word.
#[test]
fn read_candidates_reject_outward_path_after_double_quote_backslash() {
    let fixture = fixture();
    let root = &fixture.root;
    assert_rejected(
        root,
        &[
            r#"cat "a\$b" sub/link/secret"#,
            r#"cat "a\"b" sub/link/secret"#,
            "cat \"a\\\nb\" sub/link/secret",
            r#"cat "a\\b" sub/link/secret"#,
        ],
    );
}

/// A backslash outside quotes must not swallow the outward static path that
/// follows the escaped word.
#[test]
fn read_candidates_reject_outward_path_after_unquoted_backslash() {
    let fixture = fixture();
    let root = &fixture.root;
    assert_rejected(
        root,
        &[
            r"cat sub/li\nk/secret",
            r"cat a\b sub/link/secret",
            r"cat sub/link\/secret",
        ],
    );
}

/// A separator with no following space must not drop a character: `;/…`,
/// `;sub/…`, `&&../…`, `||../…`, and an absolute path right after `;` all stay
/// confined.
#[test]
fn read_candidates_reject_outward_path_immediately_after_separator() {
    let fixture = fixture();
    let root = &fixture.root;
    let outside = fixture._dir.path().join("outside");
    assert_rejected(
        root,
        &[
            "true;../secret",
            "true&&../secret",
            "true||../secret",
            "true;sub/link/secret",
            &format!("true;{}/secret", outside.display()),
        ],
    );
}

/// A quoted or escaped glob/brace metacharacter in the same word as an executed
/// one cannot be told apart after the quoting is dropped, so the word is
/// refused rather than expanded with the wrong set (Issue #582 review blocker).
#[test]
fn read_candidates_reject_mixed_quoted_and_executed_expansions() {
    let fixture = fixture();
    let root = &fixture.root;
    assert_rejected(
        root,
        &[
            r"cat {x,y\}z,sub/link/secret}",
            r"cat {x,y'}'z,sub/link/secret}",
            r"cat {a,\{b,sub/link/secret}",
            r"cat sub/li[n\]]k/secret",
            r"cat sub/li[n']']k/secret",
            r#"echo $HOME; cat sub/li[n\]]k/secret"#,
            r"cd sub && cat {a,\{b,link/secret}",
            r"cat '[id]'*",
            r"cat 'q[x]'/*",
        ],
    );
}

/// A simple echo may display the exit status `$?`; other special parameters and
/// arithmetic stay refused (Issue #582 review false rejection).
#[test]
fn read_candidates_allows_simple_echo_exit_status() {
    let fixture = fixture();
    let root = &fixture.root;
    assert_allowed(root, &[r#"echo "EXIT_CODE=$?""#, "echo $?"]);
    assert_rejected(root, &["echo $#", "echo $$", "echo $((1+2))", "cat $?"]);
}

/// A quoted literal bracket is not expanded, so an unrelated sibling symlink
/// whose name matches the bracket class is never read; the literal is still
/// refused when the bracketed name itself resolves outside (Issue #582 design 3).
#[test]
fn read_candidates_does_not_expand_a_quoted_literal_next_to_an_outward_symlink() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("ws");
    std::fs::create_dir_all(root.join("src/app/[id]")).unwrap();
    std::fs::write(root.join("src/app/[id]/route.ts"), "x").unwrap();
    let outside = dir.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("secret"), "x").unwrap();
    // `i` matches the `[id]` character class the quoted literal must not expand.
    std::os::unix::fs::symlink(&outside, root.join("src/app/i")).unwrap();
    let root = root.canonicalize().unwrap();

    assert_allowed(
        &root,
        &[
            r#"cat "src/app/[id]/route.ts""#,
            "cat 'src/app/[id]/route.ts'",
            r"cat src/app/'[id]'/route.ts",
        ],
    );
    // The unquoted spelling expands `[id]` to the outward `i` and is refused.
    assert_rejected(&root, &["cat src/app/[id]/route.ts"]);

    // A bracketed name that is itself the outward symlink is refused.
    let dir2 = tempfile::tempdir().unwrap();
    let root2 = dir2.path().join("ws");
    std::fs::create_dir_all(root2.join("src/app")).unwrap();
    let outside2 = dir2.path().join("outside");
    std::fs::create_dir_all(&outside2).unwrap();
    std::fs::write(outside2.join("route.ts"), "x").unwrap();
    std::os::unix::fs::symlink(&outside2, root2.join("src/app/[id]")).unwrap();
    let root2 = root2.canonicalize().unwrap();
    assert_rejected(&root2, &[r#"cat "src/app/[id]/route.ts""#]);
}

/// The echo/assignment exception allows a dynamic word, but an absolute path
/// embedded in that word must still be confined. `$X=<root>/sub/link/secret`
/// is a simple echo argument (allowed by the exception), and only the embedded
/// absolute-path scan extracts `<root>/sub/link/secret`, which resolves outside
/// through the `sub/link` symlink — dropping that scan would allow it.
#[test]
fn read_candidates_reject_embedded_absolute_path_in_an_allowed_dynamic_word() {
    let fixture = fixture();
    let root = &fixture.root;
    let embedded = format!("{}/sub/link/secret", root.display());
    assert_rejected(
        root,
        &[
            // A simple echo argument showing an assignment-like value.
            &format!("echo $X={embedded}"),
            // The same value inside double quotes.
            &format!("echo \"$X={embedded}\""),
            // A leading simple variable assignment.
            &format!("X=$Y={embedded}"),
        ],
    );
}
