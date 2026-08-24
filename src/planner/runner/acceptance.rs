// Pure extraction from the E-5d responsibility map (pre-split runner.rs:5178-7299).
// Event names, evidence fields, and emitted strings remain owned by the moved code.
#[allow(unused_imports)]
use super::{
    APP_BEHAVIOR_PROBE_FAILURE_KINDS, BoundCompletionContract, BrowserInteractionProbeOptions,
    BrowserReadinessObservation, Child, CompileError, Config, DEV_SERVER_CLEANUP_KILL_TIMEOUT,
    DEV_SERVER_CLEANUP_TERM_TIMEOUT, DEV_SERVER_LOG_EXCERPT_BYTES, DEV_SERVER_ROUTE, Duration,
    Instant, InteractionProbeOutcome, NEXTJS_DEV_SERVER_CONNECT_TIMEOUT,
    NEXTJS_DEV_SERVER_DEFAULT_PORT, NEXTJS_DEV_SERVER_READY_TIMEOUT,
    NEXTJS_DEV_SERVER_WAIT_INTERVAL, Path, PathBuf, ProfileBehaviorProbeReport, ProfileId,
    ProfileRuntime, ProfileRuntimeRegistry, RESTART_PARTIAL_REPAIR_GUIDANCE, ReleaseGateSummary,
    SocketAddr, Stdio, StepPlan, TEXT_ECHO_REPAIR_REQUIREMENT, TcpStream, UnattachedRefDiagnostic,
    Value, VerificationReport, bind_completion_contract_for_acceptance, bounded_process,
    build_verifier, compile_error_repair_guidance, current_final_acceptance_cycle_index,
    dedup_strings, depth_profile, effective_requested_port, emit_depth_profile, eval_events,
    evidence_hint_tokens_for_goal, external_contract_ok_after_runtime_arbitration,
    final_acceptance_evidence_arbitration, final_acceptance_release_gate_with_runtime,
    html_surface_markers_json, interaction_action_hooks_from_path,
    interaction_candidate_prompt_lines, interaction_probe, interaction_root_cause_repair_guidance,
    interaction_state_dimensions_changed_from_path, interaction_surface_fit_from_path,
    interaction_text_telemetry_from_path, json, merge_unique_strings, missing_final_artifacts,
    raw_bool_field_deep, raw_contract_hook_bool, raw_string_array_field_deep, raw_text_field_deep,
    raw_u64_field_deep, raw_value_scopes, recovery_scope_token, release_evidence_extra_dirs,
    release_gate_final_acceptance_status,
    release_gate_has_interaction_probe_infrastructure_failure, release_gate_next_action,
    release_quality_completion_status, release_recovery_acceptance_layer, release_recovery_needed,
    resolve_profile_runtime, route_bound_unattached_ref_diagnostics, runtime_acceptance_status,
    save_release_recovery_handoff, surface_fit_guidance_lines_from_value, verifier_env,
    verify_runtime_acceptance_with_browser_dirs_and_hints, write_release_evidence_json,
};
#[allow(unused_imports)]
use super::{DEV_SERVER_LIFECYCLE_STAGES, ReleaseRecoveryHandoffSummary};
use std::io::{Read, Write};

#[path = "acceptance/plan_final_probe.rs"]
mod plan_final_probe;
pub(super) fn emit_browser_probe_event(
    config: &Config,
    observation: &BrowserReadinessObservation,
    requested_port: Option<String>,
) {
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "browser_probe",
            "cycle_index": current_final_acceptance_cycle_index(),
            "profile": observation.profile,
            "status": observation.status,
            "ok": observation.ok,
            "requested_port": requested_port,
            "port": observation.port,
            "route": observation.route,
            "command": observation.command,
            "http_status": observation.http_status,
            "failure_kind": observation.failure_kind,
            "elapsed_ms": observation.elapsed_ms,
            "evidence_path": observation.evidence_path.display().to_string(),
            "output_excerpt": eval_events::body_snippet(&observation.output_excerpt),
            "child_spawned": observation.child_spawned,
            "child_reaped": observation.child_reaped,
            "ssr_has_canvas": observation.has_canvas,
            "ssr_interactive_control_count": observation.interactive_control_count,
            "has_canvas": observation.has_canvas,
            "interactive_control_count": observation.interactive_control_count,
            "title_text_excerpt": observation.title_text_excerpt,
        }),
    );
}

pub(super) fn emit_browser_interaction_probe_event(
    config: &Config,
    outcome: &InteractionProbeOutcome,
) {
    let source_diagnostics = interaction_source_diagnostics(config);
    let source_diagnostic_labels = unattached_ref_diagnostic_labels(&source_diagnostics);
    match outcome {
        InteractionProbeOutcome::Unavailable(reason) => {
            eval_events::emit(
                config.eval_events_path.as_deref(),
                json!({
                    "event": "browser_interaction_probe",
                    "cycle_index": current_final_acceptance_cycle_index(),
                    "status": "unavailable",
                    "ok": false,
                    "failure_kind": reason,
                    "evidence_path": "",
                    "source_diagnostics": &source_diagnostic_labels,
                    "unattached_ref_diagnostics": &source_diagnostics,
                    "playwright_resolution_location": "",
                    "playwright_version": "",
                }),
            );
        }
        InteractionProbeOutcome::Observation(observation) => {
            annotate_interaction_evidence_with_source_diagnostics(
                &observation.evidence_path,
                &source_diagnostics,
            );
            let workspace_evidence =
                interaction_probe::browser_interaction_evidence_path(&config.workspace_root);
            if workspace_evidence != observation.evidence_path {
                annotate_interaction_evidence_with_source_diagnostics(
                    &workspace_evidence,
                    &source_diagnostics,
                );
            }
            let resolution = observation.playwright_resolution.as_ref();
            eval_events::emit(
                config.eval_events_path.as_deref(),
                json!({
                    "event": "browser_interaction_probe",
                    "cycle_index": current_final_acceptance_cycle_index(),
                    "status": observation.status,
                    "ok": observation.ok,
                    "failure_kind": observation.failure_kind,
                    "failure_category": if observation.failure_kind.starts_with("probe_dependency_missing")
                        || observation.failure_kind.starts_with("probe_infrastructure_failed")
                    {
                        "infrastructure"
                    } else if observation.failure_kind.is_empty() {
                        ""
                    } else {
                        "app"
                    },
                    "stage": observation.stage,
                    "error": observation.error,
                    "stderr_excerpt": observation.stderr_excerpt,
                    "server_http_status": observation.server_http_status,
                    "server_http_error": observation.server_http_error,
                    "navigation_failure_kind": observation.navigation_failure_kind,
                    "cold_start_ms": observation.cold_start_ms,
                    "measured_navigation_ms": observation.measured_navigation_ms,
                    "has_canvas": observation.has_canvas,
                    "interactive_control_count": observation.interactive_control_count,
                    "steps": observation.steps,
                    "probe_mode": observation.probe_mode.as_str(),
                    "contract_hook_status": observation.contract_hook_status.as_str(),
                    "candidate_table": &observation.candidate_table,
                    "input_dispatches": &observation.input_dispatches,
                    "canvas_snapshots": &observation.canvas_snapshots,
                    "canvas_blank_before_start": observation.canvas_blank_before_start,
                    "canvas_blank_after_start": observation.canvas_blank_after_start,
                    "canvas_blank_after_inputs": observation.canvas_blank_after_inputs,
                    "source_diagnostics": &source_diagnostic_labels,
                    "unattached_ref_diagnostics": &source_diagnostics,
                    "state_dimensions_changed": &observation.state_dimensions_changed,
                    "surface_fit": &observation.surface_fit,
                    "restart_hook_reachable_after_start": observation.restart_hook_reachable_after_start,
                    "restart_hook_count_after_start": observation.restart_hook_count_after_start,
                    "persistence_after_reload": observation.persistence_after_reload.as_str(),
                    "persistence_after_reload_reason": observation.persistence_after_reload_reason.as_str(),
                    "persistence_changed_dimensions": &observation.persistence_changed_dimensions,
                    "action_hooks": &observation.action_hooks,
                    "text_entry": observation.text_entry.as_str(),
                    "text_entry_target": observation.text_entry_target.as_str(),
                    "typed_token": observation.typed_token.as_str(),
                    "token_echoed": observation.token_echoed,
                    "echo_latency_ms": observation.echo_latency_ms,
                    "token_echoed_after_reload": observation.token_echoed_after_reload,
                    "token_echo_after_reload_latency_ms": observation.token_echo_after_reload_latency_ms,
                    "text_input_state_change": observation.text_input_state_change,
                    "input_state_evaluated_after_start": observation.input_state_evaluated_after_start,
                    "primary_start_transition": observation.primary_transition_observed,
                    "informational_failure_kinds": &observation.informational_failure_kinds,
                    "duration_ms": observation.duration_ms,
                    "evidence_path": observation.evidence_path.display().to_string(),
                    "script_path": observation.script_path.display().to_string(),
                    "output_excerpt": eval_events::body_snippet(&observation.output_excerpt),
                    "child_spawned": observation.child_spawned,
                    "child_reaped": observation.child_reaped,
                    "playwright_resolution_location": resolution
                        .map(|resolution| resolution.location.as_str())
                        .unwrap_or(""),
                    "playwright_module_path": resolution
                        .map(|resolution| resolution.module_path.as_str())
                        .unwrap_or(""),
                    "playwright_node_path": resolution
                        .and_then(|resolution| resolution.node_path.as_deref())
                        .unwrap_or(""),
                    "playwright_version": resolution
                        .map(|resolution| resolution.version.as_str())
                        .unwrap_or(""),
                }),
            );
        }
    }
}

pub(super) fn interaction_source_diagnostics(config: &Config) -> Vec<UnattachedRefDiagnostic> {
    let profile = config
        .profile_inference
        .map(|inference| inference.profile.to_string())
        .unwrap_or_else(|| config.profile.clone());
    route_bound_unattached_ref_diagnostics(
        &config.workspace_root,
        resolve_profile_runtime(&profile),
    )
}

pub(super) fn unattached_ref_diagnostic_labels(
    diagnostics: &[UnattachedRefDiagnostic],
) -> Vec<String> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.diagnostic.clone())
        .collect()
}

pub(super) fn annotate_interaction_evidence_with_source_diagnostics(
    path: &Path,
    diagnostics: &[UnattachedRefDiagnostic],
) {
    if diagnostics.is_empty() || !path.is_file() {
        return;
    }
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let Ok(mut value) = serde_json::from_str::<Value>(&text) else {
        return;
    };
    if !value.is_object() {
        return;
    }
    value["source_diagnostics"] = json!(unattached_ref_diagnostic_labels(diagnostics));
    value["unattached_ref_diagnostics"] = json!(diagnostics);
    if let Ok(text) = serde_json::to_string_pretty(&value) {
        let _ = std::fs::write(path, format!("{text}\n"));
    }
}

#[cfg(test)]
pub(super) fn inferred_required_capabilities(profile: &str, goal: &str) -> Vec<String> {
    resolve_profile_runtime(profile).required_capabilities(goal)
}

#[cfg(test)]
pub(super) fn inferred_required_evidence(
    profile: &str,
    goal: &str,
    required_capabilities: &[String],
) -> Vec<String> {
    resolve_profile_runtime(profile).required_evidence(goal, required_capabilities)
}

pub(super) fn runtime_required_evidence(
    runtime: &dyn ProfileRuntime,
    goal: &str,
    required_capabilities: &[String],
) -> Vec<String> {
    runtime.required_evidence(goal, required_capabilities)
}

#[cfg(test)]
pub(super) fn inferred_required_obligations(
    profile: &str,
    goal: &str,
    required_capabilities: &[String],
) -> Vec<String> {
    let profile_id = ProfileId::parse(profile);
    ProfileRuntimeRegistry::resolve(&profile_id).required_obligations(
        &profile_id,
        goal,
        required_capabilities,
    )
}

pub(super) fn run_profile_behavior_probe(
    config: &Config,
    profile: &str,
    goal: &str,
    required_capabilities: &[String],
    profile_report: &VerificationReport,
) -> ProfileBehaviorProbeReport {
    if !profile_report.is_pass() {
        return ProfileBehaviorProbeReport::pass();
    }
    let profile_id = ProfileId::parse(profile);
    match resolve_profile_runtime(profile).run_behavior_probe(
        &profile_id,
        &config.workspace_root,
        goal,
        required_capabilities,
        config.offline,
    ) {
        Ok(report) => {
            emit_profile_behavior_probe_event(config, profile, &report);
            report
        }
        Err(err) => {
            let report = ProfileBehaviorProbeReport {
                status: "failed",
                reasons: vec![format!("profile_behavior_probe_error: {err}")],
                evidence_path: None,
            };
            emit_profile_behavior_probe_event(config, profile, &report);
            report
        }
    }
}

pub(super) fn emit_profile_behavior_probe_event(
    config: &Config,
    profile: &str,
    report: &ProfileBehaviorProbeReport,
) {
    if report.status == "pass" && report.reasons.is_empty() && report.evidence_path.is_none() {
        return;
    }
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "profile_behavior_probe",
            "cycle_index": current_final_acceptance_cycle_index(),
            "profile": profile,
            "status": report.status,
            "ok": report.status == "pass",
            "reasons": report.reasons.clone(),
            "evidence_path": report.evidence_path.clone().unwrap_or_default(),
        }),
    );
}

pub(super) fn runtime_acceptance_repair_guidance(
    profile: &str,
    goal: &str,
    acceptance: &crate::minimal_loop::evidence::RuntimeAcceptanceReport,
) -> Vec<String> {
    let mut guidance = Vec::new();
    for evidence in &acceptance.missing_evidence {
        match evidence.as_str() {
            "restart_or_recoverable_state_evidence" => {
                guidance.push(crate::minimal_loop::feedback::capability_evidence_remedy_line(
                    evidence,
                ));
            }
            "persistence_evidence" => {
                let contract = crate::planner::profile::interaction_repair_contract(profile, goal);
                guidance.extend(resolve_profile_runtime(profile).interaction_repair_guidance(
                    "browser_interaction_failed:persistence_after_reload_reset",
                    &contract,
                ));
            }
            "live_preview_evidence" | "requested_content_evidence" => {
                guidance.push(TEXT_ECHO_REPAIR_REQUIREMENT.to_string())
            }
            "challenge_or_adversary_evidence" => guidance.push(
                "wire a reachable challenge/adversary entity into state evolution, not only a static label"
                    .to_string(),
            ),
            "failure_or_collision_evidence" => guidance.push(
                "wire a collision/failure conditional that transitions to a reachable failure state"
                    .to_string(),
            ),
            "score_or_progression_evidence" => guidance.push(
                "wire score/progression updates to meaningful state transitions, not only an isolated counter"
                    .to_string(),
            ),
            "stateful_update_evidence" => guidance.push(
                "mutate application state over time or in response to input"
                    .to_string(),
            ),
            "user_input_handler_evidence" => guidance.push(
                "wire keyboard, pointer, click, touch, or form handlers to gameplay state changes"
                    .to_string(),
            ),
            _ => {}
        }
    }
    for evidence in &acceptance.unverified_evidence {
        if evidence == "restart_or_recoverable_state_evidence:unverified:terminal_state_not_reached"
        {
            guidance.push(RESTART_PARTIAL_REPAIR_GUIDANCE.to_string());
        }
    }
    for weak in &acceptance.weak_evidence {
        if let Some(reason) = weak.split(':').next_back()
            && !reason.trim().is_empty()
        {
            guidance.push(reason.trim().to_string());
        }
    }
    for diagnostic in &acceptance.diagnostics {
        if let Some(path) = diagnostic.strip_prefix("route_unbound_capability_artifact:") {
            for evidence in &acceptance.missing_evidence {
                guidance.push(format!(
                    "For missing evidence {evidence}, {path} contains capability code but is not route-bound; import it from the route page, or consolidate into page.tsx and delete the dead component"
                ));
            }
        }
    }
    dedup_strings(guidance)
}

#[derive(Debug, Clone)]
pub(super) struct NextjsDevServerProbeSpec {
    pub(super) package_manager: String,
    pub(super) args: Vec<String>,
    pub(super) command_display: String,
    pub(super) port: u16,
    pub(super) route: String,
}

#[derive(Debug, Clone)]
pub(super) struct HttpProbeResult {
    pub(super) status: i64,
    pub(super) body_excerpt: String,
}

#[cfg(test)]
pub(super) fn run_nextjs_dev_route_probe(config: &Config, evidence_path: &Path) -> Value {
    run_nextjs_dev_route_probe_with_interaction_options(
        config,
        evidence_path,
        BrowserInteractionProbeOptions::default(),
        None,
    )
}

pub(super) fn run_nextjs_dev_route_probe_with_interaction_options(
    config: &Config,
    evidence_path: &Path,
    interaction_options: BrowserInteractionProbeOptions,
    requested_port: Option<u16>,
) -> Value {
    run_nextjs_dev_route_probe_with_runtime(
        config,
        evidence_path,
        dev_server_probe_runtime_enabled(config),
        cleanup_dev_server_child,
        interaction_options,
        requested_port,
    )
}

pub(super) type DevServerCleanupFn = fn(Child, &DevServerLogPaths) -> DevServerCleanup;

pub(super) fn run_nextjs_dev_route_probe_with_runtime(
    config: &Config,
    evidence_path: &Path,
    runtime_enabled: bool,
    cleanup_fn: DevServerCleanupFn,
    interaction_options: BrowserInteractionProbeOptions,
    requested_port: Option<u16>,
) -> Value {
    if !runtime_enabled {
        let failure_kind = if cfg!(test) {
            "browser_unavailable:dev_server_probe_disabled_in_tests"
        } else {
            "browser_unavailable:dev_server_probe_disabled"
        };
        emit_dev_server_unavailable_lifecycle(
            config,
            NEXTJS_DEV_SERVER_DEFAULT_PORT,
            DEV_SERVER_ROUTE,
            "",
            failure_kind,
            evidence_path,
        );
        return dev_server_unavailable_evidence(
            NEXTJS_DEV_SERVER_DEFAULT_PORT,
            DEV_SERVER_ROUTE,
            "",
            failure_kind,
            "",
        );
    }

    let spec = match load_nextjs_dev_server_probe_spec(&config.workspace_root, requested_port) {
        Ok(spec) => spec,
        Err(failure_kind) => {
            emit_dev_server_unavailable_lifecycle(
                config,
                NEXTJS_DEV_SERVER_DEFAULT_PORT,
                DEV_SERVER_ROUTE,
                "",
                &failure_kind,
                evidence_path,
            );
            return dev_server_unavailable_evidence(
                NEXTJS_DEV_SERVER_DEFAULT_PORT,
                DEV_SERVER_ROUTE,
                "",
                &failure_kind,
                "",
            );
        }
    };

    if localhost_port_accepts_connection(spec.port) {
        let owner = dev_server_port_owner(spec.port);
        if let Some(owner) = &owner
            && owner
                .pid
                .and_then(bounded_process::registered_server_child)
                .is_some()
        {
            let reaped = owner.pid.is_some_and(|pid| {
                bounded_process::reap_registered_server_child(
                    pid,
                    config.eval_events_path.as_deref(),
                    "readiness_port_in_use_retry",
                )
            });
            eval_events::emit(
                config.eval_events_path.as_deref(),
                json!({
                    "event": "dev_server_port_in_use_retry",
                    "port": spec.port,
                    "owner_pid": owner.pid,
                    "owner_command": owner.command,
                    "registered_child": true,
                    "reaped": reaped,
                }),
            );
            std::thread::sleep(Duration::from_millis(150));
        }
    }

    if localhost_port_accepts_connection(spec.port) {
        let owner = dev_server_port_owner(spec.port);
        let failure_kind = "port_in_use";
        let owner_text = owner
            .as_ref()
            .map(DevServerPortOwner::display)
            .unwrap_or_else(|| "unknown owner".to_string());
        let output_excerpt =
            dev_server_output_excerpt_for_port(failure_kind, &owner_text, spec.port);
        emit_dev_server_unavailable_lifecycle(
            config,
            spec.port,
            &spec.route,
            &spec.command_display,
            failure_kind,
            evidence_path,
        );
        return dev_server_unavailable_evidence(
            spec.port,
            &spec.route,
            &spec.command_display,
            failure_kind,
            &output_excerpt,
        );
    }

    let (logs, stdout_log, stderr_log) = match open_dev_server_log_files(evidence_path) {
        Ok(logs) => logs,
        Err(err) => {
            let failure_kind = "browser_unavailable:dev_server_log_open_failed";
            emit_dev_server_unavailable_lifecycle(
                config,
                spec.port,
                &spec.route,
                &spec.command_display,
                failure_kind,
                evidence_path,
            );
            return dev_server_unavailable_evidence(
                spec.port,
                &spec.route,
                &spec.command_display,
                failure_kind,
                &err.to_string(),
            );
        }
    };

    let mut command =
        verifier_env::normalized_command_at_root(&spec.package_manager, &config.workspace_root);
    command
        .args(&spec.args)
        .current_dir(&config.workspace_root)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout_log))
        .stderr(Stdio::from(stderr_log))
        .env("PORT", spec.port.to_string());
    let mut child = match bounded_process::spawn_child(&mut command) {
        Ok(child) => child,
        Err(err) => {
            let failure_kind = dev_server_spawn_failure_kind(&err);
            emit_dev_server_lifecycle_stage(
                config,
                "start",
                false,
                spec.port,
                &spec.route,
                &spec.command_display,
                Some(&failure_kind),
                None,
                evidence_path,
                None,
            );
            emit_dev_server_lifecycle_stage(
                config,
                "wait",
                false,
                spec.port,
                &spec.route,
                &spec.command_display,
                Some(&failure_kind),
                None,
                evidence_path,
                None,
            );
            emit_dev_server_lifecycle_stage(
                config,
                "probe",
                false,
                spec.port,
                &spec.route,
                &spec.command_display,
                Some(&failure_kind),
                None,
                evidence_path,
                None,
            );
            emit_dev_server_lifecycle_stage(
                config,
                "cleanup",
                true,
                spec.port,
                &spec.route,
                &spec.command_display,
                Some(&failure_kind),
                None,
                evidence_path,
                None,
            );
            return dev_server_unavailable_evidence(
                spec.port,
                &spec.route,
                &spec.command_display,
                &failure_kind,
                &err.to_string(),
            );
        }
    };

    let pid = child.id();
    bounded_process::register_server_child(
        &child,
        spec.command_display.clone(),
        format!(
            "final_acceptance_cycle_{}",
            current_final_acceptance_cycle_index()
        ),
        &config.workspace_root,
    );
    emit_dev_server_lifecycle_stage(
        config,
        "start",
        true,
        spec.port,
        &spec.route,
        &spec.command_display,
        None,
        None,
        evidence_path,
        Some(pid),
    );

    let deadline = Instant::now() + NEXTJS_DEV_SERVER_READY_TIMEOUT;
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(_status)) => {
                let output_excerpt = dev_server_logs_excerpt(&logs)
                    .unwrap_or_else(|| "dev server exited before readiness".to_string());
                let failure_kind = classify_dev_server_startup_failure(&output_excerpt)
                    .unwrap_or_else(|| "browser_unavailable:dev_server_exited".to_string());
                let failure_kind = classify_dev_server_env_conflict(&failure_kind, &output_excerpt);
                let output_excerpt =
                    dev_server_output_excerpt_for_port(&failure_kind, &output_excerpt, spec.port);
                emit_dev_server_lifecycle_stage(
                    config,
                    "wait",
                    false,
                    spec.port,
                    &spec.route,
                    &spec.command_display,
                    Some(&failure_kind),
                    None,
                    evidence_path,
                    Some(pid),
                );
                emit_dev_server_lifecycle_stage(
                    config,
                    "probe",
                    false,
                    spec.port,
                    &spec.route,
                    &spec.command_display,
                    Some(&failure_kind),
                    None,
                    evidence_path,
                    Some(pid),
                );
                let evidence = dev_server_unavailable_evidence(
                    spec.port,
                    &spec.route,
                    &spec.command_display,
                    &failure_kind,
                    &output_excerpt,
                );
                write_release_evidence_json(evidence_path, &evidence);
                let cleanup = cleanup_registered_dev_server_child(cleanup_fn, child, &logs);
                emit_dev_server_cleanup_lifecycle_stage(
                    config,
                    cleanup.ok,
                    spec.port,
                    &spec.route,
                    &spec.command_display,
                    Some(&failure_kind),
                    None,
                    evidence_path,
                    Some(pid),
                    &cleanup,
                );
                return evidence;
            }
            Ok(None) => {}
            Err(err) => {
                let failure_kind = "browser_unavailable:dev_server_status_unreadable";
                let log_excerpt = dev_server_logs_excerpt(&logs).unwrap_or_default();
                let combined = format!("{} {}", err, log_excerpt);
                let failure_kind = classify_dev_server_env_conflict(failure_kind, &combined);
                let output_excerpt =
                    dev_server_output_excerpt_for_port(&failure_kind, &combined, spec.port);
                emit_dev_server_lifecycle_stage(
                    config,
                    "wait",
                    false,
                    spec.port,
                    &spec.route,
                    &spec.command_display,
                    Some(&failure_kind),
                    None,
                    evidence_path,
                    Some(pid),
                );
                emit_dev_server_lifecycle_stage(
                    config,
                    "probe",
                    false,
                    spec.port,
                    &spec.route,
                    &spec.command_display,
                    Some(&failure_kind),
                    None,
                    evidence_path,
                    Some(pid),
                );
                let evidence = dev_server_unavailable_evidence(
                    spec.port,
                    &spec.route,
                    &spec.command_display,
                    &failure_kind,
                    &output_excerpt,
                );
                write_release_evidence_json(evidence_path, &evidence);
                let cleanup = cleanup_registered_dev_server_child(cleanup_fn, child, &logs);
                emit_dev_server_cleanup_lifecycle_stage(
                    config,
                    cleanup.ok,
                    spec.port,
                    &spec.route,
                    &spec.command_display,
                    Some(&failure_kind),
                    None,
                    evidence_path,
                    Some(pid),
                    &cleanup,
                );
                return evidence;
            }
        }

        match http_get_local_route(spec.port, &spec.route) {
            Ok(response) => {
                emit_dev_server_lifecycle_stage(
                    config,
                    "wait",
                    true,
                    spec.port,
                    &spec.route,
                    &spec.command_display,
                    None,
                    Some(response.status),
                    evidence_path,
                    Some(pid),
                );
                let failure_kind =
                    classify_dev_route_failure_kind(response.status, &response.body_excerpt);
                let probe_ok = failure_kind.is_none();
                let log_excerpt = dev_server_logs_excerpt(&logs).unwrap_or_default();
                let failure_kind = failure_kind.map(|kind| {
                    let combined = format!("{}\n{}", response.body_excerpt, log_excerpt);
                    classify_dev_server_env_conflict(&kind, &combined)
                });
                let body_excerpt = failure_kind
                    .as_deref()
                    .map(|kind| {
                        dev_server_output_excerpt_for_port(kind, &response.body_excerpt, spec.port)
                    })
                    .unwrap_or_else(|| response.body_excerpt.clone());
                let output_excerpt = failure_kind
                    .as_deref()
                    .map(|kind| dev_server_output_excerpt_for_port(kind, &log_excerpt, spec.port))
                    .unwrap_or_else(|| log_excerpt.clone());
                emit_dev_server_lifecycle_stage(
                    config,
                    "probe",
                    probe_ok && failure_kind.is_none(),
                    spec.port,
                    &spec.route,
                    &spec.command_display,
                    failure_kind.as_deref(),
                    Some(response.status),
                    evidence_path,
                    Some(pid),
                );
                let evidence = if let Some(failure_kind) = failure_kind.as_deref() {
                    dev_server_failed_evidence(
                        spec.port,
                        &spec.route,
                        &spec.command_display,
                        response.status,
                        failure_kind,
                        &body_excerpt,
                        &output_excerpt,
                    )
                } else {
                    dev_server_passed_evidence(
                        spec.port,
                        &spec.route,
                        &spec.command_display,
                        response.status,
                        &response.body_excerpt,
                    )
                };
                write_release_evidence_json(evidence_path, &evidence);
                if failure_kind.is_none() {
                    let interaction_path = evidence_path.with_file_name("browser-interaction.json");
                    let run_dir = evidence_path.parent().unwrap_or(&config.workspace_root);
                    let interaction =
                        interaction_probe::probe_browser_interaction_against_running_server_with_options(
                            &config.workspace_root,
                            spec.port,
                            run_dir,
                            &interaction_path,
                            Duration::from_secs(120),
                            interaction_options,
                        );
                    emit_browser_interaction_probe_event(config, &interaction);
                }
                let cleanup = cleanup_registered_dev_server_child(cleanup_fn, child, &logs);
                emit_dev_server_cleanup_lifecycle_stage(
                    config,
                    cleanup.ok,
                    spec.port,
                    &spec.route,
                    &spec.command_display,
                    failure_kind.as_deref(),
                    Some(response.status),
                    evidence_path,
                    Some(pid),
                    &cleanup,
                );
                return evidence;
            }
            Err(_) => {
                std::thread::sleep(NEXTJS_DEV_SERVER_WAIT_INTERVAL);
            }
        }
    }

    let failure_kind = "startup_timeout";
    let log_excerpt = dev_server_logs_excerpt(&logs).unwrap_or_default();
    let failure_kind = classify_dev_server_env_conflict(failure_kind, &log_excerpt);
    let output_excerpt = dev_server_output_excerpt_for_port(&failure_kind, &log_excerpt, spec.port);
    emit_dev_server_lifecycle_stage(
        config,
        "wait",
        false,
        spec.port,
        &spec.route,
        &spec.command_display,
        Some(&failure_kind),
        None,
        evidence_path,
        Some(pid),
    );
    emit_dev_server_lifecycle_stage(
        config,
        "probe",
        false,
        spec.port,
        &spec.route,
        &spec.command_display,
        Some(&failure_kind),
        None,
        evidence_path,
        Some(pid),
    );
    let evidence = dev_server_unavailable_evidence(
        spec.port,
        &spec.route,
        &spec.command_display,
        &failure_kind,
        &output_excerpt,
    );
    write_release_evidence_json(evidence_path, &evidence);
    let cleanup = cleanup_registered_dev_server_child(cleanup_fn, child, &logs);
    emit_dev_server_cleanup_lifecycle_stage(
        config,
        cleanup.ok,
        spec.port,
        &spec.route,
        &spec.command_display,
        Some(&failure_kind),
        None,
        evidence_path,
        Some(pid),
        &cleanup,
    );
    evidence
}

pub(super) fn dev_server_probe_runtime_enabled(config: &Config) -> bool {
    if env_flag_is_false("COMMANDAGENT_DEV_SERVER_PROBE") {
        return false;
    }
    if cfg!(test)
        && !env_flag_is_true("COMMANDAGENT_TEST_DEV_SERVER_PROBE")
        && !config
            .workspace_root
            .join(".anvil")
            .join("enable-dev-server-probe-tests")
            .is_file()
    {
        return false;
    }
    true
}

pub(super) fn env_flag_is_false(name: &str) -> bool {
    crate::env_compat::var(name)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "0" | "false" | "no" | "off"
            )
        })
        .unwrap_or(false)
}

pub(super) fn env_flag_is_true(name: &str) -> bool {
    crate::env_compat::var(name)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

pub(super) fn load_nextjs_dev_server_probe_spec(
    root: &Path,
    requested_port: Option<u16>,
) -> Result<NextjsDevServerProbeSpec, String> {
    let manifest_path = root.join("package.json");
    let text = std::fs::read_to_string(&manifest_path)
        .map_err(|_| "browser_unavailable:package_json_missing".to_string())?;
    let value = serde_json::from_str::<Value>(&text)
        .map_err(|_| "browser_unavailable:package_json_invalid".to_string())?;
    let script = value
        .get("scripts")
        .and_then(Value::as_object)
        .and_then(|scripts| scripts.get("dev"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|script| !script.is_empty())
        .ok_or_else(|| "browser_unavailable:dev_script_missing".to_string())?;
    if !script_contains_next_dev(script) {
        return Err("browser_unavailable:dev_script_not_next_dev".to_string());
    }
    let port = requested_port.unwrap_or(NEXTJS_DEV_SERVER_DEFAULT_PORT);
    let (package_manager, args) = package_manager_dev_command(root);
    let command_display = std::iter::once(package_manager.as_str())
        .chain(args.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ");
    Ok(NextjsDevServerProbeSpec {
        package_manager,
        args,
        command_display,
        port,
        route: DEV_SERVER_ROUTE.to_string(),
    })
}

pub(super) fn script_contains_next_dev(script: &str) -> bool {
    let lower = script.to_ascii_lowercase();
    lower.contains("next") && lower.contains("dev")
}

pub(super) fn package_manager_dev_command(root: &Path) -> (String, Vec<String>) {
    if root.join("pnpm-lock.yaml").is_file() {
        return (
            "pnpm".to_string(),
            vec!["run".to_string(), "dev".to_string()],
        );
    }
    if root.join("yarn.lock").is_file() {
        return ("yarn".to_string(), vec!["dev".to_string()]);
    }
    (
        "npm".to_string(),
        vec!["run".to_string(), "dev".to_string()],
    )
}

pub(super) fn localhost_port_accepts_connection(port: u16) -> bool {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    TcpStream::connect_timeout(&addr, Duration::from_millis(150)).is_ok()
}

pub(super) fn http_get_local_route(port: u16, route: &str) -> Result<HttpProbeResult, String> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = TcpStream::connect_timeout(&addr, NEXTJS_DEV_SERVER_CONNECT_TIMEOUT)
        .map_err(|err| err.to_string())?;
    let _ = stream.set_read_timeout(Some(NEXTJS_DEV_SERVER_CONNECT_TIMEOUT));
    let _ = stream.set_write_timeout(Some(NEXTJS_DEV_SERVER_CONNECT_TIMEOUT));
    let path = if route.starts_with('/') {
        route.to_string()
    } else {
        format!("/{route}")
    };
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\nUser-Agent: commandagent-dev-server-probe\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|err| err.to_string())?;
    let mut response = Vec::new();
    let mut buffer = [0u8; 4096];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                response.extend_from_slice(&buffer[..n]);
                if response.len() >= 32_768 {
                    break;
                }
            }
            Err(err)
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                break;
            }
            Err(err) => return Err(err.to_string()),
        }
    }
    let response_text = String::from_utf8_lossy(&response).to_string();
    let status_line = response_text
        .lines()
        .next()
        .ok_or_else(|| "empty_http_response".to_string())?;
    let status = status_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| "http_status_missing".to_string())?
        .parse::<i64>()
        .map_err(|_| "http_status_invalid".to_string())?;
    let body = response_text
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .unwrap_or(&response_text);
    Ok(HttpProbeResult {
        status,
        body_excerpt: eval_events::body_snippet(body),
    })
}

pub(super) fn dev_server_spawn_failure_kind(err: &std::io::Error) -> String {
    match err.kind() {
        std::io::ErrorKind::NotFound => "browser_unavailable:dev_server_command_missing",
        std::io::ErrorKind::PermissionDenied => "browser_unavailable:dev_server_command_denied",
        _ => "browser_unavailable:dev_server_spawn_failed",
    }
    .to_string()
}

pub(super) fn classify_dev_server_startup_failure(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    if lower.contains("eaddrinuse") || lower.contains("address already in use") {
        return Some("port_in_use".to_string());
    }
    if lower.contains("eacces")
        || lower.contains("permission denied")
        || lower.contains("operation not permitted")
    {
        return Some("bind_denied".to_string());
    }
    if tailwind_dev_pipeline_failure(&lower) {
        return Some("tailwind_dev_pipeline_failure".to_string());
    }
    None
}

pub(super) fn classify_dev_route_failure_kind(status: i64, body_excerpt: &str) -> Option<String> {
    if status < 400 {
        return None;
    }
    let lower = body_excerpt.to_ascii_lowercase();
    if tailwind_dev_pipeline_failure(&lower) {
        return Some("tailwind_dev_pipeline_failure".to_string());
    }
    Some(format!("http_{status}"))
}

pub(super) fn classify_dev_server_env_conflict(failure_kind: &str, output: &str) -> String {
    if verifier_env::is_env_node_env_conflict_output(output) {
        verifier_env::ENV_NODE_ENV_CONFLICT_KIND.to_string()
    } else {
        failure_kind.to_string()
    }
}

#[cfg(test)]
pub(super) fn dev_server_output_excerpt(failure_kind: &str, output: &str) -> String {
    dev_server_output_excerpt_for_port(failure_kind, output, NEXTJS_DEV_SERVER_DEFAULT_PORT)
}

pub(super) fn dev_server_output_excerpt_for_port(
    failure_kind: &str,
    output: &str,
    port: u16,
) -> String {
    if failure_kind == verifier_env::ENV_NODE_ENV_CONFLICT_KIND {
        verifier_env::with_env_node_env_remediation(output)
    } else if failure_kind == "port_in_use" {
        port_in_use_remediation(output, port)
    } else {
        output.to_string()
    }
}

pub(super) fn port_in_use_remediation(output: &str, port: u16) -> String {
    let remediation = format!(
        "Port {port} is already accepting connections. This may be a leftover dev server from a previous run. Inspect it with `lsof -nP -iTCP:{port} -sTCP:LISTEN` and stop the stale process before retrying."
    );
    if output.trim().is_empty() {
        remediation
    } else {
        format!("{output}\n{remediation}")
    }
}

#[derive(Debug, Clone)]
pub(super) struct DevServerPortOwner {
    pub(super) pid: Option<u32>,
    pub(super) command: String,
}

impl DevServerPortOwner {
    pub(super) fn display(&self) -> String {
        match (self.pid, self.command.trim()) {
            (Some(pid), command) if !command.is_empty() => format!("pid {pid} ({command})"),
            (Some(pid), _) => format!("pid {pid}"),
            (None, command) if !command.is_empty() => command.to_string(),
            (None, _) => "unknown owner".to_string(),
        }
    }
}

pub(super) fn dev_server_port_owner(port: u16) -> Option<DevServerPortOwner> {
    let port_spec = format!("-iTCP:{port}");
    let mut command = std::process::Command::new("lsof");
    command
        .args(["-nP", &port_spec, "-sTCP:LISTEN", "-F", "pc"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = bounded_process::run_with_timeout(&mut command, Duration::from_secs(2)).ok()?;
    if !output.success() {
        return None;
    }
    parse_dev_server_port_owner(&String::from_utf8(output.stdout).ok()?)
}

pub(super) fn parse_dev_server_port_owner(text: &str) -> Option<DevServerPortOwner> {
    let mut pid = None;
    let mut command = None;
    for line in text.lines() {
        if let Some(raw) = line.strip_prefix('p') {
            pid = raw.parse::<u32>().ok();
        } else if let Some(raw) = line.strip_prefix('c') {
            command = Some(raw.to_string());
        }
        if pid.is_some() && command.is_some() {
            break;
        }
    }
    pid.or_else(|| command.as_ref().map(|_| 0))
        .map(|pid_value| DevServerPortOwner {
            pid: (pid_value != 0).then_some(pid_value),
            command: command.unwrap_or_default(),
        })
}

pub(super) fn tailwind_dev_pipeline_failure(lower_text: &str) -> bool {
    lower_text.contains("@tailwind")
        && (lower_text.contains("module parse failed")
            || lower_text.contains("unexpected character")
            || lower_text.contains("postcss")
            || lower_text.contains("tailwind"))
}

#[derive(Debug, Clone)]
pub(super) struct DevServerLogPaths {
    pub(super) stdout: PathBuf,
    pub(super) stderr: PathBuf,
}

pub(super) fn open_dev_server_log_files(
    evidence_path: &Path,
) -> std::io::Result<(DevServerLogPaths, std::fs::File, std::fs::File)> {
    let dir = evidence_path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;
    let paths = DevServerLogPaths {
        stdout: dir.join("dev-server.out"),
        stderr: dir.join("dev-server.err"),
    };
    let stdout = std::fs::File::create(&paths.stdout)?;
    let stderr = std::fs::File::create(&paths.stderr)?;
    Ok((paths, stdout, stderr))
}

pub(super) fn dev_server_logs_excerpt(paths: &DevServerLogPaths) -> Option<String> {
    let stdout = read_dev_server_log_excerpt(&paths.stdout).unwrap_or_default();
    let stderr = read_dev_server_log_excerpt(&paths.stderr).unwrap_or_default();
    let combined = format!("{stdout}\n{stderr}");
    let excerpt = eval_events::body_snippet(combined.trim());
    if excerpt.trim().is_empty() {
        None
    } else {
        Some(excerpt)
    }
}

pub(super) fn read_dev_server_log_excerpt(path: &Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    let start = bytes.len().saturating_sub(DEV_SERVER_LOG_EXCERPT_BYTES);
    Ok(String::from_utf8_lossy(&bytes[start..]).to_string())
}

#[derive(Debug)]
pub(super) struct DevServerCleanup {
    pub(super) ok: bool,
    pub(super) failure_kind: Option<String>,
    pub(super) output_excerpt: String,
}

pub(super) fn cleanup_dev_server_child(
    mut child: Child,
    logs: &DevServerLogPaths,
) -> DevServerCleanup {
    #[cfg(unix)]
    {
        cleanup_dev_server_child_unix(&mut child, logs)
    }
    #[cfg(not(unix))]
    {
        cleanup_dev_server_child_non_unix(&mut child, logs)
    }
}

pub(super) fn cleanup_registered_dev_server_child(
    cleanup_fn: DevServerCleanupFn,
    child: Child,
    logs: &DevServerLogPaths,
) -> DevServerCleanup {
    let pid = child.id();
    let cleanup = cleanup_fn(child, logs);
    bounded_process::unregister_server_child(pid);
    cleanup
}

#[cfg(unix)]
pub(super) fn cleanup_dev_server_child_unix(
    child: &mut Child,
    logs: &DevServerLogPaths,
) -> DevServerCleanup {
    let pid = child.id();
    let mut notes = Vec::new();
    if let Err(err) = signal_dev_server_process_group(pid, libc::SIGTERM) {
        notes.push(format!("SIGTERM process group failed: {err}"));
    }
    match wait_for_dev_server_process_group_exit(
        child,
        pid,
        Instant::now() + DEV_SERVER_CLEANUP_TERM_TIMEOUT,
    ) {
        Ok(true) => {
            return DevServerCleanup {
                ok: true,
                failure_kind: None,
                output_excerpt: dev_server_logs_excerpt(logs).unwrap_or_default(),
            };
        }
        Ok(false) => {}
        Err(err) => notes.push(format!("wait after SIGTERM failed: {err}")),
    }

    if let Err(err) = signal_dev_server_process_group(pid, libc::SIGKILL) {
        notes.push(format!("SIGKILL process group failed: {err}"));
    }
    match wait_for_dev_server_process_group_exit(
        child,
        pid,
        Instant::now() + DEV_SERVER_CLEANUP_KILL_TIMEOUT,
    ) {
        Ok(true) => DevServerCleanup {
            ok: true,
            failure_kind: None,
            output_excerpt: dev_server_logs_excerpt(logs).unwrap_or_default(),
        },
        Ok(false) => DevServerCleanup {
            ok: false,
            failure_kind: Some("dev_server_cleanup_timeout".to_string()),
            output_excerpt: cleanup_timeout_excerpt(logs, &notes),
        },
        Err(err) => {
            notes.push(format!("wait after SIGKILL failed: {err}"));
            DevServerCleanup {
                ok: false,
                failure_kind: Some("dev_server_cleanup_timeout".to_string()),
                output_excerpt: cleanup_timeout_excerpt(logs, &notes),
            }
        }
    }
}

#[cfg(unix)]
pub(super) fn signal_dev_server_process_group(
    pid: u32,
    signal: libc::c_int,
) -> std::io::Result<()> {
    let pid =
        i32::try_from(pid).map_err(|_| std::io::Error::other("child pid does not fit pid_t"))?;
    // SAFETY: `kill` is called with a process-group id derived from the child
    // pid returned by `std::process::Child` and a libc signal constant.
    let rc = unsafe { libc::kill(-pid, signal) };
    if rc == 0 {
        return Ok(());
    }
    let err = std::io::Error::last_os_error();
    if err.raw_os_error() == Some(libc::ESRCH) {
        Ok(())
    } else {
        Err(err)
    }
}

#[cfg(unix)]
pub(super) fn wait_for_dev_server_process_group_exit(
    child: &mut Child,
    pid: u32,
    deadline: Instant,
) -> std::io::Result<bool> {
    let mut child_exited = false;
    loop {
        if !child_exited && child.try_wait()?.is_some() {
            let _ = child.wait();
            child_exited = true;
        }
        if child_exited && !dev_server_process_group_exists(pid) {
            return Ok(true);
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(unix)]
pub(super) fn dev_server_process_group_exists(pgid: u32) -> bool {
    let Ok(pgid) = i32::try_from(pgid) else {
        return false;
    };
    // SAFETY: signal 0 performs existence/permission checking only, using
    // a process-group id derived from a child process spawned by this probe.
    let rc = unsafe { libc::kill(-pgid, 0) };
    if rc == 0 {
        return true;
    }
    let err = std::io::Error::last_os_error();
    err.raw_os_error() != Some(libc::ESRCH)
}

#[cfg(not(unix))]
pub(super) fn cleanup_dev_server_child_non_unix(
    child: &mut Child,
    logs: &DevServerLogPaths,
) -> DevServerCleanup {
    let _ = child.kill();
    match wait_for_dev_server_child_exit(child, Instant::now() + DEV_SERVER_CLEANUP_TERM_TIMEOUT) {
        Ok(true) => DevServerCleanup {
            ok: true,
            failure_kind: None,
            output_excerpt: dev_server_logs_excerpt(logs).unwrap_or_default(),
        },
        Ok(false) | Err(_) => DevServerCleanup {
            ok: false,
            failure_kind: Some("dev_server_cleanup_timeout".to_string()),
            output_excerpt: cleanup_timeout_excerpt(logs, &[]),
        },
    }
}

#[cfg(not(unix))]
pub(super) fn wait_for_dev_server_child_exit(
    child: &mut Child,
    deadline: Instant,
) -> std::io::Result<bool> {
    loop {
        if child.try_wait()?.is_some() {
            let _ = child.wait();
            return Ok(true);
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

pub(super) fn cleanup_timeout_excerpt(logs: &DevServerLogPaths, notes: &[String]) -> String {
    let mut parts = notes.to_vec();
    if let Some(log_excerpt) = dev_server_logs_excerpt(logs) {
        parts.push(log_excerpt);
    }
    if parts.is_empty() {
        "dev server cleanup timed out".to_string()
    } else {
        eval_events::body_snippet(&parts.join("\n"))
    }
}

pub(super) fn cleanup_stage_failure_kind<'a>(
    original_failure_kind: Option<&'a str>,
    cleanup: &'a DevServerCleanup,
) -> Option<&'a str> {
    if !cleanup.ok {
        cleanup.failure_kind.as_deref().or(original_failure_kind)
    } else {
        original_failure_kind
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_dev_server_cleanup_lifecycle_stage(
    config: &Config,
    ok: bool,
    port: u16,
    route: &str,
    command: &str,
    failure_kind: Option<&str>,
    http_status: Option<i64>,
    evidence_path: &Path,
    pid: Option<u32>,
    cleanup: &DevServerCleanup,
) {
    let stage_failure_kind = cleanup_stage_failure_kind(failure_kind, cleanup);
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "dev_server_lifecycle",
            "cycle_index": current_final_acceptance_cycle_index(),
            "profile": "nextjs",
            "stage": "cleanup",
            "ok": ok,
            "port": port,
            "route": route,
            "command": command,
            "failure_kind": stage_failure_kind.unwrap_or(""),
            "http_status": http_status,
            "pid": pid,
            "evidence_path": evidence_path.display().to_string(),
            "output_excerpt": eval_events::body_snippet(&cleanup.output_excerpt),
            "lifecycle_stages": DEV_SERVER_LIFECYCLE_STAGES,
            "probe_environment": dev_server_probe_environment(port),
        }),
    );
}

pub(super) fn emit_dev_server_unavailable_lifecycle(
    config: &Config,
    port: u16,
    route: &str,
    command: &str,
    failure_kind: &str,
    evidence_path: &Path,
) {
    emit_dev_server_lifecycle_stage(
        config,
        "start",
        false,
        port,
        route,
        command,
        Some(failure_kind),
        None,
        evidence_path,
        None,
    );
    emit_dev_server_lifecycle_stage(
        config,
        "wait",
        false,
        port,
        route,
        command,
        Some(failure_kind),
        None,
        evidence_path,
        None,
    );
    emit_dev_server_lifecycle_stage(
        config,
        "probe",
        false,
        port,
        route,
        command,
        Some(failure_kind),
        None,
        evidence_path,
        None,
    );
    emit_dev_server_lifecycle_stage(
        config,
        "cleanup",
        true,
        port,
        route,
        command,
        Some(failure_kind),
        None,
        evidence_path,
        None,
    );
}

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_dev_server_lifecycle_stage(
    config: &Config,
    stage: &str,
    ok: bool,
    port: u16,
    route: &str,
    command: &str,
    failure_kind: Option<&str>,
    http_status: Option<i64>,
    evidence_path: &Path,
    pid: Option<u32>,
) {
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "dev_server_lifecycle",
            "cycle_index": current_final_acceptance_cycle_index(),
            "profile": "nextjs",
            "stage": stage,
            "ok": ok,
            "port": port,
            "route": route,
            "command": command,
            "failure_kind": failure_kind.unwrap_or(""),
            "http_status": http_status,
            "pid": pid,
            "evidence_path": evidence_path.display().to_string(),
            "lifecycle_stages": DEV_SERVER_LIFECYCLE_STAGES,
            "probe_environment": dev_server_probe_environment(port),
        }),
    );
}

pub(super) fn dev_server_probe_environment(port: u16) -> Value {
    json!({
        "NODE_ENV": "",
        "NODE_OPTIONS": "",
        "NEXT_TELEMETRY_DISABLED": "1",
        "PORT": port.to_string(),
        "host_env_contamination": verifier_env::host_env_contamination(),
        "COMMANDAGENT_DEV_SERVER_PROBE": crate::env_compat::var("COMMANDAGENT_DEV_SERVER_PROBE").unwrap_or_default(),
        "COMMANDAGENT_TEST_DEV_SERVER_PROBE": crate::env_compat::var("COMMANDAGENT_TEST_DEV_SERVER_PROBE").unwrap_or_default(),
    })
}

pub(super) fn dev_server_unavailable_evidence(
    port: u16,
    route: &str,
    command: &str,
    failure_kind: &str,
    output_excerpt: &str,
) -> Value {
    json!({
        "status": "unavailable",
        "browser_failure_kind": failure_kind,
        "failure_kind": failure_kind,
        "dev_server": {
            "profile": "nextjs",
            "port": port,
            "route": route,
            "command": command,
            "failure_kind": failure_kind,
            "output_excerpt": eval_events::body_snippet(output_excerpt),
            "lifecycle_stages": DEV_SERVER_LIFECYCLE_STAGES,
            "probe_environment": dev_server_probe_environment(port),
        }
    })
}

pub(super) fn dev_server_failed_evidence(
    port: u16,
    route: &str,
    command: &str,
    http_status: i64,
    failure_kind: &str,
    body_excerpt: &str,
    output_excerpt: &str,
) -> Value {
    let mut value = json!({
        "status": "failed",
        "ok": false,
        "http_status": http_status,
        "route_rendered": false,
        "browser_failure_kind": failure_kind,
        "failure_kind": failure_kind,
        "body_excerpt": eval_events::body_snippet(body_excerpt),
        "dev_server": {
            "profile": "nextjs",
            "port": port,
            "route": route,
            "command": command,
            "failure_kind": failure_kind,
            "output_excerpt": eval_events::body_snippet(output_excerpt),
            "lifecycle_stages": DEV_SERVER_LIFECYCLE_STAGES,
            "probe_environment": dev_server_probe_environment(port),
        }
    });
    add_surface_markers_to_evidence(&mut value, body_excerpt);
    value
}

pub(super) fn dev_server_passed_evidence(
    port: u16,
    route: &str,
    command: &str,
    http_status: i64,
    body_excerpt: &str,
) -> Value {
    let mut value = json!({
        "status": "ready",
        "ok": true,
        "http_status": http_status,
        "route_rendered": true,
        "dev_server": {
            "profile": "nextjs",
            "port": port,
            "route": route,
            "command": command,
            "body_excerpt": eval_events::body_snippet(body_excerpt),
            "lifecycle_stages": DEV_SERVER_LIFECYCLE_STAGES,
            "probe_environment": dev_server_probe_environment(port),
        }
    });
    add_surface_markers_to_evidence(&mut value, body_excerpt);
    value
}

pub(super) fn add_surface_markers_to_evidence(value: &mut Value, body_excerpt: &str) {
    let markers = html_surface_markers_json(body_excerpt);
    for key in [
        "ssr_has_canvas",
        "ssr_interactive_control_count",
        "has_canvas",
        "interactive_control_count",
        "title_text_excerpt",
        "surface_marker_authority",
        "route_rendered_quality",
    ] {
        value[key] = markers.get(key).cloned().unwrap_or(Value::Null);
    }
}

pub(super) fn release_recovery_failure_kind(
    release_gate: &ReleaseGateSummary,
    final_acceptance_status: &str,
    primary_reason: &str,
) -> String {
    if release_gate.status == "partial" {
        if release_gate
            .reasons
            .iter()
            .any(|reason| reason.contains("interaction_unverified:terminal_state_not_reached"))
        {
            return "interaction_unverified_terminal_state_not_reached".to_string();
        }
        if release_gate
            .reasons
            .iter()
            .any(|reason| reason.contains("interaction_unverified:probe_unavailable"))
        {
            return "interaction_unverified_probe_unavailable".to_string();
        }
        if release_gate
            .reasons
            .iter()
            .any(|reason| reason.contains("browser_readiness_or_interaction_evidence_required"))
            || release_gate
                .browser_readiness_status
                .starts_with("unavailable:")
            || release_gate
                .browser_readiness_status
                .contains("browser_readiness_evidence_missing")
            || release_gate
                .browser_readiness_status
                .contains("browser_render_evidence_missing")
        {
            return "browser_readiness_missing".to_string();
        }
        if release_gate
            .interaction_evidence_status
            .contains("interaction_evidence_missing")
        {
            return "browser_interaction_evidence_missing".to_string();
        }
        return "release_gate_partial".to_string();
    }
    if release_gate.status == "failed" {
        if release_gate
            .browser_readiness_status
            .contains(verifier_env::ENV_NODE_ENV_CONFLICT_KIND)
            || release_gate
                .reasons
                .iter()
                .any(|reason| reason.contains(verifier_env::ENV_NODE_ENV_CONFLICT_KIND))
        {
            return verifier_env::ENV_NODE_ENV_CONFLICT_KIND.to_string();
        }
        if release_gate
            .browser_readiness_status
            .contains("tailwind_dev_pipeline_failure")
        {
            return "tailwind_dev_pipeline_failure".to_string();
        }
        if release_gate.browser_readiness_status.starts_with("failed:")
            || release_gate
                .reasons
                .iter()
                .any(|reason| reason.contains("browser_readiness_failed"))
        {
            return "browser_readiness_failed".to_string();
        }
        if release_gate
            .interaction_evidence_status
            .starts_with("failed:")
            || release_gate
                .reasons
                .iter()
                .any(|reason| reason.contains("browser_interaction_failed"))
        {
            return "browser_interaction_failed".to_string();
        }
        return "release_gate_failed".to_string();
    }
    if final_acceptance_status == "partial" {
        "final_acceptance_partial".to_string()
    } else if primary_reason == "ok" {
        "final_acceptance_recovery_required".to_string()
    } else {
        "final_acceptance_failed".to_string()
    }
}

pub(super) fn app_behavior_probe_failure_kind(reason: &str) -> Option<String> {
    let lower = reason.to_ascii_lowercase();
    APP_BEHAVIOR_PROBE_FAILURE_KINDS
        .iter()
        .find(|kind| lower.contains(**kind))
        .map(|kind| format!("browser_interaction_failed:{kind}"))
}

pub(super) fn release_recovery_failure_evidence(
    profile: &str,
    goal: &str,
    release_gate: &ReleaseGateSummary,
    final_acceptance_status: &str,
    primary_reason: &str,
    runtime_acceptance: Option<&crate::minimal_loop::evidence::RuntimeAcceptanceReport>,
) -> Vec<String> {
    let mut evidence = Vec::new();
    evidence.push(format!(
        "failed acceptance layer: {}",
        release_recovery_acceptance_layer(release_gate, final_acceptance_status)
    ));
    evidence.push(format!(
        "final acceptance status: {final_acceptance_status}"
    ));
    evidence.push(format!("release gate status: {}", release_gate.status));
    if primary_reason != "ok" {
        evidence.push(format!("primary reason: {primary_reason}"));
    }
    evidence.extend(
        release_gate
            .reasons
            .iter()
            .map(|reason| format!("release gate reason: {reason}")),
    );
    evidence.push(format!(
        "browser readiness: {}",
        release_gate.browser_readiness_status
    ));
    if release_gate
        .browser_readiness_status
        .contains(verifier_env::ENV_NODE_ENV_CONFLICT_KIND)
        || release_gate
            .reasons
            .iter()
            .any(|reason| reason.contains(verifier_env::ENV_NODE_ENV_CONFLICT_KIND))
    {
        evidence.push(format!(
            "host environment remediation: {}",
            verifier_env::ENV_NODE_ENV_REMEDIATION
        ));
    }
    if !release_gate.browser_readiness_evidence_path.is_empty() {
        evidence.push(format!(
            "browser readiness evidence: {}",
            release_gate.browser_readiness_evidence_path
        ));
        evidence.extend(
            compile_errors_from_release_evidence_path(
                &release_gate.browser_readiness_evidence_path,
            )
            .into_iter()
            .flat_map(|error| compile_error_repair_guidance(&[error]))
            .map(|line| format!("fix_compile_error: {line}")),
        );
    }
    evidence.push(format!(
        "interaction evidence: {}",
        release_gate.interaction_evidence_status
    ));
    if !release_gate.interaction_evidence_path.is_empty() {
        evidence.push(format!(
            "interaction evidence path: {}",
            release_gate.interaction_evidence_path
        ));
        evidence.extend(interaction_probe_failure_evidence_lines(
            profile,
            goal,
            &release_gate.interaction_evidence_path,
        ));
    }
    if let Some(report) = runtime_acceptance {
        evidence.extend(
            report
                .missing_evidence
                .iter()
                .map(|item| format!("missing runtime evidence: {item}")),
        );
        evidence.extend(
            runtime_acceptance_repair_guidance(profile, goal, report)
                .into_iter()
                .map(|item| format!("runtime repair guidance: {item}")),
        );
        evidence.extend(
            report
                .unverified_evidence
                .iter()
                .map(|item| format!("unverified runtime evidence: {item}")),
        );
        evidence.extend(
            report
                .missing_obligations
                .iter()
                .map(|item| format!("missing runtime obligation: {item}")),
        );
        evidence.extend(
            report
                .inconclusive_reasons
                .iter()
                .map(|item| format!("runtime acceptance inconclusive: {item}")),
        );
    }
    dedup_strings(evidence)
}

pub(super) fn interaction_probe_failure_evidence_lines(
    profile: &str,
    goal: &str,
    path: &str,
) -> Vec<String> {
    let Some(value) = std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .filter(Value::is_object)
    else {
        return Vec::new();
    };
    let mut lines = Vec::new();
    let failure_kind = raw_text_field_deep(&value, &["failure_kind", "browser_failure_kind"])
        .map(|kind| format!("browser_interaction_failed:{kind}"))
        .unwrap_or_default();
    lines.extend(interaction_root_cause_repair_guidance(
        profile,
        goal,
        &failure_kind,
        Some(&value),
    ));
    if let Some(cold_start_ms) = raw_u64_field_deep(&value, "cold_start_ms")
        && cold_start_ms > 10_000
    {
        let seconds = (cold_start_ms + 500) / 1000;
        lines.push(format!(
            "Note: first page load took {seconds}s (cold start; excluded from assertions)"
        ));
    }
    lines.extend(
        surface_fit_guidance_lines_from_value(&value)
            .into_iter()
            .map(|line| format!("interaction surface fit: {line}")),
    );
    if let Some(mode) = raw_text_field_deep(&value, &["probe_mode"]).filter(|mode| !mode.is_empty())
    {
        lines.push(format!("interaction probe mode: {mode}"));
    }
    if let Some(status) =
        raw_text_field_deep(&value, &["contract_hook_status"]).filter(|status| !status.is_empty())
    {
        lines.push(format!("interaction contract hook status: {status}"));
    }
    if let Some(restart_present) = raw_contract_hook_bool(&value, "restart_present") {
        lines.push(format!(
            "interaction restart hook present: {restart_present}"
        ));
    }
    if let Some(restart_reachable) =
        raw_bool_field_deep(&value, "restart_hook_reachable_after_start")
    {
        lines.push(format!(
            "interaction restart hook reachable after start: {restart_reachable}"
        ));
    }
    let inputs = raw_string_array_field_deep(&value, "input_dispatches");
    if !inputs.is_empty() {
        lines.push(format!(
            "interaction redispatched inputs: {}",
            inputs.join(", ")
        ));
    }
    let state_dimensions = raw_string_array_field_deep(&value, "state_dimensions_changed");
    if !state_dimensions.is_empty() {
        lines.push(format!(
            "interaction state dimensions changed: {}",
            state_dimensions.join(", ")
        ));
    }
    let info = raw_string_array_field_deep(&value, "informational_failure_kinds");
    if !info.is_empty() {
        lines.push(format!(
            "interaction informational findings: {}",
            info.join(", ")
        ));
    }
    lines.extend(
        interaction_candidate_prompt_lines(&value)
            .into_iter()
            .map(|line| format!("interaction candidate table: {line}")),
    );
    lines
}

pub(super) fn release_recovery_missing_capabilities(
    runtime_acceptance: Option<&crate::minimal_loop::evidence::RuntimeAcceptanceReport>,
) -> Vec<String> {
    runtime_acceptance
        .map(|report| report.missing_capabilities.clone())
        .unwrap_or_default()
}

pub(super) fn release_recovery_repair_targets(
    release_gate: &ReleaseGateSummary,
    runtime_acceptance: Option<&crate::minimal_loop::evidence::RuntimeAcceptanceReport>,
) -> Vec<String> {
    let mut targets = Vec::new();
    let browser_status = release_gate.browser_readiness_status.to_ascii_lowercase();
    let interaction_status = release_gate
        .interaction_evidence_status
        .to_ascii_lowercase();
    let interaction_probe_unavailable = release_gate
        .reasons
        .iter()
        .any(|reason| reason.contains("interaction_unverified:probe_unavailable"));
    let restart_terminal_unreached = release_gate
        .reasons
        .iter()
        .any(|reason| reason.contains("interaction_unverified:terminal_state_not_reached"));
    let interaction_probe_infrastructure =
        release_gate_has_interaction_probe_infrastructure_failure(release_gate);
    let build_verifier_compile_errors = release_gate
        .browser_readiness_status
        .contains("build_verifier_failed")
        && !compile_errors_from_release_evidence_path(
            &release_gate.browser_readiness_evidence_path,
        )
        .is_empty();
    if build_verifier_compile_errors {
        targets.push("fix_compile_error".to_string());
        targets.push("implementation".to_string());
    }
    if browser_status.contains("tailwind_dev_pipeline_failure")
        || browser_status.contains("css")
        || browser_status.contains("http_500")
    {
        targets.push("framework_config".to_string());
    }
    if browser_status.starts_with("unavailable:")
        || browser_status.contains("evidence_missing")
        || (!interaction_probe_unavailable
            && (interaction_status.starts_with("unavailable:")
                || interaction_status.contains("evidence_missing")))
    {
        targets.push("required_evidence_missing".to_string());
    }
    if browser_status.starts_with("failed:") && !build_verifier_compile_errors {
        targets.push("test_or_evidence".to_string());
    }
    if interaction_status.starts_with("failed:") && !interaction_probe_infrastructure {
        targets.extend(interaction_repair_targets_for_reason(&interaction_status));
    }
    if restart_terminal_unreached {
        targets.push("restart_reachability_or_accept_partial".to_string());
    }
    if let Some(report) = runtime_acceptance {
        targets.extend(
            report
                .missing_evidence
                .iter()
                .filter(|evidence| behavior_depth_evidence_key(evidence))
                .map(|evidence| format!("implementation:{evidence}")),
        );
        targets.extend(
            report
                .obligation_repair_targets
                .iter()
                .map(|target| format!("{}:{}", target.obligation, target.target_path)),
        );
    }
    if targets.is_empty() {
        targets.push("release_acceptance".to_string());
    }
    dedup_strings(targets)
}

pub(super) fn compile_errors_from_release_evidence_path(path: &str) -> Vec<CompileError> {
    if path.trim().is_empty() {
        return Vec::new();
    }
    let evidence_path = Path::new(path);
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return build_verifier::FullCommandOutput::read_from_path(evidence_path)
            .map(|output| build_verifier::parse_compile_errors(&output))
            .unwrap_or_default();
    };
    let mut errors = Vec::new();
    for output in release_evidence_compile_output_path_fields(&value, evidence_path) {
        for error in build_verifier::parse_compile_errors(&output) {
            if !errors.contains(&error) {
                errors.push(error);
            }
        }
    }
    errors
}

pub(super) fn release_evidence_compile_output_path_fields(
    value: &Value,
    evidence_path: &Path,
) -> Vec<build_verifier::FullCommandOutput> {
    let mut out: Vec<build_verifier::FullCommandOutput> = Vec::new();
    let base_dir = evidence_path.parent().unwrap_or_else(|| Path::new("."));
    for scope in raw_value_scopes(value) {
        for key in [
            "build_output_path",
            "full_output_path",
            "output_path",
            "stdout_path",
            "stderr_path",
        ] {
            if let Some(raw_path) = scope.get(key).and_then(Value::as_str) {
                let path = Path::new(raw_path);
                let path = if path.is_absolute() {
                    path.to_path_buf()
                } else {
                    base_dir.join(path)
                };
                if let Ok(output) = build_verifier::FullCommandOutput::read_from_path(path)
                    && !output.as_str().trim().is_empty()
                    && !out
                        .iter()
                        .any(|existing| existing.as_str() == output.as_str())
                {
                    out.push(output);
                }
            }
        }
    }
    out
}

pub(super) fn release_recovery_verify_commands(
    profile: &str,
    release_gate: &ReleaseGateSummary,
) -> Vec<String> {
    let mut commands = resolve_profile_runtime(profile).release_recovery_verify_commands(
        &release_gate.reasons,
        release_gate_has_interaction_probe_infrastructure_failure(release_gate),
    );
    if release_gate.status == "partial" {
        commands.push("do not claim release_ready until release gate evidence passes".to_string());
    }
    dedup_strings(commands)
}

pub(super) fn interaction_repair_targets_for_reason(reason: &str) -> Vec<String> {
    let lower = reason.to_ascii_lowercase();
    if lower.contains("input_state_change_missing_after_start")
        || lower.contains("input_state_change_not_evaluated_after_start")
        || lower.contains("interaction_state_change_missing")
        || lower.contains("canvas_blank")
        || lower.contains("text_input_state_change_missing")
    {
        vec!["input_state_render_wiring".to_string()]
    } else if lower.contains("token_echo_after_reload_only") || lower.contains("token_echo_missing")
    {
        vec!["live_preview_render_wiring".to_string()]
    } else if lower.contains("text_entry_missing") {
        vec!["text_input_wiring".to_string()]
    } else if lower.contains("persistence_after_reload_reset") {
        vec!["persistence_state_wiring".to_string()]
    } else if lower.contains("start_transition_missing")
        || lower.contains("primary_start_transition_missing")
    {
        vec!["start_control_wiring".to_string()]
    } else {
        vec!["capability_implementation".to_string()]
    }
}

pub(super) fn behavior_depth_evidence_key(evidence: &str) -> bool {
    matches!(
        evidence,
        "challenge_or_adversary_evidence"
            | "failure_or_collision_evidence"
            | "score_or_progression_evidence"
            | "restart_or_recoverable_state_evidence"
            | "persistence_evidence"
            | "live_preview_evidence"
    )
}

// Plan-level acceptance boundary (pre-split runner.rs:3603-3944).
pub(super) fn verify_plan_final_contract(
    plan: &StepPlan,
    required_final_artifacts: &[String],
    config: &Config,
    bound_contract: Option<&BoundCompletionContract>,
) -> anyhow::Result<()> {
    let mut required_paths = required_final_artifacts.to_vec();
    let profile_id = ProfileId::parse(&config.profile);
    let runtime = ProfileRuntimeRegistry::resolve(&profile_id);
    let mut required_capabilities = runtime.required_capabilities(&plan.goal);
    let mut required_evidence = runtime.required_evidence(&plan.goal, &required_capabilities);
    let mut required_obligations =
        runtime.required_obligations(&profile_id, &plan.goal, &required_capabilities);
    let mut evidence_hint_tokens = evidence_hint_tokens_for_goal(&plan.goal);
    let owned_bound_contract;
    let bound_contract = if let Some(bound_contract) = bound_contract {
        Some(bound_contract)
    } else {
        owned_bound_contract = bind_completion_contract_for_acceptance(
            config,
            "plan-run",
            &config.profile,
            &plan.goal,
            &required_paths,
            &required_capabilities,
            &required_evidence,
            &required_obligations,
        )?;
        owned_bound_contract.as_ref()
    };
    let mut verify_commands = Vec::new();
    let mut deferred_commands = Vec::new();
    if let Some(bound) = bound_contract {
        let contract = &bound.contract;
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
    let missing_final_artifacts = missing_final_artifacts(&config.workspace_root, &required_paths);
    let external_report = bound_contract.map(|bound| {
        bound
            .contract
            .verify_with_goal(&config.workspace_root, &plan.goal)
    });
    let runtime_acceptance_required = !required_capabilities.is_empty()
        || !required_evidence.is_empty()
        || !required_obligations.is_empty();
    let mut runtime_acceptance = runtime_acceptance_required.then(|| {
        verify_runtime_acceptance_with_browser_dirs_and_hints(
            &config.workspace_root,
            &required_paths,
            &verify_commands,
            &required_capabilities,
            &required_evidence,
            &required_obligations,
            &deferred_commands,
            &release_evidence_extra_dirs(config),
            &evidence_hint_tokens,
        )
    });
    let evidence_arbitration = runtime_acceptance.as_mut().map(|report| {
        final_acceptance_evidence_arbitration(
            config,
            report,
            &required_capabilities,
            &required_evidence,
            &required_obligations,
        )
    });
    let mut release_gate = final_acceptance_release_gate_with_runtime(
        config,
        runtime,
        &plan.goal,
        &required_capabilities,
        runtime_acceptance.as_ref(),
        false,
    );
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
    let profile_behavior_probe = plan_final_probe::PlanFinalProbe::dispatch(
        config,
        runtime,
        &profile_id,
        &plan.goal,
        &required_capabilities,
    );
    profile_behavior_probe.bind_release_gate(&mut release_gate);
    let contract_required =
        runtime.requires_completion_contract(&profile_id, &plan.goal, &required_capabilities)
            || bound_contract.is_some_and(|bound| bound.required);
    let external_contract_checked = bound_contract.is_some();
    let contract_binding_missing = contract_required && !external_contract_checked;
    let external_ok = !contract_binding_missing
        && external_contract_ok_after_runtime_arbitration(
            external_report.as_ref(),
            runtime_acceptance.as_ref(),
        );
    let runtime_ok = runtime_acceptance
        .as_ref()
        .is_none_or(|report| report.passed);
    let pack_checks = crate::planner::pack::runtime::run_final_acceptance_checks_from_environment(
        &config.workspace_root,
        &config.profile,
        config.resolved_intent(&plan.goal),
        config.eval_events_path.as_deref(),
    )?;
    let pack_checks_ok = pack_checks.as_ref().is_none_or(|summary| summary.passed);
    let release_gate_failed = release_gate.status == "failed";
    let ok = missing_final_artifacts.is_empty()
        && external_ok
        && runtime_ok
        && pack_checks_ok
        && !release_gate_failed;
    let final_acceptance_status = release_gate_final_acceptance_status(&release_gate);
    let runtime_acceptance_status =
        profile_behavior_probe.runtime_acceptance_status(runtime_ok, runtime_acceptance.as_ref());
    let (base_assurance_level, base_assurance_reason) =
        runtime.assurance_for_completion(&profile_id, &required_capabilities);
    let testimony_failed = release_gate.reasons.iter().any(|reason| {
        crate::planner::failure_vocabulary::ViolationId::is_testimony_binding(reason)
    });
    let base_assurance = if testimony_failed {
        ("failed", "testimony_binding_violation")
    } else {
        (base_assurance_level, base_assurance_reason)
    };
    let (assurance_level, assurance_reason) =
        profile_behavior_probe.assurance(&config.workspace_root, base_assurance);
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
        &config.profile,
        &state_dimensions_changed,
        &action_hooks,
        &release_gate.interaction_evidence_path,
        &text_telemetry,
    );
    let requested_port =
        effective_requested_port(resolve_profile_runtime(&config.profile), &plan.goal, None);
    let primary_reason = if !missing_final_artifacts.is_empty() {
        format!(
            "missing final artifacts: {}",
            missing_final_artifacts.join(", ")
        )
    } else if contract_binding_missing {
        "completion contract binding required but missing".to_string()
    } else if let Some(report) = runtime_acceptance.as_ref().filter(|report| !report.passed) {
        report.primary_reason.clone()
    } else if let Some(report) = external_report.as_ref().filter(|report| {
        !external_contract_ok_after_runtime_arbitration(Some(*report), runtime_acceptance.as_ref())
    }) {
        report.primary_reason()
    } else if let Some(reason) = pack_checks
        .as_ref()
        .and_then(|summary| summary.primary_reason.clone())
    {
        reason
    } else if release_gate_failed {
        format!("release gate failed: {}", release_gate.reasons.join("; "))
    } else {
        "ok".to_string()
    };
    let recovery_handoff = if !ok || release_recovery_needed(&release_gate, final_acceptance_status)
    {
        let acceptance_layer =
            release_recovery_acceptance_layer(&release_gate, final_acceptance_status);
        let failure_kind =
            release_recovery_failure_kind(&release_gate, final_acceptance_status, &primary_reason);
        let scope = format!("release-{}", recovery_scope_token(acceptance_layer));
        save_release_recovery_handoff(
            config,
            &config.profile,
            &plan.goal,
            &scope,
            acceptance_layer,
            &failure_kind,
            release_recovery_failure_evidence(
                &config.profile,
                &plan.goal,
                &release_gate,
                final_acceptance_status,
                &primary_reason,
                runtime_acceptance.as_ref(),
            ),
            missing_final_artifacts.clone(),
            release_recovery_missing_capabilities(runtime_acceptance.as_ref()),
            release_recovery_repair_targets(&release_gate, runtime_acceptance.as_ref()),
            release_recovery_verify_commands(&config.profile, &release_gate),
        )
    } else {
        None
    };
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "plan_final_contract",
            "profile": config.profile,
            "requested_port": requested_port.as_ref().map(|requested| requested.telemetry.clone()),
            "required_final_artifacts": required_paths,
            "missing_final_artifacts": missing_final_artifacts,
            "completion_contract_verification_enabled": external_contract_checked,
            "completion_contract_path_merge_enabled": external_contract_checked,
            "completion_contract_path": bound_contract
                .map(|bound| bound.path.clone())
                .unwrap_or_default(),
            "completion_contract_generated": bound_contract
                .map(|bound| bound.generated)
                .unwrap_or(false),
            "external_contract_checked": external_contract_checked,
            "external_contract_required": contract_required,
            "external_contract_ok": external_ok,
            "required_capabilities": required_capabilities,
            "required_evidence": required_evidence,
            "required_obligations": required_obligations,
            "missing_capabilities": runtime_acceptance
                .as_ref()
                .map(|report| report.missing_capabilities.clone())
                .unwrap_or_default(),
            "missing_evidence": runtime_acceptance
                .as_ref()
                .map(|report| report.missing_evidence.clone())
                .unwrap_or_default(),
            "missing_obligations": runtime_acceptance
                .as_ref()
                .map(|report| report.missing_obligations.clone())
                .unwrap_or_default(),
            "weak_evidence": runtime_acceptance
                .as_ref()
                .map(|report| report.weak_evidence.clone())
                .unwrap_or_default(),
            "runtime_acceptance_diagnostics": runtime_acceptance
                .as_ref()
                .map(|report| report.diagnostics.clone())
                .unwrap_or_default(),
            "unverified_evidence": runtime_acceptance
                .as_ref()
                .map(|report| report.unverified_evidence.clone())
                .unwrap_or_default(),
            "evidence_tiers": runtime_acceptance
                .as_ref()
                .map(|report| report.evidence_tiers.clone())
                .unwrap_or_default(),
            "evidence_arbitration": evidence_arbitration
                .as_ref()
                .map(|report| report.records.clone())
                .unwrap_or_default(),
            "evidence_arbitration_summary": evidence_arbitration
                .as_ref()
                .map(|report| report.summary.clone())
                .unwrap_or_default(),
            "artifact_obligations": runtime_acceptance
                .as_ref()
                .map(|report| report.artifact_obligations.clone())
                .unwrap_or_default(),
            "capability_evidence_bindings": runtime_acceptance
                .as_ref()
                .map(|report| report.capability_evidence_bindings.clone())
                .unwrap_or_default(),
            "obligation_repair_targets": runtime_acceptance
                .as_ref()
                .map(|report| report.obligation_repair_targets.clone())
                .unwrap_or_default(),
            "inconclusive_reasons": runtime_acceptance
                .as_ref()
                .map(|report| report.inconclusive_reasons.clone())
                .unwrap_or_default(),
            "runtime_acceptance_inconclusive": runtime_acceptance
                .as_ref()
                .map(|report| report.inconclusive)
                .unwrap_or(false),
            "runtime_acceptance_passed": profile_behavior_probe.runtime_acceptance_passed(runtime_ok),
            "runtime_acceptance_status": runtime_acceptance_status,
            "final_acceptance_status": final_acceptance_status,
            "assurance_level": assurance_level,
            "assurance_reason": assurance_reason,
            "release_quality_completion": release_quality_completion,
            "release_gate_status": release_gate.status.clone(),
            "release_gate_reasons": release_gate.reasons.clone(),
            "profile_behavior_probe_status": profile_behavior_probe.event_status(),
            "profile_behavior_probe_reasons": profile_behavior_probe.reasons(),
            "profile_behavior_probe_evidence_path": profile_behavior_probe.evidence_path(),
            "browser_readiness_status": release_gate.browser_readiness_status.clone(),
            "browser_readiness_evidence_path": release_gate.browser_readiness_evidence_path.clone(),
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
            "recovery_handoff_saved": recovery_handoff
                .as_ref()
                .is_some_and(ReleaseRecoveryHandoffSummary::has_artifact),
            "handoff_saved_not_success": recovery_handoff.is_some(),
            "ok": ok,
            "primary_reason": eval_events::body_snippet(&primary_reason),
        }),
    );
    emit_depth_profile(
        config.eval_events_path.as_deref(),
        "plan_final_contract",
        &depth_profile,
    );
    if let Err(error) = crate::planner::pack::runtime::emit_score_checkpoint_from_environment(
        &config.workspace_root,
        &config.profile,
        config.resolved_intent(&plan.goal),
        config.eval_events_path.as_deref(),
    ) {
        eprintln!("warning: score checkpoint projection failed: {error:#}");
    }
    if ok {
        return Ok(());
    }
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "loop_stop",
            "reason": "plan_final_contract_failure",
            "primary_reason": eval_events::body_snippet(&primary_reason),
        }),
    );
    anyhow::bail!("plan final contract failed: {primary_reason}")
}
