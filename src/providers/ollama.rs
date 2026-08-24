use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::config::{DEFAULT_CONTEXT_BUDGET, OllamaThink, Provider};
use crate::state::{ConversationMessage, ToolCall};
use crate::tools::args_recovery::recover_tool_arguments;
use crate::tools::registry::ToolSpec;

use super::parsing::tool_names;
use super::streaming::StreamControl;
use super::{AssistantReply, ChatClient, ResponseTiming};

#[derive(Debug, Clone)]
pub struct OllamaClient {
    base_url: String,
    http: Client,
    request_options: Value,
    think: Option<OllamaThink>,
    keep_alive: &'static str,
    retries: usize,
    last_response_timing: Option<ResponseTiming>,
}

impl OllamaClient {
    pub fn new(
        base_url: String,
        timeout_secs: u64,
        max_predict: usize,
        retries: usize,
    ) -> anyhow::Result<Self> {
        let http = Client::builder()
            .connect_timeout(std::time::Duration::from_secs(timeout_secs))
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .build()?;
        Ok(Self {
            base_url,
            http,
            request_options: json!({
                "num_ctx": DEFAULT_CONTEXT_BUDGET,
                "num_predict": max_predict,
            }),
            think: None,
            keep_alive: "10m",
            retries,
            last_response_timing: None,
        })
    }

    pub(crate) fn with_think(mut self, think: Option<OllamaThink>) -> Self {
        self.think = think;
        self
    }

    pub fn with_context_budget(mut self, context_budget: usize) -> Self {
        self.request_options["num_ctx"] = json!(context_budget);
        self
    }

    pub fn list_models(&self) -> anyhow::Result<Vec<String>> {
        let response = self
            .http
            .get(format!("{}/api/tags", self.base_url))
            .send()?;
        if !response.status().is_success() {
            anyhow::bail!("Ollama /api/tags failed: {}", response.status());
        }
        let body = response.text()?;
        parse_tags_response(&body)
    }
}

impl ChatClient for OllamaClient {
    fn label(&self) -> &str {
        "ollama"
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

    fn supports_ollama_think(&self) -> bool {
        true
    }

    fn take_response_timing(&mut self) -> Option<ResponseTiming> {
        self.last_response_timing.take()
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
        let body =
            self.chat_request_body_with_stream(model, messages, tools, native_tools_enabled, true);
        for attempt in 0..=self.retries {
            match self
                .http
                .post(format!("{}/api/chat", self.base_url))
                .json(&body)
                .send()
            {
                Ok(response) if response.status().is_success() => {
                    let mut delivered = false;
                    let result = parse_chat_stream(
                        response,
                        &tool_names(tools),
                        !native_tools_enabled,
                        &mut |chunk| {
                            delivered = true;
                            on_chunk(chunk)
                        },
                    );
                    match result {
                        Ok((reply, timing)) => {
                            self.last_response_timing = timing;
                            return Ok(reply);
                        }
                        Err(_)
                            if super::streaming::retry_allowed(
                                attempt,
                                self.retries,
                                delivered,
                            ) =>
                        {
                            continue;
                        }
                        Err(err) if delivered => {
                            return Err(super::streaming::after_first_chunk(err));
                        }
                        Err(err) => return Err(err),
                    }
                }
                Ok(response) if attempt == self.retries => {
                    return Err(super::guidance::http_status_error(
                        Provider::Ollama,
                        model,
                        response.status(),
                    ));
                }
                Ok(_) => {}
                Err(err) if attempt == self.retries => {
                    return Err(super::guidance::connection_error(
                        Provider::Ollama,
                        &self.base_url,
                        err,
                    ));
                }
                Err(_) => {}
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
        let body = self.chat_request_body(model, messages, tools, native_tools_enabled);
        for attempt in 0..=self.retries {
            match self
                .http
                .post(format!("{}/api/chat", self.base_url))
                .json(&body)
                .send()
            {
                Ok(response) if response.status().is_success() => {
                    let body = response.text()?;
                    let (reply, timing) = parse_chat_response_with_timing(
                        &body,
                        &tool_names(tools),
                        !native_tools_enabled,
                    )?;
                    self.last_response_timing = timing;
                    return Ok(reply);
                }
                Ok(response) if attempt == self.retries => {
                    return Err(super::guidance::http_status_error(
                        Provider::Ollama,
                        model,
                        response.status(),
                    ));
                }
                Ok(_) => {}
                Err(err) if attempt == self.retries => {
                    return Err(super::guidance::connection_error(
                        Provider::Ollama,
                        &self.base_url,
                        err,
                    ));
                }
                Err(_) => {}
            }
        }
        unreachable!("retry loop always returns or bails")
    }
}

impl OllamaClient {
    fn chat_request_body(
        &self,
        model: &str,
        messages: &[ConversationMessage],
        tools: &[ToolSpec],
        native_tools_enabled: bool,
    ) -> Value {
        self.chat_request_body_with_stream(model, messages, tools, native_tools_enabled, false)
    }

    fn chat_request_body_with_stream(
        &self,
        model: &str,
        messages: &[ConversationMessage],
        tools: &[ToolSpec],
        native_tools_enabled: bool,
        stream: bool,
    ) -> Value {
        let mut body = json!({
            "model": model,
            "messages": ollama_messages(messages),
            "stream": stream,
            "keep_alive": self.keep_alive,
            "options": self.request_options.clone(),
        });
        if native_tools_enabled && !tools.is_empty() {
            body["tools"] = json!(tools);
        }
        if let Some(think) = self.think {
            body["think"] = match think {
                OllamaThink::True => json!(true),
                OllamaThink::False => json!(false),
                OllamaThink::Low => json!("low"),
                OllamaThink::Medium => json!("medium"),
                OllamaThink::High => json!("high"),
            };
        }
        body
    }
}

fn ollama_messages(messages: &[ConversationMessage]) -> Vec<Value> {
    messages
        .iter()
        .map(|message| {
            let role = if message.role == "tool" {
                "tool"
            } else {
                message.role.as_str()
            };
            let mut value = json!({"role": role, "content": message.content});
            if let Some(name) = &message.name {
                value["name"] = json!(name);
            }
            value
        })
        .collect()
}

#[derive(Deserialize)]
struct TagsResponse {
    #[serde(default)]
    models: Vec<TagModel>,
}

#[derive(Deserialize)]
struct TagModel {
    name: String,
}

pub fn parse_tags_response(body: &str) -> anyhow::Result<Vec<String>> {
    let parsed: TagsResponse = serde_json::from_str(body)?;
    Ok(parsed.models.into_iter().map(|model| model.name).collect())
}

#[derive(Deserialize)]
struct ChatResponse {
    message: Option<ChatMessage>,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    prompt_eval_count: Option<u64>,
    #[serde(default)]
    eval_count: Option<u64>,
    #[serde(default)]
    prompt_eval_duration: Option<u64>,
    #[serde(default)]
    eval_duration: Option<u64>,
    #[serde(default)]
    load_duration: Option<u64>,
    #[serde(default)]
    total_duration: Option<u64>,
}

#[derive(Deserialize)]
struct ChatMessage {
    #[serde(default)]
    content: String,
    #[serde(default)]
    thinking: String,
    #[serde(default)]
    tool_calls: Vec<OllamaToolCall>,
}

#[derive(Deserialize)]
struct OllamaToolCall {
    function: OllamaFunctionCall,
}

#[derive(Deserialize)]
struct OllamaFunctionCall {
    name: String,
    #[serde(default)]
    arguments: Value,
}

pub fn parse_chat_response(
    body: &str,
    allowed_tools: &[String],
    xml_fallback: bool,
) -> anyhow::Result<AssistantReply> {
    parse_chat_response_with_timing(body, allowed_tools, xml_fallback).map(|(reply, _)| reply)
}

fn parse_chat_response_with_timing(
    body: &str,
    allowed_tools: &[String],
    xml_fallback: bool,
) -> anyhow::Result<(AssistantReply, Option<ResponseTiming>)> {
    let parsed: ChatResponse = serde_json::from_str(body)?;
    let message = parsed
        .message
        .ok_or_else(|| anyhow::anyhow!("Ollama response missing message"))?;
    let timing = Some(ResponseTiming {
        prompt_eval_duration: parsed.prompt_eval_duration,
        eval_duration: parsed.eval_duration,
        load_duration: parsed.load_duration,
        total_duration: parsed.total_duration,
    });
    if xml_fallback {
        let (tool_calls, content) =
            super::xml_fallback::extract_tool_calls(&message.content, allowed_tools)?;
        return Ok((
            AssistantReply {
                content,
                tool_calls,
                prompt_tokens: parsed.prompt_eval_count,
                completion_tokens: parsed.eval_count,
            },
            timing,
        ));
    }
    let tool_calls = message
        .tool_calls
        .into_iter()
        .map(|call| {
            let arguments =
                recover_tool_arguments(&call.function.name, call.function.arguments).arguments;
            ToolCall::new(call.function.name, arguments)
        })
        .collect();
    Ok((
        AssistantReply {
            content: message.content,
            tool_calls,
            prompt_tokens: parsed.prompt_eval_count,
            completion_tokens: parsed.eval_count,
        },
        timing,
    ))
}

pub fn parse_chat_stream<R: std::io::Read>(
    reader: R,
    allowed_tools: &[String],
    xml_fallback: bool,
    on_chunk: &mut dyn FnMut(&str) -> anyhow::Result<()>,
) -> anyhow::Result<(AssistantReply, Option<ResponseTiming>)> {
    let mut content = String::new();
    let mut tool_calls = Vec::new();
    let mut prompt_tokens = None;
    let mut completion_tokens = None;
    let mut timing = ResponseTiming::default();
    let mut done = false;
    super::streaming::read_ndjson(reader, |line| {
        let parsed: ChatResponse = serde_json::from_str(line)
            .map_err(|err| anyhow::anyhow!("malformed Ollama NDJSON: {err}"))?;
        if let Some(error) = parsed.error {
            anyhow::bail!("Ollama stream failed: {error}");
        }
        if let Some(message) = parsed.message {
            if !message.content.is_empty() {
                on_chunk(&message.content)?;
                content.push_str(&message.content);
            } else if !message.thinking.is_empty() || !message.tool_calls.is_empty() {
                on_chunk("")?;
            }
            tool_calls.extend(message.tool_calls);
        }
        prompt_tokens = parsed.prompt_eval_count.or(prompt_tokens);
        completion_tokens = parsed.eval_count.or(completion_tokens);
        timing.prompt_eval_duration = parsed.prompt_eval_duration.or(timing.prompt_eval_duration);
        timing.eval_duration = parsed.eval_duration.or(timing.eval_duration);
        timing.load_duration = parsed.load_duration.or(timing.load_duration);
        timing.total_duration = parsed.total_duration.or(timing.total_duration);
        if parsed.done {
            done = true;
            return Ok(StreamControl::Stop);
        }
        Ok(StreamControl::Continue)
    })?;
    if !done {
        anyhow::bail!("Ollama stream ended before its terminal record");
    }
    let has_timing = timing.prompt_eval_duration.is_some()
        || timing.eval_duration.is_some()
        || timing.load_duration.is_some()
        || timing.total_duration.is_some();
    if xml_fallback {
        let (tool_calls, content) =
            super::xml_fallback::extract_tool_calls(&content, allowed_tools)?;
        return Ok((
            AssistantReply {
                content,
                tool_calls,
                prompt_tokens,
                completion_tokens,
            },
            has_timing.then_some(timing),
        ));
    }
    let tool_calls = tool_calls
        .into_iter()
        .map(|call| {
            let arguments =
                recover_tool_arguments(&call.function.name, call.function.arguments).arguments;
            ToolCall::new(call.function.name, arguments)
        })
        .collect();
    Ok((
        AssistantReply {
            content,
            tool_calls,
            prompt_tokens,
            completion_tokens,
        },
        has_timing.then_some(timing),
    ))
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use super::*;
    use crate::state::ConversationMessage;

    #[test]
    fn parses_tags() {
        assert_eq!(
            parse_tags_response(r#"{"models":[{"name":"m"}]}"#).unwrap(),
            vec!["m"]
        );
    }

    #[test]
    fn request_body_is_stable_and_keeps_alive() {
        let client = OllamaClient::new("http://localhost".to_string(), 1, 42, 0)
            .unwrap()
            .with_context_budget(4096);
        let messages = vec![ConversationMessage::user("hello")];
        let tools: Vec<ToolSpec> = Vec::new();
        let model = "qwen3.6:27b-coding-nvfp4";
        let first = client.chat_request_body(model, &messages, &tools, false);
        let second = client.chat_request_body(model, &messages, &tools, false);

        assert_eq!(first, second);
        assert_eq!(first.get("keep_alive").and_then(Value::as_str), Some("10m"));
        assert_eq!(
            first
                .get("options")
                .and_then(Value::as_object)
                .and_then(|options| options.get("num_ctx"))
                .and_then(Value::as_u64),
            Some(4096)
        );
        assert_eq!(
            first
                .get("options")
                .and_then(Value::as_object)
                .and_then(|options| options.get("num_predict"))
                .and_then(Value::as_u64),
            Some(42)
        );
        assert!(first.get("tools").is_none());
        assert!(first.get("think").is_none());
        assert_eq!(
            serde_json::to_string(&first).unwrap(),
            include_str!("../../tests/corpus/apps/cm4-ollama-think/fixtures/request-unset.json")
                .trim_end()
        );
        assert_eq!(
            client.chat_request_body_with_stream(model, &messages, &tools, false, true)["stream"],
            true
        );
    }

    #[test]
    fn request_body_uses_default_context_budget_without_an_override() {
        let client = OllamaClient::new("http://localhost".to_string(), 1, 42, 0).unwrap();
        let body = client.chat_request_body(
            "qwen3.6:27b-coding-nvfp4",
            &[ConversationMessage::user("hello")],
            &[],
            false,
        );

        assert_eq!(
            body["options"]["num_ctx"],
            json!(crate::config::DEFAULT_CONTEXT_BUDGET)
        );
    }

    #[test]
    fn request_body_maps_think_to_the_top_level_with_the_correct_json_type() {
        let messages = vec![ConversationMessage::user("hello")];
        let tools: Vec<ToolSpec> = Vec::new();

        for (think, expected) in [
            (OllamaThink::True, json!(true)),
            (OllamaThink::False, json!(false)),
            (OllamaThink::Low, json!("low")),
            (OllamaThink::Medium, json!("medium")),
            (OllamaThink::High, json!("high")),
        ] {
            let client = OllamaClient::new("http://localhost".to_string(), 1, 42, 0)
                .unwrap()
                .with_context_budget(4096)
                .with_think(Some(think));
            let body = client.chat_request_body("m", &messages, &tools, false);

            assert_eq!(body.get("think"), Some(&expected));
            assert!(
                body.get("options")
                    .and_then(Value::as_object)
                    .is_some_and(|options| !options.contains_key("think"))
            );
        }
    }

    #[test]
    fn request_body_medium_matches_explicit_fixture() {
        let client = OllamaClient::new("http://localhost".to_string(), 1, 42, 0)
            .unwrap()
            .with_context_budget(4096)
            .with_think(Some(OllamaThink::Medium));
        let body = client.chat_request_body(
            "qwen3.8:27b-mlx",
            &[ConversationMessage::user("hello")],
            &[],
            false,
        );

        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            include_str!("../../tests/corpus/apps/cm4-ollama-think/fixtures/request-medium.json")
                .trim_end()
        );
    }

    #[test]
    fn parse_chat_response_records_duration_fields() {
        let (reply, timing) = parse_chat_response_with_timing(
            r#"{
                "message": {"content":"ok"},
                "prompt_eval_count": 10,
                "eval_count": 2,
                "prompt_eval_duration": 4000000000,
                "eval_duration": 2000000000,
                "load_duration": 1000000000,
                "total_duration": 7000000000
            }"#,
            &[],
            false,
        )
        .unwrap();
        assert_eq!(reply.prompt_tokens, Some(10));
        let timing = timing.expect("timing");
        assert_eq!(timing.prompt_eval_duration, Some(4_000_000_000));
        assert_eq!(timing.eval_duration, Some(2_000_000_000));
        assert_eq!(timing.load_duration, Some(1_000_000_000));
        assert_eq!(timing.total_duration, Some(7_000_000_000));
    }

    #[test]
    fn parse_chat_response_discards_separate_thinking() {
        let reply = parse_chat_response(
            r#"{"message":{"thinking":"private reasoning","content":"final answer"}}"#,
            &[],
            false,
        )
        .unwrap();

        assert_eq!(reply.content, "final answer");
    }

    #[test]
    fn parses_ndjson_stream_and_applies_xml_fallback_after_accumulation() {
        let input = concat!(
            r#"{"message":{"content":"<function=Write>{\"path\":\"a\""},"done":false}"#,
            "\n",
            r#"{"message":{"content":",\"content\":\"日🙂\"}</function>"},"done":false}"#,
            "\n",
            r#"{"done":true,"prompt_eval_count":4,"eval_count":2,"total_duration":9}"#,
            "\n"
        );
        let mut chunks = Vec::new();
        let (reply, timing) = parse_chat_stream(
            std::io::Cursor::new(input.as_bytes()),
            &["Write".to_string()],
            true,
            &mut |chunk| {
                chunks.push(chunk.to_string());
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(
            chunks.concat(),
            "<function=Write>{\"path\":\"a\",\"content\":\"日🙂\"}</function>"
        );
        assert_eq!(reply.content, "");
        assert_eq!(reply.tool_calls[0].name, "Write");
        assert_eq!(reply.tool_calls[0].arguments["content"], "日🙂");
        assert_eq!(reply.prompt_tokens, Some(4));
        assert_eq!(reply.completion_tokens, Some(2));
        assert_eq!(timing.unwrap().total_duration, Some(9));
    }

    #[test]
    fn streamed_thinking_is_not_rendered_or_saved() {
        let input = concat!(
            r#"{"message":{"thinking":"private "},"done":false}"#,
            "\n",
            r#"{"message":{"thinking":"reasoning"},"done":false}"#,
            "\n",
            r#"{"message":{"content":"final answer"},"done":false}"#,
            "\n",
            r#"{"done":true}"#,
            "\n"
        );
        let mut chunks = Vec::new();
        let (reply, _) = parse_chat_stream(
            std::io::Cursor::new(input.as_bytes()),
            &[],
            false,
            &mut |chunk| {
                chunks.push(chunk.to_string());
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(chunks, vec!["", "", "final answer"]);
        assert_eq!(reply.content, "final answer");
    }

    #[test]
    fn streamed_thinking_propagates_callback_cancellation_before_visible_content() {
        let input = concat!(
            r#"{"message":{"thinking":"private reasoning"},"done":false}"#,
            "\n",
            r#"{"message":{"content":"must not be reached"},"done":false}"#,
            "\n",
            r#"{"done":true}"#,
            "\n"
        );
        let mut callback_calls = 0;

        let error = parse_chat_stream(
            std::io::Cursor::new(input.as_bytes()),
            &[],
            false,
            &mut |_| {
                callback_calls += 1;
                anyhow::bail!("cancelled stream receiver")
            },
        )
        .unwrap_err()
        .to_string();

        assert_eq!(callback_calls, 1);
        assert!(error.contains("cancelled stream receiver"), "{error}");
    }

    #[test]
    fn incomplete_ndjson_stream_returns_error_after_partial_output() {
        let input = "{\"message\":{\"content\":\"partial\"},\"done\":false}\n";
        let mut chunks = Vec::new();
        let err = parse_chat_stream(std::io::Cursor::new(input), &[], false, &mut |chunk| {
            chunks.push(chunk.to_string());
            Ok(())
        })
        .unwrap_err()
        .to_string();
        assert_eq!(chunks.concat(), "partial");
        assert!(err.contains("terminal record"), "{err}");
    }

    #[test]
    fn runtime_connection_failure_appends_actionable_hint() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let host = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);
        let mut client = OllamaClient::new(host.clone(), 1, 1, 0).unwrap();

        let error = ChatClient::chat(
            &mut client,
            "missing:latest",
            &[ConversationMessage::user("hello")],
            &[],
            false,
        )
        .unwrap_err()
        .to_string();

        assert!(error.starts_with("Ollama request failed:"), "{error}");
        assert!(
            error.ends_with(&format!(
                "Hint: Start Ollama with `ollama serve`, verify `--ollama-host {host}`, then run `commandagent --doctor`."
            )),
            "{error}"
        );
    }

    #[test]
    fn runtime_not_found_failure_appends_pull_hint() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let host = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 1024];
            let read = stream.read(&mut request).unwrap();
            assert!(String::from_utf8_lossy(&request[..read]).starts_with("POST /api/chat "));
            write!(
                stream,
                "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            )
            .unwrap();
        });
        let mut client = OllamaClient::new(host, 1, 1, 0).unwrap();

        let error = ChatClient::chat(
            &mut client,
            "missing:latest",
            &[ConversationMessage::user("hello")],
            &[],
            false,
        )
        .unwrap_err()
        .to_string();
        server.join().unwrap();

        assert_eq!(
            error,
            "Ollama request failed: HTTP 404 Not Found\nHint: Model `missing:latest` was not found. Run `ollama pull missing:latest`, then run `commandagent --doctor`."
        );
    }
}
