use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use reqwest::Client;
use serde_json::{Value, json};

use crate::config::{Config, Provider, load_api_key};
use crate::eval_events;
use crate::state::{ConversationMessage, ToolCall};
use crate::tools::args_recovery::recover_tool_arguments;
use crate::tools::registry::ToolSpec;

use super::parsing::sanitize_schema;
use super::streaming::StreamControl;
use super::streaming::{CancelToken, HttpCall};
use super::{AssistantReply, ChatClient};

const GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com";

#[derive(Clone)]
pub struct GeminiClient {
    api_key: String,
    http: Client,
    cancel: CancelToken,
    base_url: String,
    max_predict: usize,
    retries: usize,
    eval_events_path: Option<PathBuf>,
    previous_interaction_id: Arc<Mutex<Option<String>>>,
}

impl std::fmt::Debug for GeminiClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GeminiClient")
            .field("api_key", &"<redacted>")
            .field("http", &self.http)
            .field("max_predict", &self.max_predict)
            .field("retries", &self.retries)
            .field("eval_events_path", &self.eval_events_path)
            .field("previous_interaction_id", &self.previous_interaction_id)
            .finish()
    }
}

impl GeminiClient {
    pub fn from_env(config: &Config) -> anyhow::Result<Self> {
        let api_key = load_api_key(&config.workspace_root, "GEMINI_API_KEY")?;
        let http = super::streaming::http_client(config.chat_timeout_secs)?;
        Ok(Self {
            api_key,
            http,
            cancel: CancelToken::default(),
            base_url: GEMINI_BASE_URL.to_string(),
            max_predict: config.num_predict,
            retries: config.chat_retries,
            eval_events_path: config.eval_events_path.clone(),
            previous_interaction_id: Arc::new(Mutex::new(None)),
        })
    }

    #[cfg(test)]
    fn for_test(base_url: String) -> Self {
        Self {
            api_key: "gemini-test-key".to_string(),
            http: super::streaming::http_client(30).expect("test Gemini client"),
            cancel: CancelToken::default(),
            base_url,
            max_predict: 64,
            retries: 0,
            eval_events_path: None,
            previous_interaction_id: Arc::new(Mutex::new(None)),
        }
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

    fn set_cancel_token(&mut self, token: CancelToken) {
        self.cancel = token;
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
            "{}/v1beta/models/{normalized_model}:streamGenerateContent?alt=sse",
            self.base_url
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
        let call = HttpCall::new(&self.cancel)?;
        for attempt in 0..=self.retries {
            match call.run(async {
                self.http
                    .post(&url)
                    .header("x-goog-api-key", &self.api_key)
                    .header("Content-Type", "application/json")
                    .json(&body)
                    .send()
                    .await
            })? {
                Ok(response) if response.status().is_success() => {
                    let mut delivered = false;
                    let parsed = parse_gemini_stream(call.reader(response), &mut |chunk| {
                        delivered = true;
                        on_chunk(chunk)
                    });
                    match parsed {
                        // A cancel observed before commit wins even when the
                        // buffered stream parsed cleanly.
                        _ if call.is_cancelled() => {
                            return Err(super::streaming::aborted_by_user());
                        }
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
                    let response_body = call.run(response.text())?.unwrap_or_default();
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
        let call = HttpCall::new(&self.cancel)?;
        for attempt in 0..=self.retries {
            match call.run(async {
                self.http
                    .post(format!("{}/v1beta/interactions", self.base_url))
                    .header("x-goog-api-key", &self.api_key)
                    .header("Content-Type", "application/json")
                    .json(&body)
                    .send()
                    .await
            })? {
                Ok(response) if response.status().is_success() => {
                    let body = call.run(response.text())??;
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
                    let body = call.run(response.text())?.unwrap_or_default();
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

#[cfg(test)]
mod cancel_tests {
    use super::*;
    use crate::providers::streaming::cancel_test_support::assert_cancel_closes_connection;

    #[test]
    fn cancel_closes_the_interactions_request() {
        assert_cancel_closes_connection(None, |base_url, token| {
            let mut client = GeminiClient::for_test(base_url);
            client.set_cancel_token(token);
            ChatClient::chat(
                &mut client,
                "gemini-test",
                &[ConversationMessage::user("hi")],
                &[],
                false,
            )
        });
    }

    #[test]
    fn cancel_closes_the_stream_request() {
        assert_cancel_closes_connection(
            Some("data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"partial\"}]}}]}\n\n"),
            |base_url, token| {
                let mut client = GeminiClient::for_test(base_url);
                client.set_cancel_token(token);
                client.chat_stream(
                    "gemini-test",
                    &[ConversationMessage::user("hi")],
                    &[],
                    false,
                    &mut |_chunk| Ok(()),
                )
            },
        );
    }

    #[test]
    fn cancel_mid_stream_records_no_parse_error_or_retry() {
        let dir = tempfile::tempdir().expect("tempdir");
        let events = dir.path().join("events.jsonl");
        let events_for_client = events.clone();
        assert_cancel_closes_connection(
            Some("data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"partial\"}]}}]}\n\n"),
            move |base_url, token| {
                let mut client = GeminiClient::for_test(base_url);
                client.retries = 2;
                client.eval_events_path = Some(events_for_client);
                client.set_cancel_token(token);
                client.chat_stream(
                    "gemini-test",
                    &[ConversationMessage::user("hi")],
                    &[],
                    false,
                    &mut |_chunk| Ok(()),
                )
            },
        );
        let recorded = std::fs::read_to_string(&events).unwrap_or_default();
        for forbidden in [
            "provider_parse_error",
            "provider_retry",
            "provider_response",
        ] {
            assert!(
                !recorded.contains(forbidden),
                "{forbidden} recorded: {recorded}"
            );
        }
    }

    fn serve_complete_stream(stream_body: &'static str) -> String {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listener");
        let base_url = format!("http://{}", listener.local_addr().expect("address"));
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .expect("read timeout");
            let mut data = Vec::new();
            let mut buffer = [0_u8; 4096];
            loop {
                let read = stream.read(&mut buffer).expect("request");
                if read == 0 {
                    return;
                }
                data.extend_from_slice(&buffer[..read]);
                let Some(end) = data.windows(4).position(|window| window == b"\r\n\r\n") else {
                    continue;
                };
                let head = String::from_utf8_lossy(&data[..end]).to_ascii_lowercase();
                let length = head
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length:"))
                    .and_then(|value| value.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                if data.len() >= end + 4 + length {
                    break;
                }
            }
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n{stream_body}"
            )
            .expect("stream");
            stream.flush().expect("flush");
        });
        base_url
    }

    fn event_names(events: &std::path::Path) -> Vec<String> {
        std::fs::read_to_string(events)
            .unwrap_or_default()
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .filter_map(|event| {
                event
                    .get("event")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .collect()
    }

    /// Run the corpus case: the callback receives the complete final chunk and
    /// optionally cancels the token, so the parser still returns `Ok`.
    fn run_complete_buffer_case(case: &str) {
        let cases: Value = serde_json::from_str(include_str!(
            "../../tests/corpus/apps/issue697-gemini-cancel-terminal/fixtures/cases.json"
        ))
        .expect("cases.json");
        let stream_body: &'static str = Box::leak(
            cases["stream"]
                .as_str()
                .expect("stream")
                .to_string()
                .into_boxed_str(),
        );
        let expected = &cases[case];
        let cancels = expected["callback_cancels_token"]
            .as_bool()
            .expect("cancels");
        let dir = tempfile::tempdir().expect("tempdir");
        let events = dir.path().join("events.jsonl");
        let mut client = GeminiClient::for_test(serve_complete_stream(stream_body));
        client.retries = 2;
        client.eval_events_path = Some(events.clone());
        let token = CancelToken::default();
        client.set_cancel_token(token.clone());
        let mut chunks = Vec::new();
        let result = client.chat_stream(
            "gemini-test",
            &[ConversationMessage::user("hi")],
            &[],
            false,
            &mut |chunk| {
                chunks.push(chunk.to_string());
                if cancels {
                    token.cancel();
                }
                Ok(())
            },
        );
        assert_eq!(chunks, ["done"], "the complete chunk reaches the callback");
        match expected["outcome"].as_str().expect("outcome") {
            "aborted_by_user" => {
                let err = result.expect_err("a cancelled call must not return a reply");
                assert!(err.to_string().contains("aborted_by_user"), "{err}");
            }
            "ok" => assert_eq!(result.expect("reply").content, "done"),
            other => panic!("unknown outcome {other}"),
        }
        let names = event_names(&events);
        for event in [
            "provider_response",
            "provider_parse_error",
            "provider_retry",
        ] {
            let count = names.iter().filter(|name| *name == event).count() as u64;
            assert_eq!(
                count,
                expected[event].as_u64().expect("count"),
                "{case}: {event} in {names:?}"
            );
        }
    }

    #[test]
    fn cancel_after_complete_buffer_records_no_terminal_response() {
        run_complete_buffer_case("cancelled_after_complete_buffer");
    }

    #[test]
    fn complete_buffer_without_cancel_records_one_provider_response() {
        run_complete_buffer_case("uncancelled_control");
    }
}
