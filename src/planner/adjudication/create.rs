use super::*;
use crate::planner::adjudication::{
    GateObservation, append_gate_observation, dedup_strings, disconnected_gate_observations_reason,
    execution_status_from_observed, profile_behavior_failure_reasons,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReleaseGateSummary {
    pub(super) status: String,
    pub(super) reasons: Vec<String>,
    pub(super) browser_readiness_status: String,
    pub(super) browser_readiness_evidence_path: String,
    pub(super) interaction_evidence_status: String,
    pub(super) interaction_evidence_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AcceptanceGateTelemetry {
    pub(super) browser_readiness_applicable: bool,
    pub(super) browser_readiness_execution_status: String,
    pub(super) interaction_evidence_applicable: bool,
    pub(super) interaction_evidence_execution_status: String,
}

pub(super) fn gate_execution_status(status: &str) -> String {
    execution_status_from_observed(status, &["interaction_verified_heuristic_only"])
}

pub(super) fn gate_observations<'a>(
    telemetry: &'a AcceptanceGateTelemetry,
    release_gate: &'a ReleaseGateSummary,
) -> [GateObservation<'a>; 2] {
    [
        GateObservation {
            reason_key: "browser_readiness",
            status_key: "browser_readiness_status",
            applicable: telemetry.browser_readiness_applicable,
            observed_status: &release_gate.browser_readiness_status,
            execution_status: &telemetry.browser_readiness_execution_status,
        },
        GateObservation {
            reason_key: "interaction_evidence",
            status_key: "interaction_evidence_status",
            applicable: telemetry.interaction_evidence_applicable,
            observed_status: &release_gate.interaction_evidence_status,
            execution_status: &telemetry.interaction_evidence_execution_status,
        },
    ]
}

pub(super) fn acceptance_gates_disconnected_reason(
    telemetry: &AcceptanceGateTelemetry,
    release_gate: &ReleaseGateSummary,
) -> Option<String> {
    disconnected_gate_observations_reason(&gate_observations(telemetry, release_gate))
}

pub(super) fn mark_release_gate_profile_behavior_failed(
    release_gate: &mut ReleaseGateSummary,
    profile_behavior_probe: &ProfileBehaviorProbeReport,
) {
    release_gate.status = "failed".to_string();
    release_gate.reasons = profile_behavior_failure_reasons(
        &release_gate.reasons,
        &profile_behavior_probe.reasons,
        profile_behavior_probe.evidence_path.as_deref(),
    );
}

pub(super) fn release_gate_final_acceptance_status(
    release_gate: &ReleaseGateSummary,
) -> &'static str {
    crate::planner::adjudication::final_acceptance_status_from_release_gate(&release_gate.status)
}

pub(super) fn release_quality_completion_status(
    release_gate: &ReleaseGateSummary,
    final_acceptance_status: &str,
) -> &'static str {
    crate::planner::adjudication::release_quality_from_gate_status(
        &release_gate.status,
        final_acceptance_status,
    )
}

pub(super) fn release_gate_next_action(
    release_gate: &ReleaseGateSummary,
    final_acceptance_status: &str,
) -> &'static str {
    crate::planner::adjudication::next_action_from_gate_status(
        &release_gate.status,
        final_acceptance_status,
    )
}

pub(super) fn release_recovery_needed(
    release_gate: &ReleaseGateSummary,
    final_acceptance_status: &str,
) -> bool {
    crate::planner::adjudication::recovery_needed_for_gate_status(
        &release_gate.status,
        final_acceptance_status,
    )
}

pub(super) fn release_recovery_acceptance_layer(
    release_gate: &ReleaseGateSummary,
    final_acceptance_status: &str,
) -> &'static str {
    crate::planner::adjudication::recovery_acceptance_layer_for_gate_status(
        &release_gate.status,
        final_acceptance_status,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn earned_assurance_from_release_gate(
    profile: &str,
    base_level: &str,
    base_reason: &str,
    contract_bound: bool,
    final_acceptance_status: &str,
    release_gate: &ReleaseGateSummary,
    gate_telemetry: &AcceptanceGateTelemetry,
) -> (String, String) {
    crate::planner::adjudication::earned_assurance_from_base(
        profile,
        base_level,
        base_reason,
        contract_bound,
        final_acceptance_status,
        &release_gate.status,
        &release_gate.reasons,
        &gate_observations(gate_telemetry, release_gate),
    )
}

fn maybe_run_ultra_final_browser_probe(
    config: &Config,
    plan: &UltraPlan,
    effective_profile: &str,
    required_capabilities: &[String],
    interaction_options: BrowserInteractionProbeOptions,
) -> Option<BrowserReadinessObservation> {
    let phase_text = ultra_plan_phase_signal_text(plan);
    let signal_text = ultra_plan_signal_text(plan);
    let runtime = resolve_profile_runtime(effective_profile);
    if !ultra_browser_probe_required(runtime, &signal_text, required_capabilities)
        || !ultra_browser_probe_runtime_enabled(config)
    {
        return None;
    }
    let requested_port = effective_requested_port(runtime, &plan.goal, Some(&phase_text));
    let observation = probe_browser_readiness_with_offline_and_interaction_options(
        &config.workspace_root,
        effective_profile,
        requested_port.as_ref().map(|requested| requested.port),
        Duration::from_secs(30),
        config.offline,
        interaction_options,
    );
    emit_browser_probe_event(
        config,
        &observation,
        requested_port
            .as_ref()
            .map(|requested| requested.telemetry.clone()),
    );
    Some(observation)
}
fn run_ultra_final_browser_checks_before_arbitration(
    config: &Config,
    plan: &UltraPlan,
    effective_profile: &str,
    required_capabilities: &[String],
    required_evidence: &[String],
) -> Option<BrowserReadinessObservation> {
    let phase_text = ultra_plan_phase_signal_text(plan);
    let signal_text = ultra_plan_signal_text(plan);
    let runtime = resolve_profile_runtime(effective_profile);
    if !ultra_browser_probe_required(runtime, &signal_text, required_capabilities) {
        return None;
    }
    let requested_port = effective_requested_port(runtime, &plan.goal, Some(&phase_text));
    let interaction_options =
        browser_interaction_probe_options(required_capabilities, required_evidence);
    if ultra_browser_probe_runtime_enabled(config) {
        crate::minimal_loop::probe_preflight::emit_interaction_probe_preflight(
            config.eval_events_path.as_deref(),
            &config.workspace_root,
            "ultra_final_acceptance",
        );
    }
    let browser_probe = maybe_run_ultra_final_browser_probe(
        config,
        plan,
        effective_profile,
        required_capabilities,
        interaction_options,
    );
    let _ = browser_release_gate_with_options(
        config,
        requires_canvas_surface(&signal_text, required_capabilities),
        interaction_options,
        requested_port.as_ref().map(|requested| requested.port),
    );
    browser_probe
}

pub(super) fn browser_interaction_probe_options(
    required_capabilities: &[String],
    required_evidence: &[String],
) -> BrowserInteractionProbeOptions {
    let text_entry_required = required_capabilities
        .iter()
        .chain(required_evidence.iter())
        .any(|value| {
            let lower = value.to_ascii_lowercase();
            lower.contains("text")
                || lower.contains("editor")
                || lower.contains("note")
                || lower.contains("todo")
                || lower.contains("content")
                || lower.contains("preview")
                || lower.contains("render")
                || lower == "input_output_contract"
                || lower == "requested_content"
                || lower == "requested_content_evidence"
                || lower == "live_preview"
                || lower == "live_preview_evidence"
        });
    let token_echo_required = required_capabilities
        .iter()
        .chain(required_evidence.iter())
        .any(|value| {
            let lower = value.to_ascii_lowercase();
            lower.contains("preview")
                || lower.contains("render")
                || lower.contains("content")
                || lower == "requested_content"
                || lower == "requested_content_evidence"
                || lower == "live_preview"
                || lower == "live_preview_evidence"
        });
    BrowserInteractionProbeOptions {
        persistence_required: required_capabilities
            .iter()
            .any(|capability| capability == "persistence")
            || required_evidence
                .iter()
                .any(|evidence| evidence == "persistence_evidence"),
        text_entry_required,
        token_echo_required,
    }
}

pub(super) fn report_has_production_build_failure(report: &VerificationReport) -> bool {
    !report.compile_errors.is_empty()
        || report.dependency_missing.iter().any(|reason| {
            let lower = reason.to_ascii_lowercase();
            lower.contains("next.js build")
                || lower.contains("next build")
                || lower.contains("npm run build")
                || lower.contains("dependency_setup_missing")
        })
        || report.command_failures.iter().any(|failure| {
            let lower = format!("{} {}", failure.command, failure.reason).to_ascii_lowercase();
            lower.contains("npm run build")
                || lower.contains("next build")
                || lower.contains("build_verify_failed")
                || lower.contains("implementation_compile_error")
        })
}

fn ultra_browser_probe_required(
    runtime: &dyn ProfileRuntime,
    signal_text: &str,
    required_capabilities: &[String],
) -> bool {
    runtime.browser_release_gate_profile()
        && (required_capabilities.iter().any(|capability| {
            matches!(
                capability.as_str(),
                "stateful_interaction"
                    | "player_control"
                    | "user_input_or_action"
                    | "visible_state_change"
                    | "persistence"
                    | "adversary_or_challenge"
                    | "progression_or_score"
                    | "failure_or_collision_rule"
            )
        }) || signals::contains_browser_probe_token(signal_text))
}

fn ultra_browser_probe_runtime_enabled(config: &Config) -> bool {
    #[cfg(test)]
    {
        config
            .workspace_root
            .join(".anvil")
            .join("enable-browser-probe-tests")
            .is_file()
    }
    #[cfg(not(test))]
    {
        let _ = config;
        true
    }
}

pub(super) fn external_contract_ok_after_runtime_arbitration(
    report: Option<&VerificationReport>,
    acceptance: Option<&RuntimeAcceptanceReport>,
) -> bool {
    report.is_none_or(|report| {
        report.is_pass()
            || external_contract_report_covered_by_runtime_arbitration(report, acceptance)
    })
}

fn external_contract_report_covered_by_runtime_arbitration(
    report: &VerificationReport,
    acceptance: Option<&RuntimeAcceptanceReport>,
) -> bool {
    let Some(acceptance) = acceptance.filter(|acceptance| acceptance.passed) else {
        return false;
    };
    report.missing_paths.is_empty()
        && report.command_failures.is_empty()
        && report.verifier_command_false_negatives.is_empty()
        && report.dependency_missing.is_empty()
        && report
            .profile_failures
            .iter()
            .all(|failure| external_profile_failure_covered_by_runtime(failure, acceptance))
}

fn external_profile_failure_covered_by_runtime(
    failure: &str,
    acceptance: &RuntimeAcceptanceReport,
) -> bool {
    if let Some(evidence) = failure.strip_prefix("missing_required_evidence:") {
        if evidence.starts_with("required_obligation:") {
            return false;
        }
        return evidence
            .split(',')
            .map(str::trim)
            .filter(|evidence| !evidence.is_empty())
            .all(|evidence| runtime_acceptance_satisfied_evidence(acceptance, evidence));
    }
    if let Some(weak_evidence) = failure.strip_prefix("weak_verification_evidence:") {
        return weak_evidence
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .all(|item| weak_source_evidence_covered_by_runtime(acceptance, item));
    }
    false
}

fn runtime_acceptance_satisfied_evidence(
    acceptance: &RuntimeAcceptanceReport,
    evidence: &str,
) -> bool {
    !acceptance
        .missing_evidence
        .iter()
        .any(|missing| missing == evidence)
        && acceptance
            .evidence_tiers
            .get(evidence)
            .is_some_and(|tier| tier != "absent" && tier != "weak")
}

fn weak_source_evidence_covered_by_runtime(
    acceptance: &RuntimeAcceptanceReport,
    weak_evidence: &str,
) -> bool {
    let Some(rest) = weak_evidence.strip_prefix("weak_source_evidence:") else {
        return false;
    };
    let Some((evidence, _reason)) = rest.split_once(':') else {
        return false;
    };
    runtime_acceptance_satisfied_evidence(acceptance, evidence.trim())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn bind_completion_contract_for_acceptance(
    config: &Config,
    scope: &str,
    profile: &str,
    goal: &str,
    required_paths: &[String],
    required_capabilities: &[String],
    required_evidence: &[String],
    required_obligations: &[String],
) -> anyhow::Result<Option<BoundCompletionContract>> {
    let profile_id = ProfileId::parse(profile);
    let required = ProfileRuntimeRegistry::resolve(&profile_id).requires_completion_contract(
        &profile_id,
        goal,
        required_capabilities,
    );
    if let Some(contract) = CompletionContract::load_for_config(config)? {
        let mut contract = contract;
        contract.merge_evidence_hint_tokens_from_goal(goal);
        let path = explicit_completion_contract_path(config)
            .map(|path| display_path_for_event(&config.workspace_root, &path))
            .unwrap_or_else(|| "<inline-config>".to_string());
        let fs_path = explicit_completion_contract_path(config);
        let bound = BoundCompletionContract {
            contract,
            path,
            fs_path,
            generated: false,
            required,
        };
        emit_completion_contract_bound(config, scope, profile, goal, &bound);
        return Ok(Some(bound));
    }
    if !required {
        return Ok(None);
    }
    let contract = CompletionContract {
        required_paths: required_paths.to_vec(),
        verify_commands: Vec::new(),
        profile: None,
        goal: Some(goal.to_string()),
        required_capabilities: required_capabilities.to_vec(),
        deterministic_oracles: Vec::new(),
        required_evidence: required_evidence.to_vec(),
        evidence_hint_tokens: evidence_hint_tokens_for_goal(goal),
        required_obligations: required_obligations.to_vec(),
        deferred_verify_requirements: Vec::new(),
        verify_repair_cap: 2,
    }
    .validate(&config.workspace_root)?;
    let path = generated_completion_contract_path(config, scope);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(&contract)?;
    std::fs::write(&path, format!("{text}\n"))?;
    let bound = BoundCompletionContract {
        contract,
        path: display_path_for_event(&config.workspace_root, &path),
        fs_path: Some(path),
        generated: true,
        required,
    };
    emit_completion_contract_bound(config, scope, profile, goal, &bound);
    Ok(Some(bound))
}

pub(super) fn ultra_final_acceptance_report_with_deterministic_remedies(
    plan: &UltraPlan,
    config: &Config,
    cycle_index: usize,
    setup_authority: &mut UltraRunSetupAuthorityState,
) -> anyhow::Result<(VerificationReport, Vec<String>)> {
    let mut deterministic_remedies_applied = Vec::new();
    if reconcile_manifest_changed_dependencies_if_needed(config, &plan.profile, setup_authority)?
        .is_some()
    {
        push_unique_label(
            &mut deterministic_remedies_applied,
            "manifest_changed_dependency_reconciliation",
        );
        clear_final_acceptance_browser_probe_evidence(config);
    }
    let mut report = ultra_final_acceptance_report_with_cycle(plan, config, cycle_index)?;
    if acceptance_dependency_deterministic_reconcile_needed(config, &report) {
        setup_authority.grant("declared_dependencies_not_ready");
        if reconcile_run_dependency_setup(
            config,
            &plan.profile,
            DependencyReconciliationTrigger::DeclaredDependenciesNotReady,
            setup_authority,
        )?
        .is_some()
        {
            push_unique_label(
                &mut deterministic_remedies_applied,
                "declared_dependencies_not_ready_install",
            );
            clear_final_acceptance_browser_probe_evidence(config);
            report = ultra_final_acceptance_report_with_cycle(plan, config, cycle_index)?;
        }
    }
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "final_acceptance_deterministic_remedies",
            "cycle_index": cycle_index,
            "lifecycle_stage": "final_acceptance",
            "deterministic_remedies_applied": deterministic_remedies_applied.clone(),
        }),
    );
    Ok((report, deterministic_remedies_applied))
}

pub(super) fn ultra_final_acceptance_report_inner(
    plan: &UltraPlan,
    config: &Config,
    cycle_index: usize,
) -> anyhow::Result<VerificationReport> {
    let effective_profile_id = ProfileId::parse(&plan.profile);
    let effective_profile = effective_profile_id.to_string();
    let runtime = ProfileRuntimeRegistry::resolve(&effective_profile_id);
    let mut required_paths = runtime.expected_scaffold_paths(&config.workspace_root, &plan.goal);
    let mut required_capabilities = runtime.required_capabilities(&plan.goal);
    let mut required_obligations =
        runtime.required_obligations(&effective_profile_id, &plan.goal, &required_capabilities);
    let mut required_evidence = runtime.required_evidence(&plan.goal, &required_capabilities);
    let mut requirements = ContractRequirements {
        capabilities: required_capabilities,
        evidence: required_evidence,
        obligations: required_obligations,
    };
    carry_recorded_promotion_contract_requirements(
        config,
        &effective_profile,
        &plan.goal,
        &mut requirements,
    );
    required_capabilities = requirements.capabilities;
    required_evidence = requirements.evidence;
    required_obligations = requirements.obligations;
    let mut evidence_hint_tokens = evidence_hint_tokens_for_goal(&plan.goal);
    let bound_contract = bind_completion_contract_for_acceptance(
        config,
        "ultra-plan-run",
        &effective_profile,
        &plan.goal,
        &required_paths,
        &required_capabilities,
        &required_evidence,
        &required_obligations,
    )?;
    let mut deferred_commands = Vec::new();
    let mut verify_commands = Vec::new();
    if let Some(contract) = bound_contract.as_ref().map(|bound| &bound.contract) {
        merge_unique_strings(&mut required_paths, &contract.required_paths);
        merge_unique_strings(&mut required_capabilities, &contract.required_capabilities);
        merge_unique_strings(&mut required_evidence, &contract.required_evidence);
        merge_unique_strings(&mut required_obligations, &contract.required_obligations);
        merge_unique_strings(&mut verify_commands, &contract.verify_commands);
        merge_unique_strings(&mut evidence_hint_tokens, &contract.evidence_hint_tokens);
        deferred_commands.extend(
            contract
                .deferred_verify_requirements
                .iter()
                .map(|requirement| requirement.command.clone()),
        );
    }
    merge_unique_strings(
        &mut required_evidence,
        &runtime.required_evidence(&plan.goal, &required_capabilities),
    );
    let missing = missing_final_artifacts(&config.workspace_root, &required_paths);
    let mut acceptance = verify_runtime_acceptance_with_browser_dirs_and_hints(
        &config.workspace_root,
        &required_paths,
        &verify_commands,
        &required_capabilities,
        &required_evidence,
        &required_obligations,
        &deferred_commands,
        &release_evidence_extra_dirs(config),
        &evidence_hint_tokens,
    );
    let profile_invariant_report =
        runtime.verify_phase_invariant(&config.workspace_root, &plan.goal, &ProfileSnapshot::None);
    let profile_invariant_report = hook_snapshot::report_missing_as_profile_failure_with_runtime(
        config,
        runtime,
        &plan.goal,
        profile_invariant_report,
    );
    let profile_report = if !profile_invariant_report.is_pass() {
        profile_invariant_report
    } else {
        runtime.verify_final(&config.workspace_root, &plan.goal)
    };
    let external_report = bound_contract.as_ref().map(|bound| {
        bound
            .contract
            .verify_with_goal(&config.workspace_root, &plan.goal)
    });
    let contract_required = runtime.requires_completion_contract(
        &effective_profile_id,
        &plan.goal,
        &required_capabilities,
    ) || bound_contract.as_ref().is_some_and(|bound| bound.required);
    let external_contract_checked = bound_contract.is_some();
    let contract_binding_missing = contract_required && !external_contract_checked;
    let external_ok = !contract_binding_missing
        && external_contract_ok_after_runtime_arbitration(
            external_report.as_ref(),
            Some(&acceptance),
        );
    let production_build_failed = report_has_production_build_failure(&profile_report)
        || external_report
            .as_ref()
            .is_some_and(report_has_production_build_failure);
    let browser_probe = if production_build_failed {
        None
    } else {
        run_ultra_final_browser_checks_before_arbitration(
            config,
            plan,
            &effective_profile,
            &required_capabilities,
            &required_evidence,
        )
    };
    let evidence_arbitration = final_acceptance_evidence_arbitration(
        config,
        &mut acceptance,
        &required_capabilities,
        &required_evidence,
        &required_obligations,
    );
    let mut release_gate = if production_build_failed {
        production_build_failed_release_gate()
    } else {
        let signal_text = ultra_plan_signal_text(plan);
        final_acceptance_release_gate_with_runtime(
            config,
            runtime,
            &signal_text,
            &required_capabilities,
            Some(&acceptance),
            true,
        )
    };
    crate::planner::interaction_qualification::enforce_release_gate(
        &mut release_gate.status,
        &mut release_gate.reasons,
        &mut release_gate.interaction_evidence_status,
        &release_gate.interaction_evidence_path,
        crate::planner::interaction_qualification::contract_requires_restart(
            &required_capabilities,
            &required_evidence,
        ),
    );
    let signal_text = ultra_plan_signal_text(plan);
    let gate_telemetry = acceptance_gate_telemetry(
        &effective_profile,
        &signal_text,
        &required_capabilities,
        &required_evidence,
        &release_gate,
    );
    if acceptance.passed
        && external_ok
        && let Some(reason) = acceptance_gates_disconnected_reason(&gate_telemetry, &release_gate)
    {
        release_gate.status = "failed".to_string();
        release_gate.reasons = dedup_strings(
            release_gate
                .reasons
                .iter()
                .cloned()
                .chain(std::iter::once(reason))
                .collect(),
        );
    }
    let profile_behavior_probe = run_profile_behavior_probe(
        config,
        &effective_profile,
        &plan.goal,
        &required_capabilities,
        &profile_report,
    );
    let profile_behavior_failed = profile_behavior_probe.status == "failed";
    if profile_behavior_failed {
        mark_release_gate_profile_behavior_failed(&mut release_gate, &profile_behavior_probe);
    } else if matches!(profile_behavior_probe.status, "partial" | "static") {
        release_gate.status = "partial".to_string();
        release_gate.reasons = dedup_strings(
            release_gate
                .reasons
                .iter()
                .cloned()
                .chain(std::iter::once(format!(
                    "profile_behavior_probe_{}",
                    profile_behavior_probe.status
                )))
                .collect(),
        );
    }
    let final_acceptance_status = release_gate_final_acceptance_status(&release_gate);
    let runtime_acceptance_status = match profile_behavior_probe.status {
        "failed" => "failed",
        "partial" => "partial",
        "static" => "static",
        _ => runtime_acceptance_status(acceptance.passed, Some(&acceptance)),
    };
    let (mut assurance_level, mut assurance_reason) = earned_assurance_for_completion(
        &effective_profile,
        &required_capabilities,
        !contract_required || (external_contract_checked && !contract_binding_missing),
        final_acceptance_status,
        &release_gate,
        &gate_telemetry,
        Some(&profile_behavior_probe),
    );
    crate::planner::profile_admission::cap_assurance(
        &effective_profile,
        &mut assurance_level,
        &mut assurance_reason,
    );
    let release_quality_completion =
        release_quality_completion_status(&release_gate, final_acceptance_status);
    let next_action = release_gate_next_action(&release_gate, final_acceptance_status);
    let state_dimensions_changed =
        interaction_state_dimensions_changed_from_path(&release_gate.interaction_evidence_path);
    let action_hooks = interaction_action_hooks_from_path(&release_gate.interaction_evidence_path);
    let surface_fit = interaction_surface_fit_from_path(&release_gate.interaction_evidence_path);
    let text_telemetry =
        interaction_text_telemetry_from_path(&release_gate.interaction_evidence_path);
    let depth_profile = depth_profile(
        &config.workspace_root,
        &effective_profile,
        &state_dimensions_changed,
        &action_hooks,
        &release_gate.interaction_evidence_path,
        &text_telemetry,
    );
    let plan_adherence = plan_adherence_report(plan, &config.workspace_root);
    let phase_signal_text = ultra_plan_phase_signal_text(plan);
    let requested_port = effective_requested_port(
        resolve_profile_runtime(&effective_profile),
        &plan.goal,
        Some(&phase_signal_text),
    );
    let mut compile_errors = profile_report.compile_errors.clone();
    if let Some(report) = external_report.as_ref() {
        for error in &report.compile_errors {
            if !compile_errors.contains(error) {
                compile_errors.push(error.clone());
            }
        }
    }
    let primary_reason = if !missing.is_empty() {
        format!("missing final artifacts: {}", missing.join(", "))
    } else if contract_binding_missing {
        "completion contract binding required but missing".to_string()
    } else if !profile_report.is_pass() {
        profile_report.primary_reason()
    } else if let Some(report) = external_report.as_ref().filter(|report| {
        !external_contract_ok_after_runtime_arbitration(Some(*report), Some(&acceptance))
    }) {
        report.primary_reason()
    } else if profile_behavior_failed {
        profile_behavior_probe.reasons.join("; ")
    } else if !acceptance.passed {
        acceptance.primary_reason.clone()
    } else if matches!(release_gate.status.as_str(), "partial" | "failed") {
        release_gate.reasons.join("; ")
    } else {
        acceptance.primary_reason.clone()
    };
    let recovery_handoff = if acceptance.passed
        && external_ok
        && (matches!(release_gate.status.as_str(), "partial" | "failed")
            || final_acceptance_status == "partial")
    {
        let acceptance_layer =
            release_recovery_acceptance_layer(&release_gate, final_acceptance_status);
        let failure_kind =
            release_recovery_failure_kind(&release_gate, final_acceptance_status, &primary_reason);
        let scope = format!("release-{}", recovery_scope_token(acceptance_layer));
        save_release_recovery_handoff(
            config,
            &effective_profile,
            &plan.goal,
            &scope,
            acceptance_layer,
            &failure_kind,
            release_recovery_failure_evidence(
                &effective_profile,
                &plan.goal,
                &release_gate,
                final_acceptance_status,
                &primary_reason,
                Some(&acceptance),
            ),
            missing.clone(),
            release_recovery_missing_capabilities(Some(&acceptance)),
            release_recovery_repair_targets(&release_gate, Some(&acceptance)),
            release_recovery_verify_commands(&effective_profile, &release_gate),
        )
    } else {
        None
    };
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "ultra_final_acceptance",
            "cycle_index": cycle_index,
            "profile": effective_profile.clone(),
            "effective_profile": effective_profile.clone(),
            "contract_origin": contract_origin_for_acceptance(config),
            "profile_inferred": config
                .profile_inference
                .map(|inference| inference.profile)
                .unwrap_or(""),
            "profile_inference_source": config
                .profile_inference
                .map(|inference| inference.source.as_str())
                .unwrap_or(""),
            "requested_port": requested_port.as_ref().map(|requested| requested.telemetry.clone()),
            "required_paths": required_paths.clone(),
            "missing_paths": missing.clone(),
            "completion_contract_verification_enabled": external_contract_checked,
            "completion_contract_path_merge_enabled": external_contract_checked,
            "completion_contract_path": bound_contract
                .as_ref()
                .map(|bound| bound.path.clone())
                .unwrap_or_default(),
            "completion_contract_generated": bound_contract
                .as_ref()
                .map(|bound| bound.generated)
                .unwrap_or(false),
            "external_contract_checked": external_contract_checked,
            "external_contract_required": contract_required,
            "external_contract_ok": external_ok,
            "required_capabilities": required_capabilities.clone(),
            "required_evidence": required_evidence.clone(),
            "required_obligations": required_obligations.clone(),
            "runtime_acceptance_passed": acceptance.passed && !profile_behavior_failed,
            "runtime_acceptance_status": runtime_acceptance_status,
            "runtime_acceptance_inconclusive": acceptance.inconclusive,
            "compile_errors": compile_errors.clone(),
            "compile_error_failure_kind": if compile_errors.is_empty() { "" } else { "implementation_compile_error" },
            "final_acceptance_status": final_acceptance_status,
            "assurance_level": assurance_level,
            "assurance_reason": assurance_reason,
            "release_quality_completion": release_quality_completion,
            "missing_capabilities": acceptance.missing_capabilities.clone(),
            "missing_evidence": acceptance.missing_evidence.clone(),
            "missing_obligations": acceptance.missing_obligations.clone(),
            "weak_evidence": acceptance.weak_evidence.clone(),
            "runtime_acceptance_diagnostics": acceptance.diagnostics.clone(),
            "unverified_evidence": acceptance.unverified_evidence.clone(),
            "evidence_tiers": acceptance.evidence_tiers.clone(),
            "evidence_arbitration": evidence_arbitration.records.clone(),
            "evidence_arbitration_summary": evidence_arbitration.summary.clone(),
            "artifact_obligations": acceptance.artifact_obligations.clone(),
            "capability_evidence_bindings": acceptance.capability_evidence_bindings.clone(),
            "obligation_repair_targets": acceptance.obligation_repair_targets.clone(),
            "inconclusive_reasons": acceptance.inconclusive_reasons.clone(),
            "release_gate_status": release_gate.status.clone(),
            "release_gate_reasons": release_gate.reasons.clone(),
            "profile_behavior_probe_status": profile_behavior_probe.status,
            "profile_behavior_probe_reasons": profile_behavior_probe.reasons.clone(),
            "profile_behavior_probe_evidence_path": profile_behavior_probe.evidence_path.clone().unwrap_or_default(),
            "browser_readiness_applicable": gate_telemetry.browser_readiness_applicable,
            "browser_readiness_execution_status": gate_telemetry.browser_readiness_execution_status.clone(),
            "browser_readiness_status": release_gate.browser_readiness_status.clone(),
            "browser_readiness_evidence_path": release_gate.browser_readiness_evidence_path.clone(),
            "interaction_evidence_applicable": gate_telemetry.interaction_evidence_applicable,
            "interaction_evidence_execution_status": gate_telemetry.interaction_evidence_execution_status.clone(),
            "interaction_evidence_status": release_gate.interaction_evidence_status.clone(),
            "interaction_evidence_path": release_gate.interaction_evidence_path.clone(),
            "state_dimensions_changed": state_dimensions_changed,
            "action_hooks": action_hooks,
            "surface_fit": surface_fit.raw,
            "surface_fit_summary": surface_fit.summary,
            "surface_fit_guidance": surface_fit.guidance,
            "text_entry": text_telemetry.text_entry,
            "text_entry_target": text_telemetry.text_entry_target,
            "typed_token": text_telemetry.typed_token,
            "token_echoed": text_telemetry.token_echoed,
            "echo_latency_ms": text_telemetry.echo_latency_ms,
            "token_echoed_after_reload": text_telemetry.token_echoed_after_reload,
            "token_echo_after_reload_latency_ms": text_telemetry.token_echo_after_reload_latency_ms,
            "text_input_state_change": text_telemetry.text_input_state_change,
            "next_action": next_action,
            "recovery_handoff_kind": recovery_handoff
                .as_ref()
                .map(|handoff| handoff.recovery_handoff_kind.as_str())
                .unwrap_or_default(),
            "acceptance_layer": recovery_handoff
                .as_ref()
                .map(|handoff| handoff.acceptance_layer.as_str())
                .unwrap_or_default(),
            "recovery_prompt_path": recovery_handoff
                .as_ref()
                .map(|handoff| handoff.recovery_prompt_path.as_str())
                .unwrap_or_default(),
            "recovery_ultra_plan_path": recovery_handoff
                .as_ref()
                .map(|handoff| handoff.recovery_ultra_plan_path.as_str())
                .unwrap_or_default(),
            "suggested_recovery_command": recovery_handoff
                .as_ref()
                .map(|handoff| handoff.suggested_recovery_command.as_str())
                .unwrap_or_default(),
            "suggested_recovery_yaml_command": recovery_handoff
                .as_ref()
                .map(|handoff| handoff.suggested_recovery_yaml_command.as_str())
                .unwrap_or_default(),
            "plan_adherence_present": plan_adherence.present.clone(),
            "plan_adherence_missing": plan_adherence.missing.clone(),
            "recovery_handoff_saved": recovery_handoff
                .as_ref()
                .is_some_and(ReleaseRecoveryHandoffSummary::has_artifact),
            "handoff_saved_not_success": recovery_handoff.is_some(),
            "primary_reason": eval_events::body_snippet(&primary_reason),
        }),
    );
    emit_depth_profile(
        config.eval_events_path.as_deref(),
        "ultra_final_acceptance",
        &depth_profile,
    );
    let mut report = VerificationReport::pass();
    for path in missing {
        report.push_missing_path(path);
    }
    merge_verification_report(&mut report, profile_report);
    if !acceptance.passed {
        report.push_profile_failure(acceptance.primary_reason.clone());
        for guidance in
            runtime_acceptance_repair_guidance(&effective_profile, &plan.goal, &acceptance)
        {
            report.push_profile_failure(format!("repair guidance: {guidance}"));
        }
        for target in &acceptance.obligation_repair_targets {
            report.push_profile_failure(format!(
                "missing_required_obligation_target:{}:{}",
                target.obligation, target.target_path
            ));
        }
    }
    if contract_binding_missing {
        report.push_profile_failure("completion contract binding required but missing".to_string());
    }
    if profile_behavior_failed {
        for reason in &profile_behavior_probe.reasons {
            report.push_profile_failure(reason.clone());
        }
        if let Some(path) = &profile_behavior_probe.evidence_path {
            report.push_profile_failure(format!("profile behavior evidence: {path}"));
        }
    }
    if let Some(external_report) = external_report.filter(|report| {
        !external_contract_ok_after_runtime_arbitration(Some(report), Some(&acceptance))
    }) {
        let reason = external_report.primary_reason();
        merge_verification_report(&mut report, external_report);
        report.push_profile_failure(format!("external contract failed: {}", reason));
    }
    let append_release_observations = !report.is_pass() || release_gate.status == "failed";
    if release_gate.status == "failed" {
        report.push_profile_failure(format!(
            "release gate failed: {}",
            release_gate.reasons.join("; ")
        ));
    }
    if append_release_observations {
        append_release_gate_observation_failures(&mut report, &release_gate);
    }
    if let Some(observation) = &browser_probe
        && let Some(reason) = observation.failure_reason()
    {
        report.push_profile_failure(format!("browser_readiness_failed:{reason}"));
        if !observation.output_excerpt.trim().is_empty() {
            report.push_profile_failure(format!(
                "browser_probe_output: {}",
                eval_events::body_snippet(&observation.output_excerpt)
            ));
        }
        if observation.failure_kind == "build_verifier_failed"
            && !observation.compile_errors.is_empty()
        {
            report.push_compile_errors(
                "browser readiness build verifier",
                observation.compile_errors.clone(),
            );
        }
    }
    Ok(report)
}

pub(super) fn clear_final_acceptance_browser_probe_evidence(config: &Config) {
    for path in release_evidence_candidate_paths(
        config,
        &[
            "browser-readiness.json",
            "browser.json",
            "browser-readiness-evidence.json",
            "browser-interaction.json",
        ],
    ) {
        let _ = std::fs::remove_file(path);
    }
}

pub(super) fn final_acceptance_evidence_arbitration(
    config: &Config,
    report: &mut RuntimeAcceptanceReport,
    required_capabilities: &[String],
    required_evidence: &[String],
    required_obligations: &[String],
) -> EvidenceArbitrationReport {
    behavior_evidence::arbitrate_final_acceptance(
        report,
        &config.workspace_root,
        &release_evidence_extra_dirs(config),
        required_capabilities,
        required_evidence,
        required_obligations,
    )
}

pub(super) fn acceptance_dependency_deterministic_reconcile_needed(
    config: &Config,
    report: &VerificationReport,
) -> bool {
    if !dependency_setup::package_json_declares_dependencies(&config.workspace_root) {
        return false;
    }
    verification_report_mentions_dependency_setup_missing(report)
}

pub(super) fn ultra_contract_runtime_acceptance_report(
    plan: &UltraPlan,
    config: &Config,
) -> anyhow::Result<RuntimeAcceptanceReport> {
    let effective_profile_id = ProfileId::parse(&plan.profile);
    let effective_profile = effective_profile_id.to_string();
    let runtime = ProfileRuntimeRegistry::resolve(&effective_profile_id);
    let mut required_paths = runtime.expected_scaffold_paths(&config.workspace_root, &plan.goal);
    let mut required_capabilities = runtime.required_capabilities(&plan.goal);
    let mut required_obligations =
        runtime.required_obligations(&effective_profile_id, &plan.goal, &required_capabilities);
    let mut required_evidence = runtime.required_evidence(&plan.goal, &required_capabilities);
    let mut requirements = ContractRequirements {
        capabilities: required_capabilities,
        evidence: required_evidence,
        obligations: required_obligations,
    };
    carry_recorded_promotion_contract_requirements(
        config,
        &effective_profile,
        &plan.goal,
        &mut requirements,
    );
    required_capabilities = requirements.capabilities;
    required_evidence = requirements.evidence;
    required_obligations = requirements.obligations;
    let mut evidence_hint_tokens = evidence_hint_tokens_for_goal(&plan.goal);
    let bound_contract = bind_completion_contract_for_acceptance(
        config,
        "ultra-plan-run",
        &effective_profile,
        &plan.goal,
        &required_paths,
        &required_capabilities,
        &required_evidence,
        &required_obligations,
    )?;
    let mut deferred_commands = Vec::new();
    let mut verify_commands = Vec::new();
    if let Some(contract) = bound_contract.as_ref().map(|bound| &bound.contract) {
        merge_unique_strings(&mut required_paths, &contract.required_paths);
        merge_unique_strings(&mut required_capabilities, &contract.required_capabilities);
        merge_unique_strings(&mut required_evidence, &contract.required_evidence);
        merge_unique_strings(&mut required_obligations, &contract.required_obligations);
        merge_unique_strings(&mut verify_commands, &contract.verify_commands);
        merge_unique_strings(&mut evidence_hint_tokens, &contract.evidence_hint_tokens);
        deferred_commands.extend(
            contract
                .deferred_verify_requirements
                .iter()
                .map(|requirement| requirement.command.clone()),
        );
    }
    merge_unique_strings(
        &mut required_evidence,
        &runtime.required_evidence(&plan.goal, &required_capabilities),
    );
    Ok(verify_runtime_acceptance_with_browser_dirs_and_hints(
        &config.workspace_root,
        &required_paths,
        &verify_commands,
        &required_capabilities,
        &required_evidence,
        &required_obligations,
        &deferred_commands,
        &release_evidence_extra_dirs(config),
        &evidence_hint_tokens,
    ))
}

pub(super) fn production_build_failed_release_gate() -> ReleaseGateSummary {
    ReleaseGateSummary {
        status: "not_applicable".to_string(),
        reasons: vec!["production_build_failed_before_browser_probe".to_string()],
        browser_readiness_status: "not_applicable".to_string(),
        browser_readiness_evidence_path: String::new(),
        interaction_evidence_status: "not_applicable".to_string(),
        interaction_evidence_path: String::new(),
    }
}

pub(super) fn final_acceptance_release_gate_with_runtime(
    config: &Config,
    runtime: &dyn ProfileRuntime,
    goal: &str,
    required_capabilities: &[String],
    acceptance: Option<&crate::minimal_loop::evidence::RuntimeAcceptanceReport>,
    check_browser_on_runtime_failure: bool,
) -> ReleaseGateSummary {
    let acceptance_required_evidence = acceptance
        .map(|report| report.evidence_tiers.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    let interaction_options =
        browser_interaction_probe_options(required_capabilities, &acceptance_required_evidence);
    let requested_port = effective_requested_port(runtime, goal, None);
    let requires_browser = runtime.browser_release_gate_profile()
        && (required_capabilities.iter().any(|capability| {
            matches!(
                capability.as_str(),
                "stateful_interaction"
                    | "player_control"
                    | "user_input_or_action"
                    | "visible_state_change"
                    | "persistence"
                    | "adversary_or_challenge"
                    | "progression_or_score"
                    | "failure_or_collision_rule"
            )
        }) || signals::contains_browser_probe_token(goal));
    let Some(report) = acceptance else {
        return ReleaseGateSummary {
            status: "not_applicable".to_string(),
            reasons: Vec::new(),
            browser_readiness_status: "not_applicable".to_string(),
            browser_readiness_evidence_path: String::new(),
            interaction_evidence_status: "not_applicable".to_string(),
            interaction_evidence_path: String::new(),
        };
    };
    if !report.passed {
        if requires_browser
            && check_browser_on_runtime_failure
            && runtime_acceptance_has_buildable_nextjs_boundary(report)
        {
            let mut gate = browser_release_gate_with_options(
                config,
                requires_canvas_surface(goal, required_capabilities),
                interaction_options,
                requested_port.as_ref().map(|requested| requested.port),
            );
            let mut reasons = vec![report.primary_reason.clone()];
            reasons.extend(std::mem::take(&mut gate.reasons));
            gate.status = "failed".to_string();
            gate.reasons = dedup_strings(reasons);
            return gate;
        }
        return ReleaseGateSummary {
            status: "failed".to_string(),
            reasons: vec![report.primary_reason.clone()],
            browser_readiness_status: "not_checked".to_string(),
            browser_readiness_evidence_path: String::new(),
            interaction_evidence_status: "not_checked".to_string(),
            interaction_evidence_path: String::new(),
        };
    }
    if !report.unverified_evidence.is_empty() {
        let mut gate = if requires_browser {
            browser_release_gate_with_options(
                config,
                requires_canvas_surface(goal, required_capabilities),
                interaction_options,
                requested_port.as_ref().map(|requested| requested.port),
            )
        } else {
            ReleaseGateSummary {
                status: "partial".to_string(),
                reasons: Vec::new(),
                browser_readiness_status: "not_applicable".to_string(),
                browser_readiness_evidence_path: String::new(),
                interaction_evidence_status: "not_applicable".to_string(),
                interaction_evidence_path: String::new(),
            }
        };
        let mut reasons = runtime_acceptance_unverified_release_reasons(
            report,
            interaction_probe_performed_for_run(config),
        );
        reasons.extend(std::mem::take(&mut gate.reasons));
        gate.status = "partial".to_string();
        gate.reasons = dedup_strings(reasons);
        return gate;
    }
    if requires_browser {
        return browser_release_gate_with_options(
            config,
            requires_canvas_surface(goal, required_capabilities),
            interaction_options,
            requested_port.as_ref().map(|requested| requested.port),
        );
    }
    ReleaseGateSummary {
        status: "pass".to_string(),
        reasons: Vec::new(),
        browser_readiness_status: "not_applicable".to_string(),
        browser_readiness_evidence_path: String::new(),
        interaction_evidence_status: "not_applicable".to_string(),
        interaction_evidence_path: String::new(),
    }
}

pub(super) fn acceptance_gate_telemetry(
    profile: &str,
    signal_text: &str,
    required_capabilities: &[String],
    required_evidence: &[String],
    release_gate: &ReleaseGateSummary,
) -> AcceptanceGateTelemetry {
    let browser_applicable = ultra_browser_probe_required(
        resolve_profile_runtime(profile),
        signal_text,
        required_capabilities,
    );
    let interaction_applicable =
        browser_applicable && interaction_gate_required(required_capabilities, required_evidence);
    AcceptanceGateTelemetry {
        browser_readiness_applicable: browser_applicable,
        browser_readiness_execution_status: gate_execution_status(
            &release_gate.browser_readiness_status,
        ),
        interaction_evidence_applicable: interaction_applicable,
        interaction_evidence_execution_status: gate_execution_status(
            &release_gate.interaction_evidence_status,
        ),
    }
}

pub(super) fn interaction_gate_required(
    required_capabilities: &[String],
    required_evidence: &[String],
) -> bool {
    required_capabilities
        .iter()
        .chain(required_evidence.iter())
        .any(|requirement| {
            matches!(
                requirement.as_str(),
                "stateful_interaction"
                    | "player_control"
                    | "user_input_or_action"
                    | "visible_state_change"
                    | "persistence"
                    | "adversary_or_challenge"
                    | "progression_or_score"
                    | "failure_or_collision_rule"
                    | "browser_interaction"
                    | "playable_ui"
                    | "interactive_ui_source_evidence"
                    | "visible_interactive_surface_evidence"
                    | "user_input_handler_evidence"
                    | "stateful_update_evidence"
                    | "non_static_screen_evidence"
                    | "persistence_evidence"
                    | "challenge_or_adversary_evidence"
                    | "score_or_progression_evidence"
                    | "failure_or_collision_evidence"
            )
        })
}

pub(super) fn runtime_acceptance_unverified_release_reasons(
    report: &crate::minimal_loop::evidence::RuntimeAcceptanceReport,
    probe_performed_for_run: bool,
) -> Vec<String> {
    let mut reasons = Vec::new();
    let mut saw_probe_unavailable = false;
    for evidence in &report.unverified_evidence {
        if let Some(reason) = evidence
            .split_once(":unverified:")
            .map(|(_, reason)| reason.trim())
            .filter(|reason| !reason.is_empty())
        {
            if reason == "probe_unavailable" {
                if probe_performed_for_run {
                    continue;
                }
                saw_probe_unavailable = true;
            } else {
                reasons.push(format!("interaction_unverified:{reason}"));
            }
        }
        if probe_performed_for_run && evidence.contains(":unverified:probe_unavailable") {
            continue;
        }
        reasons.push(format!("unverified_probe_required:{evidence}"));
    }
    if saw_probe_unavailable {
        reasons.insert(0, "interaction_unverified:probe_unavailable".to_string());
        reasons.push(
            crate::minimal_loop::interaction_probe::INTERACTION_PROBE_SETUP_REMEDIATION.to_string(),
        );
    }
    reasons
}

pub(super) fn interaction_probe_performed_for_run(config: &Config) -> bool {
    release_evidence_candidate_paths(
        config,
        &[
            "browser-interaction.json",
            "interaction-evidence.json",
            "interaction.json",
        ],
    )
    .into_iter()
    .filter_map(|path| std::fs::read_to_string(path).ok())
    .filter_map(|text| serde_json::from_str::<Value>(&text).ok())
    .any(|value| {
        let details = value
            .get("browser_details")
            .or_else(|| value.get("details"))
            .filter(|value| value.is_object());
        bool_field_deep(&value, details, &["interaction_performed"]) == Some(true)
    })
}

pub(super) fn runtime_acceptance_has_buildable_nextjs_boundary(
    report: &crate::minimal_loop::evidence::RuntimeAcceptanceReport,
) -> bool {
    !report.missing_evidence.iter().any(|item| {
        matches!(
            item.as_str(),
            "implementation_artifact"
                | "nextjs_route_evidence"
                | "build_command_or_dependency_missing_boundary"
        )
    }) && !report
        .missing_obligations
        .iter()
        .any(|item| item == "implementation")
}

pub(super) fn requires_canvas_surface(signal_text: &str, required_capabilities: &[String]) -> bool {
    signals::contains_canvas_token(signal_text)
        && required_capabilities.iter().any(|capability| {
            matches!(
                capability.as_str(),
                "browser_interaction"
                    | "playable_ui"
                    | "stateful_interaction"
                    | "player_control"
                    | "user_input_or_action"
                    | "visible_state_change"
                    | "adversary_or_challenge"
                    | "progression_or_score"
                    | "failure_or_collision_rule"
            )
        })
}

pub(super) fn append_release_gate_observation_failures(
    report: &mut VerificationReport,
    release_gate: &ReleaseGateSummary,
) {
    let browser_compile_errors = if !release_gate.browser_readiness_evidence_path.is_empty()
        && release_gate
            .browser_readiness_status
            .contains("build_verifier_failed")
    {
        compile_errors_from_release_evidence_path(&release_gate.browser_readiness_evidence_path)
    } else {
        Vec::new()
    };
    append_gate_observation(
        report,
        "browser readiness status",
        "browser readiness evidence",
        &release_gate.browser_readiness_status,
        &release_gate.browser_readiness_evidence_path,
    );
    if !release_gate.browser_readiness_evidence_path.is_empty() {
        report.push_compile_errors("browser readiness build verifier", browser_compile_errors);
    }
    append_gate_observation(
        report,
        "interaction evidence status",
        "interaction evidence path",
        &release_gate.interaction_evidence_status,
        &release_gate.interaction_evidence_path,
    );
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ReleaseEvidenceStatus {
    Passed,
    Failed(String),
    Unavailable(String),
}

impl ReleaseEvidenceStatus {
    fn as_status(&self) -> String {
        match self {
            Self::Passed => "passed".to_string(),
            Self::Failed(reason) => format!("failed:{reason}"),
            Self::Unavailable(reason) => format!("unavailable:{reason}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReleaseEvidence {
    status: ReleaseEvidenceStatus,
    path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ReleaseEvidenceKind {
    BrowserReadiness,
    Interaction,
}

#[cfg(test)]
pub(super) fn browser_release_gate(config: &Config) -> ReleaseGateSummary {
    browser_release_gate_with_expectations(config, false)
}

#[cfg(test)]
pub(super) fn browser_release_gate_with_expectations(
    config: &Config,
    canvas_surface_expected: bool,
) -> ReleaseGateSummary {
    browser_release_gate_with_options(
        config,
        canvas_surface_expected,
        BrowserInteractionProbeOptions::default(),
        None,
    )
}

pub(super) fn browser_release_gate_with_options(
    config: &Config,
    canvas_surface_expected: bool,
    interaction_options: BrowserInteractionProbeOptions,
    requested_port: Option<u16>,
) -> ReleaseGateSummary {
    let mut browser = read_release_evidence(
        config,
        &[
            "browser-readiness.json",
            "browser.json",
            "browser-readiness-evidence.json",
        ],
        "browser_readiness_evidence_missing",
        ReleaseEvidenceKind::BrowserReadiness,
    );
    if matches!(
        &browser.status,
        ReleaseEvidenceStatus::Unavailable(reason)
            if reason == "browser_readiness_evidence_missing"
    ) {
        browser = nextjs_dev_route_release_evidence(config, interaction_options, requested_port);
    }
    let interaction = read_release_evidence(
        config,
        &[
            "browser-interaction.json",
            "interaction-evidence.json",
            "interaction.json",
        ],
        "interaction_evidence_missing",
        ReleaseEvidenceKind::Interaction,
    );
    let browser_status = browser.status.as_status();
    let mut interaction_status = interaction.status.as_status();
    let canvas_surface_missing =
        release_gate_canvas_surface_missing(canvas_surface_expected, &browser, &interaction);
    if let ReleaseEvidenceStatus::Failed(reason) = &browser.status {
        if matches!(interaction.status, ReleaseEvidenceStatus::Unavailable(_)) {
            interaction_status = format!("not_exercised:{reason}");
        }
        return ReleaseGateSummary {
            status: "failed".to_string(),
            reasons: vec![format!("browser_readiness_failed:{reason}")],
            browser_readiness_status: browser_status,
            browser_readiness_evidence_path: browser.path,
            interaction_evidence_status: interaction_status,
            interaction_evidence_path: interaction.path,
        };
    }
    if let ReleaseEvidenceStatus::Failed(reason) = &interaction.status {
        if interaction_probe_infrastructure_failure_reason(reason) {
            let mut reasons = vec![
                reason.clone(),
                format!("app interaction untested (probe infrastructure failure: {reason})"),
            ];
            if let Some(remediation) = interaction_probe_failure_remediation(&interaction.path) {
                reasons.push(remediation);
            } else if reason == "probe_dependency_missing:browser_binaries_missing" {
                reasons.push(
                    crate::minimal_loop::interaction_probe::INTERACTION_PROBE_SETUP_REMEDIATION
                        .to_string(),
                );
            }
            return ReleaseGateSummary {
                status: "failed".to_string(),
                reasons: dedup_strings(reasons),
                browser_readiness_status: browser_status,
                browser_readiness_evidence_path: browser.path,
                interaction_evidence_status: interaction_status,
                interaction_evidence_path: interaction.path,
            };
        }
        return ReleaseGateSummary {
            status: "failed".to_string(),
            reasons: vec![format!("browser_interaction_failed:{reason}")],
            browser_readiness_status: browser_status,
            browser_readiness_evidence_path: browser.path,
            interaction_evidence_status: interaction_status,
            interaction_evidence_path: interaction.path,
        };
    }
    if let ReleaseEvidenceStatus::Unavailable(reason) = &browser.status {
        return ReleaseGateSummary {
            status: "partial".to_string(),
            reasons: vec![format!(
                "browser_readiness_or_interaction_evidence_required:{reason}"
            )],
            browser_readiness_status: browser_status,
            browser_readiness_evidence_path: browser.path,
            interaction_evidence_status: interaction_status,
            interaction_evidence_path: interaction.path,
        };
    }
    if let ReleaseEvidenceStatus::Unavailable(reason) = &interaction.status {
        let mut reasons = Vec::new();
        if canvas_surface_missing {
            reasons.push(
                "browser_readiness_or_interaction_evidence_required:rendered_without_expected_surface"
                    .to_string(),
            );
        }
        if interaction_probe_unavailable_reason_value(reason) {
            if interaction_probe_performed_for_run(config) {
                reasons.push(
                    "browser_interaction_evidence_required:interaction_detail_missing".to_string(),
                );
                return ReleaseGateSummary {
                    status: "partial".to_string(),
                    reasons: dedup_strings(reasons),
                    browser_readiness_status: browser_status,
                    browser_readiness_evidence_path: browser.path,
                    interaction_evidence_status: interaction_status,
                    interaction_evidence_path: interaction.path,
                };
            }
            reasons.extend([
                "interaction_unverified:probe_unavailable".to_string(),
                crate::minimal_loop::interaction_probe::INTERACTION_PROBE_SETUP_REMEDIATION
                    .to_string(),
            ]);
            return ReleaseGateSummary {
                status: "partial".to_string(),
                reasons: dedup_strings(reasons),
                browser_readiness_status: browser_status,
                browser_readiness_evidence_path: browser.path,
                interaction_evidence_status: interaction_status,
                interaction_evidence_path: interaction.path,
            };
        }
        reasons.push(format!("browser_interaction_evidence_required:{reason}"));
        return ReleaseGateSummary {
            status: "partial".to_string(),
            reasons: dedup_strings(reasons),
            browser_readiness_status: browser_status,
            browser_readiness_evidence_path: browser.path,
            interaction_evidence_status: interaction_status,
            interaction_evidence_path: interaction.path,
        };
    }
    ReleaseGateSummary {
        status: "pass".to_string(),
        reasons: Vec::new(),
        browser_readiness_status: browser_status,
        browser_readiness_evidence_path: browser.path,
        interaction_evidence_status: interaction_status,
        interaction_evidence_path: interaction.path,
    }
}

pub(super) fn nextjs_dev_route_release_evidence(
    config: &Config,
    interaction_options: BrowserInteractionProbeOptions,
    requested_port: Option<u16>,
) -> ReleaseEvidence {
    let path = nextjs_dev_route_evidence_path(config);
    let value = run_nextjs_dev_route_probe_with_interaction_options(
        config,
        &path,
        interaction_options,
        requested_port,
    );
    let status = classify_release_evidence_json(ReleaseEvidenceKind::BrowserReadiness, &value);
    write_release_evidence_json(&path, &value);
    ReleaseEvidence {
        status,
        path: path.display().to_string(),
    }
}

pub(super) fn write_release_evidence_json(path: &Path, value: &Value) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string_pretty(value) {
        let _ = std::fs::write(path, format!("{text}\n"));
    }
}

pub(super) fn interaction_probe_infrastructure_failure_reason(reason: &str) -> bool {
    reason.starts_with("probe_dependency_missing")
        || reason.starts_with("probe_infrastructure_failed")
}

pub(super) fn release_gate_has_interaction_probe_infrastructure_failure(
    release_gate: &ReleaseGateSummary,
) -> bool {
    release_gate
        .interaction_evidence_status
        .strip_prefix("failed:")
        .is_some_and(interaction_probe_infrastructure_failure_reason)
        || release_gate
            .reasons
            .iter()
            .any(|reason| interaction_probe_infrastructure_failure_reason(reason))
}

pub(super) fn interaction_probe_failure_remediation(path: &str) -> Option<String> {
    let value = std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())?;
    let details = value
        .get("browser_details")
        .or_else(|| value.get("details"))
        .filter(|value| value.is_object());
    text_field_deep(&value, details, &["remediation"]).filter(|remediation| !remediation.is_empty())
}

pub(super) fn nextjs_dev_route_evidence_path(config: &Config) -> PathBuf {
    if let Some(events_path) = &config.eval_events_path
        && let Some(run_dir) = events_path.parent()
    {
        return run_dir.join("browser-readiness.json");
    }
    config
        .workspace_root
        .join(".anvil")
        .join("browser-readiness.json")
}

pub(super) fn release_evidence_canvas_marker_is_false(path: &str) -> bool {
    let value = std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok());
    value.as_ref().is_some_and(|value| {
        let details = value
            .get("browser_details")
            .or_else(|| value.get("details"))
            .filter(|value| value.is_object());
        bool_field_deep(value, details, &["ssr_has_canvas", "has_canvas"]) == Some(false)
    })
}

pub(super) fn release_gate_canvas_surface_missing(
    expected: bool,
    browser: &ReleaseEvidence,
    interaction: &ReleaseEvidence,
) -> bool {
    if !expected {
        return false;
    }
    if release_interaction_surface_authoritative(interaction) {
        return release_interaction_canvas_marker(&interaction.path) == Some(false);
    }
    matches!(browser.status, ReleaseEvidenceStatus::Passed)
        && release_evidence_canvas_marker_is_false(&browser.path)
}

pub(super) fn release_interaction_surface_authoritative(interaction: &ReleaseEvidence) -> bool {
    if interaction.path.is_empty() {
        return false;
    }
    match &interaction.status {
        ReleaseEvidenceStatus::Passed => true,
        ReleaseEvidenceStatus::Failed(reason) => {
            !interaction_probe_infrastructure_failure_reason(reason)
        }
        ReleaseEvidenceStatus::Unavailable(_) => false,
    }
}

pub(super) fn release_interaction_canvas_marker(path: &str) -> Option<bool> {
    let value = std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())?;
    let details = value
        .get("browser_details")
        .or_else(|| value.get("details"))
        .filter(|value| value.is_object());
    if let Some(has_canvas) = bool_field_deep(
        &value,
        details,
        &[
            "post_js_has_canvas",
            "has_canvas",
            "canvas_found",
            "canvas_available",
        ],
    ) {
        return Some(has_canvas);
    }
    numeric_field_deep(&value, details, &["post_js_canvas_count", "canvas_count"])
        .map(|count| count > 0)
}

pub(super) fn read_release_evidence(
    config: &Config,
    names: &[&str],
    missing_reason: &'static str,
    kind: ReleaseEvidenceKind,
) -> ReleaseEvidence {
    for path in release_evidence_candidate_paths(config, names) {
        if !path.is_file() {
            continue;
        }
        let display = path.display().to_string();
        let Ok(text) = std::fs::read_to_string(&path) else {
            return ReleaseEvidence {
                status: ReleaseEvidenceStatus::Failed("evidence_unreadable".to_string()),
                path: display,
            };
        };
        let Ok(json) = serde_json::from_str::<Value>(&text) else {
            return ReleaseEvidence {
                status: ReleaseEvidenceStatus::Failed("evidence_invalid_json".to_string()),
                path: display,
            };
        };
        return ReleaseEvidence {
            status: classify_release_evidence_json(kind, &json),
            path: display,
        };
    }
    if kind == ReleaseEvidenceKind::Interaction
        && let Some(reason) = interaction_probe_unavailable_reason(&config.workspace_root)
    {
        return ReleaseEvidence {
            status: ReleaseEvidenceStatus::Unavailable(reason),
            path: String::new(),
        };
    }
    ReleaseEvidence {
        status: ReleaseEvidenceStatus::Unavailable(missing_reason.to_string()),
        path: String::new(),
    }
}

pub(super) fn interaction_probe_unavailable_reason(root: &Path) -> Option<String> {
    interaction_probe::playwright_availability(root)
        .unavailable_reason()
        .map(str::to_string)
}

pub(super) fn interaction_probe_unavailable_reason_value(reason: &str) -> bool {
    matches!(reason, "playwright_not_installed" | "probe_unavailable")
}

pub(super) fn release_evidence_candidate_paths(config: &Config, names: &[&str]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(events_path) = &config.eval_events_path
        && let Some(run_dir) = events_path.parent()
    {
        for name in names {
            out.push(run_dir.join(name));
        }
    }
    for name in names {
        out.push(
            config
                .workspace_root
                .join(".anvil")
                .join("evidence")
                .join(name),
        );
        out.push(config.workspace_root.join(".anvil").join(name));
        out.push(config.workspace_root.join(name));
    }
    out
}

pub(super) fn release_evidence_extra_dirs(config: &Config) -> Vec<PathBuf> {
    config
        .eval_events_path
        .as_ref()
        .and_then(|events_path| events_path.parent())
        .map(|run_dir| vec![run_dir.to_path_buf()])
        .unwrap_or_default()
}

pub(super) fn classify_release_evidence_json(
    kind: ReleaseEvidenceKind,
    value: &Value,
) -> ReleaseEvidenceStatus {
    let details = value
        .get("browser_details")
        .or_else(|| value.get("details"))
        .filter(|value| value.is_object());
    let text_status = text_field_deep(value, details, &["status"]);
    if let Some(status) = text_status.as_deref()
        && is_release_evidence_unavailable_status(status)
    {
        return ReleaseEvidenceStatus::Unavailable(evidence_unavailable_reason(
            value, details, status,
        ));
    }
    if let Some(status) =
        numeric_field_deep(value, details, &["http_status", "status", "status_code"])
        && status >= 400
    {
        return ReleaseEvidenceStatus::Failed(evidence_http_failure_reason(value, details, status));
    }
    if let Some(success) = bool_field_deep(
        value,
        details,
        &["ok", "success", "browser_success", "interaction_success"],
    ) && !success
    {
        return ReleaseEvidenceStatus::Failed(evidence_failure_reason(value, details));
    }
    if let Some(reason) = explicit_release_evidence_failure(kind, value, details) {
        return ReleaseEvidenceStatus::Failed(reason);
    }
    if let Some(status) = text_status.as_deref()
        && matches!(status, "failed" | "fail" | "error")
    {
        return ReleaseEvidenceStatus::Failed(evidence_failure_reason(value, details));
    }
    if let Some(kind_value) = text_field_deep(
        value,
        details,
        &["browser_failure_kind", "failure_kind", "error_kind"],
    ) && !kind_value.is_empty()
    {
        return ReleaseEvidenceStatus::Failed(kind_value);
    }
    if release_evidence_has_required_detail(kind, value, details) {
        return ReleaseEvidenceStatus::Passed;
    }
    let status_is_pass_like = text_status
        .as_deref()
        .is_some_and(|status| matches!(status, "ok" | "pass" | "passed" | "ready"));
    let success_is_true = bool_field_deep(
        value,
        details,
        &["ok", "success", "browser_success", "interaction_success"],
    ) == Some(true);
    let http_is_ok = numeric_field_deep(value, details, &["http_status", "status", "status_code"])
        .is_some_and(|status| (200..400).contains(&status));
    if success_is_true || status_is_pass_like || http_is_ok {
        return ReleaseEvidenceStatus::Unavailable(
            match kind {
                ReleaseEvidenceKind::BrowserReadiness => "browser_render_evidence_missing",
                ReleaseEvidenceKind::Interaction => "interaction_detail_missing",
            }
            .to_string(),
        );
    }
    ReleaseEvidenceStatus::Unavailable("evidence_inconclusive".to_string())
}

pub(super) fn explicit_release_evidence_failure(
    kind: ReleaseEvidenceKind,
    value: &Value,
    details: Option<&Value>,
) -> Option<String> {
    match kind {
        ReleaseEvidenceKind::BrowserReadiness => {
            if bool_field_deep(
                value,
                details,
                &["route_rendered", "rendered", "page_loaded", "dom_ready"],
            ) == Some(false)
            {
                return Some("browser_route_not_rendered".to_string());
            }
        }
        ReleaseEvidenceKind::Interaction => {
            let transition_observed =
                bool_field_deep(value, details, &["start_transition", "transition_observed"])
                    == Some(true)
                    || string_array_field_contains_deep(
                        value,
                        details,
                        "steps",
                        "start_transition",
                    )
                    || string_array_field_contains_deep(
                        value,
                        details,
                        "steps",
                        "recovery_transition",
                    );
            if bool_field_deep(value, details, &["canvas_found", "canvas_available"]) == Some(false)
            {
                return Some("canvas_unavailable".to_string());
            }
            if bool_field_deep(
                value,
                details,
                &["interactive_surface", "interaction_surface"],
            ) == Some(false)
            {
                return Some("interactive_surface_missing".to_string());
            }
            if bool_field_deep(
                value,
                details,
                &[
                    "input_event_observed",
                    "keyboard_event_observed",
                    "pointer_event_observed",
                ],
            ) == Some(false)
            {
                return Some("input_event_missing".to_string());
            }
            if bool_field_deep(value, details, &["state_changed", "visible_state_changed"])
                == Some(false)
            {
                if !transition_observed {
                    return Some("start_transition_missing".to_string());
                }
                if bool_field_deep(value, details, &["input_state_evaluated_after_start"])
                    == Some(false)
                {
                    return Some("input_state_change_not_evaluated_after_start".to_string());
                }
                return Some("input_state_change_missing_after_start".to_string());
            }
        }
    }
    None
}

pub(super) fn release_evidence_has_required_detail(
    kind: ReleaseEvidenceKind,
    value: &Value,
    details: Option<&Value>,
) -> bool {
    match kind {
        ReleaseEvidenceKind::BrowserReadiness => {
            bool_field_deep(
                value,
                details,
                &["route_rendered", "rendered", "page_loaded", "dom_ready"],
            ) == Some(true)
        }
        ReleaseEvidenceKind::Interaction => {
            let transition_observed =
                bool_field_deep(value, details, &["start_transition", "transition_observed"])
                    == Some(true)
                    || string_array_field_contains_deep(
                        value,
                        details,
                        "steps",
                        "start_transition",
                    )
                    || string_array_field_contains_deep(
                        value,
                        details,
                        "steps",
                        "recovery_transition",
                    );
            let input_state_changed = bool_field_deep(
                value,
                details,
                &[
                    "input_state_change",
                    "state_changed",
                    "visible_state_changed",
                ],
            ) == Some(true)
                || string_array_field_contains_deep(value, details, "steps", "input_state_change");
            transition_observed && input_state_changed
        }
    }
}

pub(super) fn evidence_failure_reason(value: &Value, details: Option<&Value>) -> String {
    let text_reason = text_field_deep(
        value,
        details,
        &[
            "browser_failure_kind",
            "failure_kind",
            "error_kind",
            "status",
        ],
    );
    if text_reason
        .as_deref()
        .is_some_and(prefer_release_evidence_failure_kind_over_http)
    {
        return text_reason.unwrap();
    }
    if let Some(status) =
        numeric_field_deep(value, details, &["http_status", "status", "status_code"])
        && status >= 400
    {
        return format!("http_{status}");
    }
    text_reason.unwrap_or_else(|| "browser_check_failed".to_string())
}

pub(super) fn evidence_http_failure_reason(
    value: &Value,
    details: Option<&Value>,
    status: i64,
) -> String {
    text_field_deep(
        value,
        details,
        &["browser_failure_kind", "failure_kind", "error_kind"],
    )
    .filter(|reason| prefer_release_evidence_failure_kind_over_http(reason))
    .unwrap_or_else(|| format!("http_{status}"))
}

pub(super) fn evidence_unavailable_reason(
    value: &Value,
    details: Option<&Value>,
    status: &str,
) -> String {
    text_field_deep(
        value,
        details,
        &[
            "browser_failure_kind",
            "failure_kind",
            "error_kind",
            "reason",
        ],
    )
    .filter(|reason| !reason.is_empty())
    .unwrap_or_else(|| status.to_string())
}

pub(super) fn is_release_evidence_unavailable_status(status: &str) -> bool {
    matches!(
        status,
        "not_enabled" | "adapter_not_implemented" | "unavailable" | "skipped"
    ) || status.starts_with("unavailable:")
        || status == "browser_unavailable"
        || status.starts_with("browser_unavailable:")
        || status == "skipped_offline"
        || status == "skipped_unsupported_profile"
}

pub(super) fn prefer_release_evidence_failure_kind_over_http(reason: &str) -> bool {
    matches!(
        reason,
        "tailwind_dev_pipeline_failure"
            | "css_dev_pipeline_failure"
            | "nextjs_dev_pipeline_failure"
    )
}

pub(super) fn bool_field_deep(
    value: &Value,
    details: Option<&Value>,
    keys: &[&str],
) -> Option<bool> {
    bool_field(value, keys).or_else(|| details.and_then(|details| bool_field(details, keys)))
}

pub(super) fn bool_field(value: &Value, keys: &[&str]) -> Option<bool> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_bool))
}

pub(super) fn numeric_field_deep(
    value: &Value,
    details: Option<&Value>,
    keys: &[&str],
) -> Option<i64> {
    numeric_field(value, keys).or_else(|| details.and_then(|details| numeric_field(details, keys)))
}

pub(super) fn numeric_field(value: &Value, keys: &[&str]) -> Option<i64> {
    for key in keys {
        let Some(raw) = value.get(*key) else {
            continue;
        };
        if let Some(number) = raw.as_i64() {
            return Some(number);
        }
        if let Some(text) = raw.as_str()
            && let Ok(number) = text.parse::<i64>()
        {
            return Some(number);
        }
    }
    None
}

pub(super) fn text_field_deep(
    value: &Value,
    details: Option<&Value>,
    keys: &[&str],
) -> Option<String> {
    text_field(value, keys).or_else(|| details.and_then(|details| text_field(details, keys)))
}

pub(super) fn text_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(|text| text.trim().to_ascii_lowercase())
}

pub(super) fn string_array_field_contains_deep(
    value: &Value,
    details: Option<&Value>,
    key: &str,
    needle: &str,
) -> bool {
    string_array_field_contains(value, key, needle)
        || details.is_some_and(|details| string_array_field_contains(details, key, needle))
}

pub(super) fn string_array_field_contains(value: &Value, key: &str, needle: &str) -> bool {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .any(|item| item == needle)
}
