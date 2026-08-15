use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use reqwest::blocking::{Client, RequestBuilder};
use serde_json::{Value, json};

use crate::config::{Config, OpenAiApi, Provider, normalize_lm_studio_host};
use crate::eval_events;
use crate::state::ConversationMessage;
use crate::tools::registry::ToolSpec;

use super::{
    AssistantReply, ChatClient, ProviderResponseMetadata, openai_chat_completions, openai_responses,
};

pub const LM_STUDIO_API_TOKEN_ENV: &str = "LM_STUDIO_API_TOKEN";

#[derive(Clone)]
pub struct LmStudioClient {
    api_token: Option<String>,
    http: Client,
    base_url: String,
    max_predict: usize,
    retries: usize,
    api: OpenAiApi,
    eval_events_path: Option<PathBuf>,
    response_metadata: Option<ProviderResponseMetadata>,
    responses_state: Arc<Mutex<openai_responses::ConversationState>>,
}

impl fmt::Debug for LmStudioClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LmStudioClient")
            .field("api_token", &self.api_token.as_ref().map(|_| "<redacted>"))
            .field("base_url", &self.base_url)
            .field("max_predict", &self.max_predict)
            .field("retries", &self.retries)
            .field("api", &self.api)
            .field("eval_events_path", &self.eval_events_path)
            .finish_non_exhaustive()
    }
}

impl LmStudioClient {
    pub fn from_env(config: &Config) -> anyhow::Result<Self> {
        let api_token = crate::env_compat::var(LM_STUDIO_API_TOKEN_ENV)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        Self::new(
            config.lm_studio_host.clone(),
            api_token,
            config.chat_timeout_secs,
            config.num_predict,
            config.chat_retries,
            config.openai_api,
            config.eval_events_path.clone(),
        )
    }

    pub(crate) fn new(
        base_url: String,
        api_token: Option<String>,
        timeout_secs: u64,
        max_predict: usize,
        retries: usize,
        api: OpenAiApi,
        eval_events_path: Option<PathBuf>,
    ) -> anyhow::Result<Self> {
        let http = Client::builder()
            .connect_timeout(std::time::Duration::from_secs(timeout_secs))
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .build()?;
        Ok(Self {
            api_token,
            http,
            base_url: normalize_lm_studio_host(&base_url)?,
            max_predict,
            retries,
            api,
            eval_events_path,
            response_metadata: None,
            responses_state: Arc::new(Mutex::new(openai_responses::ConversationState::default())),
        })
    }

    pub fn list_models(&self) -> anyhow::Result<Vec<String>> {
        let request = self.authorize(self.http.get(format!("{}/v1/models", self.base_url)));
        let response = request.send()?;
        if !response.status().is_success() {
            anyhow::bail!("LM Studio /v1/models failed: {}", response.status());
        }
        parse_models_response(&response.text()?)
    }

    fn chat_completions(
        &mut self,
        model: &str,
        messages: &[ConversationMessage],
        tools: &[ToolSpec],
        native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        let body = openai_chat_completions::build_lm_studio_request(
            model,
            messages,
            tools,
            native_tools_enabled,
            self.max_predict,
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
        let body = openai_responses::build_lm_studio_request(
            model,
            messages,
            tools,
            native_tools_enabled,
            self.max_predict,
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

    fn authorize(&self, request: RequestBuilder) -> RequestBuilder {
        match self.api_token.as_deref() {
            Some(token) => request.bearer_auth(token),
            None => request,
        }
    }

    fn emit_request(&self, model: &str, api: &str, tools: &[ToolSpec], native_tools_enabled: bool) {
        eval_events::emit(
            self.eval_events_path.as_deref(),
            json!({
                "event": "provider_request",
                "provider": Provider::LmStudio.as_str(),
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
            "provider": Provider::LmStudio.as_str(),
            "model": model,
            "api": api,
            "attempt": attempt,
            "tool_calls": reply.tool_calls.len(),
            "response_model": metadata.model_id,
            "system_fingerprint": metadata.system_fingerprint,
        });
        if api == "responses" {
            event["response_id"] = json!(metadata.response_id);
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
            let request = self.authorize(
                self.http
                    .post(format!("{}{endpoint}", self.base_url))
                    .json(body),
            );
            match request.send() {
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
                            "provider": Provider::LmStudio.as_str(),
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
                        Provider::LmStudio,
                        model,
                        status,
                    ));
                }
                Ok(response) => {
                    eval_events::emit(
                        self.eval_events_path.as_deref(),
                        json!({
                            "event": "provider_retry",
                            "provider": Provider::LmStudio.as_str(),
                            "model": model,
                            "api": api,
                            "status": response.status().as_u16(),
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
                            "provider": Provider::LmStudio.as_str(),
                            "model": model,
                            "api": api,
                            "error_kind": "network",
                            "attempt": attempt + 1,
                            "retry_exhausted": true,
                            "message": self.redacted_snippet(&error.to_string()),
                        }),
                    );
                    return Err(super::guidance::connection_error(
                        Provider::LmStudio,
                        &self.base_url,
                        error,
                    ));
                }
                Err(error) => {
                    eval_events::emit(
                        self.eval_events_path.as_deref(),
                        json!({
                            "event": "provider_retry",
                            "provider": Provider::LmStudio.as_str(),
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
                "provider": Provider::LmStudio.as_str(),
                "model": model,
                "api": api,
                "error_kind": "provider_parse_error",
                "message": self.redacted_snippet(&error.to_string()),
            }),
        );
        Err(error)
    }

    fn redacted_snippet(&self, value: &str) -> String {
        let redacted = match self.api_token.as_deref() {
            Some(token) => value.replace(token, "<redacted>"),
            None => value.to_string(),
        };
        eval_events::body_snippet(&redacted)
    }
}

impl ChatClient for LmStudioClient {
    fn label(&self) -> &str {
        Provider::LmStudio.as_str()
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
        self.response_metadata = None;
        match self.api {
            OpenAiApi::ChatCompletions => {
                self.chat_completions(model, messages, tools, native_tools_enabled)
            }
            OpenAiApi::Responses => self.responses(model, messages, tools, native_tools_enabled),
        }
    }
}

fn is_retryable_status(status: u16) -> bool {
    matches!(status, 429 | 500 | 502 | 503 | 504)
}

pub(crate) fn parse_models_response(body: &str) -> anyhow::Result<Vec<String>> {
    let value: Value = serde_json::from_str(body)?;
    let data = value
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("LM Studio model response missing data array"))?;
    Ok(data
        .iter()
        .filter_map(|model| model.get("id").and_then(Value::as_str))
        .map(str::to_string)
        .collect())
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use super::*;
    use crate::tools::registry::ToolRegistry;

    #[test]
    fn parses_openai_compatible_model_list() {
        assert_eq!(
            parse_models_response(r#"{"object":"list","data":[{"id":"qwen/test"}]}"#).unwrap(),
            vec!["qwen/test"]
        );
    }

    #[test]
    fn chat_completions_uses_lm_studio_token_field_and_optional_auth() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = vec![0_u8; 16 * 1024];
            let read = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..read]);
            assert!(request.starts_with("POST /v1/chat/completions "));
            assert!(
                request.contains("authorization: Bearer lm-secret")
                    || request.contains("Authorization: Bearer lm-secret")
            );
            assert!(request.contains("\"max_tokens\":128"), "{request}");
            assert!(!request.contains("max_completion_tokens"), "{request}");
            assert!(request.contains("\"tools\":"), "{request}");
            let body = r#"{"id":"chatcmpl-1","model":"qwen/test","choices":[{"message":{"content":"ok","tool_calls":[{"id":"call-1","type":"function","function":{"name":"Read","arguments":"{\"path\":\"README.md\"}"}}]}}],"usage":{"prompt_tokens":2,"completion_tokens":1}}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        let mut client = LmStudioClient::new(
            format!("http://{address}/v1"),
            Some("lm-secret".to_string()),
            2,
            128,
            0,
            OpenAiApi::ChatCompletions,
            None,
        )
        .unwrap();

        let reply = client
            .chat_completions(
                "qwen/test",
                &[ConversationMessage::user("hello")],
                ToolRegistry::default().specs(),
                true,
            )
            .unwrap();
        server.join().unwrap();

        assert_eq!(reply.content, "ok");
        assert_eq!(reply.tool_calls[0].name, "Read");
        assert_eq!(reply.tool_calls[0].arguments["path"], "README.md");
        assert_eq!(
            client.take_response_metadata().unwrap().model_id.as_deref(),
            Some("qwen/test")
        );
    }

    #[test]
    fn responses_request_omits_openai_only_state_fields() {
        let body = openai_responses::build_lm_studio_request(
            "qwen/test",
            &[ConversationMessage::user("hello")],
            &[],
            false,
            128,
            &openai_responses::ConversationState::default(),
        );

        assert_eq!(body["max_output_tokens"], 128);
        assert!(body.get("store").is_none());
        assert!(body.get("include").is_none());
    }

    #[test]
    fn responses_uses_lm_studio_route_and_parses_native_tool_call() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = vec![0_u8; 16 * 1024];
            let read = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..read]);
            assert!(request.starts_with("POST /v1/responses "));
            assert!(request.contains("\"max_output_tokens\":128"), "{request}");
            assert!(!request.contains("\"store\""), "{request}");
            assert!(!request.contains("\"include\""), "{request}");
            let body = r#"{"id":"resp-1","model":"qwen/test","output":[{"type":"function_call","call_id":"call-1","name":"Read","arguments":"{\"path\":\"README.md\"}"}],"usage":{"input_tokens":2,"output_tokens":1,"total_tokens":3}}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        let mut client = LmStudioClient::new(
            format!("http://{address}"),
            None,
            2,
            128,
            0,
            OpenAiApi::Responses,
            None,
        )
        .unwrap();

        let reply = client
            .responses(
                "qwen/test",
                &[ConversationMessage::user("inspect README")],
                &[],
                false,
            )
            .unwrap();
        server.join().unwrap();

        assert_eq!(reply.tool_calls[0].name, "Read");
        assert_eq!(reply.tool_calls[0].arguments["path"], "README.md");
        let metadata = client.take_response_metadata().unwrap();
        assert_eq!(metadata.response_id.as_deref(), Some("resp-1"));
        assert_eq!(metadata.total_tokens, Some(3));
    }

    #[test]
    fn api_token_is_redacted_from_http_error_events_and_debug() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let secret = "lm-studio-secret-not-real";
        let reflected = secret.to_string();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 4096];
            let _ = stream.read(&mut request).unwrap();
            let body = format!(r#"{{"error":"reflected {reflected}"}}"#);
            write!(
                stream,
                "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join("events.jsonl");
        let mut client = LmStudioClient::new(
            format!("http://{address}"),
            Some(secret.to_string()),
            2,
            128,
            0,
            OpenAiApi::ChatCompletions,
            Some(events.clone()),
        )
        .unwrap();

        let error = client
            .chat_completions(
                "qwen/test",
                &[ConversationMessage::user("hello")],
                &[],
                false,
            )
            .unwrap_err();
        server.join().unwrap();
        let evidence = std::fs::read_to_string(events).unwrap();
        let debug = format!("{client:?}");

        assert!(error.to_string().contains("LM Studio request failed"));
        assert!(evidence.contains("<redacted>"), "{evidence}");
        assert!(!evidence.contains(secret), "{evidence}");
        assert!(!debug.contains(secret), "{debug}");
    }
}
