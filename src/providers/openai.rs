use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use reqwest::blocking::Client;
use serde_json::{Value, json};

use crate::config::{Config, OpenAiApi, Provider, load_process_api_key};
use crate::eval_events;
use crate::state::ConversationMessage;
use crate::tools::registry::ToolSpec;

use super::{
    AssistantReply, ChatClient, ProviderResponseMetadata, openai_chat_completions, openai_responses,
};

const OPENAI_BASE_URL: &str = "https://api.openai.com";
const OPENAI_REASONING_EFFORT_ENV: &str = "COMMANDAGENT_OPENAI_REASONING_EFFORT";

#[derive(Clone)]
pub struct OpenAiClient {
    api_key: String,
    http: Client,
    base_url: String,
    max_predict: usize,
    retries: usize,
    api: OpenAiApi,
    reasoning_effort: Option<String>,
    eval_events_path: Option<PathBuf>,
    response_metadata: Option<ProviderResponseMetadata>,
    responses_state: Arc<Mutex<openai_responses::ConversationState>>,
}

impl fmt::Debug for OpenAiClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenAiClient")
            .field("api_key", &"<redacted>")
            .field("base_url", &self.base_url)
            .field("max_predict", &self.max_predict)
            .field("retries", &self.retries)
            .field("api", &self.api)
            .field("eval_events_path", &self.eval_events_path)
            .finish_non_exhaustive()
    }
}

impl OpenAiClient {
    pub fn from_env(config: &Config) -> anyhow::Result<Self> {
        let api_key = load_process_api_key("OPENAI_API_KEY")?;
        let http = Client::builder()
            .connect_timeout(std::time::Duration::from_secs(config.chat_timeout_secs))
            .timeout(std::time::Duration::from_secs(config.chat_timeout_secs))
            .build()?;
        Ok(Self {
            api_key,
            http,
            base_url: OPENAI_BASE_URL.to_string(),
            max_predict: config.num_predict,
            retries: config.chat_retries,
            api: config.openai_api,
            reasoning_effort: explicit_reasoning_effort(),
            eval_events_path: config.eval_events_path.clone(),
            response_metadata: None,
            responses_state: Arc::new(Mutex::new(openai_responses::ConversationState::default())),
        })
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
        eval_events_path: Option<PathBuf>,
    ) -> Self {
        Self::for_test_api(
            api_key,
            base_url,
            eval_events_path,
            OpenAiApi::ChatCompletions,
        )
    }

    #[cfg(test)]
    pub(crate) fn for_test_responses(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
        eval_events_path: Option<PathBuf>,
    ) -> Self {
        Self::for_test_api(api_key, base_url, eval_events_path, OpenAiApi::Responses)
    }

    #[cfg(test)]
    fn for_test_api(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
        eval_events_path: Option<PathBuf>,
        api: OpenAiApi,
    ) -> Self {
        Self {
            api_key: api_key.into(),
            http: Client::builder()
                .connect_timeout(std::time::Duration::from_secs(2))
                .timeout(std::time::Duration::from_secs(2))
                .build()
                .expect("test OpenAI client"),
            base_url: base_url.into(),
            max_predict: 128,
            retries: 0,
            api,
            reasoning_effort: None,
            eval_events_path,
            response_metadata: None,
            responses_state: Arc::new(Mutex::new(openai_responses::ConversationState::default())),
        }
    }

    fn chat_completions(
        &mut self,
        model: &str,
        messages: &[ConversationMessage],
        tools: &[ToolSpec],
        native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        let body = openai_chat_completions::build_request(
            model,
            messages,
            tools,
            native_tools_enabled,
            self.max_predict,
            self.reasoning_effort.as_deref(),
        );
        self.emit_request(model, "chat_completions", tools, native_tools_enabled);
        let (body, attempt) =
            self.post_json("/v1/chat/completions", model, "chat_completions", &body)?;
        match openai_chat_completions::parse_response(&body) {
            Ok((reply, metadata)) => {
                self.response_metadata = Some(metadata.clone());
                self.emit_response(model, "chat_completions", attempt, &reply, &metadata);
                Ok(reply)
            }
            Err(error) => self.parse_error(model, "chat_completions", error),
        }
    }

    fn responses(
        &mut self,
        model: &str,
        messages: &[ConversationMessage],
        tools: &[ToolSpec],
        native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        let state = self
            .responses_state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        let body = openai_responses::build_request(
            model,
            messages,
            tools,
            native_tools_enabled,
            self.max_predict,
            self.reasoning_effort.as_deref(),
            &state,
        );
        self.emit_request(model, "responses", tools, native_tools_enabled);
        let (body, attempt) = self.post_json("/v1/responses", model, "responses", &body)?;
        match openai_responses::parse_response(&body) {
            Ok(parsed) => {
                self.responses_state
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .record(&parsed);
                self.response_metadata = Some(parsed.metadata.clone());
                self.emit_response(model, "responses", attempt, &parsed.reply, &parsed.metadata);
                Ok(parsed.reply)
            }
            Err(error) => self.parse_error(model, "responses", error),
        }
    }

    fn emit_request(&self, model: &str, api: &str, tools: &[ToolSpec], native_tools_enabled: bool) {
        eval_events::emit(
            self.eval_events_path.as_deref(),
            json!({
                "event": "provider_request",
                "provider": "openai",
                "model": model,
                "api": api,
                "tools": if native_tools_enabled { tools.len() } else { 0 },
            }),
        );
    }

    fn emit_response(
        &self,
        model: &str,
        api: &str,
        attempt: usize,
        reply: &AssistantReply,
        metadata: &ProviderResponseMetadata,
    ) {
        let mut event = json!({
            "event": "provider_response",
            "provider": "openai",
            "model": model,
            "api": api,
            "attempt": attempt,
            "tool_calls": reply.tool_calls.len(),
            "response_model": metadata.model_id,
            "system_fingerprint": metadata.system_fingerprint,
        });
        if api == "responses" {
            event["response_id"] = json!(metadata.response_id);
            event["service_tier"] = json!(metadata.service_tier);
            event["reasoning_tokens"] = json!(metadata.reasoning_tokens);
        }
        eval_events::emit(self.eval_events_path.as_deref(), event);
    }

    fn post_json(
        &self,
        endpoint: &str,
        model: &str,
        api: &str,
        body: &Value,
    ) -> anyhow::Result<(String, usize)> {
        for attempt in 0..=self.retries {
            match self
                .http
                .post(format!("{}{endpoint}", self.base_url))
                .bearer_auth(&self.api_key)
                .json(body)
                .send()
            {
                Ok(response) if response.status().is_success() => {
                    return Ok((response.text()?, attempt + 1));
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
                            "provider": "openai",
                            "model": model,
                            "api": api,
                            "status": status.as_u16(),
                            "error_kind": "http_status",
                            "attempt": attempt + 1,
                            "retry_exhausted": attempt == self.retries,
                            "retryable": is_retryable_status(status.as_u16()),
                            "body_snippet": self.redacted_snippet(&response_body),
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
                            "api": api,
                            "status": status.as_u16(),
                            "error_kind": "http_status",
                            "attempt": attempt + 1,
                            "retryable": true,
                        }),
                    );
                }
                Err(error) if attempt == self.retries => {
                    eval_events::emit(
                        self.eval_events_path.as_deref(),
                        json!({
                            "event": "provider_error",
                            "provider": "openai",
                            "model": model,
                            "api": api,
                            "error_kind": "network",
                            "attempt": attempt + 1,
                            "retry_exhausted": true,
                            "message": self.redacted_snippet(&error.to_string()),
                        }),
                    );
                    return Err(super::guidance::connection_error(
                        Provider::Openai,
                        &self.base_url,
                        error,
                    ));
                }
                Err(error) => {
                    eval_events::emit(
                        self.eval_events_path.as_deref(),
                        json!({
                            "event": "provider_retry",
                            "provider": "openai",
                            "model": model,
                            "api": api,
                            "error_kind": "network",
                            "attempt": attempt + 1,
                            "message": self.redacted_snippet(&error.to_string()),
                        }),
                    );
                }
            }
        }
        unreachable!("retry loop always returns or bails")
    }

    fn parse_error<T>(&self, model: &str, api: &str, error: anyhow::Error) -> anyhow::Result<T> {
        eval_events::emit(
            self.eval_events_path.as_deref(),
            json!({
                "event": "provider_parse_error",
                "provider": "openai",
                "model": model,
                "api": api,
                "error_kind": "provider_parse_error",
                "message": self.redacted_snippet(&error.to_string()),
            }),
        );
        Err(error)
    }

    fn redacted_snippet(&self, value: &str) -> String {
        eval_events::body_snippet(&value.replace(&self.api_key, "<redacted>"))
    }

    fn dispatch(
        &mut self,
        model: &str,
        messages: &[ConversationMessage],
        tools: &[ToolSpec],
        native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        self.response_metadata = None;
        match self.api {
            OpenAiApi::ChatCompletions => {
                self.chat_completions(model, messages, tools, native_tools_enabled)
            }
            OpenAiApi::Responses => self.responses(model, messages, tools, native_tools_enabled),
        }
    }
}

fn explicit_reasoning_effort() -> Option<String> {
    crate::env_compat::var(OPENAI_REASONING_EFFORT_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

impl ChatClient for OpenAiClient {
    fn label(&self) -> &str {
        "openai"
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn take_response_metadata(&mut self) -> Option<ProviderResponseMetadata> {
        self.response_metadata.take()
    }

    fn supports_native_tools(&self, _model: &str) -> bool {
        true
    }

    fn allows_xml_fallback(&self) -> bool {
        true
    }

    fn chat(
        &mut self,
        model: &str,
        messages: &[ConversationMessage],
        tools: &[ToolSpec],
        native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        self.dispatch(model, messages, tools, native_tools_enabled)
    }
}

fn is_retryable_status(status: u16) -> bool {
    matches!(status, 429 | 500 | 502 | 503 | 504)
}

pub fn build_response_request(
    model: &str,
    messages: &[ConversationMessage],
    tools: &[ToolSpec],
    native_tools_enabled: bool,
    max_predict: usize,
) -> Value {
    openai_responses::build_request(
        model,
        messages,
        tools,
        native_tools_enabled,
        max_predict,
        None,
        &openai_responses::ConversationState::default(),
    )
}

pub fn parse_openai_response(body: &str) -> anyhow::Result<AssistantReply> {
    openai_responses::parse_response(body).map(|parsed| parsed.reply)
}

#[cfg(test)]
mod tests {
    use std::io::{Read as _, Write as _};
    use std::net::TcpListener;

    use super::*;
    use crate::state::ToolCall;
    use crate::tools::registry::ToolRegistry;

    #[test]
    fn explicit_api_selects_endpoint_without_model_sniffing() {
        let cases = [
            (OpenAiApi::ChatCompletions, "/v1/chat/completions"),
            (OpenAiApi::Responses, "/v1/responses"),
        ];
        for (api, endpoint) in cases {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0_u8; 16_384];
                let read = stream.read(&mut request).unwrap();
                let request = String::from_utf8_lossy(&request[..read]);
                assert!(
                    request.starts_with(&format!("POST {endpoint} ")),
                    "{request}"
                );
                let body = if api == OpenAiApi::Responses {
                    r#"{"id":"resp_1","output":[{"type":"message","content":[{"type":"output_text","text":"ok"}]}]}"#
                } else {
                    r#"{"id":"chatcmpl_1","choices":[{"message":{"content":"ok"}}]}"#
                };
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .unwrap();
            });
            let mut client = OpenAiClient::for_test_api(
                "sk-test-not-real",
                format!("http://{address}"),
                None,
                api,
            );
            let reply = client
                .dispatch(
                    "same-model-id",
                    &[ConversationMessage::user("hello")],
                    &[],
                    false,
                )
                .unwrap();
            server.join().unwrap();
            assert_eq!(reply.content, "ok");
        }
    }

    #[test]
    fn responses_reasoning_state_survives_provider_clone_boundary() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let captured = Arc::new(Mutex::new(Vec::<String>::new()));
        let server_captured = captured.clone();
        let server = std::thread::spawn(move || {
            for index in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0_u8; 32_768];
                let read = stream.read(&mut request).unwrap();
                server_captured
                    .lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&request[..read]).to_string());
                let body = if index == 0 {
                    r#"{"id":"resp_1","output":[{"type":"reasoning","id":"rs_1","encrypted_content":"opaque-state"},{"type":"function_call","id":"fc_1","call_id":"call_1","name":"Read","arguments":"{\"path\":\"README.md\"}"}]}"#
                } else {
                    r#"{"id":"resp_2","output":[{"type":"message","content":[{"type":"output_text","text":"done"}]}]}"#
                };
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .unwrap();
            }
        });
        let mut client =
            OpenAiClient::for_test_responses("sk-test-not-real", format!("http://{address}"), None);
        let mut worker = client.clone();
        let first = worker
            .responses(
                "gpt-5.6-luna",
                &[ConversationMessage::user("inspect")],
                ToolRegistry::default().specs(),
                true,
            )
            .unwrap();
        let messages = [
            ConversationMessage::user("inspect"),
            ConversationMessage::assistant(first.content, first.tool_calls),
            ConversationMessage::tool_result("Read", Some("call_1"), "contents"),
        ];
        let second = client
            .responses(
                "gpt-5.6-luna",
                &messages,
                ToolRegistry::default().specs(),
                true,
            )
            .unwrap();
        server.join().unwrap();

        assert_eq!(second.content, "done");
        let second_request = &captured.lock().unwrap()[1];
        assert!(second_request.contains("opaque-state"), "{second_request}");
        assert!(
            second_request.contains("function_call_output"),
            "{second_request}"
        );
        assert!(second_request.contains("call_1"), "{second_request}");
    }

    #[test]
    fn responses_error_redacts_api_key_from_events_and_debug() {
        let secret = "sk-proj-responses-secret-not-real-123456789";
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server_secret = secret.to_string();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 8192];
            let _ = stream.read(&mut request).unwrap();
            let body = format!(r#"{{"error":{{"message":"reflected {server_secret}"}}}}"#);
            write!(
                stream,
                "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        });
        let tmp = tempfile::tempdir().unwrap();
        let events = tmp.path().join("events.jsonl");
        let mut client = OpenAiClient::for_test_responses(
            secret,
            format!("http://{address}"),
            Some(events.clone()),
        );
        let error = client
            .responses(
                "gpt-5.6-luna",
                &[ConversationMessage::user("hello")],
                &[],
                false,
            )
            .unwrap_err()
            .to_string();
        server.join().unwrap();
        let outputs = format!(
            "{error}\n{}\n{client:?}",
            std::fs::read_to_string(events).unwrap()
        );

        assert!(!outputs.contains(secret), "secret leaked: {outputs}");
        assert!(outputs.contains("<redacted>"), "{outputs}");
    }

    #[test]
    fn public_response_parser_recovers_argument_aliases() {
        let reply = parse_openai_response(
            r#"{"output":[{"type":"function_call","name":"Write","call_id":"c1","arguments":{"file":"provider-probe.txt","body":"ok"}}]}"#,
        )
        .unwrap();
        assert_eq!(reply.tool_calls[0].arguments["path"], "provider-probe.txt");
        assert_eq!(reply.tool_calls[0].arguments["content"], "ok");
    }

    #[test]
    fn assistant_fallback_keeps_typed_tool_call() {
        let body = build_response_request(
            "gpt-5.6-luna",
            &[ConversationMessage::assistant(
                "",
                vec![ToolCall {
                    id: "call-1".to_string(),
                    name: "Read".to_string(),
                    arguments: json!({"path": "README.md"}),
                }],
            )],
            ToolRegistry::default().specs(),
            true,
            128,
        );
        assert_eq!(body["input"][0]["type"], "function_call");
        assert_eq!(body["input"][0]["call_id"], "call-1");
    }
}
