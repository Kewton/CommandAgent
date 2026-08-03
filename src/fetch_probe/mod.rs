//! Bounded acquisition boundary for stage-2 ingest.
//!
//! Only this module may invoke the fetch transport. Callers supply a closed
//! contract (added separately) and receive persisted, scrubbed evidence plus a
//! workspace-local snapshot. The LLM tool registry never receives this API.

mod evidence;
mod redaction;
mod time;
mod transport;

use std::fs;
use std::path::Path;

use anyhow::{Context, bail};
use sha2::{Digest, Sha256};

pub use evidence::{
    FETCH_EVIDENCE_PATH, FetchEvidence, FetchEvidenceEntry, FetchOutcome, write_fetch_evidence,
};
pub use redaction::{contains_secret_query, scrub_url_query};
pub use transport::{
    BoundedCurlTransport, FetchTransport, RecordedExchange, RecordedTransport, TransportRequest,
    TransportResponse,
};

/// Fixed UA from fetch-probe-design.md section 11.
pub const USER_AGENT: &str = "CommandAgentFetch/0.1 (+https://github.com/Kewton/CommandAgent)";

/// A pre-authorized request used by the boundary before the closed contract is
/// wired in. Commit 3 makes construction contingent on contract validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizedFetch {
    pub run_id: String,
    pub contract_ref: String,
    pub contract_sha256: String,
    pub source_id: String,
    pub canonical_url: String,
    pub snapshot_path: String,
    pub timeout_seconds: u16,
    pub max_response_bytes: u64,
}

/// Execute one already-authorized GET, publish only an HTTP-200 body, and write
/// one fetch-evidence envelope. Contract, robots, and cache orchestration are
/// layered on this single entry point.
pub fn fetch_authorized_with(
    root: &Path,
    request: &AuthorizedFetch,
    transport: &dyn FetchTransport,
) -> anyhow::Result<FetchEvidence> {
    crate::tools::path_guard::validate_workspace_relative(&request.snapshot_path)
        .context("fetch snapshot path is not workspace-relative")?;
    let started_ms = time::unix_epoch_ms();
    let transport_request = TransportRequest {
        url: request.canonical_url.clone(),
        user_agent: USER_AGENT.to_string(),
        timeout_seconds: request.timeout_seconds,
        max_response_bytes: request.max_response_bytes,
    };
    let result = transport.get(root, &transport_request);
    let mut entry = FetchEvidenceEntry::started(
        &request.source_id,
        scrub_url_query(&request.canonical_url),
        &request.snapshot_path,
        started_ms,
    );

    match result {
        Ok(response) => {
            entry.http_status = Some(response.http_status);
            entry.elapsed_ms = response.elapsed_ms;
            entry.redirect_location = response.redirect_location.as_deref().map(scrub_url_query);
            entry.remote_ip = response.remote_ip;
            if response.http_status != 200 {
                entry.outcome = if (300..400).contains(&response.http_status) {
                    FetchOutcome::RedirectRejected
                } else {
                    FetchOutcome::HttpRejected
                };
                entry.failure_kind = Some(format!(
                    "fetch_http_status_rejected:{}",
                    response.http_status
                ));
            } else if response.body.len() as u64 > request.max_response_bytes {
                entry.outcome = FetchOutcome::Failed;
                entry.failure_kind = Some("fetch_response_too_large".to_string());
            } else {
                let hash = sha256_hex(&response.body);
                publish_snapshot(root, &request.snapshot_path, &response.body)?;
                entry.content_sha256 = Some(hash);
                entry.content_bytes = Some(response.body.len() as u64);
                entry.outcome = FetchOutcome::Fetched;
            }
        }
        Err(error) => {
            entry.outcome = FetchOutcome::Failed;
            entry.failure_kind = Some(format!("fetch_transport_failed:{error:#}"));
        }
    }

    let evidence = FetchEvidence {
        schema_version: "commandagent.fetch-evidence/v0".to_string(),
        run_id: request.run_id.clone(),
        contract_ref: request.contract_ref.clone(),
        contract_sha256: request.contract_sha256.clone(),
        entries: vec![entry],
    };
    write_fetch_evidence(root, &evidence)?;
    Ok(evidence)
}

fn publish_snapshot(root: &Path, relative: &str, bytes: &[u8]) -> anyhow::Result<()> {
    let destination = crate::tools::path_guard::resolve_for_create(root, relative)?;
    let parent = destination
        .parent()
        .context("fetch snapshot parent missing")?;
    fs::create_dir_all(parent)?;
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .context("fetch snapshot file name is not UTF-8")?;
    let staged = parent.join(format!(".{file_name}.fetch-part"));
    if staged.exists() {
        fs::remove_file(&staged)?;
    }
    let mut file = fs::File::create(&staged)?;
    std::io::Write::write_all(&mut file, bytes)?;
    file.sync_all()?;
    drop(file);
    if destination.exists() {
        bail!("fetch snapshot already exists: {relative}");
    }
    fs::rename(&staged, &destination)?;
    Ok(())
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorized_fetch_writes_exact_snapshot_and_scrubbed_evidence() {
        let root = tempfile::tempdir().unwrap();
        let secret = "deliberate-secret-value";
        let request = AuthorizedFetch {
            run_id: "recorded-001".to_string(),
            contract_ref: "fetch.toml".to_string(),
            contract_sha256: "a".repeat(64),
            source_id: "events".to_string(),
            canonical_url: format!("https://data.example/events?token={secret}&page=1"),
            snapshot_path: "data/snapshots/events.html".to_string(),
            timeout_seconds: 3,
            max_response_bytes: 1024,
        };
        let transport = RecordedTransport::new([RecordedExchange::ok(
            "https://data.example/events?token=%3CREDACTED%3E&page=1",
            b"<html>fixture</html>",
        )]);

        let evidence = fetch_authorized_with(root.path(), &request, &transport).unwrap();

        assert_eq!(
            fs::read(root.path().join(&request.snapshot_path)).unwrap(),
            b"<html>fixture</html>"
        );
        let entry = &evidence.entries[0];
        assert_eq!(entry.outcome, FetchOutcome::Fetched);
        assert_eq!(entry.http_status, Some(200));
        assert_eq!(entry.snapshot_path, "data/snapshots/events.html");
        assert!(entry.fetched_at_utc.ends_with('Z'));
        assert!(entry.fetched_at_epoch_ms > 0);
        let serialized = fs::read_to_string(root.path().join(FETCH_EVIDENCE_PATH)).unwrap();
        assert!(!serialized.contains(secret));
        assert!(serialized.contains("%3CREDACTED%3E"));
        assert!(serialized.contains(&sha256_hex(b"<html>fixture</html>")));
    }
}
