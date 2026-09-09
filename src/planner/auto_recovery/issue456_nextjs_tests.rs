#[cfg(test)]
mod tests {
    use super::super::*;

    #[test]
    fn issue456_original_nextjs_contract_reaches_repair_without_inspection_build_or_write() {
        let root = tempfile::tempdir().unwrap();
        let (mut config, mut handoff) = setup(root.path());
        copy_fixture_tree(&Path::new(FIXTURE).join("original"), root.path());
        let contract_bytes =
            std::fs::read(Path::new(FIXTURE).join("nextjs-contract.json")).unwrap();
        let contract: Value = serde_json::from_slice(&contract_bytes).unwrap();
        std::fs::write(
            config.completion_contract_path.as_ref().unwrap(),
            &contract_bytes,
        )
        .unwrap();
        config.profile = "nextjs".into();
        handoff.profile = "nextjs".into();
        handoff.original_goal = contract["goal"].as_str().unwrap().into();
        handoff.verify_commands = vec!["npm run build".into()];
        let candidate = captured(&config, "create", &handoff);
        let mut planner = Replay::new(vec![generated(vec![
            step("premature-repair", "implement", vec![]),
            step("premature-build", "verify", vec!["npm run build".into()]),
        ])]);
        let mut execution = Replay::default();
        let replies = execution.replies.clone();
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
        let bound = driver.transaction_config.as_ref().unwrap();
        let treatment_root = bound.workspace_root.clone();
        let context = recovery_inspection::load(bound).unwrap().unwrap();
        assert!(
            context.instruction.chars().count() <= 2500,
            "instruction length={}: {}",
            context.instruction.chars().count(),
            context.instruction
        );
        let calls = context
            .read_ranges
            .iter()
            .filter(|read| {
                [API, "src/lib/store.ts", "src/lib/types.ts"].contains(&read.path.as_str())
            })
            .map(|read| {
                ToolCall::new(
                    "Read",
                    json!({"path":read.path,"start_line":read.start_line,"end_line":read.end_line}),
                )
            })
            .collect();
        replies.lock().unwrap().extend([
        AssistantReply{ content:String::new(),tool_calls:calls,prompt_tokens:None,completion_tokens:None },
        AssistantReply::text("The assignee annotation makes role optional; Member requires role. Repair the API annotation next."),
    ]);
        let result = driver.execute(prepared);
        assert_eq!(
            crate::planner::recovery_snapshot::current_source_sha256(root.path()).unwrap(),
            crate::planner::recovery_snapshot::current_source_sha256(&treatment_root).unwrap()
        );
        // Deliberately stop planner replay at repair, before real npm/build work.
        assert!(result.result.is_err());
        let events = std::fs::read_to_string(config.eval_events_path.as_ref().unwrap()).unwrap();
        let events: Vec<Value> = events
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert!(
            events
                .iter()
                .any(|event| event["event"] == "ultra_phase_execute_complete"
                    && event["phase_id"] == "inspect-current-state"),
            "{:?}",
            result.result
        );
        assert!(
            events
                .iter()
                .any(|event| event["event"] == "ultra_phase_start"
                    && event["phase_id"] == "repair-unknown"),
            "{:?}",
            result.result
        );
        assert!(
            events
                .iter()
                .any(|event| event["event"] == "recovery_read_only_post_step_repair_suppressed")
        );
        for event in &events {
            assert!(
                !(event["event"] == "tool_execute"
                    && ["Bash", "Write", "Edit"].contains(&event["name"].as_str().unwrap_or(""))),
                "{event}"
            );
            assert_ne!(event["event"], "read_only_stagnation_feedback");
            assert_ne!(event["event"], "dependency_build_lifecycle");
        }
    }
}
