use std::path::PathBuf;

use clap::{ArgAction, ArgGroup, Parser, ValueEnum};
use clap_complete::Shell;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ProviderArg {
    Ollama,
    LmStudio,
    Openai,
    Gemini,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ToolProtocolArg {
    Native,
    Text,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OpenAiApiArg {
    ChatCompletions,
    Responses,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum FooterArg {
    On,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum StreamArg {
    On,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OllamaThinkArg {
    #[value(name = "true")]
    True,
    #[value(name = "false")]
    False,
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PromptLayoutArg {
    Stable,
    Legacy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PlanPresetArg {
    Profile,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum IntentArg {
    Create,
    Fix,
    Investigate,
}

fn parse_positive_usize(value: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|error| format!("invalid positive integer `{value}`: {error}"))?;
    if parsed == 0 {
        return Err("value must be at least 1".to_string());
    }
    Ok(parsed)
}

#[derive(Debug, Clone, Parser)]
#[command(name = "commandagent")]
#[command(
    about = "Local-first coding agent with verified minimal-loop and structured-plan workflows"
)]
#[command(version = crate::build_info::VERSION)]
#[command(group(
    ArgGroup::new("pack_direct_action")
        .args(["packs", "pack_verify", "pack_pin"])
        .multiple(false)
))]
pub struct Cli {
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help_heading = "Workspace and State",
        help = "Auto-approve mutating tools and resume confirmation; recognized Bash writes remain workspace-confined. It never auto-kills a busy-port owner. Use only in a trusted workspace."
    )]
    pub yes: bool,
    #[arg(
        long,
        help_heading = "Planning and Verification",
        help = "Select a named `[preset.<name>]` assembled from configuration files."
    )]
    pub preset: Option<String>,
    #[arg(
        long,
        value_name = "ID@VERSION",
        help_heading = "Planning and Verification",
        help = "Activate an exact-version pack. A conflicting preset pack is rejected before the run."
    )]
    pub pack: Option<String>,
    #[arg(
        long,
        value_name = "SHA256",
        requires = "pack",
        help_heading = "Planning and Verification",
        help = "Require the selected pack's exact-byte hash. Requires `--pack`."
    )]
    pub pack_hash: Option<String>,
    #[arg(
        long,
        value_name = "DIR",
        help_heading = "Planning and Verification",
        help = "Load local packs and `profiles/<id>/manifest.toml` draft profiles. External profiles are forced to draft and pinned by exact-byte hash."
    )]
    pub extension_root: Option<PathBuf>,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        requires_all = ["profile", "intent"],
        conflicts_with_all = [
            "pack", "pack_hash", "workflow", "prompt", "plan_steps", "plan_run", "run_plan",
            "ultra_plan", "ultra_plan_run", "run_ultra_plan", "setup_interaction_probe", "runs",
            "ux_demo", "model_probe", "doctor", "completions", "generate_man"
        ],
        help_heading = "Actions (use one)",
        help = "List compatible admitted packs and conformant packs found under `--extension-root`, including each source. Requires `--profile` and `--intent`."
    )]
    pub packs: bool,
    #[arg(
        long,
        value_name = "DIR",
        conflicts_with_all = [
            "pack", "pack_hash", "extension_root", "workflow", "prompt", "plan_steps", "plan_run",
            "run_plan", "ultra_plan", "ultra_plan_run", "run_ultra_plan", "setup_interaction_probe",
            "runs", "ux_demo", "model_probe", "doctor", "completions", "generate_man"
        ],
        help_heading = "Actions (use one)",
        help = "Run strict conformance for one pack directory and print the same JSON report as `pack_conformance`."
    )]
    pub pack_verify: Option<PathBuf>,
    #[arg(
        long,
        value_name = "DIR",
        conflicts_with_all = [
            "pack", "pack_hash", "extension_root", "workflow", "prompt", "plan_steps", "plan_run",
            "run_plan", "ultra_plan", "ultra_plan_run", "run_ultra_plan", "setup_interaction_probe",
            "runs", "ux_demo", "model_probe", "doctor", "completions", "generate_man"
        ],
        help_heading = "Actions (use one)",
        help = "Create `pack.sha256` after green conformance, keep an identical pin unchanged, and reject a stale pin."
    )]
    pub pack_pin: Option<PathBuf>,
    #[arg(
        long,
        help_heading = "Models and Providers",
        help = "Set the approximate conversation compaction budget."
    )]
    pub context_budget: Option<usize>,
    #[arg(
        long,
        add = clap_complete::ArgValueCompleter::new(crate::cli_completion::complete_model_ids),
        help_heading = "Models and Providers",
        help = "Set the executor model ID."
    )]
    pub model: Option<String>,
    #[arg(
        long,
        value_enum,
        help_heading = "Models and Providers",
        help = "Select the executor provider."
    )]
    pub provider: Option<ProviderArg>,
    #[arg(
        long = "api",
        value_enum,
        value_name = "chat-completions|responses",
        help_heading = "Models and Providers",
        help = "Explicitly select the OpenAI-compatible API surface; model names never select it implicitly."
    )]
    pub openai_api: Option<OpenAiApiArg>,
    #[arg(
        long,
        value_enum,
        value_name = "native|text",
        help_heading = "Models and Providers",
        help = "Explicitly select native function tools or the established text/XML tool protocol."
    )]
    pub tool_protocol: Option<ToolProtocolArg>,
    #[arg(
        long,
        value_enum,
        value_name = "stable|legacy",
        help_heading = "Planning and Verification",
        help = "Choose prompt section order for A/B measurement."
    )]
    pub prompt_layout: Option<PromptLayoutArg>,
    #[arg(
        long,
        value_enum,
        value_name = "profile|none",
        help_heading = "Planning and Verification",
        help = "Override planner-tier UltraPlan preset selection. `data/fix` can synthesize F1–F3 steps; `nextjs/fix` remains none-equivalent."
    )]
    pub plan_preset: Option<PlanPresetArg>,
    #[arg(
        long,
        value_enum,
        value_name = "create|fix|investigate",
        help_heading = "Planning and Verification",
        help = "Force intent instead of goal-based resolution."
    )]
    pub intent: Option<IntentArg>,
    #[arg(
        long,
        conflicts_with = "intent",
        help_heading = "Actions (use one)",
        help = "Run a declarative workflow-circle definition. Mutually exclusive with `--intent`."
    )]
    pub workflow: Option<PathBuf>,
    #[arg(
        long,
        requires = "workflow",
        help_heading = "Planning and Verification",
        help = "Supply the existing failed origin run workspace for `--workflow`."
    )]
    pub origin: Option<PathBuf>,
    #[arg(
        long,
        add = clap_complete::ArgValueCompleter::new(crate::cli_completion::complete_model_ids),
        help_heading = "Models and Providers",
        help = "Set the planner model ID. Required when planner and executor providers differ."
    )]
    pub planner_model: Option<String>,
    #[arg(
        long,
        value_enum,
        help_heading = "Models and Providers",
        help = "Select the planner provider."
    )]
    pub planner_provider: Option<ProviderArg>,
    #[arg(
        long,
        help_heading = "Actions (use one)",
        help = "Run one minimal-loop prompt instead of entering the TUI."
    )]
    pub prompt: Option<String>,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help_heading = "Actions (use one)",
        help = "Generate and save a step plan for the trailing goal."
    )]
    pub plan_steps: bool,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help_heading = "Actions (use one)",
        help = "Generate and run a step plan for the trailing goal."
    )]
    pub plan_run: bool,
    #[arg(
        long,
        help_heading = "Actions (use one)",
        help = "Run an existing step-plan YAML file."
    )]
    pub run_plan: Option<PathBuf>,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help_heading = "Actions (use one)",
        help = "Generate and save an UltraPlan for the trailing goal."
    )]
    pub ultra_plan: bool,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help_heading = "Actions (use one)",
        help = "Generate and run an UltraPlan for the trailing goal."
    )]
    pub ultra_plan_run: bool,
    #[arg(
        long,
        help_heading = "Actions (use one)",
        help = "Run an existing UltraPlan YAML file."
    )]
    pub run_ultra_plan: Option<PathBuf>,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help_heading = "Actions (use one)",
        help = "Install or validate the managed Playwright interaction probe."
    )]
    pub setup_interaction_probe: bool,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help_heading = "Actions (use one)",
        help = "List recent runs for the current workspace without creating provider clients."
    )]
    pub runs: bool,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help_heading = "Actions (use one)",
        help = "Run the offline presentation UX demo."
    )]
    pub ux_demo: bool,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help_heading = "Actions (use one)",
        help = "Run the bounded model behavior probe battery."
    )]
    pub model_probe: bool,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help_heading = "Actions (use one)",
        help = "Diagnose configuration files, provider readiness, interaction probes, and the local environment without making network requests."
    )]
    pub doctor: bool,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        requires = "doctor",
        help_heading = "Display",
        help = "Render `--doctor` output as stable machine-readable JSON. Requires `--doctor`."
    )]
    pub json: bool,
    #[arg(
        long,
        value_enum,
        value_name = "SHELL",
        conflicts_with = "generate_man",
        help_heading = "Actions (use one)",
        help = "Generate a completion script from the current Clap definition and write it to stdout."
    )]
    pub completions: Option<Shell>,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        conflicts_with = "completions",
        help_heading = "Actions (use one)",
        help = "Generate the `commandagent(1)` man page from the current Clap definition and write it to stdout."
    )]
    pub generate_man: bool,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        conflicts_with_all = [
            "packs", "pack_verify", "pack_pin", "workflow", "prompt", "plan_steps", "plan_run",
            "run_plan", "ultra_plan", "ultra_plan_run", "run_ultra_plan",
            "setup_interaction_probe", "runs", "ux_demo", "model_probe", "doctor", "completions",
            "generate_man", "validate_manifest", "init_profile"
        ],
        help_heading = "Actions (use one)",
        help = "Create `.commandagent/config.toml` from a starter template without overwriting an existing file."
    )]
    pub init_config: bool,
    #[arg(
        long,
        value_name = "PATH",
        conflicts_with_all = [
            "packs", "pack_verify", "pack_pin", "workflow", "prompt", "plan_steps", "plan_run",
            "run_plan", "ultra_plan", "ultra_plan_run", "run_ultra_plan",
            "setup_interaction_probe", "runs", "ux_demo", "model_probe", "doctor", "completions",
            "generate_man", "init_config", "init_profile"
        ],
        help_heading = "Actions (use one)",
        help = "Validate an external profile manifest without running it."
    )]
    pub validate_manifest: Option<PathBuf>,
    #[arg(
        long,
        value_name = "ID",
        requires = "extension_root",
        conflicts_with_all = [
            "packs", "pack_verify", "pack_pin", "workflow", "prompt", "plan_steps", "plan_run",
            "run_plan", "ultra_plan", "ultra_plan_run", "run_ultra_plan",
            "setup_interaction_probe", "runs", "ux_demo", "model_probe", "doctor", "completions",
            "generate_man", "init_config", "validate_manifest"
        ],
        help_heading = "Actions (use one)",
        help = "Initialize a draft profile manifest under `--extension-root`."
    )]
    pub init_profile: Option<String>,
    #[arg(
        long,
        help_heading = "Planning and Verification",
        help = "Set a compiled profile or an external draft ID. An external ID requires the extension root that declares `profiles/<id>/manifest.toml`."
    )]
    pub profile: Option<String>,
    #[arg(
        long,
        default_value = "default",
        help_heading = "Planning and Verification",
        help = "Pass the plan presentation/generation style."
    )]
    pub style: String,
    #[arg(
        long,
        help_heading = "Workspace and State",
        help = "Load the named saved minimal-loop session for a direct `--prompt` run."
    )]
    pub resume: Option<String>,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help_heading = "Workspace and State",
        help = "Block network-dependent dependency setup and checks; it does not turn a cloud model into an offline provider."
    )]
    pub offline: bool,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help_heading = "Display",
        help = "Suppress presentation narration."
    )]
    pub quiet: bool,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help_heading = "Display",
        help = "Append one machine-readable terminal run summary as the final stdout line. Omitting it preserves existing stdout bytes."
    )]
    pub summary_json: bool,
    #[arg(
        long,
        default_value = "http://localhost:11434",
        help_heading = "Models and Providers",
        help = "Set the Ollama server base URL used by CommandAgent."
    )]
    pub ollama_host: String,
    #[arg(
        long,
        value_enum,
        value_name = "true|false|low|medium|high",
        num_args = 0..=1,
        default_missing_value = "true",
        require_equals = true,
        help_heading = "Models and Providers",
        help = "Enable Ollama thinking for every Ollama provider role. A bare flag means `true`; explicit values require `=`, for example `--think=high`."
    )]
    pub think: Option<OllamaThinkArg>,
    #[arg(
        long,
        default_value = "http://localhost:1234",
        help_heading = "Models and Providers",
        help = "Set the LM Studio base URL; an optional trailing `/v1` is normalized."
    )]
    pub lm_studio_host: String,
    #[arg(
        long,
        default_value_t = 8_192,
        help_heading = "Models and Providers",
        help = "Set the maximum provider output-token request."
    )]
    pub num_predict: usize,
    #[arg(
        long,
        default_value_t = 12,
        value_parser = parse_positive_usize,
        help_heading = "Models and Providers",
        help = "Set the minimal-loop iteration budget."
    )]
    pub max_iterations: usize,
    #[arg(
        long,
        value_parser = clap::value_parser!(u64).range(1..),
        help_heading = "Models and Providers",
        help = "Set connect and whole-request timeouts for provider calls."
    )]
    pub chat_timeout_secs: Option<u64>,
    #[arg(
        long,
        default_value_t = 1,
        help_heading = "Models and Providers",
        help = "Set retries after the initial provider attempt."
    )]
    pub chat_retries: usize,
    #[arg(
        long,
        value_enum,
        value_name = "on|off",
        help_heading = "Display",
        help = "Control visible executor and repair streaming; planner machine output stays hidden. Streaming still requires an interactive stdin and stdout TTY."
    )]
    pub stream: Option<StreamArg>,
    #[arg(
        long,
        help_heading = "Workspace and State",
        help = "Override saved session and REPL history storage."
    )]
    pub state_dir: Option<PathBuf>,
    #[arg(
        long,
        help_heading = "Workspace and State",
        help = "Set and canonicalize the active workspace before config discovery and execution."
    )]
    pub cwd: Option<PathBuf>,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help_heading = "Workspace and State",
        help = "Ignore `--resume` and create a session for a direct `--prompt` run."
    )]
    pub fresh_session: bool,
    #[arg(
        long,
        value_enum,
        value_name = "on|off",
        help_heading = "Display",
        help = "Control the fixed TUI footer; off keeps scrollback breadcrumbs."
    )]
    pub footer: Option<FooterArg>,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        conflicts_with = "footer",
        help_heading = "Display",
        help = "Disable the fixed TUI footer. Equivalent in effect to `--footer off`."
    )]
    pub no_footer: bool,
    #[arg(long, hide = true)]
    pub completion_contract_json: Option<PathBuf>,
    #[arg(
        trailing_var_arg = true,
        help = "Describe the goal for plan actions; multiple trailing words are joined."
    )]
    pub goal: Vec<String>,
}

impl Cli {
    pub fn trailing_goal(&self) -> Option<String> {
        let joined = self.goal.join(" ");
        let trimmed = joined.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn help_does_not_include_engine() {
        let help = Cli::command().render_long_help().to_string();
        assert!(!help.contains("--engine"));
    }

    #[test]
    fn yes_help_preserves_trusted_workspace_warning() {
        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("recognized Bash writes remain workspace-confined"));
        assert!(help.contains("Use only in a trusted workspace"));
    }

    #[test]
    fn help_includes_no_footer() {
        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("--no-footer"));
    }

    #[test]
    fn help_includes_footer_mode() {
        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("--footer"));
        assert!(help.contains("on|off"));
    }

    #[test]
    fn help_includes_stream_mode() {
        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("--stream"));
        assert!(help.contains("on|off"));
    }

    #[test]
    fn think_parses_bare_and_explicit_values() {
        let bare = Cli::try_parse_from(["commandagent", "--think"]).unwrap();
        assert_eq!(bare.think, Some(OllamaThinkArg::True));

        for (input, expected) in [
            ("true", OllamaThinkArg::True),
            ("false", OllamaThinkArg::False),
            ("low", OllamaThinkArg::Low),
            ("medium", OllamaThinkArg::Medium),
            ("high", OllamaThinkArg::High),
        ] {
            let argument = format!("--think={input}");
            let cli = Cli::try_parse_from(["commandagent", argument.as_str()]).unwrap();
            assert_eq!(cli.think, Some(expected));
        }
    }

    #[test]
    fn think_requires_equals_for_an_explicit_value() {
        let cli = Cli::try_parse_from(["commandagent", "--think", "high"]).unwrap();

        assert_eq!(cli.think, Some(OllamaThinkArg::True));
        assert_eq!(cli.goal, vec!["high"]);
    }

    #[test]
    fn invalid_think_value_is_rejected_before_execution() {
        let error = Cli::try_parse_from(["commandagent", "--think=maximum"]).unwrap_err();
        assert!(error.to_string().contains("invalid value 'maximum'"));
    }

    #[test]
    fn help_includes_think_values() {
        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("--think[=<true|false|low|medium|high>]"));
        assert!(help.contains("A bare flag means `true`"));
    }

    #[test]
    fn help_includes_prompt_layout() {
        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("--prompt-layout"));
        assert!(help.contains("stable|legacy"));
    }

    #[test]
    fn help_includes_plan_preset() {
        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("--plan-preset"));
        assert!(help.contains("profile|none"));
        assert!(help.contains("Override planner-tier UltraPlan preset selection"));
        assert!(help.contains("`data/fix` can synthesize F1–F3 steps"));
        assert!(help.contains("`nextjs/fix` remains none-equivalent"));
    }

    #[test]
    fn help_includes_intent() {
        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("--intent"));
        assert!(help.contains("create|fix|investigate"));
        assert!(help.contains("Force intent instead of goal-based resolution"));
    }

    #[test]
    fn help_includes_pack_direct_actions() {
        let help = Cli::command().render_long_help().to_string();
        for flag in ["--packs", "--pack-verify <DIR>", "--pack-pin <DIR>"] {
            assert!(help.contains(flag), "missing {flag} from help:\n{help}");
        }
    }

    #[test]
    fn pack_direct_actions_are_mutually_exclusive() {
        for arguments in [
            vec![
                "--profile",
                "python-cli",
                "--intent",
                "create",
                "--packs",
                "--pack-verify",
                "packs/cli-assist/1.0.0",
            ],
            vec![
                "--profile",
                "python-cli",
                "--intent",
                "create",
                "--packs",
                "--pack-pin",
                "packs/cli-assist/1.0.0",
            ],
            vec![
                "--pack-verify",
                "packs/cli-assist/1.0.0",
                "--pack-pin",
                "packs/cli-assist/1.0.0",
            ],
        ] {
            let error =
                Cli::try_parse_from(std::iter::once("commandagent").chain(arguments)).unwrap_err();
            assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
        }
    }

    #[test]
    fn pack_direct_actions_conflict_with_run_and_selection_actions() {
        for arguments in [
            vec!["--pack-verify", "pack-dir", "--prompt", "run something"],
            vec!["--pack-pin", "pack-dir", "--pack", "cli-assist@1.0.0"],
            vec![
                "--profile",
                "python-cli",
                "--intent",
                "create",
                "--packs",
                "--runs",
            ],
        ] {
            let error =
                Cli::try_parse_from(std::iter::once("commandagent").chain(arguments)).unwrap_err();
            assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
        }
    }

    #[test]
    fn packs_requires_profile_and_intent_but_allows_an_extension_root() {
        for arguments in [
            vec!["commandagent", "--packs"],
            vec!["commandagent", "--profile", "python-cli", "--packs"],
        ] {
            assert!(Cli::try_parse_from(arguments).is_err());
        }
        let cli = Cli::try_parse_from([
            "commandagent",
            "--extension-root",
            "local-packs",
            "--profile",
            "python-cli",
            "--intent",
            "create",
            "--packs",
        ])
        .unwrap();
        assert!(cli.packs);
    }

    #[test]
    fn intent_parses_three_values_and_omission() {
        let create = Cli::try_parse_from(["commandagent", "--intent", "create"]).unwrap();
        let fix = Cli::try_parse_from(["commandagent", "--intent", "fix"]).unwrap();
        let investigate = Cli::try_parse_from(["commandagent", "--intent", "investigate"]).unwrap();
        let omitted = Cli::try_parse_from(["commandagent"]).unwrap();

        assert_eq!(create.intent, Some(IntentArg::Create));
        assert_eq!(fix.intent, Some(IntentArg::Fix));
        assert_eq!(investigate.intent, Some(IntentArg::Investigate));
        assert_eq!(omitted.intent, None);
    }

    #[test]
    fn invalid_intent_is_rejected_before_execution() {
        let error = Cli::try_parse_from(["commandagent", "--intent", "research"]).unwrap_err();
        assert!(error.to_string().contains("invalid value 'research'"));
    }

    #[test]
    fn help_includes_runs() {
        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("--runs"));
    }

    #[test]
    fn help_includes_ux_demo() {
        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("--ux-demo"));
    }

    #[test]
    fn help_includes_model_probe() {
        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("--model-probe"));
    }

    #[test]
    fn help_includes_doctor_and_json() {
        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("--doctor"));
        assert!(help.contains("--json"));
        assert!(help.contains("machine-readable JSON"));
    }

    #[test]
    fn help_includes_headless_summary_json() {
        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("--summary-json"));
        assert!(help.contains("final stdout line"));
    }

    #[test]
    fn help_includes_generated_cli_artifacts() {
        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("--completions <SHELL>"));
        assert!(help.contains("bash"));
        assert!(help.contains("zsh"));
        assert!(help.contains("fish"));
        assert!(help.contains("powershell"));
        assert!(help.contains("--generate-man"));
    }

    #[test]
    fn help_groups_public_flags_by_user_task() {
        let help = Cli::command().render_long_help().to_string();
        for heading in [
            "Actions (use one):",
            "Models and Providers:",
            "Planning and Verification:",
            "Workspace and State:",
            "Display:",
        ] {
            assert!(
                help.contains(heading),
                "missing {heading} from help:\n{help}"
            );
        }
    }

    #[test]
    fn zero_iteration_and_timeout_values_are_rejected_by_clap() {
        for arguments in [
            ["commandagent", "--max-iterations", "0"],
            ["commandagent", "--chat-timeout-secs", "0"],
        ] {
            let error = Cli::try_parse_from(arguments).unwrap_err();
            assert_eq!(error.kind(), clap::error::ErrorKind::ValueValidation);
        }
    }

    #[test]
    fn manifest_lane_arguments_parse_without_backend_behavior() {
        let validate = Cli::try_parse_from([
            "commandagent",
            "--validate-manifest",
            "profiles/static-site/manifest.toml",
        ])
        .unwrap();
        assert_eq!(
            validate.validate_manifest.as_deref(),
            Some(std::path::Path::new("profiles/static-site/manifest.toml"))
        );

        let init = Cli::try_parse_from([
            "commandagent",
            "--extension-root",
            "extensions",
            "--init-profile",
            "static-site",
        ])
        .unwrap();
        assert_eq!(init.init_profile.as_deref(), Some("static-site"));

        let error =
            Cli::try_parse_from(["commandagent", "--init-profile", "static-site"]).unwrap_err();
        assert_eq!(
            error.kind(),
            clap::error::ErrorKind::MissingRequiredArgument
        );
    }

    #[test]
    fn generated_cli_artifacts_are_mutually_exclusive() {
        let error =
            Cli::try_parse_from(["commandagent", "--completions", "bash", "--generate-man"])
                .unwrap_err();
        assert!(error.to_string().contains("cannot be used with"));
    }

    #[test]
    fn json_requires_doctor() {
        let error = Cli::try_parse_from(["commandagent", "--json"]).unwrap_err();
        assert!(error.to_string().contains("--doctor"));

        let cli = Cli::try_parse_from(["commandagent", "--doctor", "--json"]).unwrap();
        assert!(cli.doctor);
        assert!(cli.json);
    }

    #[test]
    fn help_includes_quiet() {
        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("--quiet"));
    }

    #[test]
    fn help_hides_completion_contract_json() {
        let help = Cli::command().render_long_help().to_string();
        assert!(!help.contains("--completion-contract-json"));
    }

    #[test]
    fn version_includes_embedded_build_commit_or_unknown() {
        let version = Cli::command().render_version().to_string();
        assert!(version.contains(env!("CARGO_PKG_VERSION")), "{version}");
        assert!(
            version.contains(crate::build_info::COMMIT) || version.contains("unknown"),
            "{version}"
        );
        assert!(version.contains(crate::build_info::TIMESTAMP), "{version}");
    }

    #[test]
    fn sidecar_model_is_rejected_by_default() {
        let err = Cli::try_parse_from(["commandagent", "--sidecar-model", "x"]).unwrap_err();
        assert!(err.to_string().contains("unexpected argument"));
    }

    #[test]
    fn num_predict_defaults_to_source_minimal_budget() {
        let cli = Cli::parse_from(["commandagent"]);
        assert_eq!(cli.num_predict, 8_192);
    }

    #[test]
    fn config_preset_fields_are_absent_until_resolved() {
        let cli = Cli::parse_from(["commandagent"]);
        assert_eq!(cli.model, None);
        assert_eq!(cli.provider, None);
        assert_eq!(cli.context_budget, None);
    }

    #[test]
    fn lm_studio_provider_and_host_parse() {
        let cli = Cli::parse_from([
            "commandagent",
            "--provider",
            "lm-studio",
            "--planner-provider",
            "lm-studio",
            "--lm-studio-host",
            "http://127.0.0.1:4321/v1",
        ]);

        assert_eq!(cli.provider, Some(ProviderArg::LmStudio));
        assert_eq!(cli.planner_provider, Some(ProviderArg::LmStudio));
        assert_eq!(cli.lm_studio_host, "http://127.0.0.1:4321/v1");
    }

    #[test]
    fn chat_timeout_defaults_to_source_config() {
        let cli = Cli::parse_from(["commandagent"]);
        assert_eq!(cli.chat_timeout_secs, None);
    }

    #[test]
    fn ultra_plan_run_allows_profile_before_goal() {
        let cli = Cli::try_parse_from([
            "commandagent",
            "--provider",
            "ollama",
            "--model",
            "qwen3.6:27b-coding-nvfp4",
            "--planner-provider",
            "gemini",
            "--planner-model",
            "gemini-3.5-flash",
            "--ultra-plan-run",
            "--profile",
            "nextjs",
            "3011 port app",
        ])
        .unwrap();
        assert!(cli.ultra_plan_run);
        assert_eq!(cli.profile.as_deref(), Some("nextjs"));
        assert_eq!(cli.trailing_goal().as_deref(), Some("3011 port app"));
    }

    #[test]
    fn profile_absent_is_distinguishable_from_explicit_generic() {
        let implicit = Cli::parse_from(["commandagent"]);
        assert_eq!(implicit.profile, None);
        let explicit = Cli::parse_from(["commandagent", "--profile", "generic"]);
        assert_eq!(explicit.profile.as_deref(), Some("generic"));
    }
}
