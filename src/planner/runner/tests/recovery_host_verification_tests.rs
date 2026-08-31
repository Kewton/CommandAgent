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
