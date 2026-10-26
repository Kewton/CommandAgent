//! Issue #861 (IC-29): a Verify or Report step must not let runtime Bash update
//! the workspace as "deterministic verifier evidence". The cases come from the
//! synthetic corpus `tests/corpus/apps/verify-runtime-bash-mutation/`; the
//! workspace is a tempdir and no real npm, model, or network is used.

use std::path::Path;
use std::sync::{Arc, Mutex};

use commandagent::config::{
    Action, Config, ConfigFieldSources, NarrationMode, OllamaThink, OpenAiApi, PlanPreset,
    PromptLayout, Provider,
};
use commandagent::planner::runner::run_step_plan;
use commandagent::planner::step_plan::{PlanStep, StepPlan};
use commandagent::providers::{AssistantReply, ChatClient};
use commandagent::state::{ConversationMessage, ToolCall};
use commandagent::tools::allow_policy::bash_verify_command_is_auto_approvable;
use commandagent::tools::registry::ToolSpec;
use serde_json::{Value, json};

const MARKER: &str = "ISSUE861-VERIFY-STEP";

fn corpus() -> Value {
    corpus_named("verify-runtime-bash-mutation")
}

/// Issue #871: the opaque execution forms, from the synthetic corpus
/// `tests/corpus/apps/verify-runtime-bash-indirect/`.
fn indirect_corpus() -> Value {
    corpus_named("verify-runtime-bash-indirect")
}

fn corpus_named(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/corpus/apps")
        .join(name)
        .join("fixtures/cases.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// Writes the verifier in the producer step, then sends one Bash command in the
/// step whose prompt carries [`MARKER`]. Every other turn is plain text.
#[derive(Clone)]
struct ScriptedClient {
    verifier_path: String,
    verifier_content: String,
    bash_command: Option<String>,
    sent_write: Arc<Mutex<bool>>,
    /// How many times the Verify step re-sends `bash_command`.
    bash_calls: usize,
    sent_bash: Arc<Mutex<usize>>,
}

impl ChatClient for ScriptedClient {
    fn label(&self) -> &str {
        "issue861-scripted"
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
        let in_verify_step = messages
            .iter()
            .any(|message| message.content.contains(MARKER));
        let call = if in_verify_step {
            let mut sent = self.sent_bash.lock().unwrap();
            match (&self.bash_command, *sent < self.bash_calls) {
                (Some(command), true) => {
                    *sent += 1;
                    Some(ToolCall::new("Bash", json!({ "command": command })))
                }
                _ => None,
            }
        } else {
            let mut sent = self.sent_write.lock().unwrap();
            if *sent {
                None
            } else {
                *sent = true;
                Some(ToolCall::new(
                    "Write",
                    json!({ "path": self.verifier_path, "content": self.verifier_content }),
                ))
            }
        };
        Ok(match call {
            Some(call) => AssistantReply {
                content: String::new(),
                tool_calls: vec![call],
                prompt_tokens: None,
                completion_tokens: None,
            },
            None => AssistantReply::text("done"),
        })
    }
}

struct Outcome {
    result: anyhow::Result<()>,
    events: Vec<Value>,
    root: tempfile::TempDir,
}

fn seed_workspace(root: &Path, corpus: &Value) {
    for (name, content) in corpus["workspace"]["files"].as_object().unwrap() {
        std::fs::write(root.join(name), content.as_str().unwrap()).unwrap();
    }
}

/// Runs "producer writes the verifier, then a Verify step whose declared host
/// verify runs that verifier" with `bash_command` as the Verify step's Bash call.
fn run_verify_case(corpus: &Value, step_kind: &str, bash_command: Option<&str>) -> Outcome {
    run_verify_case_repeating(corpus, step_kind, bash_command, 1)
}

/// Like [`run_verify_case`], with the Bash call sent `bash_calls` times.
fn run_verify_case_repeating(
    corpus: &Value,
    step_kind: &str,
    bash_command: Option<&str>,
    bash_calls: usize,
) -> Outcome {
    let root = tempfile::tempdir().unwrap();
    seed_workspace(root.path(), corpus);
    let workspace = &corpus["workspace"];
    let verifier_path = workspace["verifier_path"].as_str().unwrap();
    let mut client = ScriptedClient {
        verifier_path: verifier_path.to_string(),
        verifier_content: workspace["verifier_content"].as_str().unwrap().to_string(),
        bash_command: bash_command.map(str::to_string),
        sent_write: Arc::default(),
        bash_calls,
        sent_bash: Arc::default(),
    };
    let plan = StepPlan {
        goal: "Create the smoke verifier, then verify the workspace".to_string(),
        steps: vec![
            PlanStep {
                id: "create-verifier".to_string(),
                kind: "implement".to_string(),
                expected_result: "pass".to_string(),
                instruction: format!("Write {verifier_path}"),
                expected_paths: vec![verifier_path.to_string()],
                verify: vec![format!("test -f {verifier_path}")],
            },
            PlanStep {
                id: "verify-widget".to_string(),
                kind: step_kind.to_string(),
                expected_result: "pass".to_string(),
                instruction: format!("{MARKER}: run the smoke verifier"),
                expected_paths: Vec::new(),
                verify: vec![workspace["verifier_command"].as_str().unwrap().to_string()],
            },
        ],
    };
    let events_path = root.path().join("events.jsonl");
    let result = run_step_plan(&mut client, &plan, &config(root.path(), &events_path)).map(|_| ());
    let events = std::fs::read_to_string(&events_path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    Outcome {
        result,
        events,
        root,
    }
}

fn verify_step_policy_events(outcome: &Outcome) -> Vec<&Value> {
    outcome
        .events
        .iter()
        .filter(|event| event["event"] == "runtime_bash_policy" && event["step_kind"] == "verify")
        .collect()
}

fn step_completed(outcome: &Outcome, step_id: &str) -> bool {
    outcome
        .events
        .iter()
        .any(|event| event["event"] == "plan_step_completed" && event["step_id"] == step_id)
}

fn read(outcome: &Outcome, name: &str) -> String {
    std::fs::read_to_string(outcome.root.path().join(name)).unwrap_or_default()
}

/// The host-level negative: the Verify step tries to rewrite the verifier the
/// producer step wrote into an always-success script.
#[test]
fn verify_bash_cannot_rewrite_the_verifier_into_always_success() {
    let corpus = corpus();
    let original = corpus["workspace"]["verifier_content"].as_str().unwrap();
    for case in corpus["host_cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let command = case["command"].as_str().unwrap();
        let outcome = run_verify_case(&corpus, "verify", Some(command));

        let policy = verify_step_policy_events(&outcome);
        assert_eq!(policy.len(), 1, "{id}: {:?}", outcome.events);
        assert_eq!(policy[0]["blocked"], true, "{id}");
        assert_eq!(policy[0]["verifier_policy_ok"], false, "{id}");
        assert_eq!(policy[0]["deterministic_verifier_evidence"], false, "{id}");
        assert_eq!(
            policy[0]["policy_error_kind"],
            "verify_command_policy_error"
        );
        assert!(
            policy[0]["reason"]
                .as_str()
                .unwrap()
                .contains("not accepted as deterministic verifier evidence"),
            "{id}: {}",
            policy[0]["reason"]
        );
        assert_eq!(
            read(&outcome, "smoke-check.sh"),
            original,
            "{id}: verifier bytes changed"
        );
        assert!(
            outcome.result.is_err(),
            "{id}: the original failing host verify must still fail"
        );
        assert!(!step_completed(&outcome, "verify-widget"), "{id}");
        assert!(
            outcome
                .events
                .iter()
                .any(|event| event["event"] == "plan_step_failed"),
            "{id}: {:?}",
            outcome.events
        );
        assert!(
            !outcome.root.path().join("sentinel-ran").exists(),
            "{id}: a segment before the writer ran"
        );
    }
}

/// Issue #871: the same host-level negative through an execution form whose
/// body cannot be inspected. The verifier stays failing, the original host verify
/// still fails, and the step does not complete. The Report step cannot declare a
/// host verify, so its refusal is pinned at the policy decision instead.
#[test]
fn verify_bash_cannot_rewrite_the_verifier_through_an_opaque_form() {
    let corpus = indirect_corpus();
    let original = corpus["workspace"]["verifier_content"].as_str().unwrap();
    for case in corpus["host_cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let command = case["command"].as_str().unwrap();
        let outcome = run_verify_case(&corpus, "verify", Some(command));

        let policy = verify_step_policy_events(&outcome);
        assert_eq!(policy.len(), 1, "{id}: {:?}", outcome.events.len());
        assert_eq!(policy[0]["blocked"], true, "{id}");
        assert_eq!(policy[0]["verifier_policy_ok"], false, "{id}");
        assert_eq!(policy[0]["deterministic_verifier_evidence"], false, "{id}");
        assert_eq!(
            policy[0]["policy_error_kind"], "verify_command_policy_error",
            "{id}"
        );
        assert!(
            policy[0]["reason"]
                .as_str()
                .unwrap()
                .contains("not accepted as deterministic verifier evidence"),
            "{id}: {}",
            policy[0]["reason"]
        );
        assert_eq!(
            read(&outcome, "smoke-check.sh"),
            original,
            "{id}: verifier bytes changed"
        );
        assert!(
            outcome.result.is_err(),
            "{id}: the original failing host verify must still fail"
        );
        assert!(!step_completed(&outcome, "verify-widget"), "{id}");
        assert!(
            outcome
                .events
                .iter()
                .any(|event| event["event"] == "plan_step_failed"),
            "{id}: no plan_step_failed event"
        );
        assert!(
            !outcome.root.path().join("sentinel-ran").exists(),
            "{id}: a segment before the refused form ran"
        );
    }
}

/// A refused writer sent again on every turn up to the iteration cap never
/// reaches the shell, so the verifier stays as written and the host verify
/// keeps failing.
#[test]
fn a_refused_writer_repeated_to_the_iteration_cap_never_changes_the_verifier() {
    let corpus = indirect_corpus();
    let original = corpus["workspace"]["verifier_content"].as_str().unwrap();
    // More sends than `max_iterations` allows turns.
    let outcome = run_verify_case_repeating(
        &corpus,
        "verify",
        Some("sh -c 'cp always-pass.sh smoke-check.sh'"),
        50,
    );
    let policy = verify_step_policy_events(&outcome);
    assert!(
        policy.len() >= 2,
        "the writer was not repeated: {}",
        policy.len()
    );
    assert!(policy.iter().all(|event| event["blocked"] == true));
    assert!(
        policy
            .iter()
            .all(|event| event["deterministic_verifier_evidence"] == false)
    );
    assert_eq!(read(&outcome, "smoke-check.sh"), original);
    assert!(outcome.result.is_err(), "the host verify must still fail");
    assert!(!step_completed(&outcome, "verify-widget"));
    assert!(outcome.events.iter().all(
        |event| event["event"] != "plan_step_completed" || event["step_id"] != "verify-widget"
    ));
}

/// The same writer sent a few times, then the model ends its turn normally: the
/// declared host verify runs afterwards on the untouched verifier and fails.
#[test]
fn a_refused_writer_then_a_normal_finish_reaches_the_failing_host_verify() {
    let corpus = indirect_corpus();
    let original = corpus["workspace"]["verifier_content"].as_str().unwrap();
    let outcome = run_verify_case_repeating(
        &corpus,
        "verify",
        Some("sh -c 'cp always-pass.sh smoke-check.sh'"),
        2,
    );
    let policy = verify_step_policy_events(&outcome);
    assert_eq!(policy.len(), 2, "{:?}", outcome.events.len());
    assert!(policy.iter().all(|event| event["blocked"] == true));
    assert_eq!(read(&outcome, "smoke-check.sh"), original);
    assert!(!step_completed(&outcome, "verify-widget"));
    // The model's session ended normally, so the failure comes from the host verify.
    let observed: Vec<&Value> = outcome
        .events
        .iter()
        .filter(|event| {
            event["event"] == "verify_command_observed" && event["step"] == "verify-widget"
        })
        .collect();
    assert_eq!(observed.len(), 1);
    assert_eq!(observed[0]["effective_command"], "sh smoke-check.sh");
    assert_eq!(observed[0]["status"], "failed");
    let failure = outcome
        .events
        .iter()
        .find(|event| {
            event["event"] == "step_verify_failure" && event["step_id"] == "verify-widget"
        })
        .expect("the host verify failure is recorded");
    assert_eq!(failure["command_failures"], 1);
    assert!(
        failure["primary_reason"]
            .as_str()
            .unwrap()
            .contains("FAIL: widget missing"),
        "{failure}"
    );
}

/// Issue #871: the explicit `bash:verify` ceiling reads the same table as the
/// runtime policy. A refused row is never auto-approved; an allowed row that
/// the ceiling also admits as a plain verifier stays admitted.
#[test]
fn bash_verify_ceiling_refuses_the_corpus_opaque_forms() {
    let corpus = indirect_corpus();
    let root = tempfile::tempdir().unwrap();
    let mut refused = 0;
    for case in corpus["decision_cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let command = case["command"].as_str().unwrap();
        let steps = case["steps"].as_array().unwrap();
        if !steps.iter().any(|step| step == "verify") {
            continue;
        }
        let approved = bash_verify_command_is_auto_approvable(command, root.path());
        match case["expect"].as_str().unwrap() {
            "reject" => {
                assert!(!approved, "{id}: {command}");
                refused += 1;
            }
            "allow" => assert!(approved, "{id}: {command}"),
            other => panic!("unknown expectation {other}"),
        }
    }
    assert!(refused >= 40, "corpus table shrank: {refused}");
}

/// Repair stays possible through Write: a Verify step that fixes a source file
/// with Write is judged by the host verify, in both directions.
#[test]
fn verify_write_repair_is_still_judged_by_the_host_verify() {
    for (content, passes) in [("good\n", true), ("still bad\n", false)] {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("src")).unwrap();
        std::fs::write(root.path().join("src/value.txt"), "bad\n").unwrap();
        std::fs::write(
            root.path().join("check.sh"),
            "grep -q '^good$' src/value.txt\n",
        )
        .unwrap();
        let mut client = ImplementClient::with_call(ToolCall::new(
            "Write",
            json!({ "path": "src/value.txt", "content": content }),
        ));
        let plan = StepPlan {
            goal: "Repair the value".to_string(),
            steps: vec![PlanStep {
                id: "verify-value".to_string(),
                kind: "verify".to_string(),
                expected_result: "pass".to_string(),
                instruction: "Verify and repair src/value.txt".to_string(),
                expected_paths: Vec::new(),
                verify: vec!["sh check.sh".to_string()],
            }],
        };
        let events_path = root.path().join("events.jsonl");
        let result = run_step_plan(&mut client, &plan, &config(root.path(), &events_path));
        assert_eq!(result.is_ok(), passes, "{content}: {result:?}");
        assert_eq!(
            std::fs::read_to_string(root.path().join("src/value.txt")).unwrap(),
            content
        );
        let events = std::fs::read_to_string(&events_path).unwrap();
        assert!(
            events.contains("\"event\":\"tool_execute\",\"name\":\"Write\""),
            "{events}"
        );
    }
}

/// An Implement step keeps its in-workspace Bash updates.
#[test]
fn implement_bash_update_inside_the_workspace_is_still_allowed() {
    let corpus = corpus();
    let root = tempfile::tempdir().unwrap();
    seed_workspace(root.path(), &corpus);
    let mut client = ImplementClient::default();
    let plan = StepPlan {
        goal: "Copy a file with Bash".to_string(),
        steps: vec![PlanStep {
            id: "copy-with-bash".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Copy always-pass.sh to copied.sh".to_string(),
            expected_paths: vec!["copied.sh".to_string()],
            verify: vec!["test -f copied.sh".to_string()],
        }],
    };
    let events_path = root.path().join("events.jsonl");
    run_step_plan(&mut client, &plan, &config(root.path(), &events_path)).unwrap();
    assert_eq!(
        std::fs::read_to_string(root.path().join("copied.sh")).unwrap(),
        "exit 0\n"
    );
}

#[derive(Clone)]
struct ImplementClient {
    call: ToolCall,
    sent: Arc<Mutex<bool>>,
}

impl Default for ImplementClient {
    fn default() -> Self {
        Self::with_call(ToolCall::new(
            "Bash",
            json!({ "command": "cp always-pass.sh copied.sh" }),
        ))
    }
}

impl ImplementClient {
    fn with_call(call: ToolCall) -> Self {
        Self {
            call,
            sent: Arc::default(),
        }
    }
}

impl ChatClient for ImplementClient {
    fn label(&self) -> &str {
        "issue861-implement"
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
        let mut sent = self.sent.lock().unwrap();
        if *sent {
            return Ok(AssistantReply::text("done"));
        }
        *sent = true;
        Ok(AssistantReply {
            content: String::new(),
            tool_calls: vec![self.call.clone()],
            prompt_tokens: None,
            completion_tokens: None,
        })
    }
}

fn config(root: &Path, events: &Path) -> Config {
    Config {
        workspace_root: root.to_path_buf(),
        state_dir: root.join("state"),
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
        openai_compatible: None,
        ollama_host: "http://localhost:11434".to_string(),
        ollama_think: None,
        lm_studio_host: "http://localhost:1234".to_string(),
        num_predict: 100,
        max_iterations: 5,
        recovery_plan_auto_runs: 0,
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
