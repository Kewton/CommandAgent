//! Bounded signal waits for the `tui_pty` integration tests.
//!
//! A PTY script decides when to send the next input by waiting for a signal
//! from the run it drives: a screen marker, the REPL prompt, a Gate 1 hash or
//! Gate 4 heading in the boundary transcript, a request received by the fake
//! provider, or a rendered file. Every waiter here runs against a monotonic
//! deadline and a short poll interval. None of them waits forever, retries a
//! failing case, or treats an unobserved signal as success. A waiter that
//! times out, or that sees the child exit first, fails with what it was
//! waiting for, the child's state, and the tail of what the screen last
//! showed. Every session kills and reaps its child and joins its drain
//! threads when it ends, including on failure.

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, ExitStatus, Output, Stdio};
use std::sync::{Arc, Mutex, MutexGuard, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// REPL prompt text. rustyline prints it only after the editor owns the
/// terminal, so seeing it means typed input is no longer echoed by the line
/// discipline in cooked mode.
pub const PROMPT: &str = "commandagent>";

/// Explicit test-only poll interval shared by every waiter.
pub const POLL_INTERVAL: Duration = Duration::from_millis(5);

/// Upper bound for the child to reach its first prompt. This is a bootstrap
/// bound for a loaded machine; it never extends a product timeout.
pub const STARTUP_TIMEOUT: Duration = Duration::from_secs(20);

/// Upper bound for one input to produce its next observable signal.
pub const STEP_TIMEOUT: Duration = Duration::from_secs(10);

/// Upper bound for the HTTP request of a fake provider to arrive in full.
const HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Grace for a drain thread to reach end-of-file after its child is gone.
const READER_JOIN_GRACE: Duration = Duration::from_secs(5);

/// Number of trailing bytes of output quoted in a failure message.
const DIAGNOSTIC_TAIL_BYTES: usize = 600;

fn lock(buffer: &Mutex<Vec<u8>>) -> MutexGuard<'_, Vec<u8>> {
    buffer
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn drain(
    mut input: impl Read + Send + 'static,
    sink: Arc<Mutex<Vec<u8>>>,
) -> JoinHandle<std::io::Result<()>> {
    thread::spawn(move || {
        let mut chunk = [0_u8; 4096];
        loop {
            match input.read(&mut chunk) {
                Ok(0) => return Ok(()),
                Ok(read) => lock(&sink).extend_from_slice(&chunk[..read]),
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
    })
}

fn tail_of(bytes: &[u8]) -> String {
    let start = bytes.len().saturating_sub(DIAGNOSTIC_TAIL_BYTES);
    String::from_utf8_lossy(&bytes[start..]).into_owned()
}

fn find_from(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    let tail = haystack.get(from..)?;
    tail.windows(needle.len())
        .position(|window| window == needle)
        .map(|offset| offset + from)
}

/// Command that runs `command_line` under a fresh `script(1)` pseudo-terminal.
pub fn script_command(command_line: &str) -> Command {
    let mut command = Command::new("script");
    if cfg!(target_os = "macos") {
        command
            .arg("-q")
            .arg("/dev/null")
            .arg("/bin/sh")
            .arg("-c")
            .arg(command_line);
    } else {
        command
            .arg("-q")
            .arg("-c")
            .arg(command_line)
            .arg("/dev/null");
    }
    command
}

/// Length in bytes of `path`, or 0 while it does not exist. Taken before an
/// input is sent, it makes a later transcript wait ignore earlier markers.
pub fn file_len(path: &Path) -> usize {
    std::fs::metadata(path).map_or(0, |meta| meta.len() as usize)
}

fn read_after(path: &Path, from: usize) -> String {
    let bytes = std::fs::read(path).unwrap_or_default();
    bytes
        .get(from..)
        .map(|tail| String::from_utf8_lossy(tail).into_owned())
        .unwrap_or_default()
}

/// A child running under a pseudo-terminal whose stdout and stderr are drained
/// continuously, so a waiter can look at the screen while the child runs.
pub struct PtySession {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout: Arc<Mutex<Vec<u8>>>,
    stderr: Arc<Mutex<Vec<u8>>>,
    readers: Vec<JoinHandle<std::io::Result<()>>>,
    exited: Option<ExitStatus>,
}

impl PtySession {
    /// Spawn `command` with piped stdio and start draining its output.
    pub fn spawn(mut command: Command) -> std::io::Result<Self> {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let stdout = Arc::new(Mutex::new(Vec::new()));
        let stderr = Arc::new(Mutex::new(Vec::new()));
        let readers = vec![
            drain(child.stdout.take().unwrap(), Arc::clone(&stdout)),
            drain(child.stderr.take().unwrap(), Arc::clone(&stderr)),
        ];
        let stdin = child.stdin.take();
        Ok(Self {
            child: Some(child),
            stdin,
            stdout,
            stderr,
            readers,
            exited: None,
        })
    }

    /// Current end of the screen output. Pass it as `from` to a waiter so that
    /// only output produced after this point can satisfy it.
    pub fn mark(&self) -> usize {
        lock(&self.stdout).len()
    }

    /// Send `bytes` to the child's terminal input.
    pub fn send(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        let result = match self.stdin.as_mut() {
            Some(stdin) => stdin.write_all(bytes).and_then(|()| stdin.flush()),
            None => Err(std::io::Error::other("PTY stdin was already closed")),
        };
        result.map_err(|error| self.failure(&format!("sending {bytes:?}"), error.kind(), &error))
    }

    fn failure(
        &self,
        what: &str,
        kind: std::io::ErrorKind,
        cause: &dyn std::fmt::Display,
    ) -> std::io::Error {
        let child = match self.exited {
            Some(status) => format!("exited with {status}"),
            None => "still running".to_string(),
        };
        let stderr = lock(&self.stderr);
        let stderr = if stderr.is_empty() {
            String::new()
        } else {
            format!("; stderr tail={:?}", tail_of(&stderr))
        };
        std::io::Error::new(
            kind,
            format!(
                "{cause} while waiting for {what}; child {child}; screen tail={:?}{stderr}",
                tail_of(&lock(&self.stdout)),
            ),
        )
    }

    fn note_exit(&mut self) {
        if self.exited.is_none()
            && let Some(child) = self.child.as_mut()
            && let Ok(Some(status)) = child.try_wait()
        {
            self.exited = Some(status);
        }
    }

    /// Poll `check` against the screen output until it returns a value. It
    /// fails at `timeout`, or as soon as the child has exited and its output is
    /// fully drained without `check` having succeeded.
    pub fn poll_until<T>(
        &mut self,
        what: &str,
        timeout: Duration,
        mut check: impl FnMut(&[u8]) -> Option<T>,
    ) -> std::io::Result<T> {
        let deadline = Instant::now() + timeout;
        loop {
            let found = check(&lock(&self.stdout));
            if let Some(found) = found {
                return Ok(found);
            }
            self.note_exit();
            if self.exited.is_some() && self.readers.iter().all(JoinHandle::is_finished) {
                let found = check(&lock(&self.stdout));
                return found.ok_or_else(|| {
                    self.failure(
                        what,
                        std::io::ErrorKind::BrokenPipe,
                        &"child exited before the signal appeared",
                    )
                });
            }
            if Instant::now() >= deadline {
                return Err(self.failure(
                    what,
                    std::io::ErrorKind::TimedOut,
                    &format!("timed out after {timeout:?}"),
                ));
            }
            thread::sleep(POLL_INTERVAL);
        }
    }

    /// Wait for `needle` on the screen after offset `from`; returns the offset
    /// just past the match.
    pub fn wait_for_text(
        &mut self,
        needle: &str,
        from: usize,
        timeout: Duration,
    ) -> std::io::Result<usize> {
        let bytes = needle.as_bytes();
        self.poll_until(
            &format!("screen text {needle:?} after offset {from}"),
            timeout,
            |screen| find_from(screen, bytes, from).map(|start| start + bytes.len()),
        )
    }

    /// Wait for a prompt on the screen after offset `from`.
    pub fn wait_for_prompt(&mut self, from: usize, timeout: Duration) -> std::io::Result<usize> {
        self.wait_for_text(PROMPT, from, timeout)
    }

    /// Wait for `needle` and then for a prompt printed after it, which is the
    /// REPL asking for the next input once the command's output is complete.
    pub fn wait_for_text_then_prompt(
        &mut self,
        needle: &str,
        from: usize,
        timeout: Duration,
    ) -> std::io::Result<usize> {
        let seen = self.wait_for_text(needle, from, timeout)?;
        self.wait_for_prompt(seen, timeout)
    }

    /// Wait for a signal sent by a fake provider thread, for example that it
    /// received a request.
    pub fn wait_for_signal(
        &mut self,
        signal: &mpsc::Receiver<()>,
        what: &str,
        timeout: Duration,
    ) -> std::io::Result<()> {
        self.poll_until(what, timeout, |_| signal.try_recv().ok())
    }

    /// Wait for `path` to exist.
    pub fn wait_for_file(&mut self, path: &Path, timeout: Duration) -> std::io::Result<()> {
        self.poll_until(&format!("file {}", path.display()), timeout, |_| {
            path.exists().then_some(())
        })
    }

    /// Wait for `expected` to be appended to the boundary transcript at `path`
    /// after byte offset `from`.
    pub fn wait_for_transcript_text(
        &mut self,
        path: &Path,
        expected: &str,
        from: usize,
        timeout: Duration,
    ) -> std::io::Result<()> {
        self.poll_until(
            &format!("{expected:?} in {} after byte {from}", path.display()),
            timeout,
            |_| read_after(path, from).contains(expected).then_some(()),
        )
        .map_err(|error| with_transcript_tail(error, path))
    }

    /// Wait for a complete Gate 1 hash in the transcript at `path` after byte
    /// offset `from` and return it.
    pub fn wait_for_gate_one_hash(
        &mut self,
        path: &Path,
        from: usize,
        timeout: Duration,
    ) -> std::io::Result<String> {
        self.poll_until(
            &format!("Gate 1 hash in {} after byte {from}", path.display()),
            timeout,
            |_| {
                let transcript = read_after(path, from);
                let start = transcript.find("sha256:")?;
                let hash = transcript[start..]
                    .chars()
                    .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == ':')
                    .collect::<String>();
                (hash.len() == "sha256:".len() + 64).then_some(hash)
            },
        )
        .map_err(|error| with_transcript_tail(error, path))
    }

    /// Close the child's input, wait up to `timeout` for it to exit (killing it
    /// when it does not), and collect its output. Kill and wait both stay
    /// bounded, and the drain threads are joined.
    pub fn finish(&mut self, timeout: Duration) -> std::io::Result<Output> {
        self.stdin = None;
        let mut child = self
            .child
            .take()
            .ok_or_else(|| std::io::Error::other("PTY child was already finished"))?;
        let deadline = Instant::now() + timeout;
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                break child.wait()?;
            }
            thread::sleep(POLL_INTERVAL);
        };
        self.exited = Some(status);
        self.join_readers()?;
        Ok(Output {
            status,
            stdout: std::mem::take(&mut *lock(&self.stdout)),
            stderr: std::mem::take(&mut *lock(&self.stderr)),
        })
    }

    fn join_readers(&mut self) -> std::io::Result<()> {
        let deadline = Instant::now() + READER_JOIN_GRACE;
        for reader in std::mem::take(&mut self.readers) {
            while !reader.is_finished() && Instant::now() < deadline {
                thread::sleep(POLL_INTERVAL);
            }
            if !reader.is_finished() {
                return Err(std::io::Error::other(
                    "PTY output pipe did not reach end-of-file after the child exited",
                ));
            }
            reader
                .join()
                .map_err(|_| std::io::Error::other("PTY output drain thread panicked"))??;
        }
        Ok(())
    }
}

impl Drop for PtySession {
    fn drop(&mut self) {
        self.stdin = None;
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = self.join_readers();
    }
}

fn with_transcript_tail(error: std::io::Error, path: &Path) -> std::io::Error {
    let transcript = std::fs::read(path).unwrap_or_default();
    std::io::Error::new(
        error.kind(),
        format!("{error}; transcript tail={:?}", tail_of(&transcript)),
    )
}

/// Read one HTTP request from `stream`, returning only once the headers and
/// the `Content-Length` body are all buffered. The accepted socket can be
/// non-blocking, so a read that would block is retried until the deadline.
/// A request that is cut short or never completes is an error, not a partial
/// request.
pub fn read_http_request(stream: &mut impl Read) -> std::io::Result<String> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 1024];
    let mut expected = None;
    let deadline = Instant::now() + HTTP_REQUEST_TIMEOUT;
    loop {
        if expected.is_some_and(|expected| request.len() >= expected) {
            return Ok(String::from_utf8_lossy(&request).into_owned());
        }
        if Instant::now() >= deadline {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!(
                    "HTTP request incomplete after {HTTP_REQUEST_TIMEOUT:?}: have {} of {expected:?} bytes",
                    request.len()
                ),
            ));
        }
        match stream.read(&mut buffer) {
            Ok(0) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    format!(
                        "peer closed after {} of {expected:?} request bytes",
                        request.len()
                    ),
                ));
            }
            Ok(read) => {
                request.extend_from_slice(&buffer[..read]);
                if expected.is_none()
                    && let Some(header_end) =
                        request.windows(4).position(|part| part == b"\r\n\r\n")
                {
                    let headers = String::from_utf8_lossy(&request[..header_end]);
                    let content_length = headers.lines().find_map(|line| {
                        line.split_once(':').and_then(|(name, value)| {
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().ok())
                                .flatten()
                        })
                    });
                    expected = Some(header_end + 4 + content_length.unwrap_or_default());
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::Interrupted
                ) =>
            {
                thread::sleep(Duration::from_millis(1));
            }
            Err(error) => return Err(error),
        }
    }
}
