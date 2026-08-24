pub mod gemini;
pub mod gemini_function_calling;
pub(crate) mod guidance;
pub mod lm_studio;
pub mod ollama;
pub mod openai;
mod openai_chat_completions;
pub mod openai_compatible;
mod openai_responses;
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
    fn supports_ollama_think(&self) -> bool {
        false
    }
    fn take_response_timing(&mut self) -> Option<ResponseTiming> {
        None
    }
    fn take_response_metadata(&mut self) -> Option<ProviderResponseMetadata> {
        None
    }
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
        Provider::LmStudio => Ok(Box::new(lm_studio::LmStudioClient::from_env(config)?)),
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
