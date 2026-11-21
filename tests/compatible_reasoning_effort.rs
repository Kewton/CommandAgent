//! Issue #931: `COMMANDAGENT_COMPATIBLE_REASONING_EFFORT` declares a reasoning
//! effort for the roles that run on the shared OpenAI-compatible transport
//! (the generic `openai-compatible` override and LM Studio). Every assertion
//! is made on the body a loopback stand-in actually received; no case reaches
//! a real provider.
//!
//! The variable is read from the process environment when a client is built,
//! so each case runs in a child copy of this test binary whose environment is
//! set from the start (`child_case`); the parent only launches it and checks
//! that it passed. The corpus `tests/corpus/apps/compatible-reasoning-effort/`
//! drives the recorded and synthetic wire cases.

use std::ffi::OsString;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use commandagent::config::Config;
use commandagent::provider_call::{self, ProviderCallScope};
use commandagent::provider_cli;
use commandagent::state::ConversationMessage;
use commandagent::tools::registry::ToolRegistry;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const ENV: &str = "COMMANDAGENT_COMPATIBLE_REASONING_EFFORT";
const OPENAI_ENV: &str = "COMMANDAGENT_OPENAI_REASONING_EFFORT";
const CASE_ENV: &str = "COMMANDAGENT_ISSUE931_CHILD_CASE";
const CHILD_TEST: &str = "child_case";
const CORPUS: &str = "tests/corpus/apps/compatible-reasoning-effort";
const EXECUTOR_MODEL: &str = "executor-model-931";
const PLANNER_MODEL: &str = "planner-model-931";
const NUM_PREDICT: u64 = 64;
const DUMMY_KEY_ENV: &str = "COMMANDAGENT_ISSUE931_DUMMY_KEY";
const DUMMY_KEY: &str = "dummy-key-931-not-a-real-secret";
const ALLOWED: [&str; 7] = ["none", "minimal", "low", "medium", "high", "xhigh", "max"];

// ---------------------------------------------------------------- parent side

/// The effort variable for a child: unset, a UTF-8 value, or raw bytes.
enum Effort {
    Unset,
    Text(String),
    #[cfg(unix)]
    NonUnicode,
}

impl Effort {
    fn from_case(case: &Value) -> Self {
        #[cfg(unix)]
        if case["env_non_unicode"] == true {
            return Self::NonUnicode;
        }
        match case["env"].as_str() {
            Some(value) => Self::Text(value.to_string()),
            None => Self::Unset,
        }
    }

    fn value(&self) -> Option<OsString> {
        match self {
            Self::Unset => None,
            Self::Text(value) => Some(OsString::from(value)),
            #[cfg(unix)]
            Self::NonUnicode => {
                use std::os::unix::ffi::OsStringExt;
                Some(OsString::from_vec(b"lo\xffw".to_vec()))
            }
        }
    }
}

/// Run `case` in a child copy of this binary with only the variables it needs.
fn run_child(case: &Value) {
    let home = tempfile::tempdir().expect("child home");
    let mut command = Command::new(std::env::current_exe().expect("test binary"));
    command
        .args(["--exact", CHILD_TEST, "--nocapture", "--test-threads=1"])
        .env_clear()
        .env("HOME", home.path())
        .env(CASE_ENV, case.to_string())
        .current_dir(home.path());
    if let Some(value) = Effort::from_case(case).value() {
        command.env(ENV, value);
    }
    if let Some(value) = case["openai_env"].as_str() {
        command.env(OPENAI_ENV, value);
    }
    if layout_roles(case)
        .iter()
        .any(|(_, provider)| *provider == "openai")
    {
        command.env("OPENAI_API_KEY", "sk-dummy-931-not-real");
    }
    if case["scenario"] == "key" {
        command.env(DUMMY_KEY_ENV, DUMMY_KEY);
    }
    let output = command.output().expect("child process");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success() && stdout.contains("1 passed") && stdout.contains("ISSUE931_OK"),
        "case {} failed:\n{stdout}\n{stderr}",
        case["id"]
    );
}

fn wire_case(id: &str, api: &str, layout: &str, env: Option<&str>, expect: &str) -> Value {
    json!({
        "id": id,
        "api": api,
        "layout": layout,
        "env": env,
        "server": "ok",
        "expect": expect,
    })
}

fn wire_excerpt(api: &str, value: &str) -> Value {
    if api == "responses" {
        json!({"reasoning": {"effort": value}})
    } else {
        json!({"reasoning_effort": value})
    }
}

const APIS: [&str; 2] = ["chat_completions", "responses"];

#[test]
fn omitted_env_keeps_the_legacy_request_for_both_apis() {
    for api in APIS {
        for env in [None, Some(""), Some("  \t ")] {
            run_child(&wire_case("omitted", api, "generic-both", env, "absent"));
        }
    }
}

#[test]
fn every_accepted_value_reaches_the_api_specific_field_once() {
    for api in APIS {
        for value in ALLOWED {
            let mut case = wire_case("declared", api, "generic-both", Some(value), "declared");
            case["wire"] = wire_excerpt(api, value);
            run_child(&case);
        }
    }
}

#[test]
fn surrounding_whitespace_is_trimmed_before_it_is_sent() {
    for api in APIS {
        let mut case = wire_case(
            "trimmed",
            api,
            "lm-studio-both",
            Some("  low\t"),
            "declared",
        );
        case["wire"] = wire_excerpt(api, "low");
        run_child(&case);
    }
}

#[test]
fn invalid_values_fail_before_any_request_and_are_never_shown() {
    for api in APIS {
        for value in ["turbo-931-raw", "true", "false", "LOW", "Low", "x-high"] {
            run_child(&wire_case(
                "invalid",
                api,
                "generic-both",
                Some(value),
                "config-error",
            ));
        }
        #[cfg(unix)]
        {
            let mut case = wire_case("non-unicode", api, "lm-studio-both", None, "config-error");
            case["env_non_unicode"] = json!(true);
            run_child(&case);
        }
    }
}

#[test]
fn only_compatible_roles_send_the_effort_for_generic_and_lm_studio() {
    for api in APIS {
        for layout in [
            "generic-executor",
            "generic-planner",
            "generic-both",
            "lm-studio-executor",
            "lm-studio-planner",
            "lm-studio-both",
        ] {
            let mut case = wire_case("roles", api, layout, Some("high"), "declared");
            case["wire"] = wire_excerpt(api, "high");
            run_child(&case);
        }
    }
}

#[test]
fn non_applicable_roles_keep_their_wire_even_with_an_invalid_value() {
    for env in ["low", "turbo-931-raw"] {
        run_child(&wire_case(
            "native",
            "chat_completions",
            "native-only",
            Some(env),
            "absent",
        ));
    }
    let mut openai = wire_case(
        "openai-executor-invalid",
        "chat_completions",
        "openai-executor-native-planner",
        Some("turbo-931-raw"),
        "absent",
    );
    openai["openai_env"] = json!("high");
    run_child(&openai);
}

#[test]
fn openai_and_compatible_efforts_stay_independent_across_mixed_roles() {
    for api in APIS {
        let mut both = wire_case(
            "mixed",
            api,
            "openai-executor-generic-planner",
            Some("low"),
            "declared",
        );
        both["openai_env"] = json!("high");
        both["wire"] = wire_excerpt(api, "low");
        run_child(&both);

        let mut openai_only = wire_case(
            "openai-only",
            api,
            "openai-executor-generic-planner",
            None,
            "absent",
        );
        openai_only["openai_env"] = json!("high");
        run_child(&openai_only);
    }
}

#[test]
fn server_rejection_is_kept_without_retry_or_downgrade() {
    for api in APIS {
        let mut case = wire_case("reject", api, "generic-both", Some("medium"), "declared");
        case["model"] = json!("glm-5.3-flash:cloud");
        case["server"] = json!("reject-400");
        case["wire"] = wire_excerpt(api, "medium");
        run_child(&case);
    }
}

#[test]
fn transport_retry_resends_the_same_snapshot() {
    for api in APIS {
        for layout in ["generic-executor", "lm-studio-planner"] {
            let mut case = wire_case("retry", api, layout, Some("max"), "declared");
            case["server"] = json!("retry-503");
            case["wire"] = wire_excerpt(api, "max");
            run_child(&case);
        }
    }
}

#[test]
fn replies_tools_usage_and_responses_replay_are_preserved() {
    for api in APIS {
        for env in [None, Some("low")] {
            let mut case = wire_case("replay", api, "generic-executor", env, "replay");
            case["scenario"] = json!("replay");
            run_child(&case);
        }
    }
}

#[test]
fn dummy_api_key_stays_in_the_authorization_header() {
    for api in APIS {
        let mut case = wire_case("key", api, "generic-executor", Some("low"), "key");
        case["scenario"] = json!("key");
        run_child(&case);
    }
}

#[test]
fn corpus_cases_match_the_wire_contract() {
    let corpus = corpus();
    let cases = corpus["cases"].as_array().expect("cases");
    assert!(cases.iter().any(|case| case["kind"] == "recorded"));
    assert!(cases.iter().any(|case| case["kind"] == "synthetic"));
    for case in cases {
        let kind = case["kind"].as_str().expect("kind");
        assert!(matches!(kind, "recorded" | "synthetic"), "{case}");
        if kind == "recorded" {
            assert!(case["source_file"].is_string(), "{case}");
        }
        run_child(case);
    }
}

#[test]
fn corpus_manifest_pins_the_fixture_and_keeps_only_minimal_excerpts() {
    let root = corpus_root();
    let fixture = std::fs::read(root.join("fixtures/cases.json")).unwrap();
    let digest = format!("{:x}", Sha256::digest(&fixture));
    let manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("source-manifest.json")).unwrap())
            .unwrap();
    assert_eq!(manifest["fixture_sha256"]["cases.json"], digest);
    let recorded = manifest["recorded_source"]["requests"]
        .as_object()
        .expect("recorded request digests");
    let corpus = corpus();
    for case in corpus["cases"].as_array().unwrap() {
        if case["kind"] == "recorded" {
            let file = case["source_file"].as_str().unwrap();
            let hash = recorded[file].as_str().expect("recorded hash");
            assert_eq!(hash.len(), 64, "{file}");
        }
    }
    let text = String::from_utf8(fixture).unwrap();
    for forbidden in [
        "/Volumes",
        "/Users",
        "127.0.0.1",
        "11434",
        "http",
        "authorization",
        "Bearer",
        "messages",
        "\"input\"",
        "instructions",
        "heldout",
    ] {
        assert!(
            !text.contains(forbidden),
            "corpus must not copy {forbidden}"
        );
    }
}

#[test]
fn doctor_reports_declared_omitted_invalid_and_not_applicable_without_values() {
    let server = Server::spawn(Script::Ok);
    let base_url = format!("http://{}/v1", server.address);
    let compatible = [
        "--provider",
        "openai-compatible",
        "--base-url",
        base_url.as_str(),
        "--model",
        EXECUTOR_MODEL,
        "--planner-provider",
        "ollama",
        "--planner-model",
        PLANNER_MODEL,
    ];
    let native = ["--provider", "ollama", "--model", EXECUTOR_MODEL];

    let declared = doctor(&compatible, Some(" low "), true);
    let check = effort_check(&declared.stdout);
    assert_eq!(check["status"], "pass");
    assert_eq!(check["details"]["status"], "declared");
    assert_eq!(check["details"]["requested_value"], "low");
    assert_eq!(check["details"]["source"], "env");
    assert_eq!(check["details"]["server_support_verified"], false);
    assert_eq!(check["details"]["applicable_roles"], json!(["executor"]));
    assert_eq!(check["details"]["roles"][1]["status"], "not_applicable");

    let omitted = doctor(&compatible, None, true);
    let check = effort_check(&omitted.stdout);
    assert_eq!(check["status"], "pass");
    assert_eq!(check["details"]["status"], "omitted");
    assert!(check["details"]["requested_value"].is_null());

    let rejected = "Turbo-931-raw";
    let invalid = doctor(&compatible, Some(rejected), true);
    assert!(!invalid.success, "an invalid declaration fails doctor");
    let mut added_failures = failed_ids(&invalid.stdout);
    added_failures.retain(|id| !failed_ids(&omitted.stdout).contains(id));
    assert_eq!(
        added_failures,
        vec!["config.compatible_reasoning_effort".to_string()],
        "only the effort check turns red"
    );
    let check = effort_check(&invalid.stdout);
    assert_eq!(check["status"], "fail");
    assert_eq!(check["details"]["status"], "invalid");
    assert!(!invalid.stdout.contains(rejected), "{}", invalid.stdout);
    assert!(!invalid.stderr.contains(rejected), "{}", invalid.stderr);

    let human = doctor(&compatible, Some(rejected), false);
    assert!(human.stdout.contains("Compatible reasoning effort"));
    assert!(human.stdout.contains(ENV), "{}", human.stdout);
    assert!(!human.stdout.contains(rejected), "{}", human.stdout);
    let human = doctor(&compatible, Some("xhigh"), false);
    assert!(
        human.stdout.contains("xhigh (declared via") && human.stdout.contains("not verified"),
        "{}",
        human.stdout
    );

    let not_applicable = doctor(&native, Some(rejected), true);
    let check = effort_check(&not_applicable.stdout);
    assert_eq!(check["status"], "pass");
    assert_eq!(check["details"]["status"], "not_applicable");
    assert_eq!(check["details"]["setting"], "invalid");
    assert!(!not_applicable.stdout.contains(rejected));
    let requests = server.finish();
    assert!(
        requests
            .iter()
            .all(|request| request.line.starts_with("GET ")),
        "doctor must not send a generation request"
    );
}

struct DoctorOutput {
    success: bool,
    stdout: String,
    stderr: String,
}

fn doctor(arguments: &[&str], effort: Option<&str>, json_output: bool) -> DoctorOutput {
    let workspace = tempfile::tempdir().unwrap();
    let home = workspace.path().join("home");
    let state = workspace.path().join("state");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&state).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_commandagent"));
    command.arg("--doctor");
    if json_output {
        command.arg("--json");
    }
    command
        .args(arguments)
        .args(["--ollama-host", "http://127.0.0.1:9"])
        .args(["--cwd", workspace.path().to_str().unwrap()])
        .args(["--state-dir", state.to_str().unwrap()])
        .env_clear()
        .env("HOME", &home)
        .current_dir(workspace.path());
    if let Some(effort) = effort {
        command.env(ENV, effort);
    }
    let output = command.output().unwrap();
    DoctorOutput {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn failed_ids(stdout: &str) -> Vec<String> {
    let report: Value = serde_json::from_str(stdout).expect("doctor JSON");
    report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|check| check["status"] == "fail")
        .filter_map(|check| check["id"].as_str().map(str::to_string))
        .collect()
}

fn effort_check(stdout: &str) -> Value {
    let report: Value = serde_json::from_str(stdout).expect("doctor JSON");
    report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == "config.compatible_reasoning_effort")
        .cloned()
        .expect("compatible effort check")
}

fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(CORPUS)
}

fn corpus() -> Value {
    let path = corpus_root().join("fixtures/cases.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

// ----------------------------------------------------------------- child side

/// Runs one case when launched by `run_child`; a no-op in a normal test run.
#[test]
fn child_case() {
    let Some(case) = std::env::var_os(CASE_ENV) else {
        return;
    };
    let case: Value = serde_json::from_str(case.to_str().expect("case JSON")).expect("case");
    match case["scenario"].as_str() {
        Some("replay") => run_replay_case(&case),
        Some("key") => run_key_case(&case),
        _ => run_wire_case(&case),
    }
    println!("ISSUE931_OK");
}

fn layout_roles(case: &Value) -> Vec<(&'static str, &'static str)> {
    let layout = case["layout"].as_str().expect("layout");
    let (executor, planner) = match layout {
        "generic-executor" => ("generic", "ollama"),
        "generic-planner" => ("ollama", "generic"),
        "generic-both" => ("generic", "generic"),
        "lm-studio-executor" => ("lm-studio", "ollama"),
        "lm-studio-planner" => ("ollama", "lm-studio"),
        "lm-studio-both" => ("lm-studio", "lm-studio"),
        "native-only" => ("ollama", "ollama"),
        "openai-executor-generic-planner" => ("openai", "generic"),
        "openai-executor-native-planner" => ("openai", "ollama"),
        other => panic!("unknown layout {other}"),
    };
    vec![("executor", executor), ("planner", planner)]
}

fn is_compatible(provider: &str) -> bool {
    matches!(provider, "generic" | "lm-studio")
}

fn role_model(case: &Value, role: &str) -> String {
    if role == "executor" {
        case["model"].as_str().unwrap_or(EXECUTOR_MODEL).to_string()
    } else {
        PLANNER_MODEL.to_string()
    }
}

fn config_for(case: &Value, compatible: SocketAddr, ollama: SocketAddr, root: &Path) -> Config {
    let roles = layout_roles(case);
    let flag = |provider: &'static str| -> &'static str {
        match provider {
            "generic" => "openai-compatible",
            other => other,
        }
    };
    let retries = match case["server"].as_str() {
        Some("reject-400") => "2",
        Some("retry-503") => "1",
        _ => "0",
    };
    let mut arguments = vec![
        "commandagent".to_string(),
        "--cwd".to_string(),
        root.to_string_lossy().into_owned(),
        "--state-dir".to_string(),
        root.join("state").to_string_lossy().into_owned(),
        "--provider".to_string(),
        flag(roles[0].1).to_string(),
        "--model".to_string(),
        role_model(case, "executor"),
        "--planner-provider".to_string(),
        flag(roles[1].1).to_string(),
        "--planner-model".to_string(),
        role_model(case, "planner"),
        "--ollama-host".to_string(),
        format!("http://{ollama}"),
        "--num-predict".to_string(),
        NUM_PREDICT.to_string(),
        "--chat-timeout-secs".to_string(),
        "10".to_string(),
        "--chat-retries".to_string(),
        retries.to_string(),
    ];
    if roles.iter().any(|(_, provider)| *provider == "generic") {
        arguments.extend(["--base-url".to_string(), format!("http://{compatible}/v1")]);
    }
    if roles.iter().any(|(_, provider)| *provider == "lm-studio") {
        arguments.extend([
            "--lm-studio-host".to_string(),
            format!("http://{compatible}"),
        ]);
    }
    if roles.iter().any(|(_, provider)| *provider != "ollama") {
        arguments.extend([
            "--api".to_string(),
            case["api"].as_str().unwrap().replace('_', "-"),
        ]);
    }
    if case["scenario"] == "key" {
        arguments.extend(["--api-key-env".to_string(), DUMMY_KEY_ENV.to_string()]);
    }
    let parsed = provider_cli::parse_from(arguments).expect("arguments");
    let mut config = Config::from_cli_with_provider_options(parsed.cli, parsed.provider_options)
        .expect("config");
    config.eval_events_path = Some(root.join("events.jsonl"));
    config
}

/// The request the compatible transport sent before Issue #931 for one user
/// turn without tools.
fn legacy_body(api: &str, model: &str) -> Value {
    if api == "responses" {
        json!({
            "model": model,
            "input": [{"role": "user", "content": [{"type": "input_text", "text": "hello"}]}],
            "max_output_tokens": NUM_PREDICT,
        })
    } else {
        json!({
            "model": model,
            "messages": [{"role": "user", "content": "hello"}],
            "max_tokens": NUM_PREDICT,
        })
    }
}

fn effort_fields() -> [&'static str; 2] {
    ["reasoning_effort", "reasoning"]
}

fn run_wire_case(case: &Value) {
    let api = case["api"].as_str().expect("api");
    let expect = case["expect"].as_str().expect("expect");
    let script = match case["server"].as_str() {
        Some("reject-400") => Script::Reject,
        Some("retry-503") => Script::RetryFirst,
        _ => Script::Ok,
    };
    let dir = tempfile::tempdir().expect("tempdir");
    let compatible = Server::spawn(script);
    let ollama = Server::spawn(Script::Ok);
    let config = config_for(case, compatible.address, ollama.address, dir.path());
    let messages = [ConversationMessage::user("hello")];
    let roles = layout_roles(case);
    let mut compatible_calls = Vec::new();
    for (role, provider) in &roles {
        let planner = *role == "planner";
        let model = role_model(case, role);
        let built = commandagent::providers::client_from_config(&config, planner);
        if expect == "config-error" && is_compatible(provider) {
            let error = format!("{:#}", built.err().expect("invalid value must fail"));
            assert!(error.contains(ENV), "{error}");
            assert!(error.contains("xhigh"), "{error}");
            if let Some(raw) = case["env"].as_str() {
                assert!(!error.contains(raw), "raw value leaked: {error}");
            }
            continue;
        }
        let mut client = built.unwrap_or_else(|error| panic!("{role} client: {error:#}"));
        if *provider == "openai" {
            // Built only: the OpenAI client has a fixed public endpoint, so no
            // request is sent. Building it must not depend on the new variable.
            assert_eq!(client.label(), "openai");
            continue;
        }
        let scope = if planner {
            ProviderCallScope::PlannerStep
        } else {
            ProviderCallScope::Executor
        };
        let outcome = provider_call::chat(
            client.as_mut(),
            &config,
            scope,
            &model,
            &messages,
            &[],
            false,
        );
        if case["server"] == "reject-400" && is_compatible(provider) {
            let error = format!("{:#}", outcome.result.expect_err("400 must fail"));
            assert!(error.contains("400"), "{error}");
        } else {
            outcome
                .result
                .unwrap_or_else(|error| panic!("{role} turn: {error:#}"));
            // A clone made after the turn sends the same snapshot.
            let mut clone = client.boxed_clone();
            clone
                .chat(&model, &messages, &[], false)
                .unwrap_or_else(|error| panic!("{role} clone: {error:#}"));
        }
        if is_compatible(provider) {
            compatible_calls.push((*role, model));
        }
    }
    let compatible_requests = compatible.finish();
    let ollama_requests = ollama.finish();

    if expect == "config-error" {
        assert!(
            compatible_requests.is_empty(),
            "an invalid value must stop before any request: {}",
            compatible_requests.len()
        );
    }
    // Per compatible role: the worker turn plus the clone turn, except that a
    // rejected turn stops the role, and a 503 adds one transport retry.
    let (per_role_requests, per_role_events) = match case["server"].as_str() {
        Some("reject-400") => (1, 1),
        Some("retry-503") => (3, 2),
        _ => (2, 2),
    };
    assert_eq!(
        compatible_requests.len(),
        compatible_calls.len() * per_role_requests,
        "compatible request count"
    );
    let endpoint = if api == "responses" {
        "POST /v1/responses "
    } else {
        "POST /v1/chat/completions "
    };
    for (role, model) in &compatible_calls {
        let bodies = compatible_requests
            .iter()
            .filter(|request| request.body["model"] == model.as_str())
            .collect::<Vec<_>>();
        assert_eq!(bodies.len(), per_role_requests, "{role} requests");
        for request in bodies {
            assert!(request.line.starts_with(endpoint), "{}", request.line);
            assert!(request.body.get("think").is_none(), "{}", request.body);
            let legacy = legacy_body(api, model);
            match expect {
                "declared" => {
                    let wire = case["wire"].as_object().expect("wire excerpt");
                    assert_eq!(wire.len(), 1, "one effort field");
                    for (field, value) in wire {
                        assert_eq!(&request.body[field], value, "{role}: {}", request.body);
                    }
                    let mut rest = request.body.clone();
                    for field in effort_fields() {
                        if !wire.contains_key(field) {
                            assert!(rest.get(field).is_none(), "{}", request.body);
                        }
                        rest.as_object_mut().unwrap().remove(field);
                    }
                    assert_eq!(rest, legacy, "{role}: only the effort field is added");
                }
                _ => assert_eq!(request.body, legacy, "{role}: legacy request"),
            }
        }
    }
    for request in &ollama_requests {
        for field in effort_fields() {
            assert!(request.body.get(field).is_none(), "{}", request.body);
        }
        let expected_think = if request.body["model"] == PLANNER_MODEL {
            Some(json!(false))
        } else {
            None
        };
        assert_eq!(request.body.get("think").cloned(), expected_think);
    }
    let native_roles = roles
        .iter()
        .filter(|(_, provider)| *provider == "ollama")
        .count();
    assert_eq!(ollama_requests.len(), native_roles * 2, "native requests");

    let events = provider_request_events(dir.path());
    assert_eq!(
        events.len(),
        compatible_calls.len() * per_role_events,
        "provider_request events"
    );
    for event in &events {
        let role = compatible_calls
            .iter()
            .find(|(_, model)| event["model"] == model.as_str())
            .map(|(role, _)| *role)
            .expect("event for a compatible role");
        if expect == "declared" {
            let wire = &case["wire"];
            let value = wire
                .get("reasoning_effort")
                .or_else(|| wire["reasoning"].get("effort"))
                .expect("declared value");
            assert_eq!(&event["requested_reasoning_effort"], value, "{event}");
            assert_eq!(event["reasoning_effort_source"], "env", "{event}");
            assert_eq!(event["provider_role"], role, "{event}");
        } else {
            for field in [
                "requested_reasoning_effort",
                "reasoning_effort_source",
                "provider_role",
            ] {
                assert!(event.get(field).is_none(), "{event}");
            }
        }
    }
}

fn provider_request_events(root: &Path) -> Vec<Value> {
    std::fs::read_to_string(root.join("events.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|event| event["event"] == "provider_request")
        .collect()
}

fn run_replay_case(case: &Value) {
    let api = case["api"].as_str().unwrap();
    let declared = case["env"].as_str();
    let dir = tempfile::tempdir().expect("tempdir");
    let compatible = Server::spawn(Script::ToolThenText);
    let ollama = Server::spawn(Script::Ok);
    let config = config_for(case, compatible.address, ollama.address, dir.path());
    let registry = ToolRegistry::default();
    let mut client = commandagent::providers::client_from_config(&config, false).expect("client");
    let first_messages = [ConversationMessage::user("Read notes.md")];
    let first = client
        .chat(EXECUTOR_MODEL, &first_messages, registry.specs(), true)
        .expect("tool turn");
    assert_eq!(first.content, "");
    assert_eq!(first.tool_calls.len(), 1);
    assert_eq!(first.tool_calls[0].id, "call-931");
    assert_eq!(first.tool_calls[0].name, "Read");
    assert_eq!(first.tool_calls[0].arguments["path"], "notes.md");
    assert_eq!(first.prompt_tokens, Some(40));
    assert_eq!(first.completion_tokens, Some(6));
    let metadata = client.take_response_metadata().expect("metadata");
    assert_eq!(metadata.cached_input_tokens, Some(32));
    assert_eq!(metadata.model_id.as_deref(), Some(EXECUTOR_MODEL));

    let second_messages = [
        ConversationMessage::user("Read notes.md"),
        ConversationMessage::assistant(first.content.clone(), first.tool_calls.clone()),
        ConversationMessage::tool_result("Read", Some("call-931"), "notes"),
    ];
    let second = client
        .chat(EXECUTOR_MODEL, &second_messages, registry.specs(), true)
        .expect("final turn");
    assert_eq!(second.content, "done");
    assert!(second.tool_calls.is_empty());

    let requests = compatible.finish();
    assert_eq!(requests.len(), 2);
    for request in &requests {
        assert!(request.body.get("tools").is_some(), "{}", request.body);
        match (api, declared) {
            ("responses", Some(value)) => {
                assert_eq!(request.body["reasoning"], json!({"effort": value}));
                assert!(request.body.get("reasoning_effort").is_none());
            }
            (_, Some(value)) => {
                assert_eq!(request.body["reasoning_effort"], value);
                assert!(request.body.get("reasoning").is_none());
            }
            (_, None) => {
                for field in effort_fields() {
                    assert!(request.body.get(field).is_none(), "{}", request.body);
                }
            }
        }
        if api == "responses" {
            assert_eq!(request.body["max_output_tokens"], NUM_PREDICT);
            assert!(request.body.get("store").is_none());
            assert!(request.body.get("include").is_none());
        } else {
            assert_eq!(request.body["max_tokens"], NUM_PREDICT);
            assert!(request.body.get("max_completion_tokens").is_none());
        }
    }
    let second_body = &requests[1].body;
    if api == "responses" {
        let input = second_body["input"].as_array().expect("input");
        assert!(
            input
                .iter()
                .any(|item| item["type"] == "reasoning" && item["id"] == "rs-931"),
            "reasoning item replayed: {second_body}"
        );
        assert!(
            input
                .iter()
                .any(|item| item["type"] == "function_call" && item["call_id"] == "call-931"),
            "{second_body}"
        );
        assert!(
            input
                .iter()
                .any(|item| item["type"] == "function_call_output"
                    && item["call_id"] == "call-931"),
            "{second_body}"
        );
    } else {
        let messages = second_body["messages"].as_array().expect("messages");
        assert_eq!(messages[1]["tool_calls"][0]["id"], "call-931");
        assert_eq!(messages[2]["role"], "tool");
        assert_eq!(messages[2]["tool_call_id"], "call-931");
    }
}

fn run_key_case(case: &Value) {
    let dir = tempfile::tempdir().expect("tempdir");
    let compatible = Server::spawn(Script::Reject);
    let ollama = Server::spawn(Script::Ok);
    let config = config_for(case, compatible.address, ollama.address, dir.path());
    let mut client = commandagent::providers::client_from_config(&config, false).expect("client");
    let error = client
        .chat(
            EXECUTOR_MODEL,
            &[ConversationMessage::user("hello")],
            &[],
            false,
        )
        .expect_err("400");
    let requests = compatible.finish();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert!(
        request
            .head
            .contains(&format!("authorization: bearer {DUMMY_KEY}")),
        "the key belongs in the header"
    );
    assert!(!request.body.to_string().contains(DUMMY_KEY));
    assert!(!format!("{error:#}").contains(DUMMY_KEY));
    let events = std::fs::read_to_string(dir.path().join("events.jsonl")).unwrap_or_default();
    assert!(!events.contains(DUMMY_KEY), "{events}");
    assert!(
        !events.contains("hello"),
        "events must not carry the prompt"
    );
    assert!(
        events.contains("\"requested_reasoning_effort\":\"low\""),
        "{events}"
    );
}

// ------------------------------------------------------------ loopback server

#[derive(Clone, Copy)]
enum Script {
    Ok,
    Reject,
    RetryFirst,
    ToolThenText,
}

struct Received {
    line: String,
    head: String,
    body: Value,
}

struct Server {
    address: SocketAddr,
    stop: Arc<AtomicBool>,
    handle: JoinHandle<Vec<Received>>,
}

impl Server {
    fn spawn(script: Script) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address");
        listener.set_nonblocking(true).expect("nonblocking");
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let handle = std::thread::spawn(move || {
            let received = Arc::new(Mutex::new(Vec::new()));
            while !thread_stop.load(Ordering::SeqCst) {
                let mut stream = match listener.accept() {
                    Ok((stream, _)) => stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(error) => panic!("accept: {error}"),
                };
                stream.set_nonblocking(false).expect("blocking stream");
                let request = read_request(&mut stream);
                let index = received.lock().unwrap().len();
                let (status, body) = respond(script, index, &request);
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .expect("response");
                stream.flush().expect("flush");
                received.lock().unwrap().push(request);
            }
            Arc::try_unwrap(received)
                .ok()
                .expect("single owner")
                .into_inner()
                .unwrap()
        });
        Self {
            address,
            stop,
            handle,
        }
    }

    fn finish(self) -> Vec<Received> {
        self.stop.store(true, Ordering::SeqCst);
        self.handle.join().expect("server thread")
    }
}

fn read_request(stream: &mut TcpStream) -> Received {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("read timeout");
    let mut data = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = stream.read(&mut buffer).expect("request");
        assert!(read > 0, "client closed before sending the request");
        data.extend_from_slice(&buffer[..read]);
        let Some(end) = data.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };
        let head = String::from_utf8_lossy(&data[..end]).to_ascii_lowercase();
        let length = head
            .lines()
            .find_map(|line| line.strip_prefix("content-length:"))
            .and_then(|value| value.trim().parse::<usize>().ok())
            .unwrap_or(0);
        if data.len() >= end + 4 + length {
            let line = String::from_utf8_lossy(&data[..end])
                .lines()
                .next()
                .unwrap_or_default()
                .to_string();
            let body = if length == 0 {
                Value::Null
            } else {
                serde_json::from_slice(&data[end + 4..end + 4 + length]).expect("JSON body")
            };
            return Received { line, head, body };
        }
    }
}

fn respond(script: Script, index: usize, request: &Received) -> (&'static str, String) {
    let path = request.line.split_whitespace().nth(1).unwrap_or_default();
    if path.ends_with("/models") {
        return (
            "200 OK",
            json!({"object": "list", "data": [{"id": EXECUTOR_MODEL}]}).to_string(),
        );
    }
    match script {
        Script::Reject => {
            return (
                "400 Bad Request",
                json!({"error": {"message": "unsupported reasoning effort"}}).to_string(),
            );
        }
        Script::RetryFirst if index == 0 => {
            return (
                "503 Service Unavailable",
                json!({"error": "busy"}).to_string(),
            );
        }
        _ => {}
    }
    let model = request.body["model"].as_str().unwrap_or_default();
    let tool_turn = matches!(script, Script::ToolThenText) && index == 0;
    let body = match path {
        "/api/chat" => json!({
            "model": model,
            "message": {"role": "assistant", "content": "ok"},
            "done": true,
        }),
        "/v1/responses" if tool_turn => json!({
            "id": "resp-931-1",
            "model": model,
            "output": [
                {"type": "reasoning", "id": "rs-931", "summary": []},
                {"type": "function_call", "call_id": "call-931", "name": "Read",
                 "arguments": "{\"path\":\"notes.md\"}"}
            ],
            "usage": {"input_tokens": 40, "output_tokens": 6, "total_tokens": 46,
                      "input_tokens_details": {"cached_tokens": 32}},
        }),
        "/v1/responses" => json!({
            "id": "resp-931",
            "model": model,
            "output": [{"type": "message", "role": "assistant",
                        "content": [{"type": "output_text", "text": if matches!(script, Script::ToolThenText) { "done" } else { "ok" }}]}],
            "usage": {"input_tokens": 4, "output_tokens": 1, "total_tokens": 5},
        }),
        _ if tool_turn => json!({
            "id": "chatcmpl-931-1",
            "model": model,
            "choices": [{"message": {"content": "", "tool_calls": [{
                "id": "call-931", "type": "function",
                "function": {"name": "Read", "arguments": "{\"path\":\"notes.md\"}"}
            }]}}],
            "usage": {"prompt_tokens": 40, "completion_tokens": 6,
                      "prompt_tokens_details": {"cached_tokens": 32}},
        }),
        _ => json!({
            "id": "chatcmpl-931",
            "model": model,
            "choices": [{"message": {"content": if matches!(script, Script::ToolThenText) { "done" } else { "ok" }}}],
            "usage": {"prompt_tokens": 4, "completion_tokens": 1},
        }),
    };
    ("200 OK", body.to_string())
}
