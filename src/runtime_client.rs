use crate::agent::minimal_loop::loop_run::ChatClient;
use crate::config::{Config, Provider};
use crate::providers::gemini::{DEFAULT_GEMINI_BASE_URL, GeminiClient};
use crate::providers::ollama::OllamaClient;
use crate::providers::openai::{DEFAULT_OPENAI_BASE_URL, OpenAiClient};
use crate::providers::{ChatMessage, ChatRequest, ChatResponse, ChatRole, ToolCall, ToolCallMode};
use serde_json::{Value, json};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

pub(crate) fn runtime_client(config: &Config) -> Result<RuntimeClient, String> {
    runtime_client_for(config, config.provider)
}

pub(crate) fn runtime_client_for(
    config: &Config,
    provider: Provider,
) -> Result<RuntimeClient, String> {
    let timeout = Duration::from_secs(config.timeout_secs);
    match provider {
        Provider::Ollama => {
            let base_url = std::env::var("OLLAMA_HOST")
                .unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());
            Ok(RuntimeClient::Ollama(
                OllamaClient::with_options(base_url, timeout, config.retries)
                    .map_err(|err| err.to_string())?,
            ))
        }
        Provider::Gemini => {
            let key = config
                .gemini_api_key
                .clone()
                .ok_or_else(|| "GEMINI_API_KEY is required for --provider gemini".to_string())?;
            Ok(RuntimeClient::Gemini(
                GeminiClient::with_options(DEFAULT_GEMINI_BASE_URL, key, timeout, config.retries)
                    .map_err(|err| err.to_string())?,
            ))
        }
        Provider::OpenAi => {
            let key = config
                .openai_api_key
                .clone()
                .ok_or_else(|| "OPENAI_API_KEY is required for --provider openai".to_string())?;
            Ok(RuntimeClient::OpenAi(
                OpenAiClient::with_options(DEFAULT_OPENAI_BASE_URL, key, timeout, config.retries)
                    .map_err(|err| err.to_string())?,
            ))
        }
    }
}

pub(crate) enum RuntimeClient {
    Ollama(OllamaClient),
    Gemini(GeminiClient),
    OpenAi(OpenAiClient),
}

impl ChatClient for RuntimeClient {
    fn chat(&mut self, request: &ChatRequest) -> Result<ChatResponse, String> {
        match self {
            Self::Ollama(client) => client.chat(request).map_err(|err| err.to_string()),
            Self::Gemini(client) => client.chat(request).map_err(|err| err.to_string()),
            Self::OpenAi(client) => client.chat(request).map_err(|err| err.to_string()),
        }
    }
}

pub(crate) struct ModelIoTracingClient<C> {
    inner: C,
    role: String,
    path: Option<PathBuf>,
    sequence: u64,
}

impl<C> ModelIoTracingClient<C> {
    pub(crate) fn from_env(inner: C, role: impl Into<String>) -> Self {
        Self {
            inner,
            role: role.into(),
            path: std::env::var_os("COMMANDAGENT_MODEL_IO_JSONL").map(PathBuf::from),
            sequence: 0,
        }
    }

    fn write_trace(&mut self, event_type: &str, payload: Value) {
        let Some(path) = self.path.as_ref() else {
            return;
        };
        if let Some(parent) = path.parent() {
            if std::fs::create_dir_all(parent).is_err() {
                return;
            }
        }
        self.sequence += 1;
        let value = json!({
            "schema_version": "1.0",
            "sequence": self.sequence,
            "role": self.role,
            "event_type": event_type,
            "payload": payload,
        });
        let Ok(line) = serde_json::to_string(&value) else {
            return;
        };
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(file, "{line}");
        }
    }
}

impl<C> ChatClient for ModelIoTracingClient<C>
where
    C: ChatClient,
{
    fn chat(&mut self, request: &ChatRequest) -> Result<ChatResponse, String> {
        self.write_trace("model.request", chat_request_payload(request));
        match self.inner.chat(request) {
            Ok(response) => {
                self.write_trace("model.response", chat_response_payload(&response));
                Ok(response)
            }
            Err(err) => {
                self.write_trace("model.error", json!({"message": err}));
                Err(err)
            }
        }
    }
}

fn chat_request_payload(request: &ChatRequest) -> Value {
    json!({
        "model": request.model,
        "tool_call_mode": tool_call_mode_text(request.tool_call_mode),
        "messages": request.messages.iter().map(chat_message_payload).collect::<Vec<_>>(),
        "tools": request.tools.iter().map(|tool| {
            json!({
                "name": tool.name,
                "description": tool.description,
                "parameters_json_schema": tool.parameters_json_schema,
            })
        }).collect::<Vec<_>>(),
    })
}

fn chat_response_payload(response: &ChatResponse) -> Value {
    json!({
        "content": response.content,
        "tool_calls": response.tool_calls.iter().map(tool_call_payload).collect::<Vec<_>>(),
        "usage": response.usage,
    })
}

fn chat_message_payload(message: &ChatMessage) -> Value {
    json!({
        "role": chat_role_text(message.role),
        "content": message.content,
        "tool_call_id": message.tool_call_id,
        "tool_name": message.tool_name,
        "tool_calls": message.tool_calls.iter().map(tool_call_payload).collect::<Vec<_>>(),
    })
}

fn tool_call_payload(call: &ToolCall) -> Value {
    json!({
        "id": call.id,
        "thought_signature": call.thought_signature,
        "name": call.name,
        "args_json": call.args_json,
    })
}

fn chat_role_text(role: ChatRole) -> &'static str {
    match role {
        ChatRole::System => "system",
        ChatRole::User => "user",
        ChatRole::Assistant => "assistant",
        ChatRole::Tool => "tool",
    }
}

fn tool_call_mode_text(mode: ToolCallMode) -> &'static str {
    match mode {
        ToolCallMode::Native => "native",
        ToolCallMode::XmlFallback => "xml_fallback",
    }
}
