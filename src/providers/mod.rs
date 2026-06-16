pub mod planner;

use crate::config::Provider;

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
        Provider::Gemini | Provider::OpenAi => ProviderCapabilities {
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
    fn api_providers_use_xml_fallback_by_default() {
        for provider in [Provider::Gemini, Provider::OpenAi] {
            let caps = capabilities(provider);
            assert!(!caps.native_tool_calls);
            assert_eq!(caps.default_tool_call_mode, ToolCallMode::XmlFallback);
        }
    }
}
