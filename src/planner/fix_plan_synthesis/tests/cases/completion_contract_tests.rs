use clap::Parser;

use super::super::super::{PhasePlan, phase_plan};
use crate::cli::Cli;
use crate::config::Config;

#[test]
fn completion_contract_binds_one_cli_reproducer_without_model_generation() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("cli.py"), "raise SystemExit(2)\n").unwrap();
    let goal = "Fix the known cli.py failure.";
    let cwd = root.path().to_string_lossy();
    let mut config = Config::from_cli(Cli::parse_from([
        "commandagent",
        "--cwd",
        cwd.as_ref(),
        "--offline",
        "--profile",
        "cli",
        "--intent",
        "fix",
        "--plan-preset",
        "profile",
        "--ultra-plan",
        goal,
    ]))
    .unwrap();
    config.eval_events_path = Some(root.path().join("events.jsonl"));
    let contract_path = root.path().join("completion-contract.json");
    std::fs::write(
        &contract_path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "required_paths": ["cli.py"],
            "verify_commands": ["python3 cli.py 7"],
            "fix_reproducer_command": "python3 cli.py 7",
            "profile": "cli"
        }))
        .unwrap(),
    )
    .unwrap();
    config.completion_contract_path = Some(contract_path);
    let plan = crate::planner::intent::explicit_fix_plan(goal, "cli", "default");

    let before = match phase_plan(&config, &plan, &plan.phases[0], None).unwrap() {
        PhasePlan::Generated(plan) => plan,
        _ => panic!("typed completion reproducer must generate the F1 plan"),
    };

    assert_eq!(before.steps.len(), 1);
    assert_eq!(before.steps[0].kind, "verify");
    assert_eq!(before.steps[0].expected_result, "fail");
    assert!(before.steps[0].expected_paths.is_empty());
    assert_eq!(before.steps[0].verify, ["python3 cli.py 7"]);
    let events = std::fs::read_to_string(config.eval_events_path.unwrap()).unwrap();
    assert!(
        events.contains("\"r_basis\":\"completion_contract:fix_reproducer_command\""),
        "{events}"
    );
}
