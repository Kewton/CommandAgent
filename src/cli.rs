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

#[derive(Debug, Clone, Parser)]
#[command(name = "commandagent")]
#[command(about = "Minimal loop + YAML plan runner MVP")]
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
        help = "Auto-approve mutating tools; recognized Bash writes remain workspace-confined"
    )]
    pub yes: bool,
    #[arg(long)]
    pub preset: Option<String>,
    #[arg(
        long,
        value_name = "ID@VERSION",
        help = "Activate an exact-version assist/eval pack"
    )]
    pub pack: Option<String>,
    #[arg(
        long,
        value_name = "SHA256",
        requires = "pack",
        help = "Require the selected pack's exact-byte hash"
    )]
    pub pack_hash: Option<String>,
    #[arg(
        long,
        value_name = "DIR",
        help = "Search this extension root before repository packs"
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
        help = "List compatible admitted and local packs"
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
        help = "Verify strict conformance for a pack directory"
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
        help = "Create or validate a pack.sha256 pin"
    )]
    pub pack_pin: Option<PathBuf>,
    #[arg(long)]
    pub context_budget: Option<usize>,
    #[arg(long)]
    pub model: Option<String>,
    #[arg(long, value_enum)]
    pub provider: Option<ProviderArg>,
    #[arg(
        long = "api",
        value_enum,
        value_name = "chat-completions|responses",
        help = "Declare the OpenAI-compatible API surface; omitted keeps chat-completions"
    )]
    pub openai_api: Option<OpenAiApiArg>,
    #[arg(
        long,
        value_enum,
        value_name = "native|text",
        help = "Declare the executor tool protocol; omitted delegates to the provider default"
    )]
    pub tool_protocol: Option<ToolProtocolArg>,
    #[arg(
        long,
        value_enum,
        value_name = "stable|legacy",
        help = "Choose prompt section order for A/B measurement"
    )]
    pub prompt_layout: Option<PromptLayoutArg>,
    #[arg(
        long,
        value_enum,
        value_name = "profile|none",
        help = "Override planner-tier UltraPlan preset selection; data/fix synthesizes F1-F3 steps, while nextjs/fix remains none-equivalent"
    )]
    pub plan_preset: Option<PlanPresetArg>,
    #[arg(
        long,
        value_enum,
        value_name = "create|fix|investigate",
        help = "Select create, fix, or investigate intent explicitly; omitted keeps goal-based resolution"
    )]
    pub intent: Option<IntentArg>,
    #[arg(long, conflicts_with = "intent")]
    pub workflow: Option<PathBuf>,
    #[arg(long, requires = "workflow")]
    pub origin: Option<PathBuf>,
    #[arg(long)]
    pub planner_model: Option<String>,
    #[arg(long, value_enum)]
    pub planner_provider: Option<ProviderArg>,
    #[arg(long)]
    pub prompt: Option<String>,
    #[arg(long, action = ArgAction::SetTrue)]
    pub plan_steps: bool,
    #[arg(long, action = ArgAction::SetTrue)]
    pub plan_run: bool,
    #[arg(long)]
    pub run_plan: Option<PathBuf>,
    #[arg(long, action = ArgAction::SetTrue)]
    pub ultra_plan: bool,
    #[arg(long, action = ArgAction::SetTrue)]
    pub ultra_plan_run: bool,
    #[arg(long)]
    pub run_ultra_plan: Option<PathBuf>,
    #[arg(long, action = ArgAction::SetTrue)]
    pub setup_interaction_probe: bool,
    #[arg(long, action = ArgAction::SetTrue, help = "List recent runs for the current workspace")]
    pub runs: bool,
    #[arg(long, action = ArgAction::SetTrue, help = "Run the offline presentation UX demo")]
    pub ux_demo: bool,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help = "Run the bounded model behavior probe battery"
    )]
    pub model_probe: bool,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help = "Diagnose configuration, providers, probes, and local environment"
    )]
    pub doctor: bool,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        requires = "doctor",
        help = "Render doctor output as stable machine-readable JSON"
    )]
    pub json: bool,
    #[arg(
        long,
        value_enum,
        value_name = "SHELL",
        conflicts_with = "generate_man",
        help = "Generate shell completions to stdout"
    )]
    pub completions: Option<Shell>,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        conflicts_with = "completions",
        help = "Generate a commandagent(1) man page to stdout"
    )]
    pub generate_man: bool,
    #[arg(long)]
    pub profile: Option<String>,
    #[arg(long, default_value = "default")]
    pub style: String,
    #[arg(long)]
    pub resume: Option<String>,
    #[arg(long, action = ArgAction::SetTrue)]
    pub offline: bool,
    #[arg(long, action = ArgAction::SetTrue, help = "Keep presentation narration quiet")]
    pub quiet: bool,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        help = "Print one machine-readable run summary JSON object as the final stdout line"
    )]
    pub summary_json: bool,
    #[arg(long, default_value = "http://localhost:11434")]
    pub ollama_host: String,
    #[arg(
        long,
        value_enum,
        value_name = "true|false|low|medium|high",
        num_args = 0..=1,
        default_missing_value = "true",
        require_equals = true,
        help = "Enable Ollama thinking for every Ollama provider role; bare --think means true"
    )]
    pub think: Option<OllamaThinkArg>,
    #[arg(long, default_value = "http://localhost:1234")]
    pub lm_studio_host: String,
    #[arg(long, default_value_t = 8_192)]
    pub num_predict: usize,
    #[arg(long, default_value_t = 12)]
    pub max_iterations: usize,
    #[arg(long)]
    pub chat_timeout_secs: Option<u64>,
    #[arg(long, default_value_t = 1)]
    pub chat_retries: usize,
    #[arg(
        long,
        value_enum,
        value_name = "on|off",
        help = "Stream assistant output in an interactive TTY REPL"
    )]
    pub stream: Option<StreamArg>,
    #[arg(long)]
    pub state_dir: Option<PathBuf>,
    #[arg(long)]
    pub cwd: Option<PathBuf>,
    #[arg(long, action = ArgAction::SetTrue)]
    pub fresh_session: bool,
    #[arg(
        long,
        value_enum,
        value_name = "on|off",
        help = "Control the fixed TUI footer; off keeps scrollback breadcrumbs only"
    )]
    pub footer: Option<FooterArg>,
    #[arg(
        long,
        action = ArgAction::SetTrue,
        conflicts_with = "footer",
        help = "Disable the fixed TUI footer"
    )]
    pub no_footer: bool,
    #[arg(long, hide = true)]
    pub completion_contract_json: Option<PathBuf>,
    #[arg(trailing_var_arg = true)]
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
    fn yes_help_preserves_workspace_confinement_warning() {
        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("recognized Bash writes remain workspace-confined"));
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
        assert!(help.contains("bare --think means true"));
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
        assert!(help.contains("data/fix synthesizes F1-F3 steps"));
        assert!(help.contains("nextjs/fix remains none-equivalent"));
    }

    #[test]
    fn help_includes_intent() {
        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("--intent"));
        assert!(help.contains("create|fix|investigate"));
        assert!(help.contains("omitted keeps goal-based resolution"));
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
