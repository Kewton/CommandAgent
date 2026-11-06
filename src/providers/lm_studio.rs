use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use reqwest::{Client, RequestBuilder};
use serde_json::{Value, json};

use crate::config::{
    Config, OpenAiApi, Provider, ProviderRole, normalize_lm_studio_host,
    normalize_openai_compatible_base_url,
};
use crate::eval_events;
use crate::state::ConversationMessage;
use crate::structured_budget::identity::SendPurpose;
use crate::structured_budget::transport::{self, BudgetedBody, RunBudgetHandle};
use crate::tools::registry::ToolSpec;

use super::compatible_effort::{self, CompatibleEffortSetting};
use super::openrouter_routing::ClientRouting;
use super::streaming::{CancelToken, HttpCall};
use super::{
    AssistantReply, ChatClient, ProviderResponseMetadata, openai_chat_completions, openai_responses,
};

pub const LM_STUDIO_API_TOKEN_ENV: &str = "LM_STUDIO_API_TOKEN";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompatibleKind {
    LmStudio,
    Generic,
}

impl CompatibleKind {
    const fn label(self) -> &'static str {
        match self {
            Self::LmStudio => "lm-studio",
            Self::Generic => "openai-compatible",
        }
    }

    const fn display_name(self) -> &'static str {
        match self {
            Self::LmStudio => "LM Studio",
            Self::Generic => "OpenAI-compatible",
        }
    }
}

#[derive(Clone)]
pub struct LmStudioClient {
    kind: CompatibleKind,
    api_token: Option<String>,
    api_key_env: Option<String>,
    http: Client,
    cancel: CancelToken,
    base_url: String,
    max_predict: usize,
    retries: usize,
    api: OpenAiApi,
    eval_events_path: Option<PathBuf>,
    response_metadata: Option<ProviderResponseMetadata>,
    responses_state: Arc<Mutex<openai_responses::ConversationState>>,
    /// Snapshot of `COMMANDAGENT_COMPATIBLE_REASONING_EFFORT` taken by the
    /// `from_*env` constructors; every clone, worker, and retry sends it.
    reasoning_effort: Option<&'static str>,
    provider_role: ProviderRole,
    /// The preset's `openrouter_provider`, fixed at construction for generic
    /// roles only (Issue #939); `None` sends and records nothing new.
    openrouter_routing: Option<ClientRouting>,
    /// The run's wallet (Issue #976). `None` is off, and then the wire stays as it was.
    run_budget: Option<Arc<RunBudgetHandle>>,
}

impl fmt::Debug for LmStudioClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LmStudioClient")
            .field("kind", &self.kind)
            .field("api_token", &self.api_token.as_ref().map(|_| "<redacted>"))
            .field("api_key_env", &self.api_key_env)
            .field("base_url", &self.base_url)
            .field("max_predict", &self.max_predict)
            .field("retries", &self.retries)
            .field("api", &self.api)
            .field("eval_events_path", &self.eval_events_path)
            .field("reasoning_effort", &self.reasoning_effort)
            .field("provider_role", &self.provider_role)
            .field("openrouter_routing", &self.openrouter_routing)
            .finish_non_exhaustive()
    }
}

/// The optional LM Studio token this process holds, trimmed, or `None` when it is blank.
pub(crate) fn process_api_token() -> Option<String> {
    crate::startup_credentials::var(LM_STUDIO_API_TOKEN_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

impl LmStudioClient {
    pub fn from_env(config: &Config) -> anyhow::Result<Self> {
        Self::from_env_for_role(config, ProviderRole::Executor)
    }

    pub(crate) fn from_env_for_role(config: &Config, role: ProviderRole) -> anyhow::Result<Self> {
        let reasoning_effort = CompatibleEffortSetting::from_process_env().for_client()?;
        let api_token = process_api_token();
        Self::new(
            config.lm_studio_host.clone(),
            api_token,
            config.chat_timeout_secs,
            config.num_predict,
            config.chat_retries,
            config.openai_api,
            config.eval_events_path.clone(),
        )
        .map(|client| client.with_reasoning_effort(reasoning_effort, role))
    }

    pub(crate) fn from_openai_compatible_env(
        config: &Config,
        role: ProviderRole,
    ) -> anyhow::Result<Self> {
        let compatible = config
            .openai_compatible
            .as_ref()
            .filter(|compatible| compatible.applies_to(role))
            .ok_or_else(|| anyhow::anyhow!("openai-compatible role configuration is missing"))?;
        let reasoning_effort = CompatibleEffortSetting::from_process_env().for_client()?;
        let api_token = compatible
            .api_key_env
            .as_deref()
            .map(crate::config::load_process_api_key)
            .transpose()?;
        Self::new_openai_compatible(
            compatible.base_url.clone(),
            compatible.api_key_env.clone(),
            api_token,
            config.chat_timeout_secs,
            config.num_predict,
            config.chat_retries,
            config.openai_api,
            config.eval_events_path.clone(),
        )
        .and_then(|client| {
            let mut client = client.with_reasoning_effort(reasoning_effort, role);
            client.openrouter_routing = ClientRouting::for_role(compatible, role);
            match &config.field_sources.run_budget {
                Some(handle) => client.under_run_budget(handle, config.chat_timeout_secs),
                None => Ok(client),
            }
        })
    }

    /// Under a wallet a client follows no redirect and never asks for more output than the policy
    /// allows, so a request a run would have made big is reserved, not refused.
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

    fn with_reasoning_effort(
        mut self,
        reasoning_effort: Option<&'static str>,
        role: ProviderRole,
    ) -> Self {
        self.reasoning_effort = reasoning_effort;
        self.provider_role = role;
        self
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
        Self::new_inner(
            CompatibleKind::LmStudio,
            normalize_lm_studio_host(&base_url)?,
            Some(LM_STUDIO_API_TOKEN_ENV.to_string()),
            api_token,
            timeout_secs,
            max_predict,
            retries,
            api,
            eval_events_path,
        )
    }

    #[allow(clippy::too_many_arguments, reason = "mirrors provider config fields")]
    pub(crate) fn new_openai_compatible(
        base_url: String,
        api_key_env: Option<String>,
        api_token: Option<String>,
        timeout_secs: u64,
        max_predict: usize,
        retries: usize,
        api: OpenAiApi,
        eval_events_path: Option<PathBuf>,
    ) -> anyhow::Result<Self> {
        Self::new_inner(
            CompatibleKind::Generic,
            normalize_openai_compatible_base_url(&base_url)?,
            api_key_env,
            api_token,
            timeout_secs,
            max_predict,
            retries,
            api,
            eval_events_path,
        )
    }

    #[allow(clippy::too_many_arguments, reason = "mirrors provider config fields")]
    fn new_inner(
        kind: CompatibleKind,
        base_url: String,
        api_key_env: Option<String>,
        api_token: Option<String>,
        timeout_secs: u64,
        max_predict: usize,
        retries: usize,
        api: OpenAiApi,
        eval_events_path: Option<PathBuf>,
    ) -> anyhow::Result<Self> {
        let http = super::streaming::http_client(timeout_secs)?;
        Ok(Self {
            kind,
            api_token,
            api_key_env,
            http,
            cancel: CancelToken::default(),
            base_url,
            max_predict,
            retries,
            api,
            eval_events_path,
            response_metadata: None,
            responses_state: Arc::new(Mutex::new(openai_responses::ConversationState::default())),
            reasoning_effort: None,
            provider_role: ProviderRole::Executor,
            openrouter_routing: None,
            run_budget: None,
        })
    }

    pub fn list_models(&self) -> anyhow::Result<Vec<String>> {
        let request = self.authorize(self.http.get(format!("{}/v1/models", self.base_url)));
        let call = HttpCall::new(&self.cancel)?;
        let response = call.run(async { request.send().await })??;
        if !response.status().is_success() {
            anyhow::bail!(
                "{} /v1/models failed: {}",
                self.kind.display_name(),
                response.status()
            );
        }
        parse_models_response(&call.run(response.text())??)
    }

    fn chat_completions(
        &mut self,
        model: &str,
        messages: &[ConversationMessage],
        tools: &[ToolSpec],
        native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        let body = match self.reasoning_effort {
            Some(effort) => openai_chat_completions::build_lm_studio_request_with_effort(
                model,
                messages,
                tools,
                native_tools_enabled,
                self.max_predict,
                effort,
            ),
            None => openai_chat_completions::build_lm_studio_request(
                model,
                messages,
                tools,
                native_tools_enabled,
                self.max_predict,
            ),
        };
        let body = self.route(body)?;
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
        let body = match self.reasoning_effort {
            Some(effort) => openai_responses::build_lm_studio_request_with_effort(
                model,
                messages,
                tools,
                native_tools_enabled,
                self.max_predict,
                effort,
                &state,
            ),
            None => openai_responses::build_lm_studio_request(
                model,
                messages,
                tools,
                native_tools_enabled,
                self.max_predict,
                &state,
            ),
        };
        let body = self.route(body)?;
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

    fn route(&self, body: Value) -> anyhow::Result<Value> {
        match &self.openrouter_routing {
            Some(routing) => routing.apply(body),
            None => Ok(body),
        }
    }

    fn authorize(&self, request: RequestBuilder) -> RequestBuilder {
        match self.api_token.as_deref() {
            Some(token) => request.bearer_auth(token),
            None => request,
        }
    }

    fn emit_request(&self, model: &str, api: &str, tools: &[ToolSpec], native_tools_enabled: bool) {
        let mut event = json!({
            "event": "provider_request",
            "provider": self.kind.label(),
            "model": model,
            "api": api,
            "tools": if native_tools_enabled { tools.len() } else { 0 },
        });
        // Additive and only when declared: an omitted effort keeps the event
        // byte-identical to the one before Issue #931.
        if let Some(effort) = self.reasoning_effort {
            event["requested_reasoning_effort"] = json!(effort);
            event["reasoning_effort_source"] = json!(compatible_effort::SOURCE_ENV);
            event["provider_role"] = json!(compatible_effort::role_name(self.provider_role));
        }
        if let Some(routing) = &self.openrouter_routing {
            routing.annotate_request(&mut event);
        }
        eval_events::emit(self.eval_events_path.as_deref(), event);
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
            "provider": self.kind.label(),
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
        // Issue #947: additive and only when reported, as a JSON string of the
        // decimal text the provider sent (e.g. "0.012345") for an aggregator
        // to read as a decimal. The turn's provider_turn_duration carries the
        // same amount; count it once per turn, never the two added together.
        // An amount that overlaps a registered secret is left out (unreported)
        // rather than scrubbed into a marker.
        if let Some(cost) = metadata
            .billed_cost_usd
            .as_ref()
            .and_then(|cost| cost.event_value(self.eval_events_path.as_deref()))
        {
            event["provider_billed_cost_usd"] = cost;
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
        let call = HttpCall::new(&self.cancel)?;
        let budgeted = self
            .run_budget
            .as_ref()
            .map(|handle| BudgetedBody::new(handle, body, send_purpose(self.provider_role)))
            .transpose()?;
        for attempt in 0..=self.retries {
            let request = self.http.post(format!("{}{endpoint}", self.base_url));
            let (request, mut send) = match &budgeted {
                Some(budgeted) => {
                    let (request, send) = budgeted.begin(request).map_err(transport::refused)?;
                    (request, Some(send))
                }
                None => (request.json(body), None),
            };
            let request = self.authorize(request);
            let outcome = call.run(async { request.send().await })?;
            transport::note_outcome(&mut send, &outcome);
            match outcome {
                Ok(response) if response.status().is_success() => {
                    let text = call
                        .run(response.text())?
                        .inspect_err(|error| transport::note_body_error(&mut send, error))?;
                    transport::settle_body(&mut send, &text).map_err(transport::refused)?;
                    if let Some(routing) = &self.openrouter_routing {
                        let event = routing.observed_event(
                            self.kind.label(),
                            api,
                            model,
                            attempt + 1,
                            &text,
                            |value| self.redacted_value(value),
                        );
                        eval_events::emit(self.eval_events_path.as_deref(), event);
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
                            "provider": self.kind.label(),
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
                    return Err(self.http_status_error(model, status));
                }
                Ok(response) => {
                    eval_events::emit(
                        self.eval_events_path.as_deref(),
                        json!({
                            "event": "provider_retry",
                            "provider": self.kind.label(),
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
                            "provider": self.kind.label(),
                            "model": model,
                            "api": api,
                            "error_kind": "network",
                            "attempt": attempt + 1,
                            "retry_exhausted": true,
                            "message": self.redacted_snippet(&error.to_string()),
                        }),
                    );
                    return Err(self.connection_error(error));
                }
                Err(error) => {
                    eval_events::emit(
                        self.eval_events_path.as_deref(),
                        json!({
                            "event": "provider_retry",
                            "provider": self.kind.label(),
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
                "provider": self.kind.label(),
                "model": model,
                "api": api,
                "error_kind": "provider_parse_error",
                "message": self.redacted_snippet(&error.to_string()),
            }),
        );
        Err(error)
    }

    fn redacted_snippet(&self, value: &str) -> String {
        eval_events::body_snippet(&self.redacted_value(value))
    }

    fn redacted_value(&self, value: &str) -> String {
        match self.api_token.as_deref() {
            Some(token) => value.replace(token, "<redacted>"),
            None => value.to_string(),
        }
    }

    fn connection_error(&self, error: impl std::fmt::Display) -> anyhow::Error {
        if self.kind == CompatibleKind::LmStudio {
            return super::guidance::connection_error(Provider::LmStudio, &self.base_url, error);
        }
        anyhow::anyhow!(
            "{} request failed: {}\nHint: Check connectivity to {}, verify `--base-url`, then run `commandagent --doctor`.",
            self.kind.display_name(),
            single_line(&error.to_string()),
            single_line(&self.base_url),
        )
    }

    fn http_status_error(&self, model: &str, status: reqwest::StatusCode) -> anyhow::Error {
        if self.kind == CompatibleKind::LmStudio {
            return super::guidance::http_status_error(Provider::LmStudio, model, status);
        }
        let hint = match status.as_u16() {
            401 | 403 => self.api_key_env.as_deref().map_or_else(
                || "Configure server authentication, then run `commandagent --doctor`.".to_string(),
                |name| {
                    format!(
                        "Set `{}` in the process environment, then run `commandagent --doctor`.",
                        single_line(name)
                    )
                },
            ),
            404 => format!(
                "Verify model `{}` exists and is served by the configured endpoint, then run `commandagent --doctor`.",
                single_line(model)
            ),
            _ => "Run `commandagent --doctor` and check the provider configuration.".to_string(),
        };
        anyhow::anyhow!(
            "{} request failed: HTTP {status}\nHint: {hint}",
            self.kind.display_name()
        )
    }
}

impl ChatClient for LmStudioClient {
    fn label(&self) -> &str {
        self.kind.label()
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
        self.response_metadata = None;
        match self.api {
            OpenAiApi::ChatCompletions => {
                self.chat_completions(model, messages, tools, native_tools_enabled)
            }
            OpenAiApi::Responses => self.responses(model, messages, tools, native_tools_enabled),
        }
    }
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

fn single_line(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_control() {
                '?'
            } else {
                character
            }
        })
        .collect()
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
    fn generic_chat_completions_uses_shared_transport_capabilities_and_identity() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = vec![0_u8; 16 * 1024];
            let read = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..read]);
            assert!(request.starts_with("POST /v1/chat/completions "));
            assert!(
                request.contains("authorization: Bearer gateway-secret")
                    || request.contains("Authorization: Bearer gateway-secret")
            );
            assert!(request.contains("\"max_tokens\":128"), "{request}");
            assert!(request.contains("\"tools\":"), "{request}");
            let body = r#"{"id":"chatcmpl-generic","model":"served-model","choices":[{"message":{"content":"ok","tool_calls":[{"id":"call-1","type":"function","function":{"name":"Read","arguments":"{\"path\":\"README.md\"}"}}]}}],"usage":{"prompt_tokens":2,"completion_tokens":1}}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join("events.jsonl");
        let mut client = LmStudioClient::new_openai_compatible(
            format!("http://{address}/v1"),
            Some("GATEWAY_TOKEN".to_string()),
            Some("gateway-secret".to_string()),
            2,
            128,
            0,
            OpenAiApi::ChatCompletions,
            Some(events.clone()),
        )
        .unwrap();

        assert_eq!(client.label(), "openai-compatible");
        assert!(client.supports_native_tools("served-model"));
        assert!(client.allows_xml_fallback());
        assert!(!client.supports_ollama_think());
        let reply = client
            .chat_completions(
                "served-model",
                &[ConversationMessage::user("hello")],
                ToolRegistry::default().specs(),
                true,
            )
            .unwrap();
        server.join().unwrap();

        assert_eq!(reply.content, "ok");
        assert_eq!(reply.tool_calls[0].name, "Read");
        let metadata = client.take_response_metadata().unwrap();
        assert_eq!(metadata.response_id.as_deref(), Some("chatcmpl-generic"));
        assert_eq!(metadata.model_id.as_deref(), Some("served-model"));
        let evidence = std::fs::read_to_string(events).unwrap();
        assert!(
            evidence.contains("\"provider\":\"openai-compatible\""),
            "{evidence}"
        );
        assert!(!evidence.contains("gateway-secret"), "{evidence}");
    }

    #[test]
    fn legacy_constructors_keep_the_reasoning_effort_unset() {
        let lm_studio = LmStudioClient::new(
            "http://127.0.0.1:9".to_string(),
            None,
            2,
            128,
            0,
            OpenAiApi::Responses,
            None,
        )
        .unwrap();
        let generic = LmStudioClient::new_openai_compatible(
            "http://127.0.0.1:9/v1".to_string(),
            None,
            None,
            2,
            128,
            0,
            OpenAiApi::ChatCompletions,
            None,
        )
        .unwrap();

        for client in [lm_studio, generic] {
            assert_eq!(client.reasoning_effort, None);
            assert_eq!(client.clone().reasoning_effort, None);
        }
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

/// One send under a wallet, through the real client and the real ledger, against a disposable
/// listener on loopback that counts what reaches it. The listener and the client are in step by
/// what the listener has read, and only a timeout row waits for a clock.
#[cfg(test)]
mod run_budget_tests {
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpListener, TcpStream};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread::JoinHandle;
    use std::time::Duration;

    use serde_json::{Value, json};

    use super::{LmStudioClient, openai_chat_completions};
    use crate::config::{Config, ProviderRole};
    use crate::planner::structured_stages::receipt::sha256_hex;
    use crate::provider_call::{self, ProviderCallOutcome, ProviderCallScope, ProviderChatRequest};
    use crate::providers::streaming::{CancelToken, HttpCall, http_client};
    use crate::state::ConversationMessage;
    use crate::structured_budget::clock::BudgetClock;
    use crate::structured_budget::ledger::{
        CommandWrites, LedgerEntry, LedgerFailure, LedgerOpen, LedgerRefusal,
    };
    use crate::structured_budget::policy::RunBudgetPolicy;
    use crate::structured_budget::reservation::{
        AttemptState, BudgetRefusal, LatchReason, Money, reservation_amount,
    };
    use crate::structured_budget::settlement::{SettleReport, UnknownReason};
    use crate::structured_budget::test_support::{FakeClock, WALLET, policy_value};
    use crate::structured_budget::transport::{
        RunBudgetHandle, SandboxView, SendRefusal, budget_refusal,
    };
    use crate::tools::os_sandbox::{
        self, SandboxAvailability, SandboxMode, SandboxPolicy, SandboxTestState,
    };

    const KNOWN: &str = r#"{"prompt_tokens":10,"completion_tokens":5,"cost":0.00001}"#;

    fn reply_body(usage: &str) -> String {
        format!(
            r#"{{"id":"c-976","model":"m-1","choices":[{{"message":{{"content":"done"}}}}],"usage":{usage}}}"#
        )
    }

    #[derive(Clone)]
    enum Reply {
        Status(u16, String),
        Redirect(u16, String),
        /// Says more bytes than it sends, then closes.
        Cut(String),
        /// Reads, answers nothing, and waits for the client to close.
        Hang,
        /// Reads and closes without a word.
        Close,
    }

    #[derive(Debug, Clone)]
    struct Received {
        head: String,
        body: Vec<u8>,
    }

    impl Received {
        /// The header lines, without the request line or the listener's own address.
        fn headers(&self) -> Vec<&str> {
            let mut lines = self
                .head
                .lines()
                .skip(1)
                .filter(|line| !line.starts_with("host:"))
                .collect::<Vec<_>>();
            lines.sort_unstable();
            lines
        }
    }

    fn read_request(stream: &mut TcpStream) -> Received {
        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .unwrap();
        let mut data = Vec::new();
        let mut buffer = [0_u8; 8192];
        loop {
            if let Some(end) = data.windows(4).position(|window| window == b"\r\n\r\n") {
                let head = String::from_utf8_lossy(&data[..end]).to_ascii_lowercase();
                let length = head
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length:"))
                    .and_then(|value| value.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                if data.len() >= end + 4 + length {
                    return Received {
                        head,
                        body: data[end + 4..end + 4 + length].to_vec(),
                    };
                }
            }
            let read = stream.read(&mut buffer).unwrap();
            assert!(read > 0, "the client closed before its request was whole");
            data.extend_from_slice(&buffer[..read]);
        }
    }

    fn answer(stream: &mut TcpStream, reply: &Reply) {
        let text = match reply {
            Reply::Status(status, body) => format!(
                "HTTP/1.1 {status} Scripted\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            ),
            Reply::Redirect(status, location) => format!(
                "HTTP/1.1 {status} Scripted\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            ),
            Reply::Cut(body) => format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len() + 100
            ),
            Reply::Hang => {
                let mut rest = [0_u8; 256];
                while matches!(stream.read(&mut rest), Ok(read) if read > 0) {}
                return;
            }
            Reply::Close => return,
        };
        // A client that has gone is its own row's concern.
        let _ = stream.write_all(text.as_bytes());
        let _ = stream.flush();
    }

    struct Fake {
        address: SocketAddr,
        stop: Arc<AtomicBool>,
        thread: JoinHandle<Vec<Received>>,
    }

    impl Fake {
        fn spawn(replies: Vec<Reply>) -> Self {
            Self::watching(replies, |_| {})
        }

        /// `on_request` runs with the whole request read and no answer sent.
        fn watching(
            replies: Vec<Reply>,
            mut on_request: impl FnMut(&Received) + Send + 'static,
        ) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            listener.set_nonblocking(true).unwrap();
            let stop = Arc::new(AtomicBool::new(false));
            let thread_stop = stop.clone();
            let thread = std::thread::spawn(move || {
                let mut replies = replies.into_iter();
                let mut received = Vec::new();
                while !thread_stop.load(Ordering::SeqCst) {
                    let mut stream = match listener.accept() {
                        Ok((stream, _)) => stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(5));
                            continue;
                        }
                        Err(error) => panic!("accept: {error}"),
                    };
                    stream.set_nonblocking(false).unwrap();
                    let request = read_request(&mut stream);
                    on_request(&request);
                    received.push(request);
                    let reply = replies
                        .next()
                        .unwrap_or_else(|| Reply::Status(500, "{}".to_string()));
                    answer(&mut stream, &reply);
                }
                received
            });
            Self {
                address,
                stop,
                thread,
            }
        }

        fn url(&self) -> String {
            format!("http://{}/v1", self.address)
        }

        fn finish(self) -> Vec<Received> {
            self.stop.store(true, Ordering::SeqCst);
            self.thread.join().unwrap()
        }
    }

    struct World {
        /// Held only by a world that installed the sandbox of the process, which only a child
        /// process does.
        _sandbox: Option<SandboxTestState>,
        _root: tempfile::TempDir,
        base: PathBuf,
        workspace: PathBuf,
        handle: Arc<RunBudgetHandle>,
    }

    fn policy_for(base: &Path, change: impl FnOnce(&mut Value)) -> Arc<RunBudgetPolicy> {
        let mut value = policy_value();
        value["ledger_dir"] = json!(base.join("ledgers").to_str().unwrap());
        change(&mut value);
        Arc::new(RunBudgetPolicy::decode(value.to_string().as_bytes(), WALLET).unwrap())
    }

    fn install(mode: SandboxMode, workspace: &Path, availability: SandboxAvailability) {
        let run_tmp = workspace.parent().unwrap().join("run-tmp");
        std::fs::create_dir_all(&run_tmp).unwrap();
        os_sandbox::install_for_test(mode, SandboxPolicy::new(workspace, &run_tmp), availability);
    }

    /// What a sandbox on `workspace` lets commands write, with the temporary [`install`] uses, for
    /// a wallet to be judged by. No sandbox of the process is read.
    fn writes_on(workspace: &Path) -> CommandWrites {
        let base = workspace.parent().unwrap();
        let run_tmp = base.join("run-tmp");
        std::fs::create_dir_all(&run_tmp).unwrap();
        CommandWrites::of(SandboxPolicy::with_home(
            workspace,
            &run_tmp,
            base.join("home"),
        ))
    }

    impl World {
        fn new() -> Self {
            Self::with(|_| {})
        }

        fn with(change: impl FnOnce(&mut Value)) -> Self {
            Self::on_clock(change, FakeClock::at(7_000))
        }

        /// A wallet on this world's workspace, which sees the sandbox applied on the same one. The
        /// sandbox of the process is not touched, so no other test sees this world.
        fn on_clock(change: impl FnOnce(&mut Value), clock: Arc<dyn BudgetClock>) -> Self {
            Self::opened(change, clock, false)
        }

        /// A wallet that reads the sandbox of the process, installed and applied on this world's
        /// workspace. Every test of the process reads that state, so this is for a child process.
        fn installed() -> Self {
            Self::opened(|_| {}, FakeClock::at(7_000), true)
        }

        fn opened(
            change: impl FnOnce(&mut Value),
            clock: Arc<dyn BudgetClock>,
            installed: bool,
        ) -> Self {
            let sandbox = installed.then(SandboxTestState::acquire);
            let root = tempfile::tempdir().unwrap();
            let base = std::fs::canonicalize(root.path()).unwrap();
            let workspace = base.join("workspace");
            std::fs::create_dir_all(&workspace).unwrap();
            let handle = RunBudgetHandle::open(LedgerOpen {
                policy: policy_for(&base, change),
                workspace: &workspace,
                explicit_profile: None,
                entry: LedgerEntry::Fresh,
                clock,
                command_writes: Some(writes_on(&workspace)),
            })
            .unwrap();
            let handle = if installed {
                install(
                    SandboxMode::Required,
                    &workspace,
                    SandboxAvailability::Available,
                );
                handle
            } else {
                handle.with_sandbox_view(SandboxView {
                    applied_on: Some(workspace.clone()),
                })
            };
            Self {
                _sandbox: sandbox,
                _root: root,
                base,
                workspace,
                handle: Arc::new(handle),
            }
        }

        fn ledger_file(&self) -> PathBuf {
            self.base.join("ledgers").join(WALLET).join("ledger.json")
        }

        fn config(&self, url: &str, handle: Option<Arc<RunBudgetHandle>>) -> Config {
            let directory = self.workspace.join(".commandagent");
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(
                directory.join("config.toml"),
                "[preset.budget976]\nopenrouter_provider = { only = [\"u\"], allow_fallbacks = false }\n",
            )
            .unwrap();
            let parsed = crate::provider_cli::parse_from([
                "commandagent",
                "--cwd",
                self.workspace.to_str().unwrap(),
                "--state-dir",
                self.base.join("state").to_str().unwrap(),
                "--model",
                "m-1",
                "--provider",
                "openai-compatible",
                "--base-url",
                url,
                "--preset",
                "budget976",
                "--chat-timeout-secs",
                "30",
            ])
            .unwrap();
            let mut config =
                Config::from_cli_with_provider_options(parsed.cli, parsed.provider_options)
                    .unwrap();
            config.eval_events_path = None;
            config.num_predict = 50;
            config.chat_retries = 0;
            config.field_sources.run_budget = handle;
            config
        }

        /// The client the factory makes, with the effort the policy wants: the factory takes it
        /// from this process's environment, which a test in this process leaves alone.
        fn client_of(&self, config: &Config) -> LmStudioClient {
            self.client_for_role(config, ProviderRole::Executor)
        }

        fn client_for_role(&self, config: &Config, role: ProviderRole) -> LmStudioClient {
            LmStudioClient::from_openai_compatible_env(config, role)
                .unwrap()
                .with_reasoning_effort(Some("low"), role)
        }

        fn client(&self, url: &str) -> LmStudioClient {
            self.client_of(&self.config(url, Some(self.handle.clone())))
        }

        fn sends(&self) -> u64 {
            self.handle.wallet().unwrap().physical_used()
        }

        /// One turn through the entry every send of a run goes through, on a worker of its own
        /// with a clone of the client. What the client holds is the client's: the config here only
        /// gives the call its deadline.
        fn ask(
            &self,
            client: &mut LmStudioClient,
            deadline_secs: u64,
            is_cancelled: impl Fn() -> bool,
        ) -> ProviderCallOutcome {
            let mut config = self.config("http://127.0.0.1:9/v1", None);
            config.chat_timeout_secs = deadline_secs;
            provider_call::chat_with_cancel_and_stream(
                client,
                &config,
                ProviderChatRequest {
                    scope: ProviderCallScope::Executor,
                    model: "m-1",
                    messages: &[ConversationMessage::user("hello-976")],
                    tools: &[],
                    native_tools_enabled: false,
                },
                is_cancelled,
                &mut |_chunk| Ok(()),
            )
        }

        fn say_hello(
            &self,
            client: &mut LmStudioClient,
        ) -> anyhow::Result<crate::providers::AssistantReply> {
            self.ask(client, 30, || false).result
        }

        /// A call that gave up returned while its worker's request is still being dropped. This
        /// waits on what the worker does, not on a length of time; the bound only ends a test that
        /// would otherwise hang.
        fn until_no_send_is_out(&self) {
            let started = std::time::Instant::now();
            while self
                .handle
                .wallet()
                .unwrap()
                .attempts
                .iter()
                .any(|attempt| attempt.state == AttemptState::Started)
            {
                assert!(
                    started.elapsed() < Duration::from_secs(30),
                    "a send stayed out"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }

    /// The bytes a client of this test's shape sends.
    fn wire_length(client: &LmStudioClient) -> u64 {
        let body = openai_chat_completions::build_lm_studio_request_with_effort(
            "m-1",
            &[ConversationMessage::user("hello-976")],
            &[],
            false,
            50,
            "low",
        );
        serde_json::to_vec(&client.route(body).unwrap())
            .unwrap()
            .len() as u64
    }

    /// What the fake counted, the ledger counted, and the file held while the POST was being read.
    #[test]
    fn a_post_is_one_physical_send_started_on_disk_with_the_bytes_the_ledger_has() {
        let world = World::new();
        let file = world.ledger_file();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_by_the_fake = seen.clone();
        let fake = Fake::watching(
            vec![Reply::Status(200, reply_body(KNOWN))],
            move |request| {
                // Another reader, of the disk: the handle's lock and its memory are not involved.
                let envelope: Value =
                    serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
                let attempts = envelope["body"]["attempts"].clone();
                seen_by_the_fake.lock().unwrap().push((
                    attempts[0]["state"].clone(),
                    attempts[0]["body_sha256"].clone(),
                    sha256_hex(&request.body),
                    attempts.as_array().map(Vec::len),
                ));
            },
        );
        let mut client = world.client(&fake.url());
        let reply = world.say_hello(&mut client).unwrap();
        let received = fake.finish();

        assert_eq!(reply.content, "done");
        assert_eq!(received.len() as u64, world.sends());
        let wallet = world.handle.wallet().unwrap();
        assert_eq!(wallet.attempts[0].state, AttemptState::Settled);
        assert_eq!(
            wallet.attempts[0].body_sha256,
            sha256_hex(&received[0].body)
        );
        let seen = seen.lock().unwrap();
        let (state, on_disk, of_the_post, count) = &seen[0];
        assert_eq!(state, &json!("started"));
        assert_eq!(on_disk, &json!(of_the_post));
        assert_eq!(*count, Some(1));
        assert_eq!(world.handle.success_gate(), Ok(()));
    }

    /// A body under a wallet asks for what the policy allows, however much the run would ask for,
    /// and for what the run asks when that is less.
    #[test]
    fn a_body_asks_for_the_smaller_of_the_run_and_the_policy() {
        for (num_predict, sent) in [(8_192, 50), (51, 50), (50, 50), (49, 49), (10, 10)] {
            let world = World::new();
            let fake = Fake::spawn(vec![Reply::Status(200, reply_body(KNOWN))]);
            let mut config = world.config(&fake.url(), Some(world.handle.clone()));
            config.num_predict = num_predict;
            let mut client = world.client_of(&config);
            let outcome = world.say_hello(&mut client);
            let received = fake.finish();
            assert!(outcome.is_ok(), "{num_predict}: {outcome:?}");
            let body: Value = serde_json::from_slice(&received[0].body).unwrap();
            assert_eq!(body["max_tokens"], json!(sent), "{num_predict}");
        }
    }

    /// A retry takes a permit of its own and the send before it left it none. Without a wallet the
    /// same answers are three POSTs, which is what the control counts.
    #[test]
    fn a_retry_is_counted_and_gets_no_second_post_under_a_wallet() {
        let replies = vec![Reply::Status(503, "{}".to_string()); 3];

        let world = World::new();
        let fake = Fake::spawn(replies.clone());
        let mut config = world.config(&fake.url(), Some(world.handle.clone()));
        config.chat_retries = 2;
        let mut client = world.client_of(&config);
        let refused = world.say_hello(&mut client).unwrap_err();
        assert_eq!(budget_refusal(&refused), Some(BudgetRefusal::Latched));
        assert_eq!(fake.finish().len(), 1);
        assert_eq!(world.sends(), 1);

        let fake = Fake::spawn(replies);
        let mut config = world.config(&fake.url(), None);
        config.chat_retries = 2;
        let mut client = world.client_of(&config);
        assert!(world.say_hello(&mut client).is_err());
        assert_eq!(fake.finish().len(), 3, "without a wallet");
    }

    /// Every row counts POSTs with one chat. Exactly at a limit is one, and one short of it none.
    #[test]
    fn each_limit_lets_a_post_out_exactly_at_it_and_none_one_short() {
        let probe = World::new();
        let length = wire_length(&probe.client("http://127.0.0.1:9/v1"));
        drop(probe);
        let tariff = crate::structured_budget::test_support::policy().tariff;
        let amount = reservation_amount(&tariff, length + 4, 50).unwrap();
        let money = |units: u128| format!("0.{units:018}");
        let at_cost = money(amount.units());
        let under_cost = money(amount.units() - 1);
        assert_ne!(amount, Money::ZERO);

        type Row = (&'static str, Box<dyn FnOnce(&mut Value)>, usize);
        let rows: Vec<Row> = vec![
            (
                "cost exactly",
                Box::new(move |v| v["limits"]["cost_usd"] = json!(at_cost)),
                1,
            ),
            (
                "cost one unit short",
                Box::new(move |v| v["limits"]["cost_usd"] = json!(under_cost)),
                0,
            ),
            (
                "cost nothing",
                Box::new(|v| v["limits"]["cost_usd"] = json!("0")),
                0,
            ),
            (
                "tokens exactly",
                Box::new(move |v| v["limits"]["tokens"] = json!(length + 4 + 50)),
                1,
            ),
            (
                "tokens one short",
                Box::new(move |v| v["limits"]["tokens"] = json!(length + 4 + 49)),
                0,
            ),
            (
                "input exactly",
                Box::new(move |v| v["max_input_tokens"] = json!(length + 4)),
                1,
            ),
            (
                "input one short",
                Box::new(move |v| v["max_input_tokens"] = json!(length + 3)),
                0,
            ),
            (
                "physical one",
                Box::new(|v| v["limits"]["physical_sends"] = json!(1)),
                1,
            ),
            (
                "physical none",
                Box::new(|v| v["limits"]["physical_sends"] = json!(0)),
                0,
            ),
            (
                "another route",
                Box::new(|v| v["route"]["model"] = json!("another-976")),
                0,
            ),
        ];
        let mut wrong = Vec::new();
        for (name, change, posts) in rows {
            let world = World::with(change);
            let fake = Fake::spawn(vec![Reply::Status(200, reply_body(KNOWN))]);
            let mut client = world.client(&fake.url());
            let _ = world.say_hello(&mut client);
            let received = fake.finish();
            let sends = world.sends();
            if received.len() != posts || sends != posts as u64 {
                wrong.push(format!("{name}: {} POSTs, {sends} sends", received.len()));
            }
        }
        assert_eq!(wrong, Vec::<String>::new());

        // The physical limit again, past the send it allows.
        let world = World::with(|v| v["limits"]["physical_sends"] = json!(1));
        let fake = Fake::spawn(vec![Reply::Status(200, reply_body(KNOWN)); 2]);
        let mut client = world.client(&fake.url());
        world.say_hello(&mut client).unwrap();
        let refused = world.say_hello(&mut client).unwrap_err();
        assert_eq!(
            budget_refusal(&refused),
            Some(BudgetRefusal::PhysicalExceeded)
        );
        assert_eq!(fake.finish().len(), 1);
    }

    /// Every way a send ends with no known bill. It holds its reservation, the wallet stops, the
    /// next chat sends nothing, and the run is no success. What the caller gets is the row's: a
    /// reply that read well is kept, as a stage's result is, and the wallet still says no.
    #[test]
    fn a_send_with_no_known_bill_is_unknown_and_nothing_goes_out_after_it() {
        use UnknownReason::{
            BodyTruncated, CostMissing, HttpError, ParseFailed, SendError, Timeout,
            UsageInconsistent, UsageMissing,
        };
        let usage = |text: &str| reply_body(text);
        let rows: Vec<(&str, Reply, UnknownReason, bool)> = vec![
            (
                "no usage",
                Reply::Status(
                    200,
                    r#"{"choices":[{"message":{"content":"done"}}]}"#.to_string(),
                ),
                UsageMissing,
                true,
            ),
            (
                "no cost",
                Reply::Status(200, usage(r#"{"prompt_tokens":10,"completion_tokens":5}"#)),
                CostMissing,
                true,
            ),
            (
                "a cost in text",
                Reply::Status(
                    200,
                    usage(r#"{"prompt_tokens":10,"completion_tokens":5,"cost":"0.1"}"#),
                ),
                CostMissing,
                true,
            ),
            (
                "no prompt tokens field",
                Reply::Status(200, usage(r#"{"completion_tokens":5,"cost":0}"#)),
                UsageMissing,
                true,
            ),
            (
                "prompt tokens in text",
                Reply::Status(
                    200,
                    usage(r#"{"prompt_tokens":"10","completion_tokens":5,"cost":0}"#),
                ),
                UsageInconsistent,
                true,
            ),
            (
                "no prompt tokens for a body",
                Reply::Status(
                    200,
                    usage(r#"{"prompt_tokens":0,"completion_tokens":0,"cost":0}"#),
                ),
                UsageInconsistent,
                true,
            ),
            (
                "not JSON",
                Reply::Status(200, "not json 976".to_string()),
                ParseFailed,
                false,
            ),
            (
                "JSON cut short with a length that says so",
                Reply::Status(200, r#"{"choices":["#.to_string()),
                ParseFailed,
                false,
            ),
            (
                "cut on the wire",
                Reply::Cut(reply_body(KNOWN)),
                BodyTruncated,
                false,
            ),
            (
                "400",
                Reply::Status(400, reply_body(KNOWN)),
                HttpError,
                false,
            ),
            (
                "429",
                Reply::Status(429, "{}".to_string()),
                HttpError,
                false,
            ),
            (
                "500",
                Reply::Status(500, "{}".to_string()),
                HttpError,
                false,
            ),
            (
                "a redirect",
                Reply::Redirect(307, "http://127.0.0.1:9/v1/chat/completions".to_string()),
                HttpError,
                false,
            ),
            ("closed with no answer", Reply::Close, SendError, false),
            ("no answer in time", Reply::Hang, Timeout, false),
        ];
        let mut wrong = Vec::new();
        for (name, reply, reason, reply_kept) in rows {
            let world = World::new();
            let fake = Fake::spawn(vec![reply, Reply::Status(200, reply_body(KNOWN))]);
            let mut config = world.config(&fake.url(), Some(world.handle.clone()));
            config.chat_timeout_secs = 1;
            let mut client = world.client_of(&config);
            let first = world.say_hello(&mut client);
            let second = world.say_hello(&mut client);
            let posts = fake.finish().len();
            let wallet = world.handle.wallet().unwrap();
            let attempt = wallet.attempts.first();
            let as_expected = first.is_ok() == reply_kept
                && posts == 1
                && wallet.physical_used() == 1
                && attempt.map(|attempt| attempt.state) == Some(AttemptState::Unknown)
                && attempt.and_then(|attempt| attempt.unknown_reason) == Some(reason)
                && wallet.latch().map(|latch| latch.reason) == Some(LatchReason::UnknownBilling)
                && wallet.held().unwrap()
                    == attempt.map_or(Money::ZERO, |attempt| attempt.reserved)
                && wallet.held().unwrap() > Money::ZERO
                && wallet.spent().unwrap() == Money::ZERO
                && second.as_ref().err().and_then(budget_refusal) == Some(BudgetRefusal::Latched)
                && world.handle.success_gate().is_err();
            if !as_expected {
                wrong.push(format!(
                    "{name}: first ok {}, {posts} POSTs, {:?}, {:?}, latch {:?}, second {:?}",
                    first.is_ok(),
                    attempt.map(|attempt| attempt.state),
                    attempt.and_then(|attempt| attempt.unknown_reason),
                    wallet.latch().map(|latch| latch.reason),
                    second.as_ref().map(|_| ()).map_err(ToString::to_string),
                ));
            }
        }
        assert_eq!(wrong, Vec::<String>::new());
    }

    /// Nobody listens: the send was still started, so it is unknown, on the same terms.
    #[test]
    fn a_send_error_with_nobody_listening_is_unknown() {
        let world = World::new();
        let address = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        let mut client = world.client(&format!("http://{address}/v1"));
        assert!(world.say_hello(&mut client).is_err());
        let wallet = world.handle.wallet().unwrap();
        assert_eq!(
            wallet.attempts[0].unknown_reason,
            Some(UnknownReason::SendError)
        );
        assert_eq!(
            wallet.latch().map(|latch| latch.reason),
            Some(LatchReason::UnknownBilling)
        );
    }

    /// The cancel fires with the whole request read and no answer sent.
    #[test]
    fn a_cancel_of_a_send_in_flight_is_unknown() {
        let world = World::new();
        let cancelled = Arc::new(AtomicBool::new(false));
        let fired = cancelled.clone();
        let fake = Fake::watching(vec![Reply::Hang], move |_| {
            fired.store(true, Ordering::SeqCst);
        });
        let mut client = world.client(&fake.url());
        let outcome = world.ask(&mut client, 30, || cancelled.load(Ordering::SeqCst));
        assert!(outcome.aborted_by_user);
        assert!(outcome.result.is_err());
        world.until_no_send_is_out();
        assert_eq!(fake.finish().len(), 1);
        let wallet = world.handle.wallet().unwrap();
        assert_eq!(
            wallet.attempts[0].unknown_reason,
            Some(UnknownReason::Cancelled)
        );
        assert_eq!(
            wallet.latch().map(|latch| latch.reason),
            Some(LatchReason::UnknownBilling)
        );
    }

    /// The call's own deadline comes first, and its end cancels the worker's request, so the send
    /// is unknown as a cancelled one. The table above has the client's timeout coming first.
    #[test]
    fn a_call_past_its_deadline_leaves_the_send_unknown_and_nothing_goes_out_after_it() {
        let world = World::new();
        let fake = Fake::spawn(vec![Reply::Hang, Reply::Status(200, reply_body(KNOWN))]);
        let mut client = world.client(&fake.url());
        let outcome = world.ask(&mut client, 1, || false);
        assert!(outcome.timed_out);
        assert!(outcome.result.is_err());
        world.until_no_send_is_out();
        let refused = world.say_hello(&mut client).unwrap_err();
        assert_eq!(budget_refusal(&refused), Some(BudgetRefusal::Latched));
        assert_eq!(fake.finish().len(), 1);
        let wallet = world.handle.wallet().unwrap();
        assert_eq!(wallet.physical_used(), 1);
        assert_eq!(
            wallet.attempts[0].unknown_reason,
            Some(UnknownReason::Cancelled)
        );
    }

    /// The body would go to another URL, as another send the wallet does not know of. The control
    /// is the shared client, which follows, to show the target would have been reached.
    #[test]
    fn a_redirect_is_not_followed_and_is_unknown() {
        for status in [307, 308] {
            let world = World::new();
            let target = Fake::spawn(vec![Reply::Status(200, reply_body(KNOWN))]);
            let location = format!("{}/chat/completions", target.url());
            let first = Fake::spawn(vec![Reply::Redirect(status, location)]);
            let mut client = world.client(&first.url());
            let outcome = world.say_hello(&mut client);
            assert_eq!(first.finish().len(), 1, "{status}");
            assert_eq!(target.finish().len(), 0, "{status}");
            assert!(outcome.is_err(), "{status}");
            assert_eq!(world.sends(), 1, "{status}");
            assert_eq!(
                world.handle.wallet().unwrap().attempts[0].unknown_reason,
                Some(UnknownReason::HttpError),
                "{status}"
            );

            let target = Fake::spawn(vec![Reply::Status(200, reply_body(KNOWN))]);
            let location = format!("{}/chat/completions", target.url());
            let first = Fake::spawn(vec![Reply::Redirect(status, location)]);
            let call = HttpCall::new(&CancelToken::default()).unwrap();
            let request = http_client(5)
                .unwrap()
                .post(format!("{}/chat/completions", first.url()))
                .body("control-976");
            let response = call.run(async { request.send().await }).unwrap().unwrap();
            assert!(response.status().is_success(), "{status}");
            assert_eq!(first.finish().len(), 1, "{status}");
            assert_eq!(target.finish()[0].body, b"control-976", "{status}");
        }
    }

    /// What is consumed is the larger, and what was billed stays what was billed.
    #[test]
    fn a_known_report_consumes_the_larger_and_keeps_what_was_billed() {
        // 10 and 5 tokens at 2 and 10 per million.
        let computed = Money::parse_decimal_up("0.00007").unwrap();
        for (cost, billed, consumed) in [
            ("0", "0", computed),
            (
                "0.0001",
                "0.0001",
                Money::parse_decimal_up("0.0001").unwrap(),
            ),
        ] {
            let world = World::new();
            let usage = format!(r#"{{"prompt_tokens":10,"completion_tokens":5,"cost":{cost}}}"#);
            let fake = Fake::spawn(vec![Reply::Status(200, reply_body(&usage))]);
            let mut client = world.client(&fake.url());
            world.say_hello(&mut client).unwrap();
            fake.finish();
            let wallet = world.handle.wallet().unwrap();
            let record = wallet.attempts[0].settlement.as_ref().unwrap();
            assert_eq!(record.billed_usd, billed);
            assert_eq!(record.consumed, consumed, "{cost}");
            assert_eq!(record.computed, computed);
            assert!(wallet.latch().is_none());
        }
    }

    /// A known amount past the reservation is settled at its value, stops the wallet, and leaves the
    /// send no longer unresolved.
    #[test]
    fn a_known_amount_past_the_reservation_is_settled_and_stops_the_wallet() {
        let world = World::new();
        let usage = r#"{"prompt_tokens":10,"completion_tokens":5,"cost":5}"#;
        let fake = Fake::spawn(vec![Reply::Status(200, reply_body(usage))]);
        let mut client = world.client(&fake.url());
        world.say_hello(&mut client).unwrap();
        fake.finish();
        let wallet = world.handle.wallet().unwrap();
        assert_eq!(wallet.attempts[0].state, AttemptState::Settled);
        assert_eq!(
            wallet.latch().map(|latch| latch.reason),
            Some(LatchReason::OverReservation)
        );
        assert_eq!(
            wallet.spent().unwrap(),
            Money::parse_decimal_up("5").unwrap()
        );
    }

    /// A response read after its send went unknown is settled once at its cost. The latch stays, a
    /// second report is refused, and one that says nothing known changes nothing.
    #[test]
    fn a_late_response_is_settled_once_and_the_latch_stays() {
        let world = World::new();
        let fake = Fake::spawn(vec![Reply::Status(500, "{}".to_string())]);
        let mut client = world.client(&fake.url());
        assert!(world.say_hello(&mut client).is_err());
        fake.finish();
        let physical = world.handle.wallet().unwrap().attempts[0].physical;

        assert_eq!(
            world.handle.settle_response(physical, "not json 976"),
            Ok(None)
        );
        let wallet = world.handle.wallet().unwrap();
        assert_eq!(wallet.attempts[0].state, AttemptState::Unknown);

        let report = world.handle.settle_response(physical, &reply_body(KNOWN));
        assert!(
            matches!(report, Ok(Some(SettleReport::Settled(_)))),
            "{report:?}"
        );
        let wallet = world.handle.wallet().unwrap();
        assert!(wallet.attempts[0].settlement.as_ref().unwrap().late);
        assert_eq!(
            wallet.latch().map(|latch| latch.reason),
            Some(LatchReason::UnknownBilling)
        );
        assert!(world.handle.success_gate().is_err());
        assert!(matches!(
            world.handle.settle_response(physical, &reply_body(KNOWN)),
            Err(SendRefusal::Wallet(_))
        ));
    }

    /// A count of `null` is a count that is not there, as a provider sends it for a detail it does
    /// not have. Without a detail the send is settled; without one of the two counts it is not.
    /// The first row is the control, with the detail there.
    #[test]
    fn a_count_of_null_is_a_count_that_is_not_there() {
        type Row = (
            &'static str,
            &'static str,
            AttemptState,
            Option<UnknownReason>,
            Option<u64>,
        );
        let rows: [Row; 5] = [
            (
                "cached tokens there",
                r#"{"prompt_tokens":10,"completion_tokens":5,"cost":0.00001,"prompt_tokens_details":{"cached_tokens":3}}"#,
                AttemptState::Settled,
                None,
                Some(3),
            ),
            (
                "cached tokens null",
                r#"{"prompt_tokens":10,"completion_tokens":5,"cost":0.00001,"prompt_tokens_details":{"cached_tokens":null}}"#,
                AttemptState::Settled,
                None,
                None,
            ),
            (
                "reasoning tokens null",
                r#"{"prompt_tokens":10,"completion_tokens":5,"cost":0.00001,"completion_tokens_details":{"reasoning_tokens":null}}"#,
                AttemptState::Settled,
                None,
                None,
            ),
            (
                "prompt tokens null",
                r#"{"prompt_tokens":null,"completion_tokens":5,"cost":0.00001}"#,
                AttemptState::Unknown,
                Some(UnknownReason::UsageMissing),
                None,
            ),
            (
                "completion tokens null",
                r#"{"prompt_tokens":10,"completion_tokens":null,"cost":0.00001}"#,
                AttemptState::Unknown,
                Some(UnknownReason::UsageMissing),
                None,
            ),
        ];
        let mut wrong = Vec::new();
        for (name, usage, state, reason, cached) in rows {
            let world = World::new();
            let fake = Fake::spawn(vec![Reply::Status(200, reply_body(usage))]);
            let mut client = world.client(&fake.url());
            let _ = world.say_hello(&mut client);
            let posts = fake.finish().len();
            let wallet = world.handle.wallet().unwrap();
            let attempt = &wallet.attempts[0];
            let got_cached = attempt
                .settlement
                .as_ref()
                .and_then(|record| record.cached_tokens);
            if posts != 1
                || attempt.state != state
                || attempt.unknown_reason != reason
                || got_cached != cached
            {
                wrong.push(format!(
                    "{name}: {posts} POSTs, {:?}, {:?}, cached {got_cached:?}",
                    attempt.state, attempt.unknown_reason
                ));
            }
        }
        assert_eq!(wrong, Vec::<String>::new());
    }

    /// The purpose on the record is the role of the client that sent, as the file has it.
    #[test]
    fn a_send_is_on_the_record_for_the_role_of_its_client() {
        let mut wrong = Vec::new();
        for (role, purpose) in [
            (ProviderRole::Executor, "executor"),
            (ProviderRole::Planner, "planner"),
            (ProviderRole::Classifier, "classifier"),
        ] {
            let world = World::new();
            let fake = Fake::spawn(vec![Reply::Status(200, reply_body(KNOWN))]);
            let config = world.config(&fake.url(), Some(world.handle.clone()));
            let mut client = world.client_for_role(&config, role);
            let outcome = world.say_hello(&mut client);
            let posts = fake.finish().len();
            let envelope: Value =
                serde_json::from_slice(&std::fs::read(world.ledger_file()).unwrap()).unwrap();
            let on_disk = envelope["body"]["attempts"][0]["purpose"].clone();
            if outcome.is_err() || posts != 1 || on_disk != json!(purpose) {
                wrong.push(format!(
                    "{role:?}: ok {}, {posts} POSTs, {on_disk}",
                    outcome.is_ok()
                ));
            }
        }
        assert_eq!(wrong, Vec::<String>::new());
    }

    /// The file is taken from under the wallet while the response is on its way, so the settling
    /// cannot be written: the response is not taken, the next chat sends nothing, and the run is
    /// no success. The control reads the file and leaves it be.
    #[test]
    fn a_response_whose_settling_cannot_reach_the_disk_is_not_taken() {
        const WRITTEN_OVER: &[u8] = b"written-over-976";
        type Change = fn(&Path);
        let rows: [(&str, Change, Option<LedgerRefusal>); 3] = [
            (
                "left alone",
                |file| {
                    std::fs::read(file).unwrap();
                },
                None,
            ),
            (
                "removed",
                |file| std::fs::remove_file(file).unwrap(),
                Some(LedgerRefusal::Vanished),
            ),
            (
                "written over",
                |file| std::fs::write(file, WRITTEN_OVER).unwrap(),
                Some(LedgerRefusal::Diverged),
            ),
        ];
        let mut wrong = Vec::new();
        for (name, change, refusal) in rows {
            let world = World::new();
            let file = world.ledger_file();
            let changed = file.clone();
            let fake = Fake::watching(vec![Reply::Status(200, reply_body(KNOWN)); 2], move |_| {
                change(&changed);
            });
            let mut client = world.client(&fake.url());
            let first = world.say_hello(&mut client);
            let second = world.say_hello(&mut client);
            let posts = fake.finish().len();
            let refusal_of = |outcome: &anyhow::Result<crate::providers::AssistantReply>| {
                outcome
                    .as_ref()
                    .err()
                    .and_then(|error| error.downcast_ref::<SendRefusal>())
                    .copied()
            };
            let wallet = world.handle.wallet().unwrap();
            let attempt = &wallet.attempts[0];
            let gate = world.handle.success_gate();
            let on_disk = std::fs::read(&file).ok();
            let as_expected = match refusal {
                None => {
                    first.is_ok()
                        && second.is_ok()
                        && posts == 2
                        && attempt.state == AttemptState::Settled
                        && gate == Ok(())
                }
                Some(refusal) => {
                    let wallet_refusal = SendRefusal::Wallet(LedgerFailure::Ledger(refusal));
                    refusal_of(&first) == Some(wallet_refusal)
                        && refusal_of(&second) == Some(wallet_refusal)
                        && posts == 1
                        && wallet.attempts.len() == 1
                        && attempt.state == AttemptState::Started
                        && attempt.unknown_reason.is_none()
                        && gate == Err(wallet_refusal)
                        && on_disk.as_deref()
                            == (refusal == LedgerRefusal::Diverged).then_some(WRITTEN_OVER)
                }
            };
            if !as_expected {
                wrong.push(format!(
                    "{name}: first {:?}, second {:?}, {posts} POSTs, {:?}, {:?}, gate {gate:?}, file there {}",
                    refusal_of(&first),
                    refusal_of(&second),
                    attempt.state,
                    attempt.unknown_reason,
                    on_disk.is_some()
                ));
            }
        }
        assert_eq!(wrong, Vec::<String>::new());
    }

    /// Moves on by a second at every reading, so where the time runs out is set by how many times
    /// it has been read and by nothing that takes time.
    #[derive(Debug)]
    struct ASecondPerReading(AtomicU64);

    impl ASecondPerReading {
        fn next(&self) -> u64 {
            7_000 + 1_000 * self.0.fetch_add(1, Ordering::SeqCst)
        }
    }

    impl BudgetClock for ASecondPerReading {
        fn wall_ms(&self) -> u64 {
            self.next()
        }

        fn monotonic_ms(&self) -> u64 {
            self.next()
        }
    }

    /// The reservation reads the clock twice to start the run, the start twice more and the next
    /// change twice again, so the start sees 3 seconds and the settling 5. Under 3 the time runs
    /// out between the reservation and the start: nothing goes out and the reservation goes back.
    /// From 3 it runs out after the start: the response is settled at its cost and is not a
    /// success. At 5 it does not run out.
    #[test]
    fn the_deadline_between_the_reservation_and_the_start_sends_nothing_and_holds_nothing() {
        let mut wrong = Vec::new();
        for wall_seconds in 1..=5 {
            let world = World::on_clock(
                |v| v["limits"]["wall_seconds"] = json!(wall_seconds),
                Arc::new(ASecondPerReading(AtomicU64::new(0))),
            );
            let fake = Fake::spawn(vec![Reply::Status(200, reply_body(KNOWN))]);
            let mut client = world.client(&fake.url());
            let outcome = world.say_hello(&mut client);
            let posts = fake.finish().len();
            let wallet = world.handle.wallet().unwrap();
            let attempt = &wallet.attempts[0];
            let latch = wallet.latch().map(|latch| latch.reason);
            let held = wallet.held().unwrap();
            let as_expected = match wall_seconds {
                1 | 2 => {
                    outcome.as_ref().err().and_then(budget_refusal)
                        == Some(BudgetRefusal::WallExceeded)
                        && posts == 0
                        && attempt.state == AttemptState::Released
                        && held == Money::ZERO
                        && wallet.physical_used() == 0
                        && latch.is_none()
                }
                3 | 4 => {
                    outcome.is_ok()
                        && posts == 1
                        && attempt.state == AttemptState::Settled
                        && latch == Some(LatchReason::ResponseAfterDeadline)
                        && held == Money::ZERO
                }
                _ => {
                    outcome.is_ok()
                        && posts == 1
                        && attempt.state == AttemptState::Settled
                        && latch.is_none()
                        && world.handle.success_gate() == Ok(())
                }
            };
            if !as_expected {
                wrong.push(format!(
                    "{wall_seconds} s: {:?}, {posts} POSTs, {:?}, latch {latch:?}, held {held:?}, {} sends",
                    outcome.as_ref().err().and_then(budget_refusal),
                    attempt.state,
                    wallet.physical_used()
                ));
            }
        }
        assert_eq!(wrong, Vec::<String>::new());
    }

    /// Off, with no sandbox installed at all: the client sends what `RequestBuilder::json` sends,
    /// byte for byte and header for header, and a body under a wallet is the same again. Asked for
    /// more output than the policy allows, a body under a wallet asks for the policy's limit, and
    /// that one count is all that differs.
    #[test]
    fn with_no_wallet_the_wire_is_what_it_was_and_a_wallet_changes_only_a_count_past_its_limit() {
        let world = World::new();
        let fake = Fake::spawn(vec![Reply::Status(200, reply_body(KNOWN)); 5]);
        let asking_for_more = |handle: Option<Arc<RunBudgetHandle>>| {
            let mut config = world.config(&fake.url(), handle);
            config.num_predict = 8_192;
            world.client_of(&config)
        };

        let mut wallet_client = world.client(&fake.url());
        world.say_hello(&mut wallet_client).unwrap();
        let mut wallet_past = asking_for_more(Some(world.handle.clone()));
        world.say_hello(&mut wallet_past).unwrap();

        // The wallet above read only its own view. Off reads nothing, and the state of the
        // process stays empty while it sends.
        let sandbox = SandboxTestState::acquire();
        os_sandbox::reset_for_test();
        let config = world.config(&fake.url(), None);
        let mut off = world.client_of(&config);
        world.say_hello(&mut off).unwrap();
        let mut off_past = asking_for_more(None);
        world.say_hello(&mut off_past).unwrap();
        assert!(os_sandbox::status().is_none(), "nothing was installed");
        drop(sandbox);

        let body = off
            .route(
                openai_chat_completions::build_lm_studio_request_with_effort(
                    "m-1",
                    &[ConversationMessage::user("hello-976")],
                    &[],
                    false,
                    50,
                    "low",
                ),
            )
            .unwrap();
        let call = HttpCall::new(&CancelToken::default()).unwrap();
        let request = http_client(30)
            .unwrap()
            .post(format!("{}/chat/completions", fake.url()))
            .json(&body);
        call.run(async { request.send().await }).unwrap().unwrap();

        let received = fake.finish();
        assert_eq!(received.len(), 5);
        let (under, under_past, off_wire, off_past, plain) = (
            &received[0],
            &received[1],
            &received[2],
            &received[3],
            &received[4],
        );
        assert_eq!(off_wire.body, serde_json::to_vec(&body).unwrap());
        assert_eq!(off_wire.body, plain.body);
        assert_eq!(off_wire.headers(), plain.headers());
        assert_eq!(off_wire.head.lines().next(), plain.head.lines().next());
        assert_eq!(under.body, off_wire.body);
        assert_eq!(under.headers(), off_wire.headers());
        assert_eq!(under.head.lines().next(), off_wire.head.lines().next());

        let off_past = String::from_utf8(off_past.body.clone()).unwrap();
        assert_eq!(off_past.matches("\"max_tokens\":8192").count(), 1);
        assert_eq!(
            String::from_utf8(under_past.body.clone()).unwrap(),
            off_past.replacen("\"max_tokens\":8192", "\"max_tokens\":50", 1)
        );
        assert_eq!(under_past.body, under.body);
        assert_eq!(under_past.headers(), under.headers());
    }

    const SANDBOX_APPLIED_CHILD: &str = "COMMANDAGENT_TEST_BUDGET_SANDBOX_APPLIED_CHILD";
    const SANDBOX_ELSEWHERE_CHILD: &str = "COMMANDAGENT_TEST_BUDGET_SANDBOX_ELSEWHERE_CHILD";

    /// Runs `child`, a test of this module, alone in a process of its own with an empty
    /// environment but its marker, and waits for it. The sandbox it installs is that process's.
    /// `ran` is the line the child prints once its checks are all through, so a child that read
    /// no marker and returned at once does not pass for one that ran.
    fn run_alone(child: &str, marker: &str, ran: &str) {
        let home = tempfile::tempdir().unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                &format!("providers::lm_studio::run_budget_tests::{child}"),
                "--nocapture",
                "--test-threads=1",
            ])
            .env_clear()
            .env("HOME", home.path())
            .env(marker, "marker-976-sandbox-child")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stdout}\n{stderr}");
        assert!(stdout.contains("1 passed"), "{stdout}\n{stderr}");
        assert!(stdout.contains(ran), "{ran} missing\n{stdout}\n{stderr}");
    }

    const SANDBOX_APPLIED_RAN: &str = "child-976-ran a_post_needs_the_sandbox_applied";
    const SANDBOX_ELSEWHERE_RAN: &str = "child-976-ran a_wallet_checked_against_another_workspace";
    const FACTORY_RAN: &str = "child-976-ran the_factory_gives_a_client";

    /// A wallet's check is only worth what the sandbox holds, and only where the wallet was checked.
    /// The sandbox is the process's own, installed for real, so the rows run in a child process.
    #[test]
    fn a_post_needs_the_sandbox_applied_on_the_workspace_of_the_wallet() {
        run_alone(
            "a_post_needs_the_sandbox_applied_on_the_workspace_of_the_wallet_child",
            SANDBOX_APPLIED_CHILD,
            SANDBOX_APPLIED_RAN,
        );
    }

    /// Runs only as the child of the test above. Every row is one chat, counted by the fake, and the
    /// file does not move when none goes out.
    #[test]
    fn a_post_needs_the_sandbox_applied_on_the_workspace_of_the_wallet_child() {
        if std::env::var_os(SANDBOX_APPLIED_CHILD).is_none() {
            return;
        }
        const UNAVAILABLE: &str = "backend-unavailable-976";
        type Row = (&'static str, fn(&World), Option<SendRefusal>);
        let rows: Vec<Row> = vec![
            ("applied, on it", |_| {}, None),
            (
                "applied, on it by a link",
                |world| {
                    let alias = world.base.join("alias");
                    std::os::unix::fs::symlink(&world.workspace, &alias).unwrap();
                    install(
                        SandboxMode::Required,
                        &alias,
                        SandboxAvailability::Available,
                    );
                },
                None,
            ),
            (
                "auto and applied",
                |world| {
                    install(
                        SandboxMode::Auto,
                        &world.workspace,
                        SandboxAvailability::Available,
                    );
                },
                None,
            ),
            (
                "applied, on another workspace",
                |world| {
                    let other = world.base.join("other");
                    std::fs::create_dir_all(&other).unwrap();
                    install(
                        SandboxMode::Required,
                        &other,
                        SandboxAvailability::Available,
                    );
                },
                Some(SendRefusal::SandboxOnAnotherWorkspace),
            ),
            (
                "applied, on a child of the workspace",
                |world| {
                    let child = world.workspace.join("child");
                    std::fs::create_dir_all(&child).unwrap();
                    install(
                        SandboxMode::Required,
                        &child,
                        SandboxAvailability::Available,
                    );
                },
                Some(SendRefusal::SandboxOnAnotherWorkspace),
            ),
            (
                "not installed",
                |_| os_sandbox::reset_for_test(),
                Some(SendRefusal::SandboxNotApplied),
            ),
            (
                "auto with no backend",
                |world| {
                    install(
                        SandboxMode::Auto,
                        &world.workspace,
                        SandboxAvailability::Unavailable(UNAVAILABLE.to_string()),
                    );
                },
                Some(SendRefusal::SandboxNotApplied),
            ),
            (
                "required with no backend",
                |world| {
                    install(
                        SandboxMode::Required,
                        &world.workspace,
                        SandboxAvailability::Unavailable(UNAVAILABLE.to_string()),
                    );
                },
                Some(SendRefusal::SandboxNotApplied),
            ),
            (
                "off",
                |world| {
                    install(
                        SandboxMode::Off,
                        &world.workspace,
                        SandboxAvailability::Unavailable(UNAVAILABLE.to_string()),
                    );
                },
                Some(SendRefusal::SandboxNotApplied),
            ),
        ];
        let mut wrong = Vec::new();
        for (name, arrange, refusal) in rows {
            let world = World::installed();
            let fake = Fake::spawn(vec![Reply::Status(200, reply_body(KNOWN))]);
            let mut client = world.client(&fake.url());
            arrange(&world);
            let before = std::fs::read(world.ledger_file()).unwrap();
            let outcome = world.say_hello(&mut client);
            let posts = fake.finish().len();
            let got = outcome
                .as_ref()
                .err()
                .and_then(|error| error.downcast_ref::<SendRefusal>())
                .copied();
            let unmoved = std::fs::read(world.ledger_file()).unwrap() == before;
            let expected_posts = usize::from(refusal.is_none());
            if got != refusal
                || posts != expected_posts
                || world.sends() != expected_posts as u64
                || unmoved == refusal.is_none()
            {
                wrong.push(format!(
                    "{name}: {posts} POSTs, {got:?}, file unmoved {unmoved}"
                ));
            }
        }
        assert_eq!(wrong, Vec::<String>::new());
        println!("{SANDBOX_APPLIED_RAN}");
    }

    /// The same the other way round: the sandbox stays and the wallet is the one checked elsewhere.
    /// The sandbox is the process's own, so this runs in a child process too.
    #[test]
    fn a_wallet_checked_against_another_workspace_sends_nothing() {
        run_alone(
            "a_wallet_checked_against_another_workspace_sends_nothing_child",
            SANDBOX_ELSEWHERE_CHILD,
            SANDBOX_ELSEWHERE_RAN,
        );
    }

    /// Runs only as the child of the test above.
    #[test]
    fn a_wallet_checked_against_another_workspace_sends_nothing_child() {
        if std::env::var_os(SANDBOX_ELSEWHERE_CHILD).is_none() {
            return;
        }
        let world = World::installed();
        let other = world.base.join("other-workspace");
        std::fs::create_dir_all(&other).unwrap();
        let mut value = policy_value();
        value["ledger_dir"] = json!(world.base.join("other-ledgers").to_str().unwrap());
        let policy = RunBudgetPolicy::decode(value.to_string().as_bytes(), WALLET).unwrap();
        let elsewhere = RunBudgetHandle::open(LedgerOpen {
            policy: Arc::new(policy),
            workspace: &other,
            explicit_profile: None,
            entry: LedgerEntry::Fresh,
            clock: FakeClock::at(7_000),
            command_writes: Some(writes_on(&other)),
        })
        .unwrap();
        let elsewhere = Arc::new(elsewhere);
        let fake = Fake::spawn(vec![Reply::Status(200, reply_body(KNOWN))]);
        let mut client = world.client_of(&world.config(&fake.url(), Some(elsewhere.clone())));
        let error = world.say_hello(&mut client).unwrap_err();
        assert_eq!(fake.finish().len(), 0);
        assert_eq!(
            error.downcast_ref::<SendRefusal>(),
            Some(&SendRefusal::SandboxOnAnotherWorkspace)
        );
        assert_eq!(elsewhere.wallet().unwrap().physical_used(), 0);
        println!("{SANDBOX_ELSEWHERE_RAN}");
    }

    #[test]
    fn two_handles_are_equal_only_when_they_are_one_handle_and_a_clone_of_a_config_carries_it() {
        let world = World::new();
        let config = world.config("http://127.0.0.1:9/v1", Some(world.handle.clone()));
        let clone = config.clone();
        assert_eq!(clone.field_sources, config.field_sources);
        assert!(Arc::ptr_eq(
            clone.field_sources.run_budget.as_ref().unwrap(),
            &world.handle
        ));

        let mut value = policy_value();
        value["ledger_dir"] = json!(world.base.join("other-ledgers").to_str().unwrap());
        let policy = RunBudgetPolicy::decode(value.to_string().as_bytes(), WALLET).unwrap();
        let other = RunBudgetHandle::open(LedgerOpen {
            policy: Arc::new(policy),
            workspace: &world.workspace,
            explicit_profile: None,
            entry: LedgerEntry::Fresh,
            clock: FakeClock::at(7_000),
            command_writes: Some(writes_on(&world.workspace)),
        })
        .unwrap();
        assert_ne!(*world.handle, other, "two wallets with nothing in them yet");
        let none = world.config("http://127.0.0.1:9/v1", None);
        assert_ne!(none.field_sources, config.field_sources);
    }

    const FACTORY_CHILD: &str = "COMMANDAGENT_TEST_BUDGET_FACTORY_CHILD";

    /// Runs only as the child of the test below, whose environment has the effort the factory reads.
    #[test]
    fn the_factory_gives_a_client_that_counts_its_send_child() {
        if std::env::var_os(FACTORY_CHILD).is_none() {
            return;
        }
        let world = World::new();
        let fake = Fake::spawn(vec![Reply::Status(200, reply_body(KNOWN))]);
        let mut config = world.config(&fake.url(), Some(world.handle.clone()));
        config.num_predict = 8_192;
        let mut client = crate::providers::client_from_config(&config, false).unwrap();
        provider_call::chat_with_cancel_and_stream(
            client.as_mut(),
            &config,
            ProviderChatRequest {
                scope: ProviderCallScope::Executor,
                model: "m-1",
                messages: &[ConversationMessage::user("hello-976")],
                tools: &[],
                native_tools_enabled: false,
            },
            || false,
            &mut |_chunk| Ok(()),
        )
        .result
        .unwrap();
        assert_eq!(fake.finish().len(), 1);
        assert_eq!(world.sends(), 1);
        assert_eq!(world.handle.success_gate(), Ok(()));
        println!("{FACTORY_RAN}");
    }

    /// The whole way through the factory, with nothing set by hand on the client it gives.
    #[test]
    fn the_factory_gives_a_client_that_counts_its_send() {
        let home = tempfile::tempdir().unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "providers::lm_studio::run_budget_tests::the_factory_gives_a_client_that_counts_its_send_child",
                "--nocapture",
                "--test-threads=1",
            ])
            .env_clear()
            .env("HOME", home.path())
            .env(FACTORY_CHILD, "1")
            .env(
                crate::providers::compatible_effort::COMPATIBLE_REASONING_EFFORT_ENV,
                "low",
            )
            .env(crate::structured_budget::admission::RUN_BUDGET_ENV, WALLET)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stdout}\n{stderr}");
        assert!(stdout.contains("1 passed"), "{stdout}\n{stderr}");
        assert!(
            stdout.contains(FACTORY_RAN),
            "{FACTORY_RAN} missing\n{stdout}\n{stderr}"
        );
    }
}

#[cfg(test)]
mod cancel_tests {
    use super::*;
    use crate::providers::streaming::cancel_test_support::assert_cancel_closes_connection;

    #[test]
    fn cancel_closes_the_chat_request() {
        assert_cancel_closes_connection(None, |base_url, token| {
            let mut client =
                LmStudioClient::new(base_url, None, 30, 64, 0, OpenAiApi::ChatCompletions, None)?;
            client.set_cancel_token(token);
            ChatClient::chat(
                &mut client,
                "lm-test",
                &[ConversationMessage::user("hi")],
                &[],
                false,
            )
        });
    }
}

/// Issue #947: an OpenAI-compatible Chat Completions `usage.cost` reaches the
/// `provider_response` and `provider_turn_duration` events of the same turn as
/// `provider_billed_cost_usd`, on the planner and the executor paths. A
/// loopback stand-in without credentials replays the synthetic corpus
/// `tests/corpus/apps/compatible-billed-cost/`; no case reaches a real
/// provider.
#[cfg(test)]
mod billed_cost_telemetry_tests {
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpListener, TcpStream};
    use std::path::Path;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread::JoinHandle;
    use std::time::Duration;

    use serde_json::{Value, json};

    use crate::config::Config;
    use crate::provider_call::{self, ProviderCallScope, ProviderChatRequest};
    use crate::providers::{ChatClient, client_from_config};
    use crate::state::ConversationMessage;

    const FIXTURE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/corpus/apps/compatible-billed-cost/fixtures/events.json"
    );
    const MODEL: &str = "compatible-model-947";

    fn corpus() -> Value {
        serde_json::from_str(&std::fs::read_to_string(FIXTURE).unwrap()).unwrap()
    }

    /// The raw response body text. `cost_token` is appended to the usage
    /// object as raw JSON text, so a number such as `1e-400` reaches the client
    /// exactly as written, never through an `f64` in the test.
    fn response_body(usage: &Value, cost_token: Option<&str>) -> String {
        let usage = match (usage, cost_token) {
            (Value::Null, None) => String::new(),
            (Value::Object(_), None) => format!(r#","usage":{usage}"#),
            (Value::Object(fields), Some(token)) if !fields.is_empty() => {
                let text = usage.to_string();
                let open = &text[..text.len() - 1];
                format!(r#","usage":{open},"cost":{token}}}"#)
            }
            other => panic!("unsupported usage {other:?} with cost {cost_token:?}"),
        };
        format!(
            r#"{{"choices":[{{"finish_reason":"stop","index":0,"message":{{"content":"done"}}}}],"created":1790000000,"id":"chatcmpl-947","model":"{MODEL}","object":"chat.completion"{usage}}}"#
        )
    }

    fn tokens_usage() -> Value {
        json!({"prompt_tokens": 12, "completion_tokens": 3})
    }

    fn read_request(stream: &mut TcpStream) -> String {
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("read timeout");
        let mut data = Vec::new();
        let mut buffer = [0_u8; 8192];
        loop {
            let read = stream.read(&mut buffer).expect("request");
            assert!(read > 0, "client closed before sending the request");
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
                return head;
            }
        }
    }

    struct Server {
        address: SocketAddr,
        stop: Arc<AtomicBool>,
        handle: JoinHandle<Vec<String>>,
    }

    impl Server {
        /// Serves `bodies` in order, one per connection, and records the head
        /// of every request it receives.
        fn spawn(bodies: Vec<String>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
            let address = listener.local_addr().expect("address");
            listener.set_nonblocking(true).expect("nonblocking");
            let stop = Arc::new(AtomicBool::new(false));
            let thread_stop = Arc::clone(&stop);
            let handle = std::thread::spawn(move || {
                let mut bodies = bodies.into_iter();
                let mut heads = Vec::new();
                while !thread_stop.load(Ordering::SeqCst) {
                    let mut stream = match listener.accept() {
                        Ok((stream, _)) => stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(10));
                            continue;
                        }
                        Err(error) => panic!("accept: {error}"),
                    };
                    stream.set_nonblocking(false).expect("blocking stream");
                    heads.push(read_request(&mut stream));
                    let (status, body) = match bodies.next() {
                        Some(body) => ("200 OK", body),
                        None => (
                            "400 Bad Request",
                            r#"{"error":"script exhausted"}"#.to_string(),
                        ),
                    };
                    write!(
                        stream,
                        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .expect("response");
                    stream.flush().expect("flush");
                }
                heads
            });
            Self {
                address,
                stop,
                handle,
            }
        }

        fn finish(self) -> Vec<String> {
            self.stop.store(true, Ordering::SeqCst);
            self.handle.join().expect("server thread")
        }
    }

    fn config_for(address: SocketAddr, root: &Path, model: &str) -> Config {
        let parsed = crate::provider_cli::parse_from([
            "commandagent",
            "--cwd",
            &root.to_string_lossy(),
            "--state-dir",
            &root.join("state").to_string_lossy(),
            "--model",
            model,
            "--provider",
            "openai-compatible",
            "--base-url",
            &format!("http://{address}/v1"),
            "--chat-timeout-secs",
            "30",
        ])
        .expect("arguments");
        let mut config =
            Config::from_cli_with_provider_options(parsed.cli, parsed.provider_options)
                .expect("config");
        config.eval_events_path = Some(root.join("events.jsonl"));
        config
    }

    #[derive(Clone, Copy, Debug)]
    enum Path947 {
        Planner,
        Executor,
    }

    impl Path947 {
        const fn key(self) -> &'static str {
            match self {
                Self::Planner => "planner",
                Self::Executor => "executor",
            }
        }
    }

    /// Replace the wall-clock values of a raw event line with `0` and keep
    /// every other byte.
    fn normalize(line: &str) -> String {
        let mut out = line.to_string();
        for key in ["\"duration_ms\":", "\"occurred_at_epoch_ms\":"] {
            if let Some(start) = out.find(key) {
                let digits = start + key.len();
                let end = out[digits..]
                    .find(|ch: char| !ch.is_ascii_digit())
                    .map_or(out.len(), |offset| digits + offset);
                out.replace_range(digits..end, "0");
            }
        }
        out
    }

    /// One provider turn per body through the same entry the planner or the
    /// executor uses, returning the raw `provider_response` and
    /// `provider_turn_duration` lines of each turn and the request heads.
    ///
    /// `model` keys the process-wide stable-prefix estimate, so each test
    /// passes its own model and the golden lines do not depend on test order.
    fn run_turns(
        path: Path947,
        model: &str,
        bodies: Vec<String>,
    ) -> (Vec<[String; 2]>, Vec<String>) {
        run_turns_with_secrets(path, model, bodies, &[])
    }

    /// [`run_turns`] with `secrets` (fake credential values only) registered
    /// in the redaction scope of the run's events path, the same scope
    /// `eval_events::emit` scrubs every event with.
    fn run_turns_with_secrets(
        path: Path947,
        model: &str,
        bodies: Vec<String>,
        secrets: &[&str],
    ) -> (Vec<[String; 2]>, Vec<String>) {
        let turns = bodies.len();
        let (results, text, heads) = run_raw_turns(path, model, bodies, secrets);
        for (turn, result) in results.into_iter().enumerate() {
            let content =
                result.unwrap_or_else(|error| panic!("{path:?} turn {turn} failed: {error}"));
            assert_eq!(content, "done");
        }
        let mut response = None;
        let mut pairs = Vec::new();
        for line in text.lines() {
            let event: Value = serde_json::from_str(line).expect("event json");
            match event["event"].as_str() {
                Some("provider_response") => {
                    assert!(response.is_none(), "two responses in one turn: {text}");
                    response = Some(line.to_string());
                }
                Some("provider_turn_duration") => {
                    let response = response.take().expect("response before turn event");
                    pairs.push([response, line.to_string()]);
                }
                _ => {}
            }
        }
        assert!(response.is_none(), "response without a turn event: {text}");
        assert_eq!(pairs.len(), turns, "{text}");
        (pairs, heads)
    }

    /// One provider turn per body, returning each turn's reply content or
    /// error text, the whole events file, and the request heads.
    fn run_raw_turns(
        path: Path947,
        model: &str,
        bodies: Vec<String>,
        secrets: &[&str],
    ) -> (Vec<Result<String, String>>, String, Vec<String>) {
        let dir = tempfile::tempdir().expect("tempdir");
        let turns = bodies.len();
        let server = Server::spawn(bodies);
        let config = config_for(server.address, dir.path(), model);
        let events_path = config.eval_events_path.clone().expect("events path");
        if !secrets.is_empty() {
            let mut catalog = crate::sensitive_data::SecretCatalog::new();
            for (index, secret) in secrets.iter().enumerate() {
                catalog
                    .register_credential(&format!("FAKE_KEY_{index}"), secret)
                    .expect("fake credential");
            }
            crate::sensitive_data::install_scope(catalog, Some(dir.path()), Some(&events_path));
        }
        let planner = matches!(path, Path947::Planner);
        let mut client: Box<dyn ChatClient> = client_from_config(&config, planner).expect("client");
        let (scope, native_tools_enabled) = match path {
            Path947::Planner => (ProviderCallScope::PlannerStep, false),
            Path947::Executor => (ProviderCallScope::Executor, true),
        };
        let messages = [ConversationMessage::user("Summarize notes.md.")];
        let mut results = Vec::new();
        for _ in 0..turns {
            let outcome = provider_call::chat_with_cancel_and_stream(
                client.as_mut(),
                &config,
                ProviderChatRequest {
                    scope,
                    model,
                    messages: &messages,
                    tools: &[],
                    native_tools_enabled,
                },
                || false,
                &mut |_chunk| Ok(()),
            );
            results.push(
                outcome
                    .result
                    .map(|reply| reply.content)
                    .map_err(|error| format!("{error:#}")),
            );
        }
        let heads = server.finish();
        if !secrets.is_empty() {
            crate::sensitive_data::set_current(None);
        }
        let text = std::fs::read_to_string(&events_path).unwrap_or_default();
        (results, text, heads)
    }

    const FIELD: &str = "provider_billed_cost_usd";

    /// `line` without the `"<FIELD>":<value>` member, or `None` when the line
    /// does not carry exactly that member.
    fn without_field(line: &str, value: &str) -> Option<String> {
        let member = format!("\"{FIELD}\":{value}");
        if line.matches(&member).count() != 1 {
            return None;
        }
        let leading = format!("{member},");
        let trailing = format!(",{member}");
        if line.contains(&leading) {
            Some(line.replacen(&leading, "", 1))
        } else if line.contains(&trailing) {
            Some(line.replacen(&trailing, "", 1))
        } else {
            None
        }
    }

    fn assert_no_credential_sent(heads: &[String], turns: usize) {
        assert_eq!(heads.len(), turns, "one request per turn: {heads:?}");
        for head in heads {
            assert!(head.starts_with("post /v1/chat/completions "), "{head}");
            assert!(!head.contains("authorization:"), "{head}");
        }
    }

    #[test]
    fn every_case_reaches_both_events_of_the_turn_on_planner_and_executor_paths() {
        let corpus = corpus();
        assert_eq!(corpus["event_field"], FIELD);
        assert_eq!(corpus["event_type"], "string");
        // The credential-overlap cases need a redaction scope; they run in
        // credential_overlap_cases_leave_the_amount_out_on_both_paths.
        let cases = corpus["cases"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|case| case.get("fake_credential").is_none())
            .collect::<Vec<_>>();
        let bodies = cases
            .iter()
            .map(|case| response_body(&case["usage"], case["cost_token"].as_str()))
            .collect::<Vec<_>>();
        let mut reported = 0;
        for path in [Path947::Executor, Path947::Planner] {
            let (pairs, heads) = run_turns(path, MODEL, bodies.clone());
            assert_no_credential_sent(&heads, cases.len());
            let base = &corpus["base_events"][path.key()];
            for (case, pair) in cases.iter().zip(&pairs) {
                let id = case["id"].as_str().unwrap();
                let expected = case["expected_provider_billed_cost_usd"].as_str();
                for (index, line) in pair.iter().enumerate() {
                    let label = format!("{path:?} {id} event {index}");
                    let line = normalize(line);
                    let base_line = base[id][index]
                        .as_str()
                        .unwrap_or_else(|| panic!("{label}: no base line"));
                    match expected {
                        None => {
                            assert!(!line.contains(FIELD), "{label}: {line}");
                            assert_eq!(
                                line, base_line,
                                "{label}: an unreported amount changes no byte"
                            );
                        }
                        Some(expected) => {
                            // The raw member, compared as text: a JSON string
                            // holding exactly the decimal text sent.
                            let member = format!("\"{expected}\"");
                            assert_eq!(
                                without_field(&line, &member).as_deref(),
                                Some(base_line),
                                "{label}: the amount is the only addition: {line}"
                            );
                        }
                    }
                }
                let response: Value = serde_json::from_str(&pair[0]).unwrap();
                let turn: Value = serde_json::from_str(&pair[1]).unwrap();
                assert_eq!(response.get(FIELD), turn.get(FIELD), "{path:?} {id}");
                if turn.get(FIELD).is_some() {
                    reported += 1;
                }
            }
        }
        assert_eq!(reported, 10, "five reported cases on each of two paths");
    }

    #[test]
    fn credential_overlap_cases_leave_the_amount_out_on_both_paths() {
        let corpus = corpus();
        let overlap = corpus["cases"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|case| case.get("fake_credential").is_some())
            .collect::<Vec<_>>();
        assert_eq!(overlap.len(), 2, "an equal and a contained fake credential");
        let secrets = overlap
            .iter()
            .map(|case| case["fake_credential"].as_str().unwrap())
            .collect::<Vec<_>>();
        // Turn 0 only settles the stable-prefix estimate; then each overlap
        // case; then amounts that overlap no registered secret.
        let mut bodies = vec![response_body(&tokens_usage(), None)];
        bodies.extend(
            overlap
                .iter()
                .map(|case| response_body(&case["usage"], case["cost_token"].as_str())),
        );
        let kept = ["0.000123", "0", "1e-400"];
        bodies.extend(
            kept.iter()
                .map(|sent| response_body(&tokens_usage(), Some(sent))),
        );
        // Its own model keeps the process-wide stable-prefix estimate of the
        // corpus model untouched; the request model is mapped back to the
        // corpus model before the base comparison.
        const OVERLAP_MODEL: &str = "compatible-model-947-secret";
        for path in [Path947::Executor, Path947::Planner] {
            let (pairs, heads) =
                run_turns_with_secrets(path, OVERLAP_MODEL, bodies.clone(), &secrets);
            assert_no_credential_sent(&heads, bodies.len());
            let base = &corpus["base_events"][path.key()];
            for (case, pair) in overlap.iter().zip(&pairs[1..]) {
                let id = case["id"].as_str().unwrap();
                assert!(case["expected_provider_billed_cost_usd"].is_null(), "{id}");
                for (index, line) in pair.iter().enumerate() {
                    let line = normalize(line).replace(
                        &format!("\"model\":\"{OVERLAP_MODEL}\""),
                        &format!("\"model\":\"{MODEL}\""),
                    );
                    assert!(!line.contains(FIELD), "{path:?} {id}: {line}");
                    assert_eq!(
                        Some(line.as_str()),
                        base[id][index].as_str(),
                        "{path:?} {id} event {index}: a left-out amount changes no other byte"
                    );
                }
            }
            for (sent, pair) in kept.iter().zip(&pairs[1 + overlap.len()..]) {
                for line in pair {
                    assert_eq!(
                        line.matches(&format!("\"{FIELD}\":\"{sent}\"")).count(),
                        1,
                        "{path:?} {sent}: {line}"
                    );
                }
            }
            // Every stored amount is a validated decimal string or absent: no
            // secret, no marker inside an amount, no made-up zero.
            for line in pairs.iter().flatten() {
                let event: Value = serde_json::from_str(line).unwrap();
                if let Some(amount) = event.get(FIELD) {
                    let text = amount.as_str().expect("the amount is a JSON string");
                    let parsed = crate::providers::BilledCostUsd::from_json_token(text);
                    assert_eq!(
                        parsed.as_ref().map(|cost| cost.as_decimal_str()),
                        Some(text)
                    );
                    assert!(kept.contains(&text), "{path:?}: {line}");
                }
                for secret in &secrets {
                    assert!(!line.contains(secret), "{path:?}: {line}");
                }
                assert!(!line.contains("redacted"), "{path:?}: {line}");
            }
        }
    }

    #[test]
    fn raw_amounts_reach_both_events_as_the_decimal_text_sent() {
        let cases: [(&str, Option<&str>); 14] = [
            ("1e-400", Some("1e-400")),
            ("1.00000000000000001", Some("1.00000000000000001")),
            (
                "0.123456789012345678901234567890",
                Some("0.123456789012345678901234567890"),
            ),
            ("0", Some("0")),
            ("0.0", Some("0.0")),
            ("0e5", Some("0e5")),
            ("9007199254740991", Some("9007199254740991")),
            ("9.007199254740991e15", Some("9.007199254740991e15")),
            ("-0", None),
            ("-0.0", None),
            ("-1e-400", None),
            ("9007199254740992", None),
            ("9.007199254740992e15", None),
            ("9007199254740991.000000000000000001", None),
        ];
        let bodies = cases
            .iter()
            .map(|(sent, _)| response_body(&tokens_usage(), Some(sent)))
            .collect::<Vec<_>>();
        for path in [Path947::Executor, Path947::Planner] {
            let (pairs, heads) = run_turns(path, "compatible-model-947-raw", bodies.clone());
            assert_no_credential_sent(&heads, cases.len());
            for ((sent, expected), pair) in cases.iter().zip(&pairs) {
                for line in pair {
                    match expected {
                        Some(expected) => assert_eq!(
                            line.matches(&format!("\"{FIELD}\":\"{expected}\"")).count(),
                            1,
                            "{path:?} {sent}: {line}"
                        ),
                        None => assert!(!line.contains(FIELD), "{path:?} {sent}: {line}"),
                    }
                    assert_eq!(line.matches(FIELD).count(), usize::from(expected.is_some()));
                }
            }
        }
    }

    #[test]
    fn a_number_beyond_f64_fails_the_whole_response_as_at_base() {
        for path in [Path947::Executor, Path947::Planner] {
            let (results, text, _) = run_raw_turns(
                path,
                "compatible-model-947-beyond",
                vec![response_body(&tokens_usage(), Some("1e400"))],
                &[],
            );
            let error = results[0]
                .as_ref()
                .expect_err("the whole response fails to parse, as at base");
            assert!(error.contains("out of range"), "{path:?}: {error}");
            assert!(!text.contains(FIELD), "{path:?}: {text}");
            assert!(
                !text.contains("\"event\":\"provider_response\""),
                "{path:?}: {text}"
            );
        }
    }

    #[test]
    fn one_turn_keeps_the_reported_amount_once_without_rounding() {
        for path in [Path947::Executor, Path947::Planner] {
            let (pairs, heads) = run_turns(
                path,
                "compatible-model-947-once",
                vec![response_body(&tokens_usage(), Some("0.012345"))],
            );
            assert_no_credential_sent(&heads, 1);
            let [response, turn] = &pairs[0];
            for line in [response, turn] {
                assert_eq!(
                    line.matches(&format!("\"{FIELD}\":\"0.012345\"")).count(),
                    1,
                    "{path:?}: {line}"
                );
                assert_eq!(line.matches(FIELD).count(), 1, "{path:?}: {line}");
            }
            let turn: Value = serde_json::from_str(turn).unwrap();
            assert_eq!(turn[FIELD].as_str(), Some("0.012345"), "{path:?}");
            assert_eq!(turn["prompt_eval_count"], 12, "{path:?}");
            assert_eq!(turn["eval_count"], 3, "{path:?}");
            let expected_scope = match path {
                Path947::Planner => "planner_step",
                Path947::Executor => "executor",
            };
            assert_eq!(turn["caller_scope"], expected_scope);
        }
    }

    #[test]
    fn a_reported_turn_does_not_carry_its_amount_into_the_next_turn() {
        for path in [Path947::Executor, Path947::Planner] {
            let (pairs, _) = run_turns(
                path,
                "compatible-model-947-carry",
                vec![
                    response_body(&tokens_usage(), Some("0.012345")),
                    response_body(&tokens_usage(), None),
                    response_body(&Value::Null, None),
                ],
            );
            let fields = pairs
                .iter()
                .flatten()
                .map(|line| serde_json::from_str::<Value>(line).unwrap()[FIELD].clone())
                .collect::<Vec<_>>();
            assert_eq!(
                fields,
                vec![
                    json!("0.012345"),
                    json!("0.012345"),
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    Value::Null
                ],
                "{path:?}"
            );
        }
    }

    #[test]
    fn corpus_is_synthetic_and_pinned_to_its_base() {
        use sha2::{Digest, Sha256};

        let root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/apps/compatible-billed-cost");
        let manifest: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join("source-manifest.json")).unwrap(),
        )
        .unwrap();
        let bytes = std::fs::read(FIXTURE).unwrap();
        assert_eq!(
            manifest["fixture_sha256"]["events.json"],
            format!("{:x}", Sha256::digest(&bytes))
        );
        let text = String::from_utf8(bytes).unwrap();
        for forbidden in [
            "/Users/",
            "/Volumes/",
            "/home/",
            "/tmp/",
            "Bearer",
            "authorization",
            "sk-",
            "api_key",
        ] {
            assert!(!text.contains(forbidden), "fixture contains {forbidden}");
        }
        let corpus = corpus();
        assert_eq!(corpus["kind"], "synthetic");
        assert_eq!(
            corpus["base_events"]["captured_at"], manifest["base_sha"],
            "the golden events come from the base commit"
        );
        // The base commit never reads usage.cost, so a case that differs from
        // synthetic-cost-absent only in its cost token has the same base lines.
        for copied in corpus["base_events"]["copied_from_absent"]
            .as_array()
            .unwrap()
        {
            let id = copied.as_str().unwrap();
            for path in ["executor", "planner"] {
                let base = &corpus["base_events"][path];
                assert_eq!(base[id], base["synthetic-cost-absent"], "{path} {id}");
            }
        }
        for case in corpus["cases"].as_array().unwrap() {
            let id = case["id"].as_str().unwrap();
            assert!(id.starts_with("synthetic-"), "{id}");
            for path in ["executor", "planner"] {
                assert_eq!(
                    corpus["base_events"][path][id].as_array().map(Vec::len),
                    Some(2),
                    "{path} {id}"
                );
            }
        }
    }
}
