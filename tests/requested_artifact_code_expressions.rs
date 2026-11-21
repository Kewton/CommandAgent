//! Issue #905: backticked code expressions and quoted data in an instruction
//! are not requested artifact paths. The e-mid instruction asks to fix
//! `dates.py`; its `parse_date("03/04/2026")` and `"31/12/2026"` examples
//! must not become obligations that only a bogus Write can satisfy.

use std::path::{Path, PathBuf};
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

const CORPUS: &str = "tests/corpus/apps/requested-artifact-code-expressions";
const BROKEN: &str = "def parse_date(text):\n    m, d, y = text.strip().split(\"/\")\n";
const FIXED: &str = "def parse_date(text):\n    d, m, y = text.strip().split(\"/\")\n";
const TESTS: &str = "import unittest\n";

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

fn instruction() -> String {
    std::fs::read_to_string(corpus_path("fixtures/recorded/instruction.ja.txt")).unwrap()
}

struct Run {
    dir: tempfile::TempDir,
    result: Result<String, String>,
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
}

fn run_e_mid(with_source: bool, replies: Vec<AssistantReply>) -> Run {
    let dir = tempfile::tempdir().unwrap();
    if with_source {
        std::fs::write(dir.path().join("dates.py"), BROKEN).unwrap();
    }
    std::fs::create_dir_all(dir.path().join("tests")).unwrap();
    std::fs::write(dir.path().join("tests/test_dates.py"), TESTS).unwrap();
    let mut client = RecordingClient::new(replies);
    let mut session = SessionSnapshot::new();
    let result = run_session(
        &mut client,
        &mut session,
        &instruction(),
        &config(dir.path()),
    )
    .map_err(|err| format!("{err:#}"));
    let requests = client.requests.lock().unwrap().clone();
    Run {
        dir,
        result,
        requests,
    }
}

const QUOTED_EXAMPLES: [&str; 2] = [r#"parse_date("03/04/2026")"#, r#""31/12/2026""#];

impl Run {
    /// Messages, other than the instruction itself, that name a quoted
    /// example from the instruction: feedback that would steer the model
    /// toward writing a file of that name.
    fn quoted_example_mentions(&self) -> Vec<String> {
        let first_line = instruction().lines().next().unwrap().to_string();
        self.requests
            .iter()
            .flatten()
            .filter(|message| !message.content.contains(&first_line))
            .filter(|message| {
                QUOTED_EXAMPLES
                    .iter()
                    .any(|example| message.content.contains(example))
            })
            .map(|message| message.content.clone())
            .collect()
    }

    fn assert_no_quoted_example_files(&self) {
        // `parse_date("03/04/2026")` and `"31/12/2026"` would land under
        // these first components.
        for first in [r#"parse_date("03"#, r#""31"#] {
            assert!(!self.dir.path().join(first).exists(), "{first} was created");
        }
    }
}

fn scope_paths(run: &Run, field: &str) -> Vec<String> {
    let scopes = run.named("step_obligation_scope");
    assert_eq!(scopes.len(), 1, "{scopes:#?}");
    serde_json::from_value(scopes[0][field].clone()).unwrap()
}

#[test]
fn fixed_source_completes_without_quoted_example_obligations() {
    let run = run_e_mid(
        true,
        vec![
            tool_reply("Read", json!({"path": "dates.py"})),
            tool_reply("Write", json!({"path": "dates.py", "content": FIXED})),
            text_reply("dates.py を直しました。"),
        ],
    );

    // Issue #913: both requested paths existed at the start, so the run ends
    // on the model's final answer rather than right after the Write.
    assert_eq!(run.result.as_deref(), Ok("dates.py を直しました。"));
    let expected = vec!["dates.py".to_string(), "tests/test_dates.py".to_string()];
    assert_eq!(scope_paths(&run, "prompt_extracted_paths"), expected);
    assert_eq!(scope_paths(&run, "effective_required_paths"), expected);
    assert!(scope_paths(&run, "initially_missing_paths").is_empty());
    let stops = run.named("loop_stop");
    assert!(stops.is_empty(), "{stops:#?}");
    assert_eq!(
        std::fs::read_to_string(run.dir.path().join("dates.py")).unwrap(),
        FIXED
    );
    run.assert_no_quoted_example_files();
    assert!(
        run.quoted_example_mentions().is_empty(),
        "{:#?}",
        run.quoted_example_mentions()
    );
}

#[test]
fn missing_requested_source_still_fails_on_that_file_alone() {
    let mut replies = vec![tool_reply("Read", json!({"path": "tests/test_dates.py"}))];
    for _ in 0..8 {
        replies.push(text_reply("問題ありません。"));
    }
    let run = run_e_mid(false, replies);

    let error = run.result.as_ref().unwrap_err();
    assert!(
        error.starts_with("artifact_follow_through_exhausted: missing expected paths: dates.py;"),
        "{error}"
    );
    assert_eq!(
        scope_paths(&run, "initially_missing_paths"),
        vec!["dates.py".to_string()]
    );
    let stops = run.named("loop_stop");
    assert_eq!(stops.len(), 1, "{stops:#?}");
    assert_eq!(stops[0]["reason"], "artifact_follow_through_exhausted");
    assert_eq!(stops[0]["non_scaffold_missing_paths"], json!(["dates.py"]));
    assert!(!run.dir.path().join("dates.py").exists());
    run.assert_no_quoted_example_files();
    assert!(
        run.quoted_example_mentions().is_empty(),
        "{:#?}",
        run.quoted_example_mentions()
    );
}

fn sha256_hex(path: &Path) -> String {
    let bytes = std::fs::read(path).unwrap();
    Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn recorded_runs() -> Vec<Value> {
    std::fs::read_to_string(corpus_path("fixtures/recorded/runs.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect()
}

#[test]
fn corpus_fixtures_match_the_manifest_digests() {
    let manifest: Value = serde_json::from_str(
        &std::fs::read_to_string(corpus_path("source-manifest.json")).unwrap(),
    )
    .unwrap();
    let digests = manifest["fixture_sha256"].as_object().unwrap();
    assert_eq!(digests.len(), 3);
    for (name, digest) in digests {
        let actual = sha256_hex(&corpus_path(&format!("fixtures/{name}")));
        assert_eq!(actual, digest.as_str().unwrap(), "{name}");
    }
}

#[test]
fn recorded_runs_promoted_the_quoted_examples_to_obligations() {
    let records = recorded_runs();
    assert_eq!(records.len(), 9);
    let instruction = instruction();
    let instruction_digest = sha256_hex(&corpus_path("fixtures/recorded/instruction.ja.txt"));
    let recorded_set = json!([
        "dates.py",
        "parse_date(\"03/04/2026\")",
        "\"31/12/2026\"",
        "tests/test_dates.py"
    ]);
    for record in &records {
        let id = record["id"].as_str().unwrap();
        assert_eq!(record["kind"], "recorded", "{id}");
        for field in [
            "campaign",
            "arm",
            "run_dir",
            "events",
            "events_sha256",
            "source_code",
            "stop_reason",
        ] {
            assert!(
                record[field].as_str().is_some_and(|v| !v.is_empty()),
                "{id} {field}"
            );
        }
        assert!(
            record["run_dir"].as_str().unwrap().starts_with("<bench>/"),
            "{id}"
        );
        assert_eq!(record["instruction_sha256"], instruction_digest, "{id}");
        let line_number = record["instruction_line"].as_u64().unwrap() as usize;
        let line = instruction.lines().nth(line_number - 1).unwrap();
        for span in record["instruction_backticked_spans"].as_array().unwrap() {
            assert!(
                line.contains(&format!("`{}`", span.as_str().unwrap())),
                "{id}: {span}"
            );
        }
        assert_eq!(record["prompt_extracted_paths"], recorded_set, "{id}");
        assert_eq!(record["explicit_required_paths"], json!([]), "{id}");
        assert_eq!(record["host_grade"], "correct", "{id}");
        let missing = record["non_scaffold_missing_paths"].as_array().unwrap();
        let writes = record["bogus_write_call_lines"].as_array().unwrap();
        match record["stop_reason"].as_str().unwrap() {
            "artifact_follow_through_exhausted" => {
                assert!(!missing.is_empty(), "{id}");
                assert!(
                    missing
                        .iter()
                        .all(|path| { QUOTED_EXAMPLES.contains(&path.as_str().unwrap()) }),
                    "{id}: only quoted examples were missing"
                );
            }
            "required_artifacts_satisfied_after_tool" => {
                assert!(missing.is_empty(), "{id}");
                assert_eq!(writes.len(), 2, "{id}: satisfied only by writing both");
            }
            other => panic!("{id}: unexpected stop {other}"),
        }
    }
}

#[test]
fn synthetic_cases_are_separate_and_name_existing_tests() {
    let cases: Value = serde_json::from_str(
        &std::fs::read_to_string(corpus_path("fixtures/synthetic/cases.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(cases["kind"], "synthetic");
    let sources = [
        "src/minimal_loop/loop_run/requested_artifacts/tests.rs",
        "tests/requested_artifact_code_expressions.rs",
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
fn corpus_carries_no_host_paths_or_secrets() {
    for name in [
        "expectations.toml",
        "source-manifest.json",
        "fixtures/recorded/instruction.ja.txt",
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
        ] {
            assert!(!text.contains(forbidden), "{name} contains {forbidden}");
        }
    }
}
