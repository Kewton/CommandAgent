use crate::config::Provider;
use crate::providers::{
    ChatMessage, ChatProvider, ChatRequest, ChatResponse, ChatRole, ExecutorProvider,
    PlannerProvider,
};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::Duration;

pub const DEFAULT_GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

#[derive(Debug, Clone)]
pub struct GeminiClient<T = ReqwestGeminiTransport> {
    base_url: String,
    api_key: String,
    transport: T,
    timeout: Duration,
    retries: u8,
}

impl GeminiClient<ReqwestGeminiTransport> {
    pub fn new(api_key: impl Into<String>) -> Result<Self, GeminiError> {
        Self::with_options(
            DEFAULT_GEMINI_BASE_URL,
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
    ) -> Result<Self, GeminiError> {
        Ok(Self::with_transport(
            base_url,
            api_key,
            ReqwestGeminiTransport::new()?,
            timeout,
            retries,
        ))
    }
}

impl<T> GeminiClient<T>
where
    T: GeminiTransport,
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

    pub fn chat(&self, request: &ChatRequest) -> Result<ChatResponse, GeminiError> {
        let payload = self.chat_payload(request);
        let url = self.endpoint(&request.model);
        let response =
            self.send_with_retries(|| self.transport.post_json(&url, &payload, self.timeout))?;
        ensure_success(response.status, &response.body)?;
        parse_generate_content_response(&response.body)
    }

    pub fn chat_payload(&self, request: &ChatRequest) -> Value {
        let (system_instruction, contents) = to_gemini_contents(&request.messages);
        let mut payload = json!({
            "contents": contents,
        });
        if let Some(system_instruction) = system_instruction {
            payload["systemInstruction"] = json!({
                "parts": [{"text": system_instruction}],
            });
        }
        payload
    }

    pub fn request_log_payload(&self, request: &ChatRequest) -> Value {
        json!({
            "provider": "gemini",
            "endpoint": "models/*:generateContent",
            "base_url": self.base_url,
            "request": self.chat_payload(request),
        })
    }

    pub fn response_log_payload(&self, response_body: &str) -> Value {
        json!({
            "provider": "gemini",
            "endpoint": "models/*:generateContent",
            "base_url": self.base_url,
            "response_body": response_body,
        })
    }

    fn endpoint(&self, model: &str) -> String {
        let model = model.strip_prefix("models/").unwrap_or(model);
        format!(
            "{}/models/{}:generateContent?key={}",
            self.base_url, model, self.api_key
        )
    }

    fn send_with_retries<F>(&self, mut send: F) -> Result<HttpResponse, GeminiError>
    where
        F: FnMut() -> Result<HttpResponse, GeminiError>,
    {
        let mut last_error = None;
        for attempt in 0..=self.retries {
            match send() {
                Ok(response) if response.status < 500 => return Ok(response),
                Ok(response) => {
                    last_error = Some(GeminiError::Http {
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
        Err(last_error.unwrap_or_else(|| GeminiError::Transport("request failed".to_string())))
    }
}

impl<T> ChatProvider for GeminiClient<T>
where
    T: GeminiTransport,
{
    fn provider(&self) -> Provider {
        Provider::Gemini
    }
}

impl<T> ExecutorProvider for GeminiClient<T> where T: GeminiTransport {}

impl<T> PlannerProvider for GeminiClient<T> where T: GeminiTransport {}

pub trait GeminiTransport: Clone {
    fn post_json(
        &self,
        url: &str,
        body: &Value,
        timeout: Duration,
    ) -> Result<HttpResponse, GeminiError>;
}

#[derive(Debug, Clone)]
pub struct ReqwestGeminiTransport {
    client: Client,
}

impl ReqwestGeminiTransport {
    pub fn new() -> Result<Self, GeminiError> {
        let client = Client::builder()
            .build()
            .map_err(|err| GeminiError::Transport(err.to_string()))?;
        Ok(Self { client })
    }
}

impl GeminiTransport for ReqwestGeminiTransport {
    fn post_json(
        &self,
        url: &str,
        body: &Value,
        timeout: Duration,
    ) -> Result<HttpResponse, GeminiError> {
        let response = self
            .client
            .post(url)
            .timeout(timeout)
            .json(body)
            .send()
            .map_err(|err| GeminiError::Transport(err.to_string()))?;
        let status = response.status().as_u16();
        let body = response
            .text()
            .map_err(|err| GeminiError::Transport(err.to_string()))?;
        Ok(HttpResponse { status, body })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeminiError {
    Transport(String),
    Http { status: u16, body: String },
    Json(String),
}

impl std::fmt::Display for GeminiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transport(message) => write!(f, "Gemini transport failed: {}", message),
            Self::Http { status, body } => {
                write!(
                    f,
                    "Gemini generateContent failed: status {}: {}",
                    status, body
                )
            }
            Self::Json(message) => write!(f, "Gemini JSON parse failed: {}", message),
        }
    }
}

impl std::error::Error for GeminiError {}

#[derive(Debug, Serialize)]
struct GeminiContent {
    role: &'static str,
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize)]
struct GeminiPart {
    text: String,
}

#[derive(Debug, Deserialize)]
struct GenerateContentResponse {
    #[serde(default)]
    candidates: Vec<GeminiCandidate>,
}

#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    content: Option<GeminiResponseContent>,
}

#[derive(Debug, Deserialize)]
struct GeminiResponseContent {
    #[serde(default)]
    parts: Vec<GeminiResponsePart>,
}

#[derive(Debug, Deserialize)]
struct GeminiResponsePart {
    #[serde(default)]
    text: String,
}

fn parse_generate_content_response(body: &str) -> Result<ChatResponse, GeminiError> {
    let parsed: GenerateContentResponse =
        serde_json::from_str(body).map_err(|err| GeminiError::Json(err.to_string()))?;
    let content = parsed
        .candidates
        .into_iter()
        .filter_map(|candidate| candidate.content)
        .flat_map(|content| content.parts)
        .map(|part| part.text)
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    Ok(ChatResponse {
        content,
        tool_calls: Vec::new(),
    })
}

fn ensure_success(status: u16, body: &str) -> Result<(), GeminiError> {
    if (200..300).contains(&status) {
        Ok(())
    } else {
        Err(GeminiError::Http {
            status,
            body: body.to_string(),
        })
    }
}

fn to_gemini_contents(messages: &[ChatMessage]) -> (Option<String>, Vec<GeminiContent>) {
    let mut system_messages = Vec::new();
    let mut contents = Vec::new();

    for message in messages {
        match message.role {
            ChatRole::System => system_messages.push(message.content.clone()),
            ChatRole::Assistant => contents.push(GeminiContent {
                role: "model",
                parts: vec![GeminiPart {
                    text: message.content.clone(),
                }],
            }),
            ChatRole::User | ChatRole::Tool => contents.push(GeminiContent {
                role: "user",
                parts: vec![GeminiPart {
                    text: message.content.clone(),
                }],
            }),
        }
    }

    let system_instruction = (!system_messages.is_empty()).then(|| system_messages.join("\n\n"));
    (system_instruction, contents)
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
    fn payload_maps_system_and_roles() {
        let client = GeminiClient::with_transport(
            "https://example.test/v1beta/",
            "key",
            MockTransport::default(),
            Duration::from_secs(1),
            0,
        );
        let request = ChatRequest {
            model: "gemini-3.5-flash".to_string(),
            messages: vec![
                ChatMessage {
                    role: ChatRole::System,
                    content: "system rules".to_string(),
                },
                ChatMessage {
                    role: ChatRole::User,
                    content: "hello".to_string(),
                },
                ChatMessage {
                    role: ChatRole::Assistant,
                    content: "hi".to_string(),
                },
            ],
            tools: Vec::new(),
            tool_call_mode: ToolCallMode::XmlFallback,
        };

        let payload = client.chat_payload(&request);

        assert_eq!(
            payload["systemInstruction"]["parts"][0]["text"],
            "system rules"
        );
        assert_eq!(payload["contents"][0]["role"], "user");
        assert_eq!(payload["contents"][1]["role"], "model");
    }

    #[test]
    fn chat_parses_text_response() {
        let transport = MockTransport::with_responses([Ok(HttpResponse {
            status: 200,
            body: r#"{"candidates":[{"content":{"parts":[{"text":"hello"},{"text":"world"}]}}]}"#
                .to_string(),
        })]);
        let client = GeminiClient::with_transport(
            "https://example.test/v1beta/",
            "key",
            transport.clone(),
            Duration::from_secs(1),
            0,
        );
        let request = ChatRequest {
            model: "gemini-3.5-flash".to_string(),
            messages: vec![ChatMessage {
                role: ChatRole::User,
                content: "hello".to_string(),
            }],
            tools: Vec::new(),
            tool_call_mode: ToolCallMode::XmlFallback,
        };

        let response = client.chat(&request).unwrap();

        assert_eq!(response.content, "hello\nworld");
        assert!(response.tool_calls.is_empty());
        assert!(transport.calls()[0].contains("/models/gemini-3.5-flash:generateContent?key=key"));
    }

    #[test]
    fn retries_transport_failures() {
        let transport = MockTransport::with_responses([
            Err(GeminiError::Transport("temporary".to_string())),
            Ok(HttpResponse {
                status: 200,
                body: r#"{"candidates":[{"content":{"parts":[{"text":"ok"}]}}]}"#.to_string(),
            }),
        ]);
        let client = GeminiClient::with_transport(
            "https://example.test/v1beta/",
            "key",
            transport.clone(),
            Duration::from_secs(1),
            1,
        );
        let request = ChatRequest {
            model: "models/gemini-3.5-flash".to_string(),
            messages: vec![ChatMessage {
                role: ChatRole::User,
                content: "hello".to_string(),
            }],
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
        responses: VecDeque<Result<HttpResponse, GeminiError>>,
        calls: Vec<String>,
        json_bodies: Vec<Value>,
    }

    impl MockTransport {
        fn with_responses<const N: usize>(
            responses: [Result<HttpResponse, GeminiError>; N],
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
    }

    impl GeminiTransport for MockTransport {
        fn post_json(
            &self,
            url: &str,
            body: &Value,
            _timeout: Duration,
        ) -> Result<HttpResponse, GeminiError> {
            let mut inner = self.inner.borrow_mut();
            inner.calls.push(format!("POST {url}"));
            inner.json_bodies.push(body.clone());
            inner
                .responses
                .pop_front()
                .unwrap_or_else(|| Err(GeminiError::Transport("no response".to_string())))
        }
    }
}
