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
}
