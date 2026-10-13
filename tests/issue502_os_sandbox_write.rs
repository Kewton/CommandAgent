#![cfg(unix)]

//! Issue #655 (stage 1a): OS write confinement for model Bash and verifier
//! commands. The lexical guard cannot see writes performed *inside* a command
//! (`sh -c 'cp ...'`, `eval`, `xargs`, `find -exec`, an interpreter's
//! `open(..., 'w')`, `dd of=`, `sed -i`) or a hardlink to an outside inode.
//! This file drives the macOS Seatbelt backend through the same `wrap_command`
//! the two launch points use, so an escape fails and the outside fixture does
//! not change.
//!
//! The macOS applied cases are `#[cfg(target_os = "macos")]` and no-op when the
//! backend is unavailable (a nested sandbox reports rc=71). The Linux applied
//! cases are `#[cfg(target_os = "linux")]`; they fail under CI when Landlock is
//! unavailable, so the runner cannot silently lose the backend. Both platforms
//! share the `off` and unavailable paths. The outside fixtures live inside one
//! test tempdir and are never written outside it.

use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::process::{Command, Stdio};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

#[cfg(unix)]
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
    #[cfg(unix)]
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
        #[cfg(unix)]
        outside_secret: outside.join("secret.txt"),
    }
}

#[cfg(unix)]
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

/// Install an available sandbox whose write-allow list is `workspace` and create
/// its run temp dir.
#[cfg(unix)]
fn install_available_for(workspace: &Path, run_tmp: &Path) {
    std::fs::create_dir_all(run_tmp).unwrap();
    os_sandbox::install_for_test(
        SandboxMode::Auto,
        SandboxPolicy::new(workspace, run_tmp),
        SandboxAvailability::Available,
    );
}

/// Install an available sandbox for the fixture and create its run temp dir.
#[cfg(unix)]
fn install_available(fixture: &Fixture) {
    install_available_for(&fixture.root, &run_tmp_path(fixture));
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
#[cfg(unix)]
fn run_wrapped(root: &Path, command: &str) -> BoundedProcessOutput {
    let mut process = Command::new("sh");
    process.arg("-c").arg(command).current_dir(root);
    let mut process = os_sandbox::wrap_command(process).expect("available sandbox wraps");
    process.stdout(Stdio::piped()).stderr(Stdio::piped());
    bounded_process::run_with_timeout(&mut process, TIMEOUT).expect("spawn wrapped command")
}

#[cfg(unix)]
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
    #[cfg(unix)]
    id: String,
    #[cfg(unix)]
    #[serde(rename = "lexicalR")]
    lexical_r: bool,
    #[cfg(unix)]
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
        "[package]\nname = \"issue502-probe\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
    )
    .unwrap();
    std::fs::write(src.join("main.rs"), "fn main() {}\n").unwrap();

    let (ok, stderr) = run_wrapped_success(&fixture.root, "cargo build");
    assert!(ok, "cargo build failed: {stderr}");
    assert!(fixture.root.join("target/debug/issue502-probe").is_file());
    os_sandbox::reset_for_test();
}

// ---------------------------------------------------------------------------
// Linux (Issue #656, stage 1b): the Landlock backend drives the same two launch
// points. These cases skip only when Landlock is genuinely unavailable, and fail
// under CI when it is, so a GitHub Actions Linux runner cannot silently lose the
// backend. The kernel release and Landlock ABI are printed for the record.
// ---------------------------------------------------------------------------

/// The indirect rows the Linux backend measures. The hardlink row is covered by
/// its own ABI2-gated test, so a kernel without ABI2 does not fail here.
#[cfg(target_os = "linux")]
const LINUX_EXCLUDED_INDIRECT_IDS: &[&str] = &["hardlink-outside"];

/// Ask the kernel for its Landlock ABI without going through the backend, so a
/// mutated `landlock::detected_abi()` cannot make these tests agree with
/// themselves. A negative return (unsupported, disabled, or seccomp-denied) is
/// reported as ABI 0.
#[cfg(target_os = "linux")]
fn kernel_landlock_abi() -> u32 {
    const LANDLOCK_CREATE_RULESET_VERSION: libc::c_uint = 1;
    let ret = unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            std::ptr::null::<libc::c_void>(),
            0usize,
            LANDLOCK_CREATE_RULESET_VERSION,
        )
    };
    if ret < 0 { 0 } else { ret as u32 }
}

/// Print the running kernel release and Landlock ABI, and refuse a silent skip
/// under CI: a GitHub Actions Linux runner must have an applied Landlock
/// backend. Returns whether the backend is available for this test run.
///
/// It also pins `landlock::detected_abi()` to the independently queried kernel
/// ABI, so a backend that reports the wrong ABI fails here instead of silently
/// building a weaker ruleset.
#[cfg(target_os = "linux")]
fn require_landlock_backend() -> bool {
    use commandagent::tools::os_sandbox::landlock;
    let abi = kernel_landlock_abi();
    let kernel = landlock::kernel_release_label();
    let available = matches!(os_sandbox::probe(), SandboxAvailability::Available);
    let reported = landlock::detected_abi();
    eprintln!("issue502/656 landlock: kernel={kernel} abi={abi} detected_abi={reported}");
    if available {
        // A backend that reports the wrong ABI would build a weaker ruleset
        // than the kernel supports; pin it to the independent query.
        assert_eq!(
            reported, abi,
            "detected_abi() must equal the kernel ABI queried independently"
        );
    }
    if std::env::var_os("CI").is_some() {
        assert!(
            available,
            "CI: the Landlock backend must be available (kernel {kernel}, abi {abi})"
        );
    }
    available
}

/// The acceptance write set under Landlock: each indirect form fails (or
/// performs no outside write) and the outside fixture is unchanged.
#[cfg(target_os = "linux")]
#[test]
fn linux_applied_indirect_writes_are_denied_and_the_outside_fixture_is_unchanged() {
    let _guard = sandbox_lock();
    if !require_landlock_backend() {
        eprintln!("SKIP: Landlock backend unavailable");
        return;
    }
    let fixture = fixture();
    install_available(&fixture);

    for case in indirect_cases() {
        if LINUX_EXCLUDED_INDIRECT_IDS.contains(&case.id.as_str()) {
            continue;
        }
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

/// A hardlink to an outside inode is refused by Landlock. ABI2 (kernel 5.19)
/// introduced the `refer` right that closes this; without it the link is not
/// closed and this test is skipped with the reason recorded.
#[cfg(target_os = "linux")]
#[test]
fn linux_applied_hardlink_to_an_outside_inode_is_denied() {
    let _guard = sandbox_lock();
    if !require_landlock_backend() {
        eprintln!("SKIP: Landlock backend unavailable");
        return;
    }
    let abi = kernel_landlock_abi();
    if abi < 2 {
        eprintln!(
            "SKIP: Landlock ABI {abi} < 2; creating a cross-directory hardlink is not closed (ABI2)"
        );
        return;
    }
    let fixture = fixture();
    install_available(&fixture);
    let hardlink = fixture.root.join("hard.txt");
    let outside = fixture
        .outside_secret
        .parent()
        .expect("outside secret has a parent");

    let command = format!("ln '{}' hard.txt", outside.join("secret.txt").display());
    let output = run_wrapped(&fixture.root, &command);
    assert!(
        !output.success(),
        "hardlink must be denied: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!hardlink.exists(), "the hardlink must not be created");
    assert_eq!(read_secret(&fixture.outside_secret), SECRET);
    os_sandbox::reset_for_test();
}

/// `truncate(2)` does not `open` the file, so an outside inode needs ABI3
/// (kernel 6.2) to be closed. ABI3 and later must deny it; an older ABI records
/// the skip instead of silently passing.
#[cfg(target_os = "linux")]
#[test]
fn linux_applied_truncate_of_an_outside_inode_is_denied() {
    let _guard = sandbox_lock();
    if !require_landlock_backend() {
        eprintln!("SKIP: Landlock backend unavailable");
        return;
    }
    let abi = kernel_landlock_abi();
    if abi < 3 {
        eprintln!(
            "SKIP: Landlock ABI {abi} < 3; truncating an outside inode via truncate(2) is not closed (ABI3)"
        );
        return;
    }
    let fixture = fixture();
    install_available(&fixture);
    let command = format!(
        "python3 -c \"import os; os.truncate('{}', 0)\"",
        fixture.outside_secret.display()
    );

    let output = run_wrapped(&fixture.root, &command);
    assert!(
        !output.success(),
        "truncate(2) of an outside inode must be denied: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(read_secret(&fixture.outside_secret), SECRET);
    os_sandbox::reset_for_test();
}

/// The public run-start status reports the Linux backend and its ABI
/// limitations: `backend == "landlock"`, `applied == true`, no reason once ABI3
/// closes every supported operation, and the unclosed operations named on an
/// older ABI.
#[cfg(target_os = "linux")]
#[test]
fn linux_installed_status_reports_the_backend_and_its_limitations() {
    let _guard = sandbox_lock();
    if !require_landlock_backend() {
        eprintln!("SKIP: Landlock backend unavailable");
        return;
    }
    let abi = kernel_landlock_abi();
    let fixture = fixture();
    let state_dir = fixture._dir.path().join("state");

    let status = os_sandbox::install(SandboxMode::Auto, &fixture.root, &state_dir);
    assert_eq!(status.backend, "landlock", "{status:?}");
    assert!(status.applied, "{status:?}");
    assert_eq!(status.mode.as_str(), "auto");
    if abi >= 3 {
        assert_eq!(
            status.reason, None,
            "ABI3+ closes truncate and cross-directory link/rename: {status:?}"
        );
    } else {
        let reason = status.reason.as_deref().unwrap_or("");
        assert!(
            reason.contains("ABI2") || reason.contains("ABI3"),
            "an older ABI must name the unclosed operations: {status:?}"
        );
    }
    assert_eq!(
        os_sandbox::status().map(|status| status.backend),
        Some("landlock")
    );

    os_sandbox::cleanup();
    os_sandbox::reset_for_test();
}

/// A direct write to the system temp directory (not the run temp directory) is
/// denied, and the probe path is never created.
#[cfg(target_os = "linux")]
#[test]
fn linux_applied_system_temp_write_is_denied() {
    let _guard = sandbox_lock();
    if !require_landlock_backend() {
        eprintln!("SKIP: Landlock backend unavailable");
        return;
    }
    let fixture = fixture();
    install_available(&fixture);
    let system_tmp = std::env::temp_dir()
        .canonicalize()
        .unwrap_or_else(|_| std::env::temp_dir());
    let target = system_tmp.join(format!("commandagent-502b-{}.txt", std::process::id()));

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

/// The verifier/acceptance launch point is wired to the same Landlock sandbox.
#[cfg(target_os = "linux")]
#[test]
fn linux_applied_verifier_commands_cannot_write_outside() {
    let _guard = sandbox_lock();
    if !require_landlock_backend() {
        eprintln!("SKIP: Landlock backend unavailable");
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
/// Landlock (`TMPDIR` points at the allowed run temp directory).
#[cfg(target_os = "linux")]
#[test]
fn linux_applied_required_tools_succeed_inside_the_sandbox() {
    let _guard = sandbox_lock();
    if !require_landlock_backend() {
        eprintln!("SKIP: Landlock backend unavailable");
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

/// A dependency-less `cargo build` succeeds inside Landlock when the cargo
/// caches are allowed. Ignored by default: run explicitly to record the applied
/// result.
#[cfg(target_os = "linux")]
#[test]
#[ignore = "heavy: run explicitly to record the applied result"]
fn linux_applied_cargo_build_succeeds() {
    let _guard = sandbox_lock();
    if !require_landlock_backend() {
        return;
    }
    let fixture = fixture();
    install_available(&fixture);
    let src = fixture.root.join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(
        fixture.root.join("Cargo.toml"),
        "[package]\nname = \"issue502-probe\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
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

/// A workflow child node whose workspace differs from the run's is recorded as
/// not isolated, so the fixed parent profile is never applied to the wrong
/// workspace. This is the behavior `run_resolved_config_for_workflow` drives for
/// every node.
#[test]
fn mismatched_node_workspace_records_the_sandbox_not_applied() {
    let _guard = sandbox_lock();
    let fixture = fixture();
    let parent = fixture._dir.path().join("parent");
    std::fs::create_dir_all(&parent).unwrap();
    let parent_tmp = fixture._dir.path().join("parent-run-tmp");
    std::fs::create_dir_all(&parent_tmp).unwrap();
    os_sandbox::install_for_test(
        SandboxMode::Auto,
        SandboxPolicy::new(&parent, &parent_tmp),
        SandboxAvailability::Available,
    );

    let status = os_sandbox::reconcile_workspace(&fixture.root).expect("installed");
    assert!(!status.applied, "{status:?}");
    assert!(
        status
            .reason
            .as_deref()
            .is_some_and(|reason| !reason.is_empty()),
        "{status:?}"
    );
    // Idempotent: a later node in the same workspace keeps the same record.
    assert_eq!(
        os_sandbox::reconcile_workspace(&fixture.root).expect("installed"),
        status
    );
    os_sandbox::reset_for_test();
}

/// `required` fails the node's Bash when the node workspace differs, so a
/// "required" run cannot silently continue without the sandbox.
#[test]
fn required_fails_when_the_node_workspace_differs() {
    let _guard = sandbox_lock();
    let fixture = fixture();
    let parent = fixture._dir.path().join("parent");
    std::fs::create_dir_all(&parent).unwrap();
    let parent_tmp = fixture._dir.path().join("parent-run-tmp");
    std::fs::create_dir_all(&parent_tmp).unwrap();
    os_sandbox::install_for_test(
        SandboxMode::Required,
        SandboxPolicy::new(&parent, &parent_tmp),
        SandboxAvailability::Available,
    );

    let status = os_sandbox::reconcile_workspace(&fixture.root).expect("installed");
    assert!(!status.applied, "{status:?}");

    let outcome = run_via_bash(&fixture.root, "true");
    assert_eq!(outcome.kind, BashOutcomeKind::Blocked, "{outcome:?}");
    assert!(
        outcome.summary.contains("bash_sandbox_unavailable"),
        "{}",
        outcome.summary
    );
    os_sandbox::reset_for_test();
}

/// With the backend applied, a mismatched node stops being confined by the
/// parent's policy and is not claimed as isolated; the child write is no longer
/// denied on the parent's behalf.
#[cfg(target_os = "macos")]
#[test]
fn applied_mismatch_drops_the_false_isolation_claim() {
    let _guard = sandbox_lock();
    if !backend_available() {
        eprintln!("SKIP: sandbox backend unavailable (nested sandbox?)");
        return;
    }
    let fixture = fixture();
    let parent = fixture._dir.path().join("parent");
    std::fs::create_dir_all(&parent).unwrap();
    let parent_tmp = fixture._dir.path().join("parent-run-tmp");
    install_available_for(&parent, &parent_tmp);

    // Before reconciling, the parent policy confines the child to the wrong
    // workspace, so a child write is denied.
    let before = run_via_bash(&fixture.root, "echo child > origin.txt");
    assert!(!before.is_success(), "{before:?}");
    assert!(!fixture.root.join("origin.txt").exists());

    let status = os_sandbox::reconcile_workspace(&fixture.root).expect("installed");
    assert!(!status.applied, "{status:?}");

    // After reconciling, no isolation is claimed or applied for the child.
    let after = run_via_bash(&fixture.root, "echo child > origin.txt");
    assert!(after.is_success(), "{after:?}");
    assert!(fixture.root.join("origin.txt").is_file());
    os_sandbox::reset_for_test();
}

/// The default run (origin == workspace) keeps the install untouched: the same
/// temp dir remains and the record still reports the sandbox applied.
#[cfg(target_os = "macos")]
#[test]
fn applied_reconcile_is_a_no_op_when_the_node_workspace_matches() {
    let _guard = sandbox_lock();
    if !backend_available() {
        eprintln!("SKIP: sandbox backend unavailable (nested sandbox?)");
        return;
    }
    let fixture = fixture();
    install_available(&fixture);
    let run_tmp = run_tmp_path(&fixture);

    let status = os_sandbox::reconcile_workspace(&fixture.root).expect("installed");
    assert!(status.applied, "{status:?}");
    assert!(run_tmp.is_dir(), "the run temp dir must be kept");

    let outcome = run_via_bash(&fixture.root, "echo ok > keeps.txt");
    assert!(outcome.is_success(), "{outcome:?}");
    assert!(fixture.root.join("keeps.txt").is_file());
    os_sandbox::reset_for_test();
}

/// A bare `mktemp` fails inside the sandbox, and its summary points at the run
/// temp directory instead of leaving the model to guess.
#[cfg(target_os = "macos")]
#[test]
fn applied_bare_mktemp_failure_summarizes_the_tmpdir_guidance() {
    let _guard = sandbox_lock();
    if !backend_available() {
        eprintln!("SKIP: sandbox backend unavailable (nested sandbox?)");
        return;
    }
    let fixture = fixture();
    install_available(&fixture);

    let outcome = run_via_bash(&fixture.root, "mktemp");
    assert_eq!(outcome.kind, BashOutcomeKind::CommandFailed, "{outcome:?}");
    assert!(
        outcome.summary.contains("mktemp -p \"$TMPDIR\""),
        "{}",
        outcome.summary
    );
    os_sandbox::reset_for_test();
}

/// Without an applied sandbox, the same temp-dir permission failure gets no
/// mktemp guidance.
#[test]
fn no_mktemp_guidance_without_an_applied_sandbox() {
    let _guard = sandbox_lock();
    let fixture = fixture();
    install_unavailable(SandboxMode::Auto, &fixture, NESTED);

    let outcome = run_via_bash(
        &fixture.root,
        "printf 'mktemp: mkstemp failed: Operation not permitted\\n' >&2; exit 1",
    );
    assert_eq!(outcome.kind, BashOutcomeKind::CommandFailed, "{outcome:?}");
    assert!(
        !outcome.summary.contains("mktemp -p \"$TMPDIR\""),
        "{}",
        outcome.summary
    );
    os_sandbox::reset_for_test();
}

/// A run creates its temp dir at install and leaves none behind after cleanup,
/// which the CLI runs on every exit path.
#[test]
fn install_creates_the_run_temp_dir_and_cleanup_removes_it() {
    let _guard = sandbox_lock();
    let fixture = fixture();
    let state_dir = fixture._dir.path().join("state");

    let status = os_sandbox::install(SandboxMode::Off, &fixture.root, &state_dir);
    assert!(!status.applied);
    let sandbox_dir = state_dir.join("sandbox");
    assert!(sandbox_dir.is_dir(), "install must create the run temp dir");
    assert_eq!(std::fs::read_dir(&sandbox_dir).unwrap().count(), 1);

    os_sandbox::cleanup();
    let remaining = std::fs::read_dir(&sandbox_dir)
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(remaining, 0, "cleanup must remove the run temp dir");
    os_sandbox::reset_for_test();
}

/// `cleanup` ends the run: the sandbox must no longer be reported as applied and
/// `required` must not pass a command through unwrapped, so a child spawned
/// during shutdown never runs without the confinement the run promised.
#[test]
fn cleanup_ends_the_run_without_leaving_the_sandbox_applied() {
    let _guard = sandbox_lock();
    let fixture = fixture();
    let run_tmp = run_tmp_path(&fixture);
    std::fs::create_dir_all(&run_tmp).unwrap();
    os_sandbox::install_for_test(
        SandboxMode::Required,
        SandboxPolicy::new(&fixture.root, &run_tmp),
        SandboxAvailability::Available,
    );
    assert!(os_sandbox::is_applied());

    os_sandbox::cleanup();

    assert!(
        !os_sandbox::is_applied(),
        "cleanup must not leave the sandbox reported as applied"
    );
    let error = os_sandbox::wrap_command(std::process::Command::new("sh"))
        .expect_err("required must refuse instead of passing through after cleanup");
    assert!(
        error.summary().contains("bash_sandbox_unavailable"),
        "{}",
        error.summary()
    );
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
