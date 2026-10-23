use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use super::super::{
    ContractEnforcement, NOOP_UI, SessionSnapshot, StepPromptContext, StepRunError, StepRunOutcome,
    run_step,
};
use super::{ORACLE_FAILED_KIND, ORACLE_PASSED_KIND, WITHOUT_ORACLE_KIND};
use crate::config::Config;
use crate::planner::step_plan::{PlanStep, StepPlan};
use crate::providers::{AssistantReply, ChatClient};
use crate::state::{ConversationMessage, ToolCall};
use crate::tools::registry::ToolSpec;

fn corpus() -> serde_json::Value {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/corpus/apps/issue838-model-verifier-edit-gate/fixtures/cases.json"
    )))
    .unwrap()
}

fn recorded_cli_q() -> serde_json::Value {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/corpus/apps/issue820-model-verifier-oracle-disagreement/fixtures/cases.json"
    )))
    .unwrap()
}

fn text(value: &serde_json::Value, key: &str) -> String {
    value[key].as_str().unwrap().to_string()
}

fn write_files(root: &Path, files: &serde_json::Value) {
    for (path, content) in files.as_object().unwrap() {
        write_file(root, path, content.as_str().unwrap());
    }
}

fn write_file(root: &Path, path: &str, content: &str) {
    let target = root.join(path);
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    std::fs::write(target, content).unwrap();
}

/// The recorded G1 cli-q workspace without the compatibility shim.
fn cli_workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write_files(dir.path(), &recorded_cli_q()["files"]);
    make_entrypoint_executable(dir.path());
    dir
}

fn make_entrypoint_executable(root: &Path) {
    // The profile's post-step repair makes the entrypoint executable in a real run.
    use std::os::unix::fs::PermissionsExt;
    let entrypoint = root.join("src/csv_stats/main.py");
    std::fs::set_permissions(entrypoint, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn config(root: &Path, profile: &str) -> Config {
    Config {
        workspace_root: root.to_path_buf(),
        state_dir: PathBuf::from("state"),
        eval_events_path: Some(root.join("run/events.jsonl")),
        completion_contract_path: None,
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
        resume: None,
        fresh_session: false,
        no_footer: false,
        narration: crate::config::NarrationMode::Normal,
        profile: profile.to_string(),
        profile_explicit: false,
        profile_inference: None,
        style: "default".to_string(),
        action: crate::config::Action::Repl,
    }
}

fn step(id: &str, kind: &str, paths: &[&str], verify: &[&str]) -> PlanStep {
    PlanStep {
        id: id.to_string(),
        kind: kind.to_string(),
        expected_result: "pass".to_string(),
        instruction: format!("{id} from the recorded verification phase"),
        expected_paths: paths.iter().map(|path| path.to_string()).collect(),
        verify: verify.iter().map(|command| command.to_string()).collect(),
    }
}

/// An earlier Implement step creates `verifier_path`; the Verify step runs it.
fn plan(goal: &str, producer: &str, verifier_step: &str, path: &str, command: &str) -> StepPlan {
    StepPlan {
        goal: goal.to_string(),
        steps: vec![
            step(producer, "implement", &[path], &[]),
            step(verifier_step, "verify", &[], &[command]),
        ],
    }
}

#[derive(Clone)]
struct ScriptedClient {
    replies: Arc<Mutex<Vec<AssistantReply>>>,
    prompts: Arc<Mutex<Vec<String>>>,
}

impl ChatClient for ScriptedClient {
    fn label(&self) -> &str {
        "scripted"
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
        if let Some(last) = messages.last() {
            self.prompts.lock().unwrap().push(last.content.clone());
        }
        let mut replies = self.replies.lock().unwrap();
        if replies.is_empty() {
            anyhow::bail!("scripted client exhausted")
        }
        Ok(replies.remove(0))
    }
}

fn tool_turn(calls: Vec<ToolCall>, done: &str) -> [AssistantReply; 2] {
    [
        AssistantReply {
            content: String::new(),
            tool_calls: calls,
            prompt_tokens: None,
            completion_tokens: None,
        },
        AssistantReply::text(done.to_string()),
    ]
}

fn write_call(path: &str, content: &str) -> ToolCall {
    ToolCall::new(
        "Write",
        serde_json::json!({"path": path, "content": content}),
    )
}

struct Run {
    result: Result<StepRunOutcome, StepRunError>,
    events: String,
    prompts: String,
}

/// Runs the last (Verify) step of `plan`; the initial turn changes nothing and
/// each element of `repairs` is the tool calls of one repair turn.
fn run_verify_step(
    dir: &tempfile::TempDir,
    cfg: &Config,
    plan: &StepPlan,
    goal: &str,
    repairs: Vec<Vec<ToolCall>>,
) -> Run {
    let noop = vec![ToolCall::new(
        "Bash",
        serde_json::json!({"command": "true"}),
    )];
    let mut replies: Vec<AssistantReply> = tool_turn(noop, "initial done").into();
    for calls in repairs {
        replies.extend(tool_turn(calls, "repair done"));
    }
    let prompts = Arc::new(Mutex::new(Vec::new()));
    let mut client = ScriptedClient {
        replies: Arc::new(Mutex::new(replies)),
        prompts: prompts.clone(),
    };
    let context = StepPromptContext {
        overall_goal: goal.to_string(),
        ..StepPromptContext::default()
    };
    let mut session = SessionSnapshot::new();
    let result = run_step(
        &mut client,
        &mut session,
        plan,
        plan.steps.last().unwrap(),
        &context,
        cfg,
        &NOOP_UI,
        "test",
        ContractEnforcement::Enforce,
        None,
        None,
    );
    let events = std::fs::read_to_string(dir.path().join("run/events.jsonl")).unwrap_or_default();
    let prompts = prompts.lock().unwrap().join("\n");
    Run {
        result,
        events,
        prompts,
    }
}

fn cli_goal() -> String {
    text(&recorded_cli_q(), "goal")
}

fn cli_plan(verifier_path: &str) -> StepPlan {
    let cli = &corpus()["cli_q"];
    plan(
        &cli_goal(),
        &text(cli, "producer_step"),
        &text(cli, "verifier_step"),
        verifier_path,
        &format!("python3 {verifier_path}"),
    )
}

fn cli_verifier_rewrite() -> Vec<ToolCall> {
    let cli = &corpus()["cli_q"];
    vec![write_call(
        &text(cli, "verifier_path"),
        &text(cli, "fixed_verifier"),
    )]
}

fn ambig_workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write_files(dir.path(), &corpus()["ambig_g"]["files"]);
    dir
}

fn ambig_plan(producer_path: &str) -> StepPlan {
    let ambig = &corpus()["ambig_g"];
    plan(
        &text(ambig, "goal"),
        &text(ambig, "producer_step"),
        &text(ambig, "verifier_step"),
        producer_path,
        &text(ambig, "verifier_command"),
    )
}

fn ambig_rewrite() -> Vec<ToolCall> {
    let ambig = &corpus()["ambig_g"];
    vec![write_call(
        &text(ambig, "verifier_path"),
        &text(ambig, "always_pass_verifier"),
    )]
}

fn failure_kind(run: &Run) -> String {
    let error = run.result.as_ref().expect_err("the step must fail");
    error.outcome.stop_reason.clone().unwrap_or_default()
}

fn event_with(run: &Run, kind: &str) -> serde_json::Value {
    run.events
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|event| event["failure_kind"] == kind && event["event"] == "step_verify_repair")
        .unwrap_or_else(|| panic!("no step_verify_repair event with {kind}: {}", run.events))
}

#[test]
fn python_cli_verifier_rewrite_passes_when_the_host_oracle_passes() {
    let dir = cli_workspace();
    let cfg = config(dir.path(), "python-cli");
    let run = run_verify_step(
        &dir,
        &cfg,
        &cli_plan("verify_output.py"),
        &cli_goal(),
        vec![cli_verifier_rewrite()],
    );
    let outcome = run
        .result
        .as_ref()
        .unwrap_or_else(|e| panic!("{}", e.message));
    assert!(
        outcome
            .changed_paths
            .contains(&"verify_output.py".to_string())
    );
    let event = event_with(&run, ORACLE_PASSED_KIND);
    assert_eq!(event["ok"], true);
    assert_eq!(
        event["allowed_action"],
        "edit_model_verifier_or_implementation"
    );
    let reason = event["primary_reason"].as_str().unwrap();
    for expected in [
        "verify_output.py",
        "verify_final=pass",
        "behavior_probe=pass",
    ] {
        assert!(reason.contains(expected), "{expected}: {reason}");
    }
    assert!(
        run.prompts.contains("Verifier authority"),
        "the first repair prompt grants the verifier fix: {}",
        run.prompts
    );
    assert!(run.prompts.contains("`verify_output.py`"));
}

#[test]
fn python_cli_verifier_rewrite_fails_when_the_host_oracle_fails() {
    let dir = cli_workspace();
    let cfg = config(dir.path(), "python-cli");
    let cli = &corpus()["cli_q"];
    let run = run_verify_step(
        &dir,
        &cfg,
        &cli_plan("verify_output.py"),
        &cli_goal(),
        vec![vec![
            write_call("verify_output.py", &text(cli, "always_pass_verifier")),
            write_call("src/csv_stats/main.py", &text(cli, "constant_table_cli")),
        ]],
    );
    assert_eq!(failure_kind(&run), ORACLE_FAILED_KIND);
    let event = event_with(&run, ORACLE_FAILED_KIND);
    assert_eq!(event["ok"], false);
    assert!(run.events.contains("\"event\":\"loop_stop\""));
    let reason = event["primary_reason"].as_str().unwrap();
    assert!(reason.contains("behavior_probe="), "{reason}");
    assert!(!reason.contains("behavior_probe=pass"), "{reason}");
}

#[test]
fn generic_verifier_rewritten_to_always_succeed_fails_without_a_host_oracle() {
    let dir = ambig_workspace();
    let cfg = config(dir.path(), "generic");
    let run = run_verify_step(
        &dir,
        &cfg,
        &ambig_plan("smoke-check.js"),
        &text(&corpus()["ambig_g"], "goal"),
        vec![ambig_rewrite()],
    );
    assert_eq!(failure_kind(&run), WITHOUT_ORACLE_KIND);
    let event = event_with(&run, WITHOUT_ORACLE_KIND);
    assert_eq!(event["ok"], false);
    assert!(
        event["primary_reason"]
            .as_str()
            .unwrap()
            .contains("model_verifier_edited: smoke-check.js")
    );
    assert!(
        !run.prompts.contains("Verifier authority"),
        "no oracle, no authority"
    );
}

#[test]
fn generic_verifier_not_produced_by_an_earlier_step_is_not_gated() {
    let dir = ambig_workspace();
    let cfg = config(dir.path(), "generic");
    let run = run_verify_step(
        &dir,
        &cfg,
        &ambig_plan("notes.md"),
        &text(&corpus()["ambig_g"], "goal"),
        vec![ambig_rewrite()],
    );
    assert!(
        run.result.is_ok(),
        "{:?}",
        run.result.err().map(|e| e.message)
    );
    assert!(!run.events.contains("model_verifier_edit"));
}

fn contract_config(dir: &tempfile::TempDir, contract: serde_json::Value) -> Config {
    let path = dir.path().join("contract.json");
    std::fs::write(&path, contract.to_string()).unwrap();
    let mut cfg = config(dir.path(), "python-cli");
    cfg.completion_contract_path = Some(path);
    cfg
}

#[test]
fn deterministic_oracle_script_rewrite_gets_no_authority_and_fails() {
    let dir = cli_workspace();
    let cfg = contract_config(
        &dir,
        serde_json::json!({"deterministic_oracles": ["python3 verify_output.py"]}),
    );
    let run = run_verify_step(
        &dir,
        &cfg,
        &cli_plan("verify_output.py"),
        &cli_goal(),
        vec![cli_verifier_rewrite()],
    );
    assert_eq!(failure_kind(&run), WITHOUT_ORACLE_KIND);
    assert!(!run.prompts.contains("Verifier authority"));
}

#[test]
fn protected_path_script_rewrite_gets_no_authority_and_fails() {
    let dir = cli_workspace();
    write_file(dir.path(), "tools/check.py", "import sys\nsys.exit(1)\n");
    let cfg = contract_config(
        &dir,
        serde_json::json!({"required_paths": ["tools/check.py"], "protected_paths": ["tools"]}),
    );
    let cli = &corpus()["cli_q"];
    let run = run_verify_step(
        &dir,
        &cfg,
        &cli_plan("tools/check.py"),
        &cli_goal(),
        vec![vec![write_call(
            "tools/check.py",
            &text(cli, "always_pass_verifier"),
        )]],
    );
    assert_eq!(failure_kind(&run), WITHOUT_ORACLE_KIND);
    assert!(!run.prompts.contains("Verifier authority"));
}

/// The verifier is untouched: only the implementation is repaired, so the
/// step behaves exactly as before the gate.
#[test]
fn implementation_only_repair_is_unchanged() {
    let dir = cli_workspace();
    let cli = &corpus()["cli_q"];
    write_file(dir.path(), "verify_output.py", &text(cli, "fixed_verifier"));
    let entrypoint = "src/csv_stats/main.py";
    let recorded = text(&recorded_cli_q()["files"], entrypoint);
    write_file(dir.path(), entrypoint, "import sys\nsys.exit(2)\n");
    make_entrypoint_executable(dir.path());
    let cfg = config(dir.path(), "python-cli");
    let run = run_verify_step(
        &dir,
        &cfg,
        &cli_plan("verify_output.py"),
        &cli_goal(),
        vec![vec![write_call(entrypoint, &recorded)]],
    );
    let outcome = run
        .result
        .as_ref()
        .unwrap_or_else(|e| panic!("{}", e.message));
    assert_eq!(outcome.changed_paths, [entrypoint]);
    assert!(!run.events.contains("model_verifier_edit"));
    assert!(!run.prompts.contains("Verifier authority"));
    let repair = run
        .events
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|event| event["event"] == "step_verify_repair")
        .unwrap();
    assert_eq!(repair["ok"], true);
    assert_eq!(repair["failure_kind"], "");
    assert_eq!(
        repair["allowed_action"],
        "edit_task_implementation_artifact"
    );
}
