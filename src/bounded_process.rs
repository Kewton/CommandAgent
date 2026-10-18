use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::json;

pub const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(20);
const DEFAULT_TIMEOUT_KILL_GRACE: Duration = Duration::from_millis(50);
pub const USER_INTERRUPT_KILL_GRACE: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredServerChild {
    pub pid: u32,
    pub command: String,
    pub origin_phase: String,
    pub workspace_root: PathBuf,
}

static SERVER_CHILDREN: OnceLock<Mutex<Vec<RegisteredServerChild>>> = OnceLock::new();

fn server_children() -> &'static Mutex<Vec<RegisteredServerChild>> {
    SERVER_CHILDREN.get_or_init(|| Mutex::new(Vec::new()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundedProcessOutcomeKind {
    Exited,
    TimedOut,
    Cancelled,
    CommandAbortedByUser,
}

#[derive(Debug)]
pub struct BoundedProcessOutput {
    pub kind: BoundedProcessOutcomeKind,
    pub status: Option<ExitStatus>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub elapsed: Duration,
}

impl BoundedProcessOutput {
    pub fn success(&self) -> bool {
        self.status.is_some_and(|status| status.success())
    }
}

pub fn spawn_child(command: &mut Command) -> io::Result<Child> {
    apply_child_env_allowlist(command);
    configure_child_session(command);
    command.spawn()
}

fn apply_child_env_allowlist(command: &mut Command) {
    let explicit_env = command
        .get_envs()
        .map(|(key, value)| (key.to_os_string(), value.map(|value| value.to_os_string())))
        .collect::<Vec<_>>();
    command.env_clear();
    for (key, value) in std::env::vars_os() {
        if child_env_allowed(&key, EnvSource::Parent) {
            command.env(key, value);
        }
    }
    for (key, value) in explicit_env {
        match value {
            Some(value) if child_env_allowed(&key, EnvSource::Explicit) => {
                command.env(key, value);
            }
            Some(_) => {}
            None => {
                command.env_remove(key);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnvSource {
    Parent,
    Explicit,
}

fn child_env_allowed(key: &std::ffi::OsStr, source: EnvSource) -> bool {
    let Some(key) = key.to_str() else {
        return false;
    };
    matches!(key, "PATH" | "HOME" | "LANG" | "TERM")
        || (source == EnvSource::Parent && matches!(key, "RUSTUP_HOME" | "CARGO_HOME"))
        || key.starts_with("LC_")
        || key.starts_with("npm_config_")
        || (source == EnvSource::Explicit
            && (matches!(
                key,
                "NODE_ENV"
                    | "NODE_OPTIONS"
                    | "NODE_PATH"
                    | "NEXT_TELEMETRY_DISABLED"
                    | "PORT"
                    | "TMPDIR"
            ) || [
                "COMMANDAGENT_FOREIGN_TOOLCHAIN_ROOT",
                "COMMANDAGENT_FOREIGN_TOOLCHAIN_EVENTS",
            ]
            .iter()
            .any(|current| crate::env_compat::matches_current_or_legacy(key, current))))
        || (source == EnvSource::Explicit && test_control_env_allowed(key))
}

#[cfg(test)]
fn test_control_env_allowed(key: &str) -> bool {
    [
        "COMMANDAGENT_BROWSER_PROBE_MOCK_CHILD",
        "COMMANDAGENT_BROWSER_PROBE_MOCK_PORT",
        "COMMANDAGENT_BROWSER_PROBE_MOCK_STATUS",
        "COMMANDAGENT_BROWSER_PROBE_MOCK_DELAY_MS",
    ]
    .iter()
    .any(|current| crate::env_compat::matches_current_or_legacy(key, current))
}

#[cfg(not(test))]
fn test_control_env_allowed(_key: &str) -> bool {
    false
}

pub fn register_server_child(
    child: &Child,
    command: impl Into<String>,
    origin_phase: impl Into<String>,
    workspace_root: &Path,
) {
    let entry = RegisteredServerChild {
        pid: child.id(),
        command: command.into(),
        origin_phase: origin_phase.into(),
        workspace_root: workspace_root.to_path_buf(),
    };
    let mut children = server_children().lock().expect("server child registry");
    children.retain(|child| child.pid != entry.pid);
    children.push(entry);
}

pub fn unregister_server_child(pid: u32) {
    let mut children = server_children().lock().expect("server child registry");
    children.retain(|child| child.pid != pid);
}

pub fn registered_server_child(pid: u32) -> Option<RegisteredServerChild> {
    server_children()
        .lock()
        .expect("server child registry")
        .iter()
        .find(|child| child.pid == pid)
        .cloned()
}

pub fn reap_registered_server_child(
    pid: u32,
    eval_events_path: Option<&Path>,
    reason: &str,
) -> bool {
    let Some(child) = registered_server_child(pid) else {
        return false;
    };
    let reaped = terminate_process_group_by_pid(pid).is_ok();
    unregister_server_child(pid);
    emit_server_reaped(eval_events_path, &child, reason, reaped);
    reaped
}

pub fn reap_registered_server_children_for_workspace(
    eval_events_path: Option<&Path>,
    reason: &str,
    workspace_root: &Path,
) -> usize {
    let children = {
        let mut guard = server_children().lock().expect("server child registry");
        let mut remaining = Vec::new();
        let mut matched = Vec::new();
        for child in std::mem::take(&mut *guard) {
            if same_workspace_root(&child.workspace_root, workspace_root) {
                matched.push(child);
            } else {
                remaining.push(child);
            }
        }
        *guard = remaining;
        matched
    };
    reap_server_children(children, eval_events_path, reason)
}

fn reap_server_children(
    children: Vec<RegisteredServerChild>,
    eval_events_path: Option<&Path>,
    reason: &str,
) -> usize {
    let mut count = 0usize;
    for child in children {
        let reaped = terminate_process_group_by_pid(child.pid).is_ok();
        if reaped {
            count += 1;
        }
        emit_server_reaped(eval_events_path, &child, reason, reaped);
    }
    count
}

fn same_workspace_root(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

fn emit_server_reaped(
    eval_events_path: Option<&Path>,
    child: &RegisteredServerChild,
    reason: &str,
    ok: bool,
) {
    crate::eval_events::emit(
        eval_events_path,
        json!({
            "event": "server_reaped",
            "pid": child.pid,
            "command": child.command,
            "origin_phase": child.origin_phase,
            "workspace_root": child.workspace_root.display().to_string(),
            "reason": reason,
            "ok": ok,
        }),
    );
}

pub fn run_with_timeout(
    command: &mut Command,
    timeout: Duration,
) -> io::Result<BoundedProcessOutput> {
    run_with_timeout_and_cancel(command, timeout, || false)
}

pub fn run_with_timeout_and_cancel<F>(
    command: &mut Command,
    timeout: Duration,
    is_cancelled: F,
) -> io::Result<BoundedProcessOutput>
where
    F: Fn() -> bool,
{
    run_with_timeout_cancel_and_force(command, timeout, is_cancelled, || false)
}

pub fn run_with_timeout_cancel_and_force<F, G>(
    command: &mut Command,
    timeout: Duration,
    is_cancelled: F,
    is_force_cancelled: G,
) -> io::Result<BoundedProcessOutput>
where
    F: Fn() -> bool,
    G: Fn() -> bool,
{
    crate::tui::status_bus::publish_command_started(command, timeout);
    let child = match spawn_child(command) {
        Ok(child) => child,
        Err(err) => {
            crate::tui::status_bus::publish_command_finished();
            return Err(err);
        }
    };
    let output =
        wait_with_timeout_cancel_and_force(child, timeout, is_cancelled, is_force_cancelled);
    crate::tui::status_bus::publish_command_finished();
    output
}

pub fn wait_with_timeout(child: Child, timeout: Duration) -> io::Result<BoundedProcessOutput> {
    wait_with_timeout_and_cancel(child, timeout, || false)
}

pub fn wait_with_timeout_and_cancel<F>(
    child: Child,
    timeout: Duration,
    is_cancelled: F,
) -> io::Result<BoundedProcessOutput>
where
    F: Fn() -> bool,
{
    wait_with_timeout_cancel_and_force(child, timeout, is_cancelled, || false)
}

pub fn wait_with_timeout_cancel_and_force<F, G>(
    child: Child,
    timeout: Duration,
    is_cancelled: F,
    is_force_cancelled: G,
) -> io::Result<BoundedProcessOutput>
where
    F: Fn() -> bool,
    G: Fn() -> bool,
{
    wait_with_timeout_cancel_force_and_grace(
        child,
        timeout,
        is_cancelled,
        is_force_cancelled,
        USER_INTERRUPT_KILL_GRACE,
    )
}

fn wait_with_timeout_cancel_force_and_grace<F, G>(
    child: Child,
    timeout: Duration,
    is_cancelled: F,
    is_force_cancelled: G,
    user_interrupt_grace: Duration,
) -> io::Result<BoundedProcessOutput>
where
    F: Fn() -> bool,
    G: Fn() -> bool,
{
    wait_with_timeout_cancel_force_grace_and_clock(
        child,
        timeout,
        is_cancelled,
        is_force_cancelled,
        user_interrupt_grace,
        &SystemGraceClock,
    )
}

fn wait_with_timeout_cancel_force_grace_and_clock<F, G, C>(
    mut child: Child,
    timeout: Duration,
    is_cancelled: F,
    is_force_cancelled: G,
    user_interrupt_grace: Duration,
    grace_clock: &C,
) -> io::Result<BoundedProcessOutput>
where
    F: Fn() -> bool,
    G: Fn() -> bool,
    C: GraceClock,
{
    let started = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            return output_from_child(
                child.wait_with_output()?,
                BoundedProcessOutcomeKind::Exited,
                started.elapsed(),
            );
        }
        if is_cancelled() {
            terminate_process_group_with_grace(
                &mut child,
                user_interrupt_grace,
                &is_force_cancelled,
                grace_clock,
            );
            return output_from_child(
                child.wait_with_output()?,
                BoundedProcessOutcomeKind::CommandAbortedByUser,
                started.elapsed(),
            );
        }
        if started.elapsed() >= timeout {
            terminate_process_group_with_grace(
                &mut child,
                DEFAULT_TIMEOUT_KILL_GRACE,
                || false,
                grace_clock,
            );
            return output_from_child(
                child.wait_with_output()?,
                BoundedProcessOutcomeKind::TimedOut,
                started.elapsed(),
            );
        }
        thread::sleep(DEFAULT_POLL_INTERVAL);
    }
}

pub fn terminate_process_group(child: &mut Child) {
    terminate_process_group_with_grace(
        child,
        DEFAULT_TIMEOUT_KILL_GRACE,
        || false,
        &SystemGraceClock,
    );
}

/// Resolves a spawned child PID into a process-group id that is safe to
/// signal with a negative `kill` target. The child is spawned with
/// `setsid()`, so its PID equals its group id, but `-1` would signal
/// every signalable process and `0`/`1` would target the caller or init.
///
/// This is a value-returning selector, so cargo-mutants can replace the whole
/// body with `Some(1)` (Issue #673). It is listed in `.cargo/mutants.toml`, and
/// every `kill` caller re-checks the converted target with a non-mutatable
/// `assert!(-pgid < -1)` in the function that calls `kill`.
#[cfg(unix)]
fn signalable_process_group(pid: u32) -> Option<i32> {
    let pgid = i32::try_from(pid).ok()?;
    (pgid > 1).then_some(pgid)
}

pub fn terminate_process_group_by_pid(pid: u32) -> io::Result<()> {
    #[cfg(unix)]
    {
        let Some(pgid) = signalable_process_group(pid) else {
            return Err(io::Error::other(
                "refusing to signal a process group outside the child range",
            ));
        };
        // Last, non-mutatable defense (Issue #673). cargo-mutants does not
        // rewrite comparisons inside `assert!`, and this is written directly in
        // the function that calls `kill`, so a mutated selector (`Some(0)`,
        // `Some(1)`, `Some(-1)`) panics here instead of executing `kill(0, ...)`
        // (the caller's own group) or `kill(-1, ...)` (every signalable
        // process). The `Err` path above is unchanged.
        assert!(
            -pgid < -1,
            "refusing to signal a process group outside the child range"
        );
        // SAFETY: `pgid` is greater than 1 and derived from a child spawned
        // with `setsid()`, so `-pgid` names only that child's own
        // process group, never pid 0/1 or the caller's group.
        unsafe {
            libc::kill(-pgid, libc::SIGTERM);
        }
        thread::sleep(DEFAULT_TIMEOUT_KILL_GRACE);
        // SAFETY: same checked group identifier as the SIGTERM above.
        unsafe {
            libc::kill(-pgid, libc::SIGKILL);
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        Err(io::Error::other(
            "process-group reaping by pid is only supported on Unix",
        ))
    }
}

trait GraceClock {
    type Mark: Copy;

    fn mark(&self) -> Self::Mark;
    fn elapsed(&self, mark: Self::Mark) -> Duration;
    fn wait(&self, duration: Duration);
}

struct SystemGraceClock;

impl GraceClock for SystemGraceClock {
    type Mark = Instant;

    fn mark(&self) -> Self::Mark {
        Instant::now()
    }

    fn elapsed(&self, mark: Self::Mark) -> Duration {
        mark.elapsed()
    }

    fn wait(&self, duration: Duration) {
        thread::sleep(duration);
    }
}

fn terminate_process_group_with_grace<F, C>(
    child: &mut Child,
    grace: Duration,
    is_force_cancelled: F,
    clock: &C,
) where
    F: Fn() -> bool,
    C: GraceClock,
{
    #[cfg(unix)]
    {
        if let Some(pgid) = signalable_process_group(child.id()) {
            // Last, non-mutatable defense (Issue #673), written directly in the
            // function that calls `kill`.
            assert!(
                -pgid < -1,
                "refusing to signal a process group outside the child range"
            );
            // SAFETY: `pgid` is greater than 1 and was created with
            // `configure_child_session()` for this child, so `-pgid` targets
            // only the child's process group, never pid 0/1 or the caller's.
            unsafe {
                libc::kill(-pgid, libc::SIGTERM);
            }
        }
    }
    let grace_started = clock.mark();
    loop {
        if child.try_wait().ok().flatten().is_some() {
            return;
        }
        if is_force_cancelled() || clock.elapsed(grace_started) >= grace {
            break;
        }
        let remaining = grace.saturating_sub(clock.elapsed(grace_started));
        clock.wait(DEFAULT_POLL_INTERVAL.min(remaining));
    }
    #[cfg(unix)]
    {
        if let Some(pgid) = signalable_process_group(child.id()) {
            // Last, non-mutatable defense (Issue #673), written directly in the
            // function that calls `kill`.
            assert!(
                -pgid < -1,
                "refusing to signal a process group outside the child range"
            );
            // SAFETY: the same checked child-derived group identifier as above.
            unsafe {
                libc::kill(-pgid, libc::SIGKILL);
            }
        }
    }
    let _ = child.kill();
}

fn output_from_child(
    output: Output,
    kind: BoundedProcessOutcomeKind,
    elapsed: Duration,
) -> io::Result<BoundedProcessOutput> {
    Ok(BoundedProcessOutput {
        kind,
        status: Some(output.status),
        stdout: output.stdout,
        stderr: output.stderr,
        elapsed,
    })
}

/// Put the child in a new session, and so a new process group, with no
/// controlling terminal (Issue #667).
///
/// `setsid()` sets both the session id and the process-group id to the child's
/// pid, so the child's group id still equals its pid and
/// `terminate_process_group*` keeps signalling exactly the child's own group
/// with `-pid`. Dropping the controlling terminal is the point: the kernel only
/// allows `TIOCSTI` when the fd names the caller's controlling terminal, so a
/// child in its own session can no longer inject keystrokes into the terminal
/// the caller was started from (`/dev/tty` also stops resolving for it).
///
/// This replaces `Command::process_group(0)`: `setsid()` fails with `EPERM` for a
/// process that is already a process-group leader, so the two cannot be combined
/// and `setsid()` alone provides the group. Losing the controlling terminal also
/// makes a terminal stdin readable, so [`detach_terminal_stdin`] points it at
/// `/dev/null` in the same `pre_exec`.
fn configure_child_session(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // SAFETY: `pre_exec` runs after fork and before exec. The closure calls
        // only `setsid`, `isatty`, `open`, `dup2`, and `close`; it does not
        // allocate, lock, or log. A forked child is never a process-group leader
        // (its pid is new), so `setsid` cannot fail with `EPERM`.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                detach_terminal_stdin()
            });
        }
    }
    #[cfg(not(unix))]
    {
        let _ = command;
    }
}

/// Replace a terminal standard input with `/dev/null` in the freshly forked
/// child.
///
/// Before `setsid()` the child sat in a background process group of the caller's
/// session, so a read from the terminal was refused with `SIGTTIN` (or `EIO`
/// once that signal is ignored). After `setsid()` the child has no controlling
/// terminal, so the job-control check no longer applies and a read would succeed,
/// letting a command read the keys the user types during a run. Pointing a
/// terminal stdin at `/dev/null` restores a non-readable stdin. A stdin that is
/// not a terminal (`Stdio::null()`, a pipe, or a file) is left unchanged, as are
/// every standard output and error.
///
/// # pre_exec contract
///
/// This runs between `fork` and `exec`, so it uses only `isatty`, `open`,
/// `dup2`, and `close` and never allocates, locks, or logs. Errors are built with
/// [`std::io::Error::last_os_error`], which does not allocate.
#[cfg(unix)]
fn detach_terminal_stdin() -> std::io::Result<()> {
    // SAFETY: this is the classic `isatty`/`open`/`dup2`/`close` sequence on the
    // child's own descriptors; the path is a fixed static C string.
    unsafe {
        if libc::isatty(0) != 1 {
            return Ok(());
        }
        let devnull = libc::open(c"/dev/null".as_ptr(), libc::O_RDONLY);
        if devnull < 0 {
            return Err(std::io::Error::last_os_error());
        }
        if libc::dup2(devnull, 0) < 0 {
            let error = std::io::Error::last_os_error();
            libc::close(devnull);
            return Err(error);
        }
        if devnull != 0 {
            libc::close(devnull);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Stdio;
    use std::sync::Mutex;

    static ENV_TEST_LOCK: Mutex<()> = Mutex::new(());

    // Test-only upper bound for children expected to terminate on their own. It
    // only waits for child completion; a fast child returns immediately, so
    // load no longer turns a pass into a timeout and the bound never becomes
    // the reason a run is accepted.
    const TEST_TIMEOUT: Duration = Duration::from_secs(30);

    #[test]
    fn run_with_timeout_kills_hanging_child() {
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("sleep 5")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = run_with_timeout(&mut command, Duration::from_millis(100)).unwrap();
        assert_eq!(output.kind, BoundedProcessOutcomeKind::TimedOut);
        assert!(output.elapsed < Duration::from_secs(2), "{output:?}");
    }

    #[test]
    fn child_env_uses_allowlist_without_provider_keys() {
        let _guard = ENV_TEST_LOCK.lock().unwrap();
        let _env_guard = TestEnvGuard::set([
            ("OLLAMA_API_KEY", "ollama-secret"),
            ("GEMINI_API_KEY", "gemini-secret"),
            ("OPENAI_API_KEY", "openai-secret"),
            ("COMMANDAGENT_TEST_UNRELATED_PARENT_SECRET", "parent-secret"),
        ]);
        let cache_dir = tempfile::tempdir().unwrap();
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("env | sort")
            .env("NODE_ENV", "test")
            .env("npm_config_cache", cache_dir.path())
            .env("GEMINI_API_KEY", "explicit-secret")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = run_with_timeout(&mut command, TEST_TIMEOUT).unwrap();

        assert_eq!(output.kind, BoundedProcessOutcomeKind::Exited, "{output:?}");
        assert!(output.success(), "{output:?}");
        let env = String::from_utf8(output.stdout).unwrap();
        assert!(env.contains("PATH="), "{env}");
        assert!(env.contains("NODE_ENV=test"), "{env}");
        assert!(env.contains("npm_config_cache="), "{env}");
        assert!(!env.contains("OLLAMA_API_KEY"), "{env}");
        assert!(!env.contains("GEMINI_API_KEY"), "{env}");
        assert!(!env.contains("OPENAI_API_KEY"), "{env}");
        assert!(
            !env.contains("COMMANDAGENT_TEST_UNRELATED_PARENT_SECRET"),
            "{env}"
        );
    }

    #[test]
    fn child_env_inherits_toolchain_homes_but_not_unknown_variables() {
        let _guard = ENV_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _env_guard = TestEnvGuard::set([
            ("RUSTUP_HOME", "/opt/test-rustup"),
            ("CARGO_HOME", "/opt/test-cargo"),
            ("COMMANDAGENT_TEST_UNKNOWN_PARENT_VAR", "unknown"),
        ]);
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("env | sort")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = run_with_timeout(&mut command, TEST_TIMEOUT).unwrap();

        assert!(output.success(), "{output:?}");
        let env = String::from_utf8(output.stdout).unwrap();
        assert!(env.contains("RUSTUP_HOME=/opt/test-rustup"), "{env}");
        assert!(env.contains("CARGO_HOME=/opt/test-cargo"), "{env}");
        assert!(
            !env.contains("COMMANDAGENT_TEST_UNKNOWN_PARENT_VAR"),
            "{env}"
        );
    }

    /// Issue #669: a toolchain installed outside `$HOME` (the official rust
    /// image: `RUSTUP_HOME=/usr/local/rustup`, `CARGO_HOME=/usr/local/cargo`)
    /// must run `cargo --version` through the real OS sandbox wrap. Without the
    /// two variables the rustup proxy looks under `$HOME/.rustup` and fails.
    /// The test reports why it did not run instead of passing silently.
    #[cfg(target_os = "linux")]
    #[test]
    fn sandboxed_cargo_runs_with_toolchain_outside_home() {
        use crate::tools::os_sandbox::{self, SandboxAvailability, SandboxMode, SandboxPolicy};

        struct ResetSandbox;
        impl Drop for ResetSandbox {
            fn drop(&mut self) {
                os_sandbox::reset_for_test();
            }
        }

        let _guard = ENV_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        let outside_home = |name: &str| {
            std::env::var_os(name)
                .map(PathBuf::from)
                .filter(|path| path.is_absolute() && !path.starts_with(&home))
        };
        if outside_home("RUSTUP_HOME").is_none() || outside_home("CARGO_HOME").is_none() {
            eprintln!(
                "SKIPPED sandboxed_cargo_runs_with_toolchain_outside_home: RUSTUP_HOME and \
                 CARGO_HOME are not both set outside $HOME"
            );
            return;
        }
        if let SandboxAvailability::Unavailable(reason) = os_sandbox::probe() {
            eprintln!(
                "SKIPPED sandboxed_cargo_runs_with_toolchain_outside_home: no OS sandbox \
                 backend: {reason}"
            );
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        let run_tmp = dir.path().join("run-tmp");
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::create_dir_all(&run_tmp).unwrap();
        let _reset = ResetSandbox;
        os_sandbox::install_for_test(
            SandboxMode::Required,
            SandboxPolicy::with_home(&workspace, &run_tmp, home),
            SandboxAvailability::Available,
        );

        let mut process = Command::new("sh");
        process
            .arg("-c")
            .arg("cargo --version")
            .current_dir(&workspace);
        let mut process = os_sandbox::wrap_command(process).expect("available sandbox wraps");
        process
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = run_with_timeout(&mut process, TEST_TIMEOUT).unwrap();

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        assert!(output.success(), "stdout={stdout:?} stderr={stderr:?}");
        assert!(stdout.starts_with("cargo "), "stdout={stdout:?}");
    }

    #[test]
    fn child_env_unsets_inherited_node_env() {
        let current_exe = std::env::current_exe().unwrap();
        let status = Command::new(current_exe)
            .args([
                "--ignored",
                "--exact",
                "bounded_process::tests::child_env_unsets_inherited_node_env_child",
                "--nocapture",
            ])
            .env("NODE_ENV", "production")
            .status()
            .unwrap();
        assert!(status.success(), "{status}");
    }

    #[test]
    #[ignore]
    fn child_env_unsets_inherited_node_env_child() {
        let mut inherited = Command::new("sh");
        inherited
            .arg("-c")
            .arg("printf '%s' \"${NODE_ENV-unset}\"")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let inherited_output = run_with_timeout(&mut inherited, TEST_TIMEOUT).unwrap();
        assert_eq!(
            inherited_output.kind,
            BoundedProcessOutcomeKind::Exited,
            "{inherited_output:?}"
        );
        assert!(inherited_output.success(), "{inherited_output:?}");
        assert_eq!(inherited_output.stdout, b"unset");

        let mut explicit = Command::new("sh");
        explicit
            .arg("-c")
            .arg("printf '%s' \"${NODE_ENV-unset}\"")
            .env("NODE_ENV", "test")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let explicit_output = run_with_timeout(&mut explicit, TEST_TIMEOUT).unwrap();
        assert_eq!(
            explicit_output.kind,
            BoundedProcessOutcomeKind::Exited,
            "{explicit_output:?}"
        );
        assert!(explicit_output.success(), "{explicit_output:?}");
        assert_eq!(explicit_output.stdout, b"test");
    }

    #[test]
    fn cancelled_child_is_classified_as_user_abort() {
        struct TestGraceClock(std::cell::Cell<Duration>);

        impl GraceClock for TestGraceClock {
            type Mark = Duration;

            fn mark(&self) -> Self::Mark {
                self.0.get()
            }

            fn elapsed(&self, mark: Self::Mark) -> Duration {
                self.0.get().saturating_sub(mark)
            }

            fn wait(&self, duration: Duration) {
                self.0.set(self.0.get() + duration);
            }
        }

        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("trap '' TERM; printf 'ready\\n'; while :; do :; done")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let grace_clock = TestGraceClock(std::cell::Cell::new(Duration::ZERO));
        let mut child = spawn_child(&mut command).unwrap();
        let mut ready = [0_u8; 6];
        std::io::Read::read_exact(child.stdout.as_mut().unwrap(), &mut ready).unwrap();
        assert_eq!(&ready, b"ready\n");
        let output = wait_with_timeout_cancel_force_grace_and_clock(
            child,
            Duration::from_secs(30),
            || true,
            || false,
            Duration::from_millis(100),
            &grace_clock,
        )
        .unwrap();

        assert_eq!(output.kind, BoundedProcessOutcomeKind::CommandAbortedByUser);
        assert_eq!(grace_clock.0.get(), Duration::from_millis(100));
        assert_eq!(USER_INTERRUPT_KILL_GRACE, Duration::from_secs(5));
    }

    #[test]
    fn registered_server_children_are_reaped_with_origin_telemetry() {
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join("events.jsonl");
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("sleep 5")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = spawn_child(&mut command).unwrap();
        let pid = child.id();

        register_server_child(&child, "npm run dev", "phase-a", dir.path());
        assert_eq!(
            registered_server_child(pid).unwrap().origin_phase,
            "phase-a"
        );

        assert_eq!(
            reap_registered_server_children_for_workspace(
                Some(&events),
                "terminal_guard",
                dir.path()
            ),
            1
        );
        let _ = child.wait();
        assert!(registered_server_child(pid).is_none());
        let event_text = std::fs::read_to_string(events).unwrap();
        assert!(event_text.contains("\"event\":\"server_reaped\""));
        assert!(event_text.contains("\"origin_phase\":\"phase-a\""));
        assert!(event_text.contains("\"reason\":\"terminal_guard\""));
    }

    #[test]
    fn workspace_scoped_reap_preserves_other_run_children() {
        let dir_a = tempfile::tempdir().unwrap();
        let dir_b = tempfile::tempdir().unwrap();
        let events_a = dir_a.path().join("events.jsonl");
        let events_b = dir_b.path().join("events.jsonl");
        let mut command_a = Command::new("sh");
        command_a
            .arg("-c")
            .arg("sleep 5")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut command_b = Command::new("sh");
        command_b
            .arg("-c")
            .arg("sleep 5")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child_a = spawn_child(&mut command_a).unwrap();
        let mut child_b = spawn_child(&mut command_b).unwrap();
        let pid_a = child_a.id();
        let pid_b = child_b.id();

        register_server_child(&child_a, "npm run dev", "phase-a", dir_a.path());
        register_server_child(&child_b, "npm run dev", "phase-b", dir_b.path());

        assert_eq!(
            reap_registered_server_children_for_workspace(
                Some(&events_a),
                "phase_transition",
                dir_a.path()
            ),
            1
        );
        let _ = child_a.wait();
        assert!(registered_server_child(pid_a).is_none());
        assert!(registered_server_child(pid_b).is_some());

        assert_eq!(
            reap_registered_server_children_for_workspace(Some(&events_b), "cleanup", dir_b.path()),
            1
        );
        let _ = child_b.wait();
        assert!(registered_server_child(pid_b).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn process_group_guard_refuses_non_child_group_ids() {
        assert_eq!(signalable_process_group(0), None);
        assert_eq!(signalable_process_group(1), None);
        assert_eq!(signalable_process_group(2), Some(2));
        assert!(terminate_process_group_by_pid(1).is_err());
    }

    /// Issue #673 対処の方向 4: pass 0 (the caller's own group) and 1 (init)
    /// straight to the function that calls `kill`, skipping the selector's own
    /// assertions, so a mutated `signalable_process_group` reaches the
    /// non-mutatable `assert!` in front of `kill` and panics there instead of
    /// signalling. Runs only in the isolated container.
    #[cfg(unix)]
    #[test]
    fn dangerous_value_terminate_process_group_by_pid_refuses_init_and_caller_group() {
        assert!(terminate_process_group_by_pid(0).is_err());
        assert!(terminate_process_group_by_pid(1).is_err());
    }

    /// Issue #673 sentinel test. It exercises a real `kill(-pgid, ...)`, so it
    /// must run only inside the isolated container: it panics unless
    /// `COMMANDAGENT_ISSUE673_CONTAINER=1` is set, and it is `#[ignore]`d so
    /// `cargo test` never runs it in a host process. Docker command (from the
    /// repository root of a container that shares no host PID namespace):
    ///
    /// ```text
    /// COMMANDAGENT_ISSUE673_CONTAINER=1 \
    ///   cargo test --lib -- --ignored --exact \
    ///   bounded_process::tests::dangerous_sentinel_same_uid_other_group_survives
    /// ```
    ///
    /// It proves the scope of the signal: a same-UID process in a **different**
    /// process group survives, while the target child's group and its
    /// grandchild are terminated.
    #[cfg(unix)]
    #[test]
    #[ignore = "run only inside the isolated container (COMMANDAGENT_ISSUE673_CONTAINER=1)"]
    fn dangerous_sentinel_same_uid_other_group_survives() {
        if std::env::var("COMMANDAGENT_ISSUE673_CONTAINER")
            .ok()
            .as_deref()
            != Some("1")
        {
            panic!("run only inside the isolated container");
        }
        use std::os::unix::process::CommandExt;

        // Sentinel: same UID, its own process group, and not the target group.
        let mut sentinel = Command::new("sh");
        sentinel
            .arg("-c")
            .arg("sleep 30")
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut sentinel = sentinel.spawn().expect("spawn the sentinel");

        // Target: a `setsid` child whose `sh` also leaves a grandchild in the
        // same process group.
        let mut target = Command::new("sh");
        target
            .arg("-c")
            .arg("sh -c 'sleep 30' & sleep 30")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut target = spawn_child(&mut target).expect("spawn the target");
        let target_pid = target.id();
        thread::sleep(Duration::from_millis(300));

        terminate_process_group_by_pid(target_pid).expect("reap the target group");
        let _ = target.wait();
        assert!(
            !group_exists(target_pid),
            "the target group must be gone after the reap"
        );
        assert!(
            sentinel.try_wait().expect("poll the sentinel").is_none(),
            "the same-UID sentinel in another group must survive"
        );

        let _ = sentinel.kill();
        let _ = sentinel.wait();
    }

    #[cfg(unix)]
    fn group_exists(pgid: u32) -> bool {
        let Ok(pgid) = i32::try_from(pgid) else {
            return false;
        };
        // SAFETY: signal 0 only probes for the group's existence.
        let rc = unsafe { libc::kill(-pgid, 0) };
        rc == 0
    }

    struct TestEnvGuard {
        saved: Vec<(&'static str, Option<std::ffi::OsString>)>,
    }

    impl TestEnvGuard {
        fn set<const N: usize>(pairs: [(&'static str, &'static str); N]) -> Self {
            let saved = pairs
                .iter()
                .map(|(key, _)| (*key, std::env::var_os(key)))
                .collect();
            for (key, value) in pairs {
                unsafe {
                    std::env::set_var(key, value);
                }
            }
            Self { saved }
        }
    }

    impl Drop for TestEnvGuard {
        fn drop(&mut self) {
            for (key, previous) in &self.saved {
                unsafe {
                    match previous {
                        Some(value) => std::env::set_var(key, value),
                        None => std::env::remove_var(key),
                    }
                }
            }
        }
    }
}
