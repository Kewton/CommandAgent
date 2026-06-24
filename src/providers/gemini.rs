use crate::config::Provider;
use crate::providers::usage::extract_usage;
use crate::providers::xml_fallback::extract_tool_calls_with_content;
use crate::providers::{
    ChatMessage, ChatProvider, ChatRequest, ChatResponse, ChatRole, ExecutorProvider,
    PlannerProvider, ToolCall, ToolCallMode, ToolSpec, tool_call_parse_error_content,
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
        let (system_instruction, contents) =
            to_gemini_contents(&request.messages, request.tool_call_mode);
        let mut payload = json!({
            "contents": contents,
        });
        if let Some(system_instruction) = system_instruction {
            payload["systemInstruction"] = json!({
                "parts": [{"text": system_instruction}],
            });
        }
        if request.tool_call_mode == ToolCallMode::Native && !request.tools.is_empty() {
            payload["tools"] = json!([{
                "functionDeclarations": to_gemini_function_declarations(&request.tools),
            }]);
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
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(rename = "functionCall", skip_serializing_if = "Option::is_none")]
    function_call: Option<GeminiFunctionCallPart>,
    #[serde(rename = "functionResponse", skip_serializing_if = "Option::is_none")]
    function_response: Option<GeminiFunctionResponsePart>,
    #[serde(rename = "thoughtSignature", skip_serializing_if = "Option::is_none")]
    thought_signature: Option<String>,
}

#[derive(Debug, Serialize)]
struct GeminiFunctionDeclaration {
    name: String,
    description: String,
    parameters: Value,
}

#[derive(Debug, Serialize)]
struct GeminiFunctionCallPart {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    name: String,
    args: Value,
}

#[derive(Debug, Serialize)]
struct GeminiFunctionResponsePart {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    name: String,
    response: Value,
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
    #[serde(rename = "functionCall")]
    function_call: Option<GeminiResponseFunctionCall>,
    #[serde(rename = "thoughtSignature")]
    thought_signature: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiResponseFunctionCall {
    id: Option<String>,
    name: Option<String>,
    #[serde(default)]
    args: Value,
}

fn parse_generate_content_response(body: &str) -> Result<ChatResponse, GeminiError> {
    let value: Value =
        serde_json::from_str(body).map_err(|err| GeminiError::Json(err.to_string()))?;
    let usage = extract_usage(Provider::Gemini, &value);
    let parsed: GenerateContentResponse =
        serde_json::from_value(value).map_err(|err| GeminiError::Json(err.to_string()))?;
    let mut text_parts = Vec::new();
    let mut native_tool_calls = Vec::new();
    let mut native_parse_errors = Vec::new();

    for (index, part) in parsed
        .candidates
        .into_iter()
        .filter_map(|candidate| candidate.content)
        .flat_map(|content| content.parts)
        .enumerate()
    {
        if !part.text.is_empty() {
            text_parts.push(part.text);
        }
        if let Some(function_call) = part.function_call {
            match parse_gemini_function_call(function_call, part.thought_signature, index) {
                Ok(call) => native_tool_calls.push(call),
                Err(err) => native_parse_errors.push(err),
            }
        }
    }

    if !native_parse_errors.is_empty() {
        return Ok(ChatResponse::new(
            tool_call_parse_error_content(native_parse_errors.join("; ")),
            Vec::new(),
        )
        .with_usage(usage));
    }

    let content = text_parts.join("\n");
    if !native_tool_calls.is_empty() {
        return Ok(ChatResponse::new(content, native_tool_calls).with_usage(usage));
    }

    let extraction = extract_tool_calls_with_content(&content)
        .map_err(|err| GeminiError::Json(err.to_string()))?;

    Ok(ChatResponse::new(extraction.content, extraction.tool_calls).with_usage(usage))
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

fn to_gemini_contents(
    messages: &[ChatMessage],
    mode: ToolCallMode,
) -> (Option<String>, Vec<GeminiContent>) {
    let mut system_messages = Vec::new();
    let mut contents = Vec::new();

    for message in messages {
        match message.role {
            ChatRole::System => system_messages.push(message.content.clone()),
            ChatRole::Assistant => contents.push(GeminiContent {
                role: "model",
                parts: assistant_parts(message, mode),
            }),
            ChatRole::User => contents.push(GeminiContent {
                role: "user",
                parts: vec![text_part(message.content.clone())],
            }),
            ChatRole::Tool => contents.push(GeminiContent {
                role: "user",
                parts: tool_result_parts(message, mode),
            }),
        }
    }

    let system_instruction = (!system_messages.is_empty()).then(|| system_messages.join("\n\n"));
    (system_instruction, contents)
}

fn to_gemini_function_declarations(tools: &[ToolSpec]) -> Vec<GeminiFunctionDeclaration> {
    tools
        .iter()
        .map(|tool| GeminiFunctionDeclaration {
            name: tool.name.clone(),
            description: tool.description.clone(),
            parameters: to_gemini_parameters_schema(&tool.parameters_json_schema),
        })
        .collect()
}

fn to_gemini_parameters_schema(schema: &Value) -> Value {
    match schema {
        Value::Object(object) => {
            let mut converted = serde_json::Map::new();
            for (key, value) in object {
                if key == "additionalProperties" {
                    continue;
                }
                converted.insert(key.clone(), to_gemini_parameters_schema(value));
            }
            Value::Object(converted)
        }
        Value::Array(items) => {
            Value::Array(items.iter().map(to_gemini_parameters_schema).collect())
        }
        other => other.clone(),
    }
}

fn parse_gemini_function_call(
    function_call: GeminiResponseFunctionCall,
    thought_signature: Option<String>,
    index: usize,
) -> Result<ToolCall, String> {
    let Some(name) = function_call
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
    else {
        return Err("gemini_native_function_call_missing_name".to_string());
    };
    if !function_call.args.is_object() {
        return Err(format!(
            "gemini_native_function_call_invalid_args: {name} args must be a JSON object"
        ));
    }
    Ok(ToolCall {
        id: Some(
            function_call
                .id
                .filter(|id| !id.trim().is_empty())
                .unwrap_or_else(|| format!("gemini-call-{index}")),
        ),
        thought_signature: thought_signature.filter(|signature| !signature.trim().is_empty()),
        name: name.to_string(),
        args_json: serde_json::to_string(&function_call.args).unwrap_or_else(|_| "{}".to_string()),
    })
}

fn assistant_parts(message: &ChatMessage, mode: ToolCallMode) -> Vec<GeminiPart> {
    if mode != ToolCallMode::Native || message.tool_calls.iter().all(|call| call.id.is_none()) {
        return vec![text_part(message.content.clone())];
    }

    let mut parts = Vec::new();
    if !message.content.trim().is_empty() {
        parts.push(text_part(message.content.clone()));
    }
    for call in message.tool_calls.iter().filter(|call| call.id.is_some()) {
        let args = serde_json::from_str(&call.args_json).unwrap_or_else(|_| json!({}));
        parts.push(GeminiPart {
            text: None,
            function_call: Some(GeminiFunctionCallPart {
                id: call.id.clone(),
                name: call.name.clone(),
                args,
            }),
            function_response: None,
            thought_signature: call.thought_signature.clone(),
        });
    }
    if parts.is_empty() {
        parts.push(text_part(String::new()));
    }
    parts
}

fn tool_result_parts(message: &ChatMessage, mode: ToolCallMode) -> Vec<GeminiPart> {
    if mode != ToolCallMode::Native {
        return vec![text_part(message.content.clone())];
    }
    let (Some(tool_name), Some(tool_call_id)) = (&message.tool_name, &message.tool_call_id) else {
        return vec![text_part(message.content.clone())];
    };
    vec![GeminiPart {
        text: None,
        function_call: None,
        function_response: Some(GeminiFunctionResponsePart {
            id: Some(tool_call_id.clone()),
            name: tool_name.clone(),
            response: json!({
                "output": message.content.clone(),
            }),
        }),
        thought_signature: None,
    }]
}

fn text_part(text: String) -> GeminiPart {
    GeminiPart {
        text: Some(text),
        function_call: None,
        function_response: None,
        thought_signature: None,
    }
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
                ChatMessage::new(ChatRole::System, "system rules"),
                ChatMessage::new(ChatRole::User, "hello"),
                ChatMessage::new(ChatRole::Assistant, "hi"),
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
    fn payload_sends_function_declarations_for_native_mode() {
        let client = GeminiClient::with_transport(
            "https://example.test/v1beta/",
            "key",
            MockTransport::default(),
            Duration::from_secs(1),
            0,
        );
        let request = ChatRequest {
            model: "gemini-3.5-flash".to_string(),
            messages: vec![ChatMessage::new(ChatRole::User, "write a file")],
            tools: crate::tools::registry::file_tool_specs(),
            tool_call_mode: ToolCallMode::Native,
        };

        let payload = client.chat_payload(&request);
        let declarations = payload["tools"][0]["functionDeclarations"]
            .as_array()
            .unwrap();
        let write = declarations
            .iter()
            .find(|declaration| declaration["name"] == "Write")
            .unwrap();

        assert_eq!(write["parameters"]["required"], json!(["path", "content"]));
        assert_eq!(write["parameters"]["properties"]["path"]["type"], "string");
        assert!(
            write["parameters"]
                .as_object()
                .unwrap()
                .get("additionalProperties")
                .is_none()
        );
    }

    #[test]
    fn payload_omits_function_declarations_for_xml_fallback_mode() {
        let client = GeminiClient::with_transport(
            "https://example.test/v1beta/",
            "key",
            MockTransport::default(),
            Duration::from_secs(1),
            0,
        );
        let request = ChatRequest {
            model: "gemini-3.5-flash".to_string(),
            messages: vec![ChatMessage::new(ChatRole::User, "write a file")],
            tools: crate::tools::registry::file_tool_specs(),
            tool_call_mode: ToolCallMode::XmlFallback,
        };

        let payload = client.chat_payload(&request);

        assert!(payload.get("tools").is_none());
    }

    #[test]
    fn payload_serializes_native_function_call_and_response_history() {
        let client = GeminiClient::with_transport(
            "https://example.test/v1beta/",
            "key",
            MockTransport::default(),
            Duration::from_secs(1),
            0,
        );
        let call = ToolCall {
            id: Some("call-1".to_string()),
            thought_signature: Some("sig-1".to_string()),
            name: "Read".to_string(),
            args_json: r#"{"path":"Cargo.toml"}"#.to_string(),
        };
        let request = ChatRequest {
            model: "gemini-3.5-flash".to_string(),
            messages: vec![
                ChatMessage::new(ChatRole::User, "read Cargo.toml"),
                ChatMessage::assistant_with_tool_calls("", vec![call]),
                ChatMessage::tool_result("contents", "Read", Some("call-1".to_string())),
            ],
            tools: crate::tools::registry::file_tool_specs(),
            tool_call_mode: ToolCallMode::Native,
        };

        let payload = client.chat_payload(&request);

        assert_eq!(
            payload["contents"][1]["parts"][0]["functionCall"]["id"],
            "call-1"
        );
        assert_eq!(
            payload["contents"][1]["parts"][0]["functionCall"]["args"]["path"],
            "Cargo.toml"
        );
        assert_eq!(
            payload["contents"][1]["parts"][0]["thoughtSignature"],
            "sig-1"
        );
        assert_eq!(
            payload["contents"][2]["parts"][0]["functionResponse"]["id"],
            "call-1"
        );
        assert_eq!(
            payload["contents"][2]["parts"][0]["functionResponse"]["response"]["output"],
            "contents"
        );
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
            messages: vec![ChatMessage::new(ChatRole::User, "hello")],
            tools: Vec::new(),
            tool_call_mode: ToolCallMode::XmlFallback,
        };

        let response = client.chat(&request).unwrap();

        assert_eq!(response.content, "hello\nworld");
        assert!(response.tool_calls.is_empty());
        assert!(transport.calls()[0].contains("/models/gemini-3.5-flash:generateContent?key=key"));
    }

    #[test]
    fn chat_parses_xml_tool_call_from_response_text() {
        let transport = MockTransport::with_responses([Ok(HttpResponse {
            status: 200,
            body: r#"{"candidates":[{"content":{"parts":[{"text":"<commandagent_tool_call>{\"name\":\"Write\",\"args\":{\"path\":\"hello.txt\",\"content\":\"ok\"}}</commandagent_tool_call>"}]}}]}"#
                .to_string(),
        })]);
        let client = GeminiClient::with_transport(
            "https://example.test/v1beta/",
            "key",
            transport,
            Duration::from_secs(1),
            0,
        );
        let request = ChatRequest {
            model: "gemini-3.5-flash".to_string(),
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
    fn chat_parses_native_function_call() {
        let transport = MockTransport::with_responses([Ok(HttpResponse {
            status: 200,
            body: r#"{"candidates":[{"content":{"parts":[{"functionCall":{"id":"call-1","name":"Write","args":{"path":"hello.txt","content":"ok"}},"thoughtSignature":"sig-1"}]}}],"usageMetadata":{"promptTokenCount":11,"candidatesTokenCount":7,"totalTokenCount":18}}"#
                .to_string(),
        })]);
        let client = GeminiClient::with_transport(
            "https://example.test/v1beta/",
            "key",
            transport,
            Duration::from_secs(1),
            0,
        );
        let request = ChatRequest {
            model: "gemini-3.5-flash".to_string(),
            messages: vec![ChatMessage::new(ChatRole::User, "create file")],
            tools: crate::tools::registry::file_tool_specs(),
            tool_call_mode: ToolCallMode::Native,
        };

        let response = client.chat(&request).unwrap();

        assert_eq!(response.content, "");
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].id.as_deref(), Some("call-1"));
        assert_eq!(
            response.tool_calls[0].thought_signature.as_deref(),
            Some("sig-1")
        );
        assert_eq!(response.tool_calls[0].name, "Write");
        assert_eq!(
            response.tool_calls[0].args_json,
            r#"{"content":"ok","path":"hello.txt"}"#
        );
        assert_eq!(response.usage.input_tokens, Some(11));
        assert_eq!(response.usage.output_tokens, Some(7));
        assert_eq!(response.usage.total_tokens, Some(18));
        assert!(response.usage.unavailable_reason.is_none());
    }

    #[test]
    fn chat_returns_parse_evidence_for_malformed_native_function_call() {
        let transport = MockTransport::with_responses([Ok(HttpResponse {
            status: 200,
            body: r#"{"candidates":[{"content":{"parts":[{"functionCall":{"args":{"path":"hello.txt"}}}]}}]}"#
                .to_string(),
        })]);
        let client = GeminiClient::with_transport(
            "https://example.test/v1beta/",
            "key",
            transport,
            Duration::from_secs(1),
            0,
        );
        let request = ChatRequest {
            model: "gemini-3.5-flash".to_string(),
            messages: vec![ChatMessage::new(ChatRole::User, "create file")],
            tools: crate::tools::registry::file_tool_specs(),
            tool_call_mode: ToolCallMode::Native,
        };

        let response = client.chat(&request).unwrap();
        let error = crate::providers::tool_call_parse_error_from_content(&response.content)
            .expect("expected provider parse evidence");

        assert_eq!(error, "gemini_native_function_call_missing_name");
        assert!(response.tool_calls.is_empty());
    }

    #[test]
    fn chat_rejects_malformed_xml_tool_call() {
        let transport = MockTransport::with_responses([Ok(HttpResponse {
            status: 200,
            body: r#"{"candidates":[{"content":{"parts":[{"text":"<commandagent_tool_call>{\"name\":\"Read\"}"}]}}]}"#
                .to_string(),
        })]);
        let client = GeminiClient::with_transport(
            "https://example.test/v1beta/",
            "key",
            transport,
            Duration::from_secs(1),
            0,
        );
        let request = ChatRequest {
            model: "gemini-3.5-flash".to_string(),
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
