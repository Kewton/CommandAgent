use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use commandagent::config::{
    Action, Config, ConfigFieldSources, NarrationMode, OllamaThink, OpenAiApi, PlanPreset,
    PromptLayout, Provider,
};
use commandagent::planner::runner::run_step_plan;
use commandagent::planner::step_plan::{PlanStep, StepPlan};
use commandagent::planner::verify::verify_step;
use commandagent::providers::{AssistantReply, ChatClient};
use commandagent::state::{ConversationMessage, ToolCall};
use commandagent::tools::registry::ToolSpec;

#[derive(Clone)]
struct CountingClient(Arc<AtomicUsize>);

impl ChatClient for CountingClient {
    fn label(&self) -> &str {
        "counting"
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn chat(
        &mut self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        if self.0.fetch_add(1, Ordering::SeqCst) > 0 {
            anyhow::bail!("unexpected repair model call");
        }
        Ok(AssistantReply {
            content: String::new(),
            tool_calls: vec![ToolCall::new(
                "Write",
                serde_json::json!({"path": "artifact.txt", "content": "ok"}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        })
    }
}

#[test]
fn exit_127_verify_failure_makes_zero_repair_model_calls() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    std::fs::write(dir.path().join("verify-127.sh"), "#!/bin/sh\nexit 127\n").unwrap();
    let plan = StepPlan {
        goal: "Create artifact.txt".to_string(),
        steps: vec![step("artifact", vec!["artifact.txt"], "sh verify-127.sh")],
    };
    let calls = Arc::new(AtomicUsize::new(0));
    let mut client = CountingClient(calls.clone());

    let err = run_step_plan(&mut client, &plan, &config(dir.path(), &events))
        .unwrap_err()
        .to_string();

    assert!(
        err.contains("deterministic_environment_error:exit_127"),
        "{err}"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1, "repair model was called");
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("\"event\":\"repair_unreachable\""));
    assert!(event_text.contains("\"repair_target\":\"verifier_command\""));
}

#[test]
fn ordinary_test_failure_remains_an_artifact_failure() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("failing-test.sh"),
        "#!/bin/sh\necho 'test assertion failed' >&2\nexit 1\n",
    )
    .unwrap();

    let report = verify_step(dir.path(), &step("test", Vec::new(), "sh failing-test.sh"));

    assert_eq!(report.command_failures.len(), 1, "{report:?}");
    assert!(
        report.verifier_command_false_negatives.is_empty(),
        "{report:?}"
    );
}

#[test]
#[cfg(unix)]
fn permission_and_missing_interpreter_failures_are_not_artifact_failures() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let denied = dir.path().join("permission-denied.sh");
    std::fs::write(&denied, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&denied, std::fs::Permissions::from_mode(0o644)).unwrap();
    let missing_interpreter = dir.path().join("missing-interpreter.sh");
    std::fs::write(&missing_interpreter, "#!/issue-236-missing-interpreter\n").unwrap();
    std::fs::set_permissions(&missing_interpreter, std::fs::Permissions::from_mode(0o755)).unwrap();

    for command in ["./permission-denied.sh", "./missing-interpreter.sh"] {
        let report = verify_step(dir.path(), &step("environment", Vec::new(), command));
        assert!(report.command_failures.is_empty(), "{command}: {report:?}");
        assert_eq!(
            report.verifier_command_false_negatives.len(),
            1,
            "{command}: {report:?}"
        );
    }
}

fn step(id: &str, expected_paths: Vec<&str>, verify: &str) -> PlanStep {
    PlanStep {
        id: id.to_string(),
        kind: "implement".to_string(),
        expected_result: "pass".to_string(),
        instruction: format!("Run {id}"),
        expected_paths: expected_paths.into_iter().map(str::to_string).collect(),
        verify: vec![verify.to_string()],
    }
}

fn config(root: &Path, events: &Path) -> Config {
    Config {
        workspace_root: root.to_path_buf(),
        state_dir: PathBuf::from("state"),
        eval_events_path: Some(events.to_path_buf()),
        completion_contract_path: None,
        yes: true,
        offline: false,
        context_budget: 1000,
        model: "test".to_string(),
        provider: Provider::Ollama,
        tool_protocol: None,
        openai_api: OpenAiApi::ChatCompletions,
        prompt_layout: PromptLayout::Stable,
        plan_preset: PlanPreset::None,
        intent_override: None,
        planner_model: "test".to_string(),
        planner_provider: Provider::Ollama,
        planner_think: Some(OllamaThink::False),
        classifier_model: "test".to_string(),
        classifier_provider: Provider::Ollama,
        ollama_host: "http://localhost:11434".to_string(),
        ollama_think: None,
        lm_studio_host: "http://localhost:1234".to_string(),
        num_predict: 100,
        max_iterations: 2,
        chat_timeout_secs: 1,
        chat_timeout_source: "override:test".to_string(),
        field_sources: ConfigFieldSources::default(),
        chat_retries: 0,
        stream: false,
        resume: None,
        fresh_session: false,
        no_footer: true,
        narration: NarrationMode::Normal,
        profile: "generic".to_string(),
        profile_explicit: true,
        profile_inference: None,
        style: "default".to_string(),
        action: Action::Repl,
    }
}
