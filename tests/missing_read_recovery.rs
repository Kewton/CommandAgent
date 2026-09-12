use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

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
struct CorrectMissingRead {
    turns: Arc<AtomicUsize>,
}

impl ChatClient for CorrectMissingRead {
    fn label(&self) -> &str {
        "missing-read-production-loop-replay"
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
        let call = match self.turns.fetch_add(1, Ordering::SeqCst) {
            0 => ToolCall::new("Read", json!({"path": TARGET})),
            1 => {
                let feedback = &messages
                    .iter()
                    .rfind(|message| message.role == "tool")
                    .expect("the model must receive the failed Read result")
                    .content;
                assert!(feedback.contains(TARGET), "{feedback}");
                assert!(
                    ["does not exist", "not found", "missing"]
                        .iter()
                        .any(|term| feedback.to_lowercase().contains(term)),
                    "{feedback}"
                );
                ToolCall::new("Write", json!({"path": TARGET, "content": SOURCE}))
            }
            2 => return Ok(AssistantReply::text("done")),
            _ => anyhow::bail!("unexpected model call after the registered verifier"),
        };
        Ok(AssistantReply {
            content: String::new(),
            tool_calls: vec![call],
            prompt_tokens: None,
            completion_tokens: None,
        })
    }
}

#[test]
fn missing_read_recovers_via_write_and_registered_verification() {
    let root = tempfile::tempdir().unwrap();
    let events = root.path().join("events.jsonl");
    let contract = root.path().join("completion-contract.json");
    std::fs::write(
        root.path().join("verify.cjs"),
        include_str!("corpus/apps/missing-read-correction/verify.cjs"),
    )
    .unwrap();
    let contract_bytes = serde_json::to_vec(&json!({
        "required_paths": [TARGET],
        "verify_commands": ["node verify.cjs"],
        "verify_repair_cap": 2,
    }))
    .unwrap();
    std::fs::write(&contract, &contract_bytes).unwrap();
    let mut config = Config::from_cli(Cli::parse_from([
        "commandagent",
        "--yes",
        "--cwd",
        root.path().to_str().unwrap(),
        "--prompt",
        "Create a persistence module returning an empty project list",
    ]))
    .unwrap();
    config.eval_events_path = Some(events.clone());
    config.completion_contract_path = Some(contract.clone());
    config.max_iterations = 6;
    let turns = Arc::new(AtomicUsize::new(0));
    let mut client = CorrectMissingRead {
        turns: turns.clone(),
    };
    let mut session = SessionSnapshot::new();
    let result = run_session_with_required_paths(
        &mut client,
        &mut session,
        "Create a persistence module returning an empty project list",
        &[TARGET.to_string()],
        &config,
    );

    assert!(result.is_ok(), "{result:?}");
    assert!((2..=3).contains(&turns.load(Ordering::SeqCst)));
    assert_eq!(
        std::fs::read_to_string(root.path().join(TARGET)).unwrap(),
        SOURCE
    );
    assert_eq!(std::fs::read(contract).unwrap(), contract_bytes);
    let events: Vec<Value> = std::fs::read_to_string(events)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let read_failure = events
        .iter()
        .position(|event| event["event"] == "tool_validation_error" && event["name"] == "Read")
        .expect("the missing Read remains a recorded error");
    let write = events
        .iter()
        .position(|event| {
            event["event"] == "tool_execute" && event["name"] == "Write" && event["status"] == "ok"
        })
        .unwrap();
    let verified = events
        .iter()
        .position(|event| event["event"] == "completion_verify" && event["ok"] == true)
        .expect("the registered verifier must pass after creation");
    assert!(read_failure < write && write < verified);
}
