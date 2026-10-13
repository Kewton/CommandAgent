//! OS write and read sandbox for model Bash and verifier commands (Issues #655
//! and #657).
//!
//! This is the common leaf module of the #502 confinement track. It wraps the
//! `sh -c` launch point of the model Bash tool (including the `bash:verify`
//! interval, which reuses the same function) and of verifier commands, so an
//! indirect write the lexical guard cannot see still fails, and since #657 an
//! indirect read of a home secret location (or, on macOS, a workspace
//! credential) fails as well, when the OS backend is available.
//!
//! Design (issues #655 and #657, decisions from #502):
//! - The backend writes a fixed profile; locations travel only through `-D`
//!   parameters, never embedded in the profile string.
//! - Availability is probed once at run start and stored process-wide, together
//!   with the mode, so a Bash spawned on another thread still sees it.
//! - `auto` continues with lexical checks only when the backend is unavailable
//!   and records the reason; `required` fails the command honestly with
//!   [`UNAVAILABLE_KIND`]; `off` keeps the previous behavior.
//! - The lexical guard is not weakened or replaced: it still runs first and can
//!   refuse a command before the sandbox is consulted.
//! - macOS uses Seatbelt (stage 1a); Linux uses Landlock (stage 1b). On any
//!   other platform the backend is reported as unavailable rather than silently
//!   pretending isolation.
//! - The read layer is a deny list shared by both backends
//!   ([`secret_locations`]); it is applied together with the write layer, and
//!   `bash_sandbox_status` reports it through the existing `reason` field (a
//!   `reason` that names the read isolation, or `None` for a fully confined
//!   sandbox).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use clap::ValueEnum;

#[cfg(target_os = "linux")]
pub mod landlock;
#[cfg(target_os = "macos")]
pub mod seatbelt;
/// The shared read-deny list (Issue #657). Compiled only where a read backend
/// exists, so it is never dead code on other platforms.
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub mod secret_locations;

/// The macOS backend name used in `bash_sandbox_status`.
pub const BACKEND_SEATBELT: &str = "seatbelt";
/// The Linux backend name used in `bash_sandbox_status`.
pub const BACKEND_LANDLOCK: &str = "landlock";
/// The backend name on platforms without an OS sandbox backend in this stage.
pub const BACKEND_NONE: &str = "none";

/// The honest failure kind emitted when `required` cannot apply the sandbox.
pub const UNAVAILABLE_KIND: &str = "bash_sandbox_unavailable";

/// The reason recorded when the sandbox is explicitly disabled.
const DISABLED_REASON: &str = "disabled by --sandbox off";
/// The reason recorded once a run has finished and its sandbox is torn down.
const RUN_FINISHED_REASON: &str = "run finished; the OS sandbox is no longer applied";
/// The refusal reason for a `required` run whose backend is available but whose
/// policy is gone (for example after [`cleanup`]).
const MISSING_POLICY_REASON: &str = "the OS sandbox has no policy to apply";
/// The reason recorded on platforms with no OS backend in 1a/1b/2.
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
const NO_PLATFORM_BACKEND: &str = "no OS sandbox backend is available on this platform (macOS Seatbelt and Linux Landlock are supported)";

/// `--sandbox auto|required|off`. `auto` is the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SandboxMode {
    /// Use the backend when available, otherwise continue with lexical checks.
    Auto,
    /// Refuse Bash and verifier commands when the backend is unavailable.
    Required,
    /// Do not use the OS sandbox; keep the previous behavior.
    Off,
}

impl SandboxMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Required => "required",
            Self::Off => "off",
        }
    }

    /// Whether this mode asks the backend to be applied.
    const fn uses_backend(self) -> bool {
        !matches!(self, Self::Off)
    }
}

/// Whether the backend could be started. The reason is a human string and never
/// contains a path or a secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxAvailability {
    Available,
    Unavailable(String),
}

/// The resolved write-allow list, the run temp directory, and the home whose
/// secret locations are denied for reading (Issue #657).
#[derive(Debug, Clone)]
pub struct SandboxPolicy {
    pub workspace_root: PathBuf,
    pub run_tmp_dir: PathBuf,
    pub caches: CachePaths,
    /// The home whose secret locations the read backend denies. Resolved from
    /// `HOME`; tests pass an explicit tempdir so they never touch the real home.
    pub home: PathBuf,
}

impl SandboxPolicy {
    /// Build a policy whose cache locations and read-deny home are resolved from
    /// `HOME`.
    pub fn new(workspace_root: &Path, run_tmp_dir: &Path) -> Self {
        Self::with_home(workspace_root, run_tmp_dir, home_dir())
    }

    /// Build a policy with an explicit home, so a test can point the read deny
    /// list at a tempdir without racing on the process `HOME`.
    pub fn with_home(workspace_root: &Path, run_tmp_dir: &Path, home: PathBuf) -> Self {
        Self {
            workspace_root: workspace_root.to_path_buf(),
            run_tmp_dir: run_tmp_dir.to_path_buf(),
            caches: resolve_cache_paths(&home),
            home,
        }
    }
}

/// The process home, or an empty path when `HOME` is unset. An empty home makes
/// the read deny list empty ([`secret_locations::home_secret_locations`]).
fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
}

/// Known dependency caches allowed for writing (decision 2). pnpm and yarn
/// stores are deliberately excluded in this stage.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CachePaths {
    pub cargo_registry: Option<PathBuf>,
    pub cargo_git: Option<PathBuf>,
    pub cargo_package_cache: Option<PathBuf>,
    pub npm: Option<PathBuf>,
    pub pip: Option<PathBuf>,
    pub playwright: Option<PathBuf>,
}

/// The outcome of a run-start install, also the payload of `bash_sandbox_status`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxStatus {
    pub mode: SandboxMode,
    pub backend: &'static str,
    pub applied: bool,
    /// Whether the read isolation of Issue #657 is in force for the whole
    /// workspace. It is applied together with the write backend, but is `false`
    /// when no home secret location is known or, on Linux, when a writable
    /// location is a home secret or an ancestor of one (its subtree then stays
    /// readable), so the record never claims isolation the read layer cannot
    /// provide. `bash_sandbox_status` reports it through the existing
    /// [`SandboxStatus::reason`] field rather than a new event field.
    pub read_isolation: bool,
    pub reason: Option<String>,
}

impl SandboxStatus {
    fn from_parts(
        mode: SandboxMode,
        availability: &SandboxAvailability,
        note: Option<String>,
        read_isolation_effective: bool,
    ) -> Self {
        let applied = mode.uses_backend() && matches!(availability, SandboxAvailability::Available);
        let reason = if mode.uses_backend() {
            match availability {
                SandboxAvailability::Unavailable(reason) => Some(reason.clone()),
                // When the backend is applied, an empty reason means fully confined;
                // a backend note names the best-effort operations it cannot close.
                SandboxAvailability::Available => note,
            }
        } else {
            None
        };
        Self {
            mode,
            backend: backend_name(),
            applied,
            read_isolation: applied && read_isolation_effective,
            reason,
        }
    }
}

/// A refusal returned when `required` cannot apply the OS sandbox.
#[derive(Debug, Clone)]
pub struct SandboxRefusal {
    pub reason: String,
}

impl SandboxRefusal {
    /// The honest `bash_sandbox_unavailable` summary returned to the caller.
    pub fn summary(&self) -> String {
        format!("{UNAVAILABLE_KIND}: {}", self.reason)
    }
}

struct Installed {
    policy: Option<SandboxPolicy>,
    mode: SandboxMode,
    availability: SandboxAvailability,
}

static INSTALLED: Mutex<Option<Installed>> = Mutex::new(None);

fn backend_name() -> &'static str {
    if cfg!(target_os = "macos") {
        BACKEND_SEATBELT
    } else if cfg!(target_os = "linux") {
        BACKEND_LANDLOCK
    } else {
        BACKEND_NONE
    }
}

/// The read-isolation clause recorded in `bash_sandbox_status`'s reason while
/// the read layer of Issue #657 is effective.
const READ_ISOLATION_APPLIED: &str = "read isolation applied";
/// The clause recorded when no home secret location is known to deny.
#[cfg(any(target_os = "macos", target_os = "linux"))]
const READ_ISOLATION_NO_HOME: &str =
    "read isolation is not effective: no home secret location is known";

/// Whether the read layer is effective for a policy, and the clause that reports
/// it in the `bash_sandbox_status` reason.
struct ReadIsolation {
    effective: bool,
    clause: String,
}

/// Classify the read isolation for a policy. It is ineffective when the home is
/// unknown (an empty deny list) or, on Linux, when a write-allowed location is a
/// home secret or an ancestor of one: the expansion cannot deny a location that
/// the write allow list already grants read on, so those secrets stay readable.
/// On macOS the read deny is applied after `(allow default)`, so a write-allowed
/// location does not re-open a secret.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn read_isolation(policy: Option<&SandboxPolicy>) -> ReadIsolation {
    let Some(policy) = policy else {
        return ReadIsolation {
            effective: false,
            clause: "read isolation is not effective: no sandbox policy".to_string(),
        };
    };
    let denied = secret_locations::home_secret_locations(&policy.home);
    if denied.is_empty() {
        return ReadIsolation {
            effective: false,
            clause: READ_ISOLATION_NO_HOME.to_string(),
        };
    }
    #[cfg(target_os = "linux")]
    {
        let gaps = landlock::read_isolation_gaps(&landlock::allowed_paths(policy), &denied);
        if !gaps.is_empty() {
            return ReadIsolation {
                effective: false,
                clause: format!(
                    "read isolation is not effective for {} home secret location(s) overlapped by a writable location",
                    gaps.len()
                ),
            };
        }
    }
    ReadIsolation {
        effective: true,
        clause: READ_ISOLATION_APPLIED.to_string(),
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn read_isolation(_policy: Option<&SandboxPolicy>) -> ReadIsolation {
    ReadIsolation {
        effective: false,
        clause: "read isolation is not effective: no backend on this platform".to_string(),
    }
}

/// The note recorded in the `bash_sandbox_status` reason while the sandbox is
/// applied. On Linux it names the write operations an older ABI cannot close and
/// the read-isolation clause; a fully closed ABI with effective reads has no
/// note (`None`), which the event reads as fully confined. On other platforms it
/// names the read-isolation clause.
fn backend_note(read: &ReadIsolation) -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        match landlock::limitation_note() {
            Some(note) => Some(format!("{note}; {}", read.clause)),
            None if read.effective => None,
            None => Some(read.clause.clone()),
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        Some(read.clause.clone())
    }
}

/// Probe the backend once. On macOS this starts `sandbox-exec` with a do-nothing
/// profile; a nested sandbox fails with rc=71, which is reported as unavailable.
/// On Linux this queries the Landlock ABI and confirms a child can enforce it.
pub fn probe() -> SandboxAvailability {
    #[cfg(target_os = "macos")]
    {
        seatbelt::probe()
    }
    #[cfg(target_os = "linux")]
    {
        landlock::probe()
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        SandboxAvailability::Unavailable(NO_PLATFORM_BACKEND.to_string())
    }
}

/// Install the process-wide sandbox state for a run and create its temp
/// directory. Returns the status recorded in `bash_sandbox_status`.
///
/// In a unit-test build this is a no-op: many lib tests run in one process and
/// must not observe each other's process-wide state. Integration tests compile
/// the library without `cfg(test)` and exercise the real install, and the
/// `install_for_test` seam drives the explicit states.
pub fn install(mode: SandboxMode, workspace_root: &Path, state_dir: &Path) -> SandboxStatus {
    #[cfg(not(test))]
    {
        install_impl(mode, workspace_root, state_dir)
    }
    #[cfg(test)]
    {
        let _ = (workspace_root, state_dir);
        SandboxStatus {
            mode,
            backend: backend_name(),
            applied: false,
            read_isolation: false,
            reason: None,
        }
    }
}

#[cfg(not(test))]
fn install_impl(mode: SandboxMode, workspace_root: &Path, state_dir: &Path) -> SandboxStatus {
    let run_tmp_dir = state_dir
        .join("sandbox")
        .join(format!("run-{}", uuid::Uuid::now_v7()));
    if let Err(err) = std::fs::create_dir_all(&run_tmp_dir) {
        let availability =
            SandboxAvailability::Unavailable(format!("run temp directory is unavailable: {err}"));
        return store(mode, None, availability);
    }
    let policy = SandboxPolicy::new(workspace_root, &run_tmp_dir);
    let availability = if mode.uses_backend() {
        probe()
    } else {
        SandboxAvailability::Unavailable(DISABLED_REASON.to_string())
    };
    store(mode, Some(policy), availability)
}

/// Install an explicit policy and availability. Used by tests to simulate an
/// unavailable backend (for example a nested sandbox rc=71).
pub fn install_for_test(
    mode: SandboxMode,
    policy: SandboxPolicy,
    availability: SandboxAvailability,
) -> SandboxStatus {
    store(mode, Some(policy), availability)
}

/// The reason recorded when a workflow child node runs in a workspace other than
/// the one the run started in, so the fixed profile would confine it to the wrong
/// workspace.
#[cfg(not(test))]
const DIFFERENT_WORKSPACE_REASON: &str =
    "workflow node runs in a workspace other than the run workspace; the OS sandbox is not applied";

/// Reconcile the installed sandbox with the workspace a workflow child node runs
/// in.
///
/// The profile and the write-allow list are fixed at run start to the run's
/// workspace, so applying them to a node running in another workspace would
/// confine it to the wrong place. When the node's workspace differs, record the
/// sandbox as not applied instead: `auto` continues with lexical checks only and
/// `required` fails the node's Bash and verifier commands, so the record never
/// claims isolation that was not in force. When the workspace matches, nothing
/// changes. Returns the status now in effect, or `None` when no sandbox was
/// installed.
///
/// In a unit-test build this is a no-op, matching [`install`].
pub fn reconcile_workspace(workspace_root: &Path) -> Option<SandboxStatus> {
    #[cfg(not(test))]
    {
        reconcile_workspace_impl(workspace_root)
    }
    #[cfg(test)]
    {
        let _ = workspace_root;
        None
    }
}

#[cfg(not(test))]
fn reconcile_workspace_impl(workspace_root: &Path) -> Option<SandboxStatus> {
    let mut installed = INSTALLED.lock().expect("sandbox state");
    let (mode, availability, policy, mismatched) = {
        let state = installed.as_ref()?;
        let mismatched = state
            .policy
            .as_ref()
            .is_some_and(|policy| policy.workspace_root != workspace_root);
        (
            state.mode,
            state.availability.clone(),
            state.policy.clone(),
            mismatched,
        )
    };
    let read = read_isolation(policy.as_ref());
    if !mismatched || !mode.uses_backend() {
        return Some(SandboxStatus::from_parts(
            mode,
            &availability,
            backend_note(&read),
            read.effective,
        ));
    }
    if !matches!(availability, SandboxAvailability::Available) {
        // The backend is already unavailable; keep the reason the run started
        // with instead of overwriting it.
        return Some(SandboxStatus::from_parts(
            mode,
            &availability,
            backend_note(&read),
            read.effective,
        ));
    }
    let availability = SandboxAvailability::Unavailable(DIFFERENT_WORKSPACE_REASON.to_string());
    let status = SandboxStatus::from_parts(mode, &availability, None, false);
    *installed = Some(Installed {
        policy,
        mode,
        availability,
    });
    Some(status)
}

fn store(
    mode: SandboxMode,
    policy: Option<SandboxPolicy>,
    availability: SandboxAvailability,
) -> SandboxStatus {
    let read = read_isolation(policy.as_ref());
    let status =
        SandboxStatus::from_parts(mode, &availability, backend_note(&read), read.effective);
    let mut installed = INSTALLED.lock().expect("sandbox state");
    *installed = Some(Installed {
        policy,
        mode,
        availability,
    });
    status
}

/// Record that the installed backend is no longer applied, for example when a
/// per-command ruleset could not be built even though the run-start probe said
/// the backend was available. Mirrors [`reconcile_workspace`]: the availability
/// becomes [`SandboxAvailability::Unavailable`] with the reason, so
/// [`is_applied`] turns false and later `required` commands refuse. The event
/// name and schema are unchanged and no field is added.
#[cfg(target_os = "linux")]
fn mark_backend_unavailable(reason: String) {
    let mut installed = INSTALLED.lock().expect("sandbox state");
    let Some(state) = installed.as_mut() else {
        return;
    };
    if matches!(state.availability, SandboxAvailability::Available) {
        state.availability = SandboxAvailability::Unavailable(reason);
    }
}

/// Clear the process-wide state. Tests only.
pub fn reset_for_test() {
    let mut installed = INSTALLED.lock().expect("sandbox state");
    *installed = None;
}

/// Remove the installed run temp directory and end the run's sandbox. After this
/// the sandbox is no longer reported as applied and `required` refuses instead of
/// passing a command through, so a child spawned while shutting down never runs
/// without the isolation the run promised. Best effort and idempotent, so it is
/// safe on every exit path (success, failure, interruption, or an unwinding
/// panic). The event stream and the workspace are untouched.
pub fn cleanup() {
    let mut installed = INSTALLED.lock().expect("sandbox state");
    let Some(state) = installed.as_mut() else {
        return;
    };
    if matches!(state.availability, SandboxAvailability::Available) {
        // The backend was applied; mark the run over so the applied verdict is
        // no longer reported and `required` refuses instead of passing through.
        state.availability = SandboxAvailability::Unavailable(RUN_FINISHED_REASON.to_string());
    }
    let run_tmp_dir = state.policy.take().map(|policy| policy.run_tmp_dir);
    drop(installed);
    if let Some(run_tmp_dir) = run_tmp_dir {
        let _ = std::fs::remove_dir_all(run_tmp_dir);
    }
}

/// Removes the run temp directory when dropped, so a run leaves no temp
/// directory behind regardless of how it ends.
pub struct SandboxCleanupGuard;

impl Drop for SandboxCleanupGuard {
    fn drop(&mut self) {
        cleanup();
    }
}

/// A guard that runs [`cleanup`] on drop.
pub fn cleanup_guard() -> SandboxCleanupGuard {
    SandboxCleanupGuard
}

/// The status recorded at run start, if any.
pub fn status() -> Option<SandboxStatus> {
    let installed = INSTALLED.lock().expect("sandbox state");
    installed.as_ref().map(|installed| {
        let read = read_isolation(installed.policy.as_ref());
        SandboxStatus::from_parts(
            installed.mode,
            &installed.availability,
            backend_note(&read),
            read.effective,
        )
    })
}

/// Whether the backend is currently applied for this run.
pub fn is_applied() -> bool {
    status().is_some_and(|status| status.applied)
}

/// The one-time startup warning for `auto` when the backend is unavailable.
pub fn startup_warning() -> Option<String> {
    let status = status()?;
    if status.mode == SandboxMode::Auto && !status.applied {
        let reason = status.reason.as_deref().unwrap_or("unknown reason");
        Some(format!(
            "warning: OS sandbox is unavailable ({reason}); Bash and verifier commands continue with lexical path checks only"
        ))
    } else {
        None
    }
}

/// Apply the installed sandbox to a prepared command.
///
/// `Ok(command)` is the (possibly wrapped) command to spawn. When `required` is
/// installed but the backend is unavailable, returns `Err` so the caller fails
/// the command instead of claiming isolation.
pub fn wrap_command(command: Command) -> Result<Command, SandboxRefusal> {
    let (mode, availability) = {
        let installed = INSTALLED.lock().expect("sandbox state");
        match installed.as_ref() {
            Some(installed) => (installed.mode, installed.availability.clone()),
            None => return Ok(command),
        }
    };
    if !mode.uses_backend() {
        return Ok(command);
    }
    match availability {
        SandboxAvailability::Unavailable(reason) => {
            if mode == SandboxMode::Required {
                Err(SandboxRefusal { reason })
            } else {
                Ok(command)
            }
        }
        SandboxAvailability::Available => {
            let policy = {
                let installed = INSTALLED.lock().expect("sandbox state");
                installed
                    .as_ref()
                    .and_then(|installed| installed.policy.clone())
            };
            match policy {
                Some(policy) => apply_backend(command, &policy, mode),
                None => {
                    // The state says the backend is available but no policy is
                    // installed (for example once the run's sandbox is torn
                    // down). `required` must refuse rather than pass a command
                    // through unconfined.
                    if mode == SandboxMode::Required {
                        Err(SandboxRefusal {
                            reason: MISSING_POLICY_REASON.to_string(),
                        })
                    } else {
                        Ok(command)
                    }
                }
            }
        }
    }
}

fn apply_backend(
    command: Command,
    policy: &SandboxPolicy,
    mode: SandboxMode,
) -> Result<Command, SandboxRefusal> {
    #[cfg(target_os = "linux")]
    {
        let mut command = command;
        match landlock::wrap(&mut command, policy) {
            Ok(()) => Ok(command),
            // The ruleset could not be built even though the backend probed as
            // available. Record the run as no longer applied, so `is_applied()`
            // turns false and later commands do not claim isolation; `required`
            // refuses, `auto` continues with lexical checks only.
            Err(reason) => {
                mark_backend_unavailable(reason.clone());
                if mode == SandboxMode::Required {
                    Err(SandboxRefusal { reason })
                } else {
                    Ok(command)
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        let _ = mode;
        Ok(seatbelt::wrap(command, policy))
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = (policy, mode);
        Ok(command)
    }
}

fn resolve_cache_paths(home: &Path) -> CachePaths {
    if home.as_os_str().is_empty() {
        return CachePaths::default();
    }
    let home = home.to_path_buf();
    CachePaths {
        cargo_registry: existing_dir(home.join(".cargo/registry")),
        cargo_git: existing_dir(home.join(".cargo/git")),
        cargo_package_cache: existing_file(home.join(".cargo/.package-cache")),
        npm: existing_dir(home.join(".npm")),
        pip: existing_dir(home.join("Library/Caches/pip")),
        playwright: existing_dir(home.join("Library/Caches/ms-playwright")),
    }
}

fn existing_dir(path: PathBuf) -> Option<PathBuf> {
    path.is_dir().then_some(path)
}

fn existing_file(path: PathBuf) -> Option<PathBuf> {
    path.is_file().then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// The installed state is process-wide, so these tests serialize.
    static SANDBOX_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn policy() -> SandboxPolicy {
        SandboxPolicy {
            workspace_root: PathBuf::from("/tmp/os-sandbox-test-work"),
            run_tmp_dir: PathBuf::from("/tmp/os-sandbox-test-tmp"),
            caches: CachePaths::default(),
            home: PathBuf::from("/tmp/os-sandbox-test-home"),
        }
    }

    /// `SandboxPolicy::new` must take the home from the process `HOME` (through
    /// `home_dir`); otherwise the read-deny list is built from an empty home and
    /// every secret stays readable. Reads only the `HOME` value: it does not read
    /// or list the home directory, and it does not mutate the environment.
    #[test]
    fn policy_new_resolves_home_from_the_environment() {
        let Some(home) = std::env::var_os("HOME") else {
            eprintln!("SKIP: HOME is not set");
            return;
        };
        let policy = SandboxPolicy::new(
            Path::new("/commandagent-home-test-ws"),
            Path::new("/commandagent-home-test-tmp"),
        );
        assert_eq!(policy.home, PathBuf::from(home));
    }

    #[test]
    fn required_and_unavailable_is_refused_with_the_honest_kind() {
        let _serial = SANDBOX_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _guard = TestStateGuard;
        install_for_test(
            SandboxMode::Required,
            policy(),
            SandboxAvailability::Unavailable("nested sandbox rc=71".to_string()),
        );

        let error = wrap_command(Command::new("sh")).expect_err("required must refuse");
        assert_eq!(
            error.summary(),
            format!("{UNAVAILABLE_KIND}: nested sandbox rc=71")
        );
    }

    #[test]
    fn auto_and_unavailable_continues_and_warns_once() {
        let _serial = SANDBOX_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _guard = TestStateGuard;
        install_for_test(
            SandboxMode::Auto,
            policy(),
            SandboxAvailability::Unavailable("nested sandbox rc=71".to_string()),
        );

        assert!(wrap_command(Command::new("sh")).is_ok());
        let warning = startup_warning().expect("auto warns");
        assert!(warning.contains("nested sandbox rc=71"), "{warning}");
        let status = status().unwrap();
        assert!(!status.applied);
        assert_eq!(status.reason.as_deref(), Some("nested sandbox rc=71"));
    }

    #[test]
    fn off_never_wraps_and_never_warns() {
        let _serial = SANDBOX_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _guard = TestStateGuard;
        install_for_test(
            SandboxMode::Off,
            policy(),
            SandboxAvailability::Unavailable(DISABLED_REASON.to_string()),
        );

        assert!(wrap_command(Command::new("sh")).is_ok());
        assert!(startup_warning().is_none());
        let status = status().unwrap();
        assert!(!status.applied);
        assert_eq!(status.reason, None);
    }

    /// A run's sandbox is torn down by taking its policy, which can leave the
    /// backend available with no policy. `required` must refuse instead of
    /// passing the command through unconfined.
    #[test]
    fn required_refuses_when_available_without_a_policy() {
        let _serial = SANDBOX_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _guard = TestStateGuard;
        *INSTALLED.lock().expect("sandbox state") = Some(Installed {
            policy: None,
            mode: SandboxMode::Required,
            availability: SandboxAvailability::Available,
        });

        let error = wrap_command(Command::new("sh")).expect_err("required must refuse");
        assert_eq!(
            error.summary(),
            format!("{UNAVAILABLE_KIND}: {MISSING_POLICY_REASON}")
        );
    }

    /// `auto` keeps its previous behavior for the same state.
    #[test]
    fn wrap_auto_continues_when_available_without_a_policy() {
        let _serial = SANDBOX_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _guard = TestStateGuard;
        *INSTALLED.lock().expect("sandbox state") = Some(Installed {
            policy: None,
            mode: SandboxMode::Auto,
            availability: SandboxAvailability::Available,
        });

        assert!(wrap_command(Command::new("sh")).is_ok());
    }

    /// Stage 1b: a substituted Landlock decision that is unavailable (for example
    /// ABI 0) makes `auto` continue with `applied=false` and `required` refuse
    /// with the honest kind. The reason comes from the real pure decision.
    #[cfg(target_os = "linux")]
    #[test]
    fn unavailable_landlock_decision_continues_auto_and_refuses_required() {
        let _serial = SANDBOX_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _guard = TestStateGuard;
        let SandboxAvailability::Unavailable(reason) =
            landlock::decide_availability("5.4.0-generic", Ok(0), Ok(()))
        else {
            panic!("ABI 0 must be unavailable");
        };
        assert!(
            reason.contains("5.4.0-generic") && reason.contains("ABI"),
            "{reason}"
        );

        install_for_test(
            SandboxMode::Auto,
            policy(),
            SandboxAvailability::Unavailable(reason.clone()),
        );
        let status = status().unwrap();
        assert!(!status.applied, "{status:?}");
        assert_eq!(status.reason.as_deref(), Some(reason.as_str()));
        assert!(wrap_command(Command::new("sh")).is_ok(), "auto continues");

        install_for_test(
            SandboxMode::Required,
            policy(),
            SandboxAvailability::Unavailable(reason.clone()),
        );
        let error = wrap_command(Command::new("sh")).expect_err("required must refuse");
        assert_eq!(error.summary(), format!("{UNAVAILABLE_KIND}: {reason}"));
    }

    /// Stage 1b: when the probe was available but `landlock::wrap` fails at run
    /// time, `auto` continues with lexical checks only and the installed sandbox
    /// stops claiming to be applied; `required` refuses.
    #[cfg(target_os = "linux")]
    #[test]
    fn auto_downgrades_when_the_backend_wrap_fails() {
        let _serial = SANDBOX_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _guard = TestStateGuard;
        let _hook = WrapFailureHookGuard;
        landlock::test_hooks::force_wrap_failure(true);

        install_for_test(SandboxMode::Auto, policy(), SandboxAvailability::Available);
        assert!(is_applied());

        assert!(
            wrap_command(Command::new("sh")).is_ok(),
            "auto must continue after a failed wrap"
        );
        assert!(!is_applied(), "a failed wrap must clear the applied claim");
        let status = status().unwrap();
        assert!(!status.applied, "{status:?}");
        assert!(
            status
                .reason
                .as_deref()
                .is_some_and(|reason| reason.contains("could not be built")),
            "{status:?}"
        );

        install_for_test(
            SandboxMode::Required,
            policy(),
            SandboxAvailability::Available,
        );
        let error = wrap_command(Command::new("sh")).expect_err("required must refuse");
        assert!(
            error.summary().contains(UNAVAILABLE_KIND),
            "{}",
            error.summary()
        );
    }

    /// Resets the Landlock wrap-failure hook on drop.
    #[cfg(target_os = "linux")]
    struct WrapFailureHookGuard;

    #[cfg(target_os = "linux")]
    impl Drop for WrapFailureHookGuard {
        fn drop(&mut self) {
            landlock::test_hooks::force_wrap_failure(false);
        }
    }

    struct TestStateGuard;

    impl Drop for TestStateGuard {
        fn drop(&mut self) {
            reset_for_test();
        }
    }
}
