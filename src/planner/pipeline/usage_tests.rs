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
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
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
    timing: Option<ResponseTiming>,
    metadata: Option<ProviderResponseMetadata>,
}

impl ScriptedClient {
    fn new(script: Vec<Scripted>) -> Self {
        Self {
            script: Arc::new(Mutex::new(script.into())),
            calls: Arc::new(AtomicUsize::new(0)),
            timing: None,
            metadata: None,
        }
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

/// Check the emitted events against the fixture case and reconcile physical
/// calls with accounting events: every call is either a
/// `provider_turn_duration` or an `accounting_record: true` call event.
fn assert_case(id: &str, config: &Config, physical_calls: usize) {
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

    let accounting_records = call_events
        .iter()
        .filter(|value| value[fixture["accounting_flag"].as_str().unwrap()] == true)
        .count();
    assert_eq!(
        physical_calls,
        case["physical_calls"].as_u64().unwrap() as usize,
        "{id}: physical calls"
    );
    assert_eq!(
        physical_calls,
        turns.len() + accounting_records,
        "{id}: every physical call must be counted exactly once"
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

    assert_case("adopted-replayed", &config, planner.calls());
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

    assert_case("discarded-settled", &config, planner.calls());
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

    assert_case("cancelled-unsettled", &config, planner.calls());
}

#[test]
fn failed_prefetch_attempts_each_leave_one_record() {
    let dir = tempfile::tempdir().unwrap();
    let config = test_config(dir.path());
    let planner = ScriptedClient::new(vec![failed(), failed()]);
    let mut pipeline = started(&planner, &config);

    assert!(pipeline.take_planner(&key(), &planner, &config).is_none());

    assert_case("failed-attempts", &config, planner.calls());
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

    assert_case("adopted-unreplayed", &config, planner.calls());
}

#[test]
fn dropped_pipeline_keeps_the_usage_of_a_settled_prefetch() {
    let dir = tempfile::tempdir().unwrap();
    let config = test_config(dir.path());
    let planner = ScriptedClient::new(vec![ok("prefetched", "resp-dropped", None)]);
    let pipeline = started(&planner, &config);
    wait_for_calls(&pipeline, 1, true);

    drop(pipeline);

    assert_case("pipeline-dropped", &config, planner.calls());
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

    assert_case(
        "http-adopted-replayed",
        &config,
        server.posts.load(Ordering::SeqCst),
    );
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

    assert_case(
        "http-discarded-missing-cache",
        &config,
        server.posts.load(Ordering::SeqCst),
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
