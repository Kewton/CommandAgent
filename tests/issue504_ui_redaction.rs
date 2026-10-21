#![cfg(unix)]

//! Issue #544 self-check (L-14): rows 18-30 of H-09 `checks-544.md`.
//!
//! The TUI display and history boundaries are exercised without a terminal,
//! model, or API: the markdown renderer, the direct streaming entry, the history
//! file, acceptance/directive sheets, and confirmation persistence. Every row is
//! a focused test named `r<row>_...` and registers only fake values.

use std::path::Path;

use clap::Parser;
use commandagent::cli::Cli;
use commandagent::config::Config;
use commandagent::planner::adjudication::contract::IntentId;
use commandagent::planner::profile::ProfileId;
use commandagent::sensitive_data::{self, RedactionContext, SecretCatalog, set_current};
use commandagent::tui::OutputRenderer;
use commandagent::tui::boundary_shell::band_catalog::value_for;
use commandagent::tui::boundary_shell::confirmation::{
    ConfirmationIdentity, ExecutionPins, PackSelection, persist_confirmation,
};
use commandagent::tui::boundary_shell::directive::DirectiveContinuation;
use commandagent::tui::boundary_shell::family_catalog::TaskFamilyId;
use commandagent::tui::boundary_shell::route::{RouteBasis, RouteCandidate};
use commandagent::tui::boundary_shell::sheet;
use commandagent::tui::editor::ReplEditor;
use commandagent::tui::markdown::{PlainRenderer, TerminalMarkdownRenderer, capture};
use serde_json::Value;

const C1: &str = "H01_CANARY_JwtStyle_NonPrefix_29486";
const C2: &str = "H09_日本語_Canary_秘密値_5521";
const C3: &str = "H09_Q\"uo\\te_Canary_7731";
const C4: &str = "H09_LINE_alpha\nH09_LINE_beta";
const C8_A: &str = "ABCDEFGHIJKL";
const C8_B: &str = "GHIJKLMNOPQRSTUV";

struct ScopeGuard;

impl Drop for ScopeGuard {
    fn drop(&mut self) {
        set_current(None);
    }
}

fn install(secrets: &[&str]) -> ScopeGuard {
    let mut catalog = SecretCatalog::new();
    for secret in secrets {
        let _ = catalog.register_plain(secret);
    }
    set_current(Some(RedactionContext::from_catalog(catalog)));
    ScopeGuard
}

fn config(root: &Path) -> Config {
    Config::from_cli(Cli::parse_from([
        "commandagent",
        "--cwd",
        &root.to_string_lossy(),
        "--model",
        "issue544-model",
    ]))
    .unwrap()
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap()
}

fn split_by_chars(value: &str, size: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    for character in value.chars() {
        current.push(character);
        if current.chars().count() == size {
            chunks.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

fn corpus() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/corpus/apps/issue504-ui-redaction/fixtures/public-projections.json");
    let value: Value = serde_json::from_str(&read(&path)).unwrap();
    value["clean_fixtures"].clone()
}

fn identity(root: &Path, request: String) -> ConfirmationIdentity {
    let route = RouteCandidate {
        profile: ProfileId::Ingest,
        intent: IntentId::Create,
        family: TaskFamilyId::List,
        bases: vec![RouteBasis {
            rule: "fixture",
            observation: "list".to_string(),
        }],
        contract_ref: "docs/ingest-profile-contract.md",
    };
    ConfirmationIdentity::new(
        request,
        root,
        &route,
        value_for("ingest", IntentId::Create, TaskFamilyId::List).unwrap(),
        ExecutionPins {
            planner_provider: "ollama".to_string(),
            planner_model: "planner".to_string(),
            executor_provider: "ollama".to_string(),
            executor_model: "executor".to_string(),
            preset: "profile".to_string(),
            think: None,
        },
        PackSelection::None,
    )
    .unwrap()
}

fn stop_events(root: &Path, stop_reason: &str) -> std::path::PathBuf {
    let events = root.join("events.jsonl");
    let reason = stop_reason.replace('"', "\\\"");
    std::fs::write(
        &events,
        format!(
            "{{\"event\":\"tui_command_stop\",\"effective_profile\":\"ingest\",\"status\":\"failed\",\"assurance_level\":\"failed\",\"stop_reason\":\"{reason}\"}}\n"
        ),
    )
    .unwrap();
    events
}

/// Run one `#[ignore]`d child test in its own process and return its output.
///
/// `Editor::save_history` reaches rustyline 14, which changes the process-wide
/// umask to 0o177 while saving (`history.rs:689`, `:818-825`). A concurrent
/// `tempfile::tempdir()` in the same test process can then be created 0o600 and
/// a later `mkdir` fails with EACCES (PR #551 CI). Running the history saves in
/// a child process keeps that window away from every other test in this binary.
fn run_isolated(child: &str) -> std::process::Output {
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "--ignored", "--nocapture", child])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "isolated child {child} failed: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

// Row 18: a new history entry is scrubbed on save while the execution input is
// kept unchanged. The body runs in an isolated child process (see
// `run_isolated`).
#[test]
fn r18_new_history_entry_is_scrubbed_and_execution_input_kept() {
    let output = run_isolated("r18_history_save_child");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("<redacted>"), "{stdout}");
    assert!(!stdout.contains(C1), "{stdout}");
}

#[test]
#[ignore]
fn r18_history_save_child() {
    let dir = tempfile::tempdir().unwrap();
    let config = config(dir.path());
    let _scope = install(&[C1]);
    let mut editor = ReplEditor::new(&config).unwrap();

    let line = format!("echo {C1}");
    editor.add_history_entry(&line).unwrap();
    assert!(line.contains(C1), "execution input must be kept");

    let path = dir.path().join("history.txt");
    editor.save_history(&path).unwrap();
    let saved = read(&path);
    assert!(!saved.contains(C1), "{saved}");
    assert!(saved.contains("<redacted>"), "{saved}");
    println!("r18 saved={saved:?}");
}

// Row 19: a legacy history entry read from disk is rewritten scrubbed at the
// next save. The user decision (2026-09-29) allows this; memory is not rewritten
// at add time, so the up-arrow recall stays intact until the save. The body runs
// in an isolated child process (see `run_isolated`).
#[test]
fn r19_legacy_history_entries_are_rewritten_at_save() {
    let output = run_isolated("r19_history_save_child");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("<redacted>"), "{stdout}");
    assert!(!stdout.contains(C1), "{stdout}");
}

#[test]
#[ignore]
fn r19_history_save_child() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("history.txt");
    // Produce a genuine rustyline history file with a raw secret entry.
    {
        let config = config(dir.path());
        let mut writer = ReplEditor::new(&config).unwrap();
        writer.add_history_entry(&format!("echo {C1}")).unwrap();
        writer.save_history(&path).unwrap();
    }
    assert!(read(&path).contains(C1), "fixture must hold the raw secret");

    let config = config(dir.path());
    let _scope = install(&[C1]);
    let mut editor = ReplEditor::new(&config).unwrap();
    editor.load_history(&path).unwrap();
    // `load_history` does not rewrite the file.
    assert!(read(&path).contains(C1));
    editor.save_history(&path).unwrap();
    let saved = read(&path);
    assert!(!saved.contains(C1), "{saved}");
    assert!(saved.contains("<redacted>"), "{saved}");
    println!("r19 saved={saved:?}");
}

// Row 20: a secret split across chunks never appears in the cumulative output,
// and finish completes the redaction.
#[test]
fn r20_stream_split_chunks_never_emit_fragments() {
    let _scope = install(&[C1]);
    let renderer = TerminalMarkdownRenderer::new(false, true);
    let capture = capture::start();
    let mut stream = renderer.begin_stream();
    let text = format!("before {C1} after");
    for chunk in split_by_chars(&text, 5) {
        stream.push_chunk(&chunk).unwrap();
        let so_far = capture.output();
        assert!(!so_far.contains(C1), "{so_far}");
        assert!(!so_far.contains(&C1[..12]), "{so_far}");
    }
    stream.finish().unwrap();
    let output = capture.output();
    assert!(!output.contains(C1), "{output}");
    assert!(output.contains("<redacted>"), "{output}");
    assert!(
        output.contains("before") && output.contains("after"),
        "{output}"
    );
}

// Row 21: two overlapping secrets are removed as one region; neither prefix
// leaks.
#[test]
fn r21_overlapping_secrets_do_not_leak_prefix() {
    let _scope = install(&[C8_A, C8_B]);
    let renderer = TerminalMarkdownRenderer::new(false, true);
    let capture = capture::start();
    let mut stream = renderer.begin_stream();

    // A..V holds both values with an overlapping region (G..L).
    stream.push_chunk("ABCDEFGHIJKLMNOPQRSTUV").unwrap();
    let partial = capture.output();
    assert!(!partial.contains("ABCDEF"), "{partial}");
    assert!(
        !partial.contains(C8_A) && !partial.contains(C8_B),
        "{partial}"
    );

    stream.finish().unwrap();
    let output = capture.output();
    assert!(
        !output.contains("ABCDEF") && !output.contains("GHIJKL"),
        "{output}"
    );
    assert_eq!(output.trim_end(), "<redacted>");
}

// Row 22: the scrubber runs before the capture/renderer/raw branch, so the
// capture and markdown exits are scrubbed.
#[test]
fn r22_capture_and_renderer_exits_are_scrubbed_before_the_branch() {
    let _scope = install(&[C1]);
    let renderer = TerminalMarkdownRenderer::new(false, true);

    {
        let capture = capture::start();
        let mut stream = renderer.begin_stream();
        stream.push_chunk(&format!("markdown {C1}\n")).unwrap();
        stream.finish().unwrap();
        let output = capture.output();
        assert!(!output.contains(C1), "{output}");
        assert!(output.contains("<redacted>"), "{output}");
    }

    let rendered = renderer.render_to_string(&format!("markdown {C1}\n"));
    assert!(!rendered.contains(C1), "{rendered}");
    assert!(rendered.contains("<redacted>"), "{rendered}");
}

// Row 23: a secret that contains markdown emphasis markers is scrubbed before
// the renderer sees it.
#[test]
fn r23_markdown_emphasis_does_not_split_secret() {
    let _scope = install(&[C1]);
    let renderer = TerminalMarkdownRenderer::new(false, true);
    let rendered = renderer.render_to_string(&format!("_{C1}_ and **{C1}** and `{C1}`\n"));
    assert!(!rendered.contains(C1), "{rendered}");
    assert!(rendered.contains("<redacted>"), "{rendered}");
    assert!(
        !rendered.contains('*') && !rendered.contains('`'),
        "{rendered}"
    );
}

// Row 24: `finish` (and the `Drop` that calls it) flushes the held-back tail
// scrubbed.
#[test]
fn r24_drop_flushes_the_scrubbed_tail() {
    let _scope = install(&[C1]);
    let renderer = TerminalMarkdownRenderer::new(false, true);
    let capture = capture::start();
    {
        let mut stream = renderer.begin_stream();
        stream.push_chunk(&format!("tail {C1}")).unwrap();
    }
    let output = capture.output();
    assert!(!output.contains(C1), "{output}");
    assert!(!output.contains("H01_CANARY"), "{output}");
    assert!(output.contains("<redacted>"), "{output}");
}

// Row 25: re-applying the scrub is idempotent and a non-secret display keeps the
// exact bytes declared in the corpus.
#[test]
fn r25_scrub_is_idempotent_and_clean_bytes_are_fixed() {
    let _scope = install(&[C1]);
    let once = sensitive_data::scrub_active(&format!("value {C1} end"));
    assert!(!once.contains(C1), "{once}");
    assert_eq!(sensitive_data::scrub_active(&once), once);

    let renderer = TerminalMarkdownRenderer::new(false, true);
    let clean = corpus();
    let input = clean["r25_markdown_input"].as_str().unwrap();
    let output = clean["r25_markdown_output"].as_str().unwrap();
    assert_eq!(renderer.render_to_string(input), output);
}

// Row 26: all three non-streaming exits scrub the secret and keep the markdown
// structure.
#[test]
fn r26_non_streaming_exits_are_scrubbed() {
    let _scope = install(&[C1, C4]);
    let renderer = TerminalMarkdownRenderer::new(false, true);

    {
        let capture = capture::start();
        renderer
            .render_assistant(&format!("# Title {C1}\n\nbody {C4}\n"))
            .unwrap();
        let output = capture.output();
        assert!(
            !output.contains(C1) && !output.contains("H09_LINE"),
            "{output}"
        );
        assert!(output.contains("Title"), "{output}");
    }
    {
        let capture = capture::start();
        PlainRenderer
            .render_assistant(&format!("plain {C1} and {C4}"))
            .unwrap();
        let output = capture.output();
        assert!(
            !output.contains(C1) && !output.contains("H09_LINE"),
            "{output}"
        );
    }

    let rendered = renderer.render_to_string(&format!("# Heading {C1}\n"));
    assert!(!rendered.contains(C1), "{rendered}");
    assert!(rendered.contains("Heading"), "{rendered}");
}

// Row 27: the persisted acceptance/directive sheet (after directive metadata is
// appended) carries no secret, and the file name stays the card hash.
#[test]
fn r27_sheet_persist_scrubs_after_directive_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let _scope = install(&[C1, C2]);
    let events = stop_events(dir.path(), &format!("leaked {C1}"));
    let identity = identity(dir.path(), format!("create ingest with {C1}"));

    let generated = sheet::generate(&identity, Some(&events), false).unwrap();
    assert!(!generated.markdown.contains(C1), "{}", generated.markdown);
    assert!(
        !generated
            .section5
            .as_deref()
            .unwrap_or_default()
            .contains(C1)
    );

    let described = sheet::with_directive_metadata(
        sheet::generate(&identity, Some(&events), false).unwrap(),
        &DirectiveContinuation {
            plan_path: dir.path().join("plan/plan.yaml"),
            plan_workspace_path: format!("plan-{C2}"),
            target_run_id: "run-fixture".to_string(),
            directive_round: 2,
            directive_hash: format!("sha256:{C2}"),
            regression_freeze: None,
        },
    );
    let path = sheet::persist_directive_round(&dir.path().join("state"), &identity, &described, 2)
        .unwrap();
    let saved = read(&path);
    assert!(!saved.contains(C1) && !saved.contains(C2), "{saved}");
    assert!(saved.contains("<redacted>"), "{saved}");
    assert!(
        path.file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with(identity.card_hash().unwrap().trim_start_matches("sha256:"))
    );
}

// Row 28: a confirmation identity with a secret in a nested field is refused
// before any record is written, and the refusal names no value.
#[test]
fn r28_confirmation_identity_with_secret_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let _scope = install(&[C1]);
    let mut identity = identity(dir.path(), "create ingest".to_string());
    identity.pins.planner_model = format!("model-{C1}");
    let hash = identity.card_hash().unwrap();
    let records = dir.path().join("records");

    let error = persist_confirmation(&records, &identity, &hash).unwrap_err();
    assert!(!error.to_string().contains(C1), "{error}");
    assert!(!format!("{error:?}").contains(C1), "{error:?}");
    assert!(!records.exists(), "a refused record must not be written");
    // The hashed source was not rewritten to fake an approval.
    assert!(identity.pins.planner_model.contains(C1));
}

// Row 29: the check runs on each field's raw value, so an escaped secret (`"`)
// that a serialized-JSON text check would miss is still refused.
#[test]
fn r29_identity_with_escaped_secret_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let _scope = install(&[C3]);
    let identity = identity(dir.path(), format!("create {C3}"));
    let hash = identity.card_hash().unwrap();
    let records = dir.path().join("records");

    let error = persist_confirmation(&records, &identity, &hash).unwrap_err();
    assert!(!error.to_string().contains(C3), "{error}");
    assert!(!records.exists());
}

// Row 30: a secret-free identity persists byte-stably, `validate()` holds, and a
// text-replaced identity does not collide with the original hash. The clean
// fixture bytes are fixed in the corpus.
#[test]
fn r30_confirmation_record_bytes_are_stable_without_secret() {
    let dir = tempfile::tempdir().unwrap();
    set_current(None);
    let request = corpus()["r30_request"].as_str().unwrap().to_string();
    let identity = identity(dir.path(), request);
    let hash = identity.card_hash().unwrap();
    let records = dir.path().join("records");

    let first = persist_confirmation(&records, &identity, &hash).unwrap();
    let bytes = std::fs::read(first.record_path()).unwrap();
    let again = persist_confirmation(&records, &identity, &hash).unwrap();
    assert_eq!(std::fs::read(again.record_path()).unwrap(), bytes);
    assert!(first.validate().is_ok());

    let mut mutated = identity.clone();
    mutated.request = format!("{}x", identity.request);
    assert_ne!(mutated.card_hash().unwrap(), hash);
}
