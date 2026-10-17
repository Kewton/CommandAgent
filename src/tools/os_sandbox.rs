//! OS write, read, and (for `--offline`) network sandbox for model Bash,
//! verifier, and dependency-setup commands (Issues #655, #657, and #658).
//!
//! This is the common leaf module of the #502 confinement track. It wraps the
//! `sh -c` launch point of the model Bash tool (including the `bash:verify`
//! interval, which reuses the same function), of verifier commands, and of the
//! dependency-setup launches, so an indirect write the lexical guard cannot see
//! still fails, and since #657 an indirect read of a home secret location (or,
//! on macOS, a workspace credential) fails as well, when the OS backend is
//! available. Since #658 an `--offline` run also blocks outbound network, so an
//! arbitrary program the lexical guard cannot name cannot reach the network.
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

/// The resolved write-allow list, the run temp directory, the home whose secret
/// locations are denied for reading (Issue #657), and the network layer of
/// Issue #658.
#[derive(Debug, Clone)]
pub struct SandboxPolicy {
    pub workspace_root: PathBuf,
    pub run_tmp_dir: PathBuf,
    pub caches: CachePaths,
    /// The home whose secret locations the read backend denies. Resolved from
    /// `HOME`; tests pass an explicit tempdir so they never touch the real home.
    pub home: PathBuf,
    /// Whether the run was started with `--offline` (Issue #658). When true the
    /// backend also blocks outbound network — macOS confines it to loopback and
    /// unix sockets, Linux (ABI4) to the dev-server ports — so an arbitrary
    /// program the lexical guard cannot name still cannot reach the network.
    /// When false the network is unchanged, exactly as before this stage.
    pub offline: bool,
    /// The TCP ports an `--offline` run still allows a `connect` to (Linux ABI4,
    /// Issue #658), so a dev server started outside the sandbox stays reachable
    /// while every other destination is refused. macOS allows all of loopback
    /// and ignores this list. Empty by default; `install` does not populate it.
    pub dev_server_ports: Vec<u16>,
}

impl SandboxPolicy {
    /// Build a policy whose cache locations and read-deny home are resolved from
    /// `HOME`. The network layer is off, so the network is unchanged.
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
            offline: false,
            dev_server_ports: Vec::new(),
        }
    }

    /// Set whether the run is `--offline`, so the backend blocks outbound
    /// network. Off by default.
    pub fn with_offline(mut self, offline: bool) -> Self {
        self.offline = offline;
        self
    }

    /// Set the TCP ports an `--offline` run leaves connectable (Linux ABI4).
    pub fn with_dev_server_ports(mut self, ports: impl IntoIterator<Item = u16>) -> Self {
        self.dev_server_ports = ports.into_iter().collect();
        self
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
    /// The directory that owns the run temp dir (`<state>/sandbox`), stored
    /// independently of `policy.run_tmp_dir` so the recursive cleanup can
    /// re-check the real path against a reference a mutation of the run temp
    /// dir cannot move (Issue #673).
    run_tmp_root: Option<PathBuf>,
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
    #[cfg(target_os = "linux")]
    {
        read_isolation_with_limit(policy, read_allow_limit())
    }
    #[cfg(not(target_os = "linux"))]
    {
        read_isolation_checked(policy)
    }
}

/// A test override of the read-allow expansion limit (0 means the default
/// [`landlock::MAX_READ_ALLOW_RULES`]), so an integration test can drive the
/// overflow without a home of thousands of siblings (Issue #668).
#[cfg(target_os = "linux")]
static READ_ALLOW_LIMIT_OVERRIDE: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// The read-allow expansion limit in force.
#[cfg(target_os = "linux")]
pub(crate) fn read_allow_limit() -> usize {
    match READ_ALLOW_LIMIT_OVERRIDE.load(std::sync::atomic::Ordering::Relaxed) {
        0 => landlock::MAX_READ_ALLOW_RULES,
        limit => limit,
    }
}

/// Override the read-allow expansion limit (`None` restores the default).
/// Tests only; [`reset_for_test`] clears it.
#[cfg(target_os = "linux")]
pub fn set_read_allow_limit_for_test(limit: Option<usize>) {
    READ_ALLOW_LIMIT_OVERRIDE.store(limit.unwrap_or(0), std::sync::atomic::Ordering::Relaxed);
}

/// [`read_isolation`] with an explicit expansion limit, so a test can drive the
/// overflow without a real home of thousands of siblings (Issue #668).
#[cfg(target_os = "linux")]
fn read_isolation_with_limit(policy: Option<&SandboxPolicy>, limit: usize) -> ReadIsolation {
    let checked = read_isolation_checked(policy);
    let Some(policy) = policy else {
        return checked;
    };
    if checked.effective && landlock::read_allow_overflows(policy, limit) {
        return ReadIsolation {
            effective: false,
            clause: format!(
                "read isolation is not effective: the read allow list would exceed {limit} rules"
            ),
        };
    }
    checked
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn read_isolation_checked(policy: Option<&SandboxPolicy>) -> ReadIsolation {
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
/// applied. On Linux it names the write operations an older ABI cannot close,
/// the network-isolation clause of Issue #658 when `--offline` cannot be
/// enforced (no ABI4), and the read-isolation clause (effective or not, in the
/// same words as macOS since Issue #668). Both platforms then name a known
/// dependency cache that was refused because it resolves outside the home.
/// macOS enforces the network layer inside Seatbelt, so it has no network
/// clause.
fn backend_note(
    read: &ReadIsolation,
    offline: bool,
    policy: Option<&SandboxPolicy>,
) -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        let mut parts = Vec::new();
        if let Some(note) = landlock::limitation_note() {
            parts.push(note);
        }
        if let Some(note) = landlock::network_limitation_note(offline) {
            parts.push(note);
        }
        // The same "applied" sentence macOS records (Issue #668), so both
        // platforms report an effective read layer in the same words.
        parts.push(read.clause.clone());
        parts.extend(cache_note(policy));
        Some(parts.join("; "))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = offline;
        let mut parts = vec![read.clause.clone()];
        parts.extend(cache_note(policy));
        Some(parts.join("; "))
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
/// directory, without the `--offline` network layer. Returns the status recorded
/// in `bash_sandbox_status`.
///
/// In a unit-test build this is a no-op: many lib tests run in one process and
/// must not observe each other's process-wide state. Integration tests compile
/// the library without `cfg(test)` and exercise the real install, and the
/// `install_for_test` seam drives the explicit states.
pub fn install(mode: SandboxMode, workspace_root: &Path, state_dir: &Path) -> SandboxStatus {
    install_with(mode, workspace_root, state_dir, false, Vec::new())
}

/// [`install`] with the run's `--offline` network layer of Issue #658.
///
/// `offline` is the run's `--offline` flag, so the backend also blocks outbound
/// network. `dev_server_ports` is the set of TCP ports an `--offline` run still
/// allows a `connect` to (Linux ABI4), so the run's dev server stays reachable;
/// macOS confines the network to loopback and ignores the list. Both defaults
/// keep the network unchanged.
pub fn install_with(
    mode: SandboxMode,
    workspace_root: &Path,
    state_dir: &Path,
    offline: bool,
    dev_server_ports: Vec<u16>,
) -> SandboxStatus {
    #[cfg(not(test))]
    {
        install_impl(mode, workspace_root, state_dir, offline, dev_server_ports)
    }
    #[cfg(test)]
    {
        let _ = (workspace_root, state_dir, offline, dev_server_ports);
        SandboxStatus {
            mode,
            backend: backend_name(),
            applied: false,
            read_isolation: false,
            reason: None,
        }
    }
}

/// The TCP port(s) an `--offline` run leaves connectable (Issue #658).
///
/// Landlock (ABI4) is port-scoped and cannot tell addresses apart, so the run's
/// dev-server port must be re-allowed for a sandboxed command to reach it. The
/// port is derived the same way preflight derives it — the port named in the
/// goal, otherwise the profile default — so the allow list matches the port the
/// run actually uses. macOS confines the network to loopback and ignores the
/// list; an empty list leaves the network fully denied.
pub fn dev_server_ports(profile: &str, goal: Option<&str>) -> Vec<u16> {
    let goal_port = goal.and_then(crate::planner::signals::requested_port_from_text);
    let default_port =
        crate::planner::profile::resolve_profile_runtime(profile).default_requested_port();
    match (goal_port, default_port) {
        (Some(port), _) => vec![port],
        (None, Some(default)) => vec![default],
        (None, None) => Vec::new(),
    }
}

#[cfg(not(test))]
fn install_impl(
    mode: SandboxMode,
    workspace_root: &Path,
    state_dir: &Path,
    offline: bool,
    dev_server_ports: Vec<u16>,
) -> SandboxStatus {
    let run_tmp_root = state_dir.join("sandbox");
    let run_tmp_dir = run_tmp_root.join(format!("run-{}", uuid::Uuid::now_v7()));
    if let Err(err) = std::fs::create_dir_all(&run_tmp_dir) {
        let availability =
            SandboxAvailability::Unavailable(format!("run temp directory is unavailable: {err}"));
        return store(mode, None, availability, None);
    }
    let policy = SandboxPolicy::new(workspace_root, &run_tmp_dir)
        .with_offline(offline)
        .with_dev_server_ports(dev_server_ports);
    let availability = if mode.uses_backend() {
        probe()
    } else {
        SandboxAvailability::Unavailable(DISABLED_REASON.to_string())
    };
    store(mode, Some(policy), availability, Some(run_tmp_root))
}

/// Install an explicit policy and availability. Used by tests to simulate an
/// unavailable backend (for example a nested sandbox rc=71).
pub fn install_for_test(
    mode: SandboxMode,
    policy: SandboxPolicy,
    availability: SandboxAvailability,
) -> SandboxStatus {
    let run_tmp_root = policy.run_tmp_dir.parent().map(Path::to_path_buf);
    store(mode, Some(policy), availability, run_tmp_root)
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
    let (mode, availability, policy, run_tmp_root, mismatched) = {
        let state = installed.as_ref()?;
        let mismatched = state
            .policy
            .as_ref()
            .is_some_and(|policy| policy.workspace_root != workspace_root);
        (
            state.mode,
            state.availability.clone(),
            state.policy.clone(),
            state.run_tmp_root.clone(),
            mismatched,
        )
    };
    let read = read_isolation(policy.as_ref());
    let offline = policy.as_ref().is_some_and(|policy| policy.offline);
    if !mismatched || !mode.uses_backend() {
        return Some(SandboxStatus::from_parts(
            mode,
            &availability,
            backend_note(&read, offline, policy.as_ref()),
            read.effective,
        ));
    }
    if !matches!(availability, SandboxAvailability::Available) {
        // The backend is already unavailable; keep the reason the run started
        // with instead of overwriting it.
        return Some(SandboxStatus::from_parts(
            mode,
            &availability,
            backend_note(&read, offline, policy.as_ref()),
            read.effective,
        ));
    }
    let availability = SandboxAvailability::Unavailable(DIFFERENT_WORKSPACE_REASON.to_string());
    let status = SandboxStatus::from_parts(mode, &availability, None, false);
    *installed = Some(Installed {
        policy,
        mode,
        availability,
        run_tmp_root,
    });
    Some(status)
}

fn store(
    mode: SandboxMode,
    policy: Option<SandboxPolicy>,
    availability: SandboxAvailability,
    run_tmp_root: Option<PathBuf>,
) -> SandboxStatus {
    let read = read_isolation(policy.as_ref());
    let offline = policy.as_ref().is_some_and(|policy| policy.offline);
    let status = SandboxStatus::from_parts(
        mode,
        &availability,
        backend_note(&read, offline, policy.as_ref()),
        read.effective,
    );
    let mut installed = INSTALLED.lock().expect("sandbox state");
    *installed = Some(Installed {
        policy,
        mode,
        availability,
        run_tmp_root,
    });
    status
}

/// The events file a downgrade is reported to. The run's path is chosen by the
/// binary at run start, so it hands it over with [`set_events_path`]; without it
/// a downgrade is recorded in the status only.
static EVENTS_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Tell the sandbox where the run's events file is, so a later downgrade can
/// re-emit `bash_sandbox_status` there (Issue #668). Pass `None` to clear it.
pub fn set_events_path(path: Option<PathBuf>) {
    *EVENTS_PATH.lock().expect("sandbox events path") = path;
}

/// The `bash_sandbox_status` event for a status. The same name and fields the
/// run-start event carries; no field is added.
pub fn status_event(status: &SandboxStatus) -> serde_json::Value {
    serde_json::json!({
        "event": "bash_sandbox_status",
        "mode": status.mode.as_str(),
        "backend": status.backend,
        "applied": status.applied,
        "reason": status.reason.clone().unwrap_or_default(),
    })
}

/// Record that the installed backend is no longer applied, for example when a
/// per-command ruleset could not be built even though the run-start probe said
/// the backend was available. Mirrors [`reconcile_workspace`]: the availability
/// becomes [`SandboxAvailability::Unavailable`] with the reason, so
/// [`is_applied`] turns false and later `required` commands refuse. The
/// downgrade is announced once by re-emitting `bash_sandbox_status` (Issue #668)
/// so an `auto` run that fell back to lexical checks does not keep a run-start
/// event that still says applied. The event name and fields are unchanged.
#[cfg(any(target_os = "linux", test))]
fn mark_backend_unavailable(reason: String) {
    let mut installed = INSTALLED.lock().expect("sandbox state");
    let Some(state) = installed.as_mut() else {
        return;
    };
    if !matches!(state.availability, SandboxAvailability::Available) {
        return;
    }
    state.availability = SandboxAvailability::Unavailable(reason);
    drop(installed);
    let path = EVENTS_PATH.lock().expect("sandbox events path").clone();
    if let (Some(path), Some(status)) = (path, status()) {
        crate::eval_events::emit(Some(&path), status_event(&status));
    }
}

/// Clear the process-wide state. Tests only.
pub fn reset_for_test() {
    let mut installed = INSTALLED.lock().expect("sandbox state");
    *installed = None;
    drop(installed);
    set_events_path(None);
    #[cfg(target_os = "linux")]
    set_read_allow_limit_for_test(None);
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
    let run_tmp_root = state.run_tmp_root.take();
    drop(installed);
    if let Some(run_tmp_dir) = run_tmp_dir {
        // Issue #673: only remove the run temp dir when it is a strict
        // descendant of the `<state>/sandbox` root stored at install time, so a
        // mutation that moves `run_tmp_dir` cannot recursively delete an
        // unrelated directory. The checked real path is the value that is
        // removed.
        if let Some(root) = run_tmp_root.as_deref()
            && let Some(checked) =
                crate::tools::dir_fd::strict_descendant_real_path(root, &run_tmp_dir)
        {
            let _ = std::fs::remove_dir_all(checked);
        }
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
        let offline = installed
            .policy
            .as_ref()
            .is_some_and(|policy| policy.offline);
        SandboxStatus::from_parts(
            installed.mode,
            &installed.availability,
            backend_note(&read, offline, installed.policy.as_ref()),
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

/// Wrap a runtime dependency-setup launch in the OS sandbox (Issue #658).
///
/// The Python `sh -c` (venv and `pip install -e .`, including `setup.py`) and
/// the `npm install` run through here, so an install-time script cannot write
/// outside the workspace and an `--offline` run cannot reach the network. `Err`
/// is the `required` refusal when the backend is unavailable; `auto` and `off`
/// return the command unchanged (unwrapped when the backend is unavailable), so
/// `auto` keeps running the setup outside the sandbox and the run status records
/// it as not applied.
///
/// In a unit-test build this is a no-op: many lib tests run in one process and
/// must not observe each other's process-wide sandbox state. Integration tests
/// compile the library without `cfg(test)` and exercise the real wrap.
pub fn wrap_dependency_setup(command: Command) -> Result<Command, SandboxRefusal> {
    #[cfg(not(test))]
    {
        wrap_command(command)
    }
    #[cfg(test)]
    {
        Ok(command)
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

/// The known cache locations, relative to the home, on either platform. Used to
/// count the ones a symlink pointed outside the home (Issue #668).
const CACHE_LOCATIONS: &[&str] = &[
    ".cargo/registry",
    ".cargo/git",
    ".cargo/.package-cache",
    ".npm",
    "Library/Caches/pip",
    "Library/Caches/ms-playwright",
    ".cache/pip",
    ".cache/ms-playwright",
];

/// Whether a known cache location, once its symlinks are resolved, is still
/// under the home. A cache that points outside the home (`~/.npm` linked at an
/// unrelated directory) is not granted write: the sandbox would otherwise open
/// the link's target (Issue #668). An empty home or an unresolvable path is not
/// inside.
pub(crate) fn cache_stays_in_home(home: &Path, cache: &Path) -> bool {
    if home.as_os_str().is_empty() {
        return false;
    }
    match (home.canonicalize(), cache.canonicalize()) {
        (Ok(home), Ok(cache)) => cache.starts_with(home),
        _ => false,
    }
}

/// The reason clause naming the known caches that exist but were refused
/// because they resolve outside the home, or `None` when there are none.
fn cache_note(policy: Option<&SandboxPolicy>) -> Option<String> {
    let home = &policy?.home;
    if home.as_os_str().is_empty() {
        return None;
    }
    let refused = CACHE_LOCATIONS
        .iter()
        .map(|relative| home.join(relative))
        .filter(|path| path.exists() && !cache_stays_in_home(home, path))
        .count();
    (refused > 0).then(|| {
        format!(
            "{refused} dependency cache location(s) resolve outside the home and are not writable"
        )
    })
}

fn resolve_cache_paths(home: &Path) -> CachePaths {
    if home.as_os_str().is_empty() {
        return CachePaths::default();
    }
    let home = home.to_path_buf();
    CachePaths {
        cargo_registry: existing_dir(&home, home.join(".cargo/registry")),
        cargo_git: existing_dir(&home, home.join(".cargo/git")),
        cargo_package_cache: existing_file(&home, home.join(".cargo/.package-cache")),
        npm: existing_dir(&home, home.join(".npm")),
        pip: existing_dir(&home, home.join("Library/Caches/pip")),
        playwright: existing_dir(&home, home.join("Library/Caches/ms-playwright")),
    }
}

fn existing_dir(home: &Path, path: PathBuf) -> Option<PathBuf> {
    (path.is_dir() && cache_stays_in_home(home, &path)).then_some(path)
}

fn existing_file(home: &Path, path: PathBuf) -> Option<PathBuf> {
    (path.is_file() && cache_stays_in_home(home, &path)).then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// The installed state is process-wide, so these tests serialize.
    static SANDBOX_TEST_LOCK: Mutex<()> = Mutex::new(());

    /// Issue #658: the `--offline` dev-server allow list is the port the run
    /// will use — the goal's port, else the profile default — so a sandboxed
    /// command can reach the dev server on Linux (ABI4) while every other
    /// connect is denied. macOS ignores the list.
    #[test]
    fn dev_server_ports_follow_the_goal_then_the_profile_default() {
        assert_eq!(
            dev_server_ports("generic", Some("serve the app on port 4000")),
            vec![4000]
        );
        assert_eq!(
            dev_server_ports("generic", Some("serve the app")),
            Vec::<u16>::new()
        );
        assert_eq!(
            dev_server_ports("nextjs", Some("build a landing page")),
            vec![crate::planner::profiles::nextjs::DEFAULT_REQUESTED_PORT]
        );
    }

    fn policy() -> SandboxPolicy {
        SandboxPolicy {
            workspace_root: PathBuf::from("/tmp/os-sandbox-test-work"),
            run_tmp_dir: PathBuf::from("/tmp/os-sandbox-test-tmp"),
            caches: CachePaths::default(),
            home: PathBuf::from("/tmp/os-sandbox-test-home"),
            offline: false,
            dev_server_ports: Vec::new(),
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
            run_tmp_root: None,
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
            run_tmp_root: None,
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

    /// Issue #668: a downgrade re-emits `bash_sandbox_status` with the same name
    /// and fields, once, so the events file no longer ends on a stale "applied".
    #[test]
    fn a_downgrade_re_emits_the_status_event_once() {
        let _serial = SANDBOX_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _guard = TestStateGuard;
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join("events.jsonl");
        install_for_test(SandboxMode::Auto, policy(), SandboxAvailability::Available);
        set_events_path(Some(events.clone()));

        mark_backend_unavailable("ruleset could not be built".to_string());
        mark_backend_unavailable("a second failure".to_string());

        let text = std::fs::read_to_string(&events).expect("the downgrade is emitted");
        let lines: Vec<serde_json::Value> = text
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(lines.len(), 1, "{text}");
        let event = &lines[0];
        assert_eq!(event["event"], "bash_sandbox_status");
        assert_eq!(event["mode"], "auto");
        assert_eq!(event["backend"], backend_name());
        assert_eq!(event["applied"], false);
        assert_eq!(event["reason"], "ruleset could not be built");
        let mut keys: Vec<&str> = event
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "applied",
                "backend",
                "event",
                "mode",
                "reason",
                "schema_version"
            ]
        );
    }

    /// Issue #668: a known cache that is a symlink to a location outside the home
    /// is not granted write and is named in the reason; a link that stays inside
    /// the home is kept.
    #[test]
    fn a_cache_symlinked_outside_the_home_is_not_allowed() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        let home = base.join("home");
        let outside = base.join("outside");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, home.join(".npm")).unwrap();
        std::fs::create_dir_all(home.join("real-cargo-git")).unwrap();
        std::fs::create_dir_all(home.join(".cargo")).unwrap();
        std::os::unix::fs::symlink(home.join("real-cargo-git"), home.join(".cargo/git")).unwrap();

        let caches = resolve_cache_paths(&home);
        assert_eq!(caches.npm, None, "{caches:?}");
        assert_eq!(
            caches.cargo_git,
            Some(home.join(".cargo/git")),
            "{caches:?}"
        );

        let policy = SandboxPolicy::with_home(&home, &base.join("run"), home.clone());
        let note = cache_note(Some(&policy)).expect("the refused cache is recorded");
        assert!(note.starts_with("1 dependency cache"), "{note}");
        assert!(cache_note(None).is_none());
    }

    /// Issue #668: past the expansion limit the read layer is recorded as not
    /// effective instead of applied.
    #[cfg(target_os = "linux")]
    #[test]
    fn an_expansion_over_the_limit_is_not_recorded_as_applied() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().canonicalize().unwrap();
        std::fs::create_dir_all(home.join(".ssh")).unwrap();
        let policy = SandboxPolicy::with_home(&home, &home.join("run"), home.clone());

        let within = read_isolation_with_limit(Some(&policy), usize::MAX);
        let over = read_isolation_with_limit(Some(&policy), 1);
        assert!(!over.effective, "{}", over.clause);
        assert!(over.clause.contains("not effective"), "{}", over.clause);
        assert!(!over.clause.contains(READ_ISOLATION_APPLIED));
        assert_ne!(within.clause, over.clause);
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
