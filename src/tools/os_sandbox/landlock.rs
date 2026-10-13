//! Linux Landlock backend (Issue #656, stage 1b).
//!
//! Confines the writes of model Bash and verifier commands with Landlock. The
//! ruleset and every allowed-location file descriptor are opened in the parent;
//! the forked child's `pre_exec` runs only `prctl(PR_SET_NO_NEW_PRIVS)` and
//! `landlock_restrict_self`, so no work that could allocate, lock, or log
//! happens between `fork` and `exec`.
//!
//! Design (issue #656, decisions from #502):
//! - ABI1 (kernel 5.13) is required. ABI2 (`refer`, link/rename across
//!   directories) and ABI3 (`truncate`) are used best-effort; when they are
//!   missing the still-open operations are named in the `bash_sandbox_status`
//!   reason.
//! - Reads and network are not handled, so they stay unchanged, matching 1a.
//! - Availability is probed once at run start from the Landlock ABI query and a
//!   child `restrict_self`, so a seccomp filter that denies either syscall is an
//!   honest "unavailable" instead of a silent pass.

use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::Duration;

use landlock::{
    ABI, AccessFs, CompatLevel, Compatible, Ruleset, RulesetAttr, RulesetCreatedAttr,
    path_beneath_rules,
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
    let fd = create_ruleset(abi, std::iter::empty::<PathBuf>())?;
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

/// Create a ruleset that denies the ABI's write accesses outside `allowed`,
/// returning its file descriptor for the child to enforce.
fn create_ruleset(abi: u32, allowed: impl IntoIterator<Item = PathBuf>) -> Result<OwnedFd, String> {
    let access = AccessFs::from_write(ABI::from(abi as i32));
    let created = Ruleset::default()
        // ABI1 must be enforceable; otherwise the backend is unavailable.
        .set_compatibility(CompatLevel::HardRequirement)
        .handle_access(access)
        .map_err(|err| format!("landlock ruleset setup failed: {err}"))?
        // Rules are best-effort so a file location silently drops directory-only
        // accesses instead of failing the whole ruleset.
        .set_compatibility(CompatLevel::BestEffort)
        .create()
        .map_err(|err| format!("landlock ruleset creation failed: {err}"))?;
    let created = created
        .add_rules(path_beneath_rules(allowed, access))
        .map_err(|err| format!("landlock rule failed: {err}"))?;
    Option::<OwnedFd>::from(created)
        .ok_or_else(|| "landlock ruleset is not enforceable on this kernel".to_string())
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
    let fd = create_ruleset(detected_abi(), allowed_paths(policy))?;
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
fn allowed_paths(policy: &SandboxPolicy) -> Vec<PathBuf> {
    let mut paths = vec![policy.workspace_root.clone(), policy.run_tmp_dir.clone()];
    paths.extend(existing(policy.caches.cargo_registry.clone()));
    paths.extend(existing(policy.caches.cargo_git.clone()));
    paths.extend(existing(policy.caches.cargo_package_cache.clone()));
    paths.extend(existing(policy.caches.npm.clone()));
    paths.extend(existing(policy.caches.pip.clone()));
    paths.extend(existing(policy.caches.playwright.clone()));
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        paths.extend(cache_paths(&home));
    }
    for device in DEVICE_PATHS {
        let path = PathBuf::from(device);
        if path.exists() {
            paths.push(path);
        }
    }
    paths
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
}
