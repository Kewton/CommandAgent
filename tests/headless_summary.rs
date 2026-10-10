use std::process::Command;

#[cfg(unix)]
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, RecvTimeoutError, Sender},
    },
    time::Duration,
};

#[test]
fn omitted_flag_preserves_stdout_bytes() {
    let workspace = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .args(["--runs", "--cwd", workspace.path().to_str().unwrap()])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(
        output.stdout,
        include_bytes!("fixtures/summary-json-omitted.stdout")
    );
}

// A read-only action with no persisted run and no selected pack has nothing to
// project, so `--summary-json` must leave its stdout bytes unchanged.
#[test]
fn runs_summary_json_is_suppressed_without_run_or_pack() {
    let workspace = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .args([
            "--runs",
            "--summary-json",
            "--cwd",
            workspace.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(
        output.stdout,
        include_bytes!("fixtures/summary-json-omitted.stdout")
    );
    assert!(
        !String::from_utf8_lossy(&output.stdout).contains("commandagent.headless-summary/v1"),
        "a run-less action with no evidence must not project a headless summary"
    );
}

#[test]
fn ux_demo_summary_json_is_suppressed_without_recording_a_run() {
    let workspace = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .args([
            "--ux-demo",
            "--summary-json",
            "--no-footer",
            "--cwd",
            workspace.path().to_str().unwrap(),
        ])
        .env("COMMANDAGENT_UX_DEMO_FAST", "1")
        .output()
        .unwrap();

    assert!(output.status.success(), "{:?}", output.status);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("commandagent.headless-summary/v1"),
        "ux-demo must not emit a headless summary: {stdout}"
    );
}

#[test]
fn requested_summary_is_the_final_stdout_line_even_on_failure() {
    let workspace = tempfile::tempdir().unwrap();
    let state = workspace.path().join("state");
    std::fs::create_dir_all(&state).unwrap();
    let events = workspace.path().join("run-evidence/events.jsonl");
    let output = Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .args([
            "--offline",
            "--provider",
            "openai",
            "--model",
            "gpt-5.6-luna",
            "--planner-provider",
            "openai",
            "--planner-model",
            "gpt-5.6-luna",
            "--prompt",
            "hello",
            "--cwd",
            workspace.path().to_str().unwrap(),
            "--state-dir",
            state.to_str().unwrap(),
            "--no-footer",
            "--summary-json",
        ])
        .env("COMMANDAGENT_EVAL_EVENTS", &events)
        .env_remove("OPENAI_API_KEY")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let last = stdout.lines().last().expect("summary line");
    let summary: serde_json::Value = serde_json::from_str(last).unwrap();
    assert_eq!(
        summary["schema_version"],
        "commandagent.headless-summary/v1"
    );
    assert_eq!(summary["run_id"], "run-evidence");
    assert_eq!(summary["events_path"], events.display().to_string());
    assert_eq!(summary["verdict"], "reduced");
    assert_eq!(summary["assurance"], "reduced");
    assert_eq!(summary["stop_class"], "direct_cli_command_failed");
    assert_eq!(summary["provider_usage_by_role"], serde_json::json!({}));
}

// A held mock Ollama endpoint. It answers startup probes and keeps the target
// chat request open so the real SIGINT lands while the provider call is in
// flight. The guard reclaims the child and the serving thread even when an
// assertion fails.
#[cfg(unix)]
struct MockProvider {
    child: Option<std::process::Child>,
    server: Option<std::thread::JoinHandle<()>>,
    shutdown: Arc<AtomicBool>,
}

#[cfg(unix)]
impl MockProvider {
    fn child_mut(&mut self) -> &mut std::process::Child {
        self.child.as_mut().expect("child process")
    }

    fn take_child(&mut self) -> std::process::Child {
        self.child.take().expect("child process")
    }

    fn stop_server(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        if let Some(server) = self.server.take() {
            let _ = server.join();
        }
    }
}

#[cfg(unix)]
impl Drop for MockProvider {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.stop_server();
    }
}

#[cfg(unix)]
fn header_end(buffer: &[u8]) -> Option<usize> {
    buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n".as_slice())
}

#[cfg(unix)]
fn content_length(headers: &[u8]) -> usize {
    for line in String::from_utf8_lossy(headers).lines() {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case("content-length") {
            return value.trim().parse().unwrap_or(0);
        }
    }
    0
}

// Returns the full request, or a diagnostic when the connection ended or timed
// out before a complete request arrived.
#[cfg(unix)]
fn read_http_request(stream: &mut TcpStream) -> Result<String, String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|err| format!("could not set the mock read timeout: {err}"))?;
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => {
                return Err(format!(
                    "the connection closed after {} bytes; no complete request arrived",
                    buffer.len()
                ));
            }
            Ok(read) => buffer.extend_from_slice(&chunk[..read]),
            Err(err)
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                return Err(format!(
                    "only {} bytes of an incomplete HTTP request arrived before the timeout",
                    buffer.len()
                ));
            }
            Err(err) => return Err(format!("reading the HTTP request failed: {err}")),
        }
        if let Some(end) = header_end(&buffer)
            && buffer.len() - (end + 4) >= content_length(&buffer[..end])
        {
            return Ok(String::from_utf8_lossy(&buffer).into_owned());
        }
    }
}

// Keep the response body open so the provider call stays in flight until the
// child exits or the guard asks the server to stop.
#[cfg(unix)]
fn hold_chat_open(stream: &mut TcpStream, shutdown: &AtomicBool) {
    let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
    let mut buffer = [0u8; 256];
    loop {
        if shutdown.load(Ordering::Acquire) {
            return;
        }
        match stream.read(&mut buffer) {
            Ok(0) => return,
            Ok(_) => {}
            Err(err)
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(_) => return,
        }
    }
}

#[cfg(unix)]
fn spawn_mock_provider(
    listener: TcpListener,
    started: Sender<Result<(), String>>,
) -> (std::thread::JoinHandle<()>, Arc<AtomicBool>) {
    let shutdown = Arc::new(AtomicBool::new(false));
    let server_shutdown = shutdown.clone();
    let server = std::thread::spawn(move || {
        let _ = listener.set_nonblocking(true);
        loop {
            if server_shutdown.load(Ordering::Acquire) {
                return;
            }
            let (mut stream, _) = match listener.accept() {
                Ok(connection) => connection,
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(_) => return,
            };
            let _ = stream.set_nonblocking(false);
            let request = match read_http_request(&mut stream) {
                Ok(request) => request,
                Err(diagnostic) => {
                    let _ = started.send(Err(diagnostic));
                    continue;
                }
            };
            if request.starts_with("POST /api/chat ") {
                let _ = started.send(Ok(()));
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nTransfer-Encoding: chunked\r\n\r\n"
                );
                let _ = stream.flush();
                hold_chat_open(&mut stream, &server_shutdown);
                return;
            }
            if request.starts_with("GET /api/tags ") {
                let body = r#"{"models":[{"name":"test-model"}]}"#;
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
            } else {
                let _ = write!(
                    stream,
                    "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
            }
            let _ = stream.flush();
        }
    });
    (server, shutdown)
}

#[cfg(unix)]
#[test]
fn sigint_emits_interrupted_summary_as_the_final_stdout_line() {
    let workspace = tempfile::tempdir().unwrap();
    let state = workspace.path().join("state");
    std::fs::create_dir_all(&state).unwrap();
    let events = workspace.path().join("run-evidence/events.jsonl");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let ollama_host = format!("http://{}", listener.local_addr().unwrap());
    let (started_tx, started_rx) = mpsc::channel();
    let (server, shutdown) = spawn_mock_provider(listener, started_tx);

    let child = Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .args([
            "--provider",
            "ollama",
            "--model",
            "test-model",
            "--planner-provider",
            "ollama",
            "--planner-model",
            "test-model",
            "--ollama-host",
            &ollama_host,
            "--chat-timeout-secs",
            "30",
            "--prompt",
            "hello",
            "--cwd",
            workspace.path().to_str().unwrap(),
            "--state-dir",
            state.to_str().unwrap(),
            "--yes",
            "--no-footer",
            "--summary-json",
        ])
        .env("COMMANDAGENT_EVAL_EVENTS", &events)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut run = MockProvider {
        child: Some(child),
        server: Some(server),
        shutdown,
    };

    // SIGINT is only sent after the mock provider has received the target chat
    // request. A finite wait keeps a missing ready signal a failure, never a
    // pass on normal exit or timeout.
    let ready_deadline = std::time::Instant::now() + Duration::from_secs(15);
    loop {
        match started_rx.recv_timeout(Duration::from_millis(50)) {
            Ok(Ok(())) => break,
            Ok(Err(diagnostic)) => {
                panic!("the chat request never completed at the mock provider: {diagnostic}");
            }
            Err(RecvTimeoutError::Timeout) => {
                if let Some(status) = run.child_mut().try_wait().unwrap() {
                    panic!("commandagent exited before the chat request started: {status}");
                }
                assert!(
                    std::time::Instant::now() < ready_deadline,
                    "timed out waiting for the chat request to start"
                );
            }
            Err(RecvTimeoutError::Disconnected) => {
                panic!("the mock provider stopped before the chat request arrived");
            }
        }
    }
    assert!(
        events.is_file(),
        "run evidence must exist before SIGINT is delivered"
    );

    let signal_result = unsafe { libc::kill(run.child_mut().id() as libc::pid_t, libc::SIGINT) };
    assert_eq!(signal_result, 0, "failed to send SIGINT");

    let exit_deadline = std::time::Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = run.child_mut().try_wait().unwrap() {
            break status;
        }
        assert!(
            std::time::Instant::now() < exit_deadline,
            "commandagent did not exit after SIGINT"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    let output = run.take_child().wait_with_output().unwrap();
    run.stop_server();

    assert_eq!(status.code(), Some(130), "{status}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let last = stdout.lines().last().expect("summary line");
    let summary: serde_json::Value = serde_json::from_str(last).unwrap();
    assert_eq!(
        summary["schema_version"],
        "commandagent.headless-summary/v1"
    );
    assert_eq!(summary["status"], "interrupted");
    assert_eq!(summary["exit_code"], 130);
    assert_eq!(summary["stop_class"], "direct_cli_command_interrupted");
    assert_eq!(summary["events_path"], events.display().to_string());

    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("\"status\":\"interrupted\""));
    assert!(event_text.contains("\"failure_kind\":\"direct_cli_command_interrupted\""));
}
