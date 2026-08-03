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
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FetchEvidenceEntry {
    pub source_id: String,
    pub requested_url: String,
    pub canonical_url: String,
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
    pub failure_kind: Option<String>,
}

impl FetchEvidenceEntry {
    pub(crate) fn started(
        source_id: &str,
        url: String,
        snapshot_path: &str,
        epoch_ms: u64,
    ) -> Self {
        Self {
            source_id: source_id.to_string(),
            requested_url: url.clone(),
            canonical_url: url,
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
