#![cfg(unix)]

//! Issue #655 (stage 1a): OS write confinement for model Bash and verifier
//! commands. The lexical guard cannot see writes performed *inside* a command
//! (`sh -c 'cp ...'`, `eval`, `xargs`, `find -exec`, an interpreter's
//! `open(..., 'w')`, `dd of=`, `sed -i`) or a hardlink to an outside inode.
//! This file drives the macOS Seatbelt backend through the same `wrap_command`
//! the two launch points use, so an escape fails and the outside fixture does
//! not change.
//!
//! The applied cases are `#[cfg(target_os = "macos")]` and no-op when the
//! backend is unavailable (a nested sandbox reports rc=71); on Linux CI only the
//! `off` and unavailable paths run, matching the issue's acceptance note. The
//! outside fixtures live inside one test tempdir and are never written outside
//! it.

use std::path::{Path, PathBuf};
#[cfg(target_os = "macos")]
use std::process::{Command, Stdio};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

#[cfg(target_os = "macos")]
use commandagent::bounded_process::{self, BoundedProcessOutput};
use commandagent::minimal_loop::verifier_env;
use commandagent::planner::verify::{NormalizedVerifyCommand, normalize_verify_command};
use commandagent::tools::allow_policy::bash_verify_command_is_auto_approvable;
use commandagent::tools::bash::{self, BashOutcome, BashOutcomeKind, path_confinement_rejection};
use commandagent::tools::os_sandbox::{self, SandboxAvailability, SandboxMode, SandboxPolicy};
use commandagent::tools::sensitive_path::command_references_secret;
use serde::Deserialize;

const SECRET: &str = "OUTSIDE_SECRET_CONTENT_502a";
const NESTED: &str = "sandbox-exec probe exited with exit status: 71";
const TIMEOUT: Duration = Duration::from_secs(120);

/// The sandbox state is process-wide, so the tests that install it serialize.
static SANDBOX_LOCK: Mutex<()> = Mutex::new(());

fn sandbox_lock() -> MutexGuard<'static, ()> {
    SANDBOX_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
    #[cfg(target_os = "macos")]
    outside_secret: PathBuf,
}

/// A workspace (`ws`) with an outward `escape` symlink and an outside fixture
/// (`outside/secret.txt`). `outside` is a sibling of the workspace inside the
/// same tempdir, so a rejected write leaves the tempdir-only fixture untouched.
fn fixture() -> Fixture {
    let dir = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).expect("create tempdir");
    let ws = dir.path().join("ws");
    let outside = dir.path().join("outside");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(ws.join("a.txt"), "x\n").unwrap();
    std::fs::write(outside.join("secret.txt"), SECRET).unwrap();
    std::os::unix::fs::symlink(&outside, ws.join("escape")).unwrap();
    let root = ws.canonicalize().unwrap();
    Fixture {
        _dir: dir,
        root,
        #[cfg(target_os = "macos")]
        outside_secret: outside.join("secret.txt"),
    }
}

#[cfg(target_os = "macos")]
fn read_secret(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|_| "<missing>".to_string())
}

fn run_tmp_path(fixture: &Fixture) -> PathBuf {
    fixture._dir.path().join("run-tmp")
}

#[cfg(target_os = "macos")]
fn backend_available() -> bool {
    matches!(os_sandbox::probe(), SandboxAvailability::Available)
}

/// Install an available sandbox for the fixture and create its run temp dir.
#[cfg(target_os = "macos")]
fn install_available(fixture: &Fixture) {
    let run_tmp = run_tmp_path(fixture);
    std::fs::create_dir_all(&run_tmp).unwrap();
    os_sandbox::install_for_test(
        SandboxMode::Auto,
        SandboxPolicy::new(&fixture.root, &run_tmp),
        SandboxAvailability::Available,
    );
}

fn install_unavailable(mode: SandboxMode, fixture: &Fixture, reason: &str) {
    os_sandbox::install_for_test(
        mode,
        SandboxPolicy::new(&fixture.root, &run_tmp_path(fixture)),
        SandboxAvailability::Unavailable(reason.to_string()),
    );
}

/// Run through the model Bash launch point (`sh -c`), including the lexical
/// guard. This is the production path the verify interval also uses.
fn run_via_bash(root: &Path, command: &str) -> BashOutcome {
    bash::run_structured_cancel_and_force(command, root, false, TIMEOUT, || false, || false)
        .expect("bash launch")
}

/// Build `sh -c <command>` and apply the installed sandbox directly, bypassing
/// the lexical guard, so the OS layer is measured on its own.
#[cfg(target_os = "macos")]
fn run_wrapped(root: &Path, command: &str) -> BoundedProcessOutput {
    let mut process = Command::new("sh");
    process.arg("-c").arg(command).current_dir(root);
    let mut process = os_sandbox::wrap_command(process).expect("available sandbox wraps");
    process.stdout(Stdio::piped()).stderr(Stdio::piped());
    bounded_process::run_with_timeout(&mut process, TIMEOUT).expect("spawn wrapped command")
}

#[cfg(target_os = "macos")]
fn run_wrapped_success(root: &Path, command: &str) -> (bool, String) {
    let output = run_wrapped(root, command);
    (
        output.success(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[derive(Deserialize)]
struct IndirectCase {
    command: String,
    #[cfg(target_os = "macos")]
    id: String,
    #[cfg(target_os = "macos")]
    #[serde(rename = "lexicalR")]
    lexical_r: bool,
    #[cfg(target_os = "macos")]
    #[serde(rename = "exitNonzero")]
    exit_nonzero: bool,
}

fn indirect_cases() -> Vec<IndirectCase> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/corpus/apps/issue502-indirect-write/fixtures/indirect-cases.jsonl");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).unwrap_or_else(|error| panic!("{line:?}: {error}")))
        .collect()
}

/// The acceptance write set: each indirect form fails (or performs no outside
/// write) under the sandbox, and the outside fixture is unchanged. Rows the
/// lexical guard already rejects are still driven here so the OS layer is shown
/// to block them independently.
#[cfg(target_os = "macos")]
#[test]
fn applied_indirect_writes_are_denied_and_the_outside_fixture_is_unchanged() {
    let _guard = sandbox_lock();
    if !backend_available() {
        eprintln!("SKIP: sandbox backend unavailable (nested sandbox?)");
        return;
    }
    let fixture = fixture();
    install_available(&fixture);

    for case in indirect_cases() {
        assert_eq!(
            path_confinement_rejection(&case.command, &fixture.root).is_some(),
            case.lexical_r,
            "{}: the lexical decision must be unchanged",
            case.id
        );
        let output = run_wrapped(&fixture.root, &case.command);
        assert_eq!(
            !output.success(),
            case.exit_nonzero,
            "{}: sandbox exit code; stderr={}",
            case.id,
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            read_secret(&fixture.outside_secret),
            SECRET,
            "{}: the outside fixture must not change",
            case.id
        );
        // When the lexical guard lets the command through, the model Bash path
        // itself must agree with the sandbox outcome, proving the launch point
        // is wired (a denied write fails; a control row still succeeds).
        if !case.lexical_r {
            let outcome = run_via_bash(&fixture.root, &case.command);
            assert_eq!(
                !outcome.is_success(),
                case.exit_nonzero,
                "{}: the bash launch point; {outcome:?}",
                case.id
            );
            assert_eq!(read_secret(&fixture.outside_secret), SECRET, "{}", case.id);
        }
    }
    os_sandbox::reset_for_test();
}

/// A hardlink to an outside inode is refused by the sandbox.
#[cfg(target_os = "macos")]
#[test]
fn applied_hardlink_to_an_outside_inode_is_denied() {
    let _guard = sandbox_lock();
    if !backend_available() {
        eprintln!("SKIP: sandbox backend unavailable (nested sandbox?)");
        return;
    }
    let fixture = fixture();
    install_available(&fixture);
    let hardlink = fixture.root.join("hard.txt");

    let output = run_wrapped(&fixture.root, "ln escape/secret.txt hard.txt");
    assert!(
        !output.success(),
        "hardlink must be denied: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!hardlink.exists(), "the hardlink must not be created");
    assert_eq!(read_secret(&fixture.outside_secret), SECRET);
    os_sandbox::reset_for_test();
}

/// A direct write to the system temp directory (not the run temp directory) is
/// denied. The attempt targets a unique non-existent path and asserts it was
/// never created, so nothing is left outside the fixture.
#[cfg(target_os = "macos")]
#[test]
fn applied_system_temp_write_is_denied() {
    let _guard = sandbox_lock();
    if !backend_available() {
        eprintln!("SKIP: sandbox backend unavailable (nested sandbox?)");
        return;
    }
    let fixture = fixture();
    install_available(&fixture);
    let system_tmp = std::env::temp_dir()
        .canonicalize()
        .unwrap_or_else(|_| std::env::temp_dir());
    let target = system_tmp.join(format!("commandagent-502a-{}.txt", std::process::id()));

    let command = format!("echo written > '{}'", target.display());
    let output = run_wrapped(&fixture.root, &command);
    assert!(
        !output.success(),
        "a write to the system temp directory must be denied: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!target.exists(), "the system temp probe must not exist");
    let _ = std::fs::remove_file(&target);
    os_sandbox::reset_for_test();
}

/// The verifier/acceptance launch point is wired to the same sandbox: a command
/// the lexical guard accepts cannot write outside.
#[cfg(target_os = "macos")]
#[test]
fn applied_verifier_commands_cannot_write_outside() {
    let _guard = sandbox_lock();
    if !backend_available() {
        eprintln!("SKIP: sandbox backend unavailable (nested sandbox?)");
        return;
    }
    let fixture = fixture();
    install_available(&fixture);
    let command: NormalizedVerifyCommand =
        normalize_verify_command("sh -c 'cp a.txt escape/secret.txt'").expect("normalize");
    assert!(
        path_confinement_rejection(command.as_str(), &fixture.root).is_none(),
        "the lexical guard must pass this form so the sandbox is what blocks it"
    );

    let outcome =
        verifier_env::run_structured_for_verify_with_profile(&command, &fixture.root, None, false)
            .expect("verifier launch");
    assert!(!outcome.is_success(), "{outcome:?}");
    assert_eq!(read_secret(&fixture.outside_secret), SECRET);
    os_sandbox::reset_for_test();
}

/// Required tools that only read or use the run temp directory succeed inside
/// the sandbox. `mktemp` is given the allowed directory explicitly because
/// macOS `mktemp` ignores `TMPDIR` and would otherwise target the per-user temp
/// directory, which the sandbox denies by design.
#[cfg(target_os = "macos")]
#[test]
fn applied_required_tools_succeed_inside_the_sandbox() {
    let _guard = sandbox_lock();
    if !backend_available() {
        eprintln!("SKIP: sandbox backend unavailable (nested sandbox?)");
        return;
    }
    let fixture = fixture();
    install_available(&fixture);
    std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(&fixture.root)
        .status()
        .expect("git init");

    for command in [
        "mktemp -p \"$TMPDIR\" probe.XXXXXX",
        "mktemp \"$TMPDIR/probe.XXXXXX\"",
        "git status --porcelain",
        "cargo --version",
        "node -e 'process.stdout.write(\"ok\")'",
        "python3 -c 'print(\"ok\")'",
        "echo inside > inside.txt",
    ] {
        let (ok, stderr) = run_wrapped_success(&fixture.root, command);
        assert!(ok, "expected success: {command}; stderr={stderr}");
    }
    assert!(fixture.root.join("inside.txt").is_file());
    os_sandbox::reset_for_test();
}

/// A dependency-less `cargo build` succeeds when the cargo caches are allowed.
#[cfg(target_os = "macos")]
#[test]
#[ignore = "heavy: run explicitly to record the applied result"]
fn applied_cargo_build_succeeds() {
    let _guard = sandbox_lock();
    if !backend_available() {
        return;
    }
    let fixture = fixture();
    install_available(&fixture);
    let src = fixture.root.join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(
        fixture.root.join("Cargo.toml"),
        "[package]\nname = \"issue502-probe\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(src.join("main.rs"), "fn main() {}\n").unwrap();

    let (ok, stderr) = run_wrapped_success(&fixture.root, "cargo build");
    assert!(ok, "cargo build failed: {stderr}");
    assert!(fixture.root.join("target/debug/issue502-probe").is_file());
    os_sandbox::reset_for_test();
}

/// The probe agrees with a direct `/usr/bin/sandbox-exec` run: `rc=0` is
/// available and any non-zero rc (for example 71 under a nested sandbox) is
/// unavailable. This stays honest whether or not the test process is already
/// sandboxed, so it catches a probe that always claims one verdict.
#[cfg(target_os = "macos")]
#[test]
fn probe_matches_a_direct_sandbox_exec_run() {
    let direct = std::process::Command::new("/usr/bin/sandbox-exec")
        .args(["-p", "(version 1)(allow default)", "true"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run sandbox-exec directly");
    assert_eq!(
        matches!(os_sandbox::probe(), SandboxAvailability::Available),
        direct.success(),
        "probe must match the direct sandbox-exec exit status ({direct})"
    );
}

/// `required` fails both launch points with `bash_sandbox_unavailable`, while
/// `auto` warns once, records `applied=false`, and continues with lexical checks.
#[test]
fn required_and_unavailable_fail_honestly_and_auto_continues() {
    let _guard = sandbox_lock();
    let fixture = fixture();

    install_unavailable(SandboxMode::Required, &fixture, NESTED);
    let outcome = run_via_bash(&fixture.root, "true");
    assert_eq!(outcome.kind, BashOutcomeKind::Blocked, "{outcome:?}");
    assert!(
        outcome.summary.contains("bash_sandbox_unavailable"),
        "{}",
        outcome.summary
    );
    let command: NormalizedVerifyCommand = normalize_verify_command("true").unwrap();
    let verifier =
        verifier_env::run_structured_for_verify_with_profile(&command, &fixture.root, None, false)
            .unwrap();
    assert_eq!(verifier.kind, BashOutcomeKind::Blocked, "{verifier:?}");
    assert!(
        verifier.summary.contains("bash_sandbox_unavailable"),
        "{}",
        verifier.summary
    );

    install_unavailable(SandboxMode::Auto, &fixture, NESTED);
    let status = os_sandbox::status().unwrap();
    assert!(!status.applied);
    assert_eq!(status.mode.as_str(), "auto");
    assert_eq!(status.reason.as_deref(), Some(NESTED));
    let warning = os_sandbox::startup_warning().expect("auto warns once");
    assert!(warning.contains(NESTED), "{warning}");
    assert!(
        run_via_bash(&fixture.root, "true").is_success(),
        "auto must continue with lexical checks only"
    );

    os_sandbox::reset_for_test();
}

/// `off` keeps the previous behavior and never claims isolation.
#[test]
fn off_never_claims_isolation() {
    let _guard = sandbox_lock();
    let fixture = fixture();
    install_unavailable(SandboxMode::Off, &fixture, "unused");
    let status = os_sandbox::status().unwrap();
    assert!(!status.applied);
    assert_eq!(status.reason, None);
    assert!(os_sandbox::startup_warning().is_none());
    assert!(run_via_bash(&fixture.root, "true").is_success());
    os_sandbox::reset_for_test();
}

fn lexical_triple(root: &Path, command: &str) -> (bool, bool, bool) {
    (
        path_confinement_rejection(command, root).is_some(),
        command_references_secret(command).is_some(),
        bash_verify_command_is_auto_approvable(command, root),
    )
}

/// With the sandbox off or unavailable, the lexical decision is exactly what it
/// is with no sandbox installed: no row flips R/S from 1 to 0, and no row flips
/// V from 0 to 1.
#[test]
fn sandbox_off_and_unavailable_do_not_change_the_lexical_decision() {
    let _guard = sandbox_lock();
    let fixture = fixture();
    let mut commands: Vec<String> = indirect_cases().into_iter().map(|c| c.command).collect();
    commands.extend(
        [
            "true",
            "cargo test",
            "cat README.md",
            "echo $HOME",
            "sh -c 'cat a.txt'",
            "cat a.txt",
        ]
        .into_iter()
        .map(str::to_string),
    );

    os_sandbox::reset_for_test();
    let baseline: Vec<(bool, bool, bool)> = commands
        .iter()
        .map(|command| lexical_triple(&fixture.root, command))
        .collect();

    for (mode, availability) in [
        (
            SandboxMode::Off,
            SandboxAvailability::Unavailable("off".to_string()),
        ),
        (
            SandboxMode::Auto,
            SandboxAvailability::Unavailable(NESTED.to_string()),
        ),
    ] {
        os_sandbox::install_for_test(
            mode,
            SandboxPolicy::new(&fixture.root, &run_tmp_path(&fixture)),
            availability,
        );
        for (command, expected) in commands.iter().zip(&baseline) {
            assert_eq!(
                &lexical_triple(&fixture.root, command),
                expected,
                "{command:?}: the lexical decision must not change"
            );
        }
    }
    os_sandbox::reset_for_test();
}
