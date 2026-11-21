use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use super::super::{ContractEnforcement, NOOP_UI, SessionSnapshot, StepPromptContext, run_step};
use super::{FAILURE_KIND, detect};
use crate::config::Config;
use crate::minimal_loop::repair_target::{RepairTarget, classify_repair_target};
use crate::planner::profile::resolve_profile_runtime;
use crate::planner::step_plan::{PlanStep, StepPlan};
use crate::planner::verify::VerificationReport;
use crate::providers::{AssistantReply, ChatClient};
use crate::state::{ConversationMessage, ToolCall};
use crate::tools::registry::ToolSpec;
use crate::verification::diagnostics::CompileError;

const UNCHANGED: &str = "verify_repair_progress_unchanged";
const FAIL_REASON: &str = "command failed: python3 verify_output.py outcome: CommandFailed \
     status: exit status: 1 summary: 8 assertion(s) failed stdout: FAIL: no table row for \
     column 'price' FAIL: no table row for column 'qty'";

fn corpus() -> serde_json::Value {
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
        let target = root.join(path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, content.as_str().unwrap()).unwrap();
    }
}

/// The recorded G1 cli-q workspace; the compatibility shim is optional.
fn workspace(shim: bool) -> tempfile::TempDir {
    let corpus = corpus();
    let dir = tempfile::tempdir().unwrap();
    write_files(dir.path(), &corpus["files"]);
    if shim {
        write_files(dir.path(), &corpus["shim"]);
    }
    // The profile's post-step repair makes the entrypoint executable in a real run.
    use std::os::unix::fs::PermissionsExt;
    let entrypoint = dir.path().join("src/csv_stats/main.py");
    std::fs::set_permissions(entrypoint, std::fs::Permissions::from_mode(0o755)).unwrap();
    dir
}

fn goal() -> String {
    text(&corpus(), "goal")
}

fn config(root: &Path, profile: &str) -> Config {
    Config {
        workspace_root: root.to_path_buf(),
        state_dir: PathBuf::from("state"),
        eval_events_path: None,
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
        instruction: format!("{id} from the recorded cli-q verification phase"),
        expected_paths: paths.iter().map(|path| path.to_string()).collect(),
        verify: verify.iter().map(|command| command.to_string()).collect(),
    }
}

/// The recorded verification phase; `produced` says whether the Implement
/// step creates the verifier the Verify step later runs.
fn plan(produced: bool) -> (StepPlan, PlanStep) {
    let corpus = corpus();
    let producer_path = if produced {
        text(&corpus, "verifier_path")
    } else {
        "notes.md".to_string()
    };
    let verifier = step(
        &text(&corpus, "verifier_step"),
        "verify",
        &[],
        &[&text(&corpus, "verifier_command")],
    );
    let plan = StepPlan {
        goal: goal(),
        steps: vec![
            step("inspect-existing-fixture", "inspect", &[], &[]),
            step(
                &text(&corpus, "producer_step"),
                "implement",
                &[&producer_path],
                &[],
            ),
            step(
                "check-syntax",
                "verify",
                &[],
                &[&text(&corpus, "host_oracle_command")],
            ),
            verifier.clone(),
        ],
    };
    (plan, verifier)
}

fn failing_report() -> VerificationReport {
    VerificationReport::command_failed("python3 verify_output.py", FAIL_REASON)
}

fn detected(root: &Path, profile: &str, produced: bool, report: &VerificationReport) -> bool {
    let (plan, step) = plan(produced);
    detect(&config(root, profile), &plan, &step, &goal(), report).is_some()
}

#[test]
fn passing_host_oracles_with_failing_model_verifier_is_a_disagreement() {
    let dir = workspace(false);
    let (plan, step) = plan(true);
    let found = detect(
        &config(dir.path(), "python-cli"),
        &plan,
        &step,
        &goal(),
        &failing_report(),
    )
    .expect("host oracles pass while the model verifier fails");
    let evidence = found.evidence.join("\n");
    for expected in [
        FAILURE_KIND,
        "model_verifier: verify_output.py",
        "model_verifier_producer_step: create-verification-script",
        "model_verifier_command: python3 verify_output.py",
        "FAIL: no table row for column 'price'",
        "verify_final=pass",
        "behavior_probe=pass",
        "behavior_probe_evidence: ",
        "not decided",
    ] {
        assert!(
            evidence.contains(expected),
            "{expected} missing: {evidence}"
        );
    }
}

#[test]
fn recorded_cli_q_with_shim_stays_progress_unchanged() {
    let dir = workspace(true);
    assert!(
        !resolve_profile_runtime("python-cli")
            .verify_final(dir.path(), &goal())
            .is_pass(),
        "the shim makes the host's verify_final fail"
    );
    assert!(!detected(dir.path(), "python-cli", true, &failing_report()));
}

#[test]
fn verifier_not_produced_by_an_earlier_implement_step_stays_progress_unchanged() {
    let dir = workspace(false);
    assert!(detected(dir.path(), "python-cli", true, &failing_report()));
    assert!(!detected(
        dir.path(),
        "python-cli",
        false,
        &failing_report()
    ));
}

#[test]
fn profile_without_behavior_oracle_stays_progress_unchanged() {
    let dir = workspace(false);
    assert!(detected(dir.path(), "python-cli", true, &failing_report()));
    assert!(!detected(dir.path(), "generic", true, &failing_report()));
}

#[test]
fn dependency_or_compile_entries_stay_progress_unchanged() {
    let dir = workspace(false);
    let mut with_dependency = failing_report();
    with_dependency.push_dependency_missing("ModuleNotFoundError: No module named 'x'");
    let mut with_compile = failing_report();
    with_compile.push_compile_errors(
        "python3 verify_output.py",
        vec![CompileError {
            path: "src/csv_stats/main.py".to_string(),
            line: 1,
            column: 1,
            message: "SyntaxError".to_string(),
            excerpt: String::new(),
            symbol: None,
            route_bound: None,
        }],
    );
    let mut with_missing_path = failing_report();
    with_missing_path.push_missing_path("src/csv_stats/main.py");
    let mut with_profile_failure = failing_report();
    with_profile_failure.push_profile_failure("profile contract failed");
    let mut with_false_negative = failing_report();
    with_false_negative.push_verifier_command_false_negative("grep x", "masks exit status");
    for report in [
        with_dependency,
        with_compile,
        with_missing_path,
        with_profile_failure,
        with_false_negative,
    ] {
        assert!(!detected(dir.path(), "python-cli", true, &report));
    }
}

#[test]
fn failing_behavior_probe_stays_progress_unchanged() {
    let dir = workspace(false);
    std::fs::write(
        dir.path().join("src/csv_stats/main.py"),
        "import sys\nsys.exit(3)\n",
    )
    .unwrap();
    let final_report = resolve_profile_runtime("python-cli").verify_final(dir.path(), &goal());
    assert!(
        final_report.is_pass(),
        "only the behavior probe may fail: {}",
        final_report.primary_reason()
    );
    assert!(!detected(dir.path(), "python-cli", true, &failing_report()));
}

#[test]
fn failed_host_oracle_command_stays_progress_unchanged() {
    let dir = workspace(false);
    let corpus = corpus();
    let command = text(&corpus, "host_oracle_command");
    let report = VerificationReport::command_failed(command.as_str(), "SyntaxError");
    assert!(!detected(dir.path(), "python-cli", true, &report));
}

#[test]
fn contract_deterministic_oracle_script_stays_progress_unchanged() {
    let dir = workspace(false);
    let (plan, step) = plan(true);
    let mut cfg = config(dir.path(), "python-cli");
    assert!(detect(&cfg, &plan, &step, &goal(), &failing_report()).is_some());
    let contract = dir.path().join("contract.json");
    std::fs::write(
        &contract,
        serde_json::json!({"deterministic_oracles": ["python3 verify_output.py"]}).to_string(),
    )
    .unwrap();
    cfg.completion_contract_path = Some(contract);
    assert!(detect(&cfg, &plan, &step, &goal(), &failing_report()).is_none());
}

#[test]
fn classification_leaves_is_pass_and_the_auto_recovery_list_unchanged() {
    assert!(!failing_report().is_pass());
    assert_ne!(
        classify_repair_target(&failing_report()),
        RepairTarget::ModelVerifier,
        "classify_repair_target must not produce the disagreement target"
    );
    assert_eq!(RepairTarget::ModelVerifier.as_str(), "model_verifier");
    let retry = include_str!("../../../auto_recovery/retry.rs");
    let body = retry
        .split("fn implementation_failure")
        .nth(1)
        .and_then(|rest| rest.split("\n}\n").next())
        .unwrap();
    let kinds: Vec<&str> = body
        .lines()
        .filter_map(|line| line.trim().trim_start_matches("| ").strip_prefix('"'))
        .filter_map(|line| line.split('"').next())
        .collect();
    assert_eq!(
        kinds,
        [
            "implementation_compile_error",
            "compile_error",
            "verification_failed",
            "bounded_repair_exhausted",
            "compile_repair_no_source_change",
            "verify_repair_progress_unchanged",
            "model_stagnation:read_only_loop",
            "model_stagnation:no_progress_recorded",
        ]
    );
}

#[derive(Clone)]
struct ScriptedClient {
    replies: Arc<Mutex<Vec<AssistantReply>>>,
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
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        let mut replies = self.replies.lock().unwrap();
        if replies.is_empty() {
            anyhow::bail!("scripted client exhausted")
        }
        Ok(replies.remove(0))
    }
}

/// Turns that only run `true`, so the CLI and the verifier stay untouched and
/// the verification report stays identical across repairs.
fn exhausting_replies() -> Vec<AssistantReply> {
    let mut replies = Vec::new();
    for label in ["initial", "one", "two", "three", "four"] {
        replies.push(AssistantReply {
            content: String::new(),
            tool_calls: vec![ToolCall::new(
                "Bash",
                serde_json::json!({"command": "true"}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        });
        replies.push(AssistantReply::text(format!("{label} incomplete")));
    }
    replies
}

struct Run {
    message: String,
    stop_reason: Option<String>,
    events: String,
    handoff: String,
}

fn run_verify_step(shim: bool) -> Run {
    let dir = workspace(shim);
    let events = dir.path().join("run/events.jsonl");
    let mut cfg = config(dir.path(), "python-cli");
    cfg.eval_events_path = Some(events.clone());
    let (plan, step) = plan(true);
    let context = StepPromptContext {
        overall_goal: goal(),
        ..StepPromptContext::default()
    };
    let mut client = ScriptedClient {
        replies: Arc::new(Mutex::new(exhausting_replies())),
    };
    let mut session = SessionSnapshot::new();
    let error = run_step(
        &mut client,
        &mut session,
        &plan,
        &step,
        &context,
        &cfg,
        &NOOP_UI,
        "test",
        ContractEnforcement::Enforce,
        None,
        None,
    )
    .expect_err("the phase still fails after bounded repair");
    let handoff = std::fs::read_dir(dir.path().join(".commandagent/plans"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| std::fs::read_to_string(entry.path()).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n");
    Run {
        message: error.message,
        stop_reason: error.outcome.stop_reason,
        events: std::fs::read_to_string(&events).unwrap_or_default(),
        handoff,
    }
}

#[test]
fn bounded_repair_exhaustion_hands_off_the_disagreement_and_still_fails() {
    let run = run_verify_step(false);
    assert_eq!(
        run.stop_reason.as_deref(),
        Some(FAILURE_KIND),
        "{}",
        run.message
    );
    assert!(
        run.events
            .contains(&format!("\"reason\":\"{FAILURE_KIND}\""))
    );
    assert!(run.events.contains("\"repair_target\":\"model_verifier\""));
    assert!(
        run.events
            .contains(&format!("\"failure_kind\":\"{FAILURE_KIND}\""))
    );
    assert!(!run.events.contains(&format!("\"reason\":\"{UNCHANGED}\"")));
    for expected in [
        "verify_output.py",
        "create-verification-script",
        "FAIL: no table row for column",
        "verify_final=pass",
        "behavior_probe=pass",
        "model_verifier",
    ] {
        assert!(
            run.handoff.contains(expected),
            "{expected}: {}",
            run.handoff
        );
    }
}

#[test]
fn recorded_cli_q_workspace_keeps_the_progress_unchanged_handoff() {
    let run = run_verify_step(true);
    assert_ne!(
        run.stop_reason.as_deref(),
        Some(FAILURE_KIND),
        "{}",
        run.message
    );
    assert!(!run.events.contains("model_verifier"));
    assert!(!run.handoff.contains("behavior_probe=pass"));
}
