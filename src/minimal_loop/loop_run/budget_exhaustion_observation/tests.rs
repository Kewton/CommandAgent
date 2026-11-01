use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use super::BudgetObservation;
use crate::config::Config;
use crate::minimal_loop::execution_progress::ExecutionProgress;
use crate::providers::{AssistantReply, ChatClient};
use crate::state::{ConversationMessage, SessionSnapshot, ToolCall};
use crate::tools::registry::ToolSpec;
use crate::tui::NOOP_UI;

use super::super::{RunSessionOptions, RunSessionStepKind, run_session_with_outcome_with_options};

const FIXTURE: &str = include_str!(
    "../../../../tests/corpus/apps/budget-exhaustion-observation/fixtures/events.jsonl"
);
const PROMPT: &str = "Keep the notes file current and check it with shell commands.";
const FIELDS: [&str; 9] = [
    "iteration_limit",
    "iterations_used",
    "iteration_budget_exhausted",
    "successful_write_count",
    "last_successful_write_iteration",
    "bash_results_after_last_write",
    "bash_failures_after_last_write",
    "last_bash_after_last_write_succeeded",
    "write_failure_after_last_write",
];

fn fields(observation: &BudgetObservation, iteration_limit: usize) -> Value {
    observation.with_fields(json!({"event": "loop_stop"}), iteration_limit)
}

#[test]
fn a_run_without_a_write_reports_null_and_counts_bash_from_the_start() {
    let mut observation = BudgetObservation::default();
    for iteration in 1..=3 {
        observation.note_iteration(iteration);
    }
    observation.note_bash(ExecutionProgress::New);
    observation.note_bash(ExecutionProgress::NotSuccessful);
    observation.note_tool_error("Edit");
    let event = fields(&observation, 3);
    assert_eq!(event["iteration_limit"], 3);
    assert_eq!(event["iterations_used"], 3);
    assert_eq!(event["iteration_budget_exhausted"], true);
    assert_eq!(event["successful_write_count"], 0);
    assert_eq!(event["last_successful_write_iteration"], Value::Null);
    assert_eq!(event["bash_results_after_last_write"], 2);
    assert_eq!(event["bash_failures_after_last_write"], 1);
    assert_eq!(event["last_bash_after_last_write_succeeded"], false);
    assert_eq!(event["write_failure_after_last_write"], true);
}

#[test]
fn a_successful_write_resets_the_bash_counts_and_the_failure_flag() {
    let mut observation = BudgetObservation::default();
    observation.note_iteration(4);
    observation.note_bash(ExecutionProgress::NotSuccessful);
    observation.note_tool_error("Write");
    observation.note_successful_write();
    let event = fields(&observation, 9);
    assert_eq!(event["successful_write_count"], 1);
    assert_eq!(event["last_successful_write_iteration"], 4);
    assert_eq!(event["bash_results_after_last_write"], 0);
    assert_eq!(event["bash_failures_after_last_write"], 0);
    assert_eq!(event["last_bash_after_last_write_succeeded"], Value::Null);
    assert_eq!(event["write_failure_after_last_write"], false);
    observation.note_bash(ExecutionProgress::Repeated);
    let event = fields(&observation, 9);
    assert_eq!(event["bash_results_after_last_write"], 1);
    assert_eq!(event["bash_failures_after_last_write"], 0);
    assert_eq!(event["last_bash_after_last_write_succeeded"], true);
}

#[test]
fn only_write_and_edit_errors_set_the_failure_flag() {
    let mut observation = BudgetObservation::default();
    for name in ["Bash", "Read", "Glob", "Grep", "write", ""] {
        observation.note_tool_error(name);
    }
    assert_eq!(
        fields(&observation, 1)["write_failure_after_last_write"],
        false
    );
}

#[test]
fn existing_fields_are_kept_and_values_are_numbers_bools_or_null() {
    let mut observation = BudgetObservation::default();
    observation.note_iteration(2);
    observation.note_successful_write();
    let event = observation.with_fields(
        json!({"event": "loop_stop", "reason": "loop_progress_exhausted", "iteration_limit": "kept"}),
        2,
    );
    let object = event.as_object().unwrap();
    assert_eq!(object.len(), 2 + FIELDS.len(), "{event}");
    assert_eq!(object["reason"], "loop_progress_exhausted");
    assert_eq!(object["iteration_limit"], "kept");
    for field in FIELDS.iter().skip(1) {
        let value = &object[*field];
        assert!(
            value.is_number() || value.is_boolean() || value.is_null(),
            "{field}: {value}"
        );
    }
    let not_object = observation.with_fields(json!("loop_stop"), 2);
    assert_eq!(not_object, json!("loop_stop"));
}

// ---- end to end through the plan-step entry (crate-internal options) ----

#[derive(Clone)]
struct Scripted {
    replies: Arc<Mutex<Vec<AssistantReply>>>,
    requests: Arc<Mutex<Vec<Vec<ConversationMessage>>>>,
    delay: Duration,
}

impl ChatClient for Scripted {
    fn label(&self) -> &str {
        "scripted"
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn supports_native_tools(&self, _model: &str) -> bool {
        true
    }

    fn chat(
        &mut self,
        _model: &str,
        messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        std::thread::sleep(self.delay);
        self.requests.lock().unwrap().push(messages.to_vec());
        let mut replies = self.replies.lock().unwrap();
        if replies.is_empty() {
            anyhow::bail!("scripted client exhausted");
        }
        Ok(replies.remove(0))
    }
}

fn config(root: &Path, max_iterations: usize) -> Config {
    Config {
        workspace_root: root.to_path_buf(),
        state_dir: root.join("state"),
        eval_events_path: Some(root.join("events.jsonl")),
        completion_contract_path: None,
        yes: true,
        offline: false,
        context_budget: 100_000,
        model: "m".to_string(),
        provider: crate::config::Provider::Ollama,
        tool_protocol: None,
        openai_api: crate::config::OpenAiApi::ChatCompletions,
        prompt_layout: crate::config::PromptLayout::Stable,
        plan_preset: crate::config::PlanPreset::None,
        intent_override: None,
        planner_model: "m".to_string(),
        planner_provider: crate::config::Provider::Ollama,
        planner_think: Some(crate::config::OllamaThink::False),
        classifier_model: "m".to_string(),
        classifier_provider: crate::config::Provider::Ollama,
        openai_compatible: None,
        ollama_host: "http://localhost:11434".to_string(),
        ollama_think: None,
        lm_studio_host: "http://localhost:1234".to_string(),
        num_predict: 100,
        max_iterations,
        recovery_plan_auto_runs: 0,
        chat_timeout_secs: 30,
        chat_timeout_source: "override:test".to_string(),
        field_sources: crate::config::ConfigFieldSources::default(),
        chat_retries: 1,
        stream: false,
        resume: None,
        fresh_session: false,
        no_footer: false,
        narration: crate::config::NarrationMode::Normal,
        profile: "generic".to_string(),
        profile_explicit: true,
        profile_inference: None,
        style: "default".to_string(),
        action: crate::config::Action::Repl,
    }
}

fn tool_call(index: usize, call: &Value) -> ToolCall {
    let (name, arguments) = if let Some(write) = call.get("write") {
        ("Write", json!({"path": write[0], "content": write[1]}))
    } else if let Some(command) = call.get("bash") {
        ("Bash", json!({"command": command}))
    } else if let Some(path) = call.get("read") {
        ("Read", json!({"path": path}))
    } else {
        panic!("unknown script call {call}");
    };
    ToolCall {
        id: format!("call-{index}"),
        name: name.to_string(),
        arguments,
    }
}

fn replies(script: &[Value]) -> Vec<AssistantReply> {
    let mut next_id = 0usize;
    script
        .iter()
        .map(|step| AssistantReply {
            content: String::new(),
            tool_calls: step
                .as_array()
                .unwrap()
                .iter()
                .map(|call| {
                    next_id += 1;
                    tool_call(next_id, call)
                })
                .collect(),
            prompt_tokens: None,
            completion_tokens: None,
        })
        .collect()
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|item| item.as_str().unwrap().to_string())
                .collect()
        })
        .unwrap_or_default()
}

struct Trace {
    root: PathBuf,
    _dir: tempfile::TempDir,
    error: Option<String>,
    requests: Vec<Vec<ConversationMessage>>,
    remaining_replies: usize,
    lines: Vec<String>,
}

impl Trace {
    fn normalize(&self, text: &str) -> String {
        let mut text = text.to_string();
        if let Ok(canonical) = self.root.canonicalize() {
            text = text.replace(&canonical.display().to_string(), "<ROOT>");
        }
        text = text.replace(&self.root.display().to_string(), "<ROOT>");
        for (pattern, replacement) in [
            (r"elapsed_ms: [0-9]+", "elapsed_ms: <N>"),
            (r#""(duration_ms|elapsed_ms)":[0-9]+"#, "\"$1\":\"<N>\""),
            (r"[0-9]+ ?ms", "<N> ms"),
        ] {
            text = regex::Regex::new(pattern)
                .unwrap()
                .replace_all(&text, replacement)
                .into_owned();
        }
        text
    }

    fn request_digests(&self) -> Vec<String> {
        self.requests
            .iter()
            .map(|messages| {
                let text = self.normalize(&serde_json::to_string(messages).unwrap());
                Sha256::digest(text.as_bytes())
                    .iter()
                    .take(8)
                    .map(|byte| format!("{byte:02x}"))
                    .collect()
            })
            .collect()
    }

    fn event_names(&self) -> Vec<String> {
        self.lines
            .iter()
            .map(|line| {
                serde_json::from_str::<Value>(line).unwrap()["event"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string()
            })
            .collect()
    }

    fn loop_stop_lines(&self) -> Vec<String> {
        self.lines
            .iter()
            .filter(|line| serde_json::from_str::<Value>(line).unwrap()["event"] == "loop_stop")
            .map(|line| self.normalize(line))
            .collect()
    }
}

fn run(row: &Value) -> Trace {
    let dir = tempfile::tempdir().unwrap();
    for seed in row["seed"].as_array().into_iter().flatten() {
        std::fs::write(
            dir.path().join(seed[0].as_str().unwrap()),
            seed[1].as_str().unwrap(),
        )
        .unwrap();
    }
    let step_kind = match row["step_kind"].as_str().unwrap() {
        "inspect" => RunSessionStepKind::Inspect,
        "implement" => RunSessionStepKind::Implement,
        "verify" => RunSessionStepKind::Verify,
        other => panic!("unknown step kind {other}"),
    };
    let mut options = RunSessionOptions::plan_step(step_kind);
    if let Some(cap) = row["wall_clock_cap_ms"].as_u64() {
        options.step_wall_clock_cap = Some(Duration::from_millis(cap));
    }
    let mut client = Scripted {
        replies: Arc::new(Mutex::new(replies(row["script"].as_array().unwrap()))),
        requests: Arc::new(Mutex::new(Vec::new())),
        delay: Duration::from_millis(row["delay_ms"].as_u64().unwrap_or(0)),
    };
    let max_iterations = row["max_iterations"].as_u64().unwrap() as usize;
    let result = run_session_with_outcome_with_options(
        &mut client,
        &mut SessionSnapshot::new(),
        PROMPT,
        &strings(&row["required_paths"]),
        &config(dir.path(), max_iterations),
        &NOOP_UI,
        options,
    );
    let lines = std::fs::read_to_string(dir.path().join("events.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect();
    let requests = client.requests.lock().unwrap().clone();
    let remaining_replies = client.replies.lock().unwrap().len();
    let mut trace = Trace {
        root: dir.path().to_path_buf(),
        _dir: dir,
        error: None,
        requests,
        remaining_replies,
        lines,
    };
    trace.error = result
        .err()
        .map(|error| trace.normalize(&format!("{error:#}")));
    trace
}

fn plan_rows() -> Vec<Value> {
    FIXTURE
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|row| row["entry"] == "plan_step")
        .collect()
}

fn plan_row(id: &str) -> Value {
    plan_rows()
        .into_iter()
        .find(|row| row["id"] == id)
        .unwrap_or_else(|| panic!("missing fixture row {id}"))
}

/// Error text, requests, event sequence, and remaining replies as recorded
/// from the base commit 20b549fc; returns the single `loop_stop` object.
fn run_against_base(row: &Value) -> (Trace, Map<String, Value>) {
    let id = row["id"].as_str().unwrap();
    let expect = &row["expect"];
    assert_eq!(expect["source"], "20b549fc", "{id}");
    let trace = run(row);
    assert_eq!(trace.error.as_deref(), expect["error"].as_str(), "{id}");
    assert_eq!(trace.remaining_replies, 0, "{id}");
    assert_eq!(
        json!(trace.request_digests()),
        expect["request_digests"],
        "{id}"
    );
    assert_eq!(json!(trace.event_names()), expect["event_names"], "{id}");
    let stops = trace.loop_stop_lines();
    assert_eq!(stops.len(), 1, "{id}: {stops:?}");
    let Value::Object(object) = serde_json::from_str::<Value>(&stops[0]).unwrap() else {
        panic!("{id}: loop_stop is not an object");
    };
    assert_eq!(object["reason"], expect["reason"], "{id}");
    (trace, object)
}

fn budget_fields_against_base(id: &str) -> Map<String, Value> {
    let row = plan_row(id);
    let (_trace, mut object) = run_against_base(&row);
    let expected = row["expect"]["budget_fields"].as_object().unwrap().clone();
    for field in FIELDS {
        let actual = object
            .remove(field)
            .unwrap_or_else(|| panic!("{id}: loop_stop lacks {field}"));
        assert_eq!(actual, expected[field], "{id}: {field}");
    }
    assert_eq!(
        serde_json::to_string(&object).unwrap(),
        row["expect"]["base_loop_stop"].as_str().unwrap(),
        "{id}: the other fields are the base bytes"
    );
    expected
}

#[test]
fn an_inspect_step_rejection_of_write_sets_the_failure_flag() {
    let fields = budget_fields_against_base("plan-inspect-write-rejected");
    assert_eq!(fields["iteration_limit"], 4);
    assert_eq!(fields["successful_write_count"], 0);
    assert_eq!(fields["bash_results_after_last_write"], 3);
    assert_eq!(fields["bash_failures_after_last_write"], 1);
    assert_eq!(fields["write_failure_after_last_write"], true);
}

#[test]
fn a_policy_substituted_bash_is_not_counted() {
    let fields = budget_fields_against_base("plan-verify-policy-substituted");
    assert_eq!(fields["iteration_limit"], 4);
    assert_eq!(fields["bash_results_after_last_write"], 2);
    assert_eq!(fields["bash_failures_after_last_write"], 1);
    assert_eq!(fields["last_bash_after_last_write_succeeded"], true);
}

#[test]
fn plan_step_stop_sites_keep_their_base_bytes_without_the_fields() {
    for id in [
        "other-site-plan-after-tool",
        "other-site-plan-step-observation",
        "other-site-plan-wall-clock",
    ] {
        let row = plan_row(id);
        assert!(row["expect"]["budget_fields"].is_null(), "{id}");
        let (trace, object) = run_against_base(&row);
        for field in FIELDS {
            assert!(!object.contains_key(field), "{id}: {field} leaked");
        }
        assert_eq!(
            trace.loop_stop_lines()[0],
            row["expect"]["base_loop_stop"].as_str().unwrap(),
            "{id}: bytes as at base"
        );
    }
}
