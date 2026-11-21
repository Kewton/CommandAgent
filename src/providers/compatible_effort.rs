//! Opt-in reasoning effort for the OpenAI-compatible transport (Issue #931).
//!
//! `COMMANDAGENT_COMPATIBLE_REASONING_EFFORT` declares one effort word for every
//! role that runs on the shared OpenAI-compatible transport: the generic
//! `openai-compatible` override and LM Studio. It is a declaration, not a
//! capability check: CommandAgent validates the word against its own vocabulary
//! and never asks the server which values the selected model supports, and it
//! never branches on a model or host name. Ollama native, OpenAI, and Gemini
//! roles neither send the value nor fail on it.

use std::ffi::OsStr;

use serde_json::{Value, json};

use crate::config::{Config, Provider, ProviderRole};

pub const COMPATIBLE_REASONING_EFFORT_ENV: &str = "COMMANDAGENT_COMPATIBLE_REASONING_EFFORT";

/// The accepted words, matched exactly after trimming surrounding whitespace.
pub const COMPATIBLE_REASONING_EFFORT_VALUES: [&str; 7] =
    ["none", "minimal", "low", "medium", "high", "xhigh", "max"];

/// The only source of a declared value.
pub(crate) const SOURCE_ENV: &str = "env";

const ROLES: [ProviderRole; 3] = [
    ProviderRole::Executor,
    ProviderRole::Planner,
    ProviderRole::Classifier,
];

/// The process-level setting. An invalid value keeps no copy of the raw text,
/// so neither `Debug` nor any projection of this type can disclose it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompatibleEffortSetting {
    Omitted,
    Declared(&'static str),
    Invalid,
}

impl CompatibleEffortSetting {
    pub(crate) fn from_process_env() -> Self {
        Self::parse(std::env::var_os(COMPATIBLE_REASONING_EFFORT_ENV).as_deref())
    }

    /// Unset, empty, and whitespace-only values omit the request field. Any
    /// other value must be one of the accepted words; case and aliases are not
    /// converted, and a non-Unicode value is invalid.
    pub(crate) fn parse(raw: Option<&OsStr>) -> Self {
        let Some(raw) = raw else {
            return Self::Omitted;
        };
        let Some(text) = raw.to_str() else {
            return Self::Invalid;
        };
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Self::Omitted;
        }
        COMPATIBLE_REASONING_EFFORT_VALUES
            .iter()
            .copied()
            .find(|value| *value == trimmed)
            .map_or(Self::Invalid, Self::Declared)
    }

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Omitted => "omitted",
            Self::Declared(_) => "declared",
            Self::Invalid => "invalid",
        }
    }

    pub(crate) const fn requested_value(self) -> Option<&'static str> {
        match self {
            Self::Declared(value) => Some(value),
            Self::Omitted | Self::Invalid => None,
        }
    }

    /// The value a compatible client sends, or the configuration error that
    /// stops the client before it is built.
    pub(crate) fn for_client(self) -> anyhow::Result<Option<&'static str>> {
        match self {
            Self::Invalid => Err(anyhow::anyhow!(invalid_message())),
            Self::Omitted | Self::Declared(_) => Ok(self.requested_value()),
        }
    }
}

/// Names the variable and the accepted words, never the rejected value.
pub(crate) fn invalid_message() -> String {
    format!(
        "{COMPATIBLE_REASONING_EFFORT_ENV} is not a supported reasoning effort (the value is not shown); use one of {} exactly, or unset it to omit the request field",
        COMPATIBLE_REASONING_EFFORT_VALUES.join(", ")
    )
}

pub(crate) const fn role_name(role: ProviderRole) -> &'static str {
    match role {
        ProviderRole::Executor => "executor",
        ProviderRole::Planner => "planner",
        ProviderRole::Classifier => "classifier",
    }
}

/// Whether `role` runs on the OpenAI-compatible transport: the generic
/// override or LM Studio, exactly as `client_from_config_for_role` resolves it.
pub(crate) fn role_applies(config: &Config, role: ProviderRole) -> bool {
    if config
        .openai_compatible
        .as_ref()
        .is_some_and(|compatible| compatible.applies_to(role))
    {
        return true;
    }
    let provider = match role {
        ProviderRole::Executor => config.provider,
        ProviderRole::Planner => config.planner_provider,
        ProviderRole::Classifier => config.classifier_provider,
    };
    provider == Provider::LmStudio
}

/// The per-role declaration shared by doctor and the headless summary. It
/// states what CommandAgent will send, not what the server applied.
pub(crate) fn declaration(config: &Config, setting: CompatibleEffortSetting) -> Value {
    let roles = ROLES
        .iter()
        .map(|role| {
            let applies = role_applies(config, *role);
            let status = if applies {
                setting.as_str()
            } else {
                "not_applicable"
            };
            let requested_value = applies.then(|| setting.requested_value()).flatten();
            json!({
                "role": role_name(*role),
                "provider": config.provider_label(*role),
                "status": status,
                "requested_value": requested_value,
                "source": requested_value.map(|_| SOURCE_ENV),
            })
        })
        .collect::<Vec<_>>();
    let applicable_roles = ROLES
        .iter()
        .filter(|role| role_applies(config, **role))
        .map(|role| role_name(*role))
        .collect::<Vec<_>>();
    let status = if applicable_roles.is_empty() {
        "not_applicable"
    } else {
        setting.as_str()
    };
    let requested_value = (!applicable_roles.is_empty())
        .then(|| setting.requested_value())
        .flatten();
    json!({
        "env": COMPATIBLE_REASONING_EFFORT_ENV,
        "status": status,
        "declared": requested_value.is_some(),
        "requested_value": requested_value,
        "source": requested_value.map(|_| SOURCE_ENV),
        "setting": setting.as_str(),
        "applicable_roles": applicable_roles,
        "roles": roles,
        "server_support_verified": false,
    })
}

#[cfg(test)]
mod tests;
