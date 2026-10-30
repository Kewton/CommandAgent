//! Issue #918: `--request-pattern jev` classifies a direct `--prompt` request
//! once, before the run, and an adopted pattern only adds a "Request guidance"
//! block to the system prompt. jev is replayed by a local mock server from the
//! synthetic corpus `tests/corpus/apps/request-pattern-routing/`; no test
//! reaches the real API.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use commandagent::config::{
    Action, Config, ConfigFieldSources, IntentId, NarrationMode, OllamaThink, OpenAiApi,
    PlanPreset, PromptLayout, Provider,
};
use commandagent::minimal_loop::loop_run::{RunSessionOutcome, run_session_with_outcome_with_ui};
use commandagent::minimal_loop::request_pattern::{
    JevClassifier, PatternClassifier, RequestPattern, RequestPatternMode, RequestPatternSettings,
    request_body, run_direct_prompt_with,
};
use commandagent::providers::{AssistantReply, ChatClient};
use commandagent::state::{ConversationMessage, SessionSnapshot, SessionStore, ToolCall};
use commandagent::tools::registry::ToolSpec;
use commandagent::tui::NOOP_UI;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const CORPUS: &str = "tests/corpus/apps/request-pattern-routing";
const REQUEST: &str = "Create notes.md with a two line summary of the project.";
const TEST_KEY: &str = "test-key-918-not-a-real-credential";
const WORKSPACE_SECRET: &str = "workspace-file-content-918";
const EVENT: &str = "request_pattern_resolved";

#[derive(Clone)]
struct RecordingClient {
    replies: Arc<Mutex<Vec<AssistantReply>>>,
    requests: Arc<Mutex<Vec<Vec<ConversationMessage>>>>,
}

impl RecordingClient {
    fn new(replies: Vec<AssistantReply>) -> Self {
        Self {
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
        true
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn chat(
        &mut self,
        _model: &str,
        messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        self.requests.lock().unwrap().push(messages.to_vec());
        let mut replies = self.replies.lock().unwrap();
        if replies.is_empty() {
            anyhow::bail!("scripted client exhausted");
        }
        Ok(replies.remove(0))
    }
}

fn tool_reply(name: &str, arguments: Value) -> AssistantReply {
    AssistantReply {
        content: String::new(),
        tool_calls: vec![ToolCall::new(name, arguments)],
        prompt_tokens: None,
        completion_tokens: None,
    }
}

fn text_reply(content: &str) -> AssistantReply {
    AssistantReply {
        content: content.to_string(),
        tool_calls: Vec::new(),
        prompt_tokens: None,
        completion_tokens: None,
    }
}

/// Writes the requested file, then answers.
fn writing_script() -> Vec<AssistantReply> {
    vec![
        tool_reply(
            "Write",
            json!({"path": "notes.md", "content": "# Notes\nA small project.\n"}),
        ),
        text_reply("Created notes.md."),
        text_reply("Created notes.md."),
    ]
}

/// Never writes anything.
fn idle_script() -> Vec<AssistantReply> {
    vec![
        text_reply("I would create notes.md."),
        text_reply("Done."),
        text_reply("Done."),
    ]
}

fn config(root: &Path, offline: bool) -> Config {
    Config {
        workspace_root: root.to_path_buf(),
        state_dir: root.join("state"),
        eval_events_path: Some(root.join("events.jsonl")),
        completion_contract_path: None,
        yes: true,
        offline,
        context_budget: 100_000,
        model: "m".to_string(),
        provider: Provider::Ollama,
        tool_protocol: None,
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
        max_iterations: 4,
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
        action: Action::Prompt(REQUEST.to_string()),
    }
}

fn jev(threshold: f64) -> RequestPatternSettings {
    RequestPatternSettings {
        mode: RequestPatternMode::Jev,
        threshold,
    }
}

/// A one-shot jev stand-in on loopback that records the raw HTTP request.
struct MockJev {
    url: String,
    received: Arc<Mutex<Option<Vec<u8>>>>,
    handle: Option<JoinHandle<()>>,
}

impl MockJev {
    fn start(status: u16, body: &str, delay: Duration) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1/systemone", listener.local_addr().unwrap());
        let received = Arc::new(Mutex::new(None));
        let sink = Arc::clone(&received);
        let body = body.to_string();
        listener.set_nonblocking(true).unwrap();
        let handle = std::thread::spawn(move || {
            // Wait a bounded time, so a run that never calls jev cannot hang.
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(_) if std::time::Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => return,
                }
            };
            stream.set_nonblocking(false).unwrap();
            *sink.lock().unwrap() = Some(read_request(&mut stream));
            std::thread::sleep(delay);
            let reason = if status == 200 { "OK" } else { "Error" };
            let response = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        });
        Self {
            url,
            received,
            handle: Some(handle),
        }
    }

    fn classifier(&self, timeout: Duration) -> JevClassifier {
        JevClassifier::new(self.url.clone(), TEST_KEY, timeout)
    }

    fn request(&mut self) -> Option<(String, Value)> {
        if let Some(handle) = self.handle.take() {
            handle.join().unwrap();
        }
        let raw = self.received.lock().unwrap().clone()?;
        let text = String::from_utf8(raw).unwrap();
        let (head, body) = text.split_once("\r\n\r\n").unwrap();
        Some((head.to_string(), serde_json::from_str(body).unwrap()))
    }
}

fn read_request(stream: &mut TcpStream) -> Vec<u8> {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut data = Vec::new();
    let mut buffer = [0u8; 4096];
    loop {
        let Ok(count) = stream.read(&mut buffer) else {
            return data;
        };
        if count == 0 {
            return data;
        }
        data.extend_from_slice(&buffer[..count]);
        let text = String::from_utf8_lossy(&data);
        if let Some(end) = text.find("\r\n\r\n") {
            let length = text[..end]
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);
            if data.len() >= end + 4 + length {
                return data;
            }
        }
    }
}

struct Run {
    dir: tempfile::TempDir,
    result: Result<RunSessionOutcome, String>,
    requests: Vec<Vec<ConversationMessage>>,
    session: SessionSnapshot,
}

impl Run {
    fn events(&self) -> Vec<Value> {
        std::fs::read_to_string(self.dir.path().join("events.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect()
    }

    fn resolved(&self) -> Vec<Value> {
        self.events()
            .into_iter()
            .filter(|event| event["event"] == EVENT)
            .collect()
    }

    /// Event names other than `request_pattern_resolved`, in order.
    fn other_event_names(&self) -> Vec<String> {
        self.events()
            .iter()
            .filter(|event| event["event"] != EVENT)
            .map(|event| event["event"].as_str().unwrap_or("").to_string())
            .collect()
    }

    /// Each run has its own temporary workspace; its path is the only part of
    /// a request that differs between two otherwise identical runs.
    fn without_root(&self, text: &str) -> String {
        text.replace(&*self.dir.path().to_string_lossy(), "<root>")
    }

    fn system_prompts(&self) -> Vec<String> {
        self.requests
            .iter()
            .map(|messages| {
                assert_eq!(messages[0].role, "system");
                self.without_root(&messages[0].content)
            })
            .collect()
    }

    /// Every request message as (role, content), workspace path normalized.
    fn request_texts(&self) -> Vec<Vec<(String, String)>> {
        self.requests
            .iter()
            .map(|messages| {
                messages
                    .iter()
                    .map(|message| (message.role.clone(), self.without_root(&message.content)))
                    .collect()
            })
            .collect()
    }

    /// Everything the run decided, without the system prompt.
    fn decision(&self) -> String {
        match &self.result {
            Ok(outcome) => format!(
                "ok final={:?} stop={:?} changed={:?} iterations={} tools={} missing={:?}/{:?}/{:?}/{:?} verify={} blocking={:?} provider_error={:?}",
                outcome.final_text,
                outcome.stop_reason,
                outcome.changed_paths,
                outcome.iterations,
                outcome.tool_calls,
                outcome.missing_required_paths,
                outcome.missing_capabilities,
                outcome.missing_evidence,
                outcome.missing_obligations,
                outcome.verify_attempts,
                outcome.last_blocking_reason,
                outcome.last_provider_error,
            ),
            Err(error) => format!("err {error}"),
        }
    }

    fn notes(&self) -> Option<String> {
        std::fs::read_to_string(self.dir.path().join("notes.md")).ok()
    }
}

fn workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("existing.txt"), WORKSPACE_SECRET).unwrap();
    dir
}

/// The direct `--prompt` run as it was before Issue #918.
fn run_plain(config_of: impl Fn(&Path) -> Config, replies: Vec<AssistantReply>) -> Run {
    let dir = workspace();
    let mut client = RecordingClient::new(replies);
    let mut session = SessionSnapshot::new();
    let result = run_session_with_outcome_with_ui(
        &mut client,
        &mut session,
        REQUEST,
        &[],
        &config_of(dir.path()),
        &NOOP_UI,
    )
    .map_err(|error| format!("{error:#}"));
    let requests = client.requests.lock().unwrap().clone();
    Run {
        dir,
        result,
        requests,
        session,
    }
}

fn run_routed(
    config_of: impl Fn(&Path) -> Config,
    replies: Vec<AssistantReply>,
    settings: RequestPatternSettings,
    classifier: Option<&dyn PatternClassifier>,
) -> Run {
    let dir = workspace();
    let mut client = RecordingClient::new(replies);
    let mut session = SessionSnapshot::new();
    let result = run_direct_prompt_with(
        &mut client,
        &mut session,
        REQUEST,
        &config_of(dir.path()),
        &NOOP_UI,
        settings,
        classifier,
    )
    .map_err(|error| format!("{error:#}"));
    let requests = client.requests.lock().unwrap().clone();
    Run {
        dir,
        result,
        requests,
        session,
    }
}

/// A classifier that always answers `pattern` with full confidence and counts
/// its calls.
struct Fixed {
    pattern: RequestPattern,
    calls: Mutex<usize>,
}

impl Fixed {
    fn new(pattern: RequestPattern) -> Self {
        Self {
            pattern,
            calls: Mutex::new(0),
        }
    }
}

impl PatternClassifier for Fixed {
    fn classify(
        &self,
        request: &str,
    ) -> Result<
        commandagent::minimal_loop::request_pattern::Classification,
        commandagent::minimal_loop::request_pattern::ClassifyFailure,
    > {
        assert_eq!(request, REQUEST);
        *self.calls.lock().unwrap() += 1;
        Ok(
            commandagent::minimal_loop::request_pattern::Classification {
                pattern: Some(self.pattern),
                confidence: 1.0,
                model: Some("jev-1.14.0".to_string()),
            },
        )
    }
}

fn block(pattern: RequestPattern) -> String {
    format!("\n\nRequest guidance:\n{}", pattern.guidance())
}

fn corpus() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(CORPUS)
        .join("fixtures/cases.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// Test 1: `off` (the default) sends byte-identical requests and records the
/// same events as the plain direct run, with no new event.
#[test]
fn off_mode_is_byte_identical_to_the_plain_direct_run() {
    for script in [writing_script(), idle_script()] {
        let plain = run_plain(|root| config(root, false), script.clone());
        let off = run_routed(
            |root| config(root, false),
            script,
            RequestPatternSettings::OFF,
            Some(&Fixed::new(RequestPattern::Create)),
        );
        assert!(!off.requests.is_empty());
        assert_eq!(off.request_texts(), plain.request_texts());
        assert_eq!(off.decision(), plain.decision());
        assert_eq!(off.other_event_names(), plain.other_event_names());
        assert!(off.resolved().is_empty());
        assert!(
            off.system_prompts()
                .iter()
                .all(|prompt| !prompt.contains("Request guidance"))
        );
    }
}

/// Tests 2 and 3, driven by the synthetic corpus through a loopback jev.
#[test]
fn corpus_cases_select_guidance_or_fall_back_to_default() {
    let corpus = corpus();
    let threshold = corpus["threshold"].as_f64().unwrap();
    assert_eq!(corpus["guidance_version"], "v1");
    let cases = corpus["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 12);
    for case in cases {
        let id = case["id"].as_str().unwrap();
        let expect = &case["expect"];
        let offline = case["offline"].as_bool().unwrap_or(false);
        let no_key = case["no_key"].as_bool().unwrap_or(false);
        let mut server = (!no_key).then(|| {
            MockJev::start(
                case["http_status"].as_u64().unwrap() as u16,
                case["body"].as_str().unwrap(),
                Duration::from_millis(case["delay_ms"].as_u64().unwrap_or(0)),
            )
        });
        let classifier = server
            .as_ref()
            .map(|server| server.classifier(Duration::from_millis(400)));
        let routed = run_routed(
            |root| config(root, offline),
            writing_script(),
            jev(threshold),
            classifier.as_ref().map(|c| c as &dyn PatternClassifier),
        );
        let plain = run_plain(|root| config(root, offline), writing_script());

        let resolved = routed.resolved();
        assert_eq!(resolved.len(), 1, "{id}: {resolved:?}");
        let event = &resolved[0];
        assert_eq!(event["source"], "jev", "{id}");
        assert_eq!(event["adopted"], expect["adopted"], "{id}");
        assert_eq!(event["pattern"], expect["pattern"], "{id}");
        assert_eq!(event["reason"], expect["reason"], "{id}");
        assert_eq!(event["confidence"], expect["confidence"], "{id}");
        assert_eq!(event["model"], expect["model"], "{id}");
        let adopted = expect["adopted"].as_bool().unwrap();
        assert_eq!(
            event["guidance_version"],
            if adopted { json!("v1") } else { Value::Null },
            "{id}"
        );
        assert!(event["latency_ms"].is_u64(), "{id}");

        let routed_prompts = routed.system_prompts();
        let plain_prompts = plain.system_prompts();
        assert_eq!(routed_prompts.len(), plain_prompts.len(), "{id}");
        let expected_block = RequestPattern::from_label(expect["pattern"].as_str().unwrap())
            .map(block)
            .unwrap_or_default();
        assert_eq!(
            expect["request_guidance_section"].as_bool().unwrap(),
            !expected_block.is_empty(),
            "{id}"
        );
        for (routed_prompt, plain_prompt) in routed_prompts.iter().zip(&plain_prompts) {
            assert_eq!(
                routed_prompt,
                &format!("{plain_prompt}{expected_block}"),
                "{id}"
            );
            let sections = routed_prompt.matches("Request guidance:").count();
            assert_eq!(sections, usize::from(adopted), "{id}");
        }
        for (routed_request, plain_request) in
            routed.request_texts().iter().zip(&plain.request_texts())
        {
            assert_eq!(
                routed_request[1..],
                plain_request[1..],
                "{id}: history kept"
            );
        }
        assert_eq!(
            routed.decision(),
            plain.decision(),
            "{id}: the run continues"
        );
        assert_eq!(
            routed.other_event_names(),
            plain.other_event_names(),
            "{id}"
        );

        if let Some(server) = server.as_mut()
            && case["delay_ms"].is_null()
        {
            let received = server.request();
            assert_eq!(
                received.is_some(),
                !offline,
                "{id}: offline never calls jev"
            );
        }
    }
}

/// Test 4: the request carries the request text and one question, the key is
/// only in the Authorization header, and no event, log line, or saved session
/// carries the key or a copy of the request from the classifier.
#[test]
fn jev_receives_only_the_request_and_the_key_stays_in_the_header() {
    let body = r#"{"model":"jev-1.14.0","answers":{"work":{"choice":"CREATE","confidence":0.97}}}"#;
    let mut server = MockJev::start(200, body, Duration::ZERO);
    let classifier = server.classifier(Duration::from_secs(3));
    let routed = run_routed(
        |root| config(root, false),
        writing_script(),
        jev(0.8),
        Some(&classifier),
    );
    let (head, sent) = server.request().expect("jev was called once");
    assert_eq!(sent, request_body(REQUEST));
    assert!(
        head.starts_with("POST /v1/systemone HTTP/1.1\r\n"),
        "{head}"
    );
    let authorization = head
        .lines()
        .filter_map(|line| line.split_once(':'))
        .filter(|(name, _)| name.eq_ignore_ascii_case("authorization"))
        .map(|(_, value)| value.trim().to_string())
        .collect::<Vec<_>>();
    assert_eq!(authorization, [format!("Bearer {TEST_KEY}")]);
    let other_headers = head
        .lines()
        .filter(|line| !line.to_ascii_lowercase().starts_with("authorization:"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!other_headers.contains(TEST_KEY));
    assert!(
        !head.lines().next().unwrap().contains(TEST_KEY),
        "not in the URL"
    );
    let sent_text = sent.to_string();
    let root = routed.dir.path().to_string_lossy().to_string();
    for leaked in [TEST_KEY, WORKSPACE_SECRET, "existing.txt", root.as_str()] {
        assert!(!sent_text.contains(leaked), "body carries {leaked}");
    }

    let events = std::fs::read_to_string(routed.dir.path().join("events.jsonl")).unwrap();
    assert!(!events.contains(TEST_KEY));
    let resolved = routed.resolved();
    assert_eq!(resolved.len(), 1);
    assert!(!resolved[0].to_string().contains(REQUEST));
    assert_eq!(resolved[0]["pattern"], "CREATE");

    let store = SessionStore::new(routed.dir.path().join("state"));
    store.save(&routed.session).unwrap();
    for entry in walk(&routed.dir.path().join("state")) {
        let saved = std::fs::read_to_string(&entry).unwrap_or_default();
        assert!(!saved.contains(TEST_KEY), "{}", entry.display());
        assert!(!saved.contains("Request guidance"), "{}", entry.display());
    }
    assert!(
        routed
            .session
            .messages
            .iter()
            .all(|message| !message.content.contains("Request guidance"))
    );
    assert_eq!(
        routed
            .session
            .messages
            .iter()
            .filter(|message| message.content == REQUEST)
            .count(),
        1,
        "the request is stored once, as before"
    );
}

/// A secret registered in the run scope (here a workspace `.env` value) is
/// replaced in the request text before jev sees it, exactly as it is in the
/// request the model provider gets.
#[test]
fn registered_secrets_are_scrubbed_from_the_request_before_jev() {
    const SECRET: &str = "fake-dotenv-secret-918-q7Zr4mLx9Kp2";
    let request = format!("Create db.conf that connects with password {SECRET}.");
    let body = r#"{"model":"jev-1.14.0","answers":{"work":{"choice":"CREATE","confidence":0.97}}}"#;
    let mut server = MockJev::start(200, body, Duration::ZERO);
    let classifier = server.classifier(Duration::from_secs(3));
    let dir = workspace();
    std::fs::write(dir.path().join(".env"), format!("DB_PASSWORD={SECRET}\n")).unwrap();
    let mut client = RecordingClient::new(writing_script());
    let mut session = SessionSnapshot::new();
    run_direct_prompt_with(
        &mut client,
        &mut session,
        &request,
        &config(dir.path(), false),
        &NOOP_UI,
        jev(0.8),
        Some(&classifier),
    )
    .unwrap();
    let marker = commandagent::sensitive_data::active_marker();
    let (_, sent) = server.request().expect("jev was called once");
    let state = sent["state"].as_str().unwrap();
    assert!(!state.contains(SECRET), "{state}");
    assert!(state.contains(&marker), "{state}");
    assert_eq!(state, request.replace(SECRET, &marker));
    assert!(!sent.to_string().contains(SECRET));
    let model_request = &client.requests.lock().unwrap()[0];
    let model_text = model_request
        .iter()
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!model_text.contains(SECRET));
}

const JEV_KEY_CHILD_ENV: &str = "COMMANDAGENT_ISSUE918_JEV_KEY_CHILD";
const JEV_KEY_CHILD_TEST: &str = "jev_api_key_is_scrubbed_like_the_provider_keys";
const FAKE_JEV_KEY: &str = "fake-jev-key-918-Hq3Wn8Tz5Vb1Lc6R";
const FAKE_OPENAI_KEY: &str = "fake-openai-key-918-Jx2Pd7Ys4Fm9Gk3N";

/// `JEV_API_KEY` joins the run's secret catalog like the provider keys, so a
/// tool result that prints the environment cannot carry the key onward. The
/// fake keys live only in a child with a cleared environment, so no other test
/// sees them.
#[test]
fn jev_api_key_is_scrubbed_like_the_provider_keys() {
    if std::env::var_os(JEV_KEY_CHILD_ENV).is_none() {
        let home = tempfile::tempdir().unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", JEV_KEY_CHILD_TEST, "--nocapture"])
            .env_clear()
            .env("HOME", home.path())
            .env(JEV_KEY_CHILD_ENV, "1")
            .env("JEV_API_KEY", FAKE_JEV_KEY)
            .env("OPENAI_API_KEY", FAKE_OPENAI_KEY)
            .current_dir(home.path())
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains("1 passed"),
            "child failed:\n{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let dir = workspace();
    let scope = commandagent::config::install_run_secret_scope(&config(dir.path(), false)).unwrap();
    let printed = format!("JEV_API_KEY={FAKE_JEV_KEY}\nOPENAI_API_KEY={FAKE_OPENAI_KEY}\n");
    let scrubbed = scope.scrub_text(&printed);
    assert!(!scrubbed.contains(FAKE_JEV_KEY), "{scrubbed}");
    assert!(!scrubbed.contains(FAKE_OPENAI_KEY), "{scrubbed}");
    assert_eq!(
        scrubbed,
        printed
            .replace(FAKE_JEV_KEY, scope.marker())
            .replace(FAKE_OPENAI_KEY, scope.marker())
    );
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return files;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files.extend(walk(&path));
        } else {
            files.push(path);
        }
    }
    files
}

/// Test 5: the pattern never changes tool permissions or completion: an
/// ANSWER run may still write, a CREATE run that writes nothing ends exactly
/// like the plain run, and every pattern decides the same as the plain run.
#[test]
fn the_pattern_changes_neither_permissions_nor_completion() {
    for script in [writing_script(), idle_script()] {
        let plain = run_plain(|root| config(root, false), script.clone());
        for pattern in RequestPattern::ALL {
            let routed = run_routed(
                |root| config(root, false),
                script.clone(),
                jev(0.8),
                Some(&Fixed::new(pattern)),
            );
            assert_eq!(routed.resolved().len(), 1);
            assert_eq!(routed.resolved()[0]["pattern"], pattern.label());
            assert_eq!(routed.decision(), plain.decision(), "{pattern:?}");
            assert_eq!(routed.notes(), plain.notes(), "{pattern:?}");
            assert_eq!(
                routed.other_event_names(),
                plain.other_event_names(),
                "{pattern:?}"
            );
        }
    }
    let answer = run_routed(
        |root| config(root, false),
        writing_script(),
        jev(0.8),
        Some(&Fixed::new(RequestPattern::Answer)),
    );
    assert_eq!(
        answer.notes().as_deref(),
        Some("# Notes\nA small project.\n"),
        "ANSWER keeps Write allowed"
    );
}

/// Test 5 for the #913 after-tool decision: a repair request on an existing
/// file decides the same with every pattern as without routing.
#[test]
fn the_pattern_does_not_change_the_after_tool_decision() {
    let repair = "Fix the typo in existing.txt so it reads correctly.";
    let script = || {
        vec![
            tool_reply("Bash", json!({"command": "cat existing.txt"})),
            text_reply("The file looks fine."),
            text_reply("Done."),
            text_reply("Done."),
        ]
    };
    let run = |classifier: Option<&dyn PatternClassifier>, settings| {
        let dir = workspace();
        let mut client = RecordingClient::new(script());
        let mut session = SessionSnapshot::new();
        let result = run_direct_prompt_with(
            &mut client,
            &mut session,
            repair,
            &config(dir.path(), false),
            &NOOP_UI,
            settings,
            classifier,
        )
        .map_err(|error| format!("{error:#}"));
        let requests = client.requests.lock().unwrap().clone();
        Run {
            dir,
            result,
            requests,
            session,
        }
    };
    struct Any(RequestPattern);
    impl PatternClassifier for Any {
        fn classify(
            &self,
            _request: &str,
        ) -> Result<
            commandagent::minimal_loop::request_pattern::Classification,
            commandagent::minimal_loop::request_pattern::ClassifyFailure,
        > {
            Ok(
                commandagent::minimal_loop::request_pattern::Classification {
                    pattern: Some(self.0),
                    confidence: 1.0,
                    model: None,
                },
            )
        }
    }
    let plain = run(None, RequestPatternSettings::OFF);
    for pattern in RequestPattern::ALL {
        let routed = run(Some(&Any(pattern)), jev(0.8));
        assert_eq!(routed.resolved().len(), 1);
        assert_eq!(routed.decision(), plain.decision(), "{pattern:?}");
        assert_eq!(
            routed.other_event_names(),
            plain.other_event_names(),
            "{pattern:?}"
        );
        let stops = |run: &Run| {
            run.events()
                .into_iter()
                .filter(|event| event["event"] == "loop_stop")
                .map(|event| event["reason"].clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(stops(&routed), stops(&plain), "{pattern:?}");
    }
}

/// Test 6: an explicit `--intent` never calls the classifier and leaves the run
/// unchanged.
#[test]
fn explicit_intent_never_calls_the_classifier() {
    for intent in [IntentId::Create, IntentId::Fix, IntentId::Investigate] {
        let with_intent = |root: &Path| Config {
            intent_override: Some(intent),
            ..config(root, false)
        };
        let classifier = Fixed::new(RequestPattern::Modify);
        let routed = run_routed(with_intent, writing_script(), jev(0.8), Some(&classifier));
        let plain = run_plain(with_intent, writing_script());
        assert_eq!(*classifier.calls.lock().unwrap(), 0, "{intent:?}");
        assert!(routed.resolved().is_empty(), "{intent:?}");
        assert_eq!(
            routed.system_prompts(),
            plain.system_prompts(),
            "{intent:?}"
        );
        assert_eq!(routed.decision(), plain.decision(), "{intent:?}");
    }
}

/// The corpus manifest pins the fixture digest and records no request text.
#[test]
fn corpus_manifest_pins_the_synthetic_fixture() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(CORPUS);
    let manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("source-manifest.json")).unwrap())
            .unwrap();
    let bytes = std::fs::read(root.join("fixtures/cases.json")).unwrap();
    let digest = format!("{:x}", Sha256::digest(&bytes));
    assert_eq!(manifest["fixture_sha256"]["cases.json"], digest);
    assert_eq!(
        manifest["dataset"]["sha256"],
        "6097c5855ccc14648006b5afb7f199b42c3040cad025a6e071fc22de38b6e440"
    );
}
