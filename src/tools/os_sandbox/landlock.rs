//! Linux Landlock backend (Issue #656, stage 1b; read isolation in Issue #657,
//! stage 2; offline network confinement in Issue #658, stage 3).
//!
//! Confines the writes and, since #657, the reads of model Bash and verifier
//! commands with Landlock. The ruleset and every allowed-location file
//! descriptor are opened in the parent; the forked child's `pre_exec` runs only
//! `prctl(PR_SET_NO_NEW_PRIVS)` and `landlock_restrict_self`, so no work that
//! could allocate, lock, or log happens between `fork` and `exec`.
//!
//! Design (issues #656, #657, and #658, decisions from #502):
//! - ABI1 (kernel 5.13) is required. ABI2 (`refer`, link/rename across
//!   directories) and ABI3 (`truncate`) are used best-effort; when they are
//!   missing the still-open operations are named in the `bash_sandbox_status`
//!   reason.
//! - Read rights: the handled read set is `AccessFs::from_read(abi)`, i.e.
//!   `Execute | ReadFile | ReadDir`. `ReadFile` opens a file for reading and
//!   `ReadDir` lists a directory; `Execute` runs a file, and both the toolchain
//!   (`cargo`, `rustup`, `node`, `python`, `git`) and the system binaries need
//!   it, so the read allow list grants all three together. Landlock checks the
//!   access on the file or directory that is opened, not on each ancestor path
//!   component, so an allowed subtree needs no separate traversal grant; a
//!   denied location is unreadable because no rule covers it. `ReadFile`,
//!   `ReadDir`, and `Execute` are all present from ABI1, so the read layer adds
//!   no new ABI requirement over the write layer.
//! - Read isolation is a deny list rewritten as an allow list
//!   ([`expand_read_allow_limited`]): Landlock cannot deny part of an allowed tree, so
//!   the home secret locations are expressed by allowing the siblings that do
//!   not contain them.
//! - Network confinement of Issue #658 is only for an `--offline` run and only
//!   from ABI4 (kernel 6.7): `AccessNet::ConnectTcp` is handled, which denies
//!   every TCP connect, and a [`NetPort`] rule re-allows the dev-server ports.
//!   Landlock cannot tell addresses apart and cannot restrict UDP, so an
//!   `--offline` run can still reach the same port on an external host and any
//!   UDP destination; those limits are documented (SECURITY.md and the
//!   cli-reference) rather than recorded in `bash_sandbox_status`. Only a kernel
//!   below ABI4 is recorded there, because then the network is left unchanged and
//!   the event says so ([`network_limitation_note`]).
//! - A non-`--offline` run handles no network access, so its network is exactly
//!   as before this stage.
//! - Availability is probed once at run start from the Landlock ABI query and a
//!   child `restrict_self`, so a seccomp filter that denies either syscall is an
//!   honest "unavailable" instead of a silent pass.
//! - Terminal isolation (Issue #667): `/dev/tty` and the `/dev/pts` directory
//!   are not allowed for writing, so a confined command cannot write to another
//!   terminal's slave; the child's own session (a `setsid()` in
//!   `bounded_process`) has no controlling terminal, so `TIOCSTI` is refused by
//!   the kernel even where Landlock cannot filter device ioctls (below ABI5).
//!   The same locations are removed from the read allow list
//!   ([`terminal_read_deny_paths`]), so a command cannot re-open the run's
//!   terminal by path and read the keys the user types; `/dev` stays listable
//!   through a `ReadDir`-only rule that never grants `ReadFile`.

use std::collections::HashSet;
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::Duration;

use landlock::{
    ABI, Access, AccessFs, AccessNet, CompatLevel, Compatible, NetPort, Ruleset, RulesetAttr,
    RulesetCreated, RulesetCreatedAttr, path_beneath_rules,
};

use super::{SandboxAvailability, SandboxPolicy};

/// The `flags` value that asks `landlock_create_ruleset` for the ABI version.
const LANDLOCK_CREATE_RULESET_VERSION: libc::c_uint = 1;

/// Bound on the child that proves `landlock_restrict_self` can be enforced.
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);

/// Device files that either read-only tools or an attached terminal expect to
/// write. Only the ones that exist are allowed.
///
/// `/dev/tty` and `/dev/pts` are deliberately absent (Issue #667). A sandboxed
/// child runs in its own session with no controlling terminal, so it has no
/// legitimate use for either, and allowing the whole `/dev/pts` directory let it
/// write to another terminal's slave. Landlock grants the handled `IoctlDev`
/// access (ABI5) only where a rule covers the location, so leaving these two out
/// also closes device ioctls on them — including `TIOCSTI` — not just writes.
/// `/dev/ptmx` stays allowed, but that does not give a usable pty: the slave it
/// would create (`/dev/pts/*`) is not allowed either, so `openpty`, `script`,
/// `expect`, and `pexpect` fail inside the isolation. That pty loss is the cost
/// of closing terminal reads and writes (Issue #667); allocating a pty worked
/// before #667.
const DEVICE_PATHS: &[&str] = &[
    "/dev/null",
    "/dev/zero",
    "/dev/full",
    "/dev/random",
    "/dev/urandom",
    "/dev/ptmx",
];

/// The Landlock ABI detected for the running kernel, or the reason it cannot be
/// used. Computed once per process.
#[derive(Debug)]
enum Detected {
    Available { abi: u32 },
    Unavailable(String),
}

static DETECTED: OnceLock<Detected> = OnceLock::new();

fn detect() -> &'static Detected {
    DETECTED.get_or_init(detect_uncached)
}

fn detect_uncached() -> Detected {
    let release = kernel_release();
    let abi_query = query_abi();
    let enforcement = match abi_query {
        Ok(abi) if abi >= 1 => probe_restrict_self(abi),
        // `decide` ignores `enforcement` unless the ABI query is supported.
        _ => Ok(()),
    };
    decide(&release, abi_query, enforcement)
}

/// The pure availability decision: the ABI query result and the child
/// enforcement result, with no syscall.
///
/// `abi_query` is the `landlock_create_ruleset(... VERSION)` result — `Ok(0)` is
/// an unsupported or disabled kernel, `Ok(n)` with `n >= 1` is ABI `n`, and
/// `Err(errno)` is a failed query (including a seccomp denial). `enforcement` is
/// the `restrict_self` result and is consulted only for a supported ABI.
fn decide(kernel: &str, abi_query: Result<u32, i32>, enforcement: Result<(), String>) -> Detected {
    match abi_query {
        Ok(0) => Detected::Unavailable(format!(
            "landlock ABI 0 is not supported by this kernel (kernel {kernel})"
        )),
        Ok(abi) => match enforcement {
            Ok(()) => Detected::Available { abi },
            Err(reason) => {
                Detected::Unavailable(format!("{reason} (kernel {kernel}, landlock ABI {abi})"))
            }
        },
        Err(errno) => Detected::Unavailable(format!(
            "landlock ABI query failed (kernel {kernel}, ABI query errno {errno})"
        )),
    }
}

/// The pure decision as a [`SandboxAvailability`], so the stage-1b tests can
/// substitute the backend judgement through `install_for_test` with the real
/// reason and no syscall.
#[cfg(test)]
pub(crate) fn decide_availability(
    kernel: &str,
    abi_query: Result<u32, i32>,
    enforcement: Result<(), String>,
) -> SandboxAvailability {
    match decide(kernel, abi_query, enforcement) {
        Detected::Available { .. } => SandboxAvailability::Available,
        Detected::Unavailable(reason) => SandboxAvailability::Unavailable(reason),
    }
}

/// Ask the kernel for its Landlock ABI version. A negative return is the kernel
/// reporting an unsupported or disabled Landlock (`ENOSYS`/`EOPNOTSUPP`) or a
/// seccomp filter denying the syscall; the `Err` carries the raw `errno`.
fn query_abi() -> Result<u32, i32> {
    // SAFETY: with a null attr pointer, size 0 and the VERSION flag, landlock_create_ruleset only
    // returns the ABI number; it reads no memory and creates no file descriptor to leak.
    let ret = unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            std::ptr::null::<libc::c_void>(),
            0usize,
            LANDLOCK_CREATE_RULESET_VERSION,
        )
    };
    if ret < 0 {
        Err(errno())
    } else {
        Ok(ret as u32)
    }
}

/// Build a ruleset for the detected ABI and confirm a child can enforce it.
///
/// This exercises both syscalls the backend depends on: creating the ruleset
/// (already done here) and `landlock_restrict_self` in a short-lived child, so a
/// seccomp profile that allows creation but denies enforcement is still
/// reported as unavailable.
fn probe_restrict_self(abi: u32) -> Result<(), String> {
    let fd = probe_ruleset(abi)?;
    let raw = fd.as_raw_fd();
    let mut command = probe_command();
    // SAFETY: `pre_exec` runs after fork. The closure performs only the two
    // syscalls in `apply_restriction` and allocates, locks, and logs nothing.
    unsafe {
        command.pre_exec(move || apply_restriction(raw));
    }
    let output = crate::bounded_process::run_with_timeout(&mut command, PROBE_TIMEOUT)
        .map_err(|err| format!("landlock probe could not start: {err}"))?;
    if output.success() {
        Ok(())
    } else {
        Err(format!(
            "landlock probe exited with {}",
            output
                .status
                .map(|status| status.to_string())
                .unwrap_or_else(|| "no status".to_string())
        ))
    }
}

/// The ruleset the availability probe enforces in its short-lived child.
///
/// It handles only the write accesses (ABI1) and allows no location, so the
/// child keeps its read and execute access and can still `exec` the probe
/// program. The probe only needs to confirm that `landlock_restrict_self` is
/// enforceable (not denied by seccomp or the kernel); handling the read set here
/// too would deny the child's own `exec` and be misread as "unavailable".
fn probe_ruleset(abi: u32) -> Result<OwnedFd, String> {
    let access = AccessFs::from_write(ABI::from(abi as i32));
    let created = Ruleset::default()
        // ABI1 must be enforceable; otherwise the backend is unavailable.
        .set_compatibility(CompatLevel::HardRequirement)
        .handle_access(access)
        .map_err(|err| format!("landlock ruleset setup failed: {err}"))?
        .create()
        .map_err(|err| format!("landlock ruleset creation failed: {err}"))?;
    Option::<OwnedFd>::from(created)
        .ok_or_else(|| "landlock ruleset is not enforceable on this kernel".to_string())
}

fn probe_command() -> Command {
    for candidate in ["/bin/true", "/usr/bin/true"] {
        if std::path::Path::new(candidate).exists() {
            let mut command = Command::new(candidate);
            command
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            return command;
        }
    }
    let mut command = Command::new("true");
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

/// Create a ruleset that denies every handled read and write access outside
/// `writable` (read+write+execute), `read_only` (read+execute), and `list_only`
/// (read-dir only), returning its file descriptor for the child to enforce.
///
/// `handle_access` uses `from_all(abi)` so both the read layer and the write
/// layer are enforced; `Execute | ReadFile | ReadDir` all exist from ABI1, so
/// this adds no ABI requirement over the write layer.
///
/// `list_only` grants `ReadDir` alone, never `ReadFile`, so a directory that
/// [`expand_read_allow_limited`] had to recurse into (an ancestor of a denied location,
/// such as `/dev` once the terminal devices are denied) stays listable while the
/// denied subtree beneath it stays unreadable.
///
/// For an `--offline` run with ABI4 (Issue #658), `AccessNet::ConnectTcp` is also
/// handled, which denies every TCP connect, and each `dev_server_ports` entry is
/// re-allowed with a [`NetPort`] rule. Before ABI4 Landlock cannot restrict the
/// network, so nothing is handled and [`network_limitation_note`] records the
/// gap instead of claiming confinement.
fn create_ruleset(
    abi: u32,
    writable: impl IntoIterator<Item = PathBuf>,
    read_only: impl IntoIterator<Item = PathBuf>,
    list_only: impl IntoIterator<Item = PathBuf>,
    offline: bool,
    dev_server_ports: &[u16],
) -> Result<OwnedFd, String> {
    let abi_value = abi;
    let abi = ABI::from(abi as i32);
    let read_access = AccessFs::from_read(abi);
    let full_access = AccessFs::from_all(abi);
    // Landlock gained the network access rights in ABI4 (kernel 6.7).
    let restrict_network = offline && abi_value >= 4;
    let mut ruleset = Ruleset::default()
        // ABI1 must be enforceable; otherwise the backend is unavailable.
        .set_compatibility(CompatLevel::HardRequirement)
        .handle_access(full_access)
        .map_err(|err| format!("landlock ruleset setup failed: {err}"))?;
    if restrict_network {
        ruleset = ruleset
            .handle_access(AccessNet::ConnectTcp)
            .map_err(|err| format!("landlock network ruleset setup failed: {err}"))?;
    }
    // Rules are best-effort so a file location silently drops directory-only
    // accesses instead of failing the whole ruleset.
    let created = ruleset
        .set_compatibility(CompatLevel::BestEffort)
        .create()
        .map_err(|err| format!("landlock ruleset creation failed: {err}"))?;
    let created = created
        .add_rules(path_beneath_rules(writable, full_access))
        .map_err(|err| format!("landlock write rule failed: {err}"))?;
    let created = created
        .add_rules(path_beneath_rules(read_only, read_access))
        .map_err(|err| format!("landlock read rule failed: {err}"))?;
    let created = created
        .add_rules(path_beneath_rules(list_only, AccessFs::ReadDir))
        .map_err(|err| format!("landlock list-only rule failed: {err}"))?;
    let created = if restrict_network {
        add_network_ports(created, dev_server_ports)?
    } else {
        created
    };
    Option::<OwnedFd>::from(created)
        .ok_or_else(|| "landlock ruleset is not enforceable on this kernel".to_string())
}

/// Re-allow a TCP connect to each dev-server port on an `--offline` ruleset that
/// otherwise handles (and so denies) every `ConnectTcp`. Landlock matches the
/// port only, so a `NetPort` rule cannot distinguish the local dev server from
/// an external host on the same port; that limitation is documented and recorded.
fn add_network_ports(
    mut created: RulesetCreated,
    dev_server_ports: &[u16],
) -> Result<RulesetCreated, String> {
    for &port in dev_server_ports {
        created = created
            .add_rule(NetPort::new(port, AccessNet::ConnectTcp))
            .map_err(|err| format!("landlock network rule failed: {err}"))?;
    }
    Ok(created)
}

/// Probe the backend once. Returns [`SandboxAvailability::Unavailable`] with an
/// honest reason when Landlock is unsupported, disabled, or denied.
pub fn probe() -> SandboxAvailability {
    match detect() {
        Detected::Available { .. } => SandboxAvailability::Available,
        Detected::Unavailable(reason) => SandboxAvailability::Unavailable(reason.clone()),
    }
}

/// The best-effort limitation of the detected ABI, recorded in
/// `bash_sandbox_status` while the sandbox is applied, or `None` when every
/// supported operation is closed.
pub fn limitation_note() -> Option<String> {
    match detect() {
        Detected::Available { abi } => limitation_for(&kernel_release(), *abi),
        Detected::Unavailable(_) => None,
    }
}

/// The pure ABI limitation: ABI1 cannot close cross-directory link/rename
/// (ABI2), and ABI1/ABI2 cannot close `truncate` (ABI3). ABI3 and later close
/// both, so there is no note.
fn limitation_for(kernel: &str, abi: u32) -> Option<String> {
    let mut open = Vec::new();
    if abi < 2 {
        open.push("linking or renaming a file across directories (ABI2)");
    }
    if abi < 3 {
        open.push("truncating a file (ABI3)");
    }
    if open.is_empty() {
        None
    } else {
        Some(format!(
            "landlock ABI {abi} (kernel {kernel}) applied; not closed: {}",
            open.join(", ")
        ))
    }
}

/// The network-isolation clause recorded in `bash_sandbox_status` for an
/// `--offline` run whose kernel cannot confine the network (Issue #658).
///
/// Landlock restricts TCP connect only from ABI4 (kernel 6.7). Before that the
/// network is left unchanged, so the event must say so rather than claim
/// confinement; `--offline` then relies on the lexical guard for the network.
/// A non-`--offline` run is unchanged and has no clause, and an unavailable
/// backend already records its own reason, so this returns `None` there.
pub fn network_limitation_note(offline: bool) -> Option<String> {
    let detected = match detect() {
        Detected::Available { abi } => Some(*abi),
        Detected::Unavailable(_) => None,
    };
    network_limitation_for(offline, detected, &kernel_release())
}

/// The pure network limitation: `None` unless an `--offline` run is on a kernel
/// whose detected ABI is below 4. `None` for an unavailable backend, whose own
/// reason already says the sandbox is not applied.
fn network_limitation_for(
    offline: bool,
    detected_abi: Option<u32>,
    kernel: &str,
) -> Option<String> {
    if !offline {
        return None;
    }
    match detected_abi {
        Some(abi) if abi >= 4 => None,
        Some(abi) => Some(format!(
            "network is not confined: landlock ABI {abi} (kernel {kernel}) cannot restrict TCP connect (ABI4, kernel 6.7, is required); --offline relies on lexical checks for the network"
        )),
        None => None,
    }
}

/// The detected Landlock ABI, or 0 when the backend is unavailable. Test record.
pub fn detected_abi() -> u32 {
    match detect() {
        Detected::Available { abi } => *abi,
        Detected::Unavailable(_) => 0,
    }
}

/// The running kernel release string, for the availability record. Test record.
pub fn kernel_release_label() -> String {
    kernel_release()
}

/// Wrap `command` so its child is confined to the policy's allowed locations.
///
/// The ruleset is created and every location descriptor opened here in the
/// parent. Returns `Err` (with the reason) when the ruleset cannot be built, so
/// a `required` run fails honestly instead of spawning an unconfined command.
pub fn wrap(command: &mut Command, policy: &SandboxPolicy) -> Result<(), String> {
    #[cfg(test)]
    if test_hooks::wrap_should_fail() {
        return Err("landlock ruleset could not be built (test hook)".to_string());
    }
    let fd = create_ruleset(
        detected_abi(),
        allowed_paths(policy),
        read_allow_paths(policy),
        list_only_paths(),
        policy.offline,
        &policy.dev_server_ports,
    )?;
    // Point tools at the run temp directory, which the ruleset allows.
    command.env("TMPDIR", &policy.run_tmp_dir);
    let raw = fd.as_raw_fd();
    // SAFETY: `pre_exec` runs after fork. The closure keeps the ruleset
    // descriptor open across the fork and calls only the two syscalls in
    // `apply_restriction`; it does not allocate, lock, or log.
    unsafe {
        command.pre_exec(move || {
            let _ruleset_descriptor = &fd;
            apply_restriction(raw)
        });
    }
    Ok(())
}

/// The locations the sandbox leaves writable: the workspace, the run temp
/// directory, the known dependency caches, and the device files that exist.
/// These also get the read and execute rights, because a location that is
/// writable must be readable and its binaries executable.
///
/// Because each of these locations also grants read on its whole subtree, a
/// writable location that is a home secret or an ancestor of one re-opens that
/// secret; [`read_isolation_gaps`] detects it.
pub(crate) fn allowed_paths(policy: &SandboxPolicy) -> Vec<PathBuf> {
    let mut paths = vec![policy.workspace_root.clone(), policy.run_tmp_dir.clone()];
    paths.extend(existing(policy.caches.cargo_registry.clone()));
    paths.extend(existing(policy.caches.cargo_git.clone()));
    paths.extend(existing(policy.caches.cargo_package_cache.clone()));
    paths.extend(existing(policy.caches.npm.clone()));
    paths.extend(existing(policy.caches.pip.clone()));
    paths.extend(existing(policy.caches.playwright.clone()));
    for device in DEVICE_PATHS {
        let path = PathBuf::from(device);
        if path.exists() {
            paths.push(path);
        }
    }
    paths
}

/// The read-only locations: the expansion of the read-deny list into its
/// complement (see [`expand_read_allow_limited`]). Without this, handling the read
/// rights would make every location unreadable. `home_secret_locations`
/// canonicalizes each location, so a symlinked secret location (`~/.ssh` that
/// points outside the home) denies its target too; the terminal devices of
/// [`terminal_read_deny_paths`] deny the run's terminal like any other denied
/// location.
fn read_allow_paths(policy: &SandboxPolicy) -> Vec<PathBuf> {
    match read_allow_plan(policy, super::read_allow_limit()) {
        ReadAllowPlan::Full(paths)
        | ReadAllowPlan::HomeClosed(paths)
        | ReadAllowPlan::HomeClosedOverLimit(paths) => paths,
    }
}

/// The most read-allow rules the deny-list expansion may produce before the
/// whole home is denied instead (Issue #668). The expansion adds one rule per
/// sibling along every path to a denied location, so it grows with the number
/// of entries beside the home's secrets.
pub(crate) const MAX_READ_ALLOW_RULES: usize = 4096;

/// How the read layer is built for a policy. The ruleset is always built: the
/// limit only decides whether the home is closed and how the status reads. A
/// rule count never makes the sandbox fall away.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReadAllowPlan {
    /// Every home secret is denied one by one and the rest stays readable.
    Full(Vec<PathBuf>),
    /// The full expansion exceeds the limit, so the whole home is denied
    /// instead. Its siblings are allowed, so the rule count no longer follows
    /// the home's entries; the writable locations stay readable through their
    /// write rules.
    HomeClosed(Vec<PathBuf>),
    /// The closed-home expansion exceeds the limit too (the home's parents are
    /// crowded). It is built anyway, as it would be without a limit; only the
    /// status records it as not effective.
    HomeClosedOverLimit(Vec<PathBuf>),
}

/// The read-allow plan for a policy under `limit` rules.
pub(crate) fn read_allow_plan(policy: &SandboxPolicy, limit: usize) -> ReadAllowPlan {
    let mut denied = super::secret_locations::home_secret_locations(&policy.home);
    denied.extend(terminal_read_deny_paths());
    if let Some(paths) = expand_read_allow_limited(Path::new("/"), &denied, limit) {
        return ReadAllowPlan::Full(paths);
    }
    if !policy.home.as_os_str().is_empty() {
        denied.push(super::secret_locations::resolve_nearest_existing(
            &policy.home,
        ));
    }
    let paths = expand_read_allow_limited(Path::new("/"), &denied, usize::MAX)
        .expect("an unlimited expansion always completes");
    if paths.len() > limit {
        ReadAllowPlan::HomeClosedOverLimit(paths)
    } else {
        ReadAllowPlan::HomeClosed(paths)
    }
}

/// The terminal device locations removed from the read allow list (Issue #667),
/// for the given device directory (normally `/dev`).
///
/// A bounded child `setsid()`s, so it has no controlling terminal, but it can
/// still re-open the run's terminal by path and read the keys the user types:
/// `<dev>/pts/N` on Linux (the run's terminal is always a pty slave) and
/// `<dev>/tty` (the controlling-terminal alias). Landlock cannot deny part of an
/// allowed tree, so these locations join the read-deny list and
/// [`expand_read_allow_limited`] leaves them and their subtrees out.
///
/// The physical console and serial terminals (`<dev>/console`, `<dev>/tty0`,
/// `<dev>/ttyS*`, …) are the other terminals a child could name, so every
/// existing entry whose name is `console` or starts with `tty` is denied too;
/// only the entries that exist are listed, and denying them costs nothing a
/// confined command needs. `<dev>/ptmx` and the null/zero/random devices are not
/// terminal paths and stay readable.
fn terminal_read_deny_paths_in(dev: &Path) -> Vec<PathBuf> {
    let mut paths = vec![dev.join("pts"), dev.join("tty")];
    if let Ok(entries) = std::fs::read_dir(dev) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name == "console" || name.starts_with("tty") {
                paths.push(entry.path());
            }
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

/// The terminal device locations for the real `/dev`.
fn terminal_read_deny_paths() -> Vec<PathBuf> {
    terminal_read_deny_paths_in(Path::new("/dev"))
}

/// The directories granted `ReadDir` alone. [`expand_read_allow_limited`] recurses into
/// every ancestor of a denied location without allowing the ancestor itself, so
/// listing such a directory would fail once the terminal devices are denied.
/// `/dev` is enumerated by tools (`ls /dev`), so it is listed here; the grant is
/// `ReadDir` only, so it never re-opens the terminal devices beneath it.
fn list_only_paths() -> Vec<PathBuf> {
    let dev = PathBuf::from("/dev");
    if dev.is_dir() { vec![dev] } else { Vec::new() }
}

/// The home secret locations whose read denial a writable location defeats, so
/// the read layer is not fully effective. A secret is defeated when a writable
/// location is the same path or an ancestor of it (the write allow then grants
/// read on the secret's subtree) or, more narrowly, when the writable location
/// is *inside* the secret (only that subtree is readable, but the denial is
/// still partial). Both writable and denied paths are canonicalized first, so a
/// workspace or cache reached through a symlink is compared by its real path.
/// Pure apart from that canonicalization.
pub(crate) fn read_isolation_gaps(writable: &[PathBuf], denied: &[PathBuf]) -> Vec<PathBuf> {
    let writable: Vec<PathBuf> = writable
        .iter()
        .map(|path| super::secret_locations::resolve_nearest_existing(path))
        .collect();
    denied
        .iter()
        .filter(|secret| {
            let secret = secret.as_path();
            writable
                .iter()
                .any(|root| secret.starts_with(root) || root.starts_with(secret))
        })
        .cloned()
        .collect()
}

/// Rewrite a deny list into a read-allow list for Landlock.
///
/// Landlock is an allow list: an object is readable only if some rule covers it,
/// and a rule on a directory also covers its whole subtree, so a deny list must
/// be expressed as its complement. For every directory on the path to a denied
/// location, allow each child that does not contain a denied location, and
/// recurse into the children that do; the denied location itself is never
/// allowed, so only its subtree stays unreadable. The toolchain, git, and the
/// system locations are covered because their top-level and home siblings are
/// allowed here.
///
/// The expansion reads the directories once, when the run starts, so a location
/// created under an expanded directory afterwards is not readable in this run.
///
/// The walk starts from an explicit root, so a test can drive it inside a
/// tempdir instead of the filesystem root. An empty deny list (for example when
/// `HOME` is unset) has no location to protect, so the walk allows every child
/// of the root and reads stay unchanged rather than becoming unreadable.
///
/// It gives up with `None` once the list would hold more than `limit` rules, so
/// an enormous directory cannot grow the ruleset without bound (Issue #668).
fn expand_read_allow_limited(
    root: &std::path::Path,
    denied: &[PathBuf],
    limit: usize,
) -> Option<Vec<PathBuf>> {
    let mut allow = Vec::new();
    let mut frontier = vec![root.to_path_buf()];
    let mut visited: HashSet<PathBuf> = HashSet::new();
    while let Some(directory) = frontier.pop() {
        if !visited.insert(directory.clone()) {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            // `path_beneath_rules` opens each allowed path with `O_PATH`, which
            // follows symlinks, so a rule for a symlink entry would attach to
            // its target. A link that points at a secret would then open that
            // secret (and a secret location that is itself a link would open
            // its target). Skip symlink entries: the target is decided by the
            // real path, which the walk reaches on its own when it is outside a
            // secret and leaves unmatched when it is a secret.
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }
            if denied.iter().any(|secret| secret == &path) {
                // The denied location itself stays unreadable and is not opened.
                continue;
            }
            if denied.iter().any(|secret| secret.starts_with(&path)) {
                // A strict ancestor of a denied location: open up its own
                // children without reading the denied subtree.
                frontier.push(path);
            } else {
                allow.push(path);
                if allow.len() > limit {
                    return None;
                }
            }
        }
    }
    Some(allow)
}

/// [`expand_read_allow_limited`] without a limit, for the tests that drive the
/// walk inside a tempdir.
#[cfg(test)]
fn expand_read_allow_from(root: &std::path::Path, denied: &[PathBuf]) -> Vec<PathBuf> {
    expand_read_allow_limited(root, denied, usize::MAX).expect("no limit")
}

fn existing(path: Option<PathBuf>) -> Option<PathBuf> {
    path.filter(|path| path.exists())
}

/// Apply the prepared ruleset in the freshly forked child.
///
/// # pre_exec contract
///
/// This runs between `fork` and `exec`, so it must not allocate, take a lock, or
/// log. It performs exactly two syscalls: `prctl(PR_SET_NO_NEW_PRIVS)` and
/// `landlock_restrict_self`. Error values are built with
/// [`std::io::Error::from_raw_os_error`], which does not allocate.
fn apply_restriction(ruleset_fd: libc::c_int) -> std::io::Result<()> {
    // SAFETY: prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) takes only integer arguments, is
    // async-signal-safe, and allocates nothing, as the pre_exec contract above requires.
    if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0 {
        return Err(std::io::Error::from_raw_os_error(errno()));
    }
    // SAFETY: a raw syscall with only integer arguments (the ruleset fd the caller still owns,
    // and zero flags), so it touches no memory and allocates nothing in the forked child.
    let rc = unsafe {
        libc::syscall(
            libc::SYS_landlock_restrict_self,
            ruleset_fd as libc::c_long,
            0 as libc::c_long,
        )
    };
    if rc != 0 {
        return Err(std::io::Error::from_raw_os_error(errno()));
    }
    Ok(())
}

/// Read `errno` without allocating.
fn errno() -> libc::c_int {
    // SAFETY: __errno_location returns a valid pointer to the calling thread's errno, which is
    // read immediately on that same thread and needs no allocation.
    unsafe { *libc::__errno_location() }
}

/// The running kernel release from `/proc`, or `unknown` when it cannot be read.
fn kernel_release() -> String {
    std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|release| release.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

/// Test-only switch that forces [`wrap`] to fail so the `auto` degradation path
/// (the probe said available but the ruleset could not be built) can be
/// exercised on a kernel that supports Landlock.
#[cfg(test)]
pub(crate) mod test_hooks {
    use std::sync::atomic::{AtomicBool, Ordering};

    static FORCE_WRAP_FAILURE: AtomicBool = AtomicBool::new(false);

    pub(crate) fn force_wrap_failure(force: bool) {
        FORCE_WRAP_FAILURE.store(force, Ordering::SeqCst);
    }

    pub(crate) fn wrap_should_fail() -> bool {
        FORCE_WRAP_FAILURE.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abi_zero_is_unavailable_and_names_the_kernel_and_abi() {
        let Detected::Unavailable(reason) = decide("6.1.0-generic", Ok(0), Ok(())) else {
            panic!("ABI 0 must be unavailable");
        };
        assert!(reason.contains("6.1.0-generic"), "{reason}");
        assert!(reason.contains("ABI"), "{reason}");
    }

    #[test]
    fn a_failed_abi_query_is_unavailable_and_names_the_kernel() {
        let Detected::Unavailable(reason) = decide("6.1.0-generic", Err(38), Ok(())) else {
            panic!("a failed query must be unavailable");
        };
        assert!(reason.contains("6.1.0-generic"), "{reason}");
        assert!(reason.contains("ABI"), "{reason}");
        assert!(reason.contains("38"), "{reason}");
    }

    #[test]
    fn a_supported_abi_is_available() {
        for abi in [1u32, 2, 3, 4, 9] {
            assert!(
                matches!(
                    decide("6.1.0", Ok(abi), Ok(())),
                    Detected::Available { abi: found } if found == abi
                ),
                "ABI {abi} must be available"
            );
        }
    }

    /// The availability probe must still let its child `exec`. The probe ruleset
    /// handles only the write accesses, so the child keeps read/execute; a
    /// regression that also handles the read set (without allowing the probe's
    /// own reads) makes the child fail with EACCES and the backend be
    /// misreported as unavailable.
    #[test]
    fn probe_child_can_exec_under_the_probe_ruleset() {
        // Use the raw ABI query, not `detected_abi()`: the latter is 0 when the
        // probe itself fails, which would hide the regression under test.
        let abi = match query_abi() {
            Ok(abi) if abi >= 1 => abi,
            _ => {
                eprintln!("SKIP: landlock is unavailable on this kernel");
                return;
            }
        };
        probe_restrict_self(abi).expect("the probe child must exec under the probe ruleset");
    }

    #[test]
    fn an_enforcement_failure_is_unavailable_and_names_the_kernel_and_abi() {
        let Detected::Unavailable(reason) = decide(
            "6.1.0",
            Ok(2),
            Err("probe exited with exit status: 1".to_string()),
        ) else {
            panic!("an enforcement failure must be unavailable");
        };
        assert!(reason.contains("6.1.0"), "{reason}");
        assert!(reason.contains("landlock ABI 2"), "{reason}");
    }

    #[test]
    fn read_allow_expansion_keeps_siblings_and_excludes_denied_subtrees() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("secret/inner")).unwrap();
        std::fs::write(root.join("secret/inner/key"), "CANARY\n").unwrap();
        std::fs::create_dir_all(root.join("nested/gh")).unwrap();
        std::fs::create_dir_all(root.join("nested/other")).unwrap();
        std::fs::create_dir_all(root.join("public")).unwrap();
        let denied = vec![root.join("secret"), root.join("nested/gh")];

        let allow = expand_read_allow_from(root, &denied);
        assert!(allow.contains(&root.join("public")), "{allow:?}");
        // A directory that only contains denied locations is opened up for its
        // own non-denied children, not allowed wholesale.
        assert!(allow.contains(&root.join("nested/other")), "{allow:?}");
        assert!(!allow.contains(&root.join("nested")), "{allow:?}");
        assert!(
            allow.iter().all(|path| !path.starts_with(&denied[0])),
            "{allow:?}"
        );
        assert!(
            allow.iter().all(|path| !path.starts_with(&denied[1])),
            "{allow:?}"
        );
    }

    /// Issue #668: denying the whole home makes the expansion independent of the
    /// home's entries, so it fits a limit the per-secret expansion exceeds.
    #[test]
    fn closing_the_home_keeps_the_expansion_small() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("home/.ssh")).unwrap();
        for index in 0..40 {
            std::fs::create_dir_all(root.join(format!("home/entry-{index}"))).unwrap();
        }
        std::fs::create_dir_all(root.join("usr")).unwrap();

        let per_secret = vec![root.join("home/.ssh")];
        assert!(expand_read_allow_limited(root, &per_secret, 10).is_none());
        let closed = expand_read_allow_limited(root, &[root.join("home")], 10)
            .expect("the closed home fits");
        assert!(closed.contains(&root.join("usr")), "{closed:?}");
        assert!(
            closed
                .iter()
                .all(|path| !path.starts_with(root.join("home")))
        );
    }

    #[test]
    fn read_isolation_gaps_reports_secrets_covered_by_writable_locations() {
        let home = PathBuf::from("/home/alice");
        let denied = vec![home.join(".ssh"), home.join(".aws")];

        // A writable location that is the home itself covers both secrets.
        assert_eq!(
            read_isolation_gaps(std::slice::from_ref(&home), &denied),
            denied
        );

        // A writable location inside one secret covers only that secret.
        let workspace = home.join(".ssh/workspace");
        assert_eq!(
            read_isolation_gaps(&[workspace], &denied),
            vec![home.join(".ssh")]
        );

        // A writable location outside the secrets covers none.
        assert!(read_isolation_gaps(&[PathBuf::from("/tmp/work")], &denied).is_empty());
    }

    #[test]
    fn read_isolation_gaps_canonicalizes_writable_locations() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir_all(real.join(".ssh")).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let denied = vec![real.canonicalize().unwrap().join(".ssh")];

        // The workspace is reached through a symlink, so only canonicalizing the
        // writable location reveals that it is the secret's ancestor.
        assert_eq!(read_isolation_gaps(&[link], &denied), denied);
    }

    #[test]
    fn an_empty_deny_list_allows_every_child_so_reads_stay_open() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("one")).unwrap();
        std::fs::create_dir_all(root.join("two")).unwrap();
        let allow = expand_read_allow_from(root, &[]);
        assert!(allow.contains(&root.join("one")), "{allow:?}");
        assert!(allow.contains(&root.join("two")), "{allow:?}");
    }

    #[test]
    fn expand_read_allow_skips_a_symlink_that_points_at_a_secret() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().canonicalize().unwrap();
        std::fs::create_dir_all(home.join(".ssh")).unwrap();
        let key = home.join(".ssh/id_ed25519");
        std::fs::write(&key, "CANARY\n").unwrap();
        let note = home.join("notes.txt");
        std::os::unix::fs::symlink(&key, &note).unwrap();
        let denied = vec![home.join(".ssh")];

        let allow = expand_read_allow_from(&home, &denied);
        assert!(
            !allow.contains(&note),
            "the symlink entry must not be allowed: {allow:?}"
        );
        let key = key.canonicalize().unwrap();
        assert!(
            allow
                .iter()
                .all(|path| path.canonicalize().ok().as_ref() != Some(&key)),
            "no rule may reach the secret inode: {allow:?}"
        );
    }

    #[test]
    fn expand_read_allow_skips_a_secret_location_that_is_a_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().canonicalize().unwrap();
        std::fs::create_dir_all(&home).unwrap();
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("id_rsa"), "CANARY\n").unwrap();
        let ssh = home.join(".ssh");
        std::os::unix::fs::symlink(&outside, &ssh).unwrap();
        let outside = outside.canonicalize().unwrap();
        let denied = vec![outside.clone()];

        let allow = expand_read_allow_from(&home, &denied);
        assert!(
            !allow.contains(&ssh),
            "the symlink entry must not be allowed: {allow:?}"
        );
        assert!(
            allow
                .iter()
                .all(|path| path.canonicalize().ok().as_ref() != Some(&outside)),
            "no rule may reach the secret target inode: {allow:?}"
        );
    }

    /// Issue #667: the terminal devices are removed from the read allow list,
    /// while the devices a confined command needs and their non-terminal siblings
    /// stay readable, and `/dev` itself is not allowed wholesale (it is an
    /// ancestor of a denied location).
    #[test]
    fn read_allow_excludes_terminal_devices_and_keeps_the_required_ones() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let dev = root.join("dev");
        std::fs::create_dir_all(dev.join("pts")).unwrap();
        std::fs::create_dir_all(dev.join("shm")).unwrap();
        for file in [
            "tty", "tty0", "console", "null", "zero", "urandom", "random", "ptmx",
        ] {
            std::fs::write(dev.join(file), "").unwrap();
        }
        let denied = vec![
            dev.join("pts"),
            dev.join("tty"),
            dev.join("tty0"),
            dev.join("console"),
        ];

        let allow = expand_read_allow_from(root, &denied);
        for kept in ["null", "zero", "urandom", "random", "ptmx", "shm"] {
            assert!(
                allow.contains(&dev.join(kept)),
                "{kept} must stay readable: {allow:?}"
            );
        }
        for gone in ["pts", "tty", "tty0", "console"] {
            let path = dev.join(gone);
            assert!(
                !allow.contains(&path),
                "{path:?} must stay unreadable: {allow:?}"
            );
            assert!(
                allow.iter().all(|entry| !entry.starts_with(&path)),
                "no rule may reach below {path:?}: {allow:?}"
            );
        }
        assert!(
            !allow.contains(&dev),
            "/dev is an ancestor of a denied location and must be recursed, not allowed: {allow:?}"
        );
    }

    /// Issue #667: the read-deny list names the pty-slave directory and the
    /// controlling-terminal alias, so `read_allow_paths` leaves the run's
    /// terminal out.
    #[test]
    fn terminal_read_deny_paths_names_the_pty_directory_and_the_controlling_tty() {
        let denied = terminal_read_deny_paths();
        assert!(denied.contains(&PathBuf::from("/dev/pts")), "{denied:?}");
        assert!(denied.contains(&PathBuf::from("/dev/tty")), "{denied:?}");
    }

    /// Issue #667: given a device directory, the deny list is exactly the
    /// pty-slave directory and every terminal device name — `console`, `tty`,
    /// `tty0`, `ttyS0`, `ttyUSB0` — and never a non-terminal entry (`null`,
    /// `urandom`, `ptmx`) or a non-terminal directory (`shm`). A mutation that
    /// widens or narrows the name test (`||` to `&&`, `==` to `!=`) changes this
    /// set, so the check pins both.
    #[test]
    fn terminal_read_deny_paths_in_selects_exactly_the_terminal_devices() {
        let dir = tempfile::tempdir().unwrap();
        let dev = dir.path();
        std::fs::create_dir_all(dev.join("pts")).unwrap();
        std::fs::create_dir_all(dev.join("shm")).unwrap();
        for file in [
            "tty", "console", "tty0", "ttyS0", "ttyUSB0", "null", "urandom", "ptmx",
        ] {
            std::fs::write(dev.join(file), "").unwrap();
        }

        let denied = terminal_read_deny_paths_in(dev);
        let expected = vec![
            dev.join("console"),
            dev.join("pts"),
            dev.join("tty"),
            dev.join("tty0"),
            dev.join("ttyS0"),
            dev.join("ttyUSB0"),
        ];
        assert_eq!(denied, expected);
    }

    /// Issue #667: `/dev` stays listable through the `ReadDir`-only rule even
    /// though a denied location sits below it.
    #[test]
    fn list_only_paths_keeps_dev_listable() {
        assert_eq!(list_only_paths(), vec![PathBuf::from("/dev")]);
    }

    /// Issue #657: the read layer needs no ABI beyond ABI1, so removing the
    /// terminal devices from the read allow list adds no new minimum. `Execute`,
    /// `ReadFile`, and `ReadDir` are all part of `from_read` from ABI1, and
    /// `ReadDir` (the `list_only` grant) is therefore always enforceable.
    #[test]
    fn read_access_is_available_from_abi1() {
        let read = AccessFs::from_read(ABI::from(1));
        assert!(read.contains(AccessFs::Execute), "{read:?}");
        assert!(read.contains(AccessFs::ReadFile), "{read:?}");
        assert!(read.contains(AccessFs::ReadDir), "{read:?}");
    }

    #[test]
    fn limitation_note_reports_the_abi2_and_abi3_gaps() {
        let abi1 = limitation_for("6.1.0", 1).expect("ABI 1 has gaps");
        assert!(abi1.contains("ABI2") && abi1.contains("ABI3"), "{abi1}");
        assert!(
            abi1.contains("landlock ABI 1") && abi1.contains("6.1.0"),
            "{abi1}"
        );

        let abi2 = limitation_for("6.1.0", 2).expect("ABI 2 still has a gap");
        assert!(abi2.contains("ABI3") && !abi2.contains("ABI2"), "{abi2}");

        assert!(limitation_for("6.1.0", 3).is_none());
        assert!(limitation_for("6.1.0", 9).is_none());
    }

    /// Issue #658: the network limitation is named only for an `--offline` run
    /// on a kernel below ABI4; ABI4 or later confines the network, a non-offline
    /// run is unchanged, and an unavailable backend has its own reason.
    #[test]
    fn network_limitation_names_the_missing_abi4_only_when_offline() {
        assert!(network_limitation_for(false, Some(1), "6.1.0").is_none());
        assert!(network_limitation_for(true, Some(4), "6.7.0").is_none());
        assert!(network_limitation_for(true, Some(9), "6.7.0").is_none());
        assert!(network_limitation_for(true, None, "6.1.0").is_none());

        let note = network_limitation_for(true, Some(3), "6.1.0")
            .expect("ABI 3 cannot confine the network");
        assert!(note.contains("ABI4") && note.contains("6.1.0"), "{note}");
    }
}
