use super::{
    Config, ProfileBehaviorProbeReport, current_final_acceptance_cycle_index, eval_events, json,
};

pub(super) fn emit_probe_event(
    config: &Config,
    profile: &str,
    report: &ProfileBehaviorProbeReport,
) {
    if report.status == "pass" && report.reasons.is_empty() && report.evidence_path.is_none() {
        return;
    }
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "profile_behavior_probe",
            "cycle_index": current_final_acceptance_cycle_index(),
            "profile": profile,
            "status": report.status,
            "ok": report.status == "pass",
            "reasons": report.reasons.clone(),
            "evidence_path": report.evidence_path.clone().unwrap_or_default(),
        }),
    );
}
