use std::collections::BTreeMap;
use std::path::PathBuf;

use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::config::{Config, Provider, load_api_key};
use crate::eval_events;
use crate::state::{ConversationMessage, ToolCall};
use crate::tools::args_recovery::recover_tool_arguments;
use crate::tools::registry::ToolSpec;

use super::parsing::sanitized_tool_schema;
use super::streaming::StreamControl;
use super::{AssistantReply, ChatClient};

const OPENAI_BASE_URL: &str = "https://api.openai.com";

#[derive(Debug, Clone)]
pub struct OpenAiClient {
    api_key: String,
    http: Client,
    max_predict: usize,
    retries: usize,
    eval_events_path: Option<PathBuf>,
}

impl OpenAiClient {
    pub fn from_env(config: &Config) -> anyhow::Result<Self> {
        let api_key = load_api_key(&config.workspace_root, "OPENAI_API_KEY")?;
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
        })
    }
}

impl ChatClient for OpenAiClient {
    fn label(&self) -> &str {
        "openai"
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn supports_native_tools(&self, _model: &str) -> bool {
        true
    }

    fn allows_xml_fallback(&self) -> bool {
        true
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    fn chat_stream(
        &mut self,
        model: &str,
        messages: &[ConversationMessage],
        tools: &[ToolSpec],
        native_tools_enabled: bool,
        on_chunk: &mut dyn FnMut(&str) -> anyhow::Result<()>,
    ) -> anyhow::Result<AssistantReply> {
        let mut body = build_response_request(
            model,
            messages,
            tools,
            native_tools_enabled,
            self.max_predict,
        );
        body["stream"] = Value::Bool(true);
        eval_events::emit(
            self.eval_events_path.as_deref(),
            json!({
                "event": "provider_request",
                "provider": "openai",
                "model": model,
                "tools": if native_tools_enabled { tools.len() } else { 0 },
            }),
        );
        for attempt in 0..=self.retries {
            match self
                .http
                .post(format!("{OPENAI_BASE_URL}/v1/responses"))
                .bearer_auth(&self.api_key)
                .json(&body)
                .send()
            {
                Ok(response) if response.status().is_success() => {
                    let mut delivered = false;
                    let parsed = parse_openai_stream(response, &mut |chunk| {
                        delivered = true;
                        on_chunk(chunk)
                    });
                    match parsed {
                        Ok(reply) => {
                            eval_events::emit(
                                self.eval_events_path.as_deref(),
                                json!({
                                    "event": "provider_response",
                                    "provider": "openai",
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
                                    "provider": "openai",
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
                        Err(err) => {
                            emit_stream_retry(
                                self.eval_events_path.as_deref(),
                                model,
                                attempt,
                                &err,
                            );
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
                            "provider": "openai",
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
                        Provider::Openai,
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
                            "provider": "openai",
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
                            "provider": "openai",
                            "model": model,
                            "error_kind": "network",
                            "attempt": attempt + 1,
                            "retry_exhausted": true,
                            "message": eval_events::body_snippet(&err.to_string()),
                        }),
                    );
                    return Err(super::guidance::connection_error(
                        Provider::Openai,
                        OPENAI_BASE_URL,
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
        native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        let body = build_response_request(
            model,
            messages,
            tools,
            native_tools_enabled,
            self.max_predict,
        );
        eval_events::emit(
            self.eval_events_path.as_deref(),
            json!({
                "event": "provider_request",
                "provider": "openai",
                "model": model,
                "tools": if native_tools_enabled { tools.len() } else { 0 },
            }),
        );
        for attempt in 0..=self.retries {
            match self
                .http
                .post(format!("{OPENAI_BASE_URL}/v1/responses"))
                .bearer_auth(&self.api_key)
                .json(&body)
                .send()
            {
                Ok(response) if response.status().is_success() => {
                    let body = response.text()?;
                    let parsed = parse_openai_response(&body);
                    match parsed {
                        Ok(reply) => {
                            eval_events::emit(
                                self.eval_events_path.as_deref(),
                                json!({
                                    "event": "provider_response",
                                    "provider": "openai",
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
                                    "provider": "openai",
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
                            "provider": "openai",
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
                        Provider::Openai,
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
                            "provider": "openai",
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
                            "provider": "openai",
                            "model": model,
                            "error_kind": "network",
                            "attempt": attempt + 1,
                            "retry_exhausted": true,
                            "message": eval_events::body_snippet(&err.to_string()),
                        }),
                    );
                    return Err(super::guidance::connection_error(
                        Provider::Openai,
                        OPENAI_BASE_URL,
                        err,
                    ));
                }
                Err(err) => {
                    eval_events::emit(
                        self.eval_events_path.as_deref(),
                        json!({
                            "event": "provider_retry",
                            "provider": "openai",
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
            "provider": "openai",
            "model": model,
            "error_kind": "stream_before_first_token",
            "attempt": attempt + 1,
            "message": eval_events::body_snippet(&err.to_string()),
        }),
    );
}

pub fn build_response_request(
    model: &str,
    messages: &[ConversationMessage],
    tools: &[ToolSpec],
    native_tools_enabled: bool,
    max_predict: usize,
) -> Value {
    let mut body = json!({
        "model": model,
        "input": openai_input(messages),
        "max_output_tokens": max_predict,
    });
    if native_tools_enabled && !tools.is_empty() {
        body["tools"] = Value::Array(tools.iter().map(sanitized_tool_schema).collect());
    }
    body
}

fn openai_input(messages: &[ConversationMessage]) -> Vec<Value> {
    messages
        .iter()
        .map(|message| {
            let content_type = if message.role == "assistant" {
                "output_text"
            } else {
                "input_text"
            };
            json!({
                "role": if message.role == "tool" { "user" } else { message.role.as_str() },
                "content": [{"type": content_type, "text": message.content}]
            })
        })
        .collect()
}

#[derive(Deserialize)]
struct OpenAiResponse {
    #[serde(default)]
    output_text: Option<String>,
    #[serde(default)]
    output: Vec<OpenAiOutput>,
    #[serde(default)]
    usage: Option<OpenAiUsage>,
}

#[derive(Deserialize)]
struct OpenAiOutput {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<Value>,
    #[serde(default)]
    call_id: Option<String>,
    #[serde(default)]
    content: Vec<OpenAiContent>,
}

#[derive(Deserialize)]
struct OpenAiContent {
    #[serde(default)]
    text: String,
}

#[derive(Deserialize)]
struct OpenAiUsage {
    #[serde(default)]
    input_tokens: Option<u64>,
    #[serde(default)]
    output_tokens: Option<u64>,
}

pub fn parse_openai_response(body: &str) -> anyhow::Result<AssistantReply> {
    let parsed: OpenAiResponse = serde_json::from_str(body)?;
    let mut text = parsed.output_text.unwrap_or_default();
    let mut tool_calls = Vec::new();
    for item in parsed.output {
        if item.kind == "function_call" {
            let name = item
                .name
                .ok_or_else(|| anyhow::anyhow!("OpenAI function_call missing name"))?;
            let arguments = normalize_function_arguments(item.arguments)?;
            let arguments = recover_tool_arguments(&name, arguments).arguments;
            tool_calls.push(ToolCall {
                id: item
                    .call_id
                    .unwrap_or_else(|| uuid::Uuid::now_v7().to_string()),
                name,
                arguments,
            });
        }
        for part in item.content {
            text.push_str(&part.text);
        }
    }
    Ok(AssistantReply {
        content: text,
        tool_calls,
        prompt_tokens: parsed.usage.as_ref().and_then(|usage| usage.input_tokens),
        completion_tokens: parsed.usage.and_then(|usage| usage.output_tokens),
    })
}

#[derive(Default)]
struct OpenAiStreamState {
    content: String,
    calls: BTreeMap<usize, OpenAiStreamCall>,
    completed_reply: Option<AssistantReply>,
    completed: bool,
}

#[derive(Default)]
struct OpenAiStreamCall {
    name: String,
    call_id: String,
    arguments: String,
}

pub fn parse_openai_stream<R: std::io::Read>(
    reader: R,
    on_chunk: &mut dyn FnMut(&str) -> anyhow::Result<()>,
) -> anyhow::Result<AssistantReply> {
    let mut state = OpenAiStreamState::default();
    super::streaming::read_sse(reader, |data| {
        let event: Value = serde_json::from_str(data)
            .map_err(|err| anyhow::anyhow!("malformed OpenAI SSE data: {err}"))?;
        let kind = event
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match kind {
            "response.output_text.delta" => {
                let delta = event
                    .get("delta")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow::anyhow!("OpenAI text delta missing delta"))?;
                if !delta.is_empty() {
                    on_chunk(delta)?;
                    state.content.push_str(delta);
                }
            }
            "response.output_item.added" => {
                let index = output_index(&event);
                if let Some(item) = event.get("item")
                    && item.get("type").and_then(Value::as_str) == Some("function_call")
                {
                    let call = state.calls.entry(index).or_default();
                    call.name = item
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    call.call_id = item
                        .get("call_id")
                        .or_else(|| item.get("id"))
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    if let Some(arguments) = item.get("arguments").and_then(Value::as_str) {
                        call.arguments = arguments.to_string();
                    }
                }
            }
            "response.function_call_arguments.delta" => {
                let delta = event.get("delta").and_then(Value::as_str).ok_or_else(|| {
                    anyhow::anyhow!("OpenAI function arguments delta missing delta")
                })?;
                state
                    .calls
                    .entry(output_index(&event))
                    .or_default()
                    .arguments
                    .push_str(delta);
            }
            "response.function_call_arguments.done" => {
                if let Some(arguments) = event.get("arguments").and_then(Value::as_str) {
                    state
                        .calls
                        .entry(output_index(&event))
                        .or_default()
                        .arguments = arguments.to_string();
                }
            }
            "response.output_item.done" => {
                if let Some(item) = event.get("item")
                    && item.get("type").and_then(Value::as_str) == Some("function_call")
                {
                    let call = state.calls.entry(output_index(&event)).or_default();
                    call.name = item
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or(&call.name)
                        .to_string();
                    call.call_id = item
                        .get("call_id")
                        .or_else(|| item.get("id"))
                        .and_then(Value::as_str)
                        .unwrap_or(&call.call_id)
                        .to_string();
                    if let Some(arguments) = item.get("arguments").and_then(Value::as_str) {
                        call.arguments = arguments.to_string();
                    }
                }
            }
            "response.completed" => {
                let response = event
                    .get("response")
                    .ok_or_else(|| anyhow::anyhow!("OpenAI completed event missing response"))?;
                state.completed_reply = Some(parse_openai_response(&response.to_string())?);
                state.completed = true;
                return Ok(StreamControl::Stop);
            }
            "response.failed" | "response.incomplete" | "error" => {
                anyhow::bail!("OpenAI stream failed: {}", eval_events::body_snippet(data));
            }
            _ => {}
        }
        Ok(StreamControl::Continue)
    })?;
    if !state.completed {
        anyhow::bail!("OpenAI stream ended before response.completed");
    }
    let fallback_calls = state
        .calls
        .into_values()
        .map(|call| {
            if call.name.is_empty() {
                anyhow::bail!("OpenAI streamed function_call missing name");
            }
            let arguments = normalize_function_arguments(Some(Value::String(call.arguments)))?;
            Ok(ToolCall {
                id: if call.call_id.is_empty() {
                    uuid::Uuid::now_v7().to_string()
                } else {
                    call.call_id
                },
                name: call.name.clone(),
                arguments: recover_tool_arguments(&call.name, arguments).arguments,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let mut reply = state
        .completed_reply
        .unwrap_or_else(|| AssistantReply::text(String::new()));
    if reply.content.is_empty() {
        reply.content = state.content;
    }
    if reply.tool_calls.is_empty() {
        reply.tool_calls = fallback_calls;
    }
    Ok(reply)
}

fn output_index(event: &Value) -> usize {
    event
        .get("output_index")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or_default()
}

fn normalize_function_arguments(value: Option<Value>) -> anyhow::Result<Value> {
    match value {
        Some(object @ Value::Object(_)) => Ok(object),
        Some(Value::String(raw)) => {
            let decoded: Value = serde_json::from_str(&raw).map_err(|err| {
                anyhow::anyhow!("OpenAI function_call arguments are not valid JSON: {err}")
            })?;
            match decoded {
                Value::Object(_) => Ok(decoded),
                other => Err(anyhow::anyhow!(
                    "OpenAI function_call arguments must decode to object, got {}",
                    json_type(&other)
                )),
            }
        }
        Some(Value::Null) | None => Err(anyhow::anyhow!("OpenAI function_call missing arguments")),
        Some(other) => Err(anyhow::anyhow!(
            "OpenAI function_call arguments must be object or JSON string, got {}",
            json_type(&other)
        )),
    }
}

fn json_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::registry::ToolRegistry;

    #[test]
    fn request_keeps_model_string() {
        let body = build_response_request(
            "gpt-5.4-mini",
            &[],
            ToolRegistry::default().specs(),
            true,
            100,
        );
        assert_eq!(body["model"], "gpt-5.4-mini");
    }

    #[test]
    fn parses_responses_sse_text_tool_and_usage() {
        let stream = concat!(
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"日\"}\n\n",
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"🙂\"}\n\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"output_text\":\"日🙂\",\"output\":[{\"type\":\"function_call\",\"name\":\"Read\",\"call_id\":\"c1\",\"arguments\":\"{\\\"path\\\":\\\"a\\\"}\"}],\"usage\":{\"input_tokens\":3,\"output_tokens\":2}}}\n\n",
            "data: [DONE]\n\n"
        );
        let mut chunks = Vec::new();
        let reply = parse_openai_stream(std::io::Cursor::new(stream.as_bytes()), &mut |chunk| {
            chunks.push(chunk.to_string());
            Ok(())
        })
        .unwrap();
        assert_eq!(chunks.concat(), "日🙂");
        assert_eq!(reply.content, "日🙂");
        assert_eq!(reply.tool_calls[0].name, "Read");
        assert_eq!(reply.tool_calls[0].arguments["path"], "a");
        assert_eq!(reply.prompt_tokens, Some(3));
        assert_eq!(reply.completion_tokens, Some(2));
    }

    #[test]
    fn malformed_sse_after_delta_keeps_partial_chunk_and_errors() {
        let stream = "data: {\"type\":\"response.output_text.delta\",\"delta\":\"partial\"}\n\ndata: {bad}\n\n";
        let mut chunks = Vec::new();
        let err = parse_openai_stream(std::io::Cursor::new(stream), &mut |chunk| {
            chunks.push(chunk.to_string());
            Ok(())
        })
        .unwrap_err()
        .to_string();
        assert_eq!(chunks.concat(), "partial");
        assert!(err.contains("malformed OpenAI SSE"), "{err}");
    }

    #[test]
    fn parses_function_call_arguments_object_for_compat() {
        let reply = parse_openai_response(
            r#"{"output":[{"type":"function_call","name":"Read","call_id":"c1","arguments":{"path":"a"}}]}"#,
        )
        .unwrap();
        assert_eq!(reply.tool_calls[0].name, "Read");
        assert_eq!(reply.tool_calls[0].arguments["path"], "a");
    }

    #[test]
    fn parses_function_call_arguments_json_string() {
        let reply = parse_openai_response(
            r#"{"output":[{"type":"function_call","name":"Grep","call_id":"c1","arguments":"{\"pattern\":\"TODO\"}"}]}"#,
        )
        .unwrap();
        assert_eq!(reply.tool_calls[0].arguments["pattern"], "TODO");
    }

    #[test]
    fn recovers_provider_argument_aliases() {
        let reply = parse_openai_response(
            r#"{"output":[{"type":"function_call","name":"Write","call_id":"c1","arguments":{"file":"provider-probe.txt","body":"ok"}}]}"#,
        )
        .unwrap();
        assert_eq!(reply.tool_calls[0].arguments["path"], "provider-probe.txt");
        assert_eq!(reply.tool_calls[0].arguments["content"], "ok");
    }

    #[test]
    fn rejects_malformed_function_call_arguments_string() {
        let err = parse_openai_response(
            r#"{"output":[{"type":"function_call","name":"Grep","arguments":"{\"pattern\""}]}"#,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("not valid JSON"));
    }

    #[test]
    fn rejects_non_object_function_call_arguments() {
        let err = parse_openai_response(
            r#"{"output":[{"type":"function_call","name":"Grep","arguments":"[\"TODO\"]"}]}"#,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("must decode to object"));
    }
}
