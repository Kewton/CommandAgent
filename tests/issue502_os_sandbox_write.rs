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

#[cfg(unix)]
use std::ffi::CStr;
#[cfg(unix)]
use std::os::fd::{AsRawFd, FromRawFd};
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::process::{Command, Stdio};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

#[cfg(unix)]
use commandagent::bounded_process::{self, BoundedProcessOutcomeKind, BoundedProcessOutput};
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

/// The running kernel release, read through the backend's public accessor (which
/// reads `/proc/sys/kernel/osrelease`).
#[cfg(target_os = "linux")]
fn landlock_kernel_release() -> String {
    commandagent::tools::os_sandbox::landlock::kernel_release_label()
}

/// The ABI-gated outcome for an ABI-dependent Linux test.
#[cfg(target_os = "linux")]
#[derive(Debug, PartialEq, Eq)]
enum AbiGate {
    /// The kernel ABI is new enough.
    Continue,
    /// The ABI is too old and this is a local run; record and skip.
    Skip(String),
    /// The ABI is too old and this is CI; the runner must provide it.
    Fail(String),
}

/// Decide whether an ABI-dependent test continues, skips, or fails. Pure: it
/// takes the CI flag, the queried ABI, and the ABI it needs, so it is testable
/// without touching the environment. The skip/fail reasons name the kernel and
/// the ABI.
#[cfg(target_os = "linux")]
fn abi_gate(ci: bool, abi: u32, required_abi: u32, kernel: &str) -> AbiGate {
    if abi >= required_abi {
        AbiGate::Continue
    } else if ci {
        AbiGate::Fail(format!(
            "CI: Landlock ABI {abi} (kernel {kernel}) is below the required ABI{required_abi}"
        ))
    } else {
        AbiGate::Skip(format!(
            "SKIP: Landlock ABI {abi} (kernel {kernel}) < {required_abi}"
        ))
    }
}

/// Apply [`abi_gate`] with the process `CI`, returning whether the test should
/// continue. A too-old ABI fails under CI and skips otherwise.
#[cfg(target_os = "linux")]
fn require_abi_or_ci_fail(abi: u32, required_abi: u32) -> bool {
    match abi_gate(
        std::env::var_os("CI").is_some(),
        abi,
        required_abi,
        &landlock_kernel_release(),
    ) {
        AbiGate::Continue => true,
        AbiGate::Skip(reason) => {
            eprintln!("{reason}");
            false
        }
        AbiGate::Fail(reason) => panic!("{reason}"),
    }
}

/// Append the kernel release and Landlock ABI to `$GITHUB_STEP_SUMMARY` once per
/// process, when that variable is set. Best effort: a write failure never fails
/// the test, and only the kernel version and ABI number are written.
#[cfg(target_os = "linux")]
fn record_kernel_and_abi_in_step_summary() {
    use std::sync::Once;

    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let Some(path) = std::env::var_os("GITHUB_STEP_SUMMARY") else {
            return;
        };
        let line = format!(
            "issue502/656 landlock: kernel={} abi={}\n",
            landlock_kernel_release(),
            kernel_landlock_abi()
        );
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .and_then(|mut file| std::io::Write::write_all(&mut file, line.as_bytes()));
    });
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
    record_kernel_and_abi_in_step_summary();
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

/// The ABI gate: a too-old ABI fails under CI, skips on a local run, and a new
/// enough ABI continues. The kernel and the queried ABI appear in the reason.
#[cfg(target_os = "linux")]
#[test]
fn linux_abi_gate_fails_under_ci_and_skips_otherwise() {
    let kernel = "6.1.0-generic";
    assert!(
        matches!(
            abi_gate(true, 1, 2, kernel),
            AbiGate::Fail(reason) if reason.contains(kernel) && reason.contains('1')
        ),
        "{:?}",
        abi_gate(true, 1, 2, kernel)
    );
    assert!(matches!(abi_gate(false, 1, 2, kernel), AbiGate::Skip(_)));
    assert_eq!(abi_gate(false, 3, 3, kernel), AbiGate::Continue);
    assert_eq!(abi_gate(true, 3, 3, kernel), AbiGate::Continue);
    assert_eq!(abi_gate(false, 8, 2, kernel), AbiGate::Continue);
}

/// A hardlink to an outside inode is refused by Landlock. ABI2 (kernel 5.19)
/// introduced the `refer` right that closes this. A too-old ABI skips on a local
/// run and fails under CI, so a CI runner cannot silently lose the closure.
#[cfg(target_os = "linux")]
#[test]
fn linux_applied_hardlink_to_an_outside_inode_is_denied() {
    let _guard = sandbox_lock();
    if !require_landlock_backend() {
        eprintln!("SKIP: Landlock backend unavailable");
        return;
    }
    let abi = kernel_landlock_abi();
    if !require_abi_or_ci_fail(abi, 2) {
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
/// (kernel 6.2) to be closed. ABI3 and later must deny it; a too-old ABI skips
/// on a local run and fails under CI.
#[cfg(target_os = "linux")]
#[test]
fn linux_applied_truncate_of_an_outside_inode_is_denied() {
    let _guard = sandbox_lock();
    if !require_landlock_backend() {
        eprintln!("SKIP: Landlock backend unavailable");
        return;
    }
    let abi = kernel_landlock_abi();
    if !require_abi_or_ci_fail(abi, 3) {
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

// ---------------------------------------------------------------------------
// Terminal isolation (Issue #667): a confined command must not be able to
// inject keystrokes into the caller's terminal with `TIOCSTI`, nor write to
// another terminal's slave.
// ---------------------------------------------------------------------------

/// The `TIOCSCTTY` request number, which the `libc` crate does not name on
/// macOS. `TCGETS`/BSD `_IO('t', 97)` differs per platform.
#[cfg(target_os = "macos")]
const TIOCSCTTY_REQUEST: libc::c_ulong = 0x2000_7461;
#[cfg(target_os = "linux")]
const TIOCSCTTY_REQUEST: libc::c_ulong = 0x540E;

/// A Python one-liner that tries `TIOCSTI` with `inject` on fd 0 and reports the
/// outcome, so the attempt travels through a real program exactly as a command
/// would. The kernel only allows `TIOCSTI` when fd 0 is the caller's controlling
/// terminal.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn tiocsti_script(inject: char) -> String {
    format!(
        r#"import fcntl, termios, sys
try:
    fcntl.ioctl(0, termios.TIOCSTI, b'{inject}')
    sys.stdout.write('TIOCSTI_OK\n')
except OSError as error:
    sys.stdout.write('TIOCSTI_FAIL %d\n' % error.errno)
"#
    )
}

/// Open a pty the test owns and return `(master, slave, slave_path)`. The master
/// is non-blocking so a test can drain it without waiting; both descriptors stay
/// open in the returned tuple. Only ptys the test created are ever used — a test
/// never opens the worker's or the user's real terminal.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn open_pty() -> (std::fs::File, std::fs::File, PathBuf) {
    let master = unsafe { libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY) };
    assert!(
        master >= 0,
        "posix_openpt: {}",
        std::io::Error::last_os_error()
    );
    assert_eq!(
        unsafe { libc::grantpt(master) },
        0,
        "grantpt: {}",
        std::io::Error::last_os_error()
    );
    assert_eq!(
        unsafe { libc::unlockpt(master) },
        0,
        "unlockpt: {}",
        std::io::Error::last_os_error()
    );
    let name = unsafe { libc::ptsname(master) };
    assert!(
        !name.is_null(),
        "ptsname: {}",
        std::io::Error::last_os_error()
    );
    let flags = unsafe { libc::fcntl(master, libc::F_GETFL) };
    let _ = unsafe { libc::fcntl(master, libc::F_SETFL, flags | libc::O_NONBLOCK) };
    let slave_path = PathBuf::from(
        unsafe { CStr::from_ptr(name) }
            .to_string_lossy()
            .into_owned(),
    );
    let slave = unsafe { libc::open(name, libc::O_RDWR | libc::O_NOCTTY) };
    assert!(
        slave >= 0,
        "open slave: {}",
        std::io::Error::last_os_error()
    );
    (
        unsafe { std::fs::File::from_raw_fd(master) },
        unsafe { std::fs::File::from_raw_fd(slave) },
        slave_path,
    )
}

/// A `Stdio` for a duplicate of `file`, so the caller keeps the original open.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn dup_stdio(file: &std::fs::File) -> Stdio {
    let fd = unsafe { libc::dup(file.as_raw_fd()) };
    assert!(fd >= 0, "dup: {}", std::io::Error::last_os_error());
    Stdio::from(unsafe { std::fs::File::from_raw_fd(fd) })
}

/// Drain the non-blocking master side of a pty into a string.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn drain_master(master: &std::fs::File) -> String {
    use std::io::Read;

    let mut reader: &std::fs::File = master;
    let mut text = String::new();
    let mut buffer = [0_u8; 256];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => text.push_str(&String::from_utf8_lossy(&buffer[..read])),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(_) => break,
        }
    }
    text
}

/// Run `sh -c command` with no sandbox and no session change, taking `stdout` as
/// the command's own terminal. The positive control for a terminal-slave write
/// must not go through `bounded_process`: that starts a new session, and a
/// session leader that opens a fresh tty blocks, so the write is measured
/// without the session change.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn run_unwrapped(root: &Path, command: &str, stdout: &std::fs::File) -> std::process::Output {
    Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(dup_stdio(stdout))
        .stderr(Stdio::piped())
        .output()
        .expect("spawn unwrapped command")
}

/// Run `sh -c command` through the installed sandbox, taking `stdout` as the
/// command's own terminal. Standard I/O is set after wrapping because the macOS
/// backend rewrites the command and a `Stdio` set beforehand would not survive.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn run_wrapped_with_stdout(
    root: &Path,
    command: &str,
    stdout: &std::fs::File,
) -> BoundedProcessOutput {
    let mut process = Command::new("sh");
    process.arg("-c").arg(command).current_dir(root);
    let mut process = os_sandbox::wrap_command(process).expect("available sandbox wraps");
    process
        .stdin(Stdio::null())
        .stdout(dup_stdio(stdout))
        .stderr(Stdio::piped());
    bounded_process::run_with_timeout(&mut process, TIMEOUT).expect("spawn wrapped command")
}

/// Issue #667: every bounded child is moved into its own session by `setsid()`,
/// so it keeps a process group whose id equals its pid (termination by `-pid`
/// still works) and has no controlling terminal to inject into. The child's
/// session must differ from the test process's session.
#[cfg(unix)]
#[test]
fn bounded_child_runs_in_its_own_session_and_process_group() {
    let mut command = Command::new("python3");
    command
        .arg("-c")
        .arg("import os; print(os.getsid(0), os.getpgrp(), os.getpid())")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = bounded_process::run_with_timeout(&mut command, TIMEOUT).expect("spawn child");
    assert!(output.success(), "{output:?}");
    let text = String::from_utf8_lossy(&output.stdout);
    let mut parts = text.split_whitespace();
    let sid: i32 = parts.next().expect("sid").parse().expect("sid number");
    let pgrp: i32 = parts.next().expect("pgrp").parse().expect("pgrp number");
    let pid: i32 = parts.next().expect("pid").parse().expect("pid number");
    assert_eq!(pgrp, pid, "the child's group id must equal its pid: {text}");
    assert_eq!(sid, pid, "the child must lead its own session: {text}");
    let caller_sid = unsafe { libc::getsid(0) };
    assert_ne!(
        sid, caller_sid,
        "the child must not share the caller's session: {text}"
    );
}

/// Issue #667: an isolated command cannot inject input into the controlling
/// terminal with `TIOCSTI`. This runs as its own process because it must become
/// a session leader and own a pty; that must not touch the shared test process,
/// and it only ever uses the pty the probe created. Injection is judged by
/// whether the injected byte appears on the pty's master side. When the
/// environment cannot exercise `TIOCSTI` (a kernel with `legacy_tiocsti=0`, or no
/// controlling terminal) the probe reports `UNVERIFIED (reason)` and records a
/// line for the run summary instead of passing silently.
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn issue667_tiocsti_session_probe() {
    if std::env::var_os("COMMANDAGENT_ISSUE667_TIOCSTI_PROBE").is_none() {
        return;
    }
    let (master, slave, _slave_path) = open_pty();
    assert_ne!(
        unsafe { libc::setsid() },
        -1,
        "setsid: {}",
        std::io::Error::last_os_error()
    );
    assert_eq!(
        unsafe { libc::ioctl(slave.as_raw_fd(), TIOCSCTTY_REQUEST, 0) },
        0,
        "TIOCSCTTY: {}",
        std::io::Error::last_os_error()
    );
    // The probe now owns a controlling terminal; the kernel sends SIGHUP to the
    // session leader when it exits, so ignore it and finish normally.
    unsafe {
        libc::signal(libc::SIGHUP, libc::SIG_IGN);
    }
    // Echo injected bytes back to the master, so the master side is the evidence
    // that an injection happened.
    let mut termios: libc::termios = unsafe { std::mem::zeroed() };
    assert_eq!(
        unsafe { libc::tcgetattr(slave.as_raw_fd(), &mut termios) },
        0,
        "tcgetattr: {}",
        std::io::Error::last_os_error()
    );
    termios.c_lflag |= libc::ECHO;
    assert_eq!(
        unsafe { libc::tcsetattr(slave.as_raw_fd(), libc::TCSANOW, &termios) },
        0,
        "tcsetattr: {}",
        std::io::Error::last_os_error()
    );

    // Positive control: a plain child in this session inherits the pty as its
    // controlling terminal, so the kernel allows `TIOCSTI` and the injected 'Z'
    // must echo onto the master. If it does not, the environment cannot exercise
    // `TIOCSTI` and the denial cannot be measured.
    let positive_out = run_tiocsti_attempt(&slave, 'Z', false);
    let positive_echo = drain_master(&master);
    if !positive_out.contains("TIOCSTI_OK") || !positive_echo.contains('Z') {
        record_unverified(&format!(
            "the positive control could not inject (child said {:?}, master echoed {:?})",
            positive_out.trim(),
            positive_echo
        ));
        return;
    }

    // Negative control: the same attempt through the bounded-process launcher,
    // which puts the child in its own session with no controlling terminal. Its
    // injected 'Y' must not reach the master.
    let negative_out = run_tiocsti_attempt(&slave, 'Y', true);
    let negative_echo = drain_master(&master);
    assert!(
        !negative_echo.contains('Y'),
        "an isolated child injected into the controlling terminal: master echoed {negative_echo:?}"
    );
    assert!(
        negative_out.contains("TIOCSTI_FAIL"),
        "TIOCSTI must fail without a controlling terminal: {negative_out:?}"
    );
    record_tiocsti_step_summary("blocked");
    println!(
        "issue667-tiocsti: BLOCKED (the positive control injected 'Z'; the isolated child could not inject 'Y')"
    );
}

/// Run one `TIOCSTI` attempt with fd 0 as `slave` and return the child's stdout.
/// A plain attempt keeps the probe's session and its controlling terminal; a
/// wrapped attempt goes through `bounded_process`, which `setsid()`s the child.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn run_tiocsti_attempt(slave: &std::fs::File, inject: char, wrapped: bool) -> String {
    let script = tiocsti_script(inject);
    let mut command = Command::new("python3");
    command
        .arg("-c")
        .arg(script)
        .stdin(dup_stdio(slave))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if wrapped {
        let output = bounded_process::run_with_timeout(&mut command, TIMEOUT)
            .expect("run the isolated attempt");
        String::from_utf8_lossy(&output.stdout).into_owned()
    } else {
        let output = command.output().expect("run the plain attempt");
        String::from_utf8_lossy(&output.stdout).into_owned()
    }
}

/// Report an unmeasurable `TIOCSTI` probe on stdout, like the other probes, so
/// the outer test can find the verdict instead of reading it as missing; the
/// `$GITHUB_STEP_SUMMARY` line still records it, so it is not a silent pass.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn record_unverified(reason: &str) {
    println!("issue667-tiocsti: UNVERIFIED ({reason})");
    record_tiocsti_step_summary(&format!("unverified: {reason}"));
}

/// Append the `TIOCSTI` verdict to `$GITHUB_STEP_SUMMARY` when it is set, with
/// the kernel's `legacy_tiocsti` value when the file exists, so a runner can tell
/// a measured denial from an unverifiable environment.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn record_tiocsti_step_summary(status: &str) {
    let legacy = std::fs::read_to_string("/proc/sys/dev/tty/legacy_tiocsti")
        .map(|value| value.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let Some(path) = std::env::var_os("GITHUB_STEP_SUMMARY") else {
        return;
    };
    let line = format!("issue667/tiocsti: {status}; legacy_tiocsti={legacy}\n");
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| std::io::Write::write_all(&mut file, line.as_bytes()));
}

/// Runs [`issue667_tiocsti_session_probe`] in a fresh process and asserts it
/// reaches a verdict. The probe prints `BLOCKED` when it demonstrated the
/// injection and its denial, or `UNVERIFIED (reason)` when the environment
/// cannot exercise `TIOCSTI`; either verdict records a `$GITHUB_STEP_SUMMARY`
/// line, so an unmeasurable environment is never a silent pass.
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn isolated_child_cannot_inject_into_the_controlling_terminal() {
    let exe = std::env::current_exe().expect("current exe");
    let output = Command::new(exe)
        .args([
            "--exact",
            "issue667_tiocsti_session_probe",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("COMMANDAGENT_ISSUE667_TIOCSTI_PROBE", "1")
        .output()
        .expect("run the tiocsti probe as a subprocess");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "the tiocsti probe failed:\n{stdout}\n{stderr}"
    );
    let verdict = stdout
        .lines()
        .find(|line| line.contains("issue667-tiocsti:"))
        .unwrap_or_else(|| panic!("the tiocsti probe reported no verdict:\n{stdout}\n{stderr}"));
    eprintln!("{verdict}");
}

/// Issue #667: with `setsid()` the child has no controlling terminal, so a
/// terminal standard input would become readable (the job-control check no
/// longer applies). The launcher must replace a terminal stdin with `/dev/null`
/// for every bounded child, isolated and unisolated alike. The probe makes its
/// own pty the session's controlling terminal and puts it on fd 0, then starts a
/// child with stdin left unset so it would inherit that terminal.
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn issue667_stdin_probe() {
    if std::env::var_os("COMMANDAGENT_ISSUE667_STDIN_PROBE").is_none() {
        return;
    }
    let (_master, slave, _slave_path) = open_pty();
    assert_ne!(
        unsafe { libc::setsid() },
        -1,
        "setsid: {}",
        std::io::Error::last_os_error()
    );
    assert_eq!(
        unsafe { libc::ioctl(slave.as_raw_fd(), TIOCSCTTY_REQUEST, 0) },
        0,
        "TIOCSCTTY: {}",
        std::io::Error::last_os_error()
    );
    unsafe {
        libc::signal(libc::SIGHUP, libc::SIG_IGN);
    }
    // Make the pty the probe's own standard input, so a child that leaves stdin
    // unset inherits a terminal and the launcher has to detach it.
    assert_ne!(
        unsafe { libc::dup2(slave.as_raw_fd(), 0) },
        -1,
        "dup2: {}",
        std::io::Error::last_os_error()
    );

    // The unisolated launch.
    assert_stdin_is_detached(&run_stdin_child(false));

    // The isolated launch, when the backend is available.
    if matches!(os_sandbox::probe(), SandboxAvailability::Available) {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = dir.path().join("ws");
        let run_tmp = dir.path().join("tmp");
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::create_dir_all(&run_tmp).unwrap();
        os_sandbox::install_for_test(
            SandboxMode::Auto,
            SandboxPolicy::new(&workspace, &run_tmp),
            SandboxAvailability::Available,
        );
        let sandboxed = run_stdin_child(true);
        os_sandbox::reset_for_test();
        assert_stdin_is_detached(&sandboxed);
        println!(
            "issue667-stdin: BLOCKED (both the isolated and unisolated launches detach a terminal stdin)"
        );
    } else {
        println!(
            "issue667-stdin: BLOCKED (the unisolated launch detaches a terminal stdin; the sandbox backend is unavailable)"
        );
    }
}

/// A child that reports whether fd 0 is a terminal and whether a read returns
/// EOF at once. It ignores `SIGTTIN` and sets fd 0 non-blocking so a terminal
/// stdin (the pre-fix behavior) reports `READ_ERR` instead of stopping.
#[cfg(any(target_os = "linux", target_os = "macos"))]
const STDIN_PROBE_SCRIPT: &str = r#"import fcntl, os, signal, sys
try:
    signal.signal(signal.SIGTTIN, signal.SIG_IGN)
except (ValueError, OSError):
    pass
sys.stdout.write('ISATTY %d\n' % (1 if os.isatty(0) else 0))
sys.stdout.flush()
try:
    flags = fcntl.fcntl(0, fcntl.F_GETFL)
    fcntl.fcntl(0, fcntl.F_SETFL, flags | os.O_NONBLOCK)
except OSError:
    pass
try:
    sys.stdout.write('READ %d\n' % len(os.read(0, 1)))
except OSError as error:
    sys.stdout.write('READ_ERR %d\n' % error.errno)
"#;

/// Run a child through `bounded_process` with standard input left unset, so it
/// inherits the probe's pty, and return its stdout. The wrapped form goes through
/// the installed sandbox. Standard I/O is set after wrapping because the macOS
/// backend rewrites the command.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn run_stdin_child(wrapped: bool) -> String {
    let mut command = Command::new("python3");
    command.arg("-c").arg(STDIN_PROBE_SCRIPT);
    let mut command = if wrapped {
        os_sandbox::wrap_command(command).expect("available sandbox wraps")
    } else {
        command
    };
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let output =
        bounded_process::run_with_timeout(&mut command, TIMEOUT).expect("run the stdin child");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Assert the child reports a non-terminal stdin that reads EOF at once.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn assert_stdin_is_detached(output: &str) {
    assert!(
        output.contains("ISATTY 0"),
        "the child's stdin must not be a terminal: {output:?}"
    );
    assert!(
        output.contains("READ 0"),
        "a detached stdin must read EOF immediately: {output:?}"
    );
}

/// Runs [`issue667_stdin_probe`] in a fresh process and asserts it reaches a
/// verdict.
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn isolated_child_stdin_is_detached_from_the_terminal() {
    let exe = std::env::current_exe().expect("current exe");
    let output = Command::new(exe)
        .args([
            "--exact",
            "issue667_stdin_probe",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("COMMANDAGENT_ISSUE667_STDIN_PROBE", "1")
        .output()
        .expect("run the stdin probe as a subprocess");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "the stdin probe failed:\n{stdout}\n{stderr}"
    );
    let verdict = stdout
        .lines()
        .find(|line| line.contains("issue667-stdin:"))
        .unwrap_or_else(|| panic!("the stdin probe reported no verdict:\n{stdout}\n{stderr}"));
    eprintln!("{verdict}");
}

/// Issue #667: to replace a terminal stdin the launcher opens `/dev/null` in the
/// freshly forked child and duplicates it onto fd 0. The descriptor it opened
/// must be closed before `exec`, otherwise the exec'd command inherits an extra
/// descriptor pointing at `/dev/null`. The probe makes its own pty the session's
/// controlling terminal and puts it on fd 0, then starts a child with stdin left
/// unset so it would inherit that terminal, and the child reports every
/// descriptor above the standard three whose device is `/dev/null`.
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn issue667_devnull_fd_probe() {
    if std::env::var_os("COMMANDAGENT_ISSUE667_DEVNULL_FD_PROBE").is_none() {
        return;
    }
    let (_master, slave, _slave_path) = open_pty();
    assert_ne!(
        unsafe { libc::setsid() },
        -1,
        "setsid: {}",
        std::io::Error::last_os_error()
    );
    assert_eq!(
        unsafe { libc::ioctl(slave.as_raw_fd(), TIOCSCTTY_REQUEST, 0) },
        0,
        "TIOCSCTTY: {}",
        std::io::Error::last_os_error()
    );
    unsafe {
        libc::signal(libc::SIGHUP, libc::SIG_IGN);
    }
    // Make the pty the probe's own standard input, so a child that leaves stdin
    // unset inherits a terminal and the launcher opens `/dev/null` for it.
    assert_ne!(
        unsafe { libc::dup2(slave.as_raw_fd(), 0) },
        -1,
        "dup2: {}",
        std::io::Error::last_os_error()
    );

    // Positive control: the scanner must report a `/dev/null` descriptor the
    // child itself opens, so a clean scan below is evidence rather than a blind
    // spot.
    let leaked = run_devnull_fd_child(false, true);
    assert!(
        leaked.contains("DEVNULL_FDS") && !leaked.contains("DEVNULL_FDS none"),
        "the scanner must detect an extra /dev/null descriptor: {leaked:?}"
    );

    // The unisolated launch.
    assert_no_devnull_descriptor(&run_devnull_fd_child(false, false));

    // The isolated launch, when the backend is available.
    if matches!(os_sandbox::probe(), SandboxAvailability::Available) {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = dir.path().join("ws");
        let run_tmp = dir.path().join("tmp");
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::create_dir_all(&run_tmp).unwrap();
        os_sandbox::install_for_test(
            SandboxMode::Auto,
            SandboxPolicy::new(&workspace, &run_tmp),
            SandboxAvailability::Available,
        );
        let sandboxed = run_devnull_fd_child(true, false);
        os_sandbox::reset_for_test();
        assert_no_devnull_descriptor(&sandboxed);
        println!(
            "issue667-devnull-fd: BLOCKED (both the isolated and unisolated launches close the /dev/null descriptor)"
        );
    } else {
        println!(
            "issue667-devnull-fd: BLOCKED (the unisolated launch closes the /dev/null descriptor; the sandbox backend is unavailable)"
        );
    }
}

/// A child that reports every descriptor above the standard three whose device
/// matches `/dev/null`, so a descriptor the launcher left open is visible from
/// inside the exec'd program. `leak` additionally opens `/dev/null` in the child
/// as a positive control that the scan can see a retained descriptor.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn devnull_fd_script(leak: bool) -> String {
    let leak_line = if leak {
        "os.open('/dev/null', os.O_RDONLY)\n"
    } else {
        ""
    };
    format!(
        r#"import os, stat, sys
{leak_line}null_rdev = os.stat('/dev/null').st_rdev
hits = []
for fd in range(3, 256):
    try:
        info = os.fstat(fd)
    except OSError:
        continue
    if stat.S_ISCHR(info.st_mode) and info.st_rdev == null_rdev:
        hits.append(fd)
sys.stdout.write('DEVNULL_FDS %s\n' % (','.join(map(str, hits)) if hits else 'none'))
"#
    )
}

/// Run a child through `bounded_process` with standard input left unset, so it
/// inherits the probe's pty, and return its stdout. The wrapped form goes through
/// the installed sandbox. Standard I/O is set after wrapping because the macOS
/// backend rewrites the command.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn run_devnull_fd_child(wrapped: bool, leak: bool) -> String {
    let mut command = Command::new("python3");
    command.arg("-c").arg(devnull_fd_script(leak));
    let mut command = if wrapped {
        os_sandbox::wrap_command(command).expect("available sandbox wraps")
    } else {
        command
    };
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let output =
        bounded_process::run_with_timeout(&mut command, TIMEOUT).expect("run the devnull-fd child");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Assert the child reports no descriptor above the standard three whose device
/// is `/dev/null`.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn assert_no_devnull_descriptor(output: &str) {
    assert!(
        output.contains("DEVNULL_FDS none"),
        "no descriptor above the standard three may point at /dev/null: {output:?}"
    );
}

/// Runs [`issue667_devnull_fd_probe`] in a fresh process and asserts it reaches a
/// verdict.
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn isolated_child_closes_the_devnull_descriptor_for_a_terminal_stdin() {
    let exe = std::env::current_exe().expect("current exe");
    let output = Command::new(exe)
        .args([
            "--exact",
            "issue667_devnull_fd_probe",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("COMMANDAGENT_ISSUE667_DEVNULL_FD_PROBE", "1")
        .output()
        .expect("run the devnull-fd probe as a subprocess");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "the devnull-fd probe failed:\n{stdout}\n{stderr}"
    );
    let verdict = stdout
        .lines()
        .find(|line| line.contains("issue667-devnull-fd:"))
        .unwrap_or_else(|| panic!("the devnull-fd probe reported no verdict:\n{stdout}\n{stderr}"));
    eprintln!("{verdict}");
}

/// Issue #667: a confined command runs in its own session with no controlling
/// terminal, but it can still re-open the run's terminal by path (`/dev/pts/N`
/// on Linux, `/dev/ttys*` on macOS) and read the keys the user types. The read
/// layer must refuse that open. The probe makes its own pty the session's
/// controlling terminal, writes one line to the master as the "user", then
/// starts a child through `bounded_process` that re-opens the slave by path and
/// reads it. `reopen` opens it directly; `setsid_reopen` forks a grandchild that
/// `setsid()`s first, which is the hole that also existed before this change.
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn issue667_terminal_read_probe() {
    if std::env::var_os("COMMANDAGENT_ISSUE667_TERMINAL_READ_PROBE").is_none() {
        return;
    }
    let (mut master, slave, slave_path) = open_pty();
    assert_ne!(
        unsafe { libc::setsid() },
        -1,
        "setsid: {}",
        std::io::Error::last_os_error()
    );
    assert_eq!(
        unsafe { libc::ioctl(slave.as_raw_fd(), TIOCSCTTY_REQUEST, 0) },
        0,
        "TIOCSCTTY: {}",
        std::io::Error::last_os_error()
    );
    unsafe {
        libc::signal(libc::SIGHUP, libc::SIG_IGN);
    }

    // Positive control: without the sandbox the child re-opens the run's
    // terminal by path and reads the line, so a denial below is the sandbox.
    for mode in ["reopen", "setsid_reopen"] {
        let output = run_terminal_read_child(&mut master, &slave, &slave_path, false, mode);
        assert!(
            output.contains(&format!("RESULT {mode} READ"))
                && output.contains(&terminal_read_token(mode, false)),
            "the unisolated child must read the run's terminal by path ({mode}): {output:?}"
        );
    }

    // The isolated launch, when the backend is available.
    if matches!(os_sandbox::probe(), SandboxAvailability::Available) {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = dir.path().join("ws");
        let run_tmp = dir.path().join("tmp");
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::create_dir_all(&run_tmp).unwrap();
        os_sandbox::install_for_test(
            SandboxMode::Auto,
            SandboxPolicy::new(&workspace, &run_tmp),
            SandboxAvailability::Available,
        );
        for mode in ["reopen", "setsid_reopen"] {
            let output = run_terminal_read_child(&mut master, &slave, &slave_path, true, mode);
            assert!(
                output.contains(&format!("RESULT {mode} ERR")),
                "the isolated child must not open the run's terminal by path ({mode}): {output:?}"
            );
            assert!(
                !output.contains(&terminal_read_token(mode, true)),
                "no typed key may reach the isolated child ({mode}): {output:?}"
            );
        }
        os_sandbox::reset_for_test();
        record_terminal_read_step_summary("blocked");
        println!(
            "issue667-terminal-read: BLOCKED (the isolated child could not re-open the terminal by path; the unisolated control read it)"
        );
    } else {
        record_terminal_read_step_summary("unverified: the sandbox backend is unavailable");
        println!(
            "issue667-terminal-read: UNVERIFIED (the sandbox backend is unavailable; only the unisolated control was measured)"
        );
    }
}

/// A Python child that re-opens the pty at `path` by name and reads it: `reopen`
/// opens it directly, `setsid_reopen` forks a grandchild that `setsid()`s first.
/// It reports `RESULT <mode> READ <bytes>`, `RESULT <mode> NOTHING`, or `RESULT
/// <mode> ERR <errno>`, so the caller can tell a read from a denied open.
#[cfg(any(target_os = "linux", target_os = "macos"))]
const TERMINAL_READ_SCRIPT: &str = r#"import os, select, signal, sys
signal.signal(signal.SIGTTIN, signal.SIG_IGN)
mode, path = sys.argv[1], sys.argv[2]
if mode == 'setsid_reopen':
    pid = os.fork()
    if pid:
        os.waitpid(pid, 0)
        sys.exit(0)
    os.setsid()
try:
    fd = os.open(path, os.O_RDONLY | os.O_NOCTTY | os.O_NONBLOCK)
    ready, _, _ = select.select([fd], [], [], 2.0)
    if ready:
        sys.stdout.write('RESULT %s READ %r\n' % (mode, os.read(fd, 128)))
    else:
        sys.stdout.write('RESULT %s NOTHING\n' % mode)
except OSError as error:
    sys.stdout.write('RESULT %s ERR %d\n' % (mode, error.errno))
sys.stdout.flush()
"#;

/// Run a child that re-opens `slave_path` after `master` receives one typed
/// line, optionally through the installed sandbox, and return its output. The
/// queued input is flushed afterwards so the next mode starts clean.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn run_terminal_read_child(
    master: &mut std::fs::File,
    slave: &std::fs::File,
    slave_path: &Path,
    sandbox: bool,
    mode: &str,
) -> String {
    use std::io::Write;

    let mut command = Command::new("python3");
    command
        .arg("-c")
        .arg(TERMINAL_READ_SCRIPT)
        .arg(mode)
        .arg(slave_path);
    let mut command = if sandbox {
        os_sandbox::wrap_command(command).expect("available sandbox wraps")
    } else {
        command
    };
    // Standard I/O is set after wrapping because the macOS backend rewrites the
    // command (same as `run_stdin_child`).
    command
        .stdin(dup_stdio(slave))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // The "user" types one line on the run's terminal while the child runs.
    master
        .write_all(format!("{}\n", terminal_read_token(mode, sandbox)).as_bytes())
        .expect("write the typed line");
    master.flush().expect("flush the typed line");
    let output = bounded_process::run_with_timeout(&mut command, TIMEOUT)
        .expect("run the terminal-read child");
    // Drop anything the child left queued so a previous mode's line is not read
    // by the next one.
    unsafe {
        libc::tcflush(slave.as_raw_fd(), libc::TCIFLUSH);
    }
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// The one line the "user" types for a mode, so the caller can tell whether the
/// child actually received it.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn terminal_read_token(mode: &str, sandbox: bool) -> String {
    format!("KEY_{mode}_{sandbox}")
}

/// Append the terminal-read verdict to `$GITHUB_STEP_SUMMARY` when it is set, so
/// a run that could not measure the denial records why instead of passing
/// silently.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn record_terminal_read_step_summary(status: &str) {
    let Some(path) = std::env::var_os("GITHUB_STEP_SUMMARY") else {
        return;
    };
    let line = format!("issue667/terminal-read: {status}\n");
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| std::io::Write::write_all(&mut file, line.as_bytes()));
}

/// Runs [`issue667_terminal_read_probe`] in a fresh process and asserts it
/// reaches a verdict. A run without the sandbox backend reports `UNVERIFIED`
/// and records a `$GITHUB_STEP_SUMMARY` line, so it is never a silent pass.
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn isolated_child_cannot_read_the_terminal_by_path() {
    let exe = std::env::current_exe().expect("current exe");
    let output = Command::new(exe)
        .args([
            "--exact",
            "issue667_terminal_read_probe",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("COMMANDAGENT_ISSUE667_TERMINAL_READ_PROBE", "1")
        .output()
        .expect("run the terminal-read probe as a subprocess");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "the terminal-read probe failed:\n{stdout}\n{stderr}"
    );
    let verdict = stdout
        .lines()
        .find(|line| line.contains("issue667-terminal-read:"))
        .unwrap_or_else(|| {
            panic!("the terminal-read probe reported no verdict:\n{stdout}\n{stderr}")
        });
    eprintln!("{verdict}");
}

/// Issue #667: a confined command keeps its own terminal (its inherited stdio)
/// but cannot open another terminal's slave for writing. Both ptys are created
/// by the test — the command's own terminal is one and the "other terminal" is
/// the second — so no real terminal is ever touched.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn assert_terminal_slave_write_is_denied(fixture: &Fixture) {
    let (_own_master, own_slave, _own_path) = open_pty();
    let (_other_master, _other_slave, other_path) = open_pty();
    let command = format!("echo written > '{}'", other_path.display());

    // Positive control: without a sandbox the same write succeeds, so the
    // fixture proves the sandbox is what denies it.
    os_sandbox::reset_for_test();
    let before = run_unwrapped(&fixture.root, &command, &own_slave);
    assert!(
        before.status.success(),
        "a write to a terminal slave must succeed without the sandbox: {}",
        String::from_utf8_lossy(&before.stderr)
    );

    install_available(fixture);
    let after = run_wrapped_with_stdout(&fixture.root, &command, &own_slave);
    assert_eq!(
        after.kind,
        BoundedProcessOutcomeKind::Exited,
        "the denied write must fail, not hang or time out: {after:?}"
    );
    assert!(
        !after.success(),
        "a write to another terminal's slave must be denied: {}",
        String::from_utf8_lossy(&after.stderr)
    );
    let denial = String::from_utf8_lossy(&after.stderr);
    assert!(
        denial.contains("Permission denied") || denial.contains("Operation not permitted"),
        "the denial must be a permission error, not a hang: {denial}"
    );
    os_sandbox::reset_for_test();
}

/// Issue #667 on Linux: `/dev/pts` is no longer in the Landlock write allow
/// list, so an `open` for writing to another terminal's slave is refused.
#[cfg(target_os = "linux")]
#[test]
fn linux_applied_terminal_slave_write_is_denied() {
    let _guard = sandbox_lock();
    if !require_landlock_backend() {
        eprintln!("SKIP: Landlock backend unavailable");
        return;
    }
    let fixture = fixture();
    assert_terminal_slave_write_is_denied(&fixture);
}

/// Issue #667 on macOS: the write profile allows `/dev` and then re-denies
/// `/dev/tty*` last, which covers the `/dev/ttys*` pty slaves, so writing to
/// another terminal is refused.
#[cfg(target_os = "macos")]
#[test]
fn applied_terminal_slave_write_is_denied() {
    let _guard = sandbox_lock();
    if !backend_available() {
        eprintln!("SKIP: sandbox backend unavailable (nested sandbox?)");
        return;
    }
    let fixture = fixture();
    assert_terminal_slave_write_is_denied(&fixture);
}

/// Issue #673: a `git` command that cannot reach the user's HOME, global Git
/// configuration, system configuration, or template/hook directory. The host
/// pager test uses this instead of the ambient `git`, so a developer's
/// `~/.gitconfig` (hooks, `init.templateDir`, signing, pager) never applies.
fn isolated_git_command(home: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .env("HOME", home)
        .env("GIT_CONFIG_GLOBAL", home.join("gitconfig"))
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("XDG_CONFIG_HOME", home.join("xdg"))
        .env("GIT_TERMINAL_PROMPT", "0")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_COMMON_DIR")
        .env_remove("GIT_TEMPLATE_DIR")
        .env_remove("GIT_ALTERNATE_OBJECT_DIRECTORIES");
    command
}

/// A secret-free HOME for the host git commands, inside the test tempdir.
fn isolated_git_home(fixture: &Fixture) -> PathBuf {
    let home = fixture._dir.path().join("iso-git-home");
    std::fs::create_dir_all(&home).unwrap();
    home
}

/// Issue #667: a command that needs a terminal (a `git` pager) still works.
/// `git` runs its pager only when stdout is a tty, so the fixture gives the
/// wrapped command the pty slave as stdout; the pager (`cat > pager.out`) then
/// runs inside the sandbox and writes the allowed workspace. This is the
/// behavior SECURITY.md keeps: stdio still works, only opening `/dev/tty`
/// directly changed.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn assert_git_pager_runs_inside_sandbox(fixture: &Fixture) {
    let (_master, slave, _slave_path) = open_pty();
    let iso_home = isolated_git_home(fixture);
    isolated_git_command(&iso_home)
        .args(["init", "-q"])
        .current_dir(&fixture.root)
        .status()
        .expect("git init");
    isolated_git_command(&iso_home)
        .args([
            "-c",
            "user.email=probe@example.invalid",
            "-c",
            "user.name=probe",
            "commit",
            "--allow-empty",
            "-qm",
            "issue667 pager probe",
        ])
        .current_dir(&fixture.root)
        .status()
        .expect("git commit");

    let mut process = Command::new("sh");
    process
        .arg("-c")
        .arg("git -c core.pager='cat > pager.out' --paginate log --oneline")
        .current_dir(&fixture.root)
        // The bounded child drops every `GIT_*`, but passes `HOME`; set it to
        // the isolated HOME so the sandboxed `git` cannot read the user's
        // global Git configuration either (Issue #673).
        .env("HOME", &iso_home);
    let mut process = os_sandbox::wrap_command(process).expect("available sandbox wraps");
    // Standard I/O is set after wrapping: the macOS backend rewrites the
    // command, so a `Stdio` set beforehand would not survive the transfer.
    process
        .stdin(Stdio::null())
        .stdout(dup_stdio(&slave))
        .stderr(dup_stdio(&slave));
    let output =
        bounded_process::run_with_timeout(&mut process, TIMEOUT).expect("run the pager command");
    assert!(
        output.success(),
        "the pager command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let pager_out = std::fs::read_to_string(fixture.root.join("pager.out")).unwrap_or_default();
    assert!(
        pager_out.contains("issue667 pager probe"),
        "git's pager did not run inside the sandbox: {pager_out:?}"
    );
}

/// Issue #673: the host git work does not reach the user's HOME or Git
/// configuration. It inspects the actual child environment (a bounded child
/// sees the isolated HOME and none of the outer `GIT_*`), and reads back a value
/// the isolated helper wrote, proving the helper uses the isolated config.
#[cfg(unix)]
#[test]
fn host_git_child_environment_is_isolated_from_the_user() {
    let fixture = fixture();
    let iso_home = isolated_git_home(&fixture);

    let mut process = Command::new("sh");
    process
        .arg("-c")
        .arg("printf '%s\\n' \"$HOME\" \"${GIT_CONFIG_GLOBAL-unset}\" \"${GIT_DIR-unset}\"")
        .current_dir(&fixture.root)
        .env("HOME", &iso_home)
        .env(
            "GIT_CONFIG_GLOBAL",
            "/nonexistent/issue673-canary-gitconfig",
        )
        .env("GIT_DIR", "/nonexistent/issue673-canary-git-dir");
    process.stdout(Stdio::piped()).stderr(Stdio::piped());
    let output = bounded_process::run_with_timeout(&mut process, TIMEOUT).expect("run child");
    let text = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines.first().copied(),
        Some(iso_home.to_string_lossy().as_ref()),
        "the child must see the isolated HOME: {text:?}"
    );
    assert_eq!(
        lines.get(1).copied(),
        Some("unset"),
        "GIT_CONFIG_GLOBAL must be dropped by the bounded child allowlist: {text:?}"
    );
    assert_eq!(
        lines.get(2).copied(),
        Some("unset"),
        "GIT_DIR must be dropped by the bounded child allowlist: {text:?}"
    );

    std::fs::write(
        iso_home.join("gitconfig"),
        "[issue673]\n\tmarker = isolated\n",
    )
    .unwrap();
    let out = isolated_git_command(&iso_home)
        .args(["config", "--global", "--get", "issue673.marker"])
        .output()
        .expect("run git config");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "isolated",
        "the isolated helper must read the isolated global config"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn linux_applied_git_pager_still_runs_inside_the_sandbox() {
    let _guard = sandbox_lock();
    if !require_landlock_backend() {
        eprintln!("SKIP: Landlock backend unavailable");
        return;
    }
    let fixture = fixture();
    install_available(&fixture);
    assert_git_pager_runs_inside_sandbox(&fixture);
    os_sandbox::reset_for_test();
}

#[cfg(target_os = "macos")]
#[test]
fn applied_git_pager_still_runs_inside_the_sandbox() {
    let _guard = sandbox_lock();
    if !backend_available() {
        eprintln!("SKIP: sandbox backend unavailable (nested sandbox?)");
        return;
    }
    let fixture = fixture();
    install_available(&fixture);
    assert_git_pager_runs_inside_sandbox(&fixture);
    os_sandbox::reset_for_test();
}
