//! Bounded acquisition boundary for stage-2 ingest.
//!
//! Only this module may invoke the fetch transport. Callers supply a closed
//! contract (added separately) and receive persisted, scrubbed evidence plus a
//! workspace-local snapshot. The LLM tool registry never receives this API.

mod cache;
mod contract;
mod endpoint;
mod evidence;
mod freshness;
mod redaction;
mod robots;
mod time;
mod transport;

use std::collections::BTreeMap;
use std::fs;
use std::net::IpAddr;
use std::path::Path;
use std::thread;
use std::time::Duration;

use anyhow::{Context, bail};
use sha2::{Digest, Sha256};

use contract::{ValidatedContract, ValidatedSource};
use endpoint::{EndpointResolver, FixedResolver, SystemResolver, is_public_ip};
pub use evidence::{
    CacheEvidence, FETCH_EVIDENCE_PATH, FetchEvidence, FetchEvidenceEntry, FetchOutcome,
    RobotsEvidence, write_fetch_evidence,
};
pub use freshness::{
    FETCH_FRESHNESS_PATH, FreshnessEvidence, FreshnessSourceEvidence, FreshnessStatus,
};
pub use redaction::{contains_secret_query, scrub_url_query};
use transport::{
    BoundedCurlTransport, FetchTransport, RecordedTransport, TransportRequest, TransportResponse,
};

/// Fixed UA from fetch-probe-design.md section 11.
pub const USER_AGENT: &str = "CommandAgentFetch/0.1 (+https://github.com/Kewton/CommandAgent)";

trait CourtesyClock: Send + Sync {
    fn wait(&self, duration: Duration);
}

#[derive(Debug, Clone, Copy, Default)]
struct SystemCourtesyClock;

impl CourtesyClock for SystemCourtesyClock {
    fn wait(&self, duration: Duration) {
        thread::sleep(duration);
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct RecordedCourtesyClock;

impl CourtesyClock for RecordedCourtesyClock {
    fn wait(&self, _duration: Duration) {}
}

/// Run the fixed v0 contract with the production DNS resolver and bounded curl
/// child. This is the only live network entry point.
pub fn run_contract(
    root: &Path,
    contract_ref: &str,
    run_id: &str,
) -> anyhow::Result<FetchEvidence> {
    let contract = contract::load(root, contract_ref)?;
    run_validated_contract_with(
        root,
        &contract,
        run_id,
        &BoundedCurlTransport::default(),
        &SystemResolver,
        &SystemCourtesyClock,
    )
}

/// Replay a closed recording through the same contract, robots, cache,
/// publication, and evidence code. This path performs no DNS or network I/O.
pub fn run_recorded_contract(
    root: &Path,
    contract_ref: &str,
    run_id: &str,
    recording_path: &Path,
) -> anyhow::Result<FetchEvidence> {
    let contract = contract::load(root, contract_ref)?;
    let transport = RecordedTransport::from_fixture(recording_path)?;
    let evidence = run_validated_contract_with(
        root,
        &contract,
        run_id,
        &transport,
        &FixedResolver("8.8.8.8".parse().expect("fixed public fixture IP")),
        &RecordedCourtesyClock,
    )?;
    if transport.remaining() != 0
        && evidence
            .entries
            .iter()
            .all(|entry| entry.outcome != FetchOutcome::CacheHit)
    {
        bail!("fetch recording contains unused exchanges");
    }
    Ok(evidence)
}

pub fn evaluate_freshness(
    root: &Path,
    contract_ref: &str,
    fetch_evidence: &FetchEvidence,
) -> anyhow::Result<FreshnessEvidence> {
    let contract = contract::load(root, contract_ref)?;
    freshness::evaluate_and_write(root, &contract, fetch_evidence, time::unix_epoch_ms())
}

fn run_validated_contract_with(
    root: &Path,
    contract: &ValidatedContract,
    run_id: &str,
    transport: &dyn FetchTransport,
    resolver: &dyn EndpointResolver,
    clock: &dyn CourtesyClock,
) -> anyhow::Result<FetchEvidence> {
    let evaluation_ms = time::unix_epoch_ms();
    let utc_date = time::utc_date(evaluation_ms);
    let mut entries = Vec::new();
    let mut request_count = 0usize;
    let mut robots_by_host = BTreeMap::<String, (TransportResponse, u64, IpAddr)>::new();

    for source in &contract.sources {
        let mut entry = FetchEvidenceEntry::started(
            &source.source_id,
            scrub_url_query(&source.url),
            &source.snapshot_path,
            &contract.contract_sha256,
            evaluation_ms,
        );
        let cache_key = match cache::lookup(root, contract, source, &utc_date) {
            Ok(cache::CacheLookup::Hit(hit)) => {
                let cache::CacheHit {
                    body,
                    fetched_at_epoch_ms,
                    fetched_at_utc,
                    http_status,
                    content_sha256,
                    content_bytes,
                    robots,
                    evidence,
                } = *hit;
                if let Err(error) = publish_snapshot(root, &source.snapshot_path, &body) {
                    fail_entry(
                        &mut entry,
                        FetchOutcome::Failed,
                        "fetch_cache_publish_failed",
                        error,
                    );
                } else {
                    entry.fetched_at_epoch_ms = fetched_at_epoch_ms;
                    entry.fetched_at_utc = fetched_at_utc;
                    entry.http_status = Some(http_status);
                    entry.content_sha256 = Some(content_sha256);
                    entry.content_bytes = Some(content_bytes);
                    entry.robots = Some(robots);
                    entry.cache = Some(evidence);
                    entry.outcome = FetchOutcome::CacheHit;
                }
                let passed = entry.outcome == FetchOutcome::CacheHit;
                entries.push(entry);
                if !passed {
                    break;
                }
                continue;
            }
            Ok(cache::CacheLookup::Miss { cache_key_sha256 }) => cache_key_sha256,
            Err(error) => {
                fail_entry(
                    &mut entry,
                    FetchOutcome::CacheCorrupt,
                    "fetch_cache_corrupt",
                    error,
                );
                entries.push(entry);
                break;
            }
        };

        let resolved_ip = match resolver.resolve(&source.host, source.port) {
            Ok(ip) => ip,
            Err(error) => {
                fail_entry(
                    &mut entry,
                    FetchOutcome::Failed,
                    "fetch_endpoint_rejected",
                    error,
                );
                entries.push(entry);
                break;
            }
        };

        let robots_url = format!("https://{}/robots.txt", source.host);
        let (robots_response, robots_checked_ms, robots_ip) =
            if let Some(cached) = robots_by_host.get(&source.host) {
                cached.clone()
            } else {
                let checked_ms = time::unix_epoch_ms();
                let response = match bounded_get(
                    root,
                    contract,
                    transport,
                    source,
                    &robots_url,
                    resolved_ip,
                    &mut request_count,
                ) {
                    Ok(response) => response,
                    Err(error) => {
                        fail_entry(
                            &mut entry,
                            FetchOutcome::RobotsDenied,
                            "robots_unavailable",
                            error,
                        );
                        entries.push(entry);
                        break;
                    }
                };
                robots_by_host.insert(
                    source.host.clone(),
                    (response.clone(), checked_ms, resolved_ip),
                );
                (response, checked_ms, resolved_ip)
            };
        if let Err(error) = verify_peer(&robots_response, robots_ip) {
            fail_entry(
                &mut entry,
                FetchOutcome::RobotsDenied,
                "robots_peer_mismatch",
                error,
            );
            entries.push(entry);
            break;
        }
        let decision = robots::decide(
            robots_response.http_status,
            &robots_response.body,
            &source.path_for_robots,
        );
        let (decision_name, decision_result) = match decision {
            Ok(decision) if decision.allowed => ("allow", Ok(decision)),
            Ok(_decision) => ("deny", Err(anyhow::anyhow!("robots_denied:rule"))),
            Err(error) => ("deny", Err(error)),
        };
        let decision_for_evidence = decision_result.as_ref().ok();
        let robots_evidence = RobotsEvidence {
            robots_url: robots_url.clone(),
            checked_at_utc: time::rfc3339_utc(robots_checked_ms),
            checked_at_epoch_ms: robots_checked_ms,
            http_status: robots_response.http_status,
            evidence_sha256: sha256_hex(&robots_response.body),
            decision: decision_name.to_string(),
            rule_group: decision_for_evidence
                .map(|decision| decision.rule_group.clone())
                .unwrap_or_else(|| "none".to_string()),
            crawl_delay_ms: decision_for_evidence
                .map(|decision| decision.crawl_delay_ms)
                .unwrap_or(0),
            matched_rule: decision_for_evidence.and_then(|decision| decision.matched_rule.clone()),
        };
        entry.robots = Some(robots_evidence.clone());
        let decision = match decision_result {
            Ok(decision) => decision,
            Err(error) => {
                fail_entry(
                    &mut entry,
                    FetchOutcome::RobotsDenied,
                    "robots_denied",
                    error,
                );
                entries.push(entry);
                break;
            }
        };
        clock.wait(Duration::from_millis(
            contract
                .policy
                .min_origin_interval_ms
                .max(decision.crawl_delay_ms),
        ));
        let response = match bounded_get(
            root,
            contract,
            transport,
            source,
            &source.url,
            resolved_ip,
            &mut request_count,
        ) {
            Ok(response) => response,
            Err(error) => {
                fail_entry(
                    &mut entry,
                    FetchOutcome::Failed,
                    "fetch_transport_failed",
                    error,
                );
                entries.push(entry);
                break;
            }
        };
        entry.http_status = Some(response.http_status);
        entry.elapsed_ms = response.elapsed_ms;
        entry.remote_ip = response.remote_ip.clone();
        entry.redirect_location = response.redirect_location.as_deref().map(scrub_url_query);
        if let Err(error) = verify_peer(&response, resolved_ip) {
            fail_entry(
                &mut entry,
                FetchOutcome::Failed,
                "fetch_peer_mismatch",
                error,
            );
        } else if (300..400).contains(&response.http_status) {
            entry.outcome = FetchOutcome::RedirectRejected;
            entry.failure_kind = Some(format!(
                "fetch_redirect_rejected:http_status={}",
                response.http_status
            ));
        } else if response.http_status != 200 {
            entry.outcome = FetchOutcome::HttpRejected;
            entry.failure_kind = Some(format!(
                "fetch_http_status_rejected:{}",
                response.http_status
            ));
        } else if response.body.len() as u64 > contract.policy.max_response_bytes {
            entry.outcome = FetchOutcome::Failed;
            entry.failure_kind = Some("fetch_response_too_large".to_string());
        } else {
            let content_sha256 = sha256_hex(&response.body);
            let fetched_at_epoch_ms = time::unix_epoch_ms();
            let fetched_at_utc = time::rfc3339_utc(fetched_at_epoch_ms);
            match cache::store(
                root,
                contract,
                source,
                &response.body,
                cache::CacheStore {
                    cache_key_sha256: &cache_key,
                    utc_date: &utc_date,
                    fetched_at_epoch_ms,
                    fetched_at_utc: &fetched_at_utc,
                    content_sha256: &content_sha256,
                    robots: &robots_evidence,
                },
            )
            .and_then(|cache| {
                publish_snapshot(root, &source.snapshot_path, &response.body)?;
                Ok(cache)
            }) {
                Ok(cache) => {
                    entry.fetched_at_epoch_ms = fetched_at_epoch_ms;
                    entry.fetched_at_utc = fetched_at_utc;
                    entry.content_sha256 = Some(content_sha256);
                    entry.content_bytes = Some(response.body.len() as u64);
                    entry.cache = Some(cache);
                    entry.outcome = FetchOutcome::Fetched;
                }
                Err(error) => fail_entry(
                    &mut entry,
                    FetchOutcome::Failed,
                    "fetch_publication_failed",
                    error,
                ),
            }
        }
        let passed = entry.outcome == FetchOutcome::Fetched;
        entries.push(entry);
        if !passed {
            break;
        }
    }

    let evidence = FetchEvidence {
        schema_version: "commandagent.fetch-evidence/v0".to_string(),
        run_id: run_id.to_string(),
        contract_ref: contract.contract_ref.clone(),
        contract_sha256: contract.contract_sha256.clone(),
        entries,
    };
    write_fetch_evidence(root, &evidence)?;
    Ok(evidence)
}

fn bounded_get(
    root: &Path,
    contract: &ValidatedContract,
    transport: &dyn FetchTransport,
    source: &ValidatedSource,
    url: &str,
    resolved_ip: IpAddr,
    request_count: &mut usize,
) -> anyhow::Result<TransportResponse> {
    if *request_count >= contract.policy.max_http_requests {
        bail!("fetch_http_request_cap_exhausted");
    }
    *request_count += 1;
    transport.get(
        root,
        &TransportRequest {
            url: url.to_string(),
            host: source.host.clone(),
            port: source.port,
            resolved_ip: resolved_ip.to_string(),
            user_agent: USER_AGENT.to_string(),
            timeout_seconds: contract.policy.timeout_seconds,
            max_response_bytes: contract.policy.max_response_bytes,
        },
    )
}

fn verify_peer(response: &TransportResponse, expected: IpAddr) -> anyhow::Result<()> {
    let observed = response
        .remote_ip
        .as_deref()
        .context("fetch transport omitted remote IP")?
        .parse::<IpAddr>()
        .context("fetch transport returned invalid remote IP")?;
    if observed != expected || !is_public_ip(observed) {
        bail!("fetch remote peer did not match the pinned public address");
    }
    Ok(())
}

fn fail_entry(
    entry: &mut FetchEvidenceEntry,
    outcome: FetchOutcome,
    kind: &str,
    error: anyhow::Error,
) {
    entry.outcome = outcome;
    entry.failure_kind = Some(format!("{kind}:{error:#}"));
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
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&staged)?;
    if let Err(error) = std::io::Write::write_all(&mut file, bytes).and_then(|()| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(&staged);
        return Err(error.into());
    }
    drop(file);
    if destination.exists() {
        if fs::read(&destination)? == bytes {
            let _ = fs::remove_file(&staged);
            return Ok(());
        }
        let _ = fs::remove_file(&staged);
        bail!("fetch snapshot exists with different bytes: {relative}");
    }
    if let Err(error) = fs::rename(&staged, &destination) {
        let _ = fs::remove_file(&staged);
        return Err(error.into());
    }
    Ok(())
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
