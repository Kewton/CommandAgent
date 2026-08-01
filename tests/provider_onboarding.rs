use std::process::{Command, Output};

fn run_without_key(provider: &str, key: &str) -> Output {
    let workspace = tempfile::tempdir().unwrap();
    let state = workspace.path().join("state");
    std::fs::create_dir_all(&state).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_commandagent"));
    command.args([
        "--offline",
        "--provider",
        provider,
        "--model",
        "test-model",
        "--planner-provider",
        provider,
        "--planner-model",
        "test-model",
        "--prompt",
        "hello",
        "--cwd",
        workspace.path().to_str().unwrap(),
        "--state-dir",
        state.to_str().unwrap(),
        "--no-footer",
    ]);
    command.env_remove("OPENAI_API_KEY");
    command.env_remove("GEMINI_API_KEY");
    command.env_remove(key);
    command.output().unwrap()
}

#[test]
fn openai_and_gemini_missing_key_failures_explain_setup_and_doctor() {
    for (provider, key, setup) in [
        ("openai", "OPENAI_API_KEY", "process environment"),
        ("gemini", "GEMINI_API_KEY", "environment or workspace .env"),
    ] {
        let output = run_without_key(provider, key);
        let stderr = String::from_utf8(output.stderr).unwrap();

        assert!(
            !output.status.success(),
            "provider={provider} stderr={stderr:?}"
        );
        for expected in [key, setup, "commandagent --doctor"] {
            assert!(
                stderr.contains(expected),
                "provider={provider} missing {expected:?}. stderr={stderr:?}"
            );
        }
    }
}
