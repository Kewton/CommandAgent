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
//!   ([`expand_read_allow`]): Landlock cannot deny part of an allowed tree, so
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
const DEVICE_PATHS: &[&str] = &[
    "/dev/null",
    "/dev/zero",
    "/dev/full",
    "/dev/random",
    "/dev/urandom",
    "/dev/tty",
    "/dev/ptmx",
    "/dev/pts",
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
/// `writable` (read+write+execute) and `read_only` (read+execute), returning its
/// file descriptor for the child to enforce.
///
/// `handle_access` uses `from_all(abi)` so both the read layer and the write
/// layer are enforced; `Execute | ReadFile | ReadDir` all exist from ABI1, so
/// this adds no ABI requirement over the write layer.
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
    paths.extend(cache_paths(&policy.home));
    for device in DEVICE_PATHS {
        let path = PathBuf::from(device);
        if path.exists() {
            paths.push(path);
        }
    }
    paths
}

/// The read-only locations: the expansion of the home secret deny list into its
/// complement (see [`expand_read_allow`]). Without this, handling the read
/// rights would make every location unreadable. `home_secret_locations`
/// canonicalizes each location, so a symlinked secret location (`~/.ssh` that
/// points outside the home) denies its target too.
fn read_allow_paths(policy: &SandboxPolicy) -> Vec<PathBuf> {
    expand_read_allow(&super::secret_locations::home_secret_locations(
        &policy.home,
    ))
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
        .map(|path| canonicalize_or_keep(path))
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

/// Canonicalize a path, keeping it as-is when it does not exist or cannot be
/// resolved.
fn canonicalize_or_keep(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
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
fn expand_read_allow(denied: &[PathBuf]) -> Vec<PathBuf> {
    expand_read_allow_from(std::path::Path::new("/"), denied)
}

/// [`expand_read_allow`] from an explicit root, so a test can drive the same
/// walk inside a tempdir instead of the filesystem root.
///
/// An empty deny list (for example when `HOME` is unset) has no location to
/// protect, so the walk allows every child of the root and reads stay unchanged
/// rather than becoming unreadable.
fn expand_read_allow_from(root: &std::path::Path, denied: &[PathBuf]) -> Vec<PathBuf> {
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
            }
        }
    }
    allow
}

/// The Linux dependency caches allowed for writing (decision 2). Only those that
/// exist are returned.
fn cache_paths(home: &std::path::Path) -> Vec<PathBuf> {
    [
        home.join(".cargo/registry"),
        home.join(".cargo/git"),
        home.join(".cargo/.package-cache"),
        home.join(".npm"),
        home.join(".cache/pip"),
        home.join(".cache/ms-playwright"),
    ]
    .into_iter()
    .filter(|path| path.exists())
    .collect()
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
    if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0 {
        return Err(std::io::Error::from_raw_os_error(errno()));
    }
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
