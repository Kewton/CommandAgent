//! Issue #543 focused self-check (L-14): every row of the H-09 `checks-543.md`
//! list is exercised with fake values only. The shared run-scoped exact-value
//! catalog is connected to the custom save boundaries and the runs display; a
//! runnable plan that contains a registered secret is refused honestly, the
//! stored run record bytes are never rewritten, and a thread with no
//! thread-local scope still redacts through the workspace registry.

use std::process::Command;

use clap::Parser;
use commandagent::cli::Cli;
use commandagent::config::{Config, ProviderCliOptions, RunsRequest};
use commandagent::minimal_loop::browser_probe::{
    BrowserReadinessObservation, browser_readiness_evidence_path, write_browser_readiness_evidence,
};
use commandagent::minimal_loop::build_verifier::{CompileError, write_build_verifier_output};
use commandagent::minimal_loop::interaction_probe::{
    browser_interaction_evidence_path, write_interaction_value,
};
use commandagent::planner::repair::{
    RecoveryHandoff, RepairContext, save_recovery_ultra_plan, save_repair_report_with_context,
    save_ultra_recovery_prompt,
};
use commandagent::planner::step_plan::{PlanStep, StepPlan, parse_step_plan};
use commandagent::planner::ultra_plan::{UltraPhase, UltraPlan, parse_ultra_plan};
use commandagent::planner::verify::VerificationReport;
use commandagent::planner::{run_plan_file, save_step_plan, save_ultra_plan};
use commandagent::providers::{AssistantReply, ChatClient};
use commandagent::runs::{render_runs_request, render_runs_table};
use commandagent::sensitive_data::{
    SecretCatalog, active_marker, install_scope, reset_scopes_for_tests,
    runnable_yaml_contains_secret, set_current,
};
use commandagent::state::ConversationMessage;
use commandagent::tools::registry::ToolSpec;
use commandagent::workflow::orchestrator::emit;
use commandagent::workflow::runner::write_node_event;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[derive(Clone)]
struct NoopClient;

impl ChatClient for NoopClient {
    fn label(&self) -> &str {
        "issue543-noop"
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
        Ok(AssistantReply::text("noop"))
    }
}

// Fake values from checks-543.md. No real key, dotenv, or model is used.
const C1: &str = "H01_CANARY_JwtStyle_NonPrefix_29486";
const C2: &str = "H09_日本語_Canary_秘密値_5521";
const C3: &str = "H09_Q\"uo\\te_Canary_7731";
const C4: &str = "H09_LINE_alpha\nH09_LINE_beta";
const C5: &str = "redacted";
const C6_EVENT: &str = "run_star";
const C6_ADJ: &str = "orkflow_adjudicat";
const C6_SCHEMA: &str = "ommandagent.runs/v";
const C7: &str = "8675309012345";
const C9: &str = "H10_SPACE_alpha H10_SPACE_beta";
const E1: &str = "H09_MODEL_Only_In_ENV_19832";
const E1_ENV: &str = "ISSUE543_L14_MODEL";
const DOCTOR_SECRET_ENV: &str = "ISSUE543_L14_DOCTOR";

fn all_values() -> [&'static str; 11] {
    [C1, C2, C3, C4, C5, C6_EVENT, C6_ADJ, C6_SCHEMA, C7, C9, E1]
}

fn install_all(root: &std::path::Path, events: &std::path::Path) {
    let mut catalog = SecretCatalog::new();
    for value in all_values() {
        // These are scrub-boundary values, including substrings of a fixed
        // identifier (C6_*). Issue #548 refuses such a value through the checked
        // credential API, so this self-check seeds the catalog directly with the
        // forced registration API; the refusal itself is covered by the #548
        // focused tests.
        catalog.register(value);
    }
    install_scope(catalog, Some(root), Some(events));
}

// The run scope registry is process-global, so tests that install or reset it
// take this lock; a concurrent reset would otherwise clear another test's
// registered scope mid-assertion.
static SCOPE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn begin_scope_test() -> std::sync::MutexGuard<'static, ()> {
    let guard = SCOPE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    reset_scopes_for_tests();
    guard
}

fn read(path: &std::path::Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Assert that none of the values (raw, 12-byte prefix, JSON escape form) occur.
fn assert_absent(haystack: &str, label: &str, values: &[&str]) {
    for value in values {
        assert!(
            !haystack.contains(value),
            "{label}: raw value leaked {value:?}\n{haystack}"
        );
        let mut end = value.len().min(12);
        while end > 0 && !value.is_char_boundary(end) {
            end -= 1;
        }
        if end > 0 {
            assert!(
                !haystack.contains(&value[..end]),
                "{label}: 12-byte prefix {:?} leaked\n{haystack}",
                &value[..end]
            );
        }
        let escaped = serde_json::to_string(value).unwrap();
        let inner = &escaped[1..escaped.len() - 1];
        if !inner.is_empty() {
            assert!(
                !haystack.contains(inner),
                "{label}: JSON escape form {inner:?} leaked\n{haystack}"
            );
        }
    }
}

fn marker() -> String {
    active_marker()
}

fn plan_with_secret(secret: &str) -> StepPlan {
    StepPlan {
        goal: format!("goal {secret}"),
        steps: vec![PlanStep {
            id: "step-1".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: format!("do the work with {secret}"),
            expected_paths: vec![],
            verify: vec![],
        }],
    }
}

fn readiness_observation(root: &std::path::Path) -> BrowserReadinessObservation {
    BrowserReadinessObservation {
        ok: true,
        status: "ready".to_string(),
        profile: "generic".to_string(),
        port: 3000,
        route: "/".to_string(),
        command: "next start".to_string(),
        http_status: Some(200),
        failure_kind: String::new(),
        evidence_path: browser_readiness_evidence_path(root),
        elapsed_ms: 12,
        output_excerpt: "listening".to_string(),
        build_output_path: String::new(),
        compile_errors: vec![],
        child_spawned: true,
        child_reaped: true,
        has_canvas: false,
        interactive_control_count: 0,
        title_text_excerpt: "title".to_string(),
    }
}

// ---------------------------------------------------------------- rows 1-3

#[test]
fn r01_build_log_body_is_scrubbed_before_write() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let events = root.join(".commandagent/runs/r01/events.jsonl");
    install_all(root, &events);

    let output = format!("compiling {C1}\nmid {C2}\n{C4}\nwarning: none\n");
    let path = write_build_verifier_output(root, "npm run build", &output).unwrap();
    let text = read(&path);
    assert_absent(&text, "r01 build log", &[C1, C2, C4]);
    assert!(text.contains(&marker()), "{text}");
    reset_scopes_for_tests();
}

#[test]
fn r02_build_log_file_name_has_no_secret_or_slug() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let events = root.join(".commandagent/runs/r02/events.jsonl");
    install_all(root, &events);

    let command = format!("npm run build --token {C1} --user {C2}");
    let path = write_build_verifier_output(root, &command, "ok\n").unwrap();
    let name = path.file_name().unwrap().to_string_lossy().to_string();
    // The boundary replaces the secret with the marker before slugging the name
    // (safe-name replacement, not refusal); neither the value nor its slug form
    // survives in the file name or the returned path.
    assert_absent(&name, "r02 file name", &[C1, C2]);
    assert!(!name.to_lowercase().contains("canary"), "{name}");
    assert!(!name.to_lowercase().contains("29486"), "{name}");
    assert!(
        !path.to_string_lossy().contains(C1),
        "returned path kept the secret"
    );
    reset_scopes_for_tests();
}

#[test]
fn r03_build_log_write_failure_returns_none_without_raw_retry() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    // Make the parent a regular file so create_dir_all fails.
    std::fs::write(root.join(".commandagent"), "not a directory").unwrap();
    let events = root.join("events.jsonl");
    install_all(root, &events);

    let result = write_build_verifier_output(root, "cmd", &format!("{C1}\n"));
    assert!(result.is_none(), "a failed write must not report success");
    assert!(!root.join(".commandagent/evidence").exists());
    reset_scopes_for_tests();
}

// ---------------------------------------------------------------- rows 4-6

#[test]
fn r04_browser_readiness_free_text_is_scrubbed_and_parses() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let events = root.join(".commandagent/runs/r04/events.jsonl");
    install_all(root, &events);

    let mut observation = readiness_observation(root);
    observation.command = format!("next start --token {C1} {C3}");
    observation.output_excerpt = format!("console {C1}");
    write_browser_readiness_evidence(root, &observation);

    let text = read(&browser_readiness_evidence_path(root));
    assert_absent(&text, "r04 readiness", &[C1, C3]);
    let value: Value = serde_json::from_str(&text).unwrap();
    for key in ["status", "route", "ok", "dev_server", "title_text_excerpt"] {
        assert!(value.get(key).is_some(), "r04 lost key {key}");
    }
    assert_eq!(value["http_status"], 200);
    reset_scopes_for_tests();
}

#[test]
fn r05_browser_readiness_fixed_key_value_is_scrubbed() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let events = root.join(".commandagent/runs/r05/events.jsonl");
    install_all(root, &events);

    // Free text that lands in a fixed schema key must still be scrubbed.
    let mut observation = readiness_observation(root);
    observation.status = format!("failed token={C1}");
    write_browser_readiness_evidence(root, &observation);
    let text = read(&browser_readiness_evidence_path(root));
    assert_absent(&text, "r05 readiness status", &[C1]);

    // A canonical enum value is unchanged.
    let mut clean = readiness_observation(root);
    clean.status = "ready".to_string();
    write_browser_readiness_evidence(root, &clean);
    let value: Value = serde_json::from_str(&read(&browser_readiness_evidence_path(root))).unwrap();
    assert_eq!(value["status"], "ready");
    assert_eq!(value["ok"], true);
    reset_scopes_for_tests();
}

#[test]
fn r06_browser_readiness_nested_containers_are_scrubbed() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let events = root.join(".commandagent/runs/r06/events.jsonl");
    install_all(root, &events);

    let mut observation = readiness_observation(root);
    observation.compile_errors = vec![
        CompileError {
            path: "src/app/page.tsx".to_string(),
            line: 1,
            column: 1,
            message: format!("bad {C1}"),
            excerpt: format!("excerpt {C2}"),
            symbol: None,
            route_bound: Some(false),
        },
        CompileError {
            path: "src/app/layout.tsx".to_string(),
            line: 2,
            column: 3,
            message: format!("worse {C2}"),
            excerpt: String::new(),
            symbol: None,
            route_bound: None,
        },
    ];
    write_browser_readiness_evidence(root, &observation);

    let text = read(&browser_readiness_evidence_path(root));
    assert_absent(&text, "r06 readiness nested", &[C1, C2]);
    let value: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["compile_errors"].as_array().unwrap().len(), 2);
    assert_eq!(value["compile_errors"][0]["path"], "src/app/page.tsx");
    reset_scopes_for_tests();
}

// ---------------------------------------------------------------- rows 7-8

#[test]
fn r07_interaction_json_and_mirror_are_scrubbed_and_equal() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let events = root.join(".commandagent/runs/r07/events.jsonl");
    install_all(root, &events);

    let evidence_path = root.join(".commandagent/evidence/custom-interaction.json");
    let value = json!({
        "status": "available",
        "output_excerpt": format!("probe {C1} {C3}"),
        "probe": {"message": format!("nested {C2}")},
    });
    write_interaction_value(root, &evidence_path, &value);

    let mirror = browser_interaction_evidence_path(root);
    assert_ne!(evidence_path, mirror);
    let evidence_text = read(&evidence_path);
    let mirror_text = read(&mirror);
    assert_absent(&evidence_text, "r07 evidence", &[C1, C2, C3]);
    assert_absent(&mirror_text, "r07 mirror", &[C1, C2, C3]);
    assert_eq!(evidence_text, mirror_text, "the two files must not differ");
    reset_scopes_for_tests();
}

#[test]
fn r08_interaction_dynamic_key_is_projected_without_collision() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let events = root.join(".commandagent/runs/r08/events.jsonl");
    // C5 is deliberately not registered so `<redacted>` stays an unrelated key.
    let mut catalog = SecretCatalog::new();
    catalog.register(C1);
    catalog.register(C2);
    install_scope(catalog, Some(root), Some(&events));

    let evidence_path = root.join(".commandagent/evidence/r08.json");
    // A dynamic key carrying C1, an unrelated marker-named key that must not be
    // renamed, and two long values.
    let value = json!({
        C1: "v1",
        "keep": C2,
        "<redacted>": "unrelated",
    });
    write_interaction_value(root, &evidence_path, &value);

    let text = read(&evidence_path);
    assert_absent(&text, "r08 evidence", &[C1, C2]);
    let parsed: Value = serde_json::from_str(&text).unwrap();
    let object = parsed.as_object().unwrap();
    assert_eq!(object.len(), 3, "element count must be preserved: {text}");
    assert!(
        object.contains_key("<redacted>"),
        "unrelated key renamed: {text}"
    );
    assert!(
        object.keys().any(|key| key.contains("#1")),
        "the projected key must be disambiguated: {text}"
    );
    reset_scopes_for_tests();
}

// --------------------------------------------------------------- rows 9-13

#[test]
fn r09_step_plan_refuses_secret_command() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let events = root.join(".commandagent/runs/r09/events.jsonl");
    install_all(root, &events);

    let mut plan = plan_with_secret("clean goal");
    plan.steps[0].verify = vec![format!("echo {C1}")];
    let error = save_step_plan(root, &plan).unwrap_err();
    assert_absent(&format!("{error}"), "r09 refusal", &[C1]);
    assert_absent(&format!("{error:?}"), "r09 refusal debug", &[C1]);
    let saved = std::fs::read_dir(root.join(".commandagent/plans"))
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(saved, 0, "a refused plan must not be saved");

    // The execution boundary refuses a hand-authored runnable plan with the same
    // honest reason and never runs it.
    let cwd = root.to_string_lossy().to_string();
    let config = Config::from_cli(Cli::parse_from([
        "commandagent",
        "--cwd",
        &cwd,
        "--model",
        "m",
    ]))
    .unwrap();
    install_all(root, &events);
    let plan_path = root.join("secret-plan.yaml");
    std::fs::write(
        &plan_path,
        format!(
            "goal: clean\nsteps:\n  - id: s1\n    kind: verify\n    expected_result: pass\n    instruction: run\n    verify:\n      - echo {C1}\n"
        ),
    )
    .unwrap();
    let mut client = NoopClient;
    let error = run_plan_file(&mut client, &plan_path, &config).unwrap_err();
    assert_absent(&format!("{error}"), "r09 run refusal", &[C1]);
    reset_scopes_for_tests();
}

#[test]
fn r10_numeric_secret_in_yaml_is_refused_by_the_boundary() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let events = root.join(".commandagent/runs/r10/events.jsonl");
    install_all(root, &events);

    // Issue #548 (hole 1) fixed the shared predicate: a numeric-only secret read
    // as a Number is now detected, not missed.
    let catalog = {
        let mut catalog = SecretCatalog::new();
        catalog.register(C7);
        catalog
    };
    let numeric_yaml = format!("args: [--token, {C7}]");
    assert!(
        runnable_yaml_contains_secret(&catalog, &numeric_yaml),
        "the shared predicate still misses a Number scalar"
    );

    // The #543 boundary refuses it through the same shared predicate, so no
    // numeric secret is persisted by this scope.
    let error = save_step_plan(root, &plan_with_command(C7)).unwrap_err();
    assert_absent(&format!("{error}"), "r10 refusal", &[C7]);
    reset_scopes_for_tests();
}

fn plan_with_command(command: &str) -> StepPlan {
    let mut plan = plan_with_secret("clean goal");
    plan.steps[0].verify = vec![command.to_string()];
    plan
}

#[test]
fn r11_step_plan_free_text_is_scrubbed_and_structure_preserved() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let events = root.join(".commandagent/runs/r11/events.jsonl");
    install_all(root, &events);

    let plan = StepPlan {
        goal: format!("goal {C1} {C4}"),
        steps: vec![
            PlanStep {
                id: "s1".to_string(),
                kind: "implement".to_string(),
                expected_result: "pass".to_string(),
                instruction: format!("do {C1}"),
                expected_paths: vec!["src/a.ts".to_string()],
                verify: vec!["cargo test".to_string()],
            },
            PlanStep {
                id: "s2".to_string(),
                kind: "verify".to_string(),
                expected_result: "pass".to_string(),
                instruction: "verify".to_string(),
                expected_paths: vec![],
                verify: vec!["cargo test".to_string()],
            },
        ],
    };
    let path = save_step_plan(root, &plan).unwrap();
    let text = read(&path);
    assert_absent(&text, "r11 step plan", &[C1, C4]);
    let reloaded = parse_step_plan(&text).unwrap();
    assert_eq!(reloaded.steps.len(), 2);
    assert_eq!(reloaded.steps[0].id, "s1");
    assert_eq!(reloaded.steps[1].kind, "verify");
    assert_eq!(reloaded.steps[0].verify, vec!["cargo test"]);
    assert_eq!(reloaded.steps[0].expected_paths, vec!["src/a.ts"]);
    reset_scopes_for_tests();
}

#[test]
fn r12_step_plan_marker_switch_keeps_string_scalars() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let events = root.join(".commandagent/runs/r12/events.jsonl");
    install_all(root, &events);
    // C5 forces the marker to `[hidden]`.
    assert_eq!(marker(), "[hidden]");

    let path = save_step_plan(root, &plan_with_secret(C1)).unwrap();
    let text = read(&path);
    assert_absent(&text, "r12 step plan", &[C1]);
    let reloaded = parse_step_plan(&text).unwrap();
    assert_eq!(reloaded.steps[0].kind, "implement");
    assert!(
        reloaded.steps[0].instruction.contains("[hidden]"),
        "{}",
        reloaded.steps[0].instruction
    );
    reset_scopes_for_tests();
}

#[test]
fn r13_secret_free_step_plan_bytes_are_unchanged() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let events = root.join(".commandagent/runs/r13/events.jsonl");
    install_all(root, &events);

    let plan = StepPlan {
        goal: "goal".to_string(),
        steps: vec![PlanStep {
            id: "s1".to_string(),
            kind: "report".to_string(),
            expected_result: "pass".to_string(),
            instruction: "summarize".to_string(),
            expected_paths: vec![],
            verify: vec![],
        }],
    };
    let with_scope = read(&save_step_plan(root, &plan).unwrap());
    reset_scopes_for_tests();
    let without_scope = read(&save_step_plan(root, &plan).unwrap());
    assert_eq!(with_scope, without_scope, "a secret-free plan changed");
    reset_scopes_for_tests();
}

// --------------------------------------------------------------- rows 14-15

#[test]
fn r14_ultra_plan_refuses_secret_phase_command() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let events = root.join(".commandagent/runs/r14/events.jsonl");
    install_all(root, &events);

    for secret in [C1, C3] {
        let plan = UltraPlan {
            goal: format!("goal {secret}"),
            profile: "generic".to_string(),
            style: "default".to_string(),
            intent: "create".to_string(),
            phases: vec![UltraPhase {
                id: "phase-1".to_string(),
                prompt: format!("npm run build --token {secret}"),
            }],
        };
        let error = save_ultra_plan(root, &plan).unwrap_err();
        assert_absent(&format!("{error}"), "r14 refusal", &[secret]);
    }
    let saved = std::fs::read_dir(root.join(".commandagent/plans"))
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(saved, 0, "a refused ultra plan must not be saved");

    // A secret-free ultra plan still saves and reloads identically.
    let clean = UltraPlan {
        goal: "goal".to_string(),
        profile: "generic".to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: vec![UltraPhase {
            id: "phase-1".to_string(),
            prompt: "prompt".to_string(),
        }],
    };
    let path = save_ultra_plan(root, &clean).unwrap();
    assert_eq!(parse_ultra_plan(&read(&path)).unwrap(), clean);
    reset_scopes_for_tests();
}

#[test]
fn r15_ultra_plan_refuses_secret_in_a_key_position() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let events = root.join(".commandagent/runs/r15/events.jsonl");
    install_all(root, &events);

    // A secret smuggled through the profile/intent slot still refuses, and the
    // refusal message never echoes the value.
    let plan = UltraPlan {
        goal: "goal".to_string(),
        profile: C1.to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: vec![UltraPhase {
            id: "phase-1".to_string(),
            prompt: "prompt".to_string(),
        }],
    };
    let error = save_ultra_plan(root, &plan).unwrap_err();
    assert_absent(&format!("{error}"), "r15 refusal", &[C1]);
    reset_scopes_for_tests();
}

// --------------------------------------------------------------- rows 16-19

#[test]
fn r16_repair_report_is_scrubbed_before_write() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let events = root.join(".commandagent/runs/r16/events.jsonl");
    install_all(root, &events);

    let mut report = VerificationReport::pass();
    report.push_command_failure(format!("npm run build {C1}"), format!("failed {C2}"));
    let context = RepairContext {
        overall_goal: Some(format!("goal {C4}")),
        verify_commands: vec![format!("cargo test {C1}")],
        ..RepairContext::default()
    };
    let path = save_repair_report_with_context(root, "step-1", &report, &context).unwrap();
    let text = read(&path);
    assert_absent(&text, "r16 repair report", &[C1, C2, C4]);
    for heading in [
        "# Repair exhausted",
        "## Command Failures",
        "## Step Contract",
    ] {
        assert!(
            text.contains(heading),
            "r16 lost structure {heading}:\n{text}"
        );
    }
    reset_scopes_for_tests();
}

#[test]
fn r17_recovery_prompt_is_scrubbed() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let events = root.join(".commandagent/runs/r17/events.jsonl");
    install_all(root, &events);

    let handoff = RecoveryHandoff {
        original_goal: format!("recover {C2}"),
        failure_kind: "compile".to_string(),
        failure_evidence: vec![format!("stdout {C1}"), C4.to_string()],
        missing_paths: vec![format!("src/{C1}.ts")],
        verify_commands: vec![format!("cargo test {C1}")],
        ..RecoveryHandoff::default()
    };
    let path = save_ultra_recovery_prompt(root, "step-1", &handoff).unwrap();
    let text = read(&path);
    assert_absent(&text, "r17 recovery prompt", &[C1, C2, C4]);
    assert!(text.contains("Original goal:"), "{text}");
    reset_scopes_for_tests();
}

#[test]
fn r18_recovery_yaml_refuses_a_secret_command() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let events = root.join(".commandagent/runs/r18/events.jsonl");
    install_all(root, &events);

    let handoff = RecoveryHandoff {
        original_goal: "recover".to_string(),
        failure_kind: "compile".to_string(),
        verify_commands: vec![format!("cargo test {C1}")],
        ..RecoveryHandoff::default()
    };
    let error = save_recovery_ultra_plan(root, "step-1", &handoff).unwrap_err();
    assert_absent(&format!("{error}"), "r18 refusal", &[C1]);
    let saved = std::fs::read_dir(root.join(".commandagent/plans"))
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(saved, 0, "a refused recovery plan must not be saved");
    reset_scopes_for_tests();
}

#[test]
fn r19_recovery_refusal_keeps_the_original_failure() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let events = root.join(".commandagent/runs/r19/events.jsonl");
    install_all(root, &events);

    let handoff = RecoveryHandoff {
        original_goal: "recover".to_string(),
        failure_kind: "release_gate".to_string(),
        verify_commands: vec![format!("cargo test {C1}")],
        ..RecoveryHandoff::default()
    };
    let error = save_recovery_ultra_plan(root, "step-1", &handoff).unwrap_err();
    // The refusal is honest: no success is reported and the failed save is not
    // silently dropped, so the caller keeps the original failure state.
    assert_absent(&format!("{error}"), "r19 refusal", &[C1]);
    assert!(
        !root
            .join(".commandagent/plans")
            .join("recovery-ultra-plan-step-1")
            .exists(),
        "a refused plan left a stray artifact"
    );
    reset_scopes_for_tests();
}

// --------------------------------------------------------------- rows 23-26

#[test]
fn r23_workflow_node_event_scrubs_an_env_model_value() {
    let exe = std::env::current_exe().unwrap();
    let status = Command::new(exe)
        .args([
            "--exact",
            "--ignored",
            "--nocapture",
            "r23_workflow_node_event_scrubs_an_env_model_value_child",
        ])
        .env(E1_ENV, E1)
        .status()
        .unwrap();
    assert!(status.success(), "r23 child exited with {status}");
}

#[test]
#[ignore]
fn r23_workflow_node_event_scrubs_an_env_model_value_child() {
    let dir = tempfile::tempdir().unwrap();
    let origin = dir.path();
    std::fs::create_dir_all(origin.join(".commandagent")).unwrap();
    std::fs::write(
        origin.join(".commandagent/config.toml"),
        format!("[preset.env]\nprovider = \"ollama\"\nmodel = \"${{{E1_ENV}}}\"\n"),
    )
    .unwrap();
    let cwd = origin.to_string_lossy().to_string();
    let config = Config::from_cli(Cli::parse_from([
        "commandagent",
        "--cwd",
        &cwd,
        "--preset",
        "env",
        "--offline",
    ]))
    .unwrap();
    assert_eq!(config.model, E1);

    let events = origin.join("evidence/node-events.jsonl");
    std::fs::create_dir_all(events.parent().unwrap()).unwrap();
    write_node_event(
        origin,
        &events,
        &json!({"event": "intent_resolved", "model": config.model, "provider": "ollama"}),
    )
    .unwrap();

    let text = read(&events);
    assert_absent(&text, "r23 node event", &[E1]);
    let value: Value = serde_json::from_str(text.trim()).unwrap();
    assert_eq!(value["event"], "intent_resolved");
    assert_eq!(value["provider"], "ollama");
}

#[test]
fn r24_workflow_node_nested_pins_are_scrubbed() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let origin = dir.path().join("origin");
    std::fs::create_dir_all(origin.join("evidence")).unwrap();
    let events = origin.join("evidence/node-events.jsonl");
    install_all(&origin, &events);

    write_node_event(
        &origin,
        &events,
        &json!({
            "event": "intent_resolved",
            "provider": "lm-studio",
            "pins": {"nested": {"model": format!("m {C1}"), "note": C2}},
        }),
    )
    .unwrap();

    let text = read(&events);
    assert_absent(&text, "r24 node event", &[C1, C2]);
    let value: Value = serde_json::from_str(text.trim()).unwrap();
    assert_eq!(value["event"], "intent_resolved");
    assert_eq!(value["provider"], "lm-studio");
    assert!(value["pins"]["nested"].get("model").is_some());
    reset_scopes_for_tests();
}

#[test]
fn r25_orchestrator_emit_scrubs_free_text_and_keeps_verdict() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let origin = dir.path().join("origin");
    std::fs::create_dir_all(origin.join("evidence")).unwrap();
    let events = origin.join("evidence/workflow-events.jsonl");
    install_all(&origin, &events);

    emit(&events, json!({"event": "workflow_started", "epoch": 1})).unwrap();
    emit(
        &events,
        json!({
            "event": "workflow_adjudicated",
            "verdict": "circle_full",
            "reason": format!("{C1} {C2}"),
            "origin": format!("/tmp/{C1}"),
            "entry": C4,
        }),
    )
    .unwrap();

    let text = read(&events);
    assert_absent(&text, "r25 workflow events", &[C1, C2, C4]);
    let values = text
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(values.len(), 2);
    assert_eq!(values[0]["event"], "workflow_started");
    assert_eq!(values[1]["event"], "workflow_adjudicated");
    assert_eq!(values[1]["verdict"], "circle_full");
    reset_scopes_for_tests();
}

#[test]
fn r26_orchestrator_keeps_fixed_event_and_envelope_names() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let origin = dir.path().join("origin");
    std::fs::create_dir_all(origin.join("evidence")).unwrap();
    let events = origin.join("evidence/workflow-events.jsonl");
    install_all(&origin, &events);

    emit(
        &events,
        json!({
            "event": "workflow_adjudicated",
            "verdict": "circle_full",
            "reason": "clean",
            "evidence_envelope": {"family": "workflow", "kind": "workflow_adjudicated"},
        }),
    )
    .unwrap();

    let value: Value = serde_json::from_str(read(&events).trim()).unwrap();
    assert_eq!(value["event"], "workflow_adjudicated");
    assert_eq!(value["verdict"], "circle_full");
    assert_eq!(value["evidence_envelope"]["kind"], "workflow_adjudicated");
    reset_scopes_for_tests();
}

// --------------------------------------------------------------- rows 27-32

fn fake_run(root: &std::path::Path, id: &str, events_body: &str) -> std::path::PathBuf {
    let run_dir = root.join(".commandagent/runs").join(id);
    std::fs::create_dir_all(&run_dir).unwrap();
    let events = run_dir.join("events.jsonl");
    std::fs::write(&events, events_body).unwrap();
    std::fs::write(
        run_dir.join("summary.md"),
        format!("Status: completed\nNotes: {C1}\n"),
    )
    .unwrap();
    events
}

#[test]
fn r27_runs_events_json_scrubs_an_old_record_without_rewriting_it() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let events = fake_run(
        root,
        "run-r27",
        &format!(
            "{{\"event\":\"run_start\",\"model\":\"{C1}\"}}\n\
             {{\"event\":\"run_stop\",\"ok\":true,\"status\":\"completed\",\"reason\":\"{C1}\"}}\n"
        ),
    );
    install_all(root, &events);
    let before = sha256(&std::fs::read(&events).unwrap());

    let json = render_runs_request(
        root,
        &RunsRequest {
            id: Some("run-r27".to_string()),
            events: true,
            filter: None,
            json: true,
        },
    )
    .unwrap();
    assert_absent(&json, "r27 runs events json", &[C1]);
    assert_eq!(before, sha256(&std::fs::read(&events).unwrap()));
    reset_scopes_for_tests();
}

#[test]
fn r28_runs_events_json_keeps_nested_event_names() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let events = fake_run(
        root,
        "run-r28",
        &format!(
            "{{\"event\":\"run_start\",\"model\":\"{C1}\"}}\n\
             {{\"event\":\"run_stop\",\"ok\":true,\"status\":\"completed\",\"reason\":\"{C1}\"}}\n"
        ),
    );
    install_all(root, &events);

    let json = render_runs_request(
        root,
        &RunsRequest {
            id: Some("run-r28".to_string()),
            events: true,
            filter: None,
            json: true,
        },
    )
    .unwrap();
    let value: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["schema_version"], "commandagent.runs/v1");
    let names = value["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["event"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["run_start", "run_stop"]);
    reset_scopes_for_tests();
}

#[test]
fn r29_runs_old_record_fixed_key_free_text_is_scrubbed() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let events = fake_run(
        root,
        "run-r29",
        &format!(
            "{{\"event\":\"run_stop\",\"ok\":true,\"status\":\"failed token={C1}\",\"tool_name\":\"{C1}\",\"action\":\"{C1}\"}}\n"
        ),
    );
    install_all(root, &events);

    for json_view in [false, true] {
        let view = render_runs_request(
            root,
            &RunsRequest {
                id: Some("run-r29".to_string()),
                events: true,
                filter: None,
                json: json_view,
            },
        )
        .unwrap();
        assert_absent(&view, "r29 runs events", &[C1]);
    }
    reset_scopes_for_tests();
}

#[test]
fn r30_runs_human_context_scrubs_before_truncation() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let padded = format!("{} {C1} tail", "x".repeat(70));
    let events = fake_run(
        root,
        "run-r30",
        &format!(
            "{{\"event\":\"run_stop\",\"ok\":true,\"status\":\"completed\",\"model\":\"{padded}\"}}\n"
        ),
    );
    install_all(root, &events);

    let human = render_runs_request(
        root,
        &RunsRequest {
            id: Some("run-r30".to_string()),
            events: true,
            filter: None,
            json: false,
        },
    )
    .unwrap();
    assert_absent(&human, "r30 human events", &[C1]);
    assert!(!human.contains("H01_CANARY"), "fragment survived: {human}");
    reset_scopes_for_tests();
}

#[test]
fn r31_runs_detail_and_list_scrub_and_keep_identity() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let events = fake_run(
        root,
        "run-r31",
        &format!(
            "{{\"event\":\"run_start\",\"model\":\"{C4}\"}}\n\
             {{\"event\":\"run_stop\",\"ok\":true,\"status\":\"completed\",\"reason\":\"{C1}\"}}\n"
        ),
    );
    install_all(root, &events);

    for json_view in [false, true] {
        let list = render_runs_request(
            root,
            &RunsRequest {
                id: None,
                events: false,
                filter: None,
                json: json_view,
            },
        )
        .unwrap();
        assert_absent(&list, "r31 runs list", &[C1, C4]);
        let detail = render_runs_request(
            root,
            &RunsRequest {
                id: Some("run-r31".to_string()),
                events: false,
                filter: None,
                json: json_view,
            },
        )
        .unwrap();
        assert_absent(&detail, "r31 runs detail", &[C1, C4]);
        assert!(detail.contains("run-r31"), "id lost: {detail}");
    }
    assert!(!render_runs_table(root).contains(C1));
    reset_scopes_for_tests();
}

#[test]
fn r32_two_workspace_display_scopes_do_not_cross_contaminate() {
    let _scope = begin_scope_test();
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let a_events = fake_run(
        a.path(),
        "run-a",
        &format!(
            "{{\"event\":\"run_start\",\"model\":\"{C2}\"}}\n\
             {{\"event\":\"run_stop\",\"ok\":true,\"status\":\"completed\",\"reason\":\"{C1}\"}}\n"
        ),
    );
    let b_events = fake_run(
        b.path(),
        "run-b",
        &format!(
            "{{\"event\":\"run_start\",\"model\":\"{C1}\"}}\n\
             {{\"event\":\"run_stop\",\"ok\":true,\"status\":\"completed\",\"reason\":\"{C2}\"}}\n"
        ),
    );
    // A knows C1 only; B knows C2 only.
    let mut a_catalog = SecretCatalog::new();
    a_catalog.register(C1);
    install_scope(a_catalog, Some(a.path()), Some(&a_events));
    let mut b_catalog = SecretCatalog::new();
    b_catalog.register(C2);
    install_scope(b_catalog, Some(b.path()), Some(&b_events));

    let a_view = render_runs_request(
        a.path(),
        &RunsRequest {
            id: Some("run-a".to_string()),
            events: true,
            filter: None,
            json: true,
        },
    )
    .unwrap();
    assert!(!a_view.contains(C1), "A leaked its own secret: {a_view}");
    assert!(a_view.contains(C2), "A imported B's catalog: {a_view}");

    let b_view = render_runs_request(
        b.path(),
        &RunsRequest {
            id: Some("run-b".to_string()),
            events: true,
            filter: None,
            json: true,
        },
    )
    .unwrap();
    assert!(!b_view.contains(C2), "B leaked its own secret: {b_view}");
    assert!(b_view.contains(C1), "B imported A's catalog: {b_view}");
    reset_scopes_for_tests();
}

// --------------------------------------------------------------- rows 33-34

#[test]
fn r33_doctor_hides_env_value_in_resolution_error() {
    let exe = std::env::current_exe().unwrap();
    let status = Command::new(exe)
        .args([
            "--exact",
            "--ignored",
            "--nocapture",
            "r33_doctor_hides_env_value_in_resolution_error_child",
        ])
        .env(DOCTOR_SECRET_ENV, C1)
        .status()
        .unwrap();
    assert!(status.success(), "r33 child exited with {status}");
}

#[test]
#[ignore]
fn r33_doctor_hides_env_value_in_resolution_error_child() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".commandagent")).unwrap();
    std::fs::write(
        dir.path().join(".commandagent/config.toml"),
        format!(
            "[preset.bad]\nmodel = \"issue543-model\"\nprovider = \"openai-compatible\"\nbase_url = \"${{{DOCTOR_SECRET_ENV}}}\"\n"
        ),
    )
    .unwrap();
    let cwd = dir.path().to_string_lossy().to_string();
    let cli = Cli::parse_from(["commandagent", "--cwd", &cwd, "--preset", "bad"]);
    let report = commandagent::doctor::diagnose_cli_with_provider_options(
        &cli,
        ProviderCliOptions::default(),
    )
    .unwrap();

    let human = report.render_human();
    let json = report.render_json().unwrap();
    assert_absent(&human, "r33 doctor human", &[C1]);
    assert_absent(&json, "r33 doctor json", &[C1]);
    assert!(
        human.contains("base-url") || json.contains("base-url"),
        "the base-url diagnosis was lost: {human}\n{json}"
    );
}

#[test]
fn r34_doctor_hides_env_value_in_a_successful_report() {
    let exe = std::env::current_exe().unwrap();
    let status = Command::new(exe)
        .args([
            "--exact",
            "--ignored",
            "--nocapture",
            "r34_doctor_hides_env_value_in_a_successful_report_child",
        ])
        .env(E1_ENV, E1)
        .status()
        .unwrap();
    assert!(status.success(), "r34 child exited with {status}");
}

#[test]
#[ignore]
fn r34_doctor_hides_env_value_in_a_successful_report_child() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".commandagent")).unwrap();
    std::fs::write(
        dir.path().join(".commandagent/config.toml"),
        format!("[preset.ok]\nprovider = \"ollama\"\nmodel = \"${{{E1_ENV}}}\"\n"),
    )
    .unwrap();
    let cwd = dir.path().to_string_lossy().to_string();
    let config = Config::from_cli(Cli::parse_from([
        "commandagent",
        "--cwd",
        &cwd,
        "--preset",
        "ok",
        "--offline",
    ]))
    .unwrap();
    assert_eq!(config.model, E1);

    let report = commandagent::doctor::diagnose(&config);
    let human = report.render_human();
    let json = report.render_json().unwrap();
    assert_absent(&human, "r34 doctor human", &[E1]);
    assert_absent(&json, "r34 doctor json", &[E1]);
    assert!(!report.checks.is_empty(), "the report lost its checks");
}

// ------------------------------------------------- H-10 blockers (L-15)

#[test]
fn b543_1_runs_events_json_scrubs_object_keys() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    // H-10: a secret in an object key survived the value-only scrub.
    let events = fake_run(
        root,
        "run-b543-1",
        &format!(
            "{{\"event\":\"tool_call\",\"arguments\":{{\"{C1}\":\"v1\",\"nested\":{{\"{C1}-inner\":2}}}}}}\n"
        ),
    );
    install_all(root, &events);

    for json_view in [false, true] {
        let view = render_runs_request(
            root,
            &RunsRequest {
                id: Some("run-b543-1".to_string()),
                events: true,
                filter: None,
                json: json_view,
            },
        )
        .unwrap();
        assert_absent(&view, "b543-1 events", &[C1]);
    }

    let json = render_runs_request(
        root,
        &RunsRequest {
            id: Some("run-b543-1".to_string()),
            events: true,
            filter: None,
            json: true,
        },
    )
    .unwrap();
    let value: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["events"][0]["event"], "tool_call");
    let arguments = value["events"][0]["arguments"].as_object().unwrap();
    assert_eq!(arguments.len(), 2, "element count changed: {json}");
    assert!(arguments.contains_key("nested"), "fixed key lost: {json}");
    let nested = arguments["nested"].as_object().unwrap();
    assert_eq!(nested.len(), 1, "nested element count changed: {json}");
    reset_scopes_for_tests();
}

#[test]
fn b543_2_recovery_yaml_scrubs_whitespace_and_newline_values() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let events = root.join(".commandagent/runs/b543-2/events.jsonl");
    install_all(root, &events);

    // H-10: `display_text` split on whitespace before scrubbing, so a value
    // with a space (C9) or a newline (C4) survived. Verify the scope-less
    // thread too: it must resolve the workspace catalog by path.
    let handoff = RecoveryHandoff {
        original_goal: format!("goal {C1} {C9} {C4}"),
        failed_phase: Some(format!("phase {C9}")),
        failed_step: Some(format!("step {C4}")),
        failure_kind: "compile".to_string(),
        failure_evidence: vec![format!("evidence {C1} {C9}"), C4.to_string()],
        missing_paths: vec![format!("src/{C9}.ts")],
        missing_capabilities: vec![format!("cap {C1}")],
        changed_paths: vec![format!("out/{C9}")],
        repair_targets: vec![format!("target {C1}")],
        profile: "generic".to_string(),
        verify_commands: vec!["cargo test".to_string()],
    };

    let path = save_recovery_ultra_plan(root, "b543-2", &handoff).unwrap();
    let text = read(&path);
    assert_absent(&text, "b543-2 yaml", &[C1, C4, C9]);
    assert!(text.contains("recovery_original_goal:"), "{text}");
    assert!(text.contains("phases:"), "{text}");
    parse_ultra_plan(&text).unwrap();

    let thread_root = root.to_path_buf();
    let thread_handoff = handoff.clone();
    let scrubbed = std::thread::spawn(move || {
        set_current(None);
        let path = save_recovery_ultra_plan(&thread_root, "b543-2b", &thread_handoff).unwrap();
        read(&path)
    })
    .join()
    .unwrap();
    assert_absent(&scrubbed, "b543-2 scope-less", &[C1, C4, C9]);
    assert!(scrubbed.contains("recovery_original_goal:"), "{scrubbed}");
    reset_scopes_for_tests();
}
