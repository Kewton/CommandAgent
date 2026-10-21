#![cfg(unix)]

//! Issue #699: a user interrupt closes the provider's HTTP connection within
//! one second, for the streaming and the non-streaming path. The mock keeps
//! the response open until the client goes away, so the EOF it observes is the
//! interrupt's doing and not a finished response or the chat timeout.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use clap::Parser;
use commandagent::cli::Cli;
use commandagent::config::Config;
use commandagent::provider_call::{self, ProviderCallScope};
use commandagent::state::ConversationMessage;

const CLOSE_BUDGET: Duration = Duration::from_secs(1);
const MOCK_DEADLINE: Duration = Duration::from_secs(10);
const CHAT_TIMEOUT_SECS: &str = "30";

struct Mock {
    eof_at: mpsc::Receiver<Option<Instant>>,
}

fn read_request(stream: &mut TcpStream) {
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
            return;
        }
    }
}

/// Accept one connection, optionally send a first stream chunk, then hold the
/// response open and report the `Instant` at which the client's EOF (or reset)
/// is observed on this thread.
fn spawn_mock(
    listener: TcpListener,
    stream_chunk: Option<&'static str>,
) -> (mpsc::Receiver<()>, Mock) {
    let (started_tx, started) = mpsc::channel();
    let (eof_tx, eof_at) = mpsc::channel();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        read_request(&mut stream);
        if let Some(chunk) = stream_chunk {
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nConnection: close\r\n\r\n{chunk}"
            )
            .expect("first chunk");
            stream.flush().expect("flush");
        }
        started_tx.send(()).expect("started");
        stream
            .set_read_timeout(Some(Duration::from_millis(50)))
            .expect("read timeout");
        let deadline = Instant::now() + MOCK_DEADLINE;
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
    (started, Mock { eof_at })
}

fn config_for(address: std::net::SocketAddr, cwd: &str) -> Config {
    config_for_provider(address, cwd, "ollama", "--ollama-host")
}

fn config_for_provider(
    address: std::net::SocketAddr,
    cwd: &str,
    provider: &str,
    host_flag: &str,
) -> Config {
    Config::from_cli(Cli::parse_from([
        "commandagent",
        "--cwd",
        cwd,
        "--model",
        "issue699-model",
        "--provider",
        provider,
        host_flag,
        &format!("http://{address}"),
        "--chat-timeout-secs",
        CHAT_TIMEOUT_SECS,
    ]))
    .expect("config")
}

/// Fire the interrupt once the mock reports the request is being held.
fn interrupt_after_started(
    started: mpsc::Receiver<()>,
) -> (Arc<AtomicBool>, Arc<Mutex<Option<Instant>>>) {
    let flag = Arc::new(AtomicBool::new(false));
    let fired_at = Arc::new(Mutex::new(None));
    let (thread_flag, thread_fired) = (Arc::clone(&flag), Arc::clone(&fired_at));
    std::thread::spawn(move || {
        started.recv().expect("mock started");
        std::thread::sleep(Duration::from_millis(100));
        *thread_fired.lock().expect("lock") = Some(Instant::now());
        thread_flag.store(true, Ordering::SeqCst);
    });
    (flag, fired_at)
}

fn provider_turn_events(events: &std::path::Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(events)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|event| event["event"] == "provider_turn_duration")
        .collect()
}

fn assert_closed_within_budget(
    mock: &Mock,
    fired_at: &Arc<Mutex<Option<Instant>>>,
    outcome: &provider_call::ProviderCallOutcome,
    events: &std::path::Path,
) {
    assert!(outcome.aborted_by_user, "call was not aborted by the user");
    let error = outcome
        .result
        .as_ref()
        .expect_err("cancelled call has no reply");
    assert!(
        provider_call::is_aborted_by_user(&error.to_string()),
        "{error:#}"
    );
    let eof = mock
        .eof_at
        .recv_timeout(Duration::from_secs(5))
        .expect("mock finished")
        .expect("server never observed EOF before its deadline");
    let fired = fired_at.lock().expect("lock").expect("interrupt fired");
    let closed_after = eof.saturating_duration_since(fired);
    eprintln!("interrupt -> server EOF: {} ms", closed_after.as_millis());
    assert!(
        closed_after < CLOSE_BUDGET,
        "server saw EOF {closed_after:?} after the interrupt"
    );

    // Nothing from the cancelled call is recorded after the interrupt: exactly
    // one aborted turn record, and no completed one.
    std::thread::sleep(Duration::from_millis(300));
    let turns = provider_turn_events(events);
    assert_eq!(turns.len(), 1, "{turns:?}");
    assert_eq!(turns[0]["aborted_by_user"], true);
    assert_eq!(turns[0]["ok"], false);
}

#[test]
fn non_streaming_interrupt_closes_the_socket_within_one_second() {
    let dir = tempfile::tempdir().expect("tempdir");
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let config = config_for(
        listener.local_addr().expect("address"),
        &dir.path().to_string_lossy(),
    );
    let events = config.eval_events_path.clone().expect("events path");
    let (started, mock) = spawn_mock(listener, None);
    let (cancel, fired_at) = interrupt_after_started(started);
    let mut client = commandagent::providers::client_from_config(&config, false).expect("client");

    let outcome = provider_call::chat_with_cancel(
        client.as_mut(),
        &config,
        provider_call::ProviderChatRequest {
            scope: ProviderCallScope::Executor,
            model: &config.model,
            messages: &[ConversationMessage::user("hello")],
            tools: &[],
            native_tools_enabled: false,
        },
        || cancel.load(Ordering::SeqCst),
    );

    assert_closed_within_budget(&mock, &fired_at, &outcome, &events);
}

#[test]
fn streaming_interrupt_closes_the_socket_within_one_second() {
    let dir = tempfile::tempdir().expect("tempdir");
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let config = config_for(
        listener.local_addr().expect("address"),
        &dir.path().to_string_lossy(),
    );
    let events = config.eval_events_path.clone().expect("events path");
    let (started, mock) = spawn_mock(
        listener,
        Some("{\"message\":{\"role\":\"assistant\",\"content\":\"partial\"},\"done\":false}\n"),
    );
    let (cancel, fired_at) = interrupt_after_started(started);
    // A planner scope streams without a TTY.
    let mut client = commandagent::providers::client_from_config(&config, true).expect("client");
    let chunks_after_cancel = Arc::new(AtomicBool::new(false));
    let observed = Arc::clone(&chunks_after_cancel);
    let cancel_for_chunks = Arc::clone(&cancel);

    let outcome = provider_call::chat_with_cancel_and_stream(
        client.as_mut(),
        &config,
        provider_call::ProviderChatRequest {
            scope: ProviderCallScope::PlannerStep,
            model: &config.planner_model,
            messages: &[ConversationMessage::user("hello")],
            tools: &[],
            native_tools_enabled: false,
        },
        || cancel.load(Ordering::SeqCst),
        &mut |_chunk| {
            if cancel_for_chunks.load(Ordering::SeqCst) {
                observed.store(true, Ordering::SeqCst);
            }
            Ok(())
        },
    );

    assert_closed_within_budget(&mock, &fired_at, &outcome, &events);
    assert!(
        !chunks_after_cancel.load(Ordering::SeqCst),
        "a chunk reached the renderer after the interrupt"
    );
}

#[test]
fn lm_studio_interrupt_closes_the_socket_within_one_second() {
    let dir = tempfile::tempdir().expect("tempdir");
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let config = config_for_provider(
        listener.local_addr().expect("address"),
        &dir.path().to_string_lossy(),
        "lm-studio",
        "--lm-studio-host",
    );
    let events = config.eval_events_path.clone().expect("events path");
    let (started, mock) = spawn_mock(listener, None);
    let (cancel, fired_at) = interrupt_after_started(started);
    let mut client = commandagent::providers::client_from_config(&config, false).expect("client");

    let outcome = provider_call::chat_with_cancel(
        client.as_mut(),
        &config,
        provider_call::ProviderChatRequest {
            scope: ProviderCallScope::Executor,
            model: &config.model,
            messages: &[ConversationMessage::user("hello")],
            tools: &[],
            native_tools_enabled: false,
        },
        || cancel.load(Ordering::SeqCst),
    );

    assert_closed_within_budget(&mock, &fired_at, &outcome, &events);
}
