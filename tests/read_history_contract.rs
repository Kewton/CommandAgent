//! Issue #845: a `Read` result reaches the model with its line breaks, its
//! secrets scrubbed and a byte budget of 24,000, not as a 500-character
//! flattened snippet.

use std::path::Path;

use commandagent::eval_events::body_snippet_whole_tokens;
use commandagent::minimal_loop::compact::compact_if_needed;
use commandagent::minimal_loop::prompt::{ToolPromptMode, build_request_messages};
use commandagent::sensitive_data::{SecretCatalog, install_scope, set_current};
use commandagent::state::{ConversationMessage, SessionSnapshot, SessionStore, ToolCall};
use commandagent::tools::read;
use commandagent::tools::registry::ToolRegistry;
use commandagent::tools::workspace_policy::WorkspacePolicy;
use serde_json::{Value, json};

const MAX_BYTES: usize = 24_000;
const NOTICE: &str = "[commandagent: read result truncated in history at 24000 bytes]";
const NOTES_TS: &str = include_str!("corpus/apps/read-history-detail/fixtures/notes.ts");
const GAME_LOGIC_TS: &str = include_str!("corpus/apps/read-history-detail/fixtures/game-logic.ts");
const REQUESTS_JSON: &str = include_str!("corpus/apps/read-history-detail/fixtures/requests.json");

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

fn fixture(name: &str) -> &'static str {
    match name {
        "notes.ts" => NOTES_TS,
        "game-logic.ts" => GAME_LOGIC_TS,
        other => panic!("unknown fixture {other}"),
    }
}

fn requests() -> Vec<Value> {
    serde_json::from_str(REQUESTS_JSON).expect("requests.json parses")
}

/// Runs the real `Read` tool against `content` written to `rel` in a tempdir.
fn run_read(rel: &str, content: &str, range: Option<(usize, usize)>) -> String {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, content).unwrap();
    read::run(
        &root,
        &path,
        range.map(|(start, _)| start),
        range.map(|(_, end)| end),
        WorkspacePolicy::NormalTask,
    )
    .unwrap()
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

fn read_of_len(total: usize) -> String {
    let mut out = String::from("src/a.ts\n");
    while out.len() < total {
        let remaining = total - out.len();
        if remaining >= 10 {
            out.push_str("abcdefghi\n");
        } else {
            out.push_str(&"z".repeat(remaining));
        }
    }
    assert_eq!(out.len(), total);
    out
}

#[test]
fn long_read_keeps_exports_and_line_breaks_in_store_and_request() {
    for case in requests() {
        let name = case["case"].as_str().unwrap();
        let rel = case["workspace_path"].as_str().unwrap();
        let output = run_read(rel, fixture(case["fixture"].as_str().unwrap()), None);
        let symbols = case["must_contain"].as_array().unwrap();

        // The property the corpus exists for: the first symbol sits after 500
        // characters and 20 lines of what the Read tool returned.
        let first = symbols[0].as_str().unwrap();
        let offset = output.find(first).unwrap();
        assert!(output[..offset].chars().count() > 500, "{name}");
        assert!(output[..offset].lines().count() > 20, "{name}");
        assert_eq!(
            output[..offset].lines().count(),
            case["first_export_line"].as_u64().unwrap() as usize,
            "{name}"
        );

        let stored = ConversationMessage::tool_result("Read", Some("call-read-1"), output.as_str());
        for symbol in symbols {
            assert!(stored.content.contains(symbol.as_str().unwrap()), "{name}");
        }
        assert!(stored.content.lines().count() > 20, "{name}");
        assert_eq!(stored.role, "tool");
        assert_eq!(stored.name.as_deref(), Some("Read"));
        assert_eq!(stored.tool_call_id.as_deref(), Some("call-read-1"));

        for mode in [ToolPromptMode::Native, ToolPromptMode::XmlFallback] {
            let messages = request_for(std::slice::from_ref(&stored), mode);
            let sent = &messages[1];
            for symbol in symbols {
                assert!(sent.content.contains(symbol.as_str().unwrap()), "{name}");
            }
            assert!(sent.content.lines().count() > 20, "{name}");
            assert_eq!(sent.role, "tool");
            assert_eq!(sent.name.as_deref(), Some("Read"));
            assert_eq!(sent.tool_call_id.as_deref(), Some("call-read-1"));
            assert_eq!(sent.content, stored.content);
        }
    }
}

#[test]
fn assistant_preamble_before_a_tool_call_is_still_dropped() {
    let history = vec![
        ConversationMessage::assistant(
            "Let me read it.",
            vec![ToolCall::new("Read", json!({"path": "src/lib/notes.ts"}))],
        ),
        ConversationMessage::tool_result(
            "Read",
            Some("call-1"),
            run_read("notes.ts", NOTES_TS, None),
        ),
    ];
    let messages = request_for(&history, ToolPromptMode::Native);
    assert!(messages[1].content.is_empty());
    assert_eq!(messages[1].tool_calls.len(), 1);
    assert!(
        messages[2]
            .content
            .contains("export async function getNotes")
    );
}

#[test]
fn constructor_and_request_each_apply_the_policy_to_a_direct_or_loaded_history() {
    let long = format!(
        "src/a.ts\n{}\nexport const TAIL_EXPORT = 1;",
        "x ".repeat(1_500)
    );
    assert!(long.chars().count() > 500);

    // A struct built without the constructor, as a session load would.
    let direct = ConversationMessage {
        role: "tool".to_string(),
        content: long.clone(),
        name: Some("Read".to_string()),
        tool_call_id: Some("call-direct".to_string()),
        tool_calls: Vec::new(),
    };
    let constructed = ConversationMessage::tool_result("Read", Some("call-ctor"), long.clone());
    assert!(constructed.content.contains("TAIL_EXPORT"));

    let dir = tempfile::tempdir().unwrap();
    let store = SessionStore::new(dir.path().to_path_buf());
    let mut session = SessionSnapshot::new();
    session.messages = vec![direct.clone(), constructed.clone()];
    store.save(&session).unwrap();
    let loaded = store.load(&session.id).unwrap();
    assert_eq!(loaded.messages.len(), 2);

    for history in [&[direct][..], &[constructed][..], &loaded.messages[..]] {
        for message in request_for(history, ToolPromptMode::Native).iter().skip(1) {
            assert!(message.content.contains("TAIL_EXPORT"));
            assert!(message.content.contains('\n'));
            assert!(message.content.len() <= MAX_BYTES);
        }
    }
}

#[test]
fn history_saved_at_500_characters_is_not_restored() {
    let old = ConversationMessage {
        role: "tool".to_string(),
        content: format!("src/a.ts {}", "x ".repeat(245))
            .trim_end()
            .to_string(),
        name: Some("Read".to_string()),
        tool_call_id: Some("call-old".to_string()),
        tool_calls: Vec::new(),
    };
    let messages = request_for(std::slice::from_ref(&old), ToolPromptMode::Native);
    assert_eq!(messages[1].content, old.content);
}

#[test]
fn read_results_around_the_byte_budget_stay_within_it() {
    for total in [MAX_BYTES - 1, MAX_BYTES, MAX_BYTES + 1, MAX_BYTES * 4] {
        let input = read_of_len(total);
        let stored = ConversationMessage::tool_result("Read", Some("c"), input.as_str());
        let sent = request_for(std::slice::from_ref(&stored), ToolPromptMode::XmlFallback);
        for content in [&stored.content, &sent[1].content] {
            assert!(content.len() <= MAX_BYTES, "total {total}");
            assert!(
                content.starts_with("src/a.ts\nabcdefghi\n"),
                "total {total}"
            );
            if total <= MAX_BYTES {
                assert_eq!(content, &input, "total {total}");
            } else {
                assert!(content.ends_with(NOTICE), "total {total}");
                assert_eq!(content.matches(NOTICE).count(), 1, "total {total}");
            }
        }
        assert_eq!(stored.content, sent[1].content, "total {total}");
    }
}

#[test]
fn multibyte_and_long_single_line_reads_stay_valid_and_bounded() {
    let wide = format!("src/a.ts\n{}", "日本語€😀".repeat(MAX_BYTES));
    let one_line = format!("src/b.ts\n{}", "q".repeat(MAX_BYTES * 2));
    for input in [wide, one_line] {
        let stored = ConversationMessage::tool_result("Read", None::<String>, input);
        assert!(stored.content.len() <= MAX_BYTES);
        assert!(stored.content.ends_with(NOTICE));
        assert!(std::str::from_utf8(stored.content.as_bytes()).is_ok());
    }
}

#[test]
fn different_ranges_of_one_file_are_both_kept() {
    let first = run_read("src/lib/notes.ts", NOTES_TS, Some((1, 20)));
    let second = run_read("src/lib/notes.ts", NOTES_TS, Some((28, 40)));
    let history = vec![
        ConversationMessage::tool_result("Read", Some("r1"), first),
        ConversationMessage::tool_result("Read", Some("r2"), second),
    ];
    let messages = request_for(&history, ToolPromptMode::Native);
    assert!(messages[1].content.contains("readNotes"));
    assert!(!messages[1].content.contains("getNotes"));
    assert!(
        messages[2]
            .content
            .contains("export async function getNotes")
    );
    assert!(messages[2].content.contains('\n'));
}

#[test]
fn compact_rereads_do_not_push_out_the_first_read() {
    let first = run_read("src/lib/notes.ts", NOTES_TS, None);
    let head = NOTES_TS.lines().take(20).collect::<Vec<_>>().join("\n");
    let reread = format!(
        "src/lib/notes.ts\n[commandagent: unchanged since the previous matching Read; returning at most the first 20 cached lines. Avoid reading it again unless the file changes.]\n{head}"
    );
    let mut history = vec![ConversationMessage::tool_result("Read", Some("r1"), first)];
    for index in 0..4 {
        history.push(ConversationMessage::tool_result(
            "Read",
            Some(format!("r{}", index + 2)),
            reread.as_str(),
        ));
    }
    let messages = request_for(&history, ToolPromptMode::Native);
    assert!(
        messages[1]
            .content
            .contains("export async function deleteNote")
    );
    for message in &messages[2..] {
        assert!(
            message
                .content
                .contains("unchanged since the previous matching Read")
        );
        assert!(message.content.contains('\n'));
    }
}

#[test]
fn tools_other_than_read_keep_the_flattened_snippet() {
    let input = format!("first line\nsecond line {}", "w ".repeat(600));
    let expected = body_snippet_whole_tokens(&input);
    // `Bash` keeps line breaks under its own policy (tests/bash_history_contract.rs).
    for name in ["Write", "Edit", "Grep", "read", "Unknown"] {
        let stored = ConversationMessage::tool_result(name, Some("c"), input.as_str());
        assert_eq!(stored.content, expected, "{name}");
        assert!(stored.content.chars().count() <= 500, "{name}");
        assert!(!stored.content.contains('\n'), "{name}");
        let direct = ConversationMessage {
            role: "tool".to_string(),
            content: input.clone(),
            name: Some(name.to_string()),
            tool_call_id: None,
            tool_calls: Vec::new(),
        };
        assert_eq!(
            request_for(&[direct], ToolPromptMode::Native)[1].content,
            expected,
            "{name}"
        );
    }
    let unnamed = ConversationMessage {
        role: "tool".to_string(),
        content: input.clone(),
        name: None,
        tool_call_id: None,
        tool_calls: Vec::new(),
    };
    assert_eq!(
        request_for(&[unnamed], ToolPromptMode::Native)[1].content,
        expected
    );
    let user = ConversationMessage::user(input.clone());
    assert_eq!(
        request_for(&[user], ToolPromptMode::Native)[1].content,
        input
    );
}

const SECRET_REGISTERED_JA: &str = "秘密のカナリア-FAKE-0004-ほげほげ";
const SECRET_REGISTERED_MULTILINE: &str =
    "-----BEGIN FAKE KEY-----\nfakecanary0005line\n-----END FAKE KEY-----";
const SECRET_STRADDLE: &str = "STRADDLE-FAKE-canary-0006-abcdefghijklmnop";
const SECRET_PREFIXED: &str = "sk-test-FAKE-canary-after-500-0001";
const SECRET_GOOGLE: &str = "AIzaFAKEcanaryHeuristic0002";
const SECRET_NAMED: &str = "FAKE_api_key=canary0003";
const FAKE_HOME_NAME: &str = "fakehomeperson";

/// Strings that must not appear anywhere: full values and distinctive pieces.
const FORBIDDEN: [&str; 14] = [
    SECRET_REGISTERED_JA,
    SECRET_REGISTERED_MULTILINE,
    SECRET_STRADDLE,
    SECRET_PREFIXED,
    SECRET_GOOGLE,
    SECRET_NAMED,
    "カナリア",
    "fakecanary0005",
    "STRADDLE",
    "canary-after-500",
    "FAKEcanaryHeuristic",
    "canary0003",
    "abcdefghijklmnop",
    FAKE_HOME_NAME,
];

fn canary_read() -> String {
    let mut out = String::from("src/lib/secrets.ts\n");
    out.push_str(&format!("// {}\n", "a".repeat(600)));
    out.push_str("export const SAFE_EXPORT_AFTER_500 = 1;\n");
    out.push_str(&format!(
        "const one = {SECRET_PREFIXED} const two = {SECRET_GOOGLE}\n"
    ));
    out.push_str(&format!("{SECRET_NAMED} and later\n"));
    out.push_str(&format!("const ja = {SECRET_REGISTERED_JA};\n"));
    out.push_str(&format!("const pem = {SECRET_REGISTERED_MULTILINE};\n"));
    out.push_str(&format!(
        "const home = /Users/{FAKE_HOME_NAME}/project/src/x.ts\n"
    ));
    // Copies of the registered value at many offsets, so one of them lands on
    // the 24,000-byte cut.
    for index in 0..80 {
        out.push_str(&format!(
            "line {index:03} {SECRET_STRADDLE} {}\n",
            "f".repeat(300)
        ));
    }
    assert!(out.len() > MAX_BYTES + 1_000);
    out
}

fn assert_clean(label: &str, text: &str) {
    for forbidden in FORBIDDEN {
        assert!(!text.contains(forbidden), "{label} leaked {forbidden:?}");
    }
}

#[test]
fn secrets_never_reach_store_request_or_session_file() {
    let _guard = ScopeGuard::install(&[
        SECRET_REGISTERED_JA,
        SECRET_REGISTERED_MULTILINE,
        SECRET_STRADDLE,
    ]);
    let stored = ConversationMessage::tool_result("Read", Some("call-secret"), canary_read());
    assert!(stored.content.len() <= MAX_BYTES);
    assert!(stored.content.ends_with(NOTICE));
    assert!(stored.content.contains("SAFE_EXPORT_AFTER_500"));
    assert!(stored.content.contains("/Users/<user>/project"));
    assert_clean("stored", &stored.content);

    // The unscrubbed original, as a loaded or directly built history would hold.
    let direct = ConversationMessage {
        role: "tool".to_string(),
        content: canary_read(),
        name: Some("Read".to_string()),
        tool_call_id: Some("call-direct".to_string()),
        tool_calls: Vec::new(),
    };
    for history in [&[stored.clone()][..], &[direct][..]] {
        for mode in [ToolPromptMode::Native, ToolPromptMode::XmlFallback] {
            let sent = &request_for(history, mode)[1];
            assert!(sent.content.len() <= MAX_BYTES);
            assert!(sent.content.contains("SAFE_EXPORT_AFTER_500"));
            assert_clean("request", &sent.content);
        }
    }

    let dir = tempfile::tempdir().unwrap();
    let store = SessionStore::new(dir.path().to_path_buf());
    let mut session = SessionSnapshot::new();
    session.messages = vec![stored];
    let path = store.save(&session).unwrap();
    assert_clean("session.json", &std::fs::read_to_string(path).unwrap());
}

#[test]
fn secret_straddling_the_cut_inside_one_long_line_leaves_no_fragment() {
    let secret = "fake-longline-secret-ABCDEFGHIJ0123456789";
    let _guard = ScopeGuard::install(&[secret]);
    // 15 bytes of the secret fall inside the head budget; the line has no
    // newline, so the cut can only land inside the secret.
    let head_budget = MAX_BYTES - NOTICE.len() - 1;
    let inside = 15;
    let pad = head_budget - inside - "src/a.ts\n".len();
    let input = format!(
        "src/a.ts\n{}{secret} {}",
        "p".repeat(pad),
        "r".repeat(2_000)
    );
    assert!(!input[..head_budget].contains(secret));
    assert!(input[..head_budget].contains(&secret[..inside]));
    let stored = ConversationMessage::tool_result("Read", Some("c"), input.as_str());
    let sent = request_for(std::slice::from_ref(&stored), ToolPromptMode::Native);
    for content in [&stored.content, &sent[1].content] {
        assert!(content.len() <= MAX_BYTES);
        assert!(content.ends_with(NOTICE));
        assert!(!content.contains(secret));
        assert!(!content.contains(&secret[..inside]));
        assert!(!content.contains("fake-longl"));
    }
}

#[test]
fn applying_the_policy_twice_does_not_grow_or_leak() {
    let _guard = ScopeGuard::install(&[
        SECRET_REGISTERED_JA,
        SECRET_REGISTERED_MULTILINE,
        SECRET_STRADDLE,
    ]);
    let stored = ConversationMessage::tool_result("Read", Some("c"), canary_read());
    let again = ConversationMessage::tool_result("Read", Some("c"), stored.content.as_str());
    let sent = request_for(std::slice::from_ref(&again), ToolPromptMode::Native);
    let sent_twice = request_for(std::slice::from_ref(&sent[1]), ToolPromptMode::Native);
    for content in [&again.content, &sent[1].content, &sent_twice[1].content] {
        assert_eq!(content, &stored.content);
        assert_eq!(content.matches(NOTICE).count(), 1);
        assert!(content.len() <= MAX_BYTES);
        assert!(content.contains("SAFE_EXPORT_AFTER_500"));
        assert_clean("double", content);
    }
}

#[test]
fn compact_only_drops_whole_messages_and_keeps_read_content_intact() {
    let reads = (0..8)
        .map(|index| {
            ConversationMessage::tool_result(
                "Read",
                Some(format!("r{index}")),
                format!("src/f{index}.ts\n{}", "line of code\n".repeat(1_800)),
            )
        })
        .collect::<Vec<_>>();
    let mut base = vec![ConversationMessage::system("system")];
    base.extend(reads.iter().cloned());
    base.push(ConversationMessage::user("latest user"));

    // Plenty of room: nothing is dropped and nothing is shortened.
    let mut roomy = base.clone();
    compact_if_needed(&mut roomy, 200_000);
    assert_eq!(roomy, base);

    // Tight budget: the existing compaction removes whole messages only.
    let mut tight = base.clone();
    compact_if_needed(&mut tight, 4_000);
    assert!(tight.len() < base.len());
    for message in tight.iter().filter(|m| m.role == "tool") {
        assert!(reads.contains(message));
        assert!(message.content.len() <= MAX_BYTES);
    }
    assert!(tight.iter().any(|m| m.content == "latest user"));
}

/// The pre-send `check_correlation` contract (Issue #1005), mirrored without loosening it:
/// every call ID is non-empty and used once, every tool output names an earlier call and
/// answers it once, and every call is answered.
fn correlation_holds(messages: &[ConversationMessage]) -> bool {
    let mut calls = std::collections::BTreeSet::new();
    let mut answered = std::collections::BTreeSet::new();
    for message in messages {
        for call in &message.tool_calls {
            if call.id.is_empty() || !calls.insert(call.id.as_str()) {
                return false;
            }
        }
        if message.role != "tool" {
            continue;
        }
        let Some(id) = message.tool_call_id.as_deref() else {
            return false;
        };
        if !calls.contains(id) || !answered.insert(id) {
            return false;
        }
    }
    answered.len() == calls.len()
}

fn bash_success(id: &str) -> ConversationMessage {
    ConversationMessage::tool_result(
        "Bash",
        Some(id.to_string()),
        format!(
            "outcome: Success\nstatus: exit status: 0\nstdout:\n{}",
            "line of output\n".repeat(300)
        ),
    )
}

fn bash_calls(ids: &[&str]) -> ConversationMessage {
    ConversationMessage::assistant(
        String::new(),
        ids.iter()
            .map(|id| ToolCall {
                id: (*id).to_string(),
                name: "Bash".to_string(),
                arguments: json!({"command": "true"}),
            })
            .collect(),
    )
}

fn read_calls(ids: &[&str]) -> ConversationMessage {
    ConversationMessage::assistant(
        String::new(),
        ids.iter()
            .map(|id| ToolCall {
                id: (*id).to_string(),
                name: "Read".to_string(),
                arguments: json!({"path": "src/a.ts"}),
            })
            .collect(),
    )
}

/// Issue #1088: when the compact budget falls inside a multi-call turn, dropping the call
/// message alone would leave its outputs without their calls. The call and its outputs are
/// removed together, so every surviving call keeps exactly one output.
#[test]
fn compaction_never_leaves_a_tool_output_without_its_call() {
    let mut messages = vec![
        ConversationMessage::system("system"),
        bash_calls(&["b1", "b2", "b3"]),
        bash_success("b1"),
        bash_success("b2"),
        bash_success("b3"),
        read_calls(&["r1", "r2"]),
        ConversationMessage::tool_result("Read", Some("r1"), "src/a.ts\nanchor one"),
        ConversationMessage::tool_result("Read", Some("r2"), "src/b.ts\nanchor two"),
        ConversationMessage::user("latest user"),
        ConversationMessage::assistant("tail".repeat(400), Vec::new()),
    ];
    let total: usize = messages.iter().map(|m| m.content.len()).sum();
    // The budget boundary lands a single Bash output short of the whole turn: the old
    // per-message removal would stop there and leave the remaining outputs orphaned.
    let context_budget = total.saturating_sub(4_000).div_ceil(4);
    compact_if_needed(&mut messages, context_budget);

    assert!(correlation_holds(&messages), "{messages:#?}");
    let calls: Vec<&str> = messages
        .iter()
        .flat_map(|m| m.tool_calls.iter().map(|c| c.id.as_str()))
        .collect();
    let outputs: Vec<&str> = messages
        .iter()
        .filter(|m| m.role == "tool")
        .filter_map(|m| m.tool_call_id.as_deref())
        .collect();
    assert_eq!(calls.len(), outputs.len());
}

/// Issue #1088: an older turn that carries protected evidence is not split. Its call message
/// stays with its outputs even when the budget presses on it.
#[test]
fn compaction_keeps_a_protected_evidence_turn_whole() {
    let mut messages = vec![
        ConversationMessage::system("system"),
        read_calls(&["r1", "r2"]),
        ConversationMessage::tool_result("Read", Some("r1"), "src/a.ts\nanchor one"),
        ConversationMessage::tool_result("Read", Some("r2"), "src/b.ts\nanchor two"),
        bash_calls(&["b1", "b2"]),
        bash_success("b1"),
        bash_success("b2"),
        ConversationMessage::user("latest user"),
        ConversationMessage::assistant("tail".repeat(400), Vec::new()),
    ];
    let total: usize = messages.iter().map(|m| m.content.len()).sum();
    let bash_chars: usize = messages[4..7].iter().map(|m| m.content.len()).sum();
    let context_budget = total.saturating_sub(bash_chars).div_ceil(4);
    compact_if_needed(&mut messages, context_budget);

    assert!(correlation_holds(&messages), "{messages:#?}");
    assert!(
        messages.iter().any(|m| m.content.contains("anchor one")),
        "protected Read evidence must survive"
    );
    assert!(
        messages.iter().any(|m| {
            m.tool_calls
                .iter()
                .any(|call| call.id == "r1" || call.id == "r2")
        }),
        "the evidence's calls must survive with their outputs"
    );
}

/// Issue #1088 (review blocker 1): a reply's output belongs to it only when its call ID answers
/// one of that reply's calls. If a later reply is removed first, its output must go with it, not
/// be absorbed into the earlier reply's protected run and left with no preceding call.
#[test]
fn compaction_does_not_absorb_a_later_replys_output_into_an_earlier_run() {
    let mut messages = vec![
        ConversationMessage::system("system"),
        read_calls(&["a1", "a2"]),
        ConversationMessage::tool_result("Read", Some("a1"), "src/a.ts\nanchor one"),
        bash_success("a2"),
        bash_calls(&["b1"]),
        bash_success("b1"),
        ConversationMessage::user("latest user"),
        ConversationMessage::assistant("tail".repeat(400), Vec::new()),
    ];
    let total: usize = messages.iter().map(|m| m.content.len()).sum();
    let b_output = messages[5].content.len();
    // Room for the two unprotected messages of reply B, but not for reply A's protected run.
    let context_budget = total.saturating_sub(b_output).div_ceil(4);
    compact_if_needed(&mut messages, context_budget);

    assert!(correlation_holds(&messages), "{messages:#?}");
    let calls: std::collections::BTreeSet<&str> = messages
        .iter()
        .flat_map(|m| m.tool_calls.iter().map(|call| call.id.as_str()))
        .collect();
    for output in messages.iter().filter(|m| m.role == "tool") {
        let id = output.tool_call_id.as_deref().unwrap_or_default();
        assert!(calls.contains(id), "output {id} has no preceding call");
    }
    for call in &calls {
        assert!(
            messages
                .iter()
                .filter(|m| m.tool_call_id.as_deref() == Some(*call))
                .count()
                == 1,
            "call {call} must have exactly one output"
        );
    }
}

/// Issue #1097 (the judge's `judge_single_call_residual`): a reply with a single call whose
/// output is not protected evidence used to keep the per-message rule. With the budget boundary
/// inside the reply's text, the call message was removed alone and its output was left naming no
/// call, so the history no longer correlated. The reply and its output now leave as one group, so
/// every remaining call keeps exactly one output. Synthetic values only.
#[test]
fn compaction_removes_a_single_call_reply_and_its_unprotected_output_together() {
    let call = ToolCall {
        id: "call_p1".to_string(),
        name: "SubmitConfig".to_string(),
        arguments: json!({"mode": "fast"}),
    };
    let text = "The object is ready. ".repeat(40);
    let mut messages = vec![
        ConversationMessage::system("system"),
        ConversationMessage::assistant(text.clone(), vec![call]),
        ConversationMessage::tool_result("SubmitConfig", Some("call_p1"), "accepted by the fake"),
        ConversationMessage::user("latest user"),
        ConversationMessage::assistant("done".to_string(), Vec::new()),
    ];
    let total: usize = messages.iter().map(|m| m.content.len()).sum();
    // The budget boundary falls inside the reply's text: removing the text alone brings the
    // history under budget, which is what used to leave the output behind.
    let budget = (total - text.len()).div_ceil(4);
    assert!(total > budget * 4, "the fixture must start over budget");
    assert!(
        total - text.len() <= budget * 4,
        "the text alone must bring the history within budget"
    );

    compact_if_needed(&mut messages, budget);

    assert!(correlation_holds(&messages), "{messages:#?}");
    let call_present = messages
        .iter()
        .any(|m| m.tool_calls.iter().any(|call| call.id == "call_p1"));
    let output_present = messages
        .iter()
        .any(|m| m.tool_call_id.as_deref() == Some("call_p1"));
    assert!(!call_present, "the single call leaves with its output");
    assert_eq!(call_present, output_present, "{messages:#?}");
    assert!(
        messages.iter().any(|m| m.content == "latest user"),
        "the latest user message must survive"
    );
}

/// Issue #1088 (review blocker 2): when only protected runs are left, the budget is served first,
/// so the oldest protected run is removed whole rather than sending a history far over budget.
/// The removed run never splits: its calls leave with their outputs.
#[test]
fn compaction_serves_the_budget_by_removing_the_oldest_protected_run_whole() {
    let ids: Vec<String> = (0..24).map(|n| format!("p{n}")).collect();
    let mut calls = Vec::new();
    let mut outputs = Vec::new();
    for (index, id) in ids.iter().enumerate() {
        let name = if index % 2 == 0 { "Read" } else { "Bash" };
        calls.push(ToolCall {
            id: id.clone(),
            name: name.to_string(),
            arguments: json!({"path": "src/a.ts", "command": "true"}),
        });
        outputs.push(if name == "Read" {
            ConversationMessage::tool_result(
                "Read",
                Some(id.as_str()),
                format!("src/a.ts\n{}", "a line of evidence\n".repeat(200)),
            )
        } else {
            bash_success(id)
        });
    }
    let mut messages = vec![ConversationMessage::system("system")];
    messages.push(ConversationMessage::assistant(String::new(), calls));
    messages.extend(outputs);
    messages.push(ConversationMessage::user("latest user"));
    let budget = 8_192;
    let base_result = per_message_compaction_len(&messages, budget);

    compact_if_needed(&mut messages, budget);

    let compacted: usize = messages.iter().map(|m| m.content.len()).sum();
    assert!(compacted <= budget * 4, "{compacted} is over budget");
    assert!(
        compacted <= base_result,
        "{compacted} is larger than the base result {base_result}"
    );
    assert!(correlation_holds(&messages), "{messages:#?}");
}

/// Issue #1096 (reviewer's example): a 7-message history over a 4,000-byte budget used to stop at
/// the four-message floor with a removable protected run (a Read call and its output) still in
/// place, so it was sent about 1.8x over budget. At the floor the whole call-and-output run now
/// leaves, so the budget is met or the overage is reported; either way the history correlates and
/// neither pinned message and no half of a split run is lost. Synthetic values only.
#[test]
fn compaction_at_the_four_message_floor_removes_a_removable_protected_run() {
    let call = ToolCall {
        id: "call_r1".to_string(),
        name: "Read".to_string(),
        arguments: json!({"path": "src/notes.ts"}),
    };
    let mut messages = vec![
        ConversationMessage::system("system"),
        ConversationMessage::user("earlier question ".repeat(60)),
        ConversationMessage::assistant("earlier answer ".repeat(60), Vec::new()),
        ConversationMessage::assistant(String::new(), vec![call]),
        ConversationMessage::tool_result(
            "Read",
            Some("call_r1"),
            format!("src/notes.ts\n{}", "a line of the file\n".repeat(400)),
        ),
        ConversationMessage::user("latest user"),
        ConversationMessage::assistant("tail note ".repeat(180), Vec::new()),
    ];
    let budget = 1_000;
    let before: usize = messages.iter().map(|m| m.content.len()).sum();
    // The old floor stopped with this removable protected run still present, over budget: the
    // four messages that survive the text removals alone are over the 4,000-byte budget.
    assert!(
        before > budget * 4,
        "the fixture must start over budget: {before}"
    );

    let outcome = compact_if_needed(&mut messages, budget);
    let after: usize = messages.iter().map(|m| m.content.len()).sum();

    assert_eq!(outcome.remaining_chars, after);
    // The budget is met: the removable protected run went whole.
    assert!(
        after <= budget * 4,
        "{after} is over the {} budget",
        budget * 4
    );
    assert!(!outcome.over_budget, "{outcome:?}");
    assert!(correlation_holds(&messages), "{messages:#?}");
    assert!(
        !messages
            .iter()
            .any(|m| m.tool_call_id.as_deref() == Some("call_r1")),
        "the removable protected run's output must leave"
    );
    assert!(
        !messages
            .iter()
            .any(|m| m.tool_calls.iter().any(|c| c.id == "call_r1")),
        "the removable protected run's call must leave with its output"
    );
    assert_eq!(messages.first().map(|m| m.content.as_str()), Some("system"));
    assert!(
        messages.iter().any(|m| m.content == "latest user"),
        "the latest user message must survive"
    );
}

/// Issue #1096: when nothing removable is left and the history is still over budget (here the
/// pinned head system message and the tail answer alone exceed it, and no call-and-output run
/// remains), compaction must report the overage explicitly instead of sending it silently.
#[test]
fn compaction_reports_an_over_budget_history_it_cannot_reduce_further() {
    let mut messages = vec![
        ConversationMessage::system(format!("system {}", "s".repeat(2_000))),
        ConversationMessage::user("latest user"),
        ConversationMessage::assistant(format!("tail {}", "t".repeat(2_000)), Vec::new()),
    ];
    let budget = 16;
    let before: usize = messages.iter().map(|m| m.content.len()).sum();
    assert!(before > budget * 4, "the fixture must start over budget");

    let outcome = compact_if_needed(&mut messages, budget);
    let after: usize = messages.iter().map(|m| m.content.len()).sum();

    assert!(
        outcome.over_budget,
        "an over-budget outcome must be reported"
    );
    assert_eq!(outcome.remaining_chars, after);
    assert!(outcome.budget_chars < after);
    // Nothing removable was split or dropped, and the pinned messages are untouched.
    assert_eq!(messages.len(), 3, "{messages:#?}");
    assert!(messages[0].content.starts_with("system "));
    assert_eq!(messages[1].content, "latest user");
    assert!(correlation_holds(&messages), "{messages:#?}");
}

/// The base per-message compaction (`origin/develop` before Issue #1088) run to completion on a
/// copy, returning the resulting character total. Only the length matters here, so the copy drops
/// the same messages the base did without needing their identity.
fn per_message_compaction_len(messages: &[ConversationMessage], context_budget: usize) -> usize {
    let approx_chars = context_budget.saturating_mul(4);
    let mut sizes: Vec<usize> = messages.iter().map(|m| m.content.len()).collect();
    let mut protected: Vec<bool> = vec![false; messages.len()];
    if messages
        .first()
        .is_some_and(|message| message.role == "system")
    {
        protected[0] = true;
    }
    if let Some(index) = messages.iter().rposition(|message| message.role == "user") {
        protected[index] = true;
    }
    let mut seen = 0;
    for (index, message) in messages.iter().enumerate().rev() {
        if message.role == "tool"
            && matches!(
                message.name.as_deref(),
                Some("Read") | Some("Edit") | Some("Bash")
            )
        {
            protected[index] = true;
            seen += 1;
            if seen >= 4 {
                break;
            }
        }
    }
    let mut total: usize = sizes.iter().sum();
    while total > approx_chars && sizes.len() > 4 {
        let remove_at = (1..sizes.len())
            .find(|index| !protected[*index])
            .unwrap_or(1);
        total = total.saturating_sub(sizes.remove(remove_at));
        protected.remove(remove_at);
    }
    total
}
