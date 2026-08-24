use std::collections::BTreeSet;

use crate::config::Config;
use crate::minimal_loop::evidence::RuntimeAcceptanceReport;
use crate::planner::profile::canonical_profile_name;
use crate::planner::profile_manifest::ManifestStatus;
use crate::planner::verify::VerificationReport;

pub(crate) const PROFILE_NOT_ADMITTED_REASON: &str = "profile_not_admitted";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GateObservation<'a> {
    pub(crate) reason_key: &'a str,
    pub(crate) status_key: &'a str,
    pub(crate) applicable: bool,
    pub(crate) observed_status: &'a str,
    pub(crate) execution_status: &'a str,
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

pub(crate) fn execution_status_from_observed(
    status: &str,
    performed_status_aliases: &[&str],
) -> String {
    if gate_status_disconnected(status) {
        "disconnected".to_string()
    } else if status == "passed" || performed_status_aliases.contains(&status) {
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

pub(crate) fn disconnected_gate_observations_reason(
    observations: &[GateObservation<'_>],
) -> Option<String> {
    let mut disconnected = Vec::new();
    for observation in observations {
        if observation.applicable && gate_status_disconnected(observation.observed_status) {
            disconnected.push(format!(
                "{}={}",
                observation.status_key, observation.observed_status
            ));
        }
    }
    (!disconnected.is_empty())
        .then(|| format!("acceptance_gates_disconnected:{}", disconnected.join(",")))
}

pub(crate) fn profile_behavior_failure_reasons(
    existing_reasons: &[String],
    probe_reasons: &[String],
    evidence_path: Option<&str>,
) -> Vec<String> {
    let mut reasons = existing_reasons.to_vec();
    if probe_reasons.is_empty() {
        reasons.push("profile_behavior_probe_failed".to_string());
    } else {
        reasons.extend(
            probe_reasons
                .iter()
                .map(|reason| format!("profile_behavior_probe_failed:{reason}")),
        );
    }
    if let Some(path) = evidence_path {
        reasons.push(format!("profile_behavior_probe_evidence:{path}"));
    }
    dedup_strings(reasons)
}

pub(crate) fn append_gate_observation(
    report: &mut VerificationReport,
    status_label: &str,
    evidence_label: &str,
    status: &str,
    evidence_path: &str,
) {
    if status != "not_checked" && status != "not_applicable" {
        report.push_profile_failure(format!("{status_label}: {status}"));
    }
    if !evidence_path.is_empty() {
        report.push_profile_failure(format!("{evidence_label}: {evidence_path}"));
    }
}

pub(crate) fn final_acceptance_status_from_release_gate(release_gate_status: &str) -> &'static str {
    match release_gate_status {
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
    release_gate_status: &str,
    release_gate_reasons: &[String],
    gate_observations: &[GateObservation<'_>],
) -> (String, String) {
    if base_level != "full" {
        return (base_level.to_string(), base_reason.to_string());
    }
    if final_acceptance_status == "partial" || release_gate_status == "partial" {
        return (
            "partial".to_string(),
            release_gate_reasons
                .first()
                .cloned()
                .unwrap_or_else(|| "acceptance_partial".to_string()),
        );
    }
    if final_acceptance_status != "full_success" || release_gate_status == "failed" {
        return (
            "partial".to_string(),
            release_gate_reasons
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
    ("full".to_string(), String::new())
}

pub(crate) fn release_quality_from_gate_status(
    release_gate_status: &str,
    final_acceptance_status: &str,
) -> &'static str {
    match release_gate_status {
        "pass" | "not_applicable" => "release_ready",
        "partial" => "partial",
        "failed" => "failed",
        _ if final_acceptance_status == "partial" => "partial",
        _ => "failed",
    }
}

pub(crate) fn next_action_from_gate_status(
    release_gate_status: &str,
    final_acceptance_status: &str,
) -> &'static str {
    match release_gate_status {
        "partial" => "collect_missing_release_evidence_or_continue_release_recovery",
        "failed" => "repair_release_gate_failure",
        _ if final_acceptance_status == "partial" => "collect_missing_final_acceptance_evidence",
        _ => "none",
    }
}

pub(crate) fn recovery_needed_for_gate_status(
    release_gate_status: &str,
    final_acceptance_status: &str,
) -> bool {
    matches!(release_gate_status, "partial" | "failed")
        || matches!(final_acceptance_status, "partial" | "failed" | "incomplete")
}

pub(crate) fn recovery_acceptance_layer_for_gate_status(
    release_gate_status: &str,
    final_acceptance_status: &str,
) -> &'static str {
    match release_gate_status {
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
