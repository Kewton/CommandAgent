//! The bounded, environment-cleared adapter process.
//!
//! This deliberately does not use `verifier_env` or
//! `bounded_process::spawn_child`: both put parts of the parent environment
//! (PATH, HOME, workspace `node_modules/.bin`, ...) back into the child. The
//! adapter child starts from an empty environment that holds only `LANG`, `TZ`,
//! and the operator-resolved tooling root. The executable is the
//! operator-resolved absolute path (never a PATH lookup) and the working
//! directory is the trusted tooling root. Both streams are bounded while they
//! are read; on overflow, timeout, or cancel the whole process group is stopped
//! and the leader is reaped.

use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Fixed first-version wall-clock ceiling for one adapter run.
pub(crate) const FIXED_TIMEOUT: Duration = Duration::from_secs(30);
/// The child environment variable that names the trusted tooling root.
pub(crate) const TOOL_ROOT_ENV: &str = "COMMANDAGENT_ADAPTER_TOOL_ROOT";
/// Keys the host passes to the child. Nothing else is inherited.
pub(crate) const CHILD_ENV_KEYS: [&str; 3] = ["LANG", "TZ", TOOL_ROOT_ENV];
/// Keys the operating system or its runtime may add on its own after `exec`.
/// They are recorded separately and never come from the host.
#[cfg(target_os = "macos")]
pub(crate) const KNOWN_PLATFORM_ENV_KEYS: &[&str] = &["__CF_USER_TEXT_ENCODING"];
#[cfg(not(target_os = "macos"))]
pub(crate) const KNOWN_PLATFORM_ENV_KEYS: &[&str] = &[];

const POLL_INTERVAL: Duration = Duration::from_millis(10);
const DRAIN_GRACE: Duration = Duration::from_millis(500);
const READ_CHUNK: usize = 8 * 1024;

#[derive(Debug, Clone)]
pub(crate) struct Launch {
    pub(crate) executable: PathBuf,
    pub(crate) args: Vec<OsString>,
    pub(crate) cwd: PathBuf,
    pub(crate) tool_root: PathBuf,
    pub(crate) stdin: Vec<u8>,
    pub(crate) timeout: Duration,
    pub(crate) max_stream_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ended {
    /// The leader exited; `None` means it was ended by a signal.
    Exited(Option<i32>),
    TimedOut,
    Cancelled,
    OutputExceeded,
    /// The leader exited but a descendant kept an output stream open.
    DescendantHeldOutput,
}

impl Ended {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Exited(Some(_)) => "exited",
            Self::Exited(None) => "signaled",
            Self::TimedOut => "timed_out",
            Self::Cancelled => "cancelled",
            Self::OutputExceeded => "output_exceeded",
            Self::DescendantHeldOutput => "descendant_held_output",
        }
    }
}

#[derive(Debug)]
pub(crate) struct Observation {
    pub(crate) ended: Ended,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
    pub(crate) elapsed: Duration,
    pub(crate) reaped: bool,
    pub(crate) group_stopped: bool,
    /// Whether the whole request reached the child before the host closed
    /// stdin. The host always closes its stdin end before `run` returns.
    pub(crate) stdin_complete: bool,
}

/// The complete child environment: `LANG`, `TZ`, and the tooling root.
pub(crate) fn child_env(tool_root: &Path) -> [(&'static str, OsString); 3] {
    [
        (CHILD_ENV_KEYS[0], OsString::from("C")),
        (CHILD_ENV_KEYS[1], OsString::from("UTC")),
        (CHILD_ENV_KEYS[2], tool_root.as_os_str().to_os_string()),
    ]
}

fn command(launch: &Launch) -> Command {
    let mut command = Command::new(&launch.executable);
    command
        .args(&launch.args)
        .current_dir(&launch.cwd)
        .env_clear()
        .envs(child_env(&launch.tool_root))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // A new process group whose id equals the child pid, so the whole
        // group can be stopped with the existing bounded_process helpers.
        command.process_group(0);
    }
    command
}

/// Run one adapter. `Err` means the process could not be spawned.
pub(crate) fn run<F>(launch: &Launch, is_cancelled: F) -> io::Result<Observation>
where
    F: Fn() -> bool,
{
    let started = Instant::now();
    let mut child = command(launch).spawn()?;
    let pid = child.id();
    // The request is written by this loop without blocking, under the same
    // deadline, so a child or descendant that holds a full stdin pipe never
    // holds the host; the host closes its end on every path.
    let mut stdin = child
        .stdin
        .take()
        .filter(|pipe| set_nonblocking(pipe).is_ok());
    let mut written = 0usize;
    let overflow = Arc::new(AtomicBool::new(false));
    let stdout = child
        .stdout
        .take()
        .map(|pipe| read_bounded(pipe, launch.max_stream_bytes, Arc::clone(&overflow)));
    let stderr = child
        .stderr
        .take()
        .map(|pipe| read_bounded(pipe, launch.max_stream_bytes, Arc::clone(&overflow)));
    let deadline = started + launch.timeout;

    let mut stop = None;
    let mut exited = None;
    while stop.is_none() && exited.is_none() {
        written += feed_stdin(&mut stdin, &launch.stdin[written..]);
        if written == launch.stdin.len() {
            stdin = None;
        }
        if is_cancelled() {
            stop = Some(Ended::Cancelled);
        } else if overflow.load(Ordering::SeqCst) {
            stop = Some(Ended::OutputExceeded);
        } else if Instant::now() >= deadline {
            stop = Some(Ended::TimedOut);
        } else {
            match child.try_wait() {
                Ok(Some(status)) => exited = Some(status.code()),
                Ok(None) => thread::sleep(POLL_INTERVAL),
                Err(error) => {
                    crate::bounded_process::terminate_process_group(&mut child);
                    let _ = child.wait();
                    return Err(error);
                }
            }
        }
    }

    let stdin_complete = written == launch.stdin.len();
    drop(stdin.take());
    let (mut ended, reaped) = match (stop, exited) {
        (Some(stopped), _) => {
            crate::bounded_process::terminate_process_group(&mut child);
            (stopped, child.wait().is_ok())
        }
        (None, code) => (Ended::Exited(code.flatten()), true),
    };
    // The group is stopped on every path, so no descendant outlives the run.
    let group_stopped = crate::bounded_process::terminate_process_group_by_pid(pid).is_ok();
    let drain_deadline = Instant::now() + DRAIN_GRACE;
    while !readers_finished(&stdout, &stderr) && Instant::now() < drain_deadline {
        thread::sleep(POLL_INTERVAL);
    }
    if !readers_finished(&stdout, &stderr) {
        // Only a descendant that left the group can still hold a stream. Its
        // output is discarded and the run fails; the reader is not joined.
        return Ok(Observation {
            ended: Ended::DescendantHeldOutput,
            stdout: Vec::new(),
            stderr: Vec::new(),
            elapsed: started.elapsed(),
            reaped,
            group_stopped,
            stdin_complete,
        });
    }
    let (stdout, stdout_over) = join_reader(stdout);
    let (stderr, stderr_over) = join_reader(stderr);
    if matches!(ended, Ended::Exited(_)) && (stdout_over || stderr_over) {
        ended = Ended::OutputExceeded;
    }
    Ok(Observation {
        ended,
        stdout,
        stderr,
        elapsed: started.elapsed(),
        reaped,
        group_stopped,
        stdin_complete,
    })
}

/// Write what the pipe accepts now. A closed or failed pipe is dropped, so the
/// child's own close ends the feeding instead of an error.
fn feed_stdin(stdin: &mut Option<std::process::ChildStdin>, pending: &[u8]) -> usize {
    let Some(pipe) = stdin.as_mut() else {
        return 0;
    };
    if pending.is_empty() {
        return 0;
    }
    match pipe.write(pending) {
        Ok(written) => written,
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => 0,
        Err(_) => {
            *stdin = None;
            0
        }
    }
}

#[cfg(unix)]
fn set_nonblocking(pipe: &std::process::ChildStdin) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    let fd = pipe.as_raw_fd();
    // SAFETY: `fd` is the open stdin pipe owned by `pipe` for this call;
    // F_GETFL only reads that descriptor's status flags.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the same owned descriptor; F_SETFL only adds O_NONBLOCK.
    if unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(unix))]
fn set_nonblocking(_pipe: &std::process::ChildStdin) -> io::Result<()> {
    Err(io::Error::other(
        "non-blocking stdin is only supported on Unix",
    ))
}

type Reader = Option<JoinHandle<(Vec<u8>, bool)>>;

fn read_bounded<R>(
    mut pipe: R,
    limit: usize,
    overflow: Arc<AtomicBool>,
) -> JoinHandle<(Vec<u8>, bool)>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut kept = Vec::new();
        let mut chunk = vec![0u8; READ_CHUNK];
        loop {
            match pipe.read(&mut chunk) {
                Ok(0) | Err(_) => return (kept, false),
                Ok(read) => {
                    if kept.len() + read > limit {
                        let room = limit - kept.len();
                        kept.extend_from_slice(&chunk[..room]);
                        overflow.store(true, Ordering::SeqCst);
                        return (kept, true);
                    }
                    kept.extend_from_slice(&chunk[..read]);
                }
            }
        }
    })
}

fn readers_finished(stdout: &Reader, stderr: &Reader) -> bool {
    [stdout, stderr]
        .into_iter()
        .all(|reader| reader.as_ref().is_none_or(JoinHandle::is_finished))
}

fn join_reader(reader: Reader) -> (Vec<u8>, bool) {
    reader
        .and_then(|handle| handle.join().ok())
        .unwrap_or_default()
}
