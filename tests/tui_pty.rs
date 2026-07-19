use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

#[test]
#[ignore]
fn tui_pty_smoke() {
    if commandagent::env_compat::var("COMMANDAGENT_PTY_TESTS")
        .ok()
        .as_deref()
        != Some("1")
    {
        return;
    }
    if cfg!(windows) {
        return;
    }
    let bin = env!("CARGO_BIN_EXE_commandagent");
    let tmp = tempfile::tempdir().unwrap();
    let output = run_script_bsd(bin, tmp.path()).or_else(|_| run_script_linux(bin, tmp.path()));
    let output = output.expect("script(1) PTY helper must be available for release/manual UAT");
    let text = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);
    assert!(
        text.contains("commandagent>"),
        "PTY output did not contain prompt. output={text:?}"
    );
    assert!(
        text.contains("local-first agent") || text.contains("commandagent"),
        "PTY output did not contain startup banner. output={text:?}"
    );
}

#[test]
#[ignore]
fn tui_pty_queues_input_during_command_and_replays_fifo() {
    if commandagent::env_compat::var("COMMANDAGENT_PTY_TESTS")
        .ok()
        .as_deref()
        != Some("1")
    {
        return;
    }
    if cfg!(windows) {
        return;
    }
    let bin = env!("CARGO_BIN_EXE_commandagent");
    let tmp = tempfile::tempdir().unwrap();
    let state_dir = tmp.path().join("state");
    let (host, chat_started, stop, server) = start_delayed_ollama();
    let output = run_queue_script(bin, tmp.path(), &state_dir, &host, chat_started)
        .expect("script(1) PTY helper and delayed fake Ollama must be available");
    stop.store(true, Ordering::SeqCst);
    server.join().unwrap();

    let text = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "PTY command failed. output={text:?}"
    );
    for expected in [
        "queued: /help",
        "queued: /status",
        "processing queued: /help",
        "processing queued: /status",
    ] {
        assert!(
            text.contains(expected),
            "missing {expected:?}. output={text:?}"
        );
    }
    let help = text.find("processing queued: /help").unwrap();
    let status = text.find("processing queued: /status").unwrap();
    assert!(
        help < status,
        "queued commands were not FIFO. output={text:?}"
    );

    let history = std::fs::read_to_string(state_dir.join("history.txt")).unwrap();
    for expected in ["/model-probe", "/help", "/status"] {
        assert!(
            history.lines().any(|line| line == expected),
            "missing {expected:?} in history={history:?}"
        );
    }
}

#[test]
#[ignore]
fn tui_pty_streams_ollama_with_spinner_and_footer_cleanup() {
    if commandagent::env_compat::var("COMMANDAGENT_PTY_TESTS")
        .ok()
        .as_deref()
        != Some("1")
        || cfg!(windows)
    {
        return;
    }
    let bin = env!("CARGO_BIN_EXE_commandagent");
    let tmp = tempfile::tempdir().unwrap();
    let state_dir = tmp.path().join("state");
    let (host, completed, saw_stream, stop, server) = start_streaming_ollama();
    let output = run_stream_script(bin, tmp.path(), &state_dir, &host, completed)
        .expect("script(1) PTY helper and streaming fake Ollama must be available");
    stop.store(true, Ordering::SeqCst);
    server.join().unwrap();

    let text = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "PTY command failed. output={text:?}"
    );
    assert!(
        saw_stream.load(Ordering::SeqCst),
        "request did not enable stream"
    );
    let first = text
        .find(r#"{"goal":"test","#)
        .unwrap_or_else(|| panic!("first stream chunk missing. output={text:?}"));
    assert!(
        text[..first].contains("\r\x1b[2K"),
        "spinner was not cleared before body output. output={text:?}"
    );
    assert!(
        text.contains(r#""expected_result":"pass"}]}"#),
        "final stream chunk missing. output={text:?}"
    );
    assert!(
        text.contains("\x1b[r") && text.contains("commandagent>"),
        "footer/raw terminal cleanup did not restore the prompt. output={text:?}"
    );
    assert!(!text.contains("stream ended before"), "output={text:?}");
}

fn run_stream_script(
    bin: &str,
    cwd: &std::path::Path,
    state_dir: &std::path::Path,
    host: &str,
    completed: mpsc::Receiver<()>,
) -> std::io::Result<std::process::Output> {
    let command_line = queue_command_line(bin, cwd, state_dir, host);
    let mut command = std::process::Command::new("script");
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
    let mut child = command
        .env("COMMANDAGENT_NO_MARKDOWN", "1")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let stdout_reader = thread::spawn(move || read_all(stdout));
    let stderr_reader = thread::spawn(move || read_all(stderr));
    let mut stdin = child.stdin.take().unwrap();
    thread::sleep(Duration::from_secs(2));
    stdin.write_all(b"/plan-steps test\n")?;
    stdin.flush()?;
    if completed.recv_timeout(Duration::from_secs(10)).is_err() {
        let _ = child.kill();
    } else {
        thread::sleep(Duration::from_millis(500));
        stdin.write_all(b"/exit\n")?;
        stdin.flush()?;
    }
    drop(stdin);
    finish_queue_child(child, stdout_reader, stderr_reader, Duration::from_secs(20))
}

fn start_streaming_ollama() -> (
    String,
    mpsc::Receiver<()>,
    Arc<AtomicBool>,
    Arc<AtomicBool>,
    thread::JoinHandle<()>,
) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let host = format!("http://{}", listener.local_addr().unwrap());
    let stop = Arc::new(AtomicBool::new(false));
    let saw_stream = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let thread_saw_stream = Arc::clone(&saw_stream);
    let (completed_tx, completed_rx) = mpsc::sync_channel(1);
    let handle = thread::spawn(move || {
        while !thread_stop.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream
                        .set_read_timeout(Some(Duration::from_secs(1)))
                        .unwrap();
                    let request = read_http_request(&mut stream);
                    if request.starts_with("GET /api/tags ") {
                        let body = r#"{"models":[{"name":"m"}]}"#;
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        let _ = stream.write_all(response.as_bytes());
                        continue;
                    }
                    thread_saw_stream.store(request.contains(r#""stream":true"#), Ordering::SeqCst);
                    let first = serde_json::json!({
                        "message": {"role": "assistant", "content": "{\"goal\":\"test\","},
                        "done": false
                    })
                    .to_string()
                        + "\n";
                    let second = serde_json::json!({
                        "message": {"role": "assistant", "content": "\"steps\":[{\"id\":\"s1\",\"kind\":\"report\",\"instruction\":\"say done\",\"expected_paths\":[],\"verify\":[],\"expected_result\":\"pass\"}]}"},
                        "done": false
                    })
                    .to_string()
                        + "\n";
                    let terminal = "{\"done\":true}\n";
                    let total = first.len() + second.len() + terminal.len();
                    let headers = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nContent-Length: {total}\r\nConnection: close\r\n\r\n"
                    );
                    let _ = stream.write_all(headers.as_bytes());
                    let _ = stream.write_all(first.as_bytes());
                    let _ = stream.flush();
                    thread::sleep(Duration::from_millis(300));
                    let _ = stream.write_all(second.as_bytes());
                    let _ = stream.write_all(terminal.as_bytes());
                    let _ = stream.flush();
                    let _ = completed_tx.try_send(());
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });
    (host, completed_rx, saw_stream, stop, handle)
}

fn read_http_request(stream: &mut impl Read) -> String {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 1024];
    let mut expected = None;
    loop {
        let Ok(read) = stream.read(&mut buffer) else {
            break;
        };
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
        if expected.is_none()
            && let Some(header_end) = request.windows(4).position(|part| part == b"\r\n\r\n")
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
        if expected.is_some_and(|expected| request.len() >= expected) {
            break;
        }
    }
    String::from_utf8_lossy(&request).into_owned()
}

fn run_queue_script(
    bin: &str,
    cwd: &std::path::Path,
    state_dir: &std::path::Path,
    host: &str,
    chat_started: mpsc::Receiver<()>,
) -> std::io::Result<std::process::Output> {
    if cfg!(target_os = "macos") {
        run_queue_script_bsd(bin, cwd, state_dir, host, chat_started)
    } else {
        run_queue_script_linux(bin, cwd, state_dir, host, chat_started)
    }
}

fn run_queue_script_bsd(
    bin: &str,
    cwd: &std::path::Path,
    state_dir: &std::path::Path,
    host: &str,
    chat_started: mpsc::Receiver<()>,
) -> std::io::Result<std::process::Output> {
    let command_line = queue_command_line(bin, cwd, state_dir, host);
    let mut command = std::process::Command::new("script");
    command
        .arg("-q")
        .arg("/dev/null")
        .arg("/bin/sh")
        .arg("-c")
        .arg(command_line);
    run_queue_child(command, chat_started)
}

fn run_queue_script_linux(
    bin: &str,
    cwd: &std::path::Path,
    state_dir: &std::path::Path,
    host: &str,
    chat_started: mpsc::Receiver<()>,
) -> std::io::Result<std::process::Output> {
    let command_line = queue_command_line(bin, cwd, state_dir, host);
    let mut command = std::process::Command::new("script");
    command
        .arg("-q")
        .arg("-c")
        .arg(command_line)
        .arg("/dev/null");
    run_queue_child(command, chat_started)
}

fn queue_command_line(
    bin: &str,
    cwd: &std::path::Path,
    state_dir: &std::path::Path,
    host: &str,
) -> String {
    let args = queue_cli_args(cwd, state_dir, host)
        .into_iter()
        .map(|arg| shell_quote(&arg))
        .collect::<Vec<_>>()
        .join(" ");
    format!("stty rows 24 cols 120; exec {} {args}", shell_quote(bin))
}

fn queue_cli_args(cwd: &std::path::Path, state_dir: &std::path::Path, host: &str) -> Vec<String> {
    vec![
        "--yes".to_string(),
        "--cwd".to_string(),
        cwd.to_string_lossy().into_owned(),
        "--state-dir".to_string(),
        state_dir.to_string_lossy().into_owned(),
        "--provider".to_string(),
        "ollama".to_string(),
        "--model".to_string(),
        "m".to_string(),
        "--planner-provider".to_string(),
        "ollama".to_string(),
        "--planner-model".to_string(),
        "m".to_string(),
        "--ollama-host".to_string(),
        host.to_string(),
        "--chat-timeout-secs".to_string(),
        "3".to_string(),
        "--chat-retries".to_string(),
        "0".to_string(),
    ]
}

fn run_queue_child(
    mut command: std::process::Command,
    chat_started: mpsc::Receiver<()>,
) -> std::io::Result<std::process::Output> {
    let mut child = command
        .env("COMMANDAGENT_NO_SPINNER", "1")
        .env("COMMANDAGENT_NO_MARKDOWN", "1")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let stdout_reader = thread::spawn(move || read_all(stdout));
    let stderr_reader = thread::spawn(move || read_all(stderr));
    let mut stdin = child.stdin.take().unwrap();
    thread::sleep(Duration::from_secs(2));
    stdin.write_all(b"/model-probe\n")?;
    stdin.flush()?;
    if chat_started.recv_timeout(Duration::from_secs(10)).is_err() {
        drop(stdin);
        let _ = child.kill();
        let output = finish_queue_child(child, stdout_reader, stderr_reader, Duration::ZERO)?;
        let text = String::from_utf8_lossy(&output.stdout).to_string()
            + &String::from_utf8_lossy(&output.stderr);
        return Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            format!("model probe did not reach fake Ollama. output={text:?}"),
        ));
    }
    for line in [b"/help\r".as_slice(), b"/status\r", b"/exit\r"] {
        stdin.write_all(line)?;
        stdin.flush()?;
        thread::sleep(Duration::from_millis(120));
    }
    drop(stdin);
    finish_queue_child(child, stdout_reader, stderr_reader, Duration::from_secs(30))
}

fn read_all(mut input: impl Read) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    input.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn finish_queue_child(
    mut child: std::process::Child,
    stdout_reader: thread::JoinHandle<std::io::Result<Vec<u8>>>,
    stderr_reader: thread::JoinHandle<std::io::Result<Vec<u8>>>,
    timeout: Duration,
) -> std::io::Result<std::process::Output> {
    let deadline = std::time::Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            break child.wait()?;
        }
        thread::sleep(Duration::from_millis(20));
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| std::io::Error::other("stdout reader panicked"))??;
    let stderr = stderr_reader
        .join()
        .map_err(|_| std::io::Error::other("stderr reader panicked"))??;
    Ok(std::process::Output {
        status,
        stdout,
        stderr,
    })
}

fn start_delayed_ollama() -> (
    String,
    mpsc::Receiver<()>,
    Arc<AtomicBool>,
    thread::JoinHandle<()>,
) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let host = format!("http://{}", listener.local_addr().unwrap());
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = stop.clone();
    let (started_tx, started_rx) = mpsc::sync_channel(1);
    let handle = thread::spawn(move || {
        let mut first_chat = true;
        while !thread_stop.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream
                        .set_read_timeout(Some(Duration::from_secs(1)))
                        .unwrap();
                    let mut request = Vec::new();
                    let mut buffer = [0_u8; 1024];
                    while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                        let Ok(read) = stream.read(&mut buffer) else {
                            break;
                        };
                        if read == 0 {
                            break;
                        }
                        request.extend_from_slice(&buffer[..read]);
                    }
                    let request = String::from_utf8_lossy(&request);
                    let body = if request.starts_with("GET /api/tags ") {
                        r#"{"models":[{"name":"m"}]}"#
                    } else {
                        if first_chat {
                            first_chat = false;
                            let _ = started_tx.try_send(());
                            thread::sleep(Duration::from_millis(700));
                        }
                        r#"{"message":{"role":"assistant","content":"probe response"}}"#
                    };
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = stream.write_all(response.as_bytes());
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });
    (host, started_rx, stop, handle)
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn run_script_bsd(bin: &str, cwd: &std::path::Path) -> std::io::Result<std::process::Output> {
    let mut child = std::process::Command::new("script")
        .arg("-q")
        .arg("/dev/null")
        .arg(bin)
        .arg("--yes")
        .arg("--cwd")
        .arg(cwd)
        .arg("--no-footer")
        .env("COMMANDAGENT_NO_SPINNER", "1")
        .env("COMMANDAGENT_NO_INTERRUPT", "1")
        .env("COMMANDAGENT_NO_MARKDOWN", "1")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    child.stdin.as_mut().unwrap().write_all(b"/exit\n")?;
    child.wait_with_output()
}

fn run_script_linux(bin: &str, cwd: &std::path::Path) -> std::io::Result<std::process::Output> {
    let command = format!("{} --yes --cwd {} --no-footer", bin, cwd.to_string_lossy());
    let mut child = std::process::Command::new("script")
        .arg("-q")
        .arg("-c")
        .arg(command)
        .arg("/dev/null")
        .env("COMMANDAGENT_NO_SPINNER", "1")
        .env("COMMANDAGENT_NO_INTERRUPT", "1")
        .env("COMMANDAGENT_NO_MARKDOWN", "1")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    child.stdin.as_mut().unwrap().write_all(b"/exit\n")?;
    child.wait_with_output()
}
