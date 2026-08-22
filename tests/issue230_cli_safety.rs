use std::path::Path;
use std::process::{Command, Output};

fn git(root: &Path, arguments: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .status()
        .unwrap();
    assert!(status.success(), "git {arguments:?}");
}

fn failed_prompt(workspace: &Path, state: &Path, extra: &[&str]) -> Output {
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
        workspace.to_str().unwrap(),
        "--state-dir",
        state.to_str().unwrap(),
        "--no-footer",
    ]);
    command.args(extra);
    command.env_remove("OPENAI_API_KEY").output().unwrap()
}

#[test]
fn dirty_git_workspace_warns_at_start_and_reports_changes_at_exit() {
    let fixture = tempfile::tempdir().unwrap();
    let workspace = fixture.path().join("workspace");
    let state = fixture.path().join("state");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir_all(&state).unwrap();
    git(&workspace, &["init", "-q"]);
    std::fs::write(workspace.join("existing.txt"), "pre-existing\n").unwrap();

    let output = failed_prompt(&workspace, &state, &[]);
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(!output.status.success(), "{stderr}");
    assert!(
        stderr.contains(commandagent::tools::git_state::DIRTY_WARNING),
        "{stderr}"
    );
    assert!(
        stderr.contains(commandagent::tools::git_state::EXIT_REPORT_HEADING),
        "{stderr}"
    );
    assert!(stderr.contains("Untracked files:"), "{stderr}");
    assert!(stderr.contains("  - existing.txt"), "{stderr}");
}

#[test]
fn allow_flag_is_accepted_by_the_binary() {
    let fixture = tempfile::tempdir().unwrap();
    let workspace = fixture.path().join("workspace");
    let state = fixture.path().join("state");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir_all(&state).unwrap();
    git(&workspace, &["init", "-q"]);

    let output = failed_prompt(&workspace, &state, &["--allow", "read,write"]);
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(!output.status.success(), "{stderr}");
    assert!(stderr.contains("OPENAI_API_KEY"), "{stderr}");
    assert!(
        !stderr.contains("unexpected argument '--allow'"),
        "{stderr}"
    );
}
