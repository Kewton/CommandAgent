use super::*;

const RUN1_FIXTURE_ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/corpus/apps/test0715_data12_pre_satisfied/fixtures/data6_qwen35_profile_001"
);

fn copy_run1_fixture(root: &Path) {
    for relative in [
        "data/sales.csv",
        "pipeline/main.py",
        "output/inspection.json",
        "output/results.json",
        "output/report.md",
    ] {
        let target = root.join(relative);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::copy(Path::new(RUN1_FIXTURE_ROOT).join(relative), target).unwrap();
    }
}

fn measured_cleaning_step() -> PlanStep {
    let plan = parse_step_plan(
        &std::fs::read_to_string(Path::new(RUN1_FIXTURE_ROOT).join("cleaning-step-plan.yaml"))
            .unwrap(),
    )
    .unwrap();
    plan.steps
        .into_iter()
        .find(|step| step.id == "implement-pipeline-main")
        .unwrap()
}

fn run_without_final_acceptance(
    client: &mut dyn ChatClient,
    plan: &StepPlan,
    cfg: &Config,
    phase_scope: Option<&str>,
) -> Result<StepPlanRunOutcome, Box<StepPlanRunError>> {
    let mut session = SessionSnapshot::new();
    run_step_plan_with_session_with_ui(
        client,
        &mut session,
        plan,
        cfg,
        &NOOP_UI,
        false,
        "data12-regression",
        phase_scope,
        None,
    )
    .map_err(Box::new)
}

#[test]
fn measured_cleaning_step_short_circuits_and_advances_after_actual_verify_pass() {
    let dir = tempfile::tempdir().unwrap();
    copy_run1_fixture(dir.path());
    let events = dir.path().join("events.jsonl");
    let plan = StepPlan {
        goal: "Continue the measured data-cleaning phase".to_string(),
        steps: vec![
            measured_cleaning_step(),
            PlanStep {
                id: "downstream-observation".to_string(),
                kind: "report".to_string(),
                expected_result: "pass".to_string(),
                instruction: "Record that execution advanced past cleaning".to_string(),
                expected_paths: Vec::new(),
                verify: Vec::new(),
            },
        ],
    };
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "data".to_string();
    cfg.eval_events_path = Some(events.clone());
    let mut client = FakeClient::new(Vec::new());

    let outcome =
        run_without_final_acceptance(&mut client, &plan, &cfg, Some("data-cleaning")).unwrap();

    assert_eq!(outcome.completed_steps, 2);
    assert!(client.messages().is_empty());
    let event = std::fs::read_to_string(events)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .find(|event| {
            event["event"] == "step_short_circuited"
                && event["step_id"] == "implement-pipeline-main"
        })
        .unwrap();
    assert_eq!(event["reason"], "pre_satisfied_verified");
    assert_eq!(event["phase_scope"], "data-cleaning");
    assert_eq!(event["verification_summary"]["status"], "pass");
    assert_eq!(event["verification_summary"]["expected_paths_checked"], 3);
    assert_eq!(event["verification_summary"]["verify_commands_executed"], 1);
    assert_eq!(event["verification_summary"]["failure_count"], 0);
}

#[test]
fn failed_verify_does_not_short_circuit_before_the_model_turn() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("output")).unwrap();
    std::fs::write(dir.path().join("output/results.json"), "{}\n").unwrap();
    let events = dir.path().join("events.jsonl");
    let plan = StepPlan {
        goal: "Repair a failed declared verify".to_string(),
        steps: vec![PlanStep {
            id: "verify-existing-results".to_string(),
            kind: "verify".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Verify the existing results artifact".to_string(),
            expected_paths: vec!["output/results.json".to_string()],
            verify: vec!["test -f output/not-present.json".to_string()],
        }],
    };
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "data".to_string();
    cfg.eval_events_path = Some(events.clone());
    let mut client = FakeClient::new(Vec::new());

    let error =
        run_without_final_acceptance(&mut client, &plan, &cfg, Some("data-cleaning")).unwrap_err();

    assert!(error.message.contains("fake client exhausted"), "{error:?}");
    assert_eq!(client.messages().len(), 1);
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(!event_text.contains("\"event\":\"step_short_circuited\""));
}

#[test]
fn nextjs_short_circuit_event_keeps_the_legacy_byte_shape() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("package.json"), "{}\n").unwrap();
    let events = dir.path().join("events.jsonl");
    let plan = StepPlan {
        goal: "Verify an existing Next.js manifest".to_string(),
        steps: vec![PlanStep {
            id: "verify-existing-manifest".to_string(),
            kind: "verify".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Verify package.json exists".to_string(),
            expected_paths: vec!["package.json".to_string()],
            verify: vec!["test -f package.json".to_string()],
        }],
    };
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = crate::planner::profile_manifest::nextjs_manifest()
        .metadata
        .id
        .clone();
    cfg.eval_events_path = Some(events.clone());
    let mut client = FakeClient::new(Vec::new());

    let outcome =
        run_without_final_acceptance(&mut client, &plan, &cfg, Some("core-implementation"))
            .unwrap();

    assert_eq!(outcome.completed_steps, 1);
    assert!(client.messages().is_empty());
    let event = std::fs::read_to_string(events)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .find(|event| event["event"] == "step_short_circuited")
        .unwrap();
    assert_eq!(
        event,
        serde_json::json!({
            "at": "start",
            "event": "step_short_circuited",
            "phase_scope": "core-implementation",
            "required_paths": ["package.json"],
            "schema_version": "1",
            "session_scope": "plan-run-step",
            "step_id": "verify-existing-manifest",
            "step_kind": "verify",
            "verify_commands": ["test -f package.json"],
        })
    );
}
