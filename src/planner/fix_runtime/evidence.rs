use super::*;

pub(super) fn persist_adjudication(
    root: &Path,
    run_id: &str,
    adjudication: &FixAdjudication,
    evidence: &FixEvidenceBundle,
) -> anyhow::Result<()> {
    persist_json(
        root,
        &adjudication_evidence_path(run_id),
        &PersistedFixAdjudication {
            schema_version: "1",
            intent: "fix",
            contract_version: FIX_CONTRACT_VERSION,
            contract_ref: FIX_CONTRACT_REF,
            run_id,
            adjudication,
            evidence,
        },
    )
}

pub(super) fn persist_json(
    root: &Path,
    relative: &str,
    value: &impl Serialize,
) -> anyhow::Result<()> {
    let path = crate::tools::path_guard::resolve_optional_existing(root, relative)?;
    let parent = path.parent().context("fix evidence parent missing")?;
    std::fs::create_dir_all(parent)?;
    crate::evidence_envelope::write_json_for_path(
        &path,
        value,
        crate::evidence_envelope::EvidenceFamily::F,
        relative,
        true,
    )
}

pub(super) fn safe_evidence_name(id: &str) -> String {
    let value = id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    if value.is_empty() {
        "unnamed".to_string()
    } else {
        value
    }
}

pub(super) fn before_evidence_path(run_id: &str) -> String {
    format!("evidence/fix-{}-before.json", safe_evidence_name(run_id))
}

pub(super) fn before_attempt_evidence_path(run_id: &str, epoch: u64) -> String {
    format!(
        "evidence/fix-{}-before-attempt-{epoch}.json",
        safe_evidence_name(run_id)
    )
}

pub(super) fn after_evidence_path(run_id: &str) -> String {
    format!("evidence/fix-{}-after.json", safe_evidence_name(run_id))
}

pub(super) fn regression_evidence_path(run_id: &str, binding_id: &str) -> String {
    format!(
        "evidence/fix-{}-regression-{}.json",
        safe_evidence_name(run_id),
        safe_evidence_name(binding_id)
    )
}

pub(super) fn adjudication_evidence_path(run_id: &str) -> String {
    format!(
        "evidence/fix-{}-adjudication.json",
        safe_evidence_name(run_id)
    )
}

pub(super) fn emit_probe_observation(
    config: &Config,
    observation: &FixEvidenceObservation,
    path: &str,
) {
    let mut event = serde_json::json!({
        "event": "fix_evidence_recorded",
        "intent": "fix",
        "contract_version": FIX_CONTRACT_VERSION,
        "contract_ref": FIX_CONTRACT_REF,
        "requirement_id": observation.requirement_id,
        "binding_id": observation.binding_id,
        "stage": observation.stage,
        "expected_polarity": observation.expected,
        "lineage": observation.lineage,
        "epoch": observation.epoch,
        "run_id": observation.run_id,
        "executed": observation.executed,
        "outcome": observation.outcome,
        "reason": eval_events::body_snippet(&observation.reason),
        "evidence_path": path,
    });
    if observation.failure_classification.is_reproducer_defect() {
        event["failure_classification"] = serde_json::json!("reproducer_defect");
    }
    eval_events::emit(config.eval_events_path.as_deref(), event);
}
