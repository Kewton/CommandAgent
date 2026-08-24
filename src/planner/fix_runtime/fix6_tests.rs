use super::*;
use crate::cli::Cli;
use crate::planner::fix_reproducer_defect::BeforePhaseOutcome;
use crate::planner::step_plan::PlanStep;
use clap::Parser;

const RUN6_COMMAND: &str = r#"python -c "import json\nd=json.load(open('output/results.json'))\nassert 'reconciliation' in d and 'values' in d""#;

fn plan() -> UltraPlan {
    UltraPlan {
        goal: "fix results schema".to_string(),
        profile: "generic".to_string(),
        style: "default".to_string(),
        intent: "fix".to_string(),
        phases: vec![UltraPhase {
            id: "reproduce-before".to_string(),
            prompt: "Bind the reproducer".to_string(),
        }],
    }
}

fn step(command: &str) -> StepPlan {
    StepPlan {
        goal: "reproduce".to_string(),
        steps: vec![PlanStep {
            id: "reproduce-before".to_string(),
            kind: "verify".to_string(),
            expected_result: "fail".to_string(),
            instruction: "Run R".to_string(),
            expected_paths: Vec::new(),
            verify: vec![command.to_string()],
        }],
    }
}

fn config(root: &Path) -> Config {
    let cwd = root.to_string_lossy().to_string();
    let mut config = Config::from_cli(Cli::parse_from([
        "commandagent",
        "--cwd",
        &cwd,
        "--offline",
        "--profile",
        "generic",
        "--intent",
        "fix",
        "--ultra-plan",
        "fix results schema",
    ]))
    .unwrap();
    config.eval_events_path = Some(root.join("events.jsonl"));
    config
}

#[test]
fn defect_before_f1_returns_feedback_and_allows_rebuilt_lineage() {
    let root = tempfile::tempdir().unwrap();
    let config = config(root.path());
    let plan = plan();
    let phase = &plan.phases[0];
    let mut runtime = FixRuntime::for_plan(&plan, &config).unwrap();

    let first = runtime
        .run_before_phase(&step(RUN6_COMMAND), &config, &plan, phase, 0)
        .unwrap();
    let BeforePhaseOutcome::RebuildRequired { feedback } = first else {
        panic!("defective R must request rebuild");
    };
    assert!(feedback.contains("再現コマンド自体が壊れている（SyntaxError）"));
    assert!(runtime.reproducer.is_none());
    assert!(runtime.before.is_none());

    let rebuilt = "test -f fixed.marker";
    assert_eq!(
        runtime
            .run_before_phase(&step(rebuilt), &config, &plan, phase, 0)
            .unwrap(),
        BeforePhaseOutcome::Confirmed
    );
    assert_eq!(runtime.epoch, 2);
    assert_eq!(runtime.reproducer.as_ref().unwrap().command, rebuilt);
    assert_eq!(
        runtime.before.as_ref().unwrap().lineage,
        reproducer_lineage(rebuilt)
    );

    let defect_path = before_attempt_evidence_path(&runtime.run_id, 1);
    let defect = std::fs::read_to_string(root.path().join(defect_path)).unwrap();
    assert!(defect.contains(r#""failure_classification": "reproducer_defect""#));
    let confirmed =
        std::fs::read_to_string(root.path().join(before_evidence_path(&runtime.run_id))).unwrap();
    assert!(!confirmed.contains("failure_classification"));

    let events = std::fs::read_to_string(config.eval_events_path.as_ref().unwrap()).unwrap();
    assert_eq!(events.matches("fix_evidence_recorded").count(), 2);
    assert_eq!(events.matches("reproducer_defect").count(), 1);
}

#[test]
fn confirmed_f1_rejects_reproducer_rebinding() {
    let root = tempfile::tempdir().unwrap();
    let config = config(root.path());
    let plan = plan();
    let phase = &plan.phases[0];
    let mut runtime = FixRuntime::for_plan(&plan, &config).unwrap();
    let original = "test -f fixed.marker";

    assert_eq!(
        runtime
            .run_before_phase(&step(original), &config, &plan, phase, 0)
            .unwrap(),
        BeforePhaseOutcome::Confirmed
    );
    let lineage = runtime.before.as_ref().unwrap().lineage.clone();
    let error = runtime
        .run_before_phase(&step("test -f switched.marker"), &config, &plan, phase, 0)
        .unwrap_err();

    assert!(error.to_string().contains("F1 lineage cannot change"));
    assert_eq!(runtime.before.as_ref().unwrap().lineage, lineage);
    assert_eq!(runtime.reproducer.as_ref().unwrap().command, original);
}
