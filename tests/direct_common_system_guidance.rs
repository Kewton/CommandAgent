//! Issue #925: `COMMANDAGENT_SYSTEM_GUIDANCE` adds one common guidance text to
//! the system prompt of every provider request of a direct `--prompt` run. The
//! in-process tests pass the checked value straight to the direct entry, and the
//! child-process tests set it only in a child's cleared environment, so no test
//! changes this process's environment. Providers are scripted or a loopback
//! Ollama stand-in replaying the synthetic corpus
//! `tests/corpus/apps/direct-common-system-guidance/`.

use std::collections::{BTreeSet, VecDeque};
use std::ffi::OsString;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use commandagent::config::{
    Action, Config, ConfigFieldSources, IntentId, NarrationMode, OllamaThink, OpenAiApi,
    PlanPreset, PromptLayout, Provider,
};
use commandagent::minimal_loop::loop_run::{RunSessionOutcome, run_session_with_outcome_with_ui};
use commandagent::minimal_loop::request_pattern::common_guidance::{
    COMMON_HEADING, SYSTEM_GUIDANCE_ENV, parse,
};
use commandagent::minimal_loop::request_pattern::{
    Classification, ClassifyFailure, PatternClassifier, RequestPattern, RequestPatternMode,
    RequestPatternSettings, run_direct_prompt_with, run_direct_prompt_with_common,
};
use commandagent::providers::{AssistantReply, ChatClient};
use commandagent::state::{ConversationMessage, SessionSnapshot, ToolCall};
use commandagent::tools::registry::ToolSpec;
use commandagent::tui::NOOP_UI;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const CORPUS: &str = "tests/corpus/apps/direct-common-system-guidance";
const REQUEST: &str = "Create notes.md with a two line summary of the project.";
const EVENT: &str = "request_pattern_resolved";

fn corpus() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(CORPUS)
        .join("fixtures/cases.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// A guidance text of the synthetic corpus.
fn guidance(id: &str) -> String {
    let corpus = corpus();
    corpus["guidance"][id]
        .as_str()
        .unwrap_or_else(|| panic!("missing guidance {id}"))
        .to_string()
}

#[derive(Clone)]
struct RecordingClient {
    native: bool,
    replies: Arc<Mutex<Vec<AssistantReply>>>,
    requests: Arc<Mutex<Vec<Vec<ConversationMessage>>>>,
    tools: Arc<Mutex<Vec<Vec<String>>>>,
}

impl RecordingClient {
    fn new(native: bool, replies: Vec<AssistantReply>) -> Self {
        Self {
            native,
            replies: Arc::new(Mutex::new(replies)),
            requests: Arc::new(Mutex::new(Vec::new())),
            tools: Arc::new(Mutex::new(Vec::new())),
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

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn chat(
        &mut self,
        _model: &str,
        messages: &[ConversationMessage],
        tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        self.requests.lock().unwrap().push(messages.to_vec());
        self.tools.lock().unwrap().push(
            tools
                .iter()
                .map(|tool| tool.function.name.clone())
                .collect(),
        );
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

/// Writes, reads, and runs a command before answering: four model turns.
fn tool_chain_script() -> Vec<AssistantReply> {
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

/// Never writes anything.
fn idle_script() -> Vec<AssistantReply> {
    vec![
        text_reply("I would create notes.md."),
        text_reply("Done."),
        text_reply("Done."),
    ]
}

/// Tries to write outside the workspace, then claims success.
fn escaping_script() -> Vec<AssistantReply> {
    vec![
        tool_reply(
            "Write",
            json!({"path": "../outside-925.md", "content": "escape\n"}),
        ),
        text_reply("Created notes.md."),
        text_reply("Created notes.md."),
    ]
}

/// Three empty replies schedule the fresh-session retry, which then writes.
fn empty_then_fresh_retry_script() -> Vec<AssistantReply> {
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

fn config(root: &Path) -> Config {
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
        action: Action::Prompt(REQUEST.to_string()),
    }
}

fn jev() -> RequestPatternSettings {
    RequestPatternSettings {
        mode: RequestPatternMode::Jev,
        threshold: 0.8,
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

    fn calls(&self) -> usize {
        *self.calls.lock().unwrap()
    }
}

impl PatternClassifier for Fixed {
    fn classify(&self, request: &str) -> Result<Classification, ClassifyFailure> {
        assert_eq!(request, REQUEST, "the classifier receives the request only");
        *self.calls.lock().unwrap() += 1;
        Ok(Classification {
            pattern: Some(self.pattern),
            confidence: 1.0,
            model: Some("jev-1.14.0".to_string()),
        })
    }
}

struct Run {
    dir: tempfile::TempDir,
    result: Result<RunSessionOutcome, String>,
    requests: Vec<Vec<ConversationMessage>>,
    tools: Vec<Vec<String>>,
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

    fn event_names(&self) -> Vec<String> {
        self.events()
            .iter()
            .map(|event| event["event"].as_str().unwrap_or("").to_string())
            .collect()
    }

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

    /// Every request message after the system prompt, as (role, content,
    /// tool calls). Only what differs between two identical runs is
    /// normalized: the workspace path, the generated tool call ids, and the
    /// measured `elapsed_ms` of a command result.
    fn histories(&self) -> Vec<Vec<(String, String, String)>> {
        self.requests
            .iter()
            .map(|messages| {
                messages[1..]
                    .iter()
                    .map(|message| {
                        let content = self
                            .without_root(&message.content)
                            .lines()
                            .map(|line| {
                                if line.starts_with("elapsed_ms: ") {
                                    "elapsed_ms: <ms>"
                                } else {
                                    line
                                }
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        let calls = message
                            .tool_calls
                            .iter()
                            .map(|call| format!("{}({})", call.name, call.arguments))
                            .collect::<Vec<_>>()
                            .join(",");
                        (message.role.clone(), content, calls)
                    })
                    .collect()
            })
            .collect()
    }

    /// Everything the run decided, without the requests.
    fn decision(&self) -> String {
        match &self.result {
            Ok(outcome) => self.without_root(&format!(
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
            )),
            Err(error) => self.without_root(&format!("err {error}")),
        }
    }

    /// How often `text` appears in each request, over all its messages.
    fn occurrences(&self, text: &str) -> Vec<usize> {
        self.requests
            .iter()
            .map(|messages| {
                messages
                    .iter()
                    .map(|message| message.content.matches(text).count())
                    .sum()
            })
            .collect()
    }
}

fn workspace() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn finish(
    dir: tempfile::TempDir,
    client: RecordingClient,
    session: SessionSnapshot,
    result: anyhow::Result<RunSessionOutcome>,
) -> Run {
    let requests = client.requests.lock().unwrap().clone();
    let tools = client.tools.lock().unwrap().clone();
    Run {
        dir,
        result: result.map_err(|error| format!("{error:#}")),
        requests,
        tools,
        session,
    }
}

/// The direct `--prompt` run as it was before Issue #918.
fn run_plain(
    config_of: impl Fn(&Path) -> Config,
    native: bool,
    script: Vec<AssistantReply>,
) -> Run {
    let dir = workspace();
    let mut client = RecordingClient::new(native, script);
    let mut session = SessionSnapshot::new();
    let result = run_session_with_outcome_with_ui(
        &mut client,
        &mut session,
        REQUEST,
        &[],
        &config_of(dir.path()),
        &NOOP_UI,
    );
    finish(dir, client, session, result)
}

/// The direct `--prompt` run as it was before Issue #925.
fn run_routed(
    config_of: impl Fn(&Path) -> Config,
    native: bool,
    script: Vec<AssistantReply>,
    settings: RequestPatternSettings,
    classifier: Option<&dyn PatternClassifier>,
) -> Run {
    let dir = workspace();
    let mut client = RecordingClient::new(native, script);
    let mut session = SessionSnapshot::new();
    let result = run_direct_prompt_with(
        &mut client,
        &mut session,
        REQUEST,
        &config_of(dir.path()),
        &NOOP_UI,
        settings,
        classifier,
    );
    finish(dir, client, session, result)
}

fn run_common(
    config_of: impl Fn(&Path) -> Config,
    native: bool,
    script: Vec<AssistantReply>,
    settings: RequestPatternSettings,
    classifier: Option<&dyn PatternClassifier>,
    common: Option<&str>,
) -> Run {
    run_common_in(
        workspace(),
        config_of,
        native,
        script,
        settings,
        classifier,
        common,
    )
}

fn run_common_in(
    dir: tempfile::TempDir,
    config_of: impl Fn(&Path) -> Config,
    native: bool,
    script: Vec<AssistantReply>,
    settings: RequestPatternSettings,
    classifier: Option<&dyn PatternClassifier>,
    common: Option<&str>,
) -> Run {
    let mut client = RecordingClient::new(native, script);
    let mut session = SessionSnapshot::new();
    let result = run_direct_prompt_with_common(
        &mut client,
        &mut session,
        REQUEST,
        &config_of(dir.path()),
        &NOOP_UI,
        settings,
        classifier,
        common,
    );
    finish(dir, client, session, result)
}

fn request_block(text: &str) -> String {
    format!("\n\nRequest guidance:\n{text}")
}

/// Removes the one request guidance block holding `common` from a system
/// prompt; native prompts end with it, XML prompts keep the XML note after it.
fn without_block(prompt: &str, block: &str) -> String {
    assert_eq!(prompt.matches(block).count(), 1, "{prompt}");
    prompt.replacen(block, "", 1)
}

/// Test 1: unset or empty, every request (system prompt, history, tool list)
/// and the run's decision and events are the same as the plain direct run's,
/// in `off` and for every jev answer and fallback.
#[test]
fn unset_or_empty_common_guidance_keeps_every_request_byte_identical() {
    let empty = parse(Some(OsString::from("").as_os_str())).unwrap();
    assert_eq!(empty, None);
    for native in [true, false] {
        for script in [writing_script(), tool_chain_script(), idle_script()] {
            let plain = run_plain(config, native, script.clone());
            for common in [None, empty.as_deref()] {
                let run = run_common(
                    config,
                    native,
                    script.clone(),
                    RequestPatternSettings::OFF,
                    Some(&Fixed::new(RequestPattern::Create)),
                    common,
                );
                assert!(run.requests.len() >= 2);
                assert_eq!(run.requests.len(), plain.requests.len());
                assert_eq!(run.system_prompts(), plain.system_prompts());
                assert_eq!(run.histories(), plain.histories());
                assert_eq!(run.tools, plain.tools);
                assert_eq!(run.decision(), plain.decision());
                assert_eq!(run.event_names(), plain.event_names());
                assert!(
                    run.system_prompts()
                        .iter()
                        .all(|prompt| !prompt.contains("Request guidance")
                            && !prompt.contains("Common guidance"))
                );
            }
        }
        for pattern in RequestPattern::ALL {
            let before = run_routed(
                config,
                native,
                writing_script(),
                jev(),
                Some(&Fixed::new(pattern)),
            );
            let after = run_common(
                config,
                native,
                writing_script(),
                jev(),
                Some(&Fixed::new(pattern)),
                None,
            );
            assert_eq!(after.system_prompts(), before.system_prompts());
            assert_eq!(after.histories(), before.histories());
            assert_eq!(after.decision(), before.decision());
            assert_eq!(after.event_names(), before.event_names());
            assert!(
                after
                    .system_prompts()
                    .iter()
                    .all(|prompt| !prompt.contains("Common guidance"))
            );
        }
        let before = run_routed(config, native, writing_script(), jev(), None);
        let after = run_common(config, native, writing_script(), jev(), None, None);
        assert_eq!(after.resolved()[0]["reason"], "missing_key");
        assert_eq!(after.system_prompts(), before.system_prompts());
        assert_eq!(after.decision(), before.decision());
    }
}

/// Test 2: set, the common guidance is in the system prompt of every request
/// exactly once (first turn, the turns after Write, Read, and Bash, native and
/// XML), nowhere else in the request, and nothing else changes: the user
/// request, the history, the tools, and the run's decision.
#[test]
fn common_guidance_reaches_every_request_exactly_once() {
    let common = guidance("multi-line");
    for native in [true, false] {
        let plain = run_plain(config, native, tool_chain_script());
        let run = run_common(
            config,
            native,
            tool_chain_script(),
            RequestPatternSettings::OFF,
            None,
            Some(&common),
        );
        assert!(run.requests.len() >= 4, "{}", run.requests.len());
        assert_eq!(run.requests.len(), plain.requests.len());
        assert_eq!(run.occurrences(&common), vec![1; run.requests.len()]);
        let block = request_block(&common);
        for (index, (prompt, plain_prompt)) in run
            .system_prompts()
            .iter()
            .zip(plain.system_prompts())
            .enumerate()
        {
            assert_eq!(
                without_block(prompt, &block),
                plain_prompt,
                "request {index}"
            );
            assert!(!prompt.contains("Common guidance"), "request {index}");
            if !native {
                assert!(
                    prompt.find(&common).unwrap()
                        < prompt.find("Native tool calls are unavailable").unwrap(),
                    "request {index}"
                );
            }
        }
        for messages in &run.requests {
            assert_eq!(messages[1].role, "user");
            assert_eq!(messages[1].content, REQUEST);
        }
        let tool_turns = run.requests[3]
            .iter()
            .filter(|message| message.role == "tool")
            .count();
        assert_eq!(
            tool_turns, 3,
            "the fourth request follows Write, Read, Bash"
        );
        assert_eq!(run.histories(), plain.histories());
        assert_eq!(run.tools, plain.tools);
        assert_eq!(run.decision(), plain.decision());
        assert_eq!(run.event_names(), plain.event_names());
        assert!(
            run.session
                .messages
                .iter()
                .all(|message| !message.content.contains(&common)),
            "the session history never stores the common guidance"
        );
    }
}

/// Test 2 (fresh retry): the empty-response fresh-session retry rebuilds the
/// request from the user request alone and still carries the common guidance.
#[test]
fn the_fresh_session_retry_keeps_the_common_guidance() {
    let common = guidance("single-line");
    for native in [true, false] {
        let plain = run_plain(config, native, empty_then_fresh_retry_script());
        let run = run_common(
            config,
            native,
            empty_then_fresh_retry_script(),
            RequestPatternSettings::OFF,
            None,
            Some(&common),
        );
        let retry = run
            .events()
            .into_iter()
            .filter(|event| {
                event["event"] == "empty_response_escalation"
                    && event["stage"] == "fresh_session_retry"
            })
            .count();
        assert_eq!(retry, 1);
        assert!(run.requests.len() >= 5);
        assert_eq!(
            run.requests[3].len(),
            2,
            "the fresh retry has system + user"
        );
        assert_eq!(run.requests[3][1].content, REQUEST);
        assert_eq!(run.occurrences(&common), vec![1; run.requests.len()]);
        let block = request_block(&common);
        for (prompt, plain_prompt) in run.system_prompts().iter().zip(plain.system_prompts()) {
            assert_eq!(without_block(prompt, &block), plain_prompt);
        }
        assert_eq!(run.histories(), plain.histories());
        assert_eq!(run.decision(), plain.decision());
    }
}

/// Test 3: with jev, an adopted pattern keeps its block and the common
/// guidance follows it in every request; the classifier is called once and the
/// event is the one the run records without common guidance. A fallback, `off`,
/// or an explicit intent never calls jev and never invents an adoption.
#[test]
fn jev_keeps_its_guidance_and_events_and_common_guidance_never_routes() {
    let common = guidance("multi-line");
    for pattern in RequestPattern::ALL {
        let before_classifier = Fixed::new(pattern);
        let before = run_routed(
            config,
            true,
            writing_script(),
            jev(),
            Some(&before_classifier),
        );
        let classifier = Fixed::new(pattern);
        let run = run_common(
            config,
            true,
            writing_script(),
            jev(),
            Some(&classifier),
            Some(&common),
        );
        assert_eq!(classifier.calls(), 1, "{pattern:?}");
        assert_eq!(before_classifier.calls(), 1, "{pattern:?}");
        let strip = |event: &Value| {
            let mut event = event.clone();
            event["latency_ms"] = Value::Null;
            event
        };
        let events = run.resolved();
        assert_eq!(events.len(), 1, "{pattern:?}");
        assert_eq!(
            strip(&events[0]),
            strip(&before.resolved()[0]),
            "{pattern:?}"
        );
        let fields = events[0]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        let expected_fields = [
            "event",
            "source",
            "pattern",
            "confidence",
            "adopted",
            "reason",
            "model",
            "latency_ms",
            "guidance_version",
            "schema_version",
        ]
        .map(str::to_string)
        .into_iter()
        .collect::<BTreeSet<_>>();
        assert_eq!(fields, expected_fields);
        assert_eq!(run.event_names(), before.event_names(), "{pattern:?}");
        assert_eq!(run.occurrences(&common), vec![1; run.requests.len()]);
        assert_eq!(
            run.occurrences(pattern.guidance()),
            vec![1; run.requests.len()]
        );
        let common_block = format!("{COMMON_HEADING}{common}");
        for (prompt, before_prompt) in run.system_prompts().iter().zip(before.system_prompts()) {
            assert!(prompt.find(pattern.guidance()).unwrap() < prompt.find(&common).unwrap());
            assert_eq!(without_block(prompt, &common_block), before_prompt);
        }
        assert_eq!(run.decision(), before.decision(), "{pattern:?}");
    }

    // A jev fallback records DEFAULT and the common guidance stands alone.
    let run = run_common(config, true, writing_script(), jev(), None, Some(&common));
    let events = run.resolved();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["pattern"], "default");
    assert_eq!(events[0]["adopted"], false);
    assert_eq!(events[0]["reason"], "missing_key");
    assert_eq!(events[0]["guidance_version"], Value::Null);
    assert_eq!(run.occurrences(&common), vec![1; run.requests.len()]);
    for prompt in run.system_prompts() {
        assert!(!prompt.contains("Common guidance"));
        for pattern in RequestPattern::ALL {
            assert!(!prompt.contains(pattern.guidance()));
        }
    }

    // `off` and an explicit intent never call jev and record nothing.
    let with_intent = |root: &Path| Config {
        intent_override: Some(IntentId::Create),
        ..config(root)
    };
    for (settings, config_of) in [
        (
            RequestPatternSettings::OFF,
            &config as &dyn Fn(&Path) -> Config,
        ),
        (jev(), &with_intent as &dyn Fn(&Path) -> Config),
    ] {
        let classifier = Fixed::new(RequestPattern::Create);
        let run = run_common(
            config_of,
            true,
            writing_script(),
            settings,
            Some(&classifier),
            Some(&common),
        );
        assert_eq!(classifier.calls(), 0);
        assert!(run.resolved().is_empty());
        assert_eq!(run.occurrences(&common), vec![1; run.requests.len()]);
        assert!(
            run.system_prompts()
                .iter()
                .all(|prompt| !prompt.contains(RequestPattern::Create.guidance()))
        );
    }
}

/// Test 5: a registered secret in the common guidance is scrubbed in every
/// request like the rest of the run, and the run records no new event.
#[test]
fn secrets_in_common_guidance_are_scrubbed_and_no_event_is_added() {
    const SECRET: &str = "fake-dotenv-secret-925-Rk4vNq8Wz2Ty";
    let dir = workspace();
    std::fs::write(dir.path().join(".env"), format!("DB_PASSWORD={SECRET}\n")).unwrap();
    let common = format!("{}\nNever print {SECRET}.", guidance("single-line"));
    let run = run_common_in(
        dir,
        config,
        true,
        writing_script(),
        RequestPatternSettings::OFF,
        None,
        Some(&common),
    );
    let plain = run_plain(config, true, writing_script());
    let marker = commandagent::sensitive_data::active_marker();
    let scrubbed = common.replace(SECRET, &marker);
    assert!(!run.requests.is_empty());
    for messages in &run.requests {
        for message in messages {
            assert!(!message.content.contains(SECRET));
        }
        assert_eq!(messages[0].content.matches(&scrubbed).count(), 1);
    }
    assert_eq!(run.event_names(), plain.event_names());
    let events = std::fs::read_to_string(run.dir.path().join("events.jsonl")).unwrap();
    assert!(!events.contains(SECRET));
    assert!(!events.contains(&guidance("single-line")));
}

/// Test 6: the common guidance changes no gate. A run that writes nothing, or
/// whose write is refused, ends exactly like the plain run, while every one of
/// its requests carried the guidance.
#[test]
fn common_guidance_relaxes_no_gate() {
    let common = guidance("claims-done");
    for script in [idle_script(), escaping_script()] {
        let plain = run_plain(config, true, script.clone());
        let run = run_common(
            config,
            true,
            script,
            RequestPatternSettings::OFF,
            None,
            Some(&common),
        );
        assert!(!run.requests.is_empty());
        assert_eq!(run.occurrences(&common), vec![1; run.requests.len()]);
        assert_eq!(run.decision(), plain.decision());
        assert_eq!(run.histories(), plain.histories());
        assert_eq!(run.event_names(), plain.event_names());
        assert!(!run.dir.path().join("notes.md").exists());
        assert!(
            !run.dir
                .path()
                .parent()
                .unwrap()
                .join("outside-925.md")
                .exists()
        );
        if let Ok(outcome) = &run.result {
            assert!(
                !outcome.missing_required_paths.is_empty()
                    || outcome.last_blocking_reason.is_some()
                    || outcome.changed_paths.is_empty(),
                "{}",
                run.decision()
            );
        }
    }
}

// ---- The real binary: the variable is read from a child's environment. ----

/// A loopback Ollama stand-in. It answers the startup probes, records every
/// chat request body, and replays `replies` (then a plain final answer).
struct MockOllama {
    host: String,
    bodies: Arc<Mutex<Vec<Value>>>,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl MockOllama {
    fn start(replies: Vec<Value>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let host = format!("http://{}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let bodies = Arc::new(Mutex::new(Vec::new()));
        let shutdown = Arc::new(AtomicBool::new(false));
        let (sink, stop) = (Arc::clone(&bodies), Arc::clone(&shutdown));
        let handle = std::thread::spawn(move || {
            let mut replies = VecDeque::from(replies);
            while !stop.load(Ordering::Acquire) {
                let mut stream = match listener.accept() {
                    Ok((stream, _)) => stream,
                    Err(_) => {
                        std::thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                };
                stream.set_nonblocking(false).unwrap();
                let Some((head, body)) = read_request(&mut stream) else {
                    continue;
                };
                let response = if head.starts_with("POST /api/chat ") {
                    sink.lock()
                        .unwrap()
                        .push(serde_json::from_str::<Value>(&body).unwrap_or(Value::Null));
                    let message = replies
                        .pop_front()
                        .unwrap_or_else(|| json!({"role": "assistant", "content": "Done."}));
                    json!({"model": "test-model", "message": message, "done": true}).to_string()
                } else if head.starts_with("GET /api/tags ") {
                    json!({"models": [{"name": "test-model"}]}).to_string()
                } else if head.starts_with("POST /api/show ") {
                    json!({"capabilities": ["completion", "tools"]}).to_string()
                } else {
                    let _ = stream.write_all(
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    );
                    continue;
                };
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
                    response.len()
                );
                let _ = stream.flush();
            }
        });
        Self {
            host,
            bodies,
            shutdown,
            handle: Some(handle),
        }
    }

    fn bodies(&self) -> Vec<Value> {
        self.bodies.lock().unwrap().clone()
    }
}

impl Drop for MockOllama {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn read_request(stream: &mut TcpStream) -> Option<(String, String)> {
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .ok()?;
    let mut data = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        let count = stream.read(&mut buffer).ok()?;
        if count == 0 {
            return None;
        }
        data.extend_from_slice(&buffer[..count]);
        let Some(end) = data.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };
        let head = String::from_utf8_lossy(&data[..end]).into_owned();
        let length = head
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
            .unwrap_or(0);
        if data.len() >= end + 4 + length {
            let body = String::from_utf8_lossy(&data[end + 4..end + 4 + length]).into_owned();
            return Some((head, body));
        }
    }
}

/// The scripted replies of a direct run. Native: Write, Read, then the final
/// answer. XML: text answers only, because the Ollama client parses XML tool
/// calls against the request's tool list, which is empty in XML mode; the
/// missing `notes.md` still makes the run send several requests.
fn mock_script(xml: bool) -> Vec<Value> {
    let write = json!({"path": "notes.md", "content": "# Notes\nA small project.\n"});
    let read = json!({"path": "notes.md"});
    if xml {
        vec![
            json!({"role": "assistant", "content": "I will write notes.md next."}),
            json!({"role": "assistant", "content": "Created notes.md."}),
        ]
    } else {
        vec![
            json!({"role": "assistant", "content": "", "tool_calls": [{"function": {"name": "Write", "arguments": write}}]}),
            json!({"role": "assistant", "content": "", "tool_calls": [{"function": {"name": "Read", "arguments": read}}]}),
            json!({"role": "assistant", "content": "Created notes.md."}),
        ]
    }
}

struct Child {
    dir: tempfile::TempDir,
    output: Output,
}

impl Child {
    fn workspace(&self) -> PathBuf {
        self.dir.path().join("work")
    }

    fn text(&self) -> String {
        format!(
            "{}{}",
            String::from_utf8_lossy(&self.output.stdout),
            String::from_utf8_lossy(&self.output.stderr)
        )
    }

    fn events(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("events.jsonl")).unwrap_or_default()
    }
}

/// Starts the shipped binary with a cleared environment. `common` is the only
/// way the variable reaches it; `None` leaves it unset.
fn spawn_child(
    mock: &MockOllama,
    args: &[&str],
    common: Option<&OsString>,
) -> (tempfile::TempDir, std::process::Child) {
    let dir = tempfile::tempdir().unwrap();
    let work = dir.path().join("work");
    let state = dir.path().join("state");
    let home = dir.path().join("home");
    for path in [&work, &state, &home] {
        std::fs::create_dir_all(path).unwrap();
    }
    let mut command = Command::new(env!("CARGO_BIN_EXE_commandagent"));
    command
        .args([
            "--provider",
            "ollama",
            "--model",
            "test-model",
            "--planner-provider",
            "ollama",
            "--planner-model",
            "test-model",
            "--ollama-host",
            &mock.host,
            "--chat-timeout-secs",
            "30",
            "--cwd",
            work.to_str().unwrap(),
            "--state-dir",
            state.to_str().unwrap(),
            "--yes",
            "--no-footer",
        ])
        .args(args)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", &home)
        .env("XDG_STATE_HOME", home.join("state"))
        .env("COMMANDAGENT_EVAL_EVENTS", dir.path().join("events.jsonl"))
        .current_dir(&work)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if let Some(common) = common {
        command.env(SYSTEM_GUIDANCE_ENV, common);
    }
    let child = command.spawn().unwrap();
    (dir, child)
}

fn run_child(mock: &MockOllama, args: &[&str], common: Option<&OsString>) -> Child {
    let (dir, child) = spawn_child(mock, args, common);
    let output = child.wait_with_output().unwrap();
    Child { dir, output }
}

fn prompt_args(protocol: &str) -> Vec<&str> {
    vec!["--tool-protocol", protocol, "--prompt", REQUEST]
}

/// The system prompt of one recorded Ollama chat body.
fn system_of(body: &Value) -> String {
    assert_eq!(body["messages"][0]["role"], "system", "{body}");
    body["messages"][0]["content"].as_str().unwrap().to_string()
}

/// The chat bodies with the run's workspace path normalized.
fn normalized(bodies: &[Value], child: &Child) -> Vec<String> {
    let root = child.workspace().to_string_lossy().to_string();
    let canonical = child
        .workspace()
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .to_string();
    bodies
        .iter()
        .map(|body| {
            body.to_string()
                .replace(&canonical, "<root>")
                .replace(&root, "<root>")
        })
        .collect()
}

/// Test 2 through the binary: the value set in the child's environment reaches
/// every chat request once, native and XML, and the user request is unchanged.
#[test]
fn child_environment_value_reaches_every_chat_request_native_and_xml() {
    let common = guidance("multi-line");
    let escaped = serde_json::to_string(&common).unwrap();
    let escaped = &escaped[1..escaped.len() - 1];
    for (protocol, xml) in [("native", false), ("text", true)] {
        let mock = MockOllama::start(mock_script(xml));
        let child = run_child(
            &mock,
            &prompt_args(protocol),
            Some(&OsString::from(&common)),
        );
        // The XML run never writes notes.md, so the missing artifact keeps
        // it going and then fails it, exactly as without the guidance.
        assert_eq!(
            child.output.status.success(),
            !xml,
            "{protocol}: {}",
            child.text()
        );
        let bodies = mock.bodies();
        assert!(
            bodies.len() >= if xml { 2 } else { 3 },
            "{protocol}: {} chat requests",
            bodies.len()
        );
        for (index, body) in bodies.iter().enumerate() {
            let system = system_of(body);
            assert_eq!(
                system.matches(&common).count(),
                1,
                "{protocol} request {index}"
            );
            assert_eq!(
                body.to_string().matches(escaped).count(),
                1,
                "{protocol} request {index}: only the system prompt carries it"
            );
            assert_eq!(body["messages"][1]["role"], "user");
            assert_eq!(body["messages"][1]["content"], REQUEST);
            assert_eq!(
                system.contains("Native tool calls are unavailable"),
                xml,
                "{protocol} request {index}"
            );
            assert_eq!(
                body.get("tools").is_some(),
                !xml,
                "{protocol} request {index}"
            );
        }
        assert!(
            bodies[bodies.len() - 1]["messages"]
                .as_array()
                .unwrap()
                .len()
                > bodies[0]["messages"].as_array().unwrap().len(),
            "{protocol}: the last request follows earlier turns"
        );
        assert_eq!(
            child.workspace().join("notes.md").is_file(),
            !xml,
            "{protocol}"
        );
        assert!(
            !child.events().contains(&common),
            "{protocol}: no event body"
        );
    }
}

/// Test 1 through the binary: an unset and an empty variable send the same
/// chat requests.
#[test]
fn child_unset_and_empty_values_send_identical_chat_requests() {
    for (protocol, xml) in [("native", false), ("text", true)] {
        let unset_mock = MockOllama::start(mock_script(xml));
        let unset = run_child(&unset_mock, &prompt_args(protocol), None);
        let empty_mock = MockOllama::start(mock_script(xml));
        let empty = run_child(&empty_mock, &prompt_args(protocol), Some(&OsString::new()));
        assert_eq!(unset.output.status.success(), !xml, "{}", unset.text());
        assert_eq!(empty.output.status.code(), unset.output.status.code());
        let (unset_bodies, empty_bodies) = (unset_mock.bodies(), empty_mock.bodies());
        assert!(unset_bodies.len() >= 2, "{protocol}");
        assert_eq!(
            normalized(&empty_bodies, &empty),
            normalized(&unset_bodies, &unset),
            "{protocol}"
        );
        for body in &unset_bodies {
            let system = system_of(body);
            assert!(!system.contains("Request guidance"), "{protocol}");
            assert!(!system.contains("Common guidance"), "{protocol}");
        }
    }
}

/// Test 4: concurrent runs keep their own value: an unset run and two runs
/// with different texts each send only their own text.
#[test]
fn concurrent_child_runs_keep_their_own_guidance() {
    let texts = [None, Some(guidance("arm-a")), Some(guidance("arm-b"))];
    let mocks = texts
        .iter()
        .map(|_| MockOllama::start(mock_script(false)))
        .collect::<Vec<_>>();
    let children = texts
        .iter()
        .zip(&mocks)
        .map(|(text, mock)| {
            let value = text.as_ref().map(OsString::from);
            spawn_child(mock, &prompt_args("native"), value.as_ref())
        })
        .collect::<Vec<_>>();
    let mut outputs = Vec::new();
    for (dir, child) in children {
        let output = child.wait_with_output().unwrap();
        outputs.push(Child { dir, output });
    }
    for (index, ((text, mock), child)) in texts.iter().zip(&mocks).zip(&outputs).enumerate() {
        assert!(
            child.output.status.success(),
            "run {index}: {}",
            child.text()
        );
        let bodies = mock.bodies();
        assert!(bodies.len() >= 3, "run {index}");
        for body in &bodies {
            let system = system_of(body);
            for (other, other_text) in texts.iter().enumerate() {
                let Some(other_text) = other_text else {
                    continue;
                };
                assert_eq!(
                    system.matches(other_text.as_str()).count(),
                    usize::from(other == index),
                    "run {index} carries text {other}"
                );
            }
            assert_eq!(system.contains("Request guidance"), text.is_some());
        }
    }
}

/// Test 4: the plan entry and the model probe never carry the variable, even
/// when it is set.
#[test]
fn plan_and_model_probe_entries_ignore_the_variable() {
    let common = guidance("multi-line");
    for args in [
        vec!["--plan-steps", "Create notes.md with a short summary."],
        vec!["--model-probe"],
    ] {
        let mock = MockOllama::start(Vec::new());
        let child = run_child(&mock, &args, Some(&OsString::from(&common)));
        let bodies = mock.bodies();
        assert!(
            !bodies.is_empty(),
            "{args:?} reached no chat request: {}",
            child.text()
        );
        for body in &bodies {
            assert!(!body.to_string().contains("Request guidance"), "{args:?}");
            for line in common.lines() {
                assert!(!body.to_string().contains(line), "{args:?}");
            }
        }
    }
}

/// Test 5 through the binary: a refused value stops the run before any chat
/// request, and the diagnostic names the variable but never the value.
#[test]
fn refused_values_fail_before_any_request_without_echoing_the_value() {
    const MARKER: &str = "marker-925-never-echoed";
    let mut values = vec![
        ("whitespace", OsString::from(" \n\t ")),
        ("too-long", OsString::from(format!("{MARKER} ").repeat(400))),
    ];
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let mut bytes = MARKER.as_bytes().to_vec();
        bytes.push(0xff);
        values.push(("non-utf8", OsString::from_vec(bytes)));
    }
    for (id, value) in values {
        let mock = MockOllama::start(mock_script(false));
        let child = run_child(&mock, &prompt_args("native"), Some(&value));
        assert!(!child.output.status.success(), "{id}");
        let text = child.text();
        assert!(text.contains(SYSTEM_GUIDANCE_ENV), "{id}: {text}");
        assert!(!text.contains(MARKER), "{id}: {text}");
        assert!(mock.bodies().is_empty(), "{id}: no chat request is sent");
        assert!(!child.events().contains(MARKER), "{id}");
        assert!(!child.workspace().join("notes.md").exists(), "{id}");
    }
}

/// Test 5: the corpus value cases, at and over the limit in UTF-8 bytes.
#[test]
fn corpus_value_cases_are_checked_by_bytes() {
    let corpus = corpus();
    assert_eq!(corpus["variable"], SYSTEM_GUIDANCE_ENV);
    let max_bytes = corpus["max_bytes"].as_u64().unwrap() as usize;
    let cases = corpus["values"].as_array().unwrap();
    assert_eq!(cases.len(), 9);
    for case in cases {
        let id = case["id"].as_str().unwrap();
        let value = match case["repeat"].as_str() {
            Some(unit) => Some(format!(
                "{}{}",
                unit.repeat(case["count"].as_u64().unwrap() as usize),
                case["suffix"].as_str().unwrap_or("")
            )),
            None => case["value"].as_str().map(str::to_string),
        };
        let parsed = parse(value.as_ref().map(OsString::from).as_deref());
        match case["expect"].as_str().unwrap() {
            "not_set" => assert_eq!(parsed.unwrap(), None, "{id}"),
            "accepted" => {
                let value = value.unwrap();
                assert!(value.len() <= max_bytes, "{id}");
                assert_eq!(parsed.unwrap(), Some(value), "{id}");
            }
            "refused" => {
                let message = format!("{:#}", parsed.unwrap_err());
                assert_eq!(
                    message,
                    format!("{SYSTEM_GUIDANCE_ENV} {}", case["reason"].as_str().unwrap()),
                    "{id}"
                );
            }
            other => panic!("{id}: unknown expectation {other}"),
        }
    }
}

/// The corpus is synthetic and pinned by digest.
#[test]
fn corpus_manifest_pins_the_synthetic_fixture() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(CORPUS);
    let manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("source-manifest.json")).unwrap())
            .unwrap();
    let bytes = std::fs::read(root.join("fixtures/cases.json")).unwrap();
    let digest = format!("{:x}", Sha256::digest(&bytes));
    assert_eq!(manifest["fixture_sha256"]["cases.json"], digest);
    let corpus = corpus();
    assert_eq!(corpus["kind"], "synthetic");
    for (id, text) in corpus["guidance"].as_object().unwrap() {
        let text = text.as_str().unwrap();
        assert!(text.len() < 200, "{id} is a short synthetic text");
        assert_eq!(
            parse(Some(OsString::from(text).as_os_str()))
                .unwrap()
                .as_deref(),
            Some(text),
            "{id}"
        );
    }
}
