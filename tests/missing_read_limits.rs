use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use clap::Parser;
use commandagent::cli::Cli;
use commandagent::config::Config;
use commandagent::minimal_loop::loop_run::run_session_with_required_paths;
use commandagent::providers::{AssistantReply, ChatClient};
use commandagent::state::{ConversationMessage, SessionSnapshot, ToolCall};
use commandagent::tools::registry::ToolSpec;
use serde_json::{Value, json};

const TARGET: &str = "src/lib/persistence.cjs";
const SOURCE: &str = include_str!("corpus/apps/missing-read-correction/implementation.cjs");

#[derive(Clone)]
struct Replay {
    batches: Arc<Mutex<VecDeque<Vec<ToolCall>>>>,
}

impl ChatClient for Replay {
    fn label(&self) -> &str {
        "missing-read-bounded-replay"
    }
    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }
    fn supports_native_tools(&self, _: &str) -> bool {
        true
    }
    fn chat(
        &mut self,
        _: &str,
        _: &[ConversationMessage],
        _: &[ToolSpec],
        _: bool,
    ) -> anyhow::Result<AssistantReply> {
        let Some(tool_calls) = self.batches.lock().unwrap().pop_front() else {
            return Ok(AssistantReply::text("done"));
        };
        Ok(AssistantReply {
            content: String::new(),
            tool_calls,
            prompt_tokens: None,
            completion_tokens: None,
        })
    }
}

fn execute(batches: Vec<Vec<ToolCall>>) -> (bool, Vec<Value>, bool) {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("ready.txt"), "existing source").unwrap();
    let contract = root.path().join("completion-contract.json");
    let bytes = serde_json::to_vec(&json!({
        "required_paths": [TARGET, "verify.cjs"], "verify_commands": ["node verify.cjs"],
        "protected_paths": ["verify.cjs"], "verify_repair_cap": 1,
    }))
    .unwrap();
    std::fs::write(&contract, &bytes).unwrap();
    let verifier = include_str!("corpus/apps/missing-read-correction/verify.cjs");
    std::fs::write(root.path().join("verify.cjs"), verifier).unwrap();
    let mut config = Config::from_cli(Cli::parse_from([
        "commandagent",
        "--yes",
        "--cwd",
        root.path().to_str().unwrap(),
        "--prompt",
        "Create the persistence module required by the registered check",
    ]))
    .unwrap();
    config.workspace_root = root.path().into();
    config.profile = "generic".into();
    config.profile_explicit = true;
    config.completion_contract_path = Some(contract.clone());
    config.eval_events_path = Some(root.path().join("events.jsonl"));
    config.max_iterations = 8;
    let mut replay = Replay {
        batches: Arc::new(Mutex::new(batches.into())),
    };
    let result = run_session_with_required_paths(
        &mut replay,
        &mut SessionSnapshot::new(),
        "Create the persistence module required by the registered check",
        &[TARGET.into()],
        &config,
    );
    let events = std::fs::read_to_string(config.eval_events_path.unwrap())
        .unwrap_or_else(|error| panic!("events unavailable: {error}; loop result: {result:?}"))
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(std::fs::read(contract).unwrap(), bytes);
    assert_eq!(
        std::fs::read_to_string(root.path().join("verify.cjs")).unwrap(),
        verifier
    );
    (result.is_ok(), events, root.path().join(TARGET).exists())
}

fn write(content: &str) -> Vec<ToolCall> {
    vec![ToolCall::new(
        "Write",
        json!({"path":TARGET, "content":content}),
    )]
}

#[test]
fn corpus_repetitions_stop_but_distinct_missing_inspection_can_continue() {
    let fixture: Value = serde_json::from_str(include_str!(
        "corpus/apps/missing-read-correction/cases.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let mut batches: Vec<Vec<ToolCall>> = case["batches"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(index, paths)| {
                paths
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|path| {
                        ToolCall::new("Read", json!({"path":path, "start_line": index + 1}))
                    })
                    .collect()
            })
            .collect();
        batches.push(write(SOURCE));
        let (ok, events, exists) = execute(batches);
        let stop = case["stop"].as_bool().unwrap();
        assert_eq!(ok, !stop, "{case}: {events:?}");
        assert_eq!(exists, !stop, "{case}: {events:?}");
        let terminal = events.iter().find(|event| {
            event["event"] == "loop_stop" && event["reason"] == "recoverable_tool_error_repeated"
        });
        assert_eq!(terminal.is_some(), stop, "{case}: {events:?}");
        if let Some(terminal) = terminal {
            assert_eq!(terminal["error_kind"], "read_path_missing");
            assert_eq!(terminal["repeat_count"], fixture["repeat_limit"]);
            assert!(events.iter().any(
                |event| event["event"] == "tool_validation_error" && event["repeat_count"] == 3
            ));
        } else {
            assert!(
                events
                    .iter()
                    .any(|event| event["event"] == "completion_verify" && event["ok"] == true)
            );
        }
    }
}

#[test]
fn wrong_or_empty_content_is_rejected_by_the_original_registered_verifier() {
    for content in ["", "module.exports = { loadProjects: () => ['wrong'] };\n"] {
        let (ok, events, exists) = execute(vec![
            vec![ToolCall::new("Read", json!({"path":TARGET}))],
            write(content),
        ]);
        assert!(!ok, "{events:?}");
        assert!(exists);
        assert!(
            events
                .iter()
                .any(|event| event["event"] == "completion_verify" && event["ok"] == false),
            "{events:?}"
        );
        assert!(
            !events
                .iter()
                .any(|event| event["event"] == "completion_verify" && event["ok"] == true)
        );
    }
}

#[test]
fn missing_read_followed_by_done_is_not_completion() {
    let (ok, events, exists) = execute(vec![vec![ToolCall::new("Read", json!({"path":TARGET}))]]);
    assert!(!ok);
    assert!(!exists);
    assert!(
        !events
            .iter()
            .any(|event| event["event"] == "completion_verify" && event["ok"] == true)
    );
}

#[test]
fn novel_missing_paths_stop_at_the_existing_artifact_recovery_budget() {
    let batches = (0..20)
        .map(|index| {
            vec![ToolCall::new(
                "Read",
                json!({"path":format!("missing-{index}")}),
            )]
        })
        .collect();
    let (ok, events, exists) = execute(batches);
    assert!(!ok);
    assert!(!exists);
    let failures = events
        .iter()
        .filter(|event| {
            event["event"] == "tool_validation_error" && event["error_kind"] == "read_path_missing"
        })
        .count();
    // Completion contracts already add a bounded artifact-recovery allowance
    // beyond max_iterations. This fixture exhausts its three repair attempts.
    assert_eq!(failures, 12, "{events:?}");
    let stop = events
        .iter()
        .find(|event| {
            event["event"] == "loop_stop" && event["reason"] == "artifact_recovery_exhausted"
        })
        .expect("original artifact-recovery limit must stop the run");
    assert_eq!(stop["attempts"], 3);
}
