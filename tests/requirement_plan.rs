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

#[derive(Clone)]
struct ScriptedClient {
    replies: Arc<Mutex<Vec<AssistantReply>>>,
    requests: Arc<Mutex<Vec<Vec<ConversationMessage>>>>,
    native_flags: Arc<Mutex<Vec<bool>>>,
    native: bool,
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
                if let Some(write) = step.get("write") {
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
                }
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
        let trace = self.run_locked(adjust);
        set_env(ENV, None);
        set_env(REVIEW_ENV, None);
        trace
    }

    fn run_locked(&self, adjust: impl FnOnce(ScriptedClient) -> ScriptedClient) -> Trace {
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
            env_change_on_first_call: None,
        });
        let mut session = SessionSnapshot::new();
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

    fn summary(&self) -> Value {
        json!({
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
            if matches!(key.as_str(), "equivalent_to_off" | "note" | "source") {
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
