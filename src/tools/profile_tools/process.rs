//! One bounded process run for a `propose-artifacts` tool (Issue #974, G3).
//!
//! The shape follows `artifact_contract_checks::process`, which stays as it is. Three
//! things are stricter here, and the reason this is its own leaf:
//!
//! - stdout and stderr share one budget, because the limit is on the whole result;
//! - a read error is a failure, never an end of file;
//! - stopping the group is not taken to mean every descendant is gone. A descendant that
//!   left the group is tracked by its parent chain and its group is stopped through the
//!   existing guarded helper, and a run that left any descendant alive is not accepted.
//!
//! Under the strict policy a fork is refused by the operating system, so production has no
//! descendant to find; this is what holds if that ever stops being true.

use std::io::{self, Read, Write};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(10);
const DRAIN_GRACE: Duration = Duration::from_millis(500);
const REAP_GRACE: Duration = Duration::from_secs(2);
const READ_CHUNK: usize = 8 * 1024;
/// Scan every tick at first, when a short-lived parent can still be seen, then less often.
const FAST_SCAN: Duration = Duration::from_millis(500);
const SLOW_SCAN_TICKS: u32 = 10;

#[derive(Debug, Clone, Copy)]
pub(super) struct RunLimits {
    pub(super) timeout: Duration,
    /// Stdout and stderr together.
    pub(super) max_stream_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Ended {
    Exited(Option<i32>),
    TimedOut,
    Cancelled,
    OutputExceeded,
    HeldOutput,
}

#[derive(Debug)]
pub(super) struct Observation {
    pub(super) ended: Ended,
    pub(super) stdout: Vec<u8>,
    pub(super) stderr: Vec<u8>,
    pub(super) reaped: bool,
    /// The whole request reached the child before the host closed stdin.
    pub(super) stdin_complete: bool,
    pub(super) read_failed: bool,
    /// Processes below the leader still alive when it ended, including one that left the
    /// group. A child that had already ended by itself is not counted.
    pub(super) descendants_left: usize,
    /// How many could not be shown gone, with any member left in the group.
    pub(super) descendants_remaining: usize,
}

impl Observation {
    /// `None` only for a process that exited 0 on its own, took the whole request, and
    /// left nothing behind. This says the run ended cleanly and nothing about its result.
    pub(super) fn failure_code(&self) -> Option<&'static str> {
        match self.ended {
            Ended::Cancelled => return Some("cancelled"),
            Ended::TimedOut => return Some("timed_out"),
            Ended::OutputExceeded => return Some("output_exceeded"),
            Ended::HeldOutput => return Some("descendant_held_output"),
            Ended::Exited(Some(0)) => {}
            Ended::Exited(Some(_)) => return Some("exit_nonzero"),
            Ended::Exited(None) => return Some("signaled"),
        }
        if self.read_failed {
            Some("stream_read_failed")
        } else if !self.reaped {
            Some("not_reaped")
        } else if self.descendants_remaining > 0 {
            Some("descendant_not_reaped")
        } else if self.descendants_left > 0 {
            Some("descendant_left_behind")
        } else if !self.stdin_complete {
            Some("stdin_incomplete")
        } else {
            None
        }
    }
}

#[derive(Debug, Default)]
struct StreamBudget {
    total: AtomicUsize,
    overflow: AtomicBool,
    read_failed: AtomicBool,
}

/// Run a command the isolation leaf built. `Err` means it could not be spawned.
#[cfg(unix)]
pub(super) fn run(
    mut command: Command,
    stdin: &[u8],
    limits: RunLimits,
    is_cancelled: &dyn Fn() -> bool,
) -> io::Result<Observation> {
    let started = Instant::now();
    let mut child = command.spawn()?;
    let pid = child.id();
    let mut pipe = child
        .stdin
        .take()
        .filter(|pipe| set_nonblocking(pipe).is_ok());
    let budget = Arc::new(StreamBudget::default());
    let stdout = child
        .stdout
        .take()
        .map(|stream| read_stream(stream, limits.max_stream_bytes, budget.clone()));
    let stderr = child
        .stderr
        .take()
        .map(|stream| read_stream(stream, limits.max_stream_bytes, budget.clone()));
    let deadline = started + limits.timeout;
    let mut tracker = lineage::Tracker::new(pid);
    let mut written = 0usize;
    let mut stop = None;
    let mut exited = None;
    let mut tick = 0u32;
    while stop.is_none() && exited.is_none() {
        written += feed_stdin(&mut pipe, &stdin[written..]);
        if written == stdin.len() {
            pipe = None;
        }
        if tick.is_multiple_of(SLOW_SCAN_TICKS) || started.elapsed() < FAST_SCAN {
            tracker.scan();
        }
        tick = tick.wrapping_add(1);
        if is_cancelled() {
            stop = Some(Ended::Cancelled);
        } else if budget.overflow.load(Ordering::SeqCst) {
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
                    tracker.stop_all();
                    return Err(error);
                }
            }
        }
    }
    let stdin_complete = written == stdin.len();
    drop(pipe.take());
    let (mut ended, reaped) = match (stop, exited) {
        (Some(stopped), _) => {
            crate::bounded_process::terminate_process_group(&mut child);
            (stopped, child.wait().is_ok())
        }
        (None, code) => (Ended::Exited(code.flatten()), true),
    };
    let (seen, remaining) = tracker.stop_all();
    let drain_deadline = Instant::now() + DRAIN_GRACE;
    while !readers_finished(&stdout, &stderr) && Instant::now() < drain_deadline {
        thread::sleep(POLL_INTERVAL);
    }
    let read_failed = budget.read_failed.load(Ordering::SeqCst);
    if !readers_finished(&stdout, &stderr) {
        // Something unseen still holds a stream. Its output is discarded and the reader is
        // left to end at its own end of file.
        return Ok(Observation {
            ended: Ended::HeldOutput,
            stdout: Vec::new(),
            stderr: Vec::new(),
            reaped,
            stdin_complete,
            read_failed,
            descendants_left: seen,
            descendants_remaining: remaining,
        });
    }
    let stdout = join_reader(stdout, &budget);
    let stderr = join_reader(stderr, &budget);
    if matches!(ended, Ended::Exited(_)) && budget.overflow.load(Ordering::SeqCst) {
        ended = Ended::OutputExceeded;
    }
    Ok(Observation {
        ended,
        stdout,
        stderr,
        reaped,
        stdin_complete,
        read_failed: budget.read_failed.load(Ordering::SeqCst),
        descendants_left: seen,
        descendants_remaining: remaining,
    })
}

#[cfg(not(unix))]
pub(super) fn run(
    _command: Command,
    _stdin: &[u8],
    _limits: RunLimits,
    _is_cancelled: &dyn Fn() -> bool,
) -> io::Result<Observation> {
    Err(io::Error::other(
        "no strict process backend on this platform",
    ))
}

/// Write what the pipe takes now, so a child that does not read never holds the host.
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
    // SAFETY: `fd` is the open pipe `pipe` owns for this call; this only reads its flags.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the same descriptor; this only adds O_NONBLOCK.
    if unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

type Reader = Option<JoinHandle<Vec<u8>>>;

/// Keeps within the shared budget. A read error is recorded, because a stream that broke
/// is not a stream that ended.
fn read_stream<R>(mut pipe: R, limit: usize, budget: Arc<StreamBudget>) -> JoinHandle<Vec<u8>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut kept = Vec::new();
        let mut chunk = vec![0u8; READ_CHUNK];
        loop {
            match pipe.read(&mut chunk) {
                Ok(0) => return kept,
                Ok(read) => {
                    let total = budget.total.fetch_add(read, Ordering::SeqCst) + read;
                    if total > limit {
                        budget.overflow.store(true, Ordering::SeqCst);
                        return kept;
                    }
                    kept.extend_from_slice(&chunk[..read]);
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => {
                    budget.read_failed.store(true, Ordering::SeqCst);
                    return kept;
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

/// A reader that panicked lost its bytes, so it counts as a failed read.
fn join_reader(reader: Reader, budget: &StreamBudget) -> Vec<u8> {
    let Some(handle) = reader else {
        budget.read_failed.store(true, Ordering::SeqCst);
        return Vec::new();
    };
    handle.join().unwrap_or_else(|_| {
        budget.read_failed.store(true, Ordering::SeqCst);
        Vec::new()
    })
}

/// Who is below the leader. A process that called `setsid` keeps its parent until that
/// parent exits, so the chain is read while the run lasts.
mod lineage {
    use std::collections::BTreeMap;
    use std::time::{Duration, Instant};

    use super::REAP_GRACE;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(super) struct Entry {
        pub(super) pid: i32,
        pub(super) parent: i32,
        pub(super) group: i32,
        pub(super) started: u64,
    }

    #[derive(Debug)]
    pub(super) struct Tracker {
        leader: i32,
        /// By pid, with the start time that tells it from a later process with the same pid.
        tracked: BTreeMap<i32, u64>,
        table_failed: bool,
    }

    impl Tracker {
        pub(super) fn new(pid: u32) -> Self {
            Self {
                // No process has -1 as a parent or a group, so nothing would match.
                leader: i32::try_from(pid).unwrap_or(-1),
                tracked: BTreeMap::new(),
                table_failed: false,
            }
        }

        pub(super) fn scan(&mut self) -> Vec<Entry> {
            let Some(table) = process_table() else {
                self.table_failed = true;
                return Vec::new();
            };
            loop {
                let before = self.tracked.len();
                for entry in &table {
                    let parent_is_ours =
                        entry.parent == self.leader || self.tracked.contains_key(&entry.parent);
                    let in_group = entry.group == self.leader && entry.pid != self.leader;
                    if parent_is_ours && entry.pid != self.leader || in_group {
                        self.tracked.entry(entry.pid).or_insert(entry.started);
                    }
                }
                if self.tracked.len() == before {
                    return table;
                }
            }
        }

        /// Stops everything tracked that is still alive, by its identity, and returns how
        /// many the leader left behind and how many are not shown gone. A child that ended
        /// by itself is neither. A table that cannot be read shows nothing gone.
        pub(super) fn stop_all(&mut self) -> (usize, usize) {
            let deadline = Instant::now() + REAP_GRACE;
            let mut left = None;
            let mut signaled = std::collections::BTreeSet::new();
            loop {
                let table = self.scan();
                let alive = table
                    .iter()
                    .filter(|entry| self.tracked.get(&entry.pid) == Some(&entry.started))
                    .copied()
                    .collect::<Vec<_>>();
                let left = *left.get_or_insert(alive.len());
                for entry in &alive {
                    // Only a group this run made: the leader's, or one a tracked descendant
                    // leads. A descendant can also join a group that is not its own, the
                    // host's included, and that one stays alive and refuses the run.
                    let ours =
                        entry.group == self.leader || self.tracked.contains_key(&entry.group);
                    if ours && signaled.insert(entry.group) {
                        let _ = crate::bounded_process::terminate_process_group_by_pid(
                            entry.group as u32,
                        );
                    }
                }
                if self.table_failed {
                    return (left.max(1), 1);
                }
                if alive.is_empty() {
                    return (left, 0);
                }
                if Instant::now() >= deadline {
                    return (left, alive.len());
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }

    /// Every process that is not a zombie: a zombie has ended and only waits for a parent.
    #[cfg(target_os = "macos")]
    fn process_table() -> Option<Vec<Entry>> {
        use std::mem::size_of;
        // SAFETY: a null buffer asks only for the count.
        let count = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
        let capacity = usize::try_from(count).ok()? + 64;
        let mut pids = vec![0i32; capacity];
        let bytes = i32::try_from(capacity * size_of::<i32>()).ok()?;
        // SAFETY: the buffer holds `bytes` bytes and outlives the call.
        let listed = unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast(), bytes) };
        let listed = usize::try_from(listed).ok()?;
        const ZOMBIE: u32 = 5;
        let mut table = Vec::with_capacity(listed);
        for pid in pids.iter().take(listed).copied().filter(|pid| *pid > 0) {
            // SAFETY: an all-zero `proc_bsdinfo` is a valid value of this plain struct.
            let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
            let size = i32::try_from(size_of::<libc::proc_bsdinfo>()).ok()?;
            // SAFETY: `info` is writable for `size` bytes and outlives the call.
            let read = unsafe {
                libc::proc_pidinfo(pid, libc::PROC_PIDTBSDINFO, 0, (&raw mut info).cast(), size)
            };
            if read == size && info.pbi_status != ZOMBIE {
                table.push(Entry {
                    pid,
                    parent: i32::try_from(info.pbi_ppid).ok()?,
                    group: i32::try_from(info.pbi_pgid).ok()?,
                    started: info.pbi_start_tvsec * 1_000_000 + info.pbi_start_tvusec,
                });
            }
        }
        Some(table)
    }

    #[cfg(target_os = "linux")]
    fn process_table() -> Option<Vec<Entry>> {
        let mut table = Vec::new();
        for entry in std::fs::read_dir("/proc").ok()?.flatten() {
            let Some(pid) = entry
                .file_name()
                .to_str()
                .and_then(|name| name.parse::<i32>().ok())
            else {
                continue;
            };
            let Ok(stat) = std::fs::read_to_string(entry.path().join("stat")) else {
                continue;
            };
            if let Some(entry) = parse_stat(pid, &stat) {
                table.push(entry);
            }
        }
        Some(table)
    }

    /// After the command name in parentheses: state, ppid, pgrp, ..., starttime as the
    /// twentieth of the fields that follow. A zombie has ended.
    #[cfg(target_os = "linux")]
    fn parse_stat(pid: i32, stat: &str) -> Option<Entry> {
        let rest = &stat[stat.rfind(')')? + 1..];
        let fields = rest.split_whitespace().collect::<Vec<_>>();
        if *fields.first()? == "Z" {
            return None;
        }
        Some(Entry {
            pid,
            parent: fields.get(1)?.parse().ok()?,
            group: fields.get(2)?.parse().ok()?,
            started: fields.get(19)?.parse().ok()?,
        })
    }

    /// Nothing here can read a process table, so nothing is shown gone. No launch reaches
    /// this: the isolation leaf has no backend on these platforms.
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    fn process_table() -> Option<Vec<Entry>> {
        None
    }

    #[cfg(test)]
    pub(super) fn listed_alive(needle_pid: i32) -> bool {
        process_table()
            .unwrap_or_default()
            .iter()
            .any(|entry| entry.pid == needle_pid)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    use super::lineage::listed_alive;
    use super::{Ended, Observation, RunLimits, run};

    fn shell(script: &str) -> Command {
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", script])
            .env_clear()
            .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt as _;
            command.process_group(0);
        }
        command
    }

    fn limits(ms: u64, bytes: usize) -> RunLimits {
        RunLimits {
            timeout: Duration::from_millis(ms),
            max_stream_bytes: bytes,
        }
    }

    fn observe(script: &str, stdin: &[u8], limit: RunLimits) -> Observation {
        run(shell(script), stdin, limit, &|| false).unwrap()
    }

    #[test]
    fn a_clean_exit_is_read_whole_and_says_nothing_about_the_result() {
        let observation = observe("cat; printf E >&2", b"payload", limits(10_000, 1024));
        assert_eq!(observation.ended, Ended::Exited(Some(0)));
        assert_eq!(observation.stdout, b"payload");
        assert_eq!(observation.stderr, b"E");
        assert_eq!(observation.failure_code(), None);
    }

    #[test]
    fn a_nonzero_exit_and_a_signal_are_failures() {
        let nonzero = observe("cat >/dev/null; exit 3", b"", limits(10_000, 1024));
        assert_eq!(nonzero.failure_code(), Some("exit_nonzero"));
        let signaled = observe("kill -9 $$", b"", limits(10_000, 1024));
        assert_eq!(signaled.failure_code(), Some("signaled"));
    }

    #[test]
    fn the_timeout_stops_and_reaps_the_leader() {
        let started = Instant::now();
        let observation = observe("exec sleep 30", b"", limits(100, 1024));
        assert_eq!(observation.ended, Ended::TimedOut);
        assert!(observation.reaped);
        assert_eq!(observation.failure_code(), Some("timed_out"));
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[test]
    fn a_cancel_stops_and_reaps_the_leader() {
        let started = Instant::now();
        let observation = run(shell("exec sleep 30"), b"", limits(30_000, 1024), &|| {
            started.elapsed() > Duration::from_millis(50)
        })
        .unwrap();
        assert_eq!(observation.ended, Ended::Cancelled);
        assert!(observation.reaped);
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[test]
    fn a_child_that_never_reads_stdin_does_not_hold_the_host() {
        let large = vec![b'x'; 1 << 20];
        let started = Instant::now();
        let observation = observe("exec sleep 30", &large, limits(200, 1024));
        assert_eq!(observation.ended, Ended::TimedOut);
        assert!(!observation.stdin_complete);
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[test]
    fn a_child_that_exits_0_without_taking_the_request_is_refused() {
        let large = vec![b'x'; 1 << 20];
        let observation = observe("exit 0", &large, limits(10_000, 1024));
        assert_eq!(observation.ended, Ended::Exited(Some(0)));
        assert_eq!(observation.failure_code(), Some("stdin_incomplete"));
    }

    #[test]
    fn stdout_stderr_and_their_total_are_one_budget_at_its_edge() {
        let at_limit = observe("printf 12345; printf 12345 >&2", b"", limits(10_000, 10));
        assert_eq!(at_limit.failure_code(), None);
        assert_eq!(at_limit.stdout.len() + at_limit.stderr.len(), 10);
        let over_total = observe("printf 12345; printf 123456 >&2", b"", limits(10_000, 10));
        assert_eq!(over_total.failure_code(), Some("output_exceeded"));
        let over_stdout = observe("printf 12345678901", b"", limits(10_000, 10));
        assert_eq!(over_stdout.failure_code(), Some("output_exceeded"));
        let over_stderr = observe("printf 12345678901 >&2", b"", limits(10_000, 10));
        assert_eq!(over_stderr.failure_code(), Some("output_exceeded"));
    }

    #[test]
    fn an_endless_writer_is_stopped_with_bounded_memory() {
        let observation = observe("yes", b"", limits(10_000, 1024));
        assert_eq!(observation.ended, Ended::OutputExceeded);
        assert!(observation.stdout.len() <= 1024);
    }

    fn pid_from(stdout: &[u8]) -> i32 {
        String::from_utf8_lossy(stdout)
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .parse()
            .unwrap_or_else(|_| panic!("no pid in {:?}", String::from_utf8_lossy(stdout)))
    }

    fn assert_gone(pid: i32) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while listed_alive(pid) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!listed_alive(pid), "descendant {pid} is still alive");
    }

    #[test]
    fn a_same_group_descendant_is_stopped_and_refused() {
        let observation = observe(
            "sleep 30 >/dev/null 2>&1 </dev/null & echo $!; sleep 0.3; exit 0",
            b"",
            limits(10_000, 1024),
        );
        assert_eq!(observation.ended, Ended::Exited(Some(0)));
        assert_eq!(observation.failure_code(), Some("descendant_left_behind"));
        assert_gone(pid_from(&observation.stdout));
    }

    /// A descendant in its own session and group, with no stream open. Stopping the group
    /// does not reach it. This is the case that "the group stopped" would hide.
    fn escaped(stdout: &str) -> String {
        format!(
            "perl -e 'use POSIX; POSIX::setsid(); open(STDIN,\"</dev/null\"); open(STDOUT,\">/dev/null\"); open(STDERR,\">/dev/null\"); exec(\"sleep\",\"30\")' & {stdout}"
        )
    }

    #[test]
    fn a_descendant_that_left_the_group_is_stopped_by_its_identity_and_refused() {
        let observation = observe(
            &escaped("echo $!; sleep 0.3; exit 0"),
            b"",
            limits(10_000, 1024),
        );
        assert_eq!(observation.failure_code(), Some("descendant_left_behind"));
        assert!(observation.descendants_left >= 1);
        assert_eq!(observation.descendants_remaining, 0);
        assert_gone(pid_from(&observation.stdout));
    }

    #[test]
    fn a_timeout_also_reaches_a_descendant_that_left_the_group() {
        let observation = observe(&escaped("echo $!; exec sleep 30"), b"", limits(300, 1024));
        assert_eq!(observation.ended, Ended::TimedOut);
        assert_eq!(observation.descendants_remaining, 0);
        assert_gone(pid_from(&observation.stdout));
    }

    #[test]
    fn a_descendant_holding_a_stream_after_the_leader_left_cannot_pass() {
        let observation = observe(
            "perl -e 'use POSIX; POSIX::setsid(); sleep 30' & sleep 0.3; exit 0",
            b"",
            limits(10_000, 1024),
        );
        assert!(observation.failure_code().is_some());
        assert!(observation.descendants_left >= 1);
    }

    #[test]
    fn a_read_error_is_a_failure_and_not_an_end_of_file() {
        struct Broken;
        impl std::io::Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("broken"))
            }
        }
        let budget = std::sync::Arc::new(super::StreamBudget::default());
        let kept = super::read_stream(Broken, 16, budget.clone())
            .join()
            .unwrap();
        assert!(kept.is_empty());
        assert!(budget.read_failed.load(std::sync::atomic::Ordering::SeqCst));
        let observation = Observation {
            ended: Ended::Exited(Some(0)),
            stdout: Vec::new(),
            stderr: Vec::new(),
            reaped: true,
            stdin_complete: true,
            read_failed: true,
            descendants_left: 0,
            descendants_remaining: 0,
        };
        assert_eq!(observation.failure_code(), Some("stream_read_failed"));
    }

    #[test]
    fn a_missing_executable_is_a_spawn_error() {
        let mut command = Command::new(Path::new("/nonexistent/profile-tool"));
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        assert!(run(command, b"", limits(100, 16), &|| false).is_err());
    }
}
