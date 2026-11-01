//! Issue #958: unit tests of the output-limit feedback leaf, and runs of the
//! minimal loop with a scripted client for the cases the public entry cannot
//! configure (a non-default max-length guard limit, XML fallback requests,
//! an exhausted malformed retry without XML fallback).

use std::borrow::Cow;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::super::{RunSessionOptions, run_session_with_outcome_with_options};
use super::{OutputLimitFeedback, lock};
use crate::config::{
    Action, Config, ConfigFieldSources, NarrationMode, OllamaThink, OpenAiApi, PlanPreset,
    PromptLayout, Provider,
};
use crate::provider_call::{ProviderCallOutcome, ProviderCallScope, ProviderChatRequest};
use crate::providers::streaming::CancelToken;
use crate::providers::{
    AssistantReply, ChatClient, ProviderResponseMetadata, ResponseTiming, ToolArgumentsParseError,
    ToolArgumentsParseFailure,
};
use crate::state::{ConversationMessage, SessionSnapshot, ToolCall};
use crate::tools::registry::ToolSpec;
use crate::tui::NOOP_UI;

const LIMIT: usize = 100;
const NO_TOOL_SENTENCE: &str = "Your previous response reached the output limit (100 tokens) before any tool call arrived. Keep your reasoning short, start your next response with one tool call, and write a large file in smaller parts.";
const TRUNCATED_SENTENCE: &str = "Your previous response reached the output limit (100 tokens) and its tool call arguments were cut off. Keep your reasoning short, start your next response with one tool call, and write a large file in smaller parts.";
const EOF_MESSAGE: &str = "OpenAI chat tool_call arguments are not valid JSON: EOF while parsing a string at line 1 column 30";

fn reply(completion: Option<u64>, tool_calls: Vec<ToolCall>) -> AssistantReply {
    AssistantReply {
        content: "thinking".to_string(),
        tool_calls,
        prompt_tokens: Some(10),
        completion_tokens: completion,
    }
}

fn typed(completion: Option<u64>, failure: ToolArgumentsParseFailure) -> anyhow::Error {
    anyhow::Error::new(ToolArgumentsParseError::for_test(
        EOF_MESSAGE,
        completion,
        failure,
    ))
}

fn composed(leaf: &mut OutputLimitFeedback, feedback: Option<&str>) -> Option<String> {
    leaf.compose(feedback.map(Cow::Borrowed))
        .map(Cow::into_owned)
}

fn events(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn feedback_events(path: &Path) -> Vec<Value> {
    events(path)
        .into_iter()
        .filter(|event| event["event"] == "output_limit_feedback")
        .collect()
}

#[derive(Clone)]
enum Step {
    Reply(AssistantReply, Option<u64>),
    Typed(Option<u64>, ToolArgumentsParseFailure),
}

/// A scripted client: it records every request it receives and answers
/// from a shared script; its metadata reports the scripted reasoning count.
#[derive(Clone)]
struct Scripted {
    native: bool,
    xml_fallback: bool,
    steps: Arc<Mutex<Vec<Step>>>,
    requests: Arc<Mutex<Vec<Vec<ConversationMessage>>>>,
    metadata: Option<ProviderResponseMetadata>,
}

impl Scripted {
    fn new(steps: Vec<Step>) -> Self {
        Self {
            native: true,
            xml_fallback: false,
            steps: Arc::new(Mutex::new(steps)),
            requests: Arc::default(),
            metadata: None,
        }
    }

    fn requests(&self) -> Vec<Vec<ConversationMessage>> {
        self.requests.lock().unwrap().clone()
    }

    fn unused(&self) -> usize {
        self.steps.lock().unwrap().len()
    }
}

impl ChatClient for Scripted {
    fn label(&self) -> &str {
        "scripted"
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn supports_native_tools(&self, _model: &str) -> bool {
        self.native
    }

    fn allows_xml_fallback(&self) -> bool {
        self.xml_fallback
    }

    fn take_response_metadata(&mut self) -> Option<ProviderResponseMetadata> {
        self.metadata.take()
    }

    fn chat(
        &mut self,
        _model: &str,
        messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        self.requests.lock().unwrap().push(messages.to_vec());
        let mut steps = self.steps.lock().unwrap();
        if steps.is_empty() {
            anyhow::bail!("scripted client exhausted");
        }
        match steps.remove(0) {
            Step::Reply(reply, reasoning) => {
                self.metadata = Some(ProviderResponseMetadata {
                    reasoning_tokens: reasoning,
                    ..ProviderResponseMetadata::default()
                });
                Ok(reply)
            }
            Step::Typed(completion, failure) => Err(typed(completion, failure)),
        }
    }
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
        num_predict: LIMIT,
        max_iterations: 6,
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
        profile_explicit: true,
        profile_inference: None,
        style: "default".to_string(),
        action: Action::Repl,
    }
}

struct Run {
    _dir: tempfile::TempDir,
    events_path: std::path::PathBuf,
    result: Result<String, String>,
    requests: Vec<Vec<ConversationMessage>>,
    unused: usize,
}

impl Run {
    /// How many times the sentence appears in request `index` (0-based).
    fn sentences_in(&self, index: usize, sentence: &str) -> usize {
        self.requests[index]
            .iter()
            .map(|message| message.content.matches(sentence).count())
            .sum()
    }

    fn feedback(&self) -> Vec<Value> {
        feedback_events(&self.events_path)
    }
}

fn run(
    client: Scripted,
    required: &[&str],
    options: RunSessionOptions,
    adjust: impl FnOnce(&mut Config),
) -> Run {
    let dir = tempfile::tempdir().unwrap();
    let mut config = config(dir.path());
    adjust(&mut config);
    let mut client = client;
    let required: Vec<String> = required.iter().map(|path| path.to_string()).collect();
    let result = run_session_with_outcome_with_options(
        &mut client,
        &mut SessionSnapshot::new(),
        "Write the notes file.",
        &required,
        &config,
        &NOOP_UI,
        options,
    )
    .map(|outcome| format!("{:?}", outcome.stop_reason))
    .map_err(|error| format!("{error:#}"));
    Run {
        events_path: dir.path().join("events.jsonl"),
        _dir: dir,
        result,
        requests: client.requests(),
        unused: client.unused(),
    }
}

fn limit_text(reasoning: Option<u64>) -> Step {
    Step::Reply(reply(Some(LIMIT as u64), Vec::new()), reasoning)
}

// ---- the leaf alone -------------------------------------------------------

/// The leaf is private to the loop, so the integration tests write the
/// sentence out themselves. This pins the leaf's text and those copies to
/// each other: changing either side alone fails here.
#[test]
fn the_sentence_matches_the_copies_in_the_integration_tests() {
    let observation = |kind| super::Observation {
        kind,
        num_predict: LIMIT,
        completion_tokens: 100,
        reasoning_tokens: None,
    };
    assert_eq!(
        super::guidance(&observation(super::Kind::NoToolCall)),
        NO_TOOL_SENTENCE
    );
    assert_eq!(
        super::guidance(&observation(super::Kind::ToolArgumentsTruncated)),
        TRUNCATED_SENTENCE
    );
    // `tests/budget_exhaustion_observation.rs` keeps the no-tool sentence at
    // its own limit of 100 as one exact literal.
    let budget = include_str!("../../../../tests/budget_exhaustion_observation.rs");
    assert!(budget.contains(&format!("\"{NO_TOOL_SENTENCE}\"")));
    // `tests/output_limit_feedback.rs` builds it from the limit and one
    // clause per kind around the same fixed text.
    let public = include_str!("../../../../tests/output_limit_feedback.rs");
    for clause in [
        "\"before any tool call arrived\"",
        "\"and its tool call arguments were cut off\"",
        "\"Your previous response reached the output limit ({num_predict} tokens) {what}. Keep your reasoning short, start your next response with one tool call, and write a large file in smaller parts.\"",
    ] {
        assert!(public.contains(clause), "{clause}");
    }
}

#[test]
fn a_tool_less_reply_at_the_limit_is_appended_once_after_the_existing_feedback() {
    let mut leaf = OutputLimitFeedback::new(None);
    assert_eq!(
        composed(&mut leaf, Some("existing")),
        Some("existing".into())
    );
    leaf.observe_reply(&reply(Some(100), Vec::new()), LIMIT);
    assert_eq!(
        composed(&mut leaf, Some("existing")),
        Some(format!("existing\n\n{NO_TOOL_SENTENCE}"))
    );
    assert_eq!(
        composed(&mut leaf, Some("existing")),
        Some("existing".into()),
        "one observation is composed into one request only"
    );
    leaf.observe_reply(&reply(Some(250), Vec::new()), LIMIT);
    assert_eq!(composed(&mut leaf, None), Some(NO_TOOL_SENTENCE.into()));
    assert_eq!(composed(&mut leaf, None), None);
}

#[test]
fn below_the_limit_with_tool_calls_or_without_a_limit_nothing_is_armed() {
    let call = ToolCall::new("Read", json!({"path": "notes.md"}));
    for (completion, tools, limit) in [
        (Some(99), Vec::new(), LIMIT),
        (None, Vec::new(), LIMIT),
        (Some(100), vec![call.clone()], LIMIT),
        (Some(5000), vec![call], LIMIT),
        (Some(100), Vec::new(), 0),
        (Some(0), Vec::new(), 0),
    ] {
        let mut leaf = OutputLimitFeedback::new(None);
        leaf.observe_reply(&reply(completion, tools), limit);
        assert_eq!(
            composed(&mut leaf, Some("existing")),
            Some("existing".into()),
            "{completion:?} {limit}"
        );
    }
}

#[test]
fn only_the_typed_end_of_input_context_at_the_limit_arms_the_truncation_sentence() {
    let mut leaf = OutputLimitFeedback::new(None);
    leaf.observe_malformed(
        &typed(Some(100), ToolArgumentsParseFailure::EndOfInput),
        LIMIT,
    );
    assert_eq!(
        composed(&mut leaf, Some("malformed")),
        Some(format!("malformed\n\n{TRUNCATED_SENTENCE}"))
    );
    // A typed context whose message names no end of input still arms: the
    // classification is read from the type, never from the text.
    let quiet = anyhow::Error::new(ToolArgumentsParseError::for_test(
        "tool_call arguments rejected",
        Some(140),
        ToolArgumentsParseFailure::EndOfInput,
    ));
    leaf.observe_malformed(&quiet, LIMIT);
    assert!(composed(&mut leaf, None).is_some());
    // The context survives an outer context layer.
    leaf.observe_malformed(
        &typed(Some(100), ToolArgumentsParseFailure::EndOfInput).context("provider call"),
        LIMIT,
    );
    assert!(composed(&mut leaf, None).is_some());
    for (error, limit) in [
        (
            typed(Some(99), ToolArgumentsParseFailure::EndOfInput),
            LIMIT,
        ),
        (typed(None, ToolArgumentsParseFailure::EndOfInput), LIMIT),
        (typed(Some(100), ToolArgumentsParseFailure::Other), LIMIT),
        (typed(Some(100), ToolArgumentsParseFailure::EndOfInput), 0),
        // The same text without the typed context: nothing is inferred.
        (
            anyhow::anyhow!("{EOF_MESSAGE} (completion_tokens=100)"),
            LIMIT,
        ),
    ] {
        leaf.observe_malformed(&error, limit);
        assert_eq!(
            composed(&mut leaf, Some("malformed")),
            Some("malformed".into())
        );
    }
}

/// A client stand-in for the watch: it reports the given reasoning count.
#[derive(Clone)]
struct Metadata(Option<u64>);

impl ChatClient for Metadata {
    fn label(&self) -> &str {
        "metadata"
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn take_response_metadata(&mut self) -> Option<ProviderResponseMetadata> {
        Some(ProviderResponseMetadata {
            reasoning_tokens: self.0,
            ..ProviderResponseMetadata::default()
        })
    }

    fn chat(
        &mut self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        Ok(reply(Some(1), Vec::new()))
    }
}

/// One provider call the way `provider_call` makes it: through a clone.
fn call(leaf: &mut OutputLimitFeedback, client: &mut Metadata, dispatch: bool) {
    let watch = leaf.watch(client);
    let mut worker = watch.boxed_clone();
    if dispatch {
        ChatClient::chat(worker.as_mut(), "m", &[], &[], true).unwrap();
        let _ = worker.take_response_metadata();
    }
    drop(worker);
    drop(watch);
    leaf.settle();
}

#[test]
fn the_event_is_written_only_for_a_request_that_reached_the_client() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("events.jsonl");
    let mut leaf = OutputLimitFeedback::new(Some(&path));
    let mut client = Metadata(Some(40));
    call(&mut leaf, &mut client, true);
    leaf.observe_reply(&reply(Some(100), Vec::new()), LIMIT);
    assert!(composed(&mut leaf, None).is_some());
    call(&mut leaf, &mut client, false);
    assert!(feedback_events(&path).is_empty(), "nothing was sent");
    assert_eq!(
        composed(&mut leaf, None),
        Some(NO_TOOL_SENTENCE.into()),
        "an unsent sentence is armed again"
    );
    call(&mut leaf, &mut client, true);
    assert_eq!(composed(&mut leaf, None), None);
    call(&mut leaf, &mut client, true);
    let written = feedback_events(&path);
    assert_eq!(written.len(), 1, "{written:?}");
    assert_eq!(
        written[0],
        json!({
            "event": "output_limit_feedback",
            "kind": "no_tool_call_at_output_limit",
            "attempt": 1,
            "num_predict": 100,
            "completion_tokens": 100,
            "prompt_version": 1,
            "provider_reasoning_tokens": 40,
            "schema_version": "1",
        })
    );
}

#[test]
fn attempts_count_sent_requests_per_kind() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("events.jsonl");
    let mut leaf = OutputLimitFeedback::new(Some(&path));
    let mut client = Metadata(None);
    for observe_truncation in [false, true, false] {
        call(&mut leaf, &mut client, true);
        if observe_truncation {
            leaf.observe_malformed(
                &typed(Some(130), ToolArgumentsParseFailure::EndOfInput),
                LIMIT,
            );
        } else {
            leaf.observe_reply(&reply(Some(120), Vec::new()), LIMIT);
        }
        assert!(composed(&mut leaf, Some("feedback")).is_some());
    }
    call(&mut leaf, &mut client, true);
    let written: Vec<(String, u64, u64)> = feedback_events(&path)
        .iter()
        .map(|event| {
            assert!(event.get("provider_reasoning_tokens").is_none(), "{event}");
            (
                event["kind"].as_str().unwrap().to_string(),
                event["attempt"].as_u64().unwrap(),
                event["completion_tokens"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        written,
        vec![
            ("no_tool_call_at_output_limit".to_string(), 1, 120),
            (
                "tool_arguments_truncated_at_output_limit".to_string(),
                1,
                130
            ),
            ("no_tool_call_at_output_limit".to_string(), 2, 120),
        ]
    );
}

#[test]
fn the_reasoning_count_comes_from_the_turn_that_hit_the_limit_only() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("events.jsonl");
    let mut leaf = OutputLimitFeedback::new(Some(&path));
    // An earlier turn reported a count; the limit turn reported none.
    call(&mut leaf, &mut Metadata(Some(77)), true);
    call(&mut leaf, &mut Metadata(None), true);
    leaf.observe_reply(&reply(Some(100), Vec::new()), LIMIT);
    assert!(composed(&mut leaf, None).is_some());
    // The request carrying the sentence reports its own count afterwards.
    call(&mut leaf, &mut Metadata(Some(55)), true);
    // An explicit zero is a value.
    leaf.observe_reply(&reply(Some(100), Vec::new()), LIMIT);
    let mut zero = Metadata(Some(0));
    call(&mut leaf, &mut zero, true);
    leaf.observe_reply(&reply(Some(100), Vec::new()), LIMIT);
    assert!(composed(&mut leaf, None).is_some());
    call(&mut leaf, &mut Metadata(Some(9)), true);
    let written = feedback_events(&path);
    assert_eq!(written.len(), 2, "{written:?}");
    assert!(written[0].get("provider_reasoning_tokens").is_none());
    assert_eq!(written[1]["provider_reasoning_tokens"], 0);
    for event in written {
        let keys: Vec<&str> = event
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        for key in keys {
            assert!(
                [
                    "event",
                    "kind",
                    "attempt",
                    "num_predict",
                    "completion_tokens",
                    "prompt_version",
                    "provider_reasoning_tokens",
                    "schema_version",
                ]
                .contains(&key),
                "{key}"
            );
        }
    }
}

// ---- the dispatch state against a late provider-call worker ----------------

/// Holds a provider-call worker right before the dispatch decision (the
/// watch's, or a bare [`Probe`]'s), and reports the decision.
#[derive(Debug)]
pub(super) struct DispatchGate {
    reached: AtomicBool,
    released: (Mutex<bool>, Condvar),
    decided: Mutex<mpsc::Sender<bool>>,
}

impl DispatchGate {
    fn new() -> (Arc<Self>, mpsc::Receiver<bool>) {
        let (sender, receiver) = mpsc::channel();
        let gate = Self {
            reached: AtomicBool::new(false),
            released: (Mutex::new(false), Condvar::new()),
            decided: Mutex::new(sender),
        };
        (Arc::new(gate), receiver)
    }

    pub(super) fn hold(&self) {
        self.reached.store(true, Ordering::SeqCst);
        let (lock, condvar) = &self.released;
        let mut released = lock.lock().unwrap();
        while !*released {
            released = condvar.wait(released).unwrap();
        }
    }

    pub(super) fn decided(&self, proceeded: bool) {
        let _ = self.decided.lock().unwrap().send(proceeded);
    }

    fn release(&self) {
        let (lock, condvar) = &self.released;
        *lock.lock().unwrap() = true;
        condvar.notify_all();
    }
}

fn wait_decided(decided: &mpsc::Receiver<bool>) -> bool {
    decided
        .recv_timeout(Duration::from_secs(60))
        .expect("the worker decides once released")
}

/// A client that counts the requests it receives, reports its reasoning
/// count in the metadata, and signals when a worker took that metadata.
/// `hold` keeps a request open until the provider call gives up on it (the
/// call cancels the token on every exit); `gate` holds a bare client's
/// worker the way the watch's gate holds a watched one.
#[derive(Clone)]
struct Probe {
    calls: Arc<Mutex<usize>>,
    reasoning: Option<u64>,
    metadata: Option<ProviderResponseMetadata>,
    token: Option<CancelToken>,
    hold: bool,
    gate: Option<Arc<DispatchGate>>,
    taken: Arc<Mutex<mpsc::Sender<()>>>,
}

impl Probe {
    fn new(reasoning: Option<u64>) -> (Self, mpsc::Receiver<()>) {
        let (sender, receiver) = mpsc::channel();
        let probe = Self {
            calls: Arc::default(),
            reasoning,
            metadata: None,
            token: None,
            hold: false,
            gate: None,
            taken: Arc::new(Mutex::new(sender)),
        };
        (probe, receiver)
    }

    fn calls(&self) -> usize {
        *self.calls.lock().unwrap()
    }
}

impl ChatClient for Probe {
    fn label(&self) -> &str {
        "probe"
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn set_cancel_token(&mut self, token: CancelToken) {
        self.token = Some(token);
    }

    fn take_response_metadata(&mut self) -> Option<ProviderResponseMetadata> {
        let metadata = self.metadata.take();
        let _ = self.taken.lock().unwrap().send(());
        metadata
    }

    fn chat(
        &mut self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        if let Some(gate) = &self.gate {
            gate.hold();
            gate.decided(true);
        }
        *self.calls.lock().unwrap() += 1;
        if self.hold {
            let token = self.token.clone().expect("the provider call sets a token");
            let started = Instant::now();
            while !token.is_cancelled() && started.elapsed() < Duration::from_secs(60) {
                std::thread::sleep(Duration::from_millis(1));
            }
            anyhow::bail!("held request interrupted");
        }
        self.metadata = Some(ProviderResponseMetadata {
            reasoning_tokens: self.reasoning,
            ..ProviderResponseMetadata::default()
        });
        Ok(reply(Some(1), Vec::new()))
    }
}

#[derive(Debug, Clone, Copy)]
enum Interrupt {
    Cancel,
    Timeout,
}

const INTERRUPTS: [Interrupt; 2] = [Interrupt::Cancel, Interrupt::Timeout];

fn request(messages: &[ConversationMessage]) -> ProviderChatRequest<'_> {
    ProviderChatRequest {
        scope: ProviderCallScope::Executor,
        model: "m",
        messages,
        tools: &[],
        native_tools_enabled: true,
    }
}

fn call_config(root: &Path, timeout_secs: u64) -> Config {
    let mut config = config(root);
    config.chat_timeout_secs = timeout_secs;
    config
}

/// One provider call the way the loop makes it: through the watch, then
/// `settle`.
fn provider_call(
    leaf: &mut OutputLimitFeedback,
    client: &mut Probe,
    root: &Path,
    timeout_secs: u64,
    is_cancelled: impl Fn() -> bool,
) -> ProviderCallOutcome {
    let config = call_config(root, timeout_secs);
    let messages = [ConversationMessage::user("hello")];
    let outcome = crate::provider_call::chat_with_cancel_and_stream(
        &mut leaf.watch(&mut *client),
        &config,
        request(&messages),
        is_cancelled,
        &mut |_| Ok(()),
    );
    leaf.settle();
    outcome
}

/// The provider call's own result for `interrupt`: whether it timed out,
/// whether the user aborted it, and its error text.
fn interrupted(outcome: &ProviderCallOutcome, interrupt: Interrupt) -> (bool, bool, String) {
    let error = format!("{:#}", outcome.result.as_ref().unwrap_err());
    match interrupt {
        Interrupt::Cancel => assert!(error.contains("aborted_by_user"), "{error}"),
        Interrupt::Timeout => assert!(
            error.ends_with("provider call exceeded configured deadline of 1s"),
            "{error}"
        ),
    }
    let seen = (outcome.timed_out, outcome.aborted_by_user, error);
    assert_eq!(
        (seen.0, seen.1),
        (
            matches!(interrupt, Interrupt::Timeout),
            matches!(interrupt, Interrupt::Cancel)
        ),
        "{seen:?}"
    );
    seen
}

/// Ends a call through the watch with `interrupt` while its worker is held
/// right before the dispatch decision, then settles. The gate stays closed.
fn held_call(
    leaf: &mut OutputLimitFeedback,
    client: &mut Probe,
    root: &Path,
    interrupt: Interrupt,
) -> (ProviderCallOutcome, Arc<DispatchGate>, mpsc::Receiver<bool>) {
    let (gate, decided) = DispatchGate::new();
    leaf.dispatch_gate = Some(Arc::clone(&gate));
    let held = Arc::clone(&gate);
    let outcome = match interrupt {
        Interrupt::Cancel => provider_call(leaf, client, root, 30, move || {
            held.reached.load(Ordering::SeqCst)
        }),
        Interrupt::Timeout => provider_call(leaf, client, root, 1, || false),
    };
    leaf.dispatch_gate = None;
    (outcome, gate, decided)
}

#[test]
fn a_late_worker_never_sends_the_sentence_and_the_next_request_carries_it() {
    for interrupt in INTERRUPTS {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let mut leaf = OutputLimitFeedback::new(Some(&path));
        let (mut client, _taken) = Probe::new(Some(40));
        // The turn that hits the limit reports 40 reasoning tokens.
        let first = provider_call(&mut leaf, &mut client, dir.path(), 30, || false);
        assert!(first.result.is_ok());
        leaf.observe_reply(&reply(Some(100), Vec::new()), LIMIT);
        // Request A carries the sentence. Its worker is held right before
        // the dispatch decision while the call ends and is settled.
        assert_eq!(
            composed(&mut leaf, Some("A")),
            Some(format!("A\n\n{NO_TOOL_SENTENCE}"))
        );
        client.reasoning = Some(999);
        let (outcome, gate, decided) = held_call(&mut leaf, &mut client, dir.path(), interrupt);
        interrupted(&outcome, interrupt);
        assert!(feedback_events(&path).is_empty(), "{interrupt:?}");
        // Released afterwards, the worker finds A closed and never asks the
        // client; nothing is written.
        gate.release();
        assert!(!wait_decided(&decided), "{interrupt:?}: A must not be sent");
        assert_eq!(client.calls(), 1, "{interrupt:?}: the client never saw A");
        assert!(feedback_events(&path).is_empty(), "{interrupt:?}");
        // Request B carries the sentence once; its dispatch is attempt 1 and
        // keeps the reasoning count of the turn that hit the limit.
        assert_eq!(
            composed(&mut leaf, Some("B")),
            Some(format!("B\n\n{NO_TOOL_SENTENCE}")),
            "{interrupt:?}"
        );
        client.reasoning = Some(55);
        let second = provider_call(&mut leaf, &mut client, dir.path(), 30, || false);
        assert!(second.result.is_ok());
        assert_eq!(client.calls(), 2, "{interrupt:?}");
        let written = feedback_events(&path);
        assert_eq!(written.len(), 1, "{interrupt:?}: {written:?}");
        assert_eq!(written[0]["attempt"], 1, "{interrupt:?}");
        assert_eq!(written[0]["provider_reasoning_tokens"], 40, "{interrupt:?}");
        assert_eq!(composed(&mut leaf, Some("C")), Some("C".into()));
    }
}

#[test]
fn a_sentence_request_that_reached_the_client_is_recorded_even_if_the_call_then_stops() {
    for interrupt in INTERRUPTS {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let mut leaf = OutputLimitFeedback::new(Some(&path));
        leaf.observe_reply(&reply(Some(100), Vec::new()), LIMIT);
        assert_eq!(composed(&mut leaf, None), Some(NO_TOOL_SENTENCE.into()));
        // The client receives the request, then the call is interrupted.
        let (mut client, _taken) = Probe::new(Some(12));
        client.hold = true;
        let calls = Arc::clone(&client.calls);
        let outcome = match interrupt {
            Interrupt::Cancel => provider_call(&mut leaf, &mut client, dir.path(), 30, move || {
                *calls.lock().unwrap() > 0
            }),
            Interrupt::Timeout => provider_call(&mut leaf, &mut client, dir.path(), 1, || false),
        };
        interrupted(&outcome, interrupt);
        assert_eq!(client.calls(), 1, "{interrupt:?}");
        let written = feedback_events(&path);
        assert_eq!(written.len(), 1, "{interrupt:?}: {written:?}");
        assert_eq!(written[0]["attempt"], 1, "{interrupt:?}");
        assert!(written[0].get("provider_reasoning_tokens").is_none());
        assert_eq!(
            composed(&mut leaf, None),
            None,
            "{interrupt:?}: a sent sentence is not armed again"
        );
    }
}

/// The provider call's result and the client's request count for a late
/// worker of a request without the sentence, through the watch or with the
/// bare client.
fn late_worker_without_sentence(
    interrupt: Interrupt,
    through_watch: bool,
) -> ((bool, bool, String), usize) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("events.jsonl");
    let mut leaf = OutputLimitFeedback::new(Some(&path));
    let (mut client, taken) = Probe::new(Some(77));
    let (outcome, gate, decided) = if through_watch {
        held_call(&mut leaf, &mut client, dir.path(), interrupt)
    } else {
        let (gate, decided) = DispatchGate::new();
        client.gate = Some(Arc::clone(&gate));
        let held = Arc::clone(&gate);
        let (timeout_secs, cancel) = match interrupt {
            Interrupt::Cancel => (30, true),
            Interrupt::Timeout => (1, false),
        };
        let config = call_config(dir.path(), timeout_secs);
        let messages = [ConversationMessage::user("hello")];
        let outcome = crate::provider_call::chat_with_cancel_and_stream(
            &mut client,
            &config,
            request(&messages),
            move || cancel && held.reached.load(Ordering::SeqCst),
            &mut |_| Ok(()),
        );
        (outcome, gate, decided)
    };
    let seen = interrupted(&outcome, interrupt);
    let late_record = Arc::clone(&leaf.call);
    gate.release();
    assert!(
        wait_decided(&decided),
        "{interrupt:?}: the late worker goes on"
    );
    taken
        .recv_timeout(Duration::from_secs(60))
        .expect("the late worker finishes");
    let calls = client.calls();
    assert!(feedback_events(&path).is_empty());
    if through_watch {
        // The late worker's reasoning count stays in its own call's record
        // (written right after the metadata is taken).
        let started = Instant::now();
        while lock(&late_record).reasoning_tokens != Some(77) {
            assert!(started.elapsed() < Duration::from_secs(60));
            std::thread::yield_now();
        }
        // A later limit turn without a count records none, and the request
        // after it reports none either.
        client.gate = None;
        client.reasoning = None;
        assert!(
            provider_call(&mut leaf, &mut client, dir.path(), 30, || false)
                .result
                .is_ok()
        );
        leaf.observe_reply(&reply(Some(100), Vec::new()), LIMIT);
        assert!(composed(&mut leaf, None).is_some());
        client.reasoning = Some(3);
        assert!(
            provider_call(&mut leaf, &mut client, dir.path(), 30, || false)
                .result
                .is_ok()
        );
        let written = feedback_events(&path);
        assert_eq!(written.len(), 1, "{written:?}");
        assert!(written[0].get("provider_reasoning_tokens").is_none());
    }
    (seen, calls)
}

#[test]
fn a_request_without_the_sentence_lets_a_late_worker_ask_the_client_as_before() {
    for interrupt in INTERRUPTS {
        let bare = late_worker_without_sentence(interrupt, false);
        let watched = late_worker_without_sentence(interrupt, true);
        assert_eq!(watched, bare, "{interrupt:?}");
        assert_eq!(
            watched.1, 1,
            "{interrupt:?}: the late worker asked the client"
        );
    }
}

/// Logs every method the watch must forward and answers with fixed values,
/// streaming two chunks.
#[derive(Clone, Default)]
struct Spy {
    log: Arc<Mutex<Vec<String>>>,
}

impl Spy {
    fn note(&self, entry: impl Into<String>) {
        self.log.lock().unwrap().push(entry.into());
    }
}

impl ChatClient for Spy {
    fn label(&self) -> &str {
        "spy"
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        self.note("boxed_clone");
        Box::new(self.clone())
    }

    fn supports_native_tools(&self, model: &str) -> bool {
        self.note(format!("native {model}"));
        model == "native"
    }

    fn allows_xml_fallback(&self) -> bool {
        self.note("xml_fallback");
        true
    }

    fn parses_xml_tool_calls_before_host(&self) -> bool {
        self.note("xml_before_host");
        true
    }

    fn supports_ollama_think(&self) -> bool {
        self.note("ollama_think");
        true
    }

    fn take_response_timing(&mut self) -> Option<ResponseTiming> {
        self.note("timing");
        Some(ResponseTiming {
            total_duration: Some(7),
            ..ResponseTiming::default()
        })
    }

    fn take_response_metadata(&mut self) -> Option<ProviderResponseMetadata> {
        self.note("metadata");
        Some(ProviderResponseMetadata {
            response_id: Some("r".to_string()),
            reasoning_tokens: Some(21),
            ..ProviderResponseMetadata::default()
        })
    }

    fn set_cancel_token(&mut self, token: CancelToken) {
        self.note(format!("cancel {}", token.is_cancelled()));
    }

    fn provider_turn_carried_result(&mut self) {
        self.note("carried");
    }

    fn supports_streaming(&self) -> bool {
        self.note("streaming");
        true
    }

    fn supports_streaming_for_model(&self, model: &str) -> bool {
        self.note(format!("streaming {model}"));
        model == "stream"
    }

    fn chat_stream(
        &mut self,
        model: &str,
        messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        native_tools_enabled: bool,
        on_chunk: &mut dyn FnMut(&str) -> anyhow::Result<()>,
    ) -> anyhow::Result<AssistantReply> {
        self.note(format!(
            "chat_stream {model} {} {native_tools_enabled}",
            messages.len()
        ));
        on_chunk("a")?;
        on_chunk("b")?;
        Ok(reply(Some(2), Vec::new()))
    }

    fn chat(
        &mut self,
        model: &str,
        messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        self.note(format!(
            "chat {model} {} {native_tools_enabled}",
            messages.len()
        ));
        Ok(reply(Some(3), Vec::new()))
    }
}

/// Every method the way a provider call uses it; the answers, the chunks,
/// and the log the client kept.
fn exercise(client: &mut dyn ChatClient, spy: &Spy) -> Vec<String> {
    let messages = [ConversationMessage::user("hello")];
    let mut seen = vec![
        client.label().to_string(),
        format!("{}", client.supports_native_tools("native")),
        format!("{}", client.allows_xml_fallback()),
        format!("{}", client.parses_xml_tool_calls_before_host()),
        format!("{}", client.supports_ollama_think()),
        format!("{}", client.supports_streaming()),
        format!("{}", client.supports_streaming_for_model("stream")),
    ];
    let mut worker = client.boxed_clone();
    worker.set_cancel_token(CancelToken::default());
    let answered = ChatClient::chat(worker.as_mut(), "plain", &messages, &[], false);
    seen.push(format!("{answered:?}"));
    let mut chunks = Vec::new();
    let streamed = worker.chat_stream("stream", &messages, &[], true, &mut |chunk| {
        chunks.push(chunk.to_string());
        Ok(())
    });
    seen.push(format!("{streamed:?} {chunks:?}"));
    seen.push(format!("{:?}", worker.take_response_timing()));
    seen.push(format!("{:?}", worker.take_response_metadata()));
    client.provider_turn_carried_result();
    seen.extend(spy.log.lock().unwrap().iter().cloned());
    seen
}

#[test]
fn the_watch_forwards_streaming_cancel_timing_metadata_and_the_carried_result() {
    let bare_spy = Spy::default();
    let bare = exercise(&mut bare_spy.clone(), &bare_spy);
    for guarded in [false, true] {
        let mut leaf = OutputLimitFeedback::new(None);
        if guarded {
            leaf.observe_reply(&reply(Some(100), Vec::new()), LIMIT);
            assert!(composed(&mut leaf, None).is_some());
        }
        let spy = Spy::default();
        let mut inner = spy.clone();
        let watched = exercise(&mut leaf.watch(&mut inner), &spy);
        assert_eq!(watched, bare, "guarded {guarded}");
        assert_eq!(lock(&leaf.call).reasoning_tokens, Some(21));
        leaf.settle();
    }
}

// ---- the minimal loop with a scripted client -------------------------------

#[test]
fn a_configured_guard_limit_is_kept_and_its_stop_sends_no_sentence() {
    let client = Scripted::new(vec![
        limit_text(Some(11)),
        limit_text(Some(12)),
        limit_text(Some(13)),
    ]);
    let options = RunSessionOptions {
        max_length_no_tool_call_limit: Some(3),
        ..RunSessionOptions::default()
    };
    let run = run(client, &["notes.md"], options, |_| {});
    let error = run.result.clone().unwrap_err();
    assert!(
        error.contains("max_length_no_tool_call_repeated"),
        "{error}"
    );
    assert_eq!(run.requests.len(), 3, "no extra request at the guard stop");
    assert_eq!(run.unused, 0);
    assert_eq!(run.sentences_in(0, NO_TOOL_SENTENCE), 0);
    assert_eq!(run.sentences_in(1, NO_TOOL_SENTENCE), 1);
    assert_eq!(run.sentences_in(2, NO_TOOL_SENTENCE), 1);
    let written = run.feedback();
    let seen: Vec<(u64, u64)> = written
        .iter()
        .map(|event| {
            (
                event["attempt"].as_u64().unwrap(),
                event["provider_reasoning_tokens"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(seen, vec![(1, 11), (2, 12)]);
}

#[test]
fn the_default_guard_stop_at_the_second_limit_reply_sends_no_sentence() {
    let client = Scripted::new(vec![limit_text(None), limit_text(None)]);
    let run = run(client, &["notes.md"], RunSessionOptions::default(), |_| {});
    assert!(
        run.result
            .as_ref()
            .unwrap_err()
            .contains("max_length_no_tool_call_repeated")
    );
    assert_eq!(run.requests.len(), 2);
    assert_eq!(run.sentences_in(1, NO_TOOL_SENTENCE), 1);
    assert_eq!(run.feedback().len(), 1);
}

#[test]
fn an_xml_fallback_request_carries_the_sentence_after_the_existing_feedback() {
    let mut client = Scripted::new(vec![limit_text(None), limit_text(None)]);
    client.native = false;
    let run = run(client, &["notes.md"], RunSessionOptions::default(), |_| {});
    assert_eq!(run.requests.len(), 2);
    let system = &run.requests[1][0].content;
    assert!(system.contains("XML fallback"), "{system}");
    let last = run.requests[1].last().unwrap();
    assert_eq!(last.role, "user");
    assert!(
        last.content.ends_with(&format!("\n\n{NO_TOOL_SENTENCE}")),
        "{}",
        last.content
    );
    assert!(last.content.len() > NO_TOOL_SENTENCE.len() + 2);
    assert_eq!(run.feedback().len(), 1);
}

#[test]
fn an_exhausted_malformed_retry_without_xml_fallback_adds_no_call_or_event() {
    let steps = vec![
        Step::Typed(Some(100), ToolArgumentsParseFailure::EndOfInput),
        Step::Typed(Some(100), ToolArgumentsParseFailure::EndOfInput),
        Step::Typed(Some(100), ToolArgumentsParseFailure::EndOfInput),
    ];
    let run = run(
        Scripted::new(steps),
        &[],
        RunSessionOptions::default(),
        |_| {},
    );
    let error = run.result.clone().unwrap_err();
    assert!(error.contains("EOF while parsing"), "{error}");
    assert_eq!(run.requests.len(), 3, "the retry limit is unchanged");
    for index in [1, 2] {
        assert_eq!(run.sentences_in(index, TRUNCATED_SENTENCE), 1, "{index}");
        let last = run.requests[index].last().unwrap();
        assert!(last.content.contains(EOF_MESSAGE), "{}", last.content);
    }
    let kinds: Vec<(String, u64)> = run
        .feedback()
        .iter()
        .map(|event| {
            (
                event["kind"].as_str().unwrap().to_string(),
                event["attempt"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        kinds,
        vec![
            ("tool_arguments_truncated_at_output_limit".to_string(), 1),
            ("tool_arguments_truncated_at_output_limit".to_string(), 2),
        ]
    );
}

#[test]
fn a_limit_reply_on_the_last_iteration_adds_no_call_or_event() {
    // `max_iterations = 1` gives four iterations with the empty-response
    // allowance; only the fourth reply is at the limit.
    let read = ToolCall::new("Read", json!({"path": "seed.md"}));
    let steps = vec![
        Step::Reply(reply(Some(5), vec![read.clone()]), None),
        Step::Reply(reply(Some(5), vec![read.clone()]), None),
        Step::Reply(reply(Some(5), vec![read]), None),
        limit_text(Some(3)),
    ];
    let run = run(
        Scripted::new(steps),
        &["notes.md"],
        RunSessionOptions::default(),
        |config| {
            config.max_iterations = 1;
            std::fs::write(config.workspace_root.join("seed.md"), "seed").unwrap();
        },
    );
    assert!(run.result.is_err(), "{:?}", run.result);
    assert_eq!(run.requests.len(), 4);
    assert_eq!(run.unused, 0);
    assert!(run.feedback().is_empty());
}

#[test]
fn a_long_reply_with_a_tool_call_gets_no_sentence() {
    let write = ToolCall::new("Write", json!({"path": "notes.md", "content": "notes"}));
    let steps = vec![
        Step::Reply(reply(Some(400), vec![write]), Some(390)),
        Step::Reply(
            AssistantReply {
                content: "done".to_string(),
                ..reply(Some(3), Vec::new())
            },
            None,
        ),
    ];
    let run = run(
        Scripted::new(steps),
        &["notes.md"],
        RunSessionOptions::default(),
        |_| {},
    );
    for index in 0..run.requests.len() {
        assert_eq!(run.sentences_in(index, NO_TOOL_SENTENCE), 0);
    }
    assert!(run.feedback().is_empty());
}
