use std::fs;
use std::path::{Path, PathBuf};

use commandagent::fetch_probe::{FETCH_EVIDENCE_PATH, FetchOutcome, run_recorded_contract};

const CONTRACT_ROOT: &str = "tests/fixtures/fetch-probe/contracts";
const RECORDING_ROOT: &str = "tests/fixtures/fetch-probe/recordings";

fn repository_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn workspace_with_contract(name: &str) -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    fs::copy(
        repository_path(&format!("{CONTRACT_ROOT}/{name}")),
        root.path().join("fetch.toml"),
    )
    .unwrap();
    root
}

fn recording(name: &str) -> PathBuf {
    repository_path(&format!("{RECORDING_ROOT}/{name}"))
}

#[test]
fn closed_contract_rejects_outside_domain_and_http_before_transport() {
    for (contract, marker) in [
        ("invalid-domain.toml", "outside allowed_domains"),
        ("invalid-http.toml", "must use HTTPS"),
    ] {
        let root = workspace_with_contract(contract);
        let error = run_recorded_contract(
            root.path(),
            "fetch.toml",
            "negative-contract",
            &recording("empty.json"),
        )
        .unwrap_err();
        assert!(error.to_string().contains(marker), "{error:#}");
        assert!(!root.path().join(FETCH_EVIDENCE_PATH).exists());
    }
}

#[test]
fn redirect_is_recorded_scrubbed_and_rejected_without_snapshot() {
    let root = workspace_with_contract("valid.toml");
    let evidence = run_recorded_contract(
        root.path(),
        "fetch.toml",
        "redirect-001",
        &recording("redirect-301.json"),
    )
    .unwrap();
    let entry = &evidence.entries[0];
    assert_eq!(entry.outcome, FetchOutcome::RedirectRejected);
    assert_eq!(entry.http_status, Some(301));
    assert_eq!(
        entry.redirect_location.as_deref(),
        Some("https://events.example.test/new?token=%3CREDACTED%3E")
    );
    assert!(!root.path().join(&entry.snapshot_path).exists());
    let serialized = fs::read_to_string(root.path().join(FETCH_EVIDENCE_PATH)).unwrap();
    assert!(!serialized.contains("fixture-secret"));
}

#[test]
fn robots_403_fails_closed_before_content_request() {
    let root = workspace_with_contract("valid.toml");
    let evidence = run_recorded_contract(
        root.path(),
        "fetch.toml",
        "robots-403-001",
        &recording("robots-403.json"),
    )
    .unwrap();
    let entry = &evidence.entries[0];
    assert_eq!(entry.outcome, FetchOutcome::RobotsDenied);
    assert!(
        entry
            .failure_kind
            .as_deref()
            .unwrap()
            .contains("robots_denied")
    );
    assert_eq!(entry.robots.as_ref().unwrap().http_status, 403);
    assert!(!root.path().join(&entry.snapshot_path).exists());
}

#[test]
fn same_day_cache_hit_uses_zero_recorded_exchanges_and_original_time() {
    let root = workspace_with_contract("valid.toml");
    let first = run_recorded_contract(
        root.path(),
        "fetch.toml",
        "cache-first",
        &recording("success.json"),
    )
    .unwrap();
    assert_eq!(first.entries[0].outcome, FetchOutcome::Fetched);
    let second = run_recorded_contract(
        root.path(),
        "fetch.toml",
        "cache-second",
        &recording("empty.json"),
    )
    .unwrap();
    assert_eq!(second.entries[0].outcome, FetchOutcome::CacheHit);
    assert_eq!(
        second.entries[0].fetched_at_epoch_ms,
        first.entries[0].fetched_at_epoch_ms
    );
    assert_eq!(
        second.entries[0].content_sha256,
        first.entries[0].content_sha256
    );
}

#[test]
fn corrupt_cache_fails_without_implicit_refetch() {
    let root = workspace_with_contract("valid.toml");
    run_recorded_contract(
        root.path(),
        "fetch.toml",
        "cache-prime",
        &recording("success.json"),
    )
    .unwrap();
    let cache_dir = root.path().join("evidence/fetch-cache");
    let body = fs::read_dir(&cache_dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|ext| ext == "body"))
        .unwrap();
    fs::write(body, b"corrupt").unwrap();

    let evidence = run_recorded_contract(
        root.path(),
        "fetch.toml",
        "cache-corrupt",
        &recording("empty.json"),
    )
    .unwrap();
    assert_eq!(evidence.entries[0].outcome, FetchOutcome::CacheCorrupt);
    assert!(
        evidence.entries[0]
            .failure_kind
            .as_deref()
            .unwrap()
            .contains("content_hash_mismatch")
    );
}
