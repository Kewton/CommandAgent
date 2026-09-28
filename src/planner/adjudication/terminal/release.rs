pub(crate) fn release_quality_completion(release_gate: &str, final_acceptance: &str) -> String {
    match release_gate {
        "pass" | "not_applicable" if final_acceptance != "not_checked" => {
            "release_ready".to_string()
        }
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
        _ if final_acceptance == "not_checked" => "run_acceptance_or_review_changes".to_string(),
        _ => "none".to_string(),
    }
}
