#![cfg(unix)]

//! Issue #658 (stage 3 of #502): the `--offline` network layer and the
//! dependency-setup launch points.
//!
//! The lexical guard of `--offline` only refuses commands it can name (`npm
//! install`, `curl`, `wget`, …). An arbitrary program (`python -c
//! "urllib..."`, `node -e "fetch(...)"`) is invisible to it. This file drives
//! the same `wrap_command` the launch points use with an `--offline` policy, so
//! an outbound TCP connect is refused by the OS while a loopback dev server
//! stays reachable. It also drives the dependency-setup launch with a fake npm,
//! so a build script that tries to write outside the workspace fails and the
//! outside fixture does not change.
//!
//! The macOS applied cases are `#[cfg(target_os = "macos")]` and no-op when the
//! backend is unavailable (a nested sandbox reports rc=71). The Linux applied
//! cases are `#[cfg(target_os = "linux")]` and fail under CI when Landlock (or
//! its ABI4 network rights) is unavailable, so the runner cannot silently lose
//! the network backend. The `off` and unavailable paths are shared. Every
//! fixture lives inside one test tempdir and never touches the real home.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use commandagent::bounded_process::{self, BoundedProcessOutput};
use commandagent::minimal_loop::dependency_setup::{
    self, NodeDependencySetupAuthority, NodeDependencySetupStatus,
};
use commandagent::tools::os_sandbox::{self, SandboxAvailability, SandboxMode, SandboxPolicy};

const TIMEOUT: Duration = Duration::from_secs(120);
/// `192.0.2.0/24` is TEST-NET-1 (RFC 5737): a reserved address that is never
/// routed, so a connect to it only fails when the OS refuses it.
const RESERVED_HOST: &str = "192.0.2.1";
const NESTED: &str = "sandbox-exec probe exited with exit status: 71";

/// The sandbox state is process-wide, so the tests that install it serialize.
static SANDBOX_LOCK: Mutex<()> = Mutex::new(());

fn sandbox_lock() -> MutexGuard<'static, ()> {
    SANDBOX_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A workspace, its run temp directory, a fake home, and a sibling outside
/// fixture, all inside one test tempdir.
struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
    run_tmp: PathBuf,
    home: PathBuf,
    outside_secret: PathBuf,
    connect_script: PathBuf,
}

/// A tiny TCP client: prints `CONNECT_OK` on success or `CONNECT_ERR <Kind>` on
/// failure, so the test can tell an OS refusal (PermissionError) from a timeout.
const CONNECT_PY: &str = r#"
import socket, sys
host, port = sys.argv[1], int(sys.argv[2])
s = socket.socket()
s.settimeout(3)
try:
    s.connect((host, port))
    print("CONNECT_OK")
except OSError as e:
    print("CONNECT_ERR", type(e).__name__)
"#;

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().expect("create tempdir");
    let root = dir.path().join("ws");
    let run_tmp = dir.path().join("run-tmp");
    let home = dir.path().join("home");
    let outside = dir.path().join("outside");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(&run_tmp).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("secret.txt"), "OUTSIDE_658\n").unwrap();
    // A write through this symlink lands outside the workspace, which the
    // sandbox must refuse.
    std::os::unix::fs::symlink(&outside, root.join("escape")).unwrap();
    let connect_script = root.join("connect.py");
    std::fs::write(&connect_script, CONNECT_PY).unwrap();
    Fixture {
        _dir: dir,
        root,
        run_tmp,
        home,
        outside_secret: outside.join("secret.txt"),
        connect_script,
    }
}

#[cfg(target_os = "macos")]
fn backend_available() -> bool {
    matches!(os_sandbox::probe(), SandboxAvailability::Available)
}

/// Ask the kernel for its Landlock ABI without going through the backend, so a
/// mutated `landlock::detected_abi()` cannot make these tests agree with
/// themselves.
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

#[cfg(target_os = "linux")]
fn landlock_kernel_release() -> String {
    commandagent::tools::os_sandbox::landlock::kernel_release_label()
}

/// The ABI-gated outcome for the ABI4 network test.
#[cfg(target_os = "linux")]
#[derive(Debug, PartialEq, Eq)]
enum AbiGate {
    Continue,
    Skip(String),
    Fail(String),
}

/// The network layer needs ABI4, so the gate fails under CI when the kernel
/// cannot confine TCP connect and skips on a local run; the reason names the
/// kernel and the queried ABI.
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
            "issue502/658 landlock network: kernel={} abi={}\n",
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
/// CI: a GitHub Actions Linux runner must have an applied backend. The network
/// layer additionally needs ABI4, which the caller gates on separately.
#[cfg(target_os = "linux")]
fn require_landlock_backend() -> bool {
    use commandagent::tools::os_sandbox::landlock;
    record_kernel_and_abi_in_step_summary();
    let abi = kernel_landlock_abi();
    let kernel = landlock::kernel_release_label();
    let available = matches!(os_sandbox::probe(), SandboxAvailability::Available);
    let reported = landlock::detected_abi();
    eprintln!("issue502/658 landlock network: kernel={kernel} abi={abi} detected_abi={reported}");
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

/// Install an available sandbox with the given offline policy.
fn install_available(policy: SandboxPolicy) -> os_sandbox::SandboxStatus {
    os_sandbox::install_for_test(SandboxMode::Auto, policy, SandboxAvailability::Available)
}

fn online_policy(fixture: &Fixture) -> SandboxPolicy {
    SandboxPolicy::with_home(&fixture.root, &fixture.run_tmp, fixture.home.clone())
}

/// Build `sh -c <command>`, apply the installed sandbox directly (bypassing the
/// lexical guard), and run it, so the OS network layer is measured on its own.
fn run_wrapped(root: &Path, command: &str) -> BoundedProcessOutput {
    let mut process = Command::new("sh");
    process.arg("-c").arg(command).current_dir(root);
    let mut process = os_sandbox::wrap_command(process).expect("available sandbox wraps");
    process.stdout(Stdio::piped()).stderr(Stdio::piped());
    bounded_process::run_with_timeout(&mut process, TIMEOUT).expect("spawn wrapped command")
}

fn connect_device(fixture: &Fixture, host: &str, port: u16) -> String {
    let command = format!(
        "python3 {script} {host} {port}",
        script = fixture.connect_script.display(),
    );
    let output = run_wrapped(&fixture.root, &command);
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// A loopback TCP listener the sandboxed client can reach, held for the test.
fn dev_server() -> (std::net::TcpListener, u16) {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind dev server");
    let port = listener.local_addr().unwrap().port();
    (listener, port)
}

// ---------------------------------------------------------------------------
// Shared: the sandbox-off and unavailable paths do not confine the network.
// ---------------------------------------------------------------------------

/// A run that is not `--offline` handles no network access, so its outbound
/// behaviour is exactly as before this stage: no OS refusal (`PermissionError`).
#[test]
fn non_offline_run_leaves_outbound_unchanged() {
    let _serial = sandbox_lock();
    let fixture = fixture();
    install_available(online_policy(&fixture));

    let result = connect_device(&fixture, RESERVED_HOST, 9);
    assert!(
        !result.contains("PermissionError"),
        "a non-offline run must not be refused by the OS: {result}"
    );

    os_sandbox::cleanup();
    os_sandbox::reset_for_test();
}

/// An unavailable backend records the reason and does not claim network
/// confinement: `auto` continues, `required` refuses honestly.
#[test]
fn unavailable_backend_records_and_does_not_claim_confinement() {
    let _serial = sandbox_lock();
    let fixture = fixture();

    os_sandbox::install_for_test(
        SandboxMode::Auto,
        online_policy(&fixture).with_offline(true),
        SandboxAvailability::Unavailable(NESTED.to_string()),
    );
    let status = os_sandbox::status().unwrap();
    assert!(!status.applied, "{status:?}");
    assert_eq!(status.reason.as_deref(), Some(NESTED));
    assert!(
        os_sandbox::wrap_command(Command::new("sh")).is_ok(),
        "auto continues"
    );

    os_sandbox::install_for_test(
        SandboxMode::Required,
        online_policy(&fixture).with_offline(true),
        SandboxAvailability::Unavailable(NESTED.to_string()),
    );
    let error = os_sandbox::wrap_command(Command::new("sh")).expect_err("required must refuse");
    assert!(
        error.summary().starts_with(os_sandbox::UNAVAILABLE_KIND),
        "{}",
        error.summary()
    );

    os_sandbox::reset_for_test();
}

// ---------------------------------------------------------------------------
// Applied: an --offline run refuses outbound TCP but allows the dev server.
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
#[test]
fn macos_offline_refuses_outbound_and_allows_the_dev_server() {
    let _serial = sandbox_lock();
    if !backend_available() {
        eprintln!("SKIP: nested sandbox; macOS network cases no-op");
        return;
    }
    let fixture = fixture();
    install_available(online_policy(&fixture).with_offline(true));

    let (_server, port) = dev_server();
    let loopback = connect_device(&fixture, "127.0.0.1", port);
    assert_eq!(loopback, "CONNECT_OK", "loopback must stay reachable");

    let reserved = connect_device(&fixture, RESERVED_HOST, 9);
    assert!(
        reserved.contains("PermissionError"),
        "an offline run must have its outbound connect refused by the OS: {reserved}"
    );

    os_sandbox::cleanup();
    os_sandbox::reset_for_test();
}

/// macOS Seatbelt: an `--offline` run lets a unix socket be created (bind and
/// listen) inside the isolation, while the reserved-address connect is still
/// refused. The socket path is inside the fixture tempdir.
#[cfg(target_os = "macos")]
#[test]
fn macos_offline_allows_unix_socket_bind_and_still_refuses_outbound() {
    const UNPROBE_PY: &str = r#"
import os, socket, sys
path = os.path.join(sys.argv[1], "probe.sock")
s = socket.socket(socket.AF_UNIX)
try:
    s.bind(path)
    s.listen(1)
    print("BIND_OK")
except OSError as e:
    print("BIND_ERR", type(e).__name__)
c = socket.socket()
c.settimeout(3)
try:
    c.connect(("192.0.2.1", 9))
    print("EXT_OK")
except OSError as e:
    print("EXT_ERR", type(e).__name__)
"#;
    let _serial = sandbox_lock();
    if !backend_available() {
        eprintln!("SKIP: nested sandbox; macOS network cases no-op");
        return;
    }
    let fixture = fixture();
    install_available(online_policy(&fixture).with_offline(true));

    let script = fixture.root.join("unixbind.py");
    std::fs::write(&script, UNPROBE_PY).unwrap();
    let command = format!(
        "python3 {script} {dir}",
        script = script.display(),
        dir = fixture.root.display(),
    );
    let output = run_wrapped(&fixture.root, &command);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("BIND_OK"),
        "a unix socket must be bindable inside the isolation: {stdout}"
    );
    assert!(
        stdout.contains("EXT_ERR") && !stdout.contains("EXT_OK"),
        "outbound must stay refused: {stdout}"
    );

    os_sandbox::cleanup();
    os_sandbox::reset_for_test();
}

/// Run `child_test` (an `#[ignore]`d test of this file) in a fresh process whose
/// stdin is an open pipe holding data, so an inherited stdin is visible to the
/// dependency-setup child while a null stdin is not.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn assert_dependency_setup_child_stdin_is_null(child_test: &str) {
    use std::io::Write;

    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", child_test, "--nocapture"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn stdin child");
    // Keep the pipe open with data while the child runs; a null stdin would not
    // see this.
    let mut stdin = child.stdin.take().expect("child stdin");
    let _ = stdin.write_all(b"x");
    let _ = stdin.flush();
    let output = child.wait_with_output().expect("wait stdin child");
    drop(stdin);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "the child check must pass: {}\n{stdout}",
        output.status
    );
    // A name that no longer matches runs 0 tests and still exits 0.
    assert!(
        stdout.contains("1 passed"),
        "the child check must have run exactly one test: {stdout}"
    );
}

/// The body of the child checks. It runs in its own process so its stdin can be a
/// pipe; the fake npm records whether fd 0 is a terminal, a pipe, and at EOF.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn dependency_setup_child_stdin_is_null_body() {
    const FAKE_NPM_STDIN: &str = r#"#!/usr/bin/env python3
import os, select, stat
fd = os.fstat(0)
ready, _, _ = select.select([0], [], [], 0.5)
eof = bool(ready) and os.read(0, 1) == b""
with open("stdin.txt", "w") as fh:
    fh.write(f"tty={os.isatty(0)} fifo={stat.S_ISFIFO(fd.st_mode)} eof={eof}\n")
os.makedirs("node_modules", exist_ok=True)
"#;
    let fixture = fixture();
    std::fs::write(
        fixture.root.join("package.json"),
        r#"{"dependencies":{"left-pad":"^1.3.0"}}"#,
    )
    .unwrap();
    let fake_npm = write_executable(&fixture.root, "fake-npm", FAKE_NPM_STDIN);
    install_available(online_policy(&fixture));

    let requirement = dependency_setup::requirement_for_node_declared_dependencies(
        &fixture.root,
        Some("node"),
        "network test",
        NodeDependencySetupAuthority::PlanSetupStep,
    );
    let observation = dependency_setup::run_node_dependency_setup_with_program(
        &fixture.root,
        &requirement,
        &fake_npm,
    );
    assert_eq!(
        observation.status,
        NodeDependencySetupStatus::Passed,
        "{observation:?}"
    );

    let recorded = std::fs::read_to_string(fixture.root.join("stdin.txt")).unwrap();
    assert!(
        recorded.contains("tty=False"),
        "stdin must not be a terminal: {recorded}"
    );
    assert!(
        recorded.contains("fifo=False"),
        "stdin must not be a pipe: {recorded}"
    );
    assert!(
        recorded.contains("eof=True"),
        "stdin must be at EOF: {recorded}"
    );

    os_sandbox::cleanup();
    os_sandbox::reset_for_test();
}

/// macOS Seatbelt: the dependency-setup child's stdin is null, not inherited.
/// `seatbelt::wrap` rebuilds the command, so the launch must set `stdin` after
/// the wrap.
#[cfg(target_os = "macos")]
#[test]
fn macos_dependency_setup_child_stdin_is_null() {
    if !backend_available() {
        eprintln!("SKIP: nested sandbox; dependency-setup stdin no-op");
        return;
    }
    assert_dependency_setup_child_stdin_is_null("macos_dependency_setup_child_stdin_is_null_child");
}

/// The child check of [`macos_dependency_setup_child_stdin_is_null`].
#[cfg(target_os = "macos")]
#[test]
#[ignore]
fn macos_dependency_setup_child_stdin_is_null_child() {
    // The child process spawned by the parent has its own sandbox state, but
    // --include-ignored also runs this test in the main process, so it must
    // serialize with the tests that install the process-wide sandbox.
    let _serial = sandbox_lock();
    dependency_setup_child_stdin_is_null_body();
}

/// Linux Landlock: the dependency-setup child's stdin is null, not inherited.
/// Landlock wraps the command in place, so this is correct today; the test
/// catches a regression that would let the child inherit the parent's stdin
/// (Issue #672).
#[cfg(target_os = "linux")]
#[test]
fn linux_dependency_setup_child_stdin_is_null() {
    assert_dependency_setup_child_stdin_is_null("linux_dependency_setup_child_stdin_is_null_child");
}

/// The child check of [`linux_dependency_setup_child_stdin_is_null`]. A Linux CI
/// runner without a working Landlock fails in `require_landlock_backend`.
#[cfg(target_os = "linux")]
#[test]
#[ignore]
fn linux_dependency_setup_child_stdin_is_null_child() {
    let _serial = sandbox_lock();
    if !require_landlock_backend() {
        eprintln!("SKIP: Landlock backend unavailable");
        return;
    }
    dependency_setup_child_stdin_is_null_body();
}

#[cfg(target_os = "linux")]
#[test]
fn linux_offline_refuses_outbound_and_allows_only_the_dev_server_port() {
    let _serial = sandbox_lock();
    if !require_landlock_backend() {
        eprintln!("SKIP: Landlock backend unavailable");
        return;
    }
    // The network layer needs ABI4 (kernel 6.7); under CI a kernel without it
    // fails here instead of silently losing the network backend.
    if !require_abi_or_ci_fail(kernel_landlock_abi(), 4) {
        return;
    }
    let fixture = fixture();
    let (_allowed_server, allowed_port) = dev_server();
    let (_other_server, other_port) = dev_server();
    install_available(
        online_policy(&fixture)
            .with_offline(true)
            .with_dev_server_ports([allowed_port]),
    );

    let allowed = connect_device(&fixture, "127.0.0.1", allowed_port);
    assert_eq!(
        allowed, "CONNECT_OK",
        "the dev-server port must stay reachable"
    );

    let other = connect_device(&fixture, "127.0.0.1", other_port);
    assert!(
        other.contains("PermissionError"),
        "landlock is port-scoped, so another loopback port must be refused: {other}"
    );

    let reserved = connect_device(&fixture, RESERVED_HOST, other_port);
    assert!(
        reserved.contains("PermissionError"),
        "an offline run must refuse an outbound connect: {reserved}"
    );

    os_sandbox::cleanup();
    os_sandbox::reset_for_test();
}

/// The ABI gate: the network layer needs ABI4, so an ABI below 4 fails under CI,
/// skips on a local run, and ABI4 or later continues; the reason names the
/// kernel and the queried ABI.
#[cfg(target_os = "linux")]
#[test]
fn linux_abi_gate_requires_abi4_fails_under_ci_and_skips_otherwise() {
    let kernel = "6.1.0-generic";
    assert!(matches!(
        abi_gate(true, 3, 4, kernel),
        AbiGate::Fail(reason) if reason.contains(kernel) && reason.contains("ABI4")
    ));
    assert!(matches!(abi_gate(false, 3, 4, kernel), AbiGate::Skip(_)));
    assert_eq!(abi_gate(false, 4, 4, kernel), AbiGate::Continue);
    assert_eq!(abi_gate(true, 4, 4, kernel), AbiGate::Continue);
    assert_eq!(abi_gate(false, 9, 4, kernel), AbiGate::Continue);
}

// ---------------------------------------------------------------------------
// Issue #672: MPTCP, and the Linux pip cache.
// ---------------------------------------------------------------------------

/// One connect from a Multipath TCP socket (`IPPROTO_MPTCP` = 262). The first
/// word of the output is the verdict: `REFUSED` when the OS refuses the connect,
/// `ALLOWED` when it goes through, and `UNVERIFIED <stage> <errno>` when the
/// socket cannot be created or the connect fails for another reason.
#[cfg(target_os = "linux")]
const MPTCP_CONNECT_PY: &str = r#"
import errno, socket, sys
host, port = sys.argv[1], int(sys.argv[2])
try:
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM, 262)
except OSError as e:
    print("UNVERIFIED socket", errno.errorcode.get(e.errno, type(e).__name__))
    sys.exit(0)
s.settimeout(3)
try:
    s.connect((host, port))
    print("ALLOWED")
except PermissionError:
    print("REFUSED")
except OSError as e:
    print("UNVERIFIED connect", errno.errorcode.get(e.errno, type(e).__name__))
"#;

/// Turn the probe output into the one-word verdict, or `UNVERIFIED (reason)`.
#[cfg(target_os = "linux")]
fn mptcp_verdict(stdout: &str) -> String {
    let mut words = stdout.split_whitespace();
    match words.next() {
        Some("REFUSED") => "REFUSED".to_string(),
        Some("ALLOWED") => "ALLOWED".to_string(),
        Some("UNVERIFIED") => format!("UNVERIFIED ({})", words.collect::<Vec<_>>().join(" ")),
        _ => format!(
            "UNVERIFIED (no verdict from the probe: {:?})",
            stdout.trim()
        ),
    }
}

/// Append one probe verdict line to the file named by
/// `COMMANDAGENT_PROBE_VERDICT_FILE`, as the TIOCSTI probe in
/// `issue502_os_sandbox_write.rs` does, so `scripts/check-probe-verdicts.sh`
/// prints it and fails the CI job on an `UNVERIFIED` line. A write failure never
/// fails a test and an unset variable changes nothing.
#[cfg(target_os = "linux")]
fn append_probe_verdict(line: &str) {
    let Some(path) = std::env::var_os("COMMANDAGENT_PROBE_VERDICT_FILE") else {
        return;
    };
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| std::io::Write::write_all(&mut file, line.as_bytes()));
}

#[cfg(target_os = "linux")]
#[test]
fn linux_mptcp_verdict_reads_the_probe_output() {
    assert_eq!(mptcp_verdict("REFUSED\n"), "REFUSED");
    assert_eq!(mptcp_verdict("ALLOWED\n"), "ALLOWED");
    assert_eq!(
        mptcp_verdict("UNVERIFIED socket EPROTONOSUPPORT\n"),
        "UNVERIFIED (socket EPROTONOSUPPORT)"
    );
    assert!(mptcp_verdict("").starts_with("UNVERIFIED ("));
}

/// Issue #672: from inside an `--offline` isolation, connect once from an MPTCP
/// socket to a loopback port the run does not allow, and record the outcome
/// (`REFUSED`, `ALLOWED`, or `UNVERIFIED (reason)`) in the probe verdict file.
/// An environment that cannot create an MPTCP socket, or has no ABI4 network
/// backend, records `UNVERIFIED` — never a silent pass. `ALLOWED` does not fail
/// the test: it is a documented limit, like UDP.
#[cfg(target_os = "linux")]
#[test]
fn linux_offline_mptcp_connect_to_a_disallowed_port_is_recorded() {
    let _serial = sandbox_lock();
    let abi = kernel_landlock_abi();
    let available = matches!(os_sandbox::probe(), SandboxAvailability::Available);
    let verdict = if !available || abi < 4 {
        format!("UNVERIFIED (no Landlock ABI4 network backend: available={available} abi={abi})")
    } else {
        let fixture = fixture();
        let (_allowed_server, allowed_port) = dev_server();
        let (_other_server, other_port) = dev_server();
        install_available(
            online_policy(&fixture)
                .with_offline(true)
                .with_dev_server_ports([allowed_port]),
        );
        let script = fixture.root.join("mptcp.py");
        std::fs::write(&script, MPTCP_CONNECT_PY).unwrap();
        let output = run_wrapped(
            &fixture.root,
            &format!("python3 {} 127.0.0.1 {other_port}", script.display()),
        );
        os_sandbox::cleanup();
        os_sandbox::reset_for_test();
        mptcp_verdict(&String::from_utf8_lossy(&output.stdout))
    };
    let mptcp_enabled = std::fs::read_to_string("/proc/sys/net/mptcp/enabled")
        .map(|value| value.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let line = format!(
        "issue672/mptcp: {verdict}; kernel={} abi={abi} net.mptcp.enabled={mptcp_enabled}\n",
        landlock_kernel_release()
    );
    eprint!("{line}");
    append_probe_verdict(&line);
}

/// Issue #672: the Linux `~/.cache/pip` cache is an allowed write location when
/// it is inside the home (decision 2, known caches only). A sibling directory in
/// `~/.cache` is not.
#[cfg(target_os = "linux")]
#[test]
fn linux_pip_cache_inside_the_home_is_writable_inside_the_isolation() {
    let _serial = sandbox_lock();
    if !require_landlock_backend() {
        eprintln!("SKIP: Landlock backend unavailable");
        return;
    }
    let fixture = fixture();
    let pip = fixture.home.join(".cache/pip");
    let sibling = fixture.home.join(".cache/other");
    std::fs::create_dir_all(&pip).unwrap();
    std::fs::create_dir_all(&sibling).unwrap();
    install_available(online_policy(&fixture).with_offline(true));

    let output = run_wrapped(
        &fixture.root,
        &format!("echo ok > '{}'", pip.join("wheel.cache").display()),
    );
    assert!(
        output.success(),
        "~/.cache/pip must be writable: {output:?}"
    );
    assert!(pip.join("wheel.cache").exists());

    let output = run_wrapped(
        &fixture.root,
        &format!("echo no > '{}'", sibling.join("x").display()),
    );
    assert!(
        !output.success() && !sibling.join("x").exists(),
        "only the known cache is writable, not its siblings: {output:?}"
    );

    os_sandbox::cleanup();
    os_sandbox::reset_for_test();
}

/// Issue #672: a `~/.cache/pip` symlinked to a directory outside the home is not
/// a write grant (Issue #668 F3), so a write through it fails and the outside
/// directory does not change.
#[cfg(target_os = "linux")]
#[test]
fn linux_pip_cache_symlinked_outside_the_home_cannot_be_written_through() {
    let _serial = sandbox_lock();
    if !require_landlock_backend() {
        eprintln!("SKIP: Landlock backend unavailable");
        return;
    }
    let fixture = fixture();
    let outside = fixture._dir.path().join("outside-pip");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::create_dir_all(fixture.home.join(".cache")).unwrap();
    std::os::unix::fs::symlink(&outside, fixture.home.join(".cache/pip")).unwrap();
    install_available(online_policy(&fixture).with_offline(true));

    let output = run_wrapped(
        &fixture.root,
        &format!(
            "echo pwned > '{}'",
            fixture.home.join(".cache/pip/pwned").display()
        ),
    );
    assert!(
        !output.success(),
        "a write through the outside-pointing pip cache link must fail: {output:?}"
    );
    assert!(
        !outside.join("pwned").exists(),
        "the outside directory must not change"
    );

    os_sandbox::cleanup();
    os_sandbox::reset_for_test();
}

/// Issue #672: the same holds when the parent `~/.cache` is the symlink to a
/// directory outside the home, so `~/.cache/pip` resolves outside it.
#[cfg(target_os = "linux")]
#[test]
fn linux_pip_cache_under_a_cache_dir_symlinked_outside_the_home_cannot_be_written_through() {
    let _serial = sandbox_lock();
    if !require_landlock_backend() {
        eprintln!("SKIP: Landlock backend unavailable");
        return;
    }
    let fixture = fixture();
    let outside = fixture._dir.path().join("outside-cache-dir");
    std::fs::create_dir_all(outside.join("pip")).unwrap();
    std::os::unix::fs::symlink(&outside, fixture.home.join(".cache")).unwrap();
    install_available(online_policy(&fixture).with_offline(true));

    let output = run_wrapped(
        &fixture.root,
        &format!(
            "echo pwned > '{}'",
            fixture.home.join(".cache/pip/pwned").display()
        ),
    );
    assert!(
        !output.success(),
        "a write through the outside-pointing ~/.cache link must fail: {output:?}"
    );
    assert!(
        !outside.join("pip/pwned").exists(),
        "the outside directory must not change"
    );

    os_sandbox::cleanup();
    os_sandbox::reset_for_test();
}

// ---------------------------------------------------------------------------
// Dependency setup runs inside the sandbox and cannot write outside it.
// ---------------------------------------------------------------------------

/// Fake npm: create `node_modules` (so the setup is observed as passed) and try
/// to write through the `escape` symlink. Under the sandbox that write is
/// refused; without it the script would exit non-zero and change the outside
/// fixture.
const FAKE_NPM: &str = r#"#!/bin/sh
mkdir -p node_modules
if printf 'pwned' > escape/secret.txt 2>/dev/null; then
  echo "outside write succeeded" >&2
  exit 1
fi
exit 0
"#;

/// A fake npm that only creates `node_modules`, so an `auto` run outside the
/// sandbox is observed as passed.
const FAKE_NPM_CLEAN: &str = "#!/bin/sh\nmkdir -p node_modules\nexit 0\n";

fn write_executable(root: &Path, name: &str, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = root.join(name);
    std::fs::write(&path, body).unwrap();
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&path, permissions).unwrap();
    path
}

fn write_fake_npm(fixture: &Fixture) -> PathBuf {
    write_executable(&fixture.root, "fake-npm", FAKE_NPM)
}

#[test]
fn dependency_setup_runs_inside_the_sandbox_and_cannot_write_outside() {
    let _serial = sandbox_lock();
    #[cfg(target_os = "macos")]
    if !backend_available() {
        eprintln!("SKIP: nested sandbox; dependency-setup confinement no-op");
        return;
    }
    // The write confinement needs ABI1; a Linux CI runner without a working
    // Landlock fails here instead of silently running the setup unconfined.
    #[cfg(target_os = "linux")]
    if !require_landlock_backend() {
        eprintln!("SKIP: Landlock backend unavailable");
        return;
    }
    let fixture = fixture();
    std::fs::write(
        fixture.root.join("package.json"),
        r#"{"dependencies":{"left-pad":"^1.3.0"}}"#,
    )
    .unwrap();
    let fake_npm = write_fake_npm(&fixture);

    install_available(online_policy(&fixture));

    let requirement = dependency_setup::requirement_for_node_declared_dependencies(
        &fixture.root,
        Some("node"),
        "network test",
        NodeDependencySetupAuthority::PlanSetupStep,
    );
    assert!(requirement.allowed, "{requirement:?}");
    let observation = dependency_setup::run_node_dependency_setup_with_program(
        &fixture.root,
        &requirement,
        &fake_npm,
    );

    assert_eq!(
        observation.status,
        NodeDependencySetupStatus::Passed,
        "the setup must succeed inside the sandbox: {observation:?}"
    );
    assert_eq!(
        std::fs::read_to_string(&fixture.outside_secret).unwrap(),
        "OUTSIDE_658\n",
        "the outside fixture must not change"
    );

    os_sandbox::cleanup();
    os_sandbox::reset_for_test();
}

/// `required` with an unavailable backend fails the dependency setup honestly
/// (the sandbox reason in the existing observation) instead of running it
/// unconfined.
#[test]
fn required_unavailable_sandbox_fails_the_dependency_setup_honestly() {
    let _serial = sandbox_lock();
    let fixture = fixture();
    std::fs::write(
        fixture.root.join("package.json"),
        r#"{"dependencies":{"left-pad":"^1.3.0"}}"#,
    )
    .unwrap();
    let fake_npm = write_fake_npm(&fixture);
    os_sandbox::install_for_test(
        SandboxMode::Required,
        online_policy(&fixture),
        SandboxAvailability::Unavailable(NESTED.to_string()),
    );

    let requirement = dependency_setup::requirement_for_node_declared_dependencies(
        &fixture.root,
        Some("node"),
        "network test",
        NodeDependencySetupAuthority::PlanSetupStep,
    );
    let observation = dependency_setup::run_node_dependency_setup_with_program(
        &fixture.root,
        &requirement,
        &fake_npm,
    );

    assert_eq!(observation.status, NodeDependencySetupStatus::Failed);
    assert!(
        observation
            .primary_reason
            .contains(os_sandbox::UNAVAILABLE_KIND),
        "the failure must name the sandbox: {}",
        observation.primary_reason
    );
    assert!(
        !fixture.root.join("node_modules").exists(),
        "the setup must not run unconfined"
    );

    os_sandbox::reset_for_test();
}

/// `auto` with an unavailable backend runs the dependency setup outside the
/// sandbox and the run status records it as not applied.
#[test]
fn auto_unavailable_sandbox_runs_the_dependency_setup_outside() {
    let _serial = sandbox_lock();
    let fixture = fixture();
    std::fs::write(
        fixture.root.join("package.json"),
        r#"{"dependencies":{"left-pad":"^1.3.0"}}"#,
    )
    .unwrap();
    let fake_npm = write_executable(&fixture.root, "fake-npm", FAKE_NPM_CLEAN);
    os_sandbox::install_for_test(
        SandboxMode::Auto,
        online_policy(&fixture),
        SandboxAvailability::Unavailable(NESTED.to_string()),
    );
    let status = os_sandbox::status().unwrap();
    assert!(
        !status.applied,
        "auto records the sandbox as not applied: {status:?}"
    );
    assert_eq!(status.reason.as_deref(), Some(NESTED));

    let requirement = dependency_setup::requirement_for_node_declared_dependencies(
        &fixture.root,
        Some("node"),
        "network test",
        NodeDependencySetupAuthority::PlanSetupStep,
    );
    let observation = dependency_setup::run_node_dependency_setup_with_program(
        &fixture.root,
        &requirement,
        &fake_npm,
    );

    assert_eq!(
        observation.status,
        NodeDependencySetupStatus::Passed,
        "auto runs the setup outside the sandbox: {observation:?}"
    );
    assert!(fixture.root.join("node_modules").exists());

    os_sandbox::reset_for_test();
}
