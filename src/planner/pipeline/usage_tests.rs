//! Issue #948: the speculative planner keeps each physical provider call's own
//! timing and metadata, replays an adopted reply without counting it twice,
//! and leaves an accounting record for every call it does not replay.
//!
//! The expectations live in
//! `tests/corpus/apps/prefetched-planner-usage/fixtures/events.json`; its
//! `base_pipeline_lines` were captured from the pipeline at base
//! 4edfc5a726feb5fa468b4a24bc9de29f41c6b1bc before this change.

use std::collections::VecDeque;
use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{
    CallAccount, CancellationUi, PROVIDER_CALL_EVENT, PairedCall, PhasePlanKey, PhasePlanPipeline,
    PlannerSessionMode, PrefetchCallRecord, PrefetchedPlanner, PrefetchedReply, RecordingClient,
    SettledCall, SharedCallLog, model_for, planner_chat_for_step_plan_attempt, step_plan_messages,
};
use crate::config::{
    Action, Config, ConfigFieldSources, NarrationMode, OllamaThink, OpenAiApi, PlanPreset,
    PromptLayout, Provider,
};
use crate::providers::openai::OpenAiClient;
use crate::providers::{AssistantReply, ChatClient, ProviderResponseMetadata, ResponseTiming};
use crate::sensitive_data::{
    RegistrationRefusal, ReservedKind, SecretCatalog, is_fixed_identifier,
};
use crate::state::ConversationMessage;
use crate::tools::registry::ToolSpec;

const CURRENT: &str = "phase-n";
const NEXT: &str = "phase-n-plus-one";
const PROMPT: &str = "Write notes.txt with one line of synthetic text.";
const FIXTURE: &str =
    include_str!("../../../tests/corpus/apps/prefetched-planner-usage/fixtures/events.json");
const MANIFEST: &str =
    include_str!("../../../tests/corpus/apps/prefetched-planner-usage/source-manifest.json");
/// Diagnostic ceiling only: it turns a missing signal into a bounded failure.
const DEADLINE: Duration = Duration::from_secs(5);

fn test_config(root: &Path) -> Config {
    Config {
        workspace_root: root.to_path_buf(),
        state_dir: PathBuf::from("state"),
        eval_events_path: Some(root.join("events.jsonl")),
        completion_contract_path: None,
        yes: true,
        offline: false,
        context_budget: 1000,
        model: "m".to_string(),
        provider: Provider::Ollama,
        openai_api: OpenAiApi::ChatCompletions,
        tool_protocol: None,
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
        max_iterations: 1,
        recovery_plan_auto_runs: 0,
        chat_timeout_secs: 10,
        chat_timeout_source: "override:test".to_string(),
        field_sources: ConfigFieldSources::default(),
        chat_retries: 0,
        stream: false,
        resume: None,
        fresh_session: false,
        no_footer: false,
        narration: NarrationMode::Normal,
        profile: "generic".to_string(),
        profile_explicit: false,
        profile_inference: None,
        style: "default".to_string(),
        action: Action::Repl,
    }
}

struct Scripted {
    result: Result<AssistantReply, String>,
    timing: Option<ResponseTiming>,
    metadata: Option<ProviderResponseMetadata>,
    hold: Option<Receiver<()>>,
}

/// A fake client whose clones share one script and one call counter. Like a
/// derived `Clone` provider, a clone copies the timing and metadata it holds.
#[derive(Clone)]
struct ScriptedClient {
    script: Arc<Mutex<VecDeque<Scripted>>>,
    calls: Arc<AtomicUsize>,
    paid: Arc<AtomicU64>,
    timing: Option<ResponseTiming>,
    metadata: Option<ProviderResponseMetadata>,
}

impl ScriptedClient {
    fn new(script: Vec<Scripted>) -> Self {
        Self {
            script: Arc::new(Mutex::new(script.into())),
            calls: Arc::new(AtomicUsize::new(0)),
            paid: Arc::new(AtomicU64::new(0)),
            timing: None,
            metadata: None,
        }
    }

    /// The prompt tokens of every reply this client (and its clones) returned.
    fn paid(&self) -> u64 {
        self.paid.load(Ordering::SeqCst)
    }

    fn holding_stale(mut self) -> Self {
        self.timing = Some(timing(9900, 9900));
        self.metadata = Some(metadata("resp-stale", Some(7)));
        self
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl ChatClient for ScriptedClient {
    fn label(&self) -> &str {
        "scripted"
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn take_response_timing(&mut self) -> Option<ResponseTiming> {
        self.timing.take()
    }

    fn take_response_metadata(&mut self) -> Option<ProviderResponseMetadata> {
        self.metadata.take()
    }

    fn chat(
        &mut self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let next = self
            .script
            .lock()
            .unwrap()
            .pop_front()
            .expect("the fake client ran out of scripted replies");
        if let Some(hold) = next.hold {
            let _ = hold.recv_timeout(DEADLINE);
        }
        self.timing = next.timing;
        self.metadata = next.metadata;
        if let Ok(reply) = &next.result {
            self.paid
                .fetch_add(reply.prompt_tokens.unwrap_or(0), Ordering::SeqCst);
        }
        next.result.map_err(|message| anyhow::anyhow!(message))
    }
}

fn reply(content: &str, prompt: u64, output: u64) -> AssistantReply {
    AssistantReply {
        prompt_tokens: Some(prompt),
        completion_tokens: Some(output),
        ..AssistantReply::text(content)
    }
}

fn metadata(id: &str, cached: Option<u64>) -> ProviderResponseMetadata {
    ProviderResponseMetadata {
        response_id: Some(id.to_string()),
        cached_input_tokens: cached,
        ..Default::default()
    }
}

fn timing(prompt_eval: u64, eval: u64) -> ResponseTiming {
    ResponseTiming {
        prompt_eval_duration: Some(prompt_eval),
        eval_duration: Some(eval),
        ..Default::default()
    }
}

fn ok(content: &str, id: &str, cached: Option<u64>) -> Scripted {
    Scripted {
        result: Ok(reply(content, 100, 5)),
        timing: None,
        metadata: Some(metadata(id, cached)),
        hold: None,
    }
}

fn failed() -> Scripted {
    Scripted {
        result: Err("synthetic provider failure".to_string()),
        timing: None,
        metadata: None,
        hold: None,
    }
}

fn event_lines(config: &Config) -> Vec<String> {
    std::fs::read_to_string(config.eval_events_path.as_ref().unwrap())
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

fn parse(line: &str) -> Value {
    serde_json::from_str(line).unwrap()
}

fn event_name(value: &Value) -> &str {
    value["event"].as_str().unwrap_or_default()
}

/// One planner call through the product path the replay takes.
fn planner_call(planner: &mut dyn ChatClient, config: &Config) -> AssistantReply {
    let ui = CancellationUi(Arc::new(AtomicBool::new(false)));
    planner_chat_for_step_plan_attempt(
        planner,
        config,
        model_for(config, true),
        &step_plan_messages("synthetic step plan request"),
        &ui,
        PlannerSessionMode::Standard,
    )
    .unwrap()
}

fn key() -> PhasePlanKey {
    PhasePlanKey::new(NEXT, PROMPT)
}

fn started(planner: &dyn ChatClient, config: &Config) -> PhasePlanPipeline {
    let mut pipeline = PhasePlanPipeline::new();
    pipeline.start(planner, config, CURRENT, NEXT, PROMPT);
    assert!(
        pipeline.pending.is_some(),
        "the synthetic phase prompt must need provider work"
    );
    pipeline
}

/// A call log whose calls made by client `generation` stop before they begin
/// until released. The worker client is generation 1 and each
/// `provider_call` clones it, so its first call is made by generation 2.
fn log_holding_generation(generation: usize) -> (SharedCallLog, Receiver<()>, mpsc::Sender<()>) {
    let (reached_tx, reached_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let reached_tx = Mutex::new(reached_tx);
    let release_rx = Mutex::new(release_rx);
    let log = SharedCallLog {
        before_begin: Some(Arc::new(move |caller| {
            if caller == generation {
                let _ = reached_tx.lock().unwrap().send(());
                let _ = release_rx.lock().unwrap().recv_timeout(DEADLINE);
            }
        })),
        ..SharedCallLog::default()
    };
    (log, reached_rx, release_tx)
}

fn started_with_log(
    planner: &dyn ChatClient,
    config: &Config,
    log: &SharedCallLog,
) -> PhasePlanPipeline {
    let mut pipeline = PhasePlanPipeline::new();
    pipeline.start_with_log(planner, config, CURRENT, NEXT, PROMPT, log.clone());
    assert!(pipeline.pending.is_some());
    pipeline
}

fn pending_calls(pipeline: &PhasePlanPipeline) -> Vec<PrefetchCallRecord> {
    pipeline.pending.as_ref().unwrap().calls.snapshot()
}

/// Wait until the worker has started `count` calls, all settled when
/// `settled` is set.
fn wait_for_calls(pipeline: &PhasePlanPipeline, count: usize, settled: bool) {
    let deadline = Instant::now() + DEADLINE;
    loop {
        let calls = pending_calls(pipeline);
        if calls.len() == count && (!settled || calls.iter().all(|call| call.settled.is_some())) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "the prefetch worker never reached {count} call(s): {calls:?}"
        );
        std::thread::yield_now();
    }
}

fn fixture() -> Value {
    serde_json::from_str(FIXTURE).unwrap()
}

fn fixture_case(id: &str) -> Value {
    fixture()["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == id)
        .unwrap_or_else(|| panic!("fixture has no case {id}"))
        .clone()
}

/// Check the emitted events against the fixture case and reconcile them with
/// what the provider actually did: every physical prefetch call has exactly one
/// call event, and the prompt tokens of every known reply are counted exactly
/// once, either by a `provider_turn_duration` or by an `accounting_record: true`
/// call event.
fn assert_case(id: &str, config: &Config, physical_calls: usize, paid_prompt_tokens: u64) {
    let case = fixture_case(id);
    let fixture = fixture();
    let lines = event_lines(config);

    let base = &fixture["base_pipeline_lines"];
    let pipeline_lines = lines
        .iter()
        .filter(|line| {
            let value = parse(line);
            let name = event_name(&value);
            name.starts_with("speculative_phase_plan_") && name != PROVIDER_CALL_EVENT
        })
        .cloned()
        .collect::<Vec<_>>();
    let expected_pipeline_lines = case["pipeline_events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|name| base[name.as_str().unwrap()].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        pipeline_lines, expected_pipeline_lines,
        "{id}: the existing pipeline events must keep their base bytes"
    );

    let call_events = lines
        .iter()
        .map(|line| parse(line))
        .filter(|value| event_name(value) == PROVIDER_CALL_EVENT)
        .collect::<Vec<_>>();
    assert_eq!(
        Value::Array(call_events.clone()),
        case["provider_call_events"],
        "{id}: provider call events"
    );

    let turns = lines
        .iter()
        .map(|line| parse(line))
        .filter(|value| event_name(value) == fixture["turn_event"])
        .collect::<Vec<_>>();
    let expected_turns = case["turns"].as_array().unwrap();
    assert_eq!(turns.len(), expected_turns.len(), "{id}: turns {turns:?}");
    for (index, (turn, expected)) in turns.iter().zip(expected_turns).enumerate() {
        for (field, value) in expected["expect"].as_object().unwrap() {
            assert_eq!(&turn[field], value, "{id}: turn {index} field {field}");
        }
        for field in expected["absent"].as_array().unwrap() {
            let field = field.as_str().unwrap();
            assert!(
                turn.get(field).is_none(),
                "{id}: turn {index} must not report {field}: {turn}"
            );
        }
    }

    assert_eq!(
        physical_calls,
        case["physical_calls"].as_u64().unwrap() as usize,
        "{id}: physical calls"
    );
    let prefetch_calls = case["prefetch_calls"].as_u64().unwrap() as usize;
    assert_eq!(
        call_events.len(),
        prefetch_calls,
        "{id}: every physical prefetch call must have exactly one call event"
    );
    assert!(prefetch_calls <= physical_calls, "{id}: prefetch calls");

    let prompt_tokens = |value: &Value| value["prompt_eval_count"].as_u64().unwrap_or(0);
    let counted = turns.iter().map(prompt_tokens).sum::<u64>()
        + call_events
            .iter()
            .filter(|value| value[fixture["accounting_flag"].as_str().unwrap()] == true)
            .map(prompt_tokens)
            .sum::<u64>();
    assert_eq!(
        counted, paid_prompt_tokens,
        "{id}: the usage of every known reply must be counted exactly once"
    );
}

fn paired(
    content: &str,
    timing: Option<ResponseTiming>,
    metadata: Option<ProviderResponseMetadata>,
) -> PrefetchedReply {
    let reply = reply(content, 100, 5);
    PrefetchedReply {
        reply: reply.clone(),
        call: Some(PairedCall {
            record: PrefetchCallRecord {
                call_index: 1,
                client_generation: 1,
                settled: Some(SettledCall {
                    ok: true,
                    reply: Some(reply),
                    timing,
                    metadata,
                }),
            },
            account: CallAccount {
                events_path: None,
                current_phase_id: CURRENT.to_string(),
                next_phase_id: NEXT.to_string(),
            },
        }),
    }
}

#[test]
fn replay_answers_with_the_paired_call_metadata_and_timing() {
    let fallback = ScriptedClient::new(vec![Scripted {
        timing: Some(timing(3300, 4400)),
        ..ok("fallback", "resp-fallback", Some(0))
    }])
    .holding_stale();
    let planner = PrefetchedPlanner::adopted(
        &fallback,
        paired(
            "prefetched",
            Some(timing(1100, 2200)),
            Some(metadata("resp-prefetch", Some(40))),
        ),
    );

    // The same order `provider_call` uses: clone, call, then take.
    let mut replay = planner.boxed_clone();
    let replayed = ChatClient::chat(replay.as_mut(), "m", &[], &[], false).unwrap();
    assert_eq!(replayed.content, "prefetched");
    assert_eq!(replay.take_response_timing(), Some(timing(1100, 2200)));
    assert_eq!(
        replay.take_response_metadata(),
        Some(metadata("resp-prefetch", Some(40)))
    );
    assert_eq!(replay.take_response_metadata(), None, "taken once");

    let mut next = planner.boxed_clone();
    let fallback_reply = ChatClient::chat(next.as_mut(), "m", &[], &[], false).unwrap();
    assert_eq!(fallback_reply.content, "fallback");
    assert_eq!(next.take_response_timing(), Some(timing(3300, 4400)));
    assert_eq!(
        next.take_response_metadata(),
        Some(metadata("resp-fallback", Some(0)))
    );
    assert_eq!(fallback.calls(), 1, "the replay made no provider call");
}

#[test]
fn replay_without_reported_metadata_never_borrows_the_fallback_values() {
    let fallback = ScriptedClient::new(Vec::new()).holding_stale();
    let mut planner = PrefetchedPlanner::adopted(&fallback, paired("prefetched", None, None));

    let replayed = ChatClient::chat(&mut planner, "m", &[], &[], false).unwrap();

    assert_eq!(replayed.content, "prefetched");
    assert_eq!(planner.take_response_timing(), None);
    assert_eq!(
        planner.take_response_metadata(),
        None,
        "unknown must stay unknown, not become the fallback's stale response"
    );
    assert_eq!(fallback.calls(), 0);
}

#[test]
fn clones_keep_their_own_call_metadata() {
    let fallback = ScriptedClient::new(vec![ok("fallback", "resp-fallback", None)]);
    let planner = PrefetchedPlanner::adopted(
        &fallback,
        paired(
            "prefetched",
            None,
            Some(metadata("resp-prefetch", Some(40))),
        ),
    );
    let mut early = planner.boxed_clone();
    let mut replay = planner.boxed_clone();

    let replayed = ChatClient::chat(replay.as_mut(), "m", &[], &[], false).unwrap();
    let fell_back = ChatClient::chat(early.as_mut(), "m", &[], &[], false).unwrap();

    assert_eq!(replayed.content, "prefetched");
    assert_eq!(fell_back.content, "fallback");
    assert_eq!(
        early.take_response_metadata(),
        Some(metadata("resp-fallback", None))
    );
    assert_eq!(
        replay.take_response_metadata(),
        Some(metadata("resp-prefetch", Some(40)))
    );
}

#[test]
fn recording_client_settles_each_call_with_its_own_values() {
    let calls = SharedCallLog::default();
    let inner = ScriptedClient::new(vec![
        Scripted {
            timing: Some(timing(1, 2)),
            ..ok("one", "resp-one", Some(10))
        },
        ok("two", "resp-two", None),
    ])
    .holding_stale();
    let mut client = RecordingClient::new(Box::new(inner), calls.clone());

    ChatClient::chat(&mut client, "m", &[], &[], false).unwrap();
    let mut clone = client.boxed_clone();
    assert_eq!(clone.take_response_metadata(), None, "a clone starts empty");
    assert_eq!(
        client.take_response_metadata(),
        Some(metadata("resp-one", Some(10)))
    );
    ChatClient::chat(clone.as_mut(), "m", &[], &[], false).unwrap();
    assert_eq!(clone.take_response_timing(), None);

    let records = calls.snapshot();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].call_index, 1);
    let first = records[0].settled.as_ref().unwrap();
    assert_eq!(first.metadata, Some(metadata("resp-one", Some(10))));
    assert_eq!(first.timing, Some(timing(1, 2)));
    let second = records[1].settled.as_ref().unwrap();
    assert_eq!(second.metadata, Some(metadata("resp-two", None)));
    assert_eq!(second.timing, None);
}

#[test]
fn adopted_prefetch_is_counted_once_by_its_replayed_turn() {
    let dir = tempfile::tempdir().unwrap();
    let config = test_config(dir.path());
    let planner = ScriptedClient::new(vec![
        Scripted {
            timing: Some(timing(1100, 2200)),
            ..ok("prefetched", "resp-prefetch", Some(40))
        },
        Scripted {
            result: Ok(reply("fallback", 90, 7)),
            timing: Some(timing(3300, 4400)),
            metadata: Some(metadata("resp-fallback", Some(0))),
            hold: None,
        },
    ]);
    let mut pipeline = started(&planner, &config);
    let mut adopted = pipeline
        .take_planner(&key(), &planner, &config)
        .expect("the prefetched reply is adopted");

    assert_eq!(
        planner_call(adopted.as_mut(), &config).content,
        "prefetched"
    );
    assert_eq!(planner_call(adopted.as_mut(), &config).content, "fallback");
    drop(adopted);
    drop(pipeline);

    assert_case("adopted-replayed", &config, planner.calls(), planner.paid());
}

#[test]
fn discarded_prefetch_keeps_the_usage_of_its_paid_reply() {
    let dir = tempfile::tempdir().unwrap();
    let config = test_config(dir.path());
    let planner = ScriptedClient::new(vec![Scripted {
        timing: Some(timing(5500, 6600)),
        ..ok("prefetched", "resp-discarded", Some(0))
    }]);
    let mut pipeline = started(&planner, &config);
    wait_for_calls(&pipeline, 1, true);

    let stale = PhasePlanKey::new(NEXT, "a prompt changed at the gate");
    assert!(pipeline.take_planner(&stale, &planner, &config).is_none());

    assert_case(
        "discarded-settled",
        &config,
        planner.calls(),
        planner.paid(),
    );
}

#[test]
fn cancelled_in_flight_prefetch_is_recorded_with_unknown_usage() {
    let dir = tempfile::tempdir().unwrap();
    let config = test_config(dir.path());
    let (release, hold) = mpsc::channel();
    let planner = ScriptedClient::new(vec![Scripted {
        hold: Some(hold),
        ..ok("prefetched", "resp-late", Some(40))
    }]);
    let mut pipeline = started(&planner, &config);
    wait_for_calls(&pipeline, 1, false);

    pipeline.cancel_and_discard(&config, "verification_failed");
    let _ = release.send(());

    // The held call had no outcome when the log closed, so none of its usage
    // is known: nothing may be counted for it.
    assert_case("cancelled-unsettled", &config, planner.calls(), 0);
}

#[test]
fn failed_prefetch_attempts_each_leave_one_record() {
    let dir = tempfile::tempdir().unwrap();
    let config = test_config(dir.path());
    let planner = ScriptedClient::new(vec![failed(), failed()]);
    let mut pipeline = started(&planner, &config);

    assert!(pipeline.take_planner(&key(), &planner, &config).is_none());

    assert_case("failed-attempts", &config, planner.calls(), planner.paid());
}

#[test]
fn adopted_reply_dropped_unreplayed_is_still_accounted() {
    let dir = tempfile::tempdir().unwrap();
    let config = test_config(dir.path());
    let planner = ScriptedClient::new(vec![ok("prefetched", "resp-unreplayed", Some(40))]);
    let mut pipeline = started(&planner, &config);

    let adopted = pipeline.take_planner(&key(), &planner, &config);
    assert!(adopted.is_some());
    drop(adopted);

    assert_case(
        "adopted-unreplayed",
        &config,
        planner.calls(),
        planner.paid(),
    );
}

#[test]
fn dropped_pipeline_keeps_the_usage_of_a_settled_prefetch() {
    let dir = tempfile::tempdir().unwrap();
    let config = test_config(dir.path());
    let planner = ScriptedClient::new(vec![ok("prefetched", "resp-dropped", None)]);
    let pipeline = started(&planner, &config);
    wait_for_calls(&pipeline, 1, true);

    drop(pipeline);

    assert_case("pipeline-dropped", &config, planner.calls(), planner.paid());
}

/// B1 (adoption side): the main path's `provider_call` is cancelled before it
/// asks for the reply. Its turn is an abort without usage and without a
/// physical call, and the adopted call is counted once by its own record.
#[test]
fn adopted_reply_cancelled_before_its_replay_is_counted_once() {
    let dir = tempfile::tempdir().unwrap();
    let config = test_config(dir.path());
    let planner = ScriptedClient::new(vec![ok("prefetched", "resp-precancelled", Some(40))]);
    let mut pipeline = started(&planner, &config);
    let mut adopted = pipeline
        .take_planner(&key(), &planner, &config)
        .expect("the prefetched reply is adopted");

    let interrupted = CancellationUi(Arc::new(AtomicBool::new(true)));
    let result = planner_chat_for_step_plan_attempt(
        adopted.as_mut(),
        &config,
        model_for(&config, true),
        &step_plan_messages("synthetic step plan request"),
        &interrupted,
        PlannerSessionMode::Standard,
    );
    assert!(result.is_err(), "the cancelled call returns no reply");
    drop(adopted);
    drop(pipeline);

    assert_case(
        "adopted-precancelled",
        &config,
        planner.calls(),
        planner.paid(),
    );
}

/// B2: the `provider_call` thread is held before its call begins while the
/// pipeline cancels, joins the worker, and closes the log. Released afterwards,
/// it starts no provider call, so no physical call is left without a record.
#[test]
fn a_call_that_begins_after_the_discard_starts_no_provider_call() {
    let dir = tempfile::tempdir().unwrap();
    let config = test_config(dir.path());
    let (log, reached, release) = log_holding_generation(2);
    let planner = ScriptedClient::new(vec![ok("late", "resp-never", Some(40))]);
    let mut pipeline = started_with_log(&planner, &config, &log);
    reached
        .recv_timeout(DEADLINE)
        .expect("the provider_call thread reached its call");

    pipeline.cancel_and_discard(&config, "verification_failed");
    release.send(()).unwrap();
    let deadline = Instant::now() + DEADLINE;
    while log.lock().refused_begins == 0 {
        assert!(Instant::now() < deadline, "the late call never began");
        std::thread::yield_now();
    }

    assert_eq!(planner.calls(), 0, "no provider call after the log closed");
    assert!(log.snapshot().is_empty());
    assert_case(
        "late-begin-refused",
        &config,
        planner.calls(),
        planner.paid(),
    );
}

/// B2: the first attempt's thread is held before its call begins until that
/// attempt reaches its deadline and the second attempt has answered. It then
/// begins last, yet the adopted reply stays paired with the second attempt's
/// call, and the late call is counted once by its own record.
#[test]
fn an_earlier_attempt_that_begins_late_never_takes_the_later_reply() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = test_config(dir.path());
    config.chat_timeout_secs = 1;
    let (log, reached, release) = log_holding_generation(2);
    let planner = ScriptedClient::new(vec![
        ok("later", "resp-later", None),
        ok("earlier-late", "resp-earlier-late", None),
    ]);
    let mut pipeline = started_with_log(&planner, &config, &log);
    reached
        .recv_timeout(DEADLINE)
        .expect("the first attempt's thread reached its call");

    wait_for_calls(&pipeline, 1, true);
    release.send(()).unwrap();
    wait_for_calls(&pipeline, 2, true);
    let records = pending_calls(&pipeline);
    assert_eq!(
        records[0].client_generation, 3,
        "the second attempt began first"
    );
    assert_eq!(
        records[1].client_generation, 2,
        "the first attempt began last"
    );

    let mut adopted = pipeline
        .take_planner(&key(), &planner, &config)
        .expect("the second attempt's reply is adopted");
    assert_eq!(planner_call(adopted.as_mut(), &config).content, "later");
    drop(adopted);
    drop(pipeline);

    assert_case(
        "late-earlier-attempt",
        &config,
        planner.calls(),
        planner.paid(),
    );
}

#[test]
fn a_pipeline_without_a_prefetch_emits_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let config = test_config(dir.path());
    let planner = ScriptedClient::new(Vec::new());
    let mut pipeline = PhasePlanPipeline::new();

    assert!(pipeline.take_planner(&key(), &planner, &config).is_none());
    pipeline.cancel_and_discard(&config, "verification_failed");
    drop(pipeline);

    assert!(event_lines(&config).is_empty());
    assert_eq!(planner.calls(), 0);
}

/// A fallback turn through the prefetched planner must match, byte for byte,
/// the turn of the bare client path that this change does not touch, apart
/// from the two fields that do not describe the call path: the measured wall
/// time and `estimated_stable_prefix_chars`, which compares the prompt with the
/// previous one sent under the same provider and model anywhere in the process.
#[test]
fn fallback_turn_matches_the_bare_client_turn() {
    fn turn_without_wall_time(config: &Config) -> String {
        let mut turns = event_lines(config)
            .iter()
            .map(|line| parse(line))
            .filter(|value| event_name(value) == "provider_turn_duration")
            .collect::<Vec<_>>();
        let mut turn = turns.pop().expect("a provider turn");
        let fields = turn.as_object_mut().unwrap();
        assert!(fields.remove("duration_ms").is_some());
        assert!(fields.remove("estimated_stable_prefix_chars").is_some());
        turn.to_string()
    }
    let scripted = || Scripted {
        timing: Some(timing(3300, 4400)),
        ..ok("fallback", "resp-fallback", Some(0))
    };

    let bare_dir = tempfile::tempdir().unwrap();
    let bare_config = test_config(bare_dir.path());
    let mut bare = ScriptedClient::new(vec![scripted()]);
    planner_call(&mut bare, &bare_config);

    let dir = tempfile::tempdir().unwrap();
    let config = test_config(dir.path());
    let fallback = ScriptedClient::new(vec![scripted()]);
    let mut planner = PrefetchedPlanner::adopted(&fallback, paired("prefetched", None, None));
    planner_call(&mut planner, &config);
    planner_call(&mut planner, &config);

    assert_eq!(
        turn_without_wall_time(&config),
        turn_without_wall_time(&bare_config)
    );
}

struct ChatServer {
    base_url: String,
    posts: Arc<AtomicUsize>,
}

/// A throwaway loopback OpenAI-compatible stand-in. It answers each POST with
/// the next scripted body and counts the POSTs it received.
fn chat_server(bodies: Vec<Value>) -> ChatServer {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let posts = Arc::new(AtomicUsize::new(0));
    let server_posts = Arc::clone(&posts);
    let bodies = Arc::new(Mutex::new(VecDeque::from(bodies)));
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else {
                continue;
            };
            serve_one(stream, &server_posts, &bodies);
        }
    });
    ChatServer { base_url, posts }
}

fn serve_one(mut stream: TcpStream, posts: &AtomicUsize, bodies: &Mutex<VecDeque<Value>>) {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    let header_end = loop {
        let read = stream.read(&mut buffer).unwrap_or(0);
        if read == 0 {
            return;
        }
        request.extend_from_slice(&buffer[..read]);
        if let Some(end) = request.windows(4).position(|window| window == b"\r\n\r\n") {
            break end + 4;
        }
    };
    let head = String::from_utf8_lossy(&request[..header_end]).to_ascii_lowercase();
    let length = head
        .lines()
        .find_map(|line| line.strip_prefix("content-length:"))
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    while request.len() < header_end + length {
        let read = stream.read(&mut buffer).unwrap_or(0);
        if read == 0 {
            return;
        }
        request.extend_from_slice(&buffer[..read]);
    }
    if head.starts_with("post /v1/chat/completions") {
        posts.fetch_add(1, Ordering::SeqCst);
    }
    let body = bodies
        .lock()
        .unwrap()
        .pop_front()
        .map(|body| body.to_string())
        .unwrap_or_else(|| "{}".to_string());
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

fn completion(id: &str, details: Option<Value>) -> Value {
    let mut usage = serde_json::json!({"prompt_tokens": 100, "completion_tokens": 5});
    if let Some(details) = details {
        usage["prompt_tokens_details"] = details;
    }
    serde_json::json!({
        "id": id,
        "model": "synthetic-model",
        "choices": [{"message": {"role": "assistant", "content": "synthetic plan"}}],
        "usage": usage,
    })
}

#[test]
fn local_http_posts_match_accounting_events_for_an_adopted_prefetch() {
    let server = chat_server(vec![
        completion(
            "resp-http-prefetch",
            Some(serde_json::json!({"cached_tokens": 40})),
        ),
        completion(
            "resp-http-fallback",
            Some(serde_json::json!({"cached_tokens": 0})),
        ),
    ]);
    let dir = tempfile::tempdir().unwrap();
    let config = test_config(dir.path());
    let planner = OpenAiClient::for_test("synthetic-test-key", server.base_url.clone(), None);
    let mut pipeline = started(&planner, &config);
    let mut adopted = pipeline
        .take_planner(&key(), &planner, &config)
        .expect("the prefetched reply is adopted");

    planner_call(adopted.as_mut(), &config);
    planner_call(adopted.as_mut(), &config);
    drop(adopted);
    drop(pipeline);

    let posts = server.posts.load(Ordering::SeqCst);
    assert_case("http-adopted-replayed", &config, posts, posts as u64 * 100);
}

#[test]
fn local_http_posts_match_accounting_events_for_a_discarded_prefetch() {
    let server = chat_server(vec![completion("resp-http-discarded", None)]);
    let dir = tempfile::tempdir().unwrap();
    let config = test_config(dir.path());
    let planner = OpenAiClient::for_test("synthetic-test-key", server.base_url.clone(), None);
    let mut pipeline = started(&planner, &config);
    wait_for_calls(&pipeline, 1, true);

    pipeline.cancel_and_discard(&config, "verification_failed");

    let posts = server.posts.load(Ordering::SeqCst);
    assert_case(
        "http-discarded-missing-cache",
        &config,
        posts,
        posts as u64 * 100,
    );
}

#[test]
fn corpus_manifest_pins_the_fixture_and_its_synthetic_origin() {
    let manifest: Value = serde_json::from_str(MANIFEST).unwrap();
    let digest = format!("{:x}", Sha256::digest(FIXTURE.as_bytes()));

    assert_eq!(manifest["fixture_sha256"]["events.json"], digest);
    assert_eq!(
        manifest["base_sha"],
        "4edfc5a726feb5fa468b4a24bc9de29f41c6b1bc"
    );
    assert_eq!(manifest["origin"], "synthetic");
    for forbidden in [
        "/Users/",
        "/Volumes/",
        "/home/",
        "/tmp/",
        "Bearer",
        "sk-",
        "api_key",
    ] {
        assert!(!FIXTURE.contains(forbidden), "fixture contains {forbidden}");
        assert!(
            !MANIFEST.contains(forbidden),
            "manifest contains {forbidden}"
        );
    }
    assert_eq!(fixture()["event"], PROVIDER_CALL_EVENT);
}

/// The fixed schema words the call event added (Issue #948). The event name is
/// emitted through a constant, so the source scan of the existing completeness
/// test does not see it; this list is checked against the fixture below.
const CALL_EVENT_WORDS: &[&str] = &[
    PROVIDER_CALL_EVENT,
    "accounted_by",
    "accounting_record",
    "adopted_unreplayed",
    "call_index",
    "disposition",
    "pipeline_dropped",
    "replayed",
    "unconsumed",
    "unsettled",
];

/// Keys the call event shares with events that existed before it.
const PRE_EXISTING_KEYS: &[&str] = &[
    "event",
    "schema_version",
    "current_phase_id",
    "next_phase_id",
    "reason",
    "ok",
    "prompt_eval_count",
    "eval_count",
    "provider_response_id",
    "provider_model_id",
    "provider_service_tier",
    "provider_cached_input_tokens",
    "provider_reasoning_tokens",
    "provider_total_tokens",
    "prompt_eval_duration",
    "eval_duration",
    "load_duration",
    "total_duration",
];

fn fixture_call_events() -> Vec<Value> {
    fixture()["cases"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|case| case["provider_call_events"].as_array().unwrap().clone())
        .collect()
}

#[test]
fn call_event_schema_words_are_reserved_fixed_identifiers() {
    for word in CALL_EVENT_WORDS {
        assert!(is_fixed_identifier(word), "{word} is not reserved");
    }

    let base_lines = fixture()["base_pipeline_lines"].to_string();
    for event in fixture_call_events() {
        for (key, value) in event.as_object().unwrap() {
            if !PRE_EXISTING_KEYS.contains(&key.as_str()) {
                assert!(CALL_EVENT_WORDS.contains(&key.as_str()), "new key {key}");
            }
            let Some(text) = value.as_str() else {
                continue;
            };
            let new_reason =
                key == "reason" && !base_lines.contains(&format!("\\\"reason\\\":\\\"{text}\\\""));
            if matches!(key.as_str(), "event" | "disposition" | "accounted_by") || new_reason {
                assert!(
                    is_fixed_identifier(text),
                    "{key} value {text} is not reserved"
                );
            }
        }
    }
}

#[test]
fn a_credential_equal_to_a_call_event_word_is_refused_without_naming_it() {
    for word in CALL_EVENT_WORDS {
        let mut catalog = SecretCatalog::new();
        let refusal = catalog
            .register_credential("SYNTHETIC_API_KEY", word)
            .expect_err("a credential equal to a schema word must be refused");
        assert!(
            matches!(
                refusal,
                RegistrationRefusal::Reserved {
                    kind: ReservedKind::FixedIdentifier,
                    ..
                }
            ),
            "unexpected refusal kind"
        );
        assert!(!refusal.to_string().contains(word), "the refusal names it");
        assert!(catalog.is_empty());
    }
}

#[test]
fn call_events_keep_their_schema_under_a_registered_credential() {
    let mut catalog = SecretCatalog::new();
    catalog
        .register_credential("SYNTHETIC_API_KEY", "synthetic-credential-0123456789")
        .expect("an ordinary credential registers");

    for event in fixture_call_events() {
        let mut scrubbed = event.clone();
        catalog.scrub_value_lenient(&mut scrubbed);
        assert_eq!(scrubbed, event, "the scrub changed a call event");
    }
}
