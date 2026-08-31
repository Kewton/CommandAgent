// StepPlan execution, bounded repair, and phase-owned context.
// Pure extraction from the E-5d responsibility map (pre-split runner.rs:960-3602).
#[allow(unused_imports)]
use super::{
    BTreeSet, BoundCompletionContract, BuildVerifierLifecycleObservation, BuildVerifierObservation,
    BuildVerifierRequirement, BuildVerifierStatus, CarriedPressure, ChatClient,
    CompileRollbackOutcome, Config, ContractEnforcement, DepthProfile, EscalationCarryoverHandle,
    FINAL_ACCEPTANCE_COMPILE_NO_SNAPSHOT_EXTRA_ATTEMPTS,
    FINAL_ACCEPTANCE_EVIDENCE_NO_CHANGE_EXTRA_ATTEMPTS, FINAL_ACCEPTANCE_REPAIR_MAX_ATTEMPTS,
    FINAL_ACCEPTANCE_REPAIR_WALL_CLOCK_CAP, FinalAcceptanceCycleDelta,
    GENERIC_INTERACTIVE_CONTRACT_CAPABILITY, GENERIC_INTERACTIVE_EVIDENCE_KEYS, Instant,
    InteractionTextTelemetry, InteractionUi, NOOP_UI, NodeDependencySetupAuthority,
    NodeDependencySetupRequirement, NodeDependencySetupStatus, Path, PathBuf,
    PhaseVerificationMode, PlanAdherenceReport, PlanLintReport, PlanStep, ProfileId,
    ProfileInferenceSource, ProfileRuntime, ProfileRuntimeRegistry, ProfileSnapshot,
    ProviderCallScope, RecoveryArtifactValidation, RecoveryHandoff, ReleaseRecoveryHandoffSummary,
    RepairContext, RepairFollowThrough, RepairSessionMode, RunSessionOptions, RunSessionOutcome,
    RunSessionStepKind, RunStopReason, RuntimeAcceptanceReport,
    STEP_REPAIR_IDENTICAL_NO_CHANGE_LIMIT, STEP_REPAIR_MAX_ITERATIONS, STEP_REPAIR_MAX_TURNS,
    STEP_TURN_MAX_ITERATIONS, SessionSnapshot, StepKind, StepPlan, ULTRA_PLAN_GENERATION_ATTEMPTS,
    UiStatus, UltraPhase, UltraPlan, Value, VerificationReport, append_context_list,
    append_final_acceptance_cycle_summary, apply_session_error_observations, attach_to_options,
    bind_completion_contract_for_acceptance, bounded_process,
    build_compact_compile_repair_prompt_with_context,
    build_compile_regeneration_prompt_with_context,
    build_final_acceptance_evidence_regeneration_prompt, build_repair_prompt_with_context,
    build_step_prompt, build_ultra_plan_lint_retry_prompt, build_ultra_plan_schema_retry_prompt,
    build_ultra_plan_tool_call_retry_prompt, capability_evidence_failure_evidence,
    capability_evidence_remedy_lines, capability_evidence_unresolved_reason,
    changed_paths_only_target, changed_snapshot_paths, classify_repair_follow_through,
    classify_repair_target, clear_final_acceptance_browser_probe_evidence,
    command_failure_summaries, comment_stripped_source_corpus,
    contract_attribute_repair_target_paths, contract_origin_for_acceptance, dependency_setup,
    effective_requested_port, emit_compile_no_snapshot_narrow_retry,
    emit_compile_regeneration_event, emit_compile_rollback_context_carried,
    emit_dependency_build_lifecycle, emit_evidence_regeneration_event,
    emit_final_acceptance_cycle_delta, emit_planner_error, emit_planner_error_for_lint,
    emit_repair_unreachable, emit_step_prompt_contract, emit_ultra_context_initialized,
    emit_ultra_phase_context_attached, emit_ultra_phase_context_updated,
    emit_ultra_plan_generation_attempt, emit_ultra_plan_generation_failed,
    emit_ultra_plan_generation_metadata_normalized, emit_ultra_plan_generation_retry,
    emit_ultra_plan_generation_succeeded, emit_ultra_plan_generation_tool_call_rejected,
    emit_ultra_plan_raw_output_shape, eval_events, evidence_repair_retry_mode,
    evidence_repair_zero_edit_eligible, exhaustion_reason_with_pending_contract_state,
    extract_requested_artifact_paths, final_acceptance_app_behavior_failure_kind,
    final_acceptance_evidence_regeneration_target, final_acceptance_recovery_failure_evidence,
    final_acceptance_recovery_failure_evidence_with_context, final_acceptance_recovery_reason,
    final_acceptance_recovery_repair_targets, final_acceptance_repair_expected_paths,
    final_acceptance_repair_prompt_with_events, final_acceptance_repair_signals,
    final_acceptance_source_snapshot, format_missing_import_findings,
    fresh_profile_invariant_failure_evidence, generate_step_plan_with_ui_for_phase, handoff_path,
    hook_snapshot, infer_profile, interaction_probe, interaction_probe_performed_for_run, json,
    lint_ultra_plan_report, merge_changed_files, merge_repair_target_paths, merge_unique_strings,
    missing_final_artifacts, model_for, normalize_ultra_plan_metadata, optional_handoff_path,
    parse_ultra_plan, pending_capability_context_items, planner_chat_with_request_retry,
    planner_stage_and_kind_for_lint, profile_before_plan,
    profile_invariant_offending_file_excerpts, profile_missing_relative_imports,
    push_context_items_capped, push_context_unique_capped, raw_bool_field_deep,
    reachability_action_labels, reachability_blocked_evidence, reachability_failure_kind,
    reachability_recovery_reason, recovery_artifact_check_summary, render_bounded_prompt_section,
    render_prompt_bullets, render_ultra_plan, repair_targeting, resolve_plan_file_path,
    resolve_profile_runtime, resolved_missing_signals, restart_hook_attachment_guidance,
    route_bound_changed_paths, run_final_acceptance_repair_with_carryover,
    run_profile_repair_with_ultra_session, run_session_with_outcome_with_options,
    runtime_missing_signals, safe_source_rel_path, save_recovery_ultra_plan,
    save_repair_report_with_context, save_step_plan, save_ultra_recovery_prompt,
    should_run_compile_no_snapshot_narrow_retry, signals, single_compile_regeneration_target,
    snapshot_last_known_good_sources, step_expected_result, step_run_outcome_from_session_error,
    suggested_recovery_ultra_plan_command, suggested_ultra_recovery_command, tool_call_names,
    try_compile_rollback_after_repair_exhaustion, try_final_acceptance_compile_regeneration,
    ultra_contract_runtime_acceptance_report,
    ultra_final_acceptance_report_with_deterministic_remedies, ultra_phase_prompt,
    ultra_plan_generation_messages, validate_recovery_artifacts, verification_missing_signals,
    verification_report_signature, verify_invariant_with_hooks, verify_plan_final_contract,
    verify_setup_dependency_state_with_setup_observed_with_offline, verify_step_with_context,
    verify_step_with_profile_setup_observed_with_offline, writable_workspace_source_path,
};

#[path = "phase/effects.rs"]
mod effects;
#[path = "phase/flow.rs"]
mod flow;
#[path = "phase/intent_completion.rs"]
mod intent_completion;
#[path = "phase/plan_step_events.rs"]
mod plan_step_events;
#[path = "phase/state.rs"]
mod state;
#[path = "phase/step_plan_execution.rs"]
mod step_plan_execution;
#[path = "phase/transition.rs"]
mod transition;
pub use flow::{
    generate_and_run_ultra_plan, generate_and_run_ultra_plan_with_ui, generate_ultra_plan,
    generate_ultra_plan_with_ui, run_ultra_plan, run_ultra_plan_file, run_ultra_plan_file_with_ui,
    run_ultra_plan_with_ui, save_ultra_plan,
};
use plan_step_events::PlanStepEvents;
pub(super) use step_plan_execution::{
    run_step_plan_with_session_with_ui, run_step_plan_with_session_with_ui_and_run_authority,
};

#[derive(Debug, Clone, Default)]
pub(super) struct StepPlanRunOutcome {
    pub(super) summary: String,
    pub(super) completed_steps: usize,
    pub(super) total_steps: usize,
    pub(super) changed_paths: Vec<String>,
    pub(super) observed_missing_capabilities: Vec<String>,
    pub(super) observed_missing_evidence: Vec<String>,
    pub(super) observed_missing_obligations: Vec<String>,
    pub(super) verify_failures: Vec<String>,
    pub(super) primary_failure: Option<String>,
    pub(super) repair_targets: Vec<String>,
    pub(super) command_failures: Vec<String>,
    pub(super) repair_attempts: usize,
    pub(super) repair_changed_paths: Vec<String>,
    pub(super) compile_rollbacks: Vec<CompileRollbackOutcome>,
    pub(super) stop_reason: Option<String>,
    pub(super) partial: bool,
}

#[derive(Debug, Clone, Default)]
pub(super) struct UltraRunContext {
    pub(super) completed_phases: Vec<String>,
    pub(super) created_or_changed_paths: Vec<String>,
    pub(super) last_failed_phase: Option<String>,
    pub(super) last_verify_failures: Vec<String>,
    pub(super) last_repair_changed_paths: Vec<String>,
    pub(super) pending_final_artifacts: Vec<String>,
    pub(super) pending_capability_evidence: Vec<String>,
    pub(super) unresolved_repair_targets: Vec<String>,
    pub(super) carry_forward_guidance: Vec<String>,
    pub(super) truncated: bool,
}

#[derive(Debug, Clone)]
pub(super) struct ProfilePromotionState {
    pub(super) eligible: bool,
    pub(super) promoted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ProfilePromotion {
    pub(super) id: String,
    pub(super) at_phase: usize,
    pub(super) phase_id: String,
    pub(super) requested_port: Option<String>,
    pub(super) contract_origin: String,
    pub(super) delta_capabilities: Vec<String>,
    pub(super) delta_requirements: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct ContractRequirements {
    pub(super) capabilities: Vec<String>,
    pub(super) evidence: Vec<String>,
    pub(super) obligations: Vec<String>,
}

pub(super) const ULTRA_CONTEXT_MAX_PHASES: usize = 12;
pub(super) const ULTRA_CONTEXT_MAX_PATHS: usize = 24;
pub(super) const ULTRA_CONTEXT_MAX_MESSAGES: usize = 10;
pub(super) const ULTRA_PROMPT_GUIDANCE_MAX_LINES: usize = 8;

impl ProfilePromotionState {
    pub(super) fn for_run(plan: &UltraPlan, config: &Config) -> Self {
        Self {
            eligible: !crate::planner::fix_runtime::applies(plan)
                && ProfileId::parse(&plan.profile) == ProfileId::Generic
                && !config.profile_explicit,
            promoted: false,
        }
    }

    pub(super) fn can_promote(&self, plan: &UltraPlan) -> bool {
        self.eligible && !self.promoted && ProfileId::parse(&plan.profile) == ProfileId::Generic
    }
}

impl UltraRunContext {
    pub(super) fn for_run(root: &Path, expected_paths: &[String]) -> Self {
        Self::new(missing_final_artifacts(root, expected_paths))
    }

    pub(super) fn new(pending_final_artifacts: Vec<String>) -> Self {
        Self {
            pending_final_artifacts,
            ..Self::default()
        }
    }

    pub(super) fn emit_attached(
        &self,
        config: &Config,
        plan: &UltraPlan,
        phase: &UltraPhase,
        index: usize,
        session: &SessionSnapshot,
    ) {
        emit_ultra_phase_context_attached(config, plan, phase, index, self, session.messages.len());
    }

    pub(super) fn update_after_phase(
        &mut self,
        phase: &UltraPhase,
        outcome: &StepPlanRunOutcome,
        pending_final_artifacts: Vec<String>,
    ) {
        self.last_failed_phase = None;
        self.pending_final_artifacts = pending_final_artifacts;
        push_context_unique_capped(
            &mut self.completed_phases,
            format!(
                "{} ({}/{})",
                phase.id, outcome.completed_steps, outcome.total_steps
            ),
            ULTRA_CONTEXT_MAX_PHASES,
            &mut self.truncated,
        );
        push_context_items_capped(
            &mut self.created_or_changed_paths,
            &outcome.changed_paths,
            ULTRA_CONTEXT_MAX_PATHS,
            &mut self.truncated,
        );
        self.merge_observed_contract_debt(outcome);
        self.last_verify_failures.clear();
        push_context_items_capped(
            &mut self.last_repair_changed_paths,
            &outcome.repair_changed_paths,
            ULTRA_CONTEXT_MAX_PATHS,
            &mut self.truncated,
        );
        self.unresolved_repair_targets.clear();
        push_context_items_capped(
            &mut self.unresolved_repair_targets,
            &outcome.repair_targets,
            ULTRA_CONTEXT_MAX_MESSAGES,
            &mut self.truncated,
        );
        for rollback in &outcome.compile_rollbacks {
            push_context_items_capped(
                &mut self.carry_forward_guidance,
                &rollback.carry_forward_guidance,
                ULTRA_CONTEXT_MAX_MESSAGES,
                &mut self.truncated,
            );
        }
    }

    pub(super) fn merge_observed_contract_debt(&mut self, outcome: &StepPlanRunOutcome) {
        push_context_items_capped(
            &mut self.pending_capability_evidence,
            &outcome.observed_contract_keys(),
            ULTRA_CONTEXT_MAX_MESSAGES,
            &mut self.truncated,
        );
    }

    pub(super) fn refresh_pending_capability_evidence(&mut self, report: &RuntimeAcceptanceReport) {
        let still_missing = runtime_missing_signals(report);
        self.pending_capability_evidence
            .retain(|item| still_missing.contains(item));
        push_context_items_capped(
            &mut self.pending_capability_evidence,
            &report.diagnostics,
            ULTRA_CONTEXT_MAX_MESSAGES,
            &mut self.truncated,
        );
    }

    pub(super) fn refresh_intent_acceptance(
        &mut self,
        plan: &UltraPlan,
        config: &Config,
    ) -> anyhow::Result<()> {
        if !crate::planner::fix_runtime::applies(plan) {
            let acceptance = ultra_contract_runtime_acceptance_report(plan, config)?;
            self.refresh_pending_capability_evidence(&acceptance);
        }
        Ok(())
    }

    pub(super) fn render_unmet_final_requirements_section(&self) -> String {
        if self.pending_capability_evidence.is_empty() {
            return "Unmet final requirements from earlier phases:\n- none".to_string();
        }
        let pending = pending_capability_context_items(&self.pending_capability_evidence);
        render_bounded_prompt_section(
            "Unmet final requirements from earlier phases:",
            &pending,
            Some("Close these requirements when they are in scope for this phase."),
            ULTRA_PROMPT_GUIDANCE_MAX_LINES,
        )
    }

    pub(super) fn update_after_failure(
        &mut self,
        phase: &UltraPhase,
        outcome: &StepPlanRunOutcome,
        pending_final_artifacts: Vec<String>,
    ) {
        self.last_failed_phase = Some(phase.id.clone());
        self.pending_final_artifacts = pending_final_artifacts;
        push_context_items_capped(
            &mut self.created_or_changed_paths,
            &outcome.changed_paths,
            ULTRA_CONTEXT_MAX_PATHS,
            &mut self.truncated,
        );
        push_context_items_capped(
            &mut self.last_verify_failures,
            &outcome.verify_failures,
            ULTRA_CONTEXT_MAX_MESSAGES,
            &mut self.truncated,
        );
        if let Some(primary) = &outcome.primary_failure {
            push_context_unique_capped(
                &mut self.last_verify_failures,
                primary.clone(),
                ULTRA_CONTEXT_MAX_MESSAGES,
                &mut self.truncated,
            );
        }
        push_context_items_capped(
            &mut self.last_repair_changed_paths,
            &outcome.repair_changed_paths,
            ULTRA_CONTEXT_MAX_PATHS,
            &mut self.truncated,
        );
        push_context_items_capped(
            &mut self.unresolved_repair_targets,
            &outcome.repair_targets,
            ULTRA_CONTEXT_MAX_MESSAGES,
            &mut self.truncated,
        );
        for rollback in &outcome.compile_rollbacks {
            push_context_items_capped(
                &mut self.carry_forward_guidance,
                &rollback.carry_forward_guidance,
                ULTRA_CONTEXT_MAX_MESSAGES,
                &mut self.truncated,
            );
        }
    }

    pub(super) fn update_after_profile_failure(
        &mut self,
        phase: &UltraPhase,
        reason: &str,
        pending_final_artifacts: Vec<String>,
    ) {
        self.last_failed_phase = Some(phase.id.clone());
        self.pending_final_artifacts = pending_final_artifacts;
        push_context_unique_capped(
            &mut self.last_verify_failures,
            eval_events::body_snippet(reason),
            ULTRA_CONTEXT_MAX_MESSAGES,
            &mut self.truncated,
        );
        push_context_unique_capped(
            &mut self.unresolved_repair_targets,
            "profile_contract".to_string(),
            ULTRA_CONTEXT_MAX_MESSAGES,
            &mut self.truncated,
        );
    }

    pub(super) fn render_prompt_section(&self) -> String {
        if self.completed_phases.is_empty()
            && self.created_or_changed_paths.is_empty()
            && self.last_failed_phase.is_none()
            && self.pending_final_artifacts.is_empty()
            && self.pending_capability_evidence.is_empty()
            && self.carry_forward_guidance.is_empty()
        {
            return "Prior ultra context:\n- none yet".to_string();
        }
        let mut lines = vec!["Prior ultra context:".to_string()];
        append_context_list(&mut lines, "Completed phases", &self.completed_phases);
        append_context_list(
            &mut lines,
            "Created or changed paths",
            &self.created_or_changed_paths,
        );
        if let Some(phase) = &self.last_failed_phase {
            lines.push(format!("- Last failed phase: {phase}"));
        }
        append_context_list(
            &mut lines,
            "Recent verify failures",
            &self.last_verify_failures,
        );
        append_context_list(
            &mut lines,
            "Recent repair changed paths",
            &self.last_repair_changed_paths,
        );
        append_context_list(
            &mut lines,
            "Pending final artifacts",
            &self.pending_final_artifacts,
        );
        append_context_list(
            &mut lines,
            "Pending capability/evidence",
            &pending_capability_context_items(&self.pending_capability_evidence),
        );
        append_context_list(
            &mut lines,
            "Unresolved repair targets",
            &self.unresolved_repair_targets,
        );
        append_context_list(
            &mut lines,
            "Carry-forward guidance",
            &self.carry_forward_guidance,
        );
        if self.truncated {
            lines.push("- Context was truncated to bounded path/failure summaries".to_string());
        }
        lines.join("\n")
    }
}

pub(super) fn try_promote_profile_at_phase_boundary(
    config: &mut Config,
    plan: &mut UltraPlan,
    context: &mut UltraRunContext,
    final_expected_paths: &mut Vec<String>,
    promotion_state: &mut ProfilePromotionState,
    phase: &UltraPhase,
    index: usize,
) -> anyhow::Result<Option<ProfilePromotion>> {
    if !promotion_state.can_promote(plan) {
        return Ok(None);
    }
    // E5B_PROFILE_DISPATCH_ALLOW: inference-boundary
    let Some(inference) = infer_profile(None, &config.workspace_root) else {
        return Ok(None);
    };
    if inference.source != ProfileInferenceSource::Workspace {
        return Ok(None);
    }
    let promoted_id = ProfileId::parse(inference.profile);
    if promoted_id == ProfileId::Generic || promoted_id == ProfileId::parse(&plan.profile) {
        return Ok(None);
    }
    let promoted = promoted_id.to_string();
    let promoted_runtime = ProfileRuntimeRegistry::resolve(&promoted_id);
    let generic_id = ProfileId::Generic;
    let generic_runtime = ProfileRuntimeRegistry::resolve(&generic_id);

    let generic_paths = generic_runtime.expected_scaffold_paths(&config.workspace_root, &plan.goal);
    let mut generic_requirements =
        runtime_contract_requirements(generic_runtime, &generic_id, &plan.goal);
    let pre_promotion_contract = bind_completion_contract_for_acceptance(
        config,
        "ultra-plan-run-pre-promotion",
        "generic",
        &plan.goal,
        &generic_paths,
        &generic_requirements.capabilities,
        &generic_requirements.evidence,
        &generic_requirements.obligations,
    )?;
    if let Some(contract) = pre_promotion_contract.as_ref().map(|bound| &bound.contract) {
        merge_unique_strings(
            &mut generic_requirements.capabilities,
            &contract.required_capabilities,
        );
        merge_unique_strings(
            &mut generic_requirements.evidence,
            &contract.required_evidence,
        );
        merge_unique_strings(
            &mut generic_requirements.obligations,
            &contract.required_obligations,
        );
    }

    let mut promoted_requirements =
        runtime_contract_requirements(promoted_runtime, &promoted_id, &plan.goal);
    carry_pre_promotion_contract_requirements_with_runtime(
        promoted_runtime,
        &promoted_id,
        &plan.goal,
        &generic_requirements,
        &mut promoted_requirements,
    );
    merge_unique_strings(
        &mut promoted_requirements.evidence,
        &promoted_runtime.required_evidence(&plan.goal, &promoted_requirements.capabilities),
    );
    let promoted_paths =
        promoted_runtime.expected_scaffold_paths(&config.workspace_root, &plan.goal);
    let bound_contract = bind_completion_contract_for_acceptance(
        config,
        "ultra-plan-run",
        &promoted,
        &plan.goal,
        &promoted_paths,
        &promoted_requirements.capabilities,
        &promoted_requirements.evidence,
        &promoted_requirements.obligations,
    )?;
    if let Some(contract) = bound_contract.as_ref().map(|bound| &bound.contract) {
        merge_unique_strings(
            &mut promoted_requirements.capabilities,
            &contract.required_capabilities,
        );
        merge_unique_strings(
            &mut promoted_requirements.evidence,
            &contract.required_evidence,
        );
        merge_unique_strings(
            &mut promoted_requirements.obligations,
            &contract.required_obligations,
        );
    }
    merge_unique_strings(
        &mut promoted_requirements.evidence,
        &promoted_runtime.required_evidence(&plan.goal, &promoted_requirements.capabilities),
    );

    let mapped_generic_capabilities = mapped_pre_promotion_capabilities_for_profile(
        promoted_runtime,
        &promoted_id,
        &generic_requirements.capabilities,
    );
    debug_assert!(
        mapped_generic_capabilities
            .iter()
            .all(|capability| promoted_requirements.capabilities.contains(capability)),
        "profile promotion dropped a pre-promotion capability"
    );
    debug_assert!(
        generic_requirements
            .evidence
            .iter()
            .all(|evidence| promoted_requirements.evidence.contains(evidence)),
        "profile promotion dropped pre-promotion evidence"
    );
    debug_assert!(
        generic_requirements
            .obligations
            .iter()
            .all(|obligation| promoted_requirements.obligations.contains(obligation)),
        "profile promotion dropped pre-promotion obligations"
    );

    let delta_capabilities = ordered_string_difference(
        &promoted_requirements.capabilities,
        &generic_requirements.capabilities,
    );
    let mut generic_requirement_keys = generic_requirements.capabilities.clone();
    merge_unique_strings(
        &mut generic_requirement_keys,
        &generic_requirements.evidence,
    );
    merge_unique_strings(
        &mut generic_requirement_keys,
        &generic_requirements.obligations,
    );
    let mut promoted_requirement_keys = promoted_requirements.capabilities.clone();
    merge_unique_strings(
        &mut promoted_requirement_keys,
        &promoted_requirements.evidence,
    );
    merge_unique_strings(
        &mut promoted_requirement_keys,
        &promoted_requirements.obligations,
    );
    let delta_requirements =
        ordered_string_difference(&promoted_requirement_keys, &generic_requirement_keys);
    push_context_items_capped(
        &mut context.pending_capability_evidence,
        &delta_requirements,
        ULTRA_CONTEXT_MAX_MESSAGES,
        &mut context.truncated,
    );

    plan.profile = promoted.clone();
    config.profile = promoted.clone();
    config.profile_inference = Some(inference);
    *final_expected_paths =
        promoted_runtime.expected_scaffold_paths(&config.workspace_root, &plan.goal);
    context.pending_final_artifacts =
        missing_final_artifacts(&config.workspace_root, final_expected_paths);
    promotion_state.promoted = true;

    let promotion = ProfilePromotion {
        id: promoted,
        at_phase: index + 1,
        phase_id: phase.id.clone(),
        requested_port: effective_requested_port(
            resolve_profile_runtime(&plan.profile),
            &plan.goal,
            Some(&ultra_plan_phase_signal_text(plan)),
        )
        .map(|requested| requested.telemetry),
        contract_origin: "promoted_union".to_string(),
        delta_capabilities,
        delta_requirements,
    };
    emit_profile_reinferred(config, &promotion);
    Ok(Some(promotion))
}

pub(super) fn ordered_string_difference(values: &[String], baseline: &[String]) -> Vec<String> {
    let baseline = baseline.iter().map(String::as_str).collect::<BTreeSet<_>>();
    values
        .iter()
        .filter(|value| !baseline.contains(value.as_str()))
        .cloned()
        .collect()
}

pub(super) fn runtime_contract_requirements(
    runtime: &dyn ProfileRuntime,
    profile_id: &ProfileId,
    goal: &str,
) -> ContractRequirements {
    let capabilities = runtime.required_capabilities(goal);
    ContractRequirements {
        evidence: runtime.required_evidence(goal, &capabilities),
        obligations: runtime.required_obligations(profile_id, goal, &capabilities),
        capabilities,
    }
}

pub(super) fn carry_pre_promotion_contract_requirements_with_runtime(
    promoted_runtime: &dyn ProfileRuntime,
    promoted_id: &ProfileId,
    goal: &str,
    pre_promotion: &ContractRequirements,
    promoted: &mut ContractRequirements,
) {
    merge_unique_strings(
        &mut promoted.capabilities,
        &mapped_pre_promotion_capabilities_for_profile(
            promoted_runtime,
            promoted_id,
            &pre_promotion.capabilities,
        ),
    );
    if pre_promotion
        .capabilities
        .iter()
        .any(|capability| capability == GENERIC_INTERACTIVE_CONTRACT_CAPABILITY)
        && signals::contains_app_intent_token(goal)
    {
        merge_unique_strings(
            &mut promoted.capabilities,
            &promoted_runtime.interactive_app_capabilities(promoted_id),
        );
    }
    merge_unique_strings(&mut promoted.evidence, &pre_promotion.evidence);
    merge_unique_strings(&mut promoted.obligations, &pre_promotion.obligations);
}

pub(super) fn mapped_pre_promotion_capabilities_for_profile(
    promoted_runtime: &dyn ProfileRuntime,
    promoted_id: &ProfileId,
    capabilities: &[String],
) -> Vec<String> {
    let mut mapped = Vec::new();
    for capability in capabilities {
        if capability == GENERIC_INTERACTIVE_CONTRACT_CAPABILITY {
            let equivalents = promoted_runtime.interactive_app_capabilities(promoted_id);
            if equivalents.is_empty() {
                merge_unique_strings(&mut mapped, std::slice::from_ref(capability));
            } else {
                merge_unique_strings(&mut mapped, &equivalents);
            }
        } else {
            merge_unique_strings(&mut mapped, std::slice::from_ref(capability));
        }
    }
    mapped
}

pub(super) fn emit_profile_reinferred(config: &Config, promotion: &ProfilePromotion) {
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "profile_reinferred",
            "id": promotion.id,
            "profile": promotion.id,
            "at_phase": promotion.at_phase,
            "phase_id": promotion.phase_id,
            "from": "workspace",
            "from_profile": "generic",
            "to_profile": promotion.id,
            "requested_port": promotion.requested_port.clone(),
            "contract_origin": promotion.contract_origin,
            "delta_capabilities": promotion.delta_capabilities.clone(),
            "delta_requirements": promotion.delta_requirements.clone(),
        }),
    );
    let line = format!(
        "Profile promoted: generic -> {} (workspace evidence, phase {})",
        promotion.id, promotion.at_phase
    );
    eprintln!("{line}");
    eval_events::write_run_summary(config.eval_events_path.as_deref(), &line);
}

pub(super) fn carry_recorded_promotion_contract_requirements(
    config: &Config,
    effective_profile: &str,
    goal: &str,
    requirements: &mut ContractRequirements,
) {
    let effective_profile_id = ProfileId::parse(effective_profile);
    if contract_origin_for_acceptance(config) != "promoted_union"
        || effective_profile_id == ProfileId::Generic
    {
        return;
    }
    let generic_id = ProfileId::Generic;
    let generic_runtime = ProfileRuntimeRegistry::resolve(&generic_id);
    let generic_requirements = runtime_contract_requirements(generic_runtime, &generic_id, goal);
    let effective_runtime = ProfileRuntimeRegistry::resolve(&effective_profile_id);
    carry_pre_promotion_contract_requirements_with_runtime(
        effective_runtime,
        &effective_profile_id,
        goal,
        &generic_requirements,
        requirements,
    );
    merge_unique_strings(
        &mut requirements.evidence,
        &effective_runtime.required_evidence(goal, &requirements.capabilities),
    );
}

impl StepPlanRunOutcome {
    pub(super) fn for_plan(plan: &StepPlan) -> Self {
        Self {
            total_steps: plan.steps.len(),
            summary: format!("plan-run complete: {} steps", plan.steps.len()),
            ..Self::default()
        }
    }

    pub(super) fn merge_step(&mut self, step: &StepRunOutcome) {
        merge_unique_strings(&mut self.changed_paths, &step.changed_paths);
        merge_unique_strings(
            &mut self.observed_missing_capabilities,
            &step.observed_missing_capabilities,
        );
        merge_unique_strings(
            &mut self.observed_missing_evidence,
            &step.observed_missing_evidence,
        );
        merge_unique_strings(
            &mut self.observed_missing_obligations,
            &step.observed_missing_obligations,
        );
        merge_unique_strings(&mut self.verify_failures, &step.verify_failures);
        merge_unique_strings(&mut self.repair_targets, &step.repair_targets);
        merge_unique_strings(&mut self.command_failures, &step.command_failures);
        merge_unique_strings(&mut self.repair_changed_paths, &step.repair_changed_paths);
        self.compile_rollbacks
            .extend(step.compile_rollbacks.iter().cloned());
        self.repair_attempts += step.repair_attempts;
        if let Some(primary) = &step.primary_failure {
            self.primary_failure = Some(primary.clone());
        }
        if let Some(stop) = &step.stop_reason {
            self.stop_reason = Some(stop.clone());
        }
        self.partial |= step.partial;
    }

    pub(super) fn mark_failure(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.primary_failure = Some(message.clone());
        self.stop_reason = Some(message);
        self.partial = true;
    }

    pub(super) fn observed_contract_keys(&self) -> Vec<String> {
        let mut out = Vec::new();
        merge_unique_strings(&mut out, &self.observed_missing_capabilities);
        merge_unique_strings(&mut out, &self.observed_missing_evidence);
        merge_unique_strings(&mut out, &self.observed_missing_obligations);
        out
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct StepRunOutcome {
    pub(super) changed_paths: Vec<String>,
    pub(crate) observed_missing_capabilities: Vec<String>,
    pub(crate) observed_missing_evidence: Vec<String>,
    pub(crate) observed_missing_obligations: Vec<String>,
    pub(super) verify_failures: Vec<String>,
    pub(super) primary_failure: Option<String>,
    pub(crate) repair_targets: Vec<String>,
    pub(super) command_failures: Vec<String>,
    pub(super) repair_attempts: usize,
    pub(super) repair_changed_paths: Vec<String>,
    pub(super) compile_rollbacks: Vec<CompileRollbackOutcome>,
    pub(super) stop_reason: Option<String>,
    pub(super) partial: bool,
}

#[derive(Debug, Clone)]
pub(super) struct StepRunError {
    pub(super) message: String,
    pub(super) outcome: StepRunOutcome,
}

#[derive(Debug, Clone)]
pub(super) struct StepPlanRunError {
    pub(super) message: String,
    pub(super) partial_outcome: StepPlanRunOutcome,
}

impl StepPlanRunError {
    pub(super) fn from_error(
        message: impl Into<String>,
        mut partial_outcome: StepPlanRunOutcome,
    ) -> Self {
        let message = message.into();
        partial_outcome.mark_failure(message.clone());
        Self {
            message,
            partial_outcome,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct UltraRunSetupAuthorityState {
    pub(super) reasons: Vec<String>,
}

impl UltraRunSetupAuthorityState {
    pub(super) fn authority(&self) -> NodeDependencySetupAuthority {
        if self.reasons.is_empty() {
            NodeDependencySetupAuthority::None
        } else {
            NodeDependencySetupAuthority::PlanSetupStep
        }
    }

    pub(super) fn grant(&mut self, reason: &str) {
        if !self.reasons.iter().any(|existing| existing == reason) {
            self.reasons.push(reason.to_string());
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DependencyReconciliationTrigger {
    Promotion,
    ManifestRepair,
    ManifestChanged,
    DeclaredDependenciesNotReady,
}

impl DependencyReconciliationTrigger {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Promotion => "promotion",
            Self::ManifestRepair => "manifest_repair",
            Self::ManifestChanged => "manifest_changed",
            Self::DeclaredDependenciesNotReady => "declared_dependencies_not_ready",
        }
    }
}

pub(super) fn reconcile_run_dependency_setup(
    config: &Config,
    profile: &str,
    trigger: DependencyReconciliationTrigger,
    setup_authority: &UltraRunSetupAuthorityState,
) -> anyhow::Result<Option<Vec<String>>> {
    let authority = setup_authority.authority();
    let Some(requirement) =
        dependency_reconciliation_requirement(&config.workspace_root, profile, trigger, authority)
    else {
        return Ok(None);
    };
    let setup = dependency_setup::run_node_dependency_setup_with_program_and_offline(
        &config.workspace_root,
        &requirement,
        Path::new("npm"),
        config.offline,
    );
    let lifecycle = dependency_reconciliation_lifecycle(&requirement, setup.clone());
    emit_dependency_build_lifecycle(
        config.eval_events_path.as_deref(),
        "ultra-plan-run",
        Some(trigger.as_str()),
        &lifecycle,
    );
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "dependency_setup_reconciliation",
            "trigger": trigger.as_str(),
            // E5B_PROFILE_DISPATCH_ALLOW: telemetry-profile
            "profile": ProfileId::parse(profile).to_string(),
            "authority": authority.as_str(),
            "setup_kind": setup.setup_kind.as_str(),
            "status": setup.status.as_str(),
            "attempted": setup.attempted,
            "command": setup.command.clone(),
            "added": setup.changed_paths.clone(),
            "primary_reason": eval_events::body_snippet(&setup.primary_reason),
            "duration_ms": setup.duration_ms,
            "timeout_ms": setup.timeout_ms,
            "classification": if setup.status == NodeDependencySetupStatus::TimedOut {
                "dependency_setup_timeout"
            } else {
                ""
            },
            "offline": config.offline,
        }),
    );
    match setup.status {
        NodeDependencySetupStatus::Passed | NodeDependencySetupStatus::NotRequired => {
            Ok(Some(setup.changed_paths))
        }
        NodeDependencySetupStatus::Blocked
            if setup.primary_reason == "dependency_setup_blocked_offline" =>
        {
            anyhow::bail!("dependency_setup_blocked_offline")
        }
        NodeDependencySetupStatus::Blocked => {
            anyhow::bail!("dependency_setup_blocked: {}", setup.primary_reason)
        }
        NodeDependencySetupStatus::Failed | NodeDependencySetupStatus::TimedOut => {
            anyhow::bail!(
                "dependency_setup_lifecycle_failed: {}",
                setup.primary_reason
            )
        }
        NodeDependencySetupStatus::Attempted => {
            anyhow::bail!("dependency_setup_lifecycle_failed: dependency setup did not finish")
        }
    }
}

pub(super) fn dependency_reconciliation_requirement(
    root: &Path,
    profile: &str,
    trigger: DependencyReconciliationTrigger,
    authority: NodeDependencySetupAuthority,
) -> Option<NodeDependencySetupRequirement> {
    let reason = format!("{} dependency reconciliation", trigger.as_str());
    let profile_id = ProfileId::parse(profile);
    resolve_profile_runtime(profile).dependency_reconciliation_requirement(
        root,
        &profile_id,
        trigger == DependencyReconciliationTrigger::ManifestChanged,
        &reason,
        authority,
    )
}

pub(super) fn reconcile_manifest_changed_dependencies_if_needed(
    config: &Config,
    profile: &str,
    setup_authority: &mut UltraRunSetupAuthorityState,
) -> anyhow::Result<Option<Vec<String>>> {
    if !dependency_setup::node_dependency_declarations_fingerprint_mismatch(&config.workspace_root)
    {
        return Ok(None);
    }
    setup_authority.grant("manifest_changed");
    reconcile_run_dependency_setup(
        config,
        profile,
        DependencyReconciliationTrigger::ManifestChanged,
        setup_authority,
    )
}

pub(super) fn verification_report_mentions_dependency_setup_missing(
    report: &VerificationReport,
) -> bool {
    report
        .dependency_missing
        .iter()
        .chain(report.profile_failures.iter())
        .any(|reason| text_mentions_dependency_setup_missing(reason))
        || report
            .command_failures
            .iter()
            .any(|failure| text_mentions_dependency_setup_missing(&failure.reason))
}

pub(super) fn text_mentions_dependency_setup_missing(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("dependency setup missing")
        || lower.contains("build dependency setup missing")
        || lower.contains("node_modules missing")
}

pub(super) fn push_unique_label(labels: &mut Vec<String>, label: &str) {
    if !labels.iter().any(|existing| existing == label) {
        labels.push(label.to_string());
    }
}

pub(super) fn dependency_reconciliation_lifecycle(
    requirement: &NodeDependencySetupRequirement,
    setup: dependency_setup::NodeDependencySetupObservation,
) -> BuildVerifierLifecycleObservation {
    let before = BuildVerifierObservation {
        command: "dependency reconciliation".to_string(),
        profile: requirement.profile.clone(),
        authority: requirement.setup_authority.as_str().to_string(),
        required_for_completion: true,
        requires_dependency_setup: true,
        dependency_ready: false,
        attempted: false,
        status: BuildVerifierStatus::DependencyMissing,
        primary_reason: requirement.reason.clone(),
        output_snippet: String::new(),
        output_path: String::new(),
        compile_errors: Vec::new(),
        foreign_toolchain: None,
    };
    let after_status = match setup.status {
        NodeDependencySetupStatus::Passed | NodeDependencySetupStatus::NotRequired => {
            BuildVerifierStatus::Passed
        }
        NodeDependencySetupStatus::Blocked | NodeDependencySetupStatus::TimedOut => {
            BuildVerifierStatus::Blocked
        }
        NodeDependencySetupStatus::Failed | NodeDependencySetupStatus::Attempted => {
            BuildVerifierStatus::Failed
        }
    };
    let after = (after_status == BuildVerifierStatus::Passed).then(|| BuildVerifierObservation {
        command: "dependency reconciliation".to_string(),
        profile: requirement.profile.clone(),
        authority: requirement.setup_authority.as_str().to_string(),
        required_for_completion: true,
        requires_dependency_setup: true,
        dependency_ready: true,
        attempted: false,
        status: BuildVerifierStatus::Passed,
        primary_reason: "dependency setup reconciliation passed".to_string(),
        output_snippet: String::new(),
        output_path: String::new(),
        compile_errors: Vec::new(),
        foreign_toolchain: None,
    });
    let final_reason = after
        .as_ref()
        .map(|observation| observation.primary_reason.clone())
        .unwrap_or_else(|| setup.primary_reason.clone());
    BuildVerifierLifecycleObservation {
        requirement: BuildVerifierRequirement {
            command: "dependency reconciliation".to_string(),
            profile: requirement.profile.clone(),
            reason: requirement.reason.clone(),
            authority: requirement.setup_authority.as_str().to_string(),
            status: "required".to_string(),
            requires_dependency_setup: true,
            required_for_completion: true,
        },
        before_setup: before,
        setup: Some(setup),
        after_setup: after,
        final_status: after_status,
        final_reason,
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct StepPromptContext {
    pub(super) overall_goal: String,
    pub(super) required_final_artifacts: Vec<String>,
    pub(super) prior_expected_paths: Vec<String>,
    pub(super) final_required_capabilities: Vec<String>,
    pub(super) final_required_evidence: Vec<String>,
    pub(super) completion_contract_path: Option<PathBuf>,
}

#[allow(clippy::result_large_err, clippy::too_many_arguments)]
pub(super) fn run_step(
    client: &mut dyn ChatClient,
    session: &mut SessionSnapshot,
    plan: &StepPlan,
    step: &PlanStep,
    prompt_context: &StepPromptContext,
    config: &Config,
    ui: &dyn InteractionUi,
    mode: &'static str,
    contract_enforcement: ContractEnforcement,
    phase_scope: Option<&str>,
    mut run_setup_authority: Option<&mut UltraRunSetupAuthorityState>,
) -> Result<StepRunOutcome, StepRunError> {
    let runtime = resolve_profile_runtime(&config.profile);
    let host_owned_recovery_verify =
        crate::planner::recovery_step_plan_binding::is_host_owned_final_success_step(
            config,
            phase_scope,
            step,
        );
    let (mut runtime_step, synthesized_precheck) = if host_owned_recovery_verify {
        (step.clone(), false)
    } else {
        runtime.runtime_step_with_profile_checks(
            &config.workspace_root,
            &prompt_context.overall_goal,
            step,
            phase_scope,
            config.eval_events_path.as_deref(),
        )
    };
    runtime
        .inject_step_material(config, &mut runtime_step)
        .map_err(|err| StepRunError {
            message: format!("step source material injection failed: {err}"),
            outcome: StepRunOutcome::default(),
        })?;
    let instruction = build_step_prompt(plan, &runtime_step, prompt_context, config.prompt_layout);
    emit_step_prompt_contract(config, &runtime_step, prompt_context, &instruction);
    if step.step_kind() == StepKind::Report
        && step.expected_paths.is_empty()
        && step.verify.is_empty()
    {
        return Ok(StepRunOutcome::default());
    }
    let mut step_config = capped_config(config, STEP_TURN_MAX_ITERATIONS);
    if step.step_kind() == StepKind::Implement
        && let Some(path) = prompt_context.completion_contract_path.clone()
    {
        step_config.completion_contract_path = Some(path);
    }
    let run_authority = run_setup_authority
        .as_deref()
        .map(UltraRunSetupAuthorityState::authority)
        .unwrap_or(NodeDependencySetupAuthority::None);
    let setup_authority = step_verify_setup_authority(plan, step, run_authority);
    let contract_setup_authority =
        step_contract_setup_authority(plan, step, phase_scope, run_authority);
    let recovery_fix_write_required =
        recovery_fix_implement_requires_write(config, step).map_err(|err| StepRunError {
            message: format!("Recovery fix origin validation failed: {err}"),
            outcome: StepRunOutcome::default(),
        })?;
    let step_options = step_run_session_options(
        plan,
        step,
        contract_enforcement,
        phase_scope,
        contract_setup_authority,
    )
    .with_repair_target_priority(repair_targeting::RepairTargetPriority::for_intent(
        config.resolved_intent(&prompt_context.overall_goal),
    ))
    .with_required_write_for_action_prompt(recovery_fix_write_required)
    .with_required_mutation_before_short_circuit(
        synthesized_precheck || recovery_fix_write_required,
    );
    let data_pre_satisfied =
        runtime.pre_satisfied_verify_first(&config.workspace_root, &runtime_step);
    let verify_first_applicable = host_owned_recovery_verify
        || data_pre_satisfied
            .unwrap_or_else(|| runtime.step_short_circuit_precheck_applicable(&runtime_step));
    if verify_first_applicable {
        let (report, build_lifecycles) = verify_step_completion_observed(
            config,
            &prompt_context.overall_goal,
            &runtime_step,
            step,
            phase_scope,
            setup_authority,
        );
        apply_runtime_command_normalizations(&mut runtime_step, &report);
        for lifecycle in &build_lifecycles {
            emit_dependency_build_lifecycle(
                config.eval_events_path.as_deref(),
                mode,
                Some(&step.id),
                lifecycle,
            );
        }
        if report.is_pass() {
            if host_owned_recovery_verify {
                eval_events::emit(
                    config.eval_events_path.as_deref(),
                    json!({
                        "event": "recovery_host_final_success_verification_passed",
                        "phase_id": phase_scope.unwrap_or(""),
                        "step_id": runtime_step.id,
                        "registered_verify_commands": runtime_step.verify,
                        "model_execution_skipped": true,
                    }),
                );
            }
            if data_pre_satisfied.is_some() {
                crate::planner::profiles::data::pre_satisfied::emit_short_circuited(
                    config.eval_events_path.as_deref(),
                    &runtime_step,
                    phase_scope,
                    &report,
                );
            } else {
                emit_runner_step_short_circuited(
                    config,
                    &runtime_step,
                    phase_scope,
                    &runtime_step.expected_paths,
                    "start",
                );
            }
            if production_build_lifecycle_passed(&build_lifecycles) {
                snapshot_last_known_good_sources(
                    config,
                    mode,
                    Some(&step.id),
                    &config.profile,
                    prompt_context.overall_goal.as_str(),
                    &runtime_step.expected_paths,
                );
            }
            return Ok(StepRunOutcome {
                stop_reason: Some("StepShortCircuited".to_string()),
                ..StepRunOutcome::default()
            });
        }
        if host_owned_recovery_verify {
            let message = format!(
                "host-owned Recovery final-success verification failed: {}",
                report.primary_reason()
            );
            eval_events::emit(
                config.eval_events_path.as_deref(),
                json!({
                    "event": "recovery_host_final_success_verification_failed",
                    "phase_id": phase_scope.unwrap_or(""),
                    "step_id": runtime_step.id,
                    "reason": eval_events::body_snippet(&report.primary_reason()),
                    "registered_verify_commands": runtime_step.verify,
                    "model_execution_skipped": true,
                }),
            );
            return Err(StepRunError {
                message: message.clone(),
                outcome: StepRunOutcome {
                    primary_failure: Some(message),
                    verify_failures: vec![report.primary_reason()],
                    command_failures: command_failure_summaries(&report),
                    stop_reason: Some(
                        "recovery_host_final_success_verification_failed".to_string(),
                    ),
                    partial: true,
                    ..StepRunOutcome::default()
                },
            });
        }
    }
    let initial = run_session_with_outcome_with_options(
        client,
        session,
        &instruction,
        &runtime_step.expected_paths,
        &step_config,
        ui,
        step_options.clone(),
    )
    .map_err(|err| {
        let message = err.to_string();
        StepRunError {
            outcome: step_run_outcome_from_session_error(&err, "initial_turn_error"),
            message,
        }
    })?;
    let mut outcome = StepRunOutcome {
        changed_paths: initial.changed_paths.clone(),
        observed_missing_capabilities: initial.missing_capabilities.clone(),
        observed_missing_evidence: initial.missing_evidence.clone(),
        observed_missing_obligations: initial.missing_obligations.clone(),
        stop_reason: Some(format!("{:?}", initial.stop_reason)),
        ..StepRunOutcome::default()
    };
    let overall_goal = prompt_context.overall_goal.as_str();
    match runtime.post_step_repair(&config.workspace_root, overall_goal) {
        Ok(true) => {
            if let Some(state) = run_setup_authority.as_deref_mut() {
                state.grant("manifest_repair");
                if let Err(err) = reconcile_run_dependency_setup(
                    config,
                    &config.profile,
                    DependencyReconciliationTrigger::ManifestRepair,
                    state,
                ) {
                    let message = err.to_string();
                    outcome.primary_failure = Some(message.clone());
                    outcome.stop_reason =
                        Some("dependency_setup_reconciliation_failed".to_string());
                    outcome.partial = true;
                    return Err(StepRunError { message, outcome });
                }
            }
        }
        Ok(false) => {}
        Err(err) => {
            outcome.primary_failure = Some(err.to_string());
            outcome.stop_reason = Some("profile_post_step_repair_error".to_string());
            outcome.partial = true;
            return Err(StepRunError {
                message: err.to_string(),
                outcome,
            });
        }
    }
    if let Some(state) = run_setup_authority.as_deref_mut()
        && let Err(err) =
            reconcile_manifest_changed_dependencies_if_needed(config, &config.profile, state)
    {
        let message = err.to_string();
        outcome.primary_failure = Some(message.clone());
        outcome.stop_reason = Some("dependency_setup_reconciliation_failed".to_string());
        outcome.partial = true;
        return Err(StepRunError { message, outcome });
    }
    let (report, build_lifecycles) = verify_step_completion_observed(
        config,
        &prompt_context.overall_goal,
        &runtime_step,
        step,
        phase_scope,
        setup_authority,
    );
    apply_runtime_command_normalizations(&mut runtime_step, &report);
    for lifecycle in &build_lifecycles {
        emit_dependency_build_lifecycle(
            config.eval_events_path.as_deref(),
            mode,
            Some(&step.id),
            lifecycle,
        );
    }
    if report.is_pass() {
        if production_build_lifecycle_passed(&build_lifecycles) {
            snapshot_last_known_good_sources(
                config,
                mode,
                Some(&step.id),
                &config.profile,
                overall_goal,
                &runtime_step.expected_paths,
            );
        }
        return Ok(outcome);
    }
    let first_target = classify_repair_target(&report).as_str().to_string();
    let repair_policy = crate::planner::profiles::data::repair_policy::StepRepairPolicy::new(
        &config.profile,
        &step.id,
        STEP_REPAIR_MAX_TURNS,
        config.eval_events_path.as_deref(),
    );
    let mut current_reachability =
        repair_policy.assess(&report, setup_authority, config.offline, 0);
    outcome.primary_failure = Some(report.primary_reason());
    outcome.verify_failures.push(report.primary_reason());
    outcome.repair_targets.push(first_target.clone());
    outcome
        .command_failures
        .extend(command_failure_summaries(&report));
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "step_verify_failure",
            "step_id": step.id,
            "repair_target": first_target,
            "primary_reason": eval_events::body_snippet(&report.primary_reason()),
            "missing_paths": report.missing_paths.clone(),
            "command_failures": report.command_failures.len(),
            "dependency_missing": report.dependency_missing.clone(),
            "profile_failures": report.profile_failures.clone(),
            "dependency_setup_authority": setup_authority.as_str(),
            "repair_reachable": current_reachability.reachable,
            "reachable": current_reachability.reachable,
            "viable_actions": reachability_action_labels(&current_reachability),
            "blocked_requirements": current_reachability.blocked_requirements.clone(),
        }),
    );
    let mut context = RepairContext {
        profile: Some(config.profile.clone()),
        overall_goal: Some(overall_goal.to_string()),
        required_final_artifacts: prompt_context.required_final_artifacts.clone(),
        step_instruction: Some(step.instruction.clone()),
        expected_paths: merge_repair_target_paths(&report, &runtime_step.expected_paths),
        verify_commands: runtime_step.verify.clone(),
        expected_result: Some(step_expected_result(step).to_string()),
        max_repair_turns: Some(STEP_REPAIR_MAX_TURNS),
        missing_paths: report.missing_paths.clone(),
        changed_files: initial.changed_paths.clone(),
        initial_stop_reason: Some(format!("{:?}", initial.stop_reason)),
        workspace_root: Some(config.workspace_root.clone()),
        eval_events_path: config.eval_events_path.clone(),
        prompt_layout: config.prompt_layout,
        ..RepairContext::default()
    };
    let mut current_report = report;
    let mut previous_missing = current_report.missing_paths.len();
    let mut repair_stop_reason = None;
    let mut terminal_repair_failure_kind: Option<String> = None;
    let mut terminal_blocked_requirements: Vec<String> = Vec::new();
    let mut no_change_repairs = 0usize;
    let mut target_not_followed_repairs = 0usize;
    let mut identical_no_change_repairs = 0usize;
    let mut hook_snapshot_feedback_given = false;
    let mut hook_snapshot_restore_used = false;
    let mut current_report_signature = verification_report_signature(&current_report);
    let repair_config = capped_config(config, STEP_REPAIR_MAX_ITERATIONS);
    let escalation_carryover = EscalationCarryoverHandle::from_pressure(CarriedPressure::default());
    if !current_reachability.reachable {
        terminal_repair_failure_kind =
            Some(reachability_failure_kind(&current_reachability).to_string());
        terminal_blocked_requirements = current_reachability.blocked_requirements.clone();
        context.progress_warning = Some(reachability_recovery_reason(&current_reachability));
        emit_repair_unreachable(
            config,
            mode,
            &step.id,
            classify_repair_target(&current_report).as_str(),
            &current_report.primary_reason(),
            &current_reachability,
        );
    }
    if current_reachability.reachable {
        for attempt in 1..=STEP_REPAIR_MAX_TURNS {
            let repair_session_mode =
                if no_change_repairs > 0 && !current_report.compile_errors.is_empty() {
                    RepairSessionMode::Compact
                } else {
                    RepairSessionMode::Appended
                };
            context.repair_attempt = Some(attempt);
            context.compile_reanchored_retry = repair_session_mode == RepairSessionMode::Compact;
            context.compile_narrow_no_snapshot_retry = false;
            let repair_options =
                attach_to_options(step_options.clone(), escalation_carryover.clone());
            let mut repair_prompt = if repair_session_mode == RepairSessionMode::Compact {
                build_compact_compile_repair_prompt_with_context(
                    &step.id,
                    &current_report,
                    &context,
                )
            } else {
                build_repair_prompt_with_context(&step.id, &current_report, &context)
            };
            repair_prompt = hook_snapshot::prefix_feedback_if_missing_with_runtime(
                config,
                runtime,
                overall_goal,
                "step_verify_repair",
                Some(&step.id),
                &mut hook_snapshot_feedback_given,
                repair_prompt,
            );
            let repair_result = match repair_session_mode {
                RepairSessionMode::Appended => run_session_with_outcome_with_options(
                    client,
                    session,
                    &repair_prompt,
                    &context.expected_paths,
                    &repair_config,
                    ui,
                    repair_options.clone(),
                ),
                RepairSessionMode::Compact => {
                    let mut compact_session = SessionSnapshot::new();
                    run_session_with_outcome_with_options(
                        client,
                        &mut compact_session,
                        &repair_prompt,
                        &context.expected_paths,
                        &repair_config,
                        ui,
                        repair_options.clone(),
                    )
                }
            };
            let repair = match repair_result {
                Ok(repair) => repair,
                Err(err)
                    if repair_session_mode == RepairSessionMode::Compact
                        && !current_report.compile_errors.is_empty()
                        && err
                            .to_string()
                            .contains("missing tool call for action prompt") =>
                {
                    RunSessionOutcome {
                        final_text: err.to_string(),
                        stop_reason: RunStopReason::AssistantFinal,
                        changed_paths: Vec::new(),
                        iterations: 0,
                        tool_calls: 0,
                        missing_required_paths: Vec::new(),
                        missing_capabilities: Vec::new(),
                        missing_evidence: Vec::new(),
                        missing_obligations: Vec::new(),
                        verify_attempts: 0,
                        last_blocking_reason: Some(err.to_string()),
                        last_provider_error: None,
                    }
                }
                Err(err) => {
                    let message = err.to_string();
                    apply_session_error_observations(&mut outcome, &err, &message);
                    outcome.primary_failure = Some(message.clone());
                    outcome.stop_reason = Some("repair_turn_error".to_string());
                    outcome.repair_attempts = attempt;
                    outcome.partial = true;
                    return Err(StepRunError {
                        message,
                        outcome: outcome.clone(),
                    });
                }
            };
            outcome.repair_attempts = attempt;
            repair_stop_reason = Some(format!("{:?}", repair.stop_reason));
            let changed_paths_before_repair = context.changed_files.clone();
            let mut repair_turn_changed_paths = repair.changed_paths.clone();
            merge_changed_files(&mut context, &repair.changed_paths);
            merge_unique_strings(&mut outcome.changed_paths, &repair.changed_paths);
            merge_unique_strings(
                &mut outcome.observed_missing_capabilities,
                &repair.missing_capabilities,
            );
            merge_unique_strings(
                &mut outcome.observed_missing_evidence,
                &repair.missing_evidence,
            );
            merge_unique_strings(
                &mut outcome.observed_missing_obligations,
                &repair.missing_obligations,
            );
            merge_unique_strings(&mut outcome.repair_changed_paths, &repair.changed_paths);
            match runtime.post_step_repair(&config.workspace_root, overall_goal) {
                Ok(true) => {
                    if let Some(state) = run_setup_authority.as_deref_mut() {
                        state.grant("manifest_repair");
                        if let Err(err) = reconcile_run_dependency_setup(
                            config,
                            &config.profile,
                            DependencyReconciliationTrigger::ManifestRepair,
                            state,
                        ) {
                            let message = err.to_string();
                            outcome.primary_failure = Some(message.clone());
                            outcome.stop_reason =
                                Some("dependency_setup_reconciliation_failed".to_string());
                            outcome.partial = true;
                            return Err(StepRunError { message, outcome });
                        }
                    }
                    let package_path = "package.json".to_string();
                    merge_changed_files(&mut context, std::slice::from_ref(&package_path));
                    merge_unique_strings(
                        &mut outcome.changed_paths,
                        std::slice::from_ref(&package_path),
                    );
                    merge_unique_strings(&mut outcome.repair_changed_paths, &[package_path]);
                    merge_unique_strings(
                        &mut repair_turn_changed_paths,
                        &["package.json".to_string()],
                    );
                }
                Ok(false) => {}
                Err(err) => {
                    outcome.primary_failure = Some(err.to_string());
                    outcome.stop_reason = Some("profile_post_step_repair_error".to_string());
                    outcome.partial = true;
                    return Err(StepRunError {
                        message: err.to_string(),
                        outcome,
                    });
                }
            }
            if let Some(state) = run_setup_authority.as_deref_mut()
                && let Err(err) = reconcile_manifest_changed_dependencies_if_needed(
                    config,
                    &config.profile,
                    state,
                )
            {
                let message = err.to_string();
                outcome.primary_failure = Some(message.clone());
                outcome.stop_reason = Some("dependency_setup_reconciliation_failed".to_string());
                outcome.partial = true;
                return Err(StepRunError { message, outcome });
            }
            let (retry, retry_lifecycles) = verify_step_with_context(
                &config.workspace_root,
                &runtime_step,
                Some(&config.profile),
                Some(overall_goal),
                setup_authority,
                config.offline,
                config.eval_events_path.as_deref(),
            );
            apply_runtime_command_normalizations(&mut runtime_step, &retry);
            context.verify_commands = runtime_step.verify.clone();
            for lifecycle in &retry_lifecycles {
                emit_dependency_build_lifecycle(
                    config.eval_events_path.as_deref(),
                    mode,
                    Some(&step.id),
                    lifecycle,
                );
            }
            let retry_target = classify_repair_target(&retry);
            let retry_reachability =
                repair_policy.assess(&retry, setup_authority, config.offline, attempt);
            let previous_target = classify_repair_target(&current_report);
            let repair_follow_through =
                classify_repair_follow_through(previous_target, &repair_turn_changed_paths);
            let repair_target_followed = repair_follow_through.followed();
            match repair_follow_through {
                RepairFollowThrough::NoChange => {
                    no_change_repairs += 1;
                }
                RepairFollowThrough::TargetNotFollowed | RepairFollowThrough::UnrelatedChange => {
                    target_not_followed_repairs += 1;
                    no_change_repairs = 0;
                }
                RepairFollowThrough::TargetMatched => {
                    no_change_repairs = 0;
                    target_not_followed_repairs = 0;
                }
            }
            let compile_repair_no_change = !current_report.compile_errors.is_empty()
                && matches!(repair_follow_through, RepairFollowThrough::NoChange);
            let repair_failure_kind = if compile_repair_no_change {
                "compile_repair_no_source_change"
            } else {
                repair_follow_through.failure_kind().unwrap_or("")
            };
            let retry_signature = verification_report_signature(&retry);
            let report_signature_unchanged = retry_signature == current_report_signature;
            if report_signature_unchanged && repair_turn_changed_paths.is_empty() {
                identical_no_change_repairs += 1;
            } else {
                identical_no_change_repairs = 0;
            }
            merge_unique_strings(
                &mut outcome.repair_targets,
                &[retry_target.as_str().to_string()],
            );
            if !retry.is_pass() {
                merge_unique_strings(&mut outcome.verify_failures, &[retry.primary_reason()]);
                merge_unique_strings(
                    &mut outcome.command_failures,
                    &command_failure_summaries(&retry),
                );
                outcome.primary_failure = Some(retry.primary_reason());
            }
            eval_events::emit(
                config.eval_events_path.as_deref(),
                json!({
                    "event": "step_verify_repair",
                    "step_id": step.id,
                    "attempt": attempt,
                    "ok": retry.is_pass(),
                    "repair_target": retry_target.as_str(),
                    "previous_repair_target": previous_target.as_str(),
                    "repair_target_followed": repair_target_followed,
                    "target_relation": repair_follow_through.as_str(),
                    "repair_follow_through": repair_follow_through.as_str(),
                    "failure_kind": repair_failure_kind,
                    "primary_reason": eval_events::body_snippet(&retry.primary_reason()),
                    "changed_paths": repair_turn_changed_paths.clone(),
                    "changed_paths_before": changed_paths_before_repair,
                    "changed_paths_after": context.changed_files.clone(),
                    "repair_turn_changed_paths": repair_turn_changed_paths.clone(),
                    "no_change_repairs": no_change_repairs,
                    "target_not_followed_repairs": target_not_followed_repairs,
                    "identical_no_change_repairs": identical_no_change_repairs,
                    "report_signature_unchanged": report_signature_unchanged,
                    "allowed_action": previous_target.allowed_action(),
                    "repair_stop_reason": repair_stop_reason.clone().unwrap_or_default(),
                    "repair_session_mode": repair_session_mode.as_str(),
                    "compile_reanchored_retry": context.compile_reanchored_retry,
                    "compile_repair_no_source_change": compile_repair_no_change,
                    "dependency_setup_authority": setup_authority.as_str(),
                    "repair_reachable": retry_reachability.reachable,
                    "reachable": retry_reachability.reachable,
                    "viable_actions": reachability_action_labels(&retry_reachability),
                    "blocked_requirements": retry_reachability.blocked_requirements.clone(),
                }),
            );
            if retry.is_pass() {
                if production_build_lifecycle_passed(&retry_lifecycles) {
                    snapshot_last_known_good_sources(
                        config,
                        mode,
                        Some(&step.id),
                        &config.profile,
                        &plan.goal,
                        &runtime_step.expected_paths,
                    );
                }
                outcome.primary_failure = None;
                outcome.stop_reason = repair_stop_reason.clone();
                return Ok(outcome);
            }
            if hook_snapshot_feedback_given && !hook_snapshot_restore_used {
                match hook_snapshot::restore_first_missing_with_runtime(
                    config,
                    runtime,
                    overall_goal,
                ) {
                    Ok(Some(restored)) => {
                        hook_snapshot_restore_used = true;
                        merge_changed_files(
                            &mut context,
                            std::slice::from_ref(&restored.restored_path),
                        );
                        merge_unique_strings(
                            &mut outcome.changed_paths,
                            std::slice::from_ref(&restored.restored_path),
                        );
                        merge_unique_strings(
                            &mut outcome.repair_changed_paths,
                            std::slice::from_ref(&restored.restored_path),
                        );
                        let (restored_retry, restored_lifecycles) = verify_step_with_context(
                            &config.workspace_root,
                            &runtime_step,
                            Some(&config.profile),
                            Some(overall_goal),
                            setup_authority,
                            config.offline,
                            config.eval_events_path.as_deref(),
                        );
                        apply_runtime_command_normalizations(&mut runtime_step, &restored_retry);
                        context.verify_commands = runtime_step.verify.clone();
                        for lifecycle in &restored_lifecycles {
                            emit_dependency_build_lifecycle(
                                config.eval_events_path.as_deref(),
                                mode,
                                Some(&step.id),
                                lifecycle,
                            );
                        }
                        eval_events::emit(
                            config.eval_events_path.as_deref(),
                            json!({
                                "event": "step_verify_repair",
                                "step_id": step.id,
                                "attempt": attempt,
                                "ok": restored_retry.is_pass(),
                                "repair_target": classify_repair_target(&restored_retry).as_str(),
                                "previous_repair_target": retry_target.as_str(),
                                "repair_target_followed": true,
                                "target_relation": "hook_snapshot_restore",
                                "repair_follow_through": "hook_snapshot_restore",
                                "failure_kind": if restored_retry.is_pass() { "" } else { "hook_snapshot_restore_unresolved" },
                                "primary_reason": eval_events::body_snippet(&restored_retry.primary_reason()),
                                "changed_paths": [restored.restored_path.clone()],
                                "repair_turn_changed_paths": [restored.restored_path.clone()],
                                "repair_session_mode": "hook_snapshot_restore",
                                "dependency_setup_authority": setup_authority.as_str(),
                                "repair_reachable": true,
                                "reachable": true,
                            }),
                        );
                        if restored_retry.is_pass() {
                            outcome.primary_failure = None;
                            outcome.stop_reason = Some("hook_snapshot_restore_applied".to_string());
                            return Ok(outcome);
                        }
                        let restored_reachability = repair_policy.assess(
                            &restored_retry,
                            setup_authority,
                            config.offline,
                            attempt,
                        );
                        if !restored_reachability.reachable {
                            terminal_repair_failure_kind =
                                Some(reachability_failure_kind(&restored_reachability).to_string());
                            terminal_blocked_requirements =
                                restored_reachability.blocked_requirements.clone();
                            context.progress_warning =
                                Some(reachability_recovery_reason(&restored_reachability));
                            current_report = restored_retry;
                            emit_repair_unreachable(
                                config,
                                mode,
                                &step.id,
                                classify_repair_target(&current_report).as_str(),
                                &current_report.primary_reason(),
                                &restored_reachability,
                            );
                            break;
                        }
                        previous_missing = restored_retry.missing_paths.len();
                        current_report_signature = verification_report_signature(&restored_retry);
                        current_report = restored_retry;
                        continue;
                    }
                    Ok(None) => {}
                    Err(err) => {
                        context.progress_warning =
                            Some(format!("Hook snapshot restore failed: {err}"));
                    }
                }
            }
            if compile_repair_no_change && repair_session_mode == RepairSessionMode::Compact {
                match single_compile_regeneration_target(&retry) {
                    Ok(target_path) => {
                        let Some(target_abs) =
                            writable_workspace_source_path(&config.workspace_root, &target_path)
                        else {
                            emit_compile_regeneration_event(
                                config,
                                Some(&step.id),
                                "step_repair",
                                false,
                                false,
                                0,
                                Some(&target_path),
                                "target_path_rejected",
                                retry.compile_errors.len(),
                                retry.compile_errors.len(),
                                &[],
                            );
                            current_report = retry;
                            terminal_repair_failure_kind =
                                Some("compile_repair_no_source_change".to_string());
                            break;
                        };
                        let before_content = match std::fs::read(&target_abs) {
                            Ok(content) => content,
                            Err(err) => {
                                emit_compile_regeneration_event(
                                    config,
                                    Some(&step.id),
                                    "step_repair",
                                    false,
                                    false,
                                    0,
                                    Some(&target_path),
                                    &format!("snapshot_read_error:{err}"),
                                    retry.compile_errors.len(),
                                    retry.compile_errors.len(),
                                    &[],
                                );
                                current_report = retry;
                                terminal_repair_failure_kind =
                                    Some("compile_repair_no_source_change".to_string());
                                break;
                            }
                        };
                        let before_error_count = retry.compile_errors.len();
                        let regeneration_prompt = build_compile_regeneration_prompt_with_context(
                            &step.id,
                            &retry,
                            &context,
                            &target_path,
                        );
                        let mut regeneration_session = SessionSnapshot::new();
                        let regeneration = run_session_with_outcome_with_options(
                            client,
                            &mut regeneration_session,
                            &regeneration_prompt,
                            std::slice::from_ref(&target_path),
                            &repair_config,
                            ui,
                            repair_options.clone(),
                        );
                        let regeneration = match regeneration {
                            Ok(regeneration) => regeneration,
                            Err(err) => {
                                let _ = std::fs::write(&target_abs, &before_content);
                                emit_compile_regeneration_event(
                                    config,
                                    Some(&step.id),
                                    "step_repair",
                                    true,
                                    false,
                                    0,
                                    Some(&target_path),
                                    &format!(
                                        "regeneration_turn_error:{}",
                                        eval_events::body_snippet(&err.to_string())
                                    ),
                                    before_error_count,
                                    before_error_count,
                                    &[],
                                );
                                current_report = retry;
                                if compile_repair_no_change && no_change_repairs >= 2 {
                                    terminal_repair_failure_kind =
                                        Some("compile_repair_no_source_change".to_string());
                                    break;
                                }
                                current_report_signature = retry_signature;
                                continue;
                            }
                        };
                        let mut regeneration_changed_paths = regeneration.changed_paths.clone();
                        regeneration_changed_paths.sort();
                        regeneration_changed_paths.dedup();
                        let one_file_write =
                            changed_paths_only_target(&regeneration_changed_paths, &target_path);
                        let (regenerated_report, regeneration_lifecycles) =
                            verify_step_with_context(
                                &config.workspace_root,
                                &runtime_step,
                                Some(&config.profile),
                                Some(overall_goal),
                                setup_authority,
                                config.offline,
                                config.eval_events_path.as_deref(),
                            );
                        apply_runtime_command_normalizations(
                            &mut runtime_step,
                            &regenerated_report,
                        );
                        context.verify_commands = runtime_step.verify.clone();
                        for lifecycle in &regeneration_lifecycles {
                            emit_dependency_build_lifecycle(
                                config.eval_events_path.as_deref(),
                                mode,
                                Some(&step.id),
                                lifecycle,
                            );
                        }
                        let after_error_count = regenerated_report.compile_errors.len();
                        let error_delta = before_error_count as i64 - after_error_count as i64;
                        if one_file_write && error_delta > 0 {
                            emit_compile_regeneration_event(
                                config,
                                Some(&step.id),
                                "step_repair",
                                true,
                                true,
                                error_delta,
                                Some(&target_path),
                                "accepted",
                                before_error_count,
                                after_error_count,
                                &regeneration_changed_paths,
                            );
                            merge_changed_files(&mut context, std::slice::from_ref(&target_path));
                            merge_unique_strings(
                                &mut outcome.changed_paths,
                                std::slice::from_ref(&target_path),
                            );
                            merge_unique_strings(
                                &mut outcome.repair_changed_paths,
                                std::slice::from_ref(&target_path),
                            );
                            if regenerated_report.is_pass() {
                                if production_build_lifecycle_passed(&regeneration_lifecycles) {
                                    snapshot_last_known_good_sources(
                                        config,
                                        mode,
                                        Some(&step.id),
                                        &config.profile,
                                        &plan.goal,
                                        &runtime_step.expected_paths,
                                    );
                                }
                                outcome.primary_failure = None;
                                outcome.stop_reason =
                                    Some("compile_regeneration_applied".to_string());
                                return Ok(outcome);
                            }
                            let regenerated_reachability = repair_policy.assess(
                                &regenerated_report,
                                setup_authority,
                                config.offline,
                                attempt,
                            );
                            if !regenerated_reachability.reachable {
                                terminal_repair_failure_kind = Some(
                                    reachability_failure_kind(&regenerated_reachability)
                                        .to_string(),
                                );
                                terminal_blocked_requirements =
                                    regenerated_reachability.blocked_requirements.clone();
                                context.progress_warning =
                                    Some(reachability_recovery_reason(&regenerated_reachability));
                                current_report = regenerated_report;
                                emit_repair_unreachable(
                                    config,
                                    mode,
                                    &step.id,
                                    classify_repair_target(&current_report).as_str(),
                                    &current_report.primary_reason(),
                                    &regenerated_reachability,
                                );
                                break;
                            }
                            current_report_signature =
                                verification_report_signature(&regenerated_report);
                            previous_missing = regenerated_report.missing_paths.len();
                            current_report = regenerated_report;
                            continue;
                        }
                        let _ = std::fs::write(&target_abs, &before_content);
                        emit_compile_regeneration_event(
                            config,
                            Some(&step.id),
                            "step_repair",
                            true,
                            false,
                            error_delta,
                            Some(&target_path),
                            if one_file_write {
                                "compile_error_count_not_decreased"
                            } else {
                                "changed_paths_not_single_target"
                            },
                            before_error_count,
                            after_error_count,
                            &regeneration_changed_paths,
                        );
                    }
                    Err(reason) => emit_compile_regeneration_event(
                        config,
                        Some(&step.id),
                        "step_repair",
                        false,
                        false,
                        0,
                        None,
                        &reason,
                        retry.compile_errors.len(),
                        retry.compile_errors.len(),
                        &[],
                    ),
                }
            }
            current_reachability = retry_reachability;
            if !current_reachability.reachable {
                terminal_repair_failure_kind =
                    Some(reachability_failure_kind(&current_reachability).to_string());
                terminal_blocked_requirements = current_reachability.blocked_requirements.clone();
                context.progress_warning =
                    Some(reachability_recovery_reason(&current_reachability));
                current_report = retry;
                emit_repair_unreachable(
                    config,
                    mode,
                    &step.id,
                    classify_repair_target(&current_report).as_str(),
                    &current_report.primary_reason(),
                    &current_reachability,
                );
                break;
            }
            let next_missing = retry.missing_paths.len();
            if next_missing >= previous_missing {
                context.progress_warning = Some(format!(
                    "Missing expected paths did not decrease after repair. Remaining: {}",
                    if retry.missing_paths.is_empty() {
                        "none".to_string()
                    } else {
                        retry.missing_paths.join(", ")
                    }
                ));
            }
            previous_missing = next_missing;
            current_report = retry;
            current_report_signature = retry_signature;
            if compile_repair_no_change && no_change_repairs >= 2 {
                terminal_repair_failure_kind = Some("compile_repair_no_source_change".to_string());
                break;
            }
            if identical_no_change_repairs >= STEP_REPAIR_IDENTICAL_NO_CHANGE_LIMIT {
                terminal_repair_failure_kind = Some(if !current_report.compile_errors.is_empty() {
                    "compile_repair_no_source_change".to_string()
                } else {
                    "verify_repair_progress_unchanged".to_string()
                });
                break;
            }
        }
    }
    let final_failure_kind =
        terminal_repair_failure_kind.unwrap_or_else(|| "bounded_repair_exhausted".to_string());
    context.repair_stop_reason = Some(final_failure_kind.to_string());
    let final_repair_target = classify_repair_target(&current_report);
    if !current_report.compile_errors.is_empty() {
        match try_compile_rollback_after_repair_exhaustion(
            config,
            &config.profile,
            overall_goal,
            phase_scope.unwrap_or(&step.id),
            &step.instruction,
            &current_report,
            &final_failure_kind,
        ) {
            Ok(Some(rollback)) => {
                outcome.primary_failure = None;
                outcome.stop_reason = Some("compile_rollback_applied".to_string());
                outcome.partial = true;
                outcome.compile_rollbacks.push(rollback);
                return Ok(outcome);
            }
            Ok(None) => {}
            Err(err) => {
                let message = err.to_string();
                outcome.primary_failure = Some(message.clone());
                outcome.stop_reason = Some("compile_rollback_error".to_string());
                outcome.partial = true;
                return Err(StepRunError { message, outcome });
            }
        }
    }
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "loop_stop",
            "reason": final_failure_kind,
            "step_id": step.id,
            "repair_target": final_repair_target.as_str(),
            "repair_stop_reason": repair_stop_reason.clone().unwrap_or_default(),
            "local_repair_exhausted": true,
        }),
    );
    let repair_report_path = match save_repair_report_with_context(
        &config.workspace_root,
        &step.id,
        &current_report,
        &context,
    ) {
        Ok(path) => path,
        Err(err) => {
            outcome.primary_failure = Some(err.to_string());
            outcome.stop_reason = Some("repair_report_save_error".to_string());
            outcome.partial = true;
            return Err(StepRunError {
                message: err.to_string(),
                outcome,
            });
        }
    };
    let step_handoff = RecoveryHandoff {
        profile: config.profile.clone(),
        original_goal: context
            .overall_goal
            .clone()
            .unwrap_or_else(|| plan.goal.clone()),
        failed_phase: None,
        failed_step: Some(step.id.clone()),
        failure_kind: final_failure_kind.to_string(),
        failure_evidence: std::iter::once(current_report.primary_reason())
            .chain(reachability_blocked_evidence(
                &terminal_blocked_requirements,
            ))
            .chain(
                current_report
                    .command_failures
                    .iter()
                    .map(|failure| format!("{}: {}", failure.command, failure.reason)),
            )
            .chain(
                current_report
                    .verifier_command_false_negatives
                    .iter()
                    .map(|failure| {
                        format!(
                            "deterministic_verify_command_bug: {}: {}",
                            failure.command, failure.reason
                        )
                    }),
            )
            .collect(),
        missing_paths: current_report.missing_paths.clone(),
        missing_capabilities: vec![final_repair_target.as_str().to_string()],
        verify_commands: context.verify_commands.clone(),
        changed_paths: context.changed_files.clone(),
        repair_targets: vec![final_repair_target.as_str().to_string()],
    };
    let recovery_plan_path =
        match save_recovery_ultra_plan(&config.workspace_root, &step.id, &step_handoff) {
            Ok(path) => Some(path),
            Err(err) => {
                let repair_report_display = handoff_path(&repair_report_path);
                eval_events::emit(
                    config.eval_events_path.as_deref(),
                    json!({
                        "event": "recovery_ultra_plan_save_failed",
                        "recovery_handoff_kind": final_failure_kind,
                        "step_id": step.id,
                        "recovery_prompt_path": repair_report_display,
                        "reason": eval_events::body_snippet(&err.to_string()),
                        "recovery_yaml_missing": true,
                    }),
                );
                None
            }
        };
    let validation =
        validate_recovery_artifacts(&repair_report_path, recovery_plan_path.as_deref());
    let raw_suggested_command =
        suggested_ultra_recovery_command(&repair_report_path, &config.profile);
    let suggested_command = if validation.prompt_command_available() {
        raw_suggested_command
    } else {
        String::new()
    };
    let suggested_yaml_command = recovery_plan_path
        .as_ref()
        .filter(|_| validation.yaml_command_available())
        .map(|path| suggested_recovery_ultra_plan_command(path));
    let repair_report_display = handoff_path(&repair_report_path);
    let recovery_plan_display = optional_handoff_path(recovery_plan_path.as_ref());
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "recovery_prompt_saved",
            "recovery_handoff_kind": final_failure_kind,
            "step_id": step.id,
            "recovery_prompt_path": &repair_report_display,
            "recovery_ultra_plan_path": &recovery_plan_display,
            "recovery_yaml_missing": recovery_plan_path.is_none(),
            "recovery_prompt_exists": validation.prompt_exists,
            "recovery_prompt_parse_ok": validation.prompt_parse_ok,
            "recovery_prompt_parse_error": validation.prompt_parse_error.as_deref().unwrap_or_default(),
            "recovery_yaml_exists": validation.yaml_exists,
            "recovery_yaml_parse_ok": validation.yaml_parse_ok,
            "recovery_yaml_parse_error": validation.yaml_parse_error.as_deref().unwrap_or_default(),
            "recovery_command_targets_valid": validation.command_targets_valid(),
            "suggested_recovery_command": suggested_command.clone(),
            "suggested_recovery_yaml_command": suggested_yaml_command.clone().unwrap_or_default(),
            "recovery_profile": config.profile,
            "local_repair_exhausted": true,
            "failure_kind": final_failure_kind,
            "status": "incomplete",
        }),
    );
    let yaml_summary = recovery_plan_path
        .as_ref()
        .map(|path| {
            let display = handoff_path(path);
            if validation.yaml_parse_ok {
                format!("Recovery UltraPlan YAML saved: {display}")
            } else {
                format!(
                    "Recovery UltraPlan YAML invalid: {} ({})",
                    display,
                    validation
                        .yaml_parse_error
                        .as_deref()
                        .unwrap_or("recovery_yaml_invalid")
                )
            }
        })
        .unwrap_or_else(|| {
            "Recovery UltraPlan YAML missing: failed to save valid recovery plan".to_string()
        });
    let prompt_command_summary = if validation.prompt_command_available() {
        format!("Suggested command: {suggested_command}")
    } else {
        format!(
            "Suggested command: unavailable because recovery prompt validation failed ({})",
            validation
                .prompt_parse_error
                .as_deref()
                .unwrap_or("recovery_prompt_invalid")
        )
    };
    let yaml_command_summary = suggested_yaml_command
        .as_ref()
        .map(|command| format!("Suggested YAML command: {command}"))
        .unwrap_or_else(|| {
            "Suggested YAML command: unavailable because recovery YAML is missing".to_string()
        });
    let artifact_check_summary = recovery_artifact_check_summary(&validation);
    eval_events::write_run_summary(
        config.eval_events_path.as_deref(),
        &format!(
            "Status: incomplete\n{}\n{}\nRecovery prompt saved: {}\n{}\n{}\nFailure: {}",
            yaml_summary,
            yaml_command_summary,
            repair_report_display,
            prompt_command_summary,
            artifact_check_summary,
            eval_events::body_snippet(&current_report.primary_reason())
        ),
    );
    let prompt_message = if validation.prompt_command_available() {
        format!("suggested command: {suggested_command}")
    } else {
        "suggested command unavailable because recovery prompt validation failed".to_string()
    };
    let yaml_message = suggested_yaml_command
        .as_ref()
        .map(|command| format!("suggested YAML command: {command}"))
        .unwrap_or_else(|| {
            "suggested YAML command unavailable because recovery YAML is missing".to_string()
        });
    let message = eval_events::render_stop_reason(&eval_events::StopReasonParts {
        free_text: format!(
            "step {} failed verification after bounded repair: {}; failure_kind={}; incomplete; {}",
            step.id,
            current_report.primary_reason(),
            final_failure_kind,
            artifact_check_summary
        ),
        paths: vec![
            format!("repair prompt saved: {repair_report_display}"),
            yaml_summary,
        ],
        commands: vec![prompt_message, yaml_message],
    });
    outcome.primary_failure = Some(current_report.primary_reason());
    outcome.stop_reason = Some(final_failure_kind.to_string());
    outcome.partial = true;
    Err(StepRunError { message, outcome })
}

pub(super) fn recovery_fix_implement_requires_write(
    config: &Config,
    step: &PlanStep,
) -> anyhow::Result<bool> {
    Ok(step.step_kind() == StepKind::Implement
        && crate::planner::recovery_contract_binding::load_fix_origin(config)?.is_some())
}

pub(super) fn step_verify_setup_authority(
    plan: &StepPlan,
    step: &PlanStep,
    fallback: NodeDependencySetupAuthority,
) -> NodeDependencySetupAuthority {
    if step.step_kind() == StepKind::Setup {
        return NodeDependencySetupAuthority::PlanSetupStep;
    }
    if step.step_kind() != StepKind::Verify {
        return fallback;
    }
    let prior_setup_exists = plan
        .steps
        .iter()
        .take_while(|candidate| candidate.id != step.id)
        .any(|candidate| candidate.step_kind() == StepKind::Setup);
    if prior_setup_exists {
        NodeDependencySetupAuthority::PlanSetupStep
    } else {
        fallback
    }
}

pub(super) fn step_contract_setup_authority(
    _plan: &StepPlan,
    step: &PlanStep,
    phase_scope: Option<&str>,
    fallback: NodeDependencySetupAuthority,
) -> NodeDependencySetupAuthority {
    if step.step_kind() != StepKind::Implement {
        return NodeDependencySetupAuthority::None;
    }
    if step_or_phase_is_dependency_setup_purpose(step, phase_scope) {
        NodeDependencySetupAuthority::PlanSetupStep
    } else {
        fallback
    }
}

pub(super) fn step_carries_setup_authority(
    plan: &StepPlan,
    step: &PlanStep,
    phase_scope: Option<&str>,
) -> bool {
    step_verify_setup_authority(plan, step, NodeDependencySetupAuthority::None).allows_setup()
        || step_contract_setup_authority(
            plan,
            step,
            phase_scope,
            NodeDependencySetupAuthority::None,
        )
        .allows_setup()
}

pub(super) fn step_or_phase_is_dependency_setup_purpose(
    step: &PlanStep,
    phase_scope: Option<&str>,
) -> bool {
    [
        step.id.as_str(),
        step.kind.as_str(),
        step.instruction.as_str(),
    ]
    .into_iter()
    .chain(phase_scope)
    .any(text_mentions_dependency_setup)
}

pub(super) fn text_mentions_dependency_setup(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    signals::contains_setup_token(text)
        && (lower.contains("depend")
            || lower.contains("workspace")
            || lower.contains("package")
            || lower.contains("npm")
            || text.contains("依存"))
}

pub(super) fn should_run_setup_step_dependency_state_lifecycle(
    root: &Path,
    step: &PlanStep,
    phase_scope: Option<&str>,
    setup_authority: NodeDependencySetupAuthority,
) -> bool {
    step.step_kind() == StepKind::Setup
        && setup_authority == NodeDependencySetupAuthority::PlanSetupStep
        && step_or_phase_is_dependency_setup_purpose(step, phase_scope)
        && dependency_setup::package_json_declares_dependencies(root)
        && !dependency_setup::node_declared_dependencies_ready(root)
}

pub(super) fn merge_verification_report(
    report: &mut VerificationReport,
    extra: VerificationReport,
) {
    for path in extra.missing_paths {
        report.push_missing_path(path);
    }
    for reason in extra.dependency_missing {
        report.push_dependency_missing(reason);
    }
    for failure in extra.command_failures {
        report.push_command_failure(failure.command, failure.reason);
    }
    for failure in extra.verifier_command_false_negatives {
        report.push_verifier_command_false_negative(failure.command, failure.reason);
    }
    for normalization in extra.runtime_command_normalizations {
        report.runtime_command_normalizations.push(normalization);
    }
    for error in extra.compile_errors {
        if !report.compile_errors.contains(&error) {
            report.compile_errors.push(error);
        }
    }
    for traceback in extra.python_tracebacks {
        report.push_python_traceback(traceback);
    }
    for reason in extra.profile_failures {
        report.push_profile_failure(reason);
    }
    report.refresh_status();
}

pub(super) fn apply_runtime_command_normalizations(
    step: &mut PlanStep,
    report: &VerificationReport,
) {
    for normalization in &report.runtime_command_normalizations {
        for command in &mut step.verify {
            if command == &normalization.original {
                *command = normalization.repaired.clone();
            }
        }
    }
}

pub(super) fn step_run_session_options(
    plan: &StepPlan,
    step: &PlanStep,
    contract_enforcement: ContractEnforcement,
    phase_scope: Option<&str>,
    setup_authority: NodeDependencySetupAuthority,
) -> RunSessionOptions {
    RunSessionOptions::plan_step_with_enforcement(
        run_session_step_kind(step),
        contract_enforcement,
        phase_scope.map(str::to_string),
    )
    .with_dependency_setup_authority(setup_authority)
    .with_path_fallback_candidates(plan_expected_paths(plan))
}

pub(super) fn plan_expected_paths(plan: &StepPlan) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for path in plan
        .steps
        .iter()
        .flat_map(|step| step.expected_paths.iter())
    {
        if seen.insert(path.clone()) {
            out.push(path.clone());
        }
    }
    out
}

pub(super) fn verify_step_completion_observed(
    config: &Config,
    goal: &str,
    runtime_step: &PlanStep,
    original_step: &PlanStep,
    phase_scope: Option<&str>,
    setup_authority: NodeDependencySetupAuthority,
) -> (VerificationReport, Vec<BuildVerifierLifecycleObservation>) {
    let (mut report, mut build_lifecycles) = verify_step_with_context(
        &config.workspace_root,
        runtime_step,
        Some(&config.profile),
        Some(goal),
        setup_authority,
        config.offline,
        config.eval_events_path.as_deref(),
    );
    if should_run_setup_step_dependency_state_lifecycle(
        &config.workspace_root,
        original_step,
        phase_scope,
        setup_authority,
    ) {
        let (dependency_report, mut dependency_lifecycles) =
            verify_setup_dependency_state_with_setup_observed_with_offline(
                &config.workspace_root,
                setup_authority,
                config.offline,
            );
        merge_verification_report(&mut report, dependency_report);
        build_lifecycles.append(&mut dependency_lifecycles);
    }
    (report, build_lifecycles)
}

pub(super) fn emit_runner_step_short_circuited(
    config: &Config,
    step: &PlanStep,
    phase_scope: Option<&str>,
    required_paths: &[String],
    at: &str,
) {
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "step_short_circuited",
            "at": at,
            "step_id": step.id,
            "step_kind": run_session_step_kind(step).as_str(),
            "phase_scope": phase_scope.unwrap_or(""),
            "required_paths": required_paths,
            "verify_commands": step.verify.clone(),
            "session_scope": "plan-run-step",
        }),
    );
}

pub(super) fn run_session_step_kind(step: &PlanStep) -> RunSessionStepKind {
    match step.step_kind() {
        StepKind::Inspect => RunSessionStepKind::Inspect,
        StepKind::Setup => RunSessionStepKind::Setup,
        StepKind::Implement => RunSessionStepKind::Implement,
        StepKind::Verify => RunSessionStepKind::Verify,
        StepKind::Report => RunSessionStepKind::Report,
        StepKind::Unknown(_) => RunSessionStepKind::Unknown,
    }
}

pub(super) fn capped_config(config: &Config, cap: usize) -> Config {
    let mut out = config.clone();
    out.max_iterations = out.max_iterations.min(cap);
    out
}

pub(super) fn required_final_artifacts(plan: &StepPlan, root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    merge_unique_strings(
        &mut out,
        &extract_requested_artifact_paths(root, &plan.goal),
    );
    for step in &plan.steps {
        merge_unique_strings(&mut out, &step.expected_paths);
    }
    out
}

pub(super) fn explicit_completion_contract_path(config: &Config) -> Option<PathBuf> {
    config.completion_contract_path.clone().or_else(|| {
        crate::env_compat::var_os("COMMANDAGENT_COMPLETION_CONTRACT").map(PathBuf::from)
    })
}

pub(super) fn generated_completion_contract_path(config: &Config, scope: &str) -> PathBuf {
    let filename = format!(
        "completion-contract-{}.json",
        scope
            .chars()
            .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
            .collect::<String>()
    );
    crate::planner::completion_contract_path::generated_path(
        &config.workspace_root,
        config.eval_events_path.as_deref(),
        &filename,
    )
}

pub(super) fn display_path_for_event(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

pub(super) fn emit_completion_contract_bound(
    config: &Config,
    scope: &str,
    profile: &str,
    goal: &str,
    bound: &BoundCompletionContract,
) {
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "completion_contract_bound",
            "session_scope": scope,
            "completion_contract_verification_enabled": true,
            "completion_contract_path_merge_enabled": true,
            "external_contract_checked": true,
            "external_contract_required": bound.required,
            "completion_contract_generated": bound.generated,
            "completion_contract_path": bound.path,
            "required_paths": bound.contract.required_paths.clone(),
            "required_capabilities": bound.contract.required_capabilities.clone(),
            "required_evidence": bound.contract.required_evidence.clone(),
            "evidence_hint_tokens": bound.contract.evidence_hint_tokens.clone(),
            "required_obligations": bound.contract.required_obligations.clone(),
        }),
    );
    // E5B_PROFILE_DISPATCH_ALLOW: telemetry-generic-contract
    if ProfileId::parse(profile) == ProfileId::Generic
        && bound
            .contract
            .required_capabilities
            .iter()
            .any(|capability| capability == GENERIC_INTERACTIVE_CONTRACT_CAPABILITY)
        && let Some(matched_intent_token) = signals::matched_app_intent_token(goal)
    {
        eval_events::emit(
            config.eval_events_path.as_deref(),
            json!({
                "event": "generic_contract_bound",
                "session_scope": scope,
                "matched_intent_token": matched_intent_token,
                "inferred_keys": GENERIC_INTERACTIVE_EVIDENCE_KEYS,
                "required_capabilities": bound.contract.required_capabilities.clone(),
                "required_evidence": bound.contract.required_evidence.clone(),
                "required_obligations": bound.contract.required_obligations.clone(),
                "completion_contract_path": bound.path,
            }),
        );
    }
}

// Intermediate invariant repair and phase/recovery event boundary.
#[allow(clippy::too_many_arguments)]
pub(super) fn repair_intermediate_profile_invariant(
    execution: &mut dyn ChatClient,
    ultra_session: &mut SessionSnapshot,
    config: &Config,
    plan: &UltraPlan,
    phase: &UltraPhase,
    index: usize,
    profile_snapshot: &ProfileSnapshot,
    step_plan: &StepPlan,
    ultra_context: &mut UltraRunContext,
    final_expected_paths: &[String],
    ui: &dyn InteractionUi,
    failed_report: VerificationReport,
    setup_authority_state: &mut UltraRunSetupAuthorityState,
) -> anyhow::Result<VerificationReport> {
    let mut retry = failed_report.clone();
    let runtime = resolve_profile_runtime(&plan.profile);
    let deterministic_error =
        match runtime.deterministic_repair(&config.workspace_root, &plan.goal, &failed_report) {
            Ok(changed) => {
                if changed {
                    setup_authority_state.grant("manifest_repair");
                    reconcile_run_dependency_setup(
                        config,
                        &plan.profile,
                        DependencyReconciliationTrigger::ManifestRepair,
                        setup_authority_state,
                    )?;
                }
                retry = verify_invariant_with_hooks(
                    config,
                    resolve_profile_runtime(&plan.profile),
                    plan,
                    profile_snapshot,
                );
                emit_profile_invariant_repair_event(
                    config,
                    plan,
                    phase,
                    index,
                    "deterministic",
                    changed,
                    retry.is_pass(),
                    &retry.primary_reason(),
                );
                None
            }
            Err(err) => {
                let message = err.to_string();
                emit_profile_invariant_repair_event(
                    config,
                    plan,
                    phase,
                    index,
                    "deterministic",
                    false,
                    false,
                    &message,
                );
                Some(message)
            }
        };
    if retry.is_pass() {
        return Ok(confirm_phase_build_after_profile_repair(
            config, plan, phase, index, step_plan, retry,
        ));
    }

    let expected_paths = runtime.filter_invariant_expected_paths(
        &config.workspace_root,
        runtime.expected_scaffold_paths(&config.workspace_root, &plan.goal),
    );
    let mut hook_snapshot_feedback_given = false;
    let mut repair_prompt = profile_invariant_model_repair_prompt(
        plan,
        phase,
        &retry,
        ultra_context,
        &expected_paths,
        config,
        deterministic_error.as_deref(),
    );
    repair_prompt = hook_snapshot::prefix_feedback_if_missing_with_runtime(
        config,
        runtime,
        &plan.goal,
        "profile_invariant_repair",
        Some(&phase.id),
        &mut hook_snapshot_feedback_given,
        repair_prompt,
    );
    let repair_config = capped_config(config, STEP_REPAIR_MAX_ITERATIONS);
    match run_profile_repair_with_ultra_session(
        execution,
        ultra_session,
        &repair_prompt,
        &plan.intent,
        &expected_paths,
        &repair_config,
        ui,
    ) {
        Ok(repair_outcome) => {
            push_context_items_capped(
                &mut ultra_context.created_or_changed_paths,
                &repair_outcome.changed_paths,
                ULTRA_CONTEXT_MAX_PATHS,
                &mut ultra_context.truncated,
            );
            push_context_items_capped(
                &mut ultra_context.last_repair_changed_paths,
                &repair_outcome.changed_paths,
                ULTRA_CONTEXT_MAX_PATHS,
                &mut ultra_context.truncated,
            );
            emit_ultra_phase_context_updated(
                config,
                plan,
                phase,
                index,
                ultra_context,
                ultra_session.messages.len(),
                true,
            );
            retry = verify_invariant_with_hooks(
                config,
                resolve_profile_runtime(&plan.profile),
                plan,
                profile_snapshot,
            );
            emit_profile_invariant_repair_event(
                config,
                plan,
                phase,
                index,
                "model",
                !repair_outcome.changed_paths.is_empty(),
                retry.is_pass(),
                &retry.primary_reason(),
            );
            if retry.is_pass() {
                return Ok(confirm_phase_build_after_profile_repair(
                    config, plan, phase, index, step_plan, retry,
                ));
            }
            if hook_snapshot_feedback_given
                && let Some(restored) =
                    hook_snapshot::restore_first_missing_with_runtime(config, runtime, &plan.goal)?
            {
                push_context_items_capped(
                    &mut ultra_context.created_or_changed_paths,
                    std::slice::from_ref(&restored.restored_path),
                    ULTRA_CONTEXT_MAX_PATHS,
                    &mut ultra_context.truncated,
                );
                push_context_items_capped(
                    &mut ultra_context.last_repair_changed_paths,
                    std::slice::from_ref(&restored.restored_path),
                    ULTRA_CONTEXT_MAX_PATHS,
                    &mut ultra_context.truncated,
                );
                retry = verify_invariant_with_hooks(
                    config,
                    resolve_profile_runtime(&plan.profile),
                    plan,
                    profile_snapshot,
                );
                emit_profile_invariant_repair_event(
                    config,
                    plan,
                    phase,
                    index,
                    "hook_snapshot_restore",
                    true,
                    retry.is_pass(),
                    &retry.primary_reason(),
                );
                if retry.is_pass() {
                    return Ok(confirm_phase_build_after_profile_repair(
                        config, plan, phase, index, step_plan, retry,
                    ));
                }
            }
        }
        Err(err) => {
            emit_profile_invariant_repair_event(
                config,
                plan,
                phase,
                index,
                "model",
                false,
                false,
                &err.to_string(),
            );
        }
    }
    let fresh_evidence = fresh_profile_invariant_failure_evidence(
        config,
        plan,
        profile_snapshot,
        final_expected_paths,
    );
    retry = fresh_evidence.report.clone();
    if retry.is_pass() {
        return Ok(confirm_phase_build_after_profile_repair(
            config, plan, phase, index, step_plan, retry,
        ));
    }
    ultra_context.update_after_profile_failure(
        phase,
        &retry.primary_reason(),
        fresh_evidence.missing_paths,
    );
    emit_ultra_phase_context_updated(
        config,
        plan,
        phase,
        index,
        ultra_context,
        ultra_session.messages.len(),
        true,
    );
    Ok(retry)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_profile_invariant_repair_event(
    config: &Config,
    plan: &UltraPlan,
    phase: &UltraPhase,
    index: usize,
    method: &str,
    changed: bool,
    ok: bool,
    reason: &str,
) {
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "profile_invariant_repair",
            "phase_id": phase.id,
            "phase_index": index + 1,
            "total_phases": plan.phases.len(),
            "final_phase": false,
            "method": method,
            "changed": changed,
            "ok": ok,
            "reason": eval_events::body_snippet(reason),
            "bounded_repair": method == "model",
        }),
    );
}

pub(super) fn confirm_phase_build_after_profile_repair(
    config: &Config,
    plan: &UltraPlan,
    phase: &UltraPhase,
    index: usize,
    step_plan: &StepPlan,
    profile_report: VerificationReport,
) -> VerificationReport {
    let build_commands = phase_build_verify_commands(step_plan);
    if build_commands.is_empty() {
        return profile_report;
    }
    let build_step = PlanStep {
        id: "profile-repair-build".to_string(),
        kind: "verify".to_string(),
        expected_result: "pass".to_string(),
        instruction: "Re-run the phase build verification after profile repair".to_string(),
        expected_paths: Vec::new(),
        verify: build_commands.clone(),
    };
    let (build_report, build_lifecycles) = verify_step_with_profile_setup_observed_with_offline(
        &config.workspace_root,
        &build_step,
        Some(&plan.profile),
        NodeDependencySetupAuthority::None,
        config.offline,
    );
    for lifecycle in &build_lifecycles {
        emit_dependency_build_lifecycle(
            config.eval_events_path.as_deref(),
            "ultra-plan-run",
            Some(&phase.id),
            lifecycle,
        );
    }
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "profile_invariant_repair_build_verify",
            "phase_id": phase.id,
            "phase_index": index + 1,
            "total_phases": plan.phases.len(),
            "commands": build_commands,
            "ok": build_report.is_pass(),
            "reason": eval_events::body_snippet(&build_report.primary_reason()),
        }),
    );
    if build_report.is_pass() {
        let expected_paths = step_plan
            .steps
            .iter()
            .flat_map(|step| step.expected_paths.iter().cloned())
            .collect::<Vec<_>>();
        snapshot_last_known_good_sources(
            config,
            "profile_invariant_repair",
            Some(&phase.id),
            &plan.profile,
            &plan.goal,
            &expected_paths,
        );
        profile_report
    } else {
        build_report
    }
}

pub(super) fn phase_build_verify_commands(plan: &StepPlan) -> Vec<String> {
    let mut commands = Vec::new();
    for command in plan.steps.iter().flat_map(|step| step.verify.iter()) {
        if is_nextjs_build_verify_command_like(command) && !commands.contains(command) {
            commands.push(command.clone());
        }
    }
    commands
}

pub(super) fn production_build_lifecycle_passed(
    lifecycles: &[BuildVerifierLifecycleObservation],
) -> bool {
    lifecycles
        .iter()
        .any(|lifecycle| lifecycle.final_status == BuildVerifierStatus::Passed)
}

pub(super) fn route_bound_source_paths(root: &Path, profile: &str) -> Vec<String> {
    resolve_profile_runtime(profile)
        .route_bound_closure(root)
        .into_iter()
        .filter_map(|path| {
            path.to_str()
                .and_then(safe_source_rel_path)
                .map(|path| path.replace('\\', "/"))
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(super) fn depth_profile(
    root: &Path,
    profile: &str,
    state_dimensions_changed: &[String],
    action_hooks: &[String],
    interaction_evidence_path: &str,
    text_telemetry: &InteractionTextTelemetry,
) -> DepthProfile {
    let route_bound_source_line_count = route_bound_source_paths(root, profile)
        .iter()
        .map(|path| source_line_count(&root.join(path)))
        .sum();
    let state_dimensions_count = state_dimensions_changed
        .iter()
        .filter(|value| !value.trim().is_empty())
        .collect::<BTreeSet<_>>()
        .len();
    let source_action_kinds = route_bound_data_anvil_action_kinds(root, profile);
    let data_anvil_action_kind_count = source_action_kinds
        .iter()
        .chain(action_hooks.iter())
        .filter(|value| !value.trim().is_empty())
        .collect::<BTreeSet<_>>()
        .len();
    let input_types_with_observed_state_change_count =
        input_types_with_observed_state_change(interaction_evidence_path, text_telemetry).len();
    let summary = format!(
        "route_bound_source_lines={} state_dimensions={} data_anvil_action_kinds={} input_types_with_observed_state_change={}",
        route_bound_source_line_count,
        state_dimensions_count,
        data_anvil_action_kind_count,
        input_types_with_observed_state_change_count
    );
    DepthProfile {
        route_bound_source_line_count,
        state_dimensions_count,
        data_anvil_action_kind_count,
        input_types_with_observed_state_change_count,
        summary,
    }
}

pub(super) fn source_line_count(path: &Path) -> usize {
    std::fs::read_to_string(path)
        .map(|content| content.lines().count())
        .unwrap_or(0)
}

pub(super) fn route_bound_data_anvil_action_kinds(root: &Path, profile: &str) -> Vec<String> {
    route_bound_source_paths(root, profile)
        .iter()
        .filter_map(|path| std::fs::read_to_string(root.join(path)).ok())
        .flat_map(|content| data_anvil_action_kinds_from_source(&content))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(super) fn data_anvil_action_kinds_from_source(source: &str) -> Vec<String> {
    ["data-anvil-action=\"", "data-anvil-action='"]
        .into_iter()
        .flat_map(|needle| {
            let quote = needle.chars().last().unwrap_or('"');
            let mut out = Vec::new();
            let mut rest = source;
            while let Some(index) = rest.find(needle) {
                let after = &rest[index + needle.len()..];
                let Some(end) = after.find(quote) else {
                    break;
                };
                let value = after[..end].trim();
                if !value.is_empty() {
                    out.push(value.to_string());
                }
                rest = &after[end + quote.len_utf8()..];
            }
            out
        })
        .collect()
}

pub(super) fn input_types_with_observed_state_change(
    interaction_evidence_path: &str,
    text_telemetry: &InteractionTextTelemetry,
) -> BTreeSet<String> {
    let mut types = BTreeSet::new();
    if text_telemetry.text_input_state_change == Some(true)
        && let Some(input_type) =
            input_type_from_text_entry_target(&text_telemetry.text_entry_target)
    {
        types.insert(input_type);
    }
    if let Some(value) = read_json_file(interaction_evidence_path)
        && raw_bool_field_deep(&value, "input_state_change") == Some(true)
        && types.is_empty()
    {
        types.insert("control".to_string());
    }
    types
}

pub(super) fn input_type_from_text_entry_target(target: &str) -> Option<String> {
    let input_type = target.split(':').next()?.trim();
    (!input_type.is_empty()).then(|| input_type.to_string())
}

pub(super) fn read_json_file(path: &str) -> Option<Value> {
    if path.trim().is_empty() {
        return None;
    }
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

pub(super) fn emit_depth_profile(path: Option<&Path>, source_event: &str, profile: &DepthProfile) {
    eval_events::emit(
        path,
        json!({
            "event": "depth_profile",
            "source_event": source_event,
            "depth_profile_summary": profile.summary.as_str(),
            "route_bound_source_line_count": profile.route_bound_source_line_count,
            "state_dimensions_count": profile.state_dimensions_count,
            "data_anvil_action_kind_count": profile.data_anvil_action_kind_count,
            "input_types_with_observed_state_change_count": profile.input_types_with_observed_state_change_count,
        }),
    );
}

pub(super) fn is_nextjs_build_verify_command_like(command: &str) -> bool {
    let lower = command.trim().to_ascii_lowercase();
    lower == "npm run build"
        || lower.starts_with("npm run build ")
        || lower == "pnpm build"
        || lower.starts_with("pnpm build ")
        || lower == "yarn build"
        || lower.starts_with("yarn build ")
        || lower == "next build"
        || lower.starts_with("next build ")
}

pub(super) fn profile_invariant_model_repair_prompt(
    plan: &UltraPlan,
    phase: &UltraPhase,
    report: &VerificationReport,
    context: &UltraRunContext,
    expected_paths: &[String],
    config: &Config,
    deterministic_error: Option<&str>,
) -> String {
    let exact_reason = report.primary_reason();
    let expected = render_prompt_bullets(expected_paths);
    let file_excerpts = profile_invariant_offending_file_excerpts(
        &config.workspace_root,
        resolve_profile_runtime(&plan.profile),
        &exact_reason,
    );
    let missing_imports = profile_missing_relative_imports(&config.workspace_root, &plan.profile);
    let import_findings = format_missing_import_findings(&config.workspace_root, &missing_imports);
    let fix_target_guidance = repair_targeting::fix_profile_invariant_target_guidance(
        &config.workspace_root,
        &plan.profile,
        &plan.intent,
        &missing_imports,
    );
    let import_scan_section = if import_findings.is_empty() {
        "Current missing relative imports:\n- none".to_string()
    } else {
        format!(
            "Current missing relative imports:\n{}",
            render_prompt_bullets(&import_findings)
        )
    };
    let deterministic_note = deterministic_error
        .map(|err| format!("\nDeterministic repair error:\n{err}\n"))
        .unwrap_or_default();
    format!(
        "Repair this intermediate profile invariant failure in one bounded turn.\n\n\
Original ultra goal:\n{goal}\n\n\
Profile: {profile}\nIntent: {intent}\nPhase id: {phase_id}\nPhase task:\n{phase_task}\n\n\
Exact invariant reason:\n\"{exact_reason}\"\n{deterministic_note}\n\
{fix_target_guidance}\
Offending file contents:\n{file_excerpts}\n\n\
{import_scan_section}\n\n\
Expected profile artifacts:\n{expected}\n\n\
{prior_context}\n\n\
Bounded repair rules:\n\
- Repair only the quoted invariant reason without weakening scripts, dependencies, tests, or profile checks.\n\
- Prefer the smallest edit to the offending file shown above.\n\
- For Tailwind, package.json must include tailwindcss/postcss/autoprefixer and postcss.config plugins must include BOTH tailwindcss and autoprefixer.\n\
- Stop after one repair pass; do not start a new planning cycle.",
        goal = plan.goal,
        profile = plan.profile,
        intent = plan.intent,
        phase_id = phase.id,
        phase_task = phase.prompt,
        exact_reason = exact_reason,
        deterministic_note = deterministic_note,
        fix_target_guidance = fix_target_guidance,
        file_excerpts = file_excerpts,
        import_scan_section = import_scan_section,
        expected = expected,
        prior_context = context.render_prompt_section(),
    )
}

pub(super) fn plan_adherence_report(plan: &UltraPlan, root: &Path) -> PlanAdherenceReport {
    let tokens = ultra_plan_requested_feature_tokens(plan);
    if tokens.is_empty() {
        return PlanAdherenceReport::default();
    }
    let corpus = comment_stripped_source_corpus(root);
    let corpus_lower = corpus.to_ascii_lowercase();
    let mut report = PlanAdherenceReport::default();
    for token in tokens {
        let present = if token.is_ascii() {
            corpus_lower.contains(&token)
        } else {
            corpus.contains(&token)
        };
        if present {
            report.present.push(token);
        } else {
            report.missing.push(token);
        }
    }
    report
}

pub(super) fn ultra_plan_requested_feature_tokens(plan: &UltraPlan) -> Vec<String> {
    let mut tokens = BTreeSet::new();
    for phase in &plan.phases {
        collect_plan_feature_tokens(&phase.prompt, &mut tokens);
    }
    tokens.into_iter().collect()
}

pub(super) fn collect_plan_feature_tokens(text: &str, tokens: &mut BTreeSet<String>) {
    let mut ascii = String::new();
    let mut katakana = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            flush_katakana_token(&mut katakana, tokens);
            ascii.push(ch.to_ascii_lowercase());
        } else if is_katakana(ch) {
            flush_ascii_token(&mut ascii, tokens);
            katakana.push(ch);
        } else {
            flush_ascii_token(&mut ascii, tokens);
            flush_katakana_token(&mut katakana, tokens);
        }
    }
    flush_ascii_token(&mut ascii, tokens);
    flush_katakana_token(&mut katakana, tokens);
}

pub(super) fn flush_ascii_token(token: &mut String, tokens: &mut BTreeSet<String>) {
    if token.len() >= 3
        && !token.chars().all(|ch| ch.is_ascii_digit())
        && !plan_adherence_stopword(token)
    {
        tokens.insert(token.clone());
    }
    token.clear();
}

pub(super) fn flush_katakana_token(token: &mut String, tokens: &mut BTreeSet<String>) {
    if token.chars().count() >= 2 {
        tokens.insert(token.clone());
    }
    token.clear();
}

pub(super) fn is_katakana(ch: char) -> bool {
    matches!(ch, '\u{30A0}'..='\u{30FF}' | '\u{31F0}'..='\u{31FF}')
}

pub(super) fn plan_adherence_stopword(token: &str) -> bool {
    signals::plan_adherence_stopword(token)
}

pub(super) fn ultra_plan_phase_signal_text(plan: &UltraPlan) -> String {
    plan.phases
        .iter()
        .flat_map(|phase| [phase.id.as_str(), phase.prompt.as_str()])
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn ultra_plan_signal_text(plan: &UltraPlan) -> String {
    let phase_text = ultra_plan_phase_signal_text(plan);
    if phase_text.is_empty() {
        plan.goal.clone()
    } else {
        format!("{}\n{}", plan.goal, phase_text)
    }
}
#[allow(clippy::too_many_arguments)]
pub(super) fn emit_ultra_phase_event(
    config: &Config,
    event: &str,
    plan: &UltraPlan,
    phase: &UltraPhase,
    index: usize,
    stage: &str,
    ok: Option<bool>,
    reason: Option<&str>,
    step_count: Option<usize>,
) {
    eval_events::typed::emit(
        config.eval_events_path.as_deref(),
        &eval_events::typed::UltraPhaseEvent::new(
            event,
            &phase.id,
            index,
            plan.phases.len(),
            stage,
            ok,
            reason,
            step_count,
        ),
    );
}

pub(super) fn emit_phase_verification_event(
    config: &Config,
    plan: &UltraPlan,
    phase: &UltraPhase,
    index: usize,
    mode: PhaseVerificationMode,
    ok: bool,
    reason: Option<&str>,
) {
    let mode = match mode {
        PhaseVerificationMode::IntermediateInvariant => "intermediate_invariant",
        PhaseVerificationMode::FinalAcceptance => "final_acceptance",
    };
    eval_events::typed::emit(
        config.eval_events_path.as_deref(),
        &eval_events::typed::PhaseVerificationEvent::new(
            &phase.id,
            index,
            plan.phases.len(),
            mode,
            ok,
            reason,
        ),
    );
}

pub(super) struct UltraPhaseRecoveryRequest<'a> {
    pub(super) failure_kind: &'a str,
    pub(super) reason: &'a str,
    pub(super) missing_paths: &'a [String],
    pub(super) missing_signals: &'a [String],
    pub(super) repair_targets: &'a [String],
    pub(super) verify_commands: &'a [String],
}

pub(super) fn save_ultra_phase_recovery_handoff(
    config: &Config,
    plan: &UltraPlan,
    phase: &UltraPhase,
    request: UltraPhaseRecoveryRequest<'_>,
) -> Option<eval_events::StopReasonParts> {
    save_ultra_phase_recovery_handoff_with_evidence(config, plan, phase, request, &[])
}

pub(super) fn save_ultra_phase_recovery_handoff_with_evidence(
    config: &Config,
    plan: &UltraPlan,
    phase: &UltraPhase,
    request: UltraPhaseRecoveryRequest<'_>,
    failure_evidence: &[String],
) -> Option<eval_events::StopReasonParts> {
    let handoff = RecoveryHandoff {
        profile: plan.profile.clone(),
        original_goal: plan.goal.clone(),
        failed_phase: Some(phase.id.clone()),
        failed_step: None,
        failure_kind: request.failure_kind.to_string(),
        failure_evidence: if failure_evidence.is_empty() {
            vec![request.reason.to_string()]
        } else {
            failure_evidence.to_vec()
        },
        missing_paths: request.missing_paths.to_vec(),
        missing_capabilities: request.missing_signals.to_vec(),
        verify_commands: request.verify_commands.to_vec(),
        changed_paths: Vec::new(),
        repair_targets: request.repair_targets.to_vec(),
    };
    let failure_kind = request.failure_kind;
    let reason = request.reason;
    let scope = format!("phase-{}", recovery_scope_token(&phase.id));
    let path = match save_ultra_recovery_prompt(&config.workspace_root, &scope, &handoff) {
        Ok(path) => path,
        Err(err) => {
            eval_events::emit(
                config.eval_events_path.as_deref(),
                json!({
                    "event": "recovery_prompt_save_failed",
                    "recovery_handoff_kind": failure_kind,
                    "phase_id": phase.id,
                    "reason": eval_events::body_snippet(&err.to_string()),
                }),
            );
            return Some(eval_events::StopReasonParts::free_text(format!(
                "recovery prompt save failed: {err}"
            )));
        }
    };
    let recovery_plan = match save_recovery_ultra_plan(&config.workspace_root, &scope, &handoff) {
        Ok(path) => Some(path),
        Err(err) => {
            let prompt_path = handoff_path(&path);
            eval_events::emit(
                config.eval_events_path.as_deref(),
                json!({
                    "event": "recovery_ultra_plan_save_failed",
                    "recovery_handoff_kind": failure_kind,
                    "phase_id": phase.id,
                    "recovery_prompt_path": prompt_path,
                    "reason": eval_events::body_snippet(&err.to_string()),
                    "recovery_yaml_missing": true,
                }),
            );
            None
        }
    };
    let validation = validate_recovery_artifacts(&path, recovery_plan.as_deref());
    let raw_prompt_command = suggested_ultra_recovery_command(&path, &plan.profile);
    let prompt_command = if validation.prompt_command_available() {
        raw_prompt_command
    } else {
        String::new()
    };
    let recovery_plan_command = recovery_plan
        .as_ref()
        .filter(|_| validation.yaml_command_available())
        .map(|path| suggested_recovery_ultra_plan_command(path));
    let prompt_path = handoff_path(&path);
    let recovery_plan_path = optional_handoff_path(recovery_plan.as_ref());
    let (completed_phases, pending_phases) = ultra_phase_status(plan, phase);
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "recovery_prompt_saved",
            "recovery_handoff_kind": failure_kind,
            "phase_id": phase.id,
            "recovery_prompt_path": &prompt_path,
            "recovery_ultra_plan_path": &recovery_plan_path,
            "recovery_yaml_missing": recovery_plan.is_none(),
            "recovery_prompt_exists": validation.prompt_exists,
            "recovery_prompt_parse_ok": validation.prompt_parse_ok,
            "recovery_prompt_parse_error": validation.prompt_parse_error.as_deref().unwrap_or_default(),
            "recovery_yaml_exists": validation.yaml_exists,
            "recovery_yaml_parse_ok": validation.yaml_parse_ok,
            "recovery_yaml_parse_error": validation.yaml_parse_error.as_deref().unwrap_or_default(),
            "recovery_command_targets_valid": validation.command_targets_valid(),
            "suggested_recovery_command": prompt_command.clone(),
            "suggested_recovery_yaml_command": recovery_plan_command.clone().unwrap_or_default(),
            "recovery_profile": plan.profile,
            "local_repair_exhausted": true,
            "status": "incomplete",
        }),
    );
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "ultra_partial_artifact_summary",
            "status": "incomplete",
            "completed_phase_ids": completed_phases.clone(),
            "failed_phase_id": phase.id,
            "pending_phase_ids": pending_phases.clone(),
            "failure_kind": failure_kind,
            "recovery_prompt_path": &prompt_path,
            "recovery_ultra_plan_path": &recovery_plan_path,
            "recovery_yaml_missing": recovery_plan.is_none(),
            "recovery_prompt_exists": validation.prompt_exists,
            "recovery_prompt_parse_ok": validation.prompt_parse_ok,
            "recovery_yaml_exists": validation.yaml_exists,
            "recovery_yaml_parse_ok": validation.yaml_parse_ok,
            "recovery_command_targets_valid": validation.command_targets_valid(),
            "suggested_recovery_command": prompt_command.clone(),
            "suggested_recovery_yaml_command": recovery_plan_command.clone().unwrap_or_default(),
        }),
    );
    let recovery_yaml_summary = recovery_plan
        .as_ref()
        .map(|path| {
            let display = handoff_path(path);
            if validation.yaml_parse_ok {
                format!("Recovery UltraPlan YAML saved: {display}")
            } else {
                format!(
                    "Recovery UltraPlan YAML invalid: {} ({})",
                    display,
                    validation
                        .yaml_parse_error
                        .as_deref()
                        .unwrap_or("recovery_yaml_invalid")
                )
            }
        })
        .unwrap_or_else(|| {
            "Recovery UltraPlan YAML missing: failed to save valid recovery plan".to_string()
        });
    let prompt_command_summary = if validation.prompt_command_available() {
        format!("Suggested command: {prompt_command}")
    } else {
        format!(
            "Suggested command: unavailable because recovery prompt validation failed ({})",
            validation
                .prompt_parse_error
                .as_deref()
                .unwrap_or("recovery_prompt_invalid")
        )
    };
    let recovery_yaml_command_summary = recovery_plan_command
        .as_ref()
        .map(|command| format!("Suggested YAML command: {command}"))
        .unwrap_or_else(|| {
            "Suggested YAML command: unavailable because recovery YAML is missing".to_string()
        });
    let artifact_check_summary = recovery_artifact_check_summary(&validation);
    eval_events::write_run_summary(
        config.eval_events_path.as_deref(),
        &render_ultra_partial_run_summary(UltraPartialRunSummary {
            completed_phases: &completed_phases,
            failed_phase: &phase.id,
            pending_phases: &pending_phases,
            failure_kind,
            reason,
            recovery_prompt_path: &prompt_path,
            recovery_yaml_summary: &recovery_yaml_summary,
            prompt_command_summary: &prompt_command_summary,
            recovery_yaml_command_summary: &recovery_yaml_command_summary,
            recovery_artifact_check: &artifact_check_summary,
            browser_evidence_missing_note: browser_evidence_missing_before_final_acceptance_note(
                config,
            ),
        }),
    );
    let prompt_message = if validation.prompt_command_available() {
        format!("suggested command: {prompt_command}")
    } else {
        "suggested command unavailable because recovery prompt validation failed".to_string()
    };
    let recovery_yaml_command_message = recovery_plan_command
        .as_ref()
        .map(|command| format!("suggested YAML command: {command}"))
        .unwrap_or_else(|| {
            "suggested YAML command unavailable because recovery YAML is missing".to_string()
        });
    Some(eval_events::StopReasonParts {
        free_text: format!("incomplete; {artifact_check_summary}"),
        paths: vec![
            format!("repair prompt saved: {prompt_path}"),
            recovery_yaml_summary,
        ],
        commands: vec![prompt_message, recovery_yaml_command_message],
    })
}

pub(super) fn render_failure_stop_reason(
    free_text: impl Into<String>,
    handoff: Option<eval_events::StopReasonParts>,
) -> String {
    let free_text = free_text.into();
    let mut parts = handoff.unwrap_or_default();
    parts.free_text = if parts.free_text.trim().is_empty() {
        free_text
    } else {
        format!("{free_text}; {}", parts.free_text)
    };
    eval_events::render_stop_reason(&parts)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn save_release_recovery_handoff(
    config: &Config,
    profile: &str,
    original_goal: &str,
    scope: &str,
    acceptance_layer: &str,
    failure_kind: &str,
    failure_evidence: Vec<String>,
    missing_paths: Vec<String>,
    missing_capabilities: Vec<String>,
    repair_targets: Vec<String>,
    verify_commands: Vec<String>,
) -> Option<ReleaseRecoveryHandoffSummary> {
    let handoff = RecoveryHandoff {
        profile: profile.to_string(),
        original_goal: original_goal.to_string(),
        failed_phase: Some(acceptance_layer.to_string()),
        failed_step: None,
        failure_kind: failure_kind.to_string(),
        failure_evidence,
        missing_paths,
        missing_capabilities,
        verify_commands,
        changed_paths: Vec::new(),
        repair_targets,
    };
    let path = match save_ultra_recovery_prompt(&config.workspace_root, scope, &handoff) {
        Ok(path) => path,
        Err(err) => {
            eval_events::emit(
                config.eval_events_path.as_deref(),
                json!({
                    "event": "recovery_prompt_save_failed",
                    "recovery_handoff_kind": failure_kind,
                    "acceptance_layer": acceptance_layer,
                    "reason": eval_events::body_snippet(&err.to_string()),
                    "status": "incomplete",
                }),
            );
            return None;
        }
    };
    let recovery_plan = match save_recovery_ultra_plan(&config.workspace_root, scope, &handoff) {
        Ok(path) => Some(path),
        Err(err) => {
            let prompt_path = handoff_path(&path);
            eval_events::emit(
                config.eval_events_path.as_deref(),
                json!({
                    "event": "recovery_ultra_plan_save_failed",
                    "recovery_handoff_kind": failure_kind,
                    "acceptance_layer": acceptance_layer,
                    "recovery_prompt_path": prompt_path,
                    "reason": eval_events::body_snippet(&err.to_string()),
                    "recovery_yaml_missing": true,
                    "status": "incomplete",
                }),
            );
            None
        }
    };
    let validation = validate_recovery_artifacts(&path, recovery_plan.as_deref());
    let raw_prompt_command = suggested_ultra_recovery_command(&path, profile);
    let prompt_command = if validation.prompt_command_available() {
        raw_prompt_command
    } else {
        String::new()
    };
    let recovery_plan_command = recovery_plan
        .as_ref()
        .filter(|_| validation.yaml_command_available())
        .map(|path| suggested_recovery_ultra_plan_command(path));
    let prompt_path = handoff_path(&path);
    let recovery_plan_path = optional_handoff_path(recovery_plan.as_ref());
    let summary = ReleaseRecoveryHandoffSummary {
        recovery_handoff_kind: failure_kind.to_string(),
        acceptance_layer: acceptance_layer.to_string(),
        recovery_prompt_path: prompt_path,
        recovery_ultra_plan_path: recovery_plan_path,
        suggested_recovery_command: prompt_command.clone(),
        suggested_recovery_yaml_command: recovery_plan_command.clone().unwrap_or_default(),
    };
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "recovery_prompt_saved",
            "recovery_handoff_kind": failure_kind,
            "acceptance_layer": acceptance_layer,
            "recovery_scope": scope,
            "recovery_prompt_path": &summary.recovery_prompt_path,
            "recovery_ultra_plan_path": &summary.recovery_ultra_plan_path,
            "recovery_yaml_missing": recovery_plan.is_none(),
            "recovery_prompt_exists": validation.prompt_exists,
            "recovery_prompt_parse_ok": validation.prompt_parse_ok,
            "recovery_prompt_parse_error": validation.prompt_parse_error.as_deref().unwrap_or_default(),
            "recovery_yaml_exists": validation.yaml_exists,
            "recovery_yaml_parse_ok": validation.yaml_parse_ok,
            "recovery_yaml_parse_error": validation.yaml_parse_error.as_deref().unwrap_or_default(),
            "recovery_command_targets_valid": validation.command_targets_valid(),
            "suggested_recovery_command": &summary.suggested_recovery_command,
            "suggested_recovery_yaml_command": &summary.suggested_recovery_yaml_command,
            "recovery_profile": profile,
            "release_acceptance_handoff": true,
            "handoff_saved_not_success": true,
            "status": "incomplete",
        }),
    );
    eval_events::append_run_summary(
        config.eval_events_path.as_deref(),
        &render_release_recovery_handoff_summary(&summary, &validation),
    );
    Some(summary)
}

pub(super) fn render_release_recovery_handoff_summary(
    summary: &ReleaseRecoveryHandoffSummary,
    validation: &RecoveryArtifactValidation,
) -> String {
    format!(
        "Recovery next action:\n\
- Status: incomplete_release_acceptance\n\
- Failed acceptance layer: {}\n\
- Recovery handoff kind: {}\n\
- Recovery prompt saved: {}\n\
- Recovery UltraPlan YAML saved: {}\n\
- Suggested command: {}\n\
- Suggested YAML command: {}\n\
- Recovery artifact check: {}",
        summary.acceptance_layer,
        summary.recovery_handoff_kind,
        missing_if_empty(&summary.recovery_prompt_path),
        missing_if_empty(&summary.recovery_ultra_plan_path),
        missing_if_empty(&summary.suggested_recovery_command),
        missing_if_empty(&summary.suggested_recovery_yaml_command),
        recovery_artifact_check_summary(validation),
    )
}

pub(super) fn missing_if_empty(value: &str) -> &str {
    if value.is_empty() { "missing" } else { value }
}

pub(super) struct UltraPartialRunSummary<'a> {
    pub(super) completed_phases: &'a [String],
    pub(super) failed_phase: &'a str,
    pub(super) pending_phases: &'a [String],
    pub(super) failure_kind: &'a str,
    pub(super) reason: &'a str,
    pub(super) recovery_prompt_path: &'a str,
    pub(super) recovery_yaml_summary: &'a str,
    pub(super) prompt_command_summary: &'a str,
    pub(super) recovery_yaml_command_summary: &'a str,
    pub(super) recovery_artifact_check: &'a str,
    pub(super) browser_evidence_missing_note: Option<&'a str>,
}

pub(super) const BROWSER_EVIDENCE_MISSING_BEFORE_FINAL_ACCEPTANCE: &str = "Browser evidence missing: run failed before final acceptance (interaction probe installed but not exercised).";

pub(super) fn render_ultra_partial_run_summary(summary: UltraPartialRunSummary<'_>) -> String {
    let browser_evidence_missing = summary
        .browser_evidence_missing_note
        .map(|note| format!("\n\n{note}"))
        .unwrap_or_default();
    format!(
        "Status: incomplete\n\n\
Completed phases:\n{}\n\n\
Failed phase:\n- {} ({})\n\n\
Pending phases:\n{}\n\n\
Recovery next action:\n- {}\n- Recovery prompt saved: {}\n- {}\n- {}\n- {}\n\n\
Failure:\n{}{}",
        render_summary_bullets(summary.completed_phases),
        summary.failed_phase,
        summary.failure_kind,
        render_summary_bullets(summary.pending_phases),
        summary.recovery_yaml_summary,
        summary.recovery_prompt_path,
        summary.prompt_command_summary,
        summary.recovery_yaml_command_summary,
        summary.recovery_artifact_check,
        eval_events::render_stop_reason_text(summary.reason),
        browser_evidence_missing,
    )
}

pub(super) fn browser_evidence_missing_before_final_acceptance_note(
    config: &Config,
) -> Option<&'static str> {
    if interaction_probe_performed_for_run(config) {
        return None;
    }
    match interaction_probe::playwright_availability(&config.workspace_root) {
        interaction_probe::ProbeAvailability::Available(_) => {
            Some(BROWSER_EVIDENCE_MISSING_BEFORE_FINAL_ACCEPTANCE)
        }
        interaction_probe::ProbeAvailability::Unavailable(_) => None,
    }
}

pub(super) fn render_summary_bullets(items: &[String]) -> String {
    if items.is_empty() {
        "- none".to_string()
    } else {
        items
            .iter()
            .map(|item| format!("- {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub(super) fn ultra_phase_status(
    plan: &UltraPlan,
    failed_phase: &UltraPhase,
) -> (Vec<String>, Vec<String>) {
    let failed_index = plan
        .phases
        .iter()
        .position(|phase| phase.id == failed_phase.id)
        .unwrap_or(0);
    let completed = plan
        .phases
        .iter()
        .take(failed_index)
        .map(|phase| phase.id.clone())
        .collect();
    let pending = plan
        .phases
        .iter()
        .skip(failed_index + 1)
        .map(|phase| phase.id.clone())
        .collect();
    (completed, pending)
}

pub(super) fn recovery_scope_token(value: &str) -> String {
    let token = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    if token.trim_matches('-').is_empty() {
        "phase".to_string()
    } else {
        token
    }
}
