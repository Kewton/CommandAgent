//! Recovery handoff projection for behavioral probe infrastructure failures.

use super::VerificationReport;

pub(super) fn infrastructure_failure_kind(report: &VerificationReport) -> Option<String> {
    let probe = super::interaction_probe_json_from_report(report)?;
    let kind = super::raw_text_field_deep(&probe, &["failure_kind"])?;
    (probe
        .get("failure_category")
        .and_then(serde_json::Value::as_str)
        == Some("infrastructure")
        || kind.starts_with("probe_infrastructure_failed:")
        || kind.starts_with("probe_dependency_missing:"))
    .then_some(kind)
}
