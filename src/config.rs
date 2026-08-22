use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

use crate::cli::{
    Cli, FooterArg, IntentArg, OllamaThinkArg, OpenAiApiArg, PlanPresetArg, PromptLayoutArg,
    ProviderArg, StreamArg, ToolProtocolArg,
};
pub use crate::planner::adjudication::contract::IntentId;
use crate::planner::intent::detect_intent;
use crate::planner::profile::{ProfileInference, infer_profile, resolve_profile_runtime};

pub const LOCAL_PROVIDER_CHAT_TIMEOUT_SECS: u64 = 600;
pub const REMOTE_PROVIDER_CHAT_TIMEOUT_SECS: u64 = 180;
pub const DEFAULT_CONTEXT_BUDGET: usize = 65_536;
pub const DEFAULT_MODEL: &str = "qwen3.6:27b-coding-nvfp4";
pub const SUPPORTED_PRESET_KEYS: &[&str] = &[
    "pack",
    "model",
    "provider",
    "api",
    "tool_protocol",
    "planner_model",
    "planner_provider",
    "planner_think",
    "classifier_model",
    "classifier_provider",
    "context_budget",
    "chat_timeout_secs",
    "profile",
    "narration",
    "footer",
    "stream",
    "prompt_layout",
    "plan_preset",
];
pub const SUPPORTED_TOP_LEVEL_KEYS: &[&str] = &[
    "extension_root",
    "narration",
    "footer",
    "stream",
    "prompt_layout",
    "plan_preset",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Provider {
    Ollama,
    LmStudio,
    Openai,
    Gemini,
}

impl Provider {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ollama => "ollama",
            Self::LmStudio => "lm-studio",
            Self::Openai => "openai",
            Self::Gemini => "gemini",
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Ollama => "Ollama",
            Self::LmStudio => "LM Studio",
            Self::Openai => "OpenAI",
            Self::Gemini => "Gemini",
        }
    }

    pub const fn is_local(self) -> bool {
        matches!(self, Self::Ollama | Self::LmStudio)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OllamaThink {
    True,
    False,
    Low,
    Medium,
    High,
}

impl OllamaThink {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::True => "true",
            Self::False => "false",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

impl From<OllamaThinkArg> for OllamaThink {
    fn from(value: OllamaThinkArg) -> Self {
        match value {
            OllamaThinkArg::True => Self::True,
            OllamaThinkArg::False => Self::False,
            OllamaThinkArg::Low => Self::Low,
            OllamaThinkArg::Medium => Self::Medium,
            OllamaThinkArg::High => Self::High,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ToolProtocol {
    Native,
    Text,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenAiApi {
    #[default]
    ChatCompletions,
    Responses,
}

impl OpenAiApi {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ChatCompletions => "chat_completions",
            Self::Responses => "responses",
        }
    }

    fn from_config_value(value: &str) -> Option<Self> {
        match value.trim() {
            "chat_completions" => Some(Self::ChatCompletions),
            "responses" => Some(Self::Responses),
            _ => None,
        }
    }
}

impl From<OpenAiApiArg> for OpenAiApi {
    fn from(value: OpenAiApiArg) -> Self {
        match value {
            OpenAiApiArg::ChatCompletions => Self::ChatCompletions,
            OpenAiApiArg::Responses => Self::Responses,
        }
    }
}

impl ToolProtocol {
    fn from_config_value(value: &str) -> Option<Self> {
        match value.trim() {
            "native" => Some(Self::Native),
            "text" => Some(Self::Text),
            _ => None,
        }
    }
}

impl From<ToolProtocolArg> for ToolProtocol {
    fn from(value: ToolProtocolArg) -> Self {
        match value {
            ToolProtocolArg::Native => Self::Native,
            ToolProtocolArg::Text => Self::Text,
        }
    }
}

impl From<ProviderArg> for Provider {
    fn from(value: ProviderArg) -> Self {
        match value {
            ProviderArg::Ollama => Self::Ollama,
            ProviderArg::LmStudio => Self::LmStudio,
            ProviderArg::Openai => Self::Openai,
            ProviderArg::Gemini => Self::Gemini,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FooterMode {
    #[default]
    On,
    Off,
}

impl FooterMode {
    pub fn is_off(self) -> bool {
        matches!(self, Self::Off)
    }

    fn from_config_value(value: &str) -> Option<Self> {
        match value.trim() {
            "on" => Some(Self::On),
            "off" => Some(Self::Off),
            _ => None,
        }
    }
}

impl From<FooterArg> for FooterMode {
    fn from(value: FooterArg) -> Self {
        match value {
            FooterArg::On => Self::On,
            FooterArg::Off => Self::Off,
        }
    }
}

fn stream_arg_enabled(value: StreamArg) -> bool {
    matches!(value, StreamArg::On)
}

fn parse_stream_mode(value: &str) -> Option<bool> {
    match value.trim() {
        "on" => Some(true),
        "off" => Some(false),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PromptLayout {
    #[default]
    Legacy,
    Stable,
}

impl PromptLayout {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Legacy => "legacy",
        }
    }

    fn from_config_value(value: &str) -> Option<Self> {
        match value.trim() {
            "stable" => Some(Self::Stable),
            "legacy" => Some(Self::Legacy),
            _ => None,
        }
    }
}

impl From<PromptLayoutArg> for PromptLayout {
    fn from(value: PromptLayoutArg) -> Self {
        match value {
            PromptLayoutArg::Stable => Self::Stable,
            PromptLayoutArg::Legacy => Self::Legacy,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlanPreset {
    #[default]
    None,
    Profile,
}

impl PlanPreset {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Profile => "profile",
        }
    }

    fn from_config_value(value: &str) -> Option<Self> {
        match value.trim() {
            "none" => Some(Self::None),
            "profile" => Some(Self::Profile),
            _ => None,
        }
    }
}

impl From<PlanPresetArg> for PlanPreset {
    fn from(value: PlanPresetArg) -> Self {
        match value {
            PlanPresetArg::None => Self::None,
            PlanPresetArg::Profile => Self::Profile,
        }
    }
}

impl From<IntentArg> for IntentId {
    fn from(value: IntentArg) -> Self {
        match value {
            IntentArg::Create => Self::Create,
            IntentArg::Fix => Self::Fix,
            IntentArg::Investigate => Self::Investigate,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Action {
    Repl,
    Prompt(String),
    PlanSteps(String),
    PlanRun(String),
    RunPlan(PathBuf),
    UltraPlan(String),
    UltraPlanRun(String),
    RunUltraPlan(PathBuf),
    SetupInteractionProbe,
    Runs,
    UxDemo,
    ModelProbe,
    Doctor,
    Workflow(PathBuf, PathBuf),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NarrationMode {
    #[default]
    Normal,
    Quiet,
}

impl NarrationMode {
    pub fn is_quiet(self) -> bool {
        matches!(self, Self::Quiet)
    }

    fn from_config_value(value: &str) -> Option<Self> {
        match value.trim() {
            "normal" => Some(Self::Normal),
            "quiet" => Some(Self::Quiet),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub workspace_root: PathBuf,
    pub state_dir: PathBuf,
    pub eval_events_path: Option<PathBuf>,
    pub completion_contract_path: Option<PathBuf>,
    pub yes: bool,
    pub offline: bool,
    pub context_budget: usize,
    pub model: String,
    pub provider: Provider,
    pub openai_api: OpenAiApi,
    pub tool_protocol: Option<ToolProtocol>,
    pub prompt_layout: PromptLayout,
    pub plan_preset: PlanPreset,
    pub intent_override: Option<IntentId>,
    pub planner_model: String,
    pub planner_provider: Provider,
    pub planner_think: Option<OllamaThink>,
    pub classifier_model: String,
    pub classifier_provider: Provider,
    pub ollama_host: String,
    pub ollama_think: Option<OllamaThink>,
    pub lm_studio_host: String,
    pub num_predict: usize,
    pub max_iterations: usize,
    pub chat_timeout_secs: u64,
    pub chat_timeout_source: String,
    pub field_sources: ConfigFieldSources,
    pub chat_retries: usize,
    pub stream: bool,
    pub resume: Option<String>,
    pub fresh_session: bool,
    pub no_footer: bool,
    pub narration: NarrationMode,
    pub profile: String,
    pub profile_explicit: bool,
    pub profile_inference: Option<ProfileInference>,
    pub style: String,
    pub action: Action,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigFieldSources {
    pub model: String,
    pub provider: String,
    pub planner_model: String,
    pub planner_provider: String,
    pub planner_think: String,
    pub classifier_model: String,
    pub classifier_provider: String,
    pub context_budget: String,
    pub chat_timeout_secs: String,
    pub prompt_layout: String,
    pub plan_preset: String,
    pub profile: String,
    pub narration: String,
    pub footer: String,
    pub stream: String,
}

impl Default for ConfigFieldSources {
    fn default() -> Self {
        Self {
            model: "default".to_string(),
            provider: "default".to_string(),
            planner_model: "default".to_string(),
            planner_provider: "default".to_string(),
            planner_think: "default".to_string(),
            classifier_model: "default".to_string(),
            classifier_provider: "default".to_string(),
            context_budget: "default".to_string(),
            chat_timeout_secs: "default".to_string(),
            prompt_layout: "default".to_string(),
            plan_preset: "default".to_string(),
            profile: "default".to_string(),
            narration: "default".to_string(),
            footer: "default".to_string(),
            stream: "default".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Sourced<T> {
    value: T,
    source: String,
}

fn sourced<T>(value: T, source: impl Into<String>) -> Sourced<T> {
    Sourced {
        value,
        source: source.into(),
    }
}

#[derive(Debug, Clone, Default)]
struct PresetConfig {
    pack: Option<Sourced<String>>,
    model: Option<Sourced<String>>,
    provider: Option<Sourced<Provider>>,
    openai_api: Option<Sourced<OpenAiApi>>,
    tool_protocol: Option<Sourced<ToolProtocol>>,
    planner_model: Option<Sourced<String>>,
    planner_provider: Option<Sourced<Provider>>,
    planner_think: Option<Sourced<OllamaThink>>,
    classifier_model: Option<Sourced<String>>,
    classifier_provider: Option<Sourced<Provider>>,
    context_budget: Option<Sourced<usize>>,
    chat_timeout_secs: Option<Sourced<u64>>,
    prompt_layout: Option<Sourced<PromptLayout>>,
    plan_preset: Option<Sourced<PlanPreset>>,
    profile: Option<Sourced<String>>,
    narration: Option<Sourced<NarrationMode>>,
    footer: Option<Sourced<FooterMode>>,
    stream: Option<Sourced<bool>>,
}

#[derive(Debug, Clone, Default)]
struct ConfigFile {
    presets: HashMap<String, PresetConfig>,
    extension_root: Option<Sourced<String>>,
    narration: Option<Sourced<NarrationMode>>,
    footer: Option<Sourced<FooterMode>>,
    stream: Option<Sourced<bool>>,
    prompt_layout: Option<Sourced<PromptLayout>>,
    plan_preset: Option<Sourced<PlanPreset>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConfigPathInspection {
    pub path: PathBuf,
    pub exists: bool,
    pub parse_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PresetInspection {
    pub name: String,
    pub found: bool,
    pub complete: bool,
    pub missing_keys: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConfigInspection {
    pub paths: Vec<ConfigPathInspection>,
    pub preset: Option<PresetInspection>,
    pub inspection_errors: Vec<String>,
}

impl Config {
    pub fn streaming_enabled(&self) -> bool {
        self.streaming_enabled_for_tty(
            crate::tui::terminal::stdin_is_tty(),
            crate::tui::terminal::stdout_is_tty(),
        )
    }

    pub(crate) fn streaming_enabled_for_tty(&self, stdin_tty: bool, stdout_tty: bool) -> bool {
        self.stream && matches!(self.action, Action::Repl) && stdin_tty && stdout_tty
    }

    pub fn plan_preset_origin(&self) -> &'static str {
        if self.field_sources.plan_preset == "default_create_ingest" {
            return "default_create_ingest";
        }
        if self.field_sources.plan_preset == "default_fix_data" {
            return "default_fix_data";
        }
        if self.field_sources.plan_preset == "default_investigate_data" {
            return "default_investigate_data";
        }
        config_source_origin(&self.field_sources.plan_preset)
    }

    pub fn resolved_intent(&self, goal: &str) -> &'static str {
        self.intent_override
            .map(IntentId::as_str)
            .unwrap_or_else(|| detect_intent(goal))
    }

    pub fn resolved_run_intent(&self) -> IntentId {
        self.intent_override.unwrap_or_else(|| {
            if action_goal(&self.action).is_some_and(|goal| detect_intent(goal) == "fix") {
                IntentId::Fix
            } else {
                IntentId::Create
            }
        })
    }

    pub const fn intent_origin(&self) -> &'static str {
        if self.intent_override.is_some() {
            "cli"
        } else {
            "default"
        }
    }

    pub fn intent_source(&self) -> &'static str {
        self.intent_override.map(IntentId::as_str).unwrap_or("")
    }

    pub fn apply_intent_override(&self, intent: &mut String) {
        if let Some(value) = self.intent_override {
            *intent = value.as_str().to_string();
        }
    }

    pub fn from_cli(cli: Cli) -> anyhow::Result<Self> {
        let workspace_root = cli
            .cwd
            .clone()
            .unwrap_or(std::env::current_dir().context("failed to read current directory")?)
            .canonicalize()
            .context("failed to canonicalize workspace root")?;
        let preset = load_named_preset(&workspace_root, cli.preset.as_deref())?;
        let extension_root = cli
            .extension_root
            .clone()
            .or(configured_extension_root(&workspace_root)?);
        if let Some(extension_root) = extension_root.as_deref() {
            crate::planner::extension_profiles::register(extension_root).with_context(|| {
                format!(
                    "load draft profiles from extension root {}",
                    extension_root.display()
                )
            })?;
        }
        let model = cli
            .model
            .clone()
            .map(|value| sourced(value, "flag"))
            .or_else(|| preset.as_ref().and_then(|preset| preset.model.clone()))
            .unwrap_or_else(|| sourced(DEFAULT_MODEL.to_string(), "default"));
        let provider = cli
            .provider
            .map(|value| sourced(Provider::from(value), "flag"))
            .or_else(|| preset.as_ref().and_then(|preset| preset.provider.clone()))
            .unwrap_or_else(|| sourced(Provider::Ollama, "default"));
        let openai_api = cli
            .openai_api
            .map(|value| sourced(OpenAiApi::from(value), "flag"))
            .or_else(|| preset.as_ref().and_then(|preset| preset.openai_api.clone()))
            .unwrap_or_else(|| sourced(OpenAiApi::ChatCompletions, "default"));
        let tool_protocol = cli
            .tool_protocol
            .map(|value| sourced(ToolProtocol::from(value), "flag"))
            .or_else(|| {
                preset
                    .as_ref()
                    .and_then(|preset| preset.tool_protocol.clone())
            });
        let planner_provider = cli
            .planner_provider
            .map(|value| sourced(Provider::from(value), "flag"))
            .or_else(|| {
                preset
                    .as_ref()
                    .and_then(|preset| preset.planner_provider.clone())
            })
            .unwrap_or_else(|| sourced(provider.value, "default"));
        let planner_model = cli
            .planner_model
            .clone()
            .map(|value| sourced(value, "flag"))
            .or_else(|| {
                preset
                    .as_ref()
                    .and_then(|preset| preset.planner_model.clone())
            })
            .or_else(|| {
                (planner_provider.value == provider.value)
                    .then(|| sourced(model.value.clone(), "default"))
            });
        let Some(planner_model) = planner_model else {
            bail!("--planner-model is required when --planner-provider differs from --provider");
        };
        let ollama_think = cli.think.map(OllamaThink::from);
        let planner_think = ollama_think
            .map(|value| sourced(value, "flag"))
            .or_else(|| {
                preset
                    .as_ref()
                    .and_then(|preset| preset.planner_think.clone())
            })
            .unwrap_or_else(|| sourced(OllamaThink::False, "default"));
        if ollama_think.is_some()
            && provider.value != Provider::Ollama
            && planner_provider.value != Provider::Ollama
        {
            bail!("--think requires provider or planner_provider to be ollama");
        }
        validate_openai_model(provider.value, &model.value, "executor")?;
        validate_openai_model(planner_provider.value, &planner_model.value, "planner")?;
        let classifier_provider = preset
            .as_ref()
            .and_then(|preset| preset.classifier_provider.clone())
            .unwrap_or_else(|| sourced(planner_provider.value, "default:planner"));
        let classifier_model = preset
            .as_ref()
            .and_then(|preset| preset.classifier_model.clone())
            .or_else(|| {
                (classifier_provider.value == planner_provider.value)
                    .then(|| sourced(planner_model.value.clone(), "default:planner"))
            });
        let Some(classifier_model) = classifier_model else {
            bail!(
                "classifier_model is required when classifier_provider differs from planner_provider"
            );
        };
        validate_openai_model(
            classifier_provider.value,
            &classifier_model.value,
            "classifier",
        )?;
        let context_budget = cli
            .context_budget
            .map(|value| sourced(value, "flag"))
            .or_else(|| {
                preset
                    .as_ref()
                    .and_then(|preset| preset.context_budget.clone())
            })
            .unwrap_or_else(|| sourced(DEFAULT_CONTEXT_BUDGET, "default"));
        let chat_timeout = cli
            .chat_timeout_secs
            .map(|value| sourced(value, "flag"))
            .or_else(|| {
                preset
                    .as_ref()
                    .and_then(|preset| preset.chat_timeout_secs.clone())
            });
        let (chat_timeout_secs, chat_timeout_source) = match chat_timeout {
            Some(value) => (value.value, value.source),
            None => resolve_chat_timeout(
                None,
                provider.value,
                planner_provider.value,
                classifier_provider.value,
            ),
        };
        let prompt_layout = cli
            .prompt_layout
            .map(|value| sourced(PromptLayout::from(value), "flag"))
            .or_else(|| {
                preset
                    .as_ref()
                    .and_then(|preset| preset.prompt_layout.clone())
            })
            .or_else(|| config_file_prompt_layout(&workspace_root))
            .unwrap_or_else(|| sourced(PromptLayout::Legacy, "default"));
        let plan_preset = cli
            .plan_preset
            .map(|value| sourced(PlanPreset::from(value), "flag"))
            .or_else(|| {
                preset
                    .as_ref()
                    .and_then(|preset| preset.plan_preset.clone())
            })
            .or_else(|| config_file_plan_preset(&workspace_root))
            .or_else(|| {
                let declared_profile = cli.profile.as_deref().or_else(|| {
                    preset
                        .as_ref()
                        .and_then(|preset| preset.profile.as_ref())
                        .map(|profile| profile.value.as_str())
                });
                resolve_profile_runtime(declared_profile.unwrap_or("generic"))
                    .default_plan_preset(cli.intent.map(IntentId::from))
                    .map(|(preset, source)| sourced(preset, source))
            })
            .unwrap_or_else(|| default_plan_preset_for_planner(&planner_model.value));
        let state_dir = cli.state_dir.clone().unwrap_or_else(default_state_dir);
        let action = action_from_cli(&cli)?;
        let eval_events_path = crate::eval_events::path_from_env_or_default(&workspace_root);
        let narration = if cli.quiet {
            sourced(NarrationMode::Quiet, "flag")
        } else {
            preset
                .as_ref()
                .and_then(|preset| preset.narration.clone())
                .or_else(|| config_file_narration(&workspace_root))
                .unwrap_or_else(|| sourced(NarrationMode::Normal, "default"))
        };
        let footer = if cli.no_footer {
            sourced(FooterMode::Off, "flag")
        } else if let Some(footer) = cli.footer {
            sourced(FooterMode::from(footer), "flag")
        } else {
            preset
                .as_ref()
                .and_then(|preset| preset.footer.clone())
                .or_else(|| config_file_footer(&workspace_root))
                .unwrap_or_else(|| sourced(FooterMode::On, "default"))
        };
        let stream = cli
            .stream
            .map(|value| sourced(stream_arg_enabled(value), "flag"))
            .or_else(|| preset.as_ref().and_then(|preset| preset.stream.clone()))
            .or_else(|| config_file_stream(&workspace_root))
            .unwrap_or_else(|| {
                if matches!(action, Action::Repl) {
                    sourced(true, "default:repl")
                } else {
                    sourced(false, "default:non_interactive")
                }
            });
        let profile_from_preset = preset.as_ref().and_then(|preset| preset.profile.clone());
        let profile_explicit = cli.profile.is_some() || profile_from_preset.is_some();
        let profile_inference = if profile_explicit {
            None
        } else {
            infer_profile(action_goal(&action), &workspace_root)
        };
        let profile = cli
            .profile
            .clone()
            .map(|value| sourced(value, "flag"))
            .or(profile_from_preset.clone())
            .or_else(|| {
                profile_inference
                    .map(|inference| sourced(inference.profile.to_string(), "default:inferred"))
            })
            .unwrap_or_else(|| sourced("generic".to_string(), "default"));
        if profile_explicit
            && matches!(
                crate::planner::profile::ProfileId::parse(&profile.value),
                crate::planner::profile::ProfileId::Other(_)
            )
            && crate::planner::profile_descriptor::descriptor_for_name(&profile.value).is_none()
        {
            bail!(
                "draft profile `{}` requires an extension root that declares profiles/{}/manifest.toml (use --extension-root or configure extension_root)",
                profile.value,
                profile.value
            );
        }
        let field_sources = ConfigFieldSources {
            model: model.source.clone(),
            provider: provider.source.clone(),
            planner_model: planner_model.source.clone(),
            planner_provider: planner_provider.source.clone(),
            planner_think: planner_think.source.clone(),
            classifier_model: classifier_model.source.clone(),
            classifier_provider: classifier_provider.source.clone(),
            context_budget: context_budget.source.clone(),
            chat_timeout_secs: chat_timeout_source.clone(),
            prompt_layout: prompt_layout.source.clone(),
            plan_preset: plan_preset.source.clone(),
            profile: profile.source.clone(),
            narration: narration.source.clone(),
            footer: footer.source.clone(),
            stream: stream.source.clone(),
        };
        Ok(Self {
            workspace_root,
            state_dir,
            eval_events_path,
            completion_contract_path: cli.completion_contract_json,
            yes: cli.yes,
            offline: cli.offline,
            context_budget: context_budget.value,
            model: model.value,
            provider: provider.value,
            openai_api: openai_api.value,
            tool_protocol: tool_protocol.map(|value| value.value),
            prompt_layout: prompt_layout.value,
            plan_preset: plan_preset.value,
            intent_override: cli.intent.map(IntentId::from),
            planner_model: planner_model.value,
            planner_provider: planner_provider.value,
            planner_think: Some(planner_think.value),
            classifier_model: classifier_model.value,
            classifier_provider: classifier_provider.value,
            ollama_host: cli.ollama_host,
            ollama_think,
            lm_studio_host: normalize_lm_studio_host(&cli.lm_studio_host)?,
            num_predict: cli.num_predict,
            max_iterations: cli.max_iterations,
            chat_timeout_secs,
            chat_timeout_source,
            field_sources,
            chat_retries: cli.chat_retries,
            stream: stream.value,
            resume: cli.resume,
            fresh_session: cli.fresh_session,
            no_footer: footer.value.is_off(),
            narration: narration.value,
            profile: profile.value,
            profile_explicit,
            profile_inference,
            style: cli.style,
            action,
        })
    }
}

fn validate_openai_model(provider: Provider, model: &str, role: &str) -> anyhow::Result<()> {
    if provider == Provider::Openai {
        crate::openai_model::validate_strict_id(model, role)?;
    }
    Ok(())
}

pub(crate) fn normalize_lm_studio_host(value: &str) -> anyhow::Result<String> {
    let trimmed = value.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        bail!("--lm-studio-host must not be empty");
    }
    let normalized = trimmed.strip_suffix("/v1").unwrap_or(trimmed);
    let parsed = reqwest::Url::parse(normalized)
        .with_context(|| format!("invalid --lm-studio-host URL `{value}`"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        bail!("--lm-studio-host must use http or https");
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        bail!(
            "--lm-studio-host must not contain credentials; use LM_STUDIO_API_TOKEN for server authentication"
        );
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        bail!("--lm-studio-host must not contain a query or fragment");
    }
    Ok(normalized.to_string())
}

fn config_source_origin(source: &str) -> &'static str {
    if source == "flag" {
        "cli"
    } else if source.starts_with("preset:") || source.starts_with("config:") {
        "config"
    } else {
        "default"
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlannerModelTier {
    Qwen27,
    Gemma,
    Other,
}

impl PlannerModelTier {
    fn default_plan_preset(self) -> PlanPreset {
        PlanPreset::None
    }

    fn default_source(self) -> &'static str {
        match self {
            Self::Qwen27 => "default:qwen27_planner",
            Self::Gemma => "default:gemma_planner",
            Self::Other => "default",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct PlannerModelTierRule {
    family_pattern: &'static str,
    variant_patterns: &'static [&'static str],
    tier: PlannerModelTier,
}

const PLANNER_MODEL_TIER_RULES: &[PlannerModelTierRule] = &[
    PlannerModelTierRule {
        family_pattern: "qwen",
        variant_patterns: &["27b", "qwen27"],
        tier: PlannerModelTier::Qwen27,
    },
    PlannerModelTierRule {
        family_pattern: "gemma",
        variant_patterns: &[],
        tier: PlannerModelTier::Gemma,
    },
];

fn default_plan_preset_for_planner(planner_model: &str) -> Sourced<PlanPreset> {
    let tier = planner_model_tier(planner_model);
    sourced(tier.default_plan_preset(), tier.default_source())
}

fn planner_model_tier(model: &str) -> PlannerModelTier {
    let model = model.to_ascii_lowercase();
    PLANNER_MODEL_TIER_RULES
        .iter()
        .find(|rule| {
            model.contains(rule.family_pattern)
                && (rule.variant_patterns.is_empty()
                    || rule
                        .variant_patterns
                        .iter()
                        .any(|pattern| model.contains(pattern)))
        })
        .map(|rule| rule.tier)
        .unwrap_or(PlannerModelTier::Other)
}

fn load_named_preset(root: &Path, name: Option<&str>) -> anyhow::Result<Option<PresetConfig>> {
    let Some(name) = name.map(str::trim).filter(|name| !name.is_empty()) else {
        return Ok(None);
    };
    let mut found = false;
    let mut merged = PresetConfig::default();
    for path in config_paths(root) {
        let Some(file) = parse_config_file_if_present(&path)? else {
            continue;
        };
        let Some(preset) = file.presets.get(name) else {
            continue;
        };
        found = true;
        merge_preset(&mut merged, preset);
        if preset_complete(&merged) {
            break;
        }
    }
    if !found {
        let paths = config_paths(root)
            .into_iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        bail!("preset '{name}' not found in {paths} (key: preset.{name})");
    }
    Ok(Some(merged))
}

fn preset_complete(preset: &PresetConfig) -> bool {
    preset_missing_keys(preset).is_empty()
}

fn preset_missing_keys(preset: &PresetConfig) -> Vec<&'static str> {
    [
        ("model", preset.model.is_some()),
        ("provider", preset.provider.is_some()),
        ("planner_model", preset.planner_model.is_some()),
        ("planner_provider", preset.planner_provider.is_some()),
        ("context_budget", preset.context_budget.is_some()),
        ("chat_timeout_secs", preset.chat_timeout_secs.is_some()),
        ("plan_preset", preset.plan_preset.is_some()),
        ("profile", preset.profile.is_some()),
        ("narration", preset.narration.is_some()),
        ("footer", preset.footer.is_some()),
        ("stream", preset.stream.is_some()),
    ]
    .into_iter()
    .filter_map(|(key, present)| (!present).then_some(key))
    .collect()
}

fn merge_preset(target: &mut PresetConfig, source: &PresetConfig) {
    merge_preset_field(&mut target.pack, &source.pack);
    merge_preset_field(&mut target.model, &source.model);
    merge_preset_field(&mut target.provider, &source.provider);
    merge_preset_field(&mut target.openai_api, &source.openai_api);
    merge_preset_field(&mut target.tool_protocol, &source.tool_protocol);
    merge_preset_field(&mut target.planner_model, &source.planner_model);
    merge_preset_field(&mut target.planner_provider, &source.planner_provider);
    merge_preset_field(&mut target.planner_think, &source.planner_think);
    merge_preset_field(&mut target.classifier_model, &source.classifier_model);
    merge_preset_field(&mut target.classifier_provider, &source.classifier_provider);
    merge_preset_field(&mut target.context_budget, &source.context_budget);
    merge_preset_field(&mut target.chat_timeout_secs, &source.chat_timeout_secs);
    merge_preset_field(&mut target.prompt_layout, &source.prompt_layout);
    merge_preset_field(&mut target.plan_preset, &source.plan_preset);
    merge_preset_field(&mut target.profile, &source.profile);
    merge_preset_field(&mut target.narration, &source.narration);
    merge_preset_field(&mut target.footer, &source.footer);
    merge_preset_field(&mut target.stream, &source.stream);
}

fn merge_preset_field<T: Clone>(target: &mut Option<Sourced<T>>, source: &Option<Sourced<T>>) {
    if target.is_none() {
        *target = source.clone();
    }
}

fn config_file_narration(root: &Path) -> Option<Sourced<NarrationMode>> {
    for path in config_paths(root) {
        if let Ok(Some(file)) = parse_config_file_if_present(&path)
            && let Some(narration) = file.narration
        {
            return Some(narration);
        }
    }
    legacy_config_file_narration(root)
}

fn config_file_footer(root: &Path) -> Option<Sourced<FooterMode>> {
    for path in config_paths(root) {
        if let Ok(Some(file)) = parse_config_file_if_present(&path)
            && let Some(footer) = file.footer
        {
            return Some(footer);
        }
    }
    legacy_config_file_footer(root)
}

fn config_file_stream(root: &Path) -> Option<Sourced<bool>> {
    for path in config_paths(root) {
        if let Ok(Some(file)) = parse_config_file_if_present(&path)
            && let Some(stream) = file.stream
        {
            return Some(stream);
        }
    }
    legacy_config_file_stream(root)
}

fn config_file_prompt_layout(root: &Path) -> Option<Sourced<PromptLayout>> {
    for path in config_paths(root) {
        if let Ok(Some(file)) = parse_config_file_if_present(&path)
            && let Some(prompt_layout) = file.prompt_layout
        {
            return Some(prompt_layout);
        }
    }
    legacy_config_file_prompt_layout(root)
}

fn config_file_plan_preset(root: &Path) -> Option<Sourced<PlanPreset>> {
    for path in config_paths(root) {
        if let Ok(Some(file)) = parse_config_file_if_present(&path)
            && let Some(plan_preset) = file.plan_preset
        {
            return Some(plan_preset);
        }
    }
    legacy_config_file_plan_preset(root)
}

fn legacy_config_file_narration(root: &Path) -> Option<Sourced<NarrationMode>> {
    legacy_config_file_value(root, "narration", NarrationMode::from_config_value)
}

fn legacy_config_file_footer(root: &Path) -> Option<Sourced<FooterMode>> {
    legacy_config_file_value(root, "footer", FooterMode::from_config_value)
}

fn legacy_config_file_stream(root: &Path) -> Option<Sourced<bool>> {
    legacy_config_file_value(root, "stream", parse_stream_mode)
}

fn legacy_config_file_prompt_layout(root: &Path) -> Option<Sourced<PromptLayout>> {
    legacy_config_file_value(root, "prompt_layout", PromptLayout::from_config_value)
}

fn legacy_config_file_plan_preset(root: &Path) -> Option<Sourced<PlanPreset>> {
    legacy_config_file_value(root, "plan_preset", PlanPreset::from_config_value)
}

fn legacy_config_file_value<T>(
    root: &Path,
    key: &str,
    parse: impl Fn(&str) -> Option<T>,
) -> Option<Sourced<T>> {
    for path in [
        root.join(".commandagent").join("config"),
        root.join(".anvil").join("config"),
    ] {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        if let Some(value) = text.lines().find_map(|line| {
            let line = line.split('#').next().unwrap_or("").trim();
            let value = line.strip_prefix(key)?.trim();
            let value = value.strip_prefix('=')?.trim();
            let value = value.trim_matches('"').trim_matches('\'');
            parse(value).map(|mode| {
                sourced(
                    mode,
                    format!(
                        "config:{}",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    ),
                )
            })
        }) {
            return Some(value);
        }
    }
    None
}

fn config_paths(root: &Path) -> Vec<PathBuf> {
    let mut paths = vec![
        root.join(".commandagent").join("config.toml"),
        root.join(".anvil").join("config.toml"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        paths.push(home.join(".commandagent").join("config.toml"));
        paths.push(home.join(".anvil").join("config.toml"));
    }
    paths
}

pub(crate) fn inspect_config_files(root: &Path, preset_name: Option<&str>) -> ConfigInspection {
    let preset_name = preset_name.map(str::trim).filter(|name| !name.is_empty());
    let mut merged = PresetConfig::default();
    let mut preset_found = false;
    let mut inspection_errors = Vec::new();
    let paths = config_paths(root)
        .into_iter()
        .map(|path| match parse_config_file_if_present(&path) {
            Ok(Some(file)) => {
                if let Some(name) = preset_name
                    && let Some(preset) = file.presets.get(name)
                {
                    preset_found = true;
                    merge_preset(&mut merged, preset);
                }
                ConfigPathInspection {
                    path,
                    exists: true,
                    parse_error: None,
                }
            }
            Ok(None) => ConfigPathInspection {
                path,
                exists: false,
                parse_error: None,
            },
            Err(error) => {
                let error = format!("{error:#}");
                inspection_errors.push(error.clone());
                ConfigPathInspection {
                    path,
                    exists: true,
                    parse_error: Some(error),
                }
            }
        })
        .collect();
    let preset = preset_name.map(|name| {
        let missing_keys = preset_missing_keys(&merged);
        PresetInspection {
            name: name.to_string(),
            found: preset_found,
            complete: preset_found && missing_keys.is_empty(),
            missing_keys,
        }
    });
    ConfigInspection {
        paths,
        preset,
        inspection_errors,
    }
}

fn parse_config_file_if_present(path: &Path) -> anyhow::Result<Option<ConfigFile>> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse_config_file(path, &text).map(Some),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| format!("failed to read {}", path.display())),
    }
}

fn parse_config_file(path: &Path, text: &str) -> anyhow::Result<ConfigFile> {
    text.parse::<toml::Table>()
        .with_context(|| format!("{} contains invalid TOML syntax", path.display()))?;
    let mut file = ConfigFile::default();
    let mut section = ConfigSection::Top;
    for (index, raw_line) in text.lines().enumerate() {
        let line_no = index + 1;
        let line = strip_toml_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        if let Some(section_name) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
        {
            section = if let Some(name) = section_name.trim().strip_prefix("preset.") {
                let name = name.trim();
                if name.is_empty() {
                    bail!(
                        "{}:{} invalid empty preset section name",
                        path.display(),
                        line_no
                    );
                }
                ConfigSection::Preset(name.to_string())
            } else {
                ConfigSection::Other
            };
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            bail!("{}:{} invalid config line", path.display(), line_no);
        };
        let key = key.trim();
        let value = value.trim();
        match &section {
            ConfigSection::Top => parse_top_level_key(path, line_no, key, value, &mut file)?,
            ConfigSection::Preset(name) => {
                let source = format!("preset:{name}");
                let preset = file.presets.entry(name.clone()).or_default();
                parse_preset_key(path, line_no, name, key, value, &source, preset)?;
            }
            ConfigSection::Other => {}
        }
    }
    Ok(file)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ConfigSection {
    Top,
    Preset(String),
    Other,
}

fn parse_top_level_key(
    path: &Path,
    line_no: usize,
    key: &str,
    value: &str,
    file: &mut ConfigFile,
) -> anyhow::Result<()> {
    if !SUPPORTED_TOP_LEVEL_KEYS.contains(&key) {
        bail!("{}:{} unknown config key '{key}'", path.display(), line_no);
    }
    match key {
        "extension_root" => {
            file.extension_root = Some(sourced(
                parse_string_value(path, line_no, key, value)?,
                format!(
                    "config:{}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ),
            ));
            Ok(())
        }
        "narration" => {
            file.narration = Some(sourced(
                parse_narration_value(path, line_no, key, value)?,
                format!(
                    "config:{}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ),
            ));
            Ok(())
        }
        "footer" => {
            file.footer = Some(sourced(
                parse_footer_value(path, line_no, key, value)?,
                format!(
                    "config:{}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ),
            ));
            Ok(())
        }
        "stream" => {
            file.stream = Some(sourced(
                parse_stream_value(path, line_no, key, value)?,
                format!(
                    "config:{}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ),
            ));
            Ok(())
        }
        "prompt_layout" => {
            file.prompt_layout = Some(sourced(
                parse_prompt_layout_value(path, line_no, key, value)?,
                format!(
                    "config:{}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ),
            ));
            Ok(())
        }
        "plan_preset" => {
            file.plan_preset = Some(sourced(
                parse_plan_preset_value(path, line_no, key, value)?,
                format!(
                    "config:{}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ),
            ));
            Ok(())
        }
        _ => unreachable!("SUPPORTED_TOP_LEVEL_KEYS contains unhandled key '{key}'"),
    }
}

fn parse_preset_key(
    path: &Path,
    line_no: usize,
    name: &str,
    key: &str,
    value: &str,
    source: &str,
    preset: &mut PresetConfig,
) -> anyhow::Result<()> {
    let full_key = format!("preset.{name}.{key}");
    if !SUPPORTED_PRESET_KEYS.contains(&key) {
        bail!(
            "{}:{} unknown config key '{full_key}'",
            path.display(),
            line_no
        );
    }
    match key {
        "pack" => {
            preset.pack = Some(sourced(
                parse_string_value(path, line_no, &full_key, value)?,
                source,
            ))
        }
        "model" => {
            preset.model = Some(sourced(
                parse_string_value(path, line_no, &full_key, value)?,
                source,
            ))
        }
        "provider" => {
            preset.provider = Some(sourced(
                parse_provider_value(path, line_no, &full_key, value)?,
                source,
            ))
        }
        "api" => {
            preset.openai_api = Some(sourced(
                parse_openai_api_value(path, line_no, &full_key, value)?,
                source,
            ))
        }
        "tool_protocol" => {
            preset.tool_protocol = Some(sourced(
                parse_tool_protocol_value(path, line_no, &full_key, value)?,
                source,
            ))
        }
        "planner_model" => {
            preset.planner_model = Some(sourced(
                parse_string_value(path, line_no, &full_key, value)?,
                source,
            ))
        }
        "planner_provider" => {
            preset.planner_provider = Some(sourced(
                parse_provider_value(path, line_no, &full_key, value)?,
                source,
            ))
        }
        "planner_think" => {
            preset.planner_think = Some(sourced(
                parse_ollama_think_value(path, line_no, &full_key, value)?,
                source,
            ))
        }
        "classifier_model" => {
            preset.classifier_model = Some(sourced(
                parse_string_value(path, line_no, &full_key, value)?,
                source,
            ))
        }
        "classifier_provider" => {
            preset.classifier_provider = Some(sourced(
                parse_provider_value(path, line_no, &full_key, value)?,
                source,
            ))
        }
        "context_budget" => {
            preset.context_budget = Some(sourced(
                parse_usize_value(path, line_no, &full_key, value)?,
                source,
            ))
        }
        "chat_timeout_secs" => {
            preset.chat_timeout_secs = Some(sourced(
                parse_u64_value(path, line_no, &full_key, value)?,
                source,
            ))
        }
        "profile" => {
            preset.profile = Some(sourced(
                parse_string_value(path, line_no, &full_key, value)?,
                source,
            ))
        }
        "narration" => {
            preset.narration = Some(sourced(
                parse_narration_value(path, line_no, &full_key, value)?,
                source,
            ))
        }
        "footer" => {
            preset.footer = Some(sourced(
                parse_footer_value(path, line_no, &full_key, value)?,
                source,
            ))
        }
        "stream" => {
            preset.stream = Some(sourced(
                parse_stream_value(path, line_no, &full_key, value)?,
                source,
            ))
        }
        "prompt_layout" => {
            preset.prompt_layout = Some(sourced(
                parse_prompt_layout_value(path, line_no, &full_key, value)?,
                source,
            ))
        }
        "plan_preset" => {
            preset.plan_preset = Some(sourced(
                parse_plan_preset_value(path, line_no, &full_key, value)?,
                source,
            ))
        }
        _ => unreachable!("SUPPORTED_PRESET_KEYS contains unhandled key '{key}'"),
    }
    Ok(())
}

pub(crate) fn selected_preset_pack(
    root: &Path,
    name: Option<&str>,
) -> anyhow::Result<Option<String>> {
    Ok(load_named_preset(root, name)?.and_then(|preset| preset.pack.map(|value| value.value)))
}

pub(crate) fn configured_extension_root(root: &Path) -> anyhow::Result<Option<PathBuf>> {
    for path in config_paths(root) {
        if let Some(file) = parse_config_file_if_present(&path)?
            && let Some(value) = file.extension_root
        {
            let configured = PathBuf::from(value.value);
            return Ok(Some(if configured.is_absolute() {
                configured
            } else {
                root.join(configured)
            }));
        }
    }
    Ok(None)
}

fn strip_toml_comment(line: &str) -> &str {
    let mut quoted = false;
    let mut escaped = false;
    for (index, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if quoted => escaped = true,
            '"' => quoted = !quoted,
            '#' if !quoted => return &line[..index],
            _ => {}
        }
    }
    line
}

fn parse_string_value(
    path: &Path,
    line_no: usize,
    key: &str,
    value: &str,
) -> anyhow::Result<String> {
    let Some(value) = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    else {
        bail!(
            "{}:{} {key} expects a quoted string",
            path.display(),
            line_no
        );
    };
    Ok(value.replace("\\\"", "\"").replace("\\\\", "\\"))
}

fn parse_provider_value(
    path: &Path,
    line_no: usize,
    key: &str,
    value: &str,
) -> anyhow::Result<Provider> {
    match parse_string_value(path, line_no, key, value)?.as_str() {
        "ollama" => Ok(Provider::Ollama),
        "lm-studio" => Ok(Provider::LmStudio),
        "openai" => Ok(Provider::Openai),
        "gemini" => Ok(Provider::Gemini),
        _ => bail!(
            "{}:{} {key} expects provider ollama|lm-studio|openai|gemini",
            path.display(),
            line_no
        ),
    }
}

fn parse_ollama_think_value(
    path: &Path,
    line_no: usize,
    key: &str,
    value: &str,
) -> anyhow::Result<OllamaThink> {
    match parse_string_value(path, line_no, key, value)?.as_str() {
        "true" => Ok(OllamaThink::True),
        "false" => Ok(OllamaThink::False),
        "low" => Ok(OllamaThink::Low),
        "medium" => Ok(OllamaThink::Medium),
        "high" => Ok(OllamaThink::High),
        _ => bail!(
            "{}:{} {key} expects think true|false|low|medium|high",
            path.display(),
            line_no
        ),
    }
}

fn parse_narration_value(
    path: &Path,
    line_no: usize,
    key: &str,
    value: &str,
) -> anyhow::Result<NarrationMode> {
    let value = parse_string_value(path, line_no, key, value)?;
    NarrationMode::from_config_value(&value).ok_or_else(|| {
        anyhow::anyhow!(
            "{}:{} {key} expects narration normal|quiet",
            path.display(),
            line_no
        )
    })
}

fn parse_footer_value(
    path: &Path,
    line_no: usize,
    key: &str,
    value: &str,
) -> anyhow::Result<FooterMode> {
    let value = parse_string_value(path, line_no, key, value)?;
    FooterMode::from_config_value(&value).ok_or_else(|| {
        anyhow::anyhow!("{}:{} {key} expects footer on|off", path.display(), line_no)
    })
}

fn parse_stream_value(path: &Path, line_no: usize, key: &str, value: &str) -> anyhow::Result<bool> {
    let value = parse_string_value(path, line_no, key, value)?;
    parse_stream_mode(&value).ok_or_else(|| {
        anyhow::anyhow!("{}:{} {key} expects stream on|off", path.display(), line_no)
    })
}

fn parse_prompt_layout_value(
    path: &Path,
    line_no: usize,
    key: &str,
    value: &str,
) -> anyhow::Result<PromptLayout> {
    let value = parse_string_value(path, line_no, key, value)?;
    PromptLayout::from_config_value(&value).ok_or_else(|| {
        anyhow::anyhow!(
            "{}:{} {key} expects prompt_layout stable|legacy",
            path.display(),
            line_no
        )
    })
}

fn parse_tool_protocol_value(
    path: &Path,
    line_no: usize,
    key: &str,
    value: &str,
) -> anyhow::Result<ToolProtocol> {
    let value = parse_string_value(path, line_no, key, value)?;
    ToolProtocol::from_config_value(&value).ok_or_else(|| {
        anyhow::anyhow!(
            "{}:{} {key} expects tool_protocol native|text",
            path.display(),
            line_no
        )
    })
}

fn parse_openai_api_value(
    path: &Path,
    line_no: usize,
    key: &str,
    value: &str,
) -> anyhow::Result<OpenAiApi> {
    let value = parse_string_value(path, line_no, key, value)?;
    OpenAiApi::from_config_value(&value).ok_or_else(|| {
        anyhow::anyhow!(
            "{}:{} {key} expects api chat_completions|responses",
            path.display(),
            line_no
        )
    })
}

fn parse_plan_preset_value(
    path: &Path,
    line_no: usize,
    key: &str,
    value: &str,
) -> anyhow::Result<PlanPreset> {
    let value = parse_string_value(path, line_no, key, value)?;
    PlanPreset::from_config_value(&value).ok_or_else(|| {
        anyhow::anyhow!(
            "{}:{} {key} expects plan_preset none|profile",
            path.display(),
            line_no
        )
    })
}

fn parse_usize_value(path: &Path, line_no: usize, key: &str, value: &str) -> anyhow::Result<usize> {
    value
        .parse::<usize>()
        .with_context(|| format!("{}:{} {key} expects an integer", path.display(), line_no))
}

fn parse_u64_value(path: &Path, line_no: usize, key: &str, value: &str) -> anyhow::Result<u64> {
    value
        .parse::<u64>()
        .with_context(|| format!("{}:{} {key} expects an integer", path.display(), line_no))
}

fn resolve_chat_timeout(
    override_secs: Option<u64>,
    provider: Provider,
    planner_provider: Provider,
    classifier_provider: Provider,
) -> (u64, String) {
    if let Some(secs) = override_secs {
        return (secs, "override:cli".to_string());
    }
    if provider.is_local() || planner_provider.is_local() || classifier_provider.is_local() {
        (
            LOCAL_PROVIDER_CHAT_TIMEOUT_SECS,
            "default:local_provider".to_string(),
        )
    } else {
        (
            REMOTE_PROVIDER_CHAT_TIMEOUT_SECS,
            "default:remote_provider".to_string(),
        )
    }
}

pub fn action_goal(action: &Action) -> Option<&str> {
    match action {
        Action::Prompt(goal)
        | Action::PlanSteps(goal)
        | Action::PlanRun(goal)
        | Action::UltraPlan(goal)
        | Action::UltraPlanRun(goal) => Some(goal.as_str()),
        Action::Repl
        | Action::RunPlan(_)
        | Action::RunUltraPlan(_)
        | Action::SetupInteractionProbe
        | Action::Runs
        | Action::UxDemo
        | Action::ModelProbe
        | Action::Doctor
        | Action::Workflow(..) => None,
    }
}

fn action_from_cli(cli: &Cli) -> anyhow::Result<Action> {
    let mut count = 0usize;
    count += cli.prompt.is_some() as usize;
    count += cli.plan_steps as usize;
    count += cli.plan_run as usize;
    count += cli.run_plan.is_some() as usize;
    count += cli.ultra_plan as usize;
    count += cli.ultra_plan_run as usize;
    count += cli.run_ultra_plan.is_some() as usize;
    count += cli.setup_interaction_probe as usize;
    count += cli.runs as usize;
    count += cli.ux_demo as usize;
    count += cli.model_probe as usize;
    count += cli.doctor as usize;
    count += cli.workflow.is_some() as usize;
    if count > 1 {
        bail!("only one action selector can be used at a time");
    }
    if let Some(definition) = cli.workflow.clone() {
        let origin = cli
            .origin
            .clone()
            .ok_or_else(|| anyhow::anyhow!("--workflow requires --origin"))?;
        if cli.intent.is_some() {
            bail!("--workflow cannot be combined with --intent");
        }
        return Ok(Action::Workflow(definition, origin));
    }
    if let Some(prompt) = cli.prompt.clone() {
        return Ok(Action::Prompt(prompt));
    }
    if let Some(path) = cli.run_plan.clone() {
        return Ok(Action::RunPlan(path));
    }
    if let Some(path) = cli.run_ultra_plan.clone() {
        return Ok(Action::RunUltraPlan(path));
    }
    let goal = cli.trailing_goal();
    if cli.plan_steps {
        return Ok(Action::PlanSteps(required_goal(goal, "--plan-steps")?));
    }
    if cli.plan_run {
        return Ok(Action::PlanRun(required_goal(goal, "--plan-run")?));
    }
    if cli.ultra_plan {
        return Ok(Action::UltraPlan(required_goal(goal, "--ultra-plan")?));
    }
    if cli.ultra_plan_run {
        return Ok(Action::UltraPlanRun(required_goal(
            goal,
            "--ultra-plan-run",
        )?));
    }
    if cli.setup_interaction_probe {
        return Ok(Action::SetupInteractionProbe);
    }
    if cli.runs {
        return Ok(Action::Runs);
    }
    if cli.ux_demo {
        return Ok(Action::UxDemo);
    }
    if cli.model_probe {
        return Ok(Action::ModelProbe);
    }
    if cli.doctor {
        return Ok(Action::Doctor);
    }
    Ok(Action::Repl)
}

fn required_goal(goal: Option<String>, action: &str) -> anyhow::Result<String> {
    goal.filter(|goal| !goal.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("{action} requires a trailing goal"))
}

pub fn default_state_dir() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_STATE_HOME") {
        return PathBuf::from(xdg).join("anvilminimal");
    }
    let home = std::env::var_os("HOME").unwrap_or_else(|| ".".into());
    PathBuf::from(home)
        .join(".local")
        .join("state")
        .join("anvilminimal")
}

pub fn load_api_key(workspace_root: &std::path::Path, name: &str) -> anyhow::Result<String> {
    if let Ok(value) = std::env::var(name)
        && !value.trim().is_empty()
    {
        return Ok(value);
    }
    let env = read_dotenv(workspace_root);
    env.get(name)
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "{name} is not set. Set {name} in the environment or workspace .env, then run `commandagent --doctor`."
            )
        })
}

pub fn load_process_api_key(name: &str) -> anyhow::Result<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "{name} is not set. Set {name} in the process environment, then run `commandagent --doctor`."
            )
        })
}

pub fn read_dotenv(workspace_root: &std::path::Path) -> HashMap<String, String> {
    let path = workspace_root.join(".env");
    let Ok(content) = std::fs::read_to_string(path) else {
        return HashMap::new();
    };
    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (key, value) = line.split_once('=')?;
            let value = value
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
            Some((key.trim().to_string(), value))
        })
        .collect()
}

pub fn redact(value: &str) -> String {
    if value.is_empty() {
        "<empty>".to_string()
    } else {
        "<redacted>".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn missing_api_key_error_includes_setup_and_doctor_remediation() {
        let dir = tempfile::tempdir().unwrap();
        let key = "COMMANDAGENT_TEST_MISSING_PROVIDER_KEY_ISSUE_46";

        let error = load_api_key(dir.path(), key).unwrap_err().to_string();

        assert_eq!(
            error,
            format!(
                "{key} is not set. Set {key} in the environment or workspace .env, then run `commandagent --doctor`."
            )
        );
    }

    #[test]
    fn process_only_api_key_error_excludes_dotenv_remediation() {
        let key = "COMMANDAGENT_TEST_MISSING_PROCESS_PROVIDER_KEY_F0";

        let error = load_process_api_key(key).unwrap_err().to_string();

        assert_eq!(
            error,
            format!(
                "{key} is not set. Set {key} in the process environment, then run `commandagent --doctor`."
            )
        );
        assert!(!error.contains(".env"));
    }

    #[test]
    fn explicit_create_overrides_fix_wording_while_omission_keeps_detection() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_string_lossy().to_string();
        let explicit = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--intent",
            "create",
            "--ultra-plan-run",
            "parserを修正して",
        ]))
        .unwrap();
        let omitted = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--ultra-plan-run",
            "parserを修正して",
        ]))
        .unwrap();

        assert_eq!(explicit.resolved_intent("parserを修正して"), "create");
        assert_eq!(omitted.resolved_intent("parserを修正して"), "fix");
    }

    #[test]
    fn cross_provider_planner_model_error() {
        let cli = Cli::parse_from([
            "commandagent",
            "--provider",
            "ollama",
            "--planner-provider",
            "gemini",
        ]);
        let err = Config::from_cli(cli).unwrap_err().to_string();
        assert!(err.contains("--planner-model is required"));
    }

    #[test]
    fn same_provider_defaults_planner_model() {
        let cli = Cli::parse_from(["commandagent", "--provider", "ollama", "--model", "m"]);
        let config = Config::from_cli(cli).unwrap();
        assert_eq!(config.planner_model, "m");
        assert_eq!(config.planner_think, Some(OllamaThink::False));
        assert_eq!(config.classifier_model, "m");
        assert_eq!(config.classifier_provider, Provider::Ollama);
        assert_eq!(config.field_sources.planner_think, "default");
        assert_eq!(config.field_sources.classifier_model, "default:planner");
        assert_eq!(config.field_sources.classifier_provider, "default:planner");
    }

    #[test]
    fn think_applies_when_either_resolved_role_uses_ollama() {
        let executor = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--provider",
            "ollama",
            "--think=high",
        ]))
        .unwrap();
        assert_eq!(executor.ollama_think, Some(OllamaThink::High));
        assert_eq!(executor.planner_think, Some(OllamaThink::High));

        let planner = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--provider",
            "openai",
            "--model",
            "gpt-5.6-sol",
            "--planner-provider",
            "ollama",
            "--planner-model",
            "qwen3",
            "--think=false",
        ]))
        .unwrap();
        assert_eq!(planner.ollama_think, Some(OllamaThink::False));
        assert_eq!(planner.planner_think, Some(OllamaThink::False));
    }

    #[test]
    fn preset_resolves_planner_think_and_independent_classifier_role() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_string_lossy().to_string();
        std::fs::create_dir_all(dir.path().join(".commandagent")).unwrap();
        std::fs::write(
            dir.path().join(".commandagent/config.toml"),
            r#"
[preset.fast]
model = "executor"
provider = "ollama"
planner_model = "planner"
planner_provider = "ollama"
planner_think = "low"
classifier_model = "gpt-5.6-luna"
classifier_provider = "openai"
"#,
        )
        .unwrap();

        let config = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--preset",
            "fast",
        ]))
        .unwrap();

        assert_eq!(config.ollama_think, None);
        assert_eq!(config.planner_think, Some(OllamaThink::Low));
        assert_eq!(config.classifier_model, "gpt-5.6-luna");
        assert_eq!(config.classifier_provider, Provider::Openai);
        assert_eq!(config.field_sources.planner_think, "preset:fast");
        assert_eq!(config.field_sources.classifier_model, "preset:fast");
        assert_eq!(config.field_sources.classifier_provider, "preset:fast");
    }

    #[test]
    fn distinct_classifier_provider_requires_an_explicit_model() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_string_lossy().to_string();
        std::fs::create_dir_all(dir.path().join(".commandagent")).unwrap();
        std::fs::write(
            dir.path().join(".commandagent/config.toml"),
            r#"
[preset.invalid]
model = "executor"
provider = "ollama"
planner_model = "planner"
planner_provider = "ollama"
classifier_provider = "gemini"
"#,
        )
        .unwrap();

        let error = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--preset",
            "invalid",
        ]))
        .unwrap_err()
        .to_string();

        assert_eq!(
            error,
            "classifier_model is required when classifier_provider differs from planner_provider"
        );
    }

    #[test]
    fn think_rejects_configuration_without_an_ollama_role() {
        let error = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--provider",
            "gemini",
            "--planner-provider",
            "gemini",
            "--think",
        ]))
        .unwrap_err()
        .to_string();

        assert_eq!(
            error,
            "--think requires provider or planner_provider to be ollama"
        );
    }

    #[test]
    fn openai_executor_rejects_ambiguous_gpt_5_6_alias() {
        let cli = Cli::parse_from(["commandagent", "--provider", "openai", "--model", "gpt-5.6"]);

        let error = Config::from_cli(cli).unwrap_err().to_string();

        assert!(error.contains("gpt-5.6-luna"), "{error}");
        assert!(error.contains("Luna, Terra, or Sol"), "{error}");
    }

    #[test]
    fn openai_executor_accepts_exact_family_and_snapshot_ids() {
        for model in [
            "gpt-5.6-luna",
            "gpt-5.6-luna-2026-07-31",
            "gpt-5.6-terra",
            "gpt-5.6-terra-2026-08-18",
            "gpt-5.6-sol",
        ] {
            let cli = Cli::parse_from(["commandagent", "--provider", "openai", "--model", model]);

            assert_eq!(Config::from_cli(cli).unwrap().model, model);
        }
    }

    #[test]
    fn openai_planner_rejects_ambiguous_alias_and_accepts_terra() {
        let rejected = Cli::parse_from([
            "commandagent",
            "--provider",
            "ollama",
            "--model",
            "executor",
            "--planner-provider",
            "openai",
            "--planner-model",
            "gpt-5.6",
        ]);
        let error = Config::from_cli(rejected).unwrap_err().to_string();
        assert!(error.contains("planner model alias"), "{error}");

        let accepted = Cli::parse_from([
            "commandagent",
            "--provider",
            "ollama",
            "--model",
            "executor",
            "--planner-provider",
            "openai",
            "--planner-model",
            "gpt-5.6-terra",
        ]);
        assert_eq!(
            Config::from_cli(accepted).unwrap().planner_model,
            "gpt-5.6-terra"
        );
    }

    #[test]
    fn openai_api_is_explicit_and_defaults_to_chat_completions() {
        let omitted = Config::from_cli(Cli::parse_from(["commandagent"])).unwrap();
        let explicit =
            Config::from_cli(Cli::parse_from(["commandagent", "--api", "responses"])).unwrap();

        assert_eq!(omitted.openai_api, OpenAiApi::ChatCompletions);
        assert_eq!(explicit.openai_api, OpenAiApi::Responses);

        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_string_lossy().to_string();
        std::fs::create_dir_all(dir.path().join(".commandagent")).unwrap();
        std::fs::write(
            dir.path().join(".commandagent/config.toml"),
            "[preset.responses]\napi = \"responses\"\n",
        )
        .unwrap();
        let preset = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--preset",
            "responses",
        ]))
        .unwrap();
        assert_eq!(preset.openai_api, OpenAiApi::Responses);
    }

    #[test]
    fn preset_rejects_unknown_openai_api() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_string_lossy().to_string();
        std::fs::create_dir_all(dir.path().join(".commandagent")).unwrap();
        std::fs::write(
            dir.path().join(".commandagent/config.toml"),
            "[preset.invalid]\napi = \"automatic\"\n",
        )
        .unwrap();

        let error = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--preset",
            "invalid",
        ]))
        .unwrap_err()
        .to_string();
        assert!(error.contains("api chat_completions|responses"), "{error}");
    }

    #[test]
    fn tool_protocol_is_declaration_only_and_accepts_flag_or_preset() {
        let omitted = Config::from_cli(Cli::parse_from(["commandagent"])).unwrap();
        let explicit =
            Config::from_cli(Cli::parse_from(["commandagent", "--tool-protocol", "text"])).unwrap();

        assert_eq!(omitted.tool_protocol, None);
        assert_eq!(explicit.tool_protocol, Some(ToolProtocol::Text));

        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_string_lossy().to_string();
        std::fs::create_dir_all(dir.path().join(".anvil")).unwrap();
        std::fs::write(
            dir.path().join(".anvil/config.toml"),
            r#"
[preset.native-tools]
tool_protocol = "native"
"#,
        )
        .unwrap();
        let from_preset = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--preset",
            "native-tools",
        ]))
        .unwrap();
        assert_eq!(from_preset.tool_protocol, Some(ToolProtocol::Native));
    }

    #[test]
    fn preset_rejects_unknown_tool_protocol() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_string_lossy().to_string();
        std::fs::create_dir_all(dir.path().join(".anvil")).unwrap();
        std::fs::write(
            dir.path().join(".anvil/config.toml"),
            "[preset.invalid]\ntool_protocol = \"automatic\"\n",
        )
        .unwrap();

        let error = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--preset",
            "invalid",
        ]))
        .unwrap_err()
        .to_string();

        assert!(error.contains("tool_protocol native|text"), "{error}");
    }

    #[test]
    fn runs_action_is_read_only_selector() {
        let dir = tempfile::tempdir().unwrap();
        let cli = Cli::parse_from([
            "commandagent",
            "--cwd",
            dir.path().to_str().unwrap(),
            "--runs",
        ]);
        let config = Config::from_cli(cli).unwrap();

        assert!(matches!(config.action, Action::Runs));
        assert!(action_goal(&config.action).is_none());
    }

    #[test]
    fn doctor_is_an_exclusive_action_selector() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            dir.path().to_str().unwrap(),
            "--doctor",
        ]))
        .unwrap();
        assert!(matches!(config.action, Action::Doctor));

        let error = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            dir.path().to_str().unwrap(),
            "--doctor",
            "--runs",
        ]))
        .unwrap_err()
        .to_string();
        assert!(error.contains("only one action selector"), "{error}");
    }

    #[test]
    fn config_inspection_reuses_parser_and_preset_completeness_keys() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".anvil")).unwrap();
        std::fs::write(
            dir.path().join(".anvil/config.toml"),
            "[preset.partial]\nprovider = \"gemini\"\n",
        )
        .unwrap();

        let inspection = inspect_config_files(dir.path(), Some("partial"));
        assert!(!inspection.paths[0].exists);
        assert!(inspection.paths[1].exists);
        assert!(inspection.paths[1].parse_error.is_none());
        let preset = inspection.preset.unwrap();
        assert!(preset.found);
        assert!(!preset.complete);
        assert!(preset.missing_keys.contains(&"model"));
        assert!(preset.missing_keys.contains(&"planner_model"));
        assert!(!preset.missing_keys.contains(&"prompt_layout"));
    }

    #[test]
    fn narration_quiet_is_read_from_cli_or_config_file() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_string_lossy().to_string();
        std::fs::create_dir_all(dir.path().join(".anvil")).unwrap();
        std::fs::write(dir.path().join(".anvil/config"), "narration = \"quiet\"\n").unwrap();
        let config = Config::from_cli(Cli::parse_from(["commandagent", "--cwd", &cwd])).unwrap();
        assert_eq!(config.narration, NarrationMode::Quiet);

        std::fs::write(dir.path().join(".anvil/config"), "narration = \"normal\"\n").unwrap();
        let config =
            Config::from_cli(Cli::parse_from(["commandagent", "--cwd", &cwd, "--quiet"])).unwrap();
        assert_eq!(config.narration, NarrationMode::Quiet);
    }

    #[test]
    fn config_path_precedence_covers_new_old_and_both() {
        let new_only = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(new_only.path().join(".commandagent")).unwrap();
        std::fs::write(
            new_only.path().join(".commandagent/config.toml"),
            "footer = \"off\"\n",
        )
        .unwrap();
        assert_eq!(
            config_file_footer(new_only.path()).map(|value| value.value),
            Some(FooterMode::Off)
        );

        let old_only = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(old_only.path().join(".anvil")).unwrap();
        std::fs::write(
            old_only.path().join(".anvil/config.toml"),
            "footer = \"off\"\n",
        )
        .unwrap();
        assert_eq!(
            config_file_footer(old_only.path()).map(|value| value.value),
            Some(FooterMode::Off)
        );

        let both = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(both.path().join(".commandagent")).unwrap();
        std::fs::create_dir_all(both.path().join(".anvil")).unwrap();
        std::fs::write(
            both.path().join(".commandagent/config.toml"),
            "footer = \"on\"\n",
        )
        .unwrap();
        std::fs::write(both.path().join(".anvil/config.toml"), "footer = \"off\"\n").unwrap();
        assert_eq!(
            config_file_footer(both.path()).map(|value| value.value),
            Some(FooterMode::On)
        );
    }

    #[test]
    fn extensionless_commandagent_config_precedes_legacy_config() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".commandagent")).unwrap();
        std::fs::create_dir_all(dir.path().join(".anvil")).unwrap();
        std::fs::write(
            dir.path().join(".commandagent/config"),
            "narration = \"normal\"\n",
        )
        .unwrap();
        std::fs::write(dir.path().join(".anvil/config"), "narration = \"quiet\"\n").unwrap();

        assert_eq!(
            legacy_config_file_narration(dir.path()).map(|value| value.value),
            Some(NarrationMode::Normal)
        );
    }

    #[test]
    fn footer_mode_is_read_from_cli_preset_or_config_file() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_string_lossy().to_string();
        std::fs::create_dir_all(dir.path().join(".anvil")).unwrap();
        std::fs::write(dir.path().join(".anvil/config.toml"), "footer = \"off\"\n").unwrap();

        let config = Config::from_cli(Cli::parse_from(["commandagent", "--cwd", &cwd])).unwrap();
        assert!(config.no_footer);
        assert_eq!(config.field_sources.footer, "config:config.toml");

        let config = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--footer",
            "on",
        ]))
        .unwrap();
        assert!(!config.no_footer);
        assert_eq!(config.field_sources.footer, "flag");

        std::fs::write(
            dir.path().join(".anvil/config.toml"),
            "[preset.local]\nfooter = \"off\"\n",
        )
        .unwrap();
        let config = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--preset",
            "local",
        ]))
        .unwrap();
        assert!(config.no_footer);
        assert_eq!(config.field_sources.footer, "preset:local");
    }

    #[test]
    fn invalid_footer_value_error_names_file_and_key() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_string_lossy().to_string();
        std::fs::create_dir_all(dir.path().join(".anvil")).unwrap();
        std::fs::write(
            dir.path().join(".anvil/config.toml"),
            "[preset.bad]\nfooter = \"blink\"\n",
        )
        .unwrap();

        let err = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--preset",
            "bad",
        ]))
        .unwrap_err()
        .to_string();

        assert!(err.contains(".anvil/config.toml"), "{err}");
        assert!(err.contains("preset.bad.footer"), "{err}");
        assert!(err.contains("on|off"), "{err}");
    }

    #[test]
    fn ollama_chat_timeout_defaults_to_local_provider_budget() {
        let cli = Cli::parse_from(["commandagent", "--provider", "ollama"]);
        let config = Config::from_cli(cli).unwrap();
        assert_eq!(config.chat_timeout_secs, LOCAL_PROVIDER_CHAT_TIMEOUT_SECS);
        assert_eq!(config.chat_timeout_source, "default:local_provider");
    }

    #[test]
    fn lm_studio_is_local_and_normalizes_optional_v1_suffix() {
        let cli = Cli::parse_from([
            "commandagent",
            "--provider",
            "lm-studio",
            "--model",
            "qwen/test",
            "--lm-studio-host",
            "http://127.0.0.1:1234/v1/",
        ]);
        let config = Config::from_cli(cli).unwrap();

        assert_eq!(config.provider, Provider::LmStudio);
        assert_eq!(config.planner_provider, Provider::LmStudio);
        assert_eq!(config.lm_studio_host, "http://127.0.0.1:1234");
        assert_eq!(config.chat_timeout_secs, LOCAL_PROVIDER_CHAT_TIMEOUT_SECS);
        assert_eq!(config.chat_timeout_source, "default:local_provider");
        assert_eq!(config.provider.as_str(), "lm-studio");
    }

    #[test]
    fn lm_studio_host_rejects_non_http_and_query_components() {
        let scheme = normalize_lm_studio_host("file:///tmp/lm-studio").unwrap_err();
        assert!(scheme.to_string().contains("http or https"));

        let query = normalize_lm_studio_host("http://localhost:1234?token=secret").unwrap_err();
        assert!(query.to_string().contains("query or fragment"));

        let credentials =
            normalize_lm_studio_host("http://operator:lm-studio-secret@localhost:1234")
                .unwrap_err()
                .to_string();
        assert!(credentials.contains("LM_STUDIO_API_TOKEN"));
        assert!(!credentials.contains("operator"));
        assert!(!credentials.contains("lm-studio-secret"));
    }

    #[test]
    fn preset_accepts_lm_studio_for_both_roles() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_string_lossy().to_string();
        std::fs::create_dir_all(dir.path().join(".commandagent")).unwrap();
        std::fs::write(
            dir.path().join(".commandagent/config.toml"),
            "[preset.local]\nmodel = \"qwen/test\"\nprovider = \"lm-studio\"\nplanner_model = \"qwen/test\"\nplanner_provider = \"lm-studio\"\n",
        )
        .unwrap();

        let config = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--preset",
            "local",
        ]))
        .unwrap();

        assert_eq!(config.provider, Provider::LmStudio);
        assert_eq!(config.planner_provider, Provider::LmStudio);
    }

    #[test]
    fn remote_chat_timeout_defaults_to_remote_provider_budget() {
        let cli = Cli::parse_from([
            "commandagent",
            "--provider",
            "openai",
            "--model",
            "gpt-test",
        ]);
        let config = Config::from_cli(cli).unwrap();
        assert_eq!(config.chat_timeout_secs, REMOTE_PROVIDER_CHAT_TIMEOUT_SECS);
        assert_eq!(config.chat_timeout_source, "default:remote_provider");
    }

    #[test]
    fn explicit_chat_timeout_wins_for_local_provider() {
        let cli = Cli::parse_from([
            "commandagent",
            "--provider",
            "ollama",
            "--chat-timeout-secs",
            "42",
        ]);
        let config = Config::from_cli(cli).unwrap();
        assert_eq!(config.chat_timeout_secs, 42);
        assert_eq!(config.chat_timeout_source, "flag");
    }

    #[test]
    fn preset_resolution_uses_flag_preset_default_precedence_and_sources() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_string_lossy().to_string();
        std::fs::create_dir_all(dir.path().join(".anvil")).unwrap();
        std::fs::write(
            dir.path().join(".anvil/config.toml"),
            r#"
[preset.local]
model = "preset-model"
provider = "ollama"
planner_model = "preset-planner"
planner_provider = "gemini"
context_budget = 12345
chat_timeout_secs = 77
prompt_layout = "legacy"
profile = "nextjs"
narration = "quiet"
"#,
        )
        .unwrap();

        let config = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--preset",
            "local",
            "--model",
            "flag-model",
            "--context-budget",
            "999",
            "--ultra-plan-run",
            "Web app",
        ]))
        .unwrap();

        assert_eq!(config.model, "flag-model");
        assert_eq!(config.field_sources.model, "flag");
        assert_eq!(config.context_budget, 999);
        assert_eq!(config.field_sources.context_budget, "flag");
        assert_eq!(config.provider, Provider::Ollama);
        assert_eq!(config.field_sources.provider, "preset:local");
        assert_eq!(config.planner_provider, Provider::Gemini);
        assert_eq!(config.field_sources.planner_provider, "preset:local");
        assert_eq!(config.planner_model, "preset-planner");
        assert_eq!(config.field_sources.planner_model, "preset:local");
        assert_eq!(config.chat_timeout_secs, 77);
        assert_eq!(config.field_sources.chat_timeout_secs, "preset:local");
        assert_eq!(config.prompt_layout, PromptLayout::Legacy);
        assert_eq!(config.field_sources.prompt_layout, "preset:local");
        assert_eq!(config.plan_preset, PlanPreset::None);
        assert_eq!(config.field_sources.plan_preset, "default");
        assert_eq!(config.profile, "nextjs");
        assert_eq!(config.field_sources.profile, "preset:local");
        assert!(config.profile_explicit);
        assert!(config.profile_inference.is_none());
        assert_eq!(config.narration, NarrationMode::Quiet);
        assert_eq!(config.field_sources.narration, "preset:local");
    }

    #[test]
    fn hybrid_a3b_preset_keeps_planner_and_executor_models_separate() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_string_lossy().to_string();
        std::fs::create_dir_all(dir.path().join(".anvil")).unwrap();
        std::fs::write(
            dir.path().join(".anvil/config.toml"),
            r#"
[preset.hybrid-a3b]
provider = "ollama"
model = "qwen3.6:35b-a3b-coding-nvfp4"
planner_provider = "gemini"
planner_model = "gemini-3.5-flash"
context_budget = 65536
chat_timeout_secs = 600
profile = "nextjs"
"#,
        )
        .unwrap();

        let config = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--preset",
            "hybrid-a3b",
            "--ultra-plan-run",
            "Web app",
        ]))
        .unwrap();

        assert_eq!(config.provider, Provider::Ollama);
        assert_eq!(config.model, "qwen3.6:35b-a3b-coding-nvfp4");
        assert_eq!(config.planner_provider, Provider::Gemini);
        assert_eq!(config.planner_model, "gemini-3.5-flash");
        assert_ne!(config.model, config.planner_model);
        assert_eq!(config.field_sources.model, "preset:hybrid-a3b");
        assert_eq!(config.field_sources.planner_model, "preset:hybrid-a3b");
    }

    #[test]
    fn prompt_layout_flag_wins_over_config_and_defaults_legacy() {
        let default = Config::from_cli(Cli::parse_from(["commandagent"])).unwrap();
        assert_eq!(default.prompt_layout, PromptLayout::Legacy);
        assert_eq!(default.field_sources.prompt_layout, "default");

        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_string_lossy().to_string();
        std::fs::create_dir_all(dir.path().join(".anvil")).unwrap();
        std::fs::write(
            dir.path().join(".anvil/config.toml"),
            "prompt_layout = \"legacy\"\n",
        )
        .unwrap();

        let from_config =
            Config::from_cli(Cli::parse_from(["commandagent", "--cwd", &cwd])).unwrap();
        assert_eq!(from_config.prompt_layout, PromptLayout::Legacy);
        assert_eq!(
            from_config.field_sources.prompt_layout,
            "config:config.toml"
        );

        let from_flag = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--prompt-layout",
            "stable",
        ]))
        .unwrap();
        assert_eq!(from_flag.prompt_layout, PromptLayout::Stable);
        assert_eq!(from_flag.field_sources.prompt_layout, "flag");
    }

    #[test]
    fn stream_defaults_by_action_and_resolves_flag_preset_config_precedence() {
        let repl = Config::from_cli(Cli::parse_from(["commandagent"])).unwrap();
        assert!(repl.stream);
        assert_eq!(repl.field_sources.stream, "default:repl");

        let prompt =
            Config::from_cli(Cli::parse_from(["commandagent", "--prompt", "hello"])).unwrap();
        assert!(!prompt.stream);
        assert_eq!(prompt.field_sources.stream, "default:non_interactive");

        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_string_lossy().to_string();
        std::fs::create_dir_all(dir.path().join(".anvil")).unwrap();
        std::fs::write(
            dir.path().join(".anvil/config.toml"),
            "stream = \"off\"\n[preset.live]\nstream = \"on\"\n",
        )
        .unwrap();

        let from_config =
            Config::from_cli(Cli::parse_from(["commandagent", "--cwd", &cwd])).unwrap();
        assert!(!from_config.stream);
        assert_eq!(from_config.field_sources.stream, "config:config.toml");

        let from_preset = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--preset",
            "live",
        ]))
        .unwrap();
        assert!(from_preset.stream);
        assert_eq!(from_preset.field_sources.stream, "preset:live");

        let from_flag = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--preset",
            "live",
            "--stream",
            "off",
        ]))
        .unwrap();
        assert!(!from_flag.stream);
        assert_eq!(from_flag.field_sources.stream, "flag");
    }

    #[test]
    fn invalid_stream_value_error_names_file_and_key() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_string_lossy().to_string();
        std::fs::create_dir_all(dir.path().join(".anvil")).unwrap();
        std::fs::write(
            dir.path().join(".anvil/config.toml"),
            "[preset.bad]\nstream = \"auto\"\n",
        )
        .unwrap();
        let err = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--preset",
            "bad",
        ]))
        .unwrap_err()
        .to_string();
        assert!(err.contains("stream on|off"), "{err}");
    }

    #[test]
    fn effective_streaming_requires_repl_and_both_ttys() {
        let mut config = Config::from_cli(Cli::parse_from(["commandagent"])).unwrap();
        assert!(config.streaming_enabled_for_tty(true, true));
        assert!(!config.streaming_enabled_for_tty(false, true));
        assert!(!config.streaming_enabled_for_tty(true, false));
        config.action = Action::Prompt("hello".to_string());
        assert!(!config.streaming_enabled_for_tty(true, true));
    }

    #[test]
    fn plan_preset_flag_config_and_preset_resolution_take_precedence() {
        let default = Config::from_cli(Cli::parse_from(["commandagent"])).unwrap();
        assert_eq!(default.plan_preset, PlanPreset::None);
        assert_eq!(default.plan_preset.as_str(), "none");
        assert_eq!(default.field_sources.plan_preset, "default:qwen27_planner");

        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_string_lossy().to_string();
        std::fs::create_dir_all(dir.path().join(".anvil")).unwrap();
        std::fs::write(
            dir.path().join(".anvil/config.toml"),
            "plan_preset = \"profile\"\n[preset.fast]\nplan_preset = \"none\"\n",
        )
        .unwrap();

        let from_config =
            Config::from_cli(Cli::parse_from(["commandagent", "--cwd", &cwd])).unwrap();
        assert_eq!(from_config.plan_preset, PlanPreset::Profile);
        assert_eq!(from_config.field_sources.plan_preset, "config:config.toml");

        let from_preset = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--preset",
            "fast",
        ]))
        .unwrap();
        assert_eq!(from_preset.plan_preset, PlanPreset::None);
        assert_eq!(from_preset.field_sources.plan_preset, "preset:fast");

        let from_flag = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--preset",
            "fast",
            "--plan-preset",
            "profile",
        ]))
        .unwrap();
        assert_eq!(from_flag.plan_preset, PlanPreset::Profile);
        assert_eq!(from_flag.field_sources.plan_preset, "flag");
        assert_eq!(from_flag.plan_preset_origin(), "cli");
    }

    #[test]
    fn qwen27_and_gemma_planners_default_none_while_explicit_opt_in_wins() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_string_lossy().to_string();
        std::fs::create_dir_all(dir.path().join(".anvil")).unwrap();
        std::fs::write(
            dir.path().join(".anvil/config.toml"),
            r#"
[preset.qwen27]
provider = "ollama"
model = "qwen3.6:27b-coding-nvfp4"
planner_provider = "ollama"
planner_model = "qwen3.6:27b-coding-nvfp4"
profile = "nextjs"

[preset.gemma]
provider = "ollama"
model = "qwen3.6:27b-coding-nvfp4"
planner_provider = "ollama"
planner_model = "gemma4:31b-cloud"
profile = "nextjs"
"#,
        )
        .unwrap();

        let qwen = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--preset",
            "qwen27",
            "--ultra-plan-run",
            "Web app",
        ]))
        .unwrap();
        assert_eq!(qwen.plan_preset, PlanPreset::None);
        assert_eq!(qwen.field_sources.plan_preset, "default:qwen27_planner");
        assert_eq!(qwen.plan_preset_origin(), "default");

        let gemma = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--preset",
            "gemma",
            "--ultra-plan-run",
            "Web app",
        ]))
        .unwrap();
        assert_eq!(gemma.plan_preset, PlanPreset::None);
        assert_eq!(gemma.field_sources.plan_preset, "default:gemma_planner");
        assert_eq!(gemma.plan_preset_origin(), "default");

        let cli_override = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--preset",
            "qwen27",
            "--plan-preset",
            "profile",
            "--ultra-plan-run",
            "Web app",
        ]))
        .unwrap();
        assert_eq!(cli_override.plan_preset, PlanPreset::Profile);
        assert_eq!(cli_override.field_sources.plan_preset, "flag");
        assert_eq!(cli_override.plan_preset_origin(), "cli");
    }

    #[test]
    fn investigate_data_defaults_to_profile_and_explicit_none_wins() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_string_lossy().to_string();
        let defaulted = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--intent",
            "investigate",
            "--profile",
            "data",
        ]))
        .unwrap();
        assert_eq!(defaulted.plan_preset, PlanPreset::Profile);
        assert_eq!(
            defaulted.field_sources.plan_preset,
            "default_investigate_data"
        );
        assert_eq!(defaulted.plan_preset_origin(), "default_investigate_data");

        let explicit_none = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--intent",
            "investigate",
            "--profile",
            "data",
            "--plan-preset",
            "none",
        ]))
        .unwrap();
        assert_eq!(explicit_none.plan_preset, PlanPreset::None);
        assert_eq!(explicit_none.plan_preset_origin(), "cli");
    }

    #[test]
    fn create_ingest_defaults_to_profile_and_explicit_none_wins() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_string_lossy().to_string();
        let defaulted = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--intent",
            "create",
            "--profile",
            "ingest",
        ]))
        .unwrap();
        assert_eq!(defaulted.plan_preset, PlanPreset::Profile);
        assert_eq!(defaulted.field_sources.plan_preset, "default_create_ingest");
        assert_eq!(defaulted.plan_preset_origin(), "default_create_ingest");

        let explicit_none = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--intent",
            "create",
            "--profile",
            "ingest",
            "--plan-preset",
            "none",
        ]))
        .unwrap();
        assert_eq!(explicit_none.plan_preset, PlanPreset::None);
        assert_eq!(explicit_none.plan_preset_origin(), "cli");
    }

    #[test]
    fn resolved_cli_planner_model_controls_none_default_source() {
        let qwen = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--planner-model",
            "Qwen3.6:27B-Coding-NVFP4",
        ]))
        .unwrap();
        assert_eq!(qwen.plan_preset, PlanPreset::None);
        assert_eq!(qwen.field_sources.plan_preset, "default:qwen27_planner");
        assert_eq!(qwen.plan_preset_origin(), "default");

        let inherited = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--model",
            "vendor/qwen-coder-27b",
        ]))
        .unwrap();
        assert_eq!(inherited.planner_model, "vendor/qwen-coder-27b");
        assert_eq!(inherited.plan_preset, PlanPreset::None);
        assert_eq!(
            inherited.field_sources.plan_preset,
            "default:qwen27_planner"
        );

        let explicit_off = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--planner-model",
            "qwen3.6:27b-coding-nvfp4",
            "--plan-preset",
            "none",
        ]))
        .unwrap();
        assert_eq!(explicit_off.plan_preset, PlanPreset::None);
        assert_eq!(explicit_off.field_sources.plan_preset, "flag");
        assert_eq!(explicit_off.plan_preset_origin(), "cli");

        let gemma = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--planner-model",
            "gemma4:31b-cloud",
        ]))
        .unwrap();
        assert_eq!(gemma.plan_preset, PlanPreset::None);
        assert_eq!(gemma.field_sources.plan_preset, "default:gemma_planner");
        assert_eq!(gemma.plan_preset_origin(), "default");
    }

    #[test]
    fn invalid_plan_preset_value_error_names_file_and_key() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_string_lossy().to_string();
        std::fs::create_dir_all(dir.path().join(".anvil")).unwrap();
        std::fs::write(
            dir.path().join(".anvil/config.toml"),
            "[preset.bad]\nplan_preset = \"always\"\n",
        )
        .unwrap();

        let err = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--preset",
            "bad",
        ]))
        .unwrap_err()
        .to_string();

        assert!(err.contains(".anvil/config.toml"), "{err}");
        assert!(err.contains("preset.bad.plan_preset"), "{err}");
        assert!(err.contains("none|profile"), "{err}");
    }

    #[test]
    fn missing_preset_error_names_file_and_key() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_string_lossy().to_string();
        std::fs::create_dir_all(dir.path().join(".anvil")).unwrap();
        std::fs::write(dir.path().join(".anvil/config.toml"), "# no preset\n").unwrap();

        let err = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--preset",
            "missing-pre-ux-final",
        ]))
        .unwrap_err()
        .to_string();

        assert!(
            err.contains("preset 'missing-pre-ux-final' not found"),
            "{err}"
        );
        assert!(err.contains(".anvil/config.toml"), "{err}");
        assert!(err.contains("preset.missing-pre-ux-final"), "{err}");
    }

    #[test]
    fn invalid_preset_value_error_names_file_and_key() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_string_lossy().to_string();
        std::fs::create_dir_all(dir.path().join(".anvil")).unwrap();
        std::fs::write(
            dir.path().join(".anvil/config.toml"),
            "[preset.bad]\nprovider = \"bogus\"\n",
        )
        .unwrap();

        let err = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--preset",
            "bad",
        ]))
        .unwrap_err()
        .to_string();

        assert!(err.contains(".anvil/config.toml"), "{err}");
        assert!(err.contains("preset.bad.provider"), "{err}");
        assert!(err.contains("ollama|lm-studio|openai|gemini"), "{err}");
    }

    #[test]
    fn profile_infers_from_goal_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_string_lossy().to_string();
        let cli = Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--ultra-plan-run",
            "Web アプリを作って",
        ]);
        let config = Config::from_cli(cli).unwrap();
        assert_eq!(config.profile, "nextjs");
        let inference = config.profile_inference.expect("profile inference");
        assert_eq!(inference.source.as_str(), "goal");
        assert!(!config.profile_explicit);
    }

    #[test]
    fn profile_infers_python_cli_from_goal_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_string_lossy().to_string();
        let cli = Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--ultra-plan-run",
            "Python コマンドラインを作って",
        ]);
        let config = Config::from_cli(cli).unwrap();
        assert_eq!(config.profile, "python-cli");
        let inference = config.profile_inference.expect("profile inference");
        assert_eq!(inference.source.as_str(), "goal");
    }

    #[test]
    fn profile_infers_from_workspace_manifest_after_goal_miss() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"dependencies":{"next":"^14.2.0"}}"#,
        )
        .unwrap();
        let cwd = dir.path().to_string_lossy().to_string();
        let cli = Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--ultra-plan-run",
            "実装して",
        ]);
        let config = Config::from_cli(cli).unwrap();
        assert_eq!(config.profile, "nextjs");
        let inference = config.profile_inference.expect("profile inference");
        assert_eq!(inference.source.as_str(), "workspace");
    }

    #[test]
    fn profile_infers_python_cli_from_pyproject_after_goal_miss() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[project]\nname = \"x\"\n",
        )
        .unwrap();
        let cwd = dir.path().to_string_lossy().to_string();
        let cli = Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--ultra-plan-run",
            "実装して",
        ]);
        let config = Config::from_cli(cli).unwrap();
        assert_eq!(config.profile, "python-cli");
        let inference = config.profile_inference.expect("profile inference");
        assert_eq!(inference.source.as_str(), "workspace");
    }

    #[test]
    fn explicit_generic_profile_suppresses_auto_inference() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[project]\nname = \"x\"\n",
        )
        .unwrap();
        let cwd = dir.path().to_string_lossy().to_string();
        let cli = Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--profile",
            "generic",
            "--ultra-plan-run",
            "Python CLI を作って",
        ]);
        let config = Config::from_cli(cli).unwrap();
        assert_eq!(config.profile, "generic");
        assert!(config.profile_explicit);
        assert!(config.profile_inference.is_none());
    }
}
