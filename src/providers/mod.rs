pub(crate) mod compatible_effort;
pub mod gemini;
pub mod gemini_function_calling;
pub(crate) mod guidance;
pub mod lm_studio;
pub mod ollama;
pub mod openai;
mod openai_chat_completions;
pub mod openai_compatible;
mod openai_responses;
pub(crate) mod openrouter_routing;
pub mod parsing;
pub(crate) mod startup;
pub mod streaming;
pub mod xml_fallback;
pub(crate) mod xml_repair;

use anyhow::bail;
use serde::{Deserialize, Serialize};

use crate::config::{Config, Provider, ProviderRole};
use crate::state::{ConversationMessage, ToolCall};
use crate::tools::registry::ToolSpec;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ResponseTiming {
    pub prompt_eval_duration: Option<u64>,
    pub eval_duration: Option<u64>,
    pub load_duration: Option<u64>,
    pub total_duration: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProviderResponseMetadata {
    pub response_id: Option<String>,
    pub model_id: Option<String>,
    pub system_fingerprint: Option<String>,
    pub created_epoch: Option<i64>,
    pub service_tier: Option<String>,
    pub cached_input_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    /// The amount the provider itself reports it billed for this response
    /// (Issue #947), never a price computed from token counts. `None` means
    /// unreported; an explicit zero is `Some`. The `provider_response` and
    /// `provider_turn_duration` events of one turn carry the same amount, so an
    /// aggregator counts it once per turn, never the two events added together.
    pub billed_cost_usd: Option<BilledCostUsd>,
}

/// A provider-reported USD amount, kept as the decimal text of the JSON number
/// the provider sent so the metadata stays `Eq` and nothing is rounded.
///
/// Only a finite, non-negative JSON number up to [`Self::MAX`] is accepted; it
/// is written back to events as the same JSON number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BilledCostUsd(String);

impl BilledCostUsd {
    /// The largest accepted amount: 2^53 - 1, beyond which an `f64` (and a
    /// JSON consumer reading it as one) no longer holds whole dollars exactly.
    pub const MAX: f64 = 9_007_199_254_740_991.0;

    /// The amount in a JSON value, or `None` for anything but a finite,
    /// non-negative number no larger than [`Self::MAX`]: null, strings,
    /// booleans, arrays, objects, negative numbers (including `-0`), and
    /// out-of-range numbers are unreported, never zero.
    pub fn from_json(value: &serde_json::Value) -> Option<Self> {
        let number = value.as_number()?;
        let amount = number.as_f64()?;
        let valid = amount.is_finite() && !amount.is_sign_negative() && amount <= Self::MAX;
        valid.then(|| Self(number.to_string()))
    }

    /// The decimal text of the accepted JSON number, e.g. `0.012345`.
    pub fn as_decimal_str(&self) -> &str {
        &self.0
    }

    /// The amount as the JSON number it was received as.
    pub fn to_json(&self) -> Option<serde_json::Value> {
        serde_json::from_str::<serde_json::Number>(&self.0)
            .ok()
            .map(serde_json::Value::Number)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssistantReply {
    pub content: String,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    #[serde(default)]
    pub prompt_tokens: Option<u64>,
    #[serde(default)]
    pub completion_tokens: Option<u64>,
}

impl AssistantReply {
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            tool_calls: Vec::new(),
            prompt_tokens: None,
            completion_tokens: None,
        }
    }
}

pub trait ChatClient: Send {
    fn label(&self) -> &str;
    fn boxed_clone(&self) -> Box<dyn ChatClient>;
    fn supports_native_tools(&self, _model: &str) -> bool {
        false
    }
    fn allows_xml_fallback(&self) -> bool {
        false
    }
    /// True when this client parses XML tool calls out of the reply text
    /// itself, before the host sees the reply.
    fn parses_xml_tool_calls_before_host(&self) -> bool {
        false
    }
    fn supports_ollama_think(&self) -> bool {
        false
    }
    fn take_response_timing(&mut self) -> Option<ResponseTiming> {
        None
    }
    fn take_response_metadata(&mut self) -> Option<ProviderResponseMetadata> {
        None
    }
    /// Receive the cancel signal for the next call. Clients that own their HTTP
    /// transport close the connection when it fires; others may ignore it.
    fn set_cancel_token(&mut self, _token: streaming::CancelToken) {}
    fn supports_streaming(&self) -> bool {
        false
    }
    fn supports_streaming_for_model(&self, _model: &str) -> bool {
        self.supports_streaming()
    }
    fn chat_stream(
        &mut self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
        _on_chunk: &mut dyn FnMut(&str) -> anyhow::Result<()>,
    ) -> anyhow::Result<AssistantReply> {
        bail!("streaming is not supported by this chat client")
    }
    fn chat(
        &mut self,
        model: &str,
        messages: &[ConversationMessage],
        tools: &[ToolSpec],
        native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply>;
}

pub fn client_from_config(config: &Config, planner: bool) -> anyhow::Result<Box<dyn ChatClient>> {
    client_from_config_for_role(
        config,
        if planner {
            ProviderRole::Planner
        } else {
            ProviderRole::Executor
        },
    )
}

pub(crate) fn client_from_config_for_role(
    config: &Config,
    role: ProviderRole,
) -> anyhow::Result<Box<dyn ChatClient>> {
    if config
        .openai_compatible
        .as_ref()
        .is_some_and(|compatible| compatible.applies_to(role))
    {
        return Ok(Box::new(
            openai_compatible::OpenAiCompatibleClient::from_openai_compatible_env(config, role)?,
        ));
    }
    let provider = match role {
        ProviderRole::Executor => config.provider,
        ProviderRole::Planner => config.planner_provider,
        ProviderRole::Classifier => config.classifier_provider,
    };
    match provider {
        Provider::Ollama => Ok(Box::new(
            ollama::OllamaClient::new(
                config.ollama_host.clone(),
                config.chat_timeout_secs,
                config.num_predict,
                config.chat_retries,
            )?
            .with_context_budget(config.context_budget)
            .with_think(if role == ProviderRole::Planner {
                config.planner_think
            } else {
                config.ollama_think
            }),
        )),
        Provider::LmStudio => Ok(Box::new(lm_studio::LmStudioClient::from_env_for_role(
            config, role,
        )?)),
        Provider::Openai => Ok(Box::new(openai::OpenAiClient::from_env(config)?)),
        Provider::Gemini => Ok(Box::new(gemini::GeminiClient::from_env(config)?)),
    }
}

pub fn model_for(config: &Config, planner: bool) -> &str {
    if planner {
        &config.planner_model
    } else {
        &config.model
    }
}

pub fn ensure_known_tool(tool_name: &str, tools: &[ToolSpec]) -> anyhow::Result<()> {
    if tools.iter().any(|tool| tool.function.name == tool_name) {
        return Ok(());
    }
    bail!("unknown tool requested by provider: {tool_name}")
}
