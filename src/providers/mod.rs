pub mod gemini;
pub mod ollama;
pub mod openai;
pub mod planner;
pub mod usage;
pub mod xml_fallback;

use crate::config::Provider;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCallMode {
    Native,
    XmlFallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderCapabilities {
    pub native_tool_calls: bool,
    pub default_tool_call_mode: ToolCallMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
    pub tool_call_id: Option<String>,
    pub tool_name: Option<String>,
    pub tool_calls: Vec<ToolCall>,
}

impl ChatMessage {
    pub fn new(role: ChatRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            tool_call_id: None,
            tool_name: None,
            tool_calls: Vec::new(),
        }
    }

    pub fn assistant_with_tool_calls(
        content: impl Into<String>,
        tool_calls: Vec<ToolCall>,
    ) -> Self {
        Self {
            role: ChatRole::Assistant,
            content: content.into(),
            tool_call_id: None,
            tool_name: None,
            tool_calls,
        }
    }

    pub fn tool_result(
        content: impl Into<String>,
        tool_name: impl Into<String>,
        tool_call_id: Option<String>,
    ) -> Self {
        Self {
            role: ChatRole::Tool,
            content: content.into(),
            tool_call_id,
            tool_name: Some(tool_name.into()),
            tool_calls: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCall {
    pub id: Option<String>,
    pub thought_signature: Option<String>,
    pub name: String,
    pub args_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolSpec>,
    pub tool_call_mode: ToolCallMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatResponse {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters_json_schema: Value,
}

const TOOL_CALL_PARSE_ERROR_PREFIX: &str = "commandagent_tool_call_parse_error:";

pub fn tool_call_parse_error_content(message: impl AsRef<str>) -> String {
    format!(
        "{} {}",
        TOOL_CALL_PARSE_ERROR_PREFIX,
        message.as_ref().trim()
    )
}

pub fn tool_call_parse_error_from_content(content: &str) -> Option<String> {
    content
        .trim()
        .strip_prefix(TOOL_CALL_PARSE_ERROR_PREFIX)
        .map(str::trim)
        .filter(|message| !message.is_empty())
        .map(ToString::to_string)
}

pub trait ChatProvider {
    fn provider(&self) -> Provider;

    fn capabilities(&self) -> ProviderCapabilities {
        capabilities(self.provider())
    }
}

pub trait ExecutorProvider: ChatProvider {}

pub trait PlannerProvider: ChatProvider {}

pub fn capabilities(provider: Provider) -> ProviderCapabilities {
    match provider {
        Provider::Ollama => ProviderCapabilities {
            native_tool_calls: true,
            default_tool_call_mode: ToolCallMode::Native,
        },
        Provider::Gemini => ProviderCapabilities {
            native_tool_calls: true,
            default_tool_call_mode: ToolCallMode::Native,
        },
        Provider::OpenAi => ProviderCapabilities {
            native_tool_calls: false,
            default_tool_call_mode: ToolCallMode::XmlFallback,
        },
    }
}

pub fn request_tool_mode(provider: Provider) -> ToolCallMode {
    capabilities(provider).default_tool_call_mode
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ollama_supports_native_tools() {
        let caps = capabilities(Provider::Ollama);
        assert!(caps.native_tool_calls);
        assert_eq!(caps.default_tool_call_mode, ToolCallMode::Native);
    }

    #[test]
    fn gemini_supports_native_tools() {
        let caps = capabilities(Provider::Gemini);
        assert!(caps.native_tool_calls);
        assert_eq!(caps.default_tool_call_mode, ToolCallMode::Native);
    }

    #[test]
    fn openai_uses_xml_fallback_by_default() {
        let caps = capabilities(Provider::OpenAi);
        assert!(!caps.native_tool_calls);
        assert_eq!(caps.default_tool_call_mode, ToolCallMode::XmlFallback);
    }
}
