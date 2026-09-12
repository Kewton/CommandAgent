//! Production Runner replay of an Inspect -> Implement repair plan. This does
//! not simulate automatic-recovery admission or claim live-model success.
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use clap::Parser;
use commandagent::cli::Cli;
use commandagent::config::Config;
use commandagent::planner::runner::run_step_plan;
use commandagent::planner::step_plan::{PlanStep, StepPlan};
use commandagent::providers::{AssistantReply, ChatClient};
use commandagent::state::{ConversationMessage, ToolCall};
use commandagent::tools::registry::ToolSpec;
use serde_json::{Value, json};

const TARGET: &str = "src/lib/persistence.cjs";
const SOURCE: &str = include_str!("corpus/apps/missing-read-correction/implementation.cjs");

#[derive(Clone)]
struct InspectionReplay {
    root: PathBuf,
    turn: Arc<Mutex<usize>>,
    frames: Arc<Mutex<Vec<Vec<ConversationMessage>>>>,
    attempt_inspect_write: bool,
    implementation: &'static str,
}

impl ChatClient for InspectionReplay {
    fn label(&self) -> &str {
        "missing-read-inspection-runner-replay"
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
        messages: &[ConversationMessage],
        _: &[ToolSpec],
        _: bool,
    ) -> anyhow::Result<AssistantReply> {
        self.frames.lock().unwrap().push(messages.to_vec());
        let mut turn = self.turn.lock().unwrap();
        let current = *turn;
        *turn += 1;
        let tool_calls = match current {
            0 => {
                let mut calls = vec![
                    ToolCall::new("Read", json!({"path":"ready.txt"})),
                    ToolCall::new("Read", json!({"path":TARGET})),
                    ToolCall::new("Read", json!({"path":"src/api/not-created.cjs"})),
                    ToolCall::new("Read", json!({"path":"verify.cjs"})),
                ];
                if self.attempt_inspect_write {
                    calls.push(ToolCall::new(
                        "Write",
                        json!({"path":TARGET, "content":"forbidden inspection write"}),
                    ));
                }
                calls
            }
            1 => {
                assert!(
                    !self.root.join(TARGET).exists(),
                    "Inspect must not create the target"
                );
                let feedback: Vec<_> = messages
                    .iter()
                    .filter(|message| message.role == "tool")
                    .collect();
                assert!(
                    feedback
                        .iter()
                        .any(|message| message.content.contains("read_path_missing")
                            && message.content.contains(TARGET)),
                    "{messages:?}"
                );
                assert!(
                    feedback
                        .iter()
                        .any(|message| message.content.contains("read_path_missing")
                            && message.content.contains("src/api/not-created.cjs")),
                    "{messages:?}"
                );
                return Ok(AssistantReply::text(
                    "INSPECT_ABSENT_PERSISTENCE: src/lib/persistence.cjs is absent. Implement the original persistence requirement and preserve node verify.cjs.",
                ));
            }
            2 => vec![ToolCall::new("Read", json!({"path":TARGET}))],
            3 => vec![ToolCall::new(
                "Write",
                json!({"path":TARGET, "content":self.implementation}),
            )],
            _ => return Ok(AssistantReply::text("done")),
        };
        Ok(AssistantReply {
            content: String::new(),
            tool_calls,
            prompt_tokens: None,
            completion_tokens: None,
        })
    }
}

fn run_inspection(attempt_write: bool, implementation: &'static str) {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("ready.txt"), "original input").unwrap();
    let verifier = include_str!("corpus/apps/missing-read-correction/verify.cjs");
    std::fs::write(root.path().join("verify.cjs"), verifier).unwrap();
    let contract = root.path().join("completion-contract.json");
    let original = serde_json::to_vec(&json!({"required_paths":[TARGET, "verify.cjs"], "verify_commands":["node verify.cjs"], "protected_paths":["verify.cjs"], "verify_repair_cap":1})).unwrap();
    std::fs::write(&contract, &original).unwrap();
    let mut config = Config::from_cli(Cli::parse_from([
        "commandagent",
        "--yes",
        "--cwd",
        root.path().to_str().unwrap(),
        "--prompt",
        "Create the persistence module required by the registered check",
    ]))
    .unwrap();
    config.profile = "generic".into();
    config.profile_explicit = true;
    config.completion_contract_path = Some(contract.clone());
    config.eval_events_path = Some(root.path().join("events.jsonl"));
    config.max_iterations = 8;
    let plan = StepPlan {
        goal: "Create the persistence module required by the registered check".into(),
        steps: vec![
            PlanStep {
                id: "inspect-recovery-state".into(),
                kind: "inspect".into(),
                expected_result: "pass".into(),
                instruction: "Inspect existing and missing files; report absence without mutation"
                    .into(),
                expected_paths: vec![],
                verify: vec![],
            },
            PlanStep {
                id: "implement-persistence".into(),
                kind: "implement".into(),
                expected_result: "pass".into(),
                instruction:
                    "Create src/lib/persistence.cjs with loadProjects returning an empty array"
                        .into(),
                expected_paths: vec![TARGET.into()],
                verify: vec!["node verify.cjs".into()],
            },
        ],
    };
    let mut client = InspectionReplay {
        root: root.path().into(),
        turn: Default::default(),
        frames: Default::default(),
        attempt_inspect_write: attempt_write,
        implementation,
    };
    let result = run_step_plan(&mut client, &plan, &config);
    let text = std::fs::read_to_string(config.eval_events_path.unwrap()).unwrap();
    let events: Vec<Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(
        result.is_ok(),
        implementation == SOURCE,
        "{result:?}\n{text}"
    );
    let frames = client.frames.lock().unwrap();
    assert!(frames.len() >= 4, "{result:?}\n{text}");
    assert!(
        frames[2]
            .iter()
            .any(|message| message.content.contains("INSPECT_ABSENT_PERSISTENCE")),
        "{:#?}",
        frames[2]
    );
    assert_eq!(std::fs::read(contract).unwrap(), original);
    assert_eq!(
        std::fs::read_to_string(root.path().join("verify.cjs")).unwrap(),
        verifier
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join("ready.txt")).unwrap(),
        "original input"
    );
    assert_eq!(
        events
            .iter()
            .any(|event| event["event"] == "inspect_mutation_tool_rejected"),
        attempt_write
    );
    assert_eq!(
        events
            .iter()
            .any(|event| event["event"] == "completion_verify" && event["ok"] == true),
        implementation == SOURCE
    );
}

#[test]
fn inspection_passes_absence_to_implementation_and_registered_verification() {
    run_inspection(false, SOURCE);
}

#[test]
fn inspection_still_rejects_write_after_missing_feedback() {
    run_inspection(true, SOURCE);
}

#[test]
fn inspection_does_not_let_wrong_implementation_pass_original_verification() {
    run_inspection(
        false,
        "module.exports = { loadProjects: () => ['wrong'] };\n",
    );
}
