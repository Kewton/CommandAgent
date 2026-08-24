use std::path::Path;

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::evidence_envelope::EvidenceFamily;

pub const FETCH_EVIDENCE_PATH: &str = "evidence/fetch-evidence.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FetchOutcome {
    Pending,
    Fetched,
    CacheHit,
    RedirectRejected,
    HttpRejected,
    RobotsDenied,
    CacheCorrupt,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FetchEvidenceEntry {
    pub source_id: String,
    pub requested_url: String,
    pub canonical_url: String,
    pub authorization: String,
    pub authorization_ref: String,
    pub authorization_sha256: String,
    pub fetched_at_utc: String,
    pub fetched_at_epoch_ms: u64,
    pub http_status: Option<u16>,
    pub content_sha256: Option<String>,
    pub content_bytes: Option<u64>,
    pub snapshot_path: String,
    pub outcome: FetchOutcome,
    pub elapsed_ms: u64,
    pub remote_ip: Option<String>,
    pub redirect_location: Option<String>,
    pub robots: Option<RobotsEvidence>,
    pub cache: Option<CacheEvidence>,
    pub failure_kind: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RobotsEvidence {
    pub robots_url: String,
    pub checked_at_utc: String,
    pub checked_at_epoch_ms: u64,
    pub http_status: u16,
    pub evidence_sha256: String,
    pub decision: String,
    pub rule_group: String,
    pub crawl_delay_ms: u64,
    pub matched_rule: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheEvidence {
    pub policy: String,
    pub utc_date: String,
    pub cache_key_sha256: String,
    pub source_fetched_at_epoch_ms: u64,
}

impl FetchEvidenceEntry {
    pub(crate) fn started(
        source_id: &str,
        url: String,
        snapshot_path: &str,
        contract_sha256: &str,
        epoch_ms: u64,
    ) -> Self {
        Self {
            source_id: source_id.to_string(),
            requested_url: url.clone(),
            canonical_url: url,
            authorization: "contract".to_string(),
            authorization_ref: format!("fetch.sources[{source_id}]"),
            authorization_sha256: contract_sha256.to_string(),
            fetched_at_utc: super::time::rfc3339_utc(epoch_ms),
            fetched_at_epoch_ms: epoch_ms,
            http_status: None,
            content_sha256: None,
            content_bytes: None,
            snapshot_path: snapshot_path.to_string(),
            outcome: FetchOutcome::Pending,
            elapsed_ms: 0,
            remote_ip: None,
            redirect_location: None,
            robots: None,
            cache: None,
            failure_kind: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FetchEvidence {
    pub schema_version: String,
    pub run_id: String,
    pub contract_ref: String,
    pub contract_sha256: String,
    pub entries: Vec<FetchEvidenceEntry>,
}

pub fn write_fetch_evidence(root: &Path, evidence: &FetchEvidence) -> anyhow::Result<()> {
    let path = crate::tools::path_guard::resolve_for_create(root, FETCH_EVIDENCE_PATH)
        .context("fetch evidence path escapes workspace")?;
    std::fs::create_dir_all(path.parent().context("fetch evidence parent missing")?)?;
    crate::evidence_envelope::write_json_for_path(
        &path,
        evidence,
        EvidenceFamily::N,
        FETCH_EVIDENCE_PATH,
        true,
    )
}
