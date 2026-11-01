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

pub(crate) use openai_chat_completions::{ToolArgumentsParseError, ToolArgumentsParseFailure};

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
    /// `provider_turn_duration` events of one turn both carry it as the JSON
    /// string `provider_billed_cost_usd` (the decimal text the provider sent,
    /// e.g. `"0.012345"`); an aggregator reads it as a decimal, never as a
    /// float, and counts it once per turn, never the two events added together.
    /// When the amount text equals or contains a registered secret value, both
    /// events leave the field out, exactly as for an unreported amount.
    pub billed_cost_usd: Option<BilledCostUsd>,
}

/// A provider-reported USD amount, kept as the exact decimal text of the JSON
/// number token the provider sent, read before any `f64` conversion, so the
/// metadata stays `Eq` and nothing is rounded or underflows (`1e-400` stays
/// `1e-400`, never zero).
///
/// Only an unsigned JSON number token no larger than [`Self::MAX`] is accepted;
/// it is written to events as that decimal text in a JSON string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BilledCostUsd(String);

impl BilledCostUsd {
    /// The largest accepted amount, 2^53 - 1, as decimal digits. It is
    /// compared digit by digit, never through an `f64`.
    pub const MAX: &'static str = "9007199254740991";

    /// The amount in a raw JSON value token, or `None` for anything but a JSON
    /// number token with no sign and a value no larger than [`Self::MAX`]:
    /// null, strings, booleans, arrays, objects, negative numbers (including
    /// `-0`), and larger numbers are unreported, never zero. `0`, `0.0`, and
    /// `0e5` are an explicit zero.
    pub fn from_json_token(token: &str) -> Option<Self> {
        let token = token.trim();
        unsigned_decimal_within(token, Self::MAX).then(|| Self(token.to_string()))
    }

    /// The decimal text of the accepted JSON number, e.g. `0.012345`.
    pub fn as_decimal_str(&self) -> &str {
        &self.0
    }

    /// The amount as written to an event bound for `events_path`: the decimal
    /// text in a JSON string, e.g. `"0.012345"`, for an aggregator to read as a
    /// decimal. `None` (the field is left out, as for an unreported amount)
    /// when the redaction context that `eval_events::emit` applies to that
    /// event would rewrite any part of the text, because the amount equals or
    /// contains a registered secret value; a marker never reaches the field,
    /// and the original text is never restored past the scrub.
    pub fn event_value(&self, events_path: Option<&std::path::Path>) -> Option<serde_json::Value> {
        let overlaps_a_secret = crate::sensitive_data::active_for(events_path)
            .is_some_and(|context| context.scrub_text(&self.0) != self.0);
        (!overlaps_a_secret).then(|| serde_json::Value::String(self.0.clone()))
    }
}

/// Whether `token` follows the JSON number grammar without a sign
/// (`0|[1-9][0-9]*`, optional `.` fraction, optional exponent) and its value is
/// at most the integer `max` (decimal digits without leading zeros). The
/// comparison is exact in decimal; nothing goes through an `f64`.
fn unsigned_decimal_within(token: &str, max: &str) -> bool {
    let (mantissa, exponent) = match token.find(['e', 'E']) {
        Some(at) => (&token[..at], Some(&token[at + 1..])),
        None => (token, None),
    };
    let (integer, fraction) = match mantissa.split_once('.') {
        Some((integer, fraction)) => (integer, Some(fraction)),
        None => (mantissa, None),
    };
    let all_digits =
        |text: &str| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit());
    if !all_digits(integer) || (integer.len() > 1 && integer.starts_with('0')) {
        return false;
    }
    if fraction.is_some_and(|fraction| !all_digits(fraction)) {
        return false;
    }
    let exponent = match exponent {
        None => 0_i64,
        Some(exponent) => {
            let (negative, digits) = match exponent.as_bytes().first() {
                Some(b'+') => (false, &exponent[1..]),
                Some(b'-') => (true, &exponent[1..]),
                _ => (false, exponent),
            };
            if !all_digits(digits) {
                return false;
            }
            // Saturate far beyond any digit count; only the sign of the
            // comparison below matters.
            let magnitude = digits.trim_start_matches('0');
            let magnitude = if magnitude.len() > 15 {
                i64::MAX / 4
            } else {
                magnitude.parse::<i64>().unwrap_or(0)
            };
            if negative { -magnitude } else { magnitude }
        }
    };
    let fraction = fraction.unwrap_or("");
    let digits = format!("{integer}{fraction}");
    let significant = digits.trim_start_matches('0');
    if significant.is_empty() {
        return true;
    }
    let trimmed = significant.trim_end_matches('0');
    // The value is `trimmed * 10^scale`, with no leading or trailing zeros.
    let fraction_len = i64::try_from(fraction.len()).unwrap_or(i64::MAX / 4);
    let trailing = i64::try_from(significant.len() - trimmed.len()).unwrap_or(0);
    let scale = exponent
        .saturating_sub(fraction_len)
        .saturating_add(trailing);
    let trimmed_len = i64::try_from(trimmed.len()).unwrap_or(i64::MAX / 4);
    let integer_digits = trimmed_len.saturating_add(scale);
    let max_len = i64::try_from(max.len()).unwrap_or(i64::MAX / 4);
    match integer_digits.cmp(&max_len) {
        std::cmp::Ordering::Less => true,
        std::cmp::Ordering::Greater => false,
        std::cmp::Ordering::Equal => {
            let head = trimmed
                .chars()
                .chain(std::iter::repeat('0'))
                .take(max.len())
                .collect::<String>();
            let has_fraction = trimmed.len() > max.len();
            match head.as_str().cmp(max) {
                std::cmp::Ordering::Less => true,
                std::cmp::Ordering::Equal => !has_fraction,
                std::cmp::Ordering::Greater => false,
            }
        }
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
    /// Called by `provider_call` on the client it was given, after the
    /// `provider_turn_duration` of the call made by this client's latest clone
    /// carried that call's result. Clients that do not care ignore it.
    fn provider_turn_carried_result(&mut self) {}
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
