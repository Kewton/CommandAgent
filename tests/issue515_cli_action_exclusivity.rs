use std::process::{Command, Output};

use clap::Parser;
use clap::error::ErrorKind;

use commandagent::cli::Cli;

fn commandagent(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .args(arguments)
        .output()
        .unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

const GENERATED_ACTIONS: [&[&str]; 2] = [&["--completions", "bash"], &["--generate-man"]];

fn action_selector_cases() -> Vec<Vec<&'static str>> {
    vec![
        vec!["--extensions"],
        vec!["--packs", "--profile", "python-cli", "--intent", "create"],
        vec!["--pack-verify", "packs/cli-assist/1.0.0"],
        vec!["--pack-pin", "packs/cli-assist/1.0.0"],
        vec!["--workflow", "workflow.yaml"],
        vec!["--prompt", "hello"],
        vec!["--plan-steps", "build it"],
        vec!["--plan-run", "build it"],
        vec!["--run-plan", "plan.yaml"],
        vec!["--ultra-plan", "build it"],
        vec!["--ultra-plan-run", "build it"],
        vec!["--run-ultra-plan", "plan.yaml"],
        vec!["--validate-plan", "plan.yaml"],
        vec!["--setup-interaction-probe"],
        vec!["--runs"],
        vec!["--ux-demo"],
        vec!["--model-probe"],
        vec!["--doctor"],
        vec!["--init-config"],
        vec!["--validate-manifest", "profiles/static-site/manifest.toml"],
        vec![
            "--init-profile",
            "static-site",
            "--extension-root",
            "extensions",
        ],
    ]
}

#[test]
fn generated_actions_conflict_with_every_other_action_selector() {
    for generated in GENERATED_ACTIONS {
        for selector in action_selector_cases() {
            let arguments = std::iter::once("commandagent")
                .chain(generated.iter().copied())
                .chain(selector.iter().copied())
                .collect::<Vec<_>>();
            let error = Cli::try_parse_from(&arguments).unwrap_err();
            assert_eq!(
                error.kind(),
                ErrorKind::ArgumentConflict,
                "expected a conflict for {arguments:?}: {error}"
            );
        }
    }
}

#[test]
fn generated_actions_conflict_with_a_trailing_goal() {
    for generated in GENERATED_ACTIONS {
        let mut arguments = vec!["commandagent"];
        arguments.extend_from_slice(generated);
        arguments.push("some goal");
        let error = Cli::try_parse_from(&arguments).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::ArgumentConflict, "{arguments:?}");
    }
}

#[test]
fn generated_actions_still_run_alone() {
    let completions = Cli::try_parse_from(["commandagent", "--completions", "bash"]).unwrap();
    assert_eq!(completions.completions, Some(clap_complete::Shell::Bash));
    assert!(!completions.generate_man);

    let man = Cli::try_parse_from(["commandagent", "--generate-man"]).unwrap();
    assert!(man.generate_man);
    assert!(man.completions.is_none());
}

#[test]
fn generated_action_conflicts_exit_two_before_a_run() {
    for arguments in [
        ["--completions", "zsh", "--doctor"].as_slice(),
        ["--generate-man", "--ux-demo"].as_slice(),
        ["--completions", "bash", "--generate-man"].as_slice(),
    ] {
        let output = commandagent(arguments);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{arguments:?}: {}",
            stderr(&output)
        );
        assert!(
            stderr(&output).contains("cannot be used with"),
            "{arguments:?}: {}",
            stderr(&output)
        );
    }
}

#[test]
fn pre_run_argument_and_configuration_rejections_exit_two() {
    for arguments in [
        ["--prompt", "hello", "--plan-steps", "hello"].as_slice(),
        ["--planner-provider", "openai", "--prompt", "hello"].as_slice(),
    ] {
        let output = commandagent(arguments);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{arguments:?}: {}",
            stderr(&output)
        );
        assert!(
            !output.status.success(),
            "{arguments:?} unexpectedly succeeded"
        );
    }
}
