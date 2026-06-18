use crate::providers::gemini::{GeminiClient, GeminiTransport};
use crate::providers::ollama::{OllamaClient, OllamaTransport};
use crate::providers::openai::{OpenAiClient, OpenAiTransport};
use crate::providers::{ChatRequest, ChatResponse};

pub trait ChatClient {
    fn chat(&mut self, request: &ChatRequest) -> Result<ChatResponse, String>;
}

impl<T> ChatClient for OllamaClient<T>
where
    T: OllamaTransport,
{
    fn chat(&mut self, request: &ChatRequest) -> Result<ChatResponse, String> {
        OllamaClient::chat(self, request).map_err(|err| err.to_string())
    }
}

impl<T> ChatClient for GeminiClient<T>
where
    T: GeminiTransport,
{
    fn chat(&mut self, request: &ChatRequest) -> Result<ChatResponse, String> {
        GeminiClient::chat(self, request).map_err(|err| err.to_string())
    }
}

impl<T> ChatClient for OpenAiClient<T>
where
    T: OpenAiTransport,
{
    fn chat(&mut self, request: &ChatRequest) -> Result<ChatResponse, String> {
        OpenAiClient::chat(self, request).map_err(|err| err.to_string())
    }
}
