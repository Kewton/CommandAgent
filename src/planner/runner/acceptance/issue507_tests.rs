//! Issue #507: pin the pass/fail boundary of the final acceptance helpers so a
//! flipped condition cannot turn a failing input into a passing one (or the
//! reverse). Each test names the cargo-mutants mutations it is meant to kill.
use super::*;
use crate::cli::Cli;
use crate::minimal_loop::completion::CompletionContract;
use crate::planner::step_plan::StepPlan;
use clap::Parser;
use serde_json::json;
use std::process::Command;

#[path = "../../final_acceptance/issue507_tests.rs"]
mod final_acceptance_issue507_tests;

fn generic_config(root: &Path) -> Config {
    let mut config = Config::from_cli(Cli::parse_from([
        "commandagent",
        "--cwd",
        root.to_str().unwrap(),
        "--profile",
        "generic",
    ]))
    .unwrap();
    config.eval_events_path = Some(root.join("run/events.jsonl"));
    config
}

fn bound(contract: Value, required: bool) -> BoundCompletionContract {
    BoundCompletionContract {
        contract: serde_json::from_value::<CompletionContract>(contract).unwrap(),
        path: "contract.json".to_string(),
        fs_path: None,
        generated: false,
        required,
    }
}

struct PlanFinal {
    outcome: Result<(), String>,
    event: Value,
}

fn run_plan_final(
    root: &Path,
    contract: Option<(Value, bool)>,
    required_paths: &[&str],
) -> PlanFinal {
    let config = generic_config(root);
    let plan = StepPlan {
        goal: "Write a small note".to_string(),
        steps: Vec::new(),
    };
    let required_paths = required_paths
        .iter()
        .map(|path| path.to_string())
        .collect::<Vec<_>>();
    let bound = contract.map(|(contract, required)| bound(contract, required));
    let outcome = verify_plan_final_contract(&plan, &required_paths, &config, bound.as_ref())
        .map_err(|err| err.to_string());
    let events = std::fs::read_to_string(root.join("run/events.jsonl")).unwrap();
    let event = events
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .rev()
        .find(|event| event["event"] == "plan_final_contract")
        .unwrap_or_else(|| panic!("missing plan_final_contract event: {events}"));
    PlanFinal { outcome, event }
}

// kills: acceptance.rs:2183 replace -> Ok(()) (the failing cases below)
// kills: acceptance.rs:2234 / 2235 / 2236 delete ! (nothing required -> runtime acceptance skipped)
// kills: acceptance.rs:2289 `&&` -> `||` (no contract, not required -> binding is not missing)
// kills: acceptance.rs:2290 delete ! (external_ok stays true)
// kills: acceptance.rs:2305 `==` -> `!=` (a not_applicable release gate is not a failure)
// kills: acceptance.rs:2310 delete ! (release gate not failed -> ok)
// kills: acceptance.rs:2345 delete ! (no missing artifacts -> primary reason "ok")
// kills: acceptance.rs:2368 delete ! (a passing run saves no recovery handoff)
#[test]
fn plan_final_contract_passes_when_nothing_is_required() {
    let dir = tempfile::tempdir().unwrap();
    let result = run_plan_final(dir.path(), None, &[]);

    assert_eq!(result.outcome, Ok(()));
    let event = &result.event;
    assert_eq!(event["ok"], true);
    assert_eq!(event["primary_reason"], "ok");
    assert_eq!(event["external_contract_required"], false);
    assert_eq!(event["external_contract_ok"], true);
    assert_eq!(event["runtime_acceptance_status"], "not_checked");
    assert_eq!(event["runtime_acceptance_passed"], true);
    assert_eq!(event["release_gate_status"], "not_applicable");
    assert_eq!(event["handoff_saved_not_success"], false);
    assert_eq!(event["recovery_handoff_saved"], false);
}

// kills: acceptance.rs:2287 `||` -> `&&` (bound.required alone makes the contract required)
// kills: acceptance.rs:2289 delete ! (a bound contract is checked, so binding is not missing)
// kills: acceptance.rs:2289 `&&` -> `||`
#[test]
fn plan_final_contract_passes_with_a_required_bound_contract() {
    let dir = tempfile::tempdir().unwrap();
    let result = run_plan_final(dir.path(), Some((json!({}), true)), &[]);

    assert_eq!(result.outcome, Ok(()));
    assert_eq!(result.event["external_contract_required"], true);
    assert_eq!(result.event["external_contract_ok"], true);
    assert_eq!(result.event["primary_reason"], "ok");
    assert_eq!(result.event["ok"], true);
}

// kills: acceptance.rs:2306-2310 `&&` -> `||` in `ok` (a missing artifact alone must fail)
// kills: acceptance.rs:2345 delete ! (missing artifacts must name the missing path)
// kills: acceptance.rs:2368 `||` -> `&&` (failure without a release-recovery need still saves a handoff)
#[test]
fn plan_final_contract_fails_for_a_missing_final_artifact_alone() {
    let dir = tempfile::tempdir().unwrap();
    let result = run_plan_final(dir.path(), None, &["nope.txt"]);

    assert_eq!(
        result.outcome,
        Err("plan final contract failed: missing final artifacts: nope.txt".to_string())
    );
    let event = &result.event;
    assert_eq!(event["ok"], false);
    assert_eq!(event["external_contract_ok"], true);
    assert_eq!(event["runtime_acceptance_passed"], true);
    assert_eq!(event["release_gate_status"], "not_applicable");
    assert_eq!(event["primary_reason"], "missing final artifacts: nope.txt");
    assert_eq!(event["handoff_saved_not_success"], true);
    assert_eq!(event["recovery_handoff_saved"], true);
}

// kills: acceptance.rs:2291 `&&` -> `||` (a failing external contract must not be treated as ok)
// kills: acceptance.rs:2355 delete ! (the external report supplies the primary reason)
#[test]
fn plan_final_contract_fails_when_only_the_external_contract_fails() {
    let dir = tempfile::tempdir().unwrap();
    let result = run_plan_final(
        dir.path(),
        Some((json!({"verify_commands": ["false"]}), false)),
        &[],
    );

    let message = result.outcome.unwrap_err();
    assert!(
        message.starts_with("plan final contract failed: command failed: false"),
        "{message}"
    );
    let event = &result.event;
    assert_eq!(event["ok"], false);
    assert_eq!(event["external_contract_ok"], false);
    assert_eq!(event["runtime_acceptance_status"], "not_checked");
    assert_eq!(event["release_gate_status"], "not_applicable");
    assert!(
        event["primary_reason"]
            .as_str()
            .unwrap()
            .starts_with("command failed: false"),
        "{}",
        event["primary_reason"]
    );
    assert_eq!(event["handoff_saved_not_success"], true);
}

// kills: acceptance.rs:2235 `||` -> `&&` and delete ! (required evidence alone requires runtime acceptance)
// kills: acceptance.rs:2236 `||` -> `&&`
// kills: acceptance.rs:2352 delete ! (a failing runtime report supplies the primary reason)
// kills: acceptance.rs:2308 `&&` -> `||` (runtime_ok failing alone must fail)
#[test]
fn plan_final_contract_requires_runtime_acceptance_for_evidence_alone() {
    let dir = tempfile::tempdir().unwrap();
    let result = run_plan_final(
        dir.path(),
        Some((
            json!({"required_evidence": ["implementation_artifact"]}),
            false,
        )),
        &[],
    );

    assert_eq!(
        result.outcome,
        Err(
            "plan final contract failed: missing_required_evidence:implementation_artifact"
                .to_string()
        )
    );
    let event = &result.event;
    assert_eq!(event["runtime_acceptance_status"], "failed");
    assert_eq!(event["runtime_acceptance_passed"], false);
    assert_eq!(event["release_gate_status"], "failed");
    assert_eq!(
        event["required_evidence"],
        json!(["implementation_artifact"])
    );
    assert_eq!(
        event["missing_evidence"],
        json!(["implementation_artifact"])
    );
    assert_eq!(event["required_capabilities"], json!([]));
    assert_eq!(event["required_obligations"], json!([]));
}

// kills: acceptance.rs:2234 delete ! and 2235 `||` -> `&&` (a capability with no evidence mapping)
#[test]
fn plan_final_contract_requires_runtime_acceptance_for_capabilities_alone() {
    let dir = tempfile::tempdir().unwrap();
    let result = run_plan_final(
        dir.path(),
        Some((json!({"required_capabilities": ["zz_unmapped"]}), false)),
        &[],
    );

    assert_eq!(
        result.outcome,
        Err(
            "plan final contract failed: missing_required_capabilities:unsupported_required_capability:zz_unmapped"
                .to_string()
        )
    );
    let event = &result.event;
    assert_eq!(event["runtime_acceptance_status"], "failed");
    assert_eq!(event["release_gate_status"], "failed");
    assert_eq!(event["required_capabilities"], json!(["zz_unmapped"]));
    assert_eq!(event["required_evidence"], json!([]));
    assert_eq!(event["required_obligations"], json!([]));
    assert_eq!(
        event["missing_capabilities"],
        json!(["unsupported_required_capability:zz_unmapped"])
    );
}

// kills: acceptance.rs:2236 `||` -> `&&` and delete ! (obligations alone require runtime acceptance)
#[test]
fn plan_final_contract_requires_runtime_acceptance_for_obligations_alone() {
    let dir = tempfile::tempdir().unwrap();
    let failing = run_plan_final(
        dir.path(),
        Some((json!({"required_obligations": ["setup"]}), false)),
        &[],
    );

    assert_eq!(
        failing.outcome,
        Err("plan final contract failed: missing_required_obligations:setup".to_string())
    );
    assert_eq!(failing.event["runtime_acceptance_status"], "failed");
    assert_eq!(failing.event["missing_obligations"], json!(["setup"]));
    assert_eq!(failing.event["required_capabilities"], json!([]));
    assert_eq!(failing.event["required_evidence"], json!([]));

    // Success side of the pair: the same obligation is met once package.json exists.
    let ready = tempfile::tempdir().unwrap();
    std::fs::write(ready.path().join("package.json"), "{}\n").unwrap();
    let passing = run_plan_final(
        ready.path(),
        Some((json!({"required_obligations": ["setup"]}), false)),
        &[],
    );

    assert_eq!(passing.outcome, Ok(()));
    assert_eq!(passing.event["runtime_acceptance_status"], "pass");
    assert_eq!(passing.event["runtime_acceptance_passed"], true);
    assert_eq!(passing.event["missing_obligations"], json!([]));
    assert_eq!(passing.event["primary_reason"], "ok");
}

// kills: acceptance.rs:2352 delete ! (the runtime report wins over a failing external report)
#[test]
fn plan_final_contract_reports_the_runtime_failure_before_the_external_failure() {
    let dir = tempfile::tempdir().unwrap();
    let result = run_plan_final(
        dir.path(),
        Some((
            json!({
                "required_evidence": ["implementation_artifact"],
                "verify_commands": ["false"]
            }),
            false,
        )),
        &[],
    );

    assert_eq!(
        result.outcome,
        Err(
            "plan final contract failed: missing_required_evidence:implementation_artifact"
                .to_string()
        )
    );
    assert_eq!(result.event["external_contract_ok"], false);
    assert_eq!(result.event["runtime_acceptance_status"], "failed");
}

// kills: acceptance.rs:250 replace -> vec![] / vec![String::new()] / vec!["xyzzy".into()]
#[test]
fn runtime_required_evidence_returns_the_runtime_evidence_for_the_capabilities() {
    let runtime = resolve_profile_runtime("generic");

    assert_eq!(
        runtime_required_evidence(
            runtime,
            "Write a small note",
            &["implementation".to_string()]
        ),
        vec!["implementation_artifact".to_string()]
    );
    assert_eq!(
        runtime_required_evidence(
            runtime,
            "Write a small note",
            &[
                "requested_content".to_string(),
                "implementation".to_string()
            ]
        ),
        runtime.required_evidence(
            "Write a small note",
            &[
                "requested_content".to_string(),
                "implementation".to_string()
            ]
        )
    );
    assert!(runtime_required_evidence(runtime, "Write a small note", &[]).is_empty());
}

// kills: acceptance.rs:1085 `<` -> `==`, `<` -> `>`, `<` -> `<=`; replace -> None
#[test]
fn dev_route_failure_kind_is_none_below_http_400_and_http_status_from_400() {
    for status in [200, 204, 302, 399] {
        assert_eq!(
            classify_dev_route_failure_kind(status, "ok"),
            None,
            "{status}"
        );
    }
    assert_eq!(
        classify_dev_route_failure_kind(400, "bad request"),
        Some("http_400".to_string())
    );
    assert_eq!(
        classify_dev_route_failure_kind(404, "not found"),
        Some("http_404".to_string())
    );
    assert_eq!(
        classify_dev_route_failure_kind(500, ""),
        Some("http_500".to_string())
    );
}

// kills: acceptance.rs:1085 replace -> Some(String::new()) / Some("xyzzy".into())
#[test]
fn dev_route_failure_kind_prefers_the_tailwind_pipeline_failure_for_error_statuses() {
    let body = "Module parse failed: Unexpected character '@' in globals.css\n@tailwind base;";
    assert_eq!(
        classify_dev_route_failure_kind(500, body),
        Some("tailwind_dev_pipeline_failure".to_string())
    );
    assert_eq!(
        classify_dev_route_failure_kind(400, "@TAILWIND base failed in PostCSS"),
        Some("tailwind_dev_pipeline_failure".to_string())
    );
    // A successful status never reports a failure kind, whatever the body says.
    assert_eq!(classify_dev_route_failure_kind(200, body), None);
}

// The bounded runner passes only allow-listed variables to a child, so the
// parent signals "run as child" with an `npm_config_` variable and hands the
// expectation, the marker and the probe variables over in a control file
// under the child's `HOME`.
const CHILD_FLAG: &str = "npm_config_issue507_child";
const CHILD_CONTROL: &str = "issue507-child-control";
const CHILD_STAMP: &str = "ISSUE507_CHILD_CHECKED";

/// Child half of the dev-server-probe checks. The probe switch reads process
/// environment variables, so the parent test starts this test in its own
/// process; mutating the shared test process would race with the other tests
/// that rely on the same switch. Outside a parent run it does nothing.
#[test]
fn dev_server_probe_runtime_enabled_child() {
    if std::env::var_os(CHILD_FLAG).is_none() {
        return;
    }
    let home = std::env::var("HOME").expect("child HOME");
    let control = std::fs::read_to_string(std::path::Path::new(&home).join(CHILD_CONTROL))
        .expect("child control file");
    let mut expect = None;
    let mut marker = false;
    for line in control.lines() {
        match line.split_once('=') {
            Some(("expect", value)) => expect = Some(value == "true"),
            Some(("marker", value)) => marker = value == "1",
            Some(("env", pair)) => {
                let (key, value) = pair.split_once('=').expect("env pair");
                // SAFETY: the parent runs this child alone with --test-threads=1.
                unsafe { std::env::set_var(key, value) };
            }
            _ => panic!("bad control line: {line}"),
        }
    }
    let expect = expect.expect("expect line");
    let dir = tempfile::tempdir().unwrap();
    if marker {
        std::fs::create_dir_all(dir.path().join(".anvil")).unwrap();
        std::fs::write(dir.path().join(".anvil/enable-dev-server-probe-tests"), "1").unwrap();
    }
    let config = generic_config(dir.path());
    let got = dev_server_probe_runtime_enabled(&config);

    println!("{CHILD_STAMP} expect={expect} got={got}");
    assert_eq!(got, expect, "expected enabled={expect}");
}

fn assert_dev_server_probe_enabled(expect: bool, marker: bool, env: &[(&str, &str)]) {
    let child_name = format!(
        "{}::dev_server_probe_runtime_enabled_child",
        module_path!().split_once("::").unwrap().1
    );
    let home = tempfile::tempdir().unwrap();
    let mut control = format!("expect={expect}\nmarker={}\n", u8::from(marker));
    for (key, value) in env {
        control.push_str(&format!("env={key}={value}\n"));
    }
    std::fs::write(home.path().join(CHILD_CONTROL), control).unwrap();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            child_name.as_str(),
            "--exact",
            "--nocapture",
            "--test-threads=1",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("HOME", home.path())
        .env(CHILD_FLAG, "1");
    let output =
        crate::bounded_process::run_with_timeout(&mut command, Duration::from_secs(120)).unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        output.status.is_some_and(|status| status.success()),
        "{text}"
    );
    let stamp = format!("{CHILD_STAMP} expect={expect} got={expect}");
    assert!(text.contains(&stamp), "child did not decide: {text}");
}

// kills: acceptance.rs:899 replace -> true; 903:12 delete !; 904:12 delete !
#[test]
fn dev_server_probe_is_disabled_in_tests_without_an_opt_in() {
    assert_dev_server_probe_enabled(false, false, &[]);
}

// kills: acceptance.rs:899 replace -> false; 904:9 `&&` -> `||`; 904:12 delete !
#[test]
fn dev_server_probe_is_enabled_by_the_workspace_marker() {
    assert_dev_server_probe_enabled(true, true, &[]);
}

// kills: acceptance.rs:899 replace -> false; 903:9 `&&` -> `||`; 903:12 delete !
#[test]
fn dev_server_probe_is_enabled_by_the_test_environment_flag() {
    assert_dev_server_probe_enabled(true, false, &[("COMMANDAGENT_TEST_DEV_SERVER_PROBE", "1")]);
    assert_dev_server_probe_enabled(
        true,
        true,
        &[("COMMANDAGENT_TEST_DEV_SERVER_PROBE", "true")],
    );
}

// An explicit opt-out wins over both opt-ins.
#[test]
fn dev_server_probe_explicit_opt_out_beats_the_test_opt_ins() {
    assert_dev_server_probe_enabled(
        false,
        true,
        &[
            ("COMMANDAGENT_TEST_DEV_SERVER_PROBE", "1"),
            ("COMMANDAGENT_DEV_SERVER_PROBE", "0"),
        ],
    );
}
