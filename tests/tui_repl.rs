use std::io::IsTerminal;
use std::path::PathBuf;

use clap::Parser;

use commandagent::cli::Cli;
use commandagent::config::{Action, Config, Provider};
use commandagent::tui::slash::{parse_slash, parse_words};

fn config(root: PathBuf) -> Config {
    Config {
        workspace_root: root,
        state_dir: PathBuf::from("state"),
        eval_events_path: None,
        completion_contract_path: None,
        yes: true,
        offline: false,
        context_budget: 1000,
        model: "m".to_string(),
        provider: Provider::Ollama,
        tool_protocol: None,
        openai_api: commandagent::config::OpenAiApi::ChatCompletions,
        prompt_layout: commandagent::config::PromptLayout::Stable,
        plan_preset: commandagent::config::PlanPreset::None,
        intent_override: None,
        planner_model: "pm".to_string(),
        planner_provider: Provider::Gemini,
        planner_think: Some(commandagent::config::OllamaThink::False),
        classifier_model: "pm".to_string(),
        classifier_provider: Provider::Gemini,
        openai_compatible: None,
        ollama_host: "http://localhost:11434".to_string(),
        ollama_think: None,
        lm_studio_host: "http://localhost:1234".to_string(),
        num_predict: 100,
        max_iterations: 4,
        recovery_plan_auto_runs: 0,
        chat_timeout_secs: 1,
        chat_timeout_source: "override:test".to_string(),
        field_sources: commandagent::config::ConfigFieldSources::default(),
        chat_retries: 1,
        stream: false,
        resume: None,
        fresh_session: false,
        no_footer: false,
        narration: commandagent::config::NarrationMode::Normal,
        profile: "generic".to_string(),
        profile_explicit: false,
        profile_inference: None,
        style: "default".to_string(),
        action: Action::Repl,
    }
}

#[test]
fn tui_non_tty_requires_action() {
    if std::io::stdin().is_terminal() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let err = commandagent::repl::run_repl(config(dir.path().to_path_buf()))
        .unwrap_err()
        .to_string();
    assert!(err.contains("stdin is not a TTY"));
}

#[test]
fn slash_command_quotes_profile_style_and_goal() {
    let words = parse_words(r#"/ultra-plan-run --profile nextjs --style compact "3011 app""#);
    assert_eq!(
        words,
        vec![
            "/ultra-plan-run",
            "--profile",
            "nextjs",
            "--style",
            "compact",
            "3011 app"
        ]
    );
}

#[test]
fn cli_repl_action_parity() {
    let dir = tempfile::tempdir().unwrap();
    let base = config(dir.path().to_path_buf());
    let parsed = parse_slash(
        r#"/ultra-plan-run --profile nextjs --style compact "3011 port app""#,
        &base,
    )
    .unwrap();
    let cli = Cli::parse_from([
        "commandagent",
        "--provider",
        "ollama",
        "--model",
        "m",
        "--planner-provider",
        "gemini",
        "--planner-model",
        "pm",
        "--ultra-plan-run",
        "--profile",
        "nextjs",
        "--style",
        "compact",
        "3011 port app",
    ]);
    let config = Config::from_cli(cli).unwrap();
    assert_eq!(parsed.command, "/ultra-plan-run");
    assert_eq!(parsed.profile, config.profile);
    assert_eq!(parsed.style, config.style);
    assert!(matches!(config.action, Action::UltraPlanRun(goal) if goal == parsed.goal));
}

#[test]
fn no_footer_flag_reaches_config() {
    let cli = Cli::parse_from(["commandagent", "--no-footer"]);
    let config = Config::from_cli(cli).unwrap();
    assert!(config.no_footer);
}

const ASSIST: &str = r#"schema_version: commandagent.pack.assist/v0
pack:
  id: nextjs-acme
  version: 1.0.0
  profile: nextjs
  intent: create
inject:
  - point: build-verification
    source: browser_interaction
    required: true
    params:
      fields:
        - dispatched_inputs
        - observed_state
        - hook_status
        - surface
        - outcome
"#;

fn write_extension_pack(extension: &std::path::Path) {
    let directory = extension.join("nextjs-acme/1.0.0");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("assist.yaml"), ASSIST).unwrap();
    let hash = commandagent::planner::pack::exact_byte_hash(Some(ASSIST.as_bytes()), None);
    std::fs::write(directory.join("pack.sha256"), format!("{hash}\n")).unwrap();
}

/// Issue #1057: the REPL selects its Gate 1 pack through the CLI's roots. A
/// pack that exists only under `--extension-root` is the bytes the CLI resolves,
/// and the REPL context resolves the same selector to the same source and hash.
#[test]
fn repl_pack_context_reads_the_extension_root_like_the_cli() {
    let workspace = tempfile::tempdir().unwrap();
    let extension = workspace.path().join("extensions");
    write_extension_pack(&extension);

    let cli = Cli::parse_from([
        "commandagent",
        "--cwd",
        workspace.path().to_str().unwrap(),
        "--profile",
        "nextjs",
        "--intent",
        "create",
        "--extension-root",
        extension.to_str().unwrap(),
        "--pack",
        "nextjs-acme@1.0.0",
    ]);
    let config = Config::from_cli(cli.clone()).unwrap();

    // The CLI resolves it from the extension root.
    let resolved = commandagent::planner::pack::catalog::PackLocator::with_extension_root(
        &config.workspace_root,
        Some(extension.clone()),
    )
    .locate_pinned("nextjs-acme", "1.0.0", None)
    .unwrap();
    assert_eq!(
        resolved.source,
        commandagent::planner::pack::catalog::PackSource::Local
    );

    // A selection through the same roots freezes that same local source and
    // hash; nothing here reads the workspace copy, which does not exist.
    let context = commandagent::tui::repl::pack_selection::ReplPackContext::new(
        config.workspace_root.clone(),
        Some(extension.clone()),
    );
    let selection = context
        .select("nextjs", "create", "nextjs-acme@1.0.0")
        .unwrap();
    assert!(matches!(
        selection,
        commandagent::tui::boundary_shell::confirmation::PackSelection::Pinned {
            source: commandagent::planner::pack::catalog::PackSource::Local,
            hash,
            ..
        } if hash == resolved.hash
    ));

    // Without the extension root the same selector is unreadable, and the
    // refusal names the selector without quoting the pack.
    let workspace_only = commandagent::tui::repl::pack_selection::ReplPackContext::new(
        config.workspace_root.clone(),
        None,
    );
    assert!(
        workspace_only
            .select("nextjs", "create", "nextjs-acme@1.0.0")
            .is_err()
    );
}
