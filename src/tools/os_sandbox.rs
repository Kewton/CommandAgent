//! OS write sandbox for model Bash and verifier commands (Issue #655).
//!
//! This is the common leaf module of the #502 write-confinement track. It wraps
//! the `sh -c` launch point of the model Bash tool (including the `bash:verify`
//! interval, which reuses the same function) and of verifier commands, so an
//! indirect write that the lexical guard cannot see still fails when the OS
//! backend is available.
//!
//! Design (issue #655, decisions from #502):
//! - The backend writes a fixed profile; locations travel only through `-D`
//!   parameters, never embedded in the profile string.
//! - Availability is probed once at run start and stored process-wide, together
//!   with the mode, so a Bash spawned on another thread still sees it.
//! - `auto` continues with lexical checks only when the backend is unavailable
//!   and records the reason; `required` fails the command honestly with
//!   [`UNAVAILABLE_KIND`]; `off` keeps the previous behavior.
//! - The lexical guard is not weakened or replaced: it still runs first and can
//!   refuse a command before the sandbox is consulted.
//! - macOS only in this stage; Linux (landlock) is a separate issue. On any
//!   other platform the backend is reported as unavailable rather than silently
//!   pretending isolation.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use clap::ValueEnum;

#[cfg(target_os = "macos")]
pub mod seatbelt;

/// The macOS backend name used in `bash_sandbox_status`.
pub const BACKEND_SEATBELT: &str = "seatbelt";
/// The backend name on platforms without a write backend in this stage.
pub const BACKEND_NONE: &str = "none";

/// The honest failure kind emitted when `required` cannot apply the sandbox.
pub const UNAVAILABLE_KIND: &str = "bash_sandbox_unavailable";

/// The reason recorded when the sandbox is explicitly disabled.
const DISABLED_REASON: &str = "disabled by --sandbox off";
/// The reason recorded once a run has finished and its sandbox is torn down.
const RUN_FINISHED_REASON: &str = "run finished; the OS write sandbox is no longer applied";
/// The refusal reason for a `required` run whose backend is available but whose
/// policy is gone (for example after [`cleanup`]).
const MISSING_POLICY_REASON: &str = "the OS write sandbox has no policy to apply";
/// The reason recorded on platforms with no write-isolation backend in 1a.
#[cfg(not(target_os = "macos"))]
const NO_PLATFORM_BACKEND: &str =
    "no OS write-isolation backend is available on this platform (macOS only in stage 1a)";

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

/// The resolved write-allow list and the run temp directory.
#[derive(Debug, Clone)]
pub struct SandboxPolicy {
    pub workspace_root: PathBuf,
    pub run_tmp_dir: PathBuf,
    pub caches: CachePaths,
}

impl SandboxPolicy {
    /// Build a policy whose cache locations are resolved from `HOME`.
    pub fn new(workspace_root: &Path, run_tmp_dir: &Path) -> Self {
        Self {
            workspace_root: workspace_root.to_path_buf(),
            run_tmp_dir: run_tmp_dir.to_path_buf(),
            caches: resolve_cache_paths(),
        }
    }
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
    pub reason: Option<String>,
}

impl SandboxStatus {
    fn from_parts(mode: SandboxMode, availability: &SandboxAvailability) -> Self {
        let applied = mode.uses_backend() && matches!(availability, SandboxAvailability::Available);
        let reason = if mode.uses_backend() {
            match availability {
                SandboxAvailability::Unavailable(reason) => Some(reason.clone()),
                SandboxAvailability::Available => None,
            }
        } else {
            None
        };
        Self {
            mode,
            backend: backend_name(),
            applied,
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
    } else {
        BACKEND_NONE
    }
}

/// Probe the backend once. On macOS this starts `sandbox-exec` with a do-nothing
/// profile; a nested sandbox fails with rc=71, which is reported as unavailable.
pub fn probe() -> SandboxAvailability {
    #[cfg(target_os = "macos")]
    {
        seatbelt::probe()
    }
    #[cfg(not(target_os = "macos"))]
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
const DIFFERENT_WORKSPACE_REASON: &str = "workflow node runs in a workspace other than the run workspace; the OS write sandbox is not applied";

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
    if !mismatched || !mode.uses_backend() {
        return Some(SandboxStatus::from_parts(mode, &availability));
    }
    if !matches!(availability, SandboxAvailability::Available) {
        // The backend is already unavailable; keep the reason the run started
        // with instead of overwriting it.
        return Some(SandboxStatus::from_parts(mode, &availability));
    }
    let availability = SandboxAvailability::Unavailable(DIFFERENT_WORKSPACE_REASON.to_string());
    let status = SandboxStatus::from_parts(mode, &availability);
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
    let status = SandboxStatus::from_parts(mode, &availability);
    let mut installed = INSTALLED.lock().expect("sandbox state");
    *installed = Some(Installed {
        policy,
        mode,
        availability,
    });
    status
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
    installed
        .as_ref()
        .map(|installed| SandboxStatus::from_parts(installed.mode, &installed.availability))
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
            "warning: OS write sandbox is unavailable ({reason}); Bash and verifier commands continue with lexical path checks only"
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
                Some(policy) => Ok(apply_backend(command, &policy)),
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

fn apply_backend(command: Command, policy: &SandboxPolicy) -> Command {
    #[cfg(target_os = "macos")]
    {
        seatbelt::wrap(command, policy)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = policy;
        command
    }
}

fn resolve_cache_paths() -> CachePaths {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return CachePaths::default();
    };
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
        }
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

    struct TestStateGuard;

    impl Drop for TestStateGuard {
        fn drop(&mut self) {
            reset_for_test();
        }
    }
}
