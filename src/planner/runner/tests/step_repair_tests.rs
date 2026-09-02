fn nextjs_interactive_app_tool_calls(page: &str) -> Vec<crate::state::ToolCall> {
    vec![
        crate::state::ToolCall::new(
            "Write",
            serde_json::json!({"path":"package.json","content":nextjs_complete_package_json()}),
        ),
        crate::state::ToolCall::new(
            "Write",
            serde_json::json!({"path":"tsconfig.json","content":nextjs_tsconfig_json()}),
        ),
        crate::state::ToolCall::new(
            "Write",
            serde_json::json!({"path":"postcss.config.js","content":nextjs_postcss_config()}),
        ),
        crate::state::ToolCall::new(
            "Write",
            serde_json::json!({"path":"tailwind.config.ts","content":nextjs_tailwind_config_ts()}),
        ),
        crate::state::ToolCall::new(
            "Write",
            serde_json::json!({"path":"src/app/page.tsx","content":page}),
        ),
        crate::state::ToolCall::new(
            "Write",
            serde_json::json!({"path":"src/app/layout.tsx","content":nextjs_layout_source()}),
        ),
        crate::state::ToolCall::new(
            "Write",
            serde_json::json!({"path":"src/app/globals.css","content":nextjs_globals_css()}),
        ),
        crate::state::ToolCall::new(
            "Write",
            serde_json::json!({"path":"src/app/global.d.ts","content":"declare module \"*.css\";"}),
        ),
    ]
}

#[test]
fn plan_run_emits_dependency_build_lifecycle_event() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src/app")).unwrap();
    std::fs::write(
        dir.path().join("src/app/page.tsx"),
        "export default function Page(){return <main/>;}",
    )
    .unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events.clone());
    let package = r#"{"scripts":{"build":"next build"},"dependencies":{"next":"^14.2.0","react":"^18.3.0","react-dom":"^18.3.0"}}"#;
    let plan = StepPlan {
        goal: "Create a Next.js app".to_string(),
        steps: vec![PlanStep {
            id: "app".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Create package.json and src/app/page.tsx then verify build"
                .to_string(),
            expected_paths: vec!["package.json".to_string(), "src/app/page.tsx".to_string()],
            verify: vec!["npm run build".to_string()],
        }],
    };
    let mut fake = FakeClient::new(vec![AssistantReply {
        content: String::new(),
        tool_calls: vec![
            crate::state::ToolCall::new(
                "Write",
                serde_json::json!({"path":"package.json","content":package}),
            ),
            crate::state::ToolCall::new(
                "Write",
                serde_json::json!({"path":"src/app/page.tsx","content":"export default function Page(){return <main/>;}"}),
            ),
        ],
        prompt_tokens: None,
        completion_tokens: None,
    }]);
    let err = run_step_plan(&mut fake, &plan, &cfg)
        .unwrap_err()
        .to_string();
    assert!(!err.is_empty());
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("\"event\":\"dependency_build_lifecycle\""));
    assert!(event_text.contains("\"mode\":\"plan-run\""));
    assert!(event_text.contains("setup_blocked"));
    assert!(event_text.contains("verification_dependency_missing"));
}

#[test]
fn plan_run_external_completion_contract_checked_at_plan_level() {
    let dir = tempfile::tempdir().unwrap();
    let contract = dir.path().join("contract.json");
    std::fs::write(
        &contract,
        r#"{"required_paths":[],"verify_commands":["test -f missing.txt"]}"#,
    )
    .unwrap();
    let mut cfg = config(dir.path().to_path_buf());
    cfg.completion_contract_path = Some(contract);
    let plan = StepPlan::single("Inspect workspace");
    let mut fake = FakeClient::new(vec![]);
    let err = run_step_plan(&mut fake, &plan, &cfg)
        .unwrap_err()
        .to_string();
    assert!(err.contains("plan final contract failed"));
}

#[test]
fn plan_run_external_completion_contract_checks_required_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let contract = dir.path().join("contract.json");
    std::fs::write(
        &contract,
        r#"{"required_paths":["date-helper.js"],"verify_commands":["node date-helper.js"],"required_capabilities":["implementation","deterministic_test"]}"#,
    )
    .unwrap();
    let mut cfg = config(dir.path().to_path_buf());
    cfg.completion_contract_path = Some(contract);
    cfg.eval_events_path = Some(events.clone());
    let plan = StepPlan {
        goal: "Create date helper".to_string(),
        steps: vec![PlanStep {
            id: "code".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Create date-helper.js".to_string(),
            expected_paths: vec!["date-helper.js".to_string()],
            verify: Vec::new(),
        }],
    };
    let mut fake = FakeClient::new(vec![
        AssistantReply {
            content: String::new(),
            tool_calls: vec![crate::state::ToolCall::new(
                "Write",
                serde_json::json!({"path":"date-helper.js","content":"exports.formatDate = (d) => String(d);"}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        },
        AssistantReply::text("done"),
    ]);
    let err = run_step_plan(&mut fake, &plan, &cfg)
        .unwrap_err()
        .to_string();
    assert!(err.contains("completion contract verify"), "{err}");
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("\"event\":\"step_obligation_scope\""));
    assert!(event_text.contains("\"step_kind\":\"implement\""));
    assert!(event_text.contains("\"completion_contract_path_merge_enabled\":true"));
    assert!(event_text.contains("\"completion_contract_verification_enabled\":true"));
    assert!(event_text.contains("\"event\":\"completion_verify\""));
    assert!(event_text.contains("\"missing_evidence\""));
    assert!(event_text.contains("\"test_artifact\""));
    assert!(event_text.contains("\"bound_verify_command\""));
}

#[test]
fn step_repair_missing_entrypoint_followthrough_creates_expected_artifact() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events.clone());
    let plan = StepPlan {
        goal: "Create app entrypoint\n\nRequired final artifacts:\n- src/app/page.tsx"
            .to_string(),
        steps: vec![PlanStep {
            id: "entrypoint".to_string(),
            kind: "verify".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Verify and repair src/app/page.tsx if missing".to_string(),
            expected_paths: Vec::new(),
            verify: vec!["test -f src/app/page.tsx".to_string()],
        }],
    };
    let mut fake = FakeClient::new(vec![
        AssistantReply {
            content: String::new(),
            tool_calls: vec![crate::state::ToolCall::new(
                "Bash",
                serde_json::json!({"command":"true"}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        },
        AssistantReply::text("initial incomplete"),
        AssistantReply {
            content: String::new(),
            tool_calls: vec![crate::state::ToolCall::new(
                "Write",
                serde_json::json!({"path":"src/app/page.tsx","content":"export default function Page(){return <main>ok</main>; }"}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        },
        AssistantReply::text("repair done"),
    ]);
    let result = run_step_plan(&mut fake, &plan, &cfg).unwrap();
    assert_eq!(result, "plan-run complete: 1 steps");
    assert!(dir.path().join("src/app/page.tsx").is_file());
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("\"event\":\"step_verify_repair\""));
    assert!(event_text.contains("\"previous_repair_target\":\"missing_entrypoint\""));
    assert!(event_text.contains("\"repair_follow_through\":\"target_matched\""));
    assert!(event_text.contains("\"repair_target_followed\":true"));
    assert!(event_text.contains("\"changed_paths_before\""));
    assert!(event_text.contains("\"changed_paths_after\""));
    assert!(event_text.contains("\"repair_turn_changed_paths\":[\"src/app/page.tsx\"]"));
    assert!(event_text.contains("\"allowed_action\":\"create_missing_entrypoint_artifact\""));
}

#[test]
fn step_repair_no_change_stops_on_progress_unchanged_and_handoff_saved() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events.clone());
    let plan = StepPlan {
        goal: "Create app entrypoint\n\nRequired final artifacts:\n- src/app/page.tsx"
            .to_string(),
        steps: vec![PlanStep {
            id: "entrypoint".to_string(),
            kind: "verify".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Verify and repair src/app/page.tsx if missing".to_string(),
            expected_paths: Vec::new(),
            verify: vec!["test -f src/app/page.tsx".to_string()],
        }],
    };
    let mut fake = FakeClient::new(vec![
        AssistantReply {
            content: String::new(),
            tool_calls: vec![crate::state::ToolCall::new(
                "Bash",
                serde_json::json!({"command":"true"}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        },
        AssistantReply::text("initial incomplete"),
        AssistantReply {
            content: String::new(),
            tool_calls: vec![crate::state::ToolCall::new(
                "Bash",
                serde_json::json!({"command":"true"}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        },
        AssistantReply::text("still incomplete"),
        AssistantReply {
            content: String::new(),
            tool_calls: vec![crate::state::ToolCall::new(
                "Bash",
                serde_json::json!({"command":"true"}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        },
        AssistantReply::text("still incomplete again"),
        AssistantReply {
            content: String::new(),
            tool_calls: vec![crate::state::ToolCall::new(
                "Bash",
                serde_json::json!({"command":"true"}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        },
        AssistantReply::text("still incomplete third"),
        AssistantReply {
            content: String::new(),
            tool_calls: vec![crate::state::ToolCall::new(
                "Bash",
                serde_json::json!({"command":"true"}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        },
        AssistantReply::text("still incomplete fourth"),
    ]);
    let err = run_step_plan(&mut fake, &plan, &cfg)
        .unwrap_err()
        .to_string();
    assert!(err.contains("repair prompt saved"), "{err}");
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("\"repair_follow_through\":\"no_change\""));
    assert!(event_text.contains("\"reason\":\"verify_repair_progress_unchanged\""));
    assert!(event_text.contains("\"event\":\"recovery_prompt_saved\""));
    assert!(event_text.contains("\"failure_kind\":\"verify_repair_progress_unchanged\""));
    assert!(event_text.contains("\"recovery_ultra_plan_path\""));
    assert!(event_text.contains("\"suggested_recovery_yaml_command\""));
    let recovery_plan = assert_single_recovery_ultra_plan(dir.path());
    assert!(recovery_plan.goal.contains("Create app entrypoint"));
    assert!(
        recovery_plan
            .phases
            .iter()
            .any(|phase| phase.prompt.contains("verify_repair_progress_unchanged"))
    );
    let repair_dir = dir.path().join(".commandagent/repairs");
    assert!(repair_dir.is_dir());
    assert!(std::fs::read_dir(repair_dir).unwrap().any(|entry| {
        entry
            .unwrap()
            .path()
            .extension()
            .is_some_and(|ext| ext == "md")
    }));
}

#[test]
fn dependency_repair_without_setup_authority_saves_unreachable_handoff_without_repair_turn() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events.clone());
    let plan = StepPlan {
        goal: "Verify dependency resolution".to_string(),
        steps: vec![PlanStep {
            id: "dependency-probe".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Create a deterministic dependency probe script".to_string(),
            expected_paths: vec!["missing-module.sh".to_string()],
            verify: vec!["sh missing-module.sh".to_string()],
        }],
    };
    let mut fake = FakeClient::new(vec![AssistantReply {
        content: String::new(),
        tool_calls: vec![crate::state::ToolCall::new(
            "Write",
            serde_json::json!({
                "path":"missing-module.sh",
                "content":"echo \"Cannot find module 'next/package.json'\" >&2\nexit 1\n"
            }),
        )],
        prompt_tokens: None,
        completion_tokens: None,
    }]);

    let err = run_step_plan(&mut fake, &plan, &cfg)
        .unwrap_err()
        .to_string();

    assert!(err.contains("dependency_setup_authority_required"), "{err}");
    assert_eq!(fake.messages().len(), 1);
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("\"event\":\"repair_unreachable\""));
    assert!(event_text.contains("\"reason\":\"dependency_setup_authority_required\""));
    assert!(
        event_text.contains("\"repair_attempts\":0")
            || !event_text.contains("step_verify_repair")
    );
    let repair_dir = dir.path().join(".commandagent/repairs");
    let prompt = std::fs::read_dir(repair_dir)
        .unwrap()
        .flatten()
        .filter_map(|entry| std::fs::read_to_string(entry.path()).ok())
        .find(|text| {
            text.contains("requires a Setup-authority step running dependency install")
        })
        .unwrap_or_default();
    assert!(
        prompt.contains("requires a Setup-authority step running dependency install"),
        "{prompt}"
    );
}

#[test]
fn step_repair_target_not_followed_streak_continues_while_report_improves() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events.clone());
    let plan = StepPlan {
        goal: "Create app entrypoint and supporting files\n\nRequired final artifacts:\n- src/app/page.tsx\n- src/app/widget.tsx\n- src/app/sidebar.tsx"
            .to_string(),
        steps: vec![PlanStep {
            id: "entrypoint".to_string(),
            kind: "verify".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Verify and repair the app entrypoint if missing".to_string(),
            expected_paths: Vec::new(),
            verify: vec![
                "test -f src/app/page.tsx".to_string(),
                "test -f src/app/widget.tsx".to_string(),
                "test -f src/app/sidebar.tsx".to_string(),
            ],
        }],
    };
    let mut fake = FakeClient::new(vec![
        AssistantReply {
            content: String::new(),
            tool_calls: vec![crate::state::ToolCall::new(
                "Bash",
                serde_json::json!({"command":"true"}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        },
        AssistantReply::text("initial incomplete"),
        AssistantReply {
            content: String::new(),
            tool_calls: vec![crate::state::ToolCall::new(
                "Write",
                serde_json::json!({"path":"src/app/widget.tsx","content":"export function Widget(){return null;}"}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        },
        AssistantReply::text("repair one done"),
        AssistantReply {
            content: String::new(),
            tool_calls: vec![crate::state::ToolCall::new(
                "Write",
                serde_json::json!({"path":"src/app/sidebar.tsx","content":"export function Sidebar(){return null;}"}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        },
        AssistantReply::text("repair two done"),
        AssistantReply {
            content: String::new(),
            tool_calls: vec![crate::state::ToolCall::new(
                "Write",
                serde_json::json!({"path":"src/app/page.tsx","content":"export default function Page(){return <main>ok</main>;}"}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        },
        AssistantReply::text("repair three done"),
    ]);
    let result = run_step_plan(&mut fake, &plan, &cfg).unwrap();
    assert_eq!(result, "plan-run complete: 1 steps");
    assert!(dir.path().join("src/app/page.tsx").exists());
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("\"repair_follow_through\":\"target_not_followed\""));
    assert!(event_text.contains("\"target_not_followed_repairs\":2"));
    assert!(event_text.contains("\"failure_kind\":\"repair_target_not_followed\""));
    assert!(!event_text.contains("\"reason\":\"repair_target_not_followed\""));
}

#[test]
fn step_repair_unrelated_change_is_telemetry_and_handoff_saved() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events.clone());
    let plan = StepPlan {
        goal: "Create app entrypoint\n\nRequired final artifacts:\n- src/app/page.tsx"
            .to_string(),
        steps: vec![PlanStep {
            id: "entrypoint".to_string(),
            kind: "verify".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Verify and repair src/app/page.tsx if missing".to_string(),
            expected_paths: Vec::new(),
            verify: vec!["test -f src/app/page.tsx".to_string()],
        }],
    };
    let mut fake = FakeClient::new(vec![
        AssistantReply {
            content: String::new(),
            tool_calls: vec![crate::state::ToolCall::new(
                "Bash",
                serde_json::json!({"command":"true"}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        },
        AssistantReply::text("initial incomplete"),
        AssistantReply {
            content: String::new(),
            tool_calls: vec![crate::state::ToolCall::new(
                "Write",
                serde_json::json!({"path":"README.md","content":"not the app entrypoint"}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        },
        AssistantReply::text("repair one done"),
        AssistantReply {
            content: String::new(),
            tool_calls: vec![crate::state::ToolCall::new(
                "Write",
                serde_json::json!({"path":"docs/notes.md","content":"still not the app entrypoint"}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        },
        AssistantReply::text("repair two done"),
        AssistantReply {
            content: String::new(),
            tool_calls: vec![crate::state::ToolCall::new(
                "Write",
                serde_json::json!({"path":"docs/notes-2.md","content":"still unrelated"}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        },
        AssistantReply::text("repair three done"),
        AssistantReply {
            content: String::new(),
            tool_calls: vec![crate::state::ToolCall::new(
                "Write",
                serde_json::json!({"path":"docs/notes-3.md","content":"still unrelated"}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        },
        AssistantReply::text("repair four done"),
    ]);
    let err = run_step_plan(&mut fake, &plan, &cfg)
        .unwrap_err()
        .to_string();
    assert!(err.contains("repair prompt saved"), "{err}");
    assert!(!dir.path().join("src/app/page.tsx").exists());
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("\"repair_follow_through\":\"unrelated_change\""));
    assert!(event_text.contains("\"reason\":\"bounded_repair_exhausted\""));
    assert!(event_text.contains("\"event\":\"recovery_prompt_saved\""));
    assert!(event_text.contains("\"failure_kind\":\"repair_unrelated_change\""));
    assert!(event_text.contains("\"failure_kind\":\"bounded_repair_exhausted\""));
    assert!(event_text.contains("\"event\":\"plan_step_started\""));
    assert!(event_text.contains("\"event\":\"plan_step_failed\""));
    assert!(event_text.contains("\"outcome\":\"bounded_repair_failed\""));
    assert!(event_text.contains("\"verification_status\":\"failed\""));
}

#[test]
fn run_plan_file_uses_same_step_runtime_options() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events.clone());
    let plan = StepPlan {
        goal: "Inspect workspace\n\nRequired final artifacts:\n- README.md".to_string(),
        steps: vec![PlanStep {
            id: "inspect".to_string(),
            kind: "inspect".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Inspect workspace".to_string(),
            expected_paths: Vec::new(),
            verify: Vec::new(),
        }],
    };
    let path = save_step_plan(dir.path(), &plan).unwrap();
    let mut fake = FakeClient::new(vec![
        AssistantReply {
            content: String::new(),
            tool_calls: vec![crate::state::ToolCall::new(
                "Bash",
                serde_json::json!({"command":"true"}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        },
        AssistantReply::text("inspected"),
    ]);
    let err = run_plan_file(&mut fake, &path, &cfg)
        .unwrap_err()
        .to_string();
    assert!(err.contains("plan final contract failed"));
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("\"session_scope\":\"plan-run-step\""));
    assert!(event_text.contains("\"prompt_extracted_paths_enabled\":false"));
    assert!(event_text.contains("\"completion_contract_verification_enabled\":false"));
}

#[test]
fn step_loop_uses_step_iteration_cap() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(dir.path().to_path_buf());
    cfg.max_iterations = 20;
    let plan = StepPlan {
        goal: "goal".to_string(),
        steps: vec![PlanStep {
            id: "s1".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Create missing file".to_string(),
            expected_paths: vec!["missing.txt".to_string()],
            verify: Vec::new(),
        }],
    };
    let replies = (0..20)
        .map(|_| AssistantReply {
            content: String::new(),
            tool_calls: vec![crate::state::ToolCall::new(
                "Bash",
                serde_json::json!({"command":"true"}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        })
        .collect();
    let mut fake = FakeClient::new(replies);
    let err = run_step_plan(&mut fake, &plan, &cfg)
        .unwrap_err()
        .to_string();
    assert!(err.contains("artifact_follow_through_exhausted"), "{err}");
    assert!(err.contains("missing.txt"), "{err}");
}

#[test]
fn repair_loop_uses_repair_iteration_cap() {
    let mut cfg = config(PathBuf::from("/tmp/work"));
    cfg.max_iterations = 20;
    assert_eq!(
        capped_config(&cfg, STEP_REPAIR_MAX_ITERATIONS).max_iterations,
        6
    );
}
