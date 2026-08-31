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
    assert!(super::phase::recovery_fix::requires_write(&cfg, &step).unwrap());

    let mut verify_step = step.clone();
    verify_step.kind = "verify".to_string();
    assert!(!super::phase::recovery_fix::requires_write(&cfg, &verify_step).unwrap());

    std::fs::remove_file(runtime.join("fix-origin.json")).unwrap();
    assert!(!super::phase::recovery_fix::requires_write(&cfg, &step).unwrap());
}

#[test]
fn recovery_fix_repairs_removed_caller_api_once_inside_same_recovery() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/corpus/apps/issue399-phase6-ab-uat/fixtures/recovery-api-preservation");
    std::fs::create_dir_all(dir.path().join("pipeline")).unwrap();
    std::fs::create_dir_all(dir.path().join("scripts")).unwrap();
    std::fs::copy(
        fixture.join("pipeline/main.py"),
        dir.path().join("pipeline/main.py"),
    )
    .unwrap();
    std::fs::copy(
        fixture.join("scripts/repro.py"),
        dir.path().join("scripts/repro.py"),
    )
    .unwrap();
    let contract_path = dir.path().join("completion-contract.json");
    std::fs::write(
        &contract_path,
        r#"{"goal":"Repair the existing pipeline without breaking callers","required_paths":["pipeline/main.py","scripts/repro.py"],"verify_commands":["python3 scripts/repro.py"],"fix_reproducer_command":"python3 scripts/repro.py","profile":"generic","verify_repair_cap":1}"#,
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
                fix_run_id: "recovery-api-preservation".to_string(),
                evidence_path: ".commandagent/recovery-runtime/fix-origin-evidence.json"
                    .to_string(),
                evidence_sha256: format!("{:x}", Sha256::digest(evidence)),
                reproducer_command: "python3 scripts/repro.py".to_string(),
            },
        )
        .unwrap(),
    )
    .unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events.clone());
    cfg.completion_contract_path = Some(contract_path);
    cfg.max_iterations = 6;
    let step = PlanStep {
        id: "repair-pipeline".to_string(),
        kind: "implement".to_string(),
        expected_result: "pass".to_string(),
        instruction: "Make the smallest bounded pipeline repair.".to_string(),
        expected_paths: vec!["pipeline/main.py".to_string()],
        verify: Vec::new(),
    };
    let plan = StepPlan {
        goal: "Repair the existing pipeline without breaking callers".to_string(),
        steps: vec![step.clone()],
    };
    let context = StepPromptContext {
        overall_goal: plan.goal.clone(),
        completion_contract_path: cfg.completion_contract_path.clone(),
        ..StepPromptContext::default()
    };
    let broken = std::fs::read_to_string(fixture.join("pipeline/broken-main.py")).unwrap();
    let repaired = std::fs::read_to_string(fixture.join("pipeline/main.py")).unwrap();
    let mut fake = FakeClient::new(vec![
        AssistantReply {
            content: String::new(),
            tool_calls: vec![crate::state::ToolCall::new(
                "Write",
                serde_json::json!({"path":"pipeline/main.py","content":broken}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        },
        AssistantReply {
            content: String::new(),
            tool_calls: vec![crate::state::ToolCall::new(
                "Write",
                serde_json::json!({"path":"pipeline/main.py","content":repaired}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        },
    ]);
    let mut session = SessionSnapshot::new();

    let outcome = run_step(
        &mut fake,
        &mut session,
        &plan,
        &step,
        &context,
        &cfg,
        &NOOP_UI,
        "test",
        ContractEnforcement::Observe,
        Some("repair-repair"),
        None,
    )
    .unwrap();

    assert_eq!(outcome.repair_attempts, 1);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("pipeline/main.py")).unwrap(),
        std::fs::read_to_string(fixture.join("pipeline/main.py")).unwrap()
    );
    assert_eq!(fake.messages().len(), 2);
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(
        event_text.contains("\"referenced_api_violations\":[{\"caller_paths\":[\"scripts/repro.py\"],\"owner_path\":\"pipeline/main.py\",\"symbol\":\"write_outputs\"}]"),
        "{event_text}"
    );
    assert!(event_text.contains("\"event\":\"step_verify_repair\""));
    assert!(event_text.contains("\"attempt\":1"));
}
