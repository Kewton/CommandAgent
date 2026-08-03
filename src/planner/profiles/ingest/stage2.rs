use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Context;
use serde::{Deserialize, Serialize};

use super::runtime::{self, CheckStatus, IngestAssurance, IngestCheckSummary, N1, N2, N3, N4, N5};
use crate::evidence_envelope::EvidenceFamily;
use crate::fetch_probe::{
    FETCH_EVIDENCE_PATH, FETCH_FRESHNESS_PATH, FetchEvidence, FetchOutcome, FreshnessEvidence,
    evaluate_freshness,
};

pub const N6: &str = "ingest_fetch_freshness";
pub const STAGE2_EVIDENCE_PATH: &str = "evidence/ingest-stage2-assurance.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stage2Summary {
    pub status: String,
    pub assurance: IngestAssurance,
    pub checks: BTreeMap<String, CheckStatus>,
    pub fetch_evidence_path: String,
    pub freshness_evidence_path: Option<String>,
    pub stage1_evidence_path: Option<String>,
    pub reasons: Vec<String>,
}

pub fn run(root: &Path, contract_ref: &str, run_id: &str) -> anyhow::Result<Stage2Summary> {
    let fetch = crate::fetch_probe::run_contract(root, contract_ref, run_id)?;
    connect(root, contract_ref, fetch)
}

pub fn run_recorded(
    root: &Path,
    contract_ref: &str,
    run_id: &str,
    recording_path: &Path,
) -> anyhow::Result<Stage2Summary> {
    let fetch =
        crate::fetch_probe::run_recorded_contract(root, contract_ref, run_id, recording_path)?;
    connect(root, contract_ref, fetch)
}

fn connect(root: &Path, contract_ref: &str, fetch: FetchEvidence) -> anyhow::Result<Stage2Summary> {
    let mut checks = [N1, N2, N3, N4, N5, N6]
        .into_iter()
        .map(|id| (id.to_string(), CheckStatus::NotExecuted))
        .collect::<BTreeMap<_, _>>();
    let mut reasons = fetch
        .entries
        .iter()
        .filter_map(|entry| entry.failure_kind.as_ref())
        .map(|failure| format!("fetch:{failure}"))
        .collect::<Vec<_>>();
    let fetch_admitted = !fetch.entries.is_empty()
        && fetch.entries.iter().all(|entry| {
            matches!(
                entry.outcome,
                FetchOutcome::Fetched | FetchOutcome::CacheHit
            )
        });
    if !fetch_admitted {
        reasons.push("stage2:fetch_not_admitted:N1-N5_not_executed".to_string());
        return finish(root, checks, reasons, None, None, IngestAssurance::Failed);
    }

    let freshness = evaluate_freshness(root, contract_ref, &fetch)?;
    if !freshness.ok {
        checks.insert(N6.to_string(), CheckStatus::Failed);
        reasons.extend(freshness.failure_kinds.clone());
        reasons.push("stage2:N6_violation:N1-N5_not_executed".to_string());
        return finish(
            root,
            checks,
            reasons,
            Some(&freshness),
            None,
            IngestAssurance::Failed,
        );
    }
    checks.insert(N6.to_string(), CheckStatus::Pass);

    let stage1 = runtime::run_manifest_checks(root)?;
    for (id, status) in &stage1.evidence.checks {
        checks.insert(id.clone(), *status);
    }
    reasons.extend(stage1.reasons.clone());
    let assurance = stage1.assurance;
    finish(
        root,
        checks,
        reasons,
        Some(&freshness),
        Some(&stage1),
        assurance,
    )
}

fn finish(
    root: &Path,
    checks: BTreeMap<String, CheckStatus>,
    mut reasons: Vec<String>,
    freshness: Option<&FreshnessEvidence>,
    stage1: Option<&IngestCheckSummary>,
    assurance: IngestAssurance,
) -> anyhow::Result<Stage2Summary> {
    reasons.sort();
    reasons.dedup();
    let summary = Stage2Summary {
        status: assurance.as_str().to_string(),
        assurance,
        checks,
        fetch_evidence_path: FETCH_EVIDENCE_PATH.to_string(),
        freshness_evidence_path: freshness.map(|_| FETCH_FRESHNESS_PATH.to_string()),
        stage1_evidence_path: stage1.map(|_| runtime::ASSURANCE_EVIDENCE_PATH.to_string()),
        reasons,
    };
    let path = crate::tools::path_guard::resolve_optional_existing(root, STAGE2_EVIDENCE_PATH)?;
    std::fs::create_dir_all(path.parent().context("stage2 evidence parent missing")?)?;
    crate::evidence_envelope::write_json_for_path(
        &path,
        &summary,
        EvidenceFamily::N,
        STAGE2_EVIDENCE_PATH,
        true,
    )?;
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fetch_probe::{FetchEvidenceEntry, FetchOutcome, write_fetch_evidence};

    #[test]
    fn fetch_failure_leaves_n1_through_n5_not_executed() {
        let root = tempfile::tempdir().unwrap();
        let fetch = FetchEvidence {
            schema_version: "commandagent.fetch-evidence/v0".to_string(),
            run_id: "failed".to_string(),
            contract_ref: "fetch.toml".to_string(),
            contract_sha256: "a".repeat(64),
            entries: vec![FetchEvidenceEntry {
                source_id: "events".to_string(),
                requested_url: "https://events.example.test/events.html".to_string(),
                canonical_url: "https://events.example.test/events.html".to_string(),
                authorization: "contract".to_string(),
                authorization_ref: "fetch.sources[events]".to_string(),
                authorization_sha256: "a".repeat(64),
                fetched_at_utc: "1970-01-01T00:00:00.000Z".to_string(),
                fetched_at_epoch_ms: 0,
                http_status: Some(403),
                content_sha256: None,
                content_bytes: None,
                snapshot_path: "data/snapshots/events.html".to_string(),
                outcome: FetchOutcome::RobotsDenied,
                elapsed_ms: 1,
                remote_ip: Some("8.8.8.8".to_string()),
                redirect_location: None,
                robots: None,
                cache: None,
                failure_kind: Some("robots_denied".to_string()),
            }],
        };
        write_fetch_evidence(root.path(), &fetch).unwrap();
        let summary = connect(root.path(), "fetch.toml", fetch).unwrap();
        assert_eq!(summary.assurance, IngestAssurance::Failed);
        assert_eq!(summary.checks[N6], CheckStatus::NotExecuted);
        for id in [N1, N2, N3, N4, N5] {
            assert_eq!(summary.checks[id], CheckStatus::NotExecuted);
        }
        assert!(summary.stage1_evidence_path.is_none());
        assert!(!root.path().join(runtime::ASSURANCE_EVIDENCE_PATH).exists());
    }
}
