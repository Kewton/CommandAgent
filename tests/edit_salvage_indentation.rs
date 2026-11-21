//! Issue #914: the whitespace-normalized Edit salvage keeps the indentation
//! levels of the region it replaces. In L-146 b-t3 and e-t3 the model sent
//! class methods without the class indentation; the salvage wrote them at
//! the top level, so the methods left the class while the file still parsed.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use commandagent::config::{
    Action, Config, ConfigFieldSources, NarrationMode, OllamaThink, OpenAiApi, PlanPreset,
    PromptLayout, Provider,
};
use commandagent::minimal_loop::run_session;
use commandagent::providers::{AssistantReply, ChatClient};
use commandagent::state::{ConversationMessage, SessionSnapshot, ToolCall};
use commandagent::tools::registry::ToolSpec;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const CORPUS: &str = "tests/corpus/apps/edit-salvage-indentation";

const REPLAY: &str = r#"class Replay:
    def __init__(self):
        self.items = []

    def push(self, rec):
        self.items.append(rec)

    def records(self):
        return list(self.items)

    def report(self):
        return {"records": len(self.items)}


def replay(lines):
    state = Replay()
    for line in lines:
        state.push(line)
    return state.report()
"#;

/// `push` without the class indentation, as the b-t3 Edits sent it.
const REPLAY_OLD: &str = "def push(self, rec):\n    self.items.append(rec)";
const REPLAY_NEW: &str =
    "def push(self, rec):\n    if rec is None:\n        return\n    self.items.append(rec)";

const STOCK: &str = r#"class Stock:
    def __init__(self):
        self.held = {}

    def add(self, key, qty):
        self.held[key] = self.held.get(key, 0) + qty

    def cancel(self, key):
        self.held.pop(key, None)


def total(stock):
    return sum(stock.held.values())
"#;

/// `cancel` without the class indentation, with `confirm` and `expire`
/// appended at that same level, as the e-t3 Edits sent them.
const STOCK_OLD: &str = "def cancel(self, key):\n    self.held.pop(key, None)";
const STOCK_NEW: &str = "def cancel(self, key):\n    self.held.pop(key, None)\n\ndef confirm(self, key):\n    return self.held.get(key, 0)\n\ndef expire(self, key):\n    self.held[key] = 0";

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
            return Ok(text_reply("done"));
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

fn text_reply(content: &str) -> AssistantReply {
    AssistantReply {
        content: content.to_string(),
        tool_calls: Vec::new(),
        prompt_tokens: None,
        completion_tokens: None,
    }
}

fn edit_reply(path: &str, old: &str, new: &str) -> AssistantReply {
    tool_reply(
        "Edit",
        json!({"path": path, "old_string": old, "new_string": new, "replace_all": false}),
    )
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

struct Run {
    dir: tempfile::TempDir,
    requests: Vec<Vec<ConversationMessage>>,
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

    fn file(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    /// Edit results the model received in later requests.
    fn edit_results(&self) -> Vec<String> {
        let mut seen = Vec::new();
        for message in self.requests.iter().flatten() {
            if message.content.contains("edit_anchor") && !seen.contains(&message.content) {
                seen.push(message.content.clone());
            }
        }
        seen
    }
}

fn run_with(file: &str, content: &str, prompt: &str, replies: Vec<AssistantReply>) -> Run {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(file), content).unwrap();
    let mut client = RecordingClient::new(replies);
    let mut session = SessionSnapshot::new();
    let _ = run_session(&mut client, &mut session, prompt, &config(dir.path()));
    let requests = client.requests.lock().unwrap().clone();
    Run { dir, requests }
}

/// Class bodies and top-level functions of a Python file, read with `ast`.
fn python_membership(path: &Path) -> Value {
    let script = "import ast, json, sys\n\
tree = ast.parse(open(sys.argv[1]).read())\n\
defs = (ast.FunctionDef, ast.AsyncFunctionDef)\n\
print(json.dumps({\n\
    'classes': {n.name: [m.name for m in n.body if isinstance(m, defs)] for n in tree.body if isinstance(n, ast.ClassDef)},\n\
    'top_level_functions': [n.name for n in tree.body if isinstance(n, defs)],\n\
}))\n";
    let output = Command::new("python3")
        .arg("-I")
        .arg("-c")
        .arg(script)
        .arg(path)
        .output()
        .expect("run python3");
    assert!(output.status.success(), "{output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn scripted_b_t3_edit_keeps_push_inside_replay() {
    let run = run_with(
        "log_replay.py",
        REPLAY,
        "Update log_replay.py so that the push method of the Replay class ignores None records.",
        vec![
            tool_reply("Read", json!({"path": "log_replay.py"})),
            edit_reply("log_replay.py", REPLAY_OLD, REPLAY_NEW),
            text_reply("Updated log_replay.py."),
        ],
    );

    let salvaged = run.named("edit_anchor_salvaged");
    assert_eq!(salvaged.len(), 1, "{salvaged:#?}");
    assert_eq!(salvaged[0]["path"], "log_replay.py");
    assert!(
        salvaged[0]["result"]
            .as_str()
            .unwrap()
            .ends_with("via edit_anchor_salvaged at line 5")
    );
    assert_eq!(
        python_membership(&run.file("log_replay.py")),
        json!({
            "classes": {"Replay": ["__init__", "push", "records", "report"]},
            "top_level_functions": ["replay"],
        })
    );
    let text = std::fs::read_to_string(run.file("log_replay.py")).unwrap();
    assert!(
        text.contains("    def push(self, rec):\n        if rec is None:\n            return\n        self.items.append(rec)\n"),
        "{text}"
    );
}

#[test]
fn scripted_e_t3_edit_keeps_confirm_and_expire_inside_stock() {
    let run = run_with(
        "inv_model.py",
        STOCK,
        "Add confirm and expire methods to the Stock class in inv_model.py.",
        vec![
            tool_reply("Read", json!({"path": "inv_model.py"})),
            edit_reply("inv_model.py", STOCK_OLD, STOCK_NEW),
            text_reply("Added the methods to inv_model.py."),
        ],
    );

    assert_eq!(run.named("edit_anchor_salvaged").len(), 1);
    assert_eq!(
        python_membership(&run.file("inv_model.py")),
        json!({
            "classes": {"Stock": ["__init__", "add", "cancel", "confirm", "expire"]},
            "top_level_functions": ["total"],
        })
    );
}

#[test]
fn scripted_mixed_level_edit_is_refused_with_the_current_excerpt() {
    // `new` starts at the real level but keeps the body shallow: the salvage
    // cannot show the levels, so the model gets the excerpt and re-anchors.
    let mixed =
        "    def push(self, rec):\n    if rec is None:\n        return\n    self.items.append(rec)";
    let exact_old = "    def push(self, rec):\n        self.items.append(rec)";
    let exact_new = "    def push(self, rec):\n        if rec is None:\n            return\n        self.items.append(rec)";
    let run = run_with(
        "log_replay.py",
        REPLAY,
        "Update log_replay.py so that the push method of the Replay class ignores None records.",
        vec![
            tool_reply("Read", json!({"path": "log_replay.py"})),
            edit_reply("log_replay.py", REPLAY_OLD, mixed),
            edit_reply("log_replay.py", exact_old, exact_new),
            text_reply("Updated log_replay.py."),
        ],
    );

    assert!(run.named("edit_anchor_salvaged").is_empty());
    let failures = run
        .named("tool_validation_error")
        .into_iter()
        .filter(|event| event["name"] == "Edit")
        .collect::<Vec<_>>();
    assert_eq!(failures.len(), 1, "{failures:#?}");
    assert_eq!(failures[0]["error_kind"], "edit_anchor_not_found");
    let results = run.edit_results();
    let refusal = results
        .iter()
        .find(|content| content.contains("edit_anchor_not_found"))
        .unwrap_or_else(|| panic!("no refusal reached the model: {results:#?}"));
    assert!(refusal.contains("log_replay.py:5"), "{refusal}");
    // Non-Read tool results reach the model as a whitespace-flattened
    // snippet, so the excerpt keeps its line numbers and text in order.
    assert!(
        refusal.contains("5 | def push(self, rec): 6 | self.items.append(rec)"),
        "{refusal}"
    );
    assert!(refusal.contains("Re-anchor mandate"), "{refusal}");
    assert_eq!(
        python_membership(&run.file("log_replay.py")),
        json!({
            "classes": {"Replay": ["__init__", "push", "records", "report"]},
            "top_level_functions": ["replay"],
        })
    );
    let text = std::fs::read_to_string(run.file("log_replay.py")).unwrap();
    assert!(text.contains("            return\n"), "{text}");
}

fn corpus_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(CORPUS)
        .join(name)
}

fn sha256_hex(path: &Path) -> String {
    Sha256::digest(std::fs::read(path).unwrap())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn corpus_fixtures_match_the_manifest_digests() {
    let manifest: Value = serde_json::from_str(
        &std::fs::read_to_string(corpus_path("source-manifest.json")).unwrap(),
    )
    .unwrap();
    let digests = manifest["fixture_sha256"].as_object().unwrap();
    assert_eq!(digests.len(), 2);
    for (name, digest) in digests {
        let actual = sha256_hex(&corpus_path(&format!("fixtures/{name}")));
        assert_eq!(actual, digest.as_str().unwrap(), "{name}");
    }
}

#[test]
fn recorded_salvages_moved_methods_to_the_top_level_at_the_salvaged_line() {
    let records = std::fs::read_to_string(corpus_path("fixtures/recorded/runs.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 6);
    let mut moved = 0;
    for record in &records {
        let id = record["id"].as_str().unwrap();
        assert_eq!(record["kind"], "recorded", "{id}");
        for field in [
            "campaign",
            "run_dir",
            "events",
            "events_sha256",
            "source_code",
        ] {
            assert!(
                record[field].as_str().is_some_and(|v| !v.is_empty()),
                "{id} {field}"
            );
        }
        assert!(record["run_dir"].as_str().unwrap().starts_with("<bench>/"));
        let target = record["target_file"].as_str().unwrap();
        let salvages = record["salvage_events"].as_array().unwrap();
        assert!(!salvages.is_empty(), "{id}");
        let target_lines = salvages
            .iter()
            .map(|salvage| &salvage["event"])
            .inspect(|event| {
                assert_eq!(event["event"], "edit_anchor_salvaged", "{id}");
                assert_eq!(
                    event["method"], "whitespace_normalized_unique_region",
                    "{id}"
                );
            })
            .filter(|event| event["path"] == target)
            .map(|event| {
                let result = event["result"].as_str().unwrap();
                result.rsplit(' ').next().unwrap().parse::<u64>().unwrap()
            })
            .collect::<Vec<_>>();
        let membership = &record["target_final_membership"];
        let top_lines = membership["top_level_function_lines"].as_object().unwrap();
        let moved_here = top_lines
            .values()
            .any(|line| target_lines.contains(&line.as_u64().unwrap()));
        match record["host_grade"].as_str().unwrap() {
            "incorrect" => {
                // A method the salvage wrote starts a top-level function.
                assert!(moved_here, "{id}: {top_lines:?} vs {target_lines:?}");
                moved += 1;
            }
            "correct" => assert!(!moved_here, "{id}"),
            other => panic!("{id}: unexpected grade {other}"),
        }
    }
    assert_eq!(moved, 5);
}

#[test]
fn synthetic_cases_are_separate_and_name_existing_tests() {
    let cases: Value = serde_json::from_str(
        &std::fs::read_to_string(corpus_path("fixtures/synthetic/cases.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(cases["kind"], "synthetic");
    let sources = [
        "src/tools/edit.rs",
        "src/tools/edit/anchor_salvage/tests.rs",
        "tests/edit_salvage_indentation.rs",
    ]
    .map(|path| std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap());
    for case in cases["cases"].as_array().unwrap() {
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
            "gemma",
            "gpt-",
            "qwen",
        ] {
            assert!(!text.contains(forbidden), "{name} contains {forbidden}");
        }
    }
}
