use std::collections::VecDeque;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{Context, bail};
use serde::Deserialize;

use crate::bounded_process::{self, BoundedProcessOutcomeKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TransportRequest {
    pub url: String,
    pub host: String,
    pub port: u16,
    pub resolved_ip: String,
    pub user_agent: String,
    pub timeout_seconds: u16,
    pub max_response_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TransportResponse {
    pub http_status: u16,
    pub body: Vec<u8>,
    pub elapsed_ms: u64,
    pub remote_ip: Option<String>,
    pub redirect_location: Option<String>,
}

pub(crate) trait FetchTransport: Send + Sync {
    fn get(&self, root: &Path, request: &TransportRequest) -> anyhow::Result<TransportResponse>;
}

#[derive(Debug, Clone)]
pub(crate) struct BoundedCurlTransport {
    program: String,
}

impl Default for BoundedCurlTransport {
    fn default() -> Self {
        Self {
            program: "curl".to_string(),
        }
    }
}

impl FetchTransport for BoundedCurlTransport {
    fn get(&self, root: &Path, request: &TransportRequest) -> anyhow::Result<TransportResponse> {
        let scratch =
            crate::tools::path_guard::resolve_optional_existing(root, "evidence/.fetch-transport")?;
        fs::create_dir_all(&scratch)?;
        let nonce = format!("{}-{}", std::process::id(), super::time::unix_epoch_ms());
        let config = scratch.join(format!("{nonce}.curlrc"));
        let body = scratch.join(format!("{nonce}.body"));
        let headers = scratch.join(format!("{nonce}.headers"));
        write_curl_config(&config, &body, &headers, request)?;

        let mut command = Command::new(&self.program);
        command
            .arg("--config")
            .arg(&config)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let output = bounded_process::run_with_timeout(
            &mut command,
            Duration::from_secs(request.timeout_seconds.into()),
        );
        let _ = fs::remove_file(&config);
        let result = output
            .context("fetch child could not start")
            .and_then(|output| {
                let metadata = String::from_utf8_lossy(&output.stdout);
                match output.kind {
                    BoundedProcessOutcomeKind::TimedOut => bail!("fetch_timeout"),
                    BoundedProcessOutcomeKind::Cancelled
                    | BoundedProcessOutcomeKind::CommandAbortedByUser => bail!("fetch_cancelled"),
                    BoundedProcessOutcomeKind::Exited if !output.success() => {
                        let raw_stderr = String::from_utf8_lossy(&output.stderr);
                        let scrubbed =
                            raw_stderr.replace(&request.url, &super::scrub_url_query(&request.url));
                        let stderr = crate::eval_events::body_snippet(&scrubbed);
                        bail!("fetch_child_failed:{stderr}")
                    }
                    BoundedProcessOutcomeKind::Exited => {
                        parse_response(&metadata, &body, &headers, output.elapsed)
                    }
                }
            });
        let _ = fs::remove_file(&body);
        let _ = fs::remove_file(&headers);
        result
    }
}

fn write_curl_config(
    config: &Path,
    body: &Path,
    headers: &Path,
    request: &TransportRequest,
) -> anyhow::Result<()> {
    let resolved_ip = request
        .resolved_ip
        .parse::<std::net::IpAddr>()
        .context("fetch resolved IP is invalid")?;
    let resolve_value = match resolved_ip {
        std::net::IpAddr::V4(ip) => ip.to_string(),
        std::net::IpAddr::V6(ip) => format!("[{ip}]"),
    };
    for value in [
        request.url.as_str(),
        request.host.as_str(),
        resolve_value.as_str(),
        request.user_agent.as_str(),
        body.to_string_lossy().as_ref(),
        headers.to_string_lossy().as_ref(),
    ] {
        if value.contains(['\n', '\r', '"', '\\']) {
            bail!("fetch curl config value contains a forbidden character");
        }
    }
    let text = format!(
        "silent\nshow-error\nrequest = \"GET\"\nurl = \"{}\"\nresolve = \"{}:{}:{}\"\nuser-agent = \"{}\"\nheader = \"Accept-Encoding: identity\"\nproto = \"=https\"\nproto-redir = \"=https\"\nmax-redirs = 0\nconnect-timeout = {}\nmax-time = {}\nmax-filesize = {}\noutput = \"{}\"\ndump-header = \"{}\"\nwrite-out = \"%{{http_code}}\\n%{{remote_ip}}\\n%{{size_download}}\\n\"\n",
        request.url,
        request.host,
        request.port,
        resolve_value,
        request.user_agent,
        request.timeout_seconds,
        request.timeout_seconds,
        request.max_response_bytes,
        body.display(),
        headers.display(),
    );
    fs::write(config, text).context("write fetch child config")
}

fn parse_response(
    metadata: &str,
    body_path: &Path,
    headers_path: &Path,
    elapsed: Duration,
) -> anyhow::Result<TransportResponse> {
    let mut lines = metadata.lines();
    let http_status = lines
        .next()
        .context("fetch child omitted HTTP status")?
        .parse::<u16>()
        .context("fetch child returned invalid HTTP status")?;
    let remote_ip = lines
        .next()
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let declared_bytes = lines
        .next()
        .context("fetch child omitted byte count")?
        .parse::<u64>()
        .context("fetch child returned invalid byte count")?;
    if lines.next().is_some() {
        bail!("fetch child returned unexpected metadata");
    }
    let body = fs::read(body_path).context("fetch child body missing")?;
    if body.len() as u64 != declared_bytes {
        bail!("fetch child byte count mismatch");
    }
    let headers = fs::read_to_string(headers_path).unwrap_or_default();
    let redirect_location = headers.lines().rev().find_map(|line| {
        line.split_once(':').and_then(|(name, value)| {
            name.eq_ignore_ascii_case("location")
                .then(|| super::scrub_url_query(value.trim()))
        })
    });
    Ok(TransportResponse {
        http_status,
        body,
        elapsed_ms: u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
        remote_ip,
        redirect_location,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecordedExchange {
    pub expected_url: String,
    pub response: TransportResponse,
}

#[derive(Debug)]
pub(crate) struct RecordedTransport {
    exchanges: Mutex<VecDeque<RecordedExchange>>,
}

impl RecordedTransport {
    pub(crate) fn new(exchanges: impl IntoIterator<Item = RecordedExchange>) -> Self {
        Self {
            exchanges: Mutex::new(exchanges.into_iter().collect()),
        }
    }

    pub(crate) fn remaining(&self) -> usize {
        self.exchanges.lock().expect("recorded transport").len()
    }

    pub(crate) fn from_fixture(path: &Path) -> anyhow::Result<Self> {
        let bytes = fs::read(path).context("read fetch recording fixture")?;
        let fixture = serde_json::from_slice::<RecordingFile>(&bytes)
            .context("parse closed fetch recording fixture")?;
        if fixture.schema_version != "commandagent.fetch-recording/v0" {
            bail!("unsupported fetch recording schema_version");
        }
        Ok(Self::new(fixture.exchanges.into_iter().map(|exchange| {
            RecordedExchange {
                expected_url: exchange.url,
                response: TransportResponse {
                    http_status: exchange.http_status,
                    body: exchange.body.into_bytes(),
                    elapsed_ms: exchange.elapsed_ms,
                    remote_ip: Some(exchange.remote_ip),
                    redirect_location: exchange.redirect_location,
                },
            }
        })))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordingFile {
    schema_version: String,
    exchanges: Vec<RecordingExchange>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordingExchange {
    url: String,
    http_status: u16,
    body: String,
    elapsed_ms: u64,
    remote_ip: String,
    redirect_location: Option<String>,
}

impl FetchTransport for RecordedTransport {
    fn get(&self, _root: &Path, request: &TransportRequest) -> anyhow::Result<TransportResponse> {
        let exchange = self
            .exchanges
            .lock()
            .expect("recorded transport")
            .pop_front()
            .context("recorded transport exhausted")?;
        let scrubbed = super::scrub_url_query(&request.url);
        if exchange.expected_url != scrubbed {
            bail!(
                "recorded URL mismatch: expected={} observed={}",
                exchange.expected_url,
                scrubbed
            );
        }
        Ok(exchange.response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn bounded_child_transport_uses_a_config_path_not_a_url_argument() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().unwrap();
        let fake = root.path().join("fake-curl.sh");
        fs::write(
            &fake,
            r#"#!/bin/sh
set -eu
test "$1" = "--config"
test "$#" = 2
cfg="$2"
body=$(sed -n 's/^output = "\(.*\)"$/\1/p' "$cfg")
headers=$(sed -n 's/^dump-header = "\(.*\)"$/\1/p' "$cfg")
printf '<h1>fixture</h1>' > "$body"
printf 'HTTP/1.1 200 OK\r\n\r\n' > "$headers"
printf '200\n192.0.2.1\n16\n'
"#,
        )
        .unwrap();
        fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
        let transport = BoundedCurlTransport {
            program: fake.display().to_string(),
        };
        let response = transport
            .get(
                root.path(),
                &TransportRequest {
                    url: "https://data.example.test/events".to_string(),
                    host: "data.example.test".to_string(),
                    port: 443,
                    resolved_ip: "8.8.8.8".to_string(),
                    user_agent: super::super::USER_AGENT.to_string(),
                    timeout_seconds: 2,
                    max_response_bytes: 1024,
                },
            )
            .unwrap();
        assert_eq!(response.http_status, 200);
        assert_eq!(response.body, b"<h1>fixture</h1>");
        assert_eq!(response.remote_ip.as_deref(), Some("192.0.2.1"));
    }

    #[test]
    fn response_parser_preserves_body_and_records_redirect_without_following() {
        let dir = tempfile::tempdir().unwrap();
        let body = dir.path().join("body");
        let headers = dir.path().join("headers");
        fs::write(&body, b"redirect body").unwrap();
        fs::write(
            &headers,
            "HTTP/1.1 301 Moved Permanently\r\nLocation: https://other.test/a?token=secret\r\n\r\n",
        )
        .unwrap();
        let response = parse_response(
            "301\n192.0.2.1\n13\n",
            &body,
            &headers,
            Duration::from_millis(5),
        )
        .unwrap();
        assert_eq!(response.http_status, 301);
        assert_eq!(response.body, b"redirect body");
        assert_eq!(
            response.redirect_location.as_deref(),
            Some("https://other.test/a?token=%3CREDACTED%3E")
        );
    }
}
