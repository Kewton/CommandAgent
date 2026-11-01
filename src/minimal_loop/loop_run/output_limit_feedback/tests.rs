//! Issue #958: unit tests of the output-limit feedback leaf, and runs of the
//! minimal loop with a scripted client for the cases the public entry cannot
//! configure (a non-default max-length guard limit, XML fallback requests,
//! an exhausted malformed retry without XML fallback).

use std::borrow::Cow;
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::super::{RunSessionOptions, run_session_with_outcome_with_options};
use super::OutputLimitFeedback;
use crate::config::{
    Action, Config, ConfigFieldSources, NarrationMode, OllamaThink, OpenAiApi, PlanPreset,
    PromptLayout, Provider,
};
use crate::providers::{
    AssistantReply, ChatClient, ProviderResponseMetadata, ToolArgumentsParseError,
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
