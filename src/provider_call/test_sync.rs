//! Synchronization helpers and synchronization tests for the provider call
//! chokepoint.
//!
//! The provider cancellation and timeout tests used to cancel after a fixed
//! sleep, so a cancellation could fire before the worker ever started and the
//! test would still pass. These helpers replace the sleep with explicit
//! `started` / `release` / `completed` / `closed` signals and a controlled
//! clock, so cancellation is bound to the run it observes and no branch waits
//! for a worker's natural completion.

use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use super::tests::test_config;
use super::{
    PROVIDER_WAIT_SLICE, ProviderCallScope, ProviderChatRequest, chat, chat_with_cancel,
    chat_with_cancel_and_stream, is_aborted_by_user, wait_clock,
};
use crate::config::Config;
use crate::providers::{AssistantReply, ChatClient};
use crate::state::ConversationMessage;
use crate::tools::registry::ToolSpec;

/// Diagnosis-only ceiling for every worker handshake. It is finite so a failed
/// branch can never block a test forever, and it is never used to decide
/// pass/fail.
const SIGNAL_WAIT_LIMIT: Duration = Duration::from_secs(5);
/// Diagnosis-only ceiling for the mock HTTP server's accept/read handshake.
const SERVER_WAIT_LIMIT: Duration = Duration::from_secs(5);

/// Test-side half of a synchronized provider worker.
pub(super) struct WorkerSignals {
    started: Option<mpsc::Receiver<()>>,
    release: Option<mpsc::Sender<()>>,
    completed: mpsc::Receiver<()>,
}

impl WorkerSignals {
    /// Take the `started` receiver so a helper thread can cancel only after
    /// the worker actually reached the provider.
    pub(super) fn take_started(&mut self) -> mpsc::Receiver<()> {
        self.started.take().expect("started signal")
    }

    /// Release the blocked worker and confirm it returned.
    pub(super) fn release_and_join(&mut self) {
        if let Some(release) = self.release.take() {
            let _ = release.send(());
        }
        self.completed
            .recv_timeout(SIGNAL_WAIT_LIMIT)
            .expect("worker completed after release");
    }

    /// Whether the worker already reported completion, used to prove the caller
    /// returned before the worker was released.
    pub(super) fn worker_finished(&self) -> bool {
        self.completed.try_recv().is_ok()
    }
}

impl Drop for WorkerSignals {
    fn drop(&mut self) {
        // Always release the worker, including on assertion failure, so no
        // branch leaves the provider thread blocked.
        if let Some(release) = self.release.take() {
            let _ = release.send(());
        }
    }
}

/// A provider client that blocks until the test releases it, reporting
/// `started` and `completed` around the blocked call.
#[derive(Clone)]
pub(super) struct ControlledClient {
    started: mpsc::Sender<()>,
    release: Arc<Mutex<mpsc::Receiver<()>>>,
    completed: mpsc::Sender<()>,
}

impl ControlledClient {
    pub(super) fn new() -> (Self, WorkerSignals) {
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (completed_tx, completed_rx) = mpsc::channel();
        (
            Self {
                started: started_tx,
                release: Arc::new(Mutex::new(release_rx)),
                completed: completed_tx,
            },
            WorkerSignals {
                started: Some(started_rx),
                release: Some(release_tx),
                completed: completed_rx,
            },
        )
    }
}

impl ChatClient for ControlledClient {
    fn label(&self) -> &str {
        "controlled-mock"
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn chat(
        &mut self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        let _ = self.started.send(());
        let _ = self
            .release
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .recv_timeout(SIGNAL_WAIT_LIMIT);
        let _ = self.completed.send(());
        Ok(AssistantReply::text("late"))
    }
}

/// Test-side half of a synchronized streaming provider worker.
pub(super) struct StreamSignals {
    started: Option<mpsc::Receiver<()>>,
    proceed: Option<mpsc::Sender<()>>,
    closed: mpsc::Receiver<()>,
}

impl StreamSignals {
    pub(super) fn take_started(&mut self) -> mpsc::Receiver<()> {
        self.started.take().expect("started signal")
    }

    /// Let the held worker attempt its next stream send.
    pub(super) fn proceed(&mut self) {
        if let Some(proceed) = self.proceed.take() {
            let _ = proceed.send(());
        }
    }

    /// Wait for the worker to observe that the caller dropped the receiver.
    pub(super) fn wait_closed(&self) {
        self.closed
            .recv_timeout(SIGNAL_WAIT_LIMIT)
            .expect("worker observed the closed receiver");
    }

    /// Whether the worker already observed the closed receiver.
    pub(super) fn closed_observed(&self) -> bool {
        self.closed.try_recv().is_ok()
    }
}

impl Drop for StreamSignals {
    fn drop(&mut self) {
        // Always release the stream worker, including on assertion failure.
        if let Some(proceed) = self.proceed.take() {
            let _ = proceed.send(());
        }
    }
}

/// A streaming provider client that holds before its first send, then reports
/// when the dropped receiver makes the send fail.
#[derive(Clone)]
pub(super) struct ControlledStreamingClient {
    started: mpsc::Sender<()>,
    proceed: Arc<Mutex<mpsc::Receiver<()>>>,
    closed: mpsc::Sender<()>,
}

impl ControlledStreamingClient {
    pub(super) fn new() -> (Self, StreamSignals) {
        let (started_tx, started_rx) = mpsc::channel();
        let (proceed_tx, proceed_rx) = mpsc::channel();
        let (closed_tx, closed_rx) = mpsc::channel();
        (
            Self {
                started: started_tx,
                proceed: Arc::new(Mutex::new(proceed_rx)),
                closed: closed_tx,
            },
            StreamSignals {
                started: Some(started_rx),
                proceed: Some(proceed_tx),
                closed: closed_rx,
            },
        )
    }
}

impl ChatClient for ControlledStreamingClient {
    fn label(&self) -> &str {
        "controlled-streaming-mock"
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    fn chat_stream(
        &mut self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
        on_chunk: &mut dyn FnMut(&str) -> anyhow::Result<()>,
    ) -> anyhow::Result<AssistantReply> {
        let _ = self.started.send(());
        let _ = self
            .proceed
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .recv_timeout(SIGNAL_WAIT_LIMIT);
        for _ in 0..200 {
            if let Err(error) = on_chunk("") {
                let _ = self.closed.send(());
                return Err(error);
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        Ok(AssistantReply::text("late"))
    }

    fn chat(
        &mut self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        Ok(AssistantReply::text("batch"))
    }
}

/// Releases the mock server on drop, so a failing assertion still unblocks it.
struct ReleaseOnDrop(mpsc::Sender<()>);

impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}

/// A temp dir that never sits under the system prefixes (`/tmp`, `/usr`,
/// `/bin`, `/opt`, `/etc`): `tempfile::tempdir()` lands under `/tmp` on Linux,
/// which the test sandbox treats as a readable system prefix. Fixtures go under
/// the target directory instead. `CARGO_TARGET_TMPDIR` is defined for
/// integration tests only, so unit tests fall back to `OUT_DIR`.
fn test_tempdir() -> tempfile::TempDir {
    let parent = option_env!("CARGO_TARGET_TMPDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("OUT_DIR")));
    std::fs::create_dir_all(&parent).expect("create test tempdir parent");
    tempfile::tempdir_in(&parent).expect("create test tempdir")
}

/// A clock advanced only by explicit slices, so timeout semantics are verified
/// without taking a real blocking wait.
struct VirtualClock {
    elapsed: Duration,
}

impl wait_clock::WaitClock for VirtualClock {
    fn elapsed(&mut self) -> Duration {
        self.elapsed
    }
}

/// A boundary that records each requested slice and never blocks.
#[derive(Default)]
struct SliceRecorder {
    slices: Vec<Duration>,
}

impl wait_clock::WaitBoundary<()> for SliceRecorder {
    fn wait(&mut self, slice: Duration) -> wait_clock::WaitSignal<()> {
        self.slices.push(slice);
        wait_clock::WaitSignal::Elapsed
    }
}

#[test]
fn controlled_clock_holds_the_deadline_until_the_timeout_elapses() {
    let mut clock = VirtualClock {
        elapsed: Duration::ZERO,
    };
    let mut boundary = SliceRecorder::default();
    let timeout = Duration::from_secs(1);

    for index in 0..4 {
        let step = wait_clock::wait_step(
            &mut clock,
            &mut boundary,
            timeout,
            PROVIDER_WAIT_SLICE,
            || false,
            |_| {},
        );
        assert_eq!(step, wait_clock::WaitStep::Sliced, "iteration {index}");
        clock.elapsed += PROVIDER_WAIT_SLICE;
    }
    assert_eq!(boundary.slices, vec![PROVIDER_WAIT_SLICE; 4]);

    let step = wait_clock::wait_step(
        &mut clock,
        &mut boundary,
        timeout,
        PROVIDER_WAIT_SLICE,
        || false,
        |_| {},
    );
    assert_eq!(step, wait_clock::WaitStep::Deadline);
}

#[test]
fn controlled_clock_clamps_the_final_wait_slice_to_the_deadline() {
    let mut clock = VirtualClock {
        elapsed: Duration::from_millis(900),
    };
    let mut boundary = SliceRecorder::default();

    let step = wait_clock::wait_step(
        &mut clock,
        &mut boundary,
        Duration::from_secs(1),
        PROVIDER_WAIT_SLICE,
        || false,
        |_| {},
    );

    assert_eq!(step, wait_clock::WaitStep::Sliced);
    assert_eq!(boundary.slices, vec![Duration::from_millis(100)]);
}

#[test]
fn cancellation_is_observed_within_one_wait_slice_without_worker_completion() {
    let mut clock = VirtualClock {
        elapsed: Duration::ZERO,
    };
    let mut boundary = SliceRecorder::default();
    let timeout = Duration::from_secs(1);
    let cancel_at = PROVIDER_WAIT_SLICE;
    let mut elapsed = Duration::ZERO;
    let mut slices_waited = 0;

    loop {
        let step = wait_clock::wait_step(
            &mut clock,
            &mut boundary,
            timeout,
            PROVIDER_WAIT_SLICE,
            || elapsed >= cancel_at,
            |_| {},
        );
        match step {
            wait_clock::WaitStep::Sliced => {
                slices_waited += 1;
                assert!(
                    slices_waited <= 1,
                    "cancellation was not observed within one wait slice"
                );
                elapsed += PROVIDER_WAIT_SLICE;
                clock.elapsed = elapsed;
            }
            wait_clock::WaitStep::Cancelled => break,
            other => panic!("unexpected step before cancellation: {other:?}"),
        }
    }

    assert_eq!(elapsed, cancel_at);
    assert_eq!(boundary.slices, vec![PROVIDER_WAIT_SLICE]);
}

#[test]
fn controlled_provider_call_times_out_without_waiting_for_worker() {
    let tmp = test_tempdir();
    let events_path = tmp.path().join("events.jsonl");
    let config = test_config(tmp.path(), events_path.clone(), 1);
    let (mut client, mut signals) = ControlledClient::new();

    let started = Instant::now();
    let outcome = chat(
        &mut client,
        &config,
        ProviderCallScope::PlannerStep,
        "m",
        &[ConversationMessage::user("plan")],
        &[],
        false,
    );

    signals
        .take_started()
        .recv_timeout(SIGNAL_WAIT_LIMIT)
        .expect("worker reached the provider before the deadline");
    assert!(outcome.timed_out);
    assert!(started.elapsed() < Duration::from_secs(3));
    let error = outcome.result.unwrap_err().to_string();
    assert!(
        error.contains("phase_step_planner_timeout"),
        "unexpected error: {error}"
    );
    signals.release_and_join();

    let events = std::fs::read_to_string(events_path).expect("events");
    assert!(events.contains("\"event\":\"provider_turn_duration\""));
    assert!(events.contains("\"caller_scope\":\"planner_step\""));
    assert!(events.contains("\"timeout_source\":\"override:test\""));
    assert!(events.contains("\"classification\":\"phase_step_planner_timeout\""));
}

#[test]
fn controlled_provider_call_aborts_promptly_when_cancelled() {
    let tmp = test_tempdir();
    let events_path = tmp.path().join("events.jsonl");
    let config = test_config(tmp.path(), events_path.clone(), 30);
    let (mut client, mut signals) = ControlledClient::new();
    let cancelled = Arc::new(AtomicBool::new(false));
    let thread_cancelled = Arc::clone(&cancelled);
    let started_signal = signals.take_started();
    let canceller = std::thread::spawn(move || {
        let started_ok = started_signal.recv_timeout(SIGNAL_WAIT_LIMIT).is_ok();
        thread_cancelled.store(true, Ordering::SeqCst);
        assert!(started_ok, "worker did not start before cancellation");
    });

    let started = Instant::now();
    let outcome = chat_with_cancel(
        &mut client,
        &config,
        ProviderChatRequest {
            scope: ProviderCallScope::PlannerStep,
            model: "m",
            messages: &[ConversationMessage::user("plan")],
            tools: &[],
            native_tools_enabled: false,
        },
        || cancelled.load(Ordering::SeqCst),
    );

    assert!(outcome.aborted_by_user);
    assert!(!outcome.timed_out);
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(
        !signals.worker_finished(),
        "worker completed before the caller released it"
    );
    signals.release_and_join();
    canceller.join().expect("canceller");

    let error = outcome.result.unwrap_err().to_string();
    assert!(is_aborted_by_user(&error), "unexpected error: {error}");
    let events = std::fs::read_to_string(events_path).expect("events");
    assert!(events.contains("\"event\":\"provider_turn_duration\""));
    assert!(events.contains("\"aborted_by_user\":true"));
    assert!(events.contains("\"classification\":\"aborted_by_user\""));
    assert!(events.contains("\"event\":\"provider_turn_aborted_by_user\""));
    assert!(!events.contains("\"event\":\"provider_turn_timeout\""));
}

#[test]
fn openai_compatible_mock_call_preserves_provider_cancellation() {
    let tmp = test_tempdir();
    let events_path = tmp.path().join("events.jsonl");
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    listener
        .set_nonblocking(true)
        .expect("nonblocking listener");
    let address = listener.local_addr().expect("address");
    let (request_tx, request_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let server = std::thread::spawn(move || {
        let accept_deadline = Instant::now() + SERVER_WAIT_LIMIT;
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= accept_deadline {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(_) => return,
            }
        };
        stream.set_nonblocking(false).ok();
        stream.set_read_timeout(Some(SERVER_WAIT_LIMIT)).ok();
        let mut request = vec![0_u8; 16 * 1024];
        let Ok(read) = stream.read(&mut request) else {
            return;
        };
        let request = String::from_utf8_lossy(&request[..read]);
        assert!(request.starts_with("POST /v1/chat/completions "));
        let _ = request_tx.send(());
        let _ = release_rx.recv_timeout(SERVER_WAIT_LIMIT);
        let body = r#"{"id":"chatcmpl-delayed","model":"served-model","choices":[{"message":{"content":"late"}}]}"#;
        let _ = write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
    });
    let release_guard = ReleaseOnDrop(release_tx);

    let cwd = tmp.path().to_string_lossy().to_string();
    let parsed = crate::provider_cli::parse_from([
        "commandagent",
        "--cwd",
        &cwd,
        "--provider",
        "openai-compatible",
        "--model",
        "served-model",
        "--base-url",
        &format!("http://{address}"),
    ])
    .expect("generic CLI");
    let mut config = Config::from_cli_with_provider_options(parsed.cli, parsed.provider_options)
        .expect("generic config");
    config.eval_events_path = Some(events_path.clone());
    config.chat_timeout_secs = 30;
    config.chat_timeout_source = "override:test".to_string();
    let mut client = crate::providers::client_from_config(&config, false).expect("client");

    let cancelled = Arc::new(AtomicBool::new(false));
    let thread_cancelled = Arc::clone(&cancelled);
    let canceller = std::thread::spawn(move || {
        let request_ok = request_rx.recv_timeout(SERVER_WAIT_LIMIT).is_ok();
        thread_cancelled.store(true, Ordering::SeqCst);
        assert!(request_ok, "mock server did not receive the request");
    });

    let started = Instant::now();
    let outcome = chat_with_cancel(
        client.as_mut(),
        &config,
        ProviderChatRequest {
            scope: ProviderCallScope::Executor,
            model: &config.model,
            messages: &[ConversationMessage::user("wait")],
            tools: &[],
            native_tools_enabled: false,
        },
        || cancelled.load(Ordering::SeqCst),
    );

    assert!(outcome.aborted_by_user);
    assert!(!outcome.timed_out);
    assert!(started.elapsed() < Duration::from_secs(1));
    canceller.join().expect("canceller");
    drop(release_guard);
    server.join().expect("server");

    let events = std::fs::read_to_string(&events_path).expect("events");
    assert!(events.contains("\"provider\":\"openai-compatible\""));
    assert!(events.contains("\"event\":\"provider_turn_aborted_by_user\""));
    assert!(!events.contains("\"event\":\"provider_turn_timeout\""));
}

#[test]
fn planner_cancellation_closes_stream_callback_with_visible_streaming_disabled() {
    let tmp = test_tempdir();
    let config = test_config(tmp.path(), tmp.path().join("events.jsonl"), 30);
    let (mut client, mut signals) = ControlledStreamingClient::new();
    let cancelled = Arc::new(AtomicBool::new(false));
    let thread_cancelled = Arc::clone(&cancelled);
    let started_signal = signals.take_started();
    let canceller = std::thread::spawn(move || {
        let started_ok = started_signal.recv_timeout(SIGNAL_WAIT_LIMIT).is_ok();
        thread_cancelled.store(true, Ordering::SeqCst);
        assert!(
            started_ok,
            "stream worker did not start before cancellation"
        );
    });
    let mut rendered_chunks = Vec::new();

    let started = Instant::now();
    let outcome = chat_with_cancel_and_stream(
        &mut client,
        &config,
        ProviderChatRequest {
            scope: ProviderCallScope::PlannerStep,
            model: "m",
            messages: &[ConversationMessage::user("plan")],
            tools: &[],
            native_tools_enabled: false,
        },
        || cancelled.load(Ordering::SeqCst),
        &mut |chunk| {
            rendered_chunks.push(chunk.to_string());
            Ok(())
        },
    );

    assert!(outcome.aborted_by_user);
    assert!(!outcome.timed_out);
    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(rendered_chunks.is_empty());
    assert!(
        !signals.closed_observed(),
        "worker observed the closed receiver before the caller returned"
    );
    signals.proceed();
    signals.wait_closed();
    canceller.join().expect("canceller");

    let events = std::fs::read_to_string(config.eval_events_path.unwrap()).expect("events");
    assert!(events.contains("\"aborted_by_user\":true"), "{events}");
    assert!(
        events.contains("\"event\":\"provider_turn_aborted_by_user\""),
        "{events}"
    );
}
