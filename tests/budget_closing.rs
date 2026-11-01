//! Issue #957: the opt-in `COMMANDAGENT_BUDGET_CLOSING=once`. Scenarios live
//! in the synthetic corpus fixture
//! `tests/corpus/apps/budget-closing/fixtures/events.jsonl` and run through the
//! public minimal loop with a scripted `ChatClient`.
//!
//! Every row's `base` was recorded from the base commit c141d740 before the
//! closing existed: the outcome, the request digests, the event-name sequence,
//! and the unused replies. Unset, empty, and `off` must reproduce it exactly.
//! The `once` expectations come from the rules in the Issue body.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use clap::Parser;
use commandagent::cli::Cli;
use commandagent::config::{
    Action, Config, ConfigFieldSources, NarrationMode, OllamaThink, OpenAiApi, PlanPreset,
    PromptLayout, Provider,
};
use commandagent::minimal_loop::loop_run::{RunStopReason, run_session_with_outcome_with_ui};
use commandagent::providers::streaming::CancelToken;
use commandagent::providers::{AssistantReply, ChatClient};
use commandagent::state::{ConversationMessage, SessionSnapshot, ToolCall};
use commandagent::tools::registry::ToolSpec;
use commandagent::tui::status::UiStatus;
use commandagent::tui::{InteractionUi, UiGuard};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const FIXTURE: &str = include_str!("corpus/apps/budget-closing/fixtures/events.jsonl");
const ENV: &str = "COMMANDAGENT_BUDGET_CLOSING";
const REVIEW_ENV: &str = "COMMANDAGENT_COMPLETION_OBLIGATION_REVIEW";
const EVENT: &str = "budget_closing";
const REVIEW_EVENT: &str = "completion_obligation_review";
/// The opening sentence of the fixed notice; it marks the request that
/// carries the notice without the test copying the whole text.
const NOTICE_MARKER: &str = "残りの呼出しは現在の回を含めて3回です。";
/// The opening sentence of the fixed deferral feedback.
const DEFERRAL_MARKER: &str = "失敗した確認または書込みが残っています。";
const EDIT_PROMPT: &str = "Write the notes file and check it with shell commands.";
const QUESTION_PROMPT: &str = "Report what the shell says about the notes file.";
const NUM_PREDICT: usize = 100;

/// The event numbers the digest does not compare, by exact key: wall-clock
/// values (`occurred_at_epoch_ms`, `duration_ms`, `elapsed_ms`), the
/// provider-turn prompt size estimate, which counts the system prompt that
/// names the temporary root and so changes with the root's length (it
/// differs between runs and platforms), and the stable-prefix estimate,
/// which is measured against the previous request kept in a process-wide
/// cache and so depends on the tests that ran before in the same binary.
/// The prompt size is checked separately: every provider turn must report a
/// positive value ([`assert_size_estimates_reported`]).
const NORMALIZED_NUMBER_FIELDS: [&str; 5] = [
    "occurred_at_epoch_ms",
    "duration_ms",
    "elapsed_ms",
    "estimated_prompt_tokens_sent",
    "estimated_stable_prefix_chars",
];

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
    native: bool,
    replies: Arc<Mutex<Vec<AssistantReply>>>,
    requests: Arc<Mutex<Vec<Vec<ConversationMessage>>>>,
    /// When set, a request carrying the notice raises this flag (the UI then
    /// reports an interrupt) and the call waits until the provider call
    /// cancels it, so the request is interrupted after it reached the client.
    abort_on_notice: Option<Arc<AtomicBool>>,
    cancel: Option<CancelToken>,
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
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        self.requests.lock().unwrap().push(messages.to_vec());
        if let Some(flag) = &self.abort_on_notice
            && messages
                .iter()
                .any(|message| message.content.contains(NOTICE_MARKER))
        {
            flag.store(true, Ordering::SeqCst);
            // Wait for the provider call to give up on this request; it
            // cancels the token on every exit. The bound only keeps a broken
            // run from hanging the test.
            let token = self.cancel.clone().expect("the provider call sets a token");
            let started = std::time::Instant::now();
            while !token.is_cancelled() && started.elapsed() < Duration::from_secs(60) {
                std::thread::sleep(Duration::from_millis(1));
            }
            anyhow::bail!("scripted request interrupted");
        }
        let mut replies = self.replies.lock().unwrap();
        if replies.is_empty() {
            anyhow::bail!("scripted client exhausted");
        }
        Ok(replies.remove(0))
    }

    fn set_cancel_token(&mut self, token: CancelToken) {
        self.cancel = Some(token);
    }
}

/// A UI that reports an interrupt once the given model call has started,
/// that is, after the request was built and before the client is called.
struct CancelAtModelCall {
    calls: Cell<usize>,
    cancel_at: usize,
    /// Raised by the client once it holds the notice request.
    received_notice: Option<Arc<AtomicBool>>,
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
            || self
                .received_notice
                .as_ref()
                .is_some_and(|flag| flag.load(Ordering::SeqCst))
    }
}

/// The iteration budget is parsed from `--max-iterations`, as the binary does.
/// A zero budget cannot be given on the command line; the row sets it on the
/// configuration directly to reach an iteration limit of three.
fn max_iterations(row: &Value) -> usize {
    let value = row["max_iterations"].as_u64().unwrap();
    if value == 0 {
        return 0;
    }
    Cli::parse_from(["commandagent", "--max-iterations", &value.to_string()]).max_iterations
}

/// `--max-iterations` + 3 empty-response recovery iterations, plus the
/// existing completion-contract allowance when the row carries a contract.
fn iteration_limit(row: &Value) -> usize {
    let base = max_iterations(row) + 3;
    match row.get("contract") {
        Some(contract) => {
            let required = contract["required_paths"].as_array().unwrap().len();
            base + 3 * required.max(1) + 1
        }
        None => base,
    }
}

fn config(root: &Path, row: &Value) -> Config {
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
        num_predict: NUM_PREDICT,
        max_iterations: max_iterations(row),
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
        profile: row["profile"].as_str().unwrap_or("generic").to_string(),
        profile_explicit: true,
        profile_inference: None,
        style: "default".to_string(),
        action: Action::Repl,
    }
}

fn tool_call(index: usize, call: &Value) -> ToolCall {
    let (name, arguments) = if let Some(write) = call.get("write") {
        ("Write", json!({"path": write[0], "content": write[1]}))
    } else if let Some(edit) = call.get("edit") {
        (
            "Edit",
            json!({"path": edit[0], "old_string": edit[1], "new_string": edit[2]}),
        )
    } else if let Some(command) = call.get("bash") {
        ("Bash", json!({"command": command}))
    } else if let Some(path) = call.get("read") {
        ("Read", json!({"path": path}))
    } else {
        panic!("unknown script call {call}");
    };
    ToolCall {
        id: format!("call-{index}"),
        name: name.to_string(),
        arguments,
    }
}

fn replies(script: &[Value]) -> Vec<AssistantReply> {
    let mut next_id = 0usize;
    script
        .iter()
        .map(|step| {
            let reply = |content: &str, tool_calls: Vec<ToolCall>| AssistantReply {
                content: content.to_string(),
                tool_calls,
                prompt_tokens: None,
                completion_tokens: None,
            };
            if let Some(text) = step.get("final") {
                return reply(text.as_str().unwrap(), Vec::new());
            }
            if step.get("empty").is_some() {
                return reply("", Vec::new());
            }
            let calls = step
                .as_array()
                .unwrap()
                .iter()
                .map(|call| {
                    next_id += 1;
                    tool_call(next_id, call)
                })
                .collect();
            reply("", calls)
        })
        .collect()
}

struct Trace {
    root: PathBuf,
    _dir: tempfile::TempDir,
    result: Result<(RunStopReason, String, usize), String>,
    requests: Vec<Vec<ConversationMessage>>,
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
        // Wall-clock durations and generated recovery-file ids differ per run.
        for (pattern, replacement) in [
            (r"elapsed_ms: [0-9]+", "elapsed_ms: <N>"),
            (r"total_duration_ms=[0-9]+", "total_duration_ms=<N>"),
            (
                r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}",
                "<ID>",
            ),
        ] {
            text = regex::Regex::new(pattern)
                .unwrap()
                .replace_all(&text, replacement)
                .into_owned();
        }
        text
    }

    fn request_digests(&self) -> Vec<String> {
        self.requests
            .iter()
            .map(|messages| {
                let text = self.normalize(&serde_json::to_string(messages).unwrap());
                Sha256::digest(text.as_bytes())
                    .iter()
                    .take(8)
                    .map(|byte| format!("{byte:02x}"))
                    .collect()
            })
            .collect()
    }

    /// One digest over every event line in order. Only the numbers of
    /// [`NORMALIZED_NUMBER_FIELDS`] are replaced, plus the temporary root,
    /// generated ids, and `elapsed_ms: N` inside strings; every other key and
    /// value, configuration numbers included, is compared as written.
    fn event_digest(&self) -> String {
        let lines = self.normalized_events_without(None).join("\n");
        Sha256::digest(lines.as_bytes())
            .iter()
            .take(8)
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    /// The normalized event lines, leaving out one event name.
    fn normalized_events_without(&self, name: Option<&str>) -> Vec<String> {
        self.events
            .iter()
            .filter(|event| name.is_none_or(|name| event["event"] != name))
            .map(|event| serde_json::to_string(&self.normalize_event(event)).unwrap())
            .collect()
    }

    fn normalize_event(&self, value: &Value) -> Value {
        match value {
            Value::Object(object) => Value::Object(
                object
                    .iter()
                    .map(|(key, value)| {
                        let value = if NORMALIZED_NUMBER_FIELDS.contains(&key.as_str())
                            && value.is_number()
                        {
                            json!("<N>")
                        } else {
                            self.normalize_event(value)
                        };
                        (key.clone(), value)
                    })
                    .collect(),
            ),
            Value::Array(items) => Value::Array(
                items
                    .iter()
                    .map(|item| self.normalize_event(item))
                    .collect(),
            ),
            Value::String(text) => Value::String(self.normalize(text)),
            other => other.clone(),
        }
    }

    /// The messages of one request, each serialized with the root normalized.
    fn request_messages(&self, index: usize) -> Vec<String> {
        self.requests[index]
            .iter()
            .map(|message| self.normalize(&serde_json::to_string(message).unwrap()))
            .collect()
    }

    fn outcome(&self) -> Value {
        match &self.result {
            Ok((stop_reason, final_text, iterations)) => json!({
                "stop_reason": format!("{stop_reason:?}"),
                "final_text": final_text,
                "iterations": iterations,
            }),
            Err(error) => json!({"error": self.normalize(error)}),
        }
    }

    fn event_names(&self, include_closing: bool) -> Vec<String> {
        self.events
            .iter()
            .filter(|event| include_closing || event["event"] != EVENT)
            .map(|event| event["event"].as_str().unwrap_or_default().to_string())
            .collect()
    }

    /// What the base commit recorded for the row.
    fn base_summary(&self) -> Value {
        json!({
            "source": "c141d740",
            "outcome": self.outcome(),
            "remaining_replies": self.remaining_replies,
            "request_digests": self.request_digests(),
            "event_names": self.event_names(true),
            "event_digest": self.event_digest(),
        })
    }

    fn closing_events(&self) -> Vec<Value> {
        self.events
            .iter()
            .filter(|event| event["event"] == EVENT)
            .cloned()
            .collect()
    }

    fn phases(&self) -> Vec<String> {
        self.closing_events()
            .iter()
            .map(|event| event["phase"].as_str().unwrap().to_string())
            .collect()
    }

    fn review_events(&self) -> Vec<Value> {
        self.events
            .iter()
            .filter(|event| event["event"] == REVIEW_EVENT)
            .cloned()
            .collect()
    }

    /// 1-based indexes of the requests whose messages carry `marker`.
    fn requests_with(&self, marker: &str) -> Vec<usize> {
        self.requests
            .iter()
            .enumerate()
            .filter(|(_, messages)| {
                messages
                    .iter()
                    .any(|message| message.content.contains(marker))
            })
            .map(|(index, _)| index + 1)
            .collect()
    }

    fn claimed(&self) -> bool {
        matches!(self.result, Ok((RunStopReason::AssistantFinal, _, _)))
    }
}

fn run_with(row: &Value, setting: Option<&str>, review: Option<&str>) -> Trace {
    let _lock = env_lock();
    set_env(ENV, setting);
    set_env(REVIEW_ENV, review);
    let dir = tempfile::tempdir().unwrap();
    // The canonical root keeps tool output the same on every platform.
    let root = dir.path().canonicalize().unwrap();
    let mut config = config(&root, row);
    if let Some(contract) = row.get("contract") {
        let path = root.join("completion-contract.json");
        std::fs::write(&path, serde_json::to_vec_pretty(contract).unwrap()).unwrap();
        config.completion_contract_path = Some(path);
    }
    let required_paths: Vec<String> = row["required_paths"]
        .as_array()
        .map(|paths| {
            paths
                .iter()
                .map(|path| path.as_str().unwrap().to_string())
                .collect()
        })
        .unwrap_or_default();
    let received_notice =
        (row["abort_on_notice"] == true).then(|| Arc::new(AtomicBool::new(false)));
    let mut client = ScriptedClient {
        native: row["native"].as_bool().unwrap_or(true),
        replies: Arc::new(Mutex::new(replies(row["script"].as_array().unwrap()))),
        requests: Arc::new(Mutex::new(Vec::new())),
        abort_on_notice: received_notice.clone(),
        cancel: None,
    };
    let prompt = match row["prompt"].as_str().unwrap() {
        "edit" => EDIT_PROMPT,
        "question" => QUESTION_PROMPT,
        other => panic!("unknown prompt {other}"),
    };
    let ui = CancelAtModelCall {
        calls: Cell::new(0),
        cancel_at: row["cancel_at_request"]
            .as_u64()
            .map_or(usize::MAX, |value| value as usize),
        received_notice,
    };
    let result = run_session_with_outcome_with_ui(
        &mut client,
        &mut SessionSnapshot::new(),
        prompt,
        &required_paths,
        &config,
        &ui,
    )
    .map(|outcome| (outcome.stop_reason, outcome.final_text, outcome.iterations))
    .map_err(|error| format!("{error:#}"));
    set_env(ENV, None);
    set_env(REVIEW_ENV, None);
    let events = std::fs::read_to_string(root.join("events.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect();
    let requests = client.requests.lock().unwrap().clone();
    let remaining_replies = client.replies.lock().unwrap().len();
    Trace {
        root,
        _dir: dir,
        result,
        requests,
        remaining_replies,
        events,
    }
}

fn review_setting(row: &Value) -> Option<&'static str> {
    (row["review"] == true).then_some("once")
}

fn run(row: &Value, setting: Option<&str>) -> Trace {
    run_with(row, setting, review_setting(row))
}

fn rows() -> Vec<Value> {
    FIXTURE
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .inspect(|row| assert_eq!(row["kind"], "synthetic", "{row}"))
        .collect()
}

fn row(id: &str) -> Value {
    rows()
        .into_iter()
        .find(|row| row["id"] == id)
        .unwrap_or_else(|| panic!("missing fixture row {id}"))
}

const FIXED_KEYS: [&str; 13] = [
    "event",
    "schema_version",
    "phase",
    "guard_reason",
    "prompt_version",
    "iteration",
    "iteration_limit",
    "remaining_calls",
    "unresolved_bash_failures",
    "unresolved_write_failures",
    "resolved_failures",
    "policy_rejected_bash",
    "final_deferrals",
];

const FIXED_STRINGS: [&str; 10] = [
    EVENT,
    "1",
    "notice",
    "final_accepted",
    "final_deferred",
    "exhausted_after_notice",
    "remaining_calls_reached",
    "no_unresolved_failure",
    "unresolved_failure",
    "iteration_budget_exhausted",
];

/// The reasons the existing iteration-budget exit can choose from.
const BUDGET_REASONS: [&str; 5] = [
    "artifact_follow_through_exhausted",
    "model_stagnation:read_only_loop",
    "model_stagnation:no_progress_recorded",
    "loop_progress_exhausted",
    "scaffold_artifact_follow_through_exhausted",
];

/// A closing event carries only fixed keys, numbers, and fixed words.
fn assert_fixed_shape(event: &Value) {
    for (key, value) in event.as_object().unwrap() {
        assert!(FIXED_KEYS.contains(&key.as_str()), "key {key}: {event}");
        match value {
            Value::Number(_) => {}
            Value::String(text) => assert!(
                FIXED_STRINGS.contains(&text.as_str()),
                "free text {text:?} in {event}"
            ),
            other => panic!("unexpected value {other} under {key}: {event}"),
        }
    }
}

/// The 1-based request that leaves three calls, counting itself.
fn notice_request(row: &Value) -> usize {
    iteration_limit(row) - 2
}

/// The prompt size estimate the event digest leaves out is still reported:
/// every provider turn that carried a request has a positive value, and the
/// stable-prefix estimate is a number.
fn assert_size_estimates_reported(trace: &Trace) {
    for event in &trace.events {
        if event["event"] != "provider_turn_duration" {
            continue;
        }
        assert!(
            event["estimated_prompt_tokens_sent"]
                .as_u64()
                .is_some_and(|value| value > 0),
            "{event}"
        );
        assert!(event["estimated_stable_prefix_chars"].is_u64(), "{event}");
    }
}

/// Unset, empty, and `off` reproduce what the base commit recorded.
fn assert_off_matches_base(row: &Value) {
    let id = row["id"].as_str().unwrap();
    assert_eq!(row["base"]["source"], "c141d740", "{id}");
    for setting in [None, Some(""), Some("off")] {
        let trace = run(row, setting);
        assert_size_estimates_reported(&trace);
        assert_eq!(trace.base_summary(), row["base"], "{id} {setting:?}");
        assert!(trace.closing_events().is_empty(), "{id} {setting:?}");
        assert!(trace.requests_with(NOTICE_MARKER).is_empty(), "{id}");
    }
}

/// The `once` run of a row in scope: the notice is in exactly one sent
/// request, the requests before it are the base requests, and every
/// closing event has the fixed shape.
fn run_once(row: &Value) -> Trace {
    let id = row["id"].as_str().unwrap();
    let trace = run(row, Some("once"));
    assert_size_estimates_reported(&trace);
    let notice = notice_request(row);
    assert_eq!(trace.requests_with(NOTICE_MARKER), vec![notice], "{id}");
    let base_digests = row["base"]["request_digests"].as_array().unwrap();
    assert_eq!(
        json!(trace.request_digests()[..notice - 1]),
        json!(base_digests[..notice - 1]),
        "{id}: requests before the notice"
    );
    assert!(
        trace.requests.len() <= iteration_limit(row),
        "{id}: no call beyond the limit"
    );
    let events = trace.closing_events();
    assert_eq!(events[0]["phase"], "notice", "{id}");
    assert_eq!(events[0]["iteration"], notice, "{id}");
    assert_eq!(events[0]["iteration_limit"], iteration_limit(row), "{id}");
    assert_eq!(events[0]["remaining_calls"], 3, "{id}");
    assert_eq!(events[0]["prompt_version"], 1, "{id}");
    assert_eq!(
        trace
            .phases()
            .iter()
            .filter(|phase| *phase == "notice")
            .count(),
        1,
        "{id}"
    );
    for event in &events {
        assert_fixed_shape(event);
    }
    // Every event before the notice is, payload for payload, what the same
    // script records with the variable off (pinned to the base above).
    let off = run(row, None);
    assert_eq!(off.event_digest(), row["base"]["event_digest"], "{id}");
    let before = trace
        .events
        .iter()
        .position(|event| event["event"] == EVENT)
        .unwrap();
    assert_eq!(
        trace.normalized_events_without(None)[..before],
        off.normalized_events_without(None)[..before],
        "{id}: events before the notice"
    );
    trace
}

/// A row whose unresolved failure must keep every final after the notice
/// from being claimed until the existing budget exit ends the run.
fn assert_guarded(id: &str) -> Trace {
    let row = row(id);
    let trace = run_once(&row);
    assert!(!trace.claimed(), "{id}: {:?}", trace.outcome());
    let phases = trace.phases();
    assert!(phases.len() >= 3, "{id}: {phases:?}");
    assert_eq!(phases.last().unwrap(), "exhausted_after_notice", "{id}");
    assert!(
        phases[1..phases.len() - 1]
            .iter()
            .all(|phase| phase == "final_deferred"),
        "{id}: {phases:?}"
    );
    assert_eq!(
        trace.requests.len(),
        iteration_limit(&row),
        "{id}: the run used the existing budget and no more"
    );
    let deferrals = phases.len() - 2;
    // Each deferral's feedback goes into the next request; the last final
    // is deferred on the last iteration, so no request follows it.
    let deferred_iterations: Vec<usize> = trace.closing_events()[1..=deferrals]
        .iter()
        .map(|event| event["iteration"].as_u64().unwrap() as usize)
        .collect();
    assert_eq!(
        *deferred_iterations.last().unwrap(),
        iteration_limit(&row),
        "{id}"
    );
    let expected: Vec<usize> = deferred_iterations[..deferrals - 1]
        .iter()
        .map(|iteration| iteration + 1)
        .collect();
    assert_eq!(trace.requests_with(DEFERRAL_MARKER), expected, "{id}");
    let error = trace.outcome()["error"].as_str().unwrap().to_string();
    assert!(
        BUDGET_REASONS
            .iter()
            .any(|reason| error.starts_with(&format!("{reason}:"))),
        "{id}: {error}"
    );
    let stop = trace
        .events
        .iter()
        .filter(|event| event["event"] == "loop_stop")
        .collect::<Vec<_>>();
    assert_eq!(stop.len(), 1, "{id}");
    assert!(
        BUDGET_REASONS.contains(&stop[0]["reason"].as_str().unwrap()),
        "{id}: {}",
        stop[0]
    );
    let exhausted = trace.closing_events().last().unwrap().clone();
    assert_eq!(exhausted["final_deferrals"], deferrals, "{id}");
    assert_eq!(exhausted["remaining_calls"], 0, "{id}");
    trace
}

fn assert_accepted(id: &str) -> Trace {
    let row = row(id);
    let trace = run_once(&row);
    assert!(trace.claimed(), "{id}: {:?}", trace.outcome());
    assert_eq!(trace.outcome(), row["base"]["outcome"], "{id}");
    assert_eq!(trace.phases(), ["notice", "final_accepted"], "{id}");
    let accepted = &trace.closing_events()[1];
    assert_eq!(accepted["unresolved_bash_failures"], 0, "{id}");
    assert_eq!(accepted["unresolved_write_failures"], 0, "{id}");
    assert_eq!(accepted["guard_reason"], "no_unresolved_failure", "{id}");
    // Without an unresolved failure every other event keeps its payload.
    let off = run(&row, None);
    assert_eq!(
        trace.normalized_events_without(Some(EVENT)),
        off.normalized_events_without(None),
        "{id}: events other than budget_closing"
    );
    trace
}

#[test]
fn every_fixture_row_is_named_by_a_test() {
    let mut ids: Vec<String> = rows()
        .iter()
        .map(|row| row["id"].as_str().unwrap().to_string())
        .collect();
    ids.sort();
    let mut named: Vec<String> = [
        "accept-after-same-check",
        "unrelated-bash-success",
        "write-without-rerun",
        "other-path-write",
        "failure-before-notice",
        "no-write-bash-failure",
        "read-and-self-report",
        "other-anchor-edit",
        "two-failed-edits-one-resolved",
        "matching-edit-resolves",
        "consecutive-finals",
        "default-budget-tool-turns",
        "limit-three-no-notice",
        "pending-feedback-then-notice",
        "fresh-retry-native",
        "fresh-retry-xml",
        "gate-missing-required-path",
        "gate-completion-without-write",
        "gate-missing-import",
        "read-only-accepted",
        "with-review-early-final",
        "with-review-tail-skip",
        "profile-not-generic",
        "completion-contract-run",
        "policy-rejected-bash",
        "unchanged-write-after-ambiguous-edit",
        "hidden-path-bash-rejected",
        "cancel-before-notice-request",
        "post-write-read-after-failed-write",
        "post-write-read-accepted",
    ]
    .iter()
    .map(|id| id.to_string())
    .collect();
    named.sort();
    assert_eq!(ids, named);
}

// Test 1: off.

#[test]
fn unset_empty_and_off_reproduce_the_base_requests_events_and_outcome() {
    for row in rows() {
        assert_off_matches_base(&row);
    }
}

#[test]
fn an_invalid_value_is_a_configuration_error_naming_only_the_variable() {
    let row = row("accept-after-same-check");
    for value in ["ONCE", "yes", "twice", "sk-test-957-NOT-A-KEY"] {
        let trace = run(&row, Some(value));
        let error = trace.outcome()["error"].as_str().unwrap().to_string();
        assert_eq!(
            error,
            format!("invalid {ENV}: expected unset, empty, `off`, or `once`")
        );
        assert!(!error.contains(value), "{error}");
        assert!(trace.requests.is_empty(), "no request is sent");
        assert!(trace.closing_events().is_empty());
    }
}

// Test 2: the notice.

#[test]
fn the_notice_goes_into_the_request_with_three_calls_left_and_follows_the_flag() {
    let row = row("default-budget-tool-turns");
    // The default --max-iterations; the limit comes from the parsed flag.
    assert_eq!(
        max_iterations(&row),
        Cli::parse_from(["commandagent"]).max_iterations
    );
    let trace = run_once(&row);
    assert_eq!(trace.requests.len(), iteration_limit(&row));
    assert_eq!(trace.phases(), ["notice", "exhausted_after_notice"]);
    // No final was given, so the run ends exactly as at base.
    assert_eq!(trace.outcome(), row["base"]["outcome"]);
    assert_eq!(trace.remaining_replies, 0);
    // A smaller --max-iterations moves the notice with the limit.
    let small = self::row("accept-after-same-check");
    assert_eq!(notice_request(&small), max_iterations(&small) + 1);
    let trace = run_once(&small);
    assert_eq!(trace.requests_with(NOTICE_MARKER), vec![3]);
}

#[test]
fn a_limit_of_three_never_sends_the_notice() {
    let row = row("limit-three-no-notice");
    assert_eq!(iteration_limit(&row), 3);
    let trace = run(&row, Some("once"));
    assert_eq!(trace.base_summary(), row["base"]);
    assert!(trace.closing_events().is_empty());
}

#[test]
fn the_notice_follows_existing_feedback_and_keeps_it() {
    let row = row("pending-feedback-then-notice");
    let off = run(&row, None);
    let trace = run_once(&row);
    let index = notice_request(&row) - 1;
    let sent = &trace.requests[index];
    let base = &off.requests[index];
    assert_eq!(sent.len(), base.len());
    let (sent_messages, base_messages) =
        (trace.request_messages(index), off.request_messages(index));
    assert_eq!(
        sent_messages[..sent.len() - 1],
        base_messages[..base.len() - 1]
    );
    let kept = &base.last().unwrap().content;
    assert_eq!(sent.last().unwrap().role, "user");
    assert!(!kept.contains(NOTICE_MARKER));
    assert!(
        sent.last()
            .unwrap()
            .content
            .starts_with(&format!("{kept}\n\n{NOTICE_MARKER}")),
        "{}",
        sent.last().unwrap().content
    );
    assert_eq!(trace.phases(), ["notice", "final_accepted"]);
    assert!(trace.claimed());
}

// Test 3: the fresh-session retry.

#[test]
fn a_fresh_session_retry_on_the_notice_request_carries_only_the_prompt_and_notice() {
    for id in ["fresh-retry-native", "fresh-retry-xml"] {
        let row = row(id);
        let off = run(&row, None);
        let trace = run_once(&row);
        let index = notice_request(&row) - 1;
        // Request four is the fresh retry after three empty replies.
        assert!(
            off.events
                .iter()
                .any(|event| event["stage"] == "fresh_session_retry"),
            "{id}"
        );
        let sent = &trace.requests[index];
        let base = &off.requests[index];
        // The retry still sends no earlier history: the base request plus
        // the notice as its own last message.
        assert_eq!(sent.len(), base.len() + 1, "{id}");
        assert_eq!(
            trace.request_messages(index)[..base.len()],
            off.request_messages(index)[..],
            "{id}"
        );
        // The base retry request is the system prompt and the user prompt.
        assert_eq!(base.len(), 2, "{id}");
        assert_eq!(sent.last().unwrap().role, "user", "{id}");
        assert!(
            sent.last().unwrap().content.starts_with(NOTICE_MARKER),
            "{id}"
        );
        assert_eq!(trace.phases(), ["notice", "final_accepted"], "{id}");
        assert!(trace.claimed(), "{id}");
    }
}

// Test 4: a final after the same check succeeded.

#[test]
fn a_final_after_the_same_check_succeeds_is_accepted() {
    let trace = assert_accepted("accept-after-same-check");
    let accepted = &trace.closing_events()[1];
    assert_eq!(accepted["resolved_failures"], 1);
    assert_eq!(accepted["final_deferrals"], 0);
}

// Test 5: finals that stay unclaimed.

#[test]
fn an_unrelated_successful_bash_does_not_resolve_the_failed_check() {
    let trace = assert_guarded("unrelated-bash-success");
    assert_eq!(trace.closing_events()[1]["unresolved_bash_failures"], 1);
}

#[test]
fn a_write_without_rerunning_the_check_does_not_resolve_it() {
    let trace = assert_guarded("write-without-rerun");
    assert_eq!(trace.closing_events()[1]["unresolved_bash_failures"], 1);
}

#[test]
fn a_write_to_another_path_does_not_resolve_a_failed_write() {
    let trace = assert_guarded("other-path-write");
    assert_eq!(trace.closing_events()[1]["unresolved_write_failures"], 1);
}

#[test]
fn a_failure_before_the_notice_still_blocks_finals_after_it() {
    let trace = assert_guarded("failure-before-notice");
    // The final in the notice reply is already guarded.
    assert_eq!(
        trace.closing_events()[1]["iteration"],
        notice_request(&row("failure-before-notice"))
    );
}

#[test]
fn a_run_without_writes_is_guarded_too() {
    assert_guarded("no-write-bash-failure");
}

#[test]
fn reads_and_self_reports_do_not_resolve_the_failed_check() {
    let trace = assert_guarded("read-and-self-report");
    assert!(
        !trace
            .events
            .iter()
            .any(|event| event["reason"] == "post_write_read_confirmation_completed")
    );
}

#[test]
fn an_edit_at_another_anchor_does_not_resolve_the_failed_edit() {
    let trace = assert_guarded("other-anchor-edit");
    assert_eq!(trace.closing_events()[1]["unresolved_write_failures"], 1);
}

#[test]
fn one_matching_success_resolves_only_its_own_failed_edit() {
    let trace = assert_guarded("two-failed-edits-one-resolved");
    let deferred = &trace.closing_events()[1];
    assert_eq!(deferred["unresolved_write_failures"], 1);
    assert_eq!(deferred["resolved_failures"], 1);
}

#[test]
fn the_matching_edit_success_resolves_the_failed_edit() {
    let trace = assert_accepted("matching-edit-resolves");
    assert_eq!(trace.closing_events()[1]["resolved_failures"], 1);
}

// Test 6: consecutive finals.

#[test]
fn consecutive_finals_are_all_deferred_until_the_budget_ends_the_run() {
    let row = row("consecutive-finals");
    let trace = assert_guarded("consecutive-finals");
    assert_eq!(
        trace.phases(),
        [
            "notice",
            "final_deferred",
            "final_deferred",
            "final_deferred",
            "exhausted_after_notice"
        ]
    );
    assert_eq!(trace.remaining_replies, 0);
    // At base the first of those finals was claimed.
    assert_eq!(row["base"]["outcome"]["stop_reason"], "AssistantFinal");
}

// Test 7: existing gates still stop after the notice.

#[test]
fn existing_gates_still_stop_finals_after_the_notice() {
    for (id, phases) in [
        (
            "gate-missing-required-path",
            &["notice", "exhausted_after_notice"][..],
        ),
        ("gate-completion-without-write", &["notice"][..]),
        (
            "gate-missing-import",
            &["notice", "exhausted_after_notice"][..],
        ),
    ] {
        let row = row(id);
        let trace = run_once(&row);
        assert_eq!(trace.outcome(), row["base"]["outcome"], "{id}");
        assert_eq!(trace.phases(), phases, "{id}");
        assert!(trace.requests_with(DEFERRAL_MARKER).is_empty(), "{id}");
    }
}

#[test]
fn a_read_only_run_is_accepted_without_a_write() {
    assert_accepted("read-only-accepted");
}

// Test 8: together with the completion obligation review.

fn review_view(trace: &Trace) -> Vec<Value> {
    trace
        .review_events()
        .iter()
        .map(|event| {
            json!({
                "phase": event["phase"],
                "reason": event["reason"],
                "iteration": event["iteration"],
                "remaining_iterations": event["remaining_iterations"],
                "triggered_iteration": event["triggered_iteration"],
            })
        })
        .collect()
}

#[test]
fn the_obligation_review_is_unchanged_when_both_are_on() {
    let early = row("with-review-early-final");
    let off = run(&early, None);
    let both = run(&early, Some("once"));
    assert_eq!(review_view(&both), review_view(&off));
    assert_eq!(review_view(&both).len(), 2);
    assert_eq!(review_view(&both)[0]["phase"], "triggered");
    // The run ends before the notice request.
    assert_eq!(both.base_summary(), early["base"]);
    let tail = row("with-review-tail-skip");
    let off = run(&tail, None);
    let both = run_once(&tail);
    assert_eq!(review_view(&both), review_view(&off));
    assert_eq!(review_view(&both).len(), 1);
    assert_eq!(review_view(&both)[0]["phase"], "skipped");
    assert_eq!(
        review_view(&both)[0]["reason"],
        "insufficient_remaining_iterations"
    );
    assert_eq!(both.phases(), ["notice", "final_accepted"]);
    assert_eq!(both.outcome(), tail["base"]["outcome"]);
    // The review event comes before the guard's decision on the same final.
    let review_at = both
        .events
        .iter()
        .position(|event| event["event"] == REVIEW_EVENT)
        .unwrap();
    let accepted_at = both
        .events
        .iter()
        .position(|event| event["phase"] == "final_accepted")
        .unwrap();
    assert!(review_at < accepted_at);
}

// Test 9: out of scope.

#[test]
fn other_profiles_and_contract_runs_behave_exactly_as_off() {
    for id in ["profile-not-generic", "completion-contract-run"] {
        let row = row(id);
        let trace = run(&row, Some("once"));
        assert_eq!(trace.base_summary(), row["base"], "{id}");
        assert!(trace.closing_events().is_empty(), "{id}");
        assert!(
            trace.requests.len() >= notice_request(&row),
            "{id}: the run reached the notice request"
        );
    }
}

#[test]
fn policy_rejected_bash_is_counted_but_not_a_failure() {
    let trace = assert_accepted("policy-rejected-bash");
    assert!(
        trace
            .events
            .iter()
            .any(|event| event["event"] == "tool_policy_error")
    );
    let accepted = &trace.closing_events()[1];
    assert_eq!(accepted["policy_rejected_bash"], 1);
    assert_eq!(accepted["resolved_failures"], 0);
}

// Review fixes.

#[test]
fn an_unchanged_write_does_not_resolve_a_failed_edit_contained_in_its_replacement() {
    let trace = assert_guarded("unchanged-write-after-ambiguous-edit");
    // The edit really failed as ambiguous and the write really succeeded.
    assert!(
        trace
            .events
            .iter()
            .any(|event| event["error_kind"] == "edit_ambiguous_anchor")
    );
    let deferred = &trace.closing_events()[1];
    assert_eq!(deferred["unresolved_write_failures"], 1);
    assert_eq!(deferred["resolved_failures"], 0);
}

#[test]
fn a_bash_refused_by_the_tool_layer_is_counted_not_guarded() {
    let trace = assert_accepted("hidden-path-bash-rejected");
    assert!(
        trace
            .events
            .iter()
            .any(|event| event["event"] == "hidden_path_feedback")
    );
    // The front policy branch did not see it; the tool layer refused it.
    assert!(
        !trace
            .events
            .iter()
            .any(|event| event["event"] == "tool_policy_error")
    );
    for event in &trace.closing_events() {
        assert_eq!(event["unresolved_bash_failures"], 0);
        assert_eq!(event["policy_rejected_bash"], 1);
    }
    // A Bash that ran and failed stays unresolved and is guarded.
    let failed = assert_guarded("failure-before-notice");
    for event in &failed.closing_events() {
        assert_eq!(event["unresolved_bash_failures"], 1);
        assert_eq!(event["policy_rejected_bash"], 0);
    }
}

#[test]
fn a_cancel_before_the_notice_request_reaches_the_client_records_no_notice() {
    let row = row("cancel-before-notice-request");
    let trace = run(&row, Some("once"));
    // The client saw only the two earlier requests.
    assert_eq!(trace.requests.len(), notice_request(&row) - 1);
    assert!(trace.closing_events().is_empty());
    assert_eq!(trace.base_summary(), row["base"]);
    assert!(
        trace
            .events
            .iter()
            .any(|event| event["event"] == "provider_turn_aborted_by_user")
    );
}

#[test]
fn an_interruption_after_the_notice_request_reached_the_client_records_the_notice() {
    let mut row = row("cancel-before-notice-request");
    row.as_object_mut().unwrap().remove("cancel_at_request");
    row["abort_on_notice"] = json!(true);
    let trace = run(&row, Some("once"));
    let notice = notice_request(&row);
    // The client received the notice request, then the call was interrupted.
    assert_eq!(trace.requests.len(), notice);
    assert_eq!(trace.requests_with(NOTICE_MARKER), vec![notice]);
    assert!(
        trace
            .events
            .iter()
            .any(|event| event["event"] == "provider_turn_aborted_by_user")
    );
    // The existing interruption error, the notice, and no terminal phase.
    assert_eq!(trace.outcome(), row["base"]["outcome"]);
    assert_eq!(trace.phases(), ["notice"]);
    assert_eq!(trace.closing_events()[0]["iteration"], notice);
    // Without the variable the same client is never interrupted.
    let off = run(&row, None);
    assert!(off.requests_with(NOTICE_MARKER).is_empty());
    assert!(off.closing_events().is_empty());
}

#[test]
fn a_post_write_read_completion_with_an_unresolved_failure_is_not_claimed() {
    let row = row("post-write-read-after-failed-write");
    // At base the second read batch ended the run as a success.
    assert_eq!(
        row["base"]["outcome"]["final_text"],
        "workspace changes confirmed by post-write reads (unverified)"
    );
    let trace = assert_guarded("post-write-read-after-failed-write");
    let events = trace.closing_events();
    assert_eq!(events[0]["unresolved_write_failures"], 1);
    assert_eq!(events[1]["phase"], "final_deferred");
    assert_eq!(events[1]["iteration"], notice_request(&row) + 1);
    assert!(
        !trace
            .events
            .iter()
            .any(|event| event["reason"] == "post_write_read_confirmation_completed")
    );
}

#[test]
fn a_post_write_read_completion_without_a_failure_still_completes() {
    let trace = assert_accepted("post-write-read-accepted");
    let accepted = &trace.closing_events()[1];
    assert_eq!(
        accepted["iteration"],
        notice_request(&row("post-write-read-accepted"))
    );
    assert!(
        trace
            .events
            .iter()
            .any(|event| event["reason"] == "post_write_read_confirmation_completed")
    );
}
