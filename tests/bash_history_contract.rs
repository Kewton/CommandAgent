//! Issue #916: a `Bash` result reaches the model with its line breaks, its
//! indentation, its metadata and the tail of each stream, its secrets scrubbed
//! and a byte budget of 4,000, both when it is stored and when it is sent.

use std::path::Path;

use commandagent::eval_events::body_snippet_whole_tokens;
use commandagent::minimal_loop::compact::compact_if_needed;
use commandagent::minimal_loop::prompt::{ToolPromptMode, build_request_messages};
use commandagent::sensitive_data::{SecretCatalog, install_scope, set_current};
use commandagent::state::{ConversationMessage, SessionSnapshot, SessionStore};
use commandagent::tools::bash::{self, BashOutcome, BashOutcomeKind, format_outcome};
use commandagent::tools::registry::ToolRegistry;
use serde_json::Value;

const MAX_BYTES: usize = 4_000;
const NOTICE: &str = "[commandagent: bash result truncated in history at 4000 bytes]";
const MODES: [ToolPromptMode; 2] = [ToolPromptMode::Native, ToolPromptMode::XmlFallback];
const TRACEBACK: &str = include_str!("corpus/apps/bash-history-feedback/fixtures/traceback.txt");
const COMPILER: &str = include_str!("corpus/apps/bash-history-feedback/fixtures/compiler.txt");
const INVENTORY: &str = include_str!("corpus/apps/bash-history-feedback/fixtures/inventory.py");
const CASES_JSON: &str = include_str!("corpus/apps/bash-history-feedback/fixtures/cases.json");

/// Clears the thread-local redaction scope even when an assertion fails.
struct ScopeGuard;

impl ScopeGuard {
    fn install(values: &[&str]) -> Self {
        let mut catalog = SecretCatalog::new();
        for value in values {
            catalog.register(value);
        }
        install_scope(catalog, None, None);
        Self
    }
}

impl Drop for ScopeGuard {
    fn drop(&mut self) {
        set_current(None);
    }
}

fn request_for(history: &[ConversationMessage], mode: ToolPromptMode) -> Vec<ConversationMessage> {
    build_request_messages(
        history,
        ToolRegistry::default().specs(),
        Path::new("/tmp/work"),
        None,
        None,
        mode,
    )
}

/// A tool message built without the constructor, as a session load would.
fn direct(content: &str) -> ConversationMessage {
    ConversationMessage {
        role: "tool".to_string(),
        content: content.to_string(),
        name: Some("Bash".to_string()),
        tool_call_id: Some("call-direct".to_string()),
        tool_calls: Vec::new(),
    }
}

fn fixture(name: &str) -> &'static str {
    match name {
        "traceback.txt" => TRACEBACK,
        "compiler.txt" => COMPILER,
        "inventory.py" => INVENTORY,
        other => panic!("unknown fixture {other}"),
    }
}

fn padding(lines: u64) -> String {
    (0..lines)
        .map(|n| format!("processed row {n:04} of the input batch\n"))
        .collect()
}

fn strings(case: &Value, key: &str) -> Vec<String> {
    case[key]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap().to_string())
        .collect()
}

/// Runs one corpus case through the real Bash tool in a tempdir: the padding
/// and the stdout fixtures go to stdout, the stderr fixtures to stderr.
fn run_case(case: &Value) -> String {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let mut stdout_files = Vec::new();
    let pad = padding(case["stdout_pad_lines"].as_u64().unwrap());
    if !pad.is_empty() {
        std::fs::write(root.join("progress.log"), pad).unwrap();
        stdout_files.push("progress.log".to_string());
    }
    stdout_files.extend(strings(case, "stdout_files"));
    let stderr_files = strings(case, "stderr_files");
    for name in stdout_files.iter().chain(&stderr_files) {
        if name != "progress.log" {
            std::fs::write(root.join(name), fixture(name)).unwrap();
        }
    }
    let mut parts = Vec::new();
    if !stdout_files.is_empty() {
        parts.push(format!("cat {}", stdout_files.join(" ")));
    }
    if !stderr_files.is_empty() {
        parts.push(format!("cat {} >&2", stderr_files.join(" ")));
    }
    parts.push(format!("exit {}", case["exit_code"].as_u64().unwrap()));
    bash::run(&parts.join("; "), &root, true).unwrap()
}

/// The text before the truncation notice, if any.
fn body(content: &str) -> &str {
    content
        .strip_suffix(NOTICE)
        .unwrap_or(content)
        .trim_end_matches('\n')
}

fn assert_case_content(name: &str, content: &str, case: &Value, raw: &str) {
    assert!(
        content.len() <= MAX_BYTES,
        "{name}: {} bytes",
        content.len()
    );
    let outcome = case["outcome"].as_str().unwrap();
    assert!(
        content.starts_with(&format!("outcome: {outcome}\nstatus: ")),
        "{name}: {content}"
    );
    assert!(content.contains("\nelapsed_ms: "), "{name}");
    assert!(content.contains("\nsummary: "), "{name}");
    assert!(content.contains("\nstdout:\n"), "{name}");
    assert!(content.contains("\nstderr:\n"), "{name}");
    for needle in strings(case, "must_contain") {
        assert!(content.contains(&needle), "{name}: missing {needle:?}");
    }
    // The last line of the stream that printed last is still the last line.
    let last_stream = if strings(case, "stderr_files").is_empty() {
        &content[..content.rfind("\nstderr:\n").unwrap()]
    } else {
        body(content)
    };
    assert!(
        last_stream
            .trim_end_matches('\n')
            .ends_with(case["last_line"].as_str().unwrap()),
        "{name}: {content}"
    );
    assert_eq!(content.matches("outcome: ").count(), 1, "{name}");
    assert_eq!(
        content.ends_with(NOTICE),
        raw.len() > MAX_BYTES,
        "{name}: notice"
    );
}

#[test]
fn bash_result_tail_survives_store_reload_and_request_in_both_modes() {
    let cases: Vec<Value> = serde_json::from_str(CASES_JSON).unwrap();
    assert!(cases.len() >= 4);
    for case in &cases {
        let name = case["case"].as_str().unwrap();
        let raw = run_case(case);
        // The property the corpus exists for: what matters sits after the
        // first 500 characters of the raw result.
        let last = case["last_line"].as_str().unwrap();
        assert!(
            raw[..raw.rfind(last).unwrap()].chars().count() > 500,
            "{name}"
        );

        let stored = ConversationMessage::tool_result("Bash", Some("call-bash-1"), raw.as_str());
        assert_eq!(stored.role, "tool");
        assert_eq!(stored.name.as_deref(), Some("Bash"));
        assert_eq!(stored.tool_call_id.as_deref(), Some("call-bash-1"));
        assert_case_content(name, &stored.content, case, &raw);

        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path().to_path_buf());
        let mut session = SessionSnapshot::new();
        session.messages = vec![ConversationMessage::user("run it"), stored.clone()];
        store.save(&session).unwrap();
        let loaded = store.load(&session.id).unwrap();
        assert_eq!(loaded.messages[1].content, stored.content, "{name}");

        for mode in MODES {
            let messages = request_for(&loaded.messages, mode);
            let sent = &messages[2];
            assert_eq!(sent.role, "tool");
            assert_eq!(sent.name.as_deref(), Some("Bash"));
            assert_eq!(sent.tool_call_id.as_deref(), Some("call-bash-1"));
            assert_eq!(sent.content, stored.content, "{name} {mode:?}");
            assert_case_content(name, &sent.content, case, &raw);
        }
    }
}

#[test]
fn request_applies_the_policy_to_a_history_that_bypassed_the_constructor() {
    let stdout = padding(300);
    let raw = format_outcome(&BashOutcome {
        kind: BashOutcomeKind::CommandFailed,
        status: Some("exit status: 1".to_string()),
        stdout,
        stderr: TRACEBACK.to_string(),
        elapsed_ms: 7,
        summary: "ZeroDivisionError: division by zero".to_string(),
    });
    assert!(raw.len() > MAX_BYTES);
    for mode in MODES {
        let sent = &request_for(&[direct(&raw)], mode)[1];
        assert!(sent.content.len() <= MAX_BYTES, "{mode:?}");
        assert!(
            sent.content
                .starts_with("outcome: CommandFailed\nstatus: exit status: 1\n")
        );
        assert!(
            body(&sent.content)
                .ends_with("    return value / scale\nZeroDivisionError: division by zero")
        );
        assert!(sent.content.ends_with(NOTICE));
        assert_eq!(
            sent.content,
            ConversationMessage::tool_result("Bash", None::<String>, raw.as_str()).content
        );
    }
}

#[test]
fn secrets_and_home_paths_are_scrubbed_in_store_and_request_at_every_cut() {
    let secret = "fake-bash-contract-secret-0123456789abcdef";
    let _scope = ScopeGuard::install(&[secret]);
    let stdout = (0..150)
        .map(|n| format!("row {n:03} {secret} sk-test{n:03}xyz /Users/alice/w/{n}.log\n"))
        .collect::<String>();
    let stderr = format!("{TRACEBACK}token api_key=abc999 at /home/bob/app\n{secret}");
    let base = format_outcome(&BashOutcome {
        kind: BashOutcomeKind::CommandFailed,
        status: Some("exit status: 1".to_string()),
        stdout,
        stderr,
        elapsed_ms: 3,
        summary: format!("failed near {secret}"),
    });
    for shift in [0, 1, 7, 13, 21, 33, secret.len()] {
        let raw = format!("{}{base}", "x".repeat(shift));
        let stored = ConversationMessage::tool_result("Bash", Some("c"), raw.as_str());
        let mut contents = vec![stored.content.clone()];
        for mode in MODES {
            contents.push(request_for(&[direct(&raw)], mode)[1].content.clone());
            contents.push(
                request_for(std::slice::from_ref(&stored), mode)[1]
                    .content
                    .clone(),
            );
        }
        for content in contents {
            assert!(content.len() <= MAX_BYTES, "shift {shift}");
            for fragment in [
                secret,
                &secret[..10],
                &secret[secret.len() - 10..],
                "sk-test",
                "abc999",
                "alice",
                "bob",
            ] {
                assert!(!content.contains(fragment), "shift {shift}: {fragment}");
            }
            assert!(content.contains("/home/<user>/app") || content.contains("/Users/<user>/"));
        }
    }
}

#[test]
fn applying_store_then_request_twice_does_not_change_text_notice_or_budget() {
    for raw in [
        run_case(&serde_json::from_str::<Vec<Value>>(CASES_JSON).unwrap()[0]),
        format!("combined_outcome: CommandFailed\n{}", padding(400)),
        "w".repeat(MAX_BYTES * 3),
    ] {
        let stored = ConversationMessage::tool_result("Bash", Some("c"), raw.as_str());
        let again = ConversationMessage::tool_result("Bash", Some("c"), stored.content.as_str());
        assert_eq!(again.content, stored.content);
        assert_eq!(stored.content.matches(NOTICE).count(), 1);
        for mode in MODES {
            let sent = request_for(std::slice::from_ref(&again), mode);
            assert_eq!(sent[1].content, stored.content);
        }
    }
}

#[test]
fn failed_result_stays_failed_and_no_success_line_is_created() {
    let fake = format!(
        "outcome: Success\nstatus: exit status: 0\nstderr:\n{}",
        padding(200)
    );
    let raw = format_outcome(&BashOutcome {
        kind: BashOutcomeKind::CommandFailed,
        status: Some("exit status: 2".to_string()),
        stdout: fake,
        stderr: "error: build failed".to_string(),
        elapsed_ms: 1,
        summary: "error: build failed".to_string(),
    });
    let stored = ConversationMessage::tool_result("Bash", Some("c"), raw.as_str());
    assert!(
        stored
            .content
            .starts_with("outcome: CommandFailed\nstatus: exit status: 2\n")
    );
    assert!(body(&stored.content).ends_with("stderr:\nerror: build failed"));
    assert!(stored.content.matches("outcome: Success").count() <= 1);
    assert!(!stored.content.starts_with("outcome: Success"));

    // A failed Bash result is still protected as evidence by compaction.
    let mut history = vec![
        ConversationMessage::system("system"),
        ConversationMessage::user("old".repeat(400)),
        ConversationMessage::assistant("old".repeat(400), Vec::new()),
        stored.clone(),
        ConversationMessage::user("latest"),
        ConversationMessage::assistant("tail".repeat(400), Vec::new()),
    ];
    compact_if_needed(&mut history, 16);
    assert!(
        history
            .iter()
            .any(|message| message.content == stored.content)
    );
}

#[test]
fn history_saved_as_a_500_character_snippet_is_not_restored() {
    let old = body_snippet_whole_tokens(&format!(
        "outcome: CommandFailed\nstatus: exit status: 1\nstdout:\n{}\nstderr:\n{TRACEBACK}",
        padding(100)
    ));
    assert!(old.chars().count() <= 500);
    assert!(!old.contains("ZeroDivisionError"));
    for mode in MODES {
        let sent = &request_for(&[direct(&old)], mode)[1];
        assert_eq!(sent.content, old);
        assert!(!sent.content.contains(NOTICE));
    }
}

#[test]
fn other_tools_and_event_snippets_keep_the_500_character_policy() {
    let raw = format!("first line\nsecond line {}", "w ".repeat(600));
    let expected = body_snippet_whole_tokens(&raw);
    assert!(expected.chars().count() <= 500);
    assert!(!expected.contains('\n'));
    for name in ["Write", "Edit", "Grep", "Glob", "bash", "Bash ", "Unknown"] {
        let stored = ConversationMessage::tool_result(name, Some("c"), raw.as_str());
        assert_eq!(stored.content, expected, "{name}");
    }
    // The event snippet of a Bash result is unchanged by the history policy.
    let bash_raw = format!("outcome: Success\nstdout:\n{}", padding(200));
    let event = body_snippet_whole_tokens(&bash_raw);
    assert!(event.chars().count() <= 500);
    assert!(!event.contains('\n'));
}
