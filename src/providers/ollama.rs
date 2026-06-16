use crate::config::Provider;
use crate::providers::{
    ChatMessage, ChatProvider, ChatRequest, ChatResponse, ChatRole, ExecutorProvider,
    PlannerProvider, ToolCall, ToolCallMode, ToolSpec,
};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct OllamaClient<T = ReqwestOllamaTransport> {
    base_url: String,
    transport: T,
    timeout: Duration,
    retries: u8,
}

impl OllamaClient<ReqwestOllamaTransport> {
    pub fn new(base_url: impl Into<String>) -> Result<Self, OllamaError> {
        Self::with_options(base_url, Duration::from_secs(120), 2)
    }

    pub fn with_options(
        base_url: impl Into<String>,
        timeout: Duration,
        retries: u8,
    ) -> Result<Self, OllamaError> {
        Ok(Self::with_transport(
            base_url,
            ReqwestOllamaTransport::new()?,
            timeout,
            retries,
        ))
    }
}

impl<T> OllamaClient<T>
where
    T: OllamaTransport,
{
    pub fn with_transport(
        base_url: impl Into<String>,
        transport: T,
        timeout: Duration,
        retries: u8,
    ) -> Self {
        Self {
            base_url: normalize_base_url(&base_url.into()),
            transport,
            timeout,
            retries,
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn list_models(&self) -> Result<Vec<String>, OllamaError> {
        let url = self.endpoint("/api/tags");
        let response = self.send_with_retries(|| self.transport.get(&url, self.timeout))?;
        ensure_success("/api/tags", response.status, &response.body)?;
        parse_tags_response(&response.body)
    }

    pub fn chat(&self, request: &ChatRequest) -> Result<ChatResponse, OllamaError> {
        let payload = self.chat_payload(request);
        let url = self.endpoint("/api/chat");
        let response =
            self.send_with_retries(|| self.transport.post_json(&url, &payload, self.timeout))?;
        ensure_success("/api/chat", response.status, &response.body)?;
        parse_chat_response(&response.body)
    }

    pub fn chat_payload(&self, request: &ChatRequest) -> Value {
        let tools = if request.tool_call_mode == ToolCallMode::Native && !request.tools.is_empty() {
            Some(to_ollama_tools(&request.tools))
        } else {
            None
        };

        serde_json::to_value(OllamaChatRequest {
            model: request.model.clone(),
            messages: request.messages.iter().map(to_ollama_message).collect(),
            stream: false,
            think: false,
            tools,
        })
        .unwrap_or_else(|_| json!({}))
    }

    pub fn request_log_payload(&self, request: &ChatRequest) -> Value {
        json!({
            "provider": "ollama",
            "endpoint": "/api/chat",
            "base_url": self.base_url,
            "request": self.chat_payload(request),
        })
    }

    pub fn response_log_payload(&self, response_body: &str) -> Value {
        json!({
            "provider": "ollama",
            "endpoint": "/api/chat",
            "base_url": self.base_url,
            "response_body": response_body,
        })
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    fn send_with_retries<F>(&self, mut send: F) -> Result<HttpResponse, OllamaError>
    where
        F: FnMut() -> Result<HttpResponse, OllamaError>,
    {
        let mut last_error = None;
        for attempt in 0..=self.retries {
            match send() {
                Ok(response) if response.status < 500 => return Ok(response),
                Ok(response) => {
                    last_error = Some(OllamaError::Http {
                        endpoint: "ollama".to_string(),
                        status: response.status,
                        body: response.body,
                    });
                }
                Err(err) => last_error = Some(err),
            }

            if attempt == self.retries {
                break;
            }
        }
        Err(last_error.unwrap_or_else(|| OllamaError::Transport("request failed".to_string())))
    }
}

impl<T> ChatProvider for OllamaClient<T>
where
    T: OllamaTransport,
{
    fn provider(&self) -> Provider {
        Provider::Ollama
    }
}

impl<T> ExecutorProvider for OllamaClient<T> where T: OllamaTransport {}

impl<T> PlannerProvider for OllamaClient<T> where T: OllamaTransport {}

pub trait OllamaTransport: Clone {
    fn get(&self, url: &str, timeout: Duration) -> Result<HttpResponse, OllamaError>;
    fn post_json(
        &self,
        url: &str,
        body: &Value,
        timeout: Duration,
    ) -> Result<HttpResponse, OllamaError>;
}

#[derive(Debug, Clone)]
pub struct ReqwestOllamaTransport {
    client: Client,
}

impl ReqwestOllamaTransport {
    pub fn new() -> Result<Self, OllamaError> {
        let client = Client::builder()
            .build()
            .map_err(|err| OllamaError::Transport(err.to_string()))?;
        Ok(Self { client })
    }
}

impl OllamaTransport for ReqwestOllamaTransport {
    fn get(&self, url: &str, timeout: Duration) -> Result<HttpResponse, OllamaError> {
        let response = self
            .client
            .get(url)
            .timeout(timeout)
            .send()
            .map_err(|err| OllamaError::Transport(err.to_string()))?;
        to_http_response(response)
    }

    fn post_json(
        &self,
        url: &str,
        body: &Value,
        timeout: Duration,
    ) -> Result<HttpResponse, OllamaError> {
        let response = self
            .client
            .post(url)
            .timeout(timeout)
            .json(body)
            .send()
            .map_err(|err| OllamaError::Transport(err.to_string()))?;
        to_http_response(response)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OllamaError {
    Transport(String),
    Http {
        endpoint: String,
        status: u16,
        body: String,
    },
    Json(String),
}

impl std::fmt::Display for OllamaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transport(message) => write!(f, "Ollama transport failed: {}", message),
            Self::Http {
                endpoint,
                status,
                body,
            } => {
                write!(f, "Ollama {} failed: status {}: {}", endpoint, status, body)
            }
            Self::Json(message) => write!(f, "Ollama JSON parse failed: {}", message),
        }
    }
}

impl std::error::Error for OllamaError {}

#[derive(Debug, Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
    think: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OllamaToolDefinition>>,
}

#[derive(Debug, Serialize)]
struct OllamaMessage {
    role: &'static str,
    content: String,
}

#[derive(Debug, Serialize)]
struct OllamaToolDefinition {
    #[serde(rename = "type")]
    kind: &'static str,
    function: OllamaToolFunction,
}

#[derive(Debug, Serialize)]
struct OllamaToolFunction {
    name: String,
    description: String,
    parameters: Value,
}

#[derive(Debug, Deserialize)]
struct TagsResponse {
    #[serde(default)]
    models: Vec<TagModel>,
}

#[derive(Debug, Deserialize)]
struct TagModel {
    name: String,
}

#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    #[serde(default)]
    message: Option<OllamaResponseMessage>,
}

#[derive(Debug, Deserialize)]
struct OllamaResponseMessage {
    #[serde(default)]
    content: String,
    #[serde(default)]
    tool_calls: Vec<OllamaResponseToolCall>,
}

#[derive(Debug, Deserialize)]
struct OllamaResponseToolCall {
    function: OllamaResponseToolFunction,
}

#[derive(Debug, Deserialize)]
struct OllamaResponseToolFunction {
    name: String,
    #[serde(default)]
    arguments: Value,
}

fn parse_tags_response(body: &str) -> Result<Vec<String>, OllamaError> {
    let parsed: TagsResponse =
        serde_json::from_str(body).map_err(|err| OllamaError::Json(err.to_string()))?;
    Ok(parsed.models.into_iter().map(|model| model.name).collect())
}

fn parse_chat_response(body: &str) -> Result<ChatResponse, OllamaError> {
    let parsed: OllamaChatResponse =
        serde_json::from_str(body).map_err(|err| OllamaError::Json(err.to_string()))?;
    let message = parsed.message.unwrap_or(OllamaResponseMessage {
        content: String::new(),
        tool_calls: Vec::new(),
    });
    let tool_calls = message
        .tool_calls
        .into_iter()
        .map(|call| ToolCall {
            name: call.function.name,
            args_json: serde_json::to_string(&call.function.arguments)
                .unwrap_or_else(|_| "{}".to_string()),
        })
        .collect();

    Ok(ChatResponse {
        content: message.content,
        tool_calls,
    })
}

fn ensure_success(endpoint: &str, status: u16, body: &str) -> Result<(), OllamaError> {
    if (200..300).contains(&status) {
        Ok(())
    } else {
        Err(OllamaError::Http {
            endpoint: endpoint.to_string(),
            status,
            body: body.to_string(),
        })
    }
}

fn to_http_response(response: reqwest::blocking::Response) -> Result<HttpResponse, OllamaError> {
    let status = response.status().as_u16();
    let body = response
        .text()
        .map_err(|err| OllamaError::Transport(err.to_string()))?;
    Ok(HttpResponse { status, body })
}

fn to_ollama_message(message: &ChatMessage) -> OllamaMessage {
    OllamaMessage {
        role: match message.role {
            ChatRole::System => "system",
            ChatRole::User => "user",
            ChatRole::Assistant => "assistant",
            ChatRole::Tool => "tool",
        },
        content: message.content.clone(),
    }
}

fn to_ollama_tools(tools: &[ToolSpec]) -> Vec<OllamaToolDefinition> {
    tools
        .iter()
        .map(|tool| OllamaToolDefinition {
            kind: "function",
            function: OllamaToolFunction {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": true,
                }),
            },
        })
        .collect()
}

fn normalize_base_url(base_url: &str) -> String {
    let mut value = base_url.trim().trim_end_matches('/').to_string();
    if value == "0.0.0.0" {
        value = "127.0.0.1:11434".to_string();
    }
    if !value.contains("://") {
        value = format!("http://{value}");
    }
    add_default_ollama_port(value)
}

fn add_default_ollama_port(value: String) -> String {
    let Some((scheme, rest)) = value.split_once("://") else {
        return value;
    };
    let (authority, path) = rest
        .split_once('/')
        .map(|(authority, path)| (authority, format!("/{path}")))
        .unwrap_or((rest, String::new()));
    if authority.contains(':') || authority.starts_with('[') {
        value
    } else {
        format!("{scheme}://{authority}:11434{path}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::rc::Rc;

    #[test]
    fn list_models_parses_tags_response() {
        let transport = MockTransport::with_responses([Ok(HttpResponse {
            status: 200,
            body: r#"{"models":[{"name":"qwen"},{"name":"llama"}]}"#.to_string(),
        })]);
        let client = OllamaClient::with_transport(
            "http://127.0.0.1:11434/",
            transport.clone(),
            Duration::from_secs(1),
            0,
        );

        let models = client.list_models().unwrap();

        assert_eq!(models, vec!["qwen", "llama"]);
        assert_eq!(
            transport.calls(),
            vec!["GET http://127.0.0.1:11434/api/tags"]
        );
    }

    #[test]
    fn normalizes_common_ollama_host_values() {
        assert_eq!(
            normalize_base_url("http://127.0.0.1:11434/"),
            "http://127.0.0.1:11434"
        );
        assert_eq!(
            normalize_base_url("127.0.0.1:11434"),
            "http://127.0.0.1:11434"
        );
        assert_eq!(normalize_base_url("localhost"), "http://localhost:11434");
        assert_eq!(normalize_base_url("0.0.0.0"), "http://127.0.0.1:11434");
    }

    #[test]
    fn chat_sends_native_tools_when_requested() {
        let transport = MockTransport::with_responses([Ok(HttpResponse {
            status: 200,
            body: r#"{"message":{"content":"","tool_calls":[{"function":{"name":"Read","arguments":{"path":"Cargo.toml"}}}]}}"#
                .to_string(),
        })]);
        let client = OllamaClient::with_transport(
            "http://127.0.0.1:11434",
            transport.clone(),
            Duration::from_secs(1),
            0,
        );
        let request = ChatRequest {
            model: "qwen".to_string(),
            messages: vec![ChatMessage {
                role: ChatRole::User,
                content: "read Cargo.toml".to_string(),
            }],
            tools: vec![ToolSpec {
                name: "Read".to_string(),
                description: "Read file".to_string(),
            }],
            tool_call_mode: ToolCallMode::Native,
        };

        let response = client.chat(&request).unwrap();

        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].name, "Read");
        let posted = transport.last_json().unwrap();
        assert!(posted.get("tools").is_some());
    }

    #[test]
    fn chat_omits_tools_for_xml_fallback_mode() {
        let transport = MockTransport::with_responses([Ok(HttpResponse {
            status: 200,
            body: r#"{"message":{"content":"ok"}}"#.to_string(),
        })]);
        let client = OllamaClient::with_transport(
            "http://127.0.0.1:11434",
            transport.clone(),
            Duration::from_secs(1),
            0,
        );
        let request = ChatRequest {
            model: "qwen".to_string(),
            messages: vec![ChatMessage {
                role: ChatRole::User,
                content: "hello".to_string(),
            }],
            tools: vec![ToolSpec {
                name: "Read".to_string(),
                description: "Read file".to_string(),
            }],
            tool_call_mode: ToolCallMode::XmlFallback,
        };

        let response = client.chat(&request).unwrap();

        assert_eq!(response.content, "ok");
        let posted = transport.last_json().unwrap();
        assert!(posted.get("tools").is_none());
    }

    #[test]
    fn retries_transport_failures() {
        let transport = MockTransport::with_responses([
            Err(OllamaError::Transport("temporary".to_string())),
            Ok(HttpResponse {
                status: 200,
                body: r#"{"models":[{"name":"qwen"}]}"#.to_string(),
            }),
        ]);
        let client = OllamaClient::with_transport(
            "http://127.0.0.1:11434",
            transport.clone(),
            Duration::from_secs(1),
            1,
        );

        let models = client.list_models().unwrap();

        assert_eq!(models, vec!["qwen"]);
        assert_eq!(transport.calls().len(), 2);
    }

    #[derive(Clone, Default)]
    struct MockTransport {
        inner: Rc<RefCell<MockInner>>,
    }

    #[derive(Default)]
    struct MockInner {
        responses: VecDeque<Result<HttpResponse, OllamaError>>,
        calls: Vec<String>,
        json_bodies: Vec<Value>,
    }

    impl MockTransport {
        fn with_responses<const N: usize>(
            responses: [Result<HttpResponse, OllamaError>; N],
        ) -> Self {
            Self {
                inner: Rc::new(RefCell::new(MockInner {
                    responses: VecDeque::from(responses),
                    calls: Vec::new(),
                    json_bodies: Vec::new(),
                })),
            }
        }

        fn calls(&self) -> Vec<String> {
            self.inner.borrow().calls.clone()
        }

        fn last_json(&self) -> Option<Value> {
            self.inner.borrow().json_bodies.last().cloned()
        }
    }

    impl OllamaTransport for MockTransport {
        fn get(&self, url: &str, _timeout: Duration) -> Result<HttpResponse, OllamaError> {
            let mut inner = self.inner.borrow_mut();
            inner.calls.push(format!("GET {url}"));
            inner
                .responses
                .pop_front()
                .unwrap_or_else(|| Err(OllamaError::Transport("no response".to_string())))
        }

        fn post_json(
            &self,
            url: &str,
            body: &Value,
            _timeout: Duration,
        ) -> Result<HttpResponse, OllamaError> {
            let mut inner = self.inner.borrow_mut();
            inner.calls.push(format!("POST {url}"));
            inner.json_bodies.push(body.clone());
            inner
                .responses
                .pop_front()
                .unwrap_or_else(|| Err(OllamaError::Transport("no response".to_string())))
        }
    }
}
