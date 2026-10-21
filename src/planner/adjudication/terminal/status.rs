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
