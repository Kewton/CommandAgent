//! Bounded dev-server launchability checks.
//!
//! This is verifier-owned runtime evidence. It does not repair files, install
//! dependencies, or keep a background server running.

use crate::agent::events::bounded_event_text;
use serde_json::Value;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const DEV_SERVER_TIMEOUT_SECS: u64 = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DevServerSmokeReport {
    pub(super) requested_port: u16,
    pub(super) terminal_state: String,
    pub(super) diagnostic_code: String,
    pub(super) diagnostic: String,
    pub(super) port_preflight: String,
    pub(super) endpoint_smoke: String,
    pub(super) cleanup_status: String,
}

impl DevServerSmokeReport {
    pub(super) fn ok(requested_port: u16) -> Self {
        Self {
            requested_port,
            terminal_state: "ok".to_string(),
            diagnostic_code: "ok".to_string(),
            diagnostic: "dev-server endpoint smoke passed".to_string(),
            port_preflight: "available".to_string(),
            endpoint_smoke: "passed".to_string(),
            cleanup_status: "cleaned_up".to_string(),
        }
    }

    fn failure(
        requested_port: u16,
        terminal_state: &str,
        diagnostic_code: &str,
        diagnostic: impl Into<String>,
        port_preflight: &str,
        endpoint_smoke: &str,
        cleanup_status: &str,
    ) -> Self {
        Self {
            requested_port,
            terminal_state: terminal_state.to_string(),
            diagnostic_code: diagnostic_code.to_string(),
            diagnostic: diagnostic.into(),
            port_preflight: port_preflight.to_string(),
            endpoint_smoke: endpoint_smoke.to_string(),
            cleanup_status: cleanup_status.to_string(),
        }
    }

    pub(super) fn is_ok(&self) -> bool {
        self.terminal_state == "ok"
    }

    pub(super) fn render_lines(&self) -> Vec<String> {
        vec![
            "active_job=dev_server_smoke".to_string(),
            "recovery_owner=dev_server".to_string(),
            "runtime_job_kind=dev_server_smoke".to_string(),
            format!(
                "runtime_job_outcome={}",
                if self.is_ok() { "passed" } else { "failed" }
            ),
            format!("terminal_state={}", self.terminal_state),
            format!("diagnostic_code={}", self.diagnostic_code),
            format!("requested_port={}", self.requested_port),
            format!("port={}", self.requested_port),
            format!("port_preflight={}", self.port_preflight),
            format!("dev_server_state={}", self.terminal_state),
            format!("endpoint_smoke={}", self.endpoint_smoke),
            format!("cleanup_status={}", self.cleanup_status),
            format!("diagnostic={}", bounded_event_text(&self.diagnostic)),
        ]
    }
}

pub(super) fn requested_dev_port(profile: &str, text: &str) -> Option<u16> {
    if !profile.eq_ignore_ascii_case("nextjs") {
        return None;
    }
    extract_requested_port(text)
}

pub(super) fn verify_nextjs_dev_server_smoke(
    cwd: &Path,
    requested_port: u16,
) -> DevServerSmokeReport {
    let Some(script) = package_script(cwd, "dev") else {
        return DevServerSmokeReport::failure(
            requested_port,
            "profile_contract_failed",
            "nextjs_dev_script_missing",
            "package.json is missing scripts.dev for the requested dev-server port",
            "not_checked",
            "not_started",
            "not_started",
        );
    };
    if !script.contains("next dev") || !script.contains(&requested_port.to_string()) {
        return DevServerSmokeReport::failure(
            requested_port,
            "profile_contract_failed",
            "nextjs_dev_script_drift",
            format!(
                "scripts.dev must contain `next dev` and requested port {requested_port}, got `{script}`"
            ),
            "not_checked",
            "not_started",
            "not_started",
        );
    }
    if !cwd.join("node_modules/.bin/next").exists() {
        return DevServerSmokeReport::failure(
            requested_port,
            "dependency_missing",
            "dependency_missing",
            "node_modules/.bin/next is missing before dev-server smoke; run bounded setup through verifier-owned setup recovery",
            "not_checked",
            "not_started",
            "not_started",
        );
    }
    if !port_available(requested_port) {
        return DevServerSmokeReport::failure(
            requested_port,
            "port_in_use",
            "nextjs_dev_server_port_in_use",
            format!("requested dev-server port {requested_port} is already in use"),
            "port_in_use",
            "not_started",
            "not_started",
        );
    }

    let mut child = match Command::new("npm")
        .arg("run")
        .arg("dev")
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(err) => {
            return DevServerSmokeReport::failure(
                requested_port,
                "setup_failed",
                "nextjs_dev_server_spawn_failed",
                format!("failed to start `npm run dev`: {err}"),
                "available",
                "not_started",
                "not_started",
            );
        }
    };

    let started = Instant::now();
    let timeout = Duration::from_secs(DEV_SERVER_TIMEOUT_SECS);
    let mut endpoint_error = String::new();
    while started.elapsed() < timeout {
        match child.try_wait() {
            Ok(Some(status)) => {
                let cleanup_status = cleanup_child(&mut child);
                return DevServerSmokeReport::failure(
                    requested_port,
                    "setup_failed",
                    "nextjs_dev_server_exited",
                    format!("dev server exited before endpoint smoke passed: {status}"),
                    "available",
                    "failed",
                    &cleanup_status,
                );
            }
            Ok(None) => {}
            Err(err) => {
                let cleanup_status = cleanup_child(&mut child);
                return DevServerSmokeReport::failure(
                    requested_port,
                    "setup_failed",
                    "nextjs_dev_server_poll_failed",
                    format!("failed to poll dev server process: {err}"),
                    "available",
                    "failed",
                    &cleanup_status,
                );
            }
        }
        match http_get_root(requested_port) {
            Ok(()) => {
                let cleanup_status = cleanup_child(&mut child);
                let mut report = DevServerSmokeReport::ok(requested_port);
                report.cleanup_status = cleanup_status;
                return report;
            }
            Err(err) => {
                endpoint_error = err;
                thread::sleep(Duration::from_millis(250));
            }
        }
    }

    let cleanup_status = cleanup_child(&mut child);
    DevServerSmokeReport::failure(
        requested_port,
        "setup_failed",
        "nextjs_dev_server_endpoint_timeout",
        format!(
            "dev server did not return a non-error non-empty response before timeout: {endpoint_error}"
        ),
        "available",
        "timeout",
        &cleanup_status,
    )
}

fn extract_requested_port(text: &str) -> Option<u16> {
    let lower = text.to_ascii_lowercase();
    if !(lower.contains("port") || text.contains("ポート")) {
        return None;
    }
    for token in numeric_tokens(text) {
        if (1024..=65535).contains(&token) {
            return Some(token);
        }
    }
    None
}

fn numeric_tokens(text: &str) -> Vec<u16> {
    let mut values = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_ascii_digit() {
            current.push(ch);
            continue;
        }
        if !current.is_empty() {
            if let Ok(value) = current.parse::<u16>() {
                values.push(value);
            }
            current.clear();
        }
    }
    if !current.is_empty()
        && let Ok(value) = current.parse::<u16>()
    {
        values.push(value);
    }
    values
}

fn package_script(cwd: &Path, name: &str) -> Option<String> {
    let text = fs::read_to_string(cwd.join("package.json")).ok()?;
    let json = serde_json::from_str::<Value>(&text).ok()?;
    json.get("scripts")?.get(name)?.as_str().map(str::to_string)
}

fn port_available(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

fn http_get_root(port: u16) -> Result<(), String> {
    let addr = format!("127.0.0.1:{port}");
    let mut stream = TcpStream::connect(addr).map_err(|err| err.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .map_err(|err| err.to_string())?;
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .map_err(|err| err.to_string())?;
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|err| err.to_string())?;
    let mut lines = response.lines();
    let status = lines.next().unwrap_or_default();
    if !(status.contains(" 200 ") || status.contains(" 30")) {
        return Err(format!("unexpected HTTP status `{status}`"));
    }
    let body = response.split("\r\n\r\n").nth(1).unwrap_or_default().trim();
    if body.is_empty() {
        return Err("empty endpoint body".to_string());
    }
    Ok(())
}

fn cleanup_child(child: &mut Child) -> String {
    match child.try_wait() {
        Ok(Some(_)) => "already_exited".to_string(),
        Ok(None) => {
            let kill_result = child.kill();
            let wait_result = child.wait();
            if kill_result.is_ok() && wait_result.is_ok() {
                "cleaned_up".to_string()
            } else {
                "cleanup_failed".to_string()
            }
        }
        Err(_) => "cleanup_failed".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn extracts_requested_port_from_english_and_japanese_goal() {
        assert_eq!(
            requested_dev_port("nextjs", "Create a Next.js app on port 3011"),
            Some(3011)
        );
        assert_eq!(
            requested_dev_port("nextjs", "3011ポートで起動可能なNext.jsアプリ"),
            Some(3011)
        );
        assert_eq!(requested_dev_port("python", "port 3011"), None);
    }

    #[test]
    fn occupied_port_is_reported_as_port_in_use() {
        let Ok(listener) = TcpListener::bind(("127.0.0.1", 0)) else {
            return;
        };
        let port = listener.local_addr().unwrap().port();
        let root = temp_workspace("port-in-use");
        fs::write(
            root.join("package.json"),
            format!(
                r#"{{"scripts":{{"dev":"next dev -p {port}"}},"dependencies":{{"next":"14.0.0","react":"18.2.0","react-dom":"18.2.0"}}}}"#
            ),
        )
        .unwrap();
        fs::create_dir_all(root.join("node_modules/.bin")).unwrap();
        fs::write(root.join("node_modules/.bin/next"), "").unwrap();

        let report = verify_nextjs_dev_server_smoke(&root, port);

        assert_eq!(report.terminal_state, "port_in_use");
        assert_eq!(report.diagnostic_code, "nextjs_dev_server_port_in_use");
        assert!(
            report
                .render_lines()
                .iter()
                .any(|line| line == "active_job=dev_server_smoke")
        );
    }

    #[test]
    fn missing_dev_script_is_profile_contract_failure() {
        let root = temp_workspace("missing-dev-script");
        fs::write(
            root.join("package.json"),
            r#"{"scripts":{"build":"next build"}}"#,
        )
        .unwrap();

        let report = verify_nextjs_dev_server_smoke(&root, 3011);

        assert_eq!(report.terminal_state, "profile_contract_failed");
        assert_eq!(report.diagnostic_code, "nextjs_dev_script_missing");
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("commandagent-dev-server-{name}-{nanos}"));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }
}
