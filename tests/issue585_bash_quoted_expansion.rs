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
use std::fs::File;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

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

fn structured_cases() -> Vec<StructuredCase> {
    let path = corpus_dir().join("fixtures/structured-cases.jsonl");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line)
                .unwrap_or_else(|error| panic!("invalid structured case {line:?}: {error}"))
        })
        .collect()
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

/// The intended false rejections from the issue: a `git commit -m` message
/// passed through a quoted-delimiter heredoc inside a command substitution, and
/// `${x:-safe}`. Both must be refused (`R/A=1`, `V=0`).
#[test]
fn quoted_expansion_rejects_intended_false_rejections() {
    let fixture = fixture();
    let root = &fixture.root;
    for command in [
        "git commit -m \"$(cat <<'EOF'\nFix it's bug\nEOF\n)\"",
        r#"echo "${x:-safe}""#,
    ] {
        let rejection = path_confinement_rejection(command, root)
            .unwrap_or_else(|| panic!("expected rejection: {command:?}"));
        assert_eq!(
            rejection.operation, UNREADABLE_OPERATION,
            "command: {command:?}"
        );
        assert!(
            !bash_verify_command_is_auto_approvable(command, root),
            "must not be auto-approved: {command:?}"
        );
    }
}

/// A heredoc body read as commands (`sh <<'EOF'`) must carry the issue form's
/// ambiguity out, while the same body as a quoted-delimiter data reader
/// (`cat <<'EOF'`) stays dropped and allowed.
#[test]
fn quoted_expansion_distinguishes_sh_and_cat_heredoc_bodies() {
    let fixture = fixture();
    let root = &fixture.root;
    let body = r#"echo "$(echo " #")" ; tee sub/link/f"#;
    let sh = format!("sh <<'EOF'\n{body}\nEOF");
    let rejection = path_confinement_rejection(&sh, root)
        .unwrap_or_else(|| panic!("expected rejection: {sh:?}"));
    assert!(
        !bash_verify_command_is_auto_approvable(&sh, root),
        "must not be auto-approved: {sh:?} (operation {})",
        rejection.operation
    );
    let cat = format!("cat <<'EOF'\n{body}\nEOF");
    assert!(
        path_confinement_rejection(&cat, root).is_none(),
        "a quoted-delimiter data body must stay allowed: {cat:?}"
    );
}

/// A line continuation between the introducer `$` and `(` keeps the introducer:
/// the ambiguous interior still refuses, while a simple interior still allows.
#[test]
fn quoted_expansion_handles_introducer_line_continuation() {
    let fixture = fixture();
    let root = &fixture.root;
    // The issue's form: the continuation hides the introducer, the inner quote
    // desyncs, and the trailing backtick also refuses.
    let with_backtick = "echo \"$\\\n(echo \" #\")\" `mark`";
    let rejection = path_confinement_rejection(with_backtick, root)
        .unwrap_or_else(|| panic!("expected rejection: {with_backtick:?}"));
    assert!(
        !bash_verify_command_is_auto_approvable(with_backtick, root),
        "must not be auto-approved (operation {})",
        rejection.operation
    );
    // Without the backtick the refusal is the ambiguous expansion itself.
    let ambiguous = "echo \"$\\\n(echo \" #\")\"";
    let rejection = path_confinement_rejection(ambiguous, root)
        .unwrap_or_else(|| panic!("expected rejection: {ambiguous:?}"));
    assert_eq!(rejection.operation, UNREADABLE_OPERATION);
    // A simple interior across the continuation stays allowed.
    let simple = "echo \"$\\\n(echo x)\"";
    assert!(
        path_confinement_rejection(simple, root).is_none(),
        "expected allow: {simple:?}"
    );
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
        // A true comment after a simple expansion stays allowed.
        r#"echo "$(echo x)" # ordinary"#,
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

/// The five fake canaries from the issue — `x`, `秘密585`, `<redacted>`,
/// `runtime_bash_policy`, and `FAKE_585_CANARY` — must leave `R/A/M/S=1`,
/// `V=0`. `x` collides with the fixed English diagnostic, and `<redacted>` hits
/// the existing placeholder gate first, so both are asserted separately; the
/// remaining values must not appear in the fixed reason, which is the same for
/// every value. `M=1` is pinned by `bash_write_guard.rs`'s unit tests.
#[test]
fn quoted_expansion_rejects_canaries_without_echoing_them() {
    let fixture = fixture();
    let root = &fixture.root;
    let canaries = ["FAKE_585_CANARY", "秘密585", "runtime_bash_policy"];
    let mut reasons = Vec::new();
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
            reasons.push(rejection.reason);
        }
    }
    // The new refusal diagnostic does not depend on the input value.
    assert!(
        reasons.windows(2).all(|window| window[0] == window[1]),
        "the fixed reason changed between canaries: {reasons:?}"
    );

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
    assert!(command_references_secret(command).is_some());
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
struct Executed {
    sh: bool,
    dash: bool,
    bash: bool,
}

/// The baseline (`e8fab6c2`) decisions the leader measured for every saved form.
#[derive(Deserialize)]
struct BaseDecision {
    #[serde(rename = "R")]
    r: bool,
    #[serde(rename = "A")]
    a: bool,
    #[serde(rename = "V")]
    v: bool,
    #[serde(rename = "S")]
    s: bool,
}

/// One saved form: its 0-based line id, command, per-shell execution marks, the
/// OR of those marks, and the baseline decisions.
#[derive(Deserialize)]
struct StructuredCase {
    id: usize,
    command: String,
    executed: Executed,
    expected_any_execution: bool,
    base: BaseDecision,
}

impl StructuredCase {
    fn any_execution(&self) -> bool {
        self.executed.sh || self.executed.dash || self.executed.bash
    }
}

/// The 8,640 saved forms from the review: the predicate must reject every form a
/// shell executes, never turn a baseline rejection into an allowance, never turn
/// a baseline `V=false` into `V=true`, and reproduce the baseline's 73 missed
/// executed forms.
#[test]
fn quoted_expansion_structured_corpus_matches_the_predicate() {
    let fixture = fixture();
    let root = &fixture.root;
    let rows = structured_cases();

    assert_eq!(rows.len(), 8_640, "saved forms");
    let ids: Vec<usize> = rows.iter().map(|row| row.id).collect();
    assert_eq!(
        ids,
        (0..8_640).collect::<Vec<_>>(),
        "ids must be 0..8639 in order"
    );
    let unique: BTreeSet<&str> = rows.iter().map(|row| row.command.as_str()).collect();
    assert_eq!(unique.len(), 8_580, "unique commands");

    let started = Instant::now();
    let mut baseline_missed = 0usize;
    for row in &rows {
        assert_eq!(
            row.expected_any_execution,
            row.any_execution(),
            "id {}: expected_any_execution must equal the OR of the shell marks",
            row.id
        );
        let a = path_confinement_rejection(&row.command, root).is_some();
        let v = bash_verify_command_is_auto_approvable(&row.command, root);
        let s = command_references_secret(&row.command).is_some();
        // `R` has no public entry point; the crate-private raw guard and the
        // public `A` carry the same decision in this fixture, so `A` stands in
        // for `R` here.
        assert!(
            !row.base.r || a,
            "id {}: R must not go true->false: {:?}",
            row.id,
            row.command
        );
        assert!(
            !row.base.a || a,
            "id {}: A must not go true->false: {:?}",
            row.id,
            row.command
        );
        assert!(
            !row.base.s || s,
            "id {}: S must not go true->false: {:?}",
            row.id,
            row.command
        );
        assert!(
            row.base.v || !v,
            "id {}: V must not go false->true: {:?}",
            row.id,
            row.command
        );
        if row.expected_any_execution {
            assert!(
                a,
                "id {}: an executed form must be rejected: {:?}",
                row.id, row.command
            );
            assert!(
                !v,
                "id {}: an executed form must not be auto-approved: {:?}",
                row.id, row.command
            );
            if !row.base.a {
                baseline_missed += 1;
            }
        }
    }
    let elapsed = started.elapsed();
    eprintln!(
        "quoted_expansion structured predicate over {} forms: {elapsed:?}",
        rows.len()
    );
    assert_eq!(
        baseline_missed, 73,
        "the baseline missed exactly 73 executed forms"
    );
}

/// Representative forms are re-run under `/bin/sh`, `dash`, and `bash` against a
/// `mark` script on a restricted `PATH`, and the mark's presence must match the
/// fixture's per-shell mark. Only mark-only forms with no write are chosen.
///
/// Issue #587: the four outcomes — the mark ran, the form finished without a
/// mark, the shell is missing, and the run timed out — are counted separately.
/// A missing shell and a timeout are *not* a pass: a missing expected shell
/// fails the test, and so does a timeout. The normal environment (`/bin/sh`,
/// `dash`, `bash`) runs every form exactly as before.
#[test]
fn quoted_expansion_representative_forms_match_shell_execution() {
    let rows = structured_cases();
    // The issue table's executed forms and the non-executing (kept-allow) forms.
    let ids = [3usize, 15, 6483, 0, 12, 24];
    let selected: Vec<&StructuredCase> = ids
        .iter()
        .map(|id| {
            rows.iter()
                .find(|row| row.id == *id)
                .unwrap_or_else(|| panic!("no saved form with id {id}"))
        })
        .collect();

    let dir = tempfile::tempdir().unwrap();
    let mark_dir = dir.path().join("bin");
    std::fs::create_dir_all(&mark_dir).unwrap();
    let mark = mark_dir.join("mark");
    std::fs::write(&mark, "#!/bin/sh\nprintf 'H585_EXEC\\n' >&2\n").unwrap();
    std::fs::set_permissions(&mark, std::fs::Permissions::from_mode(0o755)).unwrap();
    let cwd = dir.path().join("cwd");
    std::fs::create_dir_all(&cwd).unwrap();
    let restricted_path = format!("{}:/usr/bin:/bin", mark_dir.display());

    let mut marks = 0usize;
    let mut no_marks = 0usize;
    let mut timeouts: Vec<(&str, usize)> = Vec::new();
    let mut missing: Vec<&str> = Vec::new();
    for shell in ["/bin/sh", "dash", "/bin/bash"] {
        if !shell_is_available(shell) {
            // A missing shell is not a pass. Record it and fail at the end
            // instead of silently skipping the comparison.
            missing.push(shell);
            continue;
        }
        for row in &selected {
            let expected = match shell {
                "/bin/sh" => row.executed.sh,
                "/bin/bash" => row.executed.bash,
                _ => row.executed.dash,
            };
            let observed = shell_compare(shell, &row.command, &restricted_path, &cwd, dir.path())
                .unwrap_or_else(|error| panic!("failed to run {shell} for id {}: {error}", row.id));
            match observed {
                ShellOutcome::Mark => {
                    assert!(
                        expected,
                        "shell {shell} id {}: the form ran but the fixture expects no mark: {:?}",
                        row.id, row.command
                    );
                    marks += 1;
                }
                ShellOutcome::NoMark => {
                    assert!(
                        !expected,
                        "shell {shell} id {}: the fixture expects a mark but none was seen: {:?}",
                        row.id, row.command
                    );
                    no_marks += 1;
                }
                ShellOutcome::TimedOut => {
                    // A timeout is neither a mark nor a clean non-execution, so
                    // it must fail the comparison explicitly, never be folded
                    // into "no mark" (Issue #587).
                    timeouts.push((shell, row.id));
                }
            }
        }
    }
    assert!(
        timeouts.is_empty(),
        "a timed-out form is not a pass: {timeouts:?}"
    );
    assert!(
        missing.is_empty(),
        "a missing shell is not a pass; expected /bin/sh, dash, and /bin/bash but missing {missing:?}"
    );
    assert!(
        marks > 0 && no_marks > 0,
        "the comparison must observe both executed and non-executed forms: marks={marks} no_marks={no_marks}"
    );
    eprintln!(
        "quoted_expansion shell comparison: {marks} marks, {no_marks} no-marks, 0 timeouts, 0 missing"
    );
}

/// The three distinguishable results of re-running one form under one shell.
enum ShellOutcome {
    /// The `mark` script printed its sentinel.
    Mark,
    /// The form finished without printing the sentinel.
    NoMark,
    /// The form did not finish within the timeout.
    TimedOut,
}

/// Whether the shell can be spawned at all.
fn shell_is_available(shell: &str) -> bool {
    match Command::new(shell)
        .arg("-c")
        .arg(":")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(_) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => panic!("failed to probe shell {shell}: {error}"),
    }
}

/// Runs `command` under `shell` with a closed stdin, a restricted `PATH`, and a
/// two-second timeout, and distinguishes whether the mark reached stdout or
/// stderr, whether the form finished without the mark, or whether it timed out.
/// A timeout is reported separately so the caller never counts it as a pass.
fn shell_compare(
    shell: &str,
    command: &str,
    restricted_path: &str,
    cwd: &Path,
    temp: &Path,
) -> std::io::Result<ShellOutcome> {
    let out_path = temp.join("shell-out.txt");
    let err_path = temp.join("shell-err.txt");
    let mut child = Command::new(shell)
        .arg("-c")
        .arg(command)
        .current_dir(cwd)
        .env_clear()
        .env("PATH", restricted_path)
        .stdin(Stdio::null())
        .stdout(Stdio::from(File::create(&out_path)?))
        .stderr(Stdio::from(File::create(&err_path)?))
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut timed_out = false;
    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            timed_out = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    if timed_out {
        return Ok(ShellOutcome::TimedOut);
    }
    let mut combined = std::fs::read_to_string(&out_path).unwrap_or_default();
    combined.push_str(&std::fs::read_to_string(&err_path).unwrap_or_default());
    Ok(if combined.contains("H585_EXEC") {
        ShellOutcome::Mark
    } else {
        ShellOutcome::NoMark
    })
}
