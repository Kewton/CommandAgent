#![cfg(all(feature = "gui", unix))]

//! Issue #544 focused GUI test: the public projection, run/acceptance/evidence
//! documents and lists, and the session events tail/artifacts hide a known
//! canary and the execution root without starting a real model or API.
//!
//! Two workspaces are served by one process: the repository catalog and the
//! execution catalog never merge. Fixed schema identifiers are preserved, a
//! response whose dynamic key would expose a secret is refused honestly, and
//! the legacy record bytes on disk stay unchanged.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

const REPOSITORY_CANARY: &str = "H01_CANARY_JwtStyle_NonPrefix_29486";
const EXECUTION_CANARY: &str = "H01_CANARY_Only_In_Execution_Workspace_544";
const REPOSITORY_RUN: &str = "run-544";
const SESSION_ID: &str = "0198b9c8-fab8-7000-8000-000000000544";
const REFUSED_SESSION_ID: &str = "0198b9c8-fab8-7000-8000-000000000545";

struct Server {
    child: Child,
    port: u16,
    _stdout: BufReader<ChildStdout>,
}

impl Server {
    fn start(repository: &Path, execution: &Path, static_root: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_gui_server"))
            .args(["--port", "0", "--base-path", "/"])
            .arg("--repository-root")
            .arg(repository)
            .arg("--execution-root")
            .arg(execution)
            .arg("--static-dir")
            .arg(static_root)
            .env_remove("GUI_TRIAL_TOKEN")
            .env_remove("GUI_TRIAL_ALLOWED_ORIGINS")
            .env_remove("OPENAI_API_KEY")
            .env_remove("GEMINI_API_KEY")
            .env_remove("LM_STUDIO_API_TOKEN")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let mut stderr: Option<ChildStderr> = child.stderr.take();
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut port = None;
        while Instant::now() < deadline {
            let mut line = String::new();
            if stdout.read_line(&mut line).unwrap() == 0 {
                break;
            }
            if let Some(rest) = line.split("listening on http://127.0.0.1:").nth(1)
                && let Some(value) = rest.split('/').next()
                && let Ok(value) = value.parse::<u16>()
            {
                port = Some(value);
                break;
            }
        }
        let port = port.unwrap_or_else(|| {
            let mut error = String::new();
            if let Some(stderr) = stderr.as_mut() {
                let _ = stderr.read_to_string(&mut error);
            }
            let _ = child.kill();
            panic!("gui_server did not report a listening port: {error}");
        });
        Self {
            child,
            port,
            _stdout: stdout,
        }
    }

    fn get(&self, path: &str) -> (u16, String) {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        write!(
            stream,
            "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
            self.port
        )
        .unwrap();
        stream.flush().unwrap();
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).unwrap();
        parse_response(&String::from_utf8_lossy(&raw))
    }

    fn get_json(&self, path: &str) -> (u16, serde_json::Value) {
        let (status, body) = self.get(path);
        let value = serde_json::from_str(&body).unwrap_or(serde_json::Value::String(body));
        (status, value)
    }

    fn stop(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn parse_response(raw: &str) -> (u16, String) {
    let (head, body) = raw.split_once("\r\n\r\n").unwrap_or((raw, ""));
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let chunked = head
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked");
    let body = if chunked {
        decode_chunked(body)
    } else {
        body.to_string()
    };
    (status, body)
}

fn decode_chunked(body: &str) -> String {
    let mut out = String::new();
    let mut rest = body;
    loop {
        let Some((size, tail)) = rest.split_once("\r\n") else {
            break;
        };
        let Ok(size) = usize::from_str_radix(size.trim(), 16) else {
            break;
        };
        if size == 0 {
            break;
        }
        out.push_str(&tail[..size.min(tail.len())]);
        rest = tail.get(size + 2..).unwrap_or("");
    }
    out
}

struct Fixture {
    _root: tempfile::TempDir,
    repository: PathBuf,
    execution: PathBuf,
    static_root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let repository = root.path().join("repository");
        let execution = root.path().join("execution");
        let static_root = root.path().join("static");
        std::fs::create_dir_all(&repository).unwrap();
        std::fs::create_dir_all(&execution).unwrap();
        std::fs::create_dir_all(&static_root).unwrap();

        // The repository catalog protects its own dotenv value and an ordinary
        // word that collides with a fixed `status` identifier.
        std::fs::write(
            repository.join(".env"),
            format!("FAKE_API_KEY={REPOSITORY_CANARY}\nPLAIN_WORD=completed\n"),
        )
        .unwrap();

        let run_root = repository
            .join("workspace/management/runs")
            .join(REPOSITORY_RUN);
        std::fs::create_dir_all(&run_root).unwrap();
        std::fs::write(
            run_root.join("acceptance-sheet.md"),
            "# Acceptance\n\nStatus: completed\n",
        )
        .unwrap();
        std::fs::write(
            run_root.join("evidence.json"),
            format!(
                "{{\"event\":\"run_stop\",\"schema_version\":\"1\",\"status\":\"completed\",\"verdict\":\"pass\",\"message\":\"{REPOSITORY_CANARY}\",\"note\":\"{EXECUTION_CANARY}\"}}"
            ),
        )
        .unwrap();
        std::fs::write(
            run_root.join("refused.json"),
            format!("{{\"event\":\"x\",\"{REPOSITORY_CANARY}\":1}}"),
        )
        .unwrap();

        // The execution workspace owns its own catalog and the raw private path.
        std::fs::write(
            execution.join(".env"),
            format!("FAKE_SESSION_KEY={EXECUTION_CANARY}\n"),
        )
        .unwrap();
        let canonical_execution = execution.canonicalize().unwrap();
        let run = execution.join(".commandagent/runs").join(SESSION_ID);
        std::fs::create_dir_all(&run).unwrap();
        std::fs::write(
            run.join("events.jsonl"),
            format!(
                "{{\"event\":\"run_start\",\"note\":\"{REPOSITORY_CANARY} {EXECUTION_CANARY} {}\"}}\n{{\"event\":\"run_stop\",\"status\":\"completed\"}}\n",
                canonical_execution.display()
            ),
        )
        .unwrap();
        let refused = execution
            .join(".commandagent/runs")
            .join(REFUSED_SESSION_ID);
        std::fs::create_dir_all(&refused).unwrap();
        std::fs::write(
            refused.join("events.jsonl"),
            format!("{{\"event\":\"x\",\"{EXECUTION_CANARY}\":1}}\n"),
        )
        .unwrap();

        Self {
            _root: root,
            repository,
            execution,
            static_root,
        }
    }
}

#[test]
fn repository_records_are_scrubbed_preserve_fixed_schema_and_keep_legacy_bytes() {
    let fixture = Fixture::new();
    let server = Server::start(
        &fixture.repository,
        &fixture.execution,
        &fixture.static_root,
    );

    let (status, index) = server.get_json("/api/runs");
    assert_eq!(status, 200);
    let raw = index.to_string();
    assert!(!raw.contains(REPOSITORY_CANARY), "{raw}");
    let summary = index["runs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|run| run["id"] == REPOSITORY_RUN)
        .unwrap_or_else(|| panic!("missing {REPOSITORY_RUN}: {raw}"));
    // The fixed status enum is preserved while the free status text is scrubbed.
    assert_eq!(summary["state"], "pass");
    assert_eq!(summary["status_text"], "<redacted>");

    let (status, detail) = server.get_json(&format!("/api/runs/{REPOSITORY_RUN}"));
    assert_eq!(status, 200);
    assert!(!detail.to_string().contains(REPOSITORY_CANARY), "{detail}");

    let (status, evidence) = server.get_json(&format!(
        "/api/runs/{REPOSITORY_RUN}/evidence?path=evidence.json"
    ));
    assert_eq!(status, 200);
    let content = evidence["content"].as_str().unwrap();
    assert!(!content.contains(REPOSITORY_CANARY), "{content}");
    let value: serde_json::Value = serde_json::from_str(content).unwrap();
    assert_eq!(value["event"], "run_stop");
    assert_eq!(value["status"], "completed");
    assert_eq!(value["verdict"], "pass");
    assert_eq!(value["message"], "<redacted>");
    // The execution workspace's catalog never merges into the repository's.
    assert!(content.contains(EXECUTION_CANARY), "{content}");

    server.stop();

    // The on-disk legacy records are byte-identical.
    let run_root = fixture
        .repository
        .join("workspace/management/runs")
        .join(REPOSITORY_RUN);
    let legacy = std::fs::read_to_string(run_root.join("evidence.json")).unwrap();
    assert!(legacy.contains(REPOSITORY_CANARY), "{legacy}");
    assert!(legacy.contains(EXECUTION_CANARY), "{legacy}");
}

#[test]
fn session_events_and_artifacts_are_scrubbed_and_keep_legacy_bytes() {
    let fixture = Fixture::new();
    let server = Server::start(
        &fixture.repository,
        &fixture.execution,
        &fixture.static_root,
    );

    let (status, events) = server.get_json(&format!("/api/sessions/{SESSION_ID}/events?tail=100"));
    assert_eq!(status, 200);
    let content = events["content"].as_str().unwrap();
    assert!(!content.contains(EXECUTION_CANARY), "{content}");
    assert!(content.contains(REPOSITORY_CANARY), "{content}");
    assert!(content.contains("<execution-root>"), "{content}");
    let canonical_execution = fixture.execution.canonicalize().unwrap();
    assert!(
        !content.contains(canonical_execution.to_string_lossy().as_ref()),
        "{content}"
    );
    assert!(content.contains("\"status\":\"completed\""), "{content}");

    let (status, artifacts) = server.get_json(&format!("/api/sessions/{SESSION_ID}/artifacts"));
    assert_eq!(status, 200);
    assert!(
        !artifacts.to_string().contains(EXECUTION_CANARY),
        "{artifacts}"
    );
    assert!(
        artifacts
            .as_array()
            .unwrap()
            .iter()
            .any(|artifact| artifact["id"] == "events.jsonl"),
        "{artifacts}"
    );

    server.stop();

    let legacy = std::fs::read_to_string(
        fixture
            .execution
            .join(".commandagent/runs")
            .join(SESSION_ID)
            .join("events.jsonl"),
    )
    .unwrap();
    assert!(legacy.contains(EXECUTION_CANARY), "{legacy}");
    assert!(legacy.contains(REPOSITORY_CANARY), "{legacy}");
}

#[test]
fn a_dynamic_key_that_would_expose_a_secret_is_refused_honestly() {
    let fixture = Fixture::new();
    let server = Server::start(
        &fixture.repository,
        &fixture.execution,
        &fixture.static_root,
    );

    let (status, body) = server.get(&format!(
        "/api/runs/{REPOSITORY_RUN}/evidence?path=refused.json"
    ));
    assert_eq!(status, 422, "{body}");
    assert!(body.contains("secret_projection_refused"), "{body}");
    assert!(!body.contains(REPOSITORY_CANARY), "{body}");

    let (status, body) = server.get(&format!(
        "/api/sessions/{REFUSED_SESSION_ID}/events?tail=100"
    ));
    assert_eq!(status, 422, "{body}");
    assert!(body.contains("trial_secret_projection_refused"), "{body}");
    assert!(!body.contains(EXECUTION_CANARY), "{body}");

    server.stop();
}
