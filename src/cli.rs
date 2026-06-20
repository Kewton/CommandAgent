use crate::agent::minimal_loop::config::DependencySetupPolicy;
use crate::agent::minimal_loop::loop_run::{MinimalLoopConfig, run_session_with_observer};
use crate::agent::repl::{MinimalReplRunner, run_repl};
use crate::agent::slash_command::parse_slash_command;
use crate::agent::step_runner::runtime::PlannerRuntimeConfig;
use crate::agent::step_runner::runtime::SlashRuntime;
use crate::config::Config;
use crate::providers::planner::resolve_targets;
use crate::providers::request_tool_mode;
use crate::runtime_client::{runtime_client, runtime_client_for};
use crate::tui::terminal::{TerminalUi, render_final_answer};
use std::ffi::OsString;
use std::io::{self, IsTerminal};
use std::process::ExitCode;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn run<I, S>(args: I) -> ExitCode
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let args = args.into_iter().map(Into::into).collect::<Vec<_>>();

    if args
        .iter()
        .skip(1)
        .any(|arg| arg == "--help" || arg == "-h")
    {
        print_help();
        return ExitCode::SUCCESS;
    }

    if args
        .iter()
        .skip(1)
        .any(|arg| arg == "--version" || arg == "-V")
    {
        println!("commandagent {VERSION}");
        return ExitCode::SUCCESS;
    }

    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(err) => {
            eprintln!("ERROR: failed to read current directory: {err}");
            return ExitCode::FAILURE;
        }
    };

    let config = match Config::load_from(&cwd, args.clone(), |key| std::env::var(key).ok()) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("ERROR: {err}");
            return ExitCode::FAILURE;
        }
    };

    if let Some(prompt) = collect_prompt_arg(&args) {
        return match run_one_shot(config, &prompt) {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("ERROR: {err}");
                ExitCode::FAILURE
            }
        };
    }

    if io::stdin().is_terminal() {
        return match run_interactive(config) {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("ERROR: {err}");
                ExitCode::FAILURE
            }
        };
    }

    println!("CommandAgent MVP");
    println!("Run `commandagent --help` for usage, or start without a prompt from a TTY.");
    ExitCode::SUCCESS
}

fn print_help() {
    println!(
        "CommandAgent {VERSION}\n\
\n\
Usage:\n\
  commandagent [OPTIONS] [PROMPT]\n\
\n\
Options:\n\
  -h, --help                 Print help\n\
  -V, --version              Print version\n\
      --provider PROVIDER    ollama, gemini, or openai\n\
      --model MODEL          executor model\n\
      --planner-provider PROVIDER\n\
                              planner provider; defaults to executor provider\n\
      --planner-model MODEL  planner model; defaults to executor model\n\
      --context-budget N     context budget passed through configuration\n\
      --max-iterations N     max loop iterations\n\
      --yes                  approve non-interactive defaults, including one bounded dependency setup recovery\n\
      --offline              block network/dependency setup recovery\n\
\n\
CommandAgent is a minimal local-first coding agent. The MVP migration keeps\n\
only the minimal loop, interactive REPL, provider adapters, built-in tools,\n\
and /ultra-plan-run style step execution. PROMPT may be a slash command such\n\
as `/plan-run --profile python ...` for non-interactive execution."
    );
}

fn run_one_shot(config: Config, prompt: &str) -> Result<(), String> {
    if let Some(command) =
        parse_slash_command(prompt, &config.cwd).map_err(|err| err.to_string())?
    {
        let targets = resolve_targets(&config);
        let mut client = runtime_client_for(&config, targets.executor.provider)?;
        let mut planner_client = runtime_client_for(&config, targets.planner.provider)?;
        let planner_config = PlannerRuntimeConfig {
            model: targets
                .planner
                .model
                .clone()
                .unwrap_or_else(|| "default".to_string()),
            tool_call_mode: request_tool_mode(targets.planner.provider),
        };
        let mut ui = TerminalUi::stderr_from_env();
        let output = SlashRuntime {
            executor: &mut client,
            planner: &mut planner_client,
            cwd: &config.cwd,
            loop_config: minimal_loop_config(&config),
            planner_config,
        }
        .run_with_observer(command, &mut ui)?;
        if !output.trim().is_empty() {
            println!("{}", output.trim());
        }
        return Ok(());
    }

    let mut client = runtime_client(&config)?;
    let mut ui = TerminalUi::stderr_from_env();
    let result = run_session_with_observer(
        &mut client,
        &config.cwd,
        prompt,
        minimal_loop_config(&config),
        &mut ui,
    )
    .map_err(|err| err.to_string())?;
    if !result.final_answer.trim().is_empty() {
        println!("{}", render_final_answer(&result.final_answer));
    }
    Ok(())
}

fn run_interactive(config: Config) -> Result<(), String> {
    let targets = resolve_targets(&config);
    let client = runtime_client_for(&config, targets.executor.provider)?;
    let planner_client = runtime_client_for(&config, targets.planner.provider)?;
    let planner_config = PlannerRuntimeConfig {
        model: targets
            .planner
            .model
            .clone()
            .unwrap_or_else(|| "default".to_string()),
        tool_call_mode: request_tool_mode(targets.planner.provider),
    };
    let mut runner = MinimalReplRunner::new(
        client,
        planner_client,
        &config.cwd,
        minimal_loop_config(&config),
        planner_config,
    )?;
    let mut startup_ui = TerminalUi::stderr_from_env();
    startup_ui
        .render_startup_context(VERSION, &config, &targets)
        .map_err(|err| err.to_string())?;
    let stdin = io::stdin();
    let stdout = io::stdout();
    run_repl(stdin.lock(), stdout.lock(), &mut runner).map_err(|err| err.to_string())
}

fn minimal_loop_config(config: &Config) -> MinimalLoopConfig {
    MinimalLoopConfig {
        model: config
            .model
            .clone()
            .unwrap_or_else(|| "default".to_string()),
        max_iterations: config.max_iterations as usize,
        initial_tool_call_mode: request_tool_mode(config.provider),
        dependency_setup_policy: DependencySetupPolicy {
            auto_approve: config.yes,
            offline: config.offline,
            timeout_secs: 600,
        },
        ..MinimalLoopConfig::default()
    }
}

fn collect_prompt_arg(args: &[OsString]) -> Option<String> {
    let mut iter = args.iter().skip(1).peekable();
    let mut prompt = Vec::new();

    while let Some(arg) = iter.next() {
        let arg = arg.to_string_lossy();
        if arg == "--" {
            prompt.extend(iter.map(|value| value.to_string_lossy().to_string()));
            break;
        }
        if arg.starts_with('-') {
            if option_takes_value(&arg) && !arg.contains('=') {
                let _ = iter.next();
            }
            continue;
        }

        prompt.push(arg.to_string());
        prompt.extend(iter.map(|value| value.to_string_lossy().to_string()));
        break;
    }

    if prompt.is_empty() {
        None
    } else {
        Some(prompt.join(" "))
    }
}

fn option_takes_value(option: &str) -> bool {
    matches!(
        option,
        "--state-dir"
            | "--provider"
            | "--planner-provider"
            | "--model"
            | "--planner-model"
            | "--context-budget"
            | "--max-iterations"
            | "--timeout"
            | "--timeout-secs"
            | "--retries"
            | "--resume"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_exits_successfully() {
        assert_eq!(run(["commandagent", "--help"]), ExitCode::SUCCESS);
    }

    #[test]
    fn version_exits_successfully() {
        assert_eq!(run(["commandagent", "--version"]), ExitCode::SUCCESS);
    }

    #[test]
    fn collect_prompt_skips_option_values() {
        let args = [
            OsString::from("commandagent"),
            OsString::from("--model"),
            OsString::from("qwen"),
            OsString::from("--yes"),
            OsString::from("create"),
            OsString::from("file"),
        ];

        assert_eq!(collect_prompt_arg(&args).as_deref(), Some("create file"));
    }

    #[test]
    fn collect_prompt_supports_double_dash_literal() {
        let args = [
            OsString::from("commandagent"),
            OsString::from("--"),
            OsString::from("--not-an-option"),
        ];

        assert_eq!(
            collect_prompt_arg(&args).as_deref(),
            Some("--not-an-option")
        );
    }
}
