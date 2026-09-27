use crate::planner::adjudication::core::GateObservation;

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
    gate_observations: &[GateObservation<'_>],
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
    if let Some(observation) = gate_observations
        .iter()
        .find(|observation| observation.applicable && observation.execution_status != "performed")
    {
        return (
            "partial".to_string(),
            format!(
                "{}_not_performed:{}",
                observation.reason_key, observation.execution_status
            ),
        );
    }
    (level, reason)
}
