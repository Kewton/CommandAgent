use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use clap::Parser;
use commandagent::cli::Cli;
use commandagent::config::Config;
use commandagent::planner::step_plan::{PlanStep, StepPlan};
use commandagent::providers::{AssistantReply, ChatClient};
use commandagent::state::{ConversationMessage, ToolCall};
use commandagent::tools::registry::ToolSpec;

const LIST_HTML: &str = include_str!(
    "../workspace/management/bench/assets/ingest/list/data/snapshots/events-list.html"
);

#[derive(Clone)]
struct CaptureClient {
    replies: Arc<Mutex<VecDeque<AssistantReply>>>,
    messages: Arc<Mutex<Vec<Vec<ConversationMessage>>>>,
}

impl CaptureClient {
    fn new(replies: Vec<AssistantReply>) -> Self {
        Self {
            replies: Arc::new(Mutex::new(replies.into())),
            messages: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn prompt_text(&self) -> String {
        self.messages
            .lock()
            .unwrap()
            .iter()
            .flatten()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl ChatClient for CaptureClient {
    fn label(&self) -> &str {
        "capture"
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
        self.messages.lock().unwrap().push(messages.to_vec());
        self.replies
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("capture replies exhausted"))
    }
}

#[test]
fn production_steps_inject_structure_then_frozen_canonical_candidate_ids() {
    let root = tempfile::tempdir().unwrap();
    let snapshots = root.path().join("data/snapshots");
    std::fs::create_dir_all(&snapshots).unwrap();
    std::fs::write(snapshots.join("events-list.html"), LIST_HTML).unwrap();
    let events = root.path().join("events.jsonl");
    let cwd = root.path().to_string_lossy().to_string();
    let mut config = Config::from_cli(Cli::parse_from([
        "commandagent",
        "--yes",
        "--cwd",
        &cwd,
        "--intent",
        "create",
        "--profile",
        "ingest",
        "--ultra-plan",
        "extract events",
    ]))
    .unwrap();
    config.eval_events_path = Some(events.clone());
    let plan = StepPlan {
        goal: "extract events".to_string(),
        steps: vec![
            PlanStep {
                id: "declare-ingest-inspection".to_string(),
                kind: "implement".to_string(),
                expected_result: "pass".to_string(),
                instruction: "Declare the provisional ingest inspection.".to_string(),
                expected_paths: vec!["output/inspection.json".to_string()],
                verify: Vec::new(),
            },
            PlanStep {
                id: "implement-ingest-delivery".to_string(),
                kind: "implement".to_string(),
                expected_result: "pass".to_string(),
                instruction: "Implement the offline ingest delivery.".to_string(),
                expected_paths: vec!["pipeline/main.py".to_string()],
                verify: Vec::new(),
            },
        ],
    };
    let mut client = CaptureClient::new(vec![
        AssistantReply {
            content: String::new(),
            tool_calls: vec![ToolCall::new(
                "Write",
                serde_json::json!({
                    "path":"output/inspection.json",
                    "content":serde_json::json!({
                        "candidate_selector":{"kind":"css","value":"article.event"},
                        "candidate_accounting":{"accepted":[],"excluded":[]},
                        "record_format":{"fields":[
                            {"name":"name","type":"string","normalizations":["identity"]}
                        ]}
                    }).to_string()
                }),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        },
        AssistantReply::text("The provisional inspection is complete."),
        AssistantReply {
            content: String::new(),
            tool_calls: vec![
                ToolCall::new(
                    "Write",
                    serde_json::json!({"path":"pipeline/main.py","content":"pass\n"}),
                ),
                ToolCall::new(
                    "Write",
                    serde_json::json!({
                        "path":"output/inspection.json",
                        "content":serde_json::json!({
                            "candidate_selector":{"kind":"css","value":"article.event"},
                            "candidate_accounting":{
                                "accepted":[],
                                "excluded":[
                                    {"candidate_id":"data/snapshots/events-list.html#0","reason":"fixture"},
                                    {"candidate_id":"data/snapshots/events-list.html#1","reason":"fixture"},
                                    {"candidate_id":"data/snapshots/events-list.html#2","reason":"fixture"},
                                    {"candidate_id":"data/snapshots/events-list.html#3","reason":"fixture"},
                                    {"candidate_id":"data/snapshots/events-list.html#4","reason":"fixture"},
                                    {"candidate_id":"data/snapshots/events-list.html#5","reason":"fixture"},
                                    {"candidate_id":"data/snapshots/events-list.html#6","reason":"fixture"},
                                    {"candidate_id":"data/snapshots/events-list.html#7","reason":"fixture"},
                                    {"candidate_id":"data/snapshots/events-list.html#8","reason":"fixture"},
                                    {"candidate_id":"data/snapshots/events-list.html#9","reason":"fixture"}
                                ]
                            },
                            "record_format":{"fields":[
                                {"name":"name","type":"string","normalizations":["identity"]}
                            ]}
                        }).to_string()
                    }),
                ),
            ],
            prompt_tokens: None,
            completion_tokens: None,
        },
        AssistantReply::text("The delivery artifacts are complete."),
    ]);

    let result = commandagent::planner::runner::run_step_plan(&mut client, &plan, &config);
    assert!(result.is_ok(), "production plan failed: {result:?}");

    let prompt = client.prompt_text();
    for marker in [
        "Machine-injected snapshot structure material.",
        "Snapshot file: data/snapshots/events-list.html",
        "L0010 |     <article class=\"event\" id=\"list-01\">",
        "HTML tag=article occurrences=10",
        "セレクタは上記の実在構造から導出すること。",
        "Machine-frozen canonical candidate IDs.",
        "before pipeline implementation or execution",
        "Literal canonical candidate_id example: \"data/snapshots/events-list.html#0\"",
        "- data/snapshots/events-list.html#0",
        "- data/snapshots/events-list.html#9",
        "Do not alter or omit any prefix",
    ] {
        assert!(prompt.contains(marker), "missing prompt marker: {marker}");
    }
    let event_text = std::fs::read_to_string(events).unwrap();
    assert_eq!(
        event_text
            .matches("\"event\":\"ingest_snapshot_structure_injected\"")
            .count(),
        1
    );
    assert!(event_text.contains("\"relative_path\":\"data/snapshots/events-list.html\""));
    assert!(event_text.contains("\"candidate_windows\":2"));
    assert_eq!(
        event_text
            .matches("\"event\":\"ingest_candidate_ids_injected\"")
            .count(),
        1
    );
    assert!(event_text.contains("\"candidate_count\":10"));
    assert!(event_text.contains("\"frozen_before_run\":true"));
    assert!(event_text.contains("\"step_id\":\"implement-ingest-delivery\""));
}
