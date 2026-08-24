use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use reqwest::blocking::Client;
use serde_json::{Value, json};

use crate::config::{Config, Provider, load_api_key};
use crate::eval_events;
use crate::state::{ConversationMessage, ToolCall};
use crate::tools::args_recovery::recover_tool_arguments;
use crate::tools::registry::ToolSpec;

use super::parsing::sanitize_schema;
use super::streaming::StreamControl;
use super::{AssistantReply, ChatClient};

const GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com";

#[derive(Debug, Clone)]
pub struct GeminiClient {
    api_key: String,
    http: Client,
    max_predict: usize,
    retries: usize,
    eval_events_path: Option<PathBuf>,
    previous_interaction_id: Arc<Mutex<Option<String>>>,
}

impl GeminiClient {
    pub fn from_env(config: &Config) -> anyhow::Result<Self> {
        let api_key = load_api_key(&config.workspace_root, "GEMINI_API_KEY")?;
        let http = Client::builder()
            .connect_timeout(std::time::Duration::from_secs(config.chat_timeout_secs))
            .timeout(std::time::Duration::from_secs(config.chat_timeout_secs))
            .build()?;
        Ok(Self {
            api_key,
            http,
            max_predict: config.num_predict,
            retries: config.chat_retries,
            eval_events_path: config.eval_events_path.clone(),
            previous_interaction_id: Arc::new(Mutex::new(None)),
        })
    }
}

impl ChatClient for GeminiClient {
    fn label(&self) -> &str {
        "gemini"
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn supports_native_tools(&self, _model: &str) -> bool {
        true
    }

    fn allows_xml_fallback(&self) -> bool {
        false
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    fn chat_stream(
        &mut self,
        model: &str,
        messages: &[ConversationMessage],
        tools: &[ToolSpec],
        _native_tools_enabled: bool,
        on_chunk: &mut dyn FnMut(&str) -> anyhow::Result<()>,
    ) -> anyhow::Result<AssistantReply> {
        let body = build_stream_generate_content_request(messages, tools, self.max_predict);
        let normalized_model = model.strip_prefix("models/").unwrap_or(model);
        let url = format!(
            "{GEMINI_BASE_URL}/v1beta/models/{normalized_model}:streamGenerateContent?alt=sse"
        );
        eval_events::emit(
            self.eval_events_path.as_deref(),
            json!({
                "event": "provider_request",
                "provider": "gemini",
                "model": model,
                "tools": tools.len(),
                "previous_interaction": false,
            }),
        );
        for attempt in 0..=self.retries {
            match self
                .http
                .post(&url)
                .header("x-goog-api-key", &self.api_key)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
            {
                Ok(response) if response.status().is_success() => {
                    let mut delivered = false;
                    let parsed = parse_gemini_stream(response, &mut |chunk| {
                        delivered = true;
                        on_chunk(chunk)
                    });
                    match parsed {
                        Ok(reply) => {
                            eval_events::emit(
                                self.eval_events_path.as_deref(),
                                json!({
                                    "event": "provider_response",
                                    "provider": "gemini",
                                    "model": model,
                                    "attempt": attempt + 1,
                                    "tool_calls": reply.tool_calls.len(),
                                }),
                            );
                            return Ok(reply);
                        }
                        Err(err)
                            if !super::streaming::retry_allowed(
                                attempt,
                                self.retries,
                                delivered,
                            ) =>
                        {
                            eval_events::emit(
                                self.eval_events_path.as_deref(),
                                json!({
                                    "event": "provider_parse_error",
                                    "provider": "gemini",
                                    "model": model,
                                    "error_kind": "provider_stream_error",
                                    "message": eval_events::body_snippet(&err.to_string()),
                                }),
                            );
                            return Err(if delivered {
                                super::streaming::after_first_chunk(err)
                            } else {
                                err
                            });
                        }
                        Err(err) => emit_stream_retry(
                            self.eval_events_path.as_deref(),
                            model,
                            attempt,
                            &err,
                        ),
                    }
                }
                Ok(response)
                    if attempt == self.retries
                        || !is_retryable_status(response.status().as_u16()) =>
                {
                    let status = response.status();
                    let response_body = response.text().unwrap_or_default();
                    eval_events::emit(
                        self.eval_events_path.as_deref(),
                        json!({
                            "event": "provider_error",
                            "provider": "gemini",
                            "model": model,
                            "status": status.as_u16(),
                            "error_kind": "http_status",
                            "attempt": attempt + 1,
                            "retry_exhausted": attempt == self.retries,
                            "retryable": is_retryable_status(status.as_u16()),
                            "body_snippet": eval_events::body_snippet(&response_body),
                        }),
                    );
                    return Err(super::guidance::http_status_error(
                        Provider::Gemini,
                        model,
                        status,
                    ));
                }
                Ok(response) => {
                    let status = response.status();
                    eval_events::emit(
                        self.eval_events_path.as_deref(),
                        json!({
                            "event": "provider_retry",
                            "provider": "gemini",
                            "model": model,
                            "status": status.as_u16(),
                            "error_kind": "http_status",
                            "attempt": attempt + 1,
                            "retryable": true,
                        }),
                    );
                }
                Err(err) if attempt == self.retries => {
                    eval_events::emit(
                        self.eval_events_path.as_deref(),
                        json!({
                            "event": "provider_error",
                            "provider": "gemini",
                            "model": model,
                            "error_kind": "network",
                            "attempt": attempt + 1,
                            "retry_exhausted": true,
                            "message": eval_events::body_snippet(&err.to_string()),
                        }),
                    );
                    return Err(super::guidance::connection_error(
                        Provider::Gemini,
                        GEMINI_BASE_URL,
                        err,
                    ));
                }
                Err(err) => {
                    emit_stream_retry(self.eval_events_path.as_deref(), model, attempt, &err)
                }
            }
        }
        unreachable!("retry loop always returns or bails")
    }

    fn chat(
        &mut self,
        model: &str,
        messages: &[ConversationMessage],
        tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        let continues_existing_interaction = messages
            .iter()
            .any(|message| matches!(message.role.as_str(), "assistant" | "tool"));
        let previous_interaction_id = if continues_existing_interaction {
            self.previous_interaction_id
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone()
        } else {
            None
        };
        let body = super::gemini_function_calling::build_interactions_request_with_previous(
            model,
            messages,
            tools,
            self.max_predict,
            previous_interaction_id.as_deref(),
        );
        eval_events::emit(
            self.eval_events_path.as_deref(),
            json!({
                "event": "provider_request",
                "provider": "gemini",
                "model": model,
                "tools": tools.len(),
                "previous_interaction": previous_interaction_id.is_some(),
            }),
        );
        for attempt in 0..=self.retries {
            match self
                .http
                .post(format!("{GEMINI_BASE_URL}/v1beta/interactions"))
                .header("x-goog-api-key", &self.api_key)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
            {
                Ok(response) if response.status().is_success() => {
                    let body = response.text()?;
                    let parsed = super::gemini_function_calling::parse_interactions_response(&body);
                    match parsed {
                        Ok(reply) => {
                            if let Some(id) = super::gemini_function_calling::interaction_id(&body)
                            {
                                *self
                                    .previous_interaction_id
                                    .lock()
                                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(id);
                            }
                            eval_events::emit(
                                self.eval_events_path.as_deref(),
                                json!({
                                    "event": "provider_response",
                                    "provider": "gemini",
                                    "model": model,
                                    "attempt": attempt + 1,
                                    "tool_calls": reply.tool_calls.len(),
                                }),
                            );
                            return Ok(reply);
                        }
                        Err(err) => {
                            eval_events::emit(
                                self.eval_events_path.as_deref(),
                                json!({
                                    "event": "provider_parse_error",
                                    "provider": "gemini",
                                    "model": model,
                                    "error_kind": "provider_parse_error",
                                    "message": eval_events::body_snippet(&err.to_string()),
                                }),
                            );
                            return Err(err);
                        }
                    }
                }
                Ok(response)
                    if attempt == self.retries
                        || !is_retryable_status(response.status().as_u16()) =>
                {
                    let status = response.status();
                    let body = response.text().unwrap_or_default();
                    eval_events::emit(
                        self.eval_events_path.as_deref(),
                        json!({
                            "event": "provider_error",
                            "provider": "gemini",
                            "model": model,
                            "status": status.as_u16(),
                            "error_kind": "http_status",
                            "attempt": attempt + 1,
                            "retry_exhausted": attempt == self.retries,
                            "retryable": is_retryable_status(status.as_u16()),
                            "body_snippet": eval_events::body_snippet(&body),
                        }),
                    );
                    return Err(super::guidance::http_status_error(
                        Provider::Gemini,
                        model,
                        status,
                    ));
                }
                Ok(response) => {
                    let status = response.status();
                    eval_events::emit(
                        self.eval_events_path.as_deref(),
                        json!({
                            "event": "provider_retry",
                            "provider": "gemini",
                            "model": model,
                            "status": status.as_u16(),
                            "error_kind": "http_status",
                            "attempt": attempt + 1,
                            "retryable": is_retryable_status(status.as_u16()),
                        }),
                    );
                }
                Err(err) if attempt == self.retries => {
                    eval_events::emit(
                        self.eval_events_path.as_deref(),
                        json!({
                            "event": "provider_error",
                            "provider": "gemini",
                            "model": model,
                            "error_kind": "network",
                            "attempt": attempt + 1,
                            "retry_exhausted": true,
                            "message": eval_events::body_snippet(&err.to_string()),
                        }),
                    );
                    return Err(super::guidance::connection_error(
                        Provider::Gemini,
                        GEMINI_BASE_URL,
                        err,
                    ));
                }
                Err(err) => {
                    eval_events::emit(
                        self.eval_events_path.as_deref(),
                        json!({
                            "event": "provider_retry",
                            "provider": "gemini",
                            "model": model,
                            "error_kind": "network",
                            "attempt": attempt + 1,
                            "message": eval_events::body_snippet(&err.to_string()),
                        }),
                    );
                }
            }
        }
        unreachable!("retry loop always returns or bails")
    }
}

fn is_retryable_status(status: u16) -> bool {
    matches!(status, 429 | 500 | 502 | 503 | 504)
}

fn emit_stream_retry(
    events_path: Option<&std::path::Path>,
    model: &str,
    attempt: usize,
    err: &dyn std::fmt::Display,
) {
    eval_events::emit(
        events_path,
        json!({
            "event": "provider_retry",
            "provider": "gemini",
            "model": model,
            "error_kind": "stream_before_first_token",
            "attempt": attempt + 1,
            "message": eval_events::body_snippet(&err.to_string()),
        }),
    );
}

pub fn build_stream_generate_content_request(
    messages: &[ConversationMessage],
    tools: &[ToolSpec],
    max_predict: usize,
) -> Value {
    let system_text = messages
        .iter()
        .filter(|message| matches!(message.role.as_str(), "system" | "developer"))
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    let contents = messages
        .iter()
        .filter(|message| !matches!(message.role.as_str(), "system" | "developer"))
        .map(gemini_stream_content)
        .collect::<Vec<_>>();
    let mut body = json!({
        "contents": contents,
        "generationConfig": {"maxOutputTokens": max_predict},
    });
    if !system_text.is_empty() {
        body["systemInstruction"] = json!({"parts": [{"text": system_text}]});
    }
    if !tools.is_empty() {
        body["tools"] = json!([{
            "functionDeclarations": tools.iter().map(|tool| json!({
                "name": tool.function.name,
                "description": tool.function.description,
                "parameters": sanitize_schema(&tool.function.parameters),
            })).collect::<Vec<_>>()
        }]);
    }
    body
}

fn gemini_stream_content(message: &ConversationMessage) -> Value {
    if message.role == "assistant" {
        let mut parts = Vec::new();
        if !message.content.is_empty() {
            parts.push(json!({"text": message.content}));
        }
        parts.extend(
            message
                .tool_calls
                .iter()
                .map(|call| json!({"functionCall": {"name": call.name, "args": call.arguments}})),
        );
        return json!({"role": "model", "parts": parts});
    }
    if message.role == "tool" {
        return json!({
            "role": "user",
            "parts": [{
                "functionResponse": {
                    "name": message.name.as_deref().unwrap_or("tool"),
                    "response": {"result": message.content},
                }
            }]
        });
    }
    json!({"role": "user", "parts": [{"text": message.content}]})
}

pub fn parse_gemini_stream<R: std::io::Read>(
    reader: R,
    on_chunk: &mut dyn FnMut(&str) -> anyhow::Result<()>,
) -> anyhow::Result<AssistantReply> {
    let mut content = String::new();
    let mut tool_calls = Vec::new();
    let mut prompt_tokens = None;
    let mut completion_tokens = None;
    let mut saw_event = false;
    let mut finished = false;
    super::streaming::read_sse(reader, |data| {
        saw_event = true;
        let event: Value = serde_json::from_str(data)
            .map_err(|err| anyhow::anyhow!("malformed Gemini SSE data: {err}"))?;
        if let Some(error) = event.get("error") {
            anyhow::bail!("Gemini stream failed: {error}");
        }
        if let Some(candidates) = event.get("candidates").and_then(Value::as_array) {
            for candidate in candidates {
                if candidate
                    .get("finishReason")
                    .and_then(Value::as_str)
                    .is_some()
                {
                    finished = true;
                }
                if let Some(parts) = candidate
                    .get("content")
                    .and_then(|content| content.get("parts"))
                    .and_then(Value::as_array)
                {
                    for part in parts {
                        if part.get("thought").and_then(Value::as_bool) == Some(true) {
                            continue;
                        }
                        if let Some(text) = part.get("text").and_then(Value::as_str)
                            && !text.is_empty()
                        {
                            on_chunk(text)?;
                            content.push_str(text);
                        }
                        if let Some(call) = part.get("functionCall") {
                            let name =
                                call.get("name").and_then(Value::as_str).ok_or_else(|| {
                                    anyhow::anyhow!("Gemini functionCall missing name")
                                })?;
                            let arguments = call.get("args").cloned().unwrap_or_else(|| json!({}));
                            let arguments = recover_tool_arguments(name, arguments).arguments;
                            tool_calls.push(ToolCall {
                                id: call
                                    .get("id")
                                    .and_then(Value::as_str)
                                    .map(str::to_string)
                                    .unwrap_or_else(|| uuid::Uuid::now_v7().to_string()),
                                name: name.to_string(),
                                arguments,
                            });
                        }
                    }
                }
            }
        }
        if let Some(usage) = event.get("usageMetadata") {
            prompt_tokens = usage.get("promptTokenCount").and_then(Value::as_u64);
            completion_tokens = usage.get("candidatesTokenCount").and_then(Value::as_u64);
        }
        Ok(StreamControl::Continue)
    })?;
    if !saw_event {
        anyhow::bail!("Gemini stream ended without an event");
    }
    if !finished {
        anyhow::bail!("Gemini stream ended before a finish reason");
    }
    Ok(AssistantReply {
        content,
        tool_calls,
        prompt_tokens,
        completion_tokens,
    })
}

#[cfg(test)]
mod streaming_tests {
    use super::*;

    #[test]
    fn stream_request_maps_history_tools_and_budget() {
        let tools = crate::tools::registry::ToolRegistry::default();
        let body = build_stream_generate_content_request(
            &[
                ConversationMessage::system("rules"),
                ConversationMessage::user("hello"),
            ],
            tools.specs(),
            42,
        );
        assert_eq!(body["systemInstruction"]["parts"][0]["text"], "rules");
        assert_eq!(body["contents"][0]["parts"][0]["text"], "hello");
        assert_eq!(body["generationConfig"]["maxOutputTokens"], 42);
        assert!(body["tools"][0]["functionDeclarations"].is_array());
    }

    #[test]
    fn parses_generate_content_sse_text_function_and_usage() {
        let stream = concat!(
            "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"日\"}]}}]}\n\n",
            "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"🙂\"},{\"functionCall\":{\"name\":\"Read\",\"args\":{\"path\":\"a\"}}}]},\"finishReason\":\"STOP\"}],\"usageMetadata\":{\"promptTokenCount\":5,\"candidatesTokenCount\":2}}\n\n"
        );
        let mut chunks = Vec::new();
        let reply = parse_gemini_stream(std::io::Cursor::new(stream), &mut |chunk| {
            chunks.push(chunk.to_string());
            Ok(())
        })
        .unwrap();
        assert_eq!(chunks.concat(), "日🙂");
        assert_eq!(reply.content, "日🙂");
        assert_eq!(reply.tool_calls[0].name, "Read");
        assert_eq!(reply.tool_calls[0].arguments["path"], "a");
        assert_eq!(reply.prompt_tokens, Some(5));
        assert_eq!(reply.completion_tokens, Some(2));
    }

    #[test]
    fn truncated_generate_content_stream_keeps_partial_and_errors() {
        let stream =
            "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"partial\"}]}}]}\n\n";
        let mut chunks = Vec::new();
        let err = parse_gemini_stream(std::io::Cursor::new(stream), &mut |chunk| {
            chunks.push(chunk.to_string());
            Ok(())
        })
        .unwrap_err()
        .to_string();
        assert_eq!(chunks.concat(), "partial");
        assert!(err.contains("finish reason"), "{err}");
    }
}
