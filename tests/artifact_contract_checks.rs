//! Issue #933 — `artifact_contract_check` through the normal manifest, pack,
//! and UltraPlan runner paths.
//!
//! This test binary owns its process environment: it sets the operator
//! registry root, a dummy API credential, `NODE_OPTIONS`, `NODE_PATH`, and the
//! local pack selection once, and serializes the runner cases with a lock. Every
//! adapter is a synthetic POSIX shell script run by the operator-pinned
//! `/bin/sh`; every root lives in a temporary directory. No case reaches a real
//! provider, the network, a model, Jev, or a factory. The synthetic cases are
//! also listed in `tests/corpus/apps/artifact-contract-check/`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use commandagent::config::{Action, Config, Provider};
use commandagent::planner::pack::{conform, conform_directory, exact_byte_hash, parse_bytes};
use commandagent::planner::profile::{ProfileId, ProfileRuntimeRegistry};
use commandagent::planner::step_plan::{PlanStep, StepPlan};
use commandagent::planner::ultra_plan::{UltraPhase, UltraPlan, render_ultra_plan};
use commandagent::providers::{AssistantReply, ChatClient};
use commandagent::state::{ConversationMessage, ToolCall};
use commandagent::tools::registry::ToolSpec;
use commandagent::tui::status::UiStatus;
use commandagent::tui::{InteractionUi, UiGuard};
use serde_json::{Value, json};
use sha2::Digest;

const CORPUS: &str = "tests/corpus/apps/artifact-contract-check";
const DRAFT: &str = "contract-draft";
const ENV_DRAFT: &str = "contract-env";
const LINES_ADAPTER: &str = "tally-lines";
const ENV_ADAPTER: &str = "env-probe";
const PACK_ID: &str = "contract-pack";
const DUMMY_CREDENTIAL_KEY: &str = "CA933_DUMMY_API_TOKEN";
const DUMMY_CREDENTIAL: &str = "ca933-dummy-credential-not-real";
const DUMMY_OPENAI: &str = "ca933-dummy-openai-value";
const DUMMY_NODE_OPTIONS: &str = "--require=/ca933/never-loaded.js";
const DUMMY_NODE_PATH: &str = "/ca933/never-resolved";
const REQUEST: &str = r#"{"protocol":"artifact-contract.request/v1","request_id":"notes-1","inputs":[{"input_id":"notes","path":"notes.txt"}]}"#;

const PRELUDE: &str = r#"IFS= read -r envelope || :
field() { value=${envelope#*\"$1\":\"}; printf '%s' "${value%%\"*}"; }
request_id=$(field request_id)
nonce=$(field nonce)
workspace=$(field workspace)
path=$(field path)
sha=$(field sha256)
"#;
const LINES_BODY: &str = r#"count=0
while IFS= read -r line || [ -n "$line" ]; do count=$((count + 1)); done < "$workspace/$path"
if [ "$count" -eq 3 ]; then status=passed; ok=1; bad=0; code=0; else status=failed; ok=0; bad=1; code=1; fi
printf '{"protocol":"artifact-contract.result/v1","request_id":"%s","nonce":"%s","input_digests":[{"input_id":"notes","sha256":"%s"}],"checks":[{"check_id":"lines/total","target_id":"notes.txt#all","status":"%s","counts":{"expected":1,"executed":1,"passed":%s,"failed":%s,"unresolved":0}}]}\n' "$request_id" "$nonce" "$sha" "$status" "$ok" "$bad"
exit "$code"
"#;
const ENV_BODY: &str = r#"export -p > "$1/env.txt"
printf '{"protocol":"artifact-contract.result/v1","request_id":"%s","nonce":"%s","input_digests":[{"input_id":"notes","sha256":"%s"}],"checks":[{"check_id":"env/probe","target_id":null,"status":"passed","counts":{"expected":null,"executed":1,"passed":1,"failed":0,"unresolved":0}}]}\n' "$request_id" "$nonce" "$sha"
exit 0
"#;

struct Fixture {
    root: PathBuf,
    config: PathBuf,
    extension: PathBuf,
    pack: PathBuf,
    pack_hash: String,
    observe: PathBuf,
}

/// Build every root once, set the process environment once, and register the
/// extension root before any case runs.
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
        std::fs::write(tooling.join("lines.sh"), format!("{PRELUDE}{LINES_BODY}")).unwrap();
        std::fs::write(tooling.join("env.sh"), format!("{PRELUDE}{ENV_BODY}")).unwrap();
        let members = json!({
            "lines.sh": sha256_file(&tooling.join("lines.sh")),
            "env.sh": sha256_file(&tooling.join("env.sh")),
        });
        for (id, entry, args, check) in [
            (LINES_ADAPTER, "lines.sh", json!([]), "lines/total"),
            (
                ENV_ADAPTER,
                "env.sh",
                json!([observe.to_string_lossy()]),
                "env/probe",
            ),
        ] {
            let targets = if id == LINES_ADAPTER {
                json!(["notes.txt#all"])
            } else {
                json!([])
            };
            let descriptor = json!({
                "protocol": "artifact-contract.adapter/v1",
                "adapter_id": id,
                "executable": {"path": shell(), "sha256": sha256_file(&shell())},
                "tooling_root": tooling,
                "entrypoint": entry,
                "args": args,
                "members": members,
                "required": [{"check_id": check, "targets": targets}],
                "inputs": [{"input_id": "notes"}]
            });
            std::fs::write(
                config.join(format!("adapters/{id}.json")),
                serde_json::to_vec_pretty(&descriptor).unwrap(),
            )
            .unwrap();
        }
        for (profile, adapter) in [(DRAFT, LINES_ADAPTER), (ENV_DRAFT, ENV_ADAPTER)] {
            let directory = extension.join(format!("profiles/{profile}"));
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(directory.join("manifest.toml"), manifest(profile, adapter)).unwrap();
        }
        let pack = extension.join(format!("packs/{PACK_ID}/1.0.0"));
        std::fs::create_dir_all(&pack).unwrap();
        let eval = pack_eval(DRAFT, LINES_ADAPTER, "final_acceptance", "");
        std::fs::write(pack.join("eval.yaml"), &eval).unwrap();
        let pack_hash = exact_byte_hash(None, Some(eval.as_bytes()));
        std::fs::write(pack.join("pack.sha256"), format!("{pack_hash}\n")).unwrap();

        // SAFETY: set once under the OnceLock before any case reads them; this
        // test binary owns its environment and no other test reads these keys
        // concurrently with this initialization.
        unsafe {
            std::env::set_var("COMMANDAGENT_ADAPTER_CONFIG_ROOT", &config);
            std::env::set_var(DUMMY_CREDENTIAL_KEY, DUMMY_CREDENTIAL);
            std::env::set_var("OPENAI_API_KEY", DUMMY_OPENAI);
            std::env::set_var("NODE_OPTIONS", DUMMY_NODE_OPTIONS);
            std::env::set_var("NODE_PATH", DUMMY_NODE_PATH);
        }
        commandagent::planner::extension_profiles::register(&extension).unwrap();
        Fixture {
            root,
            config,
            extension,
            pack,
            pack_hash,
            observe,
        }
    })
}

/// Validate a manifest through the same file entry `--validate-manifest` uses.
fn validate_manifest(source: &str) -> Result<(), String> {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join(format!("profiles/{DRAFT}"));
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("manifest.toml"), source).unwrap();
    commandagent::planner::profile_manifest::commands::validate_file(
        &directory.join("manifest.toml"),
    )
    .map_err(|error| format!("{error:#}"))
}

fn manifest(profile: &str, adapter: &str) -> String {
    format!(
        r#"[metadata]
id = "{profile}"
display_name = "Artifact contract {profile}"
schema_version = "v2"
task_family = "generic"
[plan]
intent = "create"
phases = [
  {{ id = "implementation", prompt = "Write the requested notes for {{goal}}." }},
  {{ id = "verification", prompt = "Recheck the notes for {{goal}}." }},
]
[artifacts]
required = ["notes.txt"]
[guidance.variants.contract]
triggers = [{{ condition = "always" }}]
messages = {{ instruction = "Write notes.txt and check/request.json." }}
[[checks.final]]
id = "artifact_contract_check"
params = {{ adapter_id = "{adapter}", request = "check/request.json" }}
"#
    )
}

fn pack_eval(profile: &str, adapter: &str, at: &str, extra: &str) -> String {
    format!(
        "schema_version: commandagent.pack.eval/v0\npack:\n  id: {PACK_ID}\n  version: 1.0.0\n  profile: {profile}\n  intent: create\nchecks:\n  - id: artifact_contract_check\n    at:\n      kind: {at}\n    params:\n      adapter_id: {adapter}\n      request: check/request.json\n{extra}"
    )
}

fn shell() -> PathBuf {
    std::fs::canonicalize("/bin/sh").unwrap()
}

fn sha256_file(path: &Path) -> String {
    format!("{:x}", sha2::Sha256::digest(std::fs::read(path).unwrap()))
}

/// Serializes the cases that install the pack selection.
fn runner_lock() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn events_named<'a>(events: &'a [Value], name: &str) -> Vec<&'a Value> {
    events
        .iter()
        .filter(|event| event["event"] == name)
        .collect()
}

fn read_events(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

#[test]
fn manifest_and_pack_paths_validate_capability_floor_params_and_registry() {
    let fixture = fixture();
    let valid = manifest(DRAFT, LINES_ADAPTER);
    assert!(validate_manifest(&valid).is_ok());
    for (label, source, reason) in [
        (
            "unknown adapter",
            valid.replace(LINES_ADAPTER, "not-registered"),
            "not registered",
        ),
        (
            "model-chosen executable",
            valid.replace("request = ", "executable = \"/bin/sh\", request = "),
            "unknown parameter `executable`",
        ),
        (
            "model-chosen argv",
            valid.replace("request = ", "argv = [\"node\", \"x.mjs\"], request = "),
            "unknown parameter `argv`",
        ),
        (
            "missing request",
            valid.replace(", request = \"check/request.json\"", ""),
            "missing parameter `request`",
        ),
        (
            "absolute request",
            valid.replace("check/request.json", "/etc/request.json"),
            "invalid parameter `request`",
        ),
        (
            "parent request",
            valid.replace("check/request.json", "../request.json"),
            "invalid parameter `request`",
        ),
        (
            "non-slug adapter",
            valid.replace(LINES_ADAPTER, "Tally Lines"),
            "invalid parameter `adapter_id`",
        ),
    ] {
        let error = validate_manifest(&source).unwrap_err();
        assert!(error.contains(reason), "{label}: {error}");
    }

    let report = conform_directory(&fixture.pack).unwrap();
    assert_eq!(report.status, "conformant");
    assert_eq!(report.effective_check_count, 1);
    assert_eq!(report.exact_byte_hash, fixture.pack_hash);

    for (label, eval, reason) in [
        (
            "phase binding",
            pack_eval(
                DRAFT,
                LINES_ADAPTER,
                "phase\n      id: build-verification",
                "",
            ),
            "only at final_acceptance",
        ),
        (
            "extra param",
            pack_eval(
                DRAFT,
                LINES_ADAPTER,
                "final_acceptance",
                "      argv: [node]\n",
            ),
            "unknown parameter `argv`",
        ),
        (
            "unknown adapter",
            pack_eval(DRAFT, "not-registered", "final_acceptance", ""),
            "not registered",
        ),
    ] {
        let error = parse_bytes(None, Some(eval.as_bytes()))
            .map(|_| ())
            .unwrap_err()
            .to_string();
        assert!(error.contains(reason), "{label}: {error}");
    }
    // The additive floor admits the capability only for an opt-in local draft.
    let compiled = pack_eval("nextjs", LINES_ADAPTER, "final_acceptance", "");
    let pack = parse_bytes(None, Some(compiled.as_bytes())).unwrap();
    let error = conform(&pack).unwrap_err().to_string();
    assert!(error.contains("additive pack check"), "{error}");
}

#[test]
fn unset_registry_refuses_the_profile_before_any_provider() {
    let fixture = fixture();
    let home = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let manifest_path = fixture
        .extension
        .join(format!("profiles/{DRAFT}/manifest.toml"));
    let validate = Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .arg("--validate-manifest")
        .arg(&manifest_path)
        .current_dir(workspace.path())
        .env_clear()
        .env("HOME", home.path())
        .output()
        .unwrap();
    assert!(!validate.status.success());
    assert!(
        String::from_utf8_lossy(&validate.stderr).contains("COMMANDAGENT_ADAPTER_CONFIG_ROOT"),
        "{}",
        String::from_utf8_lossy(&validate.stderr)
    );
    let run = Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .arg("--extension-root")
        .arg(&fixture.extension)
        .args(["--profile", DRAFT, "--prompt", "Write notes"])
        .current_dir(workspace.path())
        .env_clear()
        .env("HOME", home.path())
        .output()
        .unwrap();
    assert!(!run.status.success());
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(
        stderr.contains("COMMANDAGENT_ADAPTER_CONFIG_ROOT"),
        "{stderr}"
    );
    assert!(!stderr.contains("provider"), "{stderr}");
    assert_eq!(
        std::fs::read_dir(workspace.path()).unwrap().count(),
        0,
        "the refused run left files in the workspace"
    );
}

#[test]
fn dummy_credentials_and_node_variables_never_reach_the_adapter_child() {
    let fixture = fixture();
    let workspace = tempfile::tempdir().unwrap();
    write_notes(workspace.path(), "a\nb\nc\n");
    let events = workspace.path().join("run/events.jsonl");
    let runtime = ProfileRuntimeRegistry::resolve(&ProfileId::parse(ENV_DRAFT));
    let report = runtime.verify_final_with_events(workspace.path(), "Write notes", Some(&events));
    assert!(report.is_pass(), "{}", report.primary_reason());
    let dump = std::fs::read_to_string(fixture.observe.join("env.txt")).unwrap();
    for leaked in [
        DUMMY_CREDENTIAL_KEY,
        DUMMY_CREDENTIAL,
        "OPENAI_API_KEY",
        DUMMY_OPENAI,
        "NODE_OPTIONS",
        DUMMY_NODE_OPTIONS,
        "NODE_PATH",
        DUMMY_NODE_PATH,
        "COMMANDAGENT_ADAPTER_CONFIG_ROOT",
        "HOME",
    ] {
        assert!(
            !dump.contains(leaked),
            "{leaked} reached the child:\n{dump}"
        );
    }
    assert!(dump.contains("LANG") && dump.contains("TZ"), "{dump}");
    let recorded = read_events(&events);
    let receipt = events_named(&recorded, "artifact_contract_check_result")[0];
    assert_eq!(receipt["status"], "passed");
    assert_eq!(receipt["network_isolation"], "not_provided");
    assert_eq!(
        receipt["child_env_keys"],
        json!(["LANG", "TZ", "COMMANDAGENT_ADAPTER_TOOL_ROOT"])
    );
    assert!(
        !receipt
            .to_string()
            .contains(&*fixture.root.to_string_lossy())
    );
}

#[test]
fn native_failures_and_receipts_stay_separate_from_the_domain_outcome() {
    let _fixture = fixture();
    let workspace = tempfile::tempdir().unwrap();
    let runtime = ProfileRuntimeRegistry::resolve(&ProfileId::parse(DRAFT));

    // A passing adapter does not hide an earlier native failure: the required
    // artifact is missing, so the adapter has nothing to read and both fail.
    std::fs::create_dir_all(workspace.path().join("check")).unwrap();
    std::fs::write(workspace.path().join("check/request.json"), REQUEST).unwrap();
    let report = runtime.verify_final_with_events(workspace.path(), "Write notes", None);
    assert!(!report.is_pass());
    assert!(
        report
            .primary_reason()
            .contains("required artifact missing: notes.txt"),
        "{}",
        report.primary_reason()
    );

    // A known mismatch stays a native failure with domain none.
    write_notes(workspace.path(), "a\nb\n");
    let events = workspace.path().join("run/events.jsonl");
    let report = runtime.verify_final_with_events(workspace.path(), "Write notes", Some(&events));
    assert!(!report.is_pass());
    assert!(
        report
            .primary_reason()
            .contains("artifact contract check `tally-lines`")
    );
    let recorded = read_events(&events);
    let receipt = events_named(&recorded, "artifact_contract_check_result")[0];
    assert_eq!(receipt["status"], "failed");
    assert_eq!(receipt["domain_verdict"], "none");
    assert_eq!(receipt["known_mismatch"], true);

    // A pass adds nothing beyond the receipt: no report file is written into
    // the workspace, so no required artifact can depend on the host output.
    write_notes(workspace.path(), "a\nb\nc\n");
    let report = runtime.verify_final_with_events(workspace.path(), "Write notes", Some(&events));
    assert!(report.is_pass(), "{}", report.primary_reason());
    let mut entries = std::fs::read_dir(workspace.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    entries.sort();
    assert_eq!(entries, ["check", "notes.txt", "run"]);
    let summary = std::fs::read_to_string(workspace.path().join("run/summary.md")).unwrap();
    assert!(summary.contains("adapter `tally-lines` passed; domain partial (cap partial)"));
}

fn write_notes(root: &Path, notes: &str) {
    std::fs::create_dir_all(root.join("check")).unwrap();
    std::fs::write(root.join("notes.txt"), notes).unwrap();
    std::fs::write(root.join("check/request.json"), REQUEST).unwrap();
}

#[test]
fn normal_runner_binds_manifest_and_pack_receipts_and_keeps_draft_static() {
    let fixture = fixture();
    let _lock = runner_lock();

    // UltraPlan runner: the manifest check runs in profile final acceptance.
    let (events, output) = run_ultra("first\nsecond\nthird\n");
    let output = output.unwrap_or_else(|error| panic!("{error}\n{}", render(&events)));
    assert!(output.contains("ultra-plan-run complete"), "{output}");
    assert_receipts(&events, &["draft_profile"], "passed", "partial");
    let stop = events_named(&events, "tui_command_stop")[0];
    assert_eq!(stop["effective_profile"], DRAFT);
    assert_eq!(stop["assurance_level"], "static");
    assert_eq!(stop["assurance_reason"], "profile_not_admitted");

    // Step-plan runner with the local pack selected: the pack's final
    // acceptance check runs and binds its receipt.
    let (events, output) = run_step_plan(fixture, "first\nsecond\nthird\n");
    output.unwrap_or_else(|error| panic!("{error}\n{}", render(&events)));
    assert_receipts(&events, &["local_pack"], "passed", "partial");
    for receipt in events_named(&events, "artifact_contract_check_result") {
        assert_eq!(receipt["stopped_at"], "verified", "{receipt}");
        // Every required row passed: uncapped full, capped to partial.
        assert_eq!(receipt["domain_cap_applied"], true, "{receipt}");
    }
}

#[test]
fn normal_runner_keeps_an_adapter_mismatch_as_a_native_failure() {
    let fixture = fixture();
    let _lock = runner_lock();
    let (events, output) = run_step_plan(fixture, "first\nsecond\n");
    assert!(output.is_err(), "{}", render(&events));
    assert_receipts(&events, &["local_pack"], "failed", "none");

    let (events, _output) = run_ultra("first\nsecond\n");
    assert_receipts(&events, &["draft_profile"], "failed", "none");
    let final_acceptance = events_named(&events, "ultra_final_acceptance");
    assert!(!final_acceptance.is_empty(), "{}", render(&events));
    for event in final_acceptance {
        assert_ne!(event["final_acceptance_status"], "full_success", "{event}");
    }
    for stop in events_named(&events, "tui_command_stop") {
        assert_eq!(stop["ok"], false, "{stop}");
        assert_ne!(stop["assurance_level"], "full", "{stop}");
        assert_ne!(stop["assurance_level"], "partial", "{stop}");
    }
}

fn assert_receipts(events: &[Value], sources: &[&str], status: &str, domain: &str) {
    let receipts = events_named(events, "artifact_contract_check_result");
    let mut observed = receipts
        .iter()
        .map(|receipt| receipt["source"].as_str().unwrap())
        .collect::<Vec<_>>();
    observed.sort();
    observed.dedup();
    assert_eq!(observed, sources, "{}", render(events));
    for receipt in receipts {
        assert_eq!(receipt["status"], status, "{receipt}");
        assert_eq!(receipt["domain_verdict"], domain, "{receipt}");
        assert_eq!(receipt["native_effect"], "failure_only", "{receipt}");
    }
}

fn render(events: &[Value]) -> String {
    events
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

fn note_writes(notes: &str) -> AssistantReply {
    AssistantReply {
        content: String::new(),
        tool_calls: vec![
            ToolCall::new("Write", json!({"path": "notes.txt", "content": notes})),
            ToolCall::new(
                "Write",
                json!({"path": "check/request.json", "content": REQUEST}),
            ),
        ],
        prompt_tokens: None,
        completion_tokens: None,
    }
}

fn note_plan() -> StepPlan {
    StepPlan {
        goal: "Write the release notes and the adapter request".to_string(),
        steps: vec![PlanStep {
            id: "step-1".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Write the release notes and the adapter request".to_string(),
            expected_paths: vec!["notes.txt".to_string(), "check/request.json".to_string()],
            verify: Vec::new(),
        }],
    }
}

fn run_step_plan(fixture: &Fixture, notes: &str) -> (Vec<Value>, anyhow::Result<String>) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let events_path = root.join(".anvil/runs/artifact-contract-plan/events.jsonl");
    std::fs::write(
        root.join("plan.yaml"),
        commandagent::planner::step_plan::render_step_plan(&note_plan()),
    )
    .unwrap();
    let mut cfg = config(root.to_path_buf());
    cfg.eval_events_path = Some(events_path.clone());
    cfg.profile = DRAFT.to_string();
    cfg.profile_explicit = true;
    let mut execution = FakeClient::new("execution", vec![note_writes(notes)]);
    let selection = PackSelection::install(fixture);
    let output =
        commandagent::planner::run_plan_file(&mut execution, &root.join("plan.yaml"), &cfg);
    drop(selection);
    (read_events(&events_path), output)
}

fn run_ultra(notes: &str) -> (Vec<Value>, anyhow::Result<String>) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let events_path = root.join(".anvil/runs/artifact-contract/events.jsonl");
    let plan = UltraPlan {
        goal: "Write three short release notes".to_string(),
        profile: DRAFT.to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: vec![
            UltraPhase {
                id: "implementation".to_string(),
                prompt: "Write the notes and the adapter request.".to_string(),
            },
            UltraPhase {
                id: "verification".to_string(),
                prompt: "Rewrite the notes and the adapter request unchanged.".to_string(),
            },
        ],
    };
    std::fs::write(root.join("ultra.yaml"), render_ultra_plan(&plan)).unwrap();
    let mut cfg = config(root.to_path_buf());
    cfg.eval_events_path = Some(events_path.clone());
    cfg.profile = DRAFT.to_string();
    cfg.profile_explicit = true;
    let step_plan = serde_json::to_string(&note_plan()).unwrap();
    let mut planner = FakeClient::new(
        "planner",
        vec![
            AssistantReply::text(step_plan.clone()),
            AssistantReply::text(step_plan),
        ],
    );
    let mut execution = FakeClient::new("execution", vec![note_writes(notes), note_writes(notes)]);
    let output = commandagent::tui::slash::handle_command(
        "/run-ultra-plan ultra.yaml",
        &cfg,
        &mut planner,
        &mut execution,
        &FakeUi,
    );
    (read_events(&events_path), output)
}

/// Install the local pack selection for one runner case, as the CLI does.
struct PackSelection;

impl PackSelection {
    fn install(fixture: &Fixture) -> Self {
        // SAFETY: callers hold `runner_lock`, and only the runner cases in this
        // binary read the pack selection keys.
        unsafe {
            std::env::set_var("COMMANDAGENT_PACK_DIRECTORY", &fixture.pack);
            std::env::set_var("COMMANDAGENT_PACK_ID", PACK_ID);
            std::env::set_var("COMMANDAGENT_PACK_VERSION", "1.0.0");
            std::env::set_var("COMMANDAGENT_PACK_HASH", &fixture.pack_hash);
        }
        assert!(fixture.config.is_dir());
        Self
    }
}

impl Drop for PackSelection {
    fn drop(&mut self) {
        // SAFETY: the same `runner_lock` holder removes the keys it set.
        unsafe {
            for key in [
                "COMMANDAGENT_PACK_DIRECTORY",
                "COMMANDAGENT_PACK_ID",
                "COMMANDAGENT_PACK_VERSION",
                "COMMANDAGENT_PACK_HASH",
            ] {
                std::env::remove_var(key);
            }
        }
    }
}

#[test]
fn corpus_lists_the_synthetic_cases_these_tests_run() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(CORPUS);
    let cases: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("fixtures/cases.json")).unwrap())
            .unwrap();
    let manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("source-manifest.json")).unwrap())
            .unwrap();
    let digest = format!(
        "{:x}",
        sha2::Sha256::digest(std::fs::read(root.join("fixtures/cases.json")).unwrap())
    );
    assert_eq!(manifest["fixture_sha256"]["cases.json"], digest);
    assert_eq!(manifest["kind"], "synthetic");
    let ids = cases["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| {
            assert_eq!(case["kind"], "synthetic", "{case}");
            assert_ne!(case["domain_verdict"], "full", "false full in {case}");
            case["id"].as_str().unwrap().to_string()
        })
        .collect::<Vec<_>>();
    let tests = [
        include_str!("artifact_contract_checks.rs"),
        include_str!("../src/planner/artifact_contract_checks/tests.rs"),
    ]
    .concat();
    for case in cases["cases"].as_array().unwrap() {
        let test = case["test"].as_str().unwrap();
        assert!(
            tests.contains(&format!("fn {test}(")),
            "{test} is not a test"
        );
    }
    assert!(ids.len() >= 20, "{ids:?}");
    let text = std::fs::read_to_string(root.join("fixtures/cases.json")).unwrap()
        + &std::fs::read_to_string(root.join("source-manifest.json")).unwrap();
    for forbidden in [
        "/Users/",
        "/home/",
        "/private/",
        "/Volumes/",
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

#[derive(Clone)]
struct FakeClient {
    label: &'static str,
    replies: Arc<Mutex<Vec<AssistantReply>>>,
}

impl FakeClient {
    fn new(label: &'static str, replies: Vec<AssistantReply>) -> Self {
        Self {
            label,
            replies: Arc::new(Mutex::new(replies)),
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
