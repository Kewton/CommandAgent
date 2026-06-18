use crate::agent::minimal_loop::loop_run::ChatClient;
use crate::config::{Config, Provider};
use crate::providers::gemini::{DEFAULT_GEMINI_BASE_URL, GeminiClient};
use crate::providers::ollama::OllamaClient;
use crate::providers::openai::{DEFAULT_OPENAI_BASE_URL, OpenAiClient};
use crate::providers::{ChatRequest, ChatResponse};
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
