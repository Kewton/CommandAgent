//! The strict launch of a `propose-artifacts` process (Issue #974, G3).
//!
//! The existing `os_sandbox` is not used. Its Seatbelt wrapper builds a new `Command` and
//! copies the environment over, so a cleared environment does not survive it, and its
//! policy starts from `allow default` with a writable workspace. `wrap_command` hands back
//! an unconfined command when a backend is missing. Here the only macOS policy is a
//! `deny default` one, the final `Command` itself clears its environment, and every other
//! operating system is unavailable before the start. Nothing falls back to anything.
//!
//! The process writes nothing and reaches nothing: its input arrives on stdin and its
//! candidate leaves on stdout. Locations travel as `-D` parameters and never in the policy
//! text, as in the existing wrapper.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use super::process::{self, RunLimits};
use super::{ProfileToolRefusal, RefusalKind};

const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";
const PROBE_CHILD: &str = "/bin/bash";
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);
const PROBE_MAX_BYTES: usize = 4096;

/// Starting and loading the pinned executable needs these and nothing else (measured on
/// this policy, with no `process-fork`, `mach-lookup`, or network rule). A pinned location
/// is added per tool by [`strict_profile`].
const STRICT_PROFILE_HEAD: &str = r#"(version 1)
(deny default)
(allow process-exec (literal (param "EXE")))
(allow file-read* (literal (param "EXE")))
(allow file-read* (subpath "/usr/lib") (subpath "/System") (subpath "/private/var/db/dyld") (subpath "/Library/Apple/System") (literal "/dev/urandom") (literal "/dev/random") (literal "/dev/null") (literal "/"))
(allow file-write-data (literal "/dev/null"))
(allow sysctl-read)
(deny network*)
"#;

/// The fixed small child of the probe: one round trip, then a write and two connections
/// that must each be refused by the operating system. No subshell: forking is denied too.
const PROBE_SCRIPT: &str = r#"IFS= read -r line; printf '%s\n' "$line"; { : > "$1"; }; { : > "/dev/tcp/127.0.0.1/$2"; }; { : > "/dev/udp/127.0.0.1/$2"; }; exit 0"#;

/// One launch: what the operator pinned, and nothing a model chose.
#[derive(Debug)]
pub(super) struct StrictLaunch<'a> {
    pub(super) executable: &'a Path,
    pub(super) argv: &'a [String],
    pub(super) cwd: &'a Path,
    pub(super) env: &'a BTreeMap<String, String>,
    pub(super) read_directories: Vec<&'a Path>,
    pub(super) link_directories: Vec<&'a Path>,
}

#[cfg(test)]
pub(super) mod test_hook {
    use std::cell::Cell;

    thread_local! {
        static UNCONFINED: Cell<bool> = const { Cell::new(false) };
        static DESCRIPTOR_FAULT: Cell<Option<DescriptorFault>> = const { Cell::new(None) };
    }

    /// One way of not knowing that every descriptor is marked, read on this thread before
    /// the fork. Only macOS has the steps these name; Linux has one call.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(in super::super) enum DescriptorFault {
        #[cfg(target_os = "macos")]
        TableUnknown,
        #[cfg(target_os = "macos")]
        TableTooLarge,
        #[cfg(target_os = "macos")]
        ListFailed,
        #[cfg(target_os = "macos")]
        ListFull,
        MarkRefused,
    }

    pub(in super::super) struct FaultyDescriptors;

    impl Drop for FaultyDescriptors {
        fn drop(&mut self) {
            DESCRIPTOR_FAULT.with(|fault| fault.set(None));
        }
    }

    pub(in super::super) fn descriptor_fault(fault: DescriptorFault) -> FaultyDescriptors {
        DESCRIPTOR_FAULT.with(|slot| slot.set(Some(fault)));
        FaultyDescriptors
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub(super) fn current_descriptor_fault() -> Option<DescriptorFault> {
        DESCRIPTOR_FAULT.with(Cell::get)
    }

    /// Only a unit test of the parts around the launch can reach this: it does not exist
    /// in a production build. Those tests also run where no backend exists.
    pub(in super::super) struct Unconfined;

    impl Drop for Unconfined {
        fn drop(&mut self) {
            UNCONFINED.with(|flag| flag.set(false));
        }
    }

    pub(in super::super) fn unconfined() -> Unconfined {
        UNCONFINED.with(|flag| flag.set(true));
        Unconfined
    }

    pub(super) fn is_unconfined() -> bool {
        UNCONFINED.with(Cell::get)
    }
}

fn unavailable(reason: impl Into<String>) -> ProfileToolRefusal {
    ProfileToolRefusal::new(RefusalKind::EffectUnavailable, reason)
}

/// The policy for this many pinned locations. The locations stay parameters.
pub(super) fn strict_profile(reads: usize, links: usize, metadata: usize) -> String {
    let mut profile = String::from(STRICT_PROFILE_HEAD);
    for index in 0..reads {
        profile.push_str(&format!(
            "(allow file-read* (subpath (param \"READ_{index}\")))\n"
        ));
    }
    for index in 0..links {
        profile.push_str(&format!(
            "(allow file-read-metadata (subpath (param \"LINK_{index}\")))\n"
        ));
    }
    for index in 0..metadata {
        profile.push_str(&format!(
            "(allow file-read-metadata (literal (param \"META_{index}\")))\n"
        ));
    }
    profile
}

/// A pinned path and each directory above it: only their metadata is readable, so the
/// existence and size of anything else stay hidden.
fn metadata_paths(launch: &StrictLaunch<'_>) -> BTreeSet<PathBuf> {
    let mut paths = BTreeSet::new();
    let pinned = [launch.executable, launch.cwd]
        .into_iter()
        .chain(launch.read_directories.iter().copied())
        .chain(launch.link_directories.iter().copied());
    for path in pinned {
        for ancestor in path.ancestors() {
            if ancestor != Path::new("/") {
                paths.insert(ancestor.to_path_buf());
            }
        }
    }
    paths
}

fn define(arguments: &mut Vec<OsString>, name: &str, value: &Path) {
    let mut definition = OsString::from(format!("{name}="));
    definition.push(value.as_os_str());
    arguments.push(OsString::from("-D"));
    arguments.push(definition);
}

/// `-p <policy> -D ... <executable>`: what comes before the operator's argv.
fn sandbox_arguments(launch: &StrictLaunch<'_>) -> Vec<OsString> {
    let metadata = metadata_paths(launch);
    let mut arguments = vec![
        OsString::from("-p"),
        OsString::from(strict_profile(
            launch.read_directories.len(),
            launch.link_directories.len(),
            metadata.len(),
        )),
    ];
    define(&mut arguments, "EXE", launch.executable);
    for (index, path) in launch.read_directories.iter().enumerate() {
        define(&mut arguments, &format!("READ_{index}"), path);
    }
    for (index, path) in launch.link_directories.iter().enumerate() {
        define(&mut arguments, &format!("LINK_{index}"), path);
    }
    for (index, path) in metadata.iter().enumerate() {
        define(&mut arguments, &format!("META_{index}"), path);
    }
    arguments.push(launch.executable.as_os_str().to_os_string());
    arguments
}

/// A command only this leaf can make, so a process cannot be run any other way. The bounded
/// runner takes nothing else.
#[derive(Debug)]
pub(super) struct StrictCommand {
    command: Command,
}

impl StrictCommand {
    pub(super) fn into_command(self) -> Command {
        self.command
    }

    /// For a unit test of the runner itself.
    #[cfg(test)]
    pub(super) fn for_test(command: Command) -> Self {
        Self { command }
    }
}

/// The errno of a launch stopped before its exec. An exec does not report this one, so the
/// host can tell the stop from a program that could not start.
#[cfg(unix)]
const DESCRIPTORS_NOT_MARKED: i32 = libc::ENOTRECOVERABLE;

/// This spawn error means no program ran, because a descriptor could have reached it.
#[cfg(unix)]
pub(super) fn is_descriptor_refusal(error: &std::io::Error) -> bool {
    error.raw_os_error() == Some(DESCRIPTORS_NOT_MARKED)
}

#[cfg(not(unix))]
pub(super) fn is_descriptor_refusal(_error: &std::io::Error) -> bool {
    false
}

/// The system calls between fork and exec, chosen before the fork. A plain table of function
/// pointers: nothing here allocates, locks, or formats.
#[cfg(target_os = "macos")]
#[derive(Clone, Copy)]
struct DescriptorCalls {
    table_bytes: fn() -> i32,
    list: fn(&mut [libc::proc_fdinfo]) -> i32,
    mark: fn(i32) -> bool,
}

#[cfg(target_os = "macos")]
const REAL_DESCRIPTOR_CALLS: DescriptorCalls = DescriptorCalls {
    table_bytes: || {
        // SAFETY: a null buffer asks only for a size, and `getpid` has no preconditions.
        unsafe {
            libc::proc_pidinfo(
                libc::getpid(),
                libc::PROC_PIDLISTFDS,
                0,
                std::ptr::null_mut(),
                0,
            )
        }
    },
    list: |into| {
        // SAFETY: the buffer is writable for this many bytes, and it fits in an `i32`.
        unsafe {
            libc::proc_pidinfo(
                libc::getpid(),
                libc::PROC_PIDLISTFDS,
                0,
                into.as_mut_ptr().cast(),
                std::mem::size_of_val(into) as i32,
            )
        }
    },
    mark: |descriptor| {
        // SAFETY: `fcntl` is async-signal-safe, and this only sets a flag on a number.
        let answer = unsafe { libc::fcntl(descriptor, libc::F_SETFD, libc::FD_CLOEXEC) };
        mark_held(answer, std::io::Error::last_os_error().raw_os_error())
    },
};

/// Only a number that is not open needs no mark. Any other failure leaves it open at the exec.
#[cfg(target_os = "macos")]
fn mark_held(answer: i32, errno: Option<i32>) -> bool {
    answer == 0 || errno == Some(libc::EBADF)
}

/// Between fork and exec. A descriptor the host's own caller left open would otherwise reach
/// the child: the policy judges an open, not a read or a write on what is already open.
/// Marking instead of closing leaves the runtime's own descriptors for the moment of the exec.
///
/// The exec happens only when every open descriptor is known to be marked. The list reaches
/// a fixed buffer, so the size of the whole table is judged first: a table it might not cover,
/// a list that cannot be read, a full list, or a mark that fails stops the launch. A resource
/// limit says nothing here, because lowering one leaves a higher descriptor open.
#[cfg(target_os = "macos")]
fn mark_inherited_descriptors_close_on_exec(calls: DescriptorCalls) -> std::io::Result<()> {
    const CAPACITY: usize = 4096;
    let not_marked = || std::io::Error::from_raw_os_error(DESCRIPTORS_NOT_MARKED);
    let entry = std::mem::size_of::<libc::proc_fdinfo>();
    let table = usize::try_from((calls.table_bytes)()).unwrap_or(0);
    if table == 0 || table > CAPACITY * entry {
        return Err(not_marked());
    }
    // SAFETY: an all-zero `proc_fdinfo` is a valid value of this plain struct.
    let mut listed: [libc::proc_fdinfo; CAPACITY] = unsafe { std::mem::zeroed() };
    let count = usize::try_from((calls.list)(&mut listed)).unwrap_or(0) / entry;
    if count == 0 || count >= CAPACITY {
        return Err(not_marked());
    }
    for info in &listed[..count] {
        if info.proc_fd > 2 && !(calls.mark)(info.proc_fd) {
            return Err(not_marked());
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy)]
struct DescriptorCalls {
    mark_all: fn() -> bool,
}

#[cfg(target_os = "linux")]
const REAL_DESCRIPTOR_CALLS: DescriptorCalls = DescriptorCalls {
    mark_all: || {
        // SAFETY: this only sets a flag on a range of numbers, and takes no pointer.
        unsafe {
            libc::syscall(
                libc::SYS_close_range,
                3u32,
                u32::MAX,
                libc::CLOSE_RANGE_CLOEXEC,
            ) == 0
        }
    },
};

/// The kernel marks every number from 3 up, or the launch stops. No launch reaches this in
/// production: only a unit test runs a process here.
#[cfg(target_os = "linux")]
fn mark_inherited_descriptors_close_on_exec(calls: DescriptorCalls) -> std::io::Result<()> {
    if (calls.mark_all)() {
        Ok(())
    } else {
        Err(std::io::Error::from_raw_os_error(DESCRIPTORS_NOT_MARKED))
    }
}

/// Nothing here can show every descriptor marked. No launch reaches this: this platform has
/// no backend.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "linux"))))]
#[derive(Clone, Copy)]
struct DescriptorCalls;

#[cfg(all(unix, not(any(target_os = "macos", target_os = "linux"))))]
const REAL_DESCRIPTOR_CALLS: DescriptorCalls = DescriptorCalls;

#[cfg(all(unix, not(any(target_os = "macos", target_os = "linux"))))]
fn mark_inherited_descriptors_close_on_exec(_calls: DescriptorCalls) -> std::io::Result<()> {
    Err(std::io::Error::from_raw_os_error(DESCRIPTORS_NOT_MARKED))
}

#[cfg(all(test, target_os = "macos"))]
fn faulty_calls(fault: test_hook::DescriptorFault) -> DescriptorCalls {
    use test_hook::DescriptorFault as Fault;
    let real = REAL_DESCRIPTOR_CALLS;
    match fault {
        Fault::TableUnknown => DescriptorCalls {
            table_bytes: || 0,
            ..real
        },
        Fault::TableTooLarge => DescriptorCalls {
            table_bytes: || i32::MAX,
            ..real
        },
        Fault::ListFailed => DescriptorCalls {
            list: |_| 0,
            ..real
        },
        Fault::ListFull => DescriptorCalls {
            list: |into| std::mem::size_of_val(into) as i32,
            ..real
        },
        Fault::MarkRefused => DescriptorCalls {
            mark: |_| mark_held(-1, Some(libc::EPERM)),
            ..real
        },
    }
}

#[cfg(all(test, target_os = "linux"))]
fn faulty_calls(_fault: test_hook::DescriptorFault) -> DescriptorCalls {
    DescriptorCalls { mark_all: || false }
}

#[cfg(unix)]
fn descriptor_calls() -> DescriptorCalls {
    #[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
    if let Some(fault) = test_hook::current_descriptor_fault() {
        return faulty_calls(fault);
    }
    REAL_DESCRIPTOR_CALLS
}

/// The environment is cleared on this very `Command`: the declared keys are all the child
/// gets, and the host adds none. The new process group lets the whole group be stopped.
fn final_command(program: &OsStr, prefix: &[OsString], launch: &StrictLaunch<'_>) -> StrictCommand {
    let mut command = Command::new(program);
    command
        .args(prefix)
        .args(launch.argv)
        .current_dir(launch.cwd)
        .env_clear()
        .envs(launch.env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        command.process_group(0);
        let calls = descriptor_calls();
        // SAFETY: the closure makes only async-signal-safe calls, on the stack, and takes no
        // lock and allocates nothing. An error from it stops the exec.
        unsafe {
            command.pre_exec(move || mark_inherited_descriptors_close_on_exec(calls));
        }
    }
    StrictCommand { command }
}

/// The command for one launch, or a refusal. There is no unconfined form.
pub(super) fn strict_command(
    launch: &StrictLaunch<'_>,
) -> Result<StrictCommand, ProfileToolRefusal> {
    #[cfg(test)]
    if test_hook::is_unconfined() {
        return Ok(final_command(launch.executable.as_os_str(), &[], launch));
    }
    if !cfg!(target_os = "macos") {
        return Err(no_backend());
    }
    Ok(final_command(
        OsStr::new(SANDBOX_EXEC),
        &sandbox_arguments(launch),
        launch,
    ))
}

fn no_backend() -> ProfileToolRefusal {
    unavailable(format!(
        "strict process isolation has no backend on {}; a propose-artifacts tool cannot run here",
        std::env::consts::OS
    ))
}

/// Before the first provider call: this policy really starts a child, a round trip works,
/// and the operating system refuses a write and each connection. Any other answer is
/// unavailable. A nested sandbox ends here too, because `sandbox-exec` cannot start.
pub(super) fn probe_strict() -> Result<(), ProfileToolRefusal> {
    #[cfg(test)]
    if test_hook::is_unconfined() {
        return Ok(());
    }
    if !cfg!(target_os = "macos") {
        return Err(no_backend());
    }
    run_probe().map_err(|reason| unavailable(format!("the strict isolation probe {reason}")))
}

fn run_probe() -> Result<(), String> {
    let directory = tempfile::tempdir().map_err(|_| "has no temporary directory".to_string())?;
    let directory_path = std::fs::canonicalize(directory.path())
        .map_err(|_| "has no temporary directory".to_string())?;
    let target = directory_path.join("probe-write");
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| {
            listener.set_nonblocking(true)?;
            Ok(listener)
        })
        .map_err(|_| "cannot hold a local witness socket".to_string())?;
    let port = listener
        .local_addr()
        .map_err(|_| "cannot hold a local witness socket".to_string())?
        .port();
    let nonce = uuid::Uuid::now_v7().to_string();
    let argv = [
        "-c".to_string(),
        PROBE_SCRIPT.to_string(),
        "probe".to_string(),
        target.display().to_string(),
        port.to_string(),
    ];
    let env = BTreeMap::new();
    let launch = StrictLaunch {
        executable: Path::new(PROBE_CHILD),
        argv: &argv,
        cwd: Path::new("/"),
        env: &env,
        read_directories: Vec::new(),
        link_directories: Vec::new(),
    };
    let command = final_command(
        OsStr::new(SANDBOX_EXEC),
        &sandbox_arguments(&launch),
        &launch,
    );
    let observation = process::run(
        command,
        format!("{nonce}\n").as_bytes(),
        RunLimits {
            timeout: PROBE_TIMEOUT,
            max_stream_bytes: PROBE_MAX_BYTES,
        },
        &|| false,
    )
    .map_err(|_| "could not start".to_string())?;
    if let Some(code) = observation.failure_code() {
        return Err(format!("ended with `{code}`"));
    }
    judge_probe(
        &observation.stdout,
        &observation.stderr,
        &nonce,
        target.exists(),
        listener.accept().is_ok(),
    )
}

/// A timeout or an unreachable address proves nothing: only the operating system's own
/// refusal of the write and of both connections counts.
fn judge_probe(
    stdout: &[u8],
    stderr: &[u8],
    nonce: &str,
    wrote: bool,
    connected: bool,
) -> Result<(), String> {
    if stdout != format!("{nonce}\n").as_bytes() {
        return Err("did not complete a round trip".to_string());
    }
    if wrote {
        return Err("was able to write".to_string());
    }
    if connected {
        return Err("was able to connect".to_string());
    }
    let stderr = String::from_utf8_lossy(stderr);
    let connects = stderr
        .lines()
        .filter(|line| line.ends_with("connect: Operation not permitted"))
        .count();
    let writes = stderr
        .lines()
        .filter(|line| line.contains("probe-write: Operation not permitted"))
        .count();
    if writes != 1 || connects != 2 {
        return Err(
            "did not see the operating system refuse a write and both connections".to_string(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::Path;

    use super::{StrictLaunch, judge_probe, metadata_paths, probe_strict, strict_profile};

    fn launch<'a>(env: &'a BTreeMap<String, String>, argv: &'a [String]) -> StrictLaunch<'a> {
        StrictLaunch {
            executable: Path::new("/opt/tool/bin/tool"),
            argv,
            cwd: Path::new("/opt/tool/share"),
            env,
            read_directories: vec![Path::new("/opt/tool"), Path::new("/opt/lib/one")],
            link_directories: vec![Path::new("/opt/links")],
        }
    }

    #[test]
    fn the_policy_starts_from_deny_and_never_opens_a_write_network_or_fork() {
        let profile = strict_profile(2, 1, 3);
        assert!(profile.contains("(deny default)"));
        assert!(profile.contains("(deny network*)"));
        assert!(!profile.contains("allow default"));
        assert!(!profile.contains("allow network"));
        assert!(!profile.contains("process-fork"));
        assert!(!profile.contains("mach-lookup"));
        let writes = profile
            .lines()
            .filter(|line| line.contains("file-write"))
            .collect::<Vec<_>>();
        assert_eq!(writes, [r#"(allow file-write-data (literal "/dev/null"))"#]);
        let execs = profile
            .lines()
            .filter(|line| line.contains("process-exec"))
            .collect::<Vec<_>>();
        assert_eq!(execs, [r#"(allow process-exec (literal (param "EXE")))"#]);
    }

    #[test]
    fn metadata_covers_only_pinned_paths_and_their_ancestors() {
        let env = BTreeMap::new();
        let argv = [];
        let launch = launch(&env, &argv);
        let paths = metadata_paths(&launch);
        let names = paths
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            [
                "/opt",
                "/opt/lib",
                "/opt/lib/one",
                "/opt/links",
                "/opt/tool",
                "/opt/tool/bin",
                "/opt/tool/bin/tool",
                "/opt/tool/share",
            ]
        );
    }

    #[test]
    fn the_probe_judges_only_the_operating_systems_refusals() {
        let denied = concat!(
            "/bin/bash: /t/probe-write: Operation not permitted\n",
            "/bin/bash: connect: Operation not permitted\n",
            "/bin/bash: /dev/tcp/127.0.0.1/9: Operation not permitted\n",
            "/bin/bash: connect: Operation not permitted\n",
            "/bin/bash: /dev/udp/127.0.0.1/9: Operation not permitted\n",
        );
        assert_eq!(
            judge_probe(b"n\n", denied.as_bytes(), "n", false, false),
            Ok(())
        );
        assert!(judge_probe(b"x\n", denied.as_bytes(), "n", false, false).is_err());
        assert!(judge_probe(b"n\n", denied.as_bytes(), "n", true, false).is_err());
        assert!(judge_probe(b"n\n", denied.as_bytes(), "n", false, true).is_err());
        let refused_connection = denied.replace(
            "connect: Operation not permitted",
            "connect: Connection refused",
        );
        assert!(judge_probe(b"n\n", refused_connection.as_bytes(), "n", false, false).is_err());
        assert!(judge_probe(b"n\n", b"", "n", false, false).is_err());
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn off_macos_the_probe_and_the_launch_are_unavailable() {
        let refusal = probe_strict().unwrap_err();
        assert_eq!(refusal.kind.as_str(), "unavailable");
        let env = BTreeMap::new();
        let argv = [];
        assert_eq!(
            super::strict_command(&launch(&env, &argv))
                .unwrap_err()
                .kind
                .as_str(),
            "unavailable"
        );
    }

    /// A failed probe here is a failure, not a skip: nothing else proves the policy.
    #[cfg(target_os = "macos")]
    #[test]
    fn on_macos_the_real_probe_passes() {
        probe_strict().unwrap();
    }

    /// A descriptor the caller left open, without close-on-exec, at a number this returns.
    #[cfg(unix)]
    fn inheritable(file: std::fs::File) -> (std::fs::File, i32) {
        use std::os::fd::AsRawFd as _;
        let descriptor = file.as_raw_fd();
        // SAFETY: `file` owns this open descriptor; this only clears its close-on-exec flag.
        assert_eq!(unsafe { libc::fcntl(descriptor, libc::F_SETFD, 0) }, 0);
        (file, descriptor)
    }

    /// On every OS, through the same `final_command`, and what the probe uses too.
    #[cfg(unix)]
    #[test]
    fn a_descriptor_the_caller_left_open_does_not_reach_the_child() {
        use super::super::process::{RunLimits, run};
        let temp = tempfile::tempdir().unwrap();
        let (written, secret) = (temp.path().join("written"), temp.path().join("secret"));
        std::fs::write(&written, "").unwrap();
        std::fs::write(&secret, "dummy-inherited-secret\n").unwrap();
        let (_writer, writer_fd) = inheritable(
            std::fs::OpenOptions::new()
                .append(true)
                .open(&written)
                .unwrap(),
        );
        let (_reader, reader_fd) = inheritable(std::fs::File::open(&secret).unwrap());
        let script = format!(
            "{{ echo x >&{writer_fd}; }} || echo write-refused\n{{ read -r line <&{reader_fd}; }} || echo read-refused\necho \"line=[$line]\"\n"
        );
        let argv = ["-c".to_string(), script];
        let env = BTreeMap::new();
        let launch = StrictLaunch {
            executable: Path::new("/bin/sh"),
            argv: &argv,
            cwd: Path::new("/"),
            env: &env,
            read_directories: Vec::new(),
            link_directories: Vec::new(),
        };
        let _unconfined = super::test_hook::unconfined();
        let observation = run(
            super::strict_command(&launch).unwrap(),
            b"",
            RunLimits {
                timeout: std::time::Duration::from_secs(10),
                max_stream_bytes: 4096,
            },
            &|| false,
        )
        .unwrap();
        let stdout = String::from_utf8_lossy(&observation.stdout);
        assert!(stdout.contains("write-refused"), "{stdout}");
        assert!(stdout.contains("read-refused"), "{stdout}");
        assert!(stdout.contains("line=[]"), "{stdout}");
        assert_eq!(std::fs::read_to_string(&written).unwrap(), "");
    }

    /// The launch is refused and the program never ran: its first act would leave the mark.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn assert_refused_without_an_exec(fault: super::test_hook::DescriptorFault) {
        use super::super::process::{RunLimits, run};
        let temp = tempfile::tempdir().unwrap();
        let mark = temp.path().join("started");
        let argv = ["-c".to_string(), format!(": > '{}'", mark.display())];
        let env = BTreeMap::new();
        let launch = StrictLaunch {
            executable: Path::new("/bin/sh"),
            argv: &argv,
            cwd: Path::new("/"),
            env: &env,
            read_directories: Vec::new(),
            link_directories: Vec::new(),
        };
        let _unconfined = super::test_hook::unconfined();
        let limits = RunLimits {
            timeout: std::time::Duration::from_secs(10),
            max_stream_bytes: 4096,
        };
        {
            let _fault = super::test_hook::descriptor_fault(fault);
            let error = run(
                super::strict_command(&launch).unwrap(),
                b"",
                limits,
                &|| false,
            )
            .unwrap_err();
            assert!(super::is_descriptor_refusal(&error), "{fault:?}: {error}");
            assert!(!mark.exists(), "{fault:?}: the program ran");
        }
        // The witness: the same launch does start once nothing is wrong.
        let observation = run(
            super::strict_command(&launch).unwrap(),
            b"",
            limits,
            &|| false,
        )
        .unwrap();
        assert_eq!(observation.failure_code(), None);
        assert!(mark.exists());
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn a_mark_that_fails_stops_the_launch_before_the_exec() {
        assert_refused_without_an_exec(super::test_hook::DescriptorFault::MarkRefused);
    }

    /// The probe goes through the same launch, and its answer is unavailable.
    #[cfg(target_os = "macos")]
    #[test]
    fn the_probe_does_not_pass_when_the_marks_cannot_be_shown() {
        let _fault =
            super::test_hook::descriptor_fault(super::test_hook::DescriptorFault::ListFailed);
        assert_eq!(probe_strict().unwrap_err().kind.as_str(), "unavailable");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_list_that_is_full_failed_or_too_small_stops_the_launch_before_the_exec() {
        use super::test_hook::DescriptorFault as Fault;
        for fault in [
            Fault::TableUnknown,
            Fault::TableTooLarge,
            Fault::ListFailed,
            Fault::ListFull,
        ] {
            assert_refused_without_an_exec(fault);
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn only_a_number_that_is_not_open_needs_no_mark() {
        assert!(super::mark_held(0, None));
        assert!(super::mark_held(0, Some(libc::EPERM)));
        assert!(super::mark_held(-1, Some(libc::EBADF)));
        assert!(!super::mark_held(-1, Some(libc::EPERM)));
        assert!(!super::mark_held(-1, None));
    }

    #[cfg(target_os = "macos")]
    mod real_isolation {
        use std::collections::BTreeMap;
        use std::os::fd::{AsRawFd as _, FromRawFd as _, OwnedFd};
        use std::path::{Path, PathBuf};
        use std::time::Duration;

        use super::super::super::process::{self, RunLimits};
        use super::super::{StrictLaunch, is_descriptor_refusal, strict_command};

        const OWN_PROCESS: &str = "G3_DESCRIPTOR_TEST_OWN_PROCESS";

        /// A resource limit belongs to the whole process, and other tests run beside this one.
        /// So `scenario` runs in this same test binary started again for this test alone, with
        /// only the variables it needs.
        fn in_own_process(test: &str, scenario: fn()) {
            if std::env::var_os(OWN_PROCESS).is_some() {
                scenario();
                return;
            }
            let (_, module) = module_path!().split_once("::").unwrap();
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", &format!("{module}::{test}"), "--nocapture"])
                .env_clear()
                .env(OWN_PROCESS, "1")
                .env("TMPDIR", std::env::temp_dir())
                .output()
                .unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                output.status.success() && stdout.contains("1 passed"),
                "{stdout}\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        /// The soft descriptor limit, put back at the end.
        struct SoftLimit {
            before: libc::rlimit,
        }

        impl SoftLimit {
            fn hold() -> Self {
                let mut before = libc::rlimit {
                    rlim_cur: 0,
                    rlim_max: 0,
                };
                // SAFETY: `before` is writable and outlives the call.
                let answer = unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &raw mut before) };
                assert_eq!(answer, 0);
                Self { before }
            }

            fn set(&self, soft: u64) {
                let limit = libc::rlimit {
                    rlim_cur: soft,
                    rlim_max: self.before.rlim_max,
                };
                // SAFETY: `limit` is a valid value and outlives the call.
                let answer = unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &raw const limit) };
                assert_eq!(answer, 0, "this test needs a hard limit above {soft}");
            }
        }

        impl Drop for SoftLimit {
            fn drop(&mut self) {
                // SAFETY: the value this process had before.
                unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &raw const self.before) };
            }
        }

        /// An open descriptor at this number, without close-on-exec. Closed when dropped.
        fn open_at(file: &std::fs::File, number: i32) -> OwnedFd {
            // SAFETY: `file` is open, and `number` is free in this process of its own.
            let answer = unsafe { libc::dup2(file.as_raw_fd(), number) };
            assert_eq!(answer, number);
            // SAFETY: this process has just opened `number`, and nothing else owns it.
            let held = unsafe { OwnedFd::from_raw_fd(number) };
            // SAFETY: `number` is open; this only reads its flags.
            assert_eq!(unsafe { libc::fcntl(number, libc::F_GETFD) }, 0);
            held
        }

        const SECRET: &str = "dummy-inherited-secret";

        fn reads_descriptor(number: i32) -> String {
            format!("{{ read -r line <&{number}; }}\nprintf 'line=[%s]\\n' \"$line\"\nexit 0\n")
        }

        /// The witness: with no marking, a child of a process that lowered its limit still
        /// reads this number, so the limit is no bound on what is open.
        fn assert_it_would_reach_an_unmarked_child(number: i32) {
            use std::os::unix::process::CommandExt as _;
            let mut command = std::process::Command::new("/bin/bash");
            command.args(["-c", &reads_descriptor(number)]).env_clear();
            // SAFETY: the closure does nothing. It is here so that this forks as the launch does.
            unsafe { command.pre_exec(|| Ok(())) };
            let output = command.output().unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains(&format!("line=[{SECRET}]")), "{stdout}");
        }

        /// One descriptor at `number`, opened under a raised limit, and the limit then lowered.
        fn with_descriptor_above_a_lowered_limit(number: i32, check: fn(&Stage, i32)) {
            let stage = stage();
            let secret = stage.outside.join("secret");
            std::fs::write(&secret, format!("{SECRET}\n")).unwrap();
            let file = std::fs::File::open(&secret).unwrap();
            let limit = SoftLimit::hold();
            limit.set(u64::try_from(number).unwrap() + 100);
            let _held = open_at(&file, number);
            limit.set(256);
            assert_it_would_reach_an_unmarked_child(number);
            check(&stage, number);
        }

        #[test]
        fn a_listed_descriptor_above_a_lowered_limit_is_a_bad_descriptor() {
            in_own_process(
                "a_listed_descriptor_above_a_lowered_limit_is_a_bad_descriptor",
                || {
                    with_descriptor_above_a_lowered_limit(1000, |stage, number| {
                        let observation = run(stage, &reads_descriptor(number), &BTreeMap::new());
                        let stderr = String::from_utf8_lossy(&observation.stderr);
                        assert!(stderr.contains("Bad file descriptor"), "{stderr}");
                        let stdout = String::from_utf8_lossy(&observation.stdout);
                        assert!(stdout.contains("line=[]"), "{stdout}");
                    });
                },
            );
        }

        /// The review's case: nothing could be listed, and the limit would not have covered it.
        #[test]
        fn a_failed_list_with_a_descriptor_above_a_lowered_limit_starts_nothing() {
            in_own_process(
                "a_failed_list_with_a_descriptor_above_a_lowered_limit_starts_nothing",
                || {
                    with_descriptor_above_a_lowered_limit(1000, |stage, number| {
                        use super::super::test_hook::{DescriptorFault, descriptor_fault};
                        let _fault = descriptor_fault(DescriptorFault::ListFailed);
                        let error = attempt(stage, &reads_descriptor(number), &BTreeMap::new())
                            .unwrap_err();
                        assert!(is_descriptor_refusal(&error), "{error}");
                    });
                },
            );
        }

        /// A number past what the list is known to cover: gone at the exec, or no exec at all.
        /// Either answer keeps it from the tool.
        #[test]
        fn a_descriptor_past_the_list_does_not_reach_the_tool() {
            in_own_process("a_descriptor_past_the_list_does_not_reach_the_tool", || {
                with_descriptor_above_a_lowered_limit(5000, |stage, number| {
                    match attempt(stage, &reads_descriptor(number), &BTreeMap::new()) {
                        Err(error) => assert!(is_descriptor_refusal(&error), "{error}"),
                        Ok(observation) => {
                            let stdout = String::from_utf8_lossy(&observation.stdout);
                            assert!(stdout.contains("line=[]"), "{stdout}");
                        }
                    }
                });
            });
        }

        /// More than the fixed buffer holds, really open: nothing is started.
        #[test]
        fn more_descriptors_than_the_list_holds_start_nothing() {
            in_own_process("more_descriptors_than_the_list_holds_start_nothing", || {
                let stage = stage();
                let file = std::fs::File::open(stage.tool.parent().unwrap()).unwrap();
                let limit = SoftLimit::hold();
                limit.set(6000);
                let _held = (1000..5200)
                    .map(|number| open_at(&file, number))
                    .collect::<Vec<_>>();
                let error = attempt(&stage, "exit 0\n", &BTreeMap::new()).unwrap_err();
                assert!(is_descriptor_refusal(&error), "{error}");
            });
        }

        const DENIED: &str = "Operation not permitted";

        struct Stage {
            _temp: tempfile::TempDir,
            tool: PathBuf,
            outside: PathBuf,
        }

        fn stage() -> Stage {
            let temp = tempfile::tempdir().unwrap();
            let root = std::fs::canonicalize(temp.path()).unwrap();
            let tool = root.join("tool");
            let outside = root.join("outside");
            std::fs::create_dir_all(&tool).unwrap();
            std::fs::create_dir_all(&outside).unwrap();
            Stage {
                _temp: temp,
                tool,
                outside,
            }
        }

        fn run(
            stage: &Stage,
            script: &str,
            env: &BTreeMap<String, String>,
        ) -> process::Observation {
            attempt(stage, script, env).unwrap()
        }

        fn attempt(
            stage: &Stage,
            script: &str,
            env: &BTreeMap<String, String>,
        ) -> std::io::Result<process::Observation> {
            let path = stage.tool.join("attempt.sh");
            std::fs::write(&path, script).unwrap();
            let argv = [path.display().to_string()];
            let launch = StrictLaunch {
                executable: Path::new("/bin/bash"),
                argv: &argv,
                cwd: &stage.tool,
                env,
                read_directories: vec![stage.tool.as_path()],
                link_directories: Vec::new(),
            };
            process::run(
                strict_command(&launch).unwrap(),
                b"\n",
                RunLimits {
                    timeout: Duration::from_secs(10),
                    max_stream_bytes: 16 * 1024,
                },
                &|| false,
            )
        }

        #[test]
        fn a_write_outside_inside_and_to_an_existing_file_is_refused_by_the_os() {
            let stage = stage();
            let existing = stage.outside.join("existing");
            std::fs::write(&existing, "unchanged").unwrap();
            let fresh = stage.outside.join("fresh");
            let in_tool = stage.tool.join("written");
            let observation = run(
                &stage,
                &format!(
                    "{{ : > '{}'; }}\n{{ echo x >> '{}'; }}\n{{ : > '{}'; }}\nexit 0\n",
                    fresh.display(),
                    existing.display(),
                    in_tool.display()
                ),
                &BTreeMap::new(),
            );
            let stderr = String::from_utf8_lossy(&observation.stderr);
            assert_eq!(stderr.matches(DENIED).count(), 3, "{stderr}");
            assert!(!fresh.exists());
            assert!(!in_tool.exists());
            assert_eq!(std::fs::read_to_string(&existing).unwrap(), "unchanged");
        }

        #[test]
        fn a_secret_below_a_dummy_home_and_a_system_file_are_refused_by_the_os() {
            let stage = stage();
            let secret = stage.outside.join("home/.ssh/id_dummy");
            std::fs::create_dir_all(secret.parent().unwrap()).unwrap();
            std::fs::write(&secret, "dummy-secret").unwrap();
            let observation = run(
                &stage,
                &format!(
                    "{{ read -r line < '{}'; }}\nprintf 'read=[%s]\\n' \"$line\"\n{{ read -r other < /etc/hosts; }}\n[[ -e '{}' ]]\nprintf 'stat=%s\\n' \"$?\"\nexit 0\n",
                    secret.display(),
                    secret.display()
                ),
                &BTreeMap::new(),
            );
            let stderr = String::from_utf8_lossy(&observation.stderr);
            assert_eq!(stderr.matches(DENIED).count(), 2, "{stderr}");
            let stdout = String::from_utf8_lossy(&observation.stdout);
            assert!(stdout.contains("read=[]"), "{stdout}");
            assert!(stdout.contains("stat=1"), "existence is hidden: {stdout}");
        }

        #[test]
        fn tcp_udp_and_loopback_are_refused_by_the_os_with_a_real_listener_waiting() {
            let stage = stage();
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let port = listener.local_addr().unwrap().port();
            let observation = run(
                &stage,
                &format!(
                    "{{ : > /dev/tcp/127.0.0.1/{port}; }}\n{{ : > /dev/udp/127.0.0.1/{port}; }}\n{{ : > /dev/tcp/192.0.2.1/80; }}\nexit 0\n"
                ),
                &BTreeMap::new(),
            );
            let stderr = String::from_utf8_lossy(&observation.stderr);
            assert_eq!(
                stderr.matches("connect: Operation not permitted").count(),
                3,
                "{stderr}"
            );
            assert!(
                listener.accept().is_err(),
                "the connection reached the listener"
            );
        }

        /// The errno itself, under the real policy: a descriptor the caller left open is
        /// gone at the exec, so the child's own shell reports it as bad.
        #[test]
        fn a_descriptor_the_caller_left_open_is_a_bad_descriptor_under_the_policy() {
            let stage = stage();
            let (written, secret) = (stage.outside.join("written"), stage.outside.join("secret"));
            std::fs::write(&written, "").unwrap();
            std::fs::write(&secret, "dummy-inherited-secret\n").unwrap();
            let (_writer, writer_fd) = super::inheritable(
                std::fs::OpenOptions::new()
                    .append(true)
                    .open(&written)
                    .unwrap(),
            );
            let (_reader, reader_fd) = super::inheritable(std::fs::File::open(&secret).unwrap());
            let observation = run(
                &stage,
                &format!(
                    "{{ echo x >&{writer_fd}; }}\n{{ read -r line <&{reader_fd}; }}\nprintf 'line=[%s]\\n' \"$line\"\nexit 0\n"
                ),
                &BTreeMap::new(),
            );
            let stderr = String::from_utf8_lossy(&observation.stderr);
            assert_eq!(stderr.matches("Bad file descriptor").count(), 2, "{stderr}");
            let stdout = String::from_utf8_lossy(&observation.stdout);
            assert!(stdout.contains("line=[]"), "{stdout}");
            assert_eq!(std::fs::read_to_string(&written).unwrap(), "");
        }

        #[test]
        fn a_fork_is_refused_by_the_os_so_no_descendant_can_exist() {
            let stage = stage();
            let observation = run(&stage, "( : )\nexit 0\n", &BTreeMap::new());
            let stderr = String::from_utf8_lossy(&observation.stderr);
            assert!(stderr.contains("fork: Operation not permitted"), "{stderr}");
            assert_eq!(observation.descendants_left, 0);
        }

        #[test]
        fn only_the_declared_keys_reach_the_child() {
            // SAFETY: a unit test of this module; the name is used by nothing else.
            unsafe { std::env::set_var("G3_PARENT_DUMMY_SECRET", "dummy-parent-secret") };
            let stage = stage();
            let env = BTreeMap::from([("TOOL_MODE".to_string(), "strict".to_string())]);
            let observation = run(&stage, "export -p\nexit 0\n", &env);
            // SAFETY: as above.
            unsafe { std::env::remove_var("G3_PARENT_DUMMY_SECRET") };
            let stdout = String::from_utf8_lossy(&observation.stdout);
            assert!(stdout.contains("TOOL_MODE=\"strict\""), "{stdout}");
            assert!(!stdout.contains("G3_PARENT_DUMMY_SECRET"), "{stdout}");
            assert!(!stdout.contains("dummy-parent-secret"), "{stdout}");
            for line in stdout.lines() {
                let name = line
                    .trim_start_matches("declare -x ")
                    .split('=')
                    .next()
                    .unwrap();
                // Bash marks these three itself; `__CF_USER_TEXT_ENCODING` comes from the OS.
                assert!(
                    [
                        "TOOL_MODE",
                        "PWD",
                        "OLDPWD",
                        "SHLVL",
                        "__CF_USER_TEXT_ENCODING"
                    ]
                    .contains(&name),
                    "{line}"
                );
            }
        }
    }
}
