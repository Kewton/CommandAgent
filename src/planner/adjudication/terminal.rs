#[allow(clippy::too_many_arguments)]
pub(crate) fn projected_assurance(
    assurance_level: &str,
    assurance_reason: &str,
    effective_profile: &str,
    release_gate: &str,
    final_acceptance: &str,
    release_gate_reasons: &[String],
    completion_contract_verification_enabled: bool,
    external_contract_checked: bool,
    browser_readiness_applicable: bool,
    browser_readiness_execution_status: &str,
    interaction_evidence_applicable: bool,
    interaction_evidence_execution_status: &str,
) -> (String, String) {
    let mut level = assurance_level.to_string();
    let mut reason = assurance_reason.to_string();
    if level != "full" {
        return (level, reason);
    }
    if effective_profile.trim().is_empty() {
        return (
            "partial".to_string(),
            "effective_profile_unknown".to_string(),
        );
    }
    if final_acceptance == "partial" || release_gate == "partial" {
        return (
            "partial".to_string(),
            release_gate_reasons
                .first()
                .cloned()
                .unwrap_or_else(|| "acceptance_partial".to_string()),
        );
    }
    if final_acceptance != "full_success" || release_gate == "failed" {
        level = "partial".to_string();
        reason = release_gate_reasons
            .first()
            .cloned()
            .unwrap_or_else(|| "acceptance_not_full_success".to_string());
        return (level, reason);
    }
    if !completion_contract_verification_enabled && !external_contract_checked {
        return (
            "partial".to_string(),
            "completion_contract_not_bound".to_string(),
        );
    }
    if browser_readiness_applicable && browser_readiness_execution_status != "performed" {
        return (
            "partial".to_string(),
            format!("browser_readiness_not_performed:{browser_readiness_execution_status}"),
        );
    }
    if interaction_evidence_applicable && interaction_evidence_execution_status != "performed" {
        return (
            "partial".to_string(),
            format!("interaction_evidence_not_performed:{interaction_evidence_execution_status}"),
        );
    }
    (level, reason)
}

pub(crate) fn terminal_status(ok: bool, release_gate: &str, final_acceptance: &str) -> String {
    if !ok {
        return "incomplete".to_string();
    }
    match release_gate {
        "partial" => "complete_with_partial_release_gate".to_string(),
        "failed" => "incomplete_release_gate_failed".to_string(),
        "pass" | "not_applicable" | "not_checked" | "" => match final_acceptance {
            "partial" => "complete_with_partial_release_gate".to_string(),
            "incomplete" | "failed" => "incomplete".to_string(),
            _ => "complete".to_string(),
        },
        _ => "incomplete".to_string(),
    }
}

pub(crate) fn task_status(ok: bool, release_gate: &str, final_acceptance: &str) -> String {
    if !ok {
        return "failed".to_string();
    }
    match release_gate {
        "partial" => "partial".to_string(),
        "failed" => "failed".to_string(),
        "pass" => "complete".to_string(),
        "not_applicable" | "not_checked" | "" => match final_acceptance {
            "partial" => "partial".to_string(),
            "incomplete" => "incomplete".to_string(),
            "failed" => "failed".to_string(),
            _ => "complete".to_string(),
        },
        _ => "incomplete".to_string(),
    }
}

pub(crate) fn release_quality_completion(release_gate: &str, final_acceptance: &str) -> String {
    match release_gate {
        "pass" | "not_applicable" => "release_ready".to_string(),
        "partial" => "partial".to_string(),
        "failed" => "failed".to_string(),
        _ if final_acceptance == "partial" => "partial".to_string(),
        _ if matches!(final_acceptance, "incomplete" | "failed") => "failed".to_string(),
        _ => "not_checked".to_string(),
    }
}

pub(crate) fn next_action(ok: bool, release_gate: &str, final_acceptance: &str) -> String {
    if !ok {
        return "fix_command_failure".to_string();
    }
    match release_gate {
        "partial" => "collect_missing_release_evidence_or_continue_release_recovery".to_string(),
        "failed" => "repair_release_gate_failure".to_string(),
        _ if final_acceptance == "partial" => {
            "collect_missing_final_acceptance_evidence".to_string()
        }
        _ if matches!(final_acceptance, "incomplete" | "failed") => {
            "repair_final_acceptance_failure".to_string()
        }
        _ => "none".to_string(),
    }
}
