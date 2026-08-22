use std::process::Command;

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

#[cfg(unix)]
#[test]
fn sigint_emits_interrupted_summary_as_the_final_stdout_line() {
    let workspace = tempfile::tempdir().unwrap();
    let state = workspace.path().join("state");
    std::fs::create_dir_all(&state).unwrap();
    let events = workspace.path().join("run-evidence/events.jsonl");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let ollama_host = format!("http://{}", listener.local_addr().unwrap());
    let mut child = Command::new(env!("CARGO_BIN_EXE_commandagent"))
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

    let evidence_deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !events.is_file() {
        if let Some(status) = child.try_wait().unwrap() {
            let output = child.wait_with_output().unwrap();
            panic!(
                "commandagent exited before SIGINT: {status}\nstdout={}\nstderr={}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        assert!(
            std::time::Instant::now() < evidence_deadline,
            "timed out waiting for run evidence"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    std::thread::sleep(std::time::Duration::from_millis(100));

    let signal_result = unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGINT) };
    assert_eq!(signal_result, 0, "failed to send SIGINT");

    let exit_deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if std::time::Instant::now() >= exit_deadline {
            child.kill().unwrap();
            let _ = child.wait();
            panic!("commandagent did not exit after SIGINT");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    let output = child.wait_with_output().unwrap();

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
