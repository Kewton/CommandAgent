//! Issue #945: the experimental `COMMANDAGENT_REQUIREMENT_PLAN` opt-in.
//! Scenarios live in the synthetic corpus fixture
//! `tests/corpus/apps/requirement-plan/fixtures/plan.json` and run through the
//! public minimal loop with a scripted mock `ChatClient`.
//!
//! The `default-off-*` rows pin the behavior of the source before the
//! requirement plan existed (c9f88d35): request digests, the event-name
//! sequence, the provider call count, and the outcome were recorded from that
//! source, not from the new implementation.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use commandagent::config::{
    Action, Config, ConfigFieldSources, NarrationMode, OllamaThink, OpenAiApi, PlanPreset,
    PromptLayout, Provider, ToolProtocol,
};
use commandagent::minimal_loop::loop_run::{RunSessionOutcome, run_session_with_outcome_with_ui};
use commandagent::providers::{AssistantReply, ChatClient};
use commandagent::state::{ConversationMessage, SessionSnapshot, ToolCall};
use commandagent::tools::registry::ToolSpec;
use commandagent::tui::status::UiStatus;
use commandagent::tui::{InteractionUi, UiGuard};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const ENV: &str = "COMMANDAGENT_REQUIREMENT_PLAN";
const LEGACY_ENV: &str = "ANVIL_REQUIREMENT_PLAN";
const REVIEW_ENV: &str = "COMMANDAGENT_COMPLETION_OBLIGATION_REVIEW";
const EVENT: &str = "requirement_plan";
const CORPUS: &str = "tests/corpus/apps/requirement-plan";
const OFF_GOLDEN_SOURCE: &str = "c9f88d35";
/// The opening sentence of the fixed guidance; it marks the request that
/// carries the guidance without the test copying the whole text.
const GUIDANCE_MARKER: &str = "作業を始める前に一度だけ、この返答の冒頭に要求計画ブロック";
/// The fixed header of the retained note.
const NOTE_MARKER: &str = "[requirement plan note v1:";
const CONTINUE_MARKER: &str = "Continue the original request.";

/// Every env mutation in this binary happens while this lock is held.
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn env_lock() -> MutexGuard<'static, ()> {
    ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn set_env(name: &str, setting: Option<&str>) {
    // SAFETY: every env mutation in this test binary happens while holding
    // `ENV_LOCK`, and no test in this binary reads the environment without it.
    unsafe {
        match setting {
            Some(value) => std::env::set_var(name, value),
            None => std::env::remove_var(name),
        }
    }
}

#[cfg(unix)]
fn set_env_bytes(name: &str, bytes: &[u8]) {
    use std::os::unix::ffi::OsStrExt;
    // SAFETY: as in `set_env`.
    unsafe {
        std::env::set_var(name, std::ffi::OsStr::from_bytes(bytes));
    }
}

#[derive(Clone)]
struct ScriptedClient {
    replies: Arc<Mutex<Vec<AssistantReply>>>,
    requests: Arc<Mutex<Vec<Vec<ConversationMessage>>>>,
    native_flags: Arc<Mutex<Vec<bool>>>,
    native: bool,
    /// What `parses_xml_tool_calls_before_host` reports.
    parses_xml: bool,
    /// What `allows_xml_fallback` reports.
    allows_fallback: bool,
    /// What `supports_ollama_think` reports.
    think: bool,
    /// Sets the requirement plan variable to this value during the first call.
    env_change_on_first_call: Option<&'static str>,
}

impl ChatClient for ScriptedClient {
    fn label(&self) -> &str {
        "scripted"
    }

    fn supports_native_tools(&self, _model: &str) -> bool {
        self.native
    }

    fn parses_xml_tool_calls_before_host(&self) -> bool {
        self.parses_xml
    }

    fn allows_xml_fallback(&self) -> bool {
        self.allows_fallback
    }

    fn supports_ollama_think(&self) -> bool {
        self.think
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn chat(
        &mut self,
        _model: &str,
        messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        let mut requests = self.requests.lock().unwrap();
        if requests.is_empty()
            && let Some(value) = self.env_change_on_first_call
        {
            // The caller holds `ENV_LOCK` for the whole run.
            set_env(ENV, Some(value));
        }
        requests.push(messages.to_vec());
        self.native_flags.lock().unwrap().push(native_tools_enabled);
        let mut replies = self.replies.lock().unwrap();
        if replies.is_empty() {
            anyhow::bail!("scripted client exhausted");
        }
        Ok(replies.remove(0))
    }
}

/// A UI that reports an interrupt once the given model call has started.
struct CancelAtModelCall {
    calls: Cell<usize>,
    cancel_at: usize,
}

impl InteractionUi for CancelAtModelCall {
    fn before_model_call(&self, _label: &str) -> UiGuard {
        self.calls.set(self.calls.get() + 1);
        UiGuard::noop()
    }

    fn before_tool_call(&self, _name: &str) -> UiGuard {
        UiGuard::noop()
    }

    fn publish_status(&self, _status: UiStatus) {}

    fn interrupted(&self) -> bool {
        self.calls.get() >= self.cancel_at
    }
}

fn config(root: &Path, scenario: &Scenario, contract: Option<PathBuf>) -> Config {
    Config {
        workspace_root: root.to_path_buf(),
        state_dir: root.join("state"),
        eval_events_path: Some(root.join("events.jsonl")),
        completion_contract_path: contract,
        yes: true,
        offline: false,
        context_budget: scenario.context_budget,
        model: "m".to_string(),
        provider: Provider::Ollama,
        tool_protocol: scenario.xml.then_some(ToolProtocol::Text),
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
        max_iterations: scenario.max_iterations,
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
        profile: scenario.profile.clone(),
        profile_explicit: false,
        profile_inference: None,
        style: "default".to_string(),
        action: Action::Repl,
    }
}

/// One fixture row.
#[derive(Debug, Clone)]
struct Scenario {
    id: String,
    setting: Option<String>,
    review_setting: Option<String>,
    max_iterations: usize,
    context_budget: usize,
    profile: String,
    xml: bool,
    client_parses_xml: bool,
    prompt: String,
    history: Vec<(String, String)>,
    contract: Option<Value>,
    seed: Vec<(String, String)>,
    script: Vec<Value>,
    cancel_at_request: Option<usize>,
    expect: Value,
}

fn string_pairs(value: &Value) -> Vec<(String, String)> {
    value
        .as_array()
        .map(|pairs| {
            pairs
                .iter()
                .map(|pair| {
                    (
                        pair[0].as_str().unwrap().to_string(),
                        pair[1].as_str().unwrap().to_string(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

impl Scenario {
    fn from_row(row: &Value, prompts: &Value) -> Self {
        assert_eq!(row["kind"], "synthetic", "{row}");
        let prompt_id = row["prompt"].as_str().unwrap_or("edit");
        let prompt = prompts[prompt_id]
            .as_str()
            .unwrap_or_else(|| panic!("unknown prompt id {prompt_id}"))
            .to_string();
        Self {
            id: row["id"].as_str().unwrap().to_string(),
            setting: row["setting"].as_str().map(str::to_string),
            review_setting: row["review_setting"].as_str().map(str::to_string),
            max_iterations: row["max_iterations"].as_u64().unwrap_or(6) as usize,
            context_budget: row["context_budget"].as_u64().unwrap_or(100_000) as usize,
            profile: row["profile"].as_str().unwrap_or("generic").to_string(),
            xml: row["tool_mode"].as_str() == Some("xml"),
            client_parses_xml: row["client_parses_xml"] == true,
            prompt,
            history: string_pairs(&row["history"]),
            contract: row.get("contract").cloned(),
            seed: string_pairs(&row["seed"]),
            script: row["script"].as_array().unwrap().clone(),
            cancel_at_request: row["cancel_at_request"]
                .as_u64()
                .map(|value| value as usize),
            expect: row["expect"].clone(),
        }
    }

    /// One scripted tool call: a native call, or an XML tag in the content
    /// when the scenario runs in explicit text (XML) mode.
    fn with_setting(&self, setting: Option<&str>) -> Self {
        let mut copy = self.clone();
        copy.setting = setting.map(str::to_string);
        copy
    }

    fn call_reply(
        &self,
        index: usize,
        preamble: &str,
        calls: &[(String, Value)],
    ) -> AssistantReply {
        if self.xml {
            let mut content = preamble.to_string();
            for (name, arguments) in calls {
                content.push_str(&format!(
                    "<anvil_tool_call name=\"{name}\">{arguments}</anvil_tool_call>"
                ));
            }
            return AssistantReply::text(content);
        }
        AssistantReply {
            content: preamble.to_string(),
            tool_calls: calls
                .iter()
                .enumerate()
                .map(|(offset, (name, arguments))| ToolCall {
                    id: format!("call-{index}-{offset}"),
                    name: name.clone(),
                    arguments: arguments.clone(),
                })
                .collect(),
            prompt_tokens: None,
            completion_tokens: None,
        }
    }

    fn replies(&self) -> Vec<AssistantReply> {
        self.script
            .iter()
            .enumerate()
            .map(|(index, step)| {
                let preamble = step["preamble"].as_str().unwrap_or_default();
                let mut reply = if let Some(write) = step.get("write") {
                    let call = (
                        "Write".to_string(),
                        json!({"path": write[0], "content": write[1]}),
                    );
                    self.call_reply(index, preamble, &[call])
                } else if let Some(edit) = step.get("edit") {
                    let call = (
                        "Edit".to_string(),
                        json!({"path": edit[0], "old_string": edit[1], "new_string": edit[2]}),
                    );
                    self.call_reply(index, preamble, &[call])
                } else if let Some(command) = step.get("bash") {
                    let call = ("Bash".to_string(), json!({"command": command}));
                    self.call_reply(index, preamble, &[call])
                } else if let Some(path) = step.get("read") {
                    let call = ("Read".to_string(), json!({"path": path}));
                    self.call_reply(index, preamble, &[call])
                } else if let Some(text) = step.get("final") {
                    AssistantReply::text(text.as_str().unwrap())
                } else if step.get("empty").is_some() {
                    AssistantReply::text("")
                } else if let Some(content) = step.get("raw") {
                    // Content sent verbatim, never converted to a tool call.
                    AssistantReply::text(content.as_str().unwrap())
                } else {
                    panic!("unknown script step {step}");
                };
                reply.completion_tokens = step["completion_tokens"].as_u64();
                reply
            })
            .collect()
    }

    fn run(&self) -> Trace {
        self.run_with(|client| client)
    }

    fn run_with(&self, adjust: impl FnOnce(ScriptedClient) -> ScriptedClient) -> Trace {
        let _lock = env_lock();
        set_env(LEGACY_ENV, None);
        set_env(ENV, self.setting.as_deref());
        set_env(REVIEW_ENV, self.review_setting.as_deref());
        let trace = self.run_locked(adjust, None);
        set_env(ENV, None);
        set_env(REVIEW_ENV, None);
        trace
    }

    /// Runs the script on a session that already holds an earlier run.
    fn run_after(&self, session: SessionSnapshot) -> Trace {
        let _lock = env_lock();
        set_env(ENV, self.setting.as_deref());
        let trace = self.run_locked(|client| client, Some(session));
        set_env(ENV, None);
        trace
    }

    fn run_locked(
        &self,
        adjust: impl FnOnce(ScriptedClient) -> ScriptedClient,
        earlier: Option<SessionSnapshot>,
    ) -> Trace {
        let dir = tempfile::tempdir().unwrap();
        for (path, content) in &self.seed {
            let target = dir.path().join(path);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(target, content).unwrap();
        }
        let contract = self.contract.as_ref().map(|contract| {
            let path = dir.path().join("completion-contract.json");
            std::fs::write(&path, serde_json::to_vec_pretty(contract).unwrap()).unwrap();
            path
        });
        let mut client = adjust(ScriptedClient {
            replies: Arc::new(Mutex::new(self.replies())),
            requests: Arc::new(Mutex::new(Vec::new())),
            native_flags: Arc::new(Mutex::new(Vec::new())),
            native: !self.xml,
            parses_xml: self.client_parses_xml,
            allows_fallback: false,
            think: false,
            env_change_on_first_call: None,
        });
        let mut session = earlier.unwrap_or_default();
        for (role, content) in &self.history {
            session.messages.push(match role.as_str() {
                "user" => ConversationMessage::user(content.clone()),
                "assistant" => ConversationMessage::assistant(content.clone(), Vec::new()),
                other => panic!("unknown history role {other}"),
            });
        }
        let config = config(dir.path(), self, contract);
        let ui = CancelAtModelCall {
            calls: Cell::new(0),
            cancel_at: self.cancel_at_request.unwrap_or(usize::MAX),
        };
        let result = run_session_with_outcome_with_ui(
            &mut client,
            &mut session,
            &self.prompt,
            &[],
            &config,
            &ui,
        )
        .map_err(|err| format!("{err:#}"));
        let requests = client.requests.lock().unwrap().clone();
        let native_flags = client.native_flags.lock().unwrap().clone();
        let remaining_replies = client.replies.lock().unwrap().len();
        let events = std::fs::read_to_string(dir.path().join("events.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect();
        Trace {
            id: self.id.clone(),
            root: dir.path().to_path_buf(),
            _dir: dir,
            result,
            requests,
            native_flags,
            remaining_replies,
            events,
            session,
        }
    }
}

struct Trace {
    id: String,
    root: PathBuf,
    _dir: tempfile::TempDir,
    result: Result<RunSessionOutcome, String>,
    requests: Vec<Vec<ConversationMessage>>,
    native_flags: Vec<bool>,
    remaining_replies: usize,
    events: Vec<Value>,
    session: SessionSnapshot,
}

impl Trace {
    fn normalize(&self, text: &str) -> String {
        let mut text = text.to_string();
        if let Ok(canonical) = self.root.canonicalize() {
            text = text.replace(&canonical.display().to_string(), "<ROOT>");
        }
        text = text.replace(&self.root.display().to_string(), "<ROOT>");
        regex::Regex::new(r"elapsed_ms: [0-9]+")
            .unwrap()
            .replace_all(&text, "elapsed_ms: <N>")
            .into_owned()
    }

    /// The request bytes: the serialized messages and the native-tool flag
    /// of each provider call, with the temp workspace path normalized.
    fn request_digests(&self) -> Vec<String> {
        self.requests
            .iter()
            .zip(&self.native_flags)
            .map(|(messages, native)| {
                // XML tool calls get a random UUID id; replace every id by
                // its order of first appearance.
                let mut text = self.normalize(&serde_json::to_string(messages).unwrap());
                let ids = regex::Regex::new(
                    r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}",
                )
                .unwrap();
                let mut seen: Vec<String> = Vec::new();
                for found in ids.find_iter(&text.clone()) {
                    if !seen.iter().any(|id| id == found.as_str()) {
                        seen.push(found.as_str().to_string());
                    }
                }
                for (index, id) in seen.iter().enumerate() {
                    text = text.replace(id, &format!("<ID{index}>"));
                }
                let mut hasher = Sha256::new();
                hasher.update(if *native { "native\n" } else { "text\n" });
                hasher.update(text.as_bytes());
                hasher
                    .finalize()
                    .iter()
                    .take(8)
                    .map(|byte| format!("{byte:02x}"))
                    .collect()
            })
            .collect()
    }

    fn outcome(&self) -> Value {
        match &self.result {
            Ok(outcome) => json!({
                "stop_reason": format!("{:?}", outcome.stop_reason),
                "final_text": outcome.final_text,
                "changed_paths": outcome.changed_paths,
                "iterations": outcome.iterations,
                "tool_calls": outcome.tool_calls,
                "missing_required_paths": outcome.missing_required_paths,
                "missing_capabilities": outcome.missing_capabilities,
                "missing_evidence": outcome.missing_evidence,
                "missing_obligations": outcome.missing_obligations,
                "verify_attempts": outcome.verify_attempts,
                "last_blocking_reason": outcome.last_blocking_reason,
                "last_provider_error": outcome.last_provider_error,
            }),
            Err(error) => json!({"error": self.normalize(error)}),
        }
    }

    fn event_names(&self, include_plan: bool) -> Vec<String> {
        self.events
            .iter()
            .filter(|event| include_plan || event["event"] != EVENT)
            .map(|event| event["event"].as_str().unwrap_or_default().to_string())
            .collect()
    }

    fn plan_events(&self) -> Vec<Value> {
        self.events
            .iter()
            .filter(|event| event["event"] == EVENT)
            .cloned()
            .collect()
    }

    fn plan_phases(&self) -> Vec<[String; 2]> {
        self.plan_events()
            .iter()
            .map(|event| {
                [
                    event["phase"].as_str().unwrap().to_string(),
                    event["reason"].as_str().unwrap().to_string(),
                ]
            })
            .collect()
    }

    /// Indexes of the requests whose messages contain `marker`.
    fn request_indexes(&self, marker: &str) -> Vec<usize> {
        self.requests
            .iter()
            .enumerate()
            .filter(|(_, messages)| {
                messages
                    .iter()
                    .any(|message| message.content.contains(marker))
            })
            .map(|(index, _)| index)
            .collect()
    }

    fn count(&self, index: usize, marker: &str) -> usize {
        self.requests[index]
            .iter()
            .map(|message| message.content.matches(marker).count())
            .sum()
    }

    fn summary(&self) -> Value {
        json!({
            "plan_phases": self.plan_phases(),
            "guidance_request_indexes": self.request_indexes(GUIDANCE_MARKER),
            "note_request_indexes": self.request_indexes(NOTE_MARKER),
            "outcome": self.outcome(),
            "requests": self.requests.len(),
            "remaining_replies": self.remaining_replies,
            "event_names": self.event_names(true),
            "request_digests": self.request_digests(),
            "plan_events": self.plan_events(),
        })
    }

    /// Compares every key the fixture row's `expect` object sets.
    fn assert_expectations(&self, expect: &Value) {
        let summary = self.summary();
        for (key, expected) in expect.as_object().unwrap() {
            if matches!(
                key.as_str(),
                "equivalent_to_off" | "note" | "source" | "same_outcome_as"
            ) {
                continue;
            }
            assert_eq!(
                &summary[key],
                expected,
                "{}: {key} differs; actual summary:\n{}",
                self.id,
                serde_json::to_string(&summary).unwrap()
            );
        }
    }
}

fn corpus_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(CORPUS)
        .join(name)
}

fn corpus() -> Value {
    serde_json::from_str(&std::fs::read_to_string(corpus_path("fixtures/plan.json")).unwrap())
        .unwrap()
}

fn scenarios() -> Vec<Scenario> {
    let corpus = corpus();
    corpus["scenarios"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| Scenario::from_row(row, &corpus["prompts"]))
        .collect()
}

#[test]
fn default_off_rows_match_the_golden_recorded_before_the_feature() {
    let off_rows = scenarios()
        .into_iter()
        .filter(|scenario| scenario.id.starts_with("default-off-"))
        .collect::<Vec<_>>();
    assert!(off_rows.len() >= 10, "{}", off_rows.len());
    for scenario in off_rows {
        assert_eq!(
            scenario.expect["source"], OFF_GOLDEN_SOURCE,
            "{}",
            scenario.id
        );
        assert!(
            matches!(scenario.setting.as_deref(), None | Some("") | Some("off")),
            "{}",
            scenario.id
        );
        for key in ["outcome", "requests", "event_names", "request_digests"] {
            assert!(scenario.expect.get(key).is_some(), "{}: {key}", scenario.id);
        }
        let trace = scenario.run();
        trace.assert_expectations(&scenario.expect);
        assert!(trace.plan_events().is_empty(), "{}", scenario.id);
    }
}

fn scenario(id: &str) -> Scenario {
    scenarios()
        .into_iter()
        .find(|scenario| scenario.id == id)
        .unwrap_or_else(|| panic!("fixture row {id} missing"))
}

const FIXED_KEYS: &[&str] = &[
    "event",
    "schema_version",
    "phase",
    "reason",
    "prompt_version",
    "iteration",
    "remaining_iterations",
    "obligation_count",
    "example_count",
    "note_chars",
    "plan_only_turn",
];

const FIXED_STRINGS: &[&str] = &[
    EVENT,
    "1",
    "requested",
    "captured",
    "skipped",
    "released",
    "plan_run_step",
    "profile_not_generic",
    "completion_contract",
    "resumed_conversation",
    "provider_unsupported",
    "insufficient_remaining_iterations",
    "no_provider_request",
    "first_request",
    "valid_block",
    "missing",
    "invalid",
    "oversize",
    "user_update",
    "run_completed",
    "run_interrupted",
    "budget_exhausted",
    "run_failed",
];

/// A plan event carries only known keys, numbers, nulls, one boolean, and
/// fixed words.
fn assert_fixed_shape(event: &Value) {
    for (key, value) in event.as_object().unwrap() {
        assert!(
            FIXED_KEYS.contains(&key.as_str()),
            "unexpected key {key}: {event}"
        );
        match value {
            Value::Null | Value::Number(_) => {}
            Value::Bool(_) => assert_eq!(key, "plan_only_turn", "{event}"),
            Value::String(text) => assert!(
                FIXED_STRINGS.contains(&text.as_str()),
                "free text {text:?} in {event}"
            ),
            other => panic!("unexpected value {other} under {key}: {event}"),
        }
    }
    assert_eq!(event["schema_version"], "1", "{event}");
    assert_eq!(event["prompt_version"], 1, "{event}");
}

/// Exactly one terminal phase, and it is the last plan event.
fn assert_terminal_once(trace: &Trace) {
    let phases = trace.plan_phases();
    let terminal = phases
        .iter()
        .filter(|[phase, _]| phase == "skipped" || phase == "released")
        .count();
    assert_eq!(terminal, 1, "{}: {phases:?}", trace.id);
    let [last, _] = phases.last().unwrap();
    assert!(last == "skipped" || last == "released", "{}", trace.id);
}

impl Trace {
    /// Requests, outcome, and every non-plan event equal the same script run
    /// with the plan unset.
    fn assert_equivalent_to(&self, off: &Trace) {
        assert_eq!(self.request_digests(), off.request_digests(), "{}", self.id);
        assert_eq!(self.outcome(), off.outcome(), "{}", self.id);
        assert_eq!(
            self.event_names(false),
            off.event_names(true),
            "{}",
            self.id
        );
        assert!(off.plan_events().is_empty(), "{}", self.id);
        assert!(
            self.request_indexes(GUIDANCE_MARKER).is_empty(),
            "{}",
            self.id
        );
        assert!(self.request_indexes(NOTE_MARKER).is_empty(), "{}", self.id);
    }

    fn first_index(&self, predicate: impl Fn(&Value) -> bool) -> Option<usize> {
        self.events.iter().position(predicate)
    }

    fn file_exists(&self, path: &str) -> bool {
        self.root.join(path).exists()
    }
}

fn run_fixture_row(id: &str) -> Trace {
    let row = scenario(id);
    let trace = row.run();
    trace.assert_expectations(&row.expect);
    if row.expect["equivalent_to_off"] == true {
        trace.assert_equivalent_to(&row.with_setting(None).run());
    }
    if let Some(off_id) = row.expect["same_outcome_as"].as_str() {
        // Compared with the golden recorded at c9f88d35, not with a rerun.
        let golden = scenario(off_id).expect;
        assert_eq!(golden["source"], OFF_GOLDEN_SOURCE);
        assert_eq!(trace.outcome(), golden["outcome"], "{id}");
        assert_eq!(
            json!(trace.event_names(false)),
            golden["event_names"],
            "{id}"
        );
    }
    for event in trace.plan_events() {
        assert_fixed_shape(&event);
    }
    assert_terminal_once(&trace);
    trace
}

/// Every request after the capture carries the note once, as assistant-side
/// context after the unchanged original request, never in a system message.
fn assert_note_once_after_capture(trace: &Trace, prompt: &str) {
    let captured_at = trace.plan_events()[1]["iteration"].as_u64().unwrap() as usize;
    assert_eq!(trace.plan_phases()[1][0], "captured", "{}", trace.id);
    for index in captured_at..trace.requests.len() {
        let messages = &trace.requests[index];
        assert_eq!(
            trace.count(index, NOTE_MARKER),
            1,
            "{} request {index}",
            trace.id
        );
        assert_eq!(trace.count(index, GUIDANCE_MARKER), 0, "{}", trace.id);
        let prompt_at = messages
            .iter()
            .position(|message| message.role == "user")
            .unwrap();
        assert_eq!(messages[prompt_at].content, prompt, "{}", trace.id);
        let note = messages
            .iter()
            .find(|message| message.content.contains(NOTE_MARKER))
            .unwrap();
        assert_eq!(note.role, "assistant", "{}", trace.id);
        assert!(note.content.starts_with(NOTE_MARKER), "{}", trace.id);
        assert!(
            messages[..prompt_at + 2]
                .iter()
                .any(|message| message.content.contains(NOTE_MARKER)),
            "{}: right after the request",
            trace.id
        );
        assert!(
            messages
                .iter()
                .filter(|message| message.role == "system")
                .all(|message| !message.content.contains(NOTE_MARKER)),
            "{}",
            trace.id
        );
        assert_eq!(trace.count(index, "<requirement_plan>"), 0, "{}", trace.id);
    }
    // The block never reaches the stored conversation.
    for message in &trace.session.messages {
        assert!(
            !message.content.contains("<requirement_plan>"),
            "{}",
            trace.id
        );
        assert!(!message.content.contains(NOTE_MARKER), "{}", trace.id);
    }
}

fn edit_prompt() -> String {
    corpus()["prompts"]["edit"].as_str().unwrap().to_string()
}

#[test]
fn the_first_request_alone_asks_for_expectations_first_and_assumptions() {
    let trace = run_fixture_row("once-captured-native");
    assert_eq!(trace.request_indexes(GUIDANCE_MARKER), vec![0]);
    assert_eq!(trace.count(0, GUIDANCE_MARKER), 1);
    let guidance = trace.requests[0].last().unwrap();
    assert_eq!(guidance.role, "user");
    for phrase in [
        "期待結果は実装やライブラリの既定動作に合わせず先に決め",
        "仮定・未確定",
        "最大5件",
        "最大2件",
        "確認できない例は発明しないで",
        "確認済みの証拠ではありません",
    ] {
        assert!(guidance.content.contains(phrase), "{phrase}");
    }
    // The original request is unchanged and stays a separate message.
    assert_eq!(trace.requests[0][1].content, edit_prompt());
    assert!(
        trace.requests[0]
            .iter()
            .filter(|message| message.role == "system")
            .all(|message| !message.content.contains(GUIDANCE_MARKER))
    );
}

#[test]
fn the_note_is_captured_before_the_first_tool_runs_in_native_and_xml_mode() {
    for id in ["once-captured-native", "once-captured-xml"] {
        let trace = run_fixture_row(id);
        let captured = trace
            .first_index(|event| event["event"] == EVENT && event["phase"] == "captured")
            .unwrap();
        let first_tool = trace
            .first_index(|event| event["event"] == "tool_execute")
            .unwrap();
        assert!(captured < first_tool, "{id}: {captured} < {first_tool}");
        let event = &trace.plan_events()[1];
        assert_eq!(event["iteration"], 1, "{id}");
        assert_eq!(event["obligation_count"], 1, "{id}");
        assert_eq!(event["example_count"], 1, "{id}");
        assert_eq!(event["plan_only_turn"], false, "{id}");
        assert_note_once_after_capture(&trace, &edit_prompt());
    }
}

#[test]
fn the_note_survives_the_preamble_sanitizer_once() {
    let trace = run_fixture_row("once-captured-native");
    let assistant = trace.requests[1]
        .iter()
        .find(|message| message.role == "assistant")
        .unwrap();
    assert!(assistant.content.starts_with(NOTE_MARKER));
    assert!(!assistant.content.contains("Writing the greeting."));
    assert_eq!(assistant.tool_calls.len(), 1);
}

#[test]
fn compaction_fresh_retry_and_pending_feedback_keep_one_note() {
    let trace = run_fixture_row("once-compaction");
    assert_note_once_after_capture(&trace, &edit_prompt());
    // The history really was compacted.
    let tools = trace.requests[3]
        .iter()
        .filter(|message| message.role == "tool")
        .count();
    assert!(tools < 3, "{:#?}", trace.requests[3]);

    let trace = run_fixture_row("once-fresh-retry");
    assert_note_once_after_capture(&trace, &edit_prompt());
    assert!(
        trace
            .events
            .iter()
            .any(|event| event["stage"] == "fresh_session_retry")
    );
    let fresh = trace
        .requests
        .iter()
        .position(|messages| {
            messages.len() == 4 && messages[3].content.starts_with(CONTINUE_MARKER)
        })
        .expect("a fresh retry request with the note");
    assert_eq!(trace.requests[fresh][1].content, edit_prompt());
    assert_eq!(trace.requests[fresh][2].role, "assistant");
    let off = scenario("default-off-unset-fresh-retry").expect;
    let retries = |names: &[String]| {
        names
            .iter()
            .filter(|name| *name == "empty_response_escalation")
            .count()
    };
    let off_names = serde_json::from_value::<Vec<String>>(off["event_names"].clone()).unwrap();
    assert_eq!(retries(&trace.event_names(false)), retries(&off_names));

    let trace = run_fixture_row("once-pending-feedback");
    assert_note_once_after_capture(&trace, &edit_prompt());
    let feedback_request = &trace.requests[2];
    let last = feedback_request.last().unwrap();
    assert_eq!(last.role, "user");
    assert!(!last.content.contains(NOTE_MARKER));
    assert!(!last.content.starts_with(CONTINUE_MARKER));
    assert_ne!(last.content, edit_prompt());
    assert_eq!(
        trace.result.as_ref().unwrap().stop_reason_text(),
        "AssistantFinal"
    );
}

trait StopText {
    fn stop_reason_text(&self) -> String;
}

impl StopText for RunSessionOutcome {
    fn stop_reason_text(&self) -> String {
        format!("{:?}", self.stop_reason)
    }
}

#[test]
fn a_block_only_first_reply_is_one_planning_turn_inside_the_budget() {
    for id in ["once-plan-only-turn", "once-plan-only-turn-xml"] {
        let trace = run_fixture_row(id);
        assert_eq!(trace.plan_events()[1]["plan_only_turn"], true, "{id}");
        let outcome = trace.result.as_ref().unwrap();
        assert_eq!(outcome.iterations, 3, "{id}: the planning turn counts");
        assert_eq!(outcome.tool_calls, 1, "{id}");
        assert_eq!(outcome.final_text, "greeting.txt を作成しました。", "{id}");
        assert_note_once_after_capture(&trace, &edit_prompt());
        // The planning reply is not stored and gets no empty-reply nudge.
        assert!(
            !trace
                .event_names(false)
                .contains(&"empty_response_escalation".to_string())
        );
        assert_eq!(trace.requests[1].len(), 4, "{id}: {:#?}", trace.requests[1]);
    }
    // At the budget the planning turn is one of the five existing calls.
    let trace = run_fixture_row("once-budget-after-plan-only-turn");
    let golden = scenario("default-off-unset-budget").expect;
    assert_eq!(json!(trace.requests.len()), golden["requests"]);
    assert!(trace.result.is_err(), "a budget end stays a failure");
    assert_eq!(trace.plan_events()[1]["plan_only_turn"], true);
}

#[test]
fn missing_invalid_and_oversize_notes_are_never_requested_again() {
    for id in [
        "once-missing-block",
        "once-invalid-unknown-field",
        "once-oversize-block",
        "once-question-without-block",
    ] {
        let trace = run_fixture_row(id);
        let total: usize = (0..trace.requests.len())
            .map(|index| trace.count(index, GUIDANCE_MARKER))
            .sum();
        assert_eq!(total, 1, "{id}");
        assert!(trace.request_indexes(NOTE_MARKER).is_empty(), "{id}");
        for messages in &trace.requests {
            assert_eq!(
                messages
                    .iter()
                    .filter(|message| message.role == "user")
                    .count(),
                1 + usize::from(messages == &trace.requests[0]),
                "{id}: no extra request turn"
            );
        }
    }
}

#[test]
fn tool_like_tags_in_a_broken_note_never_run() {
    let golden = scenario("default-off-explicit-xml-unclosed-block").expect;
    // Off executes the tag (recorded at c9f88d35); once does not.
    assert!(
        golden["outcome"]["changed_paths"]
            .as_array()
            .unwrap()
            .contains(&json!("leak.txt"))
    );
    for id in [
        "once-xml-unclosed-tool-tag",
        "once-xml-closed-invalid-tool-tag",
    ] {
        let trace = run_fixture_row(id);
        assert!(!trace.file_exists("leak.txt"), "{id}");
        let outcome = trace.result.as_ref().unwrap();
        assert_eq!(
            outcome.changed_paths,
            vec!["greeting.txt".to_string()],
            "{id}"
        );
        let raw_calls = trace
            .events
            .iter()
            .filter(|event| event["event"] == "tool_call_raw")
            .count();
        assert_eq!(raw_calls, 1, "{id}");
        assert!(trace.session.messages.iter().all(|message| {
            !message.content.contains("leak.txt")
                && message
                    .tool_calls
                    .iter()
                    .all(|call| call.arguments["path"] != "leak.txt")
        }));
    }
}

#[test]
fn excluded_runs_behave_exactly_as_with_the_plan_off() {
    for id in [
        "once-remaining-three-skips",
        "once-profile-not-generic",
        "once-completion-contract",
        "once-resumed-conversation",
    ] {
        run_fixture_row(id);
    }
    let trace = run_fixture_row("once-remaining-four-applies");
    assert_eq!(trace.plan_events()[0]["remaining_iterations"], 4);
}

#[test]
fn cancel_and_budget_end_keep_their_failure() {
    let trace = run_fixture_row("once-cancel-after-capture");
    assert!(trace.result.as_ref().unwrap_err().contains("interrupted"));
    let trace = run_fixture_row("once-budget-after-plan-only-turn");
    assert!(trace.result.is_err());
}

#[test]
fn a_later_user_correction_never_sees_the_old_note() {
    let first = scenario("once-captured-native").run();
    assert_eq!(first.plan_phases()[1][0], "captured");
    let mut correction = scenario("once-captured-native");
    correction.id = "once-correction".to_string();
    correction.prompt = corpus()["prompts"]["correction"]
        .as_str()
        .unwrap()
        .to_string();
    correction.script = vec![
        json!({"write": ["greeting.txt", "good morning\n"]}),
        json!({"final": "greeting.txt now says good morning."}),
    ];
    let session = first.session.clone();
    let trace = correction.run_after(session);
    assert!(trace.result.is_ok(), "{:?}", trace.result);
    assert_eq!(
        trace.plan_phases(),
        vec![["skipped".to_string(), "resumed_conversation".to_string()]]
    );
    assert!(trace.request_indexes(NOTE_MARKER).is_empty());
    assert!(trace.request_indexes(GUIDANCE_MARKER).is_empty());
    assert!(
        trace
            .request_indexes("write a short greeting file")
            .is_empty()
    );
}

#[test]
fn invalid_values_fail_before_any_request_without_echoing_the_value() {
    let base = scenario("default-off-unset");
    for value in [
        "ONCE",
        "on",
        "true",
        "1",
        "once,once",
        "sk-test-945-NOT-A-REAL-KEY",
    ] {
        let trace = base.with_setting(Some(value)).run();
        let error = trace.result.as_ref().unwrap_err();
        assert_eq!(
            error,
            &format!("invalid {ENV}: expected unset, empty, `off`, or `once`"),
            "{value}"
        );
        assert!(trace.requests.is_empty(), "{value}");
        assert!(trace.events.is_empty(), "{value}: {:?}", trace.events);
    }
    #[cfg(unix)]
    {
        let _lock = env_lock();
        set_env_bytes(ENV, &[0x6f, 0x6e, 0x63, 0x65, 0xff]);
        let trace = base.run_locked(|client| client, None);
        set_env(ENV, None);
        assert_eq!(
            trace.result.as_ref().unwrap_err(),
            &format!("invalid {ENV}: expected unset, empty, `off`, or `once`")
        );
        assert!(trace.requests.is_empty());
    }
    // The legacy name is not an alias.
    let _lock = env_lock();
    set_env(LEGACY_ENV, Some("once"));
    let trace = base.run_locked(|client| client, None);
    set_env(LEGACY_ENV, None);
    trace.assert_expectations(&base.expect);
}

#[test]
fn once_with_the_completion_obligation_review_is_refused_before_any_request() {
    let mut base = scenario("once-captured-native");
    base.review_setting = Some("once".to_string());
    let trace = base.run();
    assert_eq!(
        trace.result.as_ref().unwrap_err(),
        &format!("{ENV} and {REVIEW_ENV} cannot both be enabled")
    );
    assert!(trace.requests.is_empty());
    assert!(trace.events.is_empty(), "{:?}", trace.events);

    // With the plan off the review runs as before, untouched.
    let mut off = scenario("default-off-unset");
    off.review_setting = Some("once".to_string());
    // The review defers the first final once; the script answers it.
    off.script.push(json!({"final": "Checked greeting.txt."}));
    let trace = off.run();
    assert!(trace.result.is_ok(), "{:?}", trace.result);
    assert!(trace.plan_events().is_empty());
    assert!(
        trace
            .events
            .iter()
            .any(|event| event["event"] == "completion_obligation_review")
    );
}

#[test]
fn the_variable_is_read_once_at_run_start() {
    let base = scenario("once-captured-native");
    for change in ["off", "not-a-mode"] {
        let mut client_change = Some(change);
        let trace = base.run_with(|mut client| {
            client.env_change_on_first_call = client_change.take();
            client
        });
        trace.assert_expectations(&base.expect);
    }
    let off = scenario("default-off-unset");
    let trace = off.run_with(|mut client| {
        client.env_change_on_first_call = Some("once");
        client
    });
    trace.assert_expectations(&off.expect);
}

#[test]
fn model_claims_and_wrong_expectations_change_no_existing_judgement() {
    for id in ["once-self-claimed-verified", "once-wrong-expectation"] {
        let trace = run_fixture_row(id);
        let off = scenario(id).with_setting(None).run();
        assert_eq!(trace.outcome(), off.outcome(), "{id}");
        assert_eq!(trace.event_names(false), off.event_names(true), "{id}");
        let claims = |trace: &Trace| {
            trace
                .events
                .iter()
                .filter(|event| {
                    event.get("assurance_level").is_some()
                        || event.get("acceptance_passed").is_some()
                        || event.get("verification") == Some(&json!("passed"))
                })
                .count()
        };
        assert_eq!(claims(&trace), claims(&off), "{id}");
        for event in trace.plan_events() {
            for key in [
                "verified",
                "coverage_passed",
                "assurance_level",
                "verification",
            ] {
                assert!(event.get(key).is_none(), "{id}: {key}");
            }
        }
    }
}

/// A synthetic credential name the dotenv collector treats as a credential.
const SYNTHETIC_CREDENTIAL: &str = "SYNTHETIC_945_API_KEY";

fn with_credential(scenario: &Scenario, value: &str) -> Scenario {
    let mut copy = scenario.clone();
    copy.seed.push((
        ".env".to_string(),
        format!("{SYNTHETIC_CREDENTIAL}={value}\n"),
    ));
    copy
}

#[test]
fn events_and_requests_carry_no_note_text_prompt_or_secret() {
    const SECRET: &str = "sk-test-945-NOT-A-REAL-KEY-7f3a";
    let mut base = scenario("once-captured-native");
    let note = json!({
        "obligations": [{
            "id": "O1",
            "stated_requirement": format!("use the key {SECRET}"),
            "expected_behavior": "greeting.txt holds one greeting line",
            "assumption": "",
        }],
        "examples": [],
    });
    base.script = vec![
        json!({"write": ["greeting.txt", "hello\n"],
               "preamble": format!("<requirement_plan>{note}</requirement_plan>")}),
        json!({"final": format!("Done with {SECRET}.")}),
    ];
    let trace = with_credential(&base, SECRET).run();
    assert!(trace.result.is_ok(), "{:?}", trace.result);
    assert_eq!(trace.plan_phases()[1][0], "captured");
    // The kept note was scrubbed and is never restored to plain text.
    assert_eq!(trace.count(1, NOTE_MARKER), 1);
    for messages in &trace.requests {
        let text = serde_json::to_string(messages).unwrap();
        assert!(!text.contains(SECRET), "{text}");
    }
    assert!(trace.count(1, "use the key ") == 1);
    for event in trace.plan_events() {
        assert_fixed_shape(&event);
        let text = event.to_string();
        for forbidden in [
            SECRET,
            "greeting",
            "use the key",
            "O1",
            "obligations",
            "<requirement_plan>",
            "Write a short",
        ] {
            assert!(!text.contains(forbidden), "{forbidden} in {text}");
        }
    }

    // Invalid blocks: no unknown key name and no parser excerpt anywhere.
    for id in ["once-invalid-unknown-field", "once-oversize-block"] {
        let trace = scenario(id).run();
        for event in trace.plan_events() {
            let text = event.to_string();
            for forbidden in ["verified", "unknown field", "line", "column", "expected"] {
                assert!(!text.contains(forbidden), "{id}: {forbidden} in {text}");
            }
        }
    }
}

#[test]
fn credentials_equal_to_or_inside_a_plan_word_are_refused_without_the_value() {
    use commandagent::sensitive_data::{RegistrationRefusal, ReservedKind, SecretCatalog};
    let words = FIXED_KEYS[4..]
        .iter()
        .chain(FIXED_STRINGS[2..].iter())
        .chain([EVENT, ENV].iter())
        .copied()
        .collect::<Vec<_>>();
    for value in words
        .iter()
        .chain(["_only_turn", "_request", "ENT_PLAN"].iter())
    {
        let refusal = SecretCatalog::new()
            .register_credential(SYNTHETIC_CREDENTIAL, value)
            .expect_err(value);
        if value.chars().count() >= commandagent::sensitive_data::MIN_GENERIC_SECRET_LEN {
            assert_eq!(
                refusal,
                RegistrationRefusal::Reserved {
                    name: SYNTHETIC_CREDENTIAL.to_string(),
                    kind: ReservedKind::FixedIdentifier,
                },
                "{value}"
            );
        }
        assert!(!refusal.to_string().contains(value), "{value}: {refusal}");
    }
    let base = scenario("once-captured-native");
    for value in [
        "captured",
        "valid_block",
        "plan_only_turn",
        "requirement_plan",
    ] {
        let trace = with_credential(&base, value).run();
        let error = trace.result.as_ref().unwrap_err();
        assert!(error.contains(SYNTHETIC_CREDENTIAL), "{value}: {error}");
        assert!(!error.contains(value), "{value}: {error}");
        assert!(trace.requests.is_empty(), "{value}");
        assert!(trace.plan_events().is_empty(), "{value}");
    }
    // A registered credential leaves every plan word intact.
    let plain = base.run();
    let scrubbed = with_credential(&base, "sk-test-945-NOT-A-REAL-KEY-c0ffee").run();
    assert_eq!(scrubbed.plan_events(), plain.plan_events());
}

#[test]
fn corpus_rows_are_synthetic_and_cover_every_declared_case() {
    let corpus = corpus();
    assert_eq!(corpus["kind"], "synthetic");
    assert_eq!(corpus["off_golden_source"], OFF_GOLDEN_SOURCE);
    let ids = scenarios()
        .into_iter()
        .map(|scenario| scenario.id)
        .collect::<Vec<_>>();
    for required in [
        "default-off-unset",
        "default-off-empty",
        "default-off-explicit",
        "once-captured-native",
        "once-captured-xml",
        "once-plan-only-turn",
        "once-missing-block",
        "once-invalid-unknown-field",
        "once-oversize-block",
        "once-xml-unclosed-tool-tag",
        "once-fresh-retry",
        "once-compaction",
        "once-cancel-after-capture",
        "once-budget-after-plan-only-turn",
    ] {
        assert!(ids.iter().any(|id| id == required), "{required}");
    }
    let readme = std::fs::read_to_string(corpus_path("README.md")).unwrap();
    assert!(readme.contains("synthetic"));
    let raw = std::fs::read_to_string(corpus_path("fixtures/plan.json")).unwrap();
    assert!(!raw.contains("dev-28"));
}

#[test]
fn later_duplicate_and_nested_note_regions_never_reach_the_tool_parser() {
    for (id, greeting_from_first_reply) in [
        ("once-xml-valid-then-closed-block", true),
        ("once-xml-valid-then-unclosed-block", false),
        ("once-xml-invalid-then-closed-block", true),
        ("once-xml-invalid-then-unclosed-block", false),
        ("once-xml-nested-block", true),
        ("once-native-valid-then-unclosed-block", true),
    ] {
        let trace = run_fixture_row(id);
        assert!(!trace.file_exists("leak.txt"), "{id}");
        assert!(trace.file_exists("greeting.txt"), "{id}");
        let raw_paths = trace
            .events
            .iter()
            .filter(|event| event["event"] == "tool_call_raw")
            .count();
        assert_eq!(raw_paths, 1, "{id}: only the legitimate Write");
        let outcome = trace.result.as_ref().unwrap();
        assert_eq!(
            outcome.changed_paths,
            vec!["greeting.txt".to_string()],
            "{id}"
        );
        // The legitimate tool outside every region ran in the first reply:
        // the second reply is already the final.
        if greeting_from_first_reply {
            assert_eq!(trace.requests.len(), 2, "{id}");
        }
        // Neither the note nor the region text stays in the history.
        for message in &trace.session.messages {
            for forbidden in [
                "<requirement_plan>",
                "</requirement_plan>",
                "leak.txt",
                "stated_requirement",
            ] {
                assert!(!message.content.contains(forbidden), "{id}: {forbidden}");
            }
        }
        for messages in &trace.requests[1..] {
            let text = serde_json::to_string(messages).unwrap();
            assert!(!text.contains("leak.txt"), "{id}");
            assert!(!text.contains("stated_requirement"), "{id}");
        }
    }
}

#[test]
fn the_output_length_guard_still_counts_a_planning_turn() {
    let row = scenario("once-max-length-plan-turn");
    let trace = run_fixture_row("once-max-length-plan-turn");
    assert_eq!(trace.plan_events()[1]["plan_only_turn"], true);
    let error = trace.result.as_ref().unwrap_err();
    assert!(
        error.contains("max_length_no_tool_call_repeated"),
        "{error}"
    );
    // The same threshold as the same script with the plan off.
    let off = row.with_setting(None).run();
    let first_line = |trace: &Trace| {
        trace
            .result
            .as_ref()
            .unwrap_err()
            .lines()
            .next()
            .unwrap()
            .to_string()
    };
    assert_eq!(first_line(&trace), first_line(&off));
    assert_eq!(trace.requests.len(), off.requests.len());
    assert!(!trace.file_exists("greeting.txt"));
}

#[test]
fn the_last_iteration_failure_kind_decides_the_release_reason() {
    for (id, error_part) in [
        ("once-last-provider-failure", "scripted client exhausted"),
        ("once-last-normalization-failure", "malformed"),
        ("once-last-fatal-tool", "blocked"),
        ("once-last-cancel", "interrupted"),
    ] {
        let trace = run_fixture_row(id);
        let error = trace.result.as_ref().unwrap_err();
        assert!(error.contains(error_part), "{id}: {error}");
        assert_eq!(trace.plan_events()[2]["remaining_iterations"], Value::Null);
        // Every case reached the last iteration.
        let provider_turns = trace
            .events
            .iter()
            .filter(|event| event["event"] == "provider_turn_duration")
            .count();
        assert!(provider_turns >= 4, "{id}");
    }
    let trace = run_fixture_row("once-budget-after-plan-only-turn");
    assert_eq!(trace.plan_phases()[2], ["released", "budget_exhausted"]);
}

#[test]
fn a_client_that_parses_xml_itself_is_skipped_exactly_like_off() {
    for id in ["once-client-parses-xml", "once-client-parses-xml-text-mode"] {
        let trace = run_fixture_row(id);
        assert_eq!(
            trace.plan_phases(),
            vec![["skipped".to_string(), "provider_unsupported".to_string()]],
            "{id}"
        );
        assert!(trace.request_indexes(GUIDANCE_MARKER).is_empty(), "{id}");
    }
    // The same scripts with a client that does not parse XML still plan, and
    // Neither `allows_xml_fallback` (true for LM Studio, OpenAI-compatible,
    // and OpenAI, which do not parse XML before the host) nor
    // `supports_ollama_think` skips it: only the dedicated capability does.
    for id in ["once-captured-native", "once-captured-xml"] {
        let row = scenario(id);
        assert!(!row.client_parses_xml);
        let trace = row.run_with(|mut client| {
            client.allows_fallback = true;
            client.think = true;
            client
        });
        assert_eq!(trace.request_indexes(GUIDANCE_MARKER), vec![0], "{id}");
        assert_eq!(trace.plan_phases()[1][0], "captured", "{id}");
    }
}
