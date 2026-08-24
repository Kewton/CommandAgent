use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Context;
use serde::{Deserialize, Serialize};

use super::contract::ValidatedContract;
use super::{FetchEvidence, FetchOutcome, sha256_hex};
use crate::evidence_envelope::EvidenceFamily;

pub const FETCH_FRESHNESS_PATH: &str = "evidence/fetch-freshness.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FreshnessStatus {
    Pass,
    Violation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FreshnessSourceEvidence {
    pub source_id: String,
    pub fetched_at_epoch_ms: u64,
    pub evaluated_at_epoch_ms: u64,
    pub age_ms: Option<u64>,
    pub max_age_ms: u64,
    pub status: FreshnessStatus,
    pub failure_kind: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FreshnessEvidence {
    pub capability_id: String,
    pub status: FreshnessStatus,
    pub ok: bool,
    pub contract_ref: String,
    pub contract_sha256: String,
    pub fetch_evidence_sha256: String,
    pub evaluated_at_utc: String,
    pub evaluated_at_epoch_ms: u64,
    pub freshness_max_age_seconds: u64,
    pub sources: Vec<FreshnessSourceEvidence>,
    pub failure_kinds: Vec<String>,
}

pub(crate) fn evaluate_and_write(
    root: &Path,
    contract: &ValidatedContract,
    fetch_evidence: &FetchEvidence,
    evaluated_at_epoch_ms: u64,
) -> anyhow::Result<FreshnessEvidence> {
    let fetch_path =
        crate::tools::path_guard::resolve_existing(root, super::evidence::FETCH_EVIDENCE_PATH)
            .context("N6 requires persisted fetch evidence")?;
    let fetch_bytes = std::fs::read(fetch_path)?;
    let fetch_evidence_sha256 = sha256_hex(&fetch_bytes);
    let persisted = serde_json::from_slice::<FetchEvidence>(&fetch_bytes)
        .context("N6 persisted fetch evidence is invalid")?;
    let max_age_ms = contract
        .policy
        .freshness_max_age_seconds
        .checked_mul(1_000)
        .context("N6 max_age overflow")?;
    let expected = contract
        .sources
        .iter()
        .map(|source| (source.source_id.as_str(), source.url.as_str()))
        .collect::<BTreeMap<_, _>>();
    let mut observed = BTreeMap::new();
    let mut sources = Vec::new();
    let mut failure_kinds = Vec::new();

    if &persisted != fetch_evidence {
        failure_kinds.push("ingest_fetch_freshness:evidence_binding_mismatch".to_string());
    }
    if persisted.contract_sha256 != contract.contract_sha256
        || persisted.contract_ref != contract.contract_ref
    {
        failure_kinds.push("ingest_fetch_freshness:contract_binding_mismatch".to_string());
    }
    for entry in &persisted.entries {
        if observed.insert(entry.source_id.as_str(), entry).is_some() {
            failure_kinds.push(format!(
                "ingest_fetch_freshness:duplicate_source:{}",
                entry.source_id
            ));
            continue;
        }
        let mut source_failures = Vec::new();
        if expected.get(entry.source_id.as_str()).copied() != Some(entry.canonical_url.as_str()) {
            source_failures.push("source_binding_mismatch".to_string());
        }
        if !matches!(
            entry.outcome,
            FetchOutcome::Fetched | FetchOutcome::CacheHit
        ) || entry.http_status != Some(200)
            || entry.content_sha256.is_none()
            || entry.content_bytes.is_none()
        {
            source_failures.push("fetch_not_admitted".to_string());
        }
        let age_ms = evaluated_at_epoch_ms.checked_sub(entry.fetched_at_epoch_ms);
        match age_ms {
            None => source_failures.push("clock_reversal".to_string()),
            Some(age) if age > max_age_ms => source_failures.push("stale".to_string()),
            Some(_) => {}
        }
        let status = if source_failures.is_empty() {
            FreshnessStatus::Pass
        } else {
            FreshnessStatus::Violation
        };
        let failure_kind = (!source_failures.is_empty()).then(|| {
            format!(
                "ingest_fetch_freshness:{}:{}",
                entry.source_id,
                source_failures.join("+")
            )
        });
        if let Some(failure) = failure_kind.as_ref() {
            failure_kinds.push(failure.clone());
        }
        sources.push(FreshnessSourceEvidence {
            source_id: entry.source_id.clone(),
            fetched_at_epoch_ms: entry.fetched_at_epoch_ms,
            evaluated_at_epoch_ms,
            age_ms,
            max_age_ms,
            status,
            failure_kind,
        });
    }
    for source_id in expected.keys() {
        if !observed.contains_key(source_id) {
            failure_kinds.push(format!("ingest_fetch_freshness:missing_source:{source_id}"));
        }
    }
    if observed.len() != expected.len() {
        failure_kinds.push(format!(
            "ingest_fetch_freshness:source_count:expected={}:observed={}",
            expected.len(),
            observed.len()
        ));
    }
    failure_kinds.sort();
    failure_kinds.dedup();
    sources.sort_by(|left, right| left.source_id.cmp(&right.source_id));
    let ok = failure_kinds.is_empty()
        && sources
            .iter()
            .all(|source| source.status == FreshnessStatus::Pass);
    let evidence = FreshnessEvidence {
        capability_id: "ingest_fetch_freshness".to_string(),
        status: if ok {
            FreshnessStatus::Pass
        } else {
            FreshnessStatus::Violation
        },
        ok,
        contract_ref: contract.contract_ref.clone(),
        contract_sha256: contract.contract_sha256.clone(),
        fetch_evidence_sha256,
        evaluated_at_utc: super::time::rfc3339_utc(evaluated_at_epoch_ms),
        evaluated_at_epoch_ms,
        freshness_max_age_seconds: contract.policy.freshness_max_age_seconds,
        sources,
        failure_kinds,
    };
    write(root, &evidence)?;
    Ok(evidence)
}

fn write(root: &Path, evidence: &FreshnessEvidence) -> anyhow::Result<()> {
    let path = crate::tools::path_guard::resolve_optional_existing(root, FETCH_FRESHNESS_PATH)?;
    std::fs::create_dir_all(path.parent().context("N6 evidence parent missing")?)?;
    crate::evidence_envelope::write_json_for_path(
        &path,
        evidence,
        EvidenceFamily::N,
        FETCH_FRESHNESS_PATH,
        true,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fetch_probe::contract;

    #[test]
    fn acquisition_time_passes_boundary_and_rejects_stale_or_future_values() {
        let root = tempfile::tempdir().unwrap();
        let contract_text = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/fetch-probe/contracts/valid.toml"
        ));
        std::fs::write(root.path().join("fetch.toml"), contract_text).unwrap();
        let contract = contract::load(root.path(), "fetch.toml").unwrap();
        let make = |fetched_at_epoch_ms| FetchEvidence {
            schema_version: "commandagent.fetch-evidence/v0".to_string(),
            run_id: "n6".to_string(),
            contract_ref: contract.contract_ref.clone(),
            contract_sha256: contract.contract_sha256.clone(),
            entries: vec![super::super::FetchEvidenceEntry {
                source_id: "events".to_string(),
                requested_url: "https://events.example.test/events.html".to_string(),
                canonical_url: "https://events.example.test/events.html".to_string(),
                authorization: "contract".to_string(),
                authorization_ref: "fetch.sources[events]".to_string(),
                authorization_sha256: contract.contract_sha256.clone(),
                fetched_at_utc: super::super::time::rfc3339_utc(fetched_at_epoch_ms),
                fetched_at_epoch_ms,
                http_status: Some(200),
                content_sha256: Some("a".repeat(64)),
                content_bytes: Some(1),
                snapshot_path: "data/snapshots/events.html".to_string(),
                outcome: FetchOutcome::Fetched,
                elapsed_ms: 1,
                remote_ip: Some("8.8.8.8".to_string()),
                redirect_location: None,
                robots: None,
                cache: None,
                failure_kind: None,
            }],
        };
        let write_fetch = |evidence: &FetchEvidence| {
            super::super::write_fetch_evidence(root.path(), evidence).unwrap();
        };
        let now = 2_000_000_000_u64;
        let fresh = make(now - 1_000);
        write_fetch(&fresh);
        assert!(
            evaluate_and_write(root.path(), &contract, &fresh, now)
                .unwrap()
                .ok
        );
        let substituted = make(now - 2_000);
        assert!(
            !evaluate_and_write(root.path(), &contract, &substituted, now)
                .unwrap()
                .ok
        );
        let stale = make(now - 86_400_001);
        write_fetch(&stale);
        assert!(
            !evaluate_and_write(root.path(), &contract, &stale, now)
                .unwrap()
                .ok
        );
        let future = make(now + 1);
        write_fetch(&future);
        assert!(
            !evaluate_and_write(root.path(), &contract, &future, now)
                .unwrap()
                .ok
        );
    }
}
