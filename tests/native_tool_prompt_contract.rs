//! Issue #936: a native tool calling request carries each tool description in
//! the function schema only, so its system prompt has no `Tools:` list; a turn
//! that sends no schema (explicit text, a provider without native tools, a
//! session that disabled them, or the allowed fallback after a native error)
//! keeps the XML system prompt of before byte for byte. The before texts, the
//! registry tools, the wire schema digests, and the estimated prompt tokens
//! were taken at the base SHA of the synthetic corpus
//! `tests/corpus/apps/native-tool-prompt-contract/`; no expectation is built
//! from the new implementation. Providers are scripted or loopback stand-ins
//! without credentials, and no case reaches a real provider.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use commandagent::config::{
    Action, Config, ConfigFieldSources, NarrationMode, OllamaThink, OpenAiApi, PlanPreset,
    PromptLayout, Provider, ToolProtocol,
};
use commandagent::minimal_loop::loop_run::run_session_with_outcome_with_ui;
use commandagent::minimal_loop::prompt::{
    PromptGuidance, ToolPromptMode, build_guided_request_messages,
};
use commandagent::provider_call::{self, ProviderCallScope};
use commandagent::provider_cli;
use commandagent::providers::gemini::build_stream_generate_content_request;
use commandagent::providers::gemini_function_calling::{
    build_interactions_request, build_interactions_request_with_previous,
};
use commandagent::providers::ollama::OllamaClient;
use commandagent::providers::{AssistantReply, ChatClient, client_from_config};
use commandagent::state::{ConversationMessage, SessionSnapshot, ToolCall};
use commandagent::tools::registry::{ToolRegistry, ToolSpec};
use commandagent::tui::NOOP_UI;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const CORPUS: &str = "tests/corpus/apps/native-tool-prompt-contract";
const ROOT: &str = "<root>";
const MODEL: &str = "model-936";

fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(CORPUS)
}

fn corpus() -> Value {
    let path = corpus_root().join("fixtures/cases.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn text<'a>(corpus: &'a Value, key: &str) -> &'a str {
    corpus["inputs"][key]
        .as_str()
        .unwrap_or_else(|| panic!("missing input {key}"))
}

fn entry<'a>(corpus: &'a Value, list: &str, id: &str) -> &'a Value {
    corpus[list]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["id"] == id)
        .unwrap_or_else(|| panic!("missing {list} entry {id}"))
}

fn base_system<'a>(corpus: &'a Value, id: &str) -> &'a str {
    entry(corpus, "systems", id)["system"].as_str().unwrap()
}

/// The `- name: description` lines of the base registry tools.
fn base_tool_list(corpus: &Value) -> String {
    corpus["registry_tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| {
            format!(
                "- {}: {}",
                tool["name"].as_str().unwrap(),
                tool["description"].as_str().unwrap()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn base_descriptions(corpus: &Value) -> Vec<String> {
    corpus["registry_tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["description"].as_str().unwrap().to_string())
        .collect()
}

/// A base XML system prompt without its tool list is the system prompt the
/// same request must send natively: the list and its heading are the only
/// difference, so every other line keeps its bytes and its order.
fn expected_native(corpus: &Value, base_native: &str) -> String {
    let block = format!("\nTools:\n{}", base_tool_list(corpus));
    assert_eq!(base_native.matches(&block).count(), 1, "{base_native}");
    base_native.replacen(&block, "", 1)
}

fn expected_native_empty_tools(base_native: &str) -> String {
    assert_eq!(base_native.matches("\nTools:\n").count(), 1);
    base_native.replacen("\nTools:\n", "", 1)
}

/// The JSON-escaped body of a text, as it appears inside a serialized request.
fn escaped(text: &str) -> String {
    let quoted = serde_json::to_string(text).unwrap();
    quoted[1..quoted.len() - 1].to_string()
}

fn history(corpus: &Value) -> Vec<ConversationMessage> {
    let mut assistant = ConversationMessage::assistant(
        text(corpus, "assistant_preamble"),
        vec![ToolCall::new(
            "Read",
            json!({"path": text(corpus, "read_path")}),
        )],
    );
    assistant.tool_calls[0].id = "call-936".to_string();
    vec![
        ConversationMessage::user(text(corpus, "request")),
        assistant,
        ConversationMessage {
            role: "tool".to_string(),
            content: text(corpus, "tool_result").to_string(),
            name: Some("Read".to_string()),
            tool_call_id: Some("call-936".to_string()),
            tool_calls: Vec::new(),
        },
    ]
}

fn guidance(corpus: &Value, guided: bool) -> PromptGuidance<'_> {
    if guided {
        PromptGuidance {
            profile: Some(text(corpus, "profile_guidance")),
            request: Some(text(corpus, "request_guidance")),
        }
    } else {
        PromptGuidance::default()
    }
}

fn build(
    corpus: &Value,
    tools: &[ToolSpec],
    guided: bool,
    feedback: bool,
    mode: ToolPromptMode,
) -> Vec<ConversationMessage> {
    build_guided_request_messages(
        &history(corpus),
        tools,
        Path::new(ROOT),
        feedback.then(|| text(corpus, "pending_feedback")),
        guidance(corpus, guided),
        mode,
    )
}

/// (id, mode, default tools, guidance) of every pinned builder system.
const BUILDER_CASES: [(&str, ToolPromptMode, bool, bool); 8] = [
    ("builder-native-plain", ToolPromptMode::Native, true, false),
    ("builder-native-guided", ToolPromptMode::Native, true, true),
    (
        "builder-native-empty-tools",
        ToolPromptMode::Native,
        false,
        false,
    ),
    (
        "builder-native-empty-tools-guided",
        ToolPromptMode::Native,
        false,
        true,
    ),
    (
        "builder-xml-plain",
        ToolPromptMode::XmlFallback,
        true,
        false,
    ),
    (
        "builder-xml-guided",
        ToolPromptMode::XmlFallback,
        true,
        true,
    ),
    (
        "builder-xml-empty-tools",
        ToolPromptMode::XmlFallback,
        false,
        false,
    ),
    (
        "builder-xml-empty-tools-guided",
        ToolPromptMode::XmlFallback,
        false,
        true,
    ),
];

fn builder_tools(default_tools: bool) -> Vec<ToolSpec> {
    if default_tools {
        ToolRegistry::default().specs().to_vec()
    } else {
        Vec::new()
    }
}

// ---------------------------------------------------------------------------
// Scripted loop runs.

#[derive(Clone)]
struct RecordingClient {
    native: bool,
    xml_fallback: bool,
    replies: Arc<Mutex<Vec<anyhow::Result<AssistantReply>>>>,
    requests: Arc<Mutex<Vec<Recorded>>>,
}

#[derive(Clone)]
struct Recorded {
    messages: Vec<ConversationMessage>,
    tools: Vec<String>,
    native: bool,
}

impl RecordingClient {
    fn new(native: bool, xml_fallback: bool, replies: Vec<anyhow::Result<AssistantReply>>) -> Self {
        Self {
            native,
            xml_fallback,
            replies: Arc::new(Mutex::new(replies)),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl ChatClient for RecordingClient {
    fn label(&self) -> &str {
        "recording"
    }

    fn supports_native_tools(&self, _model: &str) -> bool {
        self.native
    }

    fn allows_xml_fallback(&self) -> bool {
        self.xml_fallback
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn chat(
        &mut self,
        _model: &str,
        messages: &[ConversationMessage],
        tools: &[ToolSpec],
        native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        self.requests.lock().unwrap().push(Recorded {
            messages: messages.to_vec(),
            tools: tools
                .iter()
                .map(|tool| tool.function.name.clone())
                .collect(),
            native: native_tools_enabled,
        });
        let mut replies = self.replies.lock().unwrap();
        if replies.is_empty() {
            anyhow::bail!("scripted client exhausted");
        }
        replies.remove(0)
    }
}

fn tool_reply(name: &str, arguments: Value) -> anyhow::Result<AssistantReply> {
    Ok(AssistantReply {
        content: String::new(),
        tool_calls: vec![ToolCall::new(name, arguments)],
        prompt_tokens: None,
        completion_tokens: None,
    })
}

fn text_reply(content: &str) -> anyhow::Result<AssistantReply> {
    Ok(AssistantReply::text(content))
}

/// Writes, reads, and runs a command before answering.
fn tool_chain_script() -> Vec<anyhow::Result<AssistantReply>> {
    vec![
        tool_reply(
            "Write",
            json!({"path": "notes.md", "content": "# Notes\nA small project.\n"}),
        ),
        tool_reply("Read", json!({"path": "notes.md"})),
        tool_reply("Bash", json!({"command": "wc -l notes.md"})),
        text_reply("Created notes.md with two lines."),
        text_reply("Created notes.md with two lines."),
    ]
}

/// Three empty replies schedule the fresh-session retry, which then writes.
fn fresh_retry_script() -> Vec<anyhow::Result<AssistantReply>> {
    vec![
        text_reply(""),
        text_reply(""),
        text_reply(""),
        tool_reply(
            "Write",
            json!({"path": "notes.md", "content": "# Notes\nA small project.\n"}),
        ),
        text_reply("Created notes.md."),
        text_reply("Created notes.md."),
    ]
}

/// A native reply the provider could not parse, which the loop answers with
/// the established XML fallback, then the run continues.
fn fallback_script(error: &str) -> Vec<anyhow::Result<AssistantReply>> {
    let mut script = vec![Err(anyhow::anyhow!("{error}"))];
    script.extend(tool_chain_script());
    script
}

fn loop_config(root: &Path, request: &str, protocol: Option<ToolProtocol>) -> Config {
    Config {
        workspace_root: root.to_path_buf(),
        state_dir: root.join("state"),
        eval_events_path: Some(root.join("events.jsonl")),
        completion_contract_path: None,
        yes: true,
        offline: false,
        context_budget: 100_000,
        model: "m".to_string(),
        provider: Provider::Ollama,
        tool_protocol: protocol,
        openai_api: OpenAiApi::ChatCompletions,
        prompt_layout: PromptLayout::Stable,
        plan_preset: PlanPreset::None,
        intent_override: None,
        planner_model: "m".to_string(),
        planner_provider: Provider::Ollama,
        planner_think: Some(OllamaThink::False),
        classifier_model: "m".to_string(),
        classifier_provider: Provider::Ollama,
        openai_compatible: None,
        ollama_host: "http://localhost:11434".to_string(),
        ollama_think: None,
        lm_studio_host: "http://localhost:1234".to_string(),
        num_predict: 100,
        max_iterations: 8,
        recovery_plan_auto_runs: 0,
        chat_timeout_secs: 30,
        chat_timeout_source: "override:test".to_string(),
        field_sources: ConfigFieldSources::default(),
        chat_retries: 1,
        stream: false,
        resume: None,
        fresh_session: false,
        no_footer: false,
        narration: NarrationMode::Normal,
        profile: "generic".to_string(),
        profile_explicit: false,
        profile_inference: None,
        style: "default".to_string(),
        action: Action::Prompt(request.to_string()),
    }
}

struct LoopRun {
    dir: tempfile::TempDir,
    requests: Vec<Recorded>,
    result: Result<(), String>,
}

impl LoopRun {
    fn without_root(&self, text: &str) -> String {
        let mut text = text.to_string();
        if let Ok(canonical) = self.dir.path().canonicalize() {
            text = text.replace(&*canonical.to_string_lossy(), ROOT);
        }
        text.replace(&*self.dir.path().to_string_lossy(), ROOT)
    }

    fn systems(&self) -> Vec<String> {
        self.requests
            .iter()
            .map(|request| {
                assert_eq!(request.messages[0].role, "system");
                self.without_root(&request.messages[0].content)
            })
            .collect()
    }

    fn events(&self) -> Vec<Value> {
        std::fs::read_to_string(self.dir.path().join("events.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .collect()
    }

    fn fallback_decisions(&self) -> Vec<Value> {
        self.events()
            .into_iter()
            .filter(|event| event["event"] == "fallback_decision")
            .collect()
    }
}

/// How a run reaches its tool protocol.
#[derive(Clone, Copy, Debug)]
enum Route {
    Native,
    ExplicitText,
    ProviderWithoutNative,
    SessionDisabled,
}

fn run_loop(
    corpus: &Value,
    route: Route,
    xml_fallback: bool,
    script: Vec<anyhow::Result<AssistantReply>>,
) -> LoopRun {
    let dir = tempfile::tempdir().unwrap();
    let request = text(corpus, "request");
    let protocol = match route {
        Route::ExplicitText => Some(ToolProtocol::Text),
        _ => None,
    };
    let native = !matches!(route, Route::ProviderWithoutNative);
    let mut client = RecordingClient::new(native, xml_fallback, script);
    let mut session = SessionSnapshot::new();
    session.native_tools_disabled = matches!(route, Route::SessionDisabled);
    let result = run_session_with_outcome_with_ui(
        &mut client,
        &mut session,
        request,
        &[],
        &loop_config(dir.path(), request, protocol),
        &NOOP_UI,
    );
    let requests = client.requests.lock().unwrap().clone();
    LoopRun {
        dir,
        requests,
        result: result.map(|_| ()).map_err(|error| format!("{error:#}")),
    }
}

// ---------------------------------------------------------------------------
// Loopback provider stand-ins.

struct Request {
    head: String,
    body: Vec<u8>,
}

impl Request {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).expect("request json")
    }
}

fn read_request(stream: &mut TcpStream) -> Request {
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
            return Request {
                head,
                body: data[end + 4..end + 4 + length].to_vec(),
            };
        }
    }
}

/// A synthetic success reply for the endpoint a request was sent to.
fn stand_in_reply(request: &Request) -> (&'static str, String) {
    let line = request.head.lines().next().unwrap_or_default();
    if line.contains("/v1/models") {
        return (
            "application/json",
            json!({"object": "list", "data": [{"id": MODEL, "object": "model"}]}).to_string(),
        );
    }
    if line.contains("/api/chat") {
        let chunk = json!({
            "model": MODEL,
            "message": {"role": "assistant", "content": "done"},
            "done": true,
            "done_reason": "stop"
        });
        if request.json()["stream"] == true {
            return ("application/x-ndjson", format!("{chunk}\n"));
        }
        return ("application/json", chunk.to_string());
    }
    if line.contains("/v1/responses") {
        return (
            "application/json",
            json!({
                "id": "resp-936",
                "object": "response",
                "status": "completed",
                "model": MODEL,
                "output": [{
                    "type": "message",
                    "role": "assistant",
                    "content": [{"type": "output_text", "text": "done"}]
                }]
            })
            .to_string(),
        );
    }
    (
        "application/json",
        json!({
            "id": "chatcmpl-936",
            "object": "chat.completion",
            "model": MODEL,
            "created": 1_790_000_000,
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "done"},
                "finish_reason": "stop"
            }]
        })
        .to_string(),
    )
}

struct Server {
    address: SocketAddr,
    stop: Arc<AtomicBool>,
    handle: JoinHandle<Vec<Request>>,
}

impl Server {
    fn spawn() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address");
        listener.set_nonblocking(true).expect("nonblocking");
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let handle = std::thread::spawn(move || {
            let mut requests = Vec::new();
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
                let request = read_request(&mut stream);
                let (content_type, body) = stand_in_reply(&request);
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .expect("response");
                stream.flush().expect("flush");
                requests.push(request);
            }
            requests
        });
        Self {
            address,
            stop,
            handle,
        }
    }

    /// The recorded requests, without the model listing probes.
    fn finish(self) -> Vec<Request> {
        self.stop.store(true, Ordering::SeqCst);
        self.handle
            .join()
            .expect("server thread")
            .into_iter()
            .filter(|request| request.head.starts_with("post "))
            .collect()
    }
}

/// A config for a loopback provider; every path stays inside `root`.
fn wire_config(address: SocketAddr, root: &Path, provider: &str, api: OpenAiApi) -> Config {
    let (host_flag, host) = if provider == "openai-compatible" {
        ("--base-url", format!("http://{address}/v1"))
    } else {
        ("--lm-studio-host", format!("http://{address}"))
    };
    let parsed = provider_cli::parse_from([
        "commandagent",
        "--cwd",
        &root.to_string_lossy(),
        "--state-dir",
        &root.join("state").to_string_lossy(),
        "--model",
        MODEL,
        "--provider",
        provider,
        host_flag,
        &host,
        "--chat-timeout-secs",
        "30",
    ])
    .expect("arguments");
    let mut config = Config::from_cli_with_provider_options(parsed.cli, parsed.provider_options)
        .expect("config");
    config.eval_events_path = Some(root.join("events.jsonl"));
    config.openai_api = api;
    config.chat_retries = 0;
    config
}

/// The OpenAI-family clients of a provider table row: (id, provider, API).
const OPENAI_FAMILY: [(&str, &str, OpenAiApi); 4] = [
    (
        "lm-studio-chat-completions",
        "lm-studio",
        OpenAiApi::ChatCompletions,
    ),
    ("lm-studio-responses", "lm-studio", OpenAiApi::Responses),
    (
        "openai-compatible-chat-completions",
        "openai-compatible",
        OpenAiApi::ChatCompletions,
    ),
    (
        "openai-compatible-responses",
        "openai-compatible",
        OpenAiApi::Responses,
    ),
];

/// Send one request per call through an existing client to a loopback
/// stand-in and return the recorded request bodies.
fn capture_openai_family(
    provider: &str,
    api: OpenAiApi,
    calls: &[(Vec<ConversationMessage>, Vec<ToolSpec>, bool)],
) -> Vec<Request> {
    let dir = tempfile::tempdir().unwrap();
    let server = Server::spawn();
    let config = wire_config(server.address, dir.path(), provider, api);
    let mut client = client_from_config(&config, false).expect("client");
    for (messages, tools, native) in calls {
        client
            .chat(MODEL, messages, tools, *native)
            .unwrap_or_else(|error| panic!("{provider} {api:?}: {error:#}"));
    }
    server.finish()
}

fn capture_ollama(
    stream: bool,
    calls: &[(Vec<ConversationMessage>, Vec<ToolSpec>, bool)],
) -> Vec<Request> {
    let server = Server::spawn();
    let mut client =
        OllamaClient::new(format!("http://{}", server.address), 30, 64, 0).expect("client");
    for (messages, tools, native) in calls {
        let reply = if stream {
            client.chat_stream(MODEL, messages, tools, *native, &mut |_| Ok(()))
        } else {
            client.chat(MODEL, messages, tools, *native)
        };
        reply.unwrap_or_else(|error| panic!("ollama stream={stream}: {error:#}"));
    }
    server.finish()
}

/// The JSON of a request's function schemas, wherever its format puts them.
fn schema_tools(body: &Value) -> Value {
    body.get("tools").cloned().unwrap_or(Value::Null)
}

fn sha256(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

/// The names and descriptions of a format's function schemas, in order.
fn schema_names_and_descriptions(tools: &Value) -> Vec<(String, String)> {
    let declarations = tools
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|tool| match tool.get("functionDeclarations") {
            Some(declarations) => declarations.as_array().unwrap().clone(),
            None => vec![tool.clone()],
        })
        .collect::<Vec<_>>();
    declarations
        .iter()
        .map(|declaration| {
            let function = declaration.get("function").unwrap_or(declaration);
            (
                function["name"].as_str().unwrap().to_string(),
                function["description"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

/// The request body of every wire format for one native turn and one turn
/// without schemas (the loop sends no tools then, and the native flag off).
fn wire_bodies(
    native_messages: &[ConversationMessage],
    xml_messages: &[ConversationMessage],
) -> Vec<(String, Value, Value)> {
    let specs = ToolRegistry::default().specs().to_vec();
    let calls = vec![
        (native_messages.to_vec(), specs.clone(), true),
        (xml_messages.to_vec(), Vec::new(), false),
    ];
    let mut bodies = Vec::new();
    for (id, provider, api) in OPENAI_FAMILY {
        let requests = capture_openai_family(provider, api, &calls);
        assert_eq!(requests.len(), 2, "{id}");
        bodies.push((id.to_string(), requests[0].json(), requests[1].json()));
    }
    for (id, stream) in [("ollama-chat", false), ("ollama-chat-stream", true)] {
        let requests = capture_ollama(stream, &calls);
        assert_eq!(requests.len(), 2, "{id}");
        assert!(
            requests
                .iter()
                .all(|request| request.head.starts_with("post /api/chat http/1.1"))
        );
        bodies.push((id.to_string(), requests[0].json(), requests[1].json()));
    }
    bodies.push((
        "gemini-interactions".to_string(),
        build_interactions_request(MODEL, native_messages, &specs, 64),
        build_interactions_request(MODEL, xml_messages, &[], 64),
    ));
    bodies.push((
        "gemini-interactions-previous".to_string(),
        build_interactions_request_with_previous(
            MODEL,
            native_messages,
            &specs,
            64,
            Some("interaction-936"),
        ),
        build_interactions_request_with_previous(
            MODEL,
            xml_messages,
            &[],
            64,
            Some("interaction-936"),
        ),
    ));
    bodies.push((
        "gemini-stream-generate-content".to_string(),
        build_stream_generate_content_request(native_messages, &specs, 64),
        build_stream_generate_content_request(xml_messages, &[], 64),
    ));
    bodies
}

/// One turn through `provider_call::chat` with a loopback OpenAI-compatible
/// stand-in: the recorded `provider_turn_duration` event and request body.
fn telemetry_turn(messages: &[ConversationMessage], native: bool) -> (Value, Value) {
    let dir = tempfile::tempdir().unwrap();
    let server = Server::spawn();
    let config = wire_config(
        server.address,
        dir.path(),
        "openai-compatible",
        OpenAiApi::ChatCompletions,
    );
    let mut client = client_from_config(&config, false).expect("client");
    let specs = ToolRegistry::default().specs().to_vec();
    let tools = if native { specs } else { Vec::new() };
    let outcome = provider_call::chat(
        client.as_mut(),
        &config,
        ProviderCallScope::Executor,
        MODEL,
        messages,
        &tools,
        native,
    );
    outcome
        .result
        .unwrap_or_else(|error| panic!("telemetry turn: {error:#}"));
    let requests = server.finish();
    assert_eq!(requests.len(), 1);
    let events = std::fs::read_to_string(dir.path().join("events.jsonl"))
        .unwrap()
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|event| event["event"] == "provider_turn_duration")
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 1, "{events:?}");
    (events[0].clone(), requests[0].json())
}

/// The request of a builder case with its system prompt replaced by `system`.
fn with_system(messages: &[ConversationMessage], system: &str) -> Vec<ConversationMessage> {
    let mut messages = messages.to_vec();
    messages[0].content = system.to_string();
    messages
}

fn guided_requests(corpus: &Value) -> (Vec<ConversationMessage>, Vec<ConversationMessage>) {
    let specs = builder_tools(true);
    (
        build(corpus, &specs, true, true, ToolPromptMode::Native),
        build(corpus, &specs, true, true, ToolPromptMode::XmlFallback),
    )
}

fn occurrences(haystack: &str, needle: &str) -> usize {
    haystack.matches(needle).count()
}

// ---------------------------------------------------------------------------
// Test 1: the builder.

#[test]
fn registry_tools_keep_the_base_order_names_descriptions_and_arguments() {
    let corpus = corpus();
    let actual = ToolRegistry::default()
        .specs()
        .iter()
        .map(|tool| {
            json!({
                "name": tool.function.name,
                "description": tool.function.description,
                "parameters": tool.function.parameters,
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(Value::Array(actual), corpus["registry_tools"]);
    let descriptions = base_descriptions(&corpus);
    assert!(descriptions[0].contains("bash:verify never permits mutation"));
    assert!(descriptions[2].contains("Parent directories are created automatically"));
}

#[test]
fn native_system_drops_only_the_tool_list_and_xml_system_keeps_the_base_bytes() {
    let corpus = corpus();
    let descriptions = base_descriptions(&corpus);
    for (id, mode, default_tools, guided) in BUILDER_CASES {
        let tools = builder_tools(default_tools);
        let messages = build(&corpus, &tools, guided, true, mode);
        let base = base_system(&corpus, id);
        let system = &messages[0].content;
        assert_eq!(messages[0].role, "system", "{id}");
        match (mode, default_tools) {
            (ToolPromptMode::XmlFallback, _) => {
                assert_eq!(system, base, "{id}: the XML system keeps its base bytes");
            }
            (ToolPromptMode::Native, true) => {
                assert_eq!(system, &expected_native(&corpus, base), "{id}");
            }
            (ToolPromptMode::Native, false) => {
                assert_eq!(system, &expected_native_empty_tools(base), "{id}");
            }
        }
        if mode == ToolPromptMode::Native {
            assert!(!system.contains("Tools:"), "{id}: {system}");
            assert!(!system.contains("anvil_tool_call"), "{id}");
            for description in &descriptions {
                assert!(
                    !system.contains(description.as_str()),
                    "{id}: {description}"
                );
            }
            for tool in ["Bash", "Read", "Write", "Edit", "Glob", "Grep"] {
                assert!(!system.contains(&format!("- {tool}:")), "{id}: {tool}");
            }
        } else if default_tools {
            for description in &descriptions {
                assert_eq!(occurrences(system, description), 1, "{id}: {description}");
            }
        }
        for line in [
            "Work only inside workspace `<root>`.",
            "Do not end with planned future work.",
            "Do not claim files, tests, or builds succeeded unless you observed them with tools.",
            "Use workspace-relative paths only, and do not read or write outside the workspace.",
            "Do not claim completion until requested files and evidence are present.",
        ] {
            assert_eq!(occurrences(system, line), 1, "{id}: {line}");
        }
        if guided {
            let profile = system.find("\n\nProfile guidance:\n").unwrap();
            let request = system.find("\n\nRequest guidance:\n").unwrap();
            assert!(profile < request, "{id}");
            assert_eq!(occurrences(system, text(&corpus, "profile_guidance")), 1);
            assert_eq!(occurrences(system, text(&corpus, "request_guidance")), 1);
        } else {
            assert!(!system.contains("guidance:"), "{id}");
        }
    }
}

#[test]
fn both_modes_send_the_same_history_and_pending_feedback() {
    let corpus = corpus();
    for default_tools in [true, false] {
        for guided in [true, false] {
            let tools = builder_tools(default_tools);
            let native = build(&corpus, &tools, guided, true, ToolPromptMode::Native);
            let xml = build(&corpus, &tools, guided, true, ToolPromptMode::XmlFallback);
            assert_eq!(native.len(), 5);
            assert_eq!(native.len(), xml.len());
            for (left, right) in native[1..].iter().zip(&xml[1..]) {
                assert_eq!(left.role, right.role);
                assert_eq!(left.content, right.content);
                assert_eq!(left.name, right.name);
                assert_eq!(left.tool_call_id, right.tool_call_id);
                assert_eq!(left.tool_calls.len(), right.tool_calls.len());
            }
            assert_eq!(native[1].role, "user");
            assert_eq!(native[1].content, text(&corpus, "request"));
            assert_eq!(native[2].role, "assistant");
            assert!(
                native[2].content.is_empty(),
                "a tool call preamble is not re-prompted"
            );
            assert_eq!(native[2].tool_calls[0].name, "Read");
            assert_eq!(native[3].role, "tool");
            assert!(native[3].content.contains("A small synthetic project."));
            assert_eq!(native[4].role, "user");
            assert_eq!(native[4].content, text(&corpus, "pending_feedback"));
            let without = build(&corpus, &tools, guided, false, ToolPromptMode::Native);
            assert_eq!(without.len(), 4);
            assert_eq!(without[0].content, native[0].content);
        }
    }
}

// ---------------------------------------------------------------------------
// Test 2: every provider wire format.

#[test]
fn every_wire_format_keeps_the_base_schema_and_sends_no_system_tool_list() {
    let corpus = corpus();
    let (native, xml) = guided_requests(&corpus);
    let descriptions = base_descriptions(&corpus);
    let base_names = corpus["registry_tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| {
            (
                tool["name"].as_str().unwrap().to_string(),
                tool["description"].as_str().unwrap().to_string(),
            )
        })
        .collect::<Vec<_>>();
    let bodies = wire_bodies(&native, &xml);
    let ids = bodies
        .iter()
        .map(|(id, _, _)| id.clone())
        .collect::<Vec<_>>();
    let pinned = corpus["wire_formats"]
        .as_array()
        .unwrap()
        .iter()
        .map(|format| format["id"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(ids, pinned);
    for (id, native_body, xml_body) in &bodies {
        let tools = schema_tools(native_body);
        assert_eq!(
            sha256(&serde_json::to_string(&tools).unwrap()),
            entry(&corpus, "wire_formats", id)["tools_sha256"],
            "{id}: the function schemas keep their base bytes"
        );
        assert_eq!(schema_names_and_descriptions(&tools), base_names, "{id}");
        let native_text = native_body.to_string();
        assert!(!native_text.contains("Tools:"), "{id}: {native_text}");
        for description in &descriptions {
            assert_eq!(
                occurrences(&native_text, &escaped(description)),
                1,
                "{id}: a native request names {description:?} in its schema only"
            );
        }
        // A continued interaction sends only the new input after the
        // previous interaction id; every other format carries the system.
        assert_eq!(
            native_text.contains(&escaped(text(&corpus, "request_guidance"))),
            id != "gemini-interactions-previous",
            "{id}"
        );

        assert_eq!(
            schema_tools(xml_body),
            Value::Null,
            "{id}: no schema in XML"
        );
        let xml_text = xml_body.to_string();
        assert_eq!(occurrences(&xml_text, "Tools:"), 1, "{id}");
        assert!(xml_text.contains("anvil_tool_call"), "{id}");
        for description in &descriptions {
            assert_eq!(
                occurrences(&xml_text, &escaped(description)),
                1,
                "{id}: a request without schemas lists {description:?} in its system"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Test 3: every route without schemas keeps the XML system.

/// The tool chain as XML text replies, which explicit text mode parses.
fn xml_tool_chain_script() -> Vec<anyhow::Result<AssistantReply>> {
    let call = |name: &str, arguments: Value| {
        text_reply(&format!(
            "<anvil_tool_call name=\"{name}\">{arguments}</anvil_tool_call>"
        ))
    };
    vec![
        call(
            "Write",
            json!({"path": "notes.md", "content": "# Notes\nA small project.\n"}),
        ),
        call("Read", json!({"path": "notes.md"})),
        call("Bash", json!({"command": "wc -l notes.md"})),
        text_reply("Created notes.md with two lines."),
        text_reply("Created notes.md with two lines."),
    ]
}

fn assert_xml_requests(run: &LoopRun, corpus: &Value, label: &str) {
    let base = base_system(corpus, "loop-xml");
    assert!(run.requests.len() >= 3, "{label}: {:?}", run.result);
    for (turn, (request, system)) in run.requests.iter().zip(run.systems()).enumerate() {
        assert!(!request.native, "{label} turn {turn}: no native flag");
        assert!(request.tools.is_empty(), "{label} turn {turn}: no schema");
        assert_eq!(system, base, "{label} turn {turn}: the base XML system");
    }
}

#[test]
fn routes_without_schemas_keep_the_base_xml_system_and_tool_list() {
    let corpus = corpus();
    for (route, script) in [
        (Route::ExplicitText, xml_tool_chain_script()),
        (Route::ProviderWithoutNative, tool_chain_script()),
        (Route::SessionDisabled, tool_chain_script()),
    ] {
        let run = run_loop(&corpus, route, true, script);
        let label = format!("{route:?}");
        assert_xml_requests(&run, &corpus, &label);
        assert!(run.result.is_ok(), "{label}: {:?}", run.result);
        assert!(run.dir.path().join("notes.md").is_file(), "{label}");
        assert!(run.fallback_decisions().is_empty(), "{label}");
    }
}

#[test]
fn allowed_native_error_falls_back_to_the_base_xml_system_for_the_rest_of_the_run() {
    let corpus = corpus();
    let run = run_loop(
        &corpus,
        Route::Native,
        true,
        fallback_script("could not parse the native reply"),
    );
    let systems = run.systems();
    assert!(run.requests[0].native);
    assert_eq!(run.requests[0].tools.len(), 6);
    assert_eq!(
        systems[0],
        expected_native(&corpus, base_system(&corpus, "loop-native"))
    );
    for (turn, request) in run.requests[1..].iter().enumerate() {
        assert!(!request.native, "turn {}", turn + 1);
        assert!(request.tools.is_empty(), "turn {}", turn + 1);
        assert_eq!(
            run.without_root(&request.messages[0].content),
            base_system(&corpus, "loop-xml"),
            "turn {}: no native system without the list stays after the fallback",
            turn + 1
        );
    }
    assert!(run.requests.len() >= 4, "{:?}", run.result);
    let decisions = run.fallback_decisions();
    assert_eq!(decisions.len(), 1, "{decisions:?}");
    assert_eq!(decisions[0]["allowed"], true);
    assert_eq!(decisions[0]["to"], "xml_fallback");
    assert!(run.result.is_ok(), "{:?}", run.result);
}

#[test]
fn native_errors_without_an_allowed_fallback_keep_their_decision() {
    let corpus = corpus();
    for (xml_fallback, error, decision) in [
        (false, "could not parse the native reply", None),
        (true, "upstream answered status 500", Some(false)),
    ] {
        let run = run_loop(&corpus, Route::Native, xml_fallback, fallback_script(error));
        assert!(run.result.is_err(), "{error}: {:?}", run.result);
        assert!(!run.requests.is_empty());
        for (request, system) in run.requests.iter().zip(run.systems()) {
            assert!(request.native, "{error}");
            assert_eq!(request.tools.len(), 6, "{error}");
            assert_eq!(
                system,
                expected_native(&corpus, base_system(&corpus, "loop-native"))
            );
        }
        let decisions = run
            .fallback_decisions()
            .into_iter()
            .map(|event| event["allowed"].as_bool().unwrap())
            .collect::<Vec<_>>();
        match decision {
            Some(allowed) => assert_eq!(decisions, vec![allowed], "{error}"),
            None => assert!(decisions.is_empty(), "{error}: {decisions:?}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Test 4: the system stays the same over the turns of one mode.

#[test]
fn one_mode_sends_the_same_system_on_every_turn() {
    let corpus = corpus();
    let descriptions = base_descriptions(&corpus);
    for (route, expected) in [
        (
            Route::Native,
            expected_native(&corpus, base_system(&corpus, "loop-native")),
        ),
        (
            Route::ProviderWithoutNative,
            base_system(&corpus, "loop-xml").to_string(),
        ),
    ] {
        for (name, script) in [
            ("tool chain", tool_chain_script()),
            ("fresh retry", fresh_retry_script()),
        ] {
            let run = run_loop(&corpus, route, false, script);
            let label = format!("{route:?} {name}");
            assert!(run.requests.len() >= 4, "{label}: {:?}", run.result);
            let lengths = run
                .requests
                .iter()
                .map(|request| request.messages.len())
                .collect::<Vec<_>>();
            assert!(
                lengths.windows(2).any(|pair| pair[0] != pair[1]),
                "{label}: the turns differ in history: {lengths:?}"
            );
            for (turn, system) in run.systems().iter().enumerate() {
                assert_eq!(system, &expected, "{label} turn {turn}");
            }
            for (turn, request) in run.requests.iter().enumerate() {
                let all = request
                    .messages
                    .iter()
                    .map(|message| message.content.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                let listed = usize::from(matches!(route, Route::ProviderWithoutNative));
                assert_eq!(occurrences(&all, "Tools:"), listed, "{label} turn {turn}");
                for description in &descriptions {
                    assert_eq!(
                        occurrences(&all, description),
                        listed,
                        "{label} turn {turn}: {description}"
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Test 5: the existing estimate of the prompt tokens sent.

#[test]
fn estimated_prompt_tokens_sent_drop_for_native_and_stay_for_xml() {
    let corpus = corpus();
    let (native, xml) = guided_requests(&corpus);
    let pinned = |id: &str| entry(&corpus, "telemetry", id)["estimated_prompt_tokens_sent"].clone();
    for (id, new_messages, base_id, is_native) in [
        ("native", &native, "builder-native-guided", true),
        ("xml", &xml, "builder-xml-guided", false),
    ] {
        let old_messages = with_system(new_messages, base_system(&corpus, base_id));
        let (old_event, old_body) = telemetry_turn(&old_messages, is_native);
        let (new_event, new_body) = telemetry_turn(new_messages, is_native);
        for event in [&old_event, &new_event] {
            assert_eq!(event["ok"], true, "{id}");
            assert_eq!(event["provider"], "openai-compatible", "{id}");
            assert_eq!(event["native_tools_enabled"], is_native, "{id}");
            assert_eq!(
                event["tools"],
                entry(&corpus, "telemetry", id)["tools"],
                "{id}"
            );
        }
        assert_eq!(
            old_event["estimated_prompt_tokens_sent"],
            pinned(id),
            "{id}: the base request still measures as at the base SHA"
        );
        assert_eq!(schema_tools(&old_body), schema_tools(&new_body), "{id}");
        assert_eq!(
            old_body["messages"].as_array().unwrap()[1..],
            new_body["messages"].as_array().unwrap()[1..],
            "{id}: only the system message differs"
        );
        let old = old_event["estimated_prompt_tokens_sent"].as_u64().unwrap();
        let new = new_event["estimated_prompt_tokens_sent"].as_u64().unwrap();
        eprintln!("{id}: estimated_prompt_tokens_sent {old} -> {new}");
        if is_native {
            assert!(new < old, "{id}: {old} -> {new}");
        } else {
            assert_eq!(new, old, "{id}");
        }
    }
}

// ---------------------------------------------------------------------------
// Test 6: the corpus.

#[test]
fn corpus_is_synthetic_and_pinned_to_the_base_sha() {
    let root = corpus_root();
    let manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("source-manifest.json")).unwrap())
            .unwrap();
    let bytes = std::fs::read(root.join("fixtures/cases.json")).unwrap();
    assert_eq!(
        manifest["fixture_sha256"]["cases.json"],
        format!("{:x}", Sha256::digest(&bytes))
    );
    let fixture = String::from_utf8(bytes).unwrap();
    for forbidden in [
        "/Users/",
        "/Volumes/",
        "/home/",
        "/private/",
        "Bearer",
        "sk-",
    ] {
        assert!(!fixture.contains(forbidden), "fixture contains {forbidden}");
    }
    let corpus = corpus();
    assert_eq!(corpus["kind"], "synthetic");
    assert_eq!(manifest["kind"], "synthetic");
    assert_eq!(corpus["base_sha"], manifest["base_sha"]);
    assert_eq!(corpus["root"], ROOT);
    let ids = corpus["systems"]
        .as_array()
        .unwrap()
        .iter()
        .map(|system| system["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    let mut expected = BUILDER_CASES.iter().map(|case| case.0).collect::<Vec<_>>();
    expected.extend(["loop-native", "loop-xml"]);
    assert_eq!(ids, expected);
}
