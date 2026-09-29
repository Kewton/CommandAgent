#![cfg(unix)]

//! Issue #544 focused test: the TUI hides registered secrets at every display
//! and history boundary without a terminal, model, or API.
//!
//! Covered: the non-streaming markdown renderer, the direct streaming UI entry
//! (including UTF-8 / chunk splits and `finish`), new history entries, the
//! acceptance/directive sheet copies, and confirmation persistence. Re-applying
//! the scrub to an already-protected input is idempotent and keeps non-secret
//! display content; the execution input is never changed.

use std::path::Path;

use clap::Parser;
use commandagent::cli::Cli;
use commandagent::config::Config;
use commandagent::planner::adjudication::contract::IntentId;
use commandagent::planner::profile::ProfileId;
use commandagent::sensitive_data::{REDACTED, RedactionContext, SecretCatalog, set_current};
use commandagent::tui::boundary_shell::band_catalog::value_for;
use commandagent::tui::boundary_shell::confirmation::{
    ConfirmationIdentity, ExecutionPins, PackSelection, persist_confirmation,
};
use commandagent::tui::boundary_shell::family_catalog::TaskFamilyId;
use commandagent::tui::boundary_shell::route::{RouteBasis, RouteCandidate};
use commandagent::tui::boundary_shell::sheet;
use commandagent::tui::editor::ReplEditor;
use commandagent::tui::markdown::{TerminalMarkdownRenderer, capture};

const CANARY: &str = "H01_CANARY_JwtStyle_NonPrefix_29486";
const MULTIBYTE_SECRET: &str = "秘密トークン値-544-abcdef";

/// Install an accessor-scoped catalog on this thread and clear it on drop.
struct ScopeGuard;

impl Drop for ScopeGuard {
    fn drop(&mut self) {
        set_current(None);
    }
}

fn install_scope(secret: &str) -> ScopeGuard {
    let mut catalog = SecretCatalog::new();
    catalog.register(secret);
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

#[test]
fn non_streaming_markdown_hides_the_secret_and_keeps_other_content() {
    let _scope = install_scope(CANARY);
    let renderer = TerminalMarkdownRenderer::new(false, true);

    let rendered = renderer.render_to_string(&format!("answer: {CANARY}\n"));
    assert!(!rendered.contains(CANARY), "{rendered}");
    assert!(rendered.contains(REDACTED), "{rendered}");
    // Idempotent on an already-protected input and non-secret content kept.
    assert_eq!(renderer.render_to_string(&rendered), rendered);
    assert!(
        renderer
            .render_to_string("plain answer")
            .contains("plain answer")
    );
}

#[test]
fn non_streaming_markdown_chunks_hide_a_secret_split_across_chunks() {
    let _scope = install_scope(MULTIBYTE_SECRET);
    let renderer = TerminalMarkdownRenderer::new(false, true);
    let text = format!("prefix {MULTIBYTE_SECRET} suffix");
    let chunks = split_by_chars(&text, 3);

    let rendered = renderer.render_chunks_to_string(chunks.iter().map(String::as_str));

    assert!(!rendered.contains(MULTIBYTE_SECRET), "{rendered}");
    assert!(
        rendered.contains("prefix") && rendered.contains("suffix"),
        "{rendered}"
    );
    assert!(rendered.contains(REDACTED), "{rendered}");
}

#[test]
fn direct_streaming_entry_hides_a_secret_split_across_utf8_chunks_and_finish() {
    let _scope = install_scope(MULTIBYTE_SECRET);
    let renderer = TerminalMarkdownRenderer::new(false, true);
    let capture = capture::start();
    let mut stream = renderer.begin_stream();
    let text = format!("prefix {MULTIBYTE_SECRET} suffix");
    for chunk in split_by_chars(&text, 3) {
        stream.push_chunk(&chunk).unwrap();
    }
    stream.finish().unwrap();

    let output = capture.output();
    assert!(!output.contains(MULTIBYTE_SECRET), "{output}");
    assert!(
        output.contains("prefix") && output.contains("suffix"),
        "{output}"
    );
    assert!(output.contains(REDACTED), "{output}");
}

#[test]
fn a_new_history_entry_is_scrubbed_while_the_execution_input_is_kept() {
    let dir = tempfile::tempdir().unwrap();
    // Config resolution installs its own (staged) scope, so install the run
    // scope afterwards, exactly as the run entry does before the prompt loop.
    let config = config(dir.path());
    let _scope = install_scope(CANARY);
    let mut editor = ReplEditor::new(&config).unwrap();

    let line = format!("echo {CANARY}");
    editor.add_history_entry(&line).unwrap();
    // The execution input the caller still holds is untouched.
    assert!(line.contains(CANARY));

    let history_path = dir.path().join("history.txt");
    editor.save_history(&history_path).unwrap();
    let saved = std::fs::read_to_string(&history_path).unwrap();
    assert!(!saved.contains(CANARY), "{saved}");
    assert!(saved.contains(REDACTED), "{saved}");
}

#[test]
fn the_acceptance_sheet_copy_is_scrubbed_while_the_identity_input_is_kept() {
    let dir = tempfile::tempdir().unwrap();
    let _scope = install_scope(CANARY);
    let events = dir.path().join("events.jsonl");
    // A legacy record that still carries the raw secret.
    std::fs::write(
        &events,
        format!(
            "{{\"event\":\"tui_command_stop\",\"effective_profile\":\"ingest\",\"status\":\"failed\",\"assurance_level\":\"failed\",\"stop_reason\":\"leaked {CANARY}\"}}\n"
        ),
    )
    .unwrap();

    let identity = identity(dir.path(), format!("create ingest with {CANARY}"));
    let generated = sheet::generate(&identity, Some(&events), false).unwrap();
    assert!(
        !generated.markdown.contains(CANARY),
        "{}",
        generated.markdown
    );
    assert!(generated.markdown.contains(REDACTED));
    assert!(
        !generated
            .section5
            .as_deref()
            .unwrap_or_default()
            .contains(CANARY)
    );

    let path = sheet::persist(&dir.path().join("state"), &identity, &generated).unwrap();
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(!saved.contains(CANARY), "{saved}");
    assert!(saved.contains(REDACTED), "{saved}");

    let round = sheet::with_directive_metadata(
        sheet::generate(&identity, Some(&events), false).unwrap(),
        &directive_continuation(dir.path()),
    );
    let directive_path =
        sheet::persist_directive_round(&dir.path().join("state"), &identity, &round, 2).unwrap();
    let directive_saved = std::fs::read_to_string(&directive_path).unwrap();
    assert!(!directive_saved.contains(CANARY), "{directive_saved}");
    assert!(directive_saved.contains(REDACTED), "{directive_saved}");

    // The execution identity that was hashed is unchanged.
    assert!(identity.request.contains(CANARY));
}

fn directive_continuation(
    root: &Path,
) -> commandagent::tui::boundary_shell::directive::DirectiveContinuation {
    commandagent::tui::boundary_shell::directive::DirectiveContinuation {
        plan_path: root.join("plan/plan.yaml"),
        plan_workspace_path: root.join("plan").display().to_string(),
        target_run_id: "run-fixture".to_string(),
        directive_round: 2,
        directive_hash: "sha256:fixture".to_string(),
        regression_freeze: None,
    }
}

#[test]
fn a_confirmation_identity_that_contains_a_secret_is_refused_before_persistence() {
    let dir = tempfile::tempdir().unwrap();
    let _scope = install_scope(CANARY);

    let dirty = identity(dir.path(), format!("create ingest with {CANARY}"));
    let dirty_hash = dirty.card_hash().unwrap();
    let records = dir.path().join("records");
    let error = persist_confirmation(&records, &dirty, &dirty_hash).unwrap_err();
    assert!(!error.to_string().contains(CANARY), "{error}");
    assert!(
        !records.exists(),
        "a refused confirmation must not write a record"
    );
    // The hashed source was not rewritten to fake an approval.
    assert!(dirty.request.contains(CANARY));

    let clean = identity(dir.path(), "create ingest".to_string());
    let clean_hash = clean.card_hash().unwrap();
    let confirmed = persist_confirmation(&records, &clean, &clean_hash).unwrap();
    assert!(confirmed.record_path().exists());
    assert_eq!(confirmed.card_hash(), clean_hash);
}
