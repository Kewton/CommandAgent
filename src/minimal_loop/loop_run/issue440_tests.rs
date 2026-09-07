#[cfg(test)]
mod issue440 {
    use super::super::max_length_guard::{MaxLengthGuard, REASON};
    use super::*;

    fn capped() -> AssistantReply {
        AssistantReply {
            completion_tokens: Some(8192),
            ..empty_reply()
        }
    }

    fn page() -> &'static str {
        include_str!("../../../tests/corpus/apps/issue440-max-length/fixtures/long-page.txt")
    }

    fn fixture() -> Vec<Value> {
        include_str!("../../../tests/corpus/apps/issue440-max-length/fixtures/i1-recovery.jsonl")
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    fn setup() -> (tempfile::TempDir, Config) {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = config(dir.path().to_path_buf());
        cfg.num_predict = 8192;
        cfg.max_iterations = 16;
        cfg.eval_events_path = Some(dir.path().join("events.jsonl"));
        std::fs::write(dir.path().join("page.tsx"), page()).unwrap();
        (dir, cfg)
    }

    fn run(cfg: &Config, replies: Vec<AssistantReply>, options: RunSessionOptions) -> String {
        let mut fake = RecordingFake::new(replies.into_iter().map(Ok).collect());
        let expected_requests = fake.replies.lock().unwrap().len();
        let error = run_session_with_outcome_with_options(
            &mut fake,
            &mut SessionSnapshot::new(),
            "Implement the inventory UI.",
            &[],
            cfg,
            &NOOP_UI,
            options,
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains(REASON), "{error}");
        assert_eq!(
            fake.requests().len(),
            expected_requests,
            "no extra request at limit"
        );
        error
    }

    fn assert_handoff(cfg: &Config, error: &str, count: usize) -> Value {
        let events = event_values(cfg.eval_events_path.as_ref().unwrap());
        let stop = events
            .iter()
            .find(|event| event["event"] == "loop_stop")
            .unwrap();
        assert_eq!(stop["reason"], REASON);
        assert_eq!(stop["max_length_no_tool_call_count"], count);
        assert_eq!(stop["recovery_yaml_missing"], false);
        assert!(error.contains("recovery prompt saved:"), "{error}");
        let saved = events
            .iter()
            .find(|event| event["event"] == "recovery_prompt_saved")
            .unwrap();
        assert_eq!(saved["failure_kind"], REASON);
        for field in ["recovery_prompt_path", "recovery_ultra_plan_path"] {
            let content =
                std::fs::read_to_string(cfg.workspace_root.join(saved[field].as_str().unwrap()))
                    .unwrap();
            assert!(content.contains(REASON), "{content}");
            assert!(
                content.contains(&format!("{count} output-limit responses")),
                "{content}"
            );
            assert!(content.contains("total_duration_ms="), "{content}");
            if field.ends_with("plan_path") {
                let _: crate::planner::ultra_plan::UltraPlan =
                    serde_yaml::from_str(&content).unwrap();
            }
        }
        stop.clone()
    }

    #[test]
    fn default_stops_after_second_reply_without_requesting_third() {
        let (_dir, cfg) = setup();
        let error = run(&cfg, vec![capped(), capped()], RunSessionOptions::default());
        let stop = assert_handoff(&cfg, &error, 2);
        assert_eq!(stop["max_length_no_tool_call_limit"], 2);
        let events = event_values(cfg.eval_events_path.as_ref().unwrap());
        let turns: Vec<_> = events
            .iter()
            .filter(|event| event["event"] == "provider_turn_duration")
            .collect();
        assert_eq!(turns.len(), 2);
        for turn in turns {
            assert_eq!(turn["tool_calls_in_response"], 0);
            assert_eq!(turn["write_or_edit_succeeded"], false);
            assert_eq!(turn["output_limit_reached"], true);
            assert_eq!(turn["finish_reason"], "stop");
            assert_eq!(turn["schema_version"], "1");
        }
    }

    #[test]
    fn i1_replay_keeps_read_only_interleaving_and_matching_duration() {
        for (limit, line, total) in [(2, 607, 262217), (3, 629, 385947)] {
            let (_dir, cfg) = setup();
            let mut guard = MaxLengthGuard::new(Some(limit));
            let mut stop_line = 0;
            let mut consumed = Vec::new();
            let source = fixture();
            for (index, event) in source.iter().enumerate() {
                if event["event"] != "provider_turn_duration" {
                    continue;
                }
                let mut reply = capped();
                reply.completion_tokens = event["eval_count"].as_u64();
                reply.tool_calls = source[index + 1..]
                    .iter()
                    .take_while(|event| event["event"] != "provider_turn_duration")
                    .filter(|event| event["event"] == "tool_call_raw")
                    .map(|event| {
                        ToolCall::new(event["name"].as_str().unwrap(), json!({"path":"page.tsx"}))
                    })
                    .collect();
                consumed.push(reply.clone());
                if guard.observe(
                    &reply,
                    cfg.num_predict,
                    Duration::from_millis(event["duration_ms"].as_u64().unwrap()),
                ) {
                    stop_line = event["source_event_line"].as_u64().unwrap();
                    break;
                }
            }
            assert_eq!(stop_line, line);
            let options = RunSessionOptions {
                max_length_no_tool_call_limit: Some(limit),
                step_id: Some("implement-main-ui".to_string()),
                ..RunSessionOptions::plan_step_with_enforcement(
                    RunSessionStepKind::Implement,
                    ContractEnforcement::Enforce,
                    Some("inspect-current-state".to_string()),
                )
            };
            let error = guard
                .stop(&cfg, &options, "Repair the inventory UI.", &[])
                .to_string();
            let stop = assert_handoff(&cfg, &error, limit);
            assert_eq!(stop["max_length_no_tool_call_duration_ms"], total);
            assert_eq!(stop["session_scope"], "plan-run-step");
            assert_eq!(stop["step_kind"], "implement");
            assert_eq!(stop["phase_scope"], "inspect-current-state");
            assert!(total < 900000);
            let profile =
                crate::time_profile::aggregate_event_path(cfg.eval_events_path.as_deref());
            assert_eq!(
                profile.other_ms, 0,
                "stop must not double count provider duration"
            );
            // The same interleaving runs through the real loop; fixture durations
            // above exercise accounting without waiting through provider delays.
            let (_run_dir, run_cfg) = setup();
            let error = run(&run_cfg, consumed, options);
            assert_handoff(&run_cfg, &error, limit);
        }
    }

    #[test]
    fn long_write_and_edit_reset_count_and_telemetry_tracks_each_response() {
        let (_dir, cfg) = setup();
        let mut write = capped();
        write.tool_calls = vec![ToolCall::new(
            "Write",
            json!({"path":"page.tsx","content":page()}),
        )];
        let mut edit = capped();
        edit.tool_calls = vec![ToolCall::new(
            "Edit",
            json!({"path":"page.tsx","old_string":"Inventory","new_string":"Stock"}),
        )];
        let error = run(
            &cfg,
            vec![capped(), write, capped(), edit, capped(), capped()],
            RunSessionOptions::default(),
        );
        assert_handoff(&cfg, &error, 2);
        let events = event_values(cfg.eval_events_path.as_ref().unwrap());
        let turns: Vec<_> = events
            .iter()
            .filter(|event| event["event"] == "provider_turn_duration")
            .collect();
        assert_eq!(turns.len(), 6);
        for (index, turn) in turns.iter().enumerate() {
            let success = index == 1 || index == 3;
            assert_eq!(turn["tool_calls_in_response"], usize::from(success));
            assert_eq!(turn["write_or_edit_succeeded"], success);
            assert_eq!(turn["output_limit_reached"], true);
        }
        assert!(
            std::fs::read_to_string(cfg.workspace_root.join("page.tsx"))
                .unwrap()
                .contains("Stock")
        );
    }

    #[test]
    fn failed_edit_short_reply_and_missing_usage_do_not_reset() {
        let (_dir, cfg) = setup();
        let mut failed = capped();
        failed.tool_calls = vec![ToolCall::new(
            "Edit",
            json!({"path":"page.tsx","old_string":"absent anchor","new_string":"Stock"}),
        )];
        let error = run(
            &cfg,
            vec![capped(), failed, empty_reply(), capped()],
            RunSessionOptions::default(),
        );
        assert_handoff(&cfg, &error, 2);
        let events = event_values(cfg.eval_events_path.as_ref().unwrap());
        for event in events
            .iter()
            .filter(|event| event["event"] == "provider_turn_duration")
        {
            assert_eq!(event["write_or_edit_succeeded"], false);
        }
        let mut guard = MaxLengthGuard::new(None);
        assert!(!guard.observe(&capped(), 8192, Duration::from_secs(120)));
        let short = AssistantReply {
            completion_tokens: Some(8191),
            ..empty_reply()
        };
        assert!(!guard.observe(&short, 8192, Duration::from_secs(3)));
        assert!(!guard.observe(&empty_reply(), 8192, Duration::from_secs(3)));
        assert!(!guard.observe(&capped(), 0, Duration::from_secs(3)));
        guard.write_or_edit_succeeded();
        assert!(!guard.observe(&capped(), 8192, Duration::from_secs(7)));
        assert!(guard.observe(&capped(), 8192, Duration::from_secs(11)));
        let error = guard
            .stop(&cfg, &RunSessionOptions::default(), "Repair UI", &[])
            .to_string();
        assert!(error.contains("total_duration_ms=18000"), "{error}");
    }
    #[test]
    fn long_normalized_text_write_is_allowed_and_counted() {
        let (_dir, mut cfg) = setup();
        cfg.tool_protocol = Some(crate::config::ToolProtocol::Text);
        let mut write = capped();
        write.content = format!(
            "<tool_call>{}</tool_call>",
            json!({
                "name":"Write", "arguments":{"path":"page.tsx", "content":page()}
            })
        );
        let error = run(
            &cfg,
            vec![capped(), write, capped(), capped()],
            RunSessionOptions::default(),
        );
        assert_handoff(&cfg, &error, 2);
        let events = event_values(cfg.eval_events_path.as_ref().unwrap());
        let turns: Vec<_> = events
            .iter()
            .filter(|event| event["event"] == "provider_turn_duration")
            .collect();
        assert_eq!(turns[1]["native_tools_enabled"], false);
        assert_eq!(turns[1]["tool_calls_in_response"], 1);
        assert_eq!(turns[1]["write_or_edit_succeeded"], true);
        assert_eq!(turns[1]["tools"], 0);
    }
}
