use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use reqwest::Client;
use serde_json::{Value, json};

use crate::config::{Config, OpenAiApi, Provider, ProviderRole, load_process_api_key};
use crate::eval_events;
use crate::state::ConversationMessage;
use crate::structured_budget::identity::SendPurpose;
use crate::structured_budget::transport::{self, BudgetedBody, RunBudgetHandle};
use crate::tools::registry::ToolSpec;

use super::streaming::{CancelToken, HttpCall};
use super::{
    AssistantReply, ChatClient, ProviderResponseMetadata, openai_chat_completions, openai_responses,
};

const OPENAI_BASE_URL: &str = "https://api.openai.com";
const OPENAI_REASONING_EFFORT_ENV: &str = "COMMANDAGENT_OPENAI_REASONING_EFFORT";

#[derive(Clone)]
pub struct OpenAiClient {
    api_key: String,
    http: Client,
    cancel: CancelToken,
    base_url: String,
    max_predict: usize,
    retries: usize,
    api: OpenAiApi,
    reasoning_effort: Option<String>,
    eval_events_path: Option<PathBuf>,
    response_metadata: Option<ProviderResponseMetadata>,
    responses_state: Arc<Mutex<openai_responses::ConversationState>>,
    /// The run's `usage_tariff` wallet (Issue #1006), shared by every clone. With it, every send
    /// is reserved and started on disk before it leaves; without it nothing here changes.
    run_budget: Option<Arc<RunBudgetHandle>>,
    provider_role: ProviderRole,
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
            .field("run_budget", &self.run_budget.is_some())
            .finish_non_exhaustive()
    }
}

impl OpenAiClient {
    pub fn from_env(config: &Config) -> anyhow::Result<Self> {
        let api_key = load_process_api_key("OPENAI_API_KEY")?;
        let http = super::streaming::http_client(config.chat_timeout_secs)?;
        Ok(Self {
            api_key,
            http,
            cancel: CancelToken::default(),
            base_url: OPENAI_BASE_URL.to_string(),
            max_predict: config.num_predict,
            retries: config.chat_retries,
            api: config.openai_api,
            reasoning_effort: explicit_reasoning_effort(),
            eval_events_path: config.eval_events_path.clone(),
            response_metadata: None,
            responses_state: Arc::new(Mutex::new(openai_responses::ConversationState::default())),
            run_budget: None,
            provider_role: ProviderRole::Executor,
        })
    }

    /// The direct client of `role` for a run that holds a `usage_tariff` wallet: it posts only to
    /// the built-in OpenAI endpoint, and each of its sends goes through that wallet. The factory
    /// builds it only after `admission::admit_responses_client` admitted the role.
    pub(crate) fn from_env_under_run_budget(
        config: &Config,
        role: ProviderRole,
        handle: &Arc<RunBudgetHandle>,
    ) -> anyhow::Result<Self> {
        let mut client = Self::from_env(config)?;
        client.provider_role = role;
        client.under_run_budget(handle, config.chat_timeout_secs)
    }

    /// Under a wallet the client follows no redirect and never asks for more output than the
    /// policy allows, so a request a run would have made big is reserved, not refused.
    fn under_run_budget(
        mut self,
        handle: &Arc<RunBudgetHandle>,
        timeout_secs: u64,
    ) -> anyhow::Result<Self> {
        self.http = transport::no_redirect_client(timeout_secs)?;
        if let Ok(ceiling) = usize::try_from(handle.max_output_tokens()) {
            self.max_predict = self.max_predict.min(ceiling);
        }
        self.run_budget = Some(handle.clone());
        Ok(self)
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
            http: super::streaming::http_client(2).expect("test OpenAI client"),
            cancel: CancelToken::default(),
            base_url: base_url.into(),
            max_predict: 128,
            retries: 0,
            api,
            reasoning_effort: None,
            eval_events_path,
            response_metadata: None,
            responses_state: Arc::new(Mutex::new(openai_responses::ConversationState::default())),
            run_budget: None,
            provider_role: ProviderRole::Executor,
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
        openai_responses::check_request(messages)?;
        let mut body = openai_responses::build_request(
            model,
            messages,
            tools,
            native_tools_enabled,
            self.max_predict,
            self.reasoning_effort.as_deref(),
            &state,
        );
        if let Some(handle) = &self.run_budget {
            under_wallet(&mut body, handle.max_output_tokens());
        }
        self.emit_request(model, "responses", tools, native_tools_enabled);
        let (body, attempt) = self.post_json("/v1/responses", model, "responses", &body)?;
        match openai_responses::parse_session_response(&body, messages, &state) {
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
        // Issue #1006: the validated cache write, additive and only when reported.
        crate::providers::usage_observation::add_event_fields(
            &mut event,
            metadata.cache_write.as_ref(),
        );
        eval_events::emit(self.eval_events_path.as_deref(), event);
    }

    fn post_json(
        &self,
        endpoint: &str,
        model: &str,
        api: &str,
        body: &Value,
    ) -> anyhow::Result<(String, usize)> {
        let call = HttpCall::new(&self.cancel)?;
        let url = format!("{}{endpoint}", self.base_url);
        // The bytes of every try, fixed once, with the endpoint they are posted to: the wallet
        // reserves them only when that is its policy's endpoint.
        let budgeted = self
            .run_budget
            .as_ref()
            .map(|handle| {
                BudgetedBody::for_responses(handle, body, send_purpose(self.provider_role), &url)
            })
            .transpose()?
            .map(|budgeted| budgeted.of_session(crate::provider_call::current_final_repair()));
        for attempt in 0..=self.retries {
            // A try cancelled before it reserves takes nothing from the wallet.
            if budgeted.is_some() && call.is_cancelled() {
                return Err(super::streaming::aborted_by_user());
            }
            let (request, mut send) = match &budgeted {
                Some(budgeted) => {
                    let (request, send) = budgeted
                        .begin(self.http.post(&url))
                        .map_err(transport::refused)?;
                    (request.bearer_auth(&self.api_key), Some(send))
                }
                None => (
                    self.http.post(&url).bearer_auth(&self.api_key).json(body),
                    None,
                ),
            };
            let outcome = call.run(async { request.send().await })?;
            transport::note_outcome(&mut send, &outcome);
            match outcome {
                Ok(response) if response.status().is_success() => {
                    let text = call
                        .run(response.text())?
                        .inspect_err(|error| transport::note_body_error(&mut send, error))?;
                    // The usage is charged before anything reads the reply, and the calls of this
                    // reply are of the send charged here, and of no other.
                    if let Some(response) = transport::settle_body_identified(&mut send, &text)
                        .map_err(transport::refused)?
                    {
                        crate::provider_call::note_settled_response(response);
                    }
                    return Ok((text, attempt + 1));
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

    fn set_cancel_token(&mut self, token: CancelToken) {
        self.cancel = token;
    }

    fn supports_native_tools(&self, _model: &str) -> bool {
        true
    }

    fn allows_xml_fallback(&self) -> bool {
        true
    }

    fn profile_tool_schema_wire(&self) -> super::profile_tool_schema::ProfileToolSchemaWire {
        super::profile_tool_schema::ProfileToolSchemaWire::for_api(self.api)
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

/// What a wallet's request carries beyond the shared builder's: the standard service tier it is
/// priced at, and no more output than the policy allows. `store: false` and the one `include`
/// are the builder's already.
fn under_wallet(body: &mut Value, ceiling: u64) {
    body["service_tier"] = json!("default");
    let asked = body["max_output_tokens"].as_u64().unwrap_or(ceiling);
    body["max_output_tokens"] = json!(asked.min(ceiling));
}

const fn send_purpose(role: ProviderRole) -> SendPurpose {
    match role {
        ProviderRole::Executor => SendPurpose::Executor,
        ProviderRole::Planner => SendPurpose::Planner,
        ProviderRole::Classifier => SendPurpose::Classifier,
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
#[path = "openai_run_budget_tests.rs"]
mod run_budget_tests;

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

#[cfg(test)]
mod cancel_tests {
    use super::*;
    use crate::providers::streaming::cancel_test_support::assert_cancel_closes_connection;

    #[test]
    fn cancel_closes_the_chat_request() {
        assert_cancel_closes_connection(None, |base_url, token| {
            let mut client = OpenAiClient::for_test("sk-test-not-real", base_url, None);
            client.set_cancel_token(token);
            ChatClient::chat(
                &mut client,
                "gpt-test",
                &[ConversationMessage::user("hi")],
                &[],
                false,
            )
        });
    }
}
