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

/// Between fork and exec, only `fcntl` and the descriptor list. A descriptor the host's own
/// caller left open would otherwise reach the child: the policy judges an open, not a read
/// or a write on what is already open. Marking instead of closing leaves the runtime's own
/// descriptors for the moment of the exec. Failure on an unused number is ignored.
#[cfg(unix)]
fn mark_inherited_descriptors_close_on_exec() {
    fn mark(descriptor: i32) {
        // SAFETY: `fcntl` is async-signal-safe, and this only sets a flag on a number; an
        // unused one returns an error that does not matter.
        unsafe {
            libc::fcntl(descriptor, libc::F_SETFD, libc::FD_CLOEXEC);
        }
    }
    #[cfg(target_os = "macos")]
    {
        const CAPACITY: usize = 4096;
        // SAFETY: an all-zero `proc_fdinfo` is a valid value of this plain struct.
        let mut listed: [libc::proc_fdinfo; CAPACITY] = unsafe { std::mem::zeroed() };
        let entry = std::mem::size_of::<libc::proc_fdinfo>();
        // SAFETY: the buffer is writable for this many bytes on this stack frame, and
        // `getpid` has no preconditions. Both are plain system calls.
        let written = unsafe {
            libc::proc_pidinfo(
                libc::getpid(),
                libc::PROC_PIDLISTFDS,
                0,
                listed.as_mut_ptr().cast(),
                (CAPACITY * entry) as i32,
            )
        };
        let count = usize::try_from(written).unwrap_or(0) / entry;
        if count > 0 && count < CAPACITY {
            for info in &listed[..count] {
                if info.proc_fd > 2 {
                    mark(info.proc_fd);
                }
            }
            return;
        }
    }
    // SAFETY: `getdtablesize` takes no argument and only reads a limit.
    let limit = unsafe { libc::getdtablesize() };
    for descriptor in 3..limit {
        mark(descriptor);
    }
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
        // SAFETY: the closure makes only async-signal-safe calls, on the stack, and takes no
        // lock and allocates nothing.
        unsafe {
            command.pre_exec(|| {
                mark_inherited_descriptors_close_on_exec();
                Ok(())
            });
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

    #[cfg(target_os = "macos")]
    mod real_isolation {
        use std::collections::BTreeMap;
        use std::path::{Path, PathBuf};
        use std::time::Duration;

        use super::super::super::process::{self, RunLimits};
        use super::super::{StrictLaunch, strict_command};

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
            .unwrap()
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
