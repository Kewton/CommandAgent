//! Speculative next-phase planning (Issue #242) and the accounting of the
//! provider calls it makes (Issue #948).
//!
//! # Provider-call accounting design (Issue #948)
//!
//! The prefetch worker runs with eval events disabled, so before #948 a real
//! prefetch call emitted no `provider_turn_duration`, the adopted reply was
//! replayed with the *fallback's* timing and metadata, and a discarded or
//! failed prefetch left no usage record at all.
//!
//! - **Pairing.** The worker drives a `RecordingClient` around its client
//!   clone. Every `RecordingClient` instance takes the next client generation
//!   from a log shared by all of them when it is created, and each physical
//!   `chat`/`chat_stream` call appends one `PrefetchCallRecord` (1-based
//!   `call_index`, the generation of the instance that made it) and settles it
//!   with that same call's `take_response_timing` and `take_response_metadata`.
//!   A clone starts with no stored timing or metadata, so no call inherits
//!   another call's values.
//! - **Which call produced the reply.** The worker thread makes its provider
//!   calls one after another, and each `provider_call` clones the worker
//!   client once, on the worker thread, before it starts its call. The adopted
//!   reply came out of the worker's last `provider_call`, so it was produced by
//!   the call of the latest generation, which settled `ok` before its reply was
//!   handed back. The pairing uses that identity, never the start order of the
//!   calls (an abandoned attempt's thread can start late) or the reply text.
//! - **Closing the log.** Once the worker thread is joined, the log is closed
//!   under its lock and its records are read in the same step. A call that
//!   begins after that (the thread of an abandoned attempt that `provider_call`
//!   no longer waits for) starts no provider call and records nothing, so no
//!   physical call goes unaccounted. A call that began before the close and
//!   settles after it stays `unsettled`.
//! - **Replay.** `PrefetchedPlanner` replays the reply once; the instance that
//!   replayed answers `take_response_*` with the paired values, which stay
//!   `None` when the provider reported none, and never asks the fallback. A
//!   fallback call answers with the fallback's own values.
//! - **Counting.** Every physical prefetch call gets exactly one additive
//!   `speculative_phase_plan_provider_call` event, so the number of physical
//!   prefetch `ChatClient` calls equals the number of these events (a
//!   provider's own transport retries inside one call are not visible here).
//!   Each call's usage is counted exactly once: by its own event when
//!   `accounting_record` is `true`, or, for `disposition = "replayed"` with
//!   `accounting_record: false`, by the `provider_turn_duration` that carried
//!   the replayed response (same `provider_response_id` when one was
//!   reported). `"unconsumed"` (`accounting_record: true`) is a call that
//!   settled but was never replayed: discarded, superseded, stale, a failed
//!   attempt, the pipeline dropped, or adopted and never replayed, including a
//!   main-path `provider_call` cancelled before it asked for the reply.
//!   `"unsettled"` (`accounting_record: true`) is a call that started but had
//!   no outcome when the log was closed; its usage is unknown and stays
//!   absent. A main-path `provider_turn_duration` that is `aborted_by_user` or
//!   `timed_out` carries no usage, and when it ended before the replay it
//!   made no physical call. Known gap: when the replay was handed to
//!   `provider_call`'s worker but that call is cancelled or reaches its
//!   deadline before it receives the worker's result, the turn is an abort
//!   without usage while the call event already says `replayed`; closing it
//!   needs `provider_call` to report whether the turn carried the response.
//! - **Fields.** `current_phase_id`, `next_phase_id`, `reason` (the pipeline
//!   reason, plus `pipeline_dropped` and `adopted_unreplayed`), `call_index`,
//!   `disposition`, `accounting_record`, and `ok` once settled. Accounting
//!   records add, each only when the provider reported it, `prompt_eval_count`
//!   and `eval_count` (reply token counts), `provider_response_id`,
//!   `provider_model_id`, `provider_service_tier`,
//!   `provider_cached_input_tokens`, `provider_reasoning_tokens`,
//!   `provider_total_tokens`, and the `ResponseTiming` durations, with the same
//!   names as `provider_turn_duration`. An explicit zero is written as `0`; a
//!   missing value is never written as `0`. The replayed record carries only
//!   `provider_response_id` for correlation and `accounted_by`. No payload,
//!   prompt, reply text, key, or header is written.
//! - **Cancel/join.** Cancellation and joining are unchanged; only the worker
//!   thread is joined, never a `provider_call` thread that stopped waiting.
//!   Records are read when the log is closed after that join. The existing
//!   `speculative_phase_plan_*` events keep their exact bytes; the call events
//!   follow them (on adoption, discard, failure, and pipeline drop) or are
//!   written at replay or when an adopted reply is dropped unreplayed.

use std::path::PathBuf;
use std::sync::{
    Arc, Mutex, MutexGuard, PoisonError,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};

use crate::config::Config;
use crate::eval_events;
use crate::providers::{AssistantReply, ChatClient, ProviderResponseMetadata, ResponseTiming};
use crate::state::ConversationMessage;
use crate::tools::registry::ToolSpec;
use crate::tui::status::UiStatus;
use crate::tui::{InteractionUi, UiGuard};

use super::super::super::{
    PlannerSessionMode, build_step_plan_user_prompt, model_for, planner_chat_for_step_plan_attempt,
    resolve_profile_runtime, step_plan_messages,
};
use super::super::{UltraRunContext, effects::PhaseMachine};
use crate::planner::fix_runtime::FixRuntime;
use crate::planner::step_plan::StepPlan;
use crate::planner::ultra_plan::{UltraPhase, UltraPlan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PhasePlanKey {
    phase_id: String,
    phase_prompt: String,
}

impl PhasePlanKey {
    pub(super) fn new(phase_id: &str, phase_prompt: &str) -> Self {
        Self {
            phase_id: phase_id.to_string(),
            phase_prompt: phase_prompt.to_string(),
        }
    }
}

pub(super) struct PhaseRun {
    machine: PhaseMachine,
    pipeline: PhasePlanPipeline,
}

impl PhaseRun {
    pub(super) fn start() -> anyhow::Result<Self> {
        Ok(Self {
            machine: PhaseMachine::start()?,
            pipeline: PhasePlanPipeline::new(),
        })
    }

    #[allow(clippy::too_many_arguments, reason = "forwards borrows to pipeline")]
    pub(super) fn resolve(
        &mut self,
        planner: &mut dyn ChatClient,
        phase_prompt: &str,
        config: &Config,
        ui: &dyn InteractionUi,
        phase: &UltraPhase,
        plan: &UltraPlan,
        fix_runtime: Option<&FixRuntime>,
        preset_plan: bool,
        final_phase: bool,
    ) -> anyhow::Result<StepPlan> {
        self.pipeline.resolve(
            planner,
            phase_prompt,
            config,
            ui,
            phase,
            plan,
            fix_runtime,
            preset_plan,
            final_phase,
        )
    }

    pub(super) fn start_next(
        &mut self,
        planner: &dyn ChatClient,
        config: &Config,
        plan: &UltraPlan,
        phases: &[UltraPhase],
        index: usize,
        context: &UltraRunContext,
    ) {
        let runtime = resolve_profile_runtime(&plan.profile);
        let synthesized_create = config.plan_preset == crate::config::PlanPreset::Profile
            && config.resolved_run_intent()
                == crate::planner::adjudication::contract::IntentId::Create
            && plan.intent == "create"
            && crate::planner::ultra_preset::is_profile_preset_plan(config, plan)
            && runtime.synthesizes_create_plan();
        let promotion_possible = crate::planner::profile::ProfileId::parse(&plan.profile)
            == crate::planner::profile::ProfileId::Generic
            && !config.profile_explicit;
        let planning_allowed = !crate::planner::adjudication::contract::is_fix_intent(&plan.intent)
            && plan.intent != "investigate"
            && !synthesized_create
            && !promotion_possible;
        self.pipeline.start_next(
            planner,
            config,
            plan,
            phases,
            index,
            context,
            planning_allowed,
        );
    }

    pub(super) fn invariant_needs_repair(&mut self, config: &Config) -> anyhow::Result<()> {
        self.pipeline
            .cancel_and_discard(config, "verification_failed");
        self.machine.invariant_needs_repair()?;
        Ok(())
    }
}

impl std::ops::Deref for PhaseRun {
    type Target = PhaseMachine;

    fn deref(&self) -> &Self::Target {
        &self.machine
    }
}

impl std::ops::DerefMut for PhaseRun {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.machine
    }
}

struct PhasePlanPipeline {
    pending: Option<PendingPrefetch>,
}

impl PhasePlanPipeline {
    const fn new() -> Self {
        Self { pending: None }
    }

    #[allow(clippy::too_many_arguments, reason = "planner reused for fallback")]
    fn resolve(
        &mut self,
        planner: &mut dyn ChatClient,
        phase_prompt: &str,
        config: &Config,
        ui: &dyn InteractionUi,
        phase: &UltraPhase,
        plan: &UltraPlan,
        fix_runtime: Option<&FixRuntime>,
        preset_plan: bool,
        final_phase: bool,
    ) -> anyhow::Result<StepPlan> {
        let key = PhasePlanKey::new(&phase.id, phase_prompt);
        if let Some(mut prefetched_planner) = self.take_planner(&key, planner, config) {
            return super::phase_plan_resolution::resolve(
                prefetched_planner.as_mut(),
                phase_prompt,
                config,
                ui,
                phase,
                plan,
                fix_runtime,
                preset_plan,
                final_phase,
            );
        }
        super::phase_plan_resolution::resolve(
            planner,
            phase_prompt,
            config,
            ui,
            phase,
            plan,
            fix_runtime,
            preset_plan,
            final_phase,
        )
    }

    #[allow(clippy::too_many_arguments, reason = "prefetch borrows plan context")]
    fn start_next(
        &mut self,
        planner: &dyn ChatClient,
        config: &Config,
        plan: &UltraPlan,
        phases: &[UltraPhase],
        index: usize,
        context: &UltraRunContext,
        planning_allowed: bool,
    ) {
        let Some(next_phase) = planning_allowed.then(|| phases.get(index + 1)).flatten() else {
            return;
        };
        let phase_prompt =
            super::super::ultra_phase_prompt(plan, next_phase, config, context, None);
        let Ok(phase_prompt) =
            crate::planner::pack::runtime::append_phase_material_from_environment(
                phase_prompt,
                &config.workspace_root,
                &plan.profile,
                &plan.intent,
                &next_phase.id,
            )
        else {
            return;
        };
        self.start(
            planner,
            config,
            &phases[index].id,
            &next_phase.id,
            &phase_prompt,
        );
    }

    fn start(
        &mut self,
        planner: &dyn ChatClient,
        config: &Config,
        current_phase_id: &str,
        next_phase_id: &str,
        next_phase_prompt: &str,
    ) {
        self.start_with_log(
            planner,
            config,
            current_phase_id,
            next_phase_id,
            next_phase_prompt,
            SharedCallLog::default(),
        );
    }

    fn start_with_log(
        &mut self,
        planner: &dyn ChatClient,
        config: &Config,
        current_phase_id: &str,
        next_phase_id: &str,
        next_phase_prompt: &str,
        calls: SharedCallLog,
    ) {
        self.cancel_and_discard(config, "superseded");
        if !model_plan_has_provider_work(config, next_phase_prompt) {
            return;
        }

        let key = PhasePlanKey::new(next_phase_id, next_phase_prompt);
        let mut worker_client: Box<dyn ChatClient> =
            Box::new(RecordingClient::new(planner.boxed_clone(), calls.clone()));
        let mut worker_config = config.clone();
        worker_config.eval_events_path = None;
        let prompt = next_phase_prompt.to_string();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let Ok(handle) = thread::Builder::new()
            .name("phase-plan-prefetch".to_string())
            .spawn(move || {
                let ui = CancellationUi(worker_cancel);
                prefetch_first_reply(worker_client.as_mut(), &worker_config, &prompt, &ui)
            })
        else {
            emit_pipeline_event(
                config,
                "speculative_phase_plan_discarded",
                current_phase_id,
                next_phase_id,
                "worker_start_failed",
            );
            return;
        };
        self.pending = Some(PendingPrefetch {
            reply: PendingReply {
                key,
                current_phase_id: current_phase_id.to_string(),
                next_phase_id: next_phase_id.to_string(),
                cancel,
                handle: Some(handle),
            },
            calls,
            events_path: config.eval_events_path.clone(),
        });
        emit_pipeline_event(
            config,
            "speculative_phase_plan_started",
            current_phase_id,
            next_phase_id,
            "verification_overlap",
        );
    }

    fn take_planner(
        &mut self,
        key: &PhasePlanKey,
        fallback: &dyn ChatClient,
        config: &Config,
    ) -> Option<Box<dyn ChatClient>> {
        let pending = self.pending.take()?;
        let account = pending.account(config.eval_events_path.clone());
        let PendingPrefetch { reply, calls, .. } = pending;
        if reply.key != *key {
            reply.cancel_and_join();
            emit_pipeline_event(
                config,
                "speculative_phase_plan_discarded",
                &account.current_phase_id,
                &account.next_phase_id,
                "stale_input",
            );
            account.emit_records("stale_input", &calls.close().records, None);
            return None;
        }

        match reply.join() {
            Ok(reply) => {
                emit_pipeline_event(
                    config,
                    "speculative_phase_plan_adopted",
                    &account.current_phase_id,
                    &account.next_phase_id,
                    "gate_passed",
                );
                let log = calls.close();
                let paired = log.adopted_call();
                account.emit_records(
                    "gate_passed",
                    &log.records,
                    paired.as_ref().map(|record| record.call_index),
                );
                Some(Box::new(PrefetchedPlanner::adopted(
                    fallback,
                    PrefetchedReply {
                        reply,
                        call: paired.map(|record| PairedCall { record, account }),
                    },
                )))
            }
            Err(_) => {
                emit_pipeline_event(
                    config,
                    "speculative_phase_plan_discarded",
                    &account.current_phase_id,
                    &account.next_phase_id,
                    "prefetch_failed",
                );
                account.emit_records("prefetch_failed", &calls.close().records, None);
                None
            }
        }
    }

    pub(super) fn cancel_and_discard(&mut self, config: &Config, reason: &'static str) {
        let Some(pending) = self.pending.take() else {
            return;
        };
        let account = pending.account(config.eval_events_path.clone());
        pending.reply.cancel_and_join();
        emit_pipeline_event(
            config,
            "speculative_phase_plan_discarded",
            &account.current_phase_id,
            &account.next_phase_id,
            reason,
        );
        account.emit_records(reason, &pending.calls.close().records, None);
    }
}

impl Drop for PhasePlanPipeline {
    fn drop(&mut self) {
        if let Some(pending) = self.pending.take() {
            let account = pending.account(pending.events_path.clone());
            pending.reply.cancel_and_join();
            account.emit_records("pipeline_dropped", &pending.calls.close().records, None);
        }
    }
}

fn model_plan_has_provider_work(config: &Config, phase_prompt: &str) -> bool {
    if crate::planner::fix_runtime::is_before_prompt(phase_prompt)
        || crate::planner::fix_diagnostics::prompt_has_diagnostic(phase_prompt)
    {
        return false;
    }
    resolve_profile_runtime(&config.profile)
        .deterministic_step_plan(phase_prompt, &config.workspace_root, phase_prompt)
        .is_none()
}

fn prefetch_first_reply(
    planner: &mut dyn ChatClient,
    config: &Config,
    phase_prompt: &str,
    ui: &dyn InteractionUi,
) -> anyhow::Result<AssistantReply> {
    let mut prompt = build_step_plan_user_prompt(phase_prompt, config);
    if let Some(guidance) = resolve_profile_runtime(&config.profile).guidance(phase_prompt) {
        prompt.push_str("\n\nProfile contract:\n");
        prompt.push_str(&guidance);
        prompt.push_str(
            "\nInclude expected_paths on the final step so deterministic verification can catch missing artifacts.",
        );
    }
    let messages = step_plan_messages(&prompt);
    planner_chat_for_step_plan_attempt(
        planner,
        config,
        model_for(config, true),
        &messages,
        ui,
        PlannerSessionMode::Standard,
    )
}

struct PendingReply {
    key: PhasePlanKey,
    current_phase_id: String,
    next_phase_id: String,
    cancel: Arc<AtomicBool>,
    handle: Option<JoinHandle<anyhow::Result<AssistantReply>>>,
}

impl PendingReply {
    fn join(mut self) -> anyhow::Result<AssistantReply> {
        let handle = self
            .handle
            .take()
            .expect("pending phase-plan worker must have a join handle");
        handle
            .join()
            .map_err(|_| anyhow::anyhow!("speculative phase-plan worker panicked"))?
    }

    fn cancel_and_join(mut self) {
        self.cancel.store(true, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

struct CancellationUi(Arc<AtomicBool>);

impl InteractionUi for CancellationUi {
    fn before_model_call(&self, _label: &str) -> UiGuard {
        UiGuard::noop()
    }

    fn before_tool_call(&self, _name: &str) -> UiGuard {
        UiGuard::noop()
    }

    fn publish_status(&self, _status: UiStatus) {}

    fn interrupted(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// A running prefetch together with the log of the provider calls its worker
/// has made so far.
struct PendingPrefetch {
    reply: PendingReply,
    calls: SharedCallLog,
    events_path: Option<PathBuf>,
}

impl PendingPrefetch {
    fn account(&self, events_path: Option<PathBuf>) -> CallAccount {
        CallAccount {
            events_path,
            current_phase_id: self.reply.current_phase_id.clone(),
            next_phase_id: self.reply.next_phase_id.clone(),
        }
    }
}

/// The outcome of one physical prefetch call, read from the client that made
/// it right after the call returned.
#[derive(Debug, Clone, PartialEq)]
struct SettledCall {
    ok: bool,
    reply: Option<AssistantReply>,
    timing: Option<ResponseTiming>,
    metadata: Option<ProviderResponseMetadata>,
}

/// One physical prefetch call. `settled` stays `None` while the call has no
/// outcome, so its usage is unknown rather than zero.
#[derive(Debug, Clone, PartialEq)]
struct PrefetchCallRecord {
    call_index: usize,
    /// The generation of the `RecordingClient` instance that made the call.
    client_generation: usize,
    settled: Option<SettledCall>,
}

#[derive(Default)]
struct CallLogState {
    records: Vec<PrefetchCallRecord>,
    latest_generation: usize,
    closed: bool,
    #[cfg(test)]
    refused_begins: usize,
}

/// The records of a closed log and the generation of the latest client.
struct ClosedCallLog {
    records: Vec<PrefetchCallRecord>,
    latest_generation: usize,
}

impl ClosedCallLog {
    /// The call that produced the adopted reply: the call made by the latest
    /// client generation, which settled `ok` before handing the reply back.
    fn adopted_call(&self) -> Option<PrefetchCallRecord> {
        self.records
            .iter()
            .find(|record| {
                record.client_generation == self.latest_generation
                    && record.settled.as_ref().is_some_and(|settled| settled.ok)
            })
            .cloned()
    }
}

/// Test-only hook run before a call begins, with the caller's generation.
#[cfg(test)]
type BeforeBegin = Arc<dyn Fn(usize) + Send + Sync>;

#[derive(Clone, Default)]
struct SharedCallLog {
    state: Arc<Mutex<CallLogState>>,
    #[cfg(test)]
    before_begin: Option<BeforeBegin>,
}

impl SharedCallLog {
    fn lock(&self) -> MutexGuard<'_, CallLogState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The generation of a newly created client instance.
    fn next_generation(&self) -> usize {
        let mut state = self.lock();
        state.latest_generation += 1;
        state.latest_generation
    }

    /// Record the start of a call, or `None` once the log is closed, in which
    /// case the caller must not start a provider call.
    fn begin(&self, client_generation: usize) -> Option<usize> {
        #[cfg(test)]
        if let Some(before_begin) = &self.before_begin {
            before_begin(client_generation);
        }
        let mut state = self.lock();
        if state.closed {
            #[cfg(test)]
            {
                state.refused_begins += 1;
            }
            return None;
        }
        let call_index = state.records.len() + 1;
        state.records.push(PrefetchCallRecord {
            call_index,
            client_generation,
            settled: None,
        });
        Some(call_index)
    }

    fn settle(&self, call_index: usize, settled: SettledCall) {
        if let Some(record) = self.lock().records.get_mut(call_index - 1) {
            record.settled = Some(settled);
        }
    }

    /// Close the log and read its records in the same step, so every call
    /// either is in the returned records or never starts.
    fn close(&self) -> ClosedCallLog {
        let mut state = self.lock();
        state.closed = true;
        ClosedCallLog {
            records: state.records.clone(),
            latest_generation: state.latest_generation,
        }
    }

    #[cfg(test)]
    fn snapshot(&self) -> Vec<PrefetchCallRecord> {
        self.lock().records.clone()
    }
}

/// The prefetch worker's client. It forwards every capability to the wrapped
/// clone and logs each physical call with that call's own timing and metadata.
struct RecordingClient {
    inner: Box<dyn ChatClient>,
    calls: SharedCallLog,
    generation: usize,
    timing: Option<ResponseTiming>,
    metadata: Option<ProviderResponseMetadata>,
}

impl RecordingClient {
    fn new(inner: Box<dyn ChatClient>, calls: SharedCallLog) -> Self {
        let generation = calls.next_generation();
        Self {
            inner,
            calls,
            generation,
            timing: None,
            metadata: None,
        }
    }

    fn record(
        &mut self,
        call: impl FnOnce(&mut dyn ChatClient) -> anyhow::Result<AssistantReply>,
    ) -> anyhow::Result<AssistantReply> {
        self.timing = None;
        self.metadata = None;
        let Some(call_index) = self.calls.begin(self.generation) else {
            anyhow::bail!(
                "the speculative phase-plan prefetch is already accounted; no provider call started"
            );
        };
        let result = call(self.inner.as_mut());
        self.timing = self.inner.take_response_timing();
        self.metadata = self.inner.take_response_metadata();
        self.calls.settle(
            call_index,
            SettledCall {
                ok: result.is_ok(),
                reply: result.as_ref().ok().cloned(),
                timing: self.timing.clone(),
                metadata: self.metadata.clone(),
            },
        );
        result
    }
}

impl ChatClient for RecordingClient {
    fn label(&self) -> &str {
        self.inner.label()
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(Self::new(self.inner.boxed_clone(), self.calls.clone()))
    }

    fn supports_native_tools(&self, model: &str) -> bool {
        self.inner.supports_native_tools(model)
    }

    fn allows_xml_fallback(&self) -> bool {
        self.inner.allows_xml_fallback()
    }

    fn parses_xml_tool_calls_before_host(&self) -> bool {
        self.inner.parses_xml_tool_calls_before_host()
    }

    fn supports_ollama_think(&self) -> bool {
        self.inner.supports_ollama_think()
    }

    fn take_response_timing(&mut self) -> Option<ResponseTiming> {
        self.timing.take()
    }

    fn take_response_metadata(&mut self) -> Option<ProviderResponseMetadata> {
        self.metadata.take()
    }

    fn set_cancel_token(&mut self, token: crate::providers::streaming::CancelToken) {
        self.inner.set_cancel_token(token);
    }

    fn supports_streaming(&self) -> bool {
        self.inner.supports_streaming()
    }

    fn supports_streaming_for_model(&self, model: &str) -> bool {
        self.inner.supports_streaming_for_model(model)
    }

    fn chat_stream(
        &mut self,
        model: &str,
        messages: &[ConversationMessage],
        tools: &[ToolSpec],
        native_tools_enabled: bool,
        on_chunk: &mut dyn FnMut(&str) -> anyhow::Result<()>,
    ) -> anyhow::Result<AssistantReply> {
        self.record(|inner| {
            inner.chat_stream(model, messages, tools, native_tools_enabled, on_chunk)
        })
    }

    fn chat(
        &mut self,
        model: &str,
        messages: &[ConversationMessage],
        tools: &[ToolSpec],
        native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        self.record(|inner| ChatClient::chat(inner, model, messages, tools, native_tools_enabled))
    }
}

/// Where and under which phase pair a prefetch's call events are written.
struct CallAccount {
    events_path: Option<PathBuf>,
    current_phase_id: String,
    next_phase_id: String,
}

impl CallAccount {
    /// Write one accounting event per record, except the replayed call, whose
    /// usage the replay's `provider_turn_duration` counts.
    fn emit_records(&self, reason: &str, records: &[PrefetchCallRecord], replayed: Option<usize>) {
        for record in records
            .iter()
            .filter(|record| Some(record.call_index) != replayed)
        {
            self.emit(reason, record, false);
        }
    }

    fn emit(&self, reason: &str, record: &PrefetchCallRecord, replayed: bool) {
        eval_events::emit(
            self.events_path.as_deref(),
            provider_call_event(self, reason, record, replayed),
        );
    }
}

fn provider_call_event(
    account: &CallAccount,
    reason: &str,
    record: &PrefetchCallRecord,
    replayed: bool,
) -> serde_json::Value {
    let disposition = if replayed {
        "replayed"
    } else if record.settled.is_some() {
        "unconsumed"
    } else {
        "unsettled"
    };
    let mut value = serde_json::json!({
        "event": PROVIDER_CALL_EVENT,
        "current_phase_id": account.current_phase_id,
        "next_phase_id": account.next_phase_id,
        "reason": reason,
        "call_index": record.call_index,
        "disposition": disposition,
        "accounting_record": !replayed,
    });
    let Some(settled) = &record.settled else {
        return value;
    };
    value["ok"] = serde_json::json!(settled.ok);
    let metadata = settled.metadata.as_ref();
    insert_reported(
        &mut value,
        "provider_response_id",
        metadata.and_then(|metadata| metadata.response_id.clone()),
    );
    if replayed {
        value["accounted_by"] = serde_json::json!("provider_turn_duration");
        return value;
    }
    let reply = settled.reply.as_ref();
    let timing = settled.timing.as_ref();
    insert_reported(
        &mut value,
        "prompt_eval_count",
        reply.and_then(|reply| reply.prompt_tokens),
    );
    insert_reported(
        &mut value,
        "eval_count",
        reply.and_then(|reply| reply.completion_tokens),
    );
    insert_reported(
        &mut value,
        "provider_model_id",
        metadata.and_then(|metadata| metadata.model_id.clone()),
    );
    insert_reported(
        &mut value,
        "provider_service_tier",
        metadata.and_then(|metadata| metadata.service_tier.clone()),
    );
    insert_reported(
        &mut value,
        "provider_cached_input_tokens",
        metadata.and_then(|metadata| metadata.cached_input_tokens),
    );
    insert_reported(
        &mut value,
        "provider_reasoning_tokens",
        metadata.and_then(|metadata| metadata.reasoning_tokens),
    );
    insert_reported(
        &mut value,
        "provider_total_tokens",
        metadata.and_then(|metadata| metadata.total_tokens),
    );
    insert_reported(
        &mut value,
        "prompt_eval_duration",
        timing.and_then(|timing| timing.prompt_eval_duration),
    );
    insert_reported(
        &mut value,
        "eval_duration",
        timing.and_then(|timing| timing.eval_duration),
    );
    insert_reported(
        &mut value,
        "load_duration",
        timing.and_then(|timing| timing.load_duration),
    );
    insert_reported(
        &mut value,
        "total_duration",
        timing.and_then(|timing| timing.total_duration),
    );
    value
}

/// Write a provider-reported value; an unreported one stays absent instead of
/// becoming zero.
fn insert_reported<T: serde::Serialize>(
    value: &mut serde_json::Value,
    key: &str,
    field: Option<T>,
) {
    if let Some(field) = field {
        value[key] = serde_json::json!(field);
    }
}

struct PrefetchedReply {
    reply: AssistantReply,
    call: Option<PairedCall>,
}

struct PairedCall {
    record: PrefetchCallRecord,
    account: CallAccount,
}

/// Holds the adopted reply until it is replayed. Dropping it unreplayed writes
/// the paired call as an accounting record, so its usage is not lost.
struct ReplaySlot(Option<PrefetchedReply>);

impl Drop for ReplaySlot {
    fn drop(&mut self) {
        if let Some(PrefetchedReply {
            call: Some(call), ..
        }) = self.0.take()
        {
            call.account.emit("adopted_unreplayed", &call.record, false);
        }
    }
}

/// What this instance's last chat call was, so `take_response_*` answers for
/// that call only.
enum LastCall {
    NoCall,
    Replayed(Box<ReplayedCall>),
    Fallback,
}

/// The timing and metadata of the call whose reply was replayed, each taken
/// once.
struct ReplayedCall {
    timing: Option<ResponseTiming>,
    metadata: Option<ProviderResponseMetadata>,
}

struct PrefetchedPlanner {
    first_reply: Arc<Mutex<ReplaySlot>>,
    fallback: Box<dyn ChatClient>,
    last_call: LastCall,
}

impl PrefetchedPlanner {
    #[cfg(test)]
    fn new(fallback: &dyn ChatClient, reply: AssistantReply) -> Self {
        Self::adopted(fallback, PrefetchedReply { reply, call: None })
    }

    fn adopted(fallback: &dyn ChatClient, reply: PrefetchedReply) -> Self {
        Self {
            first_reply: Arc::new(Mutex::new(ReplaySlot(Some(reply)))),
            fallback: fallback.boxed_clone(),
            last_call: LastCall::NoCall,
        }
    }

    /// Replay the adopted reply once, with the timing and metadata of the call
    /// that produced it.
    fn replay(&mut self) -> Option<AssistantReply> {
        let PrefetchedReply { reply, call } = self.first_reply.lock().ok()?.0.take()?;
        let (timing, metadata) = match call {
            Some(call) => {
                call.account.emit("gate_passed", &call.record, true);
                call.record
                    .settled
                    .map_or((None, None), |settled| (settled.timing, settled.metadata))
            }
            None => (None, None),
        };
        self.last_call = LastCall::Replayed(Box::new(ReplayedCall { timing, metadata }));
        Some(reply)
    }
}

impl ChatClient for PrefetchedPlanner {
    fn label(&self) -> &str {
        self.fallback.label()
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(Self {
            first_reply: Arc::clone(&self.first_reply),
            fallback: self.fallback.boxed_clone(),
            last_call: LastCall::NoCall,
        })
    }

    fn supports_native_tools(&self, model: &str) -> bool {
        self.fallback.supports_native_tools(model)
    }

    fn allows_xml_fallback(&self) -> bool {
        self.fallback.allows_xml_fallback()
    }

    fn supports_ollama_think(&self) -> bool {
        self.fallback.supports_ollama_think()
    }

    fn take_response_timing(&mut self) -> Option<ResponseTiming> {
        if let LastCall::Replayed(call) = &mut self.last_call {
            return call.timing.take();
        }
        self.fallback.take_response_timing()
    }

    fn take_response_metadata(&mut self) -> Option<ProviderResponseMetadata> {
        if let LastCall::Replayed(call) = &mut self.last_call {
            return call.metadata.take();
        }
        self.fallback.take_response_metadata()
    }

    fn set_cancel_token(&mut self, token: crate::providers::streaming::CancelToken) {
        self.fallback.set_cancel_token(token);
    }

    fn supports_streaming(&self) -> bool {
        self.fallback.supports_streaming()
    }

    fn supports_streaming_for_model(&self, model: &str) -> bool {
        self.fallback.supports_streaming_for_model(model)
    }

    fn chat_stream(
        &mut self,
        model: &str,
        messages: &[ConversationMessage],
        tools: &[ToolSpec],
        native_tools_enabled: bool,
        on_chunk: &mut dyn FnMut(&str) -> anyhow::Result<()>,
    ) -> anyhow::Result<AssistantReply> {
        if let Some(reply) = self.replay() {
            return Ok(reply);
        }
        self.last_call = LastCall::Fallback;
        self.fallback
            .chat_stream(model, messages, tools, native_tools_enabled, on_chunk)
    }

    fn chat(
        &mut self,
        model: &str,
        messages: &[ConversationMessage],
        tools: &[ToolSpec],
        native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        if let Some(reply) = self.replay() {
            return Ok(reply);
        }
        self.last_call = LastCall::Fallback;
        ChatClient::chat(
            self.fallback.as_mut(),
            model,
            messages,
            tools,
            native_tools_enabled,
        )
    }
}

/// One event per physical prefetch provider call (Issue #948).
const PROVIDER_CALL_EVENT: &str = "speculative_phase_plan_provider_call";

fn emit_pipeline_event(
    config: &Config,
    event: &'static str,
    current_phase_id: &str,
    next_phase_id: &str,
    reason: &'static str,
) {
    eval_events::emit(
        config.eval_events_path.as_deref(),
        serde_json::json!({
            "event": event,
            "current_phase_id": current_phase_id,
            "next_phase_id": next_phase_id,
            "reason": reason,
        }),
    );
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
    use std::time::{Duration, Instant};

    use super::*;

    /// Diagnostic ceiling for the test-side handshakes below. It never decides a
    /// pass on its own: the verdict comes from the signal that was actually
    /// observed, and this bound only turns a missing signal into a bounded
    /// failure instead of an unbounded wait.
    const HANDSHAKE_DEADLINE: Duration = Duration::from_secs(5);

    fn recv_within<T>(rx: &Receiver<T>, what: &str) -> T {
        match rx.recv_timeout(HANDSHAKE_DEADLINE) {
            Ok(value) => value,
            Err(RecvTimeoutError::Timeout) => panic!("timed out waiting for {what}"),
            Err(RecvTimeoutError::Disconnected) => {
                panic!("the worker stopped before signalling {what}")
            }
        }
    }

    fn pending_with_job(
        key: PhasePlanKey,
        cancel: Arc<AtomicBool>,
        job: impl FnOnce(Arc<AtomicBool>) -> anyhow::Result<AssistantReply> + Send + 'static,
    ) -> PendingReply {
        let worker_cancel = Arc::clone(&cancel);
        PendingReply {
            key,
            current_phase_id: "phase-n".to_string(),
            next_phase_id: "phase-n-plus-one".to_string(),
            cancel,
            handle: Some(thread::spawn(move || job(worker_cancel))),
        }
    }

    #[test]
    fn speculative_reply_runs_inside_the_open_verification_window() {
        let (started_tx, started_rx) = mpsc::channel();
        let (open_tx, open_rx) = mpsc::channel::<()>();
        let (observed_tx, observed_rx) = mpsc::channel::<bool>();
        let pending = pending_with_job(
            PhasePlanKey::new("next", "prompt"),
            Arc::new(AtomicBool::new(false)),
            move |_| {
                let _ = started_tx.send(());
                // Block on the window signal instead of spinning a fixed
                // deadline: a lost signal can no longer race the parent.
                let overlap_observed = open_rx.recv_timeout(HANDSHAKE_DEADLINE).is_ok();
                let _ = observed_tx.send(overlap_observed);
                if !overlap_observed {
                    anyhow::bail!("the verification window never opened before the deadline");
                }
                Ok(AssistantReply::text("prefetched"))
            },
        );

        // Only open the window once the worker is provably running, so the
        // overlap is a real concurrency claim rather than a sleep race.
        recv_within(&started_rx, "the speculative worker start");
        open_tx.send(()).expect("open the verification window");
        let overlap_observed = recv_within(&observed_rx, "the overlap observation");
        assert!(
            overlap_observed,
            "the worker must observe the open verification window before joining"
        );

        let reply = pending.join().unwrap();
        assert_eq!(reply.content, "prefetched");
    }

    #[test]
    fn failed_gate_cancels_and_discards_speculative_reply() {
        let (started_tx, started_rx) = mpsc::channel();
        let (observed_tx, observed_rx) = mpsc::channel::<(bool, Option<AssistantReply>)>();
        let pending = pending_with_job(
            PhasePlanKey::new("next", "prompt"),
            Arc::new(AtomicBool::new(false)),
            move |cancel| {
                let _ = started_tx.send(());
                let deadline = Instant::now() + HANDSHAKE_DEADLINE;
                while !cancel.load(Ordering::Acquire) && Instant::now() < deadline {
                    thread::yield_now();
                }
                // Read the product cancellation flag itself; a test-only signal
                // must never stand in for the observed flag.
                let cancel_observed = cancel.load(Ordering::Acquire);
                let handed_back = (!cancel_observed).then(|| AssistantReply::text("prefetched"));
                let _ = observed_tx.send((cancel_observed, handed_back));
                if !cancel_observed {
                    anyhow::bail!("cancellation was never observed before the deadline");
                }
                anyhow::bail!("cancelled: discard the speculative reply")
            },
        );

        // Set cancellation only after the worker is provably running and polling
        // the flag, so the observation cannot race the parent.
        recv_within(&started_rx, "the speculative worker start");
        pending.cancel_and_join();

        let (cancel_observed, handed_back) =
            recv_within(&observed_rx, "the cancellation observation");
        assert!(
            cancel_observed,
            "the worker must observe the cancellation flag"
        );
        assert!(
            handed_back.is_none(),
            "a cancelled worker must discard the speculative reply"
        );
    }

    #[test]
    fn phase_plan_key_rejects_prompt_changed_at_the_gate() {
        let planned = PhasePlanKey::new("next", "profile: generic");
        let promoted = PhasePlanKey::new("next", "profile: nextjs");

        assert_ne!(planned, promoted);
    }

    #[test]
    fn adopted_reply_is_consumed_once_before_falling_back() {
        #[derive(Clone)]
        struct Fallback(Arc<Mutex<usize>>);

        impl ChatClient for Fallback {
            fn label(&self) -> &str {
                "fallback"
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
                *self.0.lock().unwrap() += 1;
                Ok(AssistantReply::text("fallback"))
            }
        }

        let calls = Arc::new(Mutex::new(0));
        let fallback = Fallback(Arc::clone(&calls));
        let mut planner = PrefetchedPlanner::new(&fallback, AssistantReply::text("prefetched"));

        let first = ChatClient::chat(&mut planner, "model", &[], &[], false).unwrap();
        let second = ChatClient::chat(&mut planner, "model", &[], &[], false).unwrap();

        assert_eq!(first.content, "prefetched");
        assert_eq!(second.content, "fallback");
        assert_eq!(*calls.lock().unwrap(), 1);
    }

    #[test]
    fn prefetched_planner_forwards_the_cancel_token_to_the_fallback() {
        use crate::providers::streaming::CancelToken;

        #[derive(Clone)]
        struct Recording(Arc<Mutex<Vec<CancelToken>>>);

        impl ChatClient for Recording {
            fn label(&self) -> &str {
                "recording"
            }

            fn boxed_clone(&self) -> Box<dyn ChatClient> {
                Box::new(self.clone())
            }

            fn set_cancel_token(&mut self, token: CancelToken) {
                self.0.lock().unwrap().push(token);
            }

            fn chat(
                &mut self,
                _model: &str,
                _messages: &[ConversationMessage],
                _tools: &[ToolSpec],
                _native_tools_enabled: bool,
            ) -> anyhow::Result<AssistantReply> {
                Ok(AssistantReply::text("fallback"))
            }
        }

        let received = Arc::new(Mutex::new(Vec::new()));
        let fallback = Recording(Arc::clone(&received));
        let planner = PrefetchedPlanner::new(&fallback, AssistantReply::text("prefetched"));

        // The same order `provider_call` uses: clone for the worker, then hand
        // it the call's token.
        let mut worker = planner.boxed_clone();
        let token = CancelToken::default();
        worker.set_cancel_token(token.clone());

        let recorded = received.lock().unwrap();
        assert_eq!(recorded.len(), 1, "the fallback never got the token");
        assert!(!recorded[0].is_cancelled());
        token.cancel();
        assert!(recorded[0].is_cancelled(), "a different token arrived");
    }
}

#[cfg(test)]
#[path = "pipeline/usage_tests.rs"]
mod usage_tests;
