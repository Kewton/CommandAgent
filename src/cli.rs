use crate::agent::minimal_loop::loop_run::{ChatClient, MinimalLoopConfig, run_session};
use crate::agent::repl::{MinimalReplRunner, run_repl};
use crate::agent::step_runner::runtime::PlannerRuntimeConfig;
use crate::config::{Config, Provider};
use crate::providers::gemini::{DEFAULT_GEMINI_BASE_URL, GeminiClient};
use crate::providers::ollama::OllamaClient;
use crate::providers::openai::{DEFAULT_OPENAI_BASE_URL, OpenAiClient};
use crate::providers::planner::resolve_targets;
use crate::providers::{ChatRequest, ChatResponse, request_tool_mode};
use std::ffi::OsString;
use std::io::{self, IsTerminal};
use std::process::ExitCode;
use std::time::Duration;

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
      --max-iterations N     max loop iterations\n\
      --yes                  accept non-interactive defaults\n\
\n\
CommandAgent is a minimal local-first coding agent. The MVP migration keeps\n\
only the minimal loop, interactive REPL, provider adapters, built-in tools,\n\
and /ultra-plan-run style step execution."
    );
}

fn run_one_shot(config: Config, prompt: &str) -> Result<(), String> {
    let mut client = runtime_client(&config)?;
    let result = run_session(
        &mut client,
        &config.cwd,
        prompt,
        minimal_loop_config(&config),
    )
    .map_err(|err| err.to_string())?;
    if !result.final_answer.trim().is_empty() {
        println!("{}", result.final_answer.trim());
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
        ..MinimalLoopConfig::default()
    }
}

fn runtime_client(config: &Config) -> Result<RuntimeClient, String> {
    runtime_client_for(config, config.provider)
}

fn runtime_client_for(config: &Config, provider: Provider) -> Result<RuntimeClient, String> {
    let timeout = Duration::from_secs(config.timeout_secs);
    match provider {
        Provider::Ollama => {
            let base_url = std::env::var("OLLAMA_HOST")
                .unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());
            Ok(RuntimeClient::Ollama(
                OllamaClient::with_options(base_url, timeout, config.retries)
                    .map_err(|err| err.to_string())?,
            ))
        }
        Provider::Gemini => {
            let key = config
                .gemini_api_key
                .clone()
                .ok_or_else(|| "GEMINI_API_KEY is required for --provider gemini".to_string())?;
            Ok(RuntimeClient::Gemini(
                GeminiClient::with_options(DEFAULT_GEMINI_BASE_URL, key, timeout, config.retries)
                    .map_err(|err| err.to_string())?,
            ))
        }
        Provider::OpenAi => {
            let key = config
                .openai_api_key
                .clone()
                .ok_or_else(|| "OPENAI_API_KEY is required for --provider openai".to_string())?;
            Ok(RuntimeClient::OpenAi(
                OpenAiClient::with_options(DEFAULT_OPENAI_BASE_URL, key, timeout, config.retries)
                    .map_err(|err| err.to_string())?,
            ))
        }
    }
}

enum RuntimeClient {
    Ollama(OllamaClient),
    Gemini(GeminiClient),
    OpenAi(OpenAiClient),
}

impl ChatClient for RuntimeClient {
    fn chat(&mut self, request: &ChatRequest) -> Result<ChatResponse, String> {
        match self {
            Self::Ollama(client) => client.chat(request).map_err(|err| err.to_string()),
            Self::Gemini(client) => client.chat(request).map_err(|err| err.to_string()),
            Self::OpenAi(client) => client.chat(request).map_err(|err| err.to_string()),
        }
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
