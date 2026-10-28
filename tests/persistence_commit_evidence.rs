//! IC-44 downstream: the persistence commit witness changes only the probe's
//! persistence verdict. The real probe script runs through the public probe entry
//! against the fake Playwright module of the `editor-save-committed-readback`
//! corpus case, and the evidence it writes goes through the unchanged runtime
//! acceptance and behavior arbitration. For the L-137 content-q shape the inputs
//! of the three persistence release reasons are gone; for the L-135 content-g
//! shape they stay exactly as before. Every file lives in a tempdir and no real
//! browser is launched: the test stops unless `playwright` resolves to the fake.

#![cfg(unix)]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use commandagent::minimal_loop::behavior_evidence::{
    EvidenceArbitrationReport, arbitrate_final_acceptance,
};
use commandagent::minimal_loop::evidence::{
    BrowserInteractionEvidence, RuntimeAcceptanceReport, browser_interaction_evidence_for_dirs,
    verify_runtime_acceptance_with_browser_dirs_and_hints,
};
use commandagent::minimal_loop::interaction_probe::{
    BrowserInteractionObservation, BrowserInteractionProbeOptions,
    probe_browser_interaction_against_running_server_with_options,
};
use tempfile::TempDir;

const FAKE_COMMIT_PLAYWRIGHT: &str =
    include_str!("corpus/apps/editor-save-committed-readback/fixtures/fake-playwright.cjs");

const NOTES_PAGE: &str = r#""use client";
import { useEffect, useState } from "react";
export default function Page() {
  const [notes, setNotes] = useState<{ id: string; content: string }[]>([]);
  const [content, setContent] = useState("");
  useEffect(() => {
    fetch("/api/notes").then((res) => res.json()).then((data) => setNotes(data.notes));
  }, []);
  useEffect(() => {
    localStorage.setItem("draft", content);
  }, [content]);
  const save = () => fetch("/api/notes/1", { method: "PUT", body: JSON.stringify({ content }) });
  return <main data-anvil-state={JSON.stringify({ content, noteCount: notes.length })}>
    <textarea data-anvil-action="input" value={content} onChange={(event) => setContent(event.target.value)} />
    <button data-anvil-action="primary" onClick={save}>Save</button>
    <ul>{notes.map((note) => <li key={note.id}>{note.content}</li>)}</ul>
  </main>;
}
"#;

const PERSISTENCE_UNVERIFIED: &str =
    "persistence_evidence:unverified:not_evaluated:no_mutation_observed";

struct Downstream {
    observation: BrowserInteractionObservation,
    report: RuntimeAcceptanceReport,
    arbitration: EvidenceArbitrationReport,
    browser: BrowserInteractionEvidence,
    _root: TempDir,
}

fn node_available() -> bool {
    matches!(
        Command::new("node")
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status(),
        Ok(status) if status.success()
    )
}

fn spawn_http_server() -> Option<(u16, thread::JoinHandle<()>)> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).ok()?;
    let port = listener.local_addr().ok()?.port();
    listener.set_nonblocking(true).ok()?;
    let handle = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buffer = [0_u8; 1024];
                    let _ = stream.read(&mut buffer);
                    let body = "<!doctype html><title>fake probe</title>";
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = stream.write_all(response.as_bytes());
                    return;
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => return,
            }
        }
    });
    Some((port, handle))
}

fn install_fake_playwright(root: &Path, scenario: &str) {
    let playwright_dir = root.join("node_modules/playwright");
    std::fs::create_dir_all(&playwright_dir).unwrap();
    std::fs::write(
        playwright_dir.join("package.json"),
        "{\"name\":\"playwright\",\"version\":\"0.0.0-fake\",\"main\":\"index.js\"}\n",
    )
    .unwrap();
    std::fs::write(
        playwright_dir.join("index.js"),
        FAKE_COMMIT_PLAYWRIGHT.replace("__SCENARIO__", scenario),
    )
    .unwrap();
}

/// Panics unless `playwright` resolves to the fake inside `root`, so the probe can
/// never reach an installed browser.
fn assert_playwright_resolves_to_fake(root: &Path) {
    let output = Command::new("node")
        .arg("-e")
        .arg("process.stdout.write(require.resolve('playwright'))")
        .current_dir(root)
        .env_remove("NODE_PATH")
        .stdin(Stdio::null())
        .output()
        .expect("node runs");
    let resolved = PathBuf::from(String::from_utf8_lossy(&output.stdout).to_string());
    let fake = root
        .join("node_modules/playwright/index.js")
        .canonicalize()
        .unwrap();
    assert_eq!(
        resolved.canonicalize().ok(),
        Some(fake),
        "playwright must resolve to the fake module"
    );
}

fn run_downstream(scenario: &str) -> Option<Downstream> {
    if !node_available() {
        eprintln!("skipping persistence commit downstream test because node is unavailable");
        return None;
    }
    let root = tempfile::tempdir().unwrap();
    let page = root.path().join("src/app/page.tsx");
    std::fs::create_dir_all(page.parent().unwrap()).unwrap();
    std::fs::write(&page, NOTES_PAGE).unwrap();
    install_fake_playwright(root.path(), scenario);
    assert_playwright_resolves_to_fake(root.path());
    let Some((port, server)) = spawn_http_server() else {
        eprintln!("skipping persistence commit downstream test because loopback bind failed");
        return None;
    };
    let run_dir = root.path().join(".anvil/runs/test");
    let evidence_path = run_dir.join("browser-interaction.json");
    let outcome = probe_browser_interaction_against_running_server_with_options(
        root.path(),
        port,
        &run_dir,
        &evidence_path,
        Duration::from_secs(60),
        BrowserInteractionProbeOptions {
            persistence_required: true,
            text_entry_required: true,
            token_echo_required: true,
        },
    );
    let _ = server.join();
    let observation = outcome
        .observation()
        .cloned()
        .unwrap_or_else(|| panic!("{scenario}: no observation: {outcome:?}"));
    assert_eq!(
        observation
            .playwright_resolution
            .as_ref()
            .map(|resolution| resolution.version.as_str()),
        Some("0.0.0-fake"),
        "{scenario}: {observation:?}"
    );
    let required = vec!["persistence_evidence".to_string()];
    let dirs = vec![run_dir];
    let mut report = verify_runtime_acceptance_with_browser_dirs_and_hints(
        root.path(),
        &["src/app/page.tsx".to_string()],
        &[],
        &[],
        &required,
        &[],
        &[],
        &dirs,
        &[],
    );
    let arbitration =
        arbitrate_final_acceptance(&mut report, root.path(), &dirs, &[], &required, &[]);
    let browser = browser_interaction_evidence_for_dirs(root.path(), &dirs);
    Some(Downstream {
        observation,
        report,
        arbitration,
        browser,
        _root: root,
    })
}

fn persistence_observation(run: &Downstream) -> &str {
    run.arbitration
        .records
        .get("persistence_evidence")
        .map(|record| record.behavioral_observation.as_str())
        .unwrap_or("absent")
}

#[test]
fn content_q_commit_witness_clears_only_the_persistence_release_inputs() {
    let Some(run) = run_downstream("recorded_content_q") else {
        return;
    };
    let report = &run.report;
    assert_eq!(
        run.observation.persistence_after_reload, "preserved",
        "{:?}",
        run.observation
    );
    assert!(run.observation.ok, "{:?}", run.observation);
    assert!(
        run.observation.persistence_changed_dimensions.is_empty(),
        "{:?}",
        run.observation
    );
    assert_eq!(persistence_observation(&run), "preserved_after_reload");
    assert!(
        !report
            .unverified_evidence
            .iter()
            .any(|evidence| evidence.starts_with("persistence_evidence:")),
        "{report:?}"
    );
    assert_eq!(
        run.browser.interaction_evidence_status, "passed",
        "{:?}",
        run.browser
    );
}

#[test]
fn content_g_editor_shape_keeps_every_persistence_release_input() {
    let Some(run) = run_downstream("l135_editor_after_reload") else {
        return;
    };
    let report = &run.report;
    assert_eq!(
        run.observation.persistence_after_reload, "not_evaluated",
        "{:?}",
        run.observation
    );
    assert_eq!(
        persistence_observation(&run),
        "not_evaluated:no_mutation_observed"
    );
    assert!(
        report
            .unverified_evidence
            .contains(&PERSISTENCE_UNVERIFIED.to_string()),
        "{report:?}"
    );
    assert_eq!(
        run.browser.interaction_evidence_status,
        "failed:persistence_not_evaluated:no_mutation_observed",
        "{:?}",
        run.browser
    );
}
