use std::process::{Command, Output};

const WARNING: &str = "warning: --prompt is non-interactive because stdin is not a TTY; mutating tools require an explicit --allow policy or --yes. Use --yes only in a trusted workspace.";

fn run_headless_prompt(auto_approve: bool) -> Output {
    let workspace = tempfile::tempdir().unwrap();
    let state = workspace.path().join("state");
    std::fs::create_dir_all(&state).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_commandagent"));
    command.args([
        "--offline",
        "--provider",
        "openai",
        "--model",
        "test-model",
        "--prompt",
        "inspect the workspace",
        "--cwd",
        workspace.path().to_str().unwrap(),
        "--state-dir",
        state.to_str().unwrap(),
        "--no-footer",
    ]);
    if auto_approve {
        command.arg("--yes");
    }
    command.env_remove("OPENAI_API_KEY").output().unwrap()
}

#[test]
fn headless_prompt_without_yes_warns_before_execution_starts() {
    let output = run_headless_prompt(false);
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(!output.status.success(), "{stderr}");
    assert!(stderr.lines().any(|line| line == WARNING), "{stderr}");
    let warning_offset = stderr.find(WARNING).expect("startup warning offset");
    let provider_offset = stderr
        .find("OPENAI_API_KEY")
        .expect("provider error offset");
    assert!(warning_offset < provider_offset, "{stderr}");
    assert!(!stderr.contains("use interactive approval"), "{stderr}");
}

#[test]
fn headless_prompt_with_yes_does_not_emit_the_warning() {
    let output = run_headless_prompt(true);
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(!output.status.success(), "{stderr}");
    assert!(!stderr.contains(WARNING), "{stderr}");
    assert!(stderr.contains("OPENAI_API_KEY"), "{stderr}");
}
