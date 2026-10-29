//! Issue #913: a direct prompt that asks to repair files which already exist
//! must not complete right after a read-only Bash, a visible test run, or an
//! Edit nobody answered for. The run continues and the model's final answer
//! and the existing action-prompt check decide. A request that creates a
//! missing file keeps the early completion.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use commandagent::config::{
    Action, Config, ConfigFieldSources, NarrationMode, OllamaThink, OpenAiApi, PlanPreset,
    PromptLayout, Provider,
};
use commandagent::minimal_loop::feedback::completion_without_write;
use commandagent::minimal_loop::loop_run::{
    RunSessionOutcome, RunStopReason, run_session_with_outcome_with_ui,
};
use commandagent::providers::{AssistantReply, ChatClient};
use commandagent::state::{ConversationMessage, SessionSnapshot, ToolCall};
use commandagent::tools::registry::ToolSpec;
use commandagent::tui::NOOP_UI;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const CORPUS: &str = "tests/corpus/apps/prompt-fix-early-completion";
const MISSING_TOOL_CALL: &str = "missing tool call for action prompt after feedback";
const AFTER_TOOL: &str = "required_artifacts_satisfied_after_tool";

const BROKEN: &str = "\"\"\"Date helpers.\"\"\"\nfrom datetime import date\n\n\ndef parse_date(text):\n    \"\"\"Parse YYYY-MM-DD or DD/MM/YYYY.\"\"\"\n    text = text.strip()\n    if \"/\" in text:\n        m, d, y = text.split(\"/\")\n        return date(int(y), int(m), int(d))\n    y, m, d = text.split(\"-\")\n    return date(int(y), int(m), int(d))\n";
const FIXED: &str = "\"\"\"Date helpers.\"\"\"\nfrom datetime import date\n\n\ndef parse_date(text):\n    \"\"\"Parse YYYY-MM-DD or DD/MM/YYYY.\"\"\"\n    text = text.strip()\n    if \"/\" in text:\n        d, m, y = text.split(\"/\")\n        return date(int(y), int(m), int(d))\n    y, m, d = text.split(\"-\")\n    return date(int(y), int(m), int(d))\n";
/// The visible test only covers the ISO form, so it passes on BROKEN.
const VISIBLE_TEST: &str = "import unittest\nfrom datetime import date\n\nfrom dates import parse_date\n\n\nclass VisibleTest(unittest.TestCase):\n    def test_iso(self):\n        self.assertEqual(parse_date(\"2026-04-03\"), date(2026, 4, 3))\n\n\nif __name__ == \"__main__\":\n    unittest.main()\n";
const NOTES: &str = "# Notes\n\nparse_date reads DD/MM/YYYY.\n";
const VISIBLE_TEST_COMMAND: &str = "python3 -m unittest discover -s tests -t .";

#[derive(Clone)]
struct RecordingClient {
    replies: Arc<Mutex<Vec<AssistantReply>>>,
    requests: Arc<Mutex<Vec<Vec<ConversationMessage>>>>,
}

impl RecordingClient {
    fn new(replies: Vec<AssistantReply>) -> Self {
        Self {
            replies: Arc::new(Mutex::new(replies)),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl ChatClient for RecordingClient {
    fn label(&self) -> &str {
        "recording"
    }

    fn supports_native_tools(&self, _model: &str) -> bool {
        true
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn chat(
        &mut self,
        _model: &str,
        messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        self.requests.lock().unwrap().push(messages.to_vec());
        let mut replies = self.replies.lock().unwrap();
        if replies.is_empty() {
            anyhow::bail!("scripted client exhausted");
        }
        Ok(replies.remove(0))
    }
}

fn tool_reply(name: &str, arguments: Value) -> AssistantReply {
    AssistantReply {
        content: String::new(),
        tool_calls: vec![ToolCall::new(name, arguments)],
        prompt_tokens: None,
        completion_tokens: None,
    }
}

fn bash(command: &str) -> AssistantReply {
    tool_reply("Bash", json!({"command": command}))
}

fn text_reply(content: &str) -> AssistantReply {
    AssistantReply {
        content: content.to_string(),
        tool_calls: Vec::new(),
        prompt_tokens: None,
        completion_tokens: None,
    }
}

fn config(root: &Path) -> Config {
    Config {
        workspace_root: root.to_path_buf(),
        state_dir: root.join("state"),
        eval_events_path: Some(root.join("events.jsonl")),
        completion_contract_path: None,
        yes: true,
        offline: false,
        context_budget: 100_000,
        model: "m".to_string(),
        provider: Provider::Ollama,
        tool_protocol: None,
        openai_api: OpenAiApi::ChatCompletions,
        prompt_layout: PromptLayout::Stable,
        plan_preset: PlanPreset::None,
        intent_override: None,
        planner_model: "m".to_string(),
        planner_provider: Provider::Ollama,
        planner_think: Some(OllamaThink::False),
        classifier_model: "m".to_string(),
        classifier_provider: Provider::Ollama,
        openai_compatible: None,
        ollama_host: "http://localhost:11434".to_string(),
        ollama_think: None,
        lm_studio_host: "http://localhost:1234".to_string(),
        num_predict: 100,
        max_iterations: 6,
        recovery_plan_auto_runs: 0,
        chat_timeout_secs: 30,
        chat_timeout_source: "override:test".to_string(),
        field_sources: ConfigFieldSources::default(),
        chat_retries: 1,
        stream: false,
        resume: None,
        fresh_session: false,
        no_footer: false,
        narration: NarrationMode::Normal,
        profile: "generic".to_string(),
        profile_explicit: false,
        profile_inference: None,
        style: "default".to_string(),
        action: Action::Repl,
    }
}

fn corpus_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(CORPUS)
        .join(name)
}

/// The e-mid instruction as the bench sent it: it asks to `直してください`
/// and names `dates.py` and `tests/test_dates.py`, which both exist.
fn e_mid_instruction() -> String {
    std::fs::read_to_string(corpus_path("fixtures/recorded/instruction.e-mid.ja.txt")).unwrap()
}

struct Run {
    dir: tempfile::TempDir,
    result: Result<RunSessionOutcome, String>,
    requests: Vec<Vec<ConversationMessage>>,
    remaining_replies: usize,
}

/// Runs a direct prompt in an e-mid shaped workspace: a broken `dates.py`, a
/// visible test that passes on it, and `repair_notes.md`.
fn run_direct(prompt: &str, replies: Vec<AssistantReply>) -> Run {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("dates.py"), BROKEN).unwrap();
    std::fs::write(dir.path().join("repair_notes.md"), NOTES).unwrap();
    std::fs::create_dir_all(dir.path().join("tests")).unwrap();
    std::fs::write(dir.path().join("tests/__init__.py"), "").unwrap();
    std::fs::write(dir.path().join("tests/test_dates.py"), VISIBLE_TEST).unwrap();
    let mut client = RecordingClient::new(replies);
    let mut session = SessionSnapshot::new();
    let result = run_session_with_outcome_with_ui(
        &mut client,
        &mut session,
        prompt,
        &[],
        &config(dir.path()),
        &NOOP_UI,
    )
    .map_err(|err| format!("{err:#}"));
    let requests = client.requests.lock().unwrap().clone();
    let remaining_replies = client.replies.lock().unwrap().len();
    Run {
        dir,
        result,
        requests,
        remaining_replies,
    }
}

impl Run {
    fn events(&self) -> Vec<Value> {
        std::fs::read_to_string(self.dir.path().join("events.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect()
    }

    fn named(&self, name: &str) -> Vec<Value> {
        self.events()
            .into_iter()
            .filter(|event| event["event"] == name)
            .collect()
    }

    fn after_tool_stops(&self) -> usize {
        self.named("loop_stop")
            .iter()
            .filter(|event| event["reason"] == AFTER_TOOL)
            .count()
    }

    fn scope(&self, field: &str) -> Value {
        let scopes = self.named("step_obligation_scope");
        assert_eq!(scopes.len(), 1, "{scopes:#?}");
        assert_eq!(scopes[0]["session_scope"], "minimal-loop");
        assert_eq!(scopes[0]["step_kind"], "");
        scopes[0][field].clone()
    }

    fn read(&self, path: &str) -> String {
        std::fs::read_to_string(self.dir.path().join(path)).unwrap()
    }

    fn assert_sources_unchanged(&self) {
        assert_eq!(self.read("dates.py"), BROKEN);
        assert_eq!(self.read("tests/test_dates.py"), VISIBLE_TEST);
        assert_eq!(self.read("repair_notes.md"), NOTES);
    }

    fn feedback_count(&self) -> usize {
        let feedback = completion_without_write();
        self.requests
            .last()
            .map(|messages| {
                messages
                    .iter()
                    .filter(|message| message.content.contains(&feedback))
                    .count()
            })
            .unwrap_or_default()
    }

    /// The tool result the model saw after its first tool call.
    fn first_tool_result(&self) -> String {
        self.requests[1]
            .iter()
            .rev()
            .find(|message| message.role == "tool")
            .map(|message| message.content.clone())
            .unwrap_or_default()
    }

    /// A repair request answered only with a tool that changed nothing and
    /// two final answers: feedback on the first, the action-prompt error on
    /// the second, and every scripted reply consumed.
    fn assert_held_to_a_change(&self, label: &str) {
        assert_eq!(
            self.result.as_ref().err().map(String::as_str),
            Some(MISSING_TOOL_CALL),
            "{label}"
        );
        assert_eq!(self.requests.len(), 3, "{label}");
        assert_eq!(self.remaining_replies, 0, "{label}");
        assert_eq!(
            self.feedback_count(),
            1,
            "{label}: completion-without-write feedback"
        );
        assert_eq!(self.after_tool_stops(), 0, "{label}");
        assert_eq!(self.scope("initially_missing_paths"), json!([]), "{label}");
        self.assert_sources_unchanged();
    }
}

fn final_pair() -> [AssistantReply; 2] {
    [
        text_reply("dates.py を確認しました。"),
        text_reply("dates.py を確認しました。修正は不要です。"),
    ]
}

#[test]
fn read_only_bash_does_not_complete_a_repair_request() {
    for command in ["ls -la", "cat dates.py", "sed -n '1,20p' dates.py"] {
        let [first, second] = final_pair();
        let run = run_direct(&e_mid_instruction(), vec![bash(command), first, second]);

        run.assert_held_to_a_change(command);
        assert_eq!(
            run.scope("effective_required_paths"),
            json!(["dates.py", "tests/test_dates.py"]),
            "{command}"
        );
        assert!(
            run.first_tool_result().starts_with("outcome: Success"),
            "{command}: {}",
            run.first_tool_result()
        );
    }
}

#[test]
fn visible_test_success_alone_does_not_complete_a_repair_request() {
    let [first, second] = final_pair();
    let run = run_direct(
        &e_mid_instruction(),
        vec![bash(VISIBLE_TEST_COMMAND), first, second],
    );

    run.assert_held_to_a_change(VISIBLE_TEST_COMMAND);
    // The visible test really ran and passed on the broken source.
    let result = run.first_tool_result();
    assert!(result.starts_with("outcome: Success"), "{result}");
    assert!(result.contains("OK"), "{result}");
}

#[test]
fn edit_of_an_existing_source_returns_the_model_final_answer() {
    let final_text = "parse_date の日と月の順を入れ替えました。";
    let run = run_direct(
        &e_mid_instruction(),
        vec![
            tool_reply(
                "Edit",
                json!({
                    "path": "dates.py",
                    "old_string": "m, d, y = text.split(\"/\")",
                    "new_string": "d, m, y = text.split(\"/\")",
                }),
            ),
            text_reply(final_text),
        ],
    );

    let outcome = run.result.as_ref().unwrap();
    assert_eq!(outcome.stop_reason, RunStopReason::AssistantFinal);
    assert_eq!(outcome.final_text, final_text);
    assert_eq!(outcome.changed_paths, vec!["dates.py".to_string()]);
    assert_eq!(run.requests.len(), 2);
    assert_eq!(run.remaining_replies, 0, "the final answer was consumed");
    assert_eq!(run.after_tool_stops(), 0);
    assert!(run.named("loop_stop").is_empty());
    assert_eq!(run.scope("initially_missing_paths"), json!([]));
    assert_eq!(run.read("dates.py"), FIXED);
    assert_eq!(run.read("tests/test_dates.py"), VISIBLE_TEST);
}

#[test]
fn each_repair_phrase_is_held_to_a_change() {
    for prompt in [
        "`dates.py` を直して",
        "`dates.py` を直してください",
        "`dates.py` を修正して",
        "`dates.py` のバグを直す",
        "fix `dates.py`",
        "repair `dates.py`",
    ] {
        let [first, second] = final_pair();
        let run = run_direct(prompt, vec![bash("cat dates.py"), first, second]);

        run.assert_held_to_a_change(prompt);
        assert_eq!(
            run.scope("effective_required_paths"),
            json!(["dates.py"]),
            "{prompt}"
        );
    }
}

#[test]
fn read_only_requests_complete_with_the_model_answer() {
    for (prompt, command) in [
        ("`repair_notes.md` を読んで説明して", "cat repair_notes.md"),
        ("Explain correctness of dates.py.", "cat dates.py"),
        ("Explain unresolved items in dates.py.", "cat dates.py"),
        ("dates.py の見直し結果を説明して", "cat dates.py"),
        ("Do not repair dates.py; only explain it.", "cat dates.py"),
        ("dates.py の中身を説明して", "cat dates.py"),
        (
            "dates.py は直すのではなく、問題点の説明だけして",
            "cat dates.py",
        ),
        (
            "Please do not try to repair dates.py; only explain it.",
            "cat dates.py",
        ),
    ] {
        let answer = "parse_date は月/日/年の順で読んでいます。";
        let run = run_direct(prompt, vec![bash(command), text_reply(answer)]);

        let outcome = run.result.as_ref().unwrap();
        assert_eq!(
            outcome.stop_reason,
            RunStopReason::AssistantFinal,
            "{prompt}"
        );
        assert_eq!(outcome.final_text, answer, "{prompt}");
        assert_eq!(run.requests.len(), 2, "{prompt}");
        assert_eq!(run.remaining_replies, 0, "{prompt}");
        assert_eq!(run.feedback_count(), 0, "{prompt}: no feedback");
        assert_eq!(run.after_tool_stops(), 0, "{prompt}");
        run.assert_sources_unchanged();
    }

    // The control: the same kind of request that also asks for the repair.
    let [first, second] = final_pair();
    let run = run_direct(
        "dates.py を直してから説明して",
        vec![bash("cat dates.py"), first, second],
    );
    run.assert_held_to_a_change("dates.py を直してから説明して");
}

#[test]
fn creating_a_missing_file_still_ends_after_tool() {
    for (prompt, required, missing) in [
        (
            "Write a summary of the date helpers to `summary.md`.",
            json!(["summary.md"]),
            json!(["summary.md"]),
        ),
        (
            "`dates.py` を読んで、`summary.md` に説明を書いて",
            json!(["dates.py", "summary.md"]),
            json!(["summary.md"]),
        ),
    ] {
        let run = run_direct(
            prompt,
            vec![
                tool_reply("Write", json!({"path": "summary.md", "content": NOTES})),
                text_reply("unused final"),
            ],
        );

        let outcome = run.result.as_ref().unwrap();
        assert_eq!(
            outcome.stop_reason,
            RunStopReason::RequiredArtifactsSatisfiedAfterTool,
            "{prompt}"
        );
        let names = required
            .as_array()
            .unwrap()
            .iter()
            .map(|path| path.as_str().unwrap())
            .collect::<Vec<_>>()
            .join(", ");
        assert_eq!(
            outcome.final_text,
            format!("required artifacts satisfied: {names}"),
            "{prompt}"
        );
        assert_eq!(
            run.remaining_replies, 1,
            "{prompt}: the final reply is not consumed"
        );
        assert_eq!(run.after_tool_stops(), 1, "{prompt}");
        assert_eq!(run.scope("effective_required_paths"), required, "{prompt}");
        assert_eq!(run.scope("initially_missing_paths"), missing, "{prompt}");
        assert_eq!(run.read("summary.md"), NOTES);
        assert_eq!(run.read("dates.py"), BROKEN);
    }
}

fn sha256_hex(path: &Path) -> String {
    let bytes = std::fs::read(path).unwrap();
    Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn read_json(name: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(corpus_path(name)).unwrap()).unwrap()
}

#[test]
fn corpus_fixtures_match_the_manifest_digests() {
    let manifest = read_json("source-manifest.json");
    let digests = manifest["fixture_sha256"].as_object().unwrap();
    assert_eq!(digests.len(), 4);
    for (name, digest) in digests {
        let actual = sha256_hex(&corpus_path(&format!("fixtures/{name}")));
        assert_eq!(actual, digest.as_str().unwrap(), "{name}");
    }
}

#[test]
fn recorded_runs_completed_after_tool_with_preexisting_paths_only() {
    let records = std::fs::read_to_string(corpus_path("fixtures/recorded/runs.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 12);
    let mut unchanged_incorrect = 0;
    for record in &records {
        let id = record["id"].as_str().unwrap();
        assert_eq!(record["kind"], "recorded", "{id}");
        assert_eq!(record["campaign"], "L-144", "{id}");
        assert_eq!(record["source_code"], "65878b06", "{id}");
        assert!(
            record["run_dir"].as_str().unwrap().starts_with("<bench>/"),
            "{id}"
        );
        assert!(
            record["events"]
                .as_str()
                .unwrap()
                .ends_with("/events.jsonl"),
            "{id}"
        );
        assert_eq!(record["events_sha256"].as_str().unwrap().len(), 64, "{id}");
        let task = record["task"].as_str().unwrap();
        assert!(matches!(task, "e-mid" | "e-edge"), "{id}");
        assert_eq!(
            record["instruction_sha256"].as_str().unwrap(),
            sha256_hex(&corpus_path(&format!(
                "fixtures/recorded/instruction.{task}.ja.txt"
            ))),
            "{id}"
        );
        // The direct minimal loop, every required path present at the start,
        // and no completion contract.
        assert_eq!(record["session_scope"], "minimal-loop", "{id}");
        assert_eq!(record["step_kind"], "", "{id}");
        assert_eq!(record["initially_missing_paths"], json!([]), "{id}");
        assert_eq!(record["completion_contract_paths"], json!([]), "{id}");
        let required = record["effective_required_paths"].as_array().unwrap();
        assert_eq!(required.len(), 2, "{id}");
        for path in required {
            let path = path.as_str().unwrap();
            assert!(
                record["workspace_initial_sha256"][path].as_str().is_some(),
                "{id}: {path} existed at the start"
            );
        }
        assert!(
            record["step_obligation_scope_line"].as_u64().unwrap()
                < record["loop_stop_line"].as_u64().unwrap(),
            "{id}"
        );
        assert_eq!(record["stop_reason"], AFTER_TOOL, "{id}");
        let calls = record["stop_batch_tool_calls"].as_array().unwrap();
        assert!(!calls.is_empty(), "{id}");
        let mutated = calls
            .iter()
            .any(|call| matches!(call["name"].as_str(), Some("Write" | "Edit")));
        let changed = record["changed_files"].as_array().unwrap();
        assert_eq!(mutated, !changed.is_empty(), "{id}");
        if changed.is_empty() {
            assert_eq!(
                record["workspace_initial_sha256"], record["workspace_final_sha256"],
                "{id}"
            );
        }
        let grade = record["host_grade"].as_str().unwrap();
        assert!(matches!(grade, "correct" | "incorrect"), "{id}");
        if changed.is_empty() && grade == "incorrect" {
            unchanged_incorrect += 1;
        }
    }
    assert!(unchanged_incorrect >= 7, "{unchanged_incorrect}");
}

#[test]
fn synthetic_cases_are_separate_and_name_existing_tests() {
    let cases = read_json("fixtures/synthetic/cases.json");
    assert_eq!(cases["kind"], "synthetic");
    let sources = [
        "src/minimal_loop/loop_run/after_tool_completion/tests.rs",
        "tests/prompt_fix_early_completion.rs",
    ]
    .map(|path| std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap());
    let cases = cases["cases"].as_array().unwrap();
    assert!(!cases.is_empty());
    for case in cases {
        let id = case["id"].as_str().unwrap();
        assert!(case.get("events_sha256").is_none(), "{id} is synthetic");
        let test = case["test"].as_str().unwrap();
        assert!(
            sources
                .iter()
                .any(|source| source.contains(&format!("fn {test}("))),
            "{id}: {test}"
        );
    }
}

#[test]
fn corpus_carries_no_host_paths_secrets_or_model_names() {
    for name in [
        "expectations.toml",
        "source-manifest.json",
        "fixtures/recorded/instruction.e-mid.ja.txt",
        "fixtures/recorded/instruction.e-edge.ja.txt",
        "fixtures/recorded/runs.jsonl",
        "fixtures/synthetic/cases.json",
    ] {
        let text = std::fs::read_to_string(corpus_path(name)).unwrap();
        for forbidden in [
            "/Users/",
            "/home/",
            "/Volumes/",
            "OPENAI_API_KEY",
            "sk-",
            "Bearer ",
            "\"model\"",
        ] {
            assert!(!text.contains(forbidden), "{name} contains {forbidden}");
        }
    }
}
