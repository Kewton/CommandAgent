use sha2::{Digest, Sha256};

#[test]
fn recovery_contract_final_verification_fails_without_model_command_rewrite() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let contract_path = dir.path().join("completion-contract.json");
    std::fs::write(dir.path().join("ready.txt"), "ready\n").unwrap();
    std::fs::write(
        &contract_path,
        r#"{"required_paths":["ready.txt"],"verify_commands":["false"],"profile":"generic"}"#,
    )
    .unwrap();
    let runtime = dir.path().join(".commandagent/recovery-runtime");
    std::fs::create_dir_all(&runtime).unwrap();
    let evidence = b"{}\n";
    std::fs::write(runtime.join("fix-origin-evidence.json"), evidence).unwrap();
    std::fs::write(
        runtime.join("fix-origin.json"),
        serde_json::to_vec(
            &crate::planner::recovery_contract_binding::RecoveryFixOrigin {
                schema_version: "1".to_string(),
                original_intent: "fix".to_string(),
                contract_origin: crate::planner::fix_runtime::FIX_CONTRACT_ORIGIN.to_string(),
                contract_version: crate::planner::adjudication::contract::FIX_CONTRACT_VERSION
                    .to_string(),
                contract_ref: crate::planner::adjudication::contract::FIX_CONTRACT_REF.to_string(),
                fix_run_id: "host-verify-test".to_string(),
                evidence_path: ".commandagent/recovery-runtime/fix-origin-evidence.json"
                    .to_string(),
                evidence_sha256: format!("{:x}", Sha256::digest(evidence)),
                reproducer_command: "false".to_string(),
            },
        )
        .unwrap(),
    )
    .unwrap();
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events.clone());
    cfg.completion_contract_path = Some(contract_path);
    let step = PlanStep {
        id: "recovery-contract-verify".to_string(),
        kind: "verify".to_string(),
        expected_result: "pass".to_string(),
        instruction: "Run registered final-success checks".to_string(),
        expected_paths: vec!["ready.txt".to_string()],
        verify: vec!["false".to_string()],
    };
    let plan = StepPlan {
        goal: "recover ready artifact".to_string(),
        steps: vec![step.clone()],
    };
    let context = StepPromptContext {
        overall_goal: plan.goal.clone(),
        completion_contract_path: cfg.completion_contract_path.clone(),
        ..StepPromptContext::default()
    };
    let mut fake = FakeClient::new(Vec::new());
    let mut session = SessionSnapshot::new();

    let error = run_step(
        &mut fake,
        &mut session,
        &plan,
        &step,
        &context,
        &cfg,
        &NOOP_UI,
        "test",
        ContractEnforcement::Enforce,
        Some("repair-unknown"),
        None,
    )
    .unwrap_err();

    assert_eq!(
        error.outcome.stop_reason.as_deref(),
        Some("recovery_host_final_success_verification_failed")
    );
    assert!(fake.messages().is_empty());
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(
        event_text.contains("recovery_host_final_success_verification_failed"),
        "{event_text}"
    );
    assert!(event_text.contains("\"model_execution_skipped\":true"));
}

#[test]
fn data_profile_preserves_all_bound_host_recovery_commands() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    for path in ["scripts", "data", "tests"] {
        std::fs::create_dir_all(dir.path().join(path)).unwrap();
    }
    std::fs::write(
        dir.path().join("scripts/repro.py"),
        "raise SystemExit(1)\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("scripts/contract_check.py"),
        "raise SystemExit(0)\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("data/task.csv"), "value\n1\n").unwrap();
    std::fs::write(
        dir.path().join("tests/test_ok.py"),
        "def test_ok():\n    pass\n",
    )
    .unwrap();
    let runtime = dir.path().join(".commandagent/recovery-runtime");
    std::fs::create_dir_all(&runtime).unwrap();
    let evidence = b"{}\n";
    std::fs::write(runtime.join("fix-origin-evidence.json"), evidence).unwrap();
    std::fs::write(
        runtime.join("fix-origin.json"),
        serde_json::to_vec(
            &crate::planner::recovery_contract_binding::RecoveryFixOrigin {
                schema_version: "1".to_string(),
                original_intent: "fix".to_string(),
                contract_origin: crate::planner::fix_runtime::FIX_CONTRACT_ORIGIN.to_string(),
                contract_version: crate::planner::adjudication::contract::FIX_CONTRACT_VERSION
                    .to_string(),
                contract_ref: crate::planner::adjudication::contract::FIX_CONTRACT_REF.to_string(),
                fix_run_id: "data-host-verify-test".to_string(),
                evidence_path: ".commandagent/recovery-runtime/fix-origin-evidence.json"
                    .to_string(),
                evidence_sha256: format!("{:x}", Sha256::digest(evidence)),
                reproducer_command: "python3 scripts/repro.py data/task.csv".to_string(),
            },
        )
        .unwrap(),
    )
    .unwrap();
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "data".to_string();
    cfg.eval_events_path = Some(events.clone());
    let commands = vec![
        "python3 scripts/repro.py data/task.csv".to_string(),
        "python3 -m pytest -q tests".to_string(),
        "python3 scripts/contract_check.py".to_string(),
    ];
    let step = PlanStep {
        id: "recovery-contract-verify".to_string(),
        kind: "verify".to_string(),
        expected_result: "pass".to_string(),
        instruction: "Run every bound registered command".to_string(),
        expected_paths: vec!["scripts/repro.py".to_string()],
        verify: commands.clone(),
    };
    let plan = StepPlan {
        goal: "recover data pipeline".to_string(),
        steps: vec![step.clone()],
    };
    let context = StepPromptContext {
        overall_goal: plan.goal.clone(),
        ..StepPromptContext::default()
    };
    let mut fake = FakeClient::new(Vec::new());
    let mut session = SessionSnapshot::new();

    let error = run_step(
        &mut fake,
        &mut session,
        &plan,
        &step,
        &context,
        &cfg,
        &NOOP_UI,
        "test",
        ContractEnforcement::Enforce,
        Some("verify-recovery"),
        None,
    )
    .unwrap_err();

    assert_eq!(
        error.outcome.stop_reason.as_deref(),
        Some("recovery_host_final_success_verification_failed")
    );
    assert!(fake.messages().is_empty());
    let event_text = std::fs::read_to_string(events).unwrap();
    for command in commands {
        assert!(event_text.contains(&command), "{event_text}");
    }
}

#[test]
fn recovery_fix_origin_requires_write_for_implement_step_only() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("pipeline")).unwrap();
    std::fs::write(dir.path().join("pipeline/main.py"), "BROKEN = True\n").unwrap();
    let runtime = dir.path().join(".commandagent/recovery-runtime");
    std::fs::create_dir_all(&runtime).unwrap();
    let evidence = b"{}\n";
    std::fs::write(runtime.join("fix-origin-evidence.json"), evidence).unwrap();
    std::fs::write(
        runtime.join("fix-origin.json"),
        serde_json::to_vec(
            &crate::planner::recovery_contract_binding::RecoveryFixOrigin {
                schema_version: "1".to_string(),
                original_intent: "fix".to_string(),
                contract_origin: crate::planner::fix_runtime::FIX_CONTRACT_ORIGIN.to_string(),
                contract_version: crate::planner::adjudication::contract::FIX_CONTRACT_VERSION
                    .to_string(),
                contract_ref: crate::planner::adjudication::contract::FIX_CONTRACT_REF.to_string(),
                fix_run_id: "recovery-write-test".to_string(),
                evidence_path: ".commandagent/recovery-runtime/fix-origin-evidence.json"
                    .to_string(),
                evidence_sha256: format!("{:x}", Sha256::digest(evidence)),
                reproducer_command: "python3 pipeline/main.py".to_string(),
            },
        )
        .unwrap(),
    )
    .unwrap();
    let cfg = config(dir.path().to_path_buf());
    let step = PlanStep {
        id: "repair-pipeline".to_string(),
        kind: "implement".to_string(),
        expected_result: "pass".to_string(),
        instruction: "Update pipeline/main.py with the repaired implementation.".to_string(),
        expected_paths: vec!["pipeline/main.py".to_string()],
        verify: Vec::new(),
    };
    assert!(recovery_fix_implement_requires_write(&cfg, &step).unwrap());

    let mut verify_step = step.clone();
    verify_step.kind = "verify".to_string();
    assert!(!recovery_fix_implement_requires_write(&cfg, &verify_step).unwrap());

    std::fs::remove_file(runtime.join("fix-origin.json")).unwrap();
    assert!(!recovery_fix_implement_requires_write(&cfg, &step).unwrap());
}
