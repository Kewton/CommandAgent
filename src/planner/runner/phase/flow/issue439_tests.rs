#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::minimal_loop::completion::CompletionContract;
    use crate::minimal_loop::evidence::verify_runtime_acceptance_with_browser_dirs_and_hints;
    use crate::planner::recovery_contract_authority as authority;
    use crate::planner::step_plan::PlanStep;
    use clap::Parser;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/apps/issue439-node-evidence")
    }

    fn contract(name: &str) -> CompletionContract {
        serde_json::from_slice(
            &std::fs::read(fixture().join(format!("fixtures/{name}-contract.json"))).unwrap(),
        )
        .unwrap()
    }

    fn config(root: &Path) -> Config {
        let mut config =
            Config::from_cli(crate::cli::Cli::parse_from(["commandagent", "--ux-demo"])).unwrap();
        config.workspace_root = root.to_owned();
        config.eval_events_path = Some(root.join(".commandagent/runs/issue439/events.jsonl"));
        std::fs::create_dir_all(config.eval_events_path.as_ref().unwrap().parent().unwrap())
            .unwrap();
        config
    }

    fn write(root: &Path, path: &str, text: &str) {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn materialize(root: &Path, name: &str, contract: &CompletionContract) {
        if name == "r0" {
            for path in contract
                .required_paths
                .iter()
                .map(String::as_str)
                .chain(["smoke-check.js"])
            {
                write(
                    root,
                    path,
                    &std::fs::read_to_string(fixture().join(path)).unwrap(),
                );
            }
        } else {
            let files: std::collections::BTreeMap<String, String> = serde_json::from_slice(
                &std::fs::read(fixture().join("fixtures/s1-source.json")).unwrap(),
            )
            .unwrap();
            for (path, text) in files {
                write(root, &path, &text);
            }
            for evidence in ["browser-readiness.json", "browser-interaction.json"] {
                write(
                    root,
                    evidence,
                    &std::fs::read_to_string(fixture().join(format!("fixtures/s1-{evidence}")))
                        .unwrap(),
                );
            }
        }
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
            &[],
            &[root.to_owned(), fixture()],
            &contract.evidence_hint_tokens,
        )
    }

    fn step(commands: Vec<String>) -> StepPlan {
        StepPlan {
            goal: "Verify generated app".into(),
            steps: vec![PlanStep {
                id: "check-app".into(),
                kind: "verify".into(),
                instruction: "Check generated app".into(),
                expected_result: "pass".into(),
                expected_paths: vec![],
                verify: commands,
            }],
        }
    }

    #[test]
    fn issue439_r0_s1_registration_generated_contract_and_recovery_handoff() {
        for name in ["r0", "s1"] {
            let root = tempfile::tempdir().unwrap();
            let original = contract(name);
            materialize(root.path(), name, &original);
            let config = config(root.path());
            let plan = UltraPlan {
                goal: original.goal.clone().unwrap(),
                profile: "nextjs".into(),
                intent: "create".into(),
                style: "balanced".into(),
                phases: vec![],
            };
            let (_paths, _guard) = begin(&config, &plan).unwrap();
            let mut commands = original.verify_commands.clone();
            commands.extend([
                "test -f package.json".into(),
                "cat package.json".into(),
                "node -p \"String(require('./package.json').scripts.build)=='next build' ? true : process.exit(1)\"".into(),
            ]);
            let admitted = step(commands);
            register_plan(&config, &admitted).unwrap();
            initialize(&config, &plan, &original.required_paths).unwrap();
            let generated = authority::load_for_handoff(&config).unwrap().unwrap();
            assert_eq!(generated.verify_commands, original.verify_commands);
            let bound = authority::bind_for_recovery(&config, &admitted.steps[0].verify).unwrap();
            let handoff = CompletionContract::load_for_config(&bound)
                .unwrap()
                .unwrap();
            assert_eq!(handoff, generated);
            let report = acceptance(root.path(), &handoff);
            assert!(report.passed, "{name}: {report:?}");
            let browser = crate::minimal_loop::evidence::browser_interaction_evidence_for_dirs(
                root.path(),
                &[root.path().to_owned(), fixture()],
            );
            assert_eq!(browser.interaction_evidence_status, "passed");
            assert!(report.weak_evidence.is_empty());
            assert!(report.weak_evidence_sources.is_empty());

            let events: Vec<serde_json::Value> = serde_json::from_slice(
                &std::fs::read(fixture().join(format!("fixtures/{name}-events.json"))).unwrap(),
            )
            .unwrap();
            assert!(
                events
                    .iter()
                    .any(|event| event["event"] == "ultra_final_acceptance"
                        && event["weak_evidence"]
                            == serde_json::json!(["node_smoke_without_assertion"]))
            );
            if name == "r0" {
                assert_eq!(
                    events
                        .iter()
                        .filter(
                            |event| event["event"] == "ultra_phase_complete" && event["ok"] == true
                        )
                        .count(),
                    4
                );
            }
        }
    }

    #[test]
    fn issue439_future_npm_and_node_tests_survive_registration_and_handoff() {
        let root = tempfile::tempdir().unwrap();
        let original = contract("r0");
        materialize(root.path(), "r0", &original);
        let config = config(root.path());
        let plan = UltraPlan {
            goal: original.goal.clone().unwrap(),
            profile: "nextjs".into(),
            intent: "create".into(),
            style: "balanced".into(),
            phases: vec![],
        };
        let (_paths, _guard) = begin(&config, &plan).unwrap();
        register_plan(
            &config,
            &step(vec!["npm test".into(), "node --test".into()]),
        )
        .unwrap();
        let generated = authority::load_for_handoff(&config).unwrap().unwrap();
        assert!(generated.verify_commands.contains(&"npm test".into()));
        assert!(generated.verify_commands.contains(&"node --test".into()));
        assert!(!acceptance(root.path(), &generated).passed);
        write(
            root.path(),
            "tests/app.test.cjs",
            "const assert = require('node:assert'); assert.equal(1 + 1, 2);\n",
        );
        let bound = authority::bind_for_recovery(&config, &[]).unwrap();
        let handoff = CompletionContract::load_for_config(&bound)
            .unwrap()
            .unwrap();
        assert_eq!(handoff.verify_commands, generated.verify_commands);
        assert!(acceptance(root.path(), &handoff).passed);
    }

    #[test]
    fn issue439_configured_s1_registry_is_immutable() {
        let root = tempfile::tempdir().unwrap();
        let original = contract("s1");
        materialize(root.path(), "s1", &original);
        let mut config = config(root.path());
        let path = root.path().join("configured-contract.json");
        let bytes = serde_json::to_vec_pretty(&original).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        config.completion_contract_path = Some(path.clone());
        register_plan(
            &config,
            &step(vec!["npm test".into(), "test -f package.json".into()]),
        )
        .unwrap();
        let bound = authority::bind_for_recovery(&config, &["node --test".into()]).unwrap();
        assert_eq!(std::fs::read(path).unwrap(), bytes);
        assert_eq!(
            CompletionContract::load_for_config(&bound)
                .unwrap()
                .unwrap()
                .verify_commands,
            original.verify_commands
        );
        assert!(acceptance(root.path(), &original).passed);
    }

    #[test]
    fn issue439_real_smoke_failure_remains_a_command_failure() {
        let root = tempfile::tempdir().unwrap();
        let original = contract("r0");
        materialize(root.path(), "r0", &original);
        let plan = step(vec!["node smoke-check.js".into()]);
        let passed = crate::planner::verify::verify_step(root.path(), &plan.steps[0]);
        assert!(passed.is_pass(), "{passed:?}");
        let page = std::fs::read_to_string(root.path().join("src/app/page.tsx")).unwrap();
        write(
            root.path(),
            "src/app/page.tsx",
            &page.replace("data-anvil-state", "data-removed-state"),
        );
        let failed = crate::planner::verify::verify_step(root.path(), &plan.steps[0]);
        assert!(!failed.is_pass(), "{failed:?}");
        assert_eq!(failed.command_failures.len(), 1);
        assert_eq!(failed.command_failures[0].command, "node smoke-check.js");
        assert!(
            failed.command_failures[0].reason.contains("exit"),
            "{failed:?}"
        );
    }

    #[test]
    fn issue439_final_event_identifies_weak_contract_commands() {
        let root = tempfile::tempdir().unwrap();
        write(root.path(), "app.js", "console.log('ok');\n");
        let mut config = config(root.path());
        config.profile = "generic".into();
        let mut contract = contract("r0");
        contract.profile = Some("generic".into());
        contract.goal = Some("Create a small app".into());
        contract.required_paths = vec!["app.js".into()];
        contract.required_capabilities.clear();
        contract.required_evidence = vec!["bound_verify_command".into()];
        contract.required_obligations.clear();
        contract.verify_commands = vec!["node app.js".into()];
        let path = root.path().join("contract.json");
        std::fs::write(&path, serde_json::to_vec(&contract).unwrap()).unwrap();
        config.completion_contract_path = Some(path);
        let plan = UltraPlan {
            goal: "Create a small app".into(),
            profile: "generic".into(),
            style: "balanced".into(),
            intent: "create".into(),
            phases: vec![],
        };
        let report =
            crate::planner::runner::final_acceptance::ultra_final_acceptance_report(&plan, &config)
                .unwrap();
        assert!(!report.is_pass());
        let events = std::fs::read_to_string(config.eval_events_path.unwrap()).unwrap();
        let event = events
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .find(|event| event["event"] == "ultra_final_acceptance")
            .unwrap();
        assert_eq!(
            event["weak_evidence"],
            serde_json::json!(["node_smoke_without_assertion"])
        );
        assert_eq!(
            event["weak_evidence_sources"],
            serde_json::json!([{
                "source": "contract_verify_command", "command": "node app.js", "reason": "node_smoke_without_assertion"
            }])
        );
    }

    #[test]
    fn issue439_retained_hook_final_verifier_checks_the_exact_source_path() {
        let root = tempfile::tempdir().unwrap();
        let original = contract("s1");
        materialize(root.path(), "s1", &original);
        let command = original
            .verify_commands
            .iter()
            .find(|command| command.starts_with("node -p"))
            .unwrap();
        // Isolate final command execution from the historical npm/browser results.
        let check: CompletionContract = serde_json::from_value(serde_json::json!({
            "required_paths": ["src/app/page.tsx"], "verify_commands": [command]
        }))
        .unwrap();
        let passed = check.verify_with_goal(root.path(), "Verify the declared source hook");
        assert!(passed.is_pass(), "{passed:?}");
        let page = std::fs::read_to_string(root.path().join("src/app/page.tsx")).unwrap();
        // The same hook in another source and a recorded browser pass cannot satisfy
        // a contract whose command explicitly names src/app/page.tsx.
        write(root.path(), "src/components/Other.tsx", &page);
        write(
            root.path(),
            "src/app/page.tsx",
            &page.replace("data-anvil-state", "data-removed-state"),
        );
        let failed = check.verify_with_goal(root.path(), "Verify the declared source hook");
        assert!(!failed.is_pass());
        assert_eq!(failed.command_failures.len(), 1, "{failed:?}");
        assert_eq!(failed.command_failures[0].command, *command);
    }
}
