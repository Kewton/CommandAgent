//! Issue #903: a seeded step repair gets a short write advisory on its first
//! request. The host still re-runs the original verifier, so the advisory
//! never turns a write into a success by itself.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use commandagent::config::{
    Action, Config, ConfigFieldSources, NarrationMode, OllamaThink, OpenAiApi, PlanPreset,
    PromptLayout, Provider,
};
use commandagent::planner::run_step_plan_with_ui;
use commandagent::planner::step_plan::{PlanStep, StepPlan};
use commandagent::providers::{AssistantReply, ChatClient};
use commandagent::state::{ConversationMessage, ToolCall};
use commandagent::tools::registry::ToolSpec;
use commandagent::tui::NOOP_UI;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const ADVISORY: &str = "Make the concrete repair with Write/Edit now";
const TARGET: &str = "src/gameLogic.ts";
const BROKEN: &str = "export const score: number = \"high\";\n";
const FIXED: &str = "export const score: number = 1;\n";
const CORPUS: &str = "tests/corpus/apps/repair-start-write-guidance";

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

    fn requests(&self) -> Vec<Vec<ConversationMessage>> {
        self.requests.lock().unwrap().clone()
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
        max_iterations: 8,
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

fn verify_plan() -> StepPlan {
    StepPlan {
        goal: format!("Fix the score type\n\nRequired final artifacts:\n- {TARGET}"),
        steps: vec![PlanStep {
            id: "verify-types".to_string(),
            kind: "verify".to_string(),
            expected_result: "pass".to_string(),
            instruction: format!("Verify the score type in {TARGET} and repair it if wrong"),
            expected_paths: Vec::new(),
            verify: vec![format!("grep -q 'score: number = 1;' {TARGET}")],
        }],
    }
}

struct PlanRun {
    dir: tempfile::TempDir,
    result: Result<String, String>,
    requests: Vec<Vec<ConversationMessage>>,
}

impl PlanRun {
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

    fn source(&self) -> String {
        std::fs::read_to_string(self.dir.path().join(TARGET)).unwrap()
    }

    fn advisory_requests(&self) -> Vec<usize> {
        self.requests
            .iter()
            .enumerate()
            .filter(|(_, request)| {
                request
                    .iter()
                    .any(|message| message.content.contains(ADVISORY))
            })
            .map(|(index, _)| index)
            .collect()
    }
}

fn run_plan(replies: Vec<AssistantReply>) -> PlanRun {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join(TARGET), BROKEN).unwrap();
    let mut client = RecordingClient::new(replies);
    let result = run_step_plan_with_ui(&mut client, &verify_plan(), &config(dir.path()), &NOOP_UI)
        .map_err(|err| err.to_string());
    let requests = client.requests();
    PlanRun {
        dir,
        result,
        requests,
    }
}

#[test]
fn verify_repair_write_passes_only_through_host_reverification() {
    let run = run_plan(vec![
        tool_reply("Bash", json!({"command": "true"})),
        tool_reply("Write", json!({"path": TARGET, "content": FIXED})),
    ]);

    assert_eq!(run.result.as_deref(), Ok("plan-run complete: 1 steps"));
    assert_eq!(run.source(), FIXED);
    // The first observation turn has no carryover; only the repair start does.
    assert_eq!(run.advisory_requests(), vec![1]);
    let queued = run.named("repair_start_write_guidance_queued");
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0]["enforcement_changed"], false);
    let repairs = run.named("step_verify_repair");
    assert_eq!(repairs.len(), 1, "{repairs:#?}");
    assert_eq!(repairs[0]["ok"], true);
    assert!(run.named("read_only_tool_rejected").is_empty());
}

#[test]
fn verify_repair_write_that_keeps_the_failure_is_not_a_success() {
    let wrong = "export const score: number = 2;\n";
    let mut replies = vec![tool_reply("Bash", json!({"command": "true"}))];
    for _ in 0..8 {
        replies.push(tool_reply(
            "Write",
            json!({"path": TARGET, "content": wrong}),
        ));
    }
    let run = run_plan(replies);

    assert!(run.result.is_err(), "{:?}", run.result);
    assert_eq!(run.source(), wrong, "the failing source stays as written");
    assert!(!run.advisory_requests().is_empty());
    assert!(!run.advisory_requests().contains(&0));
    let repairs = run.named("step_verify_repair");
    assert!(!repairs.is_empty());
    assert!(
        repairs.iter().all(|repair| repair["ok"] != true),
        "{repairs:#?}"
    );
}

fn corpus_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(CORPUS)
        .join(name)
}

fn sha256_hex(path: &Path) -> String {
    let bytes = std::fs::read(path).unwrap();
    Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn recorded_sequences() -> Vec<Value> {
    std::fs::read_to_string(corpus_path("fixtures/recorded-sequences.jsonl"))
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
    assert_eq!(digests.len(), 2);
    for (name, digest) in digests {
        let actual = sha256_hex(&corpus_path(&format!("fixtures/{name}")));
        assert_eq!(actual, digest.as_str().unwrap(), "{name}");
    }
}

#[test]
fn recorded_sequences_show_the_first_request_without_write_guidance() {
    let records = recorded_sequences();
    assert!(!records.is_empty());
    for record in &records {
        let id = record["id"].as_str().unwrap();
        assert_eq!(record["kind"], "recorded", "{id}");
        for field in [
            "campaign",
            "source",
            "source_sha256",
            "source_code",
            "event_lines",
        ] {
            assert!(
                record[field].as_str().is_some_and(|v| !v.is_empty()),
                "{id} {field}"
            );
        }
        assert!(
            record["source"].as_str().unwrap().starts_with("<user>/"),
            "{id}"
        );
        assert_eq!(record["first_request_feedback"], "not_recorded", "{id}");
        assert_eq!(record["seed"]["pre_advanced"], true, "{id}");
        let seeded = record["seed"]["initial_read_only_streak"].as_u64().unwrap();
        let sequence: Vec<&str> = record["sequence"]
            .as_array()
            .unwrap()
            .iter()
            .map(|step| step.as_str().unwrap())
            .collect();
        // Before #903 the first tool batch ran unguarded and the write_required
        // feedback (at the seeded streak) only followed it.
        assert!(sequence[1].starts_with("turn=1 Read executed"), "{id}");
        assert_eq!(
            sequence[2],
            format!("feedback stage=write_required read_only_streak={seeded}"),
            "{id}"
        );
        let rejections: Vec<&&str> = sequence
            .iter()
            .filter(|step| step.contains("rejected attempts="))
            .collect();
        assert_eq!(rejections.len(), 2, "{id}");
        assert!(
            sequence
                .last()
                .unwrap()
                .contains("write_required_no_write_attempts=2")
        );
        assert!(
            !sequence
                .iter()
                .any(|step| step.contains("Write ok") || step.contains("Edit ok"))
        );
    }
}

#[test]
fn synthetic_cases_are_separate_and_name_existing_tests() {
    let cases: Value = serde_json::from_str(
        &std::fs::read_to_string(corpus_path("fixtures/scripted-cases.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(cases["kind"], "synthetic");
    let threshold = cases["write_required_threshold"].as_u64().unwrap();
    let leaf_tests = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/minimal_loop/loop_run/repair_start_feedback/tests.rs"),
    )
    .unwrap();
    for case in cases["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        assert!(case.get("source").is_none(), "{id} is synthetic");
        let test = case["test"].as_str().unwrap();
        assert!(leaf_tests.contains(&format!("fn {test}(")), "{id}: {test}");
        let initial = match case["expected_initial_streak"].as_str().unwrap() {
            "0" => 0,
            "T-2" => threshold - 2,
            "T-1" => threshold - 1,
            "T" => threshold,
            other => panic!("{id}: unknown streak {other}"),
        };
        let advisory = !case["carried"].is_null()
            && initial + 1 >= threshold
            && case["first_request_strong_feedback"] == false;
        assert_eq!(case["first_request_advisory"], advisory, "{id}");
    }
}

#[test]
fn corpus_carries_no_host_paths_or_secrets() {
    for name in [
        "expectations.toml",
        "source-manifest.json",
        "fixtures/recorded-sequences.jsonl",
        "fixtures/scripted-cases.json",
    ] {
        let text = std::fs::read_to_string(corpus_path(name)).unwrap();
        for forbidden in ["/Users/", "/home/", "OPENAI_API_KEY", "sk-", "Bearer "] {
            assert!(!text.contains(forbidden), "{name} contains {forbidden}");
        }
    }
}
