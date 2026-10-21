//! Pure rendering of the human completion summary from a projection.

use super::{
    CompletionProjection, body_snippet, interaction_unverified_probe_unavailable, missing_if_empty,
    render_stop_reason_text,
};

pub(super) fn render(
    lifecycle_stage: &str,
    action: Option<&str>,
    command: Option<&str>,
    stop_reason: &str,
    failure_kind: &str,
    projection: &CompletionProjection,
) -> String {
    let mut lines = vec![
        format!("Status: {}", projection.status),
        format!("Lifecycle: {lifecycle_stage}"),
        format!("Process: {}", process_lifecycle_status(projection)),
        format!("Session/REPL status: {}", session_status(lifecycle_stage)),
    ];
    let stop_reason = render_stop_reason_text(stop_reason);
    if let Some(action) = action {
        lines.push(format!("Action: {action}"));
    }
    if let Some(command) = command {
        lines.push(format!("Command: {command}"));
    }
    push_status_fields(&mut lines, projection, &stop_reason);
    push_profile_assurance(&mut lines, projection);
    push_unverified(&mut lines, projection);
    push_probe_notices(&mut lines, projection);
    push_plan_and_recovery(&mut lines, projection);
    push_failure_tail(
        &mut lines,
        lifecycle_stage,
        failure_kind,
        &stop_reason,
        projection,
    );
    lines.join("\n")
}

fn push_status_fields(
    lines: &mut Vec<String>,
    projection: &CompletionProjection,
    stop_reason: &str,
) {
    lines.extend([
        format!("Command status: {}", projection.command_completion),
        format!("Command completion: {}", projection.command_completion),
        format!("Task status: {}", projection.task_status),
        format!(
            "Effective profile: {}",
            missing_if_empty(&projection.effective_profile)
        ),
        format!(
            "Prompt layout: {}",
            missing_if_empty(&projection.prompt_layout)
        ),
        format!("Contract origin: {}", projection.contract_origin),
        format!("Runtime acceptance: {}", projection.runtime_acceptance),
        format!("Final acceptance: {}", projection.final_acceptance),
        format!("Release gate: {}", projection.release_gate),
        format!("Requested port: {}", missing_if_empty(&projection.requested_port)),
        format!(
            "Evidence arbitration: {}",
            missing_if_empty(&projection.evidence_arbitration_summary)
        ),
        format!(
            "Depth profile: {}",
            missing_if_empty(&projection.depth_profile_summary)
        ),
        format!(
            "completion_contract_verification_enabled={}",
            projection.completion_contract_verification_enabled
        ),
        format!(
            "completion_contract_path_merge_enabled={}",
            projection.completion_contract_path_merge_enabled
        ),
        format!(
            "completion_contract_path={}",
            missing_if_empty(&projection.completion_contract_path)
        ),
        format!(
            "completion_contract_generated={}",
            projection.completion_contract_generated
        ),
        format!(
            "external_contract_checked={}",
            projection.external_contract_checked
        ),
        format!("external_contract_ok={}", projection.external_contract_ok),
        format!(
            "browser_readiness_applicable={}",
            projection.browser_readiness_applicable
        ),
        format!(
            "browser_readiness_execution_status={}",
            projection.browser_readiness_execution_status
        ),
        format!(
            "interaction_evidence_applicable={}",
            projection.interaction_evidence_applicable
        ),
        format!(
            "interaction_evidence_execution_status={}",
            projection.interaction_evidence_execution_status
        ),
        format!("Planner repaired: {}", projection.planner_repaired),
        format!("Planner release risk: {}", projection.planner_release_risk),
        format!(
            "Planner diagnostics: normalizations={} retries={} quality_warnings={} quality_issues={}",
            projection.planner_verify_normalization_count,
            projection.planner_retry_count,
            projection.planner_quality_warning_count,
            projection.planner_quality_issue_count
        ),
        format!(
            "Context truncation warning: {}",
            if projection.context_truncation_warning_count > 0 {
                format!(
                    "suspected (warnings={})",
                    projection.context_truncation_warning_count
                )
            } else {
                "none".to_string()
            }
        ),
        format!(
            "Release quality completion: {}",
            projection.release_quality_completion
        ),
        "Release gate reasons:".to_string(),
        render_summary_bullets(&projection.release_gate_reasons),
        format!("Browser readiness: {}", projection.browser_readiness),
        format!(
            "Browser readiness evidence: {}",
            missing_if_empty(&projection.browser_readiness_evidence_path)
        ),
        format!("Interaction evidence: {}", projection.interaction_evidence),
        format!(
            "Interaction evidence path: {}",
            missing_if_empty(&projection.interaction_evidence_path)
        ),
        format!(
            "State dimensions changed: {}",
            if projection.state_dimensions_changed.is_empty() {
                "none".to_string()
            } else {
                projection.state_dimensions_changed.join(", ")
            }
        ),
        format!(
            "Action hooks: {}",
            if projection.action_hooks.is_empty() {
                "none".to_string()
            } else {
                projection.action_hooks.join(", ")
            }
        ),
        format!(
            "Surface fit: {}",
            missing_if_empty(&projection.surface_fit_summary)
        ),
        format!(
            "Text entry target: {}",
            missing_if_empty(&projection.text_entry_target)
        ),
        format!("Typed token: {}", missing_if_empty(&projection.typed_token)),
        format!(
            "Token echoed: {}",
            missing_if_empty(&projection.token_echoed)
        ),
        format!(
            "Text input state change: {}",
            missing_if_empty(&projection.text_input_state_change)
        ),
        format!("Next action: {}", projection.next_action),
        format!("Recovery next action: {}", projection.next_action),
        format!("Stop reason: {stop_reason}"),
    ]);
}

fn push_profile_assurance(lines: &mut Vec<String>, projection: &CompletionProjection) {
    if !projection.profile.is_empty() {
        lines.push(format!("Profile: {}", projection.profile));
    }
    if !projection.profile_inferred.is_empty() {
        lines.push(format!(
            "profile_inferred: {} (from: {})",
            projection.profile_inferred, projection.profile_inference_source
        ));
    }
    if !projection.assurance_level.is_empty() {
        let suffix = if projection.assurance_reason.is_empty() {
            String::new()
        } else {
            format!(" ({})", projection.assurance_reason)
        };
        lines.push(format!(
            "Assurance: {}{}",
            projection.assurance_level, suffix
        ));
    }
}

fn push_unverified(lines: &mut Vec<String>, projection: &CompletionProjection) {
    if !projection.unverified_evidence.is_empty() {
        lines.push("Unverified (probe required):".to_string());
        lines.push(render_summary_bullets(&projection.unverified_evidence));
        if projection
            .unverified_evidence
            .iter()
            .any(|evidence| evidence.contains(":unverified:probe_unavailable"))
        {
            lines.push(
                crate::minimal_loop::interaction_probe::INTERACTION_PROBE_SETUP_REMEDIATION
                    .to_string(),
            );
        }
        if projection
            .unverified_evidence
            .iter()
            .any(|evidence| evidence.contains(":unverified:terminal_state_not_reached"))
        {
            lines.push(
                "Restart verification: either expose an in-play restart control, or accept the partial classification (the restart exists but cannot be behaviorally verified by the generic probe)."
                    .to_string(),
            );
        }
    }
    if !projection.surface_fit_guidance.is_empty() {
        lines.push(format!(
            "Surface fit guidance: {}",
            projection.surface_fit_guidance
        ));
    }
}

fn push_probe_notices(lines: &mut Vec<String>, projection: &CompletionProjection) {
    if interaction_unverified_probe_unavailable(
        &projection.release_gate,
        &projection.release_gate_reasons,
    ) {
        lines.push(
            format!(
                "Interaction verification: interaction_unverified:probe_unavailable; {}.",
                crate::minimal_loop::interaction_probe::INTERACTION_PROBE_SETUP_REMEDIATION
            )
            .to_string(),
        );
    }
    if projection
        .evidence_arbitration_summary
        .starts_with("static (probe infrastructure failure:")
        || projection.release_gate_reasons.iter().any(|reason| {
            reason.contains("probe_dependency_missing")
                || reason.contains("probe_infrastructure_failed")
        })
    {
        lines.push(
            "Interaction verification: app interaction untested (probe infrastructure failure)."
                .to_string(),
        );
    }
    if projection.probe_preflight_status == "failed" {
        let remediation = if projection.probe_preflight_remediation.trim().is_empty() {
            crate::minimal_loop::interaction_probe::INTERACTION_PROBE_SETUP_REMEDIATION
        } else {
            projection.probe_preflight_remediation.trim()
        };
        lines.push(format!(
            "Interaction probe preflight: failed; preflight時点から基盤未準備: {}; {}.",
            missing_if_empty(&projection.probe_preflight_reason),
            remediation
        ));
    }
    let host_env_contamination = crate::minimal_loop::verifier_env::host_env_contamination();
    if !host_env_contamination.is_empty() {
        lines.push(format!(
            "Host env: {} detected (verifiers ran with a cleaned environment)",
            host_env_contamination.join(", ")
        ));
    }
}

fn push_plan_and_recovery(lines: &mut Vec<String>, projection: &CompletionProjection) {
    if !projection.plan_adherence_present.is_empty()
        || !projection.plan_adherence_missing.is_empty()
    {
        lines.extend([
            "Plan adherence:".to_string(),
            "Present tokens:".to_string(),
            render_summary_bullets(&projection.plan_adherence_present),
            "Missing tokens:".to_string(),
            render_summary_bullets(&projection.plan_adherence_missing),
        ]);
    }
    if !projection.compile_rollback_summaries.is_empty() {
        lines.push("Compile rollback applied:".to_string());
        lines.push(render_summary_bullets(
            &projection.compile_rollback_summaries,
        ));
    }
    if !projection.recovery_prompt_path.is_empty()
        || !projection.recovery_ultra_plan_path.is_empty()
        || !projection.suggested_recovery_command.is_empty()
        || !projection.suggested_recovery_yaml_command.is_empty()
    {
        lines.extend([
            "Recovery handoff:".to_string(),
            format!(
                "- Recovery prompt saved: {}",
                missing_if_empty(&projection.recovery_prompt_path)
            ),
            format!(
                "- Recovery UltraPlan YAML saved: {}",
                missing_if_empty(&projection.recovery_ultra_plan_path)
            ),
            format!(
                "- Suggested command: {}",
                missing_if_empty(&projection.suggested_recovery_command)
            ),
            format!(
                "- Suggested YAML command: {}",
                missing_if_empty(&projection.suggested_recovery_yaml_command)
            ),
        ]);
    }
}

fn push_failure_tail(
    lines: &mut Vec<String>,
    lifecycle_stage: &str,
    failure_kind: &str,
    stop_reason: &str,
    projection: &CompletionProjection,
) {
    if !failure_kind.is_empty() {
        lines.push(format!("Failure kind: {failure_kind}"));
    }
    if lifecycle_stage == "tui_command" && projection.command_completion == "failed" {
        let first_stop_line = stop_reason.lines().next().unwrap_or("unknown");
        lines.push(format!(
            "TUI command failed: {}",
            body_snippet(first_stop_line)
        ));
    }
}

pub(super) fn session_status(lifecycle_stage: &str) -> &'static str {
    match lifecycle_stage {
        "tui_command" => "repl_ready",
        "process" => "process_exited",
        _ => "unknown",
    }
}

pub(super) fn process_lifecycle_status(projection: &CompletionProjection) -> String {
    if matches!(
        projection.command_completion.as_str(),
        "aborted" | "interrupted"
    ) {
        projection.command_completion.clone()
    } else {
        "REPL exited cleanly (not task status)".to_string()
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
