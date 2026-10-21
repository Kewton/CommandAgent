// Test-only mock browser transport. The mock child binds an OS-assigned port
// (requested port `0`) and keeps the listener open, then publishes the real
// port through a ready file. The parent reads that file instead of racing for a
// fixed port, so concurrent test processes no longer collide.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::time::Duration;

use super::{
    BrowserReadinessObservation, ProbeCommand, ProbeOptions, probe_browser_readiness_with_options,
};
use crate::minimal_loop::interaction_probe;

/// Ready-file convention shared with the issue448 test transport wrapper. Keep
/// the literal in sync with `install_issue448_mock_npm_transport`.
const MOCK_PORT_FILE_RELATIVE: &str = ".anvil/evidence/browser-probe-mock-port.txt";

const MOCK_CHILD_TEST_NAME: &str =
    "minimal_loop::browser_probe::tests::browser_probe_mock_server_child";

pub(super) fn read_mock_port(root: &Path) -> Option<u16> {
    let text = std::fs::read_to_string(root.join(MOCK_PORT_FILE_RELATIVE)).ok()?;
    text.trim().parse::<u16>().ok().filter(|port| *port != 0)
}

/// Drop a previous run's ready file so the parent never reads a stale port.
pub(super) fn clear_mock_port(root: &Path) {
    let _ = std::fs::remove_file(root.join(MOCK_PORT_FILE_RELATIVE));
}

fn publish_mock_port(path: &Path, port: u16) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, port.to_string());
}

/// Body of the ignored `browser_probe_mock_server_child` test. It only runs
/// when the parent marks the process with the mock-child environment.
pub(super) fn run_mock_server_child() {
    if crate::env_compat::var("COMMANDAGENT_BROWSER_PROBE_MOCK_CHILD")
        .ok()
        .as_deref()
        != Some("1")
    {
        return;
    }
    let requested_port = crate::env_compat::var("COMMANDAGENT_BROWSER_PROBE_MOCK_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(0);
    let status = crate::env_compat::var("COMMANDAGENT_BROWSER_PROBE_MOCK_STATUS").unwrap();
    let startup_delay = crate::env_compat::var("COMMANDAGENT_BROWSER_PROBE_MOCK_DELAY_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    if startup_delay > 0 {
        std::thread::sleep(Duration::from_millis(startup_delay));
    }
    let listener = TcpListener::bind(("127.0.0.1", requested_port)).unwrap();
    let actual_port = listener.local_addr().unwrap().port();
    publish_mock_port(Path::new(MOCK_PORT_FILE_RELATIVE), actual_port);
    if status == "hang" {
        std::thread::sleep(Duration::from_secs(30));
        return;
    }
    let (mut stream, _) = listener.accept().unwrap();
    let mut request = [0u8; 512];
    let _ = stream.read(&mut request);
    let env_sensitive_failure = status == "env-sensitive"
        && (std::env::var_os("NODE_ENV").is_some() || std::env::var_os("NODE_OPTIONS").is_some());
    let code = if status == "500" || status == "node-env-marker" || env_sensitive_failure {
        500
    } else {
        200
    };
    if status == "node-env-marker" {
        eprintln!("Next.js detected a non-standard \"NODE_ENV\" value.");
    } else if code == 500 {
        eprintln!("Module parse failed: Unexpected character '@' (1:0)");
    }
    let body = if status == "html-canvas" {
        "<html><head><title>Space Test</title></head><body><canvas></canvas><button>Start</button></body></html>"
    } else if status == "node-env-marker" {
        "Next.js detected a non-standard \"NODE_ENV\" value."
    } else if code == 500 {
        "Module parse failed: Unexpected character '@'"
    } else {
        "ok"
    };
    let response = format!(
        "HTTP/1.1 {code} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).unwrap();
}

fn mock_child_command(
    status: &str,
    startup_delay_ms: u64,
    mock_port: u16,
    extra_env: Vec<(String, String)>,
) -> ProbeCommand {
    let exe = std::env::current_exe().unwrap();
    let mut env = vec![
        (
            "COMMANDAGENT_BROWSER_PROBE_MOCK_CHILD".to_string(),
            "1".to_string(),
        ),
        (
            "COMMANDAGENT_BROWSER_PROBE_MOCK_PORT".to_string(),
            mock_port.to_string(),
        ),
        (
            "COMMANDAGENT_BROWSER_PROBE_MOCK_STATUS".to_string(),
            status.to_string(),
        ),
        (
            "COMMANDAGENT_BROWSER_PROBE_MOCK_DELAY_MS".to_string(),
            startup_delay_ms.to_string(),
        ),
    ];
    env.extend(extra_env);
    ProbeCommand {
        program: exe.display().to_string(),
        args: vec![
            "--ignored".to_string(),
            "--exact".to_string(),
            MOCK_CHILD_TEST_NAME.to_string(),
            "--nocapture".to_string(),
        ],
        env,
        display: "mock browser probe child".to_string(),
    }
}

fn probe_with_options(
    root: &Path,
    command: ProbeCommand,
    port: Option<u16>,
    dynamic_port: bool,
    timeout: Duration,
) -> BrowserReadinessObservation {
    probe_browser_readiness_with_options(
        root,
        crate::planner::profiles::nextjs::PROFILE_ID,
        ProbeOptions {
            port,
            timeout,
            offline: false,
            require_build: false,
            command_override: Some(command),
            interaction_options: interaction_probe::BrowserInteractionProbeOptions::default(),
            dynamic_port,
        },
    )
}

/// Success/timeout mock child that takes an OS-assigned port and publishes it.
pub(super) fn probe_with_mock_child(
    root: &Path,
    status: &str,
    startup_delay_ms: u64,
    timeout: Duration,
) -> BrowserReadinessObservation {
    probe_with_mock_child_and_env(root, status, startup_delay_ms, timeout, Vec::new())
}

pub(super) fn probe_with_mock_child_and_env(
    root: &Path,
    status: &str,
    startup_delay_ms: u64,
    timeout: Duration,
    extra_env: Vec<(String, String)>,
) -> BrowserReadinessObservation {
    let command = mock_child_command(status, startup_delay_ms, 0, extra_env);
    probe_with_options(root, command, None, true, timeout)
}

/// Negative path: the child is pinned to `port` (already held by the caller) so
/// the probe must report `port_in_use` without spawning.
pub(super) fn probe_with_mock_child_on_port(
    root: &Path,
    port: u16,
    status: &str,
    startup_delay_ms: u64,
    timeout: Duration,
) -> BrowserReadinessObservation {
    let command = mock_child_command(status, startup_delay_ms, port, Vec::new());
    probe_with_options(root, command, Some(port), false, timeout)
}
