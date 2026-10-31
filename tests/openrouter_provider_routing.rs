//! Issue #939: a preset's typed `openrouter_provider` routing is sent as the
//! top-level `provider` object by the generic OpenAI-compatible roles only, and
//! the response's top-level `provider` is recorded as reported in
//! `openrouter_provider_observed`. Every assertion is made on what a loopback
//! stand-in bound to `127.0.0.1:0` actually received; no case reaches a real
//! provider, and every key is a fake value.
//!
//! Each case runs in a child copy of this test binary with a cleared
//! environment, a temporary `HOME`, and a temporary workspace (`child_case`),
//! so no real config file, dotenv, or credential is read. The corpus
//! `tests/corpus/apps/openrouter-provider-routing/` holds the cases; the parent
//! tests below run them by group.

use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use commandagent::config::Config;
use commandagent::provider_call::{self, ProviderCallOverrides, ProviderCallScope};
use commandagent::provider_cli;
use commandagent::providers::ChatClient;
use commandagent::state::ConversationMessage;
use commandagent::tools::registry::ToolRegistry;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const CASE_ENV: &str = "COMMANDAGENT_ISSUE939_CHILD_CASE";
const EFFORT_ENV: &str = "COMMANDAGENT_COMPATIBLE_REASONING_EFFORT";
const CHILD_TEST: &str = "child_case";
const CORPUS: &str = "tests/corpus/apps/openrouter-provider-routing";
const PRESET: &str = "routed";
const MODELS: [&str; 3] = [
    "executor-model-939",
    "planner-model-939",
    "classifier-model-939",
];
const ROLES: [&str; 3] = ["executor", "planner", "classifier"];
const NUM_PREDICT: u64 = 64;
const DUMMY_KEY_ENV: &str = "COMMANDAGENT_ISSUE939_DUMMY_KEY";
const DUMMY_KEY: &str = "dummy-key-939-not-a-real-secret";
const OBSERVED: &str = "openrouter_provider_observed";

// ---------------------------------------------------------------- parent side

fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(CORPUS)
}

fn corpus() -> Value {
    let path = corpus_root().join("fixtures/cases.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn cases(group: &str) -> Vec<Value> {
    let selected = corpus()["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["group"] == group)
        .cloned()
        .collect::<Vec<_>>();
    assert!(!selected.is_empty(), "no corpus case in group {group}");
    selected
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
    if let Some(effort) = case["effort"].as_str() {
        command.env(EFFORT_ENV, effort);
    }
    if let Some(value) = case["key_value"].as_str() {
        command.env(DUMMY_KEY_ENV, value);
    } else if uses_key(case) {
        command.env(DUMMY_KEY_ENV, DUMMY_KEY);
    }
    let output = command.output().expect("child process");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success() && stdout.contains("1 passed") && stdout.contains("ISSUE939_OK"),
        "case {} failed:\n{stdout}\n{stderr}",
        case["id"]
    );
}

fn uses_key(case: &Value) -> bool {
    case["scenario"] == "key" || case["api_key"] == true || case["key_value"].is_string()
}

fn run_group(group: &str) {
    for case in cases(group) {
        run_child(&case);
    }
}

/// Verification 1: the config leaf through the public `Config` path.
#[test]
fn config_errors_name_the_location_and_never_the_value() {
    run_group("config-error");
}

/// Security review B1: dotted, quoted, and section forms of the routing key are
/// refused through the public `Config` path without the unknown suffix.
#[test]
fn unsupported_routing_forms_never_show_the_unknown_key() {
    run_group("config-form");
}

/// Security review B3: an observation status value cannot be registered as a
/// credential, and an ordinary key keeps the status enum intact.
#[test]
fn reserved_status_values_cannot_be_registered_as_credentials() {
    run_group("reserved");
}

/// Verification 1: layers and `extends` replace the table as one value.
#[test]
fn config_layers_and_inheritance_replace_the_whole_table() {
    run_group("layers");
}

/// Verification 1 and 5: a role the CLI changes follows the resolved provider.
#[test]
fn cli_role_changes_decide_where_routing_applies() {
    run_group("cli-role");
}

/// Verification 2 and 4: unset and `{}` keep the legacy request bytes.
#[test]
fn unset_and_cleared_routing_keep_the_legacy_request_and_events() {
    run_group("unset");
}

/// Verification 4: relay-equivalent, variant, order, and allow_fallbacks
/// combinations on both APIs with effort low.
#[test]
fn declared_routing_reaches_both_apis_once_beside_effort() {
    run_group("declared");
}

/// Verification 5: mixed generic / builtin roles, LM Studio, Ollama native.
#[test]
fn routing_reaches_only_the_generic_roles() {
    run_group("roles");
}

/// Verification 5: native and text tool modes keep the routing.
#[test]
fn native_and_text_tool_turns_keep_the_routing() {
    run_group("tools");
}

/// Verification 5: 400 / 404 stay failures with one request; 503 retries send
/// the same body.
#[test]
fn rejections_are_not_resent_and_retries_keep_the_routing() {
    run_group("transport");
}

/// Verification 6: the reported provider is recorded as reported or as
/// missing / invalid / unparseable, never filled in.
#[test]
fn response_provider_is_observed_without_being_filled_in() {
    run_group("observe");
}

/// Verification 6 and contract 8: a reflected fake credential never reaches
/// the events, and the key stays in the Authorization header.
#[test]
fn reflected_fake_credential_is_removed_from_the_observation() {
    run_group("key");
}

/// Verification 7: every corpus case runs and belongs to a named group.
#[test]
fn corpus_cases_match_the_wire_contract() {
    let corpus = corpus();
    let known = [
        "config-error",
        "layers",
        "cli-role",
        "unset",
        "declared",
        "roles",
        "tools",
        "transport",
        "observe",
        "key",
        "config-form",
        "reserved",
    ];
    let mut ids = BTreeSet::new();
    for case in corpus["cases"].as_array().unwrap() {
        assert_eq!(case["kind"], "synthetic", "{case}");
        assert!(
            known.contains(&case["group"].as_str().unwrap_or_default()),
            "{case}"
        );
        assert!(
            ids.insert(case["id"].as_str().unwrap().to_string()),
            "{case}"
        );
    }
    for group in known {
        assert!(!cases(group).is_empty());
    }
}

/// Verification 7: the manifest pins the fixture and the legacy bytes.
#[test]
fn corpus_manifest_pins_the_fixture_and_the_base_sha() {
    let root = corpus_root();
    let fixture = std::fs::read(root.join("fixtures/cases.json")).unwrap();
    let digest = format!("{:x}", Sha256::digest(&fixture));
    let manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("source-manifest.json")).unwrap())
            .unwrap();
    assert_eq!(manifest["fixture_sha256"]["cases.json"], digest);
    assert_eq!(
        manifest["base_sha"],
        "06dc565df12f7342394bd335c416c1a44c530cf1"
    );
    let text = String::from_utf8(fixture).unwrap();
    for forbidden in [
        "/Volumes",
        "/Users",
        "127.0.0.1",
        "11535",
        "openrouter.ai",
        "authorization",
        "Bearer",
        "heldout",
    ] {
        assert!(
            !text.contains(forbidden),
            "corpus must not copy {forbidden}"
        );
    }
    let legacy = &corpus()["legacy"];
    for api in ["chat_completions", "responses"] {
        assert!(legacy["request"][api].is_string(), "{api}");
        assert!(legacy["provider_request"][api].is_string(), "{api}");
    }
}

/// Doctor reports the requested object, its source, and the roles, stays
/// unverified, sends no generation request, and never passes a rejected value.
#[test]
fn doctor_reports_routing_without_verifying_the_server() {
    let server = Server::spawn(Script::default());
    let base_url = format!("http://{}/v1", server.address);
    let generic_planner = [
        ("provider", "ollama"),
        ("planner_provider", "openai-compatible"),
        ("classifier_provider", "openai-compatible"),
    ];

    let declared = doctor(
        &base_url,
        &generic_planner,
        Some(r#"{ only = ["open-inference/fp4"], allow_fallbacks = false }"#),
    );
    let check = routing_check(&declared.stdout);
    assert_eq!(check["status"], "pass");
    let details = &check["details"];
    assert_eq!(details["status"], "declared");
    assert_eq!(details["declared"], true);
    assert_eq!(
        details["requested_provider"],
        json!({"only": ["open-inference/fp4"], "allow_fallbacks": false})
    );
    assert_eq!(details["source"], "preset:routed");
    assert_eq!(details["server_support_verified"], false);
    assert_eq!(
        details["applicable_roles"],
        json!(["planner", "classifier"])
    );
    assert_eq!(details["roles"][0]["role"], "executor");
    assert_eq!(details["roles"][0]["status"], "not_applicable");
    assert!(details["roles"][0]["requested_provider"].is_null());
    assert_eq!(details["roles"][1]["status"], "declared");
    assert_eq!(details["roles"][1]["provider"], "openai-compatible");
    assert!(
        check["message"]
            .as_str()
            .unwrap()
            .contains("server support not verified")
    );

    let disabled = doctor(&base_url, &generic_planner, Some("{}"));
    let check = routing_check(&disabled.stdout);
    assert_eq!(check["details"]["status"], "disabled");
    assert_eq!(check["details"]["declared"], false);
    assert!(check["details"]["requested_provider"].is_null());
    assert_eq!(check["details"]["source"], "preset:routed");

    let omitted = doctor(&base_url, &generic_planner, None);
    let check = routing_check(&omitted.stdout);
    assert_eq!(check["details"]["status"], "omitted");
    assert!(check["details"]["source"].is_null());

    let native = doctor(
        &base_url,
        &[
            ("provider", "ollama"),
            ("planner_provider", "ollama"),
            ("classifier_provider", "lm-studio"),
        ],
        Some("{}"),
    );
    let check = routing_check(&native.stdout);
    assert_eq!(check["details"]["status"], "not_applicable");
    assert_eq!(check["details"]["applicable_roles"], json!([]));
    assert_eq!(check["details"]["server_support_verified"], false);

    let secret = "rejected-939-value";
    let invalid = doctor(
        &base_url,
        &generic_planner,
        Some(&format!(r#"{{ only = ["{secret}"], model = "{secret}" }}"#)),
    );
    assert!(!invalid.stdout.contains(secret), "{}", invalid.stdout);
    assert!(!invalid.stderr.contains(secret), "{}", invalid.stderr);
    let reported_ok = serde_json::from_str::<Value>(&invalid.stdout)
        .ok()
        .and_then(|report| {
            report["checks"].as_array().map(|checks| {
                checks.iter().any(|check| {
                    check["id"] == "config.openrouter_provider_routing"
                        && check["details"]["status"] == "declared"
                })
            })
        })
        .unwrap_or(false);
    assert!(
        !invalid.success && !reported_ok,
        "an invalid routing must not look healthy: {}\n{}",
        invalid.stdout,
        invalid.stderr
    );

    let requests = server.finish();
    assert!(
        requests
            .iter()
            .all(|request| request.line.starts_with("GET ")),
        "doctor must not send a generation request"
    );
}

/// Security review B1: doctor never shows an unknown routing key, whether it
/// is written inline, as a dotted key, or as a nested section.
#[test]
fn doctor_never_shows_an_unknown_routing_key_in_any_form() {
    let server = Server::spawn(Script::default());
    let base_url = format!("http://{}/v1", server.address);
    let generic = [
        ("provider", "openai-compatible"),
        ("planner_provider", "openai-compatible"),
        ("classifier_provider", "openai-compatible"),
    ];
    let marker = "PRIVATE_MARKER_939";
    let inline = format!(r#"{{ only = ["a"], {marker} = ["b"] }}"#);
    let forms = [
        (Some(inline.as_str()), String::new()),
        (None, format!("openrouter_provider.{marker} = [\"b\"]\n")),
        (
            None,
            format!("openrouter_provider.\"{marker}\" = [\"b\"]\n"),
        ),
        (
            None,
            format!("[preset.{PRESET}.openrouter_provider]\n{marker} = [\"b\"]\n"),
        ),
        (
            None,
            format!("[preset.{PRESET}.openrouter_provider.{marker}]\nonly = [\"b\"]\n"),
        ),
    ];
    for (routing, tail) in &forms {
        let output = doctor_with_tail(&base_url, &generic, *routing, tail);
        assert!(!output.success, "{}\n{}", output.stdout, output.stderr);
        assert!(!output.stdout.contains(marker), "{}", output.stdout);
        assert!(!output.stderr.contains(marker), "{}", output.stderr);
        assert!(
            output.stdout.contains("openrouter_provider")
                || output.stderr.contains("openrouter_provider"),
            "the diagnostic names the fixed key: {}\n{}",
            output.stdout,
            output.stderr
        );
    }
    server.finish();
}

struct DoctorOutput {
    success: bool,
    stdout: String,
    stderr: String,
}

fn doctor(base_url: &str, providers: &[(&str, &str)], routing: Option<&str>) -> DoctorOutput {
    doctor_with_tail(base_url, providers, routing, "")
}

fn doctor_with_tail(
    base_url: &str,
    providers: &[(&str, &str)],
    routing: Option<&str>,
    tail: &str,
) -> DoctorOutput {
    let workspace = tempfile::tempdir().unwrap();
    let home = workspace.path().join("home");
    let state = workspace.path().join("state");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&state).unwrap();
    let mut preset = format!(
        "[preset.{PRESET}]\nmodel = \"{}\"\nplanner_model = \"{}\"\nclassifier_model = \"{}\"\nbase_url = \"{base_url}\"\n",
        MODELS[0], MODELS[1], MODELS[2]
    );
    for (key, value) in providers {
        preset.push_str(&format!("{key} = \"{value}\"\n"));
    }
    if let Some(routing) = routing {
        preset.push_str(&format!("openrouter_provider = {routing}\n"));
    }
    preset.push_str(tail);
    let config_dir = workspace.path().join(".commandagent");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(config_dir.join("config.toml"), preset).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .args(["--doctor", "--json", "--preset", PRESET])
        .args(["--ollama-host", "http://127.0.0.1:9"])
        .args(["--lm-studio-host", "http://127.0.0.1:9"])
        .args(["--cwd", workspace.path().to_str().unwrap()])
        .args(["--state-dir", state.to_str().unwrap()])
        .env_clear()
        .env("HOME", &home)
        .current_dir(workspace.path())
        .output()
        .unwrap();
    DoctorOutput {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn routing_check(stdout: &str) -> Value {
    let report: Value = serde_json::from_str(stdout).expect("doctor JSON");
    report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == "config.openrouter_provider_routing")
        .cloned()
        .expect("routing check")
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
        Some("config-error") => run_config_error_case(&case),
        Some("observe") | Some("key") => run_observe_case(&case),
        _ => run_wire_case(&case),
    }
    println!("ISSUE939_OK");
}

fn api(case: &Value) -> &str {
    case["api"].as_str().unwrap_or("chat_completions")
}

fn layout(case: &Value) -> Vec<String> {
    case["layout"]
        .as_array()
        .map(|roles| {
            roles
                .iter()
                .map(|role| role.as_str().unwrap().to_string())
                .collect()
        })
        .unwrap_or_else(|| vec!["generic".to_string(); 3])
}

fn preset_provider(provider: &str) -> &str {
    if provider == "generic" {
        "openai-compatible"
    } else {
        provider
    }
}

struct Servers {
    generic: Server,
    lm_studio: Server,
    ollama: Server,
}

impl Servers {
    fn spawn(script: Script) -> Self {
        Self {
            generic: Server::spawn(script),
            lm_studio: Server::spawn(Script::default()),
            ollama: Server::spawn(Script::default()),
        }
    }
}

/// Writes the workspace config (and an optional home layer) and resolves the
/// public `Config`. `routing` is the raw TOML value line, or `None`.
fn build_config(
    case: &Value,
    root: &Path,
    servers: &Servers,
    routing: Option<&str>,
) -> anyhow::Result<Config> {
    let roles = layout(case);
    let mut text = String::new();
    if let Some(parent) = case["parent_routing"].as_str() {
        text.push_str(&format!(
            "[preset.base]\nopenrouter_provider = {parent}\n\n"
        ));
    }
    if let Some(other) = case["unselected_routing"].as_str() {
        text.push_str(&format!(
            "[preset.other]\nopenrouter_provider = {other}\n\n"
        ));
    }
    text.push_str(&format!("[preset.{PRESET}]\n"));
    if case["parent_routing"].is_string() {
        text.push_str("extends = \"base\"\n");
    }
    for (index, key) in ["", "planner_", "classifier_"].iter().enumerate() {
        text.push_str(&format!("{key}model = \"{}\"\n", MODELS[index]));
        text.push_str(&format!(
            "{key}provider = \"{}\"\n",
            preset_provider(&roles[index])
        ));
    }
    if roles.iter().any(|role| role == "generic") {
        text.push_str(&format!(
            "base_url = \"http://{}/v1\"\n",
            servers.generic.address
        ));
    }
    if uses_key(case) {
        text.push_str(&format!("api_key_env = \"{DUMMY_KEY_ENV}\"\n"));
    }
    text.push_str(&format!("api = \"{}\"\n", api(case)));
    if let Some(routing) = routing {
        text.push_str(&format!("openrouter_provider = {routing}\n"));
    }
    if let Some(tail) = case["raw_tail"].as_str() {
        text.push_str(tail);
    }
    let config_dir = root.join(".commandagent");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(config_dir.join("config.toml"), text).unwrap();
    if let Some(home_routing) = case["home_routing"].as_str() {
        let home = PathBuf::from(std::env::var_os("HOME").expect("child HOME"));
        let home_dir = home.join(".commandagent");
        std::fs::create_dir_all(&home_dir).unwrap();
        std::fs::write(
            home_dir.join("config.toml"),
            format!("[preset.{PRESET}]\nopenrouter_provider = {home_routing}\n"),
        )
        .unwrap();
    } else if let Some(home) = std::env::var_os("HOME") {
        // The companion run shares the child HOME; it must not see the layer.
        let _ = std::fs::remove_file(PathBuf::from(home).join(".commandagent/config.toml"));
    }

    let retries = match case["server"].as_str() {
        Some("retry-503") => "1",
        Some("reject-400") | Some("reject-404") => "2",
        _ => "0",
    };
    let mut arguments = vec![
        "commandagent".to_string(),
        "--cwd".to_string(),
        root.to_string_lossy().into_owned(),
        "--state-dir".to_string(),
        root.join("state").to_string_lossy().into_owned(),
        "--preset".to_string(),
        PRESET.to_string(),
        "--ollama-host".to_string(),
        format!("http://{}", servers.ollama.address),
        "--lm-studio-host".to_string(),
        format!("http://{}", servers.lm_studio.address),
        "--num-predict".to_string(),
        NUM_PREDICT.to_string(),
        "--chat-timeout-secs".to_string(),
        "10".to_string(),
        "--chat-retries".to_string(),
        retries.to_string(),
    ];
    for argument in case["cli"].as_array().into_iter().flatten() {
        let argument = argument.as_str().unwrap();
        arguments.push(if argument == "<generic-url>" {
            format!("http://{}/v1", servers.generic.address)
        } else {
            argument.to_string()
        });
    }
    let parsed = provider_cli::parse_from(arguments)?;
    let mut config = Config::from_cli_with_provider_options(parsed.cli, parsed.provider_options)?;
    config.eval_events_path = Some(root.join("events.jsonl"));
    Ok(config)
}

fn run_config_error_case(case: &Value) {
    let dir = tempfile::tempdir().expect("tempdir");
    let servers = Servers::spawn(Script::default());
    let error = format!(
        "{:#}",
        build_config(case, dir.path(), &servers, case["routing"].as_str())
            .expect_err("routing must be rejected")
    );
    for expected in case["expect_error"].as_array().unwrap() {
        assert!(error.contains(expected.as_str().unwrap()), "{error}");
    }
    if let Some(secret) = case["secret"].as_str() {
        assert!(!error.contains(secret), "rejected value leaked: {error}");
    }
    assert!(servers.generic.finish().is_empty());
    assert!(servers.lm_studio.finish().is_empty());
    assert!(servers.ollama.finish().is_empty());
}

fn tools_for(case: &Value) -> (Vec<commandagent::tools::registry::ToolSpec>, bool) {
    match case["tools"].as_str() {
        Some("native") => (ToolRegistry::default().specs().to_vec(), true),
        Some("text") => (ToolRegistry::default().specs().to_vec(), false),
        _ => (Vec::new(), false),
    }
}

/// Every role sends its turns: executor and planner run one `provider_call`
/// turn and one turn on a clone; the classifier runs two classifier calls.
fn drive_roles(case: &Value, config: &Config) -> Vec<(String, anyhow::Result<()>)> {
    let (tools, native) = tools_for(case);
    let messages = [ConversationMessage::user("hello")];
    let mut outcomes = Vec::new();
    for (index, role) in ROLES.iter().enumerate() {
        let model = MODELS[index];
        let result = if *role == "classifier" {
            let mut planner =
                commandagent::providers::client_from_config(config, true).expect("planner");
            (0..2).try_for_each(|_| {
                provider_call::chat_with_cancel_and_response_limit_and_overrides(
                    planner.as_mut(),
                    config,
                    provider_call::ProviderChatRequest {
                        scope: ProviderCallScope::PlannerStep,
                        model,
                        messages: &messages,
                        tools: &tools,
                        native_tools_enabled: native,
                    },
                    || false,
                    1 << 20,
                    ProviderCallOverrides::classifier(config),
                )
                .result
                .map(|_| ())
            })
        } else {
            let planner = *role == "planner";
            let mut client =
                commandagent::providers::client_from_config(config, planner).expect("client");
            let scope = if planner {
                ProviderCallScope::PlannerStep
            } else {
                ProviderCallScope::Executor
            };
            provider_call::chat(
                client.as_mut(),
                config,
                scope,
                model,
                &messages,
                &tools,
                native,
            )
            .result
            .and_then(|_| {
                let mut clone: Box<dyn ChatClient> = client.boxed_clone();
                clone.chat(model, &messages, &tools, native).map(|_| ())
            })
        };
        outcomes.push((role.to_string(), result));
    }
    outcomes
}

fn events(root: &Path) -> Vec<Value> {
    std::fs::read_to_string(root.join("events.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .map(|mut event| {
            if let Some(object) = event.as_object_mut() {
                object.remove("occurred_at_epoch_ms");
            }
            event
        })
        .collect()
}

fn named<'a>(events: &'a [Value], name: &str) -> Vec<&'a Value> {
    events
        .iter()
        .filter(|event| event["event"] == name)
        .collect()
}

fn effort_fields_once(api: &str, body: &Value, effort: Option<&str>) {
    match (api, effort) {
        ("responses", Some(value)) => {
            assert_eq!(body["reasoning"], json!({"effort": value}), "{body}");
            assert!(body.get("reasoning_effort").is_none(), "{body}");
        }
        (_, Some(value)) => {
            assert_eq!(body["reasoning_effort"], value, "{body}");
            assert!(body.get("reasoning").is_none(), "{body}");
        }
        (_, None) => {
            assert!(body.get("reasoning").is_none(), "{body}");
            assert!(body.get("reasoning_effort").is_none(), "{body}");
        }
    }
}

/// Runs the case with its routing and again with no routing line at all
/// (the companion), then compares what each server received.
fn run_wire_case(case: &Value) {
    let api = api(case);
    let effort = case["effort"].as_str();
    let script = Script::from_case(case);
    let routed_dir = tempfile::tempdir().expect("tempdir");
    let routed = Servers::spawn(script.clone());
    let config = build_config(case, routed_dir.path(), &routed, case["routing"].as_str())
        .unwrap_or_else(|error| panic!("config: {error:#}"));
    let outcomes = drive_roles(case, &config);

    let companion_dir = tempfile::tempdir().expect("tempdir");
    let companion = Servers::spawn(script);
    let mut plain_case = case.clone();
    plain_case["home_routing"] = Value::Null;
    plain_case["parent_routing"] = Value::Null;
    let companion_config = build_config(&plain_case, companion_dir.path(), &companion, None)
        .unwrap_or_else(|error| panic!("companion config: {error:#}"));
    let companion_outcomes = drive_roles(&plain_case, &companion_config);

    let generic_roles = case["generic_roles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|role| role.as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    let expected = case["expect_provider"].clone();
    let rejected = matches!(
        case["server"].as_str(),
        Some("reject-400") | Some("reject-404")
    );
    for ((role, outcome), (_, companion_outcome)) in outcomes.iter().zip(&companion_outcomes) {
        if rejected && generic_roles.contains(role) {
            let error = format!(
                "{:#}",
                outcome.as_ref().expect_err("rejection stays a failure")
            );
            let status = &case["server"].as_str().unwrap()[7..];
            assert!(error.contains(status), "{error}");
            assert!(companion_outcome.is_err());
        } else {
            assert!(
                outcome.is_ok(),
                "{role}: {:#}",
                outcome.as_ref().err().unwrap()
            );
            assert!(companion_outcome.is_ok(), "{role} companion");
        }
    }

    let generic = routed.generic.finish();
    let generic_plain = companion.generic.finish();
    assert_eq!(generic.len(), generic_plain.len(), "generic request count");
    let per_role = match case["server"].as_str() {
        Some("reject-400") | Some("reject-404") => 1,
        Some("retry-503") => 3,
        _ => 2,
    };
    assert_eq!(
        generic.len(),
        generic_roles.len() * per_role,
        "generic requests"
    );
    let endpoint = if api == "responses" {
        "POST /v1/responses "
    } else {
        "POST /v1/chat/completions "
    };
    for (request, plain) in generic.iter().zip(&generic_plain) {
        assert!(request.line.starts_with(endpoint), "{}", request.line);
        assert_eq!(request.line, plain.line);
        assert_eq!(request.authorization(), plain.authorization());
        effort_fields_once(api, &request.body, effort);
        match expected.as_object() {
            Some(object) => {
                assert_eq!(request.body["provider"], expected, "{}", request.raw);
                assert_eq!(
                    request.body["provider"].as_object().unwrap().len(),
                    object.len()
                );
                let mut rest = request.body.clone();
                rest.as_object_mut().unwrap().remove("provider");
                assert_eq!(rest, plain.body, "only the provider object is added");
                assert_eq!(request.raw.matches("\"provider\":").count(), 1);
            }
            None => {
                assert!(request.body.get("provider").is_none(), "{}", request.raw);
                assert_eq!(request.raw, plain.raw, "legacy request bytes");
            }
        }
        assert!(plain.body.get("provider").is_none());
    }
    if case["server"] == "retry-503" {
        for pair in generic.chunks(3) {
            assert_eq!(pair[0].raw, pair[1].raw, "a retry resends the same body");
        }
    }
    if let Some(legacy) = case["legacy_bytes"].as_str() {
        let legacy = corpus()["legacy"]["request"][legacy]
            .as_str()
            .unwrap()
            .replace("{model}", MODELS[0]);
        let first = generic
            .iter()
            .find(|request| request.body["model"] == MODELS[0])
            .expect("executor request");
        assert_eq!(first.raw, legacy, "base-SHA request bytes");
    }
    for server in [
        routed.lm_studio,
        routed.ollama,
        companion.lm_studio,
        companion.ollama,
    ] {
        for request in server.finish() {
            assert!(request.body.get("provider").is_none(), "{}", request.raw);
        }
    }

    let routed_events = events(routed_dir.path());
    let plain_events = events(companion_dir.path());
    for name in [
        "provider_response",
        "provider_error",
        "provider_retry",
        "provider_parse_error",
    ] {
        assert_eq!(
            named(&routed_events, name),
            named(&plain_events, name),
            "{name} events are unchanged"
        );
    }
    let requests = named(&routed_events, "provider_request");
    let plain_requests = named(&plain_events, "provider_request");
    assert_eq!(requests.len(), plain_requests.len());
    for (event, plain) in requests.iter().zip(&plain_requests) {
        let generic_event = event["provider"] == "openai-compatible";
        if generic_event && expected.is_object() {
            assert_eq!(event["requested_provider"], expected, "{event}");
            assert_eq!(event["provider_routing_source"], case["expect_source"]);
            let role = ROLES[MODELS
                .iter()
                .position(|model| event["model"] == *model)
                .unwrap()];
            assert_eq!(event["provider_role"], role, "{event}");
            let mut rest = (*event).clone();
            let object = rest.as_object_mut().unwrap();
            object.remove("requested_provider");
            object.remove("provider_routing_source");
            if effort.is_none() {
                object.remove("provider_role");
            }
            assert_eq!(&rest, *plain, "only the routing fields are added");
        } else {
            assert_eq!(event, plain, "provider_request is unchanged");
            assert!(event.get("requested_provider").is_none(), "{event}");
        }
    }
    if let Some(legacy) = case["legacy_bytes"].as_str() {
        let legacy = corpus()["legacy"]["provider_request"][legacy]
            .as_str()
            .unwrap()
            .replace("{model}", MODELS[0]);
        let first = requests
            .iter()
            .find(|event| event["model"] == MODELS[0])
            .expect("executor provider_request");
        assert_eq!(first.to_string(), legacy, "base-SHA provider_request");
    }

    let observed = named(&routed_events, OBSERVED);
    assert!(named(&plain_events, OBSERVED).is_empty());
    if expected.is_object() && !rejected {
        // One per 2xx body: the 503 attempt records nothing.
        assert_eq!(observed.len(), generic_roles.len() * 2, "observed events");
        for event in &observed {
            assert_eq!(event["provider"], "openai-compatible");
            assert_eq!(event["requested_provider"], expected);
            assert_eq!(event["response_provider"], "OpenInference");
            assert_eq!(event["response_provider_status"], "reported");
            assert_eq!(event["schema_version"], "1");
            assert_eq!(event["api"], api);
        }
        if case["server"] == "retry-503" {
            assert!(observed.iter().any(|event| event["attempt"] == 2));
        }
    } else {
        assert!(observed.is_empty(), "{observed:?}");
    }
}

/// One generic executor turn against a scripted 2xx body.
fn run_observe_case(case: &Value) {
    let api = api(case);
    let dir = tempfile::tempdir().expect("tempdir");
    let servers = Servers::spawn(Script::from_case(case));
    let config = build_config(case, dir.path(), &servers, case["routing"].as_str())
        .unwrap_or_else(|error| panic!("config: {error:#}"));
    let mut client = commandagent::providers::client_from_config(&config, false).expect("client");
    let registry = ToolRegistry::default();
    let outcome = client.chat(
        MODELS[0],
        &[ConversationMessage::user("hello")],
        registry.specs(),
        true,
    );
    let requests = servers.generic.finish();
    assert_eq!(requests.len(), 1, "one request, no resend");
    assert!(requests[0].body["provider"].is_object());
    let events = events(dir.path());
    let observed = named(&events, OBSERVED);
    assert_eq!(observed.len(), 1, "{events:?}");
    let event = observed[0];
    assert_eq!(event["response_provider_status"], case["expect_status"]);
    assert_eq!(event["response_provider"], case["expect_response_provider"]);
    assert_eq!(event["provider"], "openai-compatible");
    assert_eq!(event["api"], api);
    assert_eq!(event["attempt"], 1);
    assert_eq!(event["model"], MODELS[0]);
    assert_eq!(event["provider_role"], "executor");
    for forbidden in ["hello", "notes.md", "reasoning-939", "Authorization"] {
        assert!(!event.to_string().contains(forbidden), "{event}");
    }
    if case["expect_reply"] == "parse-error" {
        let error = format!("{:#}", outcome.expect_err("parse error stays a failure"));
        assert!(!error.is_empty());
        assert_eq!(named(&events, "provider_parse_error").len(), 1);
        assert!(named(&events, "provider_response").is_empty());
    } else {
        let reply = outcome.unwrap_or_else(|error| panic!("reply: {error:#}"));
        assert_eq!(reply.content, "ok");
        assert_eq!(named(&events, "provider_response").len(), 1);
    }
    let all = std::fs::read_to_string(dir.path().join("events.jsonl")).unwrap();
    assert!(
        all.contains(&format!(
            "\"response_provider_status\":\"{}\"",
            case["expect_status"].as_str().unwrap()
        )),
        "the status enum survives the scrub: {all}"
    );
    if case["scenario"] == "key" {
        assert!(
            requests[0]
                .head
                .contains(&format!("authorization: bearer {DUMMY_KEY}")),
            "the key belongs in the header"
        );
        assert!(!requests[0].raw.contains(DUMMY_KEY));
        let all = std::fs::read_to_string(dir.path().join("events.jsonl")).unwrap();
        assert!(!all.contains(DUMMY_KEY), "{all}");
    }
}

// ------------------------------------------------------------ loopback server

#[derive(Clone, Default)]
struct Script {
    status: Option<String>,
    retry_first: bool,
    /// The top-level `provider` of a 2xx body; `None` leaves it out.
    response_provider: Option<Value>,
    reflect_key: bool,
    raw_body: Option<String>,
    broken_tool_args: bool,
}

impl Script {
    fn from_case(case: &Value) -> Self {
        let status = match case["server"].as_str() {
            Some("reject-400") => Some("400 Bad Request".to_string()),
            Some("reject-404") => Some("404 Not Found".to_string()),
            _ => None,
        };
        let response_provider = match case.get("response_provider") {
            Some(value) => Some(value.clone()),
            None if case["scenario"] == "observe" => None,
            None => Some(json!("OpenInference")),
        };
        Self {
            status,
            retry_first: case["server"] == "retry-503",
            response_provider,
            reflect_key: case["scenario"] == "key",
            raw_body: case["raw_body"].as_str().map(str::to_string),
            broken_tool_args: case["broken_tool_args"] == true,
        }
    }
}

struct Received {
    line: String,
    head: String,
    raw: String,
    body: Value,
}

impl Received {
    fn authorization(&self) -> Option<&str> {
        self.head
            .lines()
            .find(|line| line.starts_with("authorization:"))
    }
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
            let received = Arc::new(Mutex::new(Vec::<Received>::new()));
            let mut retried = BTreeSet::new();
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
                let (status, body) = respond(&script, &mut retried, &request);
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
            let raw = String::from_utf8_lossy(&data[end + 4..end + 4 + length]).into_owned();
            let body = if length == 0 {
                Value::Null
            } else {
                serde_json::from_str(&raw).expect("JSON body")
            };
            return Received {
                line,
                head,
                raw,
                body,
            };
        }
    }
}

fn respond(
    script: &Script,
    retried: &mut BTreeSet<String>,
    request: &Received,
) -> (String, String) {
    let path = request.line.split_whitespace().nth(1).unwrap_or_default();
    let model = request.body["model"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    if path.ends_with("/models") {
        return (
            "200 OK".to_string(),
            json!({"object": "list", "data": [{"id": MODELS[0]}]}).to_string(),
        );
    }
    if path == "/api/chat" {
        return (
            "200 OK".to_string(),
            json!({"model": model, "message": {"role": "assistant", "content": "ok"}, "done": true})
                .to_string(),
        );
    }
    if let Some(status) = &script.status {
        return (
            status.clone(),
            json!({"error": {"message": "provider routing rejected"}}).to_string(),
        );
    }
    if script.retry_first && retried.insert(model.clone()) {
        return (
            "503 Service Unavailable".to_string(),
            json!({"error": "busy"}).to_string(),
        );
    }
    if let Some(raw) = &script.raw_body {
        return ("200 OK".to_string(), raw.clone());
    }
    let arguments = if script.broken_tool_args {
        "{\"path\":"
    } else {
        "{\"path\":\"notes.md\"}"
    };
    let mut body = if path == "/v1/responses" {
        let output = if script.broken_tool_args {
            json!([{"type": "function_call", "call_id": "call-939", "name": "Read", "arguments": arguments}])
        } else {
            json!([{"type": "message", "role": "assistant",
                    "content": [{"type": "output_text", "text": "ok"}]}])
        };
        json!({
            "id": "resp-939",
            "model": model,
            "output": output,
            "usage": {"input_tokens": 4, "output_tokens": 1, "total_tokens": 5},
        })
    } else {
        let message = if script.broken_tool_args {
            json!({"content": "", "tool_calls": [{"id": "call-939", "type": "function",
                   "function": {"name": "Read", "arguments": arguments}}]})
        } else {
            json!({"content": "ok", "reasoning_content": "reasoning-939"})
        };
        json!({
            "id": "chatcmpl-939",
            "model": model,
            "choices": [{"message": message}],
            "usage": {"prompt_tokens": 4, "completion_tokens": 1},
        })
    };
    if path.starts_with("/v1/") {
        let provider = if script.reflect_key {
            let token = request
                .head
                .lines()
                .find_map(|line| line.strip_prefix("authorization: bearer "))
                .unwrap_or_default()
                .to_string();
            Some(json!(token))
        } else {
            script.response_provider.clone()
        };
        if let Some(provider) = provider {
            body["provider"] = provider;
        }
    }
    ("200 OK".to_string(), body.to_string())
}
