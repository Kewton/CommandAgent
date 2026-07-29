use super::*;
use crate::planner::failure_vocabulary::AssuranceReasonId;

pub(super) fn assurance_for_completion(
    profile: &str,
    required_capabilities: &[String],
) -> (&'static str, &'static str) {
    let profile = canonical_profile_name(profile);
    if profile == "data" {
        ("static", "data_profile_probe_not_run")
    } else if profile == "generic" {
        if required_capabilities
            .iter()
            .any(|capability| capability == GENERIC_INTERACTIVE_CONTRACT_CAPABILITY)
        {
            ("static", eval_events::GENERIC_STATIC_ASSURANCE_REASON)
        } else {
            ("reduced", eval_events::GENERIC_REDUCED_ASSURANCE_REASON)
        }
    } else {
        ("full", "")
    }
}

pub(super) fn earned_assurance_for_completion(
    profile: &str,
    required_capabilities: &[String],
    contract_bound: bool,
    final_acceptance_status: &str,
    release_gate: &ReleaseGateSummary,
    gate_telemetry: &AcceptanceGateTelemetry,
    profile_behavior_probe: Option<&ProfileBehaviorProbeReport>,
) -> (String, String) {
    let data_profile = canonical_profile_name(profile) == "data";
    if data_profile {
        let status = profile_behavior_probe.map(|report| report.status);
        if status != Some("pass") {
            let level = match status {
                Some("partial") => "partial",
                Some("failed") => "failed",
                _ => "static",
            };
            let reason = profile_behavior_probe
                .and_then(|report| report.reasons.first())
                .cloned()
                .unwrap_or_else(|| AssuranceReasonId::data_assurance(level).to_string());
            return (level.to_string(), reason);
        }
    }
    let (base_level, base_reason) = if data_profile {
        ("full", "")
    } else {
        assurance_for_completion(profile, required_capabilities)
    };
    earned_assurance_from_release_gate(
        profile,
        base_level,
        base_reason,
        contract_bound,
        final_acceptance_status,
        release_gate,
        gate_telemetry,
    )
}
