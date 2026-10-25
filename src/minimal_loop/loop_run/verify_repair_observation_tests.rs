use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::{defers_tool_observation_completion, is_step_repair_turn};
use crate::config::Config;
use crate::minimal_loop::stagnation_carryover::{EscalationCarryoverHandle, attach_to_options};
use crate::planner::run_step_plan_with_ui;
use crate::planner::step_plan::{PlanStep, StepPlan};
use crate::providers::{AssistantReply, ChatClient};
use crate::state::{ConversationMessage, SessionSnapshot, ToolCall};
use crate::tools::registry::ToolSpec;
use crate::tui::NOOP_UI;

use super::super::{
    RunSessionOptions, RunSessionStepKind, RunStopReason, run_session_with_outcome_with_options,
};

#[derive(Clone)]
struct Fake {
    replies: Arc<Mutex<Vec<AssistantReply>>>,
}

impl Fake {
    fn new(replies: Vec<AssistantReply>) -> Self {
        Self {
            replies: Arc::new(Mutex::new(replies)),
        }
    }

    fn remaining_replies(&self) -> usize {
        self.replies.lock().unwrap().len()
    }
}

impl ChatClient for Fake {
    fn label(&self) -> &str {
        "fake"
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
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        let mut replies = self.replies.lock().unwrap();
        if replies.is_empty() {
            anyhow::bail!("fake client exhausted");
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

fn read_reply(path: &str) -> AssistantReply {
    tool_reply("Read", json!({"path": path}))
}

fn write_reply(path: &str, content: &str) -> AssistantReply {
    tool_reply("Write", json!({"path": path, "content": content}))
}

fn event_values(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect()
}

fn repair_options() -> RunSessionOptions {
    attach_to_options(
        RunSessionOptions::plan_step(RunSessionStepKind::Verify),
        EscalationCarryoverHandle::new(),
    )
}

fn config(root: PathBuf) -> Config {
    Config {
        workspace_root: root,
        state_dir: PathBuf::from("state"),
        yes: true,
        offline: false,
        context_budget: 1000,
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
        max_iterations: 4,
        recovery_plan_auto_runs: 0,
        chat_timeout_secs: 1,
        chat_timeout_source: "override:test".to_string(),
        field_sources: crate::config::ConfigFieldSources::default(),
        chat_retries: 1,
        stream: false,
        eval_events_path: None,
        completion_contract_path: None,
        resume: None,
        fresh_session: false,
        no_footer: false,
        narration: crate::config::NarrationMode::Normal,
        profile: "generic".to_string(),
        profile_explicit: false,
        profile_inference: None,
        style: "default".to_string(),
        action: crate::config::Action::Repl,
    }
}

fn seed_workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/gameLogic.ts"), "export const a = 1;\n").unwrap();
    dir
}

#[test]
fn helper_defers_only_unchanged_batches_of_repair_turns() {
    let repair = repair_options();
    let first_observation = RunSessionOptions::plan_step(RunSessionStepKind::Verify);
    let changed = vec!["src/a.ts".to_string()];

    assert!(is_step_repair_turn(&repair));
    assert!(!is_step_repair_turn(&first_observation));
    assert!(defers_tool_observation_completion(&repair, false, &[]));
    assert!(!defers_tool_observation_completion(&repair, true, &[]));
    assert!(!defers_tool_observation_completion(
        &repair, false, &changed
    ));
    assert!(!defers_tool_observation_completion(
        &first_observation,
        false,
        &[]
    ));
}

#[test]
fn repair_turn_reads_then_writes_in_second_batch() {
    let dir = seed_workspace();
    let mut fake = Fake::new(vec![
        read_reply("src/gameLogic.ts"),
        write_reply("src/gameLogic.ts", "export const a = 2;\n"),
    ]);
    let mut session = SessionSnapshot::new();

    let outcome = run_session_with_outcome_with_options(
        &mut fake,
        &mut session,
        "Repair step `verify-engine-types`: TS2305 no exported member.",
        &[],
        &config(dir.path().to_path_buf()),
        &NOOP_UI,
        repair_options(),
    )
    .unwrap();

    assert_eq!(fake.remaining_replies(), 0);
    assert_eq!(outcome.stop_reason, RunStopReason::AssistantFinal);
    assert_eq!(outcome.final_text, "step tool observation completed");
    assert_eq!(outcome.tool_calls, 2);
    assert_eq!(outcome.changed_paths, vec!["src/gameLogic.ts".to_string()]);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("src/gameLogic.ts")).unwrap(),
        "export const a = 2;\n"
    );
}

#[test]
fn first_verify_observation_still_completes_after_one_read_batch() {
    let dir = seed_workspace();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events.clone());
    let mut fake = Fake::new(vec![
        read_reply("src/gameLogic.ts"),
        write_reply("src/gameLogic.ts", "export const a = 2;\n"),
    ]);
    let mut session = SessionSnapshot::new();

    let outcome = run_session_with_outcome_with_options(
        &mut fake,
        &mut session,
        "Run the verify command.",
        &[],
        &cfg,
        &NOOP_UI,
        RunSessionOptions::plan_step(RunSessionStepKind::Verify),
    )
    .unwrap();

    assert_eq!(fake.remaining_replies(), 1);
    assert_eq!(outcome.stop_reason, RunStopReason::AssistantFinal);
    assert_eq!(outcome.tool_calls, 1);
    assert!(outcome.changed_paths.is_empty());
    assert!(event_values(&events).iter().any(|event| {
        event["event"] == "loop_stop" && event["reason"] == "step_tool_observation_completed"
    }));
}

#[test]
fn repair_turn_read_only_batches_never_complete_the_session() {
    let dir = seed_workspace();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events.clone());
    cfg.max_iterations = 3;
    let mut fake = Fake::new(vec![
        read_reply("src/gameLogic.ts"),
        read_reply("src/gameLogic.ts"),
        read_reply("src/gameLogic.ts"),
        read_reply("src/gameLogic.ts"),
    ]);
    let mut session = SessionSnapshot::new();

    let result = run_session_with_outcome_with_options(
        &mut fake,
        &mut session,
        "Repair step `verify-engine-types`: TS2305 no exported member.",
        &[],
        &cfg,
        &NOOP_UI,
        repair_options(),
    );

    let err = result
        .expect_err("read-only repair batches must not complete")
        .to_string();
    assert!(err.contains("model_stagnation:read_only_loop"), "{err}");
    assert!(fake.remaining_replies() < 3, "loop stopped after one batch");
    assert!(!event_values(&events).iter().any(|event| {
        event["event"] == "loop_stop" && event["reason"] == "step_tool_observation_completed"
    }));
}

fn verify_step_plan() -> StepPlan {
    StepPlan {
        goal: "Create the page\n\nRequired final artifacts:\n- src/app/page.tsx".to_string(),
        steps: vec![PlanStep {
            id: "entrypoint".to_string(),
            kind: "verify".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Verify and repair src/app/page.tsx if missing".to_string(),
            expected_paths: Vec::new(),
            verify: vec!["test -f src/app/page.tsx".to_string()],
        }],
    }
}

#[test]
fn verify_failure_repair_reads_then_writes_and_reverify_passes() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("notes.md"), "page lives in src/app\n").unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events.clone());
    cfg.max_iterations = 8;
    let mut fake = Fake::new(vec![
        tool_reply("Bash", json!({"command": "true"})),
        read_reply("notes.md"),
        write_reply(
            "src/app/page.tsx",
            "export default function Page(){return <main>ok</main>;}",
        ),
    ]);

    let result = run_step_plan_with_ui(&mut fake, &verify_step_plan(), &cfg, &NOOP_UI).unwrap();

    assert_eq!(result, "plan-run complete: 1 steps");
    assert!(dir.path().join("src/app/page.tsx").is_file());
    let events = event_values(&events);
    let repairs: Vec<&Value> = events
        .iter()
        .filter(|event| event["event"] == "step_verify_repair")
        .collect();
    assert_eq!(repairs.len(), 1, "repairs: {repairs:#?}");
    assert_eq!(repairs[0]["ok"], true);
    assert!(!events.iter().any(|event| {
        event["failure_kind"] == "verify_repair_no_change"
            || event["reason"] == "verify_repair_no_change"
    }));
}
