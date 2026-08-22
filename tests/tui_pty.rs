use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

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
    for expected in [
        "start: plain-text request → review Gate 1 → /confirm <hash> | help: /help",
        "D-3c Gate 1 confirmation is required before execution. Start with a plain-text request, review the Gate 1 card, then enter /confirm <hash>.",
    ] {
        assert!(
            text.contains(expected),
            "PTY output did not contain stable first-run guidance {expected:?}. output={text:?}"
        );
    }
}

#[test]
#[ignore]
fn tui_pty_warns_when_ollama_is_stopped_and_keeps_prompt_usable() {
    if commandagent::env_compat::var("COMMANDAGENT_PTY_TESTS")
        .ok()
        .as_deref()
        != Some("1")
        || cfg!(windows)
    {
        return;
    }
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let host = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let tmp = tempfile::tempdir().unwrap();

    let output = run_startup_diagnostic_script(
        env!("CARGO_BIN_EXE_commandagent"),
        tmp.path(),
        &tmp.path().join("state"),
        &host,
        "missing:latest",
    )
    .expect("script(1) PTY helper must be available");
    let text = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success(), "output={text:?}");
    for expected in [
        "warning: Ollama is unreachable",
        host.as_str(),
        "ollama serve",
        "--ollama-host",
        "commandagent --doctor",
        "continuing.",
        "commandagent>",
    ] {
        assert!(
            text.contains(expected),
            "missing {expected:?}. output={text:?}"
        );
    }
}

#[test]
#[ignore]
fn tui_pty_warns_when_ollama_model_is_missing_and_keeps_prompt_usable() {
    if commandagent::env_compat::var("COMMANDAGENT_PTY_TESTS")
        .ok()
        .as_deref()
        != Some("1")
        || cfg!(windows)
    {
        return;
    }
    let (host, server) = start_tags_only_ollama();
    let tmp = tempfile::tempdir().unwrap();

    let output = run_startup_diagnostic_script(
        env!("CARGO_BIN_EXE_commandagent"),
        tmp.path(),
        &tmp.path().join("state"),
        &host,
        "missing:latest",
    )
    .expect("script(1) PTY helper and fake Ollama must be available");
    server.join().unwrap();
    let text = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success(), "output={text:?}");
    for expected in [
        "warning: Ollama model `missing:latest` is not installed",
        host.as_str(),
        "ollama pull missing:latest",
        "commandagent --doctor",
        "commandagent>",
    ] {
        assert!(
            text.contains(expected),
            "missing {expected:?}. output={text:?}"
        );
    }
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
fn tui_pty_suppresses_planner_stream_with_spinner_and_footer_cleanup() {
    if commandagent::env_compat::var("COMMANDAGENT_PTY_TESTS")
        .ok()
        .as_deref()
        != Some("1")
        || cfg!(windows)
    {
        return;
    }
    let bin = env!("CARGO_BIN_EXE_commandagent");
    for (command, started, completed, response_count) in [
        ("/plan-steps test", "planning steps", "step plan ready", 1),
        (
            "/ultra-plan-run test",
            "planning the overall plan",
            "overall plan ready",
            3,
        ),
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let state_dir = tmp.path().join("state");
        let StreamingOllama {
            host,
            started: _started,
            completed: response_completed,
            disconnected: _disconnected,
            saw_stream,
            stop,
            server,
        } = start_streaming_ollama(response_count);
        let output = run_stream_script(
            bin,
            tmp.path(),
            &state_dir,
            &host,
            response_completed,
            command,
            false,
        )
        .expect("script(1) PTY helper and streaming fake Ollama must be available");
        stop.store(true, Ordering::SeqCst);
        server.join().unwrap();

        let text = String::from_utf8_lossy(&output.stdout).to_string()
            + &String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "PTY command failed for {command}. output={text:?}"
        );
        assert!(
            saw_stream.load(Ordering::SeqCst),
            "request did not enable stream for {command}"
        );
        assert!(
            !text.contains(r#"{"goal":"test","#),
            "planner stream JSON reached the terminal for {command}. output={text:?}"
        );
        assert!(
            !text.contains(r#""expected_result":"pass"}]}"#),
            "planner stream tail reached the terminal for {command}. output={text:?}"
        );
        assert!(
            text.contains(started) && text.contains(completed),
            "planner breadcrumbs were not preserved for {command}. output={text:?}"
        );
        assert!(
            text.contains("\r\x1b[2K"),
            "spinner was not cleared after {command}. output={text:?}"
        );
        assert!(
            text.contains("\x1b[r") && text.contains("commandagent>"),
            "footer/raw terminal cleanup did not restore the prompt after {command}. output={text:?}"
        );
        assert!(!text.contains("stream ended before"), "output={text:?}");
    }
}

#[test]
#[ignore]
fn tui_pty_planner_stream_interrupt_cleans_spinner_footer_and_status() {
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
    let StreamingOllama {
        host,
        started,
        completed: _completed,
        disconnected: _disconnected,
        saw_stream,
        stop,
        server,
    } = start_streaming_ollama(1);
    let output = run_stream_script(
        bin,
        tmp.path(),
        &state_dir,
        &host,
        started,
        "/plan-steps interrupt-test",
        true,
    )
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
    assert!(
        !text.contains(r#"{"goal":"test","#) && !text.contains(r#""expected_result":"pass"}]}"#),
        "interrupted planner stream reached the terminal. output={text:?}"
    );
    assert!(text.contains("planning steps"), "output={text:?}");
    assert!(
        text.contains("INTERRUPTED") && !text.contains("step plan ready"),
        "Esc did not interrupt the in-flight planner turn. output={text:?}"
    );
    assert!(
        text.contains("\r\x1b[2K"),
        "spinner was not cleared after Esc. output={text:?}"
    );
    assert!(
        text.contains("\x1b[r") && text.contains("commandagent>"),
        "footer/raw terminal cleanup did not restore the prompt after Esc. output={text:?}"
    );
    let status_output = text
        .rsplit_once("### Status")
        .map(|(_, suffix)| suffix)
        .unwrap_or_else(|| panic!("post-interrupt /status output was missing. output={text:?}"));
    assert!(
        !status_output.contains("[stopping: aborting current operation"),
        "stopping footer survived into the post-interrupt /status command. output={text:?}"
    );
    assert!(
        !status_output.contains("Current scope: interrupt requested"),
        "interrupt scope survived into the post-interrupt /status command. output={text:?}"
    );
}

#[test]
#[ignore]
fn tui_pty_planner_interrupt_closes_http_and_reaches_gate_four_within_one_second() {
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
    let StreamingOllama {
        host,
        started,
        completed: _completed,
        disconnected,
        saw_stream,
        stop,
        server,
    } = start_streaming_ollama(1);
    let (output, gate_four_elapsed, disconnect_elapsed) =
        run_gate_four_interrupt_script(bin, tmp.path(), &state_dir, &host, started, disconnected)
            .expect("script(1) PTY helper and interruptible fake Ollama must be available");
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
        "planner request did not enable stream with --stream off"
    );
    assert!(
        gate_four_elapsed < Duration::from_secs(1),
        "Gate 4 took {gate_four_elapsed:?} after Esc. output={text:?}"
    );
    assert!(
        disconnect_elapsed < Duration::from_secs(1),
        "Ollama HTTP connection stayed open for {disconnect_elapsed:?} after Esc"
    );
    assert!(
        text.contains("Gate 4") && text.to_ascii_lowercase().contains("interrupted"),
        "honest interrupted Gate 4 was not rendered. output={text:?}"
    );
    assert!(
        !text.contains(r#"{"goal":"test","#),
        "planner stream payload reached the terminal. output={text:?}"
    );
}

#[test]
#[ignore]
fn tui_pty_screen_state_preserves_long_accepted_goal_across_footer_modes() {
    if commandagent::env_compat::var("COMMANDAGENT_PTY_TESTS")
        .ok()
        .as_deref()
        != Some("1")
        || cfg!(windows)
    {
        return;
    }
    let bin = env!("CARGO_BIN_EXE_commandagent");
    for (footer, no_color) in [(true, false), (true, true), (false, false), (false, true)] {
        let tmp = tempfile::tempdir().unwrap();
        let state_dir = tmp.path().join("state");
        let (host, completed, stop, server) = start_ultra_plan_ollama();
        let output = run_receipt_screen_script(
            bin,
            tmp.path(),
            &state_dir,
            &host,
            completed,
            footer,
            no_color,
        )
        .expect("script(1) PTY helper and fake Ollama must be available");
        stop.store(true, Ordering::SeqCst);
        server.join().unwrap();

        let text = String::from_utf8_lossy(&output.stdout).to_string()
            + &String::from_utf8_lossy(&output.stderr);
        let visible = normalize_screen_text(&text);
        assert!(
            output.status.success(),
            "PTY command failed (footer={footer}, no_color={no_color}). output={text:?}"
        );
        for expected in [
            "Unknown command: /hepl",
            "Did you mean /help?",
            "Input was not run: 日本語の自由文",
            "Use /ultra-plan-run <goal> or /plan-run <goal>.",
            "Accepted command",
            "- Command: /ultra-plan-run",
            "- Profile: nextjs (explicit)",
            "- Style: compact (explicit)",
            "- Prompt layout: stable (explicit)",
            "- Requested port: 3011 (goal)",
            "Active command: /ultra-plan-run",
            "── Phase 1/2: game-engine ──",
            "Current phase:",
            "TASK FAILED",
            "Primary stop reason:",
        ] {
            assert!(
                visible.contains(expected),
                "missing {expected:?} (footer={footer}, no_color={no_color}). visible={visible:?} raw={text:?}"
            );
        }
        assert_eq!(
            visible.matches("Unknown command: /hepl").count(),
            1,
            "typo guidance was duplicated (footer={footer}, no_color={no_color}). visible={visible:?}"
        );
        assert_eq!(
            visible.matches("TASK FAILED").count(),
            1,
            "real failure was duplicated (footer={footer}, no_color={no_color}). visible={visible:?}"
        );
        assert!(!visible.contains("Terminal summary"), "visible={visible:?}");
        assert!(!visible.contains("error:"), "visible={visible:?}");
        assert!(
            visible.contains("あなたが考える最高に面白くかっこいいスペースインベーダーゲームを")
                && visible.contains("3011番ポートで作ってください"),
            "long CJK Goal was not preserved (footer={footer}, no_color={no_color}). visible={visible:?}"
        );
        assert_receipt_cursor_columns(&text, 72, footer, no_color);
        assert_eq!(
            text.contains("\x1b[1;"),
            footer,
            "footer scroll region mode mismatch. output={text:?}"
        );
        assert!(
            text.contains("\x1b]2;CommandAgent — Phase 1/2: game-engine\x07"),
            "phase title OSC 2 was missing (footer={footer}). output={text:?}"
        );
        assert!(
            text.contains("\x1b]2;\x07"),
            "empty title OSC 2 cleanup was missing (footer={footer}). output={text:?}"
        );
        if no_color {
            assert!(
                !text.contains("\x1b[2m"),
                "NO_COLOR emitted dim SGR: {text:?}"
            );
        }
    }
}

fn run_receipt_screen_script(
    bin: &str,
    cwd: &std::path::Path,
    state_dir: &std::path::Path,
    host: &str,
    completed: mpsc::Receiver<()>,
    footer: bool,
    no_color: bool,
) -> std::io::Result<std::process::Output> {
    let mut args = queue_cli_args(cwd, state_dir, host);
    args.extend(["--stream".to_string(), "off".to_string()]);
    if !footer {
        args.push("--no-footer".to_string());
    }
    let args = args
        .into_iter()
        .map(|arg| shell_quote(&arg))
        .collect::<Vec<_>>()
        .join(" ");
    let command_line = format!(
        "stty rows 18 cols 48; (sleep 2.2; stty rows 22 cols 72 </dev/tty) & exec {} {args}",
        shell_quote(bin)
    );
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
    if no_color {
        command.env("NO_COLOR", "1");
    }
    let mut child = command
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
    stdin.write_all(b"/hepl\r")?;
    stdin.flush()?;
    thread::sleep(Duration::from_millis(250));
    stdin.write_all("日本語の自由文\r".as_bytes())?;
    stdin.flush()?;
    thread::sleep(Duration::from_millis(250));
    stdin.write_all(
        "/ultra-plan-run --profile nextjs --style compact --prompt-layout stable \"あなたが考える最高に面白くかっこいいスペースインベーダーゲームを、CJKの長い説明を保ったまま3011番ポートで作ってください\"\n"
            .as_bytes(),
    )?;
    stdin.flush()?;
    if completed.recv_timeout(Duration::from_secs(10)).is_err() {
        let _ = child.kill();
    } else {
        thread::sleep(Duration::from_secs(1));
        stdin.write_all(b"/status\r")?;
        stdin.flush()?;
        thread::sleep(Duration::from_secs(1));
        stdin.write_all(b"/exit\r")?;
        stdin.flush()?;
    }
    drop(stdin);
    finish_queue_child(child, stdout_reader, stderr_reader, Duration::from_secs(20))
}

fn start_ultra_plan_ollama() -> (
    String,
    mpsc::Receiver<()>,
    Arc<AtomicBool>,
    thread::JoinHandle<()>,
) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let host = format!("http://{}", listener.local_addr().unwrap());
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let (completed_tx, completed_rx) = mpsc::sync_channel(1);
    let handle = thread::spawn(move || {
        let mut chat_count = 0usize;
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
                    chat_count += 1;
                    thread::sleep(Duration::from_millis(700));
                    let content = concat!(
                        "goal: \"あなたが考える最高に面白くかっこいいスペースインベーダーゲームを、CJKの長い説明を保ったまま3011番ポートで作ってください\"\n",
                        "profile: \"nextjs\"\n",
                        "style: \"compact\"\n",
                        "intent: \"create\"\n",
                        "phases:\n",
                        "  - id: \"game-engine\"\n",
                        "    prompt: \"Implement the game engine\"\n",
                        "  - id: \"verify\"\n",
                        "    prompt: \"Verify the game\"\n"
                    );
                    let body = serde_json::json!({
                        "message": {"role": "assistant", "content": content},
                        "done": true
                    })
                    .to_string();
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = stream.write_all(response.as_bytes());
                    let _ = stream.flush();
                    if chat_count == 4 {
                        let _ = completed_tx.try_send(());
                    }
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });
    (host, completed_rx, stop, handle)
}

fn normalize_screen_text(value: &str) -> String {
    let mut plain = String::new();
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            if chars.peek() == Some(&'[') {
                let _ = chars.next();
                for next in chars.by_ref() {
                    if ('@'..='~').contains(&next) {
                        break;
                    }
                }
            }
            continue;
        }
        if ch == '\r' {
            plain.push('\n');
        } else if ch == '\n' || !ch.is_control() {
            plain.push(ch);
        }
    }
    plain.lines().map(str::trim_start).collect::<String>()
}

fn assert_receipt_cursor_columns(
    transcript: &str,
    terminal_cols: usize,
    footer: bool,
    no_color: bool,
) {
    const FIELDS: [&str; 8] = [
        "- Input: ",
        "- Command: ",
        "- Goal: ",
        "- Profile: ",
        "- Style: ",
        "- Prompt layout: ",
        "- Requested port: ",
        "- Run ID: ",
    ];

    let receipt_start = transcript
        .find("Accepted command")
        .unwrap_or_else(|| panic!("accepted receipt missing. output={transcript:?}"));
    let run_id_start = receipt_start
        + transcript[receipt_start..]
            .find("- Run ID: ")
            .unwrap_or_else(|| panic!("receipt run ID missing. output={transcript:?}"));
    let receipt_end = transcript[run_id_start..]
        .find('\n')
        .map_or(transcript.len(), |offset| run_id_start + offset + 1);
    let receipt = &transcript[receipt_start..receipt_end];
    let mut field_seen = [false; FIELDS.len()];
    let mut continuation_seen = [false; FIELDS.len()];
    let mut continuation_indent = None;
    let mut current_field = None;
    let mut line_offset = 0;

    for raw_line in receipt.split_inclusive('\n') {
        let line = raw_line
            .strip_suffix('\n')
            .unwrap_or(raw_line)
            .strip_suffix('\r')
            .unwrap_or(raw_line.strip_suffix('\n').unwrap_or(raw_line));
        let leading_spaces = line.bytes().take_while(|byte| *byte == b' ').count();
        let content = &line[leading_spaces..];
        assert!(!content.is_empty(), "empty receipt line in {receipt:?}");

        let content_index = receipt_start + line_offset + leading_spaces;
        let actual_column = cursor_column_at(transcript, content_index, terminal_cols);
        assert_eq!(
            actual_column, leading_spaces,
            "receipt line started in the wrong terminal column (footer={footer}, no_color={no_color}, line={line:?}, expected_column={leading_spaces}, actual_column={actual_column}). output={transcript:?}"
        );

        if content == "Accepted command" {
            assert_eq!(leading_spaces, 0, "receipt heading was indented: {line:?}");
        } else if let Some((index, prefix)) = FIELDS
            .iter()
            .enumerate()
            .find(|(_, prefix)| content.starts_with(*prefix))
        {
            assert_eq!(
                leading_spaces, 0,
                "top-level receipt field was indented: {line:?}"
            );
            field_seen[index] = true;
            current_field = Some(index);
            continuation_indent = Some(commandagent::util::display_width(prefix));
        } else {
            let expected_indent = continuation_indent
                .unwrap_or_else(|| panic!("orphaned receipt continuation: {line:?}"));
            assert_eq!(
                leading_spaces, expected_indent,
                "receipt continuation had the wrong intentional indentation: {line:?}"
            );
            continuation_seen[current_field.expect("continuation field is set")] = true;
        }
        line_offset += raw_line.len();
    }

    for (index, field) in FIELDS.iter().enumerate() {
        assert!(
            field_seen[index],
            "missing top-level receipt field {field:?} (footer={footer}, no_color={no_color}). receipt={receipt:?}"
        );
    }
    for index in [0, 2] {
        assert!(
            continuation_seen[index],
            "long CJK receipt field did not exercise continuation indentation for {:?} (footer={footer}, no_color={no_color}). receipt={receipt:?}",
            FIELDS[index]
        );
    }
}

fn cursor_column_at(transcript: &str, target: usize, terminal_cols: usize) -> usize {
    assert!(transcript.is_char_boundary(target));
    assert!(terminal_cols > 0);
    let bytes = transcript.as_bytes();
    let mut index = 0;
    let mut column = 0;
    let mut saved_column = 0;

    while index < target {
        if bytes[index] == b'\x1b' {
            if bytes.get(index + 1) == Some(&b'[') {
                let Some(final_offset) = bytes[index + 2..]
                    .iter()
                    .position(|byte| (b'@'..=b'~').contains(byte))
                else {
                    break;
                };
                let final_index = index + 2 + final_offset;
                let parameters = &transcript[index + 2..final_index];
                match bytes[final_index] {
                    b'H' | b'f' => {
                        column = csi_parameter(parameters, 1, 1).saturating_sub(1);
                    }
                    b'G' | b'`' => {
                        column = csi_parameter(parameters, 0, 1).saturating_sub(1);
                    }
                    b'C' | b'a' => {
                        column = column.saturating_add(csi_parameter(parameters, 0, 1));
                    }
                    b'D' => {
                        column = column.saturating_sub(csi_parameter(parameters, 0, 1));
                    }
                    b'E' | b'F' => column = 0,
                    b's' => saved_column = column,
                    b'u' => column = saved_column,
                    _ => {}
                }
                index = final_index + 1;
                continue;
            }
            if bytes.get(index + 1) == Some(&b']') {
                index += 2;
                while index < target {
                    if bytes[index] == b'\x07' {
                        index += 1;
                        break;
                    }
                    if bytes[index] == b'\x1b' && bytes.get(index + 1) == Some(&b'\\') {
                        index += 2;
                        break;
                    }
                    index += 1;
                }
                continue;
            }
            index = (index + 2).min(bytes.len());
            continue;
        }

        let ch = transcript[index..]
            .chars()
            .next()
            .expect("index remains on a UTF-8 boundary");
        match ch {
            '\r' => column = 0,
            '\n' => {}
            '\u{8}' => column = column.saturating_sub(1),
            '\t' => column = ((column / 8) + 1) * 8,
            ch if !ch.is_control() => {
                column = (column + commandagent::util::char_display_width(ch)) % terminal_cols;
            }
            _ => {}
        }
        index += ch.len_utf8();
    }
    column
}

fn csi_parameter(parameters: &str, index: usize, default: usize) -> usize {
    parameters
        .split(';')
        .nth(index)
        .and_then(|value| {
            value
                .trim_start_matches(|ch: char| !ch.is_ascii_digit())
                .parse()
                .ok()
        })
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn run_stream_script(
    bin: &str,
    cwd: &std::path::Path,
    state_dir: &std::path::Path,
    host: &str,
    signal: mpsc::Receiver<()>,
    planner_command: &str,
    interrupt: bool,
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
    stdin.write_all(format!("{planner_command}\n").as_bytes())?;
    stdin.flush()?;
    if signal.recv_timeout(Duration::from_secs(10)).is_err() {
        let _ = child.kill();
    } else {
        if interrupt {
            stdin.write_all(b"\x1b")?;
            stdin.flush()?;
            thread::sleep(Duration::from_millis(500));
            stdin.write_all(b"/status\n")?;
            stdin.flush()?;
            thread::sleep(Duration::from_secs(1));
        }
        thread::sleep(Duration::from_millis(500));
        stdin.write_all(b"/exit\n")?;
        stdin.flush()?;
    }
    drop(stdin);
    finish_queue_child(child, stdout_reader, stderr_reader, Duration::from_secs(20))
}

fn run_gate_four_interrupt_script(
    bin: &str,
    cwd: &std::path::Path,
    state_dir: &std::path::Path,
    host: &str,
    started: mpsc::Receiver<()>,
    disconnected: mpsc::Receiver<()>,
) -> std::io::Result<(std::process::Output, Duration, Duration)> {
    let mut args = queue_cli_args(cwd, state_dir, host);
    args.extend(["--stream".to_string(), "off".to_string()]);
    let args = args
        .into_iter()
        .map(|arg| shell_quote(&arg))
        .collect::<Vec<_>>()
        .join(" ");
    let command_line = format!("stty rows 24 cols 120; exec {} {args}", shell_quote(bin));
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
    stdin.write_all(b"Fix a Next.js compile error\n")?;
    stdin.flush()?;

    let transcript = state_dir.join("boundary-transcript.md");
    let card_hash = match wait_for_gate_one_hash(&transcript, Duration::from_secs(10)) {
        Ok(card_hash) => card_hash,
        Err(error) => {
            let _ = child.kill();
            drop(stdin);
            let _ = finish_queue_child(child, stdout_reader, stderr_reader, Duration::from_secs(1));
            return Err(error);
        }
    };
    thread::sleep(Duration::from_millis(500));
    stdin.write_all(format!("/confirm {card_hash}\n").as_bytes())?;
    stdin.flush()?;
    if started.recv_timeout(Duration::from_secs(10)).is_err() {
        let _ = child.kill();
        drop(stdin);
        let output =
            finish_queue_child(child, stdout_reader, stderr_reader, Duration::from_secs(1))?;
        let text = String::from_utf8_lossy(&output.stdout).to_string()
            + &String::from_utf8_lossy(&output.stderr);
        return Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            format!("planner call did not reach fake Ollama. output={text:?}"),
        ));
    }

    let interrupted_at = Instant::now();
    stdin.write_all(b"\x1b")?;
    stdin.flush()?;
    let gate_four_elapsed =
        match wait_for_transcript_text(&transcript, "## Gate 4", Duration::from_secs(2)) {
            Ok(()) => interrupted_at.elapsed(),
            Err(error) => {
                let _ = child.kill();
                drop(stdin);
                let _ =
                    finish_queue_child(child, stdout_reader, stderr_reader, Duration::from_secs(1));
                return Err(error);
            }
        };
    let disconnect_elapsed = disconnected
        .recv_timeout(Duration::from_secs(2).saturating_sub(interrupted_at.elapsed()))
        .map(|()| interrupted_at.elapsed())
        .unwrap_or(Duration::MAX);
    stdin.write_all(b"/exit\r")?;
    stdin.flush()?;
    drop(stdin);
    let output = finish_queue_child(child, stdout_reader, stderr_reader, Duration::from_secs(20))?;
    Ok((output, gate_four_elapsed, disconnect_elapsed))
}

fn wait_for_gate_one_hash(path: &std::path::Path, timeout: Duration) -> std::io::Result<String> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Ok(transcript) = std::fs::read_to_string(path)
            && let Some(start) = transcript.find("sha256:")
        {
            let hash = transcript[start..]
                .chars()
                .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == ':')
                .collect::<String>();
            if hash.len() == "sha256:".len() + 64 {
                return Ok(hash);
            }
        }
        if Instant::now() >= deadline {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("Gate 1 hash did not appear in {}", path.display()),
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn wait_for_transcript_text(
    path: &std::path::Path,
    expected: &str,
    timeout: Duration,
) -> std::io::Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        if std::fs::read_to_string(path).is_ok_and(|text| text.contains(expected)) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("{expected:?} did not appear in {}", path.display()),
            ));
        }
        thread::sleep(Duration::from_millis(5));
    }
}

struct StreamingOllama {
    host: String,
    started: mpsc::Receiver<()>,
    completed: mpsc::Receiver<()>,
    disconnected: mpsc::Receiver<()>,
    saw_stream: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    server: thread::JoinHandle<()>,
}

fn start_streaming_ollama(completion_after: usize) -> StreamingOllama {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let host = format!("http://{}", listener.local_addr().unwrap());
    let stop = Arc::new(AtomicBool::new(false));
    let saw_stream = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let thread_saw_stream = Arc::clone(&saw_stream);
    let (started_tx, started_rx) = mpsc::sync_channel(1);
    let (completed_tx, completed_rx) = mpsc::sync_channel(1);
    let (disconnected_tx, disconnected_rx) = mpsc::sync_channel(1);
    let handle = thread::spawn(move || {
        let mut chat_count = 0usize;
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
                    chat_count += 1;
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
                    let _ = started_tx.try_send(());
                    thread::sleep(Duration::from_millis(300));
                    let _ = stream.write_all(second.as_bytes());
                    let _ = stream.write_all(terminal.as_bytes());
                    let _ = stream.flush();
                    if chat_count == completion_after {
                        let _ = completed_tx.try_send(());
                    }
                    stream
                        .set_read_timeout(Some(Duration::from_millis(20)))
                        .unwrap();
                    let disconnect_deadline = Instant::now() + Duration::from_millis(600);
                    while Instant::now() < disconnect_deadline {
                        let mut probe = [0_u8; 1];
                        match stream.read(&mut probe) {
                            Ok(0) => {
                                let _ = disconnected_tx.try_send(());
                                break;
                            }
                            Ok(_) => {}
                            Err(error)
                                if matches!(
                                    error.kind(),
                                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                                ) => {}
                            Err(_) => {
                                let _ = disconnected_tx.try_send(());
                                break;
                            }
                        }
                    }
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });
    StreamingOllama {
        host,
        started: started_rx,
        completed: completed_rx,
        disconnected: disconnected_rx,
        saw_stream,
        stop,
        server: handle,
    }
}

fn read_http_request(stream: &mut impl Read) -> String {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 1024];
    let mut expected = None;
    while let Ok(read) = stream.read(&mut buffer) {
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

fn run_startup_diagnostic_script(
    bin: &str,
    cwd: &std::path::Path,
    state_dir: &std::path::Path,
    host: &str,
    model: &str,
) -> std::io::Result<std::process::Output> {
    let args = [
        "--yes".to_string(),
        "--cwd".to_string(),
        cwd.to_string_lossy().into_owned(),
        "--state-dir".to_string(),
        state_dir.to_string_lossy().into_owned(),
        "--provider".to_string(),
        "ollama".to_string(),
        "--model".to_string(),
        model.to_string(),
        "--planner-provider".to_string(),
        "ollama".to_string(),
        "--planner-model".to_string(),
        model.to_string(),
        "--ollama-host".to_string(),
        host.to_string(),
        "--no-footer".to_string(),
    ]
    .into_iter()
    .map(|arg| shell_quote(&arg))
    .collect::<Vec<_>>()
    .join(" ");
    let command_line = format!("exec {} {args}", shell_quote(bin));
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

fn start_tags_only_ollama() -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let host = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let (mut stream, _) = loop {
            match listener.accept() {
                Ok(connection) => break connection,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "startup probe did not reach fake Ollama"
                    );
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("fake Ollama accept failed: {error}"),
            }
        };
        let request = read_http_request(&mut stream);
        assert!(request.starts_with("GET /api/tags "), "request={request:?}");
        let body = r#"{"models":[{"name":"installed:latest"}]}"#;
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
    });
    (host, server)
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn enter_first_run_guidance_command(child: &mut std::process::Child) -> std::io::Result<()> {
    let mut stdin = child.stdin.take().unwrap();
    thread::sleep(Duration::from_millis(1500));
    stdin.write_all(b"/plan-run first request\n")?;
    stdin.flush()?;
    thread::sleep(Duration::from_millis(500));
    stdin.write_all(b"/exit\n")?;
    stdin.flush()
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
    enter_first_run_guidance_command(&mut child)?;
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
    enter_first_run_guidance_command(&mut child)?;
    child.wait_with_output()
}
