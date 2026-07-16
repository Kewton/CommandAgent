use std::collections::BTreeSet;

use crate::config::Config;
use crate::minimal_loop::build_verifier::CompileError;
use crate::minimal_loop::evidence::RuntimeAcceptanceReport;
use crate::planner::profile::{ProfileBehaviorProbeReport, canonical_profile_name};
use crate::planner::profile_manifest::ManifestStatus;
use crate::planner::verify::VerificationReport;

pub(crate) const PROFILE_NOT_ADMITTED_REASON: &str = "profile_not_admitted";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReleaseGateSummary {
    pub(crate) status: String,
    pub(crate) reasons: Vec<String>,
    pub(crate) browser_readiness_status: String,
    pub(crate) browser_readiness_evidence_path: String,
    pub(crate) interaction_evidence_status: String,
    pub(crate) interaction_evidence_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AcceptanceGateTelemetry {
    pub(crate) browser_readiness_applicable: bool,
    pub(crate) browser_readiness_execution_status: String,
    pub(crate) interaction_evidence_applicable: bool,
    pub(crate) interaction_evidence_execution_status: String,
}

pub(crate) fn contract_origin_for_acceptance(config: &Config) -> &'static str {
    if config
        .eval_events_path
        .as_deref()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .is_some_and(|text| {
            text.lines()
                .any(|line| line.contains(r#""event":"profile_reinferred""#))
        })
    {
        "promoted_union"
    } else {
        "initial"
    }
}

pub(crate) fn dedup_strings(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .filter(|value| seen.insert(value.clone()))
        .collect()
}

pub(crate) fn gate_execution_status(status: &str) -> String {
    if gate_status_disconnected(status) {
        "disconnected".to_string()
    } else if matches!(status, "passed" | "interaction_verified_heuristic_only") {
        "performed".to_string()
    } else if status.starts_with("failed") {
        "performed_failed".to_string()
    } else if status.starts_with("unavailable") {
        "unavailable".to_string()
    } else {
        status.to_string()
    }
}

pub(crate) fn gate_status_disconnected(status: &str) -> bool {
    let status = status.trim();
    status.is_empty()
        || matches!(status, "not_applicable" | "not_checked" | "skipped")
        || status.starts_with("skipped:")
}

pub(crate) fn acceptance_gates_disconnected_reason(
    telemetry: &AcceptanceGateTelemetry,
    release_gate: &ReleaseGateSummary,
) -> Option<String> {
    let mut disconnected = Vec::new();
    if telemetry.browser_readiness_applicable
        && gate_status_disconnected(&release_gate.browser_readiness_status)
    {
        disconnected.push(format!(
            "browser_readiness_status={}",
            release_gate.browser_readiness_status
        ));
    }
    if telemetry.interaction_evidence_applicable
        && gate_status_disconnected(&release_gate.interaction_evidence_status)
    {
        disconnected.push(format!(
            "interaction_evidence_status={}",
            release_gate.interaction_evidence_status
        ));
    }
    (!disconnected.is_empty())
        .then(|| format!("acceptance_gates_disconnected:{}", disconnected.join(",")))
}

pub(crate) fn mark_release_gate_profile_behavior_failed(
    release_gate: &mut ReleaseGateSummary,
    profile_behavior_probe: &ProfileBehaviorProbeReport,
) {
    let mut reasons = release_gate.reasons.clone();
    if profile_behavior_probe.reasons.is_empty() {
        reasons.push("profile_behavior_probe_failed".to_string());
    } else {
        reasons.extend(
            profile_behavior_probe
                .reasons
                .iter()
                .map(|reason| format!("profile_behavior_probe_failed:{reason}")),
        );
    }
    if let Some(path) = &profile_behavior_probe.evidence_path {
        reasons.push(format!("profile_behavior_probe_evidence:{path}"));
    }
    release_gate.status = "failed".to_string();
    release_gate.reasons = dedup_strings(reasons);
}

pub(crate) fn append_release_gate_observations(
    report: &mut VerificationReport,
    release_gate: &ReleaseGateSummary,
    browser_compile_errors: Vec<CompileError>,
) {
    if release_gate.browser_readiness_status != "not_checked"
        && release_gate.browser_readiness_status != "not_applicable"
    {
        report.push_profile_failure(format!(
            "browser readiness status: {}",
            release_gate.browser_readiness_status
        ));
    }
    if !release_gate.browser_readiness_evidence_path.is_empty() {
        report.push_profile_failure(format!(
            "browser readiness evidence: {}",
            release_gate.browser_readiness_evidence_path
        ));
        report.push_compile_errors("browser readiness build verifier", browser_compile_errors);
    }
    if release_gate.interaction_evidence_status != "not_checked"
        && release_gate.interaction_evidence_status != "not_applicable"
    {
        report.push_profile_failure(format!(
            "interaction evidence status: {}",
            release_gate.interaction_evidence_status
        ));
    }
    if !release_gate.interaction_evidence_path.is_empty() {
        report.push_profile_failure(format!(
            "interaction evidence path: {}",
            release_gate.interaction_evidence_path
        ));
    }
}

pub(crate) fn release_gate_final_acceptance_status(
    release_gate: &ReleaseGateSummary,
) -> &'static str {
    match release_gate.status.as_str() {
        "pass" | "not_applicable" => "full_success",
        "partial" => "partial",
        "failed" => "incomplete",
        _ => "incomplete",
    }
}

pub(crate) fn runtime_acceptance_status(
    runtime_ok: bool,
    report: Option<&RuntimeAcceptanceReport>,
) -> &'static str {
    match report {
        Some(report) if report.inconclusive => "inconclusive",
        Some(report) if !report.unverified_evidence.is_empty() => "partial",
        Some(_) if runtime_ok => "pass",
        Some(_) => "failed",
        None => "not_checked",
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn earned_assurance_from_base(
    profile: &str,
    base_level: &str,
    base_reason: &str,
    contract_bound: bool,
    final_acceptance_status: &str,
    release_gate: &ReleaseGateSummary,
    gate_telemetry: &AcceptanceGateTelemetry,
) -> (String, String) {
    if base_level != "full" {
        return (base_level.to_string(), base_reason.to_string());
    }
    if final_acceptance_status == "partial" || release_gate.status == "partial" {
        return (
            "partial".to_string(),
            release_gate
                .reasons
                .first()
                .cloned()
                .unwrap_or_else(|| "acceptance_partial".to_string()),
        );
    }
    if final_acceptance_status != "full_success" || release_gate.status == "failed" {
        return (
            "partial".to_string(),
            release_gate
                .reasons
                .first()
                .cloned()
                .unwrap_or_else(|| "acceptance_not_full_success".to_string()),
        );
    }
    if canonical_profile_name(profile).is_empty() {
        return (
            "partial".to_string(),
            "effective_profile_unknown".to_string(),
        );
    }
    if !contract_bound {
        return (
            "partial".to_string(),
            "completion_contract_not_bound".to_string(),
        );
    }
    if gate_telemetry.browser_readiness_applicable
        && gate_telemetry.browser_readiness_execution_status != "performed"
    {
        return (
            "partial".to_string(),
            format!(
                "browser_readiness_not_performed:{}",
                gate_telemetry.browser_readiness_execution_status
            ),
        );
    }
    if gate_telemetry.interaction_evidence_applicable
        && gate_telemetry.interaction_evidence_execution_status != "performed"
    {
        return (
            "partial".to_string(),
            format!(
                "interaction_evidence_not_performed:{}",
                gate_telemetry.interaction_evidence_execution_status
            ),
        );
    }
    ("full".to_string(), String::new())
}

pub(crate) fn release_quality_completion_status(
    release_gate: &ReleaseGateSummary,
    final_acceptance_status: &str,
) -> &'static str {
    match release_gate.status.as_str() {
        "pass" | "not_applicable" => "release_ready",
        "partial" => "partial",
        "failed" => "failed",
        _ if final_acceptance_status == "partial" => "partial",
        _ => "failed",
    }
}

pub(crate) fn release_gate_next_action(
    release_gate: &ReleaseGateSummary,
    final_acceptance_status: &str,
) -> &'static str {
    match release_gate.status.as_str() {
        "partial" => "collect_missing_release_evidence_or_continue_release_recovery",
        "failed" => "repair_release_gate_failure",
        _ if final_acceptance_status == "partial" => "collect_missing_final_acceptance_evidence",
        _ => "none",
    }
}

pub(crate) fn release_recovery_needed(
    release_gate: &ReleaseGateSummary,
    final_acceptance_status: &str,
) -> bool {
    matches!(release_gate.status.as_str(), "partial" | "failed")
        || matches!(final_acceptance_status, "partial" | "failed" | "incomplete")
}

pub(crate) fn release_recovery_acceptance_layer(
    release_gate: &ReleaseGateSummary,
    final_acceptance_status: &str,
) -> &'static str {
    match release_gate.status.as_str() {
        "partial" | "failed" => "release_gate",
        _ if final_acceptance_status == "partial" => "final_acceptance_partial",
        _ => "final_acceptance",
    }
}

pub(crate) fn cap_assurance_for_status(
    status: ManifestStatus,
    level: &mut String,
    reason: &mut String,
) {
    if status == ManifestStatus::Draft && matches!(level.as_str(), "full" | "partial") {
        *level = "static".to_string();
        *reason = PROFILE_NOT_ADMITTED_REASON.to_string();
    }
}
