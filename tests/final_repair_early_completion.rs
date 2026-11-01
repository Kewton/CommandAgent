//! Issue #956 — a final-acceptance repair of an existing invalid artifact is
//! not completed by a read-only Bash command, a passing inspection, or the
//! mere presence of its paths. Through the public UltraPlan entry
//! (`/run-ultra-plan`), a failed artifact-check receipt binds its artifacts
//! to the repair session; the session keeps the next provider request until
//! a bound artifact's raw bytes change (or a Write/Edit hands the decision to
//! the repair loop's snapshot), and then the same check reruns.
//!
//! This test binary owns its process environment: it sets the operator
//! registry root once and serializes the runner cases with a lock. The
//! adapter is a synthetic POSIX shell script run by the operator-pinned
//! `/bin/sh`; the planner and executor are scripted fake clients; every root
//! lives in a temporary directory. No case reaches a real provider, the
//! network, a model, or a credential. The expected sequences live in
//! `tests/corpus/apps/final-repair-early-completion/fixtures/events.json`;
//! each `base_sequence` and `base_executor_calls` was observed by running
//! this file against the unchanged sources of develop 3ec27982.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use commandagent::config::{Action, Config, Provider};
use commandagent::minimal_loop::loop_run::run_session_with_outcome_with_ui;
use commandagent::planner::ultra_plan::{UltraPhase, UltraPlan, render_ultra_plan};
use commandagent::providers::{AssistantReply, ChatClient};
use commandagent::state::{ConversationMessage, SessionSnapshot, ToolCall};
use commandagent::tools::registry::ToolSpec;
use commandagent::tui::status::UiStatus;
use commandagent::tui::{InteractionUi, UiGuard};
use serde_json::{Value, json};
use sha2::Digest;

const CORPUS: &str = "tests/corpus/apps/final-repair-early-completion";
const ADAPTER: &str = "fr-gate";
const UNPINNED_ADAPTER: &str = "fr-unpinned";
const BOUND_PROFILE: &str = "fr-bound";
const UNBOUND_PROFILE: &str = "fr-unbound";
const REQUEST_PATH: &str = "check/request.json";
const TARGET: &str = "contract/terms.json";
const NOTES: &str = "notes/delivery.md";
const BROKEN: &str = "{\"state\": \"draft\"\n";
const FIXED: &str = "{\"state\": \"ready\"}\n";
const REPAIR_MAX_ATTEMPTS: u64 = 2;
const REPAIR_MAX_ITERATIONS: u64 = 6;

/// Passes when the bound input holds `ready`; anything else is a failed
/// required row. Every start appends the request ID to the observation file.
const ADAPTER_BODY: &str = r#"IFS= read -r envelope || :
field() { value=${envelope#*\"$1\":\"}; printf '%s' "${value%%\"*}"; }
request_id=$(field request_id)
nonce=$(field nonce)
workspace=$(field workspace)
path=$(field path)
sha=$(field sha256)
printf '%s\n' "$request_id" >> "$1/calls"
verdict=failed
while IFS= read -r line || [ -n "$line" ]; do
  case $line in
    *state*ready*) verdict=passed ;;
  esac
done < "$workspace/$path"
case $verdict in
  passed) counts='"executed":1,"passed":1,"failed":0,"unresolved":0'; code=0 ;;
  *) counts='"executed":1,"passed":0,"failed":1,"unresolved":0'; code=1 ;;
esac
printf '{"protocol":"artifact-contract.result/v1","request_id":"%s","nonce":"%s","input_digests":[{"input_id":"terms","sha256":"%s"}],"checks":[{"check_id":"terms/state","target_id":null,"status":"%s","counts":{"expected":1,%s}}]}\n' "$request_id" "$nonce" "$sha" "$verdict" "$counts"
exit "$code"
"#;

struct Fixture {
    observe: PathBuf,
}

fn fixture() -> &'static Fixture {
    static FIXTURE: OnceLock<Fixture> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let root = std::fs::canonicalize(tempfile::tempdir().unwrap().keep()).unwrap();
        let tooling = root.join("tooling");
        let config = root.join("config");
        let extension = root.join("extension");
        let observe = root.join("observe");
        for dir in [&tooling, &config.join("adapters"), &observe] {
            std::fs::create_dir_all(dir).unwrap();
        }
        std::fs::write(tooling.join("gate.sh"), ADAPTER_BODY).unwrap();
        let descriptor = json!({
            "protocol": "artifact-contract.adapter/v1",
            "adapter_id": ADAPTER,
            "executable": {"path": shell(), "sha256": sha256_file(&shell())},
            "tooling_root": tooling,
            "entrypoint": "gate.sh",
            "args": [observe.to_string_lossy()],
            "members": {"gate.sh": sha256_file(&tooling.join("gate.sh"))},
            "required": [{"check_id": "terms/state", "targets": []}],
            "inputs": [{"input_id": "terms"}],
            "limits": {"timeout_seconds": 2, "max_stream_bytes": 24000}
        });
        std::fs::write(
            config.join(format!("adapters/{ADAPTER}.json")),
            serde_json::to_vec_pretty(&descriptor).unwrap(),
        )
        .unwrap();
        // The unbound profile's adapter has tooling of its own that changes
        // after registration: its receipt stops at the pin and names no
        // artifact.
        let unpinned_tooling = root.join("unpinned-tooling");
        std::fs::create_dir_all(&unpinned_tooling).unwrap();
        std::fs::write(unpinned_tooling.join("gate.sh"), ADAPTER_BODY).unwrap();
        let mut unpinned = descriptor.clone();
        unpinned["adapter_id"] = json!(UNPINNED_ADAPTER);
        unpinned["tooling_root"] = json!(unpinned_tooling);
        std::fs::write(
            config.join(format!("adapters/{UNPINNED_ADAPTER}.json")),
            serde_json::to_vec_pretty(&unpinned).unwrap(),
        )
        .unwrap();
        for (profile, adapter) in [
            (BOUND_PROFILE, ADAPTER),
            (UNBOUND_PROFILE, UNPINNED_ADAPTER),
        ] {
            let directory = extension.join(format!("profiles/{profile}"));
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(directory.join("manifest.toml"), manifest(profile, adapter)).unwrap();
        }
        // SAFETY: set once under the OnceLock before any case reads it; this
        // test binary owns its environment.
        unsafe {
            std::env::set_var("COMMANDAGENT_ADAPTER_CONFIG_ROOT", &config);
        }
        commandagent::planner::extension_profiles::register(&extension).unwrap();
        std::fs::write(
            unpinned_tooling.join("gate.sh"),
            format!("{ADAPTER_BODY}# changed after registration\n"),
        )
        .unwrap();
        Fixture { observe }
    })
}

fn manifest(profile: &str, adapter: &str) -> String {
    format!(
        r#"[metadata]
id = "{profile}"
display_name = "Final repair {profile}"
schema_version = "v2"
task_family = "generic"
[plan]
intent = "create"
phases = [
  {{ id = "implementation", prompt = "Write the requested contract for {{goal}}." }},
  {{ id = "verification", prompt = "Recheck the contract for {{goal}}." }},
]
[artifacts]
required = ["{TARGET}", "{NOTES}"]
[guidance.variants.contract]
triggers = [{{ condition = "always" }}]
messages = {{ instruction = "Write the contract files and {REQUEST_PATH}." }}
[[checks.final]]
id = "artifact_contract_check"
params = {{ adapter_id = "{adapter}", request = "{REQUEST_PATH}" }}
"#
    )
}

fn shell() -> PathBuf {
    std::fs::canonicalize("/bin/sh").unwrap()
}

fn sha256_file(path: &Path) -> String {
    format!("{:x}", sha2::Sha256::digest(std::fs::read(path).unwrap()))
}

fn runner_lock() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn read_events(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

fn events_named<'a>(events: &'a [Value], name: &str) -> Vec<&'a Value> {
    events
        .iter()
        .filter(|event| event["event"] == name)
        .collect()
}

/// The repair-relevant events in order: from the first repair start, each
/// executed tool and each held completion is named as well.
fn projection(events: &[Value]) -> Vec<String> {
    let mut repairing = false;
    events
        .iter()
        .filter_map(|event| {
            let text = |key: &str| event[key].as_str().unwrap_or_default().to_string();
            Some(match event["event"].as_str()? {
                "artifact_contract_check_result" => {
                    format!("receipt:{}:{}", text("status"), text("stopped_at"))
                }
                "final_acceptance_repair_start" => {
                    repairing = true;
                    format!("repair_start:max={}", event["max_attempts"])
                }
                "tool_execute" if repairing => {
                    format!("tool:{}:{}", text("name"), text("status"))
                }
                "step_completion_blocked" if repairing => format!("blocked:{}", text("reason")),
                "final_acceptance_repair_complete" => format!(
                    "repair_complete:{}:[{}]",
                    event["attempt"],
                    event["changed_paths"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(",")
                ),
                "final_acceptance_repair_no_source_change" => {
                    format!("no_source_change:{}", event["attempt"])
                }
                "final_acceptance_repair_exhausted" => {
                    format!("exhausted:{}", text("exhausted_reason"))
                }
                "final_acceptance_cycle_complete" => format!("cycle_complete:{}", event["ok"]),
                "tui_command_stop" => format!("stop:{}", event["ok"]),
                _ => return None,
            })
        })
        .collect()
}

fn render(events: &[Value]) -> String {
    events
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

fn request(request_id: &str) -> String {
    format!(
        r#"{{"protocol":"artifact-contract.request/v1","request_id":"{request_id}","inputs":[{{"input_id":"terms","path":"{TARGET}"}}]}}"#
    )
}

fn calls(calls: Vec<(&str, Value)>) -> AssistantReply {
    AssistantReply {
        content: String::new(),
        tool_calls: calls
            .into_iter()
            .map(|(name, arguments)| ToolCall::new(name, arguments))
            .collect(),
        prompt_tokens: None,
        completion_tokens: None,
    }
}

fn bash(command: &str) -> AssistantReply {
    calls(vec![("Bash", json!({"command": command}))])
}

fn write(path: &str, content: &str) -> AssistantReply {
    calls(vec![("Write", json!({"path": path, "content": content}))])
}

fn read(path: &str) -> AssistantReply {
    calls(vec![("Read", json!({"path": path}))])
}

fn text(content: &str) -> AssistantReply {
    AssistantReply::text(content.to_string())
}

struct Case {
    id: &'static str,
    profile: &'static str,
    /// Replies after the two plan phases, in order.
    repairs: Vec<AssistantReply>,
}

struct Run {
    events: Vec<Value>,
    output: anyhow::Result<String>,
    executor_calls: usize,
    planner_calls: usize,
    adapter_calls: usize,
    workspace: tempfile::TempDir,
    elapsed: Duration,
}

fn run(case: Case) -> Run {
    let fixture = fixture();
    let _lock = runner_lock();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let events_path = root.join(format!(".anvil/runs/{}/events.jsonl", case.id));
    let plan = UltraPlan {
        goal: "Write the contract terms".to_string(),
        profile: case.profile.to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: vec![
            UltraPhase {
                id: "implementation".to_string(),
                prompt: "Write the contract and the adapter request.".to_string(),
            },
            UltraPhase {
                id: "verification".to_string(),
                prompt: "Rewrite the contract and the adapter request unchanged.".to_string(),
            },
        ],
    };
    std::fs::write(root.join("ultra.yaml"), render_ultra_plan(&plan)).unwrap();
    let mut cfg = config(root.to_path_buf());
    cfg.eval_events_path = Some(events_path.clone());
    cfg.profile = case.profile.to_string();
    cfg.profile_explicit = true;
    let step_plan = json!({
        "goal": "Write the contract",
        "steps": [{
            "id": "step-1",
            "kind": "implement",
            "expected_result": "pass",
            "instruction": "Write the contract and the adapter request",
            "expected_paths": [TARGET, NOTES, REQUEST_PATH],
            "verify": []
        }]
    })
    .to_string();
    let mut planner = FakeClient::new(
        "planner",
        vec![
            AssistantReply::text(step_plan.clone()),
            AssistantReply::text(step_plan),
        ],
    );
    let request = request(case.id);
    let initial = || {
        calls(vec![
            ("Write", json!({"path": TARGET, "content": BROKEN})),
            ("Write", json!({"path": NOTES, "content": "# Delivery\n"})),
            ("Write", json!({"path": REQUEST_PATH, "content": request})),
        ])
    };
    let mut replies = vec![initial(), initial()];
    replies.extend(case.repairs);
    let mut execution = FakeClient::new("execution", replies);
    let started = Instant::now();
    let output = commandagent::tui::slash::handle_command(
        "/run-ultra-plan ultra.yaml",
        &cfg,
        &mut planner,
        &mut execution,
        &FakeUi,
    );
    let elapsed = started.elapsed();
    let adapter_calls = std::fs::read_to_string(fixture.observe.join("calls"))
        .unwrap_or_default()
        .lines()
        .filter(|line| *line == case.id)
        .count();
    Run {
        events: read_events(&events_path),
        output,
        executor_calls: execution.calls.load(Ordering::SeqCst),
        planner_calls: planner.calls.load(Ordering::SeqCst),
        adapter_calls,
        workspace: dir,
        elapsed,
    }
}

fn corpus_case(id: &str) -> Value {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(CORPUS);
    let fixture: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("fixtures/events.json")).unwrap())
            .unwrap();
    fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == id)
        .unwrap_or_else(|| panic!("{id} is not in the corpus"))
        .clone()
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item.as_str().unwrap().to_string())
        .collect()
}

/// The observed sequence and counts equal the corpus; the existing repair
/// limits and provider counters hold.
fn assert_case(id: &str, run: &Run) {
    let observed = projection(&run.events);
    eprintln!(
        "{id}: executor_calls={} sequence={}",
        run.executor_calls,
        json!(observed)
    );
    let expected = corpus_case(id);
    assert_eq!(
        observed,
        strings(&expected["sequence"]),
        "{id}: sequence\n{}",
        render(&run.events)
    );
    assert_eq!(
        run.executor_calls as u64, expected["executor_calls"],
        "{id}: executor calls"
    );
    assert_eq!(run.planner_calls, 2, "{id}");
    let receipts = events_named(&run.events, "artifact_contract_check_result");
    assert_eq!(receipts.len() as u64, expected["receipts"], "{id}");
    assert_eq!(run.adapter_calls as u64, expected["adapter_calls"], "{id}");
    let starts = events_named(&run.events, "final_acceptance_repair_start");
    for start in &starts {
        assert_eq!(start["max_attempts"], REPAIR_MAX_ATTEMPTS, "{start}");
        assert_eq!(start["max_iterations"], REPAIR_MAX_ITERATIONS, "{start}");
    }
    let completes = events_named(&run.events, "final_acceptance_repair_complete");
    assert!(completes.len() as u64 <= REPAIR_MAX_ATTEMPTS, "{id}");
    // Every repair request is one counted provider turn of the same
    // executor: no other client, critic, or budget is used.
    let repair_turns = run
        .events
        .iter()
        .filter(|event| {
            event["event"] == "provider_turn_duration" && event["caller_scope"] == "repair"
        })
        .count();
    assert_eq!(repair_turns + 2, run.executor_calls, "{id}: repair turns");
    let passed = expected["outcome"] == "passed";
    assert_eq!(
        run.output.is_ok(),
        passed,
        "{id}: {:?}",
        run.output.as_ref().err()
    );
    // No held completion carries file content.
    for blocked in events_named(&run.events, "step_completion_blocked") {
        let text = blocked.to_string();
        assert!(!text.contains("draft"), "{text}");
        assert!(!text.contains("ready"), "{text}");
    }
}

fn target(run: &Run) -> String {
    std::fs::read_to_string(run.workspace.path().join(TARGET)).unwrap()
}

// --- the defect and its controls -------------------------------------------

/// The reported shape: a successful read-only `cat` of the invalid artifact,
/// then the planned Write. The session sends the next request, the Write
/// lands, and the same check passes.
#[test]
fn a_bash_cat_before_the_write_keeps_the_next_request() {
    let run = run(Case {
        id: "bash-cat-then-write-1",
        profile: BOUND_PROFILE,
        repairs: vec![bash(&format!("cat {TARGET}")), write(TARGET, FIXED)],
    });
    assert_case("bash-cat-then-write-1", &run);
    assert_eq!(target(&run), FIXED);
}

/// A passing inspection (`test -f`, `ls`) is not a repair either.
#[test]
fn a_passing_inspection_before_the_write_keeps_the_next_request() {
    let run = run(Case {
        id: "inspect-then-write-1",
        profile: BOUND_PROFILE,
        repairs: vec![
            bash(&format!("test -f {TARGET} && ls contract")),
            write(TARGET, FIXED),
        ],
    });
    assert_case("inspect-then-write-1", &run);
}

/// Control: the same bytes repaired by a native Read, then Write.
#[test]
fn a_native_read_then_write_repairs_as_before() {
    let run = run(Case {
        id: "read-then-write-1",
        profile: BOUND_PROFILE,
        repairs: vec![read(TARGET), write(TARGET, FIXED)],
    });
    assert_case("read-then-write-1", &run);
}

/// Control: Write first repairs in one request, as before.
#[test]
fn a_write_first_repairs_in_one_request() {
    let run = run(Case {
        id: "write-first-1",
        profile: BOUND_PROFILE,
        repairs: vec![write(TARGET, FIXED)],
    });
    assert_case("write-first-1", &run);
}

/// A Bash edit of the bound artifact is a raw-byte change: no further
/// request, and the same check reruns. A Write count does not decide it.
#[test]
fn a_bash_edit_of_the_bound_artifact_reruns_the_check_without_another_request() {
    let run = run(Case {
        id: "bash-edit-target-1",
        profile: BOUND_PROFILE,
        repairs: vec![bash(&format!(
            "printf '%s\\n' '{{\"state\": \"ready\"}}' > {TARGET}"
        ))],
    });
    assert_case("bash-edit-target-1", &run);
    assert_eq!(target(&run), FIXED);
}

/// A Bash change of an expected file the failed check does not read is not
/// a repair of the bound artifact: the session keeps the next request, in
/// the same attempt.
#[test]
fn a_bash_change_of_an_unrelated_expected_file_keeps_the_next_request() {
    let run = run(Case {
        id: "bash-unrelated-then-write-1",
        profile: BOUND_PROFILE,
        repairs: vec![
            bash(&format!("printf '%s\\n' '# Delivery v2' > {NOTES}")),
            write(TARGET, FIXED),
        ],
    });
    assert_case("bash-unrelated-then-write-1", &run);
}

/// A final answer without tools that claims the repair, after a `cat`,
/// is held too (the no-tool exit keeps the obligation).
#[test]
fn a_self_reported_final_answer_keeps_the_next_request() {
    let run = run(Case {
        id: "cat-final-then-write-1",
        profile: BOUND_PROFILE,
        repairs: vec![
            bash(&format!("cat {TARGET}")),
            text("The contract is repaired."),
            write(TARGET, FIXED),
        ],
    });
    assert_case("cat-final-then-write-1", &run);
}

/// A change undone in the same batch is no change.
#[test]
fn a_change_undone_keeps_the_next_request() {
    let run = run(Case {
        id: "bash-undo-then-write-1",
        profile: BOUND_PROFILE,
        repairs: vec![
            bash(&format!(
                "cp {TARGET} keep.json && printf x >> {TARGET} && cp keep.json {TARGET}"
            )),
            write(TARGET, FIXED),
        ],
    });
    assert_case("bash-undo-then-write-1", &run);
}

/// A session that only keeps reading ends as a non-success inside the
/// existing iteration limits of the existing bounded repair.
#[test]
fn reading_only_ends_as_a_non_success_inside_the_existing_limits() {
    let repairs = (0..40)
        .map(|index| bash(&format!("cat {TARGET} # {index}")))
        .collect();
    let run = run(Case {
        id: "read-only-1",
        profile: BOUND_PROFILE,
        repairs,
    });
    assert_case("read-only-1", &run);
    assert_eq!(target(&run), BROKEN);
}

/// Control: without a host binding (the receipt stopped before naming its
/// artifacts) the session ends after the `cat` exactly as before.
#[test]
fn without_a_binding_the_session_ends_as_before() {
    let run = run(Case {
        id: "unbound-cat-then-write-1",
        profile: UNBOUND_PROFILE,
        repairs: vec![bash(&format!("cat {TARGET}")), write(TARGET, FIXED)],
    });
    assert_case("unbound-cat-then-write-1", &run);
}

/// A bound artifact replaced by a FIFO is never opened for a blocking read:
/// the public entry returns in finite time, as a non-success.
#[cfg(unix)]
#[test]
fn a_fifo_artifact_is_refused_and_the_entry_returns() {
    let run = run(Case {
        id: "fifo-1",
        profile: BOUND_PROFILE,
        repairs: vec![
            bash(&format!("rm {TARGET} && mkfifo {TARGET}")),
            bash("ls contract"),
            bash("ls check"),
        ],
    });
    assert!(run.elapsed < Duration::from_secs(120), "{:?}", run.elapsed);
    assert_case("fifo-1", &run);
}

// --- direct prompt (CA bench generic minimal loop) ----------------------------

/// A direct prompt (scope MinimalLoop, no step kind) keeps its call sequence:
/// no binding exists there, so no new obligation, read, or request is added.
#[test]
fn a_direct_prompt_keeps_its_requests() {
    let _lock = runner_lock();
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("notes.txt"), "spring\n").unwrap();
    let cfg = config(dir.path().to_path_buf());
    let mut client = FakeClient::new(
        "execution",
        vec![bash("cat notes.txt"), text("The note says spring.")],
    );
    let mut session = SessionSnapshot::new();
    let outcome = run_session_with_outcome_with_ui(
        &mut client,
        &mut session,
        "Show what notes.txt says.",
        &["notes.txt".to_string()],
        &cfg,
        &FakeUi,
    );
    let calls = client.calls.load(Ordering::SeqCst) as u64;
    eprintln!("direct: calls={calls} ok={}", outcome.is_ok());
    let expected = corpus_case("direct-cat-1");
    assert_eq!(calls, expected["executor_calls"]);
    assert_eq!(outcome.is_ok(), expected["outcome"] == "passed");
}

// --- corpus ---------------------------------------------------------------------

#[test]
fn corpus_lists_the_cases_these_tests_run() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(CORPUS);
    let events = std::fs::read(root.join("fixtures/events.json")).unwrap();
    let fixture: Value = serde_json::from_slice(&events).unwrap();
    let manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("source-manifest.json")).unwrap())
            .unwrap();
    assert_eq!(
        manifest["fixture_sha256"]["events.json"],
        format!("{:x}", sha2::Sha256::digest(&events))
    );
    assert_eq!(manifest["kind"], "synthetic");
    assert_eq!(fixture["kind"], "synthetic");
    assert_eq!(manifest["base_sha"], fixture["base_sha"]);
    let tests = include_str!("final_repair_early_completion.rs");
    for case in fixture["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let test = case["test"].as_str().unwrap();
        assert!(
            tests.contains(&format!("fn {test}(")),
            "{test} is not a test"
        );
        assert!(tests.contains(&format!("\"{id}\"")), "{id} is not run");
    }
    let text = String::from_utf8(events).unwrap()
        + &std::fs::read_to_string(root.join("source-manifest.json")).unwrap()
        + &std::fs::read_to_string(root.join("README.md")).unwrap();
    for forbidden in [
        "/Users/",
        "/home/",
        "/private/",
        "/Volumes/",
        "/tmp/",
        "sk-",
        "API_KEY=",
    ] {
        assert!(
            !text.contains(forbidden),
            "corpus must not hold {forbidden}"
        );
    }
}

fn config(root: PathBuf) -> Config {
    Config {
        workspace_root: root.clone(),
        state_dir: root.join("state"),
        eval_events_path: None,
        completion_contract_path: None,
        yes: true,
        offline: false,
        context_budget: 1000,
        model: "m".to_string(),
        provider: Provider::Ollama,
        tool_protocol: None,
        openai_api: commandagent::config::OpenAiApi::ChatCompletions,
        prompt_layout: commandagent::config::PromptLayout::Stable,
        plan_preset: commandagent::config::PlanPreset::None,
        intent_override: None,
        planner_model: "pm".to_string(),
        planner_provider: Provider::Gemini,
        planner_think: Some(commandagent::config::OllamaThink::False),
        classifier_model: "pm".to_string(),
        classifier_provider: Provider::Gemini,
        openai_compatible: None,
        ollama_host: "http://localhost:11434".to_string(),
        ollama_think: None,
        lm_studio_host: "http://localhost:1234".to_string(),
        num_predict: 100,
        max_iterations: 6,
        recovery_plan_auto_runs: 0,
        chat_timeout_secs: 1,
        chat_timeout_source: "override:test".to_string(),
        field_sources: commandagent::config::ConfigFieldSources::default(),
        chat_retries: 1,
        stream: false,
        resume: None,
        fresh_session: false,
        no_footer: false,
        narration: commandagent::config::NarrationMode::Normal,
        profile: "generic".to_string(),
        profile_explicit: false,
        profile_inference: None,
        style: "default".to_string(),
        action: Action::Repl,
    }
}

/// A scripted client that counts every chat call it receives.
#[derive(Clone)]
struct FakeClient {
    label: &'static str,
    replies: Arc<Mutex<Vec<AssistantReply>>>,
    calls: Arc<AtomicUsize>,
}

impl FakeClient {
    fn new(label: &'static str, replies: Vec<AssistantReply>) -> Self {
        Self {
            label,
            replies: Arc::new(Mutex::new(replies)),
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl ChatClient for FakeClient {
    fn label(&self) -> &str {
        self.label
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
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut replies = self.replies.lock().unwrap();
        if replies.is_empty() {
            anyhow::bail!("{} fake replies exhausted", self.label);
        }
        Ok(replies.remove(0))
    }
}

struct FakeUi;

impl InteractionUi for FakeUi {
    fn before_model_call(&self, _label: &str) -> UiGuard {
        UiGuard::noop()
    }

    fn before_tool_call(&self, _name: &str) -> UiGuard {
        UiGuard::noop()
    }

    fn publish_status(&self, _status: UiStatus) {}

    fn interrupted(&self) -> bool {
        false
    }
}
