use std::process::{Command, Stdio};

#[test]
fn help_exits_successfully() {
    let output = Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .arg("--help")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("CommandAgent"));
    assert!(stdout.contains("Usage:"));
}

#[test]
fn version_exits_successfully() {
    let output = Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .arg("--version")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with("commandagent "));
}

#[test]
fn non_tty_no_prompt_prints_mvp_message() {
    let output = Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .stdin(Stdio::null())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("CommandAgent MVP"));
    assert!(stdout.contains("Run `commandagent --help` for usage"));
}
