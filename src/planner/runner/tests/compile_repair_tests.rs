#[test]
#[cfg(unix)]
fn compile_error_final_acceptance_skips_readiness_probe() {
    let _probe_guard = dev_server_probe_test_guard();
    let dir = tempfile::tempdir().unwrap();
    let port = free_local_port();
    let events = dir.path().join("events.jsonl");
    enable_dev_server_probe_test_override(dir.path());
    let contract = write_compile_error_nextjs_workspace(dir.path(), port);
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    cfg.completion_contract_path = Some(contract);
    let plan = UltraPlan {
        goal: explicit_port_goal("Create an interactive browser app", port),
        profile: "nextjs".to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: vec![
            UltraPhase {
                id: "inspect-one".to_string(),
                prompt: "Inspect the existing app.".to_string(),
            },
            UltraPhase {
                id: "inspect-two".to_string(),
                prompt: "Inspect final readiness.".to_string(),
            },
        ],
    };

    let report = ultra_final_acceptance_report(&plan, &cfg).unwrap();

    assert_eq!(
        classify_repair_target(&report),
        RepairTarget::Implementation
    );
    assert_eq!(report.compile_errors.len(), 1, "{report:?}");
    assert!(report.dependency_missing.is_empty(), "{report:?}");
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("implementation_compile_error"));
    assert!(event_text.contains("\"compile_errors\""));
    assert!(!event_text.contains("\"event\":\"browser_probe\""));
    assert!(!event_text.contains("\"event\":\"dev_server_lifecycle\""));
    assert!(!dir.path().join("browser-readiness.json").exists());
}

#[test]
#[cfg(unix)]
fn compile_error_repair_prompt_anchors_file_and_then_runs_readiness() {
    let _probe_guard = dev_server_probe_test_guard();
    let dir = tempfile::tempdir().unwrap();
    let port = free_local_port();
    let events = dir.path().join("events.jsonl");
    enable_dev_server_probe_test_override(dir.path());
    let contract = write_compile_error_nextjs_workspace(dir.path(), port);
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    cfg.completion_contract_path = Some(contract);
    let plan = UltraPlan {
        goal: explicit_port_goal("Create an interactive browser app", port),
        profile: "nextjs".to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: vec![
            UltraPhase {
                id: "inspect-one".to_string(),
                prompt: "Inspect the existing app.".to_string(),
            },
            UltraPhase {
                id: "inspect-two".to_string(),
                prompt: "Inspect final readiness.".to_string(),
            },
        ],
    };
    let fixed_component = r#""use client";
	import { useState } from "react";
	export function SpaceInvaders(){
	  const [score, setScore] = useState(0);
	  const fire = () => setScore((value) => value + 1);
	  const reset = () => setScore(0);
	  return <main><button onClick={() => fire()}>Fire</button><button onClick={() => reset()}>Restart</button><canvas /><p>score {score}</p></main>;
    }
    "#;
    let route_page = "import { SpaceInvaders } from '@/components/SpaceInvaders';\nexport default function Page(){return <SpaceInvaders />;}\n";
    let initial_report = ultra_final_acceptance_report(&plan, &cfg).unwrap();
    assert_eq!(
        classify_repair_target(&initial_report),
        RepairTarget::Implementation,
        "{initial_report:?}"
    );
    let expected_paths = final_acceptance_repair_expected_paths(&plan, &cfg, &initial_report)
        .expect("expected paths");
    let repair_prompt = final_acceptance_repair_prompt(
        &cfg.workspace_root,
        PromptLayout::Stable,
        &plan,
        &initial_report,
        &UltraRunContext::default(),
        RepairTarget::Implementation.as_str(),
        &expected_paths,
        &[],
        (1, FINAL_ACCEPTANCE_REPAIR_MAX_ATTEMPTS),
        false,
        false,
    );
    let mut execution = FakeClient::new(vec![
        AssistantReply {
            content: String::new(),
            tool_calls: vec![
                crate::state::ToolCall::new(
                    "Write",
                    serde_json::json!({"path":"src/components/SpaceInvaders.tsx","content":fixed_component}),
                ),
                crate::state::ToolCall::new(
                    "Write",
                    serde_json::json!({"path":"src/app/page.tsx","content":route_page}),
                ),
            ],
            prompt_tokens: None,
            completion_tokens: None,
        },
        AssistantReply::text("compile error repaired"),
        AssistantReply::text("compile error repaired"),
        AssistantReply::text("compile error repaired"),
        AssistantReply::text("compile error repaired"),
        AssistantReply::text("compile error repaired"),
    ]);
    let mut session = SessionSnapshot::new();

    let outcome = run_final_acceptance_repair_with_ultra_session(
        &mut execution,
        &mut session,
        &repair_prompt,
        &expected_paths,
        &cfg,
        &NOOP_UI,
    )
    .unwrap_or_else(|err| {
        let event_text = std::fs::read_to_string(&events).unwrap_or_default();
        panic!("{err}\nEvents:\n{event_text}");
    });
    assert!(
        outcome
            .changed_paths
            .contains(&"src/components/SpaceInvaders.tsx".to_string())
    );
    let repaired_report = ultra_final_acceptance_report(&plan, &cfg).unwrap();

    assert!(
        repaired_report.compile_errors.is_empty(),
        "{repaired_report:?}"
    );
    assert!(
        repair_prompt.contains("src/components/SpaceInvaders.tsx:137:28"),
        "{repair_prompt}"
    );
    assert!(
        repair_prompt.contains("Compile repair edit mandate"),
        "{repair_prompt}"
    );
    assert!(
        repair_prompt.contains("You MUST modify src/components/SpaceInvaders.tsx"),
        "{repair_prompt}"
    );
    assert!(repair_prompt.contains("define reset"), "{repair_prompt}");
    assert!(
        repair_prompt.contains("replace the reference with an existing handler"),
        "{repair_prompt}"
    );
    assert!(
        repair_prompt.contains("remove the dead code"),
        "{repair_prompt}"
    );
    assert!(
        repair_prompt.contains("the file is not imported by any route"),
        "{repair_prompt}"
    );
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(
        event_text.contains("\"event\":\"completion_verify\""),
        "{event_text}"
    );
    assert!(event_text.contains("\"ok\":true"), "{event_text}");
    assert!(
        event_text.contains("\"event\":\"browser_probe\""),
        "{event_text}"
    );
    assert!(
        event_text.contains("\"browser_readiness_status\":\"passed\""),
        "{event_text}"
    );
}

#[test]
#[cfg(unix)]
fn step_verify_api_mismatch_prompt_and_poll_fix_passes_build() {
    let dir = tempfile::tempdir().unwrap();
    let port = 3011;
    let events = dir.path().join(".anvil/runs/api-mismatch/events.jsonl");
    write_api_mismatch_build_shim(dir.path());
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    let mut execution = FakeClient::new(vec![
        api_mismatch_initial_reply(port),
        api_mismatch_poll_fix_reply(),
    ]);

    let result =
        run_step_plan(&mut execution, &api_mismatch_step_plan(), &cfg).unwrap_or_else(|err| {
            let event_text = std::fs::read_to_string(&events).unwrap_or_default();
            panic!("{err}\nEvents:\n{event_text}");
        });

    assert!(result.contains("plan-run complete"), "{result}");
    let repair_prompt = execution
        .messages()
        .iter()
        .map(|messages| {
            messages
                .iter()
                .map(|message| message.content.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .find(|text| text.contains("Property 'onStateChange'"))
        .expect("repair prompt");
    assert!(
        repair_prompt.contains(
            "Imported definition context for `SpaceInvadersEngine` from src/lib/game-engine.ts:"
        ),
        "{repair_prompt}"
    );
    assert!(
        repair_prompt.contains("Public API surface for `SpaceInvadersEngine`"),
        "{repair_prompt}"
    );
    assert!(repair_prompt.contains("public start();"), "{repair_prompt}");
    assert!(repair_prompt.contains("public pause();"), "{repair_prompt}");
    assert!(
        repair_prompt.contains("public getState(): GameState;"),
        "{repair_prompt}"
    );
    assert!(
        repair_prompt.contains(
            "call an existing member (e.g. poll getState() from the rAF loop), or add onStateChange to SpaceInvadersEngine's definition -- keep both files consistent"
        ),
        "{repair_prompt}"
    );
    let event_text = std::fs::read_to_string(&events).unwrap();
    assert!(event_text.contains("\"event\":\"step_verify_repair\""));
    assert!(event_text.contains("\"repair_session_mode\":\"appended\""));
    assert!(event_text.contains("\"ok\":true"), "{event_text}");
}

#[test]
#[cfg(unix)]
fn step_verify_compile_repair_recovers_in_compact_session_after_appended_no_edit() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir
        .path()
        .join(".anvil/runs/api-mismatch-compact/events.jsonl");
    write_api_mismatch_build_shim(dir.path());
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    let mut execution = CompactAwareCompileRepairClient::new();

    let result =
        run_step_plan(&mut execution, &api_mismatch_step_plan(), &cfg).unwrap_or_else(|err| {
            let event_text = std::fs::read_to_string(&events).unwrap_or_default();
            panic!("{err}\nEvents:\n{event_text}");
        });

    assert!(result.contains("plan-run complete"), "{result}");
    assert_eq!(execution.compact_repair_calls(), 1);
    assert!(execution.appended_repair_calls() >= 2);
    let prompts = execution
        .messages()
        .iter()
        .map(|messages| {
            messages
                .iter()
                .map(|message| message.content.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect::<Vec<_>>();
    let compact_prompt = prompts
        .iter()
        .find(|prompt| prompt.contains("Repair session mode: compact"))
        .expect("compact repair prompt");
    assert!(
        compact_prompt.contains("Compile error frames and remedies"),
        "{compact_prompt}"
    );
    assert!(
        compact_prompt.contains("Tool schema reminder"),
        "{compact_prompt}"
    );
    assert!(
        !compact_prompt.contains("Overall goal:"),
        "{compact_prompt}"
    );
    let event_text = std::fs::read_to_string(&events).unwrap();
    assert!(
        event_text.contains("\"repair_session_mode\":\"appended\""),
        "{event_text}"
    );
    assert!(
        event_text.contains("\"repair_session_mode\":\"compact\""),
        "{event_text}"
    );
    assert!(event_text.contains("\"ok\":true"), "{event_text}");
}

#[test]
#[cfg(unix)]
fn step_verify_compile_regeneration_recovers_after_compact_zero_edit() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join(".anvil/runs/regeneration/events.jsonl");
    write_api_mismatch_build_shim(dir.path());
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    let mut execution = RegenerationCompileRepairClient::new(api_mismatch_poll_fix_reply());

    let result =
        run_step_plan(&mut execution, &api_mismatch_step_plan(), &cfg).unwrap_or_else(|err| {
            let event_text = std::fs::read_to_string(&events).unwrap_or_default();
            panic!("{err}\nEvents:\n{event_text}");
        });

    assert!(result.contains("plan-run complete"), "{result}");
    assert_eq!(execution.regeneration_calls(), 1);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("src/app/SpaceInvadersGame.tsx")).unwrap(),
        api_mismatch_poll_fixed_game_source()
    );
    let prompts = execution
        .messages()
        .iter()
        .map(|messages| {
            messages
                .iter()
                .map(|message| message.content.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect::<Vec<_>>();
    let regeneration_prompt = prompts
        .iter()
        .find(|prompt| prompt.contains("Repair session mode: compact regeneration"))
        .expect("regeneration prompt");
    assert!(
        regeneration_prompt.contains(
            "Write the complete corrected file via the Write tool (full content, one file only): src/app/SpaceInvadersGame.tsx"
        ),
        "{regeneration_prompt}"
    );
    assert!(
        regeneration_prompt.contains("Current content of src/app/SpaceInvadersGame.tsx"),
        "{regeneration_prompt}"
    );
    assert!(
        regeneration_prompt.contains(
            "Imported definition context for `SpaceInvadersEngine` from src/lib/game-engine.ts:"
        ),
        "{regeneration_prompt}"
    );
    let event_text = std::fs::read_to_string(&events).unwrap();
    assert!(
        event_text.contains("\"event\":\"repair_regeneration\""),
        "{event_text}"
    );
    assert!(event_text.contains("\"fired\":true"), "{event_text}");
    assert!(event_text.contains("\"accepted\":true"), "{event_text}");
    assert!(event_text.contains("\"error_delta\":1"), "{event_text}");
    assert!(
        event_text.contains("\"target_path\":\"src/app/SpaceInvadersGame.tsx\""),
        "{event_text}"
    );
}

#[test]
#[cfg(unix)]
fn step_verify_compile_regeneration_fires_after_edit_but_fail_then_compact_zero_edit() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir
        .path()
        .join(".anvil/runs/regeneration-after-edit-fail/events.jsonl");
    write_api_mismatch_build_shim(dir.path());
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    let mut execution = EditThenRegenerationCompileRepairClient::new();

    let result =
        run_step_plan(&mut execution, &api_mismatch_step_plan(), &cfg).unwrap_or_else(|err| {
            let event_text = std::fs::read_to_string(&events).unwrap_or_default();
            panic!("{err}\nEvents:\n{event_text}");
        });

    assert!(result.contains("plan-run complete"), "{result}");
    assert_eq!(execution.appended_repair_calls(), 2);
    assert_eq!(execution.compact_repair_calls(), 1);
    assert_eq!(execution.regeneration_calls(), 1);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("src/app/SpaceInvadersGame.tsx")).unwrap(),
        api_mismatch_poll_fixed_game_source()
    );
    let event_text = std::fs::read_to_string(&events).unwrap();
    assert!(
        event_text.contains("\"repair_follow_through\":\"target_matched\""),
        "{event_text}"
    );
    assert!(
        event_text.contains("\"repair_session_mode\":\"compact\""),
        "{event_text}"
    );
    assert!(
        event_text.contains("\"event\":\"repair_regeneration\""),
        "{event_text}"
    );
    assert!(event_text.contains("\"fired\":true"), "{event_text}");
    assert!(event_text.contains("\"accepted\":true"), "{event_text}");
    assert!(event_text.contains("\"error_delta\":1"), "{event_text}");
}

#[test]
#[cfg(unix)]
fn step_verify_compile_regeneration_restores_snapshot_when_not_improved() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir
        .path()
        .join(".anvil/runs/regeneration-reject/events.jsonl");
    write_api_mismatch_build_shim(dir.path());
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    let mut execution = RegenerationCompileRepairClient::new(AssistantReply {
        content: String::new(),
        tool_calls: vec![crate::state::ToolCall::new(
            "Write",
            serde_json::json!({"path":"src/app/SpaceInvadersGame.tsx","content":api_mismatch_broken_game_source()}),
        )],
        prompt_tokens: None,
        completion_tokens: None,
    });

    let err = run_step_plan(&mut execution, &api_mismatch_step_plan(), &cfg)
        .unwrap_err()
        .to_string();

    assert!(err.contains("compile_repair_no_source_change"), "{err}");
    assert_eq!(execution.regeneration_calls(), 1);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("src/app/SpaceInvadersGame.tsx")).unwrap(),
        api_mismatch_broken_game_source()
    );
    let event_text = std::fs::read_to_string(&events).unwrap();
    assert!(
        event_text.contains("\"event\":\"repair_regeneration\""),
        "{event_text}"
    );
    assert!(event_text.contains("\"fired\":true"), "{event_text}");
    assert!(event_text.contains("\"accepted\":false"), "{event_text}");
    assert!(event_text.contains("\"error_delta\":0"), "{event_text}");
    assert!(
        event_text.contains("\"reason\":\"compile_error_count_not_decreased\""),
        "{event_text}"
    );
}

#[test]
fn compile_regeneration_skip_event_records_decision_reason() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir
        .path()
        .join(".anvil/runs/regeneration-skip/events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events.clone());

    emit_compile_regeneration_event(
        &cfg,
        Some("verify-build"),
        "step_repair",
        false,
        false,
        0,
        None,
        "multi_file_compile_failure",
        2,
        2,
        &[],
    );

    let event_text = std::fs::read_to_string(&events).unwrap();
    assert!(
        event_text.contains("\"event\":\"repair_regeneration\""),
        "{event_text}"
    );
    assert!(event_text.contains("\"fired\":false"), "{event_text}");
    assert!(
        event_text.contains("\"reason\":\"multi_file_compile_failure\""),
        "{event_text}"
    );
    assert!(
        event_text.contains("\"regeneration_skipped_reason\":\"multi_file_compile_failure\""),
        "{event_text}"
    );
}

#[test]
#[cfg(unix)]
fn step_verify_compile_zero_edit_reanchors_then_reports_no_source_change() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir
        .path()
        .join(".anvil/runs/api-mismatch-no-edit/events.jsonl");
    write_api_mismatch_build_shim(dir.path());
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    let mut execution = FakeClient::new(vec![
        api_mismatch_initial_reply(3011),
        api_mismatch_read_only_reply(),
        AssistantReply::text("No source behavior changed."),
        api_mismatch_read_only_reply(),
        AssistantReply::text("Still no source behavior changed."),
    ]);

    let err = run_step_plan(&mut execution, &api_mismatch_step_plan(), &cfg)
        .unwrap_err()
        .to_string();

    assert!(err.contains("compile_repair_no_source_change"), "{err}");
    let prompts = execution
        .messages()
        .iter()
        .map(|messages| {
            messages
                .iter()
                .map(|message| message.content.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect::<Vec<_>>();
    assert!(
        prompts
            .iter()
            .any(|prompt| prompt.contains("Compile repair re-anchor")),
        "{prompts:#?}"
    );
    assert!(
        prompts
            .iter()
            .any(|prompt| prompt.contains("Repair session mode: compact")),
        "{prompts:#?}"
    );
    let event_text = std::fs::read_to_string(&events).unwrap();
    assert!(
        event_text.contains("\"failure_kind\":\"compile_repair_no_source_change\""),
        "{event_text}"
    );
    assert!(
        event_text.contains("\"repair_session_mode\":\"appended\""),
        "{event_text}"
    );
    assert!(
        event_text.contains("\"repair_session_mode\":\"compact\""),
        "{event_text}"
    );
    assert!(
        event_text.contains("\"compile_reanchored_retry\":true"),
        "{event_text}"
    );
    assert!(
        event_text.contains("\"reason\":\"compile_repair_no_source_change\""),
        "{event_text}"
    );
    assert!(
        !event_text.contains("\"reason\":\"verify_repair_no_change\""),
        "{event_text}"
    );
}

#[test]
#[cfg(unix)]
fn compile_repair_no_edit_reanchors_then_no_snapshot_gets_narrow_retry() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join(".anvil/runs/no-snapshot/events.jsonl");
    write_static_compile_repair_workspace(dir.path(), static_good_page_source());
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "generic".to_string();
    cfg.eval_events_path = Some(events.clone());
    cfg.completion_contract_path = Some(write_static_build_contract(dir.path()));
    let plan = static_compile_repair_plan();
    let mut planner = FakeClient::new(vec![
        AssistantReply::text(static_phase_step_plan_json(false)),
        AssistantReply::text(static_phase_step_plan_json(false)),
    ]);
    let mut execution = FakeClient::new(vec![
        write_static_page_reply(static_good_page_source()),
        write_static_page_reply(static_broken_page_source()),
        AssistantReply::text("The compile error is in src/app/page.tsx."),
        AssistantReply::text("It needs a syntax fix."),
        AssistantReply::text("The compile error remains in src/app/page.tsx."),
        AssistantReply::text("It still needs a syntax fix."),
        AssistantReply::text("Only fix the compile frame in src/app/page.tsx."),
        AssistantReply::text("No restructuring is needed."),
    ]);

    let err = run_ultra_plan(&mut planner, &mut execution, &plan, &cfg)
        .unwrap_err()
        .to_string();

    assert!(
        err.contains("ultra final acceptance failed after bounded repair"),
        "{err}"
    );
    assert!(err.contains("implementation_compile_error"), "{err}");
    assert!(err.contains("src/app/page.tsx"), "{err}");
    let event_text = std::fs::read_to_string(&events).unwrap();
    assert_eq!(
        event_text
            .matches("\"event\":\"final_acceptance_repair_no_source_change\"")
            .count(),
        3,
        "{event_text}"
    );
    assert!(
        event_text.contains("\"compile_reanchored_retry\":true"),
        "{event_text}"
    );
    assert!(
        event_text.contains("\"event\":\"compile_no_snapshot_narrow_retry\""),
        "{event_text}"
    );
    assert!(
        event_text.contains("\"compile_narrow_no_snapshot_retry\":true"),
        "{event_text}"
    );
    assert!(
        event_text.contains("\"narrow_no_snapshot_retry\":true"),
        "{event_text}"
    );
    assert!(
        event_text.contains("\"failure_kind\":\"compile_repair_no_source_change\""),
        "{event_text}"
    );
    assert!(
        event_text.contains("\"failure_kind\":\"implementation_compile_error\""),
        "{event_text}"
    );
    assert!(
        event_text.contains("\"event\":\"compile_rollback_skipped\""),
        "{event_text}"
    );
    assert!(event_text.contains("snapshot_missing:src/app/page.tsx"));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("src/app/page.tsx")).unwrap(),
        static_broken_page_source()
    );
}

#[test]
#[cfg(unix)]
fn step_verify_compile_repair_exhaustion_rolls_back_snapshot_and_continues() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join(".anvil/runs/step-rollback/events.jsonl");
    write_static_compile_repair_workspace(dir.path(), static_good_page_source());
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    cfg.completion_contract_path = Some(write_static_build_contract(dir.path()));
    let plan = UltraPlan {
        goal: "Create a static Next.js page".to_string(),
        profile: "nextjs".to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: vec![
            UltraPhase {
                id: "phase-one".to_string(),
                prompt: "Confirm the current build is good.".to_string(),
            },
            UltraPhase {
                id: "phase-two".to_string(),
                prompt: "Apply a risky page update and verify the build.".to_string(),
            },
            UltraPhase {
                id: "phase-three".to_string(),
                prompt: "Continue after rollback with the recovered page.".to_string(),
            },
        ],
    };
    let mut planner = FakeClient::new(vec![
        AssistantReply::text(static_phase_step_plan_json(true)),
        AssistantReply::text(static_breaking_build_step_plan_json()),
        AssistantReply::text(static_phase_step_plan_json(false)),
    ]);
    let mut execution = FakeClient::new(vec![
        read_static_page_reply(),
        write_static_page_reply(static_broken_page_source()),
        bash_true_reply(),
        AssistantReply::text("The compile error is unchanged."),
        bash_true_reply(),
        AssistantReply::text("The compile error is still unchanged."),
        bash_true_reply(),
        AssistantReply::text("The compile error remains unchanged."),
        bash_true_reply(),
        AssistantReply::text("The compile error remains unchanged again."),
        bash_true_reply(),
        AssistantReply::text("Continue after rollback."),
        write_static_page_reply(static_broken_page_source()),
        read_static_page_reply(),
        AssistantReply::text("phase three inspected after rollback."),
        read_static_page_reply(),
        AssistantReply::text("phase three verified rollback state."),
        read_static_page_reply(),
        AssistantReply::text("phase three preserved recovered page."),
        read_static_page_reply(),
        AssistantReply::text("phase three complete"),
    ]);

    let result =
        run_ultra_plan(&mut planner, &mut execution, &plan, &cfg).unwrap_or_else(|err| {
            let event_text = std::fs::read_to_string(&events).unwrap_or_default();
            panic!("{err}\nEvents:\n{event_text}");
        });

    assert!(result.contains("ultra-plan-run complete"), "{result}");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("src/app/page.tsx")).unwrap(),
        static_good_page_source()
    );
    assert_eq!(planner.messages().len(), 3);
    let phase_three_prompt = planner_request_text(&planner, 2);
    assert!(
        phase_three_prompt.contains("Carry-forward guidance"),
        "{phase_three_prompt}"
    );
    assert!(
        phase_three_prompt
            .contains("phase phase-two changes to src/app/page.tsx were rolled back"),
        "{phase_three_prompt}"
    );
    assert!(
        phase_three_prompt.contains("Update src/app/page.tsx and verify the build."),
        "{phase_three_prompt}"
    );
    let event_text = std::fs::read_to_string(&events).unwrap();
    assert!(
        event_text.contains("\"event\":\"compile_snapshot_saved\""),
        "{event_text}"
    );
    assert!(
        event_text.contains("\"event\":\"compile_rollback_applied\""),
        "{event_text}"
    );
    assert!(
        event_text.contains("\"event\":\"compile_rollback_context_carried\""),
        "{event_text}"
    );
    assert!(
        !event_text.contains("\"event\":\"recovery_prompt_saved\""),
        "{event_text}"
    );
    assert!(
        event_text.contains("\"phase_id\":\"phase-two\""),
        "{event_text}"
    );
}

#[test]
#[cfg(unix)]
fn step_verify_compile_repair_exhaustion_without_snapshot_still_fails() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join(".anvil/runs/step-no-snapshot/events.jsonl");
    write_static_compile_repair_workspace(dir.path(), static_good_page_source());
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    let mut execution = FakeClient::new(vec![
        write_static_page_reply(static_broken_page_source()),
        bash_true_reply(),
        AssistantReply::text("The compile error is unchanged."),
        bash_true_reply(),
        AssistantReply::text("The compile error is still unchanged."),
        bash_true_reply(),
        AssistantReply::text("The compile error remains unchanged."),
        bash_true_reply(),
        AssistantReply::text("The compile error remains unchanged again."),
    ]);

    let err = run_step_plan(&mut execution, &static_breaking_build_step_plan(), &cfg)
        .unwrap_err()
        .to_string();

    assert!(err.contains("repair prompt saved"), "{err}");
    assert!(err.contains("compile_repair_no_source_change"), "{err}");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("src/app/page.tsx")).unwrap(),
        static_broken_page_source()
    );
    let event_text = std::fs::read_to_string(&events).unwrap();
    assert!(
        event_text.contains("\"event\":\"compile_rollback_skipped\""),
        "{event_text}"
    );
    assert!(
        event_text.contains("snapshot_missing:src/app/page.tsx"),
        "{event_text}"
    );
    assert!(
        event_text.contains("\"event\":\"recovery_prompt_saved\""),
        "{event_text}"
    );
    assert!(
        event_text.contains("\"failure_kind\":\"compile_repair_no_source_change\""),
        "{event_text}"
    );
}

#[test]
#[cfg(unix)]
fn compile_repair_exhaustion_rolls_back_last_known_good_and_continues() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join(".anvil/runs/rollback/events.jsonl");
    write_static_compile_repair_workspace(dir.path(), static_good_page_source());
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "generic".to_string();
    cfg.eval_events_path = Some(events.clone());
    cfg.completion_contract_path = Some(write_static_build_contract(dir.path()));
    let plan = static_compile_repair_plan();
    let mut planner = FakeClient::new(vec![
        AssistantReply::text(static_phase_step_plan_json(true)),
        AssistantReply::text(static_phase_step_plan_json(false)),
    ]);
    let mut execution = FakeClient::new(vec![
        read_static_page_reply(),
        write_static_page_reply(static_broken_page_source()),
        AssistantReply::text("The compile error is in src/app/page.tsx."),
        AssistantReply::text("It needs a syntax fix."),
        AssistantReply::text("The compile error remains in src/app/page.tsx."),
        AssistantReply::text("It still needs a syntax fix."),
    ]);

    let result =
        run_ultra_plan(&mut planner, &mut execution, &plan, &cfg).unwrap_or_else(|err| {
            let event_text = std::fs::read_to_string(&events).unwrap_or_default();
            panic!("{err}\nEvents:\n{event_text}");
        });

    assert!(result.contains("ultra-plan-run complete"), "{result}");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("src/app/page.tsx")).unwrap(),
        static_good_page_source()
    );
    let event_text = std::fs::read_to_string(&events).unwrap();
    assert!(
        event_text.contains("\"event\":\"compile_snapshot_saved\""),
        "{event_text}"
    );
    assert!(
        event_text.contains("\"event\":\"compile_rollback_applied\""),
        "{event_text}"
    );
    assert!(
        event_text.contains("\"event\":\"compile_rollback_context_carried\""),
        "{event_text}"
    );
    assert!(
        event_text.contains("phase phase-two changes to src/app/page.tsx were rolled back; re-apply: Update the page copy for the final app."),
        "{event_text}"
    );
    let summary = std::fs::read_to_string(events.parent().unwrap().join("summary.md")).unwrap();
    assert!(summary.contains("Compile rollback applied:"), "{summary}");
    assert!(summary.contains("src/app/page.tsx"), "{summary}");
}

#[test]
fn compile_error_recovery_handoff_orders_fix_compile_error_first() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join(".anvil/runs/recovery-order/events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events);
    let plan = static_compile_repair_plan();
    let mut report = VerificationReport::pass();
    report.compile_errors.push(CompileError {
        path: "src/app/page.tsx".to_string(),
        line: 12,
        column: 1,
        message: "Expected ';', '}' or <eof>".to_string(),
        excerpt: "12 | BROKEN_SYNTAX\n   | ^".to_string(),
        symbol: None,
        route_bound: None,
    });
    let targets =
        final_acceptance_recovery_repair_targets(&report, RepairTarget::Implementation);
    assert_eq!(
        targets.first().map(String::as_str),
        Some("fix_compile_error")
    );
    let evidence = final_acceptance_recovery_failure_evidence(
        "generic",
        "",
        &report,
        "compile_repair_no_source_change",
    );
    assert!(
        evidence
            .first()
            .is_some_and(|line| line.contains("fix_compile_error: Compile error")),
        "{evidence:?}"
    );
    assert!(
        evidence
            .iter()
            .any(|line| line.contains("12 | BROKEN_SYNTAX"))
    );

    let _handoff = save_ultra_phase_recovery_handoff_with_evidence(
        &cfg,
        &plan,
        &plan.phases[1],
        UltraPhaseRecoveryRequest {
            failure_kind: "compile_repair_no_source_change",
            reason: "compile repair did not edit files",
            missing_paths: &[],
            missing_signals: &[],
            repair_targets: &targets,
            verify_commands: &[],
        },
        &evidence,
    )
    .expect("recovery handoff");

    let repair_text = std::fs::read_dir(dir.path().join(".commandagent/repairs"))
        .unwrap()
        .map(|entry| std::fs::read_to_string(entry.unwrap().path()).unwrap())
        .find(|text| text.contains("Failure evidence:"))
        .expect("repair prompt");
    let evidence_index = repair_text.find("fix_compile_error").unwrap();
    let target_index = repair_text
        .find("Repair targets:\n- fix_compile_error")
        .unwrap();
    assert!(evidence_index < target_index, "{repair_text}");
    let recovery_plan = assert_single_recovery_ultra_plan(dir.path());
    let repair_phase = &recovery_plan.phases[1].prompt;
    let evidence_index = repair_phase.find("fix_compile_error").unwrap();
    let target_index = repair_phase
        .find("Repair targets:\n- fix_compile_error")
        .unwrap();
    assert!(evidence_index < target_index, "{repair_phase}");
}

#[test]
fn build_verifier_readiness_failure_targets_compile_repair_with_arity_remedy() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src/app")).unwrap();
    std::fs::write(
        dir.path().join("src/app/page.tsx"),
        "type Sprite = { x: number; y: number };\n\
function renderSprite(ctx: CanvasRenderingContext2D, sprite: Sprite, scale: number) {\n\
  ctx.fillRect(sprite.x, sprite.y, scale, scale);\n\
}\n\
export default function Page() {\n\
  renderSprite(document.createElement('canvas').getContext('2d')!, { x: 1, y: 2 }, 12, 'debug');\n\
  return <main />;\n\
}\n",
    )
    .unwrap();
    let full_output_path = dir.path().join("build-output.log");
    std::fs::write(
        &full_output_path,
        "./src/app/page.tsx:6:3\nType error: Expected 3 arguments, but got 4.\n",
    )
    .unwrap();
    let readiness = dir.path().join("browser-readiness.json");
    std::fs::write(
        &readiness,
        serde_json::to_string_pretty(&serde_json::json!({
            "status": "failed",
            "ok": false,
            "failure_kind": "build_verifier_failed",
            "output_excerpt": "./src/app/page.tsx:6:3\nType error: Expected 3 arguments, but got 4.\n",
            "build_output_path": "build-output.log"
        }))
        .unwrap(),
    )
    .unwrap();
    let gate = ReleaseGateSummary {
        status: "failed".to_string(),
        reasons: vec!["browser_readiness_failed:build_verifier_failed".to_string()],
        browser_readiness_status: "failed:build_verifier_failed".to_string(),
        browser_readiness_evidence_path: readiness.display().to_string(),
        interaction_evidence_status: "not_exercised:build_verifier_failed".to_string(),
        interaction_evidence_path: String::new(),
    };

    assert_eq!(
        release_recovery_repair_targets(&gate, None),
        vec![
            "fix_compile_error".to_string(),
            "implementation".to_string()
        ]
    );
    let mut report = VerificationReport::pass();
    append_release_gate_observation_failures(&mut report, &gate);
    assert_eq!(
        classify_repair_target(&report),
        RepairTarget::Implementation
    );

    let plan = static_compile_repair_plan();
    let prompt = final_acceptance_repair_prompt(
        dir.path(),
        PromptLayout::Stable,
        &plan,
        &report,
        &UltraRunContext::default(),
        RepairTarget::Implementation.as_str(),
        &["src/app/page.tsx".to_string()],
        &[],
        (1, 2),
        false,
        false,
    );
    assert!(
        prompt.contains("Compile error: src/app/page.tsx:6:3"),
        "{prompt}"
    );
    assert!(
        prompt.contains(
            "TypeScript call-arity repair for `renderSprite`: Expected 3 arguments, but got 4."
        ),
        "{prompt}"
    );
    assert!(
        prompt.contains("Actual same-file signature for `renderSprite`: function renderSprite"),
        "{prompt}"
    );
    assert!(
        prompt.contains(
            "remove the extra argument, or extend the signature -- keep call sites consistent"
        ),
        "{prompt}"
    );
}

#[test]
fn browser_probe_build_verifier_failure_routes_to_compile_ladder() {
    let _probe_guard = dev_server_probe_test_guard();
    let dir = tempfile::tempdir().unwrap();
    let port = free_local_port();
    enable_dev_server_probe_test_override(dir.path());
    let contract = write_compile_error_nextjs_workspace(dir.path(), port);
    std::fs::write(
        dir.path().join("src/components/SpaceInvaders.tsx"),
        "import { useState } from \"react\";\n\
export default function SpaceInvaders() {\n\
  const [playerX, setPlayerX] = useState(0);\n\
  const reset = () => setPlayerX(0);\n\
  playerX = playerX + 1;\n\
  return <main><button onClick={reset}>Restart</button><canvas /><p>{playerX}</p></main>;\n\
}\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("browser-build-output.log"),
        "Failed to compile.\n\n\
./src/components/SpaceInvaders.tsx:5:3\n\
Type error: Cannot assign to 'playerX' because it is a constant.\n\n\
  2 | export default function SpaceInvaders() {\n\
  3 |   const [playerX, setPlayerX] = useState(0);\n\
  4 |   const reset = () => setPlayerX(0);\n\
> 5 |   playerX = playerX + 1;\n\
|   ^\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("browser-readiness.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "status": "failed",
            "ok": false,
            "failure_kind": "build_verifier_failed",
            "output_excerpt": "command failed: npm run build summary: Failed to compile. Type error: Cannot assign to 'playerX' because it is a constant.",
            "build_output_path": "browser-build-output.log"
        }))
        .unwrap(),
    )
    .unwrap();
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.completion_contract_path = Some(contract);
    let gate = browser_release_gate(&cfg);
    assert_eq!(gate.status, "failed", "{gate:?}");
    assert_eq!(
        gate.browser_readiness_status,
        "failed:build_verifier_failed"
    );
    assert!(
        gate.browser_readiness_evidence_path
            .ends_with("browser-readiness.json"),
        "{gate:?}"
    );
    assert_eq!(
        release_recovery_repair_targets(&gate, None),
        vec![
            "fix_compile_error".to_string(),
            "implementation".to_string()
        ]
    );
    let mut report = VerificationReport::pass();
    append_release_gate_observation_failures(&mut report, &gate);
    assert_eq!(
        classify_repair_target(&report),
        RepairTarget::Implementation,
        "{report:?}"
    );
    assert_eq!(report.compile_errors.len(), 1, "{report:?}");
    assert_eq!(
        report.compile_errors[0].path,
        "src/components/SpaceInvaders.tsx"
    );
    assert_eq!(report.compile_errors[0].line, 5);
    assert_eq!(report.compile_errors[0].column, 3);
    let prompt = final_acceptance_repair_prompt(
        dir.path(),
        PromptLayout::Stable,
        &UltraPlan {
            goal: "Create an interactive browser game".to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            intent: "create".to_string(),
            phases: vec![UltraPhase {
                id: "final".to_string(),
                prompt: "Finalize the interactive browser game.".to_string(),
            }],
        },
        &report,
        &UltraRunContext::default(),
        RepairTarget::Implementation.as_str(),
        &["src/components/SpaceInvaders.tsx".to_string()],
        &[],
        (1, FINAL_ACCEPTANCE_REPAIR_MAX_ATTEMPTS),
        false,
        false,
    );
    assert!(
        prompt.contains("TypeScript const-reassignment repair for `playerX`"),
        "{prompt}"
    );
    assert!(
        prompt.contains("Declaration site for `playerX` in src/components/SpaceInvaders.tsx:3"),
        "{prompt}"
    );
    assert!(
        prompt.contains("declare with let, or lift into state if it changes per frame -- keep declaration and all assignments consistent"),
        "{prompt}"
    );
}

#[test]
fn readiness_compile_repair_parses_full_build_output_not_truncated_excerpt() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src/app")).unwrap();
    let mut page = String::new();
    for _ in 0..438 {
        page.push('\n');
    }
    page.push_str("const PLAYER_W = 12;\n");
    page.push_str("export default function Page(){ return <main>{player}</main>; }\n");
    std::fs::write(dir.path().join("src/app/page.tsx"), page).unwrap();
    let full_output_path = dir.path().join("full-build-output.log");
    std::fs::write(
        &full_output_path,
        "Failed to compile.\n\n\
./src/app/page.tsx:440:25\n\
Type error: Cannot find name 'player'. Did you mean 'PLAYER_W'?\n\n\
  438 |\n\
  439 |\n\
> 440 | export default function Page(){ return <main>{player}</main>; }\n\
  |                         ^\n",
    )
    .unwrap();
    let readiness = dir.path().join("browser-readiness.json");
    std::fs::write(
        &readiness,
        serde_json::to_string_pretty(&serde_json::json!({
            "status": "failed",
            "ok": false,
            "failure_kind": "build_verifier_failed",
            "output_excerpt": "command failed: npm run build summary: Failed to compile. Type error: Cannot find name 'player'. Did you mean 'PLAYER_W'?",
            "build_output_path": "full-build-output.log"
        }))
        .unwrap(),
    )
    .unwrap();
    let errors = compile_errors_from_release_evidence_path(&readiness.display().to_string());
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].path, "src/app/page.tsx");
    assert_eq!(errors[0].line, 440);
    assert_eq!(errors[0].column, 25);
    assert_eq!(errors[0].symbol.as_deref(), Some("player"));

    let gate = ReleaseGateSummary {
        status: "failed".to_string(),
        reasons: vec!["browser_readiness_failed:build_verifier_failed".to_string()],
        browser_readiness_status: "failed:build_verifier_failed".to_string(),
        browser_readiness_evidence_path: readiness.display().to_string(),
        interaction_evidence_status: "not_exercised:build_verifier_failed".to_string(),
        interaction_evidence_path: String::new(),
    };
    assert_eq!(
        release_recovery_repair_targets(&gate, None),
        vec![
            "fix_compile_error".to_string(),
            "implementation".to_string()
        ]
    );
    let mut report = VerificationReport::pass();
    append_release_gate_observation_failures(&mut report, &gate);
    assert_eq!(
        classify_repair_target(&report),
        RepairTarget::Implementation
    );
    let prompt = final_acceptance_repair_prompt(
        dir.path(),
        PromptLayout::Stable,
        &static_compile_repair_plan(),
        &report,
        &UltraRunContext::default(),
        RepairTarget::Implementation.as_str(),
        &["src/app/page.tsx".to_string()],
        &[],
        (1, 2),
        false,
        false,
    );
    assert!(
        prompt.contains("Compile error: src/app/page.tsx:440:25"),
        "{prompt}"
    );
    assert!(
        prompt.contains("Compiler suggestion: Did you mean 'PLAYER_W'?"),
        "{prompt}"
    );
    assert!(
        prompt.contains("Cannot-find-name repair for `player`"),
        "{prompt}"
    );
}
