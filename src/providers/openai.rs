use crate::config::Provider;
use crate::providers::xml_fallback::extract_tool_calls_with_content;
use crate::providers::{
    ChatMessage, ChatProvider, ChatRequest, ChatResponse, ChatRole, ExecutorProvider,
    PlannerProvider,
};
use reqwest::blocking::Client;
use serde_json::{Value, json};
use std::time::Duration;

pub const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";

#[derive(Debug, Clone)]
pub struct OpenAiClient<T = ReqwestOpenAiTransport> {
    base_url: String,
    api_key: String,
    transport: T,
    timeout: Duration,
    retries: u8,
}

impl OpenAiClient<ReqwestOpenAiTransport> {
    pub fn new(api_key: impl Into<String>) -> Result<Self, OpenAiError> {
        Self::with_options(
            DEFAULT_OPENAI_BASE_URL,
            api_key,
            Duration::from_secs(120),
            2,
        )
    }

    pub fn with_options(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        timeout: Duration,
        retries: u8,
    ) -> Result<Self, OpenAiError> {
        Ok(Self::with_transport(
            base_url,
            api_key,
            ReqwestOpenAiTransport::new()?,
            timeout,
            retries,
        ))
    }
}

impl<T> OpenAiClient<T>
where
    T: OpenAiTransport,
{
    pub fn with_transport(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        transport: T,
        timeout: Duration,
        retries: u8,
    ) -> Self {
        Self {
            base_url: normalize_base_url(&base_url.into()),
            api_key: api_key.into(),
            transport,
            timeout,
            retries,
        }
    }

    pub fn chat(&self, request: &ChatRequest) -> Result<ChatResponse, OpenAiError> {
        let payload = self.chat_payload(request);
        let url = self.endpoint();
        let response = self.send_with_retries(|| {
            self.transport
                .post_json(&url, &self.api_key, &payload, self.timeout)
        })?;
        ensure_success(response.status, &response.body)?;
        parse_response(&response.body)
    }

    pub fn chat_payload(&self, request: &ChatRequest) -> Value {
        json!({
            "model": request.model,
            "input": request.messages.iter().map(to_response_input_item).collect::<Vec<_>>(),
        })
    }

    pub fn request_log_payload(&self, request: &ChatRequest) -> Value {
        json!({
            "provider": "openai",
            "endpoint": "/responses",
            "base_url": self.base_url,
            "request": self.chat_payload(request),
        })
    }

    pub fn response_log_payload(&self, response_body: &str) -> Value {
        json!({
            "provider": "openai",
            "endpoint": "/responses",
            "base_url": self.base_url,
            "response_body": response_body,
        })
    }

    fn endpoint(&self) -> String {
        format!("{}/responses", self.base_url)
    }

    fn send_with_retries<F>(&self, mut send: F) -> Result<HttpResponse, OpenAiError>
    where
        F: FnMut() -> Result<HttpResponse, OpenAiError>,
    {
        let mut last_error = None;
        for attempt in 0..=self.retries {
            match send() {
                Ok(response) if response.status < 500 => return Ok(response),
                Ok(response) => {
                    last_error = Some(OpenAiError::Http {
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
        Err(last_error.unwrap_or_else(|| OpenAiError::Transport("request failed".to_string())))
    }
}

impl<T> ChatProvider for OpenAiClient<T>
where
    T: OpenAiTransport,
{
    fn provider(&self) -> Provider {
        Provider::OpenAi
    }
}

impl<T> ExecutorProvider for OpenAiClient<T> where T: OpenAiTransport {}

impl<T> PlannerProvider for OpenAiClient<T> where T: OpenAiTransport {}

pub trait OpenAiTransport: Clone {
    fn post_json(
        &self,
        url: &str,
        api_key: &str,
        body: &Value,
        timeout: Duration,
    ) -> Result<HttpResponse, OpenAiError>;
}

#[derive(Debug, Clone)]
pub struct ReqwestOpenAiTransport {
    client: Client,
}

impl ReqwestOpenAiTransport {
    pub fn new() -> Result<Self, OpenAiError> {
        let client = Client::builder()
            .build()
            .map_err(|err| OpenAiError::Transport(err.to_string()))?;
        Ok(Self { client })
    }
}

impl OpenAiTransport for ReqwestOpenAiTransport {
    fn post_json(
        &self,
        url: &str,
        api_key: &str,
        body: &Value,
        timeout: Duration,
    ) -> Result<HttpResponse, OpenAiError> {
        let response = self
            .client
            .post(url)
            .bearer_auth(api_key)
            .timeout(timeout)
            .json(body)
            .send()
            .map_err(|err| OpenAiError::Transport(err.to_string()))?;
        let status = response.status().as_u16();
        let body = response
            .text()
            .map_err(|err| OpenAiError::Transport(err.to_string()))?;
        Ok(HttpResponse { status, body })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenAiError {
    Transport(String),
    Http { status: u16, body: String },
    Json(String),
}

impl std::fmt::Display for OpenAiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transport(message) => write!(f, "OpenAI transport failed: {}", message),
            Self::Http { status, body } => {
                write!(
                    f,
                    "OpenAI Responses API failed: status {}: {}",
                    status, body
                )
            }
            Self::Json(message) => write!(f, "OpenAI JSON parse failed: {}", message),
        }
    }
}

impl std::error::Error for OpenAiError {}

fn parse_response(body: &str) -> Result<ChatResponse, OpenAiError> {
    let value: Value =
        serde_json::from_str(body).map_err(|err| OpenAiError::Json(err.to_string()))?;

    if let Some(output_text) = value.get("output_text").and_then(Value::as_str) {
        let extraction = extract_tool_calls_with_content(output_text)
            .map_err(|err| OpenAiError::Json(err.to_string()))?;
        return Ok(ChatResponse {
            content: extraction.content,
            tool_calls: extraction.tool_calls,
        });
    }

    let mut parts = Vec::new();
    if let Some(output) = value.get("output").and_then(Value::as_array) {
        for item in output {
            if item.get("type").and_then(Value::as_str) != Some("message") {
                continue;
            }
            let Some(content) = item.get("content").and_then(Value::as_array) else {
                continue;
            };
            for part in content {
                match part.get("type").and_then(Value::as_str) {
                    Some("output_text") => {
                        if let Some(text) = part.get("text").and_then(Value::as_str) {
                            parts.push(text.to_string());
                        }
                    }
                    Some("refusal") => {
                        if let Some(text) = part.get("refusal").and_then(Value::as_str) {
                            parts.push(text.to_string());
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    let content = parts.join("\n");
    let extraction = extract_tool_calls_with_content(&content)
        .map_err(|err| OpenAiError::Json(err.to_string()))?;

    Ok(ChatResponse {
        content: extraction.content,
        tool_calls: extraction.tool_calls,
    })
}

fn ensure_success(status: u16, body: &str) -> Result<(), OpenAiError> {
    if (200..300).contains(&status) {
        Ok(())
    } else {
        Err(OpenAiError::Http {
            status,
            body: body.to_string(),
        })
    }
}

fn to_response_input_item(message: &ChatMessage) -> Value {
    let (role, content_type) = match message.role {
        ChatRole::System => ("system", "input_text"),
        ChatRole::User | ChatRole::Tool => ("user", "input_text"),
        ChatRole::Assistant => ("assistant", "output_text"),
    };
    json!({
        "role": role,
        "content": [{
            "type": content_type,
            "text": message.content,
        }],
    })
}

fn normalize_base_url(base_url: &str) -> String {
    base_url.trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::ToolCallMode;
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::rc::Rc;

    #[test]
    fn payload_uses_input_text_for_user_and_output_text_for_assistant() {
        let client = OpenAiClient::with_transport(
            "https://api.openai.test/v1/",
            "key",
            MockTransport::default(),
            Duration::from_secs(1),
            0,
        );
        let request = ChatRequest {
            model: "gpt-5.4-mini".to_string(),
            messages: vec![
                ChatMessage::new(ChatRole::User, "hello"),
                ChatMessage::new(ChatRole::Assistant, "hi"),
            ],
            tools: Vec::new(),
            tool_call_mode: ToolCallMode::XmlFallback,
        };

        let payload = client.chat_payload(&request);

        assert_eq!(payload["input"][0]["content"][0]["type"], "input_text");
        assert_eq!(payload["input"][1]["content"][0]["type"], "output_text");
    }

    #[test]
    fn chat_parses_output_message_text() {
        let transport = MockTransport::with_responses([Ok(HttpResponse {
            status: 200,
            body:
                r#"{"output":[{"type":"message","content":[{"type":"output_text","text":"ok"}]}]}"#
                    .to_string(),
        })]);
        let client = OpenAiClient::with_transport(
            "https://api.openai.test/v1/",
            "key",
            transport.clone(),
            Duration::from_secs(1),
            0,
        );
        let request = ChatRequest {
            model: "gpt-5.4-mini".to_string(),
            messages: vec![ChatMessage::new(ChatRole::User, "hello")],
            tools: Vec::new(),
            tool_call_mode: ToolCallMode::XmlFallback,
        };

        let response = client.chat(&request).unwrap();

        assert_eq!(response.content, "ok");
        assert!(response.tool_calls.is_empty());
        assert_eq!(transport.last_api_key().as_deref(), Some("key"));
    }

    #[test]
    fn chat_parses_xml_tool_call_from_output_text() {
        let transport = MockTransport::with_responses([Ok(HttpResponse {
            status: 200,
            body: r#"{"output_text":"<commandagent_tool_call>{\"name\":\"Write\",\"args\":{\"path\":\"hello.txt\",\"content\":\"ok\"}}</commandagent_tool_call>"}"#
                .to_string(),
        })]);
        let client = OpenAiClient::with_transport(
            "https://api.openai.test/v1/",
            "key",
            transport,
            Duration::from_secs(1),
            0,
        );
        let request = ChatRequest {
            model: "gpt-5.4-mini".to_string(),
            messages: vec![ChatMessage::new(ChatRole::User, "create file")],
            tools: Vec::new(),
            tool_call_mode: ToolCallMode::XmlFallback,
        };

        let response = client.chat(&request).unwrap();

        assert_eq!(response.content, "");
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].name, "Write");
        assert_eq!(
            response.tool_calls[0].args_json,
            r#"{"content":"ok","path":"hello.txt"}"#
        );
    }

    #[test]
    fn chat_parses_xml_tool_call_from_output_items() {
        let transport = MockTransport::with_responses([Ok(HttpResponse {
            status: 200,
            body: r#"{"output":[{"type":"message","content":[{"type":"output_text","text":"<commandagent_tool_call>{\"name\":\"Read\",\"args\":{\"path\":\"Cargo.toml\"}}</commandagent_tool_call>"}]}]}"#
                .to_string(),
        })]);
        let client = OpenAiClient::with_transport(
            "https://api.openai.test/v1/",
            "key",
            transport,
            Duration::from_secs(1),
            0,
        );
        let request = ChatRequest {
            model: "gpt-5.4-mini".to_string(),
            messages: vec![ChatMessage::new(ChatRole::User, "read file")],
            tools: Vec::new(),
            tool_call_mode: ToolCallMode::XmlFallback,
        };

        let response = client.chat(&request).unwrap();

        assert_eq!(response.content, "");
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].name, "Read");
        assert_eq!(response.tool_calls[0].args_json, r#"{"path":"Cargo.toml"}"#);
    }

    #[test]
    fn chat_rejects_malformed_xml_tool_call() {
        let transport = MockTransport::with_responses([Ok(HttpResponse {
            status: 200,
            body: r#"{"output_text":"<commandagent_tool_call>{\"name\":\"Read\"}"}"#.to_string(),
        })]);
        let client = OpenAiClient::with_transport(
            "https://api.openai.test/v1/",
            "key",
            transport,
            Duration::from_secs(1),
            0,
        );
        let request = ChatRequest {
            model: "gpt-5.4-mini".to_string(),
            messages: vec![ChatMessage::new(ChatRole::User, "read file")],
            tools: Vec::new(),
            tool_call_mode: ToolCallMode::XmlFallback,
        };

        let err = client.chat(&request).unwrap_err();

        assert!(
            err.to_string()
                .contains("unclosed <commandagent_tool_call>")
        );
    }

    #[test]
    fn retries_transport_failures() {
        let transport = MockTransport::with_responses([
            Err(OpenAiError::Transport("temporary".to_string())),
            Ok(HttpResponse {
                status: 200,
                body: r#"{"output_text":"ok"}"#.to_string(),
            }),
        ]);
        let client = OpenAiClient::with_transport(
            "https://api.openai.test/v1/",
            "key",
            transport.clone(),
            Duration::from_secs(1),
            1,
        );
        let request = ChatRequest {
            model: "gpt-5.4-mini".to_string(),
            messages: vec![ChatMessage::new(ChatRole::User, "hello")],
            tools: Vec::new(),
            tool_call_mode: ToolCallMode::XmlFallback,
        };

        let response = client.chat(&request).unwrap();

        assert_eq!(response.content, "ok");
        assert_eq!(transport.calls().len(), 2);
    }

    #[derive(Clone, Default)]
    struct MockTransport {
        inner: Rc<RefCell<MockInner>>,
    }

    #[derive(Default)]
    struct MockInner {
        responses: VecDeque<Result<HttpResponse, OpenAiError>>,
        calls: Vec<String>,
        api_keys: Vec<String>,
        json_bodies: Vec<Value>,
    }

    impl MockTransport {
        fn with_responses<const N: usize>(
            responses: [Result<HttpResponse, OpenAiError>; N],
        ) -> Self {
            Self {
                inner: Rc::new(RefCell::new(MockInner {
                    responses: VecDeque::from(responses),
                    calls: Vec::new(),
                    api_keys: Vec::new(),
                    json_bodies: Vec::new(),
                })),
            }
        }

        fn calls(&self) -> Vec<String> {
            self.inner.borrow().calls.clone()
        }

        fn last_api_key(&self) -> Option<String> {
            self.inner.borrow().api_keys.last().cloned()
        }
    }

    impl OpenAiTransport for MockTransport {
        fn post_json(
            &self,
            url: &str,
            api_key: &str,
            body: &Value,
            _timeout: Duration,
        ) -> Result<HttpResponse, OpenAiError> {
            let mut inner = self.inner.borrow_mut();
            inner.calls.push(format!("POST {url}"));
            inner.api_keys.push(api_key.to_string());
            inner.json_bodies.push(body.clone());
            inner
                .responses
                .pop_front()
                .unwrap_or_else(|| Err(OpenAiError::Transport("no response".to_string())))
        }
    }
}
