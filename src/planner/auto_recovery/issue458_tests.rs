#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::planner::recovery_snapshot::current_source_sha256;
    use crate::planner::repair::{RecoveryHandoff, save_recovery_ultra_plan};
    use serde_json::Value;

    const FIXTURE: &str =
        include_str!("../../../tests/corpus/apps/issue458-recovery-finish/cases.json");
    const CHECK: &str = "grep -qx good app.txt";

    include!("issue458_safety_tests.rs");

    fn setup(root: &Path, limit: u8, check: Option<&str>) -> Config {
        std::fs::write(root.join("app.txt"), "broken\n").unwrap();
        std::fs::write(root.join("frozen.txt"), "protected\n").unwrap();
        let mut config = config(root, limit);
        config.profile = "generic".into();
        config.profile_explicit = true;
        config.offline = true;
        config.yes = true;
        if let Some(check) = check {
            let path = root.join(".commandagent/contract.json");
            std::fs::write(
                &path,
                json!({
                    "goal":"Create ready.txt", "profile":"generic",
                    "required_paths":["app.txt", "frozen.txt"],
                    "verify_commands":[check], "protected_paths":["frozen.txt"]
                })
                .to_string(),
            )
            .unwrap();
            config.completion_contract_path = Some(path);
        }
        config
    }

    fn captured(config: &Config, kind: &str, diagnostic: &str) -> RecoveryCandidate {
        let capture = AttemptCaptureGuard::begin();
        record_execution_origin(config, "create").unwrap();
        let handoff = RecoveryHandoff {
            profile: "generic".into(),
            original_goal: "Create ready.txt".into(),
            failure_kind: kind.into(),
            failed_step: Some("repair-output".into()),
            failure_evidence: vec![diagnostic.into()],
            repair_targets: vec!["app.txt".into()],
            verify_commands: vec![CHECK.into()],
            ..RecoveryHandoff::default()
        };
        save_recovery_ultra_plan(&config.workspace_root, "issue458", &handoff).unwrap();
        capture.finish().unwrap()
    }

    fn runner<'a>(
        config: &'a Config,
        planner: &'a mut UnusedClient,
        execution: &'a mut UnusedClient,
    ) -> RunnerRecoveryDriver<'a> {
        RunnerRecoveryDriver {
            planner,
            execution,
            config,
            ui: &crate::tui::NOOP_UI,
            transaction_snapshot: None,
            transaction_treatment: None,
            transaction_config: None,
            transaction_observer_identity: None,
        }
    }

    fn events(config: &Config) -> Vec<Value> {
        std::fs::read_to_string(config.eval_events_path.as_ref().unwrap())
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    // Only the model execution and its effects are deterministic injections.
    // Every observation, YAML/resume check, snapshot, finish and adoption is production code.
    struct FinishHarness<'a> {
        real: RunnerRecoveryDriver<'a>,
        actions: VecDeque<String>,
        starts: Vec<u8>,
        original_hash: String,
    }

    impl RecoveryDriver for FinishHarness<'_> {
        type Prepared = crate::runs::ResumePlan;
        fn preflight(&mut self, candidate: &RecoveryCandidate, used: u8) -> RecoveryPreflight {
            self.real.preflight(candidate, used)
        }
        fn prepare(
            &mut self,
            candidate: &RecoveryCandidate,
        ) -> Result<Self::Prepared, CandidateStop> {
            self.real.prepare(candidate)
        }
        fn normalized(&self, prepared: &Self::Prepared) -> anyhow::Result<Vec<u8>> {
            self.real.normalized(prepared)
        }
        fn start(
            &mut self,
            used: u8,
            candidate: &RecoveryCandidate,
            prepared: &Self::Prepared,
        ) -> Result<(), CandidateStop> {
            assert_eq!(
                current_source_sha256(&self.real.config.workspace_root).unwrap(),
                self.original_hash
            );
            // A continuation is saved under control, never inside an old treatment.
            assert!(
                !candidate
                    .path
                    .to_string_lossy()
                    .contains("recovery-treatments")
            );
            self.real.start(used, candidate, prepared)?;
            let treatment = self.real.transaction_config.as_ref().unwrap();
            assert_eq!(
                std::fs::read_to_string(treatment.workspace_root.join("app.txt")).unwrap(),
                "broken\n"
            );
            assert!(!treatment.workspace_root.join("rejected-only.txt").exists());
            if used > 1 {
                assert!(candidate.handoff.changed_paths.is_empty());
                assert_eq!(candidate.original_intent.as_deref(), Some("create"));
                assert!(
                    !candidate
                        .plan
                        .phases
                        .iter()
                        .any(|phase| phase.prompt.contains("recovery-treatments"))
                );
            }
            self.starts.push(used);
            Ok(())
        }
        fn execute(&mut self, _prepared: Self::Prepared) -> AttemptOutcome {
            let config = self.real.transaction_config.as_ref().unwrap();
            let action = self
                .actions
                .pop_front()
                .expect("no execution beyond scripted bound");
            if action == "success" {
                std::fs::write(config.workspace_root.join("app.txt"), "good\n").unwrap();
                crate::eval_events::emit(
                    config.eval_events_path.as_deref(),
                    json!({"event":"ultra_plan_complete", "ok":true}),
                );
                return success("latest verified success");
            }
            std::fs::write(config.workspace_root.join("app.txt"), "unverified\n").unwrap();
            std::fs::write(
                config.workspace_root.join("rejected-only.txt"),
                "discard me\n",
            )
            .unwrap();
            if action == "verification-next" {
                std::fs::write(config.workspace_root.join("app.txt"), "different failure\n")
                    .unwrap();
                return success("execution alone is insufficient");
            }
            if action == "verification" {
                return success("execution alone is insufficient");
            }
            if action == "interrupted" {
                return failed(AttemptFailure::Interrupted);
            }
            let capture = AttemptCaptureGuard::begin();
            record_execution_origin(config, "recover").unwrap();
            let handoff = RecoveryHandoff {
                profile: "generic".into(),
                original_goal: "Create ready.txt".into(),
                failed_step: Some("repair-output".into()),
                failure_kind: action,
                failure_evidence: vec![format!(
                    "{}: latest repair diagnosis",
                    config.workspace_root.join("app.txt").display()
                )],
                repair_targets: vec![
                    config
                        .workspace_root
                        .join("app.txt")
                        .to_string_lossy()
                        .into(),
                ],
                changed_paths: vec!["rejected-only.txt".into()],
                verify_commands: vec![CHECK.into()],
                ..RecoveryHandoff::default()
            };
            save_recovery_ultra_plan(&config.workspace_root, "child", &handoff).unwrap();
            let child = capture.finish().unwrap();
            crate::eval_events::emit(
                config.eval_events_path.as_deref(),
                json!({
                    "event":"ultra_plan_failed", "ok":false,
                    "recovery_ultra_plan_path": child.path, "reason":"latest repair diagnosis"
                }),
            );
            failed(recoverable(child))
        }
        fn finish(
            &mut self,
            used: u8,
            candidate: &RecoveryCandidate,
            outcome: AttemptOutcome,
        ) -> AttemptOutcome {
            self.real.finish(used, candidate, outcome)
        }
    }

    #[test]
    fn issue458_production_finish_corpus_bounds_retry_cycles_and_success() {
        let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
        for case in fixture["sequences"].as_array().unwrap() {
            let dir = tempfile::tempdir().unwrap();
            let config = setup(
                dir.path(),
                case["limit"].as_u64().unwrap() as u8,
                Some(CHECK),
            );
            let first = captured(&config, "compile_error", "initial compile diagnosis");
            let mut planner = UnusedClient;
            let mut execution = UnusedClient;
            let mut driver = FinishHarness {
                real: runner(&config, &mut planner, &mut execution),
                actions: case["actions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().into())
                    .collect(),
                starts: vec![],
                original_hash: current_source_sha256(dir.path()).unwrap(),
            };
            let result = drive(&config, failed(recoverable(first)), &mut driver);
            let expected: Vec<u8> = (1..=case["used"].as_u64().unwrap() as u8).collect();
            assert_eq!(driver.starts, expected, "{case}: {result:?}");
            let events = events(&config);
            if case["id"] == "execution_then_success" {
                let expected = fixture["execution_then_success_events"].as_array().unwrap();
                let observed: Vec<_> = events
                    .iter()
                    .filter_map(|event| {
                        expected
                            .contains(&event["event"])
                            .then_some(event["event"].clone())
                    })
                    .collect();
                assert_eq!(&observed, expected);
            }
            let last = events.last().unwrap();
            assert_eq!(
                last["recovery_plan_auto_run_stop_reason"], case["stop"],
                "{case}: {result:?}"
            );
            assert_eq!(last["recovery_plan_auto_runs_used"], case["used"]);
            let decisions: Vec<_> = events
                .iter()
                .filter(|e| e["event"] == "recovery_promotion_decision")
                .collect();
            assert_eq!(decisions.len(), expected.len());
            for (i, decision) in decisions.iter().enumerate() {
                assert_eq!(decision["recovery_plan_auto_run_current"], i + 1);
                let promoted = i + 1 == decisions.len() && case["stop"] == "recovery_succeeded";
                assert_eq!(
                    decision["decision"],
                    if promoted { "promoted" } else { "rejected" }
                );
            }
            assert!(!dir.path().join("rejected-only.txt").exists());
            if case["stop"] == "recovery_succeeded" {
                assert_eq!(result.unwrap(), "latest verified success");
                let snapshot = crate::eval_events::latest_completion_snapshot(
                    config.eval_events_path.as_deref(),
                );
                let projection = crate::eval_events::project_completion(true, &snapshot);
                assert!(projection.recovery_ultra_plan_path.is_empty());
                assert!(projection.suggested_recovery_yaml_command.is_empty());
                assert!(!format!("{projection:?}").contains("latest repair diagnosis"));
                assert!(!format!("{projection:?}").contains("recovery-ultra-plan-child"));
            } else {
                assert!(result.is_err());
                assert_eq!(
                    current_source_sha256(dir.path()).unwrap(),
                    driver.original_hash
                );
                if matches!(
                    case["stop"].as_str(),
                    Some("limit_reached" | "cycle_detected")
                ) && case["used"] != 0
                {
                    let snapshot = crate::eval_events::latest_completion_snapshot(
                        config.eval_events_path.as_deref(),
                    );
                    let projection = crate::eval_events::project_completion(false, &snapshot);
                    assert!(
                        projection.recovery_ultra_plan_path.contains("continuation"),
                        "{projection:?}"
                    );
                    assert!(
                        !projection
                            .recovery_ultra_plan_path
                            .contains("recovery-treatments")
                    );
                }
            }
        }
    }

    #[test]
    fn issue458_real_post_observation_cycles_despite_timing_path_and_output_noise() {
        let dir = tempfile::tempdir().unwrap();
        let config = setup(dir.path(), 4, Some("python3 check.py"));
        std::fs::write(dir.path().join("check.py"), "import os, time, sys\nprint('latest diagnosis', os.getcwd(), flush=True)\ntime.sleep(0.01 if 'attempt-1/' in os.getcwd() else 0.03)\nsys.exit(1)\n").unwrap();
        let first = captured(&config, "compile_error", "initial diagnosis");
        let mut planner = UnusedClient;
        let mut execution = UnusedClient;
        let mut driver = FinishHarness {
            real: runner(&config, &mut planner, &mut execution),
            actions: vec!["verification".into(), "verification".into()].into(),
            starts: vec![],
            original_hash: current_source_sha256(dir.path()).unwrap(),
        };
        let error = drive(&config, failed(recoverable(first)), &mut driver).unwrap_err();
        assert!(error.to_string().contains("cycle detected"), "{error:#}");
        assert!(
            format!("{error:#}").contains("latest diagnosis"),
            "latest readable failure lost: {error:#}"
        );
        assert_eq!(driver.starts, [1, 2]);
        let events = events(&config);
        let observations: Vec<_> = events
            .iter()
            .filter(|e| {
                e["event"] == "recovery_preflight_observation"
                    && e["observation_phase"] == "post_recovery"
            })
            .collect();
        assert_eq!(observations.len(), 2);
        assert_ne!(observations[0]["reason"], observations[1]["reason"]);
        for event in observations {
            assert!(
                event["reason"]
                    .as_str()
                    .unwrap()
                    .contains("latest diagnosis")
            );
        }
        assert_eq!(
            events.last().unwrap()["recovery_plan_auto_run_stop_reason"],
            "cycle_detected"
        );
        assert_eq!(
            current_source_sha256(dir.path()).unwrap(),
            driver.original_hash
        );
    }

    #[test]
    fn issue458_changed_real_command_diagnosis_can_reach_the_next_attempt() {
        let dir = tempfile::tempdir().unwrap();
        let config = setup(dir.path(), 3, Some("python3 check.py"));
        std::fs::write(dir.path().join("check.py"), "from pathlib import Path\nimport sys\nvalue = Path('app.txt').read_text().strip()\nprint('diagnosis:', value)\nsys.exit(0 if value == 'good' else 1)\n").unwrap();
        let first = captured(&config, "compile_error", "initial diagnosis");
        let mut planner = UnusedClient;
        let mut execution = UnusedClient;
        let mut driver = FinishHarness {
            real: runner(&config, &mut planner, &mut execution),
            actions: vec![
                "verification".into(),
                "verification-next".into(),
                "success".into(),
            ]
            .into(),
            starts: vec![],
            original_hash: current_source_sha256(dir.path()).unwrap(),
        };
        assert_eq!(
            drive(&config, failed(recoverable(first)), &mut driver).unwrap(),
            "latest verified success"
        );
        assert_eq!(driver.starts, [1, 2, 3]);
        assert_eq!(
            events(&config).last().unwrap()["recovery_plan_auto_run_stop_reason"],
            "recovery_succeeded"
        );
    }

    #[test]
    fn issue458_finish_terminal_observation_and_transaction_matrix() {
        for mode in [
            "verification",
            "not_configured",
            "unavailable",
            "inconsistent",
            "authority",
            "drift",
            "protected",
            "snapshot",
            "treatment",
            "config",
            "identity",
            "promotion",
        ] {
            let dir = tempfile::tempdir().unwrap();
            let check = match mode {
                "not_configured" => None,
                "unavailable" => Some("python3 mutate.py"),
                "inconsistent" => Some("test -f app.txt"),
                _ => Some(CHECK),
            };
            let config = setup(dir.path(), 2, check);
            if mode == "unavailable" {
                std::fs::write(
                    dir.path().join("mutate.py"),
                    "from pathlib import Path\nPath('app.txt').write_text('mutated')\n",
                )
                .unwrap();
            }
            if mode == "inconsistent" {
                let path = config.completion_contract_path.as_ref().unwrap();
                let mut contract: Value =
                    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
                contract["required_obligations"] = json!(["investigation"]);
                std::fs::write(path, contract.to_string()).unwrap();
            }
            if mode == "promotion" {
                let path = dir.path().join("app.txt");
                let mut permissions = std::fs::metadata(&path).unwrap().permissions();
                permissions.set_readonly(true);
                std::fs::set_permissions(path, permissions).unwrap();
            }
            let before = current_source_sha256(dir.path()).unwrap();
            let first = captured(&config, "compile_error", "initial compile diagnosis");
            let mut planner = UnusedClient;
            let mut execution = UnusedClient;
            let mut real = runner(&config, &mut planner, &mut execution);
            let prepared = real.prepare(&first).unwrap();
            if mode == "not_configured" {
                assert_eq!(real.preflight(&first, 0), RecoveryPreflight::NotConfigured);
                assert_eq!(
                    real.start(1, &first, &prepared),
                    Err(CandidateStop::ObserverIdentityBindFailed)
                );
                assert!(
                    real.finish(1, &first, success("no authority"))
                        .result
                        .is_err()
                );
                assert_eq!(current_source_sha256(dir.path()).unwrap(), before);
                continue;
            }
            real.start(1, &first, &prepared)
                .unwrap_or_else(|error| panic!("{mode}: {error:?}"));
            let treatment = real
                .transaction_config
                .as_ref()
                .unwrap()
                .workspace_root
                .clone();
            match mode {
                "authority" => {
                    let path = treatment.join(".commandagent/altered-contract.json");
                    std::fs::write(&path, r#"{"goal":"changed","verify_commands":["true"]}"#)
                        .unwrap();
                    real.transaction_config
                        .as_mut()
                        .unwrap()
                        .completion_contract_path = Some(path);
                }
                "drift" => {
                    std::fs::write(dir.path().join("app.txt"), "external drift\n").unwrap();
                }
                "protected" => {
                    std::fs::write(treatment.join("frozen.txt"), "forbidden\n").unwrap();
                }
                "snapshot" => {
                    real.transaction_snapshot = None;
                }
                "treatment" => {
                    real.transaction_treatment = None;
                }
                "config" => {
                    real.transaction_config = None;
                }
                "identity" => {
                    real.transaction_observer_identity = None;
                }
                "promotion" => {
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        std::fs::set_permissions(
                            treatment.join("app.txt"),
                            std::fs::Permissions::from_mode(0o600),
                        )
                        .unwrap();
                    }
                    std::fs::write(treatment.join("app.txt"), "good\n").unwrap();
                }
                _ => {}
            }
            let outcome = real.finish(1, &first, success("candidate execution finished"));
            assert!(outcome.result.is_err(), "{mode}: unexpected promotion");
            assert_eq!(current_source_sha256(dir.path()).unwrap(), before, "{mode}");
            if mode == "verification" {
                assert!(
                    matches!(outcome.failure, Some(AttemptFailure::Recoverable(_))),
                    "{outcome:?}"
                );
            } else {
                assert!(
                    matches!(outcome.failure, Some(AttemptFailure::NonRecoverable)),
                    "{mode}: {outcome:?}"
                );
            }
            let events = events(&config);
            assert!(
                !events
                    .iter()
                    .any(|e| e["event"] == "recovery_treatment_promoted"),
                "{mode}"
            );
            if mode == "promotion" {
                assert!(events.iter().any(|e| {
                    e["reason"]
                        .as_str()
                        .is_some_and(|r| r.starts_with("treatment_promotion_failed:"))
                }));
            }
        }
    }
}
