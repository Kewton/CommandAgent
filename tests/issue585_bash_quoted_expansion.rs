#![cfg(unix)]

//! Issue #585: the comment/heredoc elision did not model `$(...)` / `${...}`.
//! Inside a double-quoted string it read a `"` that belongs to the expansion's
//! own quoting as the end of the surrounding double quote, so a following
//! word-start `#` looked like a comment and the write behind it escaped the
//! workspace. The guard now recognizes only the limited simple forms and fails
//! closed on every other double-quoted expansion, keeping the targets and cwd
//! candidates the existing scan obtained.
//!
//! Every test name contains `quoted_expansion` so the mutation-test filter
//! selects these tests together with the unit tests in
//! `shell_lexical/quoted_expansion.rs`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use commandagent::tools::allow_policy::bash_verify_command_is_auto_approvable;
use commandagent::tools::bash::path_confinement_rejection;
use commandagent::tools::sensitive_path::command_references_secret;

/// The operation the fail-closed rejection carries. The unit tests in
/// `bash_write_guard.rs` and `quoted_expansion.rs` pin the same literal.
const UNREADABLE_OPERATION: &str = "unreadable shell text";

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

/// A workspace with an escaping `sub/link` symlink, an inside `a.txt`, an inside
/// credential file, and the directories the cwd candidates need.
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("ws");
    for directory in ["sub", "tests", "src"] {
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

fn corpus_dir() -> PathBuf {
    Path::new("tests/corpus/apps/issue585-bash-quoted-expansion").to_path_buf()
}

/// The double-quoted expansions whose interior cannot be read reject together
/// with the whole command (`R/A=1`, `V=0`). `M=1` is pinned by the crate-private
/// unit tests in `bash_write_guard.rs`.
#[test]
fn quoted_expansion_rejects_ambiguous_double_quoted_expansions() {
    let fixture = fixture();
    let root = &fixture.root;
    let cases = [
        // The issue's hole: the inner quote desyncs the scan and the `#` drops
        // the write. The readable elision is `echo "$(echo " `, so the base
        // commit allowed it.
        r#"echo "$(echo " #x" )" ; tee /tmp/f"#,
        r#"echo "$(echo " #x" )" ; tee tests/spec.rs"#,
        // An inner quote alone is already ambiguous.
        r#"echo "$(echo "a")""#,
        // Parameter operators, arithmetic, subscripts, special parameters.
        r#"echo "${x:-"y"}""#,
        r#"echo "${x#y}""#,
        r#"echo "${#x}""#,
        r#"echo "${x[0]}""#,
        r#"echo "$((1+2))""#,
        // Nesting, a newline, an escape, and a backtick inside a double-quoted
        // expansion.
        r#"echo "$(echo "$(x)")""#,
        "echo \"$(echo\nx)\"",
        r#"echo "$(a\b)""#,
        r#"echo "$(echo `x`)""#,
        // Unterminated.
        r#"echo "$(echo"#,
        // A protected path and a cd-relative protected path.
        r#"echo "$(echo "x")" ; tee tests/spec.rs"#,
        r#"cd tests && echo "$(echo "x")" ; tee spec.rs"#,
        // A backtick after and inside an ambiguous expansion.
        r#"echo "$(echo " #x" )" `y`"#,
    ];
    for command in cases {
        let rejection = path_confinement_rejection(command, root)
            .unwrap_or_else(|| panic!("expected rejection: {command:?}"));
        assert!(
            !bash_verify_command_is_auto_approvable(command, root),
            "must not be auto-approved: {command:?} (operation {})",
            rejection.operation
        );
    }
}

/// A form whose only write is the ambiguous expansion itself gets the
/// fail-closed operation.
#[test]
fn quoted_expansion_records_the_operation() {
    let fixture = fixture();
    let root = &fixture.root;
    for command in [
        r#"echo "$(echo "x")""#,
        r#"echo "${x:-"y"}""#,
        r#"echo "$((1+2))""#,
        r#"echo "$(echo "$(x)")""#,
    ] {
        let rejection = path_confinement_rejection(command, root)
            .unwrap_or_else(|| panic!("expected rejection: {command:?}"));
        assert_eq!(
            rejection.operation, UNREADABLE_OPERATION,
            "command: {command:?}"
        );
    }
}

/// The simple forms, single quotes, true comments, escaped introducers, quoted
/// data heredocs, and unquoted expansions from the issue body must not change.
#[test]
fn quoted_expansion_keeps_simple_unquoted_and_data_forms() {
    let fixture = fixture();
    let root = &fixture.root;
    let cases = [
        // The two maintained simple forms.
        r#"echo "${HOME}""#,
        r#"echo "${key}""#,
        r#"echo "$(git rev-parse --show-toplevel)""#,
        r#"echo "$(cat path/to/file.txt)""#,
        r#"echo "$(x)""#,
        // Unquoted expansions keep their existing handling.
        r#"echo $(echo "x")"#,
        r#"echo $((1+2))"#,
        r#"echo ${x:-y}"#,
        // Single quotes make every byte literal.
        r#"echo '$(echo "x")'"#,
        r#"echo '${x:-"y"}'"#,
        // A true comment is not a command.
        r#"echo x # $(echo "y")"#,
        r#"echo x # '${x:-"y"}'"#,
        r#"echo $(echo ')') #'"#,
        // An escaped introducer is not an expansion.
        r#"echo "\$(echo "x")""#,
        r#"echo "\${x:-"y"}""#,
        // A quoted-delimiter heredoc body is data.
        "cat <<'EOF'\n$(echo \"x\")\nEOF",
        // Everyday verification commands.
        "cargo test",
        "npm test",
        "cd sub && npm test",
        "git commit -m 'Fix `x` bug'",
    ];
    for command in cases {
        assert!(
            path_confinement_rejection(command, root).is_none(),
            "expected allow: {command:?}"
        );
    }
}

/// The credential scan fails closed on an ambiguous expansion (`S=1`).
#[test]
fn quoted_expansion_detects_secrets() {
    for command in [
        r#"echo "$(echo "x")""#,
        r#"echo "$(echo " #x" )" ; cat .env"#,
        r#"cat "$(cat ".env")""#,
        "cat .env",
        "cat `cat .env`",
    ] {
        assert!(
            command_references_secret(command).is_some(),
            "expected secret reference: {command:?}"
        );
    }
    assert!(command_references_secret("cat .env.example").is_none());
}

/// Fake canaries in a nested expansion, a false comment, and the refusal return
/// must leave `R/A/M/S=1`, `V=0`, and must not appear in the fixed diagnostic.
#[test]
fn quoted_expansion_rejects_canaries_without_echoing_them() {
    let fixture = fixture();
    let root = &fixture.root;
    let canaries = [
        "FAKE_585_CANARY",
        "secret585",
        "runtime_bash_policy",
        "CANARY_585",
    ];
    for canary in canaries {
        for template in [
            r#"echo "$(echo "{canary}")""#,
            r#"echo "$(echo " #{canary}" )" ; tee /tmp/f"#,
            r#"echo "$(echo " #{canary}")""#,
            r#"cat "$(cat "{canary}")""#,
        ] {
            let command = template.replace("{canary}", canary);
            let rejection = path_confinement_rejection(&command, root)
                .unwrap_or_else(|| panic!("expected rejection: {command:?}"));
            assert_eq!(
                rejection.operation, UNREADABLE_OPERATION,
                "command: {command:?}"
            );
            assert!(
                !bash_verify_command_is_auto_approvable(&command, root),
                "must not be auto-approved: {command:?}"
            );
            assert!(
                command_references_secret(&command).is_some(),
                "expected secret reference: {command:?}"
            );
            // The new refusal reason is fixed and never interpolates the command.
            assert!(
                !rejection.reason.contains(canary),
                "the reason leaked the canary: {}",
                rejection.reason
            );
        }
    }
    // A one-byte canary cannot be pinned against the fixed English diagnostic
    // (`x` occurs in "text" and "lexical"), so only rejection/V/S are asserted.
    let command = r#"echo "$(echo "x")" ; cat .env"#;
    let rejection = path_confinement_rejection(command, root).expect("rejected");
    assert_eq!(rejection.operation, UNREADABLE_OPERATION);
    assert!(!bash_verify_command_is_auto_approvable(command, root));
    assert!(command_references_secret(command).is_some());

    // The `<redacted>` display placeholder keeps its existing priority.
    let command = r#"echo "$(echo "<redacted>")" ; tee /tmp/f"#;
    let rejection = path_confinement_rejection(command, root).expect("rejected");
    assert_eq!(rejection.operation, "placeholder path");
    assert!(!bash_verify_command_is_auto_approvable(command, root));
}

#[derive(Deserialize)]
struct DecisionCase {
    id: String,
    command: String,
    #[serde(default)]
    executes: bool,
    #[serde(default)]
    reject: bool,
    #[serde(default)]
    operation: Option<String>,
    #[serde(default)]
    secret: bool,
    #[serde(default)]
    auto_approvable: Option<bool>,
}

/// The representative table is committed under the corpus directory and the
/// test drives the real decision on every row.
#[test]
fn quoted_expansion_decision_corpus_matches_the_predicate() {
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
        if case.reject {
            let rejection = rejection
                .unwrap_or_else(|| panic!("{}: expected rejection: {:?}", case.id, case.command));
            if let Some(operation) = &case.operation {
                assert_eq!(
                    &rejection.operation, operation,
                    "{}: operation mismatch",
                    case.id
                );
            }
            if case.executes {
                assert!(
                    !bash_verify_command_is_auto_approvable(&case.command, root),
                    "{}: must not be auto-approved",
                    case.id
                );
            }
        } else {
            assert!(
                rejection.is_none(),
                "{}: expected allow: {:?}",
                case.id,
                case.command
            );
        }
        assert_eq!(
            command_references_secret(&case.command).is_some(),
            case.secret,
            "{}: secret mismatch",
            case.id
        );
        if let Some(expected) = case.auto_approvable {
            assert_eq!(
                bash_verify_command_is_auto_approvable(&case.command, root),
                expected,
                "{}: auto-approval mismatch",
                case.id
            );
        }
    }
    assert!(seen >= 25, "the decision table is too small: {seen}");
}

#[derive(Deserialize)]
struct StructuredCase {
    command: String,
    #[serde(default)]
    executes: bool,
}

/// Six ambiguous cores, 30 tails, and 48 prefixes give the 8,640 saved forms
/// (8,640 unique for this generation; the review's 8,580 differs by its own 60
/// duplicates). Every executed form must be rejected.
#[test]
fn quoted_expansion_structured_corpus_matches_the_predicate() {
    let fixture = fixture();
    let root = &fixture.root;
    let expected = structured_commands();
    assert_eq!(
        expected.len(),
        8_640,
        "the generation must stay at 8,640 forms"
    );

    let path = corpus_dir().join("fixtures/structured-cases.jsonl");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    let mut commands = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let case: StructuredCase = serde_json::from_str(line)
            .unwrap_or_else(|error| panic!("invalid structured case: {error}"));
        assert!(case.executes, "every saved form executes a mark: {line}");
        commands.push(case.command);
    }
    // The fixture is a regenerated snapshot of the same generation.
    assert_eq!(commands, expected, "structured-cases.jsonl is stale");
    let unique: BTreeSet<&String> = commands.iter().collect();
    assert!(unique.len() >= 8_580, "unique forms: {}", unique.len());

    for command in &commands {
        assert!(
            path_confinement_rejection(command, root).is_some(),
            "an executed form must be rejected: {command:?}"
        );
    }
}

/// Regenerates the structured fixture from [`structured_commands`]. Run with
/// `cargo test --test issue585_bash_quoted_expansion -- --ignored`.
#[test]
#[ignore = "regenerates tests/corpus/apps/issue585-bash-quoted-expansion/fixtures/structured-cases.jsonl"]
fn quoted_expansion_regenerate_structured_fixture() {
    let path = corpus_dir().join("fixtures/structured-cases.jsonl");
    let mut out = String::new();
    for command in structured_commands() {
        let line = serde_json::json!({ "command": command, "executes": true });
        out.push_str(&line.to_string());
        out.push('\n');
    }
    std::fs::write(&path, out)
        .unwrap_or_else(|error| panic!("failed to write {}: {error}", path.display()));
}

/// The six ambiguous double-quoted expansions. Each interior contains a quote or
/// another byte the simple-form check refuses.
const CORES: &[&str] = &[
    r#""$(echo "x")""#,
    r#""${x:-"y"}""#,
    r#""$(cat "f")""#,
    r#""$(echo " #c" )""#,
    r#""${x%"y"}""#,
    r#""$(printf "%s" "z")""#,
];

/// The 30 suffixes the review paired with each core.
const TAILS: &[&str] = &[
    "",
    " ; tee /tmp/f",
    " ; tee tests/spec.rs",
    " ; touch out.txt",
    " ; mkdir out",
    " ; cat .env",
    " `x`",
    " $(y)",
    " ${z}",
    " ; cd sub && tee f",
    " > out.txt",
    " >> out.txt",
    " | tee out.txt",
    " && tee out.txt",
    " ; echo done",
    " ; rm out.txt",
    " ; cp a b",
    " ; chmod 600 f",
    " ; printf x",
    " ; true",
    " ; false",
    " ; ls",
    " ; pwd",
    " ; cat f",
    " ; head -n 1 f",
    " ; tail -n 1 f",
    " ; wc -l f",
    " ; grep x f",
    " ; sort f",
    " ; date",
];

/// The 48 leading contexts, including the comment-line apostrophe the issue's
/// reference counterexample uses.
const PREFIXES: &[&str] = &[
    "",
    "echo ",
    "true && ",
    "false || ",
    "cat a.txt && ",
    "cd sub && ",
    "cd tests && ",
    "mkdir -p out && ",
    "( ",
    "! ",
    "env ",
    "command ",
    "nohup ",
    "timeout 5 ",
    "if true; then ",
    "while true; do ",
    "for i in 1; do ",
    "f() { ",
    "{ ",
    "sudo ",
    "nice ",
    "ionice -c 3 ",
    "stdbuf -oL ",
    "time -p ",
    "printf '%s' ",
    "printf x; ",
    "x=1; ",
    "foo=bar ",
    "PATH=$PWD/bin:$PATH ",
    "cat a | ",
    "cat a || ",
    "cat a & ",
    "(cd sub && ",
    "if [ -f a ]; then ",
    "case x in x) ",
    "until false; do ",
    "select x in a; do ",
    "coproc ",
    "doas ",
    "exec ",
    "builtin ",
    "eval ",
    "trap ",
    "hash ",
    "readonly x=1; ",
    "export x=1; ",
    "# '\n",
    "umask 022; ",
];

fn structured_commands() -> Vec<String> {
    let mut commands = Vec::with_capacity(PREFIXES.len() * CORES.len() * TAILS.len());
    for prefix in PREFIXES {
        for core in CORES {
            for tail in TAILS {
                commands.push(format!("{prefix}{core}{tail}"));
            }
        }
    }
    commands
}
