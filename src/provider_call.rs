use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock, mpsc};
use std::time::{Duration, Instant};

use anyhow::anyhow;
use serde_json::json;

use crate::config::Config;
use crate::eval_events;
use crate::providers::{AssistantReply, ChatClient, ProviderResponseMetadata, ResponseTiming};
use crate::state::ConversationMessage;
use crate::tools::registry::ToolSpec;

pub const PROVIDER_WAIT_SLICE: Duration = Duration::from_millis(250);
const ABORTED_BY_USER_ERROR: &str = "aborted_by_user: interrupted by user";
const CONTEXT_UNDERCUT_PERCENT: u64 = 70;
const CONTEXT_UNDERCUT_PERSISTENCE: usize = 2;
type ProviderChunkCallback<'a> = dyn FnMut(&str) -> anyhow::Result<()> + 'a;

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

pub struct ProviderCallOutcome {
    pub result: anyhow::Result<AssistantReply>,
    pub elapsed: Duration,
    pub timed_out: bool,
    pub aborted_by_user: bool,
}

pub struct ProviderChatRequest<'a> {
    pub scope: ProviderCallScope,
    pub model: &'a str,
    pub messages: &'a [ConversationMessage],
    pub tools: &'a [ToolSpec],
    pub native_tools_enabled: bool,
}

struct ProviderTurnTelemetry<'a> {
    scope: ProviderCallScope,
    provider: &'a str,
    model: &'a str,
    tool_count: usize,
    native_tools_enabled: bool,
    estimated_prompt_tokens: u64,
    estimated_stable_prefix_chars: u64,
    prompt_eval_count: Option<u64>,
    eval_count: Option<u64>,
    prompt_eval_duration: Option<u64>,
    eval_duration: Option<u64>,
    load_duration: Option<u64>,
    total_duration: Option<u64>,
    prefill_seconds: Option<f64>,
    generation_seconds: Option<f64>,
    load_seconds: Option<f64>,
    tokens_per_second_eval: Option<f64>,
    response_metadata: Option<ProviderResponseMetadata>,
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
    mut on_chunk: Option<&mut ProviderChunkCallback<'_>>,
) -> ProviderCallOutcome
where
    F: Fn() -> bool,
{
    let scope = request.scope;
    let model = request.model;
    let messages = request.messages;
    let tools = request.tools;
    let native_tools_enabled = request.native_tools_enabled;
    let provider = client.label().to_string();
    let stream = stream_allowed
        && on_chunk.is_some()
        && config.stream
        && client.supports_streaming_for_model(model);
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
    crate::tui::status_bus::publish_provider_started(scope.as_str(), config.chat_timeout_secs);
    crate::tui::presentation::emit_provider_turn_started(
        scope.as_str(),
        model,
        config.chat_timeout_secs,
    );
    let started = Instant::now();
    let mut next_progress_at = Duration::from_secs(60);

    if is_cancelled() {
        return provider_aborted_by_user(
            config,
            ProviderTurnTelemetryBase {
                scope,
                provider: &provider,
                model,
                tool_count: tools.len(),
                native_tools_enabled,
                estimated_prompt_tokens,
                estimated_stable_prefix_chars,
                elapsed: started.elapsed(),
                timed_out: false,
                aborted_by_user: true,
            },
        );
    }

    let mut worker_client = client.boxed_clone();
    let model = model.to_string();
    let tool_count = tools.len();
    let messages = messages.to_vec();
    let tools = tools.to_vec();
    let worker_model = model.clone();
    let (tx, rx) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
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
            }
        }));
        let message = match worker_result {
            Ok(result) => ProviderWorkerMessage::Completed(Box::new(result)),
            Err(payload) => ProviderWorkerMessage::Panicked(payload),
        };
        let _ = tx.send(message);
    });

    loop {
        if is_cancelled() {
            return provider_aborted_by_user(
                config,
                ProviderTurnTelemetryBase {
                    scope,
                    provider: &provider,
                    model: &model,
                    tool_count,
                    native_tools_enabled,
                    estimated_prompt_tokens,
                    estimated_stable_prefix_chars,
                    elapsed: started.elapsed(),
                    timed_out: false,
                    aborted_by_user: true,
                },
            );
        }
        let elapsed = started.elapsed();
        if elapsed >= timeout {
            let elapsed = started.elapsed();
            crate::tui::status_bus::publish_provider_finished(elapsed);
            emit_provider_turn_duration(
                config,
                ProviderTurnTelemetry {
                    scope,
                    provider: &provider,
                    model: &model,
                    tool_count,
                    native_tools_enabled,
                    estimated_prompt_tokens,
                    estimated_stable_prefix_chars,
                    prompt_eval_count: None,
                    eval_count: None,
                    prompt_eval_duration: None,
                    eval_duration: None,
                    load_duration: None,
                    total_duration: None,
                    prefill_seconds: None,
                    generation_seconds: None,
                    load_seconds: None,
                    tokens_per_second_eval: None,
                    response_metadata: None,
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
            return ProviderCallOutcome {
                result: Err(anyhow!(
                    "{}: provider call exceeded configured deadline of {}s",
                    scope.timeout_kind(),
                    config.chat_timeout_secs
                )),
                elapsed,
                timed_out: true,
                aborted_by_user: false,
            };
        }
        emit_provider_progress_if_due(started, config.chat_timeout_secs, &mut next_progress_at);
        let slice = PROVIDER_WAIT_SLICE.min(timeout.saturating_sub(elapsed));
        match rx.recv_timeout(slice) {
            Ok(ProviderWorkerMessage::Chunk(chunk)) => {
                if render_stream_chunks
                    && let Some(on_chunk) = on_chunk.as_deref_mut()
                    && let Err(err) = on_chunk(&chunk)
                {
                    let elapsed = started.elapsed();
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
                                estimated_prompt_tokens,
                                estimated_stable_prefix_chars,
                                elapsed,
                                timed_out: false,
                                aborted_by_user: false,
                            },
                            &result,
                            None,
                            None,
                        ),
                    );
                    return ProviderCallOutcome {
                        result,
                        elapsed,
                        timed_out: false,
                        aborted_by_user: false,
                    };
                }
            }
            Ok(ProviderWorkerMessage::Completed(worker_result)) => {
                let ProviderWorkerResult {
                    result,
                    timing,
                    response_metadata,
                } = *worker_result;
                let result = enforce_response_limit(result, max_response_bytes);
                let elapsed = started.elapsed();
                crate::tui::status_bus::publish_provider_finished(elapsed);
                if result.is_ok() {
                    crate::tui::presentation::emit_provider_turn_completed(
                        scope.as_str(),
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
                            estimated_prompt_tokens,
                            estimated_stable_prefix_chars,
                            elapsed,
                            timed_out: false,
                            aborted_by_user: false,
                        },
                        &result,
                        timing,
                        response_metadata,
                    ),
                );
                return ProviderCallOutcome {
                    result,
                    elapsed,
                    timed_out: false,
                    aborted_by_user: false,
                };
            }
            Ok(ProviderWorkerMessage::Panicked(payload)) => {
                let elapsed = started.elapsed();
                crate::tui::status_bus::publish_provider_finished(elapsed);
                emit_provider_turn_duration(
                    config,
                    ProviderTurnTelemetry {
                        scope,
                        provider: &provider,
                        model: &model,
                        tool_count,
                        native_tools_enabled,
                        estimated_prompt_tokens,
                        estimated_stable_prefix_chars,
                        prompt_eval_count: None,
                        eval_count: None,
                        prompt_eval_duration: None,
                        eval_duration: None,
                        load_duration: None,
                        total_duration: None,
                        prefill_seconds: None,
                        generation_seconds: None,
                        load_seconds: None,
                        tokens_per_second_eval: None,
                        response_metadata: None,
                        finish_reason: "panic".to_string(),
                        elapsed,
                        timed_out: false,
                        aborted_by_user: false,
                        ok: false,
                    },
                );
                std::panic::resume_unwind(payload);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let elapsed = started.elapsed();
                crate::tui::status_bus::publish_provider_finished(elapsed);
                emit_provider_turn_duration(
                    config,
                    ProviderTurnTelemetry {
                        scope,
                        provider: &provider,
                        model: &model,
                        tool_count,
                        native_tools_enabled,
                        estimated_prompt_tokens,
                        estimated_stable_prefix_chars,
                        prompt_eval_count: None,
                        eval_count: None,
                        prompt_eval_duration: None,
                        eval_duration: None,
                        load_duration: None,
                        total_duration: None,
                        prefill_seconds: None,
                        generation_seconds: None,
                        load_seconds: None,
                        tokens_per_second_eval: None,
                        response_metadata: None,
                        finish_reason: "error".to_string(),
                        elapsed,
                        timed_out: false,
                        aborted_by_user: false,
                        ok: false,
                    },
                );
                return ProviderCallOutcome {
                    result: Err(anyhow!("provider call worker disconnected")),
                    elapsed,
                    timed_out: false,
                    aborted_by_user: false,
                };
            }
        }
    }
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

fn emit_provider_progress_if_due(
    started: Instant,
    deadline_secs: u64,
    next_progress_at: &mut Duration,
) {
    let elapsed = started.elapsed();
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
            estimated_prompt_tokens: base.estimated_prompt_tokens,
            estimated_stable_prefix_chars: base.estimated_stable_prefix_chars,
            prompt_eval_count: None,
            eval_count: None,
            prompt_eval_duration: None,
            eval_duration: None,
            load_duration: None,
            total_duration: None,
            prefill_seconds: None,
            generation_seconds: None,
            load_seconds: None,
            tokens_per_second_eval: None,
            response_metadata: None,
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
    }
}

fn provider_turn_telemetry_from_result<'a>(
    base: ProviderTurnTelemetryBase<'a>,
    result: &anyhow::Result<AssistantReply>,
    timing: Option<ResponseTiming>,
    response_metadata: Option<ProviderResponseMetadata>,
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
        estimated_prompt_tokens: base.estimated_prompt_tokens,
        estimated_stable_prefix_chars: base.estimated_stable_prefix_chars,
        prompt_eval_count: reply.and_then(|reply| reply.prompt_tokens),
        eval_count: reply.and_then(|reply| reply.completion_tokens),
        prompt_eval_duration,
        eval_duration,
        load_duration,
        total_duration,
        prefill_seconds,
        generation_seconds,
        load_seconds,
        tokens_per_second_eval,
        response_metadata,
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
    }
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
    struct HangingClient {
        delay: Duration,
    }

    impl ChatClient for HangingClient {
        fn label(&self) -> &str {
            "hanging"
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
            std::thread::sleep(self.delay);
            Ok(AssistantReply::text("late"))
        }
    }

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

    fn test_config(root: &std::path::Path, events_path: PathBuf, chat_timeout_secs: u64) -> Config {
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
            ollama_host: "http://localhost:11434".to_string(),
            num_predict: 100,
            max_iterations: 1,
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
    fn cloned_provider_call_times_out_without_waiting_for_worker() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let events_path = tmp.path().join("events.jsonl");
        let config = test_config(tmp.path(), events_path.clone(), 1);
        let mut client = HangingClient {
            delay: Duration::from_secs(5),
        };

        let started = Instant::now();
        let outcome = chat(
            &mut client,
            &config,
            ProviderCallScope::PlannerStep,
            "m",
            &[ConversationMessage::user("plan".to_string())],
            &[],
            false,
        );

        assert!(outcome.timed_out);
        assert!(started.elapsed() < Duration::from_secs(3));
        let error = outcome.result.unwrap_err().to_string();
        assert!(
            error.contains("phase_step_planner_timeout"),
            "unexpected error: {error}"
        );
        let events = std::fs::read_to_string(events_path).expect("events");
        assert!(events.contains("\"event\":\"provider_turn_duration\""));
        assert!(events.contains("\"caller_scope\":\"planner_step\""));
        assert!(events.contains("\"timeout_source\":\"override:test\""));
        assert!(events.contains("\"classification\":\"phase_step_planner_timeout\""));
    }

    #[test]
    fn cloned_provider_call_aborts_promptly_when_cancelled() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let events_path = tmp.path().join("events.jsonl");
        let config = test_config(tmp.path(), events_path.clone(), 30);
        let mut client = HangingClient {
            delay: Duration::from_secs(30),
        };
        let cancelled = Arc::new(AtomicBool::new(false));
        let thread_cancelled = Arc::clone(&cancelled);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            thread_cancelled.store(true, Ordering::SeqCst);
        });

        let started = Instant::now();
        let outcome = chat_with_cancel(
            &mut client,
            &config,
            ProviderChatRequest {
                scope: ProviderCallScope::PlannerStep,
                model: "m",
                messages: &[ConversationMessage::user("plan".to_string())],
                tools: &[],
                native_tools_enabled: false,
            },
            || cancelled.load(Ordering::SeqCst),
        );

        assert!(outcome.aborted_by_user);
        assert!(!outcome.timed_out);
        assert!(started.elapsed() < Duration::from_secs(2));
        let error = outcome.result.unwrap_err().to_string();
        assert!(is_aborted_by_user(&error), "unexpected error: {error}");
        let events = std::fs::read_to_string(events_path).expect("events");
        assert!(events.contains("\"event\":\"provider_turn_duration\""));
        assert!(events.contains("\"aborted_by_user\":true"));
        assert!(events.contains("\"classification\":\"aborted_by_user\""));
        assert!(events.contains("\"event\":\"provider_turn_aborted_by_user\""));
        assert!(!events.contains("\"event\":\"provider_turn_timeout\""));
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
    fn planner_scopes_stream_transport_without_forwarding_machine_chunks() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut config = test_config(tmp.path(), tmp.path().join("events.jsonl"), 30);
        config.stream = true;
        let mut client = StreamingClient::new();

        for scope in [
            ProviderCallScope::PlannerStep,
            ProviderCallScope::PlannerUltra,
        ] {
            let mut chunks = Vec::new();
            let outcome = chat_with_cancel_inner(
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
                None,
                true,
                Some(&mut |chunk| {
                    chunks.push(chunk.to_string());
                    Ok(())
                }),
            );

            assert_eq!(outcome.result.unwrap().content, "hello");
            assert!(chunks.is_empty(), "scope={}", scope.as_str());
        }
        assert_eq!(client.stream_calls.load(Ordering::SeqCst), 2);
        assert_eq!(client.chat_calls.load(Ordering::SeqCst), 0);
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
