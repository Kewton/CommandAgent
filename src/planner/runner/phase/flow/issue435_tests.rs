#[cfg(test)]
mod cases {
    use super::super::*;
    use crate::minimal_loop::completion::CompletionContract;
    use crate::minimal_loop::evidence::verify_runtime_acceptance_with_browser_dirs_and_hints;
    use crate::minimal_loop::repair_target::{RepairTarget, classify_repair_target};
    use crate::planner::recovery_contract_authority as authority;
    use crate::planner::step_plan::PlanStep;
    use crate::planner::verify::VerificationReport;
    use clap::Parser;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/corpus/apps/issue435-artifact-only-registration")
    }

    fn original_contract() -> CompletionContract {
        serde_json::from_slice(
            &std::fs::read(fixture().join("fixtures/original-contract.json")).unwrap(),
        )
        .unwrap()
    }

    fn config(root: &Path) -> Config {
        let mut config =
            Config::from_cli(crate::cli::Cli::parse_from(["commandagent", "--ux-demo"])).unwrap();
        config.workspace_root = root.to_path_buf();
        config.eval_events_path = Some(root.join(".commandagent/runs/issue435/events.jsonl"));
        std::fs::create_dir_all(config.eval_events_path.as_ref().unwrap().parent().unwrap())
            .unwrap();
        config
    }

    fn acceptance(
        root: &Path,
        contract: &CompletionContract,
    ) -> crate::minimal_loop::evidence::RuntimeAcceptanceReport {
        verify_runtime_acceptance_with_browser_dirs_and_hints(
            root,
            &contract.required_paths,
            &contract.verify_commands,
            &contract.required_capabilities,
            &contract.required_evidence,
            &contract.required_obligations,
            &contract
                .deferred_verify_requirements
                .iter()
                .map(|requirement| requirement.command.clone())
                .collect::<Vec<_>>(),
            &[fixture()],
            &contract.evidence_hint_tokens,
        )
    }

    #[test]
    fn r0_contract_registration_removes_inspections_without_weakening_acceptance() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path());
        let original = original_contract();
        for rel in &original.required_paths {
            let target = root.path().join(rel);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::copy(fixture().join(rel), target).unwrap();
        }
        let before = acceptance(root.path(), &original);
        assert!(!before.passed);
        assert_eq!(
            before.primary_reason,
            "weak_verification_evidence:artifact_only_verify:test -f 'package.json'"
        );
        let cases: serde_json::Value = serde_json::from_slice(
            &std::fs::read(fixture().join("fixtures/campaign-cases.json")).unwrap(),
        )
        .unwrap();
        let plan = UltraPlan {
            goal: original.goal.clone().unwrap(),
            profile: "nextjs".into(),
            intent: "create".into(),
            style: "balanced".into(),
            phases: cases["r0_phases"]
                .as_array()
                .unwrap()
                .iter()
                .map(|phase| {
                    assert_eq!(phase["ok"], true);
                    UltraPhase {
                        id: phase["phase_id"].as_str().unwrap().into(),
                        prompt: "R0 recorded phase".into(),
                    }
                })
                .collect(),
        };
        assert_eq!(plan.phases.len(), 4);
        let (_paths, _guard) = begin(&config, &plan).unwrap();
        // Reproduce admitted checks across all four recorded phases. Execution and
        // build success are historical inputs; this test exercises registration.
        for (phase, kind) in plan
            .phases
            .iter()
            .zip(["setup", "implement", "implement", "verify"])
        {
            let step = PlanStep {
                id: phase.id.clone(),
                kind: kind.into(),
                expected_result: "pass".into(),
                instruction: phase.prompt.clone(),
                expected_paths: original.required_paths.clone(),
                verify: original.verify_commands.clone(),
            };
            register_plan(
                &config,
                &StepPlan {
                    goal: plan.goal.clone(),
                    steps: vec![step],
                },
            )
            .unwrap();
        }
        initialize(&config, &plan, &original.required_paths).unwrap();
        let generated = authority::load_for_handoff(&config).unwrap().unwrap();
        assert_eq!(generated.verify_commands, ["npm run build"]);
        assert_eq!(
            generated.required_capabilities,
            original.required_capabilities
        );
        assert_eq!(generated.required_evidence, original.required_evidence);
        assert_eq!(
            generated.required_obligations,
            original.required_obligations
        );
        assert_eq!(generated.required_paths, original.required_paths);
        let after = acceptance(root.path(), &generated);
        assert!(after.passed, "{after:?}");
        assert!(after.weak_evidence.is_empty(), "{after:?}");

        // Existing/configured weak commands still fail even alongside a real build.
        for command in [
            "test -f 'package.json'",
            "test -e 'package.json'",
            "cat package.json",
        ] {
            let mut weak = generated.clone();
            weak.verify_commands.push(command.into());
            let report = acceptance(root.path(), &weak);
            assert!(!report.passed, "{command}: {report:?}");
            assert!(
                report
                    .weak_evidence
                    .contains(&format!("artifact_only_verify:{command}"))
            );
        }

        // A recorded browser pass and build command cannot rescue an engine scaffold.
        std::fs::remove_file(root.path().join("src/app/page.tsx")).unwrap();
        crate::planner::profiles::nextjs::complete_scaffold(
            root.path(),
            &["src/app/page.tsx".into()],
        )
        .unwrap();
        let scaffold = acceptance(root.path(), &generated);
        assert!(!scaffold.passed, "{scaffold:?}");
        assert!(
            scaffold
                .weak_evidence
                .iter()
                .any(|reason| reason == "non_implementation_obligation_only:scaffold"),
            "{scaffold:?}"
        );
    }

    #[test]
    fn inspection_only_and_mixed_steps_keep_substantive_registered_checks() {
        for profile in ["nextjs", "generic"] {
            let root = tempfile::tempdir().unwrap();
            let config = config(root.path());
            let plan = UltraPlan {
                goal: "Create an app".into(),
                profile: profile.into(),
                intent: "create".into(),
                style: "balanced".into(),
                phases: vec![],
            };
            let (_paths, _guard) = begin(&config, &plan).unwrap();
            let mut steps = StepPlan::single("Inspect generated artifacts");
            steps.steps[0].kind = "verify".into();
            steps.steps[0].verify = vec![
                "test -f 'package.json'".into(),
                "test -e src/app/page.tsx".into(),
                "cat package.json".into(),
                "test\t-f\tapp.js".into(),
            ];
            let original_checks = steps.steps[0].verify.clone();
            register_plan(&config, &steps).unwrap();
            assert_eq!(steps.steps[0].verify, original_checks);
            let contract = authority::load_for_handoff(&config).unwrap().unwrap();
            assert_eq!(
                contract.verify_commands,
                if ProfileId::parse(profile) == ProfileId::Nextjs {
                    vec!["npm run build"]
                } else {
                    vec![]
                }
            );
            steps.steps[0].verify.extend([
                "npm run build".into(),
                "test -f 'package.json' && npm test".into(),
                "cargo test".into(),
            ]);
            register_plan(&config, &steps).unwrap();
            let contract = authority::load_for_handoff(&config).unwrap().unwrap();
            assert_eq!(contract.verify_commands.len(), 3);
            for check in ["npm run build", "npm test", "cargo test"] {
                assert!(contract.verify_commands.contains(&check.into()));
            }
            steps.steps[0].verify =
                vec!["test -f package.json && cargo test --manifest-path ../Cargo.toml".into()];
            assert!(register_plan(&config, &steps).is_err());
            assert_eq!(
                authority::load_for_handoff(&config)
                    .unwrap()
                    .unwrap()
                    .verify_commands,
                contract.verify_commands
            );
        }
    }

    #[test]
    fn empty_generated_contract_filters_failure_handoff_and_retains_profile_fallback() {
        for profile in ["nextjs", "generic"] {
            for mixed in [false, true] {
                let root = tempfile::tempdir().unwrap();
                let config = config(root.path());
                let _guard = authority::begin_run(&config);
                let mut contract = original_contract();
                contract.profile = Some(profile.into());
                contract.verify_commands.clear();
                let path = config
                    .eval_events_path
                    .as_ref()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .join("completion-contract-ultra-plan-run.json");
                std::fs::write(&path, serde_json::to_vec(&contract).unwrap()).unwrap();
                authority::record_generated_contract(&config, "ultra-plan-run", &path);
                let mut commands = vec![
                    "test -f 'package.json'".into(),
                    "cat package.json".into(),
                    "test -e src/app/page.tsx".into(),
                ];
                if mixed {
                    commands.push("test -f 'package.json' && npm test".into());
                }
                let bound = authority::bind_for_recovery(&config, &commands).unwrap();
                let bound = CompletionContract::load_for_config(&bound)
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    bound.verify_commands,
                    if mixed {
                        vec!["npm test"]
                    } else if ProfileId::parse(profile) == ProfileId::Nextjs {
                        vec!["npm run build"]
                    } else {
                        vec![]
                    }
                );
            }
        }
    }

    #[test]
    fn campaign_evidence_failures_target_implementation_and_preserve_config_diagnostics() {
        let fixture: serde_json::Value = serde_json::from_slice(
            &std::fs::read(fixture().join("fixtures/campaign-cases.json")).unwrap(),
        )
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let mut report = VerificationReport::pass();
            for failure in case["profile_failures"].as_array().unwrap() {
                report.push_profile_failure(failure.as_str().unwrap());
            }
            assert_eq!(
                classify_repair_target(&report).as_str(),
                case["expected_target"].as_str().unwrap(),
                "{case}"
            );
            assert!(!report.is_pass());
            report.push_profile_failure("scripts.build must be next build");
            assert_eq!(classify_repair_target(&report), RepairTarget::PackageConfig);
        }
        for reason in [
            "artifact_only_verify:test -f 'package.json'",
            "repair guidance: test -f 'package.json'",
            "weak_verification_evidence:artifact_only_verify:cat package.json",
        ] {
            let mut report = VerificationReport::pass();
            report.push_profile_failure(reason);
            assert_ne!(classify_repair_target(&report), RepairTarget::PackageConfig);
            report.push_profile_failure("tsconfig module resolution invalid");
            assert_eq!(
                classify_repair_target(&report),
                RepairTarget::FrameworkConfig
            );
        }
    }
}
