use super::*;

#[test]
fn issue465_tool_write_cannot_replace_host_record_and_repair_can_still_proceed() {
    let (_root, config) = setup();
    let record = std::fs::read(config.workspace_root.join(RECORD)).unwrap();
    let mut replay = Replay::new(vec![
        tool("Write", json!({"path":RECORD,"content":"{}"})),
        cat("cat src/api.js"),
        fix_store(),
        AssistantReply::text("Repaired"),
    ]);
    let result =
        crate::planner::run_step_plan_with_ui(&mut replay, &plan(), &config, &crate::tui::NOOP_UI);
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(
        std::fs::read(config.workspace_root.join(RECORD)).unwrap(),
        record
    );
    // Prove the actual Write was rejected. Prompt text is not tool evidence,
    // and a macOS /private/ workspace can accidentally match English keywords.
    let log = events(&config);
    let writes: Vec<_> = log
        .iter()
        .enumerate()
        .filter(|(_, e)| e["event"] == "tool_call_raw" && e["name"] == "Write")
        .collect();
    assert_eq!(writes.len(), 1);
    let hidden: Vec<_> = log
        .iter()
        .enumerate()
        .filter(|(_, e)| e["event"] == "hidden_path_feedback")
        .collect();
    assert_eq!(hidden.len(), 1);
    let (hidden_index, rejection) = hidden[0];
    assert_eq!(rejection["tool"], "Write");
    assert_eq!(rejection["path"], RECORD);
    assert_eq!(rejection["attempt"], 1);
    let denied_index = log
        .iter()
        .position(|e| {
            e["event"] == "tool_validation_error"
                && e["name"] == "Write"
                && e["error_kind"] == "workspace_policy_blocked"
                && e["repeat_count"] == 1
        })
        .expect("Write must return the workspace policy rejection");
    assert!(
        !log.iter().any(|e| {
            e["event"] == "tool_execute" && e["name"] == "Write" && e["status"] == "ok"
        })
    );
    let blocked_index = log
        .iter()
        .position(|e| e["reason"] == "recovery_repair_unresolved")
        .unwrap();
    let edit_index = log
        .iter()
        .position(|e| e["event"] == "tool_execute" && e["name"] == "Edit" && e["status"] == "ok")
        .expect("the later related-store repair must execute successfully");
    let resolved_index = log
        .iter()
        .position(|e| {
            e["event"] == "recovery_repair_obligation_observed" && e["status"] == "resolved"
        })
        .expect("fresh target confirmation must resolve the repair");
    assert!(writes[0].0 < hidden_index && hidden_index < denied_index);
    assert!(
        denied_index < blocked_index && blocked_index < edit_index && edit_index < resolved_index
    );
}

#[test]
fn issue465_bash_record_removal_cannot_turn_recovery_into_unbound_completion() {
    let (_root, config) = setup();
    let result = run(
        &config,
        options(&config, &plan(), 0),
        vec![
            cat(
                "python3 -c 'from pathlib import Path; Path(\".commandagent/recovery-runtime/repair-obligation.json\").unlink()'",
            ),
            cat("cat src/api.js"),
            AssistantReply::text("Completed"),
        ],
    );
    assert!(result.is_err(), "{result:?}");
    eprintln!(
        "ISSUE465_BASH_RECORD_REMOVAL record_exists={} completed=false",
        config.workspace_root.join(RECORD).exists()
    );
    let log = events(&config);
    assert!(!log.iter().any(|e| e["status"] == "resolved"));
    assert!(
        !log.iter()
            .any(|e| e["reason"] == "required_artifacts_satisfied_after_tool")
    );
    if !config.workspace_root.join(RECORD).exists() {
        let result = crate::planner::run_step_plan_with_ui(
            &mut Replay::default(),
            &plan(),
            &config,
            &crate::tui::NOOP_UI,
        );
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("host record missing")
        );
    }
}

#[test]
fn issue465_runner_handoff_rejects_deleted_replaced_and_foreign_attempt_records() {
    for mutation in ["deleted", "replaced", "foreign", "both-records-deleted"] {
        let (_root, config) = setup();
        let (_other_root, other) = setup();
        let opts = options(&config, &plan(), 0);
        let path = config.workspace_root.join(RECORD);
        let bytes = std::fs::read(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        match mutation {
            "replaced" => {
                let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                value["sources"] = json!({});
                std::fs::write(&path, value.to_string()).unwrap();
            }
            "foreign" => {
                std::fs::copy(other.workspace_root.join(RECORD), &path).unwrap();
            }
            "both-records-deleted" => {
                std::fs::remove_file(
                    config
                        .workspace_root
                        .join(".commandagent/recovery-runtime/inspection-context.json"),
                )
                .unwrap();
            }
            _ => {}
        }
        assert!(
            opts.recovery_obligation
                .as_ref()
                .unwrap()
                .feedback(&config, &opts)
                .is_some()
        );
        let result = crate::planner::run_step_plan_with_ui(
            &mut Replay::default(),
            &plan(),
            &config,
            &crate::tui::NOOP_UI,
        );
        assert!(result.is_err(), "{mutation}: {result:?}");
        let message = result.unwrap_err().to_string();
        assert!(
            message.contains("provenance") || message.contains("host record missing"),
            "{mutation}: {message}"
        );
        assert!(!events(&config).iter().any(|e| e["status"] == "resolved"));
    }
}

#[test]
fn issue465_captured_continuation_retains_original_obligation_with_new_attempt() {
    let (_root, config) = setup();
    let original = load(&config).unwrap().unwrap();
    let context = crate::planner::recovery_inspection::load(&config)
        .unwrap()
        .unwrap();
    let next_root = tempfile::tempdir().unwrap();
    for path in [
        API,
        STORE,
        "src/types.js",
        "checks/target.js",
        "contract.json",
        "package.json",
    ] {
        let destination = next_root.path().join(path);
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
        std::fs::copy(config.workspace_root.join(path), destination).unwrap();
    }
    let mut next = config.clone();
    next.workspace_root = next_root.path().to_path_buf();
    next.completion_contract_path = Some(next_root.path().join("contract.json"));
    next.eval_events_path = Some(next_root.path().join(".commandagent/events.jsonl"));
    let handoff = RecoveryHandoff {
        failure_evidence: vec!["later boundary still fails".into()],
        ..RecoveryHandoff::default()
    };
    // The retained control has no runtime record; only the captured host context
    // carries the original obligation into this newly isolated attempt.
    crate::planner::recovery_inspection::bind_context(
        &next,
        &next,
        Some("create"),
        &handoff,
        Some(&context),
    )
    .unwrap();
    let inherited = load(&next).unwrap().unwrap();
    assert_ne!(inherited.attempt_id, original.attempt_id);
    assert_eq!(inherited.sources, original.sources);
    assert!(inherited.diagnostics.contains(&original.diagnostics));
    assert!(inherited.diagnostics.contains("later boundary still fails"));
    let opts = options(&next, &plan(), 0);
    assert!(
        opts.recovery_obligation
            .as_ref()
            .unwrap()
            .feedback(&next, &opts)
            .is_some()
    );
}
