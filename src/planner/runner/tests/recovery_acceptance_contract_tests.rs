#[test]
fn generated_contract_binds_profile_only_at_run_scope() {
    let root = tempfile::tempdir().unwrap();
    let config = config(root.path().to_path_buf());
    let run = bind_completion_contract_for_acceptance(
        &config,
        "ultra-plan-run",
        "generic",
        "Create a note app",
        &[],
        &["stateful_interaction".to_string()],
        &[],
        &[],
    )
    .unwrap()
    .expect("run contract should bind");
    let step = bind_completion_contract_for_acceptance(
        &config,
        "plan-run",
        "nextjs",
        "Create the phase artifact",
        &[],
        &["stateful_interaction".to_string()],
        &[],
        &[],
    )
    .unwrap()
    .expect("step contract should bind");

    assert_eq!(run.contract.profile.as_deref(), Some("generic"));
    assert_eq!(step.contract.profile, None);
}

#[test]
fn persistence_reset_runtime_guidance_names_reload_persistence_repair() {
    let report = RuntimeAcceptanceReport {
        missing_evidence: vec!["persistence_evidence".to_string()],
        ..RuntimeAcceptanceReport::default()
    };

    let guidance =
        runtime_acceptance_repair_guidance("nextjs", "Create a persistent notes app", &report)
            .join("\n");

    assert!(
        guidance.contains("persist the committed domain entity"),
        "{guidance}"
    );
    assert!(
        guidance.contains("server-side file/API/DB persistence"),
        "{guidance}"
    );
    assert!(
        guidance.contains("do not substitute persistence of a cleared draft input"),
        "{guidance}"
    );
}

#[test]
fn recovery_handoff_keeps_probe_infrastructure_failure_context() {
    let root = tempfile::tempdir().unwrap();
    let evidence_path = root.path().join("browser-interaction.json");
    std::fs::write(
        &evidence_path,
        serde_json::json!({
            "ok": false,
            "failure_category": "infrastructure",
            "failure_kind": "probe_infrastructure_failed:probe_script_error",
            "stage": "persistence_reload",
            "error": "element is not enabled"
        })
        .to_string(),
    )
    .unwrap();
    let mut report = VerificationReport::profile_failed(
        "missing_required_evidence:persistence_evidence",
    );
    report.push_profile_failure(format!(
        "interaction evidence path: {}",
        evidence_path.display()
    ));

    let reason = final_acceptance_recovery_reason(
        "nextjs",
        "Create a JSON-backed todo app",
        &report,
        "acceptance failed",
        "repair exhausted",
    );
    let evidence = final_acceptance_recovery_failure_evidence(
        "nextjs",
        "Create a JSON-backed todo app",
        &report,
        "acceptance failed",
    )
    .join("\n");

    assert!(
        reason.contains("probe_infrastructure_failed:probe_script_error"),
        "{reason}"
    );
    assert!(evidence.contains("persistence_reload"), "{evidence}");
    assert!(evidence.contains("element is not enabled"), "{evidence}");
}

#[test]
fn http_mutation_failure_preserves_status_and_targets_api_contract() {
    let failure = app_behavior_probe_failure_kind(
        "release gate failed: http_mutation_failed:DELETE:405; reload followed",
    );

    assert_eq!(
        failure.as_deref(),
        Some("browser_interaction_failed:http_mutation_failed:DELETE:405")
    );
    assert_eq!(
        interaction_repair_targets_for_reason(failure.as_deref().unwrap()),
        ["client_api_mutation_contract"]
    );
}
