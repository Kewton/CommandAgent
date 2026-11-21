//! Issue #940: the experimental `COMMANDAGENT_COMPLETION_OBLIGATION_REVIEW`
//! opt-in. Scenarios live in the synthetic corpus fixture
//! `tests/corpus/apps/completion-obligation-review/fixtures/review.jsonl` and
//! run through the public minimal loop with a scripted mock `ChatClient`.
//!
//! The default-off rows pin the behavior of the source before the review
//! existed (a49e55c9): request digests, the event-name sequence, and the
//! outcome were recorded from that source, not from the new implementation.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use commandagent::config::{
    Action, Config, ConfigFieldSources, NarrationMode, OllamaThink, OpenAiApi, PlanPreset,
    PromptLayout, Provider,
};
use commandagent::minimal_loop::loop_run::{RunSessionOutcome, run_session_with_outcome_with_ui};
use commandagent::providers::{AssistantReply, ChatClient};
use commandagent::state::{ConversationMessage, SessionSnapshot, ToolCall};
use commandagent::tools::registry::ToolSpec;
use commandagent::tui::status::UiStatus;
use commandagent::tui::{InteractionUi, UiGuard};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const ENV: &str = "COMMANDAGENT_COMPLETION_OBLIGATION_REVIEW";
const LEGACY_ENV: &str = "ANVIL_COMPLETION_OBLIGATION_REVIEW";
const EVENT: &str = "completion_obligation_review";
const CORPUS: &str = "tests/corpus/apps/completion-obligation-review";
/// The opening sentence of the fixed review request; it marks the request
/// that carries the review without the test copying the whole prompt.
const REVIEW_MARKER: &str = "完了前に一度だけ確認してください。";
const EDIT_PROMPT: &str = "Write a short greeting file for the project and report what you did.";
const QUESTION_PROMPT: &str = "Explain what the project notes say.";

/// Every env mutation in this binary happens while this lock is held.
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn env_lock() -> MutexGuard<'static, ()> {
    ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn set_env(setting: Option<&str>) {
    // SAFETY: every env mutation in this test binary happens while holding
    // `ENV_LOCK`, and no test in this binary reads the environment without it.
    unsafe {
        std::env::remove_var(LEGACY_ENV);
        match setting {
            Some(value) => std::env::set_var(ENV, value),
            None => std::env::remove_var(ENV),
        }
    }
}

#[derive(Clone)]
struct ScriptedClient {
    replies: Arc<Mutex<Vec<AssistantReply>>>,
    requests: Arc<Mutex<Vec<Vec<ConversationMessage>>>>,
    /// Sets the review variable to this value during the first chat call.
    env_change_on_first_call: Option<&'static str>,
}

impl ScriptedClient {
    fn new(replies: Vec<AssistantReply>) -> Self {
        Self {
            replies: Arc::new(Mutex::new(replies)),
            requests: Arc::new(Mutex::new(Vec::new())),
            env_change_on_first_call: None,
        }
    }
}

impl ChatClient for ScriptedClient {
    fn label(&self) -> &str {
        "scripted"
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
        let mut requests = self.requests.lock().unwrap();
        if requests.is_empty()
            && let Some(value) = self.env_change_on_first_call
        {
            // The caller holds `ENV_LOCK` for the whole run.
            set_env(Some(value));
        }
        requests.push(messages.to_vec());
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

fn config(root: &Path, max_iterations: usize, profile: &str, contract: Option<PathBuf>) -> Config {
    Config {
        workspace_root: root.to_path_buf(),
        state_dir: root.join("state"),
        eval_events_path: Some(root.join("events.jsonl")),
        completion_contract_path: contract,
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
        max_iterations,
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
        profile: profile.to_string(),
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
    max_iterations: usize,
    profile: String,
    prompt: &'static str,
    contract: Option<Value>,
    seed: Vec<(String, String)>,
    script: Vec<Value>,
    cancel_at_request: Option<usize>,
    expect: Value,
}

impl Scenario {
    fn from_row(row: &Value) -> Self {
        assert_eq!(row["kind"], "synthetic", "{row}");
        let prompt = match row["prompt"].as_str().unwrap_or("edit") {
            "edit" => EDIT_PROMPT,
            "question" => QUESTION_PROMPT,
            other => panic!("unknown prompt id {other}"),
        };
        Self {
            id: row["id"].as_str().unwrap().to_string(),
            setting: row["setting"].as_str().map(str::to_string),
            max_iterations: row["max_iterations"].as_u64().unwrap() as usize,
            profile: row["profile"].as_str().unwrap_or("generic").to_string(),
            prompt,
            contract: row.get("contract").cloned(),
            seed: row["seed"]
                .as_array()
                .map(|files| {
                    files
                        .iter()
                        .map(|file| {
                            (
                                file[0].as_str().unwrap().to_string(),
                                file[1].as_str().unwrap().to_string(),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default(),
            script: row["script"].as_array().unwrap().clone(),
            cancel_at_request: row["cancel_at_request"]
                .as_u64()
                .map(|value| value as usize),
            expect: row["expect"].clone(),
        }
    }

    fn with_setting(&self, setting: Option<&str>) -> Self {
        let mut copy = self.clone();
        copy.setting = setting.map(str::to_string);
        copy
    }

    fn replies(&self) -> Vec<AssistantReply> {
        self.script
            .iter()
            .enumerate()
            .map(|(index, step)| {
                let id = format!("call-{index}");
                let call = |name: &str, arguments: Value| AssistantReply {
                    content: String::new(),
                    tool_calls: vec![ToolCall {
                        id: id.clone(),
                        name: name.to_string(),
                        arguments,
                    }],
                    prompt_tokens: None,
                    completion_tokens: None,
                };
                if let Some(write) = step.get("write") {
                    call("Write", json!({"path": write[0], "content": write[1]}))
                } else if let Some(edit) = step.get("edit") {
                    call(
                        "Edit",
                        json!({"path": edit[0], "old_string": edit[1], "new_string": edit[2]}),
                    )
                } else if let Some(command) = step.get("bash") {
                    call("Bash", json!({"command": command}))
                } else if let Some(arguments) = step.get("bash_arguments") {
                    call("Bash", arguments.clone())
                } else if let Some(text) = step.get("final") {
                    AssistantReply {
                        content: text.as_str().unwrap().to_string(),
                        tool_calls: Vec::new(),
                        prompt_tokens: None,
                        completion_tokens: None,
                    }
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
        set_env(self.setting.as_deref());
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
        let mut client = adjust(ScriptedClient::new(self.replies()));
        let mut session = SessionSnapshot::new();
        let config = config(dir.path(), self.max_iterations, &self.profile, contract);
        let ui = CancelAtModelCall {
            calls: Cell::new(0),
            cancel_at: self.cancel_at_request.unwrap_or(usize::MAX),
        };
        let result = run_session_with_outcome_with_ui(
            &mut client,
            &mut session,
            self.prompt,
            &[],
            &config,
            &ui,
        )
        .map_err(|err| format!("{err:#}"));
        set_env(None);
        let requests = client.requests.lock().unwrap().clone();
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

    fn request_digests(&self) -> Vec<String> {
        self.requests
            .iter()
            .map(|messages| {
                let text = self.normalize(&serde_json::to_string(messages).unwrap());
                let digest = Sha256::digest(text.as_bytes());
                digest
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

    fn event_names(&self, include_review: bool) -> Vec<String> {
        self.events
            .iter()
            .filter(|event| include_review || event["event"] != EVENT)
            .map(|event| event["event"].as_str().unwrap_or_default().to_string())
            .collect()
    }

    fn review_events(&self) -> Vec<Value> {
        self.events
            .iter()
            .filter(|event| event["event"] == EVENT)
            .cloned()
            .collect()
    }

    /// Indexes of the requests whose messages carry the review request.
    fn review_request_indexes(&self) -> Vec<usize> {
        self.requests
            .iter()
            .enumerate()
            .filter(|(_, messages)| {
                messages
                    .iter()
                    .any(|message| message.content.contains(REVIEW_MARKER))
            })
            .map(|(index, _)| index)
            .collect()
    }

    fn summary(&self) -> Value {
        json!({
            "outcome": self.outcome(),
            "requests": self.requests.len(),
            "remaining_replies": self.remaining_replies,
            "review_request_indexes": self.review_request_indexes(),
            "event_names": self.event_names(true),
            "request_digests": self.request_digests(),
            "review_events": self.review_events(),
        })
    }

    /// Compares every key the fixture row sets.
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

    /// Requests, outcome, and every non-review event are what the same
    /// script produces with the review unset.
    fn assert_equivalent_to(&self, off: &Trace) {
        assert_eq!(self.request_digests(), off.request_digests(), "{}", self.id);
        assert_eq!(self.outcome(), off.outcome(), "{}", self.id);
        assert_eq!(
            self.event_names(false),
            off.event_names(true),
            "{}",
            self.id
        );
        assert!(off.review_events().is_empty(), "{}", self.id);
        assert!(self.review_request_indexes().is_empty(), "{}", self.id);
    }
}

fn corpus_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(CORPUS)
        .join(name)
}

fn scenarios() -> Vec<Scenario> {
    std::fs::read_to_string(corpus_path("fixtures/review.jsonl"))
        .unwrap()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| Scenario::from_row(&serde_json::from_str::<Value>(line).unwrap()))
        .collect()
}

fn scenario(id: &str) -> Scenario {
    scenarios()
        .into_iter()
        .find(|scenario| scenario.id == id)
        .unwrap_or_else(|| panic!("fixture row {id} missing"))
}

fn run_fixture_row(id: &str) -> Trace {
    let scenario = scenario(id);
    let trace = scenario.run();
    trace.assert_expectations(&scenario.expect);
    if scenario.expect["equivalent_to_off"] == true {
        trace.assert_equivalent_to(&scenario.with_setting(None).run());
    }
    trace
}

const FIXED_KEYS: &[&str] = &[
    "event",
    "schema_version",
    "phase",
    "reason",
    "prompt_version",
    "iteration",
    "remaining_iterations",
    "min_remaining_iterations",
    "triggered_iteration",
    "edits_after_prompt",
    "bash_results_after_prompt",
    "successful_bash_after_prompt",
    "unsuccessful_bash_after_prompt",
    "successful_bash_since_latest_edit",
];

const FIXED_STRINGS: &[&str] = &[
    EVENT,
    "1",
    "triggered",
    "closed",
    "skipped",
    "normal_final_after_successful_edit",
    "next_final",
    "scope_exit_before_next_final",
    "plan_run_step",
    "profile_not_generic",
    "completion_contract",
    "no_successful_edit",
    "insufficient_remaining_iterations",
];

/// A review event carries only known keys, numbers, nulls, and fixed words.
fn assert_fixed_shape(event: &Value) {
    for (key, value) in event.as_object().unwrap() {
        assert!(
            FIXED_KEYS.contains(&key.as_str()),
            "unexpected key {key}: {event}"
        );
        match value {
            Value::Null | Value::Number(_) => {}
            Value::String(text) => assert!(
                FIXED_STRINGS.contains(&text.as_str()),
                "free text {text:?} in {event}"
            ),
            other => panic!("unexpected value {other} under {key}: {event}"),
        }
    }
}

#[test]
fn default_off_rows_keep_the_recorded_pre_review_behavior() {
    for id in [
        "default-off-unset",
        "default-off-empty",
        "default-off-explicit",
    ] {
        let trace = run_fixture_row(id);
        assert!(trace.review_events().is_empty(), "{id}");
        assert!(trace.review_request_indexes().is_empty(), "{id}");
    }
}

#[test]
fn default_off_rows_with_a_longer_script_stop_at_the_first_final() {
    for id in ["default-off-unset-long", "default-off-explicit-long"] {
        let trace = run_fixture_row(id);
        assert!(trace.review_events().is_empty(), "{id}");
    }
}

#[test]
fn once_defers_the_first_normal_final_and_returns_the_next_one() {
    let trace = run_fixture_row("once-triggered-executed-final");
    let outcome = trace.result.as_ref().unwrap();
    // The pre-review final is never returned for the changed workspace.
    assert_ne!(outcome.final_text, "greeting.txt を作成しました。");
    for event in trace.review_events() {
        assert_fixed_shape(&event);
    }
}

#[test]
fn self_reported_review_is_not_a_release_condition_and_never_asks_twice() {
    for id in ["once-self-report-final", "once-plain-second-final"] {
        let trace = run_fixture_row(id);
        assert_eq!(trace.review_request_indexes(), vec![2], "{id}");
        let phases = trace
            .review_events()
            .iter()
            .map(|event| event["phase"].as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(phases, ["triggered", "closed"], "{id}");
    }
}

#[test]
fn a_later_edit_drops_older_successful_bash_observations() {
    run_fixture_row("once-reedit-invalidates");
}

#[test]
fn failed_tool_errored_and_policy_rejected_bash_is_not_a_success_observation() {
    let trace = run_fixture_row("once-unsuccessful-observations");
    // The three Bash calls really were a non-zero exit, a tool error, and a
    // policy rejection that never ran.
    let tool_result = |index: usize| {
        trace.requests[index]
            .iter()
            .rev()
            .find(|message| message.role == "tool")
            .map(|message| message.content.clone())
            .unwrap_or_default()
    };
    assert!(
        tool_result(3).starts_with("outcome: CommandFailed"),
        "{}",
        tool_result(3)
    );
    let named = |name: &str| {
        trace
            .events
            .iter()
            .filter(|event| event["event"] == name)
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(
        named("tool_validation_error").len(),
        2,
        "{:#?}",
        trace.events
    );
    let policy = named("tool_policy_error");
    assert_eq!(policy.len(), 1, "{:#?}", trace.events);
    assert_eq!(
        policy[0]["policy_error_kind"],
        "bash_path_confinement_error"
    );
    let closed = &trace.review_events()[1];
    assert_eq!(closed["bash_results_after_prompt"], 2);
    assert_eq!(closed["successful_bash_after_prompt"], 0);
    assert_eq!(closed["successful_bash_since_latest_edit"], 0);
}

#[test]
fn remaining_iteration_boundary_is_three() {
    run_fixture_row("once-remaining-three-fires");
    run_fixture_row("once-remaining-two-skips");
}

#[test]
fn excluded_runs_behave_exactly_as_with_the_review_off() {
    for id in [
        "once-read-only-run",
        "once-profile-not-generic",
        "once-completion-contract",
    ] {
        let trace = run_fixture_row(id);
        assert!(
            trace
                .review_events()
                .iter()
                .all(|event| event["phase"] == "skipped"),
            "{id}"
        );
    }
}

#[test]
fn existing_completion_blocker_runs_before_the_review() {
    let trace = run_fixture_row("once-existing-blocker-first");
    // The blocker's feedback was answered before the review was requested.
    let blocker_request = trace
        .requests
        .iter()
        .position(|messages| {
            messages
                .iter()
                .any(|message| message.content.contains("helper.js"))
                && !messages
                    .iter()
                    .any(|message| message.content.contains(REVIEW_MARKER))
        })
        .unwrap();
    assert!(blocker_request < trace.review_request_indexes()[0]);
}

#[test]
fn cancel_and_budget_exhaustion_after_the_request_keep_their_failure() {
    for id in [
        "once-cancel-after-request",
        "once-budget-exhausted-after-request",
    ] {
        let trace = run_fixture_row(id);
        assert!(trace.result.is_err(), "{id}: a failure stays a failure");
        let closed = trace.review_events();
        assert_eq!(closed.len(), 2, "{id}");
        assert_eq!(closed[1]["reason"], "scope_exit_before_next_final", "{id}");
    }
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
        "sk-test-940-NOT-A-REAL-KEY",
    ] {
        let trace = base.with_setting(Some(value)).run();
        let error = trace.result.as_ref().unwrap_err();
        // The same fixed text for every value: nothing of the value is echoed.
        assert_eq!(
            error,
            &format!("invalid {ENV}: expected unset, empty, `off`, or `once`"),
            "{value}"
        );
        assert!(trace.requests.is_empty(), "{value}");
        assert!(trace.events.is_empty(), "{value}: {:?}", trace.events);
    }
    let secret = "sk-test-940-NOT-A-REAL-KEY";
    let trace = base.with_setting(Some(secret)).run();
    assert!(!trace.result.as_ref().unwrap_err().contains(secret));
}

#[test]
fn the_variable_is_read_once_at_run_start() {
    let base = scenario("once-triggered-executed-final");
    for change in ["off", "not-a-mode"] {
        let mut client_change = Some(change);
        let trace = base.run_with(|mut client| {
            client.env_change_on_first_call = client_change.take();
            client
        });
        trace.assert_expectations(&base.expect);
    }
    let off = scenario("default-off-unset-long");
    let trace = off.run_with(|mut client| {
        client.env_change_on_first_call = Some("once");
        client
    });
    trace.assert_expectations(&off.expect);
}

#[test]
fn review_events_copy_no_prompt_command_model_text_output_or_secret() {
    const SECRET: &str = "sk-test-940-NOT-A-REAL-KEY-7f3a";
    let mut scenario = scenario("once-reedit-invalidates");
    scenario.script = vec![
        json!({"write": ["greeting.txt", format!("hello {SECRET}\n")]}),
        json!({"final": format!("Wrote the greeting with {SECRET}.")}),
        json!({"bash": format!("grep -q {SECRET} greeting.txt && echo found-{SECRET}")}),
        json!({"edit": ["greeting.txt", "hello", "hello world"]}),
        json!({"final": format!("Checked {SECRET}; covered everything.")}),
    ];
    let trace = scenario.run();
    let review = trace.review_events();
    assert_eq!(review.len(), 2);
    for event in &review {
        assert_fixed_shape(event);
        let text = event.to_string();
        for forbidden in [SECRET, "grep", "greeting", "hello", "found", REVIEW_MARKER] {
            assert!(!text.contains(forbidden), "{forbidden} in {text}");
        }
        assert!(event.get("coverage_passed").is_none());
        assert!(event.get("verification").is_none());
        assert!(event.get("assurance_level").is_none());
    }
    // No other event gained an acceptance or verification claim either.
    let off = scenario.with_setting(None).run();
    let claim_keys = |trace: &Trace| {
        trace
            .events
            .iter()
            .filter(|event| event["event"] != EVENT)
            .filter(|event| {
                event.get("assurance_level").is_some()
                    || event.get("acceptance_passed").is_some()
                    || event.get("verification") == Some(&json!("passed"))
            })
            .count()
    };
    assert_eq!(claim_keys(&trace), claim_keys(&off));
}

#[test]
fn corpus_rows_are_synthetic_and_cover_every_declared_case() {
    let ids = scenarios()
        .into_iter()
        .map(|scenario| scenario.id)
        .collect::<Vec<_>>();
    for required in [
        "default-off-unset",
        "default-off-empty",
        "default-off-explicit",
        "once-triggered-executed-final",
        "once-self-report-final",
        "once-cancel-after-request",
    ] {
        assert!(ids.iter().any(|id| id == required), "{required}");
    }
    let readme = std::fs::read_to_string(corpus_path("README.md")).unwrap();
    assert!(readme.contains("synthetic"));
}

/// A synthetic credential name the dotenv collector treats as a credential.
const SYNTHETIC_CREDENTIAL: &str = "SYNTHETIC_940_API_KEY";

/// The phase, reason, and field words the review emits, all reserved as fixed
/// identifiers so the shared secret scrub can never rewrite them.
const REVIEW_FIXED_WORDS: &[&str] = &[
    "triggered",
    "closed",
    "skipped",
    "normal_final_after_successful_edit",
    "next_final",
    "scope_exit_before_next_final",
    "plan_run_step",
    "profile_not_generic",
    "completion_contract",
    "no_successful_edit",
    "insufficient_remaining_iterations",
    "prompt_version",
    "iteration",
    "remaining_iterations",
    "min_remaining_iterations",
    "triggered_iteration",
    "edits_after_prompt",
    "bash_results_after_prompt",
    "successful_bash_after_prompt",
    "unsuccessful_bash_after_prompt",
    "successful_bash_since_latest_edit",
];

fn with_credential(scenario: &Scenario, value: &str) -> Scenario {
    let mut copy = scenario.clone();
    copy.seed.push((
        ".env".to_string(),
        format!("{SYNTHETIC_CREDENTIAL}={value}\n"),
    ));
    copy
}

#[test]
fn credentials_equal_to_or_inside_a_review_word_are_refused_without_the_value() {
    use commandagent::sensitive_data::{RegistrationRefusal, ReservedKind, SecretCatalog};

    // Fragments of the reserved words (at least the minimum secret length).
    let fragments = [
        "_after_prompt",
        "remaining_iter",
        "after_successful",
        "ope_exit",
    ];
    for value in REVIEW_FIXED_WORDS.iter().chain(fragments.iter()) {
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

    // Through the public minimal-loop entry: the run refuses before any
    // request and the error never names the value.
    let base = scenario("once-triggered-executed-final");
    for value in [
        "triggered",
        "next_final",
        "bash_results_after_prompt",
        "triggered_iteration",
        "_after_prompt",
    ] {
        let trace = with_credential(&base, value).run();
        let error = trace.result.as_ref().unwrap_err();
        assert!(error.contains(SYNTHETIC_CREDENTIAL), "{value}: {error}");
        assert!(!error.contains(value), "{value}: {error}");
        assert!(trace.requests.is_empty(), "{value}");
        assert!(trace.review_events().is_empty(), "{value}");
    }
}

#[test]
fn a_registered_credential_keeps_every_review_phase_reason_and_field() {
    const SECRET: &str = "sk-test-940-NOT-A-REAL-KEY-c0ffee";
    let mut base = scenario("once-reedit-invalidates");
    base.script = vec![
        json!({"write": ["greeting.txt", "hello\n"]}),
        json!({"final": "Wrote the greeting."}),
        json!({"bash": format!("echo {SECRET}")}),
        json!({"edit": ["greeting.txt", "hello", "hello world"]}),
        json!({"final": "Checked; covered everything."}),
    ];
    let plain = base.run();
    let scrubbed = with_credential(&base, SECRET).run();
    assert!(scrubbed.result.is_ok(), "{:?}", scrubbed.result);

    // The credential is registered for the run's event log: the same emit
    // path scrubs it there and keeps it in the run without the credential.
    let probe = |trace: &Trace| {
        let path = trace.root.join("events.jsonl");
        commandagent::eval_events::emit(Some(&path), json!({"event": EVENT, "probe": SECRET}));
        std::fs::read_to_string(path).unwrap().contains(SECRET)
    };
    assert!(probe(&plain));
    assert!(!probe(&scrubbed));

    let review = scrubbed.review_events();
    assert_eq!(review, plain.review_events());
    assert_eq!(review.len(), 2, "{review:?}");
    assert_eq!(review[0]["phase"], "triggered");
    assert_eq!(review[0]["reason"], "normal_final_after_successful_edit");
    assert_eq!(review[1]["phase"], "closed");
    assert_eq!(review[1]["reason"], "next_final");
    for event in &review {
        assert_fixed_shape(event);
        assert!(!event.to_string().contains("<redacted>"), "{event}");
    }
    for key in [
        "prompt_version",
        "iteration",
        "remaining_iterations",
        "min_remaining_iterations",
    ] {
        assert!(review[0].get(key).is_some(), "{key}: {}", review[0]);
    }
    for key in [
        "prompt_version",
        "triggered_iteration",
        "iteration",
        "remaining_iterations",
        "edits_after_prompt",
        "bash_results_after_prompt",
        "successful_bash_after_prompt",
        "unsuccessful_bash_after_prompt",
        "successful_bash_since_latest_edit",
    ] {
        assert!(review[1].get(key).is_some(), "{key}: {}", review[1]);
    }
    assert_eq!(review[1]["bash_results_after_prompt"], 1);
}
