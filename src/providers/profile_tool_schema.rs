//! Which clients can carry a G2 schema tool (Issue #973, Responses since Issue #1005).
//!
//! Support is declared, by the client or by the configuration that would build it.
//! Nothing is inferred from a label, and a client that declares nothing, including an
//! injected one, is refused rather than assumed. Chat Completions and the Responses API
//! carry schema tools on the native tool protocol; both Gemini APIs and Ollama are
//! refused here without any change to those clients.

use serde_json::Value;

use crate::config::{Config, OpenAiApi, Provider, ProviderRole};

use super::parsing::sanitize_schema;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileToolSchemaWire {
    /// The Chat Completions request and parser, on the native tool protocol.
    ChatCompletions,
    /// The Responses request and parser, on the native tool protocol (Issue #1005).
    Responses,
    /// The default: Gemini, Ollama, a wrapper, and an injected client.
    Undeclared,
}

impl ProfileToolSchemaWire {
    pub(crate) const fn carries_schema_tools(self) -> bool {
        matches!(self, Self::ChatCompletions | Self::Responses)
    }

    pub(crate) const fn describe(self) -> &'static str {
        match self {
            Self::ChatCompletions => "uses Chat Completions",
            Self::Responses => "uses the Responses API",
            Self::Undeclared => "declares no schema wire",
        }
    }

    pub(super) const fn for_api(api: OpenAiApi) -> Self {
        match api {
            OpenAiApi::ChatCompletions => Self::ChatCompletions,
            OpenAiApi::Responses => Self::Responses,
        }
    }
}

/// What `client_from_config_for_role` would build for this role, without building it.
pub(crate) fn wire_for_role(config: &Config, role: ProviderRole) -> ProfileToolSchemaWire {
    if config
        .openai_compatible
        .as_ref()
        .is_some_and(|compatible| compatible.applies_to(role))
    {
        return ProfileToolSchemaWire::for_api(config.openai_api);
    }
    let provider = match role {
        ProviderRole::Executor => config.provider,
        ProviderRole::Planner => config.planner_provider,
        ProviderRole::Classifier => config.classifier_provider,
    };
    match provider {
        Provider::LmStudio | Provider::Openai => ProfileToolSchemaWire::for_api(config.openai_api),
        Provider::Ollama | Provider::Gemini => ProfileToolSchemaWire::Undeclared,
    }
}

/// The shared sanitizer treats every object as a schema node, so a property or an
/// enum value with one of its keyword names would be lost on the way out.
pub(crate) fn survives_sanitizer(schema: &Value) -> bool {
    sanitize_schema(schema) == *schema
}
