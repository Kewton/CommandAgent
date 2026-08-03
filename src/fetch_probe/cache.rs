use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

use super::contract::{ValidatedContract, ValidatedSource};
use super::evidence::{CacheEvidence, RobotsEvidence};
use super::sha256_hex;

const CACHE_DIR: &str = "evidence/fetch-cache";
const CACHE_POLICY: &str = "canonical-url-utc-day";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CacheMetadata {
    schema_version: String,
    contract_sha256: String,
    canonical_url: String,
    utc_date: String,
    cache_key_sha256: String,
    fetched_at_epoch_ms: u64,
    fetched_at_utc: String,
    http_status: u16,
    content_sha256: String,
    content_bytes: u64,
    robots: RobotsEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheLookup {
    Miss { cache_key_sha256: String },
    Hit(Box<CacheHit>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheHit {
    pub body: Vec<u8>,
    pub fetched_at_epoch_ms: u64,
    pub fetched_at_utc: String,
    pub http_status: u16,
    pub content_sha256: String,
    pub content_bytes: u64,
    pub robots: RobotsEvidence,
    pub evidence: CacheEvidence,
}

pub fn lookup(
    root: &Path,
    contract: &ValidatedContract,
    source: &ValidatedSource,
    utc_date: &str,
) -> anyhow::Result<CacheLookup> {
    let key = cache_key(&source.url, utc_date);
    let (metadata_path, body_path) = cache_paths(root, &key)?;
    let metadata_exists = metadata_path.exists();
    let body_exists = body_path.exists();
    if !metadata_exists && !body_exists {
        return Ok(CacheLookup::Miss {
            cache_key_sha256: key,
        });
    }
    if metadata_exists != body_exists {
        bail!("fetch_cache_corrupt:metadata_body_pair_incomplete");
    }
    let metadata_bytes = fs::read(&metadata_path).context("fetch_cache_corrupt:read_metadata")?;
    let metadata = serde_json::from_slice::<CacheMetadata>(&metadata_bytes)
        .context("fetch_cache_corrupt:parse_metadata")?;
    if metadata.schema_version != "commandagent.fetch-cache/v0"
        || metadata.contract_sha256 != contract.contract_sha256
        || metadata.canonical_url != source.url
        || metadata.utc_date != utc_date
        || metadata.cache_key_sha256 != key
        || metadata.http_status != 200
    {
        bail!("fetch_cache_corrupt:metadata_binding_mismatch");
    }
    let body = fs::read(&body_path).context("fetch_cache_corrupt:read_body")?;
    if body.len() as u64 != metadata.content_bytes || sha256_hex(&body) != metadata.content_sha256 {
        bail!("fetch_cache_corrupt:content_hash_mismatch");
    }
    Ok(CacheLookup::Hit(Box::new(CacheHit {
        body,
        fetched_at_epoch_ms: metadata.fetched_at_epoch_ms,
        fetched_at_utc: metadata.fetched_at_utc,
        http_status: metadata.http_status,
        content_sha256: metadata.content_sha256,
        content_bytes: metadata.content_bytes,
        robots: metadata.robots,
        evidence: CacheEvidence {
            policy: CACHE_POLICY.to_string(),
            utc_date: utc_date.to_string(),
            cache_key_sha256: key,
            source_fetched_at_epoch_ms: metadata.fetched_at_epoch_ms,
        },
    })))
}

pub struct CacheStore<'a> {
    pub cache_key_sha256: &'a str,
    pub utc_date: &'a str,
    pub fetched_at_epoch_ms: u64,
    pub fetched_at_utc: &'a str,
    pub content_sha256: &'a str,
    pub robots: &'a RobotsEvidence,
}

pub fn store(
    root: &Path,
    contract: &ValidatedContract,
    source: &ValidatedSource,
    body: &[u8],
    value: CacheStore<'_>,
) -> anyhow::Result<CacheEvidence> {
    if value.cache_key_sha256 != cache_key(&source.url, value.utc_date) {
        bail!("fetch cache key changed before publication");
    }
    let (metadata_path, body_path) = cache_paths(root, value.cache_key_sha256)?;
    fs::create_dir_all(
        metadata_path
            .parent()
            .context("fetch cache parent missing")?,
    )?;
    if metadata_path.exists() || body_path.exists() {
        bail!("fetch cache destination already exists");
    }
    let metadata = CacheMetadata {
        schema_version: "commandagent.fetch-cache/v0".to_string(),
        contract_sha256: contract.contract_sha256.clone(),
        canonical_url: source.url.clone(),
        utc_date: value.utc_date.to_string(),
        cache_key_sha256: value.cache_key_sha256.to_string(),
        fetched_at_epoch_ms: value.fetched_at_epoch_ms,
        fetched_at_utc: value.fetched_at_utc.to_string(),
        http_status: 200,
        content_sha256: value.content_sha256.to_string(),
        content_bytes: body.len() as u64,
        robots: value.robots.clone(),
    };
    let mut metadata_bytes = serde_json::to_vec_pretty(&metadata)?;
    metadata_bytes.push(b'\n');
    atomic_write(&body_path, body)?;
    if let Err(error) = atomic_write(&metadata_path, &metadata_bytes) {
        let _ = fs::remove_file(&body_path);
        return Err(error);
    }
    Ok(CacheEvidence {
        policy: CACHE_POLICY.to_string(),
        utc_date: value.utc_date.to_string(),
        cache_key_sha256: value.cache_key_sha256.to_string(),
        source_fetched_at_epoch_ms: value.fetched_at_epoch_ms,
    })
}

fn cache_key(url: &str, utc_date: &str) -> String {
    sha256_hex(format!("{url}\n{utc_date}").as_bytes())
}

fn cache_paths(root: &Path, key: &str) -> anyhow::Result<(PathBuf, PathBuf)> {
    let metadata = crate::tools::path_guard::resolve_optional_existing(
        root,
        &format!("{CACHE_DIR}/{key}.json"),
    )?;
    let body = crate::tools::path_guard::resolve_optional_existing(
        root,
        &format!("{CACHE_DIR}/{key}.body"),
    )?;
    Ok((metadata, body))
}

fn atomic_write(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .context("fetch cache file name is not UTF-8")?;
    let staged = path.with_file_name(format!(".{file_name}.part"));
    let mut file = fs::File::create(&staged)?;
    std::io::Write::write_all(&mut file, bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&staged, path)?;
    Ok(())
}
