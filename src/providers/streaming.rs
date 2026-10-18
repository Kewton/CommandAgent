use std::future::Future;
use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::Poll;
use std::time::Duration;

use anyhow::{Context, bail};
use tokio::sync::Notify;

const READ_BUFFER_BYTES: usize = 8 * 1024;
const MAX_STREAM_LINE_BYTES: usize = 8 * 1024 * 1024;
const AFTER_FIRST_CHUNK_PREFIX: &str = "provider stream failed after partial output:";

const ABORTED_BY_USER_ERROR: &str = "aborted_by_user: interrupted by user";

/// Cancellation signal shared between `provider_call` and a provider client.
/// Firing it makes every in-flight [`HttpCall`] drop its request, which closes
/// the HTTP connection.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<CancelState>);

#[derive(Debug, Default)]
struct CancelState {
    cancelled: AtomicBool,
    notify: Notify,
}

impl CancelToken {
    pub fn cancel(&self) {
        self.0.cancelled.store(true, Ordering::SeqCst);
        self.0.notify.notify_one();
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::SeqCst)
    }

    async fn cancelled(&self) {
        loop {
            let notified = self.0.notify.notified();
            if self.is_cancelled() {
                return;
            }
            notified.await;
        }
    }
}

pub fn aborted_by_user() -> anyhow::Error {
    anyhow::anyhow!(ABORTED_BY_USER_ERROR)
}

/// Async HTTP client with the timeout semantics of the former blocking client.
/// Idle connections are not pooled because each call owns its own runtime.
pub fn http_client(timeout_secs: u64) -> reqwest::Result<reqwest::Client> {
    let timeout = Duration::from_secs(timeout_secs);
    reqwest::Client::builder()
        .connect_timeout(timeout)
        .timeout(timeout)
        .pool_max_idle_per_host(0)
        .build()
}

/// One provider call's current-thread runtime. Each wait races the request or
/// body read against the cancel token; on cancel the future is dropped and the
/// runtime is dropped with this value, so the socket closes.
pub struct HttpCall {
    runtime: tokio::runtime::Runtime,
    cancel: CancelToken,
}

impl HttpCall {
    pub fn new(cancel: &CancelToken) -> anyhow::Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("failed to start provider HTTP runtime")?;
        Ok(Self {
            runtime,
            cancel: cancel.clone(),
        })
    }

    pub fn run<F: Future>(&self, future: F) -> anyhow::Result<F::Output> {
        if self.cancel.is_cancelled() {
            return Err(aborted_by_user());
        }
        let mut future = std::pin::pin!(future);
        let mut cancelled = std::pin::pin!(self.cancel.cancelled());
        self.runtime.block_on(std::future::poll_fn(|cx| {
            if cancelled.as_mut().poll(cx).is_ready() {
                return Poll::Ready(Err(aborted_by_user()));
            }
            future.as_mut().poll(cx).map(Ok)
        }))
    }

    pub fn reader(&self, response: reqwest::Response) -> BodyReader<'_> {
        BodyReader {
            call: self,
            response,
            buffer: Vec::new(),
            offset: 0,
        }
    }
}

/// `std::io::Read` over an async response body, so the line parsers stay
/// synchronous.
pub struct BodyReader<'a> {
    call: &'a HttpCall,
    response: reqwest::Response,
    buffer: Vec<u8>,
    offset: usize,
}

impl Read for BodyReader<'_> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        if self.offset >= self.buffer.len() {
            match self.call.run(self.response.chunk()) {
                Ok(Ok(Some(chunk))) => {
                    self.buffer = chunk.to_vec();
                    self.offset = 0;
                }
                Ok(Ok(None)) => return Ok(0),
                Ok(Err(err)) => return Err(std::io::Error::other(err)),
                Err(err) => return Err(std::io::Error::other(err)),
            }
        }
        let count = out.len().min(self.buffer.len() - self.offset);
        out[..count].copy_from_slice(&self.buffer[self.offset..self.offset + count]);
        self.offset += count;
        Ok(count)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamControl {
    Continue,
    Stop,
}

pub fn after_first_chunk(err: anyhow::Error) -> anyhow::Error {
    anyhow::anyhow!("{AFTER_FIRST_CHUNK_PREFIX} {err:#}")
}

pub fn is_after_first_chunk_error(message: &str) -> bool {
    message.contains(AFTER_FIRST_CHUNK_PREFIX)
}

pub fn retry_allowed(attempt: usize, retries: usize, delivered_chunk: bool) -> bool {
    !delivered_chunk && attempt < retries
}

pub fn read_ndjson<R, F>(reader: R, mut on_line: F) -> anyhow::Result<()>
where
    R: Read,
    F: FnMut(&str) -> anyhow::Result<StreamControl>,
{
    read_utf8_lines(reader, |line| {
        if line.trim().is_empty() {
            return Ok(StreamControl::Continue);
        }
        on_line(line)
    })
}

pub fn read_sse<R, F>(reader: R, mut on_data: F) -> anyhow::Result<()>
where
    R: Read,
    F: FnMut(&str) -> anyhow::Result<StreamControl>,
{
    let mut data_lines = Vec::new();
    let mut stopped = false;
    read_utf8_lines(reader, |line| {
        if stopped {
            return Ok(StreamControl::Stop);
        }
        if line.is_empty() {
            stopped = emit_sse_event(&mut data_lines, &mut on_data)? == StreamControl::Stop;
            return Ok(if stopped {
                StreamControl::Stop
            } else {
                StreamControl::Continue
            });
        }
        if line.starts_with(':') {
            return Ok(StreamControl::Continue);
        }
        if let Some(data) = line.strip_prefix("data:") {
            data_lines.push(data.strip_prefix(' ').unwrap_or(data).to_string());
        }
        Ok(StreamControl::Continue)
    })?;
    if !stopped && !data_lines.is_empty() {
        let _ = emit_sse_event(&mut data_lines, &mut on_data)?;
    }
    Ok(())
}

fn emit_sse_event<F>(data_lines: &mut Vec<String>, on_data: &mut F) -> anyhow::Result<StreamControl>
where
    F: FnMut(&str) -> anyhow::Result<StreamControl>,
{
    if data_lines.is_empty() {
        return Ok(StreamControl::Continue);
    }
    let data = data_lines.join("\n");
    data_lines.clear();
    if data.trim() == "[DONE]" {
        return Ok(StreamControl::Stop);
    }
    on_data(&data)
}

fn read_utf8_lines<R, F>(mut reader: R, mut on_line: F) -> anyhow::Result<()>
where
    R: Read,
    F: FnMut(&str) -> anyhow::Result<StreamControl>,
{
    let mut pending = Vec::new();
    let mut read_buffer = [0_u8; READ_BUFFER_BYTES];
    loop {
        let read = reader
            .read(&mut read_buffer)
            .context("failed to read provider stream")?;
        if read == 0 {
            break;
        }
        pending.extend_from_slice(&read_buffer[..read]);
        while let Some(newline) = pending.iter().position(|byte| *byte == b'\n') {
            let mut line = pending.drain(..=newline).collect::<Vec<_>>();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            let line =
                std::str::from_utf8(&line).context("provider stream contains invalid UTF-8")?;
            if on_line(line)? == StreamControl::Stop {
                return Ok(());
            }
        }
        if pending.len() > MAX_STREAM_LINE_BYTES {
            bail!("provider stream line exceeds {MAX_STREAM_LINE_BYTES} bytes");
        }
    }
    if !pending.is_empty() {
        let line = std::str::from_utf8(&pending)
            .context("provider stream contains incomplete or invalid UTF-8")?;
        let _ = on_line(line)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Cursor};

    struct OneByteReader<R> {
        inner: R,
    }

    impl<R: Read> Read for OneByteReader<R> {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            self.inner.read(&mut buffer[..1])
        }
    }

    #[test]
    fn ndjson_preserves_utf8_split_across_reads() {
        let source = "{\"text\":\"日🙂\"}\n";
        let mut lines = Vec::new();
        read_ndjson(
            OneByteReader {
                inner: Cursor::new(source.as_bytes()),
            },
            |line| {
                lines.push(line.to_string());
                Ok(StreamControl::Continue)
            },
        )
        .unwrap();
        assert_eq!(lines, vec![r#"{"text":"日🙂"}"#]);
    }

    #[test]
    fn sse_combines_data_lines_and_stops_at_done() {
        let source = "event: x\r\ndata: one\r\ndata: two\r\n\r\ndata: [DONE]\n\ndata: ignored\n\n";
        let mut events = Vec::new();
        read_sse(Cursor::new(source), |data| {
            events.push(data.to_string());
            Ok(StreamControl::Continue)
        })
        .unwrap();
        assert_eq!(events, vec!["one\ntwo"]);
    }

    #[test]
    fn invalid_utf8_is_a_clear_error() {
        let err = read_ndjson(Cursor::new(vec![b'{', 0xff, b'}', b'\n']), |_| {
            Ok(StreamControl::Continue)
        })
        .unwrap_err()
        .to_string();
        assert!(err.contains("invalid UTF-8"), "{err}");
    }

    #[test]
    fn retry_is_allowed_only_before_first_chunk_and_with_budget() {
        assert!(retry_allowed(0, 1, false));
        assert!(!retry_allowed(0, 1, true));
        assert!(!retry_allowed(1, 1, false));
    }
}

/// Test support shared by the provider modules: a mock server that holds its
/// response open and reports when the client's connection closes.
#[cfg(test)]
pub(crate) mod cancel_test_support {
    use super::CancelToken;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    const CLOSE_BUDGET: Duration = Duration::from_secs(1);
    const SERVER_DEADLINE: Duration = Duration::from_secs(10);

    /// Run `call` against a server that keeps the response open. The token is
    /// cancelled 100 ms after the server holds the request; the server must
    /// then see the connection close within one second. `first_chunk` is sent
    /// as a streaming 200 response before holding.
    pub(crate) fn assert_cancel_closes_connection<T>(
        first_chunk: Option<&'static str>,
        call: impl FnOnce(String, CancelToken) -> anyhow::Result<T>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let base_url = format!("http://{}", listener.local_addr().expect("address"));
        let (started_tx, started_rx) = mpsc::channel();
        let (eof_tx, eof_rx) = mpsc::channel();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .expect("read timeout");
            let mut data = Vec::new();
            let mut buffer = [0_u8; 4096];
            loop {
                let read = stream.read(&mut buffer).expect("request");
                assert!(read > 0, "client closed before sending the request");
                data.extend_from_slice(&buffer[..read]);
                let Some(end) = data.windows(4).position(|window| window == b"\r\n\r\n") else {
                    continue;
                };
                let head = String::from_utf8_lossy(&data[..end]).to_ascii_lowercase();
                let length = head
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length:"))
                    .and_then(|value| value.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                if data.len() >= end + 4 + length {
                    break;
                }
            }
            if let Some(chunk) = first_chunk {
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n{chunk}"
                )
                .expect("first chunk");
                stream.flush().expect("flush");
            }
            started_tx.send(()).expect("started");
            stream
                .set_read_timeout(Some(Duration::from_millis(50)))
                .expect("read timeout");
            let deadline = Instant::now() + SERVER_DEADLINE;
            let mut sink = [0_u8; 64];
            let eof = loop {
                match stream.read(&mut sink) {
                    Ok(0) => break Some(Instant::now()),
                    Ok(_) => {}
                    Err(err)
                        if matches!(
                            err.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                        ) => {}
                    Err(_) => break Some(Instant::now()),
                }
                if Instant::now() >= deadline {
                    break None;
                }
            };
            let _ = eof_tx.send(eof);
        });

        let token = CancelToken::default();
        let (fired_tx, fired_rx) = mpsc::channel();
        let canceller = token.clone();
        std::thread::spawn(move || {
            started_rx.recv().expect("server holds the request");
            std::thread::sleep(Duration::from_millis(100));
            let fired = Instant::now();
            canceller.cancel();
            let _ = fired_tx.send(fired);
        });

        let result = call(base_url, token);
        assert!(result.is_err(), "a cancelled call must not return a reply");
        let fired = fired_rx.recv().expect("cancel fired");
        let eof = eof_rx
            .recv_timeout(Duration::from_secs(15))
            .expect("server finished")
            .expect("server never observed the connection closing");
        let closed_after = eof.saturating_duration_since(fired);
        eprintln!("cancel -> server EOF: {} ms", closed_after.as_millis());
        assert!(
            closed_after < CLOSE_BUDGET,
            "server saw EOF {closed_after:?} after the cancel"
        );
    }
}
