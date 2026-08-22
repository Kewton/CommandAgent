use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};

use crate::config::Config;
use crate::eval_events;
use crate::providers::{AssistantReply, ChatClient};
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

    #[allow(clippy::too_many_arguments)]
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
    pending: Option<PendingReply>,
}

impl PhasePlanPipeline {
    const fn new() -> Self {
        Self { pending: None }
    }

    #[allow(clippy::too_many_arguments)]
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

    #[allow(clippy::too_many_arguments)]
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
        self.cancel_and_discard(config, "superseded");
        if !model_plan_has_provider_work(config, next_phase_prompt) {
            return;
        }

        let key = PhasePlanKey::new(next_phase_id, next_phase_prompt);
        let mut worker_client = planner.boxed_clone();
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
        self.pending = Some(PendingReply {
            key,
            current_phase_id: current_phase_id.to_string(),
            next_phase_id: next_phase_id.to_string(),
            cancel,
            handle: Some(handle),
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
        let current_phase_id = pending.current_phase_id.clone();
        let next_phase_id = pending.next_phase_id.clone();
        if pending.key != *key {
            pending.cancel_and_join();
            emit_pipeline_event(
                config,
                "speculative_phase_plan_discarded",
                &current_phase_id,
                &next_phase_id,
                "stale_input",
            );
            return None;
        }

        match pending.join() {
            Ok(reply) => {
                emit_pipeline_event(
                    config,
                    "speculative_phase_plan_adopted",
                    &current_phase_id,
                    &next_phase_id,
                    "gate_passed",
                );
                Some(Box::new(PrefetchedPlanner::new(fallback, reply)))
            }
            Err(_) => {
                emit_pipeline_event(
                    config,
                    "speculative_phase_plan_discarded",
                    &current_phase_id,
                    &next_phase_id,
                    "prefetch_failed",
                );
                None
            }
        }
    }

    pub(super) fn cancel_and_discard(&mut self, config: &Config, reason: &'static str) {
        let Some(pending) = self.pending.take() else {
            return;
        };
        let current_phase_id = pending.current_phase_id.clone();
        let next_phase_id = pending.next_phase_id.clone();
        pending.cancel_and_join();
        emit_pipeline_event(
            config,
            "speculative_phase_plan_discarded",
            &current_phase_id,
            &next_phase_id,
            reason,
        );
    }
}

impl Drop for PhasePlanPipeline {
    fn drop(&mut self) {
        if let Some(pending) = self.pending.take() {
            pending.cancel_and_join();
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

struct PrefetchedPlanner {
    first_reply: Arc<Mutex<Option<AssistantReply>>>,
    fallback: Box<dyn ChatClient>,
}

impl PrefetchedPlanner {
    fn new(fallback: &dyn ChatClient, reply: AssistantReply) -> Self {
        Self {
            first_reply: Arc::new(Mutex::new(Some(reply))),
            fallback: fallback.boxed_clone(),
        }
    }

    fn take_first_reply(&self) -> Option<AssistantReply> {
        self.first_reply.lock().ok()?.take()
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

    fn take_response_timing(&mut self) -> Option<crate::providers::ResponseTiming> {
        self.fallback.take_response_timing()
    }

    fn take_response_metadata(&mut self) -> Option<crate::providers::ProviderResponseMetadata> {
        self.fallback.take_response_metadata()
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
        if let Some(reply) = self.take_first_reply() {
            return Ok(reply);
        }
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
        if let Some(reply) = self.take_first_reply() {
            return Ok(reply);
        }
        ChatClient::chat(
            self.fallback.as_mut(),
            model,
            messages,
            tools,
            native_tools_enabled,
        )
    }
}

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
    use std::time::{Duration, Instant};

    use super::*;

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
        let verification_open = Arc::new(AtomicBool::new(false));
        let overlap_observed = Arc::new(AtomicBool::new(false));
        let open_for_worker = Arc::clone(&verification_open);
        let observed_by_worker = Arc::clone(&overlap_observed);
        let pending = pending_with_job(
            PhasePlanKey::new("next", "prompt"),
            Arc::new(AtomicBool::new(false)),
            move |_| {
                let deadline = Instant::now() + Duration::from_secs(1);
                while !open_for_worker.load(Ordering::Acquire) && Instant::now() < deadline {
                    thread::yield_now();
                }
                observed_by_worker
                    .store(open_for_worker.load(Ordering::Acquire), Ordering::Release);
                Ok(AssistantReply::text("prefetched"))
            },
        );

        verification_open.store(true, Ordering::Release);
        let reply = pending.join().unwrap();

        assert!(overlap_observed.load(Ordering::Acquire));
        assert_eq!(reply.content, "prefetched");
    }

    #[test]
    fn failed_gate_cancels_and_discards_speculative_reply() {
        let cancellation_observed = Arc::new(AtomicBool::new(false));
        let observed_by_worker = Arc::clone(&cancellation_observed);
        let pending = pending_with_job(
            PhasePlanKey::new("next", "prompt"),
            Arc::new(AtomicBool::new(false)),
            move |cancel| {
                let deadline = Instant::now() + Duration::from_secs(1);
                while !cancel.load(Ordering::Acquire) && Instant::now() < deadline {
                    thread::yield_now();
                }
                observed_by_worker.store(cancel.load(Ordering::Acquire), Ordering::Release);
                anyhow::bail!("cancelled")
            },
        );

        pending.cancel_and_join();

        assert!(cancellation_observed.load(Ordering::Acquire));
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
}
