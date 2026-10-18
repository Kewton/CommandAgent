#![cfg(unix)]

//! Issue #657 (stage 2 of #502): OS read confinement for model Bash and
//! verifier commands. The lexical guard cannot see a credential read whose name
//! does not appear literally (`cat notes.txt` through a symlink, `cat
//! "$BASH_ENV"`, or a name an interpreter assembles at run time). This file
//! drives the macOS Seatbelt and Linux Landlock read layers through the same
//! `wrap_command` the two launch points use, so those indirect reads fail. The
//! corpus table `indirect-cases.jsonl` is the machine-checked part.
//!
//! Every fixture is built inside one test tempdir: the fake home, the canary
//! "key", and the symlinks never touch the real home or `~/.ssh`. The home is
//! passed to the backend as a policy argument (`SandboxPolicy::with_home`), so
//! no test mutates the process `HOME` and parallel tests cannot race on it. The
//! workspace root is deliberately left un-canonicalized (a system temp dir under
//! `/var/folders/...` on macOS), so the macOS cases prove the regexes canonicalize
//! it.
//!
//! The macOS applied cases are `#[cfg(target_os = "macos")]` and no-op when the
//! backend is unavailable (a nested sandbox reports rc=71). The Linux applied
//! cases are `#[cfg(target_os = "linux")]` and fail under CI when Landlock is
//! unavailable, so the runner cannot silently lose the read backend. The kernel
//! and Landlock ABI are printed for the record.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use commandagent::bounded_process::{self, BoundedProcessOutput};
#[cfg(target_os = "linux")]
use commandagent::tools::bash::path_confinement_rejection;
use commandagent::tools::os_sandbox::{self, SandboxAvailability, SandboxMode, SandboxPolicy};
use commandagent::tools::sensitive_path::command_references_secret;
use serde::Deserialize;

/// The fake home `~/.ssh/id_ed25519` canary. Never a real key.
const HOME_KEY_CANARY: &str = "HOME_KEY_CANARY_657";
/// The fake home `~/.config/gh/hosts.yml` canary.
const GH_CANARY: &str = "HOME_GH_CANARY_657";
/// The content of every fake workspace credential (never a real secret).
const WS_CANARY: &str = "WORKSPACE_SECRET_CANARY_657";
/// The content of a readable dotenv template.
const TEMPLATE_OK: &str = "TEMPLATE_OK_657";
/// The nested-sandbox reason used to simulate an unavailable backend.
const NESTED: &str = "sandbox-exec probe exited with exit status: 71";
const TIMEOUT: Duration = Duration::from_secs(120);

/// The sandbox state is process-wide, so the tests that install it serialize.
static SANDBOX_LOCK: Mutex<()> = Mutex::new(());

fn sandbox_lock() -> MutexGuard<'static, ()> {
    SANDBOX_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A workspace (`ws`) and a sibling fake home (`home`) inside one un-canonical
/// test tempdir. The home holds fake key material and the workspace holds fake
/// credentials in the shapes #501 denies: a plain `.env`, mixed-case names, a
/// credential-named directory with a child, a template below a credential
/// directory, the path-sequence credentials, and non-secret symlink aliases.
struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
    home: PathBuf,
    ssh_secret: PathBuf,
    gh_secret: PathBuf,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().expect("create tempdir");
    let ws = dir.path().join("ws");
    let home = dir.path().join("home");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::create_dir_all(home.join(".ssh")).unwrap();
    std::fs::create_dir_all(home.join(".config/gh")).unwrap();
    std::fs::create_dir_all(home.join(".config/other")).unwrap();
    std::fs::write(home.join(".ssh/id_ed25519"), format!("{HOME_KEY_CANARY}\n")).unwrap();
    std::fs::write(home.join(".config/gh/hosts.yml"), format!("{GH_CANARY}\n")).unwrap();
    std::fs::write(home.join(".config/other/settings"), "OTHER_OK\n").unwrap();
    std::fs::write(ws.join("a.txt"), "x\n").unwrap();

    // Workspace credentials, in the shapes #501 denies.
    std::fs::write(ws.join(".env"), format!("{WS_CANARY}\n")).unwrap();
    std::fs::write(ws.join(".Env.local"), format!("{WS_CANARY}\n")).unwrap();
    std::fs::write(ws.join("SERVICE-ACCOUNT.json"), format!("{WS_CANARY}\n")).unwrap();
    std::fs::create_dir_all(ws.join(".SSH")).unwrap();
    std::fs::write(ws.join(".SSH/id_rsa"), format!("{WS_CANARY}\n")).unwrap();
    std::fs::create_dir_all(ws.join(".env.d")).unwrap();
    std::fs::write(ws.join(".env.d/x"), format!("{WS_CANARY}\n")).unwrap();
    std::fs::create_dir_all(ws.join(".aws")).unwrap();
    std::fs::write(ws.join(".aws/credentials"), format!("{WS_CANARY}\n")).unwrap();
    std::fs::create_dir_all(ws.join(".docker")).unwrap();
    std::fs::write(ws.join(".docker/config.json"), format!("{WS_CANARY}\n")).unwrap();
    std::fs::create_dir_all(ws.join(".config/gcloud")).unwrap();
    std::fs::write(
        ws.join(".config/gcloud/application_default_credentials.json"),
        format!("{WS_CANARY}\n"),
    )
    .unwrap();
    // Templates: a plain one, a subdir one, and one below a credential directory
    // (which must stay denied).
    std::fs::write(ws.join(".env.example"), format!("{TEMPLATE_OK}\n")).unwrap();
    std::fs::create_dir_all(ws.join("sub")).unwrap();
    std::fs::write(ws.join("sub/.env.example"), format!("{TEMPLATE_OK}\n")).unwrap();
    std::fs::write(ws.join(".SSH/.env.example"), format!("{WS_CANARY}\n")).unwrap();

    // Non-secret symlink aliases: `notes.txt` for the home key, `link.txt` for
    // the workspace `.env`.
    std::os::unix::fs::symlink(home.join(".ssh/id_ed25519"), home.join("notes.txt")).unwrap();
    std::os::unix::fs::symlink(ws.join(".env"), ws.join("link.txt")).unwrap();

    Fixture {
        ssh_secret: home.join(".ssh/id_ed25519"),
        gh_secret: home.join(".config/gh/hosts.yml"),
        _dir: dir,
        root: ws,
        home,
    }
}

fn run_tmp_path(fixture: &Fixture) -> PathBuf {
    fixture._dir.path().join("run-tmp")
}

#[cfg(target_os = "macos")]
fn backend_available() -> bool {
    matches!(os_sandbox::probe(), SandboxAvailability::Available)
}

/// Install an available sandbox for `home` and create its run temp dir.
fn install_available_for(workspace: &Path, run_tmp: &Path, home: &Path) {
    std::fs::create_dir_all(run_tmp).unwrap();
    os_sandbox::install_for_test(
        SandboxMode::Auto,
        SandboxPolicy::with_home(workspace, run_tmp, home.to_path_buf()),
        SandboxAvailability::Available,
    );
}

/// Install an available sandbox for the fixture's workspace and fake home.
fn install_available(fixture: &Fixture) {
    install_available_for(&fixture.root, &run_tmp_path(fixture), &fixture.home);
}

/// Install an explicit available policy and return its status, for the status
/// tests that do not run a command.
fn install_status(workspace: &Path, run_tmp: &Path, home: &Path) -> os_sandbox::SandboxStatus {
    install_available_for(workspace, run_tmp, home);
    os_sandbox::status().expect("installed")
}

/// Build `sh -c <command>`, set `envs`, and apply the installed sandbox
/// directly, bypassing the lexical guard, so the OS read layer is measured on
/// its own.
fn run_wrapped_env(root: &Path, command: &str, envs: &[(&str, &str)]) -> BoundedProcessOutput {
    let mut process = Command::new("sh");
    process.arg("-c").arg(command).current_dir(root);
    for (key, value) in envs {
        process.env(key, value);
    }
    let mut process = os_sandbox::wrap_command(process).expect("available sandbox wraps");
    process.stdout(Stdio::piped()).stderr(Stdio::piped());
    bounded_process::run_with_timeout(&mut process, TIMEOUT).expect("spawn wrapped command")
}

fn run_wrapped(root: &Path, command: &str) -> BoundedProcessOutput {
    run_wrapped_env(root, command, &[])
}

/// Assert a credential read fails under the OS layer and leaks no canary.
fn assert_read_denied(root: &Path, command: &str, envs: &[(&str, &str)], canary: &str) {
    let output = run_wrapped_env(root, command, envs);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stdout.contains(canary) && !stderr.contains(canary),
        "the canary must not leak for `{command}`; stdout={stdout:?} stderr={stderr:?}"
    );
    assert!(
        !output.success(),
        "the read must fail for `{command}`; stdout={stdout:?} stderr={stderr:?}"
    );
}

/// Assert a read succeeds and returns the expected content.
fn assert_read_allowed(root: &Path, command: &str, expected: &str) {
    let output = run_wrapped(root, command);
    assert!(
        output.success(),
        "the read must succeed for `{command}`; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains(expected),
        "`{command}` must return {expected:?}; stdout={:?}",
        String::from_utf8_lossy(&output.stdout)
    );
}

/// One row of the machine-checked read table.
#[derive(Deserialize)]
struct IndirectReadCase {
    id: String,
    command: String,
    /// Whether the lexical guard already refuses the command.
    #[serde(rename = "lexicalR")]
    lexical_r: bool,
    /// Whether the read fails once the OS layer is applied.
    #[serde(rename = "exitNonzero")]
    exit_nonzero: bool,
}

fn indirect_cases() -> Vec<IndirectReadCase> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/corpus/apps/issue502-indirect-read/fixtures/indirect-cases.jsonl");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).unwrap_or_else(|error| panic!("{line:?}: {error}")))
        .collect()
}

/// Drive the corpus rows through the applied read layer. `HOME` and `BASH_ENV`
/// point at the fake home key, so a row can read it only through a name that
/// does not show up lexically.
fn drive_indirect_reads(fixture: &Fixture) {
    let home = fixture.home.to_string_lossy().into_owned();
    let ssh = fixture.ssh_secret.to_string_lossy().into_owned();
    for case in indirect_cases() {
        assert_eq!(
            command_references_secret(&case.command).is_some(),
            case.lexical_r,
            "{}: the lexical decision must be unchanged",
            case.id
        );
        let output = run_wrapped_env(
            &fixture.root,
            &case.command,
            &[("HOME", home.as_str()), ("BASH_ENV", ssh.as_str())],
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            !output.success(),
            case.exit_nonzero,
            "{}: OS read outcome; stdout={stdout:?} stderr={stderr:?}",
            case.id
        );
        assert!(
            !stdout.contains(HOME_KEY_CANARY) && !stderr.contains(HOME_KEY_CANARY),
            "{}: the home key must not leak; stdout={stdout:?} stderr={stderr:?}",
            case.id
        );
    }
}

// ---------------------------------------------------------------------------
// The lexical layer is unchanged and shared: a workspace credential named
// directly is refused, a non-secret symlink name is not.
// ---------------------------------------------------------------------------

#[test]
fn workspace_dotenv_reference_is_lexical_and_a_symlink_name_is_not() {
    assert!(
        command_references_secret("cat .env").is_some(),
        "a directly named workspace credential stays refused lexically"
    );
    assert!(
        command_references_secret("cat link.txt").is_none(),
        "a non-secret symlink name is not a lexical reference"
    );
}

/// With no home secret location known (an empty `HOME`), the read layer has
/// nothing to deny, so the status must say the read isolation is ineffective
/// rather than claim it is applied. This uses the explicit install seam, so it
/// needs no backend.
#[test]
fn empty_home_reports_ineffective_read_isolation() {
    let _guard = sandbox_lock();
    let fixture = fixture();
    let status = install_status(&fixture.root, &run_tmp_path(&fixture), Path::new(""));
    assert!(status.applied, "{status:?}");
    assert!(!status.read_isolation, "{status:?}");
    assert!(
        status
            .reason
            .as_deref()
            .is_some_and(|reason| reason.contains("read isolation is not effective")),
        "{status:?}"
    );
    os_sandbox::reset_for_test();
}

// ---------------------------------------------------------------------------
// macOS (Issue #657, stage 2): the Seatbelt read layer denies home secrets and
// workspace credentials, and keeps the strict templates readable.
// ---------------------------------------------------------------------------

/// The applied read set on macOS: every indirect home-secret form fails and the
/// canary never appears. The home is the fake tempdir, never the real one.
#[cfg(target_os = "macos")]
#[test]
fn applied_indirect_reads_are_denied_on_macos() {
    let _guard = sandbox_lock();
    if !backend_available() {
        eprintln!("SKIP: sandbox backend unavailable (nested sandbox?)");
        return;
    }
    let fixture = fixture();
    install_available(&fixture);

    drive_indirect_reads(&fixture);

    // A lower- and an upper-case home secret path, and a non-credential sibling.
    assert_read_denied(
        &fixture.root,
        &format!("cat '{}'", fixture.gh_secret.display()),
        &[],
        GH_CANARY,
    );
    assert_read_denied(
        &fixture.root,
        &format!(
            "cat '{}'",
            fixture.home.join(".config/GH/hosts.yml").display()
        ),
        &[],
        GH_CANARY,
    );
    let other = fixture.home.join(".config/other/settings");
    assert_read_allowed(
        &fixture.root,
        &format!("cat '{}'", other.display()),
        "OTHER_OK",
    );
    assert_read_allowed(&fixture.root, "cat a.txt", "x");

    os_sandbox::reset_for_test();
}

/// The workspace credential rule denies the case-insensitive names, a
/// credential-named directory's contents, the #501 path sequences, and a
/// template below a credential directory, while the plain and subdir templates
/// stay readable. The workspace root is not canonicalized, so this also proves
/// the regex root is canonicalized.
#[cfg(target_os = "macos")]
#[test]
fn applied_workspace_credentials_are_denied_but_templates_stay_readable() {
    let _guard = sandbox_lock();
    if !backend_available() {
        eprintln!("SKIP: sandbox backend unavailable (nested sandbox?)");
        return;
    }
    let fixture = fixture();
    install_available(&fixture);

    for command in [
        "cat .env",
        "cat link.txt",
        "cat .Env.local",
        "cat SERVICE-ACCOUNT.json",
        "cat .SSH/id_rsa",
        "cat .env.d/x",
        "cat .aws/credentials",
        "cat .docker/config.json",
        "cat .config/gcloud/application_default_credentials.json",
        "cat .SSH/.env.example",
    ] {
        assert_read_denied(&fixture.root, command, &[], WS_CANARY);
    }
    assert_read_allowed(&fixture.root, "cat .env.example", TEMPLATE_OK);
    assert_read_allowed(&fixture.root, "cat sub/.env.example", TEMPLATE_OK);

    os_sandbox::reset_for_test();
}

/// A workspace below a symlinked directory is still confined: the workspace
/// regex root is canonicalized, so Seatbelt's resolved path matches.
#[cfg(target_os = "macos")]
#[test]
fn applied_workspace_under_a_symlinked_root_is_denied() {
    let _guard = sandbox_lock();
    if !backend_available() {
        eprintln!("SKIP: sandbox backend unavailable (nested sandbox?)");
        return;
    }
    let fixture = fixture();
    let real = fixture._dir.path().join("real");
    std::fs::create_dir_all(real.join("ws")).unwrap();
    std::fs::write(real.join("ws/.env"), format!("{WS_CANARY}\n")).unwrap();
    std::fs::write(real.join("ws/.env.example"), format!("{TEMPLATE_OK}\n")).unwrap();
    let link = fixture._dir.path().join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let workspace = link.join("ws"); // deliberately not canonicalized
    let run_tmp = fixture._dir.path().join("link-run-tmp");
    install_available_for(&workspace, &run_tmp, &fixture.home);

    assert_read_denied(&workspace, "cat .env", &[], WS_CANARY);
    assert_read_allowed(&workspace, "cat .env.example", TEMPLATE_OK);

    os_sandbox::reset_for_test();
}

/// The run-start status reports the applied read isolation through the existing
/// `reason` field, not a new event field.
#[cfg(target_os = "macos")]
#[test]
fn applied_status_reports_read_isolation_in_the_reason() {
    let _guard = sandbox_lock();
    if !backend_available() {
        eprintln!("SKIP: sandbox backend unavailable (nested sandbox?)");
        return;
    }
    let fixture = fixture();
    install_available(&fixture);

    let status = os_sandbox::status().expect("installed");
    assert!(status.applied, "{status:?}");
    assert!(status.read_isolation, "{status:?}");
    assert_eq!(
        status.reason.as_deref(),
        Some("read isolation applied"),
        "{status:?}"
    );

    os_sandbox::reset_for_test();
}

/// On macOS the read deny is applied after `(allow default)`, so a workspace
/// that is the home itself does not re-open a secret: the status still reports
/// the read isolation applied and the secret stays denied.
#[cfg(target_os = "macos")]
#[test]
fn applied_workspace_equal_to_home_still_denies_secrets() {
    let _guard = sandbox_lock();
    if !backend_available() {
        eprintln!("SKIP: sandbox backend unavailable (nested sandbox?)");
        return;
    }
    let fixture = fixture();
    let status = install_status(&fixture.home, &run_tmp_path(&fixture), &fixture.home);
    assert!(status.applied, "{status:?}");
    assert!(status.read_isolation, "{status:?}");
    assert_eq!(
        status.reason.as_deref(),
        Some("read isolation applied"),
        "{status:?}"
    );

    assert_read_denied(
        &fixture.home,
        &format!("cat '{}'", fixture.ssh_secret.display()),
        &[],
        HOME_KEY_CANARY,
    );
    os_sandbox::reset_for_test();
}

/// A symlinked home secret location (`~/.ssh` pointing outside the home) denies
/// its canonical target.
#[cfg(target_os = "macos")]
#[test]
fn applied_symlinked_home_secret_location_denies_its_target() {
    let _guard = sandbox_lock();
    if !backend_available() {
        eprintln!("SKIP: sandbox backend unavailable (nested sandbox?)");
        return;
    }
    let fixture = fixture();
    let home = fixture._dir.path().join("home2");
    let outside = fixture._dir.path().join("outside");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("id_rsa"), format!("{HOME_KEY_CANARY}\n")).unwrap();
    std::os::unix::fs::symlink(&outside, home.join(".ssh")).unwrap();
    let run_tmp = fixture._dir.path().join("home2-run-tmp");
    install_available_for(&fixture.root, &run_tmp, &home);

    assert_read_denied(
        &fixture.root,
        &format!("cat '{}/.ssh/id_rsa'", home.display()),
        &[],
        HOME_KEY_CANARY,
    );
    os_sandbox::reset_for_test();
}

/// The model Bash launch point is wired to the same read layer: an indirect read
/// the lexical guard accepts still fails.
#[cfg(target_os = "macos")]
#[test]
fn applied_launch_point_denies_an_indirect_read() {
    let _guard = sandbox_lock();
    if !backend_available() {
        eprintln!("SKIP: sandbox backend unavailable (nested sandbox?)");
        return;
    }
    let fixture = fixture();
    install_available(&fixture);

    let command = format!("cat '{}'", fixture.home.join("notes.txt").display());
    assert!(
        command_references_secret(&command).is_none(),
        "the lexical guard must pass this form so the read layer is what blocks it"
    );
    let outcome = commandagent::tools::bash::run_structured_cancel_and_force(
        &command,
        &fixture.root,
        false,
        TIMEOUT,
        || false,
        || false,
    )
    .expect("bash launch");
    assert!(!outcome.is_success(), "{outcome:?}");
    assert!(
        !outcome.summary.contains(HOME_KEY_CANARY),
        "{}",
        outcome.summary
    );

    os_sandbox::reset_for_test();
}

// ---------------------------------------------------------------------------
// Linux (Issue #657, stage 2): the Landlock read layer denies home secrets and
// keeps the toolchain, git, and the system readable and executable.
// ---------------------------------------------------------------------------

/// Ask the kernel for its Landlock ABI without going through the backend, so a
/// mutated `landlock::detected_abi()` cannot make these tests agree with
/// themselves.
#[cfg(target_os = "linux")]
fn kernel_landlock_abi() -> u32 {
    const LANDLOCK_CREATE_RULESET_VERSION: libc::c_uint = 1;
    // SAFETY: with a null attr pointer, size 0 and the VERSION flag, landlock_create_ruleset only
    // returns the ABI number; it reads no memory and creates no file descriptor.
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

#[cfg(target_os = "linux")]
fn landlock_kernel_release() -> String {
    commandagent::tools::os_sandbox::landlock::kernel_release_label()
}

/// The ABI-gated outcome for an ABI-dependent Linux test.
#[cfg(target_os = "linux")]
#[derive(Debug, PartialEq, Eq)]
enum AbiGate {
    Continue,
    Skip(String),
    Fail(String),
}

/// The skip/fail reasons name the kernel and the ABI, so a missing read backend
/// is never hidden under CI.
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

/// The read layer needs ABI1, the same as the write layer, so this mostly pins
/// the CI contract: an ABI below 1 fails under CI and skips otherwise.
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
/// process, when that variable is set. Best effort.
#[cfg(target_os = "linux")]
fn record_kernel_and_abi_in_step_summary() {
    use std::sync::Once;

    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let Some(path) = std::env::var_os("GITHUB_STEP_SUMMARY") else {
            return;
        };
        let line = format!(
            "issue502/657 landlock read: kernel={} abi={}\n",
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

/// Print the kernel release and Landlock ABI, and refuse a silent skip under
/// CI: a GitHub Actions Linux runner must have an applied read backend. A read
/// layer that is not applied cannot be distinguished from "unavailable", so the
/// backend must be available or the test fails under CI.
#[cfg(target_os = "linux")]
fn require_landlock_backend() -> bool {
    use commandagent::tools::os_sandbox::landlock;
    record_kernel_and_abi_in_step_summary();
    let abi = kernel_landlock_abi();
    let kernel = landlock::kernel_release_label();
    let available = matches!(os_sandbox::probe(), SandboxAvailability::Available);
    let reported = landlock::detected_abi();
    eprintln!("issue502/657 landlock read: kernel={kernel} abi={abi} detected_abi={reported}");
    if available {
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

/// The applied read set on Linux: every indirect home-secret form fails and the
/// canary never appears.
#[cfg(target_os = "linux")]
#[test]
fn linux_applied_indirect_reads_are_denied() {
    let _guard = sandbox_lock();
    if !require_landlock_backend() {
        eprintln!("SKIP: Landlock backend unavailable");
        return;
    }
    let abi = kernel_landlock_abi();
    if !require_abi_or_ci_fail(abi, 1) {
        return;
    }
    let fixture = fixture();
    install_available(&fixture);

    drive_indirect_reads(&fixture);
    assert_read_denied(
        &fixture.root,
        &format!("cat '{}'", fixture.gh_secret.display()),
        &[],
        GH_CANARY,
    );

    let other = fixture.home.join(".config/other/settings");
    assert_read_allowed(
        &fixture.root,
        &format!("cat '{}'", other.display()),
        "OTHER_OK",
    );
    assert_read_allowed(&fixture.root, "cat a.txt", "x");

    os_sandbox::reset_for_test();
}

/// A symlinked home secret location (`~/.ssh` pointing outside the home) denies
/// its canonical target on Linux too: the expansion canonicalizes each location
/// before building the read-allow list.
#[cfg(target_os = "linux")]
#[test]
fn linux_applied_symlinked_home_secret_location_denies_its_target() {
    let _guard = sandbox_lock();
    if !require_landlock_backend() {
        eprintln!("SKIP: Landlock backend unavailable");
        return;
    }
    let fixture = fixture();
    let home = fixture._dir.path().join("home2");
    let outside = fixture._dir.path().join("outside");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("id_rsa"), format!("{HOME_KEY_CANARY}\n")).unwrap();
    std::os::unix::fs::symlink(&outside, home.join(".ssh")).unwrap();
    let run_tmp = fixture._dir.path().join("home2-run-tmp");
    install_available_for(&fixture.root, &run_tmp, &home);

    assert_read_denied(
        &fixture.root,
        &format!("cat '{}/.ssh/id_rsa'", home.display()),
        &[],
        HOME_KEY_CANARY,
    );
    os_sandbox::reset_for_test();
}

/// When a writable location is the home (or an ancestor of a secret), the write
/// allow list already grants read on the secret's subtree, so the read layer
/// cannot deny it. The status must say so instead of claiming the read isolation
/// is applied. This uses the explicit install seam, so it needs no backend.
#[cfg(target_os = "linux")]
#[test]
fn linux_workspace_equal_to_home_reports_ineffective_read_isolation() {
    let _guard = sandbox_lock();
    let fixture = fixture();
    let status = install_status(&fixture.home, &run_tmp_path(&fixture), &fixture.home);
    assert!(status.applied, "{status:?}");
    assert!(!status.read_isolation, "{status:?}");
    assert!(
        status
            .reason
            .as_deref()
            .is_some_and(|reason| reason.contains("read isolation is not effective for")),
        "{status:?}"
    );
    os_sandbox::reset_for_test();
}

/// The Linux read layer denies only the home secret locations. A workspace
/// `.env` named directly is closed by the lexical guard; a non-secret symlink to
/// it is not closed by the read layer (documented limitation, decision 4), so
/// this test records that gap instead of hiding it.
#[cfg(target_os = "linux")]
#[test]
fn linux_workspace_credential_symlink_is_not_closed_by_read_isolation() {
    let _guard = sandbox_lock();
    if !require_landlock_backend() {
        eprintln!("SKIP: Landlock backend unavailable");
        return;
    }
    let fixture = fixture();
    install_available(&fixture);

    assert!(
        command_references_secret("cat .env").is_some(),
        "a directly named workspace credential is refused lexically"
    );
    assert!(command_references_secret("cat link.txt").is_none());
    assert!(path_confinement_rejection("cat link.txt", &fixture.root).is_none());
    let output = run_wrapped(&fixture.root, "cat link.txt");
    assert!(
        output.success(),
        "the Linux read layer does not close a workspace-credential symlink; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    os_sandbox::reset_for_test();
}

/// The applied read list keeps the toolchain, git, and the system locations
/// readable and executable, and the writable run temp directory usable.
#[cfg(target_os = "linux")]
#[test]
fn linux_applied_toolchain_git_and_system_are_readable_and_executable() {
    let _guard = sandbox_lock();
    if !require_landlock_backend() {
        eprintln!("SKIP: Landlock backend unavailable");
        return;
    }
    let fixture = fixture();
    install_available(&fixture);

    let script = r#"
for tool in cargo rustc rustup npm node python3 git; do
  if command -v "$tool" >/dev/null 2>&1; then
    "$tool" --version >/dev/null || { echo "FAIL $tool"; exit 1; }
  fi
done
ls /usr >/dev/null && ls /etc >/dev/null && ls /proc >/dev/null && ls /dev >/dev/null
echo TOOLCHAIN_OK
"#;
    let output = run_wrapped(&fixture.root, script);
    assert!(
        output.success() && String::from_utf8_lossy(&output.stdout).contains("TOOLCHAIN_OK"),
        "the toolchain, git, and system locations must stay readable and executable; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    os_sandbox::reset_for_test();
}

/// The run-start status reports the read isolation through the existing reason:
/// an older ABI leaves a write operation open and the reason names the read
/// isolation with it; a fully closed ABI records the same "read isolation
/// applied" sentence macOS does (Issue #668).
#[cfg(target_os = "linux")]
#[test]
fn linux_installed_status_reports_read_isolation() {
    let _guard = sandbox_lock();
    if !require_landlock_backend() {
        eprintln!("SKIP: Landlock backend unavailable");
        return;
    }
    let fixture = fixture();
    let state_dir = fixture._dir.path().join("state");

    let status = os_sandbox::install(SandboxMode::Auto, &fixture.root, &state_dir);
    assert_eq!(status.backend, "landlock", "{status:?}");
    assert!(status.applied, "{status:?}");
    assert!(status.read_isolation, "{status:?}");
    let reason = status
        .reason
        .as_deref()
        .expect("the read layer is reported");
    assert!(reason.contains("read isolation applied"), "{reason}");
    let reported = os_sandbox::status().expect("installed");
    assert_eq!(reported.read_isolation, status.read_isolation);
    assert_eq!(reported.reason, status.reason);

    os_sandbox::cleanup();
    os_sandbox::reset_for_test();
}

/// A dependency-less `cargo build` succeeds with the read layer applied. Ignored
/// by default; run explicitly to record the applied result.
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
        "[package]\nname = \"issue502-read-probe\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
    )
    .unwrap();
    std::fs::write(src.join("main.rs"), "fn main() {}\n").unwrap();

    let output = run_wrapped(&fixture.root, "cargo build");
    assert!(
        output.success(),
        "cargo build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        fixture
            .root
            .join("target/debug/issue502-read-probe")
            .is_file()
    );
    os_sandbox::reset_for_test();
}

/// A cached, offline `npm ci` succeeds with the read layer applied, unless the
/// private-registry `~/.npmrc` it would read is a home secret (the documented
/// consequence of denying home npm config). Ignored by default; run explicitly
/// to record the applied result.
#[cfg(target_os = "linux")]
#[test]
#[ignore = "heavy: run explicitly to record the applied result"]
fn linux_applied_npm_ci_succeeds_when_cached() {
    let _guard = sandbox_lock();
    if !require_landlock_backend() {
        return;
    }
    let fixture = fixture();
    install_available(&fixture);
    std::fs::write(
        fixture.root.join("package.json"),
        "{\"name\":\"issue502-read-probe\",\"version\":\"0.1.0\",\"private\":true}\n",
    )
    .unwrap();
    std::fs::write(
        fixture.root.join("package-lock.json"),
        "{\"name\":\"issue502-read-probe\",\"version\":\"0.1.0\",\"lockfileVersion\":3,\"requires\":true,\"packages\":{\"\":{\"name\":\"issue502-read-probe\",\"version\":\"0.1.0\"}}}\n",
    )
    .unwrap();

    let output = run_wrapped_env(
        &fixture.root,
        "npm ci --offline",
        &[("NEXT_TELEMETRY_DISABLED", "1")],
    );
    assert!(
        output.success(),
        "npm ci failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
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
            abi_gate(true, 0, 1, kernel),
            AbiGate::Fail(reason) if reason.contains(kernel) && reason.contains('0')
        ),
        "{:?}",
        abi_gate(true, 0, 1, kernel)
    );
    assert!(matches!(abi_gate(false, 0, 1, kernel), AbiGate::Skip(_)));
    assert_eq!(abi_gate(false, 1, 1, kernel), AbiGate::Continue);
    assert_eq!(abi_gate(true, 1, 1, kernel), AbiGate::Continue);
    assert_eq!(abi_gate(false, 8, 1, kernel), AbiGate::Continue);
}

// ---------------------------------------------------------------------------
// Shared unavailable path: `required` fails honestly and `auto` continues.
// ---------------------------------------------------------------------------

#[test]
fn required_and_unavailable_fail_honestly_and_auto_continues() {
    let _guard = sandbox_lock();
    let fixture = fixture();

    for mode in [SandboxMode::Required, SandboxMode::Auto] {
        os_sandbox::install_for_test(
            mode,
            SandboxPolicy::with_home(&fixture.root, &run_tmp_path(&fixture), fixture.home.clone()),
            SandboxAvailability::Unavailable(NESTED.to_string()),
        );
        let status = os_sandbox::status().unwrap();
        assert!(!status.applied, "{status:?}");
        assert!(!status.read_isolation, "{status:?}");
        assert_eq!(status.reason.as_deref(), Some(NESTED), "{status:?}");
    }

    assert!(NESTED.contains("71"));
    os_sandbox::reset_for_test();
}

/// Issue #668: a home that crowds the read-allow expansion past the rule limit
/// closes the whole home instead of opening reads. The status does not record
/// the read isolation as applied, the home secret stays unreadable, and the
/// write confinement stays in force.
#[cfg(target_os = "linux")]
#[test]
fn linux_expansion_over_the_limit_is_not_recorded_as_applied() {
    let _guard = sandbox_lock();
    if !require_landlock_backend() {
        eprintln!("SKIP: Landlock backend unavailable");
        return;
    }
    let fixture = fixture();
    crowd_home(&fixture);

    let within = install_status(&fixture.root, &run_tmp_path(&fixture), &fixture.home);
    assert!(within.read_isolation, "{within:?}");
    os_sandbox::reset_for_test();

    os_sandbox::set_read_allow_limit_for_test(Some(CROWDED_LIMIT));
    let status = install_status(&fixture.root, &run_tmp_path(&fixture), &fixture.home);
    assert!(status.applied, "{status:?}");
    assert!(!status.read_isolation, "{status:?}");
    let reason = status.reason.as_deref().expect("the overflow is reported");
    assert!(
        reason.contains("read isolation is not effective"),
        "{reason}"
    );
    assert!(reason.contains("home is unreadable"), "{reason}");
    assert!(!reason.contains("read isolation applied"), "{reason}");

    assert_home_closed(&fixture);
    let outside = fixture._dir.path().join("outside-write");
    let output = run_wrapped(&fixture.root, &format!("echo x > '{}'", outside.display()));
    assert!(!output.success(), "writes stay confined");
    assert!(!outside.exists());

    os_sandbox::reset_for_test();
}

/// Issue #668: an overflow that first appears when a command is wrapped (the
/// home grew after the run started) also keeps the home secret unreadable.
#[cfg(target_os = "linux")]
#[test]
fn linux_expansion_that_overflows_after_run_start_keeps_the_home_closed() {
    let _guard = sandbox_lock();
    if !require_landlock_backend() {
        eprintln!("SKIP: Landlock backend unavailable");
        return;
    }
    let fixture = fixture();
    let started = install_status(&fixture.root, &run_tmp_path(&fixture), &fixture.home);
    assert!(started.read_isolation, "{started:?}");

    crowd_home(&fixture);
    os_sandbox::set_read_allow_limit_for_test(Some(CROWDED_LIMIT));
    assert_home_closed(&fixture);

    os_sandbox::reset_for_test();
}

/// When even the closed-home expansion exceeds the limit (the home's parent is
/// crowded), the sandbox is still built, in `auto` and `required` alike: the
/// status stays applied, the home secret is unreadable, and writes outside the
/// workspace fail. Only the reason says the read isolation is not effective.
#[cfg(target_os = "linux")]
#[test]
fn linux_crowded_home_parent_keeps_the_sandbox_applied() {
    let _guard = sandbox_lock();
    if !require_landlock_backend() {
        eprintln!("SKIP: Landlock backend unavailable");
        return;
    }
    for mode in [SandboxMode::Auto, SandboxMode::Required] {
        let fixture = fixture();
        std::fs::create_dir_all(run_tmp_path(&fixture)).unwrap();
        // The home's parent is the fixture dir; crowd it beyond the limit.
        for index in 0..(CROWDED_LIMIT + 500) {
            std::fs::write(fixture._dir.path().join(format!("sibling-{index}")), "x").unwrap();
        }
        os_sandbox::set_read_allow_limit_for_test(Some(CROWDED_LIMIT));
        os_sandbox::install_for_test(
            mode,
            SandboxPolicy::with_home(&fixture.root, &run_tmp_path(&fixture), fixture.home.clone()),
            SandboxAvailability::Available,
        );
        let status = os_sandbox::status().expect("installed");
        assert!(status.applied, "{mode:?}: {status:?}");
        assert!(!status.read_isolation, "{mode:?}: {status:?}");
        let reason = status.reason.as_deref().expect("reported");
        assert!(reason.contains("even with the home closed"), "{reason}");
        assert!(!reason.contains("read isolation applied"), "{reason}");

        assert_read_denied(
            &fixture.root,
            &format!("cat '{}'", fixture.ssh_secret.display()),
            &[],
            HOME_KEY_CANARY,
        );
        let outside = fixture._dir.path().join("outside-write");
        let output = run_wrapped(&fixture.root, &format!("echo x > '{}'", outside.display()));
        assert!(!output.success(), "{mode:?}: writes stay confined");
        assert!(!outside.exists());
        assert!(
            os_sandbox::status().expect("installed").applied,
            "{mode:?}: the sandbox stays applied after the command"
        );
        os_sandbox::reset_for_test();
    }
}

/// The limit the crowded-home tests use, above the rules the closed-home
/// expansion needs and below the rules the full expansion needs.
#[cfg(target_os = "linux")]
const CROWDED_LIMIT: usize = 1000;

/// Give the fake home more entries than [`CROWDED_LIMIT`].
#[cfg(target_os = "linux")]
fn crowd_home(fixture: &Fixture) {
    for index in 0..(CROWDED_LIMIT + 500) {
        std::fs::write(fixture.home.join(format!("entry-{index}")), "x").unwrap();
    }
}

/// The home secret and a plain home file are unreadable, while the workspace
/// stays readable.
#[cfg(target_os = "linux")]
fn assert_home_closed(fixture: &Fixture) {
    assert_read_denied(
        &fixture.root,
        &format!("cat '{}'", fixture.ssh_secret.display()),
        &[],
        HOME_KEY_CANARY,
    );
    assert_read_denied(
        &fixture.root,
        &format!("cat '{}'", fixture.home.join("entry-0").display()),
        &[],
        "unused-canary",
    );
}
