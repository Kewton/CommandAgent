#[cfg(test)]
mod tests {
    use super::super::*;

    #[test]
    fn issue456_python_and_rust_keep_related_reads_in_create_and_fix_recovery() {
        let cases: Vec<Value> = serde_json::from_slice(
            &std::fs::read(Path::new(FIXTURE).join("language-cases.json")).unwrap(),
        )
        .unwrap();
        for case in cases {
            for intent in ["create", "fix"] {
                run_language_recovery(&case, intent);
            }
        }
    }

    fn run_language_recovery(case: &Value, intent: &str) {
        let root = tempfile::tempdir().unwrap();
        let (mut config, mut handoff) = setup(root.path());
        let target = case["target"].as_str().unwrap();
        let dependency = case["dependency"].as_str().unwrap();
        for (path, source) in [
            (target, case["target_source"].as_str().unwrap()),
            (dependency, case["dependency_source"].as_str().unwrap()),
        ] {
            std::fs::create_dir_all(root.path().join(path).parent().unwrap()).unwrap();
            std::fs::write(root.path().join(path), source).unwrap();
        }
        // Completion contracts can also require non-text assets; suggestions
        // must not turn their presence into a treatment binding failure.
        std::fs::write(root.path().join("asset.bin"), [0, 255, 128]).unwrap();
        // Generic profile is valid for both languages; no JS/TS resolver can
        // enumerate these dependencies, and the contract only names the entry.
        let command = format!("test -f {target}");
        std::fs::write(
            config.completion_contract_path.as_ref().unwrap(),
            json!({"goal":"Recover the local helper call", "profile":"generic",
                "required_paths":[target, "asset.bin", "frozen/check.ts", "contract.json"], "verify_commands":[command],
                "protected_paths":["frozen/check.ts", "contract.json"]})
            .to_string(),
        )
        .unwrap();
        crate::minimal_loop::completion::CompletionContract::load_for_config(&config).unwrap();
        config.max_iterations = 4;
        handoff.original_goal = "Recover the local helper call".into();
        handoff.repair_targets = vec![target.into()];
        handoff.failure_evidence = vec![format!("{target}: inspect the helper return contract")];
        handoff.verify_commands = vec![command];
        let candidate = captured(&config, intent, &handoff);
        let mut proposed = step("premature-edit", "implement", vec![]);
        proposed.expected_paths = vec![target.into()];
        let mut planner = Replay::new(vec![generated(vec![proposed])]);
        let outside = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(outside.path(), "outside_read_must_not_leak").unwrap();
        let mut execution = Replay::new(vec![
            AssistantReply {
                content: String::new(),
                tool_calls: vec![
                    ToolCall::new("Read", json!({"path":target})),
                    ToolCall::new("Read", json!({"path":dependency})),
                    ToolCall::new("Write", json!({"path":dependency,"content":"changed"})),
                ],
                prompt_tokens: None,
                completion_tokens: None,
            },
            AssistantReply::text(
                "The local helper definition is available. Repair the caller in the next phase.",
            ),
        ]);
        let requests = execution.requests.clone();
        let mut driver = RunnerRecoveryDriver {
            planner: &mut planner,
            execution: &mut execution,
            config: &config,
            ui: &crate::tui::NOOP_UI,
            transaction_snapshot: None,
            transaction_treatment: None,
            transaction_config: None,
            transaction_observer_identity: None,
        };
        let prepared = driver.prepare(&candidate).unwrap();
        driver.start(1, &candidate, &prepared).unwrap();
        let bound = driver.transaction_config.as_ref().unwrap().clone();
        let context = recovery_inspection::load(&bound).unwrap().unwrap();
        assert!(!context.scoped_reads, "{} {intent}", case["language"]);
        assert!(!context.read_paths.contains(&dependency.to_string()));
        assert!(context.instruction.contains("not an exhaustive list"));
        assert_eq!(
            recovery_inspection::tool_rejection(
                &bound,
                Some(recovery_inspection::PHASE),
                &ToolCall::new("Read", json!({"path":dependency}))
            ),
            None
        );
        std::fs::write(
            bound.workspace_root.join(".commandagent/private.txt"),
            "private_read_must_not_leak",
        )
        .unwrap();
        // Existing protected-path enforcement also remains in the writable
        // repair phase, independently of the new inspection allowlist.
        let tool_context = crate::tools::registry::ToolContext {
            root: bound.workspace_root.clone(),
            mode: crate::mode::ExecutionMode::Act,
            auto_approve: true,
            interactive_approval: false,
            offline: true,
            workspace_policy: crate::tools::workspace_policy::WorkspacePolicy::NormalTask,
            eval_events_path: None,
            expected_paths: vec![],
            protected_paths: vec!["frozen/check.ts".into()],
        };
        let registry = crate::tools::registry::ToolRegistry::default();
        // These existing guards fail the step, so check them separately from
        // the successful inspection replay, through the same tool dispatcher.
        for path in [
            outside.path().to_str().unwrap(),
            ".commandagent/private.txt",
        ] {
            assert!(
                registry
                    .execute("Read", &json!({"path":path}), &tool_context)
                    .is_err()
            );
        }
        assert!(
            crate::tools::registry::ToolRegistry::default()
                .execute(
                    "Edit",
                    &json!({"path":"frozen/check.ts","old_string":"frozen","new_string":"changed"}),
                    &tool_context,
                )
                .is_err()
        );
        let result = driver.execute(prepared);
        assert!(result.result.is_err()); // Stop replay at the repair planner.
        let events: Vec<Value> = std::fs::read_to_string(config.eval_events_path.as_ref().unwrap())
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        for (event, phase) in [
            ("ultra_phase_execute_complete", "inspect-current-state"),
            ("ultra_phase_start", "repair-unknown"),
        ] {
            assert!(
                events
                    .iter()
                    .any(|item| item["event"] == event && item["phase_id"] == phase),
                "{} {intent}: {:?}",
                case["language"],
                result.result
            );
        }
        let requests = requests.lock().unwrap();
        let results = requests
            .iter()
            .flatten()
            .filter(|message| message.role == "tool")
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            results.contains(case["definition"].as_str().unwrap()),
            "{results}"
        );
        assert!(!results.contains("outside_read_must_not_leak"));
        assert!(!results.contains("private_read_must_not_leak"));
        assert!(
            events
                .iter()
                .any(|item| item["event"] == "inspect_mutation_tool_rejected")
        );
        assert_eq!(
            std::fs::read_to_string(bound.workspace_root.join(dependency)).unwrap(),
            case["dependency_source"].as_str().unwrap()
        );
        assert_eq!(
            std::fs::read_to_string(bound.workspace_root.join("frozen/check.ts")).unwrap(),
            "frozen\n"
        );
        // A fresh recover capture must not silently upgrade the next candidate
        // to the JS/TS allowlist, even if its next diagnostic names a TS file.
        let mut continued = handoff.clone();
        continued.repair_targets = vec![API.into()];
        let next = captured(&bound, "recover", &continued);
        let next_config = start_transaction(&config, &next, 2);
        assert!(
            !recovery_inspection::load(&next_config)
                .unwrap()
                .unwrap()
                .scoped_reads
        );
    }
}
