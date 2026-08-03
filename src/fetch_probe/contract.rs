use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, bail};
use serde::Deserialize;
use url::{Host, Url};

use super::{USER_AGENT, sha256_hex};

pub const SCHEMA_VERSION: &str = "commandagent.fetch/v0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedContract {
    pub contract_ref: String,
    pub contract_sha256: String,
    pub policy: FetchPolicy,
    pub sources: Vec<ValidatedSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContractFile {
    fetch: FetchTable,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct FetchTable {
    schema_version: String,
    allowed_domains: Vec<String>,
    max_fetches: usize,
    max_http_requests: usize,
    timeout_seconds: u16,
    max_response_bytes: u64,
    freshness_max_age_seconds: u64,
    cache_policy: String,
    robots_policy: String,
    user_agent: String,
    min_origin_interval_ms: u64,
    redirect_policy: String,
    sources: Vec<FetchSource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchPolicy {
    pub allowed_domains: BTreeSet<String>,
    pub max_fetches: usize,
    pub max_http_requests: usize,
    pub timeout_seconds: u16,
    pub max_response_bytes: u64,
    pub freshness_max_age_seconds: u64,
    pub min_origin_interval_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct FetchSource {
    source_id: String,
    url: String,
    snapshot_path: String,
    authorization: Authorization,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Authorization {
    Contract,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedSource {
    pub source_id: String,
    pub url: String,
    pub host: String,
    pub port: u16,
    pub path_for_robots: String,
    pub snapshot_path: String,
}

pub fn load(root: &Path, relative: &str) -> anyhow::Result<ValidatedContract> {
    let path = crate::tools::path_guard::resolve_existing(root, relative)
        .context("fetch contract path must be an existing workspace-relative file")?;
    let bytes = std::fs::read(&path).context("read fetch contract")?;
    let text = std::str::from_utf8(&bytes).context("fetch contract must be UTF-8")?;
    let file = toml::from_str::<ContractFile>(text).context("parse closed fetch contract")?;
    validate(relative, &bytes, file)
}

fn validate(relative: &str, bytes: &[u8], file: ContractFile) -> anyhow::Result<ValidatedContract> {
    let fetch = file.fetch;
    if fetch.schema_version != SCHEMA_VERSION {
        bail!("unsupported fetch schema_version: {}", fetch.schema_version);
    }
    if fetch.cache_policy != "canonical-url-utc-day" {
        bail!("fetch cache_policy must be canonical-url-utc-day");
    }
    if fetch.robots_policy != "respect" {
        bail!("fetch robots_policy must be respect");
    }
    if fetch.redirect_policy != "reject" {
        bail!("fetch redirect_policy must be reject");
    }
    if fetch.user_agent != USER_AGENT {
        bail!("fetch user_agent must equal the fixed product UA");
    }
    if fetch.max_fetches == 0 || fetch.sources.is_empty() || fetch.sources.len() > fetch.max_fetches
    {
        bail!("fetch sources must be non-empty and within max_fetches");
    }
    if fetch.max_http_requests == 0 {
        bail!("fetch max_http_requests must be positive");
    }
    if fetch.timeout_seconds == 0 || fetch.timeout_seconds > 300 {
        bail!("fetch timeout_seconds must be within 1..=300");
    }
    if fetch.max_response_bytes == 0 {
        bail!("fetch max_response_bytes must be positive");
    }
    if fetch.min_origin_interval_ms < 1_000 {
        bail!("fetch min_origin_interval_ms must be at least 1000");
    }
    let allowed_domains = validate_domains(fetch.allowed_domains)?;
    let mut source_ids = BTreeSet::new();
    let mut urls = BTreeSet::new();
    let mut snapshots = BTreeSet::new();
    let mut sources = Vec::with_capacity(fetch.sources.len());
    for source in fetch.sources {
        if !source_ids.insert(source.source_id.clone()) || source.source_id.trim().is_empty() {
            bail!("fetch source_id values must be non-empty and unique");
        }
        if !urls.insert(source.url.clone()) {
            bail!("fetch source URLs must be unique");
        }
        if !snapshots.insert(source.snapshot_path.clone()) {
            bail!("fetch snapshot_path values must be unique");
        }
        if source.authorization != Authorization::Contract {
            bail!("fetch source authorization must be contract in v0");
        }
        sources.push(validate_source(source, &allowed_domains)?);
    }
    Ok(ValidatedContract {
        contract_ref: relative.to_string(),
        contract_sha256: sha256_hex(bytes),
        policy: FetchPolicy {
            allowed_domains,
            max_fetches: fetch.max_fetches,
            max_http_requests: fetch.max_http_requests,
            timeout_seconds: fetch.timeout_seconds,
            max_response_bytes: fetch.max_response_bytes,
            freshness_max_age_seconds: fetch.freshness_max_age_seconds,
            min_origin_interval_ms: fetch.min_origin_interval_ms,
        },
        sources,
    })
}

fn validate_domains(domains: Vec<String>) -> anyhow::Result<BTreeSet<String>> {
    if domains.is_empty() {
        bail!("fetch allowed_domains must not be empty");
    }
    let mut out = BTreeSet::new();
    for domain in domains {
        if domain.is_empty()
            || domain != domain.to_ascii_lowercase()
            || domain.starts_with('.')
            || domain.ends_with('.')
            || domain.contains('*')
            || domain.contains(['/', ':', '@'])
            || domain == "localhost"
            || domain.parse::<std::net::IpAddr>().is_ok()
        {
            bail!("fetch allowed domain is not an exact canonical DNS name: {domain}");
        }
        match Host::parse(&domain) {
            Ok(Host::Domain(canonical)) if canonical == domain => {}
            _ => bail!("fetch allowed domain is not an exact canonical DNS name: {domain}"),
        }
        if !out.insert(domain.clone()) {
            bail!("fetch allowed_domains contains a duplicate: {domain}");
        }
    }
    Ok(out)
}

fn validate_source(
    source: FetchSource,
    allowed_domains: &BTreeSet<String>,
) -> anyhow::Result<ValidatedSource> {
    let parsed = Url::parse(&source.url).context("fetch source URL is invalid")?;
    if parsed.scheme() != "https" {
        bail!("fetch source URL must use HTTPS");
    }
    if !parsed.username().is_empty() || parsed.password().is_some() || parsed.fragment().is_some() {
        bail!("fetch source URL may not contain user-info or a fragment");
    }
    if parsed.port().is_some() {
        bail!("fetch source URL may not override the HTTPS port in v0");
    }
    if parsed.as_str() != source.url {
        bail!("fetch source URL must already be canonical");
    }
    validate_percent_encoding(&source.url)?;
    let encoded_path = parsed.path().to_ascii_lowercase();
    if encoded_path.contains("%2f") || encoded_path.contains("%5c") {
        bail!("fetch source URL path contains an encoded separator");
    }
    if super::contains_secret_query(&source.url) {
        bail!("fetch source URL query contains credential-like material");
    }
    let host = parsed
        .host_str()
        .context("fetch source URL has no host")?
        .to_string();
    if !allowed_domains.contains(&host) {
        bail!("fetch source URL host is outside allowed_domains: {host}");
    }
    crate::tools::path_guard::validate_workspace_relative(&source.snapshot_path)?;
    if !source.snapshot_path.starts_with("data/snapshots/") {
        bail!("fetch snapshot_path must be below data/snapshots/");
    }
    if source.snapshot_path.ends_with('/') || Path::new(&source.snapshot_path).file_name().is_none()
    {
        bail!("fetch snapshot_path must name a file");
    }
    let mut path_for_robots = parsed.path().to_string();
    if let Some(query) = parsed.query() {
        path_for_robots.push('?');
        path_for_robots.push_str(query);
    }
    Ok(ValidatedSource {
        source_id: source.source_id,
        url: source.url,
        host,
        port: 443,
        path_for_robots,
        snapshot_path: source.snapshot_path,
    })
}

fn validate_percent_encoding(url: &str) -> anyhow::Result<()> {
    let bytes = url.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len()
                || !bytes[index + 1].is_ascii_hexdigit()
                || !bytes[index + 2].is_ascii_hexdigit()
            {
                bail!("fetch source URL contains malformed percent encoding");
            }
            index += 3;
        } else {
            index += 1;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid(url: &str, domains: &str) -> String {
        format!(
            r#"[fetch]
schema_version = "commandagent.fetch/v0"
allowed_domains = ["{domains}"]
max_fetches = 1
max_http_requests = 2
timeout_seconds = 5
max_response_bytes = 4096
freshness_max_age_seconds = 86400
cache_policy = "canonical-url-utc-day"
robots_policy = "respect"
user_agent = "{USER_AGENT}"
min_origin_interval_ms = 1000
redirect_policy = "reject"

[[fetch.sources]]
source_id = "events"
url = "{url}"
snapshot_path = "data/snapshots/events.html"
authorization = "contract"
"#
        )
    }

    fn parse(text: &str) -> anyhow::Result<ValidatedContract> {
        let file = toml::from_str::<ContractFile>(text)?;
        validate("fetch.toml", text.as_bytes(), file)
    }

    #[test]
    fn closed_contract_rejects_unknown_http_domain_and_query_secret() {
        assert!(
            parse(
                &(valid("https://events.example.test/events", "events.example.test")
                    + "unknown = true\n")
            )
            .is_err()
        );
        assert!(
            parse(&valid(
                "http://events.example.test/events",
                "events.example.test"
            ))
            .is_err()
        );
        assert!(
            parse(&valid(
                "https://outside.example.test/events",
                "events.example.test"
            ))
            .is_err()
        );
        assert!(
            parse(&valid(
                "https://events.example.test/events?access_token=secret",
                "events.example.test"
            ))
            .is_err()
        );
        assert!(parse(&valid("https://8.8.8.8/events", "8.8.8.8")).is_err());
        assert!(
            parse(&valid(
                "https://events.example.test/%ZZ",
                "events.example.test"
            ))
            .is_err()
        );
        assert!(
            parse(&valid(
                "https://events.example.test/a%2Fb",
                "events.example.test"
            ))
            .is_err()
        );
        assert!(
            parse(
                &valid("https://events.example.test/events", "events.example.test")
                    .replace(USER_AGENT, "GenericBrowser/1.0")
            )
            .is_err()
        );
    }
}
