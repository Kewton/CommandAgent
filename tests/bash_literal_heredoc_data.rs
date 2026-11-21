#![cfg(unix)]

//! Issue #924: the lexical Bash guards dropped a data-reader heredoc body only
//! behind a quoted delimiter and re-read every plain `<<` body as shell text,
//! so a JSON fragment such as `"/b"` or an apostrophe in prose was refused as an
//! outside path or an unterminated quote. A lone literal `cat` with one plain
//! `<<` heredoc whose ASCII-identifier delimiter terminates and whose UTF-8 body
//! holds no `$`, backtick, or backslash passes the same bytes as the quoted
//! spelling, so its body now leaves the path-candidate view as well. The raw
//! body credential scan, the real redirect target, and every command after the
//! terminator are still inspected.
//!
//! Every test name contains `literal_heredoc` so the mutation-test filter
//! selects these tests together with the unit tests in `literal_data/tests.rs`.

use std::path::{Path, PathBuf};
use std::process::Command;

use commandagent::mode::ExecutionMode;
use commandagent::tools::allow_policy::bash_verify_command_is_auto_approvable;
use commandagent::tools::bash::path_confinement_rejection;
use commandagent::tools::registry::tool_error_kind;
use commandagent::tools::registry::{ToolContext, ToolRegistry};
use commandagent::tools::sensitive_path::command_references_secret;
use commandagent::tools::workspace_policy::WorkspacePolicy;
use serde::Deserialize;
use serde_json::json;

struct Fixture {
    dir: tempfile::TempDir,
    root: PathBuf,
}

/// A tempdir under the cargo target directory, outside the system prefixes the
/// read guard admits (Issue #604).
fn tempdir() -> tempfile::TempDir {
    let dir = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).expect("create tempdir");
    assert!(
        !["/usr", "/bin", "/opt", "/etc", "/tmp"]
            .iter()
            .any(|prefix| dir.path().starts_with(prefix)),
        "the fixture tempdir must not sit under a system prefix: {}",
        dir.path().display()
    );
    dir
}

/// A workspace with an escaping `sub/link` symlink, an inside `.env`, an inside
/// `in.txt`, a protected `tests/spec.rs`, and an outside directory holding `secret`.
fn fixture() -> Fixture {
    let dir = tempdir();
    let root = dir.path().join("ws");
    for directory in ["sub", "tests"] {
        std::fs::create_dir_all(root.join(directory)).unwrap();
    }
    std::fs::write(root.join(".env"), "workspace-secret").unwrap();
    std::fs::write(root.join("tests/spec.rs"), "// spec\n").unwrap();
    std::fs::write(root.join("in.txt"), "input\n").unwrap();
    let outside = dir.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("secret"), "outside-secret").unwrap();
    std::os::unix::fs::symlink(&outside, root.join("sub/link")).unwrap();
    let root = root.canonicalize().unwrap();
    Fixture { dir, root }
}

fn context(fixture: &Fixture, protected: &[&str]) -> ToolContext {
    ToolContext {
        root: fixture.root.clone(),
        mode: ExecutionMode::Act,
        auto_approve: true,
        interactive_approval: false,
        offline: true,
        workspace_policy: WorkspacePolicy::NormalTask,
        eval_events_path: Some(fixture.dir.path().join("events.jsonl")),
        expected_paths: Vec::new(),
        protected_paths: protected.iter().map(|path| path.to_string()).collect(),
    }
}

fn rejected(root: &Path, command: &str) -> bool {
    path_confinement_rejection(command, root).is_some()
}

#[derive(Deserialize)]
struct Cases {
    schema: String,
    recorded: Vec<RecordedCase>,
    synthetic: Vec<SyntheticCase>,
}

#[derive(Deserialize)]
struct RecordedCase {
    id: String,
    observed_before_fix: Observed,
    command_shape: CommandShape,
    body: RecordedBody,
    synthetic_twin: String,
}

#[derive(Deserialize)]
struct Observed {
    decision: String,
    policy_error_kind: String,
}

#[derive(Deserialize)]
struct CommandShape {
    reader: String,
    reader_arguments: usize,
    heredoc_operator: String,
    delimiter: String,
    delimiter_quoted: bool,
    terminated: bool,
}

#[derive(Deserialize)]
struct RecordedBody {
    text: String,
    utf8: bool,
    contains_dollar: bool,
    contains_backtick: bool,
    contains_backslash: bool,
}

#[derive(Deserialize)]
struct SyntheticCase {
    id: String,
    command: String,
    quoted_twin: Option<String>,
    expect: String,
    secret: bool,
}

fn cases() -> Cases {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/corpus/apps/bash-literal-heredoc-data/fixtures/cases.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
    let cases: Cases = serde_json::from_str(&text).expect("cases.json must parse");
    assert_eq!(cases.schema, "bash-literal-heredoc-data/v1");
    cases
}

/// Every synthetic corpus row drives the real confinement decision and the
/// credential scan; an allowed row equals its quoted twin on both, and on the
/// verify auto-approval gate.
#[test]
fn literal_heredoc_corpus_rows_match_the_real_decision() {
    let fixture = fixture();
    let root = &fixture.root;
    let cases = cases();
    let mut allowed = 0usize;
    let mut refused = 0usize;
    for case in &cases.synthetic {
        let expect_reject = match case.expect.as_str() {
            "reject" => true,
            "allow" => false,
            other => panic!("{}: unknown expectation {other:?}", case.id),
        };
        assert_eq!(
            rejected(root, &case.command),
            expect_reject,
            "{}: confinement decision for {:?}",
            case.id,
            case.command
        );
        assert_eq!(
            command_references_secret(&case.command).is_some(),
            case.secret,
            "{}: credential scan for {:?}",
            case.id,
            case.command
        );
        if let Some(twin) = &case.quoted_twin {
            assert!(
                !expect_reject,
                "{}: a quoted twin pins an allowance",
                case.id
            );
            assert_eq!(
                rejected(root, twin),
                rejected(root, &case.command),
                "{}: the quoted twin must decide the same",
                case.id
            );
            assert_eq!(
                command_references_secret(twin).is_some(),
                command_references_secret(&case.command).is_some(),
                "{}: the quoted twin must scan the same",
                case.id
            );
            assert_eq!(
                bash_verify_command_is_auto_approvable(twin, root),
                bash_verify_command_is_auto_approvable(&case.command, root),
                "{}: the quoted twin must gate auto-approval the same",
                case.id
            );
        }
        if expect_reject {
            refused += 1;
        } else {
            allowed += 1;
        }
    }
    assert!(
        allowed >= 4 && refused >= 20,
        "allowed={allowed} refused={refused}"
    );
}

/// The recorded row keeps only the pre-fix observation; its measured shape is
/// the subset the new predicate admits, and its synthetic twin is allowed now.
#[test]
fn literal_heredoc_recorded_row_is_a_pre_fix_observation_in_the_subset() {
    let fixture = fixture();
    let cases = cases();
    let [recorded] = cases.recorded.as_slice() else {
        panic!("exactly one recorded row");
    };
    assert_eq!(recorded.id, "recorded-l154-plain-cat-json-body");
    assert_eq!(recorded.observed_before_fix.decision, "reject");
    assert_eq!(
        recorded.observed_before_fix.policy_error_kind,
        "bash_path_confinement_error"
    );
    assert_eq!(recorded.body.text, "<omitted>");
    let shape = &recorded.command_shape;
    assert_eq!(shape.reader, "cat");
    assert_eq!(shape.reader_arguments, 0);
    assert_eq!(shape.heredoc_operator, "<<");
    assert!(!shape.delimiter_quoted);
    assert!(shape.terminated);
    assert!(
        shape
            .delimiter
            .bytes()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == b'_')
    );
    let body = &recorded.body;
    assert!(
        body.utf8 && !body.contains_dollar && !body.contains_backtick && !body.contains_backslash
    );
    let twin = cases
        .synthetic
        .iter()
        .find(|case| case.id == recorded.synthetic_twin)
        .expect("the synthetic twin exists");
    assert_eq!(twin.expect, "allow");
    assert!(!rejected(&fixture.root, &twin.command));
}

/// Bodies a data fixture carries that the kept-body reading misread: JSON
/// pointers, URLs, parentheses, quotes, apostrophes, and blanks.
const INERT_BODIES: &[&str] = &[
    "[{\"op\": \"add\", \"path\": \"/b\", \"value\": 2}]\n{\"ref\": \"/a/0\"}\n",
    "see https://example.invalid/a/b?c=d#e (and /domain/key)\n",
    "it's a \"quoted\" word, and 'one' more\n  blanks  \n\n",
];

/// The plain and quoted spellings decide the same for any ASCII-identifier
/// delimiter, and the real shell hands `cat` the same bytes for both.
#[test]
fn literal_heredoc_plain_and_quoted_spellings_are_equivalent() {
    let fixture = fixture();
    let root = &fixture.root;
    let shell_dir = tempdir();
    for delimiter in ["EOF", "Payload_2", "_d"] {
        for body in INERT_BODIES {
            let plain = format!("cat <<{delimiter} > out.json\n{body}{delimiter}");
            let quoted = format!("cat <<'{delimiter}' > out.json\n{body}{delimiter}");
            assert!(!rejected(root, &plain), "plain must be allowed: {plain:?}");
            assert!(
                !rejected(root, &quoted),
                "quoted must be allowed: {quoted:?}"
            );
            assert_eq!(
                command_references_secret(&plain).is_some(),
                command_references_secret(&quoted).is_some()
            );
            // The shell's own reading: both spellings write the body unchanged.
            for (name, command) in [("plain", &plain), ("quoted", &quoted)] {
                let status = Command::new("/bin/sh")
                    .arg("-c")
                    .arg(command)
                    .current_dir(shell_dir.path())
                    .status()
                    .expect("run /bin/sh");
                assert!(status.success(), "{name}: {command:?}");
                let written = std::fs::read(shell_dir.path().join("out.json")).unwrap();
                assert_eq!(written, body.as_bytes(), "{name}: {command:?}");
            }
        }
    }
}

/// A body the predicate cannot prove inert keeps the kept-body reading, so a
/// write it names is still refused; dropping every unquoted body would allow
/// each of these.
#[test]
fn literal_heredoc_unproven_bodies_keep_the_existing_refusal() {
    let fixture = fixture();
    let root = &fixture.root;
    for command in [
        "cat <<EOF\n$X\ntee sub/link/f\nEOF",
        "cat <<EOF\ncost $5\ntee sub/link/f\nEOF",
        "cat <<EOF\n${X}\ntee sub/link/f\nEOF",
        "cat <<EOF\na\\b\ntee sub/link/f\nEOF",
        "cat <<EOF\n$(tee sub/link/f)\nEOF",
        "cat <<EOF\n`tee sub/link/f`\nEOF",
        "cat <<EOF\ndata\ntee sub/link/f",
        "cat <<-EOF\n\ttee sub/link/f\n\tEOF",
        "cat <<E-F\ntee sub/link/f\nE-F",
        "cat <<A <<B\ntee sub/link/f\nA\nx\nB",
        "cat - <<EOF\ntee sub/link/f\nEOF",
        "sh <<'OUTER'\ncat <<INNER\ntee sub/link/f\nINNER\nOUTER",
        "python3 <<EOF\nopen(\"sub/link/f\", \"w\")\nEOF",
        "alias cat=sh\ncat <<EOF\ntee sub/link/f\nEOF",
        "PATH=sub/link cat <<EOF\ntee sub/link/f\nEOF",
        "cat <<EOF | sh\ntee sub/link/f\nEOF",
        "# note\ncat <<EOF\ntee sub/link/f\nEOF",
        "cat <<EOF\nx\nEOF\r\ntee sub/link/f",
    ] {
        assert!(rejected(root, command), "expected rejection: {command:?}");
    }
}

/// The real redirect target and every command after the terminator are still
/// inspected, and the raw body credential scan still reads the dropped body.
#[test]
fn literal_heredoc_keeps_target_suffix_and_secret_checks() {
    let fixture = fixture();
    let root = &fixture.root;
    for command in [
        "cat <<EOF > sub/link/out.json\n{\"p\": \"/domain/key\"}\nEOF",
        "cat > sub/link/out.json <<EOF\n{\"p\": \"/domain/key\"}\nEOF",
        "cat >> ../out.json <<EOF\n{\"p\": \"/domain/key\"}\nEOF",
        "cat <<EOF > a.json\n{\"p\": \"/b\"}\nEOF\ntee sub/link/f < a.json",
        "cat <<EOF > a.json\n{\"p\": \"/b\"}\nEOF\ncat sub/link/secret",
        "cat <<EOF > a.json\n{\"p\": \"/b\"}\nEOF\ncd sub && tee link/f < ../a.json",
    ] {
        assert!(rejected(root, command), "expected rejection: {command:?}");
    }
    for command in [
        "cat <<EOF\ncat .env\nEOF",
        "cat <<EOF > notes.txt\nsee ~/.ssh/id_rsa\nEOF",
        "cat <<EOF > a.json\n{\"p\": \"/b\"}\nEOF\ncat .env",
    ] {
        assert!(
            command_references_secret(command).is_some(),
            "expected secret detection: {command:?}"
        );
    }
    // A guard that passes is not success evidence: the verify auto-approval
    // still refuses a rewritten text, exactly as for the quoted spelling.
    for command in [
        "cat <<EOF > a.json\n{\"p\": \"/b\"}\nEOF",
        "cat <<'EOF' > a.json\n{\"p\": \"/b\"}\nEOF",
    ] {
        assert!(
            !bash_verify_command_is_auto_approvable(command, root),
            "must not be auto-approved: {command:?}"
        );
    }
}

/// A here-string or another stdin redirection beside the plain heredoc keeps
/// today's decision for the plain spelling: the body is re-read, so an
/// apostrophe in it is refused, while the same redirection without the heredoc
/// is allowed (the refusal comes from the kept body, not the redirection).
#[test]
fn literal_heredoc_extra_stdin_redirection_keeps_the_plain_refusal() {
    let fixture = fixture();
    let root = &fixture.root;
    for (line, without_heredoc) in [
        ("cat <<EOF <<<x", "cat <<<x"),
        ("cat <<<x <<EOF", "cat <<<x"),
        ("cat <<EOF < in.txt", "cat < in.txt"),
        ("cat <<EOF 0<in.txt", "cat 0<in.txt"),
        ("cat 0<in.txt <<EOF", "cat 0<in.txt"),
        ("cat <<EOF > out.json < in.txt", "cat > out.json < in.txt"),
    ] {
        assert!(
            !rejected(root, without_heredoc),
            "the redirection alone must be allowed: {without_heredoc:?}"
        );
        let command = format!("{line}\nit's {{\"p\": \"/b\"}}\nEOF");
        assert!(rejected(root, &command), "expected rejection: {command:?}");
        let command = format!("{line}\ntee sub/link/f\nEOF");
        assert!(rejected(root, &command), "expected rejection: {command:?}");
    }
    // The lone heredoc with stdout redirections stays allowed.
    for command in [
        "cat <<EOF > out.json 2> err.log\nit's {\"p\": \"/b\"}\nEOF",
        "cat >> out.json <<EOF 2>> err.log\nit's {\"p\": \"/b\"}\nEOF",
    ] {
        assert!(!rejected(root, command), "expected allow: {command:?}");
    }
}

/// Through the real Bash tool: the allowed plain spelling writes the body inside
/// the workspace and a harmless consumer reads the same bytes in the same call,
/// with no provider involved. Refused forms never reach the shell.
#[test]
fn literal_heredoc_real_bash_tool_writes_and_refuses() {
    let fixture = fixture();
    let context = context(&fixture, &["tests/spec.rs"]);
    let registry = ToolRegistry::default();
    let body = "[{\"k\": \"/a b\", \"v\": \"it's\"}]\n{\"ref\": \"/b\"}\n";
    let command = format!("cat <<EOF > input.jsonl\n{body}EOF\ncat input.jsonl");
    let output = registry
        .execute("Bash", &json!({ "command": command }), &context)
        .unwrap_or_else(|err| panic!("plain cat heredoc must run: {err}"));
    assert_eq!(
        std::fs::read(fixture.root.join("input.jsonl")).unwrap(),
        body.as_bytes()
    );
    assert!(output.contains("{\"ref\": \"/b\"}"), "{output}");

    let outside = fixture.dir.path().join("outside");
    for (command, kind) in [
        (
            format!("cat <<EOF > sub/link/out.json\n{body}EOF"),
            "bash_path_confinement_error",
        ),
        (
            format!("cat <<EOF > a.json\n{body}EOF\ntee sub/link/f < a.json"),
            "bash_path_confinement_error",
        ),
        (
            format!("cat <<EOF\n$X\n{body}tee sub/link/f\nEOF"),
            "bash_path_confinement_error",
        ),
        (
            format!("cat <<EOF > tests/spec.rs\n{body}EOF"),
            "protected_path_mutation_rejected",
        ),
    ] {
        let error = registry
            .execute("Bash", &json!({ "command": command }), &context)
            .unwrap_err();
        assert_eq!(tool_error_kind(&error), kind, "{command:?}");
    }
    assert!(!outside.join("out.json").exists());
    assert!(!outside.join("f").exists());
    assert!(!fixture.root.join("a.json").exists());
    assert_eq!(
        std::fs::read_to_string(fixture.root.join("tests/spec.rs")).unwrap(),
        "// spec\n"
    );
}
