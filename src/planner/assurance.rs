use super::*;
use crate::planner::failure_vocabulary::AssuranceReasonId;
use crate::planner::profile::{ProfileId, ProfileRuntimeRegistry};

pub(super) fn earned_assurance_for_completion(
    profile: &str,
    required_capabilities: &[String],
    contract_bound: bool,
    final_acceptance_status: &str,
    release_gate: &ReleaseGateSummary,
    gate_telemetry: &AcceptanceGateTelemetry,
    profile_behavior_probe: Option<&ProfileBehaviorProbeReport>,
) -> (String, String) {
    let profile_id = ProfileId::parse(profile);
    let runtime = ProfileRuntimeRegistry::resolve(&profile_id);
    let data_profile = profile_id == ProfileId::Data;
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
        runtime.assurance_for_completion(&profile_id, required_capabilities)
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
