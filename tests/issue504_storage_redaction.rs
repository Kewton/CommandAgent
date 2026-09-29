//! Issue #543 focused test: the shared run-scoped exact-value catalog is
//! connected to the custom save boundaries (build log, browser readiness JSON,
//! interaction JSON/mirror, StepPlan/UltraPlan YAML, repair/recovery saves,
//! workflow node/orchestrator events) and the runs display. A runnable YAML
//! whose command/path contains a registered secret is refused honestly, while
//! a secret-free plan reloads and verifies with the same result. The stored run
//! record bytes are never rewritten by the display path.

use std::process::Command;

use clap::Parser;
use commandagent::cli::Cli;
use commandagent::config::{Config, ProviderCliOptions, RunsRequest};
use commandagent::minimal_loop::browser_probe::{
    BrowserReadinessObservation, browser_readiness_evidence_path, write_browser_readiness_evidence,
};
use commandagent::minimal_loop::build_verifier::write_build_verifier_output;
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
    REDACTED, SecretCatalog, install_scope, reset_scopes_for_tests,
};
use commandagent::state::ConversationMessage;
use commandagent::tools::registry::ToolSpec;
use commandagent::workflow::orchestrator::emit;
use commandagent::workflow::runner::write_node_event;
use serde_json::{Value, json};

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

const CANARY: &str = "H01_CANARY_JwtStyle_NonPrefix_29486";
const ALPHA: &str = "H543_ALPHA_ONLY_4821";
const BETA: &str = "H543_BETA_ONLY_7394";
const DOCTOR_SECRET_ENV: &str = "ISSUE543_DOCTOR_SECRET";

fn install(root: &std::path::Path, events: &std::path::Path, secret: &str) {
    let mut catalog = SecretCatalog::new();
    catalog.register(secret);
    install_scope(catalog, Some(root), Some(events));
}

// The run scope registry is process-global, so tests that install or reset it
// take this lock. Without it a concurrent `reset_scopes_for_tests` would clear
// another test's registered scope mid-assertion.
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

fn assert_no_canary(text: &str, label: &str) {
    assert!(!text.contains(CANARY), "{label} leaked the canary:\n{text}");
    assert!(
        !text.contains("H01_CANARY"),
        "{label} leaked a fragment:\n{text}"
    );
}

#[test]
fn build_verifier_log_and_file_name_never_keep_a_secret() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let events = root.join(".commandagent/runs/run-1/events.jsonl");
    install(root, &events, CANARY);

    let command = format!("npm run build --prefix {CANARY}");
    let output = format!("compiling {CANARY}\nwarning: nothing\n");
    let path = write_build_verifier_output(root, &command, &output).unwrap();

    let text = read(&path);
    assert_no_canary(&text, "build log");
    assert!(text.contains(REDACTED), "{text}");
    let file_name = path.file_name().unwrap().to_string_lossy().to_string();
    assert!(
        !file_name.contains("canary"),
        "the log file name kept the secret: {file_name}"
    );
    reset_scopes_for_tests();
}

#[test]
fn browser_readiness_json_scrubs_free_text_but_keeps_schema() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let events = root.join(".commandagent/runs/run-1/events.jsonl");
    install(root, &events, CANARY);

    let observation = BrowserReadinessObservation {
        ok: true,
        status: "ready".to_string(),
        profile: "generic".to_string(),
        port: 3000,
        route: "/".to_string(),
        command: format!("next start --token {CANARY}"),
        http_status: Some(200),
        failure_kind: String::new(),
        evidence_path: browser_readiness_evidence_path(root),
        elapsed_ms: 12,
        output_excerpt: format!("listening with {CANARY}"),
        build_output_path: String::new(),
        compile_errors: vec![],
        child_spawned: true,
        child_reaped: true,
        has_canvas: false,
        interactive_control_count: 0,
        title_text_excerpt: format!("title {CANARY}"),
    };
    write_browser_readiness_evidence(root, &observation);

    let text = read(&browser_readiness_evidence_path(root));
    assert_no_canary(&text, "browser readiness");
    assert!(text.contains(REDACTED), "{text}");
    let value: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["status"], "ready");
    assert_eq!(value["ok"], true);
    reset_scopes_for_tests();
}

#[test]
fn interaction_json_and_mirror_are_scrubbed() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let events = root.join(".commandagent/runs/run-1/events.jsonl");
    install(root, &events, CANARY);

    let evidence_path = root.join(".commandagent/evidence/custom-interaction.json");
    let value = json!({
        "status": "available",
        "output_excerpt": format!("probe {CANARY}"),
        "probe": {"message": format!("nested {CANARY}")},
    });
    write_interaction_value(root, &evidence_path, &value);

    let mirror = browser_interaction_evidence_path(root);
    assert_ne!(evidence_path, mirror);
    for path in [&evidence_path, &mirror] {
        let text = read(path);
        assert_no_canary(&text, &path.display().to_string());
        let parsed: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed["status"], "available");
        assert!(text.contains(REDACTED), "{text}");
    }
    reset_scopes_for_tests();
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

#[test]
fn step_plan_free_text_is_scrubbed_and_stays_loadable() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let events = root.join(".commandagent/runs/run-1/events.jsonl");
    install(root, &events, CANARY);

    let path = save_step_plan(root, &plan_with_secret(CANARY)).unwrap();
    let text = read(&path);
    assert_no_canary(&text, "step plan");
    assert!(text.contains(REDACTED), "{text}");
    let reloaded = parse_step_plan(&text).unwrap();
    assert_eq!(reloaded.steps[0].kind, "implement");
    assert_eq!(reloaded.steps[0].expected_result, "pass");
    reset_scopes_for_tests();
}

#[test]
fn runnable_plan_command_secret_is_refused_without_leaking() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let events = root.join(".commandagent/runs/run-1/events.jsonl");
    let cwd = root.to_string_lossy().to_string();
    let config = Config::from_cli(Cli::parse_from([
        "commandagent",
        "--cwd",
        &cwd,
        "--model",
        "m",
    ]))
    .unwrap();
    install(root, &events, CANARY);

    let mut plan = plan_with_secret("clean goal");
    plan.steps[0].verify = vec![format!("echo {CANARY}")];
    let error = save_step_plan(root, &plan).unwrap_err();
    assert!(!error.to_string().contains(CANARY), "{error}");
    let saved = std::fs::read_dir(root.join(".commandagent/plans"))
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(saved, 0, "a refused plan must not be saved");

    // The execution boundary refuses a hand-authored runnable plan with the
    // same honest reason and never runs it.
    let plan_path = root.join("secret-plan.yaml");
    std::fs::write(
        &plan_path,
        format!(
            "goal: clean\nsteps:\n  - id: s1\n    kind: verify\n    expected_result: pass\n    instruction: run\n    verify:\n      - echo {CANARY}\n"
        ),
    )
    .unwrap();
    let mut client = NoopClient;
    let error = run_plan_file(&mut client, &plan_path, &config).unwrap_err();
    assert!(!error.to_string().contains(CANARY), "{error}");
    reset_scopes_for_tests();
}

#[test]
fn ultra_plan_is_scrubbed_and_reloads_identically() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let events = root.join(".commandagent/runs/run-1/events.jsonl");
    install(root, &events, CANARY);

    let plan = UltraPlan {
        goal: format!("goal {CANARY}"),
        profile: "generic".to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: vec![UltraPhase {
            id: "phase-1".to_string(),
            prompt: format!("prompt {CANARY}"),
        }],
    };
    let clean_render = parse_ultra_plan(&{
        let path = save_ultra_plan(root, &plan).unwrap();
        let text = read(&path);
        assert_no_canary(&text, "ultra plan");
        assert!(text.contains(REDACTED), "{text}");
        text
    })
    .unwrap();
    assert_eq!(clean_render.profile, "generic");
    assert_eq!(clean_render.intent, "create");
    assert_eq!(clean_render.phases.len(), 1);

    // A secret-free plan reloads and validates identically.
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
    let reloaded = parse_ultra_plan(&read(&path)).unwrap();
    assert_eq!(reloaded, clean);
    reset_scopes_for_tests();
}

#[test]
fn repair_markdown_and_recovery_yaml_are_scrubbed() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let events = root.join(".commandagent/runs/run-1/events.jsonl");
    install(root, &events, CANARY);

    let report = VerificationReport::pass();
    let context = RepairContext {
        overall_goal: Some(format!("goal {CANARY}")),
        ..RepairContext::default()
    };
    let report_path = save_repair_report_with_context(root, "step-1", &report, &context).unwrap();
    assert_no_canary(&read(&report_path), "repair report");
    assert!(read(&report_path).contains(REDACTED));

    let handoff = RecoveryHandoff {
        original_goal: format!("recover {CANARY}"),
        ..RecoveryHandoff::default()
    };
    let prompt_path = save_ultra_recovery_prompt(root, "step-1", &handoff).unwrap();
    assert_no_canary(&read(&prompt_path), "recovery prompt");
    assert!(read(&prompt_path).contains(REDACTED));

    let plan_path = save_recovery_ultra_plan(root, "step-1", &handoff).unwrap();
    let plan_text = read(&plan_path);
    assert_no_canary(&plan_text, "recovery yaml");
    assert!(plan_text.contains(REDACTED), "{plan_text}");
    parse_ultra_plan(&plan_text).unwrap();
    reset_scopes_for_tests();
}

#[test]
fn workflow_node_and_orchestrator_events_keep_order_and_verdict() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let origin = dir.path().join("origin");
    std::fs::create_dir_all(origin.join("evidence")).unwrap();
    let events = origin.join("evidence/workflow-events.jsonl");
    install(&origin, &events, CANARY);

    write_node_event(
        &origin,
        &origin.join("evidence/node-events.jsonl"),
        &json!({"event": "intent_resolved", "model": CANARY, "provider": "lm-studio"}),
    )
    .unwrap();

    emit(&events, json!({"event": "workflow_started", "epoch": 1})).unwrap();
    emit(
        &events,
        json!({"event": "workflow_adjudicated", "verdict": "circle_full", "reason": CANARY}),
    )
    .unwrap();
    emit(
        &events,
        json!({"event": "workflow_node_completed", "node": CANARY}),
    )
    .unwrap();

    let node_text = read(&origin.join("evidence/node-events.jsonl"));
    assert_no_canary(&node_text, "node event");
    let node: Value = serde_json::from_str(node_text.trim()).unwrap();
    assert_eq!(node["event"], "intent_resolved");
    assert_eq!(node["provider"], "lm-studio");

    let text = read(&events);
    assert_no_canary(&text, "workflow events");
    let names = text
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .map(|value| value["event"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec![
            "workflow_started",
            "workflow_adjudicated",
            "workflow_node_completed"
        ]
    );
    let adjudicated = text
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .find(|value| value["event"] == "workflow_adjudicated")
        .unwrap();
    assert_eq!(adjudicated["verdict"], "circle_full");
    reset_scopes_for_tests();
}

fn fake_run(root: &std::path::Path, id: &str) -> std::path::PathBuf {
    let run_dir = root.join(".commandagent/runs").join(id);
    std::fs::create_dir_all(&run_dir).unwrap();
    let events = run_dir.join("events.jsonl");
    std::fs::write(
        &events,
        format!(
            "{{\"event\":\"run_start\",\"model\":\"{BETA}\"}}\n\
             {{\"event\":\"run_stop\",\"ok\":true,\"status\":\"completed\",\"reason\":\"{ALPHA}\"}}\n"
        ),
    )
    .unwrap();
    std::fs::write(
        run_dir.join("summary.md"),
        format!("Status: completed\nNotes: {ALPHA} and {BETA}\n"),
    )
    .unwrap();
    events
}

#[test]
fn runs_display_scrubs_known_values_but_leaves_the_stored_bytes() {
    let _scope = begin_scope_test();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let events = fake_run(root, "run-1");
    // This workspace's catalog knows ALPHA only.
    install(root, &events, ALPHA);

    for json_view in [false, true] {
        let table = render_runs_request(
            root,
            &RunsRequest {
                id: None,
                events: false,
                filter: None,
                json: json_view,
            },
        )
        .unwrap();
        assert!(
            !table.contains(ALPHA),
            "runs list leaked ALPHA (json={json_view}): {table}"
        );

        let detail = render_runs_request(
            root,
            &RunsRequest {
                id: Some("run-1".to_string()),
                events: false,
                filter: None,
                json: json_view,
            },
        )
        .unwrap();
        assert!(
            !detail.contains(ALPHA),
            "runs detail leaked ALPHA (json={json_view}): {detail}"
        );
        // A value this workspace never registered is not claimed as a secret.
        assert!(
            detail.contains(BETA),
            "runs detail over-redacted (json={json_view}): {detail}"
        );

        let events_view = render_runs_request(
            root,
            &RunsRequest {
                id: Some("run-1".to_string()),
                events: true,
                filter: None,
                json: json_view,
            },
        )
        .unwrap();
        assert!(
            !events_view.contains(ALPHA),
            "runs events leaked ALPHA (json={json_view}): {events_view}"
        );
        assert!(
            events_view.contains(BETA),
            "runs events over-redacted (json={json_view})"
        );
    }
    assert!(!render_runs_table(root).contains(ALPHA));

    // The stored record bytes are unchanged.
    let raw = read(&events);
    assert!(raw.contains(ALPHA), "the stored run record was rewritten");
    assert!(raw.contains(BETA));
    reset_scopes_for_tests();
}

#[test]
fn two_workspace_display_scopes_do_not_cross_contaminate() {
    let _scope = begin_scope_test();
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let a_events = fake_run(a.path(), "run-a");
    let b_events = fake_run(b.path(), "run-b");
    install(a.path(), &a_events, ALPHA);
    install(b.path(), &b_events, BETA);

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
    assert!(!a_view.contains(ALPHA), "A leaked its own secret: {a_view}");
    assert!(a_view.contains(BETA), "A imported B's catalog: {a_view}");

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
    assert!(!b_view.contains(BETA), "B leaked its own secret: {b_view}");
    assert!(b_view.contains(ALPHA), "B imported A's catalog: {b_view}");
    reset_scopes_for_tests();
}

#[test]
fn doctor_diagnostics_hide_a_config_env_expansion_value() {
    let exe = std::env::current_exe().unwrap();
    let status = Command::new(exe)
        .args([
            "--exact",
            "--ignored",
            "--nocapture",
            "doctor_diagnostics_hide_a_config_env_expansion_value_child",
        ])
        .env(DOCTOR_SECRET_ENV, CANARY)
        .status()
        .unwrap();
    assert!(status.success(), "doctor child exited with {status}");
}

#[test]
#[ignore]
fn doctor_diagnostics_hide_a_config_env_expansion_value_child() {
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
    assert!(!human.contains(CANARY), "human diagnostics leaked: {human}");
    assert!(!json.contains(CANARY), "json diagnostics leaked: {json}");
    // Non-value diagnostics survive.
    assert!(
        human.contains("base-url") || json.contains("base-url"),
        "the base-url diagnosis was lost: {human}\n{json}"
    );
}
