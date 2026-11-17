//! The strict launch of a `propose-artifacts` process on Linux (Issue #982).
//!
//! `isolation.rs` builds the final `Command` and routes here. This leaf adds one fixed sequence
//! of plain system calls between fork and exec, in the child, on memory the parent built:
//!
//! 1. new user, mount, network, IPC, and UTS namespaces; `setgroups` denied and the host's
//!    uid and gid mapped to a non-zero inner id; a fixed host name;
//! 2. an empty `tmpfs` root holding, read-only, only what [`root`] lists; the root itself
//!    read-only, `pivot_root` into it, the old root detached, and the working directory
//!    entered there (std's `current_dir` would run before this, on the host's tree);
//! 3. `PR_SET_NO_NEW_PRIVS`, then Landlock: read where the root allows it, run only the pinned
//!    executable and its ELF interpreter, write only `/dev/null`;
//! 4. last, the seccomp filter of [`filter`].
//!
//! No `/proc` is mounted, so another process's arguments and environment are not there to
//! read. There is no PID namespace: the tool stays the host's direct child with its host pid,
//! so the bounded run in `process.rs` stops, reaps, and looks for descendants as it does on
//! macOS. Any step that fails stops the launch before the exec, with the step named, and
//! nothing falls back to a launch without it.
//!
//! Between fork and exec only async-signal-safe work happens: one system call per step through
//! its `libc` wrapper, on paths, buffers, a Landlock ruleset descriptor, and a BPF program the
//! parent made. Nothing there allocates, takes a lock, formats, or can panic.

mod filter;
mod root;

use std::collections::BTreeMap;
use std::ffi::{CStr, CString, OsStr};
use std::io;
use std::os::fd::{AsRawFd as _, OwnedFd};
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use landlock::{
    ABI, Access as _, AccessFs, CompatLevel, Compatible as _, PathBeneath, PathFd, Ruleset,
    RulesetAttr as _, RulesetCreatedAttr as _,
};

use super::super::process::{self, Ended, RunLimits};
use super::{PROBE_MAX_BYTES, PROBE_TIMEOUT, StrictLaunch, final_command, probe_area};
use root::{Grant, Op};

/// The Landlock floor: ABI 5 is the first that also handles `ioctl` on devices. A kernel below
/// it is unavailable (ubuntu-22.04's 6.8 has ABI 4).
const LANDLOCK_FLOOR: ABI = ABI::V5;
/// The uid and gid the tool has inside. Not 0: an exec drops every capability there.
const INNER_ID: u32 = 1000;
const HOST_NAME: &[u8] = b"commandagent-tool";
const NAMESPACES: libc::c_int = libc::CLONE_NEWUSER
    | libc::CLONE_NEWNS
    | libc::CLONE_NEWNET
    | libc::CLONE_NEWIPC
    | libc::CLONE_NEWUTS;

/// One step of the sequence, as the refusal names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum Step {
    Namespaces = 1,
    Setgroups,
    UidMap,
    GidMap,
    HostName,
    PrivateMounts,
    EmptyRoot,
    Directory,
    File,
    Link,
    Bind,
    BindReadOnly,
    RootReadOnly,
    EnterRoot,
    PivotRoot,
    DetachOldRoot,
    WorkingDirectory,
    NoNewPrivileges,
    Landlock,
    Seccomp,
}

const STEPS: [Step; 20] = [
    Step::Namespaces,
    Step::Setgroups,
    Step::UidMap,
    Step::GidMap,
    Step::HostName,
    Step::PrivateMounts,
    Step::EmptyRoot,
    Step::Directory,
    Step::File,
    Step::Link,
    Step::Bind,
    Step::BindReadOnly,
    Step::RootReadOnly,
    Step::EnterRoot,
    Step::PivotRoot,
    Step::DetachOldRoot,
    Step::WorkingDirectory,
    Step::NoNewPrivileges,
    Step::Landlock,
    Step::Seccomp,
];

impl Step {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Namespaces => "unshare(user, mount, network, IPC, UTS)",
            Self::Setgroups => "setgroups deny",
            Self::UidMap => "uid_map",
            Self::GidMap => "gid_map",
            Self::HostName => "sethostname",
            Self::PrivateMounts => "private mount propagation",
            Self::EmptyRoot => "tmpfs root",
            Self::Directory => "a directory in the root",
            Self::File => "a bind target in the root",
            Self::Link => "a link in the root",
            Self::Bind => "a bind mount",
            Self::BindReadOnly => "a read-only bind remount",
            Self::RootReadOnly => "the root read-only",
            Self::EnterRoot => "chdir to the root",
            Self::PivotRoot => "pivot_root",
            Self::DetachOldRoot => "detaching the old root",
            Self::WorkingDirectory => "chdir to the working directory",
            Self::NoNewPrivileges => "PR_SET_NO_NEW_PRIVS",
            Self::Landlock => "landlock_restrict_self",
            Self::Seccomp => "seccomp",
        }
    }
}

/// A refused step travels to the parent as the spawn error's number: this base, the step, and
/// the errno. No errno reaches this range, so the parent can tell a refusal from an exec error.
const STEP_BASE: i32 = 0x4c00_0000;
const STEP_SHIFT: u32 = 12;
const ERRNO_MASK: i32 = (1 << STEP_SHIFT) - 1;

fn step_error(step: Step, errno: i32) -> io::Error {
    io::Error::from_raw_os_error(STEP_BASE | (step as i32) << STEP_SHIFT | (errno & ERRNO_MASK))
}

/// The step that stopped a launch before its exec, with its errno.
pub(super) fn refused_step(error: &io::Error) -> Option<(Step, i32)> {
    let raw = error.raw_os_error()?;
    if raw & !((0xff << STEP_SHIFT) | ERRNO_MASK) != STEP_BASE {
        return None;
    }
    let number = (raw >> STEP_SHIFT) & 0xff;
    let step = STEPS.iter().copied().find(|step| *step as i32 == number)?;
    Some((step, raw & ERRNO_MASK))
}

/// What a refusal says.
pub(super) fn describe_refusal(step: Step, errno: i32) -> String {
    format!(
        "was refused at `{}` ({})",
        step.name(),
        io::Error::from_raw_os_error(errno)
    )
}

fn errno() -> i32 {
    // SAFETY: reads this thread's errno, which is async-signal-safe.
    unsafe { *libc::__errno_location() }
}

/// Everything the child needs, made by the parent. The child only reads it, and writes its own
/// pid into the filter it copied at the fork.
pub(super) struct Confinement {
    uid_line: Vec<u8>,
    gid_line: Vec<u8>,
    root: CString,
    ops: Vec<Op>,
    cwd: CString,
    ruleset: OwnedFd,
    filter: Vec<libc::sock_filter>,
    pid_slots: Vec<usize>,
    #[cfg(test)]
    fault: Option<Step>,
    /// The empty host directory the root is mounted on. It goes when the `Command` goes, after
    /// the spawn, when the child has left it behind at the pivot.
    _staging: tempfile::TempDir,
}

fn ruleset(grants: &BTreeMap<PathBuf, Grant>) -> Result<OwnedFd, String> {
    let landlock = |error: &dyn std::fmt::Display| format!("Landlock: {error}");
    let mut ruleset = Ruleset::default()
        .set_compatibility(CompatLevel::HardRequirement)
        .handle_access(AccessFs::from_all(LANDLOCK_FLOOR))
        .map_err(|error| landlock(&error))?
        .create()
        .map_err(|error| landlock(&error))?;
    for (path, grant) in grants {
        let directory = path.is_dir();
        let access = match grant {
            Grant::Visible => continue,
            Grant::Read if directory => AccessFs::ReadFile | AccessFs::ReadDir,
            Grant::Read => AccessFs::ReadFile.into(),
            Grant::Run => AccessFs::ReadFile | AccessFs::Execute,
            Grant::ReadWrite => AccessFs::ReadFile | AccessFs::WriteFile | AccessFs::Truncate,
        };
        let opened = PathFd::new(path).map_err(|error| landlock(&error))?;
        ruleset = ruleset
            .add_rule(PathBeneath::new(opened, access))
            .map_err(|error| format!("Landlock rule for `{}`: {error}", path.display()))?;
    }
    Option::<OwnedFd>::from(ruleset).ok_or_else(|| "Landlock made no ruleset".to_string())
}

/// The parent's whole share of the work, before the fork.
pub(super) fn prepare(launch: &StrictLaunch<'_>) -> Result<Confinement, String> {
    let arch = filter::native().ok_or_else(|| {
        format!(
            "no seccomp table for {}; only x86_64 and aarch64 have one",
            std::env::consts::ARCH
        )
    })?;
    let staging = tempfile::Builder::new()
        .prefix(".commandagent-root-")
        .tempdir()
        .map_err(|error| format!("no directory to mount the root on: {error}"))?;
    let at = std::fs::canonicalize(staging.path())
        .map_err(|error| format!("no directory to mount the root on: {error}"))?;
    let plan = root::plan(
        &root::Wanted {
            executable: launch.executable,
            read: &launch.read_directories,
            visible: &launch.link_directories,
        },
        &at,
    )?;
    let ruleset = ruleset(&plan.grants)?;
    let program = filter::build(arch);
    // SAFETY: `getuid` and `getgid` have no preconditions.
    let (uid, gid) = unsafe { (libc::getuid(), libc::getgid()) };
    Ok(Confinement {
        uid_line: format!("{INNER_ID} {uid} 1\n").into_bytes(),
        gid_line: format!("{INNER_ID} {gid} 1\n").into_bytes(),
        root: root::cstring(&at)?,
        ops: plan.ops,
        cwd: root::cstring(launch.cwd)?,
        ruleset,
        filter: program.instructions,
        pid_slots: program.pid_slots,
        #[cfg(test)]
        fault: kernel_tests::planted_fault(),
        _staging: staging,
    })
}

/// Opens, writes, and closes one file of `/proc/self`.
fn write_once(path: &CStr, data: &[u8]) -> bool {
    // SAFETY: `open`, `write`, and `close` are async-signal-safe, and `path` and `data` were made
    // by the parent and outlive the calls.
    unsafe {
        let descriptor = libc::open(path.as_ptr(), libc::O_WRONLY | libc::O_CLOEXEC);
        if descriptor < 0 {
            return false;
        }
        let written = libc::write(descriptor, data.as_ptr().cast(), data.len());
        libc::close(descriptor);
        usize::try_from(written).is_ok_and(|written| written == data.len())
    }
}

impl Confinement {
    /// The answer for one step: the errno of its call, or a planted test fault.
    fn judge(&self, step: Step, done: bool) -> io::Result<()> {
        let errno = errno();
        #[cfg(test)]
        if self.fault == Some(step) {
            return Err(step_error(step, libc::EPERM));
        }
        if done {
            Ok(())
        } else {
            Err(step_error(step, errno))
        }
    }

    /// The sequence, between fork and exec. Called from `pre_exec` only.
    fn enter(&mut self) -> io::Result<()> {
        let none = std::ptr::null::<libc::c_char>();
        let nothing = std::ptr::null::<libc::c_void>();
        // SAFETY: for each call below: one system call through its `libc` wrapper, which takes no
        // lock and does not allocate, on a pointer to a `CStr`, buffer, or program the parent
        // made, which lives as long as this closure.
        unsafe {
            self.judge(Step::Namespaces, libc::unshare(NAMESPACES) == 0)?;
            self.judge(
                Step::Setgroups,
                write_once(c"/proc/self/setgroups", b"deny"),
            )?;
            self.judge(
                Step::UidMap,
                write_once(c"/proc/self/uid_map", &self.uid_line),
            )?;
            self.judge(
                Step::GidMap,
                write_once(c"/proc/self/gid_map", &self.gid_line),
            )?;
            self.judge(
                Step::HostName,
                libc::sethostname(HOST_NAME.as_ptr().cast(), HOST_NAME.len()) == 0,
            )?;
            self.judge(
                Step::PrivateMounts,
                libc::mount(
                    none,
                    c"/".as_ptr(),
                    none,
                    libc::MS_REC | libc::MS_PRIVATE,
                    nothing,
                ) == 0,
            )?;
            self.judge(
                Step::EmptyRoot,
                libc::mount(
                    c"tmpfs".as_ptr(),
                    self.root.as_ptr(),
                    c"tmpfs".as_ptr(),
                    libc::MS_NOSUID | libc::MS_NODEV,
                    c"size=1m,mode=0755".as_ptr().cast(),
                ) == 0,
            )?;
            for op in &self.ops {
                match op {
                    Op::Directory(path) => {
                        let made = libc::mkdir(path.as_ptr(), 0o755) == 0;
                        self.judge(Step::Directory, made || errno() == libc::EEXIST)?;
                    }
                    Op::File(path) => {
                        let flags = libc::O_CREAT | libc::O_WRONLY | libc::O_CLOEXEC;
                        let descriptor = libc::open(path.as_ptr(), flags, 0o644);
                        self.judge(Step::File, descriptor >= 0)?;
                        libc::close(descriptor);
                    }
                    Op::Link { target, path } => {
                        let made = libc::symlink(target.as_ptr(), path.as_ptr()) == 0;
                        self.judge(Step::Link, made)?;
                    }
                    Op::Bind {
                        source,
                        target,
                        remount,
                    } => {
                        let bound = libc::mount(
                            source.as_ptr(),
                            target.as_ptr(),
                            none,
                            libc::MS_BIND,
                            nothing,
                        );
                        self.judge(Step::Bind, bound == 0)?;
                        let read_only = libc::mount(none, target.as_ptr(), none, *remount, nothing);
                        self.judge(Step::BindReadOnly, read_only == 0)?;
                    }
                }
            }
            let read_only = libc::MS_REMOUNT
                | libc::MS_BIND
                | libc::MS_RDONLY
                | libc::MS_NOSUID
                | libc::MS_NODEV;
            self.judge(
                Step::RootReadOnly,
                libc::mount(none, self.root.as_ptr(), none, read_only, nothing) == 0,
            )?;
            self.judge(Step::EnterRoot, libc::chdir(self.root.as_ptr()) == 0)?;
            // `pivot_root(".", ".")` puts the old root on top of the new one; detaching the top
            // of "." then leaves only the new root, with no directory kept for the old one.
            let pivoted = libc::syscall(libc::SYS_pivot_root, c".".as_ptr(), c".".as_ptr());
            self.judge(Step::PivotRoot, pivoted == 0)?;
            self.judge(
                Step::DetachOldRoot,
                libc::umount2(c".".as_ptr(), libc::MNT_DETACH) == 0,
            )?;
            self.judge(Step::WorkingDirectory, libc::chdir(self.cwd.as_ptr()) == 0)?;
            let (on, unused): (libc::c_ulong, libc::c_ulong) = (1, 0);
            self.judge(
                Step::NoNewPrivileges,
                libc::prctl(libc::PR_SET_NO_NEW_PRIVS, on, unused, unused, unused) == 0,
            )?;
            let restricted = libc::syscall(
                libc::SYS_landlock_restrict_self,
                self.ruleset.as_raw_fd(),
                0u32,
            );
            self.judge(Step::Landlock, restricted == 0)?;
            // No PID namespace: the pid here is the host pid the tool signals itself with.
            let pid = libc::getpid().cast_unsigned();
            for slot in &self.pid_slots {
                if let Some(instruction) = self.filter.get_mut(*slot) {
                    instruction.k = pid;
                }
            }
            let program = libc::sock_fprog {
                len: u16::try_from(self.filter.len()).unwrap_or(0),
                filter: self.filter.as_mut_ptr(),
            };
            let installed = libc::syscall(
                libc::SYS_seccomp,
                libc::SECCOMP_SET_MODE_FILTER,
                0u32,
                &raw const program,
            );
            self.judge(Step::Seccomp, installed == 0)
        }
    }
}

/// Adds the sequence to the final command, or says why it cannot be made.
pub(super) fn confine(command: &mut Command, launch: &StrictLaunch<'_>) -> Result<(), String> {
    let mut confinement = prepare(launch)?;
    // SAFETY: `enter` makes only async-signal-safe calls, on memory made before the fork; it
    // allocates nothing and takes no lock. An error from it stops the exec.
    unsafe {
        command.pre_exec(move || confinement.enter());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The probe.
// ---------------------------------------------------------------------------

const PROBE_SHELL: &str = "/bin/bash";

/// One round trip, then a write into a bound directory, a read of a host file the root does
/// not hold, a TCP and a UDP connection, and a fork, each of which must be refused by the kernel.
/// The fork is last: bash stops at a refused fork.
const PROBE_SCRIPT: &str = r#"IFS= read -r line; printf '%s\n' "$line"; { : > "$1"; }; { : < "$2"; }; { : > "/dev/tcp/127.0.0.1/$3"; }; { : > "/dev/udp/127.0.0.1/$4"; }; ( : ); exit 0"#;

/// Before the first provider call, as on macOS: the whole sequence in a real child. Any other
/// answer than the expected refusals is unavailable, and a timeout proves nothing.
pub(super) fn probe() -> Result<(), String> {
    let area = tempfile::tempdir().map_err(|_| "has no temporary directory".to_string())?;
    let result = probe_in(area.path());
    probe_area::release(area);
    result
}

/// What the probe saw, for [`judge_probe`].
struct ProbeSeen<'a> {
    stdout: &'a [u8],
    stderr: &'a [u8],
    ended: Ended,
    clean: bool,
    wrote: bool,
    connected: bool,
}

fn probe_in(area: &Path) -> Result<(), String> {
    let no_area = |_| "has no temporary directory".to_string();
    let area = std::fs::canonicalize(area).map_err(no_area)?;
    let bound = area.join("bound");
    let hidden = area.join("hidden");
    std::fs::create_dir(&bound).map_err(no_area)?;
    std::fs::write(&hidden, "outside\n").map_err(no_area)?;
    let target = bound.join("probe-write");
    let no_witness = |_| "cannot hold a local witness socket".to_string();
    let tcp = std::net::TcpListener::bind("127.0.0.1:0").map_err(no_witness)?;
    tcp.set_nonblocking(true).map_err(no_witness)?;
    let udp = std::net::UdpSocket::bind("127.0.0.1:0").map_err(no_witness)?;
    udp.set_nonblocking(true).map_err(no_witness)?;
    let ports = [tcp.local_addr(), udp.local_addr()].map(|address| address.map(|a| a.port()));
    let [Ok(tcp_port), Ok(udp_port)] = ports else {
        return Err("cannot hold a local witness socket".to_string());
    };
    let nonce = uuid::Uuid::now_v7().to_string();
    let argv = [
        "-c".to_string(),
        PROBE_SCRIPT.to_string(),
        "probe".to_string(),
        target.display().to_string(),
        hidden.display().to_string(),
        tcp_port.to_string(),
        udp_port.to_string(),
    ];
    let env = BTreeMap::new();
    let launch = StrictLaunch {
        executable: Path::new(PROBE_SHELL),
        argv: &argv,
        cwd: &bound,
        env: &env,
        read_directories: vec![bound.as_path()],
        link_directories: Vec::new(),
    };
    let mut strict = final_command(OsStr::new(PROBE_SHELL), &[], &launch, false);
    confine(&mut strict.command, &launch)
        .map_err(|reason| format!("cannot be prepared: {reason}"))?;
    let observation = process::run(
        strict,
        format!("{nonce}\n").as_bytes(),
        RunLimits {
            timeout: PROBE_TIMEOUT,
            max_stream_bytes: PROBE_MAX_BYTES,
        },
        &|| false,
    )
    .map_err(|error| match refused_step(&error) {
        Some((step, errno)) => describe_refusal(step, errno),
        None => "could not start".to_string(),
    })?;
    let mut datagram = [0u8; 16];
    let seen = ProbeSeen {
        stdout: &observation.stdout,
        stderr: &observation.stderr,
        ended: observation.ended,
        clean: observation.reaped
            && observation.stdin_complete
            && !observation.read_failed
            && observation.descendants_left == 0
            && observation.descendants_remaining == 0,
        wrote: target.exists(),
        connected: tcp.accept().is_ok() || udp.recv(&mut datagram).is_ok(),
    };
    judge_probe(&seen, &nonce)
}

/// Only the kernel's own refusal of each attempt counts: a timeout, a signal, or an unreachable
/// address is not one.
fn judge_probe(seen: &ProbeSeen<'_>, nonce: &str) -> Result<(), String> {
    if seen.stdout != format!("{nonce}\n").as_bytes() {
        return Err("did not complete a round trip".to_string());
    }
    if seen.wrote {
        return Err("was able to write".to_string());
    }
    if seen.connected {
        return Err("was able to connect".to_string());
    }
    match seen.ended {
        Ended::Exited(Some(code)) if code != 0 => {}
        Ended::Exited(Some(_)) => return Err("was able to fork".to_string()),
        Ended::Exited(None) => return Err("ended with `signaled`".to_string()),
        Ended::TimedOut => return Err("ended with `timed_out`".to_string()),
        Ended::Cancelled => return Err("ended with `cancelled`".to_string()),
        Ended::OutputExceeded => return Err("ended with `output_exceeded`".to_string()),
        Ended::HeldOutput => return Err("ended with `descendant_held_output`".to_string()),
    }
    if !seen.clean {
        return Err("did not end cleanly".to_string());
    }
    let stderr = String::from_utf8_lossy(seen.stderr);
    let count = |ending: &str| stderr.lines().filter(|line| line.ends_with(ending)).count();
    let counts = [
        count("probe-write: Read-only file system"),
        count("hidden: No such file or directory"),
        count("socket: Operation not permitted"),
        count("fork: Operation not permitted"),
    ];
    if counts != [1, 1, 2, 1] {
        return Err(
            "did not see the kernel refuse the write, the read, both sockets, and the fork"
                .to_string(),
        );
    }
    Ok(())
}

/// The tests against the real kernel; their body is `kernel_tests::tests`.
#[cfg(test)]
mod kernel_tests;
