#[cfg(test)]
mod cases {
    use super::super::*;
    use crate::minimal_loop::completion::CompletionContract;
    use crate::planner::step_plan::PlanStep;
    use clap::Parser;

    fn config(root: &Path, profile: &str) -> Config {
        let mut config =
            Config::from_cli(crate::cli::Cli::parse_from(["commandagent", "--ux-demo"])).unwrap();
        config.workspace_root = root.to_path_buf();
        config.profile = profile.to_string();
        let run = root.join(".commandagent/runs/fresh-session");
        std::fs::create_dir_all(&run).unwrap();
        config.eval_events_path = Some(run.join("events.jsonl"));
        config
    }

    fn plan(profile: &str) -> UltraPlan {
        UltraPlan {
            goal: "Create a persistent todo app".into(),
            profile: profile.into(),
            style: "balanced".into(),
            intent: "create".into(),
            phases: vec![UltraPhase {
                id: "core-implementation".into(),
                prompt: "Implement the app".into(),
            }],
        }
    }

    #[test]
    fn generation_registers_profile_authority_before_any_step_and_preserves_admitted_checks() {
        for profile in ["nextjs", "generic"] {
            let root = tempfile::tempdir().unwrap();
            let config = config(root.path(), profile);
            let _authority = crate::planner::recovery_contract_authority::begin_run(&config);
            let plan = plan(profile);
            initialize(&config, &plan, &["src/app/page.tsx".into()]).unwrap();
            let initial = crate::planner::recovery_contract_authority::load_for_handoff(&config)
                .unwrap()
                .unwrap();
            assert_eq!(
                initial.verify_commands,
                if ProfileId::parse(profile) == ProfileId::Nextjs {
                    vec!["npm run build"]
                } else {
                    vec![]
                }
            );
            assert!(!initial.required_capabilities.is_empty());
            let mut steps = StepPlan {
                goal: "A narrower phase goal".into(),
                steps: vec![
                    PlanStep {
                        id: "implement".into(),
                        kind: "implement".into(),
                        expected_result: "pass".into(),
                        instruction: "Implement the app".into(),
                        expected_paths: vec![],
                        verify: vec!["cargo test".into()],
                    },
                    PlanStep {
                        id: "negative".into(),
                        kind: "verify".into(),
                        expected_result: "fail".into(),
                        instruction: "Observe the failing case".into(),
                        expected_paths: vec![],
                        verify: vec!["cargo test --test must-remain-missing".into()],
                    },
                ],
            };
            for kind in ["setup", "inspect", "report"] {
                let mut transient = steps.steps[0].clone();
                transient.kind = kind.into();
                transient.verify = vec![format!("cargo test --test transient-{kind}")];
                steps.steps.push(transient);
            }
            register_plan(&config, &steps).unwrap();
            // Final acceptance regenerates the run contract with updated paths.
            initialize(
                &config,
                &plan,
                &["src/app/page.tsx".into(), "package.json".into()],
            )
            .unwrap();
            let refreshed = crate::planner::recovery_contract_authority::load_for_handoff(&config)
                .unwrap()
                .unwrap();
            assert!(refreshed.verify_commands.contains(&"cargo test".into()));
            assert!(
                !refreshed
                    .verify_commands
                    .contains(&"cargo test --test must-remain-missing".into())
            );
            assert_eq!(
                refreshed.required_capabilities,
                initial.required_capabilities
            );
            assert_eq!(refreshed.required_evidence, initial.required_evidence);
            assert_eq!(refreshed.required_obligations, initial.required_obligations);
            assert_eq!(refreshed.goal.as_deref(), Some(plan.goal.as_str()));
            assert_eq!(refreshed.required_paths.len(), 2);
            assert!(
                !refreshed
                    .verify_commands
                    .iter()
                    .any(|command| command.contains("transient-"))
            );
        }
    }

    #[test]
    fn every_phase_and_final_failure_handoff_inherits_generated_run_commands() {
        for failure_kind in [
            "phase_execute_error",
            "profile_invariant_failure",
            "final_acceptance_repair_failed",
            "final_acceptance_repair_exhausted",
            "implementation_compile_error",
        ] {
            let root = tempfile::tempdir().unwrap();
            let config = config(root.path(), "nextjs");
            let _authority = crate::planner::recovery_contract_authority::begin_run(&config);
            let plan = plan("nextjs");
            initialize(&config, &plan, &["src/app/page.tsx".into()]).unwrap();
            let saved = save_ultra_phase_recovery_handoff_with_evidence(
                &config,
                &plan,
                &plan.phases[0],
                UltraPhaseRecoveryRequest {
                    failure_kind,
                    reason: "Business acceptance is incomplete",
                    missing_paths: &[],
                    missing_signals: &["persistence".into()],
                    repair_targets: &["src/app/page.tsx".into()],
                    verify_commands: &[],
                },
                &["Build or business observation failed".into()],
            );
            assert!(saved.is_some());
            let events = std::fs::read_to_string(config.eval_events_path.unwrap()).unwrap();
            let saved: serde_json::Value = events
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .find(|event: &serde_json::Value| event["event"] == "recovery_prompt_saved")
                .unwrap();
            assert_eq!(saved["recovery_handoff_kind"], failure_kind);
            assert_eq!(saved["status"], "incomplete");
            let yaml = std::fs::read_to_string(
                root.path()
                    .join(saved["recovery_ultra_plan_path"].as_str().unwrap()),
            )
            .unwrap();
            assert!(yaml.contains("npm run build"), "{failure_kind}: {yaml}");
            assert!(yaml.contains("persistence"), "{failure_kind}: {yaml}");
            assert!(yaml.contains("Build or business observation failed"));
        }
    }

    #[test]
    fn explicit_contract_is_unchanged_by_initialization_registration_and_handoff() {
        let root = tempfile::tempdir().unwrap();
        let mut config = config(root.path(), "nextjs");
        let path = root.path().join("explicit.json");
        let text =
            r#"{"goal":"User goal","verify_commands":["test -f user.py"],"profile":"nextjs"}"#;
        std::fs::write(&path, text).unwrap();
        config.completion_contract_path = Some(path.clone());
        initialize(&config, &plan("nextjs"), &[]).unwrap();
        let mut steps = StepPlan::single("step");
        steps.steps[0].verify = vec!["npm run build".into()];
        register_plan(&config, &steps).unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), text);
        assert_eq!(
            CompletionContract::load_for_config(&config)
                .unwrap()
                .unwrap()
                .verify_commands,
            ["test -f user.py"]
        );
    }

    #[test]
    fn ultra_run_registers_build_authority_before_a_phase_plan_failure() {
        #[derive(Clone)]
        struct UnavailablePlanner;
        impl ChatClient for UnavailablePlanner {
            fn label(&self) -> &str {
                "local failure fixture"
            }
            fn boxed_clone(&self) -> Box<dyn ChatClient> {
                Box::new(self.clone())
            }
            fn chat(
                &mut self,
                _: &str,
                _: &[crate::state::ConversationMessage],
                _: &[crate::tools::registry::ToolSpec],
                _: bool,
            ) -> anyhow::Result<crate::providers::AssistantReply> {
                anyhow::bail!("deterministic planner failure")
            }
        }
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path(), "nextjs");
        let _authority = crate::planner::recovery_contract_authority::begin_run(&config);
        let mut initial = plan("nextjs");
        initial.phases.push(UltraPhase {
            id: "acceptance".into(),
            prompt: "Verify the complete app".into(),
        });
        let result = run_ultra_plan_with_ui(
            &mut UnavailablePlanner,
            &mut UnavailablePlanner,
            &initial,
            &config,
            &crate::tui::NOOP_UI,
        );
        assert!(result.is_err());
        let contract = crate::planner::recovery_contract_authority::load_for_handoff(&config)
            .unwrap()
            .unwrap_or_else(|| panic!("run authority must precede phase planning: {result:?}"));
        assert_eq!(contract.verify_commands, ["npm run build"]);
    }
}
