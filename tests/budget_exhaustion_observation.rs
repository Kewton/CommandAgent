//! Issue #959: the iteration-budget `loop_stop` carries nine additive
//! observation fields. Scenarios live in the synthetic corpus fixture
//! `tests/corpus/apps/budget-exhaustion-observation/fixtures/events.jsonl` and
//! run through the public minimal loop with a scripted `ChatClient`.
//!
//! Every row pins the behavior of the base commit 20b549fc: the stop reason,
//! the error text, the event-name sequence, the request digests, and the
//! `loop_stop` line were recorded from that source before the observation
//! existed. Only the nine new fields come from the rules in the Issue body.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use clap::Parser;
use commandagent::cli::Cli;
use commandagent::config::{
    Action, Config, ConfigFieldSources, NarrationMode, OllamaThink, OpenAiApi, PlanPreset,
    PromptLayout, Provider,
};
use commandagent::minimal_loop::loop_run::run_session_with_outcome_with_ui;
use commandagent::providers::{AssistantReply, ChatClient};
use commandagent::state::{ConversationMessage, SessionSnapshot, ToolCall};
use commandagent::tools::registry::ToolSpec;
use commandagent::tui::NOOP_UI;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

const FIXTURE: &str =
    include_str!("corpus/apps/budget-exhaustion-observation/fixtures/events.jsonl");
const PROMPT: &str = "Keep the notes file current and check it with shell commands.";
const NUM_PREDICT: usize = 100;
const BUDGET_FIELDS: [&str; 9] = [
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

#[derive(Clone)]
struct ScriptedClient {
    replies: Arc<Mutex<Vec<AssistantReply>>>,
    requests: Arc<Mutex<Vec<Vec<ConversationMessage>>>>,
}

impl ChatClient for ScriptedClient {
    fn label(&self) -> &str {
        "scripted"
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

/// The iteration budget is parsed from `--max-iterations`, as the binary does.
fn parsed_max_iterations(value: u64) -> usize {
    Cli::parse_from(["commandagent", "--max-iterations", &value.to_string()]).max_iterations
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
        num_predict: NUM_PREDICT,
        max_iterations,
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
        profile_explicit: true,
        profile_inference: None,
        style: "default".to_string(),
        action: Action::Repl,
    }
}

fn tool_call(index: usize, call: &Value) -> ToolCall {
    let (name, arguments) = if let Some(write) = call.get("write") {
        ("Write", json!({"path": write[0], "content": write[1]}))
    } else if let Some(edit) = call.get("edit") {
        (
            "Edit",
            json!({"path": edit[0], "old_string": edit[1], "new_string": edit[2]}),
        )
    } else if let Some(command) = call.get("bash") {
        ("Bash", json!({"command": command}))
    } else if let Some(path) = call.get("read") {
        ("Read", json!({"path": path}))
    } else if let Some(pattern) = call.get("glob") {
        ("Glob", json!({"pattern": pattern}))
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
        .map(|step| {
            if step.get("capped").is_some() {
                return AssistantReply {
                    content: String::new(),
                    tool_calls: Vec::new(),
                    prompt_tokens: None,
                    completion_tokens: Some(NUM_PREDICT as u64),
                };
            }
            let tool_calls = step
                .as_array()
                .unwrap()
                .iter()
                .map(|call| {
                    next_id += 1;
                    tool_call(next_id, call)
                })
                .collect();
            AssistantReply {
                content: String::new(),
                tool_calls,
                prompt_tokens: None,
                completion_tokens: None,
            }
        })
        .collect()
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
        // Wall-clock durations and generated recovery-file ids differ per run.
        for (pattern, replacement) in [
            (r"elapsed_ms: [0-9]+", "elapsed_ms: <N>"),
            (r"total_duration_ms=[0-9]+", "total_duration_ms=<N>"),
            (
                r#""max_length_no_tool_call_duration_ms":[0-9]+"#,
                "\"max_length_no_tool_call_duration_ms\":\"<N>\"",
            ),
            (
                r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}",
                "<ID>",
            ),
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

    fn events(&self) -> Vec<Value> {
        self.lines
            .iter()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    fn event_names(&self) -> Vec<String> {
        self.events()
            .iter()
            .map(|event| event["event"].as_str().unwrap_or_default().to_string())
            .collect()
    }

    /// The single `loop_stop` line exactly as it was written.
    fn loop_stop_line(&self) -> String {
        let stops: Vec<&String> = self
            .lines
            .iter()
            .filter(|line| serde_json::from_str::<Value>(line).unwrap()["event"] == "loop_stop")
            .collect();
        assert_eq!(stops.len(), 1, "{:?}", self.lines);
        self.normalize(stops[0])
    }
}

fn run(row: &Value) -> Trace {
    let dir = tempfile::tempdir().unwrap();
    let max_iterations = parsed_max_iterations(row["max_iterations"].as_u64().unwrap());
    let required_paths: Vec<String> = row["required_paths"]
        .as_array()
        .map(|paths| {
            paths
                .iter()
                .map(|path| path.as_str().unwrap().to_string())
                .collect()
        })
        .unwrap_or_default();
    let mut client = ScriptedClient {
        replies: Arc::new(Mutex::new(replies(row["script"].as_array().unwrap()))),
        requests: Arc::new(Mutex::new(Vec::new())),
    };
    let config = config(dir.path(), max_iterations);
    let result = run_session_with_outcome_with_ui(
        &mut client,
        &mut SessionSnapshot::new(),
        PROMPT,
        &required_paths,
        &config,
        &NOOP_UI,
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

fn rows() -> Vec<Value> {
    FIXTURE
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .inspect(|row| assert_eq!(row["kind"], "synthetic", "{row}"))
        // Plan-step rows need crate-internal options; the leaf tests run them.
        .filter(|row| row.get("entry").is_none())
        .collect()
}

fn row(id: &str) -> Value {
    rows()
        .into_iter()
        .find(|row| row["id"] == id)
        .unwrap_or_else(|| panic!("missing fixture row {id}"))
}

/// The base behavior the row pins: everything except the nine new fields.
fn assert_base_behavior(row: &Value, trace: &Trace) {
    let id = row["id"].as_str().unwrap();
    let expect = &row["expect"];
    assert_eq!(expect["source"], "20b549fc", "{id}");
    assert_eq!(
        trace.error.as_deref(),
        expect["error"].as_str(),
        "{id}: error text"
    );
    assert_eq!(
        trace.remaining_replies, 0,
        "{id}: every scripted reply used"
    );
    assert_eq!(
        json!(trace.request_digests()),
        expect["request_digests"],
        "{id}: requests"
    );
    assert_eq!(
        json!(trace.event_names()),
        expect["event_names"],
        "{id}: event sequence"
    );
}

fn budget_object(row: &Value, trace: &Trace) -> Map<String, Value> {
    let line = trace.loop_stop_line();
    let Value::Object(object) = serde_json::from_str::<Value>(&line).unwrap() else {
        panic!("loop_stop is not an object: {line}");
    };
    assert_eq!(object["reason"], row["expect"]["reason"], "{line}");
    object
}

fn check_budget_row(row: &Value) -> Map<String, Value> {
    let id = row["id"].as_str().unwrap();
    let trace = run(row);
    assert_base_behavior(row, &trace);
    let mut object = budget_object(row, &trace);
    let expected = row["expect"]["budget_fields"].as_object().unwrap();
    assert_eq!(expected.len(), BUDGET_FIELDS.len(), "{id}");
    for field in BUDGET_FIELDS {
        let actual = object
            .remove(field)
            .unwrap_or_else(|| panic!("{id}: loop_stop lacks {field}"));
        assert!(
            actual.is_number() || actual.is_boolean() || actual.is_null(),
            "{id}: {field} must be a number, bool, or null: {actual}"
        );
        assert_eq!(&actual, &expected[field], "{id}: {field}");
    }
    let limit = row["max_iterations"].as_u64().unwrap() + 3;
    assert_eq!(
        expected["iteration_limit"], limit,
        "{id}: limit is --max-iterations + 3"
    );
    assert_eq!(expected["iterations_used"], limit, "{id}");
    assert_eq!(expected["iteration_budget_exhausted"], true, "{id}");
    // With the nine fields removed, the line is byte-for-byte the base line.
    let base = row["expect"]["base_loop_stop"].as_str().unwrap();
    assert_eq!(serde_json::to_string(&object).unwrap(), base, "{id}");
    let roundtrip = serde_json::to_string(&serde_json::from_str::<Value>(base).unwrap()).unwrap();
    assert_eq!(roundtrip, base, "{id}: base line is canonical");
    expected.clone()
}

fn check_other_site_row(row: &Value) {
    let id = row["id"].as_str().unwrap();
    assert!(row["expect"]["budget_fields"].is_null(), "{id}");
    let trace = run(row);
    assert_base_behavior(row, &trace);
    let object = budget_object(row, &trace);
    for field in BUDGET_FIELDS {
        assert!(!object.contains_key(field), "{id}: {field} leaked");
    }
    let base = row["expect"]["base_loop_stop"].as_str().unwrap();
    assert_eq!(trace.loop_stop_line(), base, "{id}: bytes as at base");
}

const BUDGET_ROWS: [&str; 10] = [
    "all-tools-default-budget",
    "max-iterations-four",
    "no-writes",
    "no-bash-after-last-write",
    "write-failure-after-last-write",
    "environment-blocker",
    "same-response-reset",
    "repeated-success",
    "write-rejected",
    "policy-blocked-bash",
];
const OTHER_SITE_ROWS: [&str; 3] = [
    "other-site-post-write-read",
    "other-site-recoverable-repeat",
    "other-site-max-length",
];

#[test]
fn every_public_fixture_row_has_a_test() {
    let mut ids: Vec<String> = rows()
        .iter()
        .map(|row| row["id"].as_str().unwrap().to_string())
        .collect();
    ids.sort();
    let mut named: Vec<String> = BUDGET_ROWS
        .iter()
        .chain(OTHER_SITE_ROWS.iter())
        .map(|id| id.to_string())
        .collect();
    named.sort();
    assert_eq!(ids, named);
}

#[test]
fn fifteen_tool_turns_exhaust_the_default_budget_with_the_observation() {
    let row = row("all-tools-default-budget");
    assert_eq!(row["script"].as_array().unwrap().len(), 15);
    let fields = check_budget_row(&row);
    assert_eq!(fields["iteration_limit"], 15);
    assert_eq!(fields["iterations_used"], 15);
    assert_eq!(fields["successful_write_count"], 7);
    assert_eq!(fields["last_successful_write_iteration"], 13);
    assert_eq!(fields["bash_results_after_last_write"], 2);
    assert_eq!(fields["bash_failures_after_last_write"], 1);
    assert_eq!(fields["last_bash_after_last_write_succeeded"], false);
    assert_eq!(
        row["expect"]["reason"],
        "model_stagnation:no_progress_recorded"
    );
}

#[test]
fn max_iterations_flag_sets_the_limit_instead_of_fifteen() {
    let row = row("max-iterations-four");
    assert_eq!(parsed_max_iterations(4), 4);
    let fields = check_budget_row(&row);
    assert_eq!(fields["iteration_limit"], 7);
    assert_eq!(fields["iterations_used"], 7);
    assert_eq!(fields["successful_write_count"], 2);
    assert_eq!(fields["last_successful_write_iteration"], 3);
    assert_eq!(fields["bash_results_after_last_write"], 4);
    assert_eq!(fields["last_bash_after_last_write_succeeded"], true);
}

#[test]
fn a_run_without_writes_counts_bash_from_the_start() {
    let fields = check_budget_row(&row("no-writes"));
    assert_eq!(fields["successful_write_count"], 0);
    assert_eq!(fields["last_successful_write_iteration"], Value::Null);
    assert_eq!(fields["bash_results_after_last_write"], 5);
    assert_eq!(fields["bash_failures_after_last_write"], 3);
    assert_eq!(fields["write_failure_after_last_write"], false);
}

#[test]
fn no_bash_after_the_last_write_leaves_zero_and_null() {
    let fields = check_budget_row(&row("no-bash-after-last-write"));
    assert_eq!(fields["last_successful_write_iteration"], 2);
    assert_eq!(fields["bash_results_after_last_write"], 0);
    assert_eq!(fields["bash_failures_after_last_write"], 0);
    assert_eq!(fields["last_bash_after_last_write_succeeded"], Value::Null);
}

#[test]
fn an_edit_failure_after_the_last_write_stays_flagged() {
    let fields = check_budget_row(&row("write-failure-after-last-write"));
    assert_eq!(fields["write_failure_after_last_write"], true);
    assert_eq!(fields["bash_results_after_last_write"], 2);
    assert_eq!(fields["last_bash_after_last_write_succeeded"], true);
}

#[test]
fn a_blocker_keeps_loop_progress_exhausted_and_carries_the_fields() {
    let row = row("environment-blocker");
    assert_eq!(row["expect"]["reason"], "loop_progress_exhausted");
    let fields = check_budget_row(&row);
    assert_eq!(fields["bash_results_after_last_write"], 2);
    assert_eq!(fields["bash_failures_after_last_write"], 1);
    assert_eq!(fields["last_bash_after_last_write_succeeded"], false);
}

#[test]
fn a_write_between_two_bash_calls_of_one_reply_resets_in_tool_order() {
    let fields = check_budget_row(&row("same-response-reset"));
    assert_eq!(fields["last_successful_write_iteration"], 4);
    assert_eq!(fields["bash_results_after_last_write"], 1);
    assert_eq!(fields["bash_failures_after_last_write"], 0);
    assert_eq!(fields["last_bash_after_last_write_succeeded"], true);
}

#[test]
fn a_repeated_successful_command_counts_as_success() {
    let fields = check_budget_row(&row("repeated-success"));
    assert_eq!(fields["bash_results_after_last_write"], 3);
    assert_eq!(fields["bash_failures_after_last_write"], 0);
    assert_eq!(fields["last_bash_after_last_write_succeeded"], true);
}

#[test]
fn a_refused_write_sets_the_failure_flag() {
    let fields = check_budget_row(&row("write-rejected"));
    assert_eq!(fields["successful_write_count"], 1);
    assert_eq!(fields["write_failure_after_last_write"], true);
}

#[test]
fn policy_blocked_bash_is_not_counted() {
    let fields = check_budget_row(&row("policy-blocked-bash"));
    assert_eq!(fields["bash_results_after_last_write"], 1);
    assert_eq!(fields["bash_failures_after_last_write"], 0);
    assert_eq!(fields["last_bash_after_last_write_succeeded"], true);
}

#[test]
fn other_stop_sites_keep_their_base_bytes_without_the_fields() {
    for id in OTHER_SITE_ROWS {
        check_other_site_row(&row(id));
    }
}
