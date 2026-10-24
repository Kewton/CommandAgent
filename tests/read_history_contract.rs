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
    for name in ["Bash", "Write", "Edit", "Grep", "read", "Unknown"] {
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
