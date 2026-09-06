use super::*;

#[test]
fn e1_successful_reads_do_not_promote_failed_build_or_business_acceptance() {
    for (label, commands, evidence, expected_reason) in [
        (
            "build failure",
            vec!["sh fixtures/build-failure.sh"],
            vec![],
            "command failed: sh fixtures/build-failure.sh",
        ),
        (
            "approval predicate failure",
            vec!["true", "sh fixtures/verify-approval.sh"],
            vec![],
            "command failed: sh fixtures/verify-approval.sh",
        ),
        (
            "missing business interaction evidence",
            vec!["true"],
            vec!["interaction_smoke"],
            "registered_observations_passed_but_completion_contract_acceptance_failed",
        ),
    ] {
        let root = tempfile::tempdir().unwrap();
        copy_fixture_tree(
            Path::new("tests/corpus/apps/issue428-bash-dynamic-route"),
            root.path(),
        );
        let mut config = config(root.path(), 1);
        config.offline = true;
        let contract = root.path().join("completion-contract.json");
        std::fs::write(
            &contract,
            serde_json::to_vec(&json!({
                "profile": "generic",
                "required_paths": ["src/app/api/expenses/[id]/approve/route.ts"],
                "verify_commands": commands,
                "required_evidence": evidence,
            }))
            .unwrap(),
        )
        .unwrap();
        config.completion_contract_path = Some(contract);
        let snapshot =
            crate::planner::recovery_snapshot::capture_for_transaction(root.path(), 1).unwrap();
        let treatment =
            crate::planner::recovery_snapshot::prepare_treatment(root.path(), &snapshot, 1)
                .unwrap();
        let treatment_config =
            crate::planner::recovery_contract_binding::bind_config(&config, &treatment).unwrap();
        let reads: Vec<serde_json::Value> = serde_json::from_str(
            &std::fs::read_to_string(treatment.join("fixtures/reads.json")).unwrap(),
        )
        .unwrap();
        for read in reads {
            let command = read["command"].as_str().unwrap();
            let outcome = crate::tools::bash::run_structured(
                command,
                &treatment,
                true,
                Duration::from_secs(5),
                || false,
            )
            .unwrap();
            assert!(outcome.is_success(), "{label}: {command}: {outcome:?}");
            assert!(outcome.stdout.contains(read["contains"].as_str().unwrap()));
        }
        // An attempted repair is visible only in the isolated treatment.
        let route = "src/app/api/expenses/[id]/approve/route.ts";
        let original = std::fs::read(root.path().join(route)).unwrap();
        let mut repaired = original.clone();
        repaired
            .extend_from_slice(b"\n// Treatment inspected the route but has not repaired it.\n");
        std::fs::write(treatment.join(route), &repaired).unwrap();

        let mut planner = UnusedClient;
        let mut execution = UnusedClient;
        let mut driver = RunnerRecoveryDriver {
            planner: &mut planner,
            execution: &mut execution,
            config: &config,
            ui: &crate::tui::NoopUi,
            transaction_snapshot: Some(snapshot),
            transaction_treatment: Some(treatment.clone()),
            transaction_config: Some(treatment_config),
            transaction_observer_identity: recovery_observer_identity(&config),
        };
        let outcome = driver.finish(
            1,
            &candidate("expense-recovery"),
            success("reads completed"),
        );
        assert!(
            outcome.result.is_err(),
            "{label}: incomplete treatment promoted"
        );
        assert_eq!(std::fs::read(root.path().join(route)).unwrap(), original);
        assert_eq!(std::fs::read(treatment.join(route)).unwrap(), repaired);
        let events: Vec<serde_json::Value> =
            std::fs::read_to_string(config.eval_events_path.unwrap())
                .unwrap()
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect();
        let decision = events
            .iter()
            .find(|event| event["event"] == "recovery_promotion_decision")
            .unwrap();
        assert_eq!(decision["decision"], "rejected", "{label}: {decision}");
        assert!(
            decision["reason"]
                .as_str()
                .unwrap()
                .starts_with(expected_reason),
            "{label}: {decision}"
        );
        assert!(
            !events
                .iter()
                .any(|event| event["event"] == "recovery_treatment_promoted")
        );
    }
}
