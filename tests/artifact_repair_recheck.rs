//! Issue #949 — a final-acceptance repair that changes the bytes of an
//! artifact an admitted `artifact_contract_check` inspected reruns the same
//! check inside the existing bounded repair, through the public UltraPlan
//! entry (`/run-ultra-plan`).
//!
//! This test binary owns its process environment: it sets the operator
//! registry root once and serializes the runner cases with a lock. The
//! adapter is a synthetic POSIX shell script run by the operator-pinned
//! `/bin/sh`; the planner and executor are scripted fake clients; every root
//! lives in a temporary directory. No case reaches a real provider, the
//! network, a model, or a credential. The expected event sequences live in
//! `tests/corpus/apps/artifact-repair-recheck/fixtures/events.json`; the
//! `base_sequence` of each case was observed at develop 4edfc5a7.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use commandagent::config::{Action, Config, Provider};
use commandagent::planner::ultra_plan::{UltraPhase, UltraPlan, render_ultra_plan};
use commandagent::providers::{AssistantReply, ChatClient};
use commandagent::state::{ConversationMessage, ToolCall};
use commandagent::tools::registry::ToolSpec;
use commandagent::tui::status::UiStatus;
use commandagent::tui::{InteractionUi, UiGuard};
use serde_json::{Value, json};
use sha2::Digest;

const CORPUS: &str = "tests/corpus/apps/artifact-repair-recheck";
const ADAPTER: &str = "state-gate";
const JSON_PROFILE: &str = "recheck-json";
const YAML_PROFILE: &str = "recheck-yaml";
const SPLIT_PROFILE: &str = "recheck-split";
const PLAIN_PROFILE: &str = "recheck-plain";
const REQUEST_PATH: &str = "check/request.json";
const REPAIR_MAX_ATTEMPTS: u64 = 2;

/// Reads the bound input line by line. `ready` passes; `unresolved` and
/// `notrun` report those required-row states; `crash` exits 3 with no
/// envelope; `hang` spins until the operator timeout; anything else fails.
/// Every start appends the request ID to the operator's observation file.
const ADAPTER_BODY: &str = r#"IFS= read -r envelope || :
field() { value=${envelope#*\"$1\":\"}; printf '%s' "${value%%\"*}"; }
request_id=$(field request_id)
nonce=$(field nonce)
workspace=$(field workspace)
path=$(field path)
sha=$(field sha256)
raw=$path; path=""
while :; do
  case $raw in
    *'\\'*) path="$path${raw%%\\\\*}\\"; raw=${raw#*\\\\} ;;
    *) path="$path$raw"; break ;;
  esac
done
printf '%s\n' "$request_id" >> "$1/calls"
verdict=failed
while IFS= read -r line || [ -n "$line" ]; do
  case $line in
    *state*ready*) verdict=passed ;;
    *state*unresolved*) verdict=unresolved ;;
    *state*notrun*) verdict=not_run ;;
    *state*crash*) exit 3 ;;
    *state*hang*) while :; do :; done ;;
  esac
done < "$workspace/$path"
case $verdict in
  passed) counts='"executed":1,"passed":1,"failed":0,"unresolved":0'; code=0 ;;
  unresolved) counts='"executed":1,"passed":0,"failed":0,"unresolved":1'; code=1 ;;
  not_run) counts='"executed":0,"passed":0,"failed":0,"unresolved":0'; code=1 ;;
  *) counts='"executed":1,"passed":0,"failed":1,"unresolved":0'; code=1 ;;
esac
printf '{"protocol":"artifact-contract.result/v1","request_id":"%s","nonce":"%s","input_digests":[{"input_id":"terms","sha256":"%s"}],"checks":[{"check_id":"terms/state","target_id":null,"status":"%s","counts":{"expected":1,%s}}]}\n' "$request_id" "$nonce" "$sha" "$verdict" "$counts"
exit "$code"
"#;

struct Fixture {
    observe: PathBuf,
}

/// Build the operator registry and the three generic draft profiles once,
/// set the registry variable once, and register the extension root.
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
        for (profile, declared) in [
            (JSON_PROFILE, "\"contract/terms.json\""),
            (YAML_PROFILE, "\"contract/terms.yaml\""),
            (SPLIT_PROFILE, "\"contract/schema.yaml\""),
        ] {
            let directory = extension.join(format!("profiles/{profile}"));
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(directory.join("manifest.toml"), manifest(profile, declared)).unwrap();
        }
        // A draft without an artifact check that requires a file no phase
        // writes, so its final acceptance fails on its own.
        let plain = extension.join(format!("profiles/{PLAIN_PROFILE}"));
        std::fs::create_dir_all(&plain).unwrap();
        let source = manifest(PLAIN_PROFILE, "\"notes/other.txt\"");
        let source = format!(
            "{}[[checks.final]]\nid = \"scaffold_files_present\"\nparams = {{ files = [\"notes/summary.md\"] }}\n",
            source.split("[[checks.final]]").next().unwrap()
        );
        std::fs::write(plain.join("manifest.toml"), source).unwrap();
        // SAFETY: set once under the OnceLock before any case reads it; this
        // test binary owns its environment.
        unsafe {
            std::env::set_var("COMMANDAGENT_ADAPTER_CONFIG_ROOT", &config);
        }
        commandagent::planner::extension_profiles::register(&extension).unwrap();
        Fixture { observe }
    })
}

fn manifest(profile: &str, declared: &str) -> String {
    format!(
        r#"[metadata]
id = "{profile}"
display_name = "Artifact recheck {profile}"
schema_version = "v2"
task_family = "generic"
[plan]
intent = "create"
phases = [
  {{ id = "implementation", prompt = "Write the requested contract for {{goal}}." }},
  {{ id = "verification", prompt = "Recheck the contract for {{goal}}." }},
]
[artifacts]
required = [{declared}]
[guidance.variants.contract]
triggers = [{{ condition = "always" }}]
messages = {{ instruction = "Write the contract files and {REQUEST_PATH}." }}
[[checks.final]]
id = "artifact_contract_check"
params = {{ adapter_id = "{ADAPTER}", request = "{REQUEST_PATH}" }}
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

/// The repair-relevant events in order, in the projection the corpus
/// fixture records.
fn projection(events: &[Value]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| {
            let text = |key: &str| event[key].as_str().unwrap_or_default().to_string();
            Some(match event["event"].as_str()? {
                "artifact_contract_check_result" => {
                    format!("receipt:{}:{}", text("status"), text("stopped_at"))
                }
                "final_acceptance_repair_start" => {
                    format!("repair_start:max={}", event["max_attempts"])
                }
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

fn request(request_id: &str, input: &str) -> String {
    let input = serde_json::to_string(input).unwrap();
    format!(
        r#"{{"protocol":"artifact-contract.request/v1","request_id":"{request_id}","inputs":[{{"input_id":"terms","path":{input}}}]}}"#
    )
}

fn writes(files: &[(&str, &str)]) -> AssistantReply {
    AssistantReply {
        content: String::new(),
        tool_calls: files
            .iter()
            .map(|(path, content)| {
                ToolCall::new("Write", json!({"path": path, "content": content}))
            })
            .collect(),
        prompt_tokens: None,
        completion_tokens: None,
    }
}

struct Case {
    id: &'static str,
    profile: &'static str,
    input: &'static str,
    /// Files the two plan phases write (besides the request).
    initial: Vec<(&'static str, &'static str)>,
    /// One reply per repair attempt.
    repairs: Vec<Vec<(&'static str, &'static str)>>,
}

struct Run {
    events: Vec<Value>,
    output: anyhow::Result<String>,
    executor_calls: usize,
    planner_calls: usize,
    adapter_calls: usize,
    workspace: tempfile::TempDir,
}

fn run(case: &Case) -> Run {
    run_in(case, tempfile::tempdir().unwrap())
}

/// Run `case` in an existing workspace, with its own events file.
fn run_in(case: &Case, dir: tempfile::TempDir) -> Run {
    let fixture = fixture();
    let _lock = runner_lock();
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
    let mut expected_paths = case
        .initial
        .iter()
        .map(|(path, _)| (*path).to_string())
        .collect::<Vec<_>>();
    expected_paths.push(REQUEST_PATH.to_string());
    let step_plan = json!({
        "goal": "Write the contract",
        "steps": [{
            "id": "step-1",
            "kind": "implement",
            "expected_result": "pass",
            "instruction": "Write the contract and the adapter request",
            "expected_paths": expected_paths,
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
    let request = request(case.id, case.input);
    let mut initial = case.initial.clone();
    initial.push((REQUEST_PATH, request.as_str()));
    let mut replies = vec![writes(&initial), writes(&initial)];
    replies.extend(case.repairs.iter().map(|files| writes(files)));
    let mut execution = FakeClient::new("execution", replies);
    let output = commandagent::tui::slash::handle_command(
        "/run-ultra-plan ultra.yaml",
        &cfg,
        &mut planner,
        &mut execution,
        &FakeUi,
    );
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

/// The observed sequence equals the corpus sequence, the counts equal the
/// corpus counts, and the existing repair limit and provider counters hold.
fn assert_case(case: &Case, run: &Run) {
    let expected = corpus_case(case.id);
    let observed = projection(&run.events);
    assert_eq!(
        observed,
        strings(&expected["sequence"]),
        "{}: sequence\n{}",
        case.id,
        render(&run.events)
    );
    let receipts = events_named(&run.events, "artifact_contract_check_result");
    assert_eq!(receipts.len() as u64, expected["receipts"], "{}", case.id);
    assert_eq!(
        run.adapter_calls,
        receipts.len(),
        "{}: adapter starts",
        case.id
    );
    for receipt in &receipts {
        assert_eq!(receipt["request_id"], case.id, "{receipt}");
        assert_eq!(receipt["source"], "draft_profile", "{receipt}");
        assert_eq!(receipt["native_effect"], "failure_only", "{receipt}");
        assert_ne!(receipt["domain_verdict"], "full", "{receipt}");
    }
    // The existing bounded repair is the only budget: its limit is unchanged,
    // no attempt exceeds it, and every repair turn is one counted provider
    // call of the existing executor.
    let starts = events_named(&run.events, "final_acceptance_repair_start");
    for start in &starts {
        assert_eq!(start["max_attempts"], REPAIR_MAX_ATTEMPTS, "{start}");
        assert_eq!(start["max_iterations"], 6, "{start}");
    }
    let completes = events_named(&run.events, "final_acceptance_repair_complete");
    assert_eq!(starts.len(), completes.len(), "{}", case.id);
    assert_eq!(
        completes.len() as u64,
        expected["repair_attempts"],
        "{}",
        case.id
    );
    assert!(completes.len() as u64 <= REPAIR_MAX_ATTEMPTS, "{}", case.id);
    // No field is added to or removed from the existing repair event.
    let base = corpus_case("json-same-bytes-1");
    let base_keys = |value: &Value| {
        value
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>()
    };
    for complete in &completes {
        assert_eq!(
            base_keys(complete),
            base_keys(&base["base_events"]["final_acceptance_repair_complete"]),
            "{complete}"
        );
        assert_eq!(complete["max_attempts"], REPAIR_MAX_ATTEMPTS, "{complete}");
        assert!(complete["attempt"].as_u64().unwrap() <= REPAIR_MAX_ATTEMPTS);
    }
    let repair_turns = run
        .events
        .iter()
        .filter(|event| {
            event["event"] == "provider_turn_duration" && event["caller_scope"] == "repair"
        })
        .count();
    assert_eq!(
        repair_turns,
        completes.len(),
        "{}: repair provider turns",
        case.id
    );
    assert_eq!(run.executor_calls, 2 + completes.len(), "{}", case.id);
    assert_eq!(run.planner_calls, 2, "{}", case.id);
    let passed = expected["outcome"] == "passed";
    assert_eq!(
        run.output.is_ok(),
        passed,
        "{}: {:?}",
        case.id,
        run.output.as_ref().err()
    );
    if passed {
        let output = run.output.as_ref().unwrap();
        assert!(output.contains("ultra-plan-run complete"), "{output}");
    }
}

/// The last receipt is bound to the bytes the workspace holds at the end.
fn assert_last_receipt_binds_final_bytes(case: &Case, run: &Run) {
    let receipts = events_named(&run.events, "artifact_contract_check_result");
    let last = receipts.last().unwrap();
    assert_eq!(
        last["input_digests"]["terms"],
        sha256_file(&run.workspace.path().join(case.input)),
        "{}: {last}",
        case.id
    );
}

fn json_case(id: &'static str, repairs: Vec<Vec<(&'static str, &'static str)>>) -> Case {
    Case {
        id,
        profile: JSON_PROFILE,
        input: "contract/terms.json",
        initial: vec![("contract/terms.json", "{\"state\": \"draft\"}\n")],
        repairs,
    }
}

#[test]
fn a_repaired_declared_json_contract_reruns_the_same_check() {
    let case = json_case(
        "json-ready-1",
        vec![vec![("contract/terms.json", "{\"state\": \"ready\"}\n")]],
    );
    let run = run(&case);
    assert_case(&case, &run);
    assert_last_receipt_binds_final_bytes(&case, &run);
    let receipts = events_named(&run.events, "artifact_contract_check_result");
    assert_ne!(receipts[0]["input_digests"], receipts[1]["input_digests"]);
    assert!(events_named(&run.events, "final_acceptance_repair_no_source_change").is_empty());
}

#[test]
fn a_repaired_declared_yaml_contract_reruns_the_same_check() {
    let case = Case {
        id: "yaml-ready-1",
        profile: YAML_PROFILE,
        input: "contract/terms.yaml",
        initial: vec![("contract/terms.yaml", "state: draft\n")],
        repairs: vec![vec![("contract/terms.yaml", "state: ready\n")]],
    };
    let run = run(&case);
    assert_case(&case, &run);
    assert_last_receipt_binds_final_bytes(&case, &run);
}

/// A same-bytes rewrite is reported by the session but is no change: the
/// run ends exactly as at the base, with byte-identical repair events.
#[test]
fn a_same_bytes_rewrite_reported_as_changed_does_not_recheck() {
    let case = json_case(
        "json-same-bytes-1",
        vec![vec![("contract/terms.json", "{\"state\": \"draft\"}\n")]],
    );
    let run = run(&case);
    assert_case(&case, &run);
    assert_unchanged_from_base(&case, &run);
}

/// A case the change must not touch: its sequence is the base sequence and
/// its repair events are byte-identical to the events observed at the base.
fn assert_unchanged_from_base(case: &Case, run: &Run) {
    let expected = corpus_case(case.id);
    assert_eq!(
        expected["sequence"], expected["base_sequence"],
        "{}",
        case.id
    );
    for name in [
        "final_acceptance_repair_complete",
        "final_acceptance_repair_no_source_change",
    ] {
        let observed = events_named(&run.events, name);
        assert_eq!(observed.len(), 1, "{}: {name}", case.id);
        assert_eq!(
            observed[0].to_string(),
            expected["base_events"][name].to_string(),
            "{}: {name} changed from the base",
            case.id
        );
    }
}

/// Every change reruns the check, but a rerun that still fails stays a
/// failure, and the third receipt is the last: the repair limit holds. The
/// first repair only reformats the JSON, which is a raw-byte change.
#[test]
fn a_rerun_that_still_fails_stays_failed_within_the_repair_limit() {
    let case = json_case(
        "json-still-failing-1",
        vec![
            vec![("contract/terms.json", "{\n  \"state\": \"draft\"\n}\n")],
            vec![("contract/terms.json", "{\"state\": \"blocked\"}\n")],
        ],
    );
    let run = run(&case);
    assert_case(&case, &run);
    assert_last_receipt_binds_final_bytes(&case, &run);
}

/// An adapter that exits without an envelope, and a required row reported
/// unresolved, keep the rerun a native failure.
#[test]
fn an_adapter_failure_or_unresolved_row_after_a_change_stays_failed() {
    let case = json_case(
        "json-crash-unresolved-1",
        vec![
            vec![("contract/terms.json", "{\"state\": \"crash\"}\n")],
            vec![("contract/terms.json", "{\"state\": \"unresolved\"}\n")],
        ],
    );
    let run = run(&case);
    assert_case(&case, &run);
    let receipts = events_named(&run.events, "artifact_contract_check_result");
    assert_eq!(receipts[2]["required_unresolved"], true, "{}", receipts[2]);
}

/// An adapter timeout and a required row reported not run keep the rerun a
/// native failure.
#[test]
fn an_adapter_timeout_or_not_run_row_after_a_change_stays_failed() {
    let case = json_case(
        "json-timeout-not-run-1",
        vec![
            vec![("contract/terms.json", "{\"state\": \"hang\"}\n")],
            vec![("contract/terms.json", "{\"state\": \"notrun\"}\n")],
        ],
    );
    let run = run(&case);
    assert_case(&case, &run);
    let receipts = events_named(&run.events, "artifact_contract_check_result");
    assert_eq!(
        receipts[1]["process"]["ended"], "timed_out",
        "{}",
        receipts[1]
    );
    assert_eq!(receipts[2]["required_not_run"], true, "{}", receipts[2]);
}

/// A declared artifact the check does not read is compared too: changing it
/// reruns the same check (which still fails on its own input), and a
/// same-bytes rewrite of it afterwards ends the repair as no change.
#[test]
fn a_changed_declared_artifact_outside_the_check_inputs_reruns_the_check() {
    let case = Case {
        id: "split-declared-1",
        profile: SPLIT_PROFILE,
        input: "data/terms.json",
        initial: vec![
            ("contract/schema.yaml", "version: 1\n"),
            ("data/terms.json", "{\"state\": \"draft\"}\n"),
        ],
        repairs: vec![
            vec![("contract/schema.yaml", "version: 2\n")],
            vec![("contract/schema.yaml", "version: 2\n")],
        ],
    };
    let run = run(&case);
    assert_case(&case, &run);
    assert_last_receipt_binds_final_bytes(&case, &run);
}

/// An input the admitted check bound, though no profile declares it, is
/// compared by its bytes, and an unrelated JSON the session also wrote and
/// reported is not.
#[test]
fn a_changed_bound_input_outside_the_declared_artifacts_reruns_the_check() {
    let case = Case {
        id: "split-bound-1",
        profile: SPLIT_PROFILE,
        input: "data/terms.json",
        initial: vec![
            ("contract/schema.yaml", "version: 1\n"),
            ("data/terms.json", "{\"state\": \"draft\"}\n"),
        ],
        repairs: vec![vec![
            ("data/terms.json", "{\"state\": \"ready\"}\n"),
            ("notes/unrelated.json", "{\"note\": 1}\n"),
        ]],
    };
    let run = run(&case);
    assert_case(&case, &run);
    assert_last_receipt_binds_final_bytes(&case, &run);
    let complete = events_named(&run.events, "final_acceptance_repair_complete")[0];
    assert!(
        strings(&complete["reported_changed_paths"]).contains(&"notes/unrelated.json".to_string()),
        "{complete}"
    );
}

/// B3: the check's input and a look-alike file it does not read lie side by
/// side; the input name is compared verbatim.
fn look_alike_case(id: &'static str, input: &'static str, repaired: &'static str) -> Case {
    Case {
        id,
        profile: SPLIT_PROFILE,
        input,
        initial: vec![
            ("contract/schema.yaml", "version: 1\n"),
            (input, "{\"state\": \"draft\"}\n"),
            ("data/terms.json", "{\"state\": \"draft\"}\n"),
        ],
        repairs: vec![vec![(repaired, "{\"state\": \"ready\"}\n")]],
    }
}

fn assert_look_alike_untouched(case: &Case, run: &Run) {
    if case.input != "data/terms.json" {
        assert_eq!(
            std::fs::read_to_string(run.workspace.path().join("data/terms.json")).unwrap(),
            "{\"state\": \"draft\"}\n",
            "{}",
            case.id
        );
    }
}

#[test]
fn a_repaired_trailing_space_input_reruns_the_check() {
    let case = look_alike_case("space-real-1", "data/terms.json ", "data/terms.json ");
    let run = run(&case);
    assert_case(&case, &run);
    assert_last_receipt_binds_final_bytes(&case, &run);
    assert_look_alike_untouched(&case, &run);
}

#[test]
fn a_repaired_look_alike_of_a_trailing_space_input_does_not_recheck() {
    let case = look_alike_case("space-decoy-1", "data/terms.json ", "data/terms.json");
    let run = run(&case);
    assert_case(&case, &run);
    assert_last_receipt_binds_final_bytes(&case, &run);
}

#[cfg(unix)]
#[test]
fn a_repaired_backslash_input_reruns_the_check() {
    let case = look_alike_case("backslash-real-1", "data\\terms.json", "data\\terms.json");
    let run = run(&case);
    assert_case(&case, &run);
    assert_last_receipt_binds_final_bytes(&case, &run);
    assert_look_alike_untouched(&case, &run);
}

#[cfg(unix)]
#[test]
fn a_repaired_look_alike_of_a_backslash_input_does_not_recheck() {
    let case = look_alike_case("backslash-decoy-1", "data\\terms.json", "data/terms.json");
    let run = run(&case);
    assert_case(&case, &run);
    assert_last_receipt_binds_final_bytes(&case, &run);
}

/// A report-only change to a JSON no check reads, with the inspected input
/// untouched, is no change.
#[test]
fn a_reported_change_to_an_unrelated_json_does_not_recheck() {
    let case = Case {
        id: "split-unrelated-1",
        profile: SPLIT_PROFILE,
        input: "data/terms.json",
        initial: vec![
            ("contract/schema.yaml", "version: 1\n"),
            ("data/terms.json", "{\"state\": \"draft\"}\n"),
        ],
        repairs: vec![vec![("notes/unrelated.json", "{\"note\": 1}\n")]],
    };
    let run = run(&case);
    assert_case(&case, &run);
    assert_unchanged_from_base(&case, &run);
}

/// A later run in a workspace an artifact run already used compares none of
/// the earlier run's inputs: its sequence is the corpus (and base) sequence.
fn assert_later_run_ignores_earlier_inputs(case: &Case, second: &Run, expect_receipts: usize) {
    let observed = projection(&second.events);
    let expected = corpus_case(case.id);
    assert_eq!(
        observed,
        strings(&expected["sequence"]),
        "{}: sequence\n{}",
        case.id,
        render(&second.events)
    );
    assert_eq!(
        expected["sequence"], expected["base_sequence"],
        "{}",
        case.id
    );
    assert_eq!(
        expected["receipts"].as_u64(),
        Some(expect_receipts as u64),
        "{}",
        case.id
    );
    assert_eq!(
        events_named(&second.events, "artifact_contract_check_result").len(),
        expect_receipts,
        "{observed:?}"
    );
    let completes = events_named(&second.events, "final_acceptance_repair_complete");
    assert_eq!(completes.len(), 1, "{}", render(&second.events));
    assert_eq!(completes[0]["changed_paths"], json!([]), "{observed:?}");
    assert_eq!(
        events_named(&second.events, "final_acceptance_repair_no_source_change").len(),
        1,
        "{observed:?}"
    );
}

#[test]
fn b1_a_later_run_without_a_check_does_not_compare_earlier_inputs() {
    let first = run(&json_case(
        "b1-first-plain-1",
        vec![vec![("contract/terms.json", "{\"state\": \"draft\"}\n")]],
    ));
    assert!(first.output.is_err());
    let case = Case {
        id: "b1-second-plain-1",
        profile: PLAIN_PROFILE,
        input: "contract/terms.json",
        initial: vec![("notes/other.txt", "x\n")],
        repairs: vec![vec![("contract/terms.json", "{\"state\": \"ready\"}\n")]],
    };
    let second = run_in(&case, first.workspace);
    assert_later_run_ignores_earlier_inputs(&case, &second, 0);
}

#[test]
fn b1_a_later_run_with_another_check_does_not_compare_earlier_inputs() {
    let first = run(&json_case(
        "b1-first-other-1",
        vec![vec![("contract/terms.json", "{\"state\": \"draft\"}\n")]],
    ));
    assert!(first.output.is_err());
    let case = Case {
        id: "b1-second-other-1",
        profile: SPLIT_PROFILE,
        input: "data/terms.json",
        initial: vec![
            ("contract/schema.yaml", "version: 1\n"),
            ("data/terms.json", "{\"state\": \"draft\"}\n"),
        ],
        repairs: vec![vec![("contract/terms.json", "{\"state\": \"ready\"}\n")]],
    };
    let second = run_in(&case, first.workspace);
    assert_later_run_ignores_earlier_inputs(&case, &second, 1);
}

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
    let tests = include_str!("artifact_repair_recheck.rs");
    let cases = fixture["cases"].as_array().unwrap();
    assert!(cases.len() >= 9, "{cases:?}");
    for case in cases {
        let id = case["id"].as_str().unwrap();
        let test = case["test"].as_str().unwrap();
        assert!(
            tests.contains(&format!("fn {test}(")),
            "{test} is not a test"
        );
        assert!(tests.contains(&format!("\"{id}\"")), "{id} is not run");
        assert!(case["receipts"].as_u64().unwrap() <= 1 + REPAIR_MAX_ATTEMPTS);
        assert!(case["repair_attempts"].as_u64().unwrap() <= REPAIR_MAX_ATTEMPTS);
        let sequence = strings(&case["sequence"]);
        let base = strings(&case["base_sequence"]);
        // At the base every case stopped before a second receipt; a case
        // whose profile runs no artifact check has none.
        let base_receipts = base
            .iter()
            .filter(|item| item.starts_with("receipt:"))
            .count();
        assert!(base_receipts <= 1, "{id}");
        assert_eq!(
            base_receipts == 0,
            case["receipts"] == 0,
            "{id}: a case without a receipt has none at the base either"
        );
        assert_eq!(sequence.first(), base.first(), "{id}");
        if case["outcome"] == "passed" {
            assert_eq!(
                sequence.last().map(String::as_str),
                Some("stop:true"),
                "{id}"
            );
        } else {
            assert_eq!(
                sequence.last().map(String::as_str),
                Some("stop:false"),
                "{id}"
            );
        }
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
