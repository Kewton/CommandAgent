#[cfg(test)]
mod issue420 {
    use super::*;

    const PAGE: &str = "src/app/page.tsx";
    const API: &str = "src/app/api/projects/route.ts";
    const IMPLEMENTED: &str =
        "export default function Page(){ return <main>Agency projects</main>; }\n";

    fn fixture() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/apps/issue420-step-completion")
    }

    fn setup() -> (tempfile::TempDir, Config, CompletionContract) {
        let dir = tempfile::tempdir().unwrap();
        for path in [PAGE, API, "package.json"] {
            let target = dir.path().join(path);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::copy(fixture().join(path), target).unwrap();
        }
        let contract: CompletionContract = serde_json::from_value(json!({
            "required_paths": [PAGE, API],
            "required_evidence": ["nextjs_route_evidence", "build_command_or_dependency_missing_boundary"],
            "required_obligations": ["implementation"],
        })).unwrap();
        let mut cfg = config(dir.path().to_path_buf());
        cfg.profile = "nextjs".to_string();
        cfg.eval_events_path = Some(dir.path().join("events.jsonl"));
        cfg.completion_contract_path = Some(dir.path().join("contract.json"));
        std::fs::write(
            cfg.completion_contract_path.as_ref().unwrap(),
            serde_json::to_vec(&contract).unwrap(),
        )
        .unwrap();
        (dir, cfg, contract)
    }

    fn options(enforcement: ContractEnforcement) -> RunSessionOptions {
        let mut options = RunSessionOptions::plan_step_with_enforcement(
            RunSessionStepKind::Implement,
            enforcement,
            Some("core-implementation".to_string()),
        );
        options.step_id = Some("implement-page".to_string());
        options
    }

    fn write_reply(path: &str, content: &str) -> AssistantReply {
        AssistantReply {
            content: String::new(),
            tool_calls: vec![ToolCall::new(
                "Write",
                json!({"path":path,"content":content}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        }
    }

    fn run(
        cfg: &Config,
        options: RunSessionOptions,
        replies: Vec<AssistantReply>,
    ) -> anyhow::Result<RunSessionOutcome> {
        let mut fake = Fake::new(replies.into_iter().map(Ok).collect());
        run_session_with_outcome_with_options(
            &mut fake,
            &mut SessionSnapshot::new(),
            "Implement the agency project management page.",
            &[PAGE.to_string()],
            cfg,
            &NOOP_UI,
            options,
        )
    }

    #[test]
    fn campaign_reaudit_preserves_19_outcomes_and_rejects_422_only_iteration_success() {
        let fixture: Value = serde_json::from_slice(
            &std::fs::read(fixture().join("fixtures/campaign-events.json")).unwrap(),
        )
        .unwrap();
        let cases = fixture["cases"].as_array().unwrap();
        let mut counts = [0; 5];
        let mut runs = BTreeSet::new();
        for case in cases {
            runs.insert(case["run"].as_str().unwrap());
            let terminal = &case["terminal"];
            match terminal["terminal_status"].as_str().unwrap() {
                "skipped" => counts[0] += 1,
                "failed" => counts[1] += 1,
                "completed" if terminal["changed_path_count"] == 0 => counts[3] += 1,
                "completed" => counts[2] += 1,
                other => panic!("unexpected terminal {other}"),
            }
            counts[4] += usize::from(case["short_circuit"].get("step_id").is_none());
            assert_eq!(case["step_id"], terminal["step_id"]);
            if case["short_circuit"]["at"] == "start" {
                assert_eq!(case["expectation"]["runner_skip_preserved"], true);
                assert_eq!(terminal["terminal_status"], "skipped");
                continue;
            }
            let (_dir, cfg, mut contract) = setup();
            contract.required_obligations =
                serde_json::from_value(case["completion_verify"]["required_obligations"].clone())
                    .unwrap();
            let mut options = options(ContractEnforcement::Observe);
            options.step_id = Some(case["step_id"].as_str().unwrap().to_string());
            options.phase_scope = Some(
                case["short_circuit"]["phase_scope"]
                    .as_str()
                    .unwrap()
                    .to_string(),
            );
            let paths = vec![PAGE.to_string()];
            let implementation = ImplementationCompletion::capture(&cfg, &options, &paths);
            // #422 rejects this page's role, but the backend satisfies the run.
            assert_eq!(
                case["completion_verify"]["page_obligation"]["role"],
                "scaffold"
            );
            assert_eq!(
                case["completion_verify"]["page_obligation"]["satisfies_implementation"],
                false
            );
            assert!(
                contract.verify(&cfg.workspace_root).is_pass(),
                "{:?}",
                contract.verify(&cfg.workspace_root)
            );
            assert_eq!(case["expectation"]["legacy_422_short_circuit"], true);
            let mut attempts = 0;
            let outcome = maybe_short_circuit_satisfied_step(
                &cfg,
                &options,
                "Implement requested UI",
                &paths,
                Some(&contract),
                ShortCircuitContext {
                    verify_attempts: &mut attempts,
                    at: StepShortCircuitAt::Iteration,
                    write_or_edit_seen: false,
                    implementation: &implementation,
                    changed_paths: &[],
                },
            )
            .unwrap();
            assert!(outcome.is_none(), "{case}");
            assert_eq!(case["expectation"]["iteration_placeholder_blocked"], true);
            assert_eq!(attempts, 0);
            let events = event_values(cfg.eval_events_path.as_ref().unwrap());
            assert!(events.iter().all(|e| e["event"] != "step_short_circuited"));
            let blocked = events
                .iter()
                .find(|e| e["event"] == "step_completion_blocked")
                .unwrap();
            assert_eq!(blocked["step_id"], case["step_id"]);
            assert_eq!(blocked["scaffold_placeholder_paths"], json!([PAGE]));
        }
        assert_eq!(cases.len(), 19);
        assert_eq!(runs.len(), 9);
        assert_eq!(counts, [8, 2, 3, 6, 11]);
        assert_eq!(fixture["expected_totals"]["completed_unchanged"], counts[3]);
    }

    #[test]
    fn api_before_ui_requires_page_write_under_enforce_and_observe() {
        for enforcement in [ContractEnforcement::Enforce, ContractEnforcement::Observe] {
            let (_dir, cfg, _) = setup();
            let outcome = run(
                &cfg,
                options(enforcement),
                vec![
                    read_reply(API),
                    read_reply(PAGE),
                    write_reply(PAGE, IMPLEMENTED),
                ],
            )
            .unwrap();
            assert_eq!(outcome.changed_paths, vec![PAGE]);
            assert_eq!(outcome.iterations, 3);
            assert_eq!(
                std::fs::read_to_string(cfg.workspace_root.join(PAGE)).unwrap(),
                IMPLEMENTED
            );
            assert!(
                !event_values(cfg.eval_events_path.as_ref().unwrap())
                    .iter()
                    .any(|e| e["event"] == "step_short_circuited")
            );
        }
    }

    #[test]
    fn unrelated_write_and_textual_final_cannot_discharge_placeholder() {
        for contract_enabled in [true, false] {
            let (_dir, mut cfg, _) = setup();
            if !contract_enabled {
                cfg.completion_contract_path = None;
            }
            let outcome = run(
                &cfg,
                options(ContractEnforcement::Observe),
                vec![
                    read_reply(PAGE),
                    write_reply("notes.txt", "unrelated"),
                    AssistantReply::text("done"),
                    write_reply(PAGE, IMPLEMENTED),
                ],
            )
            .unwrap();
            assert!(outcome.changed_paths.contains(&PAGE.to_string()));
            let events = event_values(cfg.eval_events_path.as_ref().unwrap());
            assert!(events.iter().any(
                |e| e["event"] == "step_completion_blocked" && e["write_or_edit_seen"] == true
            ));
            assert!(!events.iter().any(|e| e["event"] == "step_short_circuited"));
        }
    }

    #[test]
    fn read_only_placeholder_exhausts_without_success() {
        let (_dir, cfg, _) = setup();
        let result = run(
            &cfg,
            options(ContractEnforcement::Observe),
            (0..40).map(|_| read_reply(PAGE)).collect(),
        );
        assert!(result.is_err());
        assert!(
            crate::planner::profiles::nextjs::is_engine_owned_scaffold_page(
                PAGE,
                &std::fs::read_to_string(cfg.workspace_root.join(PAGE)).unwrap()
            )
        );
        assert!(
            !event_values(cfg.eval_events_path.as_ref().unwrap())
                .iter()
                .any(|e| e["event"] == "step_short_circuited")
        );
    }

    #[test]
    fn verified_existing_page_has_no_write_and_auditable_short_circuit() {
        let (_dir, cfg, _) = setup();
        std::fs::write(cfg.workspace_root.join(PAGE), IMPLEMENTED).unwrap();
        let outcome = run(
            &cfg,
            options(ContractEnforcement::Observe),
            vec![read_reply(PAGE)],
        )
        .unwrap();
        assert!(outcome.changed_paths.is_empty());
        assert_eq!(outcome.iterations, 1);
        assert_eq!(outcome.tool_calls, 1);
        let events = event_values(cfg.eval_events_path.as_ref().unwrap());
        let event = events
            .iter()
            .find(|e| e["event"] == "step_short_circuited")
            .unwrap();
        assert_eq!(event["at"], "iteration");
        assert_eq!(event["step_id"], "implement-page");
        assert_eq!(event["write_or_edit_seen"], false);
        assert_eq!(event["scaffold_placeholder_paths"], json!([]));
        assert_eq!(event["satisfaction_basis"], "verified_existing_artifacts");
        let hash = &event["expected_path_hashes"][0];
        assert_eq!(hash["path"], PAGE);
        assert_eq!(hash["before_sha256"], hash["after_sha256"]);
        assert_eq!(hash["after_sha256"].as_str().unwrap().len(), 64);
        assert_eq!(
            event["completion_requirements"]["required_obligations"],
            json!(["implementation"])
        );
    }

    #[test]
    fn existing_non_placeholder_with_failed_contract_cannot_short_circuit() {
        let (_dir, cfg, mut contract) = setup();
        std::fs::write(cfg.workspace_root.join(PAGE), IMPLEMENTED).unwrap();
        contract.verify_commands = vec!["test -f missing-proof.txt".to_string()];
        let options = options(ContractEnforcement::Observe);
        let paths = vec![PAGE.to_string()];
        let implementation = ImplementationCompletion::capture(&cfg, &options, &paths);
        let mut attempts = 0;
        let outcome = maybe_short_circuit_satisfied_step(
            &cfg,
            &options,
            "Implement",
            &paths,
            Some(&contract),
            ShortCircuitContext {
                verify_attempts: &mut attempts,
                at: StepShortCircuitAt::Iteration,
                write_or_edit_seen: false,
                implementation: &implementation,
                changed_paths: &[],
            },
        )
        .unwrap();
        assert!(outcome.is_none());
        assert_eq!(attempts, 0);
    }

    #[test]
    fn bash_page_change_is_preserved_without_write_edit_observation() {
        let (_dir, cfg, _) = setup();
        std::fs::write(cfg.workspace_root.join("replacement.tsx"), IMPLEMENTED).unwrap();
        let bash = AssistantReply {
            content: String::new(),
            tool_calls: vec![ToolCall::new(
                "Bash",
                json!({"command": "cp replacement.tsx src/app/page.tsx"}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        };
        let outcome = run(
            &cfg,
            options(ContractEnforcement::Observe),
            vec![read_reply(PAGE), bash, read_reply(PAGE)],
        )
        .unwrap();
        assert_eq!(outcome.changed_paths, vec![PAGE]);
        let events = event_values(cfg.eval_events_path.as_ref().unwrap());
        assert!(
            events.iter().any(|e| e["event"] == "tool_execute"
                && e["name"] == "Bash"
                && e["status"] == "ok")
        );
        assert!(
            !events
                .iter()
                .any(|e| e["event"] == "tool_execute"
                    && (e["name"] == "Write" || e["name"] == "Edit"))
        );
        if let Some(event) = events.iter().find(|e| e["event"] == "step_short_circuited") {
            assert_eq!(event["write_or_edit_seen"], false);
            assert_eq!(event["changed_paths"], json!([PAGE]));
            assert_eq!(event["satisfaction_basis"], "verified_changed_artifacts");
        }
    }

    #[test]
    fn recovery_and_synthetic_mutation_options_still_require_write() {
        let (_dir, cfg, contract) = setup();
        std::fs::write(cfg.workspace_root.join(PAGE), IMPLEMENTED).unwrap();
        for options in [
            RunSessionOptions::final_acceptance_repair(),
            options(ContractEnforcement::Observe).with_required_mutation_before_short_circuit(true),
        ] {
            let paths = vec![PAGE.to_string()];
            let implementation = ImplementationCompletion::capture(&cfg, &options, &paths);
            let mut attempts = 0;
            let outcome = maybe_short_circuit_satisfied_step(
                &cfg,
                &options,
                "Repair",
                &paths,
                Some(&contract),
                ShortCircuitContext {
                    verify_attempts: &mut attempts,
                    at: StepShortCircuitAt::Iteration,
                    write_or_edit_seen: false,
                    implementation: &implementation,
                    changed_paths: &[],
                },
            )
            .unwrap();
            assert!(outcome.is_none());
            assert_eq!(attempts, 0);
        }
    }
}
