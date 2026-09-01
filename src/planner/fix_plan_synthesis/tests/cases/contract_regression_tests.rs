use super::*;

#[test]
fn data_verify_phase_uses_only_completion_contract_regressions() {
    let root = tempfile::tempdir().unwrap();
    workspace(root.path(), "raise RuntimeError('broken')\n");
    let goal = "Fix the registered data/task-02.csv failure.";
    let mut config = config(root.path(), "data", "fix", goal);
    let contract_path = root.path().join("completion-contract.json");
    std::fs::write(
        &contract_path,
        serde_json::to_vec(&serde_json::json!({
            "required_paths": ["pipeline/main.py", "data/task-02.csv"],
            "verify_commands": [
                "python3 scripts/repro.py data/task-02.csv",
                "python3 -m pytest -q tests",
                "python3 scripts/contract_check.py"
            ],
            "fix_reproducer_command": "python3 scripts/repro.py data/task-02.csv",
            "profile": "data"
        }))
        .unwrap(),
    )
    .unwrap();
    config.completion_contract_path = Some(contract_path);
    let plan = crate::planner::intent::explicit_fix_plan(goal, "data", "default");
    let mut runtime = FixRuntime::for_plan(&plan, &config).unwrap();
    runtime.reproducer = Some(crate::planner::fix_runtime::ReproducerBinding {
        command: "python3 scripts/repro.py data/task-02.csv".to_string(),
        lineage: "registered".to_string(),
    });

    let verified = unwrap_generated(
        phase_plan(&config, &plan, plan.phases.last().unwrap(), Some(&runtime)).unwrap(),
    );

    assert_eq!(
        verified.steps[1].verify,
        [
            "python3 -m pytest -q tests",
            "python3 scripts/contract_check.py"
        ]
    );
    assert!(
        verified.steps[1]
            .verify
            .iter()
            .all(|command| !command.contains("anvil-catalog-check:pipeline_probe"))
    );
}
