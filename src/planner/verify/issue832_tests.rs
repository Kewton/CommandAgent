//! #832: the catalog verify of the `nextjs-port-scripts` template is the dev-required form the
//! classifier admits, so the template is no longer refused by the host's own admission.
use std::path::Path;

use clap::Parser;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{
    NodeDependencySetupAuthority, PlanStep, normalize_verify_command_for_oracle_repair,
    package_json_port_script_check_command,
    verify_step_with_setup_observed_with_offline_and_events,
};
use crate::cli::Cli;
use crate::config::Config;
use crate::minimal_loop::evidence::command_diagnosis;
use crate::planner::adjudication::contract::IntentId;
use crate::planner::capability_catalog::{ResolvedCapability, resolve};
use crate::planner::profile::resolve_profile_runtime;
use crate::planner::profile_descriptor::NEXTJS_PROFILE_ID;
use crate::planner::profiles::nextjs::package_script_check::{
    Check, PortScript, evaluate, legacy_inline_command,
};
use crate::planner::recovery_step_plan_binding::admission::{Admission, Decision};
use crate::planner::step_plan::repair_generated_step_plan_contract;

const FIXTURE: &str = "tests/corpus/apps/issue832-port-script-template-admission";
const DEV_ONLY_BUILDER_3011: &str = "node -p \"['dev'].every(function(k){return String(Object(require('./package.json').scripts)[k]).split(' ').some(function(t,i,a){return t=='next' ? a.slice(i+1).find(function(x){return x})==k : false}) ? String(Object(require('./package.json').scripts)[k]).split(' ').some(function(t,i,a){return t=='--port=3011' ? true : t=='-p' ? a.slice(i+1).find(function(x){return x})=='3011' : t=='-p3011' ? true : t=='--port' ? a.slice(i+1).find(function(x){return x})=='3011' : false}) : false}) ? true : process.exit(1)\"";

/// Issue #879: the host now assembles the dev check as this token, judged in Rust.
const DEV_TOKEN_3011: &str = "anvil-nextjs-check:package_script_port:dev:3011";

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(FIXTURE)
            .join(name),
    )
    .unwrap()
}

fn recorded() -> Value {
    serde_json::from_str(&fixture("template-plan.json")).unwrap()
}

fn catalog_command(port: i64) -> String {
    let mut params = toml::value::Table::new();
    params.insert("port".to_string(), toml::Value::Integer(port));
    match resolve("package_json_port_script", &params).unwrap() {
        ResolvedCapability::ShellCheck(command) => command,
        other => panic!("package_json_port_script resolved to {other:?}"),
    }
}

fn config(root: &Path) -> Config {
    let mut config = Config::from_cli(Cli::parse_from(["commandagent", "--ux-demo"])).unwrap();
    config.workspace_root = root.to_path_buf();
    config.profile = NEXTJS_PROFILE_ID.to_string();
    config.intent_override = Some(IntentId::Create);
    config.eval_events_path = Some(root.join("events.jsonl"));
    config
}

fn node_available() -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join("node").is_file()))
}

/// Run the check through the production verifier in a workspace whose `package.json` holds the
/// given scripts; `None` leaves the workspace without a `package.json`.
fn check_passes(command: &str, scripts: Option<(&str, &str)>) -> bool {
    let dir = tempfile::tempdir().unwrap();
    if let Some((dev, start)) = scripts {
        let manifest = serde_json::json!({ "scripts": { "dev": dev, "start": start } });
        std::fs::write(dir.path().join("package.json"), manifest.to_string()).unwrap();
    }
    let step = PlanStep {
        id: "verify-port".to_string(),
        kind: "verify".to_string(),
        expected_result: "pass".to_string(),
        instruction: "Check the package.json port scripts".to_string(),
        expected_paths: Vec::new(),
        verify: vec![command.to_string()],
    };
    verify_step_with_setup_observed_with_offline_and_events(
        dir.path(),
        &step,
        NodeDependencySetupAuthority::None,
        false,
        None,
    )
    .0
    .is_pass()
}

#[test]
fn issue832_manifest_pins_the_recorded_template_plan() {
    let manifest: Value = serde_json::from_str(&fixture("source-manifest.json")).unwrap();
    for (name, digest) in manifest["fixture_sha256"].as_object().unwrap() {
        let actual = Sha256::digest(fixture(name).as_bytes());
        assert_eq!(format!("{actual:x}"), digest.as_str().unwrap(), "{name}");
    }
    let plan = recorded();
    let rejected = plan["rejected_template_plan"]["steps"][1]["verify"][0]
        .as_str()
        .unwrap();
    assert!(rejected.contains("['dev','start'].some("), "{rejected}");
}

#[test]
fn issue832_catalog_command_is_byte_identical_to_the_dev_required_builder() {
    assert_eq!(
        package_json_port_script_check_command("3011"),
        DEV_TOKEN_3011
    );
    // The token stands for exactly the dev-required inline check recorded by #832.
    assert_eq!(
        legacy_inline_command(Check::PortScript {
            script: PortScript::Dev,
            port: 3011
        }),
        DEV_ONLY_BUILDER_3011
    );
    for port in [3011_i64, 4321] {
        assert_eq!(
            catalog_command(port),
            package_json_port_script_check_command(&port.to_string())
        );
        assert!(!catalog_command(port).contains("['dev','start']"));
    }
    // An out-of-range port can never be satisfied, so it fails closed: the token keeps the
    // text, does not parse, and the host judgement rejects it.
    let unparsable = package_json_port_script_check_command("99999");
    assert_eq!(
        unparsable,
        "anvil-nextjs-check:package_script_port:dev:99999"
    );
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("package.json"),
        r#"{"scripts":{"dev":"next dev -p 99999"}}"#,
    )
    .unwrap();
    assert!(evaluate(root.path(), &unparsable).is_err());
}

#[test]
fn issue832_catalog_command_is_static_syntax_without_an_immutable_failure() {
    let root = tempfile::tempdir().unwrap();
    for port in [3011_i64, 4321] {
        let command = catalog_command(port);
        let diagnoses = command_diagnosis::collect(root.path(), std::slice::from_ref(&command));
        assert_eq!(diagnoses[0].kind, "static_syntax", "{port}: {diagnoses:?}");
        assert_eq!(
            command_diagnosis::immutable_failure(&diagnoses),
            None,
            "{port}"
        );
    }
    let recorded_weak = recorded()["rejected_template_plan"]["steps"][1]["verify"][0]
        .as_str()
        .unwrap()
        .to_string();
    let weak = command_diagnosis::collect(root.path(), &[recorded_weak]);
    assert_eq!(weak[0].kind, "weak", "{weak:?}");
    assert!(command_diagnosis::immutable_failure(&weak).is_some());
}

#[test]
fn issue832_dev_required_strictness_table() {
    let command = package_json_port_script_check_command("3011");
    // Token logic holds without node: dev is the only script that is checked.
    assert_eq!(command, DEV_TOKEN_3011);
    let legacy = legacy_inline_command(Check::PortScript {
        script: PortScript::Dev,
        port: 3011,
    });
    assert!(legacy.starts_with("node -p \"['dev'].every("), "{legacy}");
    assert!(!legacy.contains("'start'"), "{legacy}");
    let have_node = node_available();
    if !have_node {
        eprintln!("SKIPPED: node is not available; only the Rust judgement of the table runs");
    }
    let table = [
        (Some(("next dev -p 3011", "next start")), true),
        (Some(("next dev --port 3011", "next start")), true),
        (Some(("next dev", "next start -p 3011")), false),
        (Some(("next dev -p 3000", "next start -p 3011")), false),
        (Some(("PORT=3011 next dev", "next start")), false),
        (None, false),
    ];
    for (scripts, expected) in table {
        // The Rust judgement of the token.
        let dir = tempfile::tempdir().unwrap();
        if let Some((dev, start)) = scripts {
            let manifest = serde_json::json!({ "scripts": { "dev": dev, "start": start } });
            std::fs::write(dir.path().join("package.json"), manifest.to_string()).unwrap();
        }
        assert_eq!(
            evaluate(dir.path(), &command).is_ok(),
            expected,
            "{scripts:?}"
        );
        // The former inline check gives the same verdict (node runs only when present).
        if have_node {
            assert_eq!(check_passes(&legacy, scripts), expected, "{scripts:?}");
        }
    }
}

#[test]
fn issue832_game_g_final_phase_is_admitted_with_the_nextjs_port_scripts_template() {
    // Mirrors the order of `deterministic_step_plan_for_phase` (src/planner/runner/driver.rs:692,
    // develop 2d3e78b0): template :706 -> repair_generated_step_plan_contract :717 ->
    // admission.strengthen :728 -> repair :729 -> canonicalize_create_plan :730 ->
    // admission.sanitize :736 -> admission.check :738. The real function and the
    // `deterministic_step_plan_used` event are checked by the G1 GAME rerun.
    let recorded = recorded();
    let prompt = format!(
        "Original ultra goal: {}\nProfile: nextjs\nStyle: default\nIntent: create\nPhase id: {}\nPhase task: {}",
        recorded["goal"].as_str().unwrap(),
        recorded["phase_id"].as_str().unwrap(),
        recorded["phase_task"].as_str().unwrap(),
    );
    let root = tempfile::tempdir().unwrap();
    let config = config(root.path());
    let runtime = resolve_profile_runtime(NEXTJS_PROFILE_ID);
    // Mirror `deterministic_step_plan_for_phase` up to the admission decision.
    let template = runtime
        .deterministic_step_plan(&prompt, root.path(), &prompt)
        .expect("a deterministic template is selected for the final phase");
    assert_eq!(
        template.template_id,
        recorded["template_id"].as_str().unwrap()
    );
    let mut plan = template.plan;
    let model_plan = plan.clone();
    repair_generated_step_plan_contract(&mut plan);
    let mut admission = Admission::default();
    admission.strengthen(&config, &mut plan);
    repair_generated_step_plan_contract(&mut plan);
    runtime.canonicalize_create_plan(&mut plan, true, true, config.eval_events_path.as_deref());
    admission.sanitize(&mut plan, Some(root.path()));
    let decision = admission
        .check(
            &config,
            Some("final-build-verification"),
            &model_plan,
            &mut plan,
            1,
        )
        .unwrap();
    if let Decision::Retry(feedback) = decision {
        panic!("the template must not be refused by its own admission: {feedback}");
    }
    let verify = plan
        .steps
        .iter()
        .flat_map(|step| step.verify.iter())
        .collect::<Vec<_>>();
    assert!(
        verify.iter().any(|command| **command == DEV_TOKEN_3011),
        "{verify:?}"
    );
}

#[test]
fn issue832_grep_port_repair_stays_a_package_script_assertion_with_the_port_flag() {
    let repair = normalize_verify_command_for_oracle_repair(r#"grep -q '"3011"' package.json"#)
        .expect("the port-only grep is repaired");
    assert_eq!(repair.kind, "package_json_script_assertion", "{repair:?}");
    assert_eq!(repair.normalized, DEV_TOKEN_3011, "{repair:?}");
    assert_eq!(
        repair.normalized,
        package_json_port_script_check_command("3011"),
        "{repair:?}"
    );
}
