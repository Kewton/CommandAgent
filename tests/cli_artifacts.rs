use std::fs;
use std::process::{Command, Output};

fn commandagent(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .args(args)
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

#[test]
fn help_lists_completion_shells_and_man_generation() {
    let output = commandagent(&["--help"]);
    let text = stdout(&output);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(text.contains("--completions <SHELL>"), "{text}");
    for shell in ["bash", "elvish", "fish", "powershell", "zsh"] {
        assert!(text.contains(shell), "missing {shell} in:\n{text}");
    }
    assert!(text.contains("--generate-man"), "{text}");
}

#[test]
fn completion_scripts_are_generated_to_stdout_for_supported_shells() {
    let cases = [
        ("bash", "_commandagent()"),
        ("zsh", "#compdef commandagent"),
        ("fish", "complete -c commandagent"),
        ("powershell", "Register-ArgumentCompleter"),
        ("elvish", "set edit:completion:arg-completer[commandagent]"),
    ];

    for (shell, marker) in cases {
        let output = commandagent(&["--completions", shell]);
        let generated = stdout(&output);
        let error = stderr(&output);
        assert!(output.status.success(), "{shell}: {error}");
        assert!(error.is_empty(), "{shell}: {error}");
        assert!(generated.contains(marker), "{shell}: {generated}");
        assert!(generated.contains("commandagent"), "{shell}: {generated}");
        assert!(generated.contains("generate-man"), "{shell}: {generated}");
    }
}

#[test]
fn invalid_completion_shell_has_clap_error_and_possible_values() {
    let output = commandagent(&["--completions", "tcsh"]);
    let error = stderr(&output);

    assert_eq!(output.status.code(), Some(2), "{error}");
    assert!(stdout(&output).is_empty());
    assert!(error.contains("invalid value 'tcsh'"), "{error}");
    assert!(error.contains("possible values"), "{error}");
    assert!(error.contains("bash"), "{error}");
    assert!(error.contains("zsh"), "{error}");
    assert!(error.contains("fish"), "{error}");
    assert!(error.contains("powershell"), "{error}");
}

#[test]
fn man_page_is_generated_to_stdout_from_the_cli_definition() {
    let output = commandagent(&["--generate-man"]);
    let generated = stdout(&output);
    let error = stderr(&output);

    assert!(output.status.success(), "{error}");
    assert!(error.is_empty(), "{error}");
    assert!(generated.contains(".TH commandagent 1"), "{generated}");
    assert!(generated.contains("\\-\\-completions"), "{generated}");
    assert!(generated.contains("\\-\\-generate\\-man"), "{generated}");
}

#[test]
fn artifact_generation_does_not_write_to_the_working_directory() {
    for args in [
        ["--completions", "bash"].as_slice(),
        ["--generate-man"].as_slice(),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_commandagent"))
            .args(args)
            .current_dir(directory.path())
            .output()
            .unwrap();

        assert!(output.status.success(), "{}", stderr(&output));
        assert!(
            fs::read_dir(directory.path()).unwrap().next().is_none(),
            "generation created a file for {args:?}"
        );
    }
}
