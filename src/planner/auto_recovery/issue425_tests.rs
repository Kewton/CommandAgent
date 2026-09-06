#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::minimal_loop::completion::CompletionContract;
    use crate::planner::recovery_contract_authority;

    fn generated_contract(config: &Config, profile: &str, commands: &[&str]) -> PathBuf {
        let path = crate::planner::completion_contract_path::generated_path(
            &config.workspace_root,
            config.eval_events_path.as_deref(),
            "completion-contract-ultra-plan-run.json",
        );
        std::fs::write(
            &path,
            serde_json::json!({
                "goal": "Create a persistent todo app", "profile": profile,
                "required_paths": ["app.js"], "verify_commands": commands,
                "required_evidence": ["persistence_evidence"],
            })
            .to_string(),
        )
        .unwrap();
        recovery_contract_authority::record_generated_contract(config, "ultra-plan-run", &path);
        path
    }

    #[test]
    fn selected_stagnation_step_keeps_authority_when_competing_with_phase_candidates() {
        for profile in ["nextjs", "generic"] {
            for initially_registered in [false, true] {
                for step_has_verify in [false, true] {
                    let root = tempfile::tempdir().unwrap();
                    let mut config = config(root.path(), 2);
                    let _authority = recovery_contract_authority::begin_run(&config);
                    config.profile = profile.into();
                    let commands = if initially_registered {
                        vec!["test -f app.js"]
                    } else {
                        vec![]
                    };
                    generated_contract(&config, profile, &commands);
                    // These are admitted original-plan checks, before any failure proposal.
                    if step_has_verify {
                        recovery_contract_authority::register_step_plan_commands(
                            &config,
                            &["test -f app.js".into()],
                        )
                        .unwrap();
                    }
                    let step_path = root
                        .path()
                        .join(".commandagent/completion-contract-plan-run.json");
                    std::fs::write(
                        &step_path,
                        r#"{"goal":"Execute one narrow step","verify_commands":[]}"#,
                    )
                    .unwrap();
                    recovery_contract_authority::record_generated_contract(
                        &config, "plan-run", &step_path,
                    );
                    let mut step_config = config.clone();
                    step_config.completion_contract_path = Some(step_path);
                    let capture = AttemptCaptureGuard::begin();
                    record_candidate(
                        "first-phase.yaml".into(),
                        plan("phase"),
                        "phase_execute_error".into(),
                        None,
                        vec![],
                    );
                    crate::minimal_loop::stagnation_escalation::save_read_only_write_required_handoff(
                    &step_config,
                    "Execute one step",
                    "plan-run-step",
                    "implement",
                    Some("core-implementation"),
                    &["app.js".into()],
                    "selected",
                    &[],
                    5,
                    2,
                )
                .expect("stagnation handoff");
                    record_candidate(
                        "later-phase.yaml".into(),
                        plan("phase"),
                        "profile_invariant_failure".into(),
                        None,
                        vec!["test -f unrelated".into()],
                    );
                    let selected = capture.finish().unwrap();
                    assert_eq!(
                        selected.handoff.failure_kind,
                        "model_stagnation:read_only_loop"
                    );
                    assert_eq!(
                        selected.handoff.original_goal,
                        "Create a persistent todo app"
                    );
                    let bound = recovery_contract_authority::bind_for_recovery(
                        &config,
                        &selected.handoff.verify_commands,
                    )
                    .unwrap();
                    let contract = CompletionContract::load_for_config(&bound)
                        .unwrap()
                        .unwrap();
                    if profile == "generic" && !initially_registered && !step_has_verify {
                        assert!(contract.verify_commands.is_empty());
                        assert_eq!(
                            bind_candidate_verify_commands(&bound, selected, "missing app.js")
                                .unwrap_err(),
                            CandidateStop::ContractCommandBindFailed
                        );
                        continue;
                    }
                    assert!(!contract.verify_commands.is_empty());
                    let rebound =
                        bind_candidate_verify_commands(&bound, selected, "missing app.js").unwrap();
                    assert_eq!(rebound.handoff.verify_commands, contract.verify_commands);
                    assert!(
                        !rebound
                            .handoff
                            .verify_commands
                            .contains(&"test -f unrelated".into())
                    );
                    let mut driver = driver(vec![failed(AttemptFailure::NonRecoverable)]);
                    let result = drive(&bound, failed(recoverable(rebound)), &mut driver);
                    assert!(result.is_err());
                    assert_eq!(driver.starts, [1]);
                    let events = std::fs::read_to_string(config.eval_events_path.unwrap()).unwrap();
                    assert!(!events.contains("contract_command_bind_failed"));
                }
            }
        }
    }

    #[test]
    fn data_candidate_uses_only_preexisting_registered_commands() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path(), 2);
        let _authority = recovery_contract_authority::begin_run(&config);
        let path = generated_contract(
            &config,
            "data",
            &["python3 pipeline/main.py data/input.csv"],
        );
        let before = std::fs::read(&path).unwrap();
        let bound = recovery_contract_authority::bind_for_recovery(
            &config,
            &["python3 unregistered.py".into()],
        )
        .unwrap();
        let mut original = candidate("repair");
        original.handoff.profile = "data".into();
        original.plan.profile = "data".into();
        original.handoff.verify_commands = vec!["python3 unregistered.py".into()];
        original.handoff.repair_targets = vec!["pipeline/main.py".into()];
        let rebound =
            bind_candidate_verify_commands(&bound, original, "registered pipeline failed").unwrap();
        assert_eq!(
            rebound.handoff.verify_commands,
            ["python3 pipeline/main.py data/input.csv"]
        );
        assert_eq!(std::fs::read(path).unwrap(), before);
    }

    struct MeasuredRepairDriver {
        config: Config,
        starts: Vec<u8>,
        preflight_failed: bool,
        build_passed: bool,
        business_passed: bool,
    }

    #[test]
    fn reopened_handoff_corpus_binds_registered_commands_and_starts_recovery() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/corpus/apps/issue425-generated-recovery-contract/fixtures/reopened-handoffs.json"
    )).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let root = tempfile::tempdir().unwrap();
            let config = config(root.path(), 2);
            let _authority = recovery_contract_authority::begin_run(&config);
            let strings = |key: &str| {
                case[key]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_str().unwrap().to_string())
                    .collect::<Vec<_>>()
            };
            let registered = strings("registered");
            let profile = case["profile"].as_str().unwrap();
            generated_contract(
                &config,
                profile,
                &registered.iter().map(String::as_str).collect::<Vec<_>>(),
            );
            let mut original = candidate("Create a persistent todo app");
            original.plan.profile = profile.into();
            original.handoff.profile = profile.into();
            original.handoff.failure_kind = case["failure_kind"].as_str().unwrap().into();
            original.handoff.failed_step = case["failed_step"].as_str().map(str::to_string);
            original.handoff.verify_commands = strings("handoff");
            original.handoff.repair_targets = vec!["app.js".into()];
            let bound = recovery_contract_authority::bind_for_recovery(
                &config,
                &original.handoff.verify_commands,
            )
            .unwrap();
            let rebound =
                bind_candidate_verify_commands(&bound, original, "business observation failed")
                    .unwrap();
            assert_eq!(
                rebound.handoff.verify_commands,
                strings("expected"),
                "{}",
                case["id"]
            );
            let mut driver = driver(vec![failed(AttemptFailure::NonRecoverable)]);
            assert!(drive(&bound, failed(recoverable(rebound)), &mut driver).is_err());
            assert_eq!(driver.starts, [1], "{}", case["id"]);
            let events = std::fs::read_to_string(config.eval_events_path.unwrap()).unwrap();
            assert!(
                !events.contains("contract_command_bind_failed"),
                "{}",
                case["id"]
            );
            assert!(!events.contains("recovery_succeeded"), "{}", case["id"]);
        }
    }

    impl RecoveryDriver for MeasuredRepairDriver {
        type Prepared = RecoveryCandidate;

        fn preflight(&mut self, candidate: &RecoveryCandidate) -> RecoveryPreflight {
            let result = recovery_preflight(&self.config, candidate, 0);
            self.preflight_failed = matches!(&result, RecoveryPreflight::Failed { .. });
            result
        }

        fn prepare(
            &mut self,
            candidate: &RecoveryCandidate,
        ) -> Result<Self::Prepared, CandidateStop> {
            prepare_candidate(&self.config, candidate)?;
            Ok(candidate.clone())
        }

        fn normalized(&self, candidate: &Self::Prepared) -> anyhow::Result<Vec<u8>> {
            normalized_plan(&candidate.plan)
        }

        fn start(
            &mut self,
            used: u8,
            candidate: &RecoveryCandidate,
            _: &Self::Prepared,
        ) -> Result<(), CandidateStop> {
            self.starts.push(used);
            emit_with_candidate(
                &self.config,
                "recovery_plan_auto_run_start",
                used,
                candidate,
                None,
                None,
            );
            Ok(())
        }

        fn execute(&mut self, candidate: Self::Prepared) -> AttemptOutcome {
            // A deterministic local repair replaces the broken source; no provider is used.
            std::fs::write(
                self.config.workspace_root.join("app.js"),
                "export const todo = [];\n",
            )
            .unwrap();
            let step = crate::planner::step_plan::PlanStep {
                id: "measure-build".into(),
                kind: "verify".into(),
                expected_result: "pass".into(),
                instruction: "Measure registered build after repair".into(),
                expected_paths: vec!["app.js".into()],
                verify: candidate.handoff.verify_commands.clone(),
            };
            self.build_passed =
                crate::planner::verify::verify_step(&self.config.workspace_root, &step).is_pass();
            let acceptance =
                crate::planner::runner::recovery_acceptance::runtime_acceptance_report(
                    &candidate.plan,
                    &self.config,
                )
                .unwrap();
            self.business_passed = acceptance.passed;
            assert!(
                acceptance
                    .missing_evidence
                    .contains(&"persistence_evidence".into())
            );
            failed(AttemptFailure::NonRecoverable)
        }
    }

    #[test]
    fn independent_sessions_measure_recovery_build_and_business_acceptance_separately() {
        for _session in 0..2 {
            let root = tempfile::tempdir().unwrap();
            let config = config(root.path(), 2);
            let _authority = recovery_contract_authority::begin_run(&config);
            // This is a synthetic local build fixture, not a live Next.js business E2E.
            std::fs::write(root.path().join("package.json"), r#"{"type":"module"}"#).unwrap();
            std::fs::write(root.path().join("app.js"), "export const todo = ;\n").unwrap();
            generated_contract(&config, "generic", &[]);
            recovery_contract_authority::register_step_plan_commands(
                &config,
                &["node --check app.js".into()],
            )
            .unwrap();
            let bound = recovery_contract_authority::bind_for_recovery(&config, &[]).unwrap();
            let mut initial = candidate("Create a persistent todo app");
            initial.handoff.failed_phase = Some("core-implementation".into());
            initial.handoff.verify_commands.clear();
            initial.handoff.repair_targets = vec!["app.js".into()];
            let mut driver = MeasuredRepairDriver {
                config: bound.clone(),
                starts: vec![],
                preflight_failed: false,
                build_passed: false,
                business_passed: false,
            };
            let result = drive(&bound, failed(recoverable(initial)), &mut driver);
            assert!(
                result.is_err(),
                "missing business acceptance must remain failure"
            );
            assert_eq!(driver.starts, [1], "{result:?}");
            assert!(driver.preflight_failed);
            assert!(
                driver.build_passed,
                "local build must actually pass after repair"
            );
            assert!(!driver.business_passed);
            let events = std::fs::read_to_string(config.eval_events_path.unwrap()).unwrap();
            assert!(events.contains("recovery_plan_auto_run_start"));
            assert!(!events.contains("contract_command_bind_failed"));
            assert!(!events.contains("recovery_succeeded"));
        }
    }
}
