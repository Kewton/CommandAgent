use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock, mpsc};
use std::time::{Duration, Instant};

use anyhow::anyhow;
use serde_json::json;

use crate::config::{Config, OllamaThink, Provider};
use crate::eval_events;
use crate::providers::streaming::CancelToken;
use crate::providers::usage_observation::CacheWriteObservation;
use crate::providers::{AssistantReply, ChatClient, ProviderResponseMetadata, ResponseTiming};
use crate::state::ConversationMessage;
use crate::structured_budget::identity::ResponseIdentity;
use crate::structured_budget::session::FinalRepairMarker;
use crate::tools::registry::ToolSpec;

pub const PROVIDER_WAIT_SLICE: Duration = Duration::from_millis(250);
pub const CLASSIFIER_NUM_PREDICT_CAP: usize = 64;
const ABORTED_BY_USER_ERROR: &str = "aborted_by_user: interrupted by user";
const CONTEXT_UNDERCUT_PERCENT: u64 = 70;
const CONTEXT_UNDERCUT_PERSISTENCE: usize = 2;
type ProviderChunkCallback<'a> = dyn FnMut(&str) -> anyhow::Result<()> + 'a;

mod wait_clock;

#[cfg(test)]
mod test_sync;

use wait_clock::WaitClock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderCallScope {
    PlannerUltra,
    PlannerStep,
    Executor,
    Repair,
}

impl ProviderCallScope {
    pub const ALL: [Self; 4] = [
        Self::PlannerUltra,
        Self::PlannerStep,
        Self::Executor,
        Self::Repair,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::PlannerUltra => "planner_ultra",
            Self::PlannerStep => "planner_step",
            Self::Executor => "executor",
            Self::Repair => "repair",
        }
    }

    pub fn timeout_kind(self) -> &'static str {
        match self {
            Self::PlannerUltra => "planner_ultra_timeout",
            Self::PlannerStep => "phase_step_planner_timeout",
            Self::Executor | Self::Repair => "provider_turn_timeout",
        }
    }

    fn is_planner(self) -> bool {
        matches!(self, Self::PlannerUltra | Self::PlannerStep)
    }

    fn renders_stream_chunks(self) -> bool {
        !self.is_planner()
    }

    pub fn screen_label(self) -> &'static str {
        crate::tui::status_bus::provider_scope_label(self.as_str())
    }
}

thread_local! {
    /// The final-repair marker of the session, or of the call, running on this thread (Issue #976).
    static FINAL_REPAIR: std::cell::RefCell<Option<FinalRepairMarker>> =
        const { std::cell::RefCell::new(None) };
}

/// The marker on this thread: on a session's thread its session's, on a call's worker its call's.
pub(crate) fn current_final_repair() -> Option<FinalRepairMarker> {
    FINAL_REPAIR.with(|current| current.borrow().clone())
}

/// Around one session: its marker, or none for a session that is no final repair, is on the
/// thread while it runs, and the one before it is back when it ends.
#[must_use]
pub(crate) struct FinalRepairSession {
    previous: Option<FinalRepairMarker>,
}

impl Drop for FinalRepairSession {
    fn drop(&mut self) {
        let previous = self.previous.take();
        FINAL_REPAIR.with(|current| *current.borrow_mut() = previous);
    }
}

pub(crate) fn enter_final_repair(marker: Option<&FinalRepairMarker>) -> FinalRepairSession {
    FinalRepairSession {
        previous: FINAL_REPAIR.with(|current| current.replace(marker.cloned())),
    }
}

/// Puts the marker of its call on the call's worker thread, which runs that call alone.
fn install_final_repair_on_worker(marker: Option<FinalRepairMarker>) {
    FINAL_REPAIR.with(|current| *current.borrow_mut() = marker);
}

thread_local! {
    /// On a call's worker, the response a wallet settled for that call, once one did (Issue
    /// #976). On every other thread there is no place for one, and nothing is kept.
    static SETTLED: std::cell::Cell<Option<Option<ResponseIdentity>>> =
        const { std::cell::Cell::new(None) };
}

/// The send of the call this thread runs was settled as `response`. A thread that is no call's
/// worker keeps nothing, so no later call can read it.
pub(crate) fn note_settled_response(response: ResponseIdentity) {
    SETTLED.with(|settled| {
        if settled.get().is_some() {
            settled.set(Some(Some(response)));
        }
    });
}

thread_local! {
    /// On a call's worker, the cache write its reply reported, once the reply's reader saw it
    /// (Issue #1006). Apart from the response metadata, so a reply that then fails to parse still
    /// carries it to the turn's event, and a metadata of `None` stays `None`.
    static CACHE_WRITE: std::cell::Cell<Option<Option<CacheWriteObservation>>> =
        const { std::cell::Cell::new(None) };
}

/// The reply of the call this thread runs reported `observation` (or nothing valid). A thread
/// that is no call's worker keeps nothing.
pub(crate) fn note_cache_write(observation: Option<CacheWriteObservation>) {
    CACHE_WRITE.with(|slot| {
        if slot.get().is_some() {
            slot.set(Some(observation));
        }
    });
}

fn open_cache_write_on_worker() {
    CACHE_WRITE.with(|slot| slot.set(Some(None)));
}

fn take_cache_write_on_worker() -> Option<CacheWriteObservation> {
    CACHE_WRITE.with(std::cell::Cell::take).flatten()
}

/// What `run` noted as a call's worker would, for a test of a reader outside a call.
#[cfg(test)]
pub(crate) fn noting_cache_write<R>(run: impl FnOnce() -> R) -> (R, Option<CacheWriteObservation>) {
    open_cache_write_on_worker();
    let result = run();
    (result, take_cache_write_on_worker())
}

/// Gives the call's worker a place for its response, empty.
fn open_settled_on_worker() {
    SETTLED.with(|settled| settled.set(Some(None)));
}

/// What this worker's call settled, taken on the worker that read the reply, with the reply.
fn take_settled_on_worker() -> Option<ResponseIdentity> {
    SETTLED.with(std::cell::Cell::take).flatten()
}

pub struct ProviderCallOutcome {
    pub result: anyhow::Result<AssistantReply>,
    pub elapsed: Duration,
    pub timed_out: bool,
    pub aborted_by_user: bool,
    /// The send this very call's reply came from, when a wallet settled it (Issue #976). Only a
    /// reply that is this call's result has one: a call that gave up, failed, or was cut has none.
    pub(crate) response: Option<ResponseIdentity>,
}

pub struct ProviderChatRequest<'a> {
    pub scope: ProviderCallScope,
    pub model: &'a str,
    pub messages: &'a [ConversationMessage],
    pub tools: &'a [ToolSpec],
    pub native_tools_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderCallOverrides {
    provider: Provider,
    think: Option<OllamaThink>,
    num_predict: usize,
    display_scope: Option<&'static str>,
}

impl ProviderCallOverrides {
    pub fn classifier(config: &Config) -> Self {
        Self {
            provider: config.classifier_provider,
            think: Some(OllamaThink::False),
            num_predict: config.num_predict.min(CLASSIFIER_NUM_PREDICT_CAP),
            display_scope: None,
        }
    }

    pub fn gate_one_classifier(config: &Config) -> Self {
        let mut overrides = Self::classifier(config);
        overrides.display_scope = Some("classifier");
        overrides
    }
}

struct ProviderTurnTelemetry<'a> {
    scope: ProviderCallScope,
    provider: &'a str,
    model: &'a str,
    tool_count: usize,
    native_tools_enabled: bool,
    think: Option<OllamaThink>,
    estimated_prompt_tokens: u64,
    estimated_stable_prefix_chars: u64,
    prompt_eval_count: Option<u64>,
    eval_count: Option<u64>,
    tool_calls_in_response: usize,
    prompt_eval_duration: Option<u64>,
    eval_duration: Option<u64>,
    load_duration: Option<u64>,
    total_duration: Option<u64>,
    prefill_seconds: Option<f64>,
    generation_seconds: Option<f64>,
    load_seconds: Option<f64>,
    tokens_per_second_eval: Option<f64>,
    response_metadata: Option<ProviderResponseMetadata>,
    /// The reply's validated cache write, on its own path (Issue #1006).
    cache_write: Option<CacheWriteObservation>,
    finish_reason: String,
    elapsed: Duration,
    timed_out: bool,
    aborted_by_user: bool,
    ok: bool,
}

struct ProviderTurnTelemetryBase<'a> {
    scope: ProviderCallScope,
    provider: &'a str,
    model: &'a str,
    tool_count: usize,
    native_tools_enabled: bool,
    think: Option<OllamaThink>,
    estimated_prompt_tokens: u64,
    estimated_stable_prefix_chars: u64,
    elapsed: Duration,
    timed_out: bool,
    aborted_by_user: bool,
}

struct ProviderWorkerResult {
    result: anyhow::Result<AssistantReply>,
    timing: Option<ResponseTiming>,
    response_metadata: Option<ProviderResponseMetadata>,
    response: Option<ResponseIdentity>,
    cache_write: Option<CacheWriteObservation>,
}

enum ProviderWorkerMessage {
    Chunk(String),
    Completed(Box<ProviderWorkerResult>),
    Panicked(Box<dyn std::any::Any + Send + 'static>),
}

pub fn chat(
    client: &mut dyn ChatClient,
    config: &Config,
    scope: ProviderCallScope,
    model: &str,
    messages: &[ConversationMessage],
    tools: &[ToolSpec],
    native_tools_enabled: bool,
) -> ProviderCallOutcome {
    chat_with_cancel(
        client,
        config,
        ProviderChatRequest {
            scope,
            model,
            messages,
            tools,
            native_tools_enabled,
        },
        || false,
    )
}

pub fn chat_with_cancel<F>(
    client: &mut dyn ChatClient,
    config: &Config,
    request: ProviderChatRequest<'_>,
    is_cancelled: F,
) -> ProviderCallOutcome
where
    F: Fn() -> bool,
{
    chat_with_cancel_inner(client, config, request, is_cancelled, None, false, None)
}

pub fn chat_with_cancel_and_response_limit<F>(
    client: &mut dyn ChatClient,
    config: &Config,
    request: ProviderChatRequest<'_>,
    is_cancelled: F,
    max_response_bytes: usize,
) -> ProviderCallOutcome
where
    F: Fn() -> bool,
{
    chat_with_cancel_inner(
        client,
        config,
        request,
        is_cancelled,
        Some(max_response_bytes),
        false,
        None,
    )
}

pub fn chat_with_cancel_and_response_limit_and_overrides<F>(
    client: &mut dyn ChatClient,
    config: &Config,
    request: ProviderChatRequest<'_>,
    is_cancelled: F,
    max_response_bytes: usize,
    overrides: ProviderCallOverrides,
) -> ProviderCallOutcome
where
    F: Fn() -> bool,
{
    chat_with_cancel_inner_with_overrides(
        client,
        config,
        request,
        is_cancelled,
        Some(max_response_bytes),
        false,
        None,
        Some(overrides),
    )
}

pub fn chat_with_cancel_and_stream<F>(
    client: &mut dyn ChatClient,
    config: &Config,
    request: ProviderChatRequest<'_>,
    is_cancelled: F,
    on_chunk: &mut ProviderChunkCallback<'_>,
) -> ProviderCallOutcome
where
    F: Fn() -> bool,
{
    chat_with_cancel_inner(
        client,
        config,
        request,
        is_cancelled,
        None,
        config.streaming_enabled(),
        Some(on_chunk),
    )
}

fn chat_with_cancel_inner<F>(
    client: &mut dyn ChatClient,
    config: &Config,
    request: ProviderChatRequest<'_>,
    is_cancelled: F,
    max_response_bytes: Option<usize>,
    stream_allowed: bool,
    on_chunk: Option<&mut ProviderChunkCallback<'_>>,
) -> ProviderCallOutcome
where
    F: Fn() -> bool,
{
    chat_with_cancel_inner_with_overrides(
        client,
        config,
        request,
        is_cancelled,
        max_response_bytes,
        stream_allowed,
        on_chunk,
        None,
    )
}

#[allow(clippy::too_many_arguments, reason = "borrowed call and trace inputs")]
fn chat_with_cancel_inner_with_overrides<F>(
    client: &mut dyn ChatClient,
    config: &Config,
    request: ProviderChatRequest<'_>,
    is_cancelled: F,
    max_response_bytes: Option<usize>,
    stream_allowed: bool,
    mut on_chunk: Option<&mut ProviderChunkCallback<'_>>,
    overrides: Option<ProviderCallOverrides>,
) -> ProviderCallOutcome
where
    F: Fn() -> bool,
{
    let scope = request.scope;
    let model = request.model;
    let messages = request.messages;
    let tools = request.tools;
    let trace_messages = messages;
    let trace_tools = tools;
    let native_tools_enabled = request.native_tools_enabled;
    let redaction = crate::sensitive_data::active_for(config.eval_events_path.as_deref());
    let display_scope = overrides
        .and_then(|overrides| overrides.display_scope)
        .unwrap_or_else(|| scope.as_str());
    let mut worker_client = match worker_client_with_overrides(client, config, model, overrides) {
        Ok(client) => client,
        Err(error) => {
            return ProviderCallOutcome {
                result: Err(error),
                elapsed: Duration::ZERO,
                timed_out: false,
                aborted_by_user: false,
                response: None,
            };
        }
    };
    let provider = worker_client.label().to_string();
    let configured_think = overrides
        .map(|overrides| overrides.think)
        .unwrap_or_else(|| {
            if scope.is_planner() {
                config.planner_think
            } else {
                config.ollama_think
            }
        });
    let think = worker_client
        .supports_ollama_think()
        .then_some(configured_think)
        .flatten();
    let stream = on_chunk.is_some()
        && worker_client.supports_streaming_for_model(model)
        && (scope.is_planner() || (stream_allowed && config.stream));
    let render_stream_chunks = stream && scope.renders_stream_chunks();
    let timeout = Duration::from_secs(config.chat_timeout_secs);
    let estimated_prompt_tokens =
        estimate_prompt_tokens_sent(messages, tools, native_tools_enabled);
    let estimated_stable_prefix_chars = estimate_stable_prefix_chars(
        scope,
        &provider,
        model,
        messages,
        tools,
        native_tools_enabled,
        config.prompt_layout.as_str(),
    );
    crate::tui::status_bus::publish_provider_started(display_scope, config.chat_timeout_secs);
    crate::tui::presentation::emit_provider_turn_started(
        display_scope,
        model,
        config.chat_timeout_secs,
    );
    let started = Instant::now();
    let mut next_progress_at = Duration::from_secs(60);

    if is_cancelled() {
        let outcome = provider_aborted_by_user(
            config,
            ProviderTurnTelemetryBase {
                scope,
                provider: &provider,
                model,
                tool_count: tools.len(),
                native_tools_enabled,
                think,
                estimated_prompt_tokens,
                estimated_stable_prefix_chars,
                elapsed: started.elapsed(),
                timed_out: false,
                aborted_by_user: true,
            },
        );
        record_provider_trace(
            config,
            scope,
            &provider,
            model,
            trace_messages,
            trace_tools,
            native_tools_enabled,
            &outcome.result,
        );
        return outcome;
    }

    let model = model.to_string();
    let tool_count = tools.len();
    let mut messages = messages.to_vec();
    let mut tools = tools.to_vec();
    if let Some(context) = redaction.as_ref() {
        scrub_conversation(context, &mut messages);
        scrub_tool_specs(context, &mut tools);
    }
    if let Err(error) = crate::tools::profile_tools::schema::invocation::check_send(
        &tools,
        overrides.map(|_| worker_client.as_ref()),
    ) {
        return ProviderCallOutcome {
            result: Err(error),
            elapsed: Duration::ZERO,
            timed_out: false,
            aborted_by_user: false,
            response: None,
        };
    }
    let schema_tools = crate::tools::profile_tools::schema::invocation::for_provider_worker();
    let worker_model = model.clone();
    let worker_redaction = redaction.clone();
    // A send of the session's own client is of its final repair, if it is one (Issue #976); the
    // worker keeps this call's marker even after the call gives up on it.
    let worker_final_repair = overrides.is_none().then(current_final_repair).flatten();
    let (tx, rx) = mpsc::sync_channel(1);
    // Fires on every exit of this call (cancel, deadline, completion) so the
    // worker's HTTP request is dropped and its connection closes.
    let cancel_on_exit = CancelOnDrop(CancelToken::default());
    worker_client.set_cancel_token(cancel_on_exit.0.clone());
    std::thread::spawn(move || {
        // Provider worker threads have no thread-local scope; install the
        // captured one so snippets they build are scrubbed before truncation.
        crate::sensitive_data::set_current(worker_redaction);
        crate::tools::profile_tools::schema::invocation::install_on_provider_worker(schema_tools);
        install_final_repair_on_worker(worker_final_repair);
        open_settled_on_worker();
        open_cache_write_on_worker();
        let worker_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let result = if stream {
                worker_client.chat_stream(
                    &worker_model,
                    &messages,
                    &tools,
                    native_tools_enabled,
                    &mut |chunk| {
                        tx.send(ProviderWorkerMessage::Chunk(chunk.to_string()))
                            .map_err(|_| anyhow!("provider stream receiver disconnected"))
                    },
                )
            } else {
                worker_client.chat(&worker_model, &messages, &tools, native_tools_enabled)
            };
            let timing = worker_client.take_response_timing();
            let response_metadata = worker_client.take_response_metadata();
            ProviderWorkerResult {
                result,
                timing,
                response_metadata,
                response: take_settled_on_worker(),
                cache_write: take_cache_write_on_worker(),
            }
        }));
        let message = match worker_result {
            Ok(result) => ProviderWorkerMessage::Completed(Box::new(result)),
            Err(payload) => ProviderWorkerMessage::Panicked(payload),
        };
        let _ = tx.send(message);
    });

    let mut display_scrubber = redaction.as_ref().map(|context| context.stream_scrubber());
    let mut clock = InstantWaitClock { started };
    let mut boundary = ChannelWaitBoundary { receiver: &rx };
    loop {
        let step = wait_clock::wait_step(
            &mut clock,
            &mut boundary,
            timeout,
            PROVIDER_WAIT_SLICE,
            &is_cancelled,
            |elapsed| {
                emit_provider_progress_if_due(
                    elapsed,
                    config.chat_timeout_secs,
                    &mut next_progress_at,
                );
            },
        );
        match step {
            wait_clock::WaitStep::Cancelled => {
                let outcome = provider_aborted_by_user(
                    config,
                    ProviderTurnTelemetryBase {
                        scope,
                        provider: &provider,
                        model: &model,
                        tool_count,
                        native_tools_enabled,
                        think,
                        estimated_prompt_tokens,
                        estimated_stable_prefix_chars,
                        elapsed: clock.elapsed(),
                        timed_out: false,
                        aborted_by_user: true,
                    },
                );
                record_provider_trace(
                    config,
                    scope,
                    &provider,
                    &model,
                    trace_messages,
                    trace_tools,
                    native_tools_enabled,
                    &outcome.result,
                );
                return outcome;
            }
            wait_clock::WaitStep::Deadline => {
                let elapsed = clock.elapsed();
                crate::tui::status_bus::publish_provider_finished(elapsed);
                emit_provider_turn_duration(
                    config,
                    ProviderTurnTelemetry {
                        scope,
                        provider: &provider,
                        model: &model,
                        tool_count,
                        native_tools_enabled,
                        think,
                        estimated_prompt_tokens,
                        estimated_stable_prefix_chars,
                        prompt_eval_count: None,
                        eval_count: None,
                        tool_calls_in_response: 0,
                        prompt_eval_duration: None,
                        eval_duration: None,
                        load_duration: None,
                        total_duration: None,
                        prefill_seconds: None,
                        generation_seconds: None,
                        load_seconds: None,
                        tokens_per_second_eval: None,
                        response_metadata: None,
                        cache_write: None,
                        finish_reason: "timeout".to_string(),
                        elapsed,
                        timed_out: true,
                        aborted_by_user: false,
                        ok: false,
                    },
                );
                if scope.is_planner() {
                    emit_provider_turn_timeout(config, scope, &provider, &model, elapsed);
                }
                let result = Err(anyhow!(
                    "{}: provider call exceeded configured deadline of {}s",
                    scope.timeout_kind(),
                    config.chat_timeout_secs
                ));
                record_provider_trace(
                    config,
                    scope,
                    &provider,
                    &model,
                    trace_messages,
                    trace_tools,
                    native_tools_enabled,
                    &result,
                );
                return ProviderCallOutcome {
                    result,
                    elapsed,
                    timed_out: true,
                    aborted_by_user: false,
                    response: None,
                };
            }
            wait_clock::WaitStep::Message(ProviderWorkerMessage::Chunk(chunk)) => {
                let scrubbed = match display_scrubber.as_mut() {
                    Some(scrubber) => scrubber.push(&chunk),
                    None => chunk,
                };
                if render_stream_chunks
                    && !scrubbed.is_empty()
                    && let Some(on_chunk) = on_chunk.as_deref_mut()
                    && let Err(err) = on_chunk(&scrubbed)
                {
                    let elapsed = clock.elapsed();
                    crate::tui::status_bus::publish_provider_finished(elapsed);
                    let result = Err(anyhow!("failed to render provider stream: {err:#}"));
                    emit_provider_turn_duration(
                        config,
                        provider_turn_telemetry_from_result(
                            ProviderTurnTelemetryBase {
                                scope,
                                provider: &provider,
                                model: &model,
                                tool_count,
                                native_tools_enabled,
                                think,
                                estimated_prompt_tokens,
                                estimated_stable_prefix_chars,
                                elapsed,
                                timed_out: false,
                                aborted_by_user: false,
                            },
                            &result,
                            None,
                            None,
                            None,
                        ),
                    );
                    record_provider_trace(
                        config,
                        scope,
                        &provider,
                        &model,
                        trace_messages,
                        trace_tools,
                        native_tools_enabled,
                        &result,
                    );
                    return ProviderCallOutcome {
                        result,
                        elapsed,
                        timed_out: false,
                        aborted_by_user: false,
                        response: None,
                    };
                }
            }
            wait_clock::WaitStep::Message(ProviderWorkerMessage::Completed(worker_result)) => {
                if render_stream_chunks
                    && let (Some(scrubber), Some(on_chunk)) =
                        (display_scrubber.as_mut(), on_chunk.as_deref_mut())
                {
                    let tail = scrubber.finish();
                    if !tail.is_empty() {
                        let _ = on_chunk(&tail);
                    }
                }
                let ProviderWorkerResult {
                    result,
                    timing,
                    response_metadata,
                    response,
                    cache_write,
                } = *worker_result;
                let result = enforce_response_limit(result, max_response_bytes);
                let response = response.filter(|_| result.is_ok());
                let elapsed = clock.elapsed();
                crate::tui::status_bus::publish_provider_finished(elapsed);
                if result.is_ok() {
                    crate::tui::presentation::emit_provider_turn_completed(
                        display_scope,
                        elapsed.as_secs(),
                    );
                }
                emit_provider_turn_duration(
                    config,
                    provider_turn_telemetry_from_result(
                        ProviderTurnTelemetryBase {
                            scope,
                            provider: &provider,
                            model: &model,
                            tool_count,
                            native_tools_enabled,
                            think,
                            estimated_prompt_tokens,
                            estimated_stable_prefix_chars,
                            elapsed,
                            timed_out: false,
                            aborted_by_user: false,
                        },
                        &result,
                        timing,
                        response_metadata,
                        cache_write,
                    ),
                );
                client.provider_turn_carried_result();
                record_provider_trace(
                    config,
                    scope,
                    &provider,
                    &model,
                    trace_messages,
                    trace_tools,
                    native_tools_enabled,
                    &result,
                );
                return ProviderCallOutcome {
                    result,
                    elapsed,
                    timed_out: false,
                    aborted_by_user: false,
                    response,
                };
            }
            wait_clock::WaitStep::Message(ProviderWorkerMessage::Panicked(payload)) => {
                let elapsed = clock.elapsed();
                crate::tui::status_bus::publish_provider_finished(elapsed);
                emit_provider_turn_duration(
                    config,
                    ProviderTurnTelemetry {
                        scope,
                        provider: &provider,
                        model: &model,
                        tool_count,
                        native_tools_enabled,
                        think,
                        estimated_prompt_tokens,
                        estimated_stable_prefix_chars,
                        prompt_eval_count: None,
                        eval_count: None,
                        tool_calls_in_response: 0,
                        prompt_eval_duration: None,
                        eval_duration: None,
                        load_duration: None,
                        total_duration: None,
                        prefill_seconds: None,
                        generation_seconds: None,
                        load_seconds: None,
                        tokens_per_second_eval: None,
                        response_metadata: None,
                        cache_write: None,
                        finish_reason: "panic".to_string(),
                        elapsed,
                        timed_out: false,
                        aborted_by_user: false,
                        ok: false,
                    },
                );
                let message = payload
                    .downcast_ref::<&str>()
                    .copied()
                    .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
                    .unwrap_or("non-string panic payload");
                let result = Err(anyhow!("provider call panicked: {message}"));
                record_provider_trace(
                    config,
                    scope,
                    &provider,
                    &model,
                    trace_messages,
                    trace_tools,
                    native_tools_enabled,
                    &result,
                );
                std::panic::resume_unwind(payload);
            }
            wait_clock::WaitStep::Sliced => continue,
            wait_clock::WaitStep::Disconnected => {
                let elapsed = clock.elapsed();
                crate::tui::status_bus::publish_provider_finished(elapsed);
                emit_provider_turn_duration(
                    config,
                    ProviderTurnTelemetry {
                        scope,
                        provider: &provider,
                        model: &model,
                        tool_count,
                        native_tools_enabled,
                        think,
                        estimated_prompt_tokens,
                        estimated_stable_prefix_chars,
                        prompt_eval_count: None,
                        eval_count: None,
                        tool_calls_in_response: 0,
                        prompt_eval_duration: None,
                        eval_duration: None,
                        load_duration: None,
                        total_duration: None,
                        prefill_seconds: None,
                        generation_seconds: None,
                        load_seconds: None,
                        tokens_per_second_eval: None,
                        response_metadata: None,
                        cache_write: None,
                        finish_reason: "error".to_string(),
                        elapsed,
                        timed_out: false,
                        aborted_by_user: false,
                        ok: false,
                    },
                );
                let result = Err(anyhow!("provider call worker disconnected"));
                record_provider_trace(
                    config,
                    scope,
                    &provider,
                    &model,
                    trace_messages,
                    trace_tools,
                    native_tools_enabled,
                    &result,
                );
                return ProviderCallOutcome {
                    result,
                    elapsed,
                    timed_out: false,
                    aborted_by_user: false,
                    response: None,
                };
            }
        }
    }
}

/// Scrub the outgoing conversation copy so a registered secret never reaches a
/// provider request body. The caller's original messages are untouched. A tool
/// protocol name is a fixed identifier and is preserved; only free fields and
/// the model-derived ids are scrubbed.
fn scrub_conversation(
    context: &crate::sensitive_data::RedactionContext,
    messages: &mut [ConversationMessage],
) {
    for message in messages {
        message.content = context.scrub_text(&message.content);
        if let Some(tool_call_id) = message.tool_call_id.as_mut() {
            *tool_call_id = context.scrub_text(tool_call_id);
        }
        for call in &mut message.tool_calls {
            call.id = context.scrub_text(&call.id);
            context.scrub_value_free(&mut call.arguments);
        }
    }
}

/// Scrub the tool schema copy sent with a provider request. `kind` and
/// `function.name` are fixed protocol identifiers and are preserved; the
/// parameter schema is arbitrary input, so it is scrubbed as free input.
fn scrub_tool_specs(context: &crate::sensitive_data::RedactionContext, tools: &mut [ToolSpec]) {
    for tool in tools {
        tool.function.description = context.scrub_text(&tool.function.description);
        context.scrub_value_free(&mut tool.function.parameters);
    }
}

#[allow(clippy::too_many_arguments, reason = "borrowed call and trace inputs")]
fn record_provider_trace(
    config: &Config,
    scope: ProviderCallScope,
    provider: &str,
    model: &str,
    messages: &[ConversationMessage],
    tools: &[ToolSpec],
    native_tools_enabled: bool,
    result: &anyhow::Result<AssistantReply>,
) {
    if let Err(error) = crate::run_trace::record_provider_exchange(
        config.eval_events_path.as_deref(),
        scope.as_str(),
        provider,
        model,
        messages,
        tools,
        native_tools_enabled,
        result,
    ) {
        eprintln!("warning: failed to write provider trace: {error:#}");
    }
}

fn worker_client_with_overrides(
    client: &dyn ChatClient,
    config: &Config,
    model: &str,
    overrides: Option<ProviderCallOverrides>,
) -> anyhow::Result<Box<dyn ChatClient>> {
    let Some(overrides) = overrides else {
        return Ok(client.boxed_clone());
    };
    if client.label() != config.provider_label(crate::config::ProviderRole::Planner) {
        return Ok(client.boxed_clone());
    }

    let mut call_config = config.clone();
    call_config.classifier_provider = overrides.provider;
    call_config.classifier_model = model.to_string();
    call_config.ollama_think = overrides.think;
    call_config.num_predict = overrides.num_predict;
    crate::providers::client_from_config_for_role(
        &call_config,
        crate::config::ProviderRole::Classifier,
    )
}

fn enforce_response_limit(
    result: anyhow::Result<AssistantReply>,
    max_response_bytes: Option<usize>,
) -> anyhow::Result<AssistantReply> {
    let reply = result?;
    if let Some(limit) = max_response_bytes
        && reply.content.len() > limit
    {
        return Err(anyhow!(
            "provider_response_limit: response was {} bytes; maximum is {limit} bytes",
            reply.content.len()
        ));
    }
    Ok(reply)
}

pub fn is_aborted_by_user(error: &str) -> bool {
    error.contains("aborted_by_user") || error.contains("interrupted by user")
}

pub fn is_scoped_timeout(scope: ProviderCallScope, error: &str) -> bool {
    error.contains(scope.timeout_kind())
}

struct CancelOnDrop(CancelToken);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

/// Production wait clock: the real monotonic `Instant` captured at call start.
struct InstantWaitClock {
    started: Instant,
}

impl wait_clock::WaitClock for InstantWaitClock {
    fn elapsed(&mut self) -> Duration {
        self.started.elapsed()
    }
}

/// Production wait boundary: a blocking, bounded receive from the worker
/// channel. It preserves the distinction between an elapsed slice and a
/// disconnected worker so the loop cannot spin on a dead sender.
struct ChannelWaitBoundary<'a> {
    receiver: &'a mpsc::Receiver<ProviderWorkerMessage>,
}

impl wait_clock::WaitBoundary<ProviderWorkerMessage> for ChannelWaitBoundary<'_> {
    fn wait(&mut self, slice: Duration) -> wait_clock::WaitSignal<ProviderWorkerMessage> {
        match self.receiver.recv_timeout(slice) {
            Ok(message) => wait_clock::WaitSignal::Ready(message),
            Err(mpsc::RecvTimeoutError::Timeout) => wait_clock::WaitSignal::Elapsed,
            Err(mpsc::RecvTimeoutError::Disconnected) => wait_clock::WaitSignal::Disconnected,
        }
    }
}

fn emit_provider_progress_if_due(
    elapsed: Duration,
    deadline_secs: u64,
    next_progress_at: &mut Duration,
) {
    if let Some(elapsed_secs) = provider_progress_due(elapsed, next_progress_at) {
        crate::tui::presentation::emit_provider_turn_progress(elapsed_secs, deadline_secs);
    }
}

fn provider_progress_due(elapsed: Duration, next_progress_at: &mut Duration) -> Option<u64> {
    if elapsed < *next_progress_at {
        return None;
    }
    while elapsed >= *next_progress_at {
        *next_progress_at += Duration::from_secs(60);
    }
    Some(elapsed.as_secs())
}

fn estimate_stable_prefix_chars(
    scope: ProviderCallScope,
    provider: &str,
    model: &str,
    messages: &[ConversationMessage],
    tools: &[ToolSpec],
    native_tools_enabled: bool,
    prompt_layout: &str,
) -> u64 {
    let prompt = prompt_prefix_text(messages, tools, native_tools_enabled);
    let key = format!(
        "{}:{}:{}:{}:{}:{}",
        provider,
        model,
        scope.as_str(),
        prompt_layout,
        native_tools_enabled,
        tools.len()
    );
    let mut cache = prompt_prefix_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let prefix = cache
        .get(&key)
        .map(|previous| longest_common_prefix_chars(previous, &prompt))
        .unwrap_or(0);
    cache.insert(key, prompt);
    prefix as u64
}

fn prompt_prefix_text(
    messages: &[ConversationMessage],
    tools: &[ToolSpec],
    native_tools_enabled: bool,
) -> String {
    let mut text = String::new();
    for message in messages {
        text.push_str("role:");
        text.push_str(&message.role);
        text.push('\n');
        if let Some(name) = &message.name {
            text.push_str("name:");
            text.push_str(name);
            text.push('\n');
        }
        text.push_str("content:\n");
        text.push_str(&message.content);
        text.push('\n');
    }
    if native_tools_enabled {
        text.push_str("tools:\n");
        if let Ok(value) = serde_json::to_string(tools) {
            text.push_str(&value);
        }
    }
    text
}

fn longest_common_prefix_chars(left: &str, right: &str) -> usize {
    left.chars()
        .zip(right.chars())
        .take_while(|(left, right)| left == right)
        .count()
}

fn prompt_prefix_cache() -> &'static Mutex<BTreeMap<String, String>> {
    static CACHE: OnceLock<Mutex<BTreeMap<String, String>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn estimate_prompt_tokens_sent(
    messages: &[ConversationMessage],
    tools: &[ToolSpec],
    native_tools_enabled: bool,
) -> u64 {
    let mut chars = 0usize;
    for message in messages {
        chars = chars
            .saturating_add(message.role.chars().count())
            .saturating_add(message.name.as_deref().unwrap_or("").chars().count())
            .saturating_add(message.content.chars().count())
            .saturating_add(8);
    }
    if native_tools_enabled {
        chars = chars.saturating_add(
            serde_json::to_string(tools)
                .map(|value| value.chars().count())
                .unwrap_or_default(),
        );
    }
    (chars.saturating_add(3) / 4).max(1) as u64
}

fn provider_aborted_by_user(
    config: &Config,
    base: ProviderTurnTelemetryBase<'_>,
) -> ProviderCallOutcome {
    let elapsed = base.elapsed;
    crate::tui::status_bus::publish_provider_finished(elapsed);
    emit_provider_turn_duration(
        config,
        ProviderTurnTelemetry {
            scope: base.scope,
            provider: base.provider,
            model: base.model,
            tool_count: base.tool_count,
            native_tools_enabled: base.native_tools_enabled,
            think: base.think,
            estimated_prompt_tokens: base.estimated_prompt_tokens,
            estimated_stable_prefix_chars: base.estimated_stable_prefix_chars,
            prompt_eval_count: None,
            eval_count: None,
            tool_calls_in_response: 0,
            prompt_eval_duration: None,
            eval_duration: None,
            load_duration: None,
            total_duration: None,
            prefill_seconds: None,
            generation_seconds: None,
            load_seconds: None,
            tokens_per_second_eval: None,
            response_metadata: None,
            cache_write: None,
            finish_reason: "aborted_by_user".to_string(),
            elapsed,
            timed_out: false,
            aborted_by_user: true,
            ok: false,
        },
    );
    emit_provider_turn_aborted_by_user(config, base.scope, base.provider, base.model, elapsed);
    ProviderCallOutcome {
        result: Err(anyhow!(ABORTED_BY_USER_ERROR)),
        elapsed,
        timed_out: false,
        aborted_by_user: true,
        response: None,
    }
}

fn provider_turn_telemetry_from_result<'a>(
    base: ProviderTurnTelemetryBase<'a>,
    result: &anyhow::Result<AssistantReply>,
    timing: Option<ResponseTiming>,
    response_metadata: Option<ProviderResponseMetadata>,
    cache_write: Option<CacheWriteObservation>,
) -> ProviderTurnTelemetry<'a> {
    let reply = result.as_ref().ok();
    let prompt_eval_duration = timing
        .as_ref()
        .and_then(|timing| timing.prompt_eval_duration);
    let eval_duration = timing.as_ref().and_then(|timing| timing.eval_duration);
    let load_duration = timing.as_ref().and_then(|timing| timing.load_duration);
    let total_duration = timing.as_ref().and_then(|timing| timing.total_duration);
    let prefill_seconds = prompt_eval_duration.map(duration_seconds_from_ollama);
    let generation_seconds = eval_duration.map(duration_seconds_from_ollama);
    let load_seconds = load_duration.map(duration_seconds_from_ollama);
    let tokens_per_second_eval = match (
        reply.and_then(|reply| reply.completion_tokens),
        generation_seconds,
    ) {
        (Some(tokens), Some(seconds)) if seconds > 0.0 => Some(tokens as f64 / seconds),
        _ => None,
    };
    ProviderTurnTelemetry {
        scope: base.scope,
        provider: base.provider,
        model: base.model,
        tool_count: base.tool_count,
        native_tools_enabled: base.native_tools_enabled,
        think: base.think,
        estimated_prompt_tokens: base.estimated_prompt_tokens,
        estimated_stable_prefix_chars: base.estimated_stable_prefix_chars,
        prompt_eval_count: reply.and_then(|reply| reply.prompt_tokens),
        eval_count: reply.and_then(|reply| reply.completion_tokens),
        tool_calls_in_response: reply.map_or(0, |reply| reply.tool_calls.len()),
        prompt_eval_duration,
        eval_duration,
        load_duration,
        total_duration,
        prefill_seconds,
        generation_seconds,
        load_seconds,
        tokens_per_second_eval,
        response_metadata,
        cache_write,
        finish_reason: if base.timed_out {
            "timeout".to_string()
        } else if result.is_ok() {
            "stop".to_string()
        } else {
            "error".to_string()
        },
        elapsed: base.elapsed,
        timed_out: base.timed_out,
        aborted_by_user: base.aborted_by_user,
        ok: result.is_ok(),
    }
}

fn duration_seconds_from_ollama(value: u64) -> f64 {
    value as f64 / 1_000_000_000.0
}

fn emit_provider_turn_duration(config: &Config, telemetry: ProviderTurnTelemetry<'_>) {
    let mut value = json!({
        "event": "provider_turn_duration",
        "caller_scope": telemetry.scope.as_str(),
        "provider": telemetry.provider,
        "model": eval_events::body_snippet(telemetry.model),
        "think": telemetry.think.map(OllamaThink::as_str).unwrap_or("omitted"),
        "duration_ms": telemetry.elapsed.as_millis().min(u128::from(u64::MAX)) as u64,
        "timeout_ms": Duration::from_secs(config.chat_timeout_secs).as_millis().min(u128::from(u64::MAX)) as u64,
        "timeout_secs": config.chat_timeout_secs,
        "timeout_source": config.chat_timeout_source,
        "prompt_layout": config.prompt_layout.as_str(),
        "timed_out": telemetry.timed_out,
        "aborted_by_user": telemetry.aborted_by_user,
        "ok": telemetry.ok,
        "tools": telemetry.tool_count,
        "native_tools_enabled": telemetry.native_tools_enabled,
        "estimated_prompt_tokens_sent": telemetry.estimated_prompt_tokens,
        "estimated_stable_prefix_chars": telemetry.estimated_stable_prefix_chars,
        "prompt_eval_count": telemetry.prompt_eval_count,
        "eval_count": telemetry.eval_count,
        "tool_calls_in_response": telemetry.tool_calls_in_response,
        "write_or_edit_succeeded": false,
        "prompt_eval_duration": telemetry.prompt_eval_duration,
        "eval_duration": telemetry.eval_duration,
        "load_duration": telemetry.load_duration,
        "total_duration": telemetry.total_duration,
        "prefill_seconds": telemetry.prefill_seconds,
        "generation_seconds": telemetry.generation_seconds,
        "load_seconds": telemetry.load_seconds,
        "tokens_per_second_eval": telemetry.tokens_per_second_eval,
        "finish_reason": telemetry.finish_reason.as_str(),
    });
    if telemetry.aborted_by_user {
        value["classification"] = json!("aborted_by_user");
    }
    if let Some(metadata) = &telemetry.response_metadata {
        value["provider_response_id"] = json!(metadata.response_id);
        value["provider_model_id"] = json!(metadata.model_id);
        value["system_fingerprint"] = json!(metadata.system_fingerprint);
        value["provider_created_epoch"] = json!(metadata.created_epoch);
        value["provider_service_tier"] = json!(metadata.service_tier);
        if metadata.cached_input_tokens.is_some() {
            value["provider_cached_input_tokens"] = json!(metadata.cached_input_tokens);
        }
        if metadata.reasoning_tokens.is_some() {
            value["provider_reasoning_tokens"] = json!(metadata.reasoning_tokens);
        }
        if metadata.total_tokens.is_some() {
            value["provider_total_tokens"] = json!(metadata.total_tokens);
        }
        // Issue #947: the provider-reported billed amount, additive and only
        // when reported, as a JSON string of the decimal text the provider
        // sent (e.g. "0.012345"); an aggregator reads it as a decimal, never
        // a float. The same turn's provider_response repeats it, so an
        // aggregator counts it once per turn from this event and never adds
        // the two. It is never a price computed from token counts. An amount
        // that overlaps a registered secret is left out (unreported) rather
        // than scrubbed into a marker.
        if let Some(cost) = metadata
            .billed_cost_usd
            .as_ref()
            .and_then(|cost| cost.event_value(config.eval_events_path.as_deref()))
        {
            value["provider_billed_cost_usd"] = cost;
        }
    }
    // Issue #1006: additive and only when the reply's reader obtained and validated it, on a turn
    // that failed to parse too. The same response's provider_response repeats it; count it once
    // per turn, from this event.
    crate::providers::usage_observation::add_event_fields(
        &mut value,
        telemetry.cache_write.as_ref(),
    );
    eval_events::emit(config.eval_events_path.as_deref(), value);
    maybe_emit_context_truncation_warning(config, &telemetry);
}

fn maybe_emit_context_truncation_warning(config: &Config, telemetry: &ProviderTurnTelemetry<'_>) {
    let Some(prompt_eval_count) = telemetry.prompt_eval_count else {
        return;
    };
    let undercut = prompt_eval_count.saturating_mul(100)
        < telemetry
            .estimated_prompt_tokens
            .saturating_mul(CONTEXT_UNDERCUT_PERCENT);
    let key = format!(
        "{}:{}:{}",
        telemetry.provider,
        telemetry.model,
        telemetry.scope.as_str()
    );
    let count = {
        let mut counts = context_under_cut_counts()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let count = counts.entry(key).or_default();
        if undercut {
            *count += 1;
        } else {
            *count = 0;
        }
        *count
    };
    if undercut && count >= CONTEXT_UNDERCUT_PERSISTENCE {
        eval_events::emit(
            config.eval_events_path.as_deref(),
            json!({
                "event": "context_truncation_suspected",
                "level": "WARNING",
                "caller_scope": telemetry.scope.as_str(),
                "provider": telemetry.provider,
                "model": eval_events::body_snippet(telemetry.model),
                "estimated_prompt_tokens_sent": telemetry.estimated_prompt_tokens,
                "prompt_eval_count": prompt_eval_count,
                "eval_count": telemetry.eval_count,
                "finish_reason": telemetry.finish_reason.as_str(),
                "persistent_undercut_count": count,
                "threshold_percent": CONTEXT_UNDERCUT_PERCENT,
            }),
        );
    }
}

fn context_under_cut_counts() -> &'static Mutex<BTreeMap<String, usize>> {
    static COUNTS: OnceLock<Mutex<BTreeMap<String, usize>>> = OnceLock::new();
    COUNTS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn emit_provider_turn_timeout(
    config: &Config,
    scope: ProviderCallScope,
    provider: &str,
    model: &str,
    elapsed: Duration,
) {
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
        "event": "provider_turn_timeout",
        "caller_scope": scope.as_str(),
        "provider": provider,
        "model": eval_events::body_snippet(model),
        "classification": scope.timeout_kind(),
        "duration_ms": elapsed.as_millis().min(u128::from(u64::MAX)) as u64,
            "timeout_secs": config.chat_timeout_secs,
            "timeout_source": config.chat_timeout_source,
            "prompt_layout": config.prompt_layout.as_str(),
        }),
    );
}

fn emit_provider_turn_aborted_by_user(
    config: &Config,
    scope: ProviderCallScope,
    provider: &str,
    model: &str,
    elapsed: Duration,
) {
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "provider_turn_aborted_by_user",
            "caller_scope": scope.as_str(),
            "provider": provider,
            "model": eval_events::body_snippet(model),
            "classification": "aborted_by_user",
            "duration_ms": elapsed.as_millis().min(u128::from(u64::MAX)) as u64,
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Action, Provider};
    use std::io::{Read as _, Write as _};
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    #[derive(Clone)]
    struct TokenClient;

    impl ChatClient for TokenClient {
        fn label(&self) -> &str {
            "token-mock"
        }

        fn boxed_clone(&self) -> Box<dyn ChatClient> {
            Box::new(self.clone())
        }

        fn chat(
            &mut self,
            _model: &str,
            _messages: &[ConversationMessage],
            _tools: &[ToolSpec],
            _native_tools_enabled: bool,
        ) -> anyhow::Result<AssistantReply> {
            Ok(AssistantReply {
                content: "ok".to_string(),
                tool_calls: Vec::new(),
                prompt_tokens: Some(10),
                completion_tokens: Some(2),
            })
        }
    }

    #[derive(Clone)]
    struct TimingClient;

    impl ChatClient for TimingClient {
        fn label(&self) -> &str {
            "timing-mock"
        }

        fn boxed_clone(&self) -> Box<dyn ChatClient> {
            Box::new(self.clone())
        }

        fn chat(
            &mut self,
            _model: &str,
            _messages: &[ConversationMessage],
            _tools: &[ToolSpec],
            _native_tools_enabled: bool,
        ) -> anyhow::Result<AssistantReply> {
            Ok(AssistantReply {
                content: "ok".to_string(),
                tool_calls: Vec::new(),
                prompt_tokens: Some(16),
                completion_tokens: Some(4),
            })
        }

        fn take_response_timing(&mut self) -> Option<ResponseTiming> {
            Some(ResponseTiming {
                prompt_eval_duration: Some(4_000_000_000),
                eval_duration: Some(2_000_000_000),
                load_duration: Some(1_000_000_000),
                total_duration: Some(7_000_000_000),
            })
        }
    }

    #[derive(Clone)]
    struct PanicClient;

    impl ChatClient for PanicClient {
        fn label(&self) -> &str {
            "panic-mock"
        }

        fn boxed_clone(&self) -> Box<dyn ChatClient> {
            Box::new(self.clone())
        }

        fn chat(
            &mut self,
            _model: &str,
            _messages: &[ConversationMessage],
            _tools: &[ToolSpec],
            _native_tools_enabled: bool,
        ) -> anyhow::Result<AssistantReply> {
            panic!("provider worker panic")
        }
    }

    #[derive(Clone)]
    struct StreamingClient {
        chat_calls: Arc<AtomicUsize>,
        stream_calls: Arc<AtomicUsize>,
    }

    impl StreamingClient {
        fn new() -> Self {
            Self {
                chat_calls: Arc::new(AtomicUsize::new(0)),
                stream_calls: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    impl ChatClient for StreamingClient {
        fn label(&self) -> &str {
            "streaming-mock"
        }

        fn boxed_clone(&self) -> Box<dyn ChatClient> {
            Box::new(self.clone())
        }

        fn supports_streaming(&self) -> bool {
            true
        }

        fn chat_stream(
            &mut self,
            _model: &str,
            _messages: &[ConversationMessage],
            _tools: &[ToolSpec],
            _native_tools_enabled: bool,
            on_chunk: &mut dyn FnMut(&str) -> anyhow::Result<()>,
        ) -> anyhow::Result<AssistantReply> {
            self.stream_calls.fetch_add(1, Ordering::SeqCst);
            on_chunk("hel")?;
            on_chunk("lo")?;
            Ok(AssistantReply::text("hello"))
        }

        fn chat(
            &mut self,
            _model: &str,
            _messages: &[ConversationMessage],
            _tools: &[ToolSpec],
            _native_tools_enabled: bool,
        ) -> anyhow::Result<AssistantReply> {
            self.chat_calls.fetch_add(1, Ordering::SeqCst);
            Ok(AssistantReply::text("hello"))
        }
    }

    #[test]
    fn planner_scope_timeout_kinds_are_stable() {
        assert_eq!(
            ProviderCallScope::PlannerStep.timeout_kind(),
            "phase_step_planner_timeout"
        );
        assert_eq!(
            ProviderCallScope::PlannerUltra.timeout_kind(),
            "planner_ultra_timeout"
        );
    }

    pub(super) fn test_config(
        root: &std::path::Path,
        events_path: PathBuf,
        chat_timeout_secs: u64,
    ) -> Config {
        Config {
            workspace_root: root.to_path_buf(),
            state_dir: PathBuf::from("state"),
            yes: true,
            offline: false,
            context_budget: 1000,
            model: "m".to_string(),
            provider: Provider::Ollama,
            tool_protocol: None,
            openai_api: crate::config::OpenAiApi::ChatCompletions,
            prompt_layout: crate::config::PromptLayout::Stable,
            plan_preset: crate::config::PlanPreset::None,
            intent_override: None,
            planner_model: "m".to_string(),
            planner_provider: Provider::Ollama,
            planner_think: Some(crate::config::OllamaThink::False),
            classifier_model: "m".to_string(),
            classifier_provider: Provider::Ollama,
            openai_compatible: None,
            ollama_host: "http://localhost:11434".to_string(),
            ollama_think: None,
            lm_studio_host: "http://localhost:1234".to_string(),
            num_predict: 100,
            max_iterations: 1,
            recovery_plan_auto_runs: 0,
            chat_timeout_secs,
            chat_timeout_source: "override:test".to_string(),
            field_sources: crate::config::ConfigFieldSources::default(),
            chat_retries: 1,
            stream: false,
            eval_events_path: Some(events_path),
            completion_contract_path: None,
            resume: None,
            fresh_session: false,
            no_footer: false,
            narration: crate::config::NarrationMode::Normal,
            profile: "generic".to_string(),
            profile_explicit: false,
            profile_inference: None,
            style: "default".to_string(),
            action: Action::Repl,
        }
    }

    fn serve_ollama_chat_once(listener: TcpListener) -> std::thread::JoinHandle<serde_json::Value> {
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut request = [0_u8; 16_384];
            let read = stream.read(&mut request).expect("request");
            let request = String::from_utf8_lossy(&request[..read]);
            assert!(request.starts_with("POST /api/chat "), "{request}");
            let body = request
                .split_once("\r\n\r\n")
                .map(|(_, body)| body)
                .expect("HTTP request body");
            let request_body = serde_json::from_str(body).expect("Ollama request JSON");
            let response = r#"{"model":"fixture","message":{"role":"assistant","content":"ok"},"done":true,"prompt_eval_count":4,"eval_count":1}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response.len(),
                response
            )
            .expect("response");
            request_body
        })
    }

    fn provider_turn(events_path: &std::path::Path) -> serde_json::Value {
        std::fs::read_to_string(events_path)
            .expect("events")
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .find(|event| event["event"] == "provider_turn_duration")
            .expect("provider turn duration")
    }

    #[test]
    fn planner_ollama_call_uses_role_think_and_records_evidence() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let events_path = tmp.path().join("events.jsonl");
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address");
        let server = serve_ollama_chat_once(listener);
        let mut config = test_config(tmp.path(), events_path.clone(), 2);
        config.ollama_host = format!("http://{address}");
        config.ollama_think = None;
        config.planner_think = Some(OllamaThink::False);
        let mut client = crate::providers::client_from_config(&config, true).expect("planner");

        let outcome = chat(
            client.as_mut(),
            &config,
            ProviderCallScope::PlannerStep,
            &config.planner_model,
            &[ConversationMessage::user("plan")],
            &[],
            false,
        );

        assert_eq!(outcome.result.expect("reply").content, "ok");
        let request = server.join().expect("server");
        assert_eq!(request["think"], false);
        assert_eq!(provider_turn(&events_path)["think"], "false");
    }

    #[test]
    fn executor_ollama_call_keeps_omitted_think_default() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let events_path = tmp.path().join("events.jsonl");
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address");
        let server = serve_ollama_chat_once(listener);
        let mut config = test_config(tmp.path(), events_path.clone(), 2);
        config.ollama_host = format!("http://{address}");
        config.ollama_think = None;
        let mut client = crate::providers::client_from_config(&config, false).expect("executor");

        let outcome = chat(
            client.as_mut(),
            &config,
            ProviderCallScope::Executor,
            &config.model,
            &[ConversationMessage::user("execute")],
            &[],
            false,
        );

        assert_eq!(outcome.result.expect("reply").content, "ok");
        let request = server.join().expect("server");
        assert!(request.get("think").is_none(), "{request}");
        assert_eq!(provider_turn(&events_path)["think"], "omitted");
    }

    #[test]
    fn classifier_ollama_call_forces_think_false_and_caps_num_predict() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let events_path = tmp.path().join("events.jsonl");
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address");
        let server = serve_ollama_chat_once(listener);
        let mut config = test_config(tmp.path(), events_path.clone(), 2);
        config.ollama_host = format!("http://{address}");
        config.num_predict = 8_192;
        config.planner_think = Some(OllamaThink::High);
        config.classifier_model = "classifier".to_string();
        let mut planner = crate::providers::client_from_config(&config, true).expect("planner");

        let outcome = chat_with_cancel_and_response_limit_and_overrides(
            planner.as_mut(),
            &config,
            ProviderChatRequest {
                scope: ProviderCallScope::PlannerStep,
                model: &config.classifier_model,
                messages: &[ConversationMessage::user("classify")],
                tools: &[],
                native_tools_enabled: false,
            },
            || false,
            512,
            ProviderCallOverrides::classifier(&config),
        );

        assert_eq!(outcome.result.expect("reply").content, "ok");
        let request = server.join().expect("server");
        assert_eq!(request["model"], "classifier");
        assert_eq!(request["think"], false);
        assert_eq!(
            request["options"]["num_predict"],
            CLASSIFIER_NUM_PREDICT_CAP
        );
        assert_eq!(provider_turn(&events_path)["think"], "false");
    }

    #[test]
    fn openai_error_redacts_key_across_chokepoint_outputs() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let events_path = tmp.path().join("events.jsonl");
        let mut config = test_config(tmp.path(), events_path.clone(), 2);
        config.provider = Provider::Openai;
        config.model = "gpt-5.6-luna".to_string();
        let secret = "sk-proj-deliberate-redaction-secret-123456789";
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address");
        let secret_for_server = secret.to_string();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut request = [0_u8; 8192];
            let read = stream.read(&mut request).expect("request");
            let request = String::from_utf8_lossy(&request[..read]);
            assert!(request.starts_with("POST /v1/chat/completions "));
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains(&format!("authorization: bearer {secret_for_server}"))
            );
            let body = format!(r#"{{"error":"reflected {secret_for_server}"}}"#);
            write!(
                stream,
                "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .expect("response");
        });
        let mut client = crate::providers::openai::OpenAiClient::for_test(
            secret,
            format!("http://{address}"),
            Some(events_path.clone()),
        );

        let outcome = chat(
            &mut client,
            &config,
            ProviderCallScope::Executor,
            &config.model,
            &[ConversationMessage::user("hello")],
            &[],
            false,
        );
        let error = outcome.result.unwrap_err().to_string();
        crate::eval_events::write_run_summary(Some(&events_path), &error);
        server.join().expect("server");

        let outputs = format!(
            "{error}\n{}\n{}\n{client:?}",
            std::fs::read_to_string(&events_path).expect("events"),
            std::fs::read_to_string(tmp.path().join("summary.md")).expect("summary")
        );
        assert!(!outputs.contains(secret), "secret leaked: {outputs}");
        assert!(outputs.contains("<redacted>"), "{outputs}");
    }

    #[test]
    fn openai_response_metadata_reaches_provider_turn_event() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let events_path = tmp.path().join("events.jsonl");
        let mut config = test_config(tmp.path(), events_path.clone(), 2);
        config.provider = Provider::Openai;
        config.model = "gpt-5.6-luna".to_string();
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut request = [0_u8; 8192];
            let _ = stream.read(&mut request).expect("request");
            let body = r#"{"id":"chatcmpl-f0","model":"gpt-5.6-luna-2026-07-31","created":1785456000,"system_fingerprint":"fp_f0","service_tier":"default","choices":[{"message":{"content":"hello"}}],"usage":{"prompt_tokens":3,"completion_tokens":1}}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .expect("response");
        });
        let mut client = crate::providers::openai::OpenAiClient::for_test(
            "sk-test-only-not-real-123456789",
            format!("http://{address}"),
            Some(events_path.clone()),
        );

        let outcome = chat(
            &mut client,
            &config,
            ProviderCallScope::Executor,
            &config.model,
            &[ConversationMessage::user("hello")],
            &[],
            false,
        );
        server.join().expect("server");
        assert_eq!(outcome.result.unwrap().content, "hello");
        let events = std::fs::read_to_string(events_path).expect("events");
        let turn = events
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .find(|event| event["event"] == "provider_turn_duration")
            .expect("provider turn event");
        assert_eq!(turn["provider_response_id"], "chatcmpl-f0");
        assert_eq!(turn["provider_model_id"], "gpt-5.6-luna-2026-07-31");
        assert_eq!(turn["system_fingerprint"], "fp_f0");
        assert_eq!(turn["provider_created_epoch"], 1_785_456_000_i64);
        assert_eq!(turn["provider_service_tier"], "default");
    }

    #[test]
    fn openai_responses_reasoning_usage_reaches_provider_turn_event() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let events_path = tmp.path().join("events.jsonl");
        let mut config = test_config(tmp.path(), events_path.clone(), 2);
        config.provider = Provider::Openai;
        config.openai_api = crate::config::OpenAiApi::Responses;
        config.model = "gpt-5.6-luna".to_string();
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut request = [0_u8; 8192];
            let _ = stream.read(&mut request).expect("request");
            let body = r#"{"id":"resp-f0b","model":"gpt-5.6-luna-2026-08-01","created_at":1785542400,"service_tier":"default","output":[{"type":"message","content":[{"type":"output_text","text":"hello"}]}],"usage":{"input_tokens":8,"input_tokens_details":{"cached_tokens":3},"output_tokens":6,"output_tokens_details":{"reasoning_tokens":4},"total_tokens":14}}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .expect("response");
        });
        let mut client = crate::providers::openai::OpenAiClient::for_test_responses(
            "sk-test-only-not-real-123456789",
            format!("http://{address}"),
            Some(events_path.clone()),
        );

        let outcome = chat(
            &mut client,
            &config,
            ProviderCallScope::Executor,
            &config.model,
            &[ConversationMessage::user("hello")],
            &[],
            false,
        );
        server.join().expect("server");
        assert_eq!(outcome.result.unwrap().content, "hello");
        let events = std::fs::read_to_string(events_path).expect("events");
        let turn = events
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .find(|event| event["event"] == "provider_turn_duration")
            .expect("provider turn event");
        assert_eq!(turn["provider_response_id"], "resp-f0b");
        assert_eq!(turn["provider_reasoning_tokens"], 4);
        assert_eq!(turn["provider_cached_input_tokens"], 3);
        assert_eq!(turn["provider_total_tokens"], 14);
    }

    #[test]
    fn lm_studio_requested_and_returned_model_ids_reach_turn_event() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let events_path = tmp.path().join("events.jsonl");
        let mut config = test_config(tmp.path(), events_path.clone(), 2);
        config.provider = Provider::LmStudio;
        config.model = "qwen3.5-9b-mlx".to_string();
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut request = [0_u8; 8192];
            let _ = stream.read(&mut request).expect("request");
            let body = r#"{"id":"chatcmpl-local","model":"qwen3.5-9b-mlx@4bit","created":1787000000,"system_fingerprint":"lm-runtime-1","choices":[{"message":{"content":"hello"}}],"usage":{"prompt_tokens":3,"completion_tokens":1}}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .expect("response");
        });
        let mut client = crate::providers::lm_studio::LmStudioClient::new(
            format!("http://{address}"),
            None,
            2,
            128,
            0,
            crate::config::OpenAiApi::ChatCompletions,
            Some(events_path.clone()),
        )
        .expect("LM Studio client");

        let outcome = chat(
            &mut client,
            &config,
            ProviderCallScope::Executor,
            &config.model,
            &[ConversationMessage::user("hello")],
            &[],
            false,
        );
        server.join().expect("server");
        assert_eq!(outcome.result.unwrap().content, "hello");
        let events = std::fs::read_to_string(events_path).expect("events");
        let turn = events
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .find(|event| event["event"] == "provider_turn_duration")
            .expect("provider turn event");
        assert_eq!(turn["model"], "qwen3.5-9b-mlx");
        assert_eq!(turn["provider_model_id"], "qwen3.5-9b-mlx@4bit");
        assert_eq!(turn["system_fingerprint"], "lm-runtime-1");
    }

    #[test]
    fn cloned_provider_call_resumes_worker_panic_on_caller_thread() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let events_path = tmp.path().join("events.jsonl");
        let config = test_config(tmp.path(), events_path.clone(), 30);
        let mut client = PanicClient;

        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = chat(
                &mut client,
                &config,
                ProviderCallScope::Executor,
                "m",
                &[ConversationMessage::user("execute".to_string())],
                &[],
                false,
            );
        }));

        assert!(panic.is_err());
        let events = std::fs::read_to_string(events_path).expect("events");
        assert!(events.contains("\"event\":\"provider_turn_duration\""));
        assert!(events.contains("\"finish_reason\":\"panic\""));
        assert!(events.contains("\"ok\":false"));
    }

    #[test]
    fn streaming_worker_delivers_incremental_chunks_and_same_final_reply() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let events_path = tmp.path().join("events.jsonl");
        let mut config = test_config(tmp.path(), events_path, 30);
        config.stream = true;
        let mut client = StreamingClient::new();
        let mut chunks = Vec::new();
        let outcome = chat_with_cancel_inner(
            &mut client,
            &config,
            ProviderChatRequest {
                scope: ProviderCallScope::Executor,
                model: "m",
                messages: &[ConversationMessage::user("prompt")],
                tools: &[],
                native_tools_enabled: false,
            },
            || false,
            None,
            true,
            Some(&mut |chunk| {
                chunks.push(chunk.to_string());
                Ok(())
            }),
        );
        assert_eq!(outcome.result.unwrap().content, "hello");
        assert_eq!(chunks, ["hel", "lo"]);
        assert_eq!(client.stream_calls.load(Ordering::SeqCst), 1);
        assert_eq!(client.chat_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn display_stream_callback_is_scrubbed_across_chunk_boundaries() {
        const CANARY: &str = "H01_CANARY_JwtStyle_NonPrefix_29486";
        let tmp = tempfile::tempdir().expect("tempdir");
        let events = tmp.path().join("events.jsonl");
        let mut config = test_config(tmp.path(), events.clone(), 30);
        config.stream = true;
        let mut catalog = crate::sensitive_data::SecretCatalog::new();
        catalog.register(CANARY);
        crate::sensitive_data::install_scope(catalog, Some(tmp.path()), Some(&events));

        #[derive(Clone)]
        struct CanaryStreamingClient {
            seen: Arc<Mutex<Vec<ConversationMessage>>>,
        }
        impl ChatClient for CanaryStreamingClient {
            fn label(&self) -> &str {
                "canary-streaming-mock"
            }
            fn boxed_clone(&self) -> Box<dyn ChatClient> {
                Box::new(self.clone())
            }
            fn supports_streaming(&self) -> bool {
                true
            }
            fn chat(
                &mut self,
                _model: &str,
                _messages: &[ConversationMessage],
                _tools: &[ToolSpec],
                _native_tools_enabled: bool,
            ) -> anyhow::Result<AssistantReply> {
                Ok(AssistantReply::text("batch"))
            }
            fn chat_stream(
                &mut self,
                _model: &str,
                messages: &[ConversationMessage],
                _tools: &[ToolSpec],
                _native_tools_enabled: bool,
                on_chunk: &mut dyn FnMut(&str) -> anyhow::Result<()>,
            ) -> anyhow::Result<AssistantReply> {
                *self.seen.lock().unwrap() = messages.to_vec();
                on_chunk("before ")?;
                on_chunk(&CANARY[..7])?;
                on_chunk(&CANARY[7..])?;
                on_chunk(" after")?;
                Ok(AssistantReply::text("before after"))
            }
        }

        let seen = Arc::new(Mutex::new(Vec::new()));
        let mut client = CanaryStreamingClient { seen: seen.clone() };
        let messages = vec![ConversationMessage::user(format!("hook {CANARY}"))];
        let mut chunks = Vec::new();
        let outcome = chat_with_cancel_inner(
            &mut client,
            &config,
            ProviderChatRequest {
                scope: ProviderCallScope::Executor,
                model: "m",
                messages: &messages,
                tools: &[],
                native_tools_enabled: false,
            },
            || false,
            None,
            true,
            Some(&mut |chunk| {
                chunks.push(chunk.to_string());
                Ok(())
            }),
        );

        assert!(outcome.result.is_ok());
        let display = chunks.concat();
        assert!(!display.contains(CANARY), "{display}");
        assert_eq!(display, "before <redacted> after");
        let sent = seen.lock().unwrap().clone();
        assert!(!sent[0].content.contains(CANARY));
        assert!(sent[0].content.contains("<redacted>"));
        crate::sensitive_data::reset_scopes_for_tests();
    }

    #[test]
    fn planner_scopes_stream_transport_without_forwarding_machine_chunks() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let config = test_config(tmp.path(), tmp.path().join("events.jsonl"), 30);
        let mut client = StreamingClient::new();

        for scope in [
            ProviderCallScope::PlannerStep,
            ProviderCallScope::PlannerUltra,
        ] {
            let mut chunks = Vec::new();
            let outcome = chat_with_cancel_and_stream(
                &mut client,
                &config,
                ProviderChatRequest {
                    scope,
                    model: "m",
                    messages: &[ConversationMessage::user("plan")],
                    tools: &[],
                    native_tools_enabled: false,
                },
                || false,
                &mut |chunk| {
                    chunks.push(chunk.to_string());
                    Ok(())
                },
            );

            assert_eq!(outcome.result.unwrap().content, "hello");
            assert!(chunks.is_empty(), "scope={}", scope.as_str());
        }
        assert_eq!(client.stream_calls.load(Ordering::SeqCst), 2);
        assert_eq!(client.chat_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn disabled_visible_streaming_keeps_executor_on_batch_transport() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let config = test_config(tmp.path(), tmp.path().join("events.jsonl"), 30);
        let mut client = StreamingClient::new();
        let mut chunks = Vec::new();

        let outcome = chat_with_cancel_and_stream(
            &mut client,
            &config,
            ProviderChatRequest {
                scope: ProviderCallScope::Executor,
                model: "m",
                messages: &[ConversationMessage::user("execute")],
                tools: &[],
                native_tools_enabled: false,
            },
            || false,
            &mut |chunk| {
                chunks.push(chunk.to_string());
                Ok(())
            },
        );

        assert_eq!(outcome.result.unwrap().content, "hello");
        assert!(chunks.is_empty());
        assert_eq!(client.stream_calls.load(Ordering::SeqCst), 0);
        assert_eq!(client.chat_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn cancellation_at_stream_chunk_boundary_keeps_delivered_prefix() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let events_path = tmp.path().join("events.jsonl");
        let mut config = test_config(tmp.path(), events_path, 30);
        config.stream = true;
        let mut client = StreamingClient::new();
        let cancelled = Arc::new(AtomicBool::new(false));
        let callback_cancelled = Arc::clone(&cancelled);
        let mut chunks = Vec::new();
        let outcome = chat_with_cancel_inner(
            &mut client,
            &config,
            ProviderChatRequest {
                scope: ProviderCallScope::Executor,
                model: "m",
                messages: &[ConversationMessage::user("prompt")],
                tools: &[],
                native_tools_enabled: false,
            },
            || cancelled.load(Ordering::SeqCst),
            None,
            true,
            Some(&mut |chunk| {
                chunks.push(chunk.to_string());
                callback_cancelled.store(true, Ordering::SeqCst);
                Ok(())
            }),
        );
        assert!(outcome.aborted_by_user);
        assert_eq!(chunks, ["hel"]);
        assert!(is_aborted_by_user(&outcome.result.unwrap_err().to_string()));
    }

    #[test]
    fn ordinary_provider_call_keeps_stream_capable_client_non_streaming() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let events_path = tmp.path().join("events.jsonl");
        let mut config = test_config(tmp.path(), events_path, 30);
        config.stream = true;
        let mut client = StreamingClient::new();
        let outcome = chat(
            &mut client,
            &config,
            ProviderCallScope::Executor,
            "m",
            &[ConversationMessage::user("prompt")],
            &[],
            false,
        );
        assert_eq!(outcome.result.unwrap().content, "hello");
        assert_eq!(client.stream_calls.load(Ordering::SeqCst), 0);
        assert_eq!(client.chat_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn response_limit_is_enforced_inside_the_provider_chokepoint() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let events_path = tmp.path().join("events.jsonl");
        let config = test_config(tmp.path(), events_path.clone(), 30);
        let mut client = TokenClient;
        let outcome = chat_with_cancel_and_response_limit(
            &mut client,
            &config,
            ProviderChatRequest {
                scope: ProviderCallScope::PlannerStep,
                model: "m",
                messages: &[ConversationMessage::user("classify")],
                tools: &[],
                native_tools_enabled: false,
            },
            || false,
            1,
        );

        assert!(
            outcome
                .result
                .unwrap_err()
                .to_string()
                .contains("provider_response_limit")
        );
        let events = std::fs::read_to_string(events_path).expect("events");
        assert!(events.contains("\"caller_scope\":\"planner_step\""));
        assert!(events.contains("\"finish_reason\":\"error\""));
        assert!(events.contains("\"ok\":false"));
    }

    #[test]
    fn trace_flag_records_the_shared_provider_chokepoint() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let events_path = tmp.path().join("run/events.jsonl");
        let config = test_config(tmp.path(), events_path, 30);
        let _trace = crate::run_trace::install(true);
        let mut client = TokenClient;

        let outcome = chat(
            &mut client,
            &config,
            ProviderCallScope::Executor,
            "trace-model",
            &[ConversationMessage::user(
                "trace sk-test-secret under /Users/alice/project",
            )],
            &[],
            false,
        );

        assert!(outcome.result.is_ok());
        let traces = std::fs::read_dir(tmp.path().join("run/trace"))
            .unwrap()
            .flatten()
            .map(|entry| std::fs::read_to_string(entry.path()).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(traces.len(), 1);
        assert!(traces[0].contains("\"scope\": \"executor\""));
        assert!(traces[0].contains("/Users/<user>/project"));
        assert!(!traces[0].contains("sk-test-secret"));
    }

    #[test]
    fn fake_client_without_stream_support_uses_legacy_chat_path() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let events_path = tmp.path().join("events.jsonl");
        let mut config = test_config(tmp.path(), events_path, 30);
        config.stream = true;
        let mut client = TokenClient;
        let mut chunks = Vec::new();
        let outcome = chat_with_cancel_inner(
            &mut client,
            &config,
            ProviderChatRequest {
                scope: ProviderCallScope::Executor,
                model: "m",
                messages: &[ConversationMessage::user("prompt")],
                tools: &[],
                native_tools_enabled: false,
            },
            || false,
            None,
            true,
            Some(&mut |chunk| {
                chunks.push(chunk.to_string());
                Ok(())
            }),
        );
        assert_eq!(outcome.result.unwrap().content, "ok");
        assert!(chunks.is_empty());
    }

    #[test]
    fn provider_turn_records_token_usage_and_context_truncation_warning() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let events_path = tmp.path().join("events.jsonl");
        let config = test_config(tmp.path(), events_path.clone(), 30);
        let mut client = TokenClient;
        let long_prompt = "x".repeat(2_000);
        for _ in 0..2 {
            let outcome = chat(
                &mut client,
                &config,
                ProviderCallScope::Executor,
                "token-test-model",
                &[ConversationMessage::user(long_prompt.clone())],
                &[],
                false,
            );
            assert!(outcome.result.is_ok());
        }

        let events = std::fs::read_to_string(events_path).expect("events");
        assert!(events.contains("\"event\":\"provider_turn_duration\""));
        assert!(events.contains("\"prompt_layout\":\"stable\""));
        assert!(events.contains("\"estimated_prompt_tokens_sent\""));
        assert!(events.contains("\"prompt_eval_count\":10"));
        assert!(events.contains("\"eval_count\":2"));
        assert!(events.contains("\"finish_reason\":\"stop\""));
        assert!(!events.contains("\"provider_model_id\""));
        assert!(!events.contains("\"system_fingerprint\""));
        assert!(events.contains("\"event\":\"context_truncation_suspected\""));
        assert!(events.contains("\"level\":\"WARNING\""));
        assert!(events.contains("\"persistent_undercut_count\":2"));
    }

    #[test]
    fn provider_turn_records_response_durations_and_derived_metrics() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let events_path = tmp.path().join("events.jsonl");
        let config = test_config(tmp.path(), events_path.clone(), 30);
        let mut client = TimingClient;

        let outcome = chat(
            &mut client,
            &config,
            ProviderCallScope::Executor,
            "timing-test-model",
            &[ConversationMessage::user("prompt".to_string())],
            &[],
            false,
        );

        assert!(outcome.result.is_ok());
        let events = std::fs::read_to_string(events_path).expect("events");
        let turn = events
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .find(|event| {
                event
                    .get("event")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|name| name == "provider_turn_duration")
            })
            .expect("provider turn duration event");
        assert_eq!(
            turn.get("prompt_eval_duration")
                .and_then(serde_json::Value::as_u64),
            Some(4_000_000_000)
        );
        assert_eq!(
            turn.get("eval_duration")
                .and_then(serde_json::Value::as_u64),
            Some(2_000_000_000)
        );
        assert_eq!(
            turn.get("load_duration")
                .and_then(serde_json::Value::as_u64),
            Some(1_000_000_000)
        );
        assert_eq!(
            turn.get("total_duration")
                .and_then(serde_json::Value::as_u64),
            Some(7_000_000_000)
        );
        assert_eq!(
            turn.get("prefill_seconds")
                .and_then(serde_json::Value::as_f64),
            Some(4.0)
        );
        assert_eq!(
            turn.get("generation_seconds")
                .and_then(serde_json::Value::as_f64),
            Some(2.0)
        );
        assert_eq!(
            turn.get("load_seconds").and_then(serde_json::Value::as_f64),
            Some(1.0)
        );
        assert_eq!(
            turn.get("tokens_per_second_eval")
                .and_then(serde_json::Value::as_f64),
            Some(2.0)
        );
    }

    #[test]
    fn provider_turn_records_estimated_stable_prefix_chars() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let events_path = tmp.path().join("events.jsonl");
        let config = test_config(tmp.path(), events_path.clone(), 30);
        let mut client = TokenClient;
        let model = format!("prefix-test-{}", uuid::Uuid::now_v7());
        for suffix in ["first", "second"] {
            let outcome = chat(
                &mut client,
                &config,
                ProviderCallScope::PlannerStep,
                &model,
                &[ConversationMessage::user(format!(
                    "Stable prefix section.\n\nVariable tail: {suffix}"
                ))],
                &[],
                false,
            );
            assert!(outcome.result.is_ok());
        }

        let text = std::fs::read_to_string(events_path).expect("events");
        let turns = text
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .filter(|event| {
                event
                    .get("event")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|name| name == "provider_turn_duration")
            })
            .collect::<Vec<_>>();
        assert_eq!(turns.len(), 2);
        assert_eq!(
            turns[0]
                .get("estimated_stable_prefix_chars")
                .and_then(serde_json::Value::as_u64),
            Some(0)
        );
        assert!(
            turns[1]
                .get("estimated_stable_prefix_chars")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
                >= "role:user\ncontent:\nStable prefix section."
                    .chars()
                    .count() as u64,
            "{text}"
        );
    }

    #[test]
    fn provider_scope_screen_labels_do_not_leak_enum_names() {
        for scope in ProviderCallScope::ALL {
            let label = scope.screen_label();
            assert!(!label.contains('_'), "{scope:?}: {label}");
            assert_ne!(label, scope.as_str());
            assert!(!label.trim().is_empty());
        }
    }

    #[test]
    fn provider_progress_due_is_bounded_to_once_per_minute() {
        let mut next = Duration::from_secs(60);

        assert_eq!(
            provider_progress_due(Duration::from_secs(30), &mut next),
            None
        );
        assert_eq!(
            provider_progress_due(Duration::from_secs(59), &mut next),
            None
        );
        assert_eq!(
            provider_progress_due(Duration::from_secs(60), &mut next),
            Some(60)
        );
        assert_eq!(next, Duration::from_secs(120));
        assert_eq!(
            provider_progress_due(Duration::from_secs(61), &mut next),
            None
        );
        assert_eq!(
            provider_progress_due(Duration::from_secs(120), &mut next),
            Some(120)
        );
        assert_eq!(next, Duration::from_secs(180));
    }
}

#[cfg(test)]
mod cache_write_event_tests {
    use std::io::{Read as _, Write as _};
    use std::net::TcpListener;

    use super::*;
    use crate::config::Provider;
    use crate::state::ConversationMessage;

    const R0: &str = r#"{"id":"resp-c0","model":"m-snap","created_at":1785542400,"service_tier":"default","output":[{"type":"message","content":[{"type":"output_text","text":"hello"}]}],"usage":{"input_tokens":8,"input_tokens_details":{"cached_tokens":3},"output_tokens":6,"output_tokens_details":{"reasoning_tokens":4},"total_tokens":14}}"#;
    const R1: &str = r#"{"id":"resp-c1","model":"m-snap","created_at":1785542400,"service_tier":"default","output":[{"type":"message","content":[{"type":"output_text","text":"hello"}]}],"usage":{"input_tokens":8,"input_tokens_details":{"cached_tokens":3,"cache_write_tokens":2,"cache_write_tokens_details":{"ttl_30m_tokens":1,"ttl_12h_tokens":1}},"output_tokens":6,"output_tokens_details":{"reasoning_tokens":4},"total_tokens":14}}"#;
    const C0: &str = r#"{"id":"chatcmpl-c0","model":"m-snap","created":1787000000,"choices":[{"message":{"content":"hello"}}],"usage":{"prompt_tokens":8,"completion_tokens":6,"prompt_tokens_details":{"cached_tokens":3},"cost":0.0012}}"#;
    const C1: &str = r#"{"id":"chatcmpl-c1","model":"m-snap","created":1787000000,"choices":[{"message":{"content":"hello"}}],"usage":{"prompt_tokens":8,"completion_tokens":6,"prompt_tokens_details":{"cached_tokens":3,"cache_write_tokens":2,"cache_write_tokens_details":{"ttl_30m_tokens":1,"ttl_12h_tokens":1}},"cost":0.0012}}"#;

    /// The provider_response and provider_turn_duration lines of one call, `duration_ms` set to 0.
    fn capture(responses: bool, reply: &str) -> Vec<String> {
        let (lines, failed) = capture_any(responses, reply);
        assert!(!failed, "{reply}");
        lines
    }

    fn capture_any(responses: bool, reply: &str) -> (Vec<String>, bool) {
        let reply = reply.to_string();
        let tmp = tempfile::tempdir().expect("tempdir");
        let events_path = tmp.path().join("events.jsonl");
        let mut config = super::tests::test_config(tmp.path(), events_path.clone(), 2);
        config.model = "m-1".to_string();
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut request = [0_u8; 16384];
            let _ = stream.read(&mut request).expect("request");
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                reply.len(),
                reply
            )
            .expect("response");
        });
        let mut client: Box<dyn crate::providers::ChatClient> = if responses {
            config.provider = Provider::Openai;
            config.openai_api = crate::config::OpenAiApi::Responses;
            Box::new(crate::providers::openai::OpenAiClient::for_test_responses(
                "sk-test-only-not-real-123456789",
                format!("http://{address}"),
                Some(events_path.clone()),
            ))
        } else {
            config.provider = Provider::LmStudio;
            Box::new(
                crate::providers::lm_studio::LmStudioClient::new(
                    format!("http://{address}"),
                    None,
                    2,
                    128,
                    0,
                    crate::config::OpenAiApi::ChatCompletions,
                    Some(events_path.clone()),
                )
                .expect("client"),
            )
        };
        let outcome = chat(
            client.as_mut(),
            &config,
            ProviderCallScope::Executor,
            &config.model,
            &[ConversationMessage::user("hello")],
            &[],
            false,
        );
        server.join().expect("server");
        let failed = outcome.result.is_err();
        let lines = std::fs::read_to_string(events_path)
            .expect("events")
            .lines()
            .filter_map(|line| {
                let mut value: serde_json::Value = serde_json::from_str(line).ok()?;
                let name = value["event"].as_str()?.to_string();
                if name != "provider_response" && name != "provider_turn_duration" {
                    return None;
                }
                normalize(&mut value);
                Some(value.to_string())
            })
            .collect();
        (lines, failed)
    }

    const BASE_R0: [&str; 2] = [
        r#"{"api":"responses","attempt":1,"event":"provider_response","model":"m-1","provider":"openai","reasoning_tokens":4,"response_id":"resp-c0","response_model":"m-snap","schema_version":"1","service_tier":"default","system_fingerprint":null,"tool_calls":0}"#,
        r#"{"aborted_by_user":false,"caller_scope":"executor","duration_ms":0,"estimated_prompt_tokens_sent":5,"estimated_stable_prefix_chars":0,"eval_count":6,"eval_duration":null,"event":"provider_turn_duration","finish_reason":"stop","generation_seconds":null,"load_duration":null,"load_seconds":null,"model":"m-1","native_tools_enabled":false,"ok":true,"prefill_seconds":null,"prompt_eval_count":8,"prompt_eval_duration":null,"prompt_layout":"stable","provider":"openai","provider_cached_input_tokens":3,"provider_created_epoch":1785542400,"provider_model_id":"m-snap","provider_reasoning_tokens":4,"provider_response_id":"resp-c0","provider_service_tier":"default","provider_total_tokens":14,"schema_version":"1","system_fingerprint":null,"think":"omitted","timed_out":false,"timeout_ms":2000,"timeout_secs":2,"timeout_source":"override:test","tokens_per_second_eval":null,"tool_calls_in_response":0,"tools":0,"total_duration":null,"write_or_edit_succeeded":false}"#,
    ];
    const BASE_R1: [&str; 2] = [
        r#"{"api":"responses","attempt":1,"event":"provider_response","model":"m-1","provider":"openai","reasoning_tokens":4,"response_id":"resp-c1","response_model":"m-snap","schema_version":"1","service_tier":"default","system_fingerprint":null,"tool_calls":0}"#,
        r#"{"aborted_by_user":false,"caller_scope":"executor","duration_ms":0,"estimated_prompt_tokens_sent":5,"estimated_stable_prefix_chars":25,"eval_count":6,"eval_duration":null,"event":"provider_turn_duration","finish_reason":"stop","generation_seconds":null,"load_duration":null,"load_seconds":null,"model":"m-1","native_tools_enabled":false,"ok":true,"prefill_seconds":null,"prompt_eval_count":8,"prompt_eval_duration":null,"prompt_layout":"stable","provider":"openai","provider_cached_input_tokens":3,"provider_created_epoch":1785542400,"provider_model_id":"m-snap","provider_reasoning_tokens":4,"provider_response_id":"resp-c1","provider_service_tier":"default","provider_total_tokens":14,"schema_version":"1","system_fingerprint":null,"think":"omitted","timed_out":false,"timeout_ms":2000,"timeout_secs":2,"timeout_source":"override:test","tokens_per_second_eval":null,"tool_calls_in_response":0,"tools":0,"total_duration":null,"write_or_edit_succeeded":false}"#,
    ];
    const BASE_C0: [&str; 2] = [
        r#"{"api":"chat_completions","attempt":1,"event":"provider_response","model":"m-1","provider":"lm-studio","provider_billed_cost_usd":"0.0012","response_model":"m-snap","schema_version":"1","system_fingerprint":null,"tool_calls":0}"#,
        r#"{"aborted_by_user":false,"caller_scope":"executor","duration_ms":0,"estimated_prompt_tokens_sent":5,"estimated_stable_prefix_chars":0,"eval_count":6,"eval_duration":null,"event":"provider_turn_duration","finish_reason":"stop","generation_seconds":null,"load_duration":null,"load_seconds":null,"model":"m-1","native_tools_enabled":false,"ok":true,"prefill_seconds":null,"prompt_eval_count":8,"prompt_eval_duration":null,"prompt_layout":"stable","provider":"lm-studio","provider_billed_cost_usd":"0.0012","provider_cached_input_tokens":3,"provider_created_epoch":1787000000,"provider_model_id":"m-snap","provider_response_id":"chatcmpl-c0","provider_service_tier":null,"schema_version":"1","system_fingerprint":null,"think":"omitted","timed_out":false,"timeout_ms":2000,"timeout_secs":2,"timeout_source":"override:test","tokens_per_second_eval":null,"tool_calls_in_response":0,"tools":0,"total_duration":null,"write_or_edit_succeeded":false}"#,
    ];
    const BASE_C1: [&str; 2] = [
        r#"{"api":"chat_completions","attempt":1,"event":"provider_response","model":"m-1","provider":"lm-studio","provider_billed_cost_usd":"0.0012","response_model":"m-snap","schema_version":"1","system_fingerprint":null,"tool_calls":0}"#,
        r#"{"aborted_by_user":false,"caller_scope":"executor","duration_ms":0,"estimated_prompt_tokens_sent":5,"estimated_stable_prefix_chars":25,"eval_count":6,"eval_duration":null,"event":"provider_turn_duration","finish_reason":"stop","generation_seconds":null,"load_duration":null,"load_seconds":null,"model":"m-1","native_tools_enabled":false,"ok":true,"prefill_seconds":null,"prompt_eval_count":8,"prompt_eval_duration":null,"prompt_layout":"stable","provider":"lm-studio","provider_billed_cost_usd":"0.0012","provider_cached_input_tokens":3,"provider_created_epoch":1787000000,"provider_model_id":"m-snap","provider_response_id":"chatcmpl-c1","provider_service_tier":null,"schema_version":"1","system_fingerprint":null,"think":"omitted","timed_out":false,"timeout_ms":2000,"timeout_secs":2,"timeout_source":"override:test","tokens_per_second_eval":null,"tool_calls_in_response":0,"tools":0,"total_duration":null,"write_or_edit_succeeded":false}"#,
    ];

    /// What varies between runs, and only that: the wall time, and the stable-prefix estimate a
    /// process keeps across calls (it depends on what ran before in the same process).
    fn normalize(value: &mut serde_json::Value) {
        for key in ["duration_ms", "estimated_stable_prefix_chars"] {
            if value.get(key).is_some() {
                value[key] = serde_json::json!(0);
            }
        }
    }

    fn normalized(line: &str) -> String {
        let mut value: serde_json::Value = serde_json::from_str(line).unwrap();
        normalize(&mut value);
        value.to_string()
    }

    /// The line without the three new names, and those three, apart.
    fn split(line: &str) -> (String, serde_json::Map<String, serde_json::Value>) {
        let mut value: serde_json::Value = serde_json::from_str(line).unwrap();
        let mut taken = serde_json::Map::new();
        for key in [
            "provider_cache_write_input_tokens",
            "provider_cache_write_input_tokens_30m",
            "provider_cache_write_input_tokens_12h",
        ] {
            if let Some(found) = value.as_object_mut().unwrap().remove(key) {
                taken.insert(key.to_string(), found);
            }
        }
        (value.to_string(), taken)
    }

    #[test]
    fn a_reply_with_no_cache_write_gives_the_lines_the_base_gave_byte_for_byte() {
        for (responses, reply, base) in [(true, R0, BASE_R0), (false, C0, BASE_C0)] {
            let lines = capture(responses, reply);
            let base = base.map(normalized);
            assert_eq!(lines, base.to_vec(), "{reply}");
        }
    }

    #[test]
    fn a_reply_with_a_cache_write_adds_the_three_fields_and_nothing_else() {
        for (responses, reply, base) in [(true, R1, BASE_R1), (false, C1, BASE_C1)] {
            let lines = capture(responses, reply);
            assert_eq!(lines.len(), 2);
            let expected = serde_json::json!({
                "provider_cache_write_input_tokens": 2,
                "provider_cache_write_input_tokens_30m": 1,
                "provider_cache_write_input_tokens_12h": 1
            });
            for (line, base) in lines.iter().zip(base) {
                let (rest, taken) = split(line);
                assert_eq!(rest, normalized(base), "{reply}");
                assert_eq!(serde_json::Value::Object(taken), expected, "{line}");
            }
        }
    }

    #[test]
    fn the_comparison_without_the_three_names_still_sees_any_other_change() {
        // The removal takes the three names only: a change to any other field, or a line more,
        // is still a difference.
        let base = normalized(BASE_R1[1]);
        for key in [
            "provider_cached_input_tokens",
            "provider_total_tokens",
            "eval_count",
            "ok",
        ] {
            let mut changed: serde_json::Value = serde_json::from_str(&base).unwrap();
            changed[key] = serde_json::json!("changed");
            assert_ne!(split(&changed.to_string()).0, base, "{key}");
        }
        let mut added: serde_json::Value = serde_json::from_str(&base).unwrap();
        added["provider_cache_write_input_tokens_1h"] = serde_json::json!(1);
        assert_ne!(split(&added.to_string()).0, base);
    }

    /// The cache-write fields of the turn event and of the response event of one call.
    fn fields_of(responses: bool, reply: &str) -> (serde_json::Value, serde_json::Value) {
        let lines = capture(responses, reply);
        let pick = |name: &str| {
            let line = lines
                .iter()
                .find(|line| line.contains(&format!("\"event\":\"{name}\"")))
                .unwrap();
            serde_json::Value::Object(split(line).1)
        };
        (pick("provider_turn_duration"), pick("provider_response"))
    }

    fn responses_reply(details: serde_json::Value) -> String {
        serde_json::json!({
            "id": "resp", "model": "m-snap", "service_tier": "default",
            "output": [{"type": "message", "content": [{"type": "output_text", "text": "hello"}]}],
            "usage": {"input_tokens": 100, "input_tokens_details": details, "output_tokens": 6}
        })
        .to_string()
    }

    fn chat_reply(usage: serde_json::Value) -> String {
        serde_json::json!({
            "id": "chat", "model": "m-snap",
            "choices": [{"message": {"content": "hello"}}],
            "usage": usage
        })
        .to_string()
    }

    #[test]
    fn each_kind_of_report_is_told_apart_and_both_events_agree() {
        use serde_json::json;
        let rows = [
            (
                "explicit zero",
                json!({"cached_tokens": 0, "cache_write_tokens": 0,
                "cache_write_tokens_details": {"ttl_30m_tokens": 0, "ttl_12h_tokens": 0}}),
                json!({"provider_cache_write_input_tokens": 0, "provider_cache_write_input_tokens_30m": 0,
                    "provider_cache_write_input_tokens_12h": 0}),
            ),
            ("missing", json!({"cached_tokens": 0}), json!({})),
            (
                "null",
                json!({"cached_tokens": 0, "cache_write_tokens": null}),
                json!({}),
            ),
            (
                "bad type",
                json!({"cached_tokens": 0, "cache_write_tokens": "40"}),
                json!({}),
            ),
            (
                "negative",
                json!({"cached_tokens": 0, "cache_write_tokens": -1}),
                json!({}),
            ),
            (
                "total only",
                json!({"cached_tokens": 0, "cache_write_tokens": 40}),
                json!({"provider_cache_write_input_tokens": 40}),
            ),
            (
                "ttl mismatch",
                json!({"cached_tokens": 0, "cache_write_tokens": 40,
                "cache_write_tokens_details": {"ttl_30m_tokens": 30, "ttl_12h_tokens": 11}}),
                json!({}),
            ),
            (
                "mixed ttl",
                json!({"cached_tokens": 0, "cache_write_tokens": 40,
                "cache_write_tokens_details": {"ttl_30m_tokens": 30, "ttl_12h_tokens": 10}}),
                json!({"provider_cache_write_input_tokens": 40, "provider_cache_write_input_tokens_30m": 30,
                    "provider_cache_write_input_tokens_12h": 10}),
            ),
            (
                "one ttl",
                json!({"cached_tokens": 0, "cache_write_tokens": 40,
                "cache_write_tokens_details": {"ttl_30m_tokens": 30}}),
                json!({"provider_cache_write_input_tokens": 40}),
            ),
            (
                "past the input",
                json!({"cached_tokens": 61, "cache_write_tokens": 40}),
                json!({}),
            ),
            (
                "past u32",
                json!({"cached_tokens": 0, "cache_write_tokens": 5_000_000_000_u64}),
                json!({}),
            ),
        ];
        for (name, details, expected) in rows {
            let (turn, response) = fields_of(true, &responses_reply(details));
            assert_eq!(turn, expected, "responses {name}");
            assert_eq!(response, turn, "responses {name}");
        }
        // The same reader on the Chat paths, and nothing from a name the table does not list.
        let rows = [
            (
                "chat ttl",
                json!({"prompt_tokens": 100, "completion_tokens": 6,
                "prompt_tokens_details": {"cached_tokens": 0, "cache_write_tokens": 7,
                    "cache_write_tokens_details": {"ttl_30m_tokens": 7, "ttl_12h_tokens": 0}}}),
                json!({"provider_cache_write_input_tokens": 7, "provider_cache_write_input_tokens_30m": 7,
                    "provider_cache_write_input_tokens_12h": 0}),
            ),
            (
                "decoy at the top",
                json!({"prompt_tokens": 100, "completion_tokens": 6, "cache_write_tokens": 7}),
                json!({}),
            ),
            (
                "decoy another name",
                json!({"prompt_tokens": 100, "completion_tokens": 6,
                "prompt_tokens_details": {"cache_creation_input_tokens": 7}}),
                json!({}),
            ),
            (
                "decoy the responses path",
                json!({"prompt_tokens": 100, "completion_tokens": 6,
                "input_tokens_details": {"cache_write_tokens": 7}}),
                json!({}),
            ),
        ];
        for (name, usage, expected) in rows {
            let (turn, response) = fields_of(false, &chat_reply(usage));
            assert_eq!(turn, expected, "{name}");
            assert_eq!(response, turn, "{name}");
        }
    }

    #[test]
    fn an_aggregate_over_turn_events_alone_counts_each_write_once() {
        use serde_json::json;
        let writes = [(5_u64, 2_u64, 3_u64), (0, 0, 0), (90, 89, 1)];
        let mut all = Vec::new();
        for (total, ttl_30m, ttl_12h) in writes {
            let reply = responses_reply(json!({"cached_tokens": 0, "cache_write_tokens": total,
                "cache_write_tokens_details": {"ttl_30m_tokens": ttl_30m, "ttl_12h_tokens": ttl_12h}}));
            all.extend(capture(true, &reply));
        }
        // One without any report, which adds nothing.
        all.extend(capture(true, &responses_reply(json!({"cached_tokens": 0}))));
        let sum = |event: &str, key: &str| -> u64 {
            all.iter()
                .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
                .filter(|value| value["event"] == event)
                .filter_map(|value| value[key].as_u64())
                .sum()
        };
        let oracle_total: u64 = writes.iter().map(|w| w.0).sum();
        let oracle_30m: u64 = writes.iter().map(|w| w.1).sum();
        let oracle_12h: u64 = writes.iter().map(|w| w.2).sum();
        assert_eq!(
            sum(
                "provider_turn_duration",
                "provider_cache_write_input_tokens"
            ),
            oracle_total
        );
        assert_eq!(
            sum(
                "provider_turn_duration",
                "provider_cache_write_input_tokens_30m"
            ),
            oracle_30m
        );
        assert_eq!(
            sum(
                "provider_turn_duration",
                "provider_cache_write_input_tokens_12h"
            ),
            oracle_12h
        );
        // Reading both events would count each write twice: an aggregator reads turns only.
        assert_eq!(
            sum("provider_response", "provider_cache_write_input_tokens"),
            oracle_total
        );
    }

    /// A reply that fails to parse after its usage was read: the turn is an error, its metadata
    /// is as it was (none of the five metadata fields), and the cache write is still on it.
    #[test]
    fn a_turn_whose_reply_fails_to_parse_keeps_its_cache_write_and_no_metadata() {
        use serde_json::json;
        let reply = json!({
            "id": "resp", "model": "m-snap", "service_tier": "default",
            "output": [{"type": "function_call", "call_id": "c", "arguments": "{}"}],
            "usage": {"input_tokens": 100, "input_tokens_details": {"cached_tokens": 0, "cache_write_tokens": 9,
                "cache_write_tokens_details": {"ttl_30m_tokens": 4, "ttl_12h_tokens": 5}}, "output_tokens": 6}
        })
        .to_string();
        let (lines, failed) = capture_any(true, &reply);
        assert!(failed);
        let turn: serde_json::Value = serde_json::from_str(
            lines
                .iter()
                .find(|l| l.contains("provider_turn_duration"))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(turn["ok"], json!(false));
        for key in [
            "provider_response_id",
            "provider_model_id",
            "provider_service_tier",
            "provider_created_epoch",
            "system_fingerprint",
        ] {
            assert!(turn.get(key).is_none(), "{key}");
        }
        assert_eq!(turn["provider_cache_write_input_tokens"], json!(9));
        assert_eq!(turn["provider_cache_write_input_tokens_30m"], json!(4));
        assert_eq!(turn["provider_cache_write_input_tokens_12h"], json!(5));
        // No provider_response is written for it.
        assert!(
            !lines
                .iter()
                .any(|l| l.contains("\"event\":\"provider_response\""))
        );
    }

    /// Success, then a failure, then a reply with nothing: no call keeps what an earlier one had.
    #[test]
    fn no_call_carries_the_cache_write_of_the_one_before() {
        use serde_json::json;
        let first = capture(
            true,
            &responses_reply(json!({"cached_tokens": 0, "cache_write_tokens": 3})),
        );
        assert!(
            first
                .iter()
                .all(|l| l.contains("provider_cache_write_input_tokens"))
        );
        let (failed, _) = capture_any(
            true,
            r#"{"id":"r","output":[{"type":"function_call","call_id":"c","arguments":"{}"}]}"#,
        );
        assert!(
            failed
                .iter()
                .all(|l| !l.contains("provider_cache_write_input_tokens"))
        );
        let last = capture(true, &responses_reply(json!({"cached_tokens": 0})));
        assert!(
            last.iter()
                .all(|l| !l.contains("provider_cache_write_input_tokens"))
        );
    }
}
