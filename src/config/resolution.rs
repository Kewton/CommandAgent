//! Provider and preset resolution for [`Config`](super::Config).
//!
//! Holds the CLI > preset > default precedence for the executor, planner and
//! classifier roles, the OpenAI-compatible role assignment, and the preset
//! file layering (`extends`, per-file merge, preset key parsing). Deciding the
//! CLI action and starting a provider stay outside this leaf.

use anyhow::{Context, bail};

use super::{
    DEFAULT_MODEL, OllamaThink, OpenAiApi, OpenAiCompatibleConfig, PresetConfig, Provider,
    ProviderCliOptions, ProviderSelection, SUPPORTED_PRESET_KEYS, Sourced, ToolProtocol,
    config_paths, normalize_openai_compatible_base_url, parse_config_file_if_present,
    parse_footer_value, parse_narration_value, parse_ollama_think_value, parse_openai_api_value,
    parse_plan_preset_value, parse_prompt_layout_value, parse_provider_value, parse_stream_value,
    parse_string_value, parse_tool_protocol_value, parse_u64_value, parse_usize_value, sourced,
    validate_api_key_env_name, validate_openai_model,
};
use crate::cli::Cli;
use std::path::Path;

pub(super) struct ProviderResolution {
    pub(super) model: Sourced<String>,
    pub(super) provider: Sourced<ProviderSelection>,
    pub(super) openai_api: Sourced<OpenAiApi>,
    pub(super) tool_protocol: Option<Sourced<ToolProtocol>>,
    pub(super) planner_provider: Sourced<ProviderSelection>,
    pub(super) planner_model: Sourced<String>,
    pub(super) planner_think: Sourced<OllamaThink>,
    pub(super) ollama_think: Option<OllamaThink>,
    pub(super) classifier_provider: Sourced<ProviderSelection>,
    pub(super) classifier_model: Sourced<String>,
    pub(super) openai_compatible: Option<OpenAiCompatibleConfig>,
}

pub(super) fn resolve_providers(
    cli: &Cli,
    preset: &Option<PresetConfig>,
    provider_options: &ProviderCliOptions,
) -> anyhow::Result<ProviderResolution> {
    let model = cli
        .model
        .clone()
        .map(|value| sourced(value, "flag"))
        .or_else(|| preset.as_ref().and_then(|preset| preset.model.clone()))
        .unwrap_or_else(|| sourced(DEFAULT_MODEL.to_string(), "default"));
    let provider = cli
        .provider
        .map(|value| {
            sourced(
                if provider_options.executor_openai_compatible {
                    ProviderSelection::OpenAiCompatible
                } else {
                    ProviderSelection::BuiltIn(Provider::from(value))
                },
                "flag",
            )
        })
        .or_else(|| preset.as_ref().and_then(|preset| preset.provider.clone()))
        .unwrap_or_else(|| sourced(ProviderSelection::BuiltIn(Provider::Ollama), "default"));
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
        .map(|value| {
            sourced(
                if provider_options.planner_openai_compatible {
                    ProviderSelection::OpenAiCompatible
                } else {
                    ProviderSelection::BuiltIn(Provider::from(value))
                },
                "flag",
            )
        })
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
        && provider.value.provider() != Provider::Ollama
        && planner_provider.value.provider() != Provider::Ollama
    {
        bail!("--think requires provider or planner_provider to be ollama");
    }
    validate_openai_model(provider.value.provider(), &model.value, "executor")?;
    validate_openai_model(
        planner_provider.value.provider(),
        &planner_model.value,
        "planner",
    )?;
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
        classifier_provider.value.provider(),
        &classifier_model.value,
        "classifier",
    )?;
    let openai_compatible_roles = (
        provider.value.is_openai_compatible(),
        planner_provider.value.is_openai_compatible(),
        classifier_provider.value.is_openai_compatible(),
    );
    let any_openai_compatible =
        openai_compatible_roles.0 || openai_compatible_roles.1 || openai_compatible_roles.2;
    if !any_openai_compatible
        && (provider_options.base_url.is_some() || provider_options.api_key_env.is_some())
    {
        bail!(
            "--base-url and --api-key-env require provider or planner_provider to be openai-compatible"
        );
    }
    let configured_base_url = provider_options.base_url.clone().or_else(|| {
        preset
            .as_ref()
            .and_then(|preset| preset.base_url.as_ref())
            .map(|value| value.value.clone())
    });
    let configured_api_key_env = provider_options.api_key_env.clone().or_else(|| {
        preset
            .as_ref()
            .and_then(|preset| preset.api_key_env.as_ref())
            .map(|value| value.value.clone())
    });
    let openai_compatible = if any_openai_compatible {
        let base_url = configured_base_url
            .context("openai-compatible requires --base-url or preset key base_url")?;
        let api_key_env = configured_api_key_env
            .map(|value| validate_api_key_env_name(&value))
            .transpose()?;
        Some(OpenAiCompatibleConfig {
            base_url: normalize_openai_compatible_base_url(&base_url)?,
            api_key_env,
            executor: openai_compatible_roles.0,
            planner: openai_compatible_roles.1,
            classifier: openai_compatible_roles.2,
        })
    } else {
        None
    };
    Ok(ProviderResolution {
        model,
        provider,
        openai_api,
        tool_protocol,
        planner_provider,
        planner_model,
        planner_think,
        ollama_think,
        classifier_provider,
        classifier_model,
        openai_compatible,
    })
}

pub(super) fn load_named_preset(
    root: &Path,
    name: Option<&str>,
) -> anyhow::Result<Option<PresetConfig>> {
    let Some(name) = name.map(str::trim).filter(|name| !name.is_empty()) else {
        return Ok(None);
    };
    resolve_named_preset(root, name, &mut Vec::new()).map(Some)
}

pub(super) fn resolve_named_preset(
    root: &Path,
    name: &str,
    chain: &mut Vec<String>,
) -> anyhow::Result<PresetConfig> {
    if let Some(cycle_start) = chain.iter().position(|entry| entry == name) {
        let mut cycle = chain[cycle_start..].to_vec();
        cycle.push(name.to_string());
        bail!("preset inheritance cycle: {}", cycle.join(" -> "));
    }
    chain.push(name.to_string());

    let mut found = false;
    let mut merged = PresetConfig::default();
    for path in config_paths(root) {
        let Some(file) = parse_config_file_if_present(&path, Some(name))? else {
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
    if let Some(parent) = merged.extends.as_ref().map(|value| value.value.clone()) {
        let inherited = resolve_named_preset(root, &parent, chain)
            .with_context(|| format!("preset '{name}' extends '{parent}'"))?;
        merge_inherited_preset(&mut merged, &inherited);
    }
    chain.pop();
    Ok(merged)
}

pub(super) fn preset_complete(preset: &PresetConfig) -> bool {
    preset_missing_keys(preset).is_empty()
}

pub(super) fn preset_missing_keys(preset: &PresetConfig) -> Vec<&'static str> {
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

pub(super) fn merge_preset(target: &mut PresetConfig, source: &PresetConfig) {
    merge_preset_field(&mut target.extends, &source.extends);
    merge_inherited_preset(target, source);
}

pub(super) fn merge_inherited_preset(target: &mut PresetConfig, source: &PresetConfig) {
    merge_preset_field(&mut target.pack, &source.pack);
    merge_preset_field(&mut target.model, &source.model);
    merge_preset_field(&mut target.provider, &source.provider);
    merge_preset_field(&mut target.base_url, &source.base_url);
    merge_preset_field(&mut target.api_key_env, &source.api_key_env);
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

pub(super) fn merge_preset_field<T: Clone>(
    target: &mut Option<Sourced<T>>,
    source: &Option<Sourced<T>>,
) {
    if target.is_none() {
        *target = source.clone();
    }
}

pub(super) fn parse_preset_key(
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
        "extends" => {
            preset.extends = Some(sourced(
                parse_string_value(path, line_no, &full_key, value)?,
                source,
            ))
        }
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
        "base_url" => {
            preset.base_url = Some(sourced(
                parse_string_value(path, line_no, &full_key, value)?,
                source,
            ))
        }
        "api_key_env" => {
            preset.api_key_env = Some(sourced(
                parse_string_value(path, line_no, &full_key, value)?,
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
