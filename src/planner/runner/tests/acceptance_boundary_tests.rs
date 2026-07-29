#[test]
fn dev_server_marker_with_contamination_is_env_node_env_conflict() {
    let status = run_ignored_runner_harness(
        "planner::runner::tests::dev_server_marker_with_contamination_is_env_node_env_conflict_child",
    );
    assert!(status.success(), "{status}");
}

#[test]
#[ignore]
fn dev_server_marker_with_contamination_is_env_node_env_conflict_child() {
    let output = "warn - You are using a non-standard \"NODE_ENV\" value.";
    let kind = classify_dev_server_env_conflict("http_500", output);
    assert_eq!(kind, verifier_env::ENV_NODE_ENV_CONFLICT_KIND);
    assert!(
        dev_server_output_excerpt(&kind, output)
            .contains(verifier_env::ENV_NODE_ENV_REMEDIATION)
    );
}

#[test]
fn dev_server_port_owner_parser_preserves_pid_and_command() {
    let owner = parse_dev_server_port_owner("p30110\ncnext-server\n").unwrap();
    assert_eq!(owner.pid, Some(30110));
    assert_eq!(owner.command, "next-server");
    assert_eq!(owner.display(), "pid 30110 (next-server)");
}

#[test]
fn nextjs_dev_route_probe_disabled_records_lifecycle_stages() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    let evidence_path = nextjs_dev_route_evidence_path(&cfg);
    let evidence = run_nextjs_dev_route_probe(&cfg, &evidence_path);
    assert_eq!(
        evidence.get("status").and_then(Value::as_str),
        Some("unavailable")
    );
    assert_eq!(
        classify_release_evidence_json(ReleaseEvidenceKind::BrowserReadiness, &evidence),
        ReleaseEvidenceStatus::Unavailable(
            "browser_unavailable:dev_server_probe_disabled_in_tests".to_string()
        )
    );
    let dev_server = evidence
        .get("dev_server")
        .and_then(Value::as_object)
        .expect("dev server evidence object");
    let stages = dev_server
        .get("lifecycle_stages")
        .and_then(Value::as_array)
        .expect("lifecycle stages");
    assert_eq!(
        stages.iter().filter_map(Value::as_str).collect::<Vec<_>>(),
        vec!["start", "wait", "probe", "cleanup"]
    );
    let environment = dev_server
        .get("probe_environment")
        .and_then(Value::as_object)
        .expect("probe environment");
    assert_eq!(
        environment.get("PORT").and_then(Value::as_str),
        Some("3011")
    );
    assert!(environment.contains_key("NODE_ENV"));
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("\"event\":\"dev_server_lifecycle\""));
    assert!(event_text.contains("\"stage\":\"start\""));
    assert!(event_text.contains("\"stage\":\"wait\""));
    assert!(event_text.contains("\"stage\":\"probe\""));
    assert!(event_text.contains("\"stage\":\"cleanup\""));
    assert!(event_text.contains("\"probe_environment\""));
    assert!(
        event_text.contains("browser_unavailable:dev_server_probe_disabled_in_tests"),
        "{event_text}"
    );
}

#[test]
#[cfg(unix)]
fn dev_server_cleanup_kills_grandchild_process_group_without_pipe_deadlock() {
    let dir = tempfile::tempdir().unwrap();
    let port = free_local_port();
    let events = dir.path().join("events.jsonl");
    write_fake_nextjs_dev_workspace(dir.path(), port, true);
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    let evidence_path = nextjs_dev_route_evidence_path(&cfg);

    let started = Instant::now();
    let evidence = run_nextjs_dev_route_probe_with_runtime(
        &cfg,
        &evidence_path,
        true,
        cleanup_dev_server_child,
        BrowserInteractionProbeOptions::default(),
        Some(port),
    );

    assert!(
        started.elapsed() < Duration::from_secs(5),
        "cleanup should be bounded and fast"
    );
    assert_eq!(evidence.get("ok").and_then(Value::as_bool), Some(true));
    assert!(evidence_path.is_file(), "browser readiness evidence");
    assert!(evidence_path.with_file_name("dev-server.out").is_file());
    assert!(evidence_path.with_file_name("dev-server.err").is_file());
    let events_json = read_jsonl_events(&events);
    assert_eq!(
        dev_server_stage_names(&events_json),
        vec!["start", "wait", "probe", "cleanup"]
    );
    let cleanup = events_json
        .iter()
        .find(|event| {
            event.get("event").and_then(Value::as_str) == Some("dev_server_lifecycle")
                && event.get("stage").and_then(Value::as_str) == Some("cleanup")
        })
        .expect("cleanup event");
    assert_eq!(cleanup.get("ok").and_then(Value::as_bool), Some(true));
    let pid = events_json
        .iter()
        .find(|event| {
            event.get("event").and_then(Value::as_str) == Some("dev_server_lifecycle")
                && event.get("stage").and_then(Value::as_str) == Some("start")
        })
        .and_then(|event| event.get("pid"))
        .and_then(Value::as_u64)
        .expect("dev server pid") as u32;
    assert!(
        wait_until_process_group_gone(pid, Duration::from_secs(2)),
        "process group {pid} should be gone"
    );
}

#[test]
#[cfg(unix)]
fn dev_server_writes_readiness_before_forced_cleanup_failure() {
    let dir = tempfile::tempdir().unwrap();
    let port = free_local_port();
    let events = dir.path().join("events.jsonl");
    write_fake_nextjs_dev_workspace(dir.path(), port, false);
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    let evidence_path = nextjs_dev_route_evidence_path(&cfg);

    let evidence = run_nextjs_dev_route_probe_with_runtime(
        &cfg,
        &evidence_path,
        true,
        forced_cleanup_timeout_after_real_cleanup,
        BrowserInteractionProbeOptions::default(),
        Some(port),
    );

    assert_eq!(evidence.get("ok").and_then(Value::as_bool), Some(true));
    let readiness_text =
        std::fs::read_to_string(&evidence_path).expect("readiness evidence written");
    assert!(readiness_text.contains("\"route_rendered\": true"));
    let events_json = read_jsonl_events(&events);
    assert_eq!(
        dev_server_stage_names(&events_json),
        vec!["start", "wait", "probe", "cleanup"]
    );
    let cleanup = events_json
        .iter()
        .find(|event| {
            event.get("event").and_then(Value::as_str) == Some("dev_server_lifecycle")
                && event.get("stage").and_then(Value::as_str) == Some("cleanup")
        })
        .expect("cleanup event");
    assert_eq!(cleanup.get("ok").and_then(Value::as_bool), Some(false));
    assert_eq!(
        cleanup.get("failure_kind").and_then(Value::as_str),
        Some("dev_server_cleanup_timeout")
    );
}

#[test]
#[ignore]
#[cfg(unix)]
#[allow(clippy::zombie_processes)]
fn fake_dev_server_package_manager_child() {
    if crate::env_compat::var("COMMANDAGENT_FAKE_DEV_SERVER_CHILD")
        .ok()
        .as_deref()
        != Some("1")
    {
        return;
    }
    if crate::env_compat::var("COMMANDAGENT_FAKE_DEV_SERVER_GRANDCHILD")
        .ok()
        .as_deref()
        == Some("1")
    {
        let _ = std::process::Command::new("sh")
            .arg("-c")
            .arg("sleep 300")
            .spawn()
            .expect("spawn grandchild");
    }
    let port = std::env::var("PORT")
        .expect("PORT")
        .parse::<u16>()
        .expect("PORT number");
    let listener = std::net::TcpListener::bind(("127.0.0.1", port)).expect("bind fake server");
    listener.set_nonblocking(true).unwrap();
    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let mut request = [0_u8; 1024];
                let _ = stream.read(&mut request);
                let body = "<html><body>fake next ready</body></html>";
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(err) => panic!("fake server accept failed: {err}"),
        }
    }
}

#[test]
fn interaction_probe_failure_evidence_notes_slow_cold_start() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("browser-interaction.json");
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&serde_json::json!({
            "ok": true,
            "status": "passed",
            "cold_start_ms": 20_400,
            "probe_mode": "heuristic"
        }))
        .unwrap(),
    )
    .unwrap();

    let lines =
        interaction_probe_failure_evidence_lines("generic", "", &path.display().to_string());

    assert!(
        lines.iter().any(|line| {
            line == "Note: first page load took 20s (cold start; excluded from assertions)"
        }),
        "{lines:?}"
    );
}

#[test]
#[cfg(unix)]
fn unattached_canvas_ref_guidance_leads_repair_and_reprobe_passes() {
    let _probe_guard = dev_server_probe_test_guard();
    let dir = tempfile::tempdir().unwrap();
    let port = free_local_port();
    let events = dir.path().join(".anvil/runs/unattached-ref/events.jsonl");
    enable_dev_server_probe_test_override(dir.path());
    write_probe_nextjs_workspace(dir.path(), port, unattached_canvas_ref_game_page_source());
    std::fs::write(
        dir.path().join("src/app/useGame.ts"),
        canvas_ref_game_hook_source(),
    )
    .unwrap();
    interaction_probe::write_test_availability_override(dir.path(), true);
    interaction_probe::write_test_result_overrides(
        dir.path(),
        &[
            interaction_state_missing_probe_result(),
            interaction_state_changed_probe_result(),
        ],
    );
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    let plan = UltraPlan {
        goal: explicit_port_goal("Create an interactive browser game", port),
        profile: "nextjs".to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: vec![UltraPhase {
            id: "final".to_string(),
            prompt: "Final acceptance".to_string(),
        }],
    };

    let report = ultra_final_acceptance_report(&plan, &cfg).unwrap();

    assert!(!report.is_pass(), "{report:?}");
    let emitted_events = read_jsonl_events(&events);
    let interaction_event = emitted_events
        .iter()
        .find(|event| {
            event.get("event").and_then(Value::as_str) == Some("browser_interaction_probe")
        })
        .unwrap_or_else(|| panic!("{emitted_events:#?}"));
    assert_eq!(
        interaction_event
            .get("source_diagnostics")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(Value::as_str),
        Some("unattached_ref:canvasRef"),
        "{interaction_event:#?}"
    );
    let expected_paths = final_acceptance_repair_expected_paths(&plan, &cfg, &report).unwrap();
    let prompt = final_acceptance_repair_prompt(
        &cfg.workspace_root,
        PromptLayout::Stable,
        &plan,
        &report,
        &UltraRunContext::default(),
        classify_repair_target(&report).as_str(),
        &expected_paths,
        &[],
        (1, FINAL_ACCEPTANCE_REPAIR_MAX_ATTEMPTS),
        false,
        false,
    );
    let attach = interaction_source_diagnostics(&cfg)
        .into_iter()
        .find(|diagnostic| diagnostic.diagnostic == "unattached_ref:canvasRef")
        .map(|diagnostic| diagnostic.guidance)
        .expect("unattached ref guidance");
    let attach_at = prompt.find(&attach).unwrap_or_else(|| panic!("{prompt}"));
    let repair_guidance = &crate::planner::profiles::nextjs::knowledge::get().repair_guidance;
    let render_at = prompt
        .find(&repair_guidance.canvas_render_loop_checklist)
        .unwrap_or_else(|| panic!("{prompt}"));
    let input_at = prompt
        .find(&repair_guidance.canvas_input_wiring_checklist)
        .unwrap_or_else(|| panic!("{prompt}"));
    assert!(attach_at < render_at && render_at < input_at, "{prompt}");
    let failure_kind = final_acceptance_app_behavior_failure_kind(&report).unwrap();
    let recovery_evidence = final_acceptance_recovery_failure_evidence(
        &plan.profile,
        &plan.goal,
        &report,
        &failure_kind,
    );
    assert_eq!(
        recovery_evidence.first().map(String::as_str),
        Some(attach.as_str()),
        "{recovery_evidence:?}"
    );
    assert!(
        recovery_evidence
            .iter()
            .any(|line| line == &repair_guidance.canvas_render_loop_checklist),
        "{recovery_evidence:?}"
    );

    let mut fake = FakeClient::new(vec![probe_nextjs_scaffold_reply(
        port,
        attached_canvas_ref_game_page_source(),
    )]);
    let mut session = SessionSnapshot::new();
    let outcome = run_final_acceptance_repair_with_ultra_session(
        &mut fake,
        &mut session,
        &prompt,
        &expected_paths,
        &cfg,
        &NOOP_UI,
    )
    .unwrap();
    assert!(
        outcome
            .changed_paths
            .iter()
            .any(|path| path == "src/app/page.tsx"),
        "{outcome:?}"
    );
    clear_final_acceptance_browser_probe_evidence(&cfg);

    let repaired = ultra_final_acceptance_report(&plan, &cfg).unwrap();

    assert!(repaired.is_pass(), "{repaired:?}");
}

#[test]
#[cfg(unix)]
#[ignore = "covered by focused final-acceptance repair/reprobe tests without intermediate phase repair"]
fn behavioral_interaction_failure_repairs_and_reprobes_to_success() {
    let _probe_guard = dev_server_probe_test_guard();
    let dir = tempfile::tempdir().unwrap();
    let port = free_local_port();
    let events = dir.path().join(".anvil/runs/repair-success/events.jsonl");
    enable_dev_server_probe_test_override(dir.path());
    write_probe_nextjs_workspace(dir.path(), port, hollow_canvas_game_page_source());
    interaction_probe::write_test_availability_override(dir.path(), true);
    interaction_probe::write_test_result_overrides(
        dir.path(),
        &[
            interaction_state_missing_probe_result(),
            interaction_state_changed_probe_result(),
        ],
    );
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    let scaffold_plan = generated_nextjs_fixture_plan_json_with_kind(
        "Create buildable app",
        "check_scaffold.py",
        "setup",
    );
    let mut planner = FakeClient::new(vec![
        AssistantReply::text(scaffold_plan),
        AssistantReply::text(final_marker_implement_step_plan_json()),
    ]);
    let mut execution = FakeClient::new(vec![
        probe_nextjs_scaffold_reply(port, interactive_game_page_source().to_string()),
        probe_nextjs_scaffold_reply(port, interactive_game_page_variant(1)),
        probe_nextjs_scaffold_reply(port, interactive_game_page_variant(2)),
        probe_nextjs_scaffold_reply(port, interactive_game_page_variant(3)),
        probe_nextjs_scaffold_reply(port, interactive_game_page_variant(4)),
    ]);
    let plan = UltraPlan {
        goal: "Create an interactive browser game".to_string(),
        profile: "nextjs".to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: vec![
            UltraPhase {
                id: "first".to_string(),
                prompt: "First implementation pass".to_string(),
            },
            UltraPhase {
                id: "final".to_string(),
                prompt: "Final implementation pass".to_string(),
            },
        ],
    };

    let result = run_ultra_plan(&mut planner, &mut execution, &plan, &cfg).unwrap();

    assert_eq!(result, "ultra-plan-run complete: 2 phases");
    let event_text = std::fs::read_to_string(&events).unwrap();
    assert!(event_text.contains("\"event\":\"final_acceptance_repair_start\""));
    assert!(!event_text.contains("\"event\":\"final_acceptance_repair_exhausted\""));
    assert!(event_text.contains("\"event\":\"ultra_plan_complete\""));
    assert!(
        event_text
            .matches("\"event\":\"browser_interaction_probe\"")
            .count()
            >= 2,
        "{event_text}"
    );
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
        .find(|prompt| prompt.contains("Repair the final acceptance failure"))
        .expect("repair prompt");
    assert!(
        repair_prompt.contains(
            "ArrowLeft keydown, ArrowRight keydown, Space keydown, canvas/center click"
        )
    );
    assert!(repair_prompt.contains("player=20 score=0 health=3"));
    assert!(repair_prompt.contains(canvas_game_repair_guidance()));
    assert!(repair_prompt.contains("Route-bound implementation targets:"));
    assert!(repair_prompt.contains("src/app/page.tsx"));
}

#[test]
#[cfg(unix)]
#[ignore = "covered by focused final-acceptance repair/reprobe tests without intermediate phase repair"]
fn behavioral_interaction_failure_exhausts_after_two_reprobe_cycles() {
    let _probe_guard = dev_server_probe_test_guard();
    let dir = tempfile::tempdir().unwrap();
    let port = free_local_port();
    let events = dir.path().join(".anvil/runs/repair-exhaust/events.jsonl");
    enable_dev_server_probe_test_override(dir.path());
    write_probe_nextjs_workspace(dir.path(), port, interactive_game_page_source());
    interaction_probe::write_test_availability_override(dir.path(), true);
    interaction_probe::write_test_result_overrides(
        dir.path(),
        &[
            interaction_state_missing_probe_result(),
            interaction_state_missing_probe_result(),
            interaction_state_missing_probe_result(),
        ],
    );
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    let scaffold_plan = generated_nextjs_fixture_plan_json_with_kind(
        "Create buildable app",
        "check_scaffold.py",
        "setup",
    );
    let mut planner = FakeClient::new(vec![
        AssistantReply::text(scaffold_plan),
        AssistantReply::text(final_marker_implement_step_plan_json()),
    ]);
    let mut execution = FakeClient::new(vec![
        probe_nextjs_scaffold_reply(port, interactive_game_page_source().to_string()),
        probe_nextjs_scaffold_reply(port, interactive_game_page_variant(1)),
        probe_nextjs_scaffold_reply(port, interactive_game_page_variant(2)),
        probe_nextjs_scaffold_reply(port, interactive_game_page_variant(3)),
        probe_nextjs_scaffold_reply(port, interactive_game_page_variant(4)),
        probe_nextjs_scaffold_reply(port, interactive_game_page_variant(5)),
    ]);
    let plan = UltraPlan {
        goal: "Create an interactive browser game".to_string(),
        profile: "nextjs".to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: vec![
            UltraPhase {
                id: "first".to_string(),
                prompt: "First implementation pass".to_string(),
            },
            UltraPhase {
                id: "final".to_string(),
                prompt: "Final implementation pass".to_string(),
            },
        ],
    };

    let err = run_ultra_plan(&mut planner, &mut execution, &plan, &cfg)
        .unwrap_err()
        .to_string();

    assert!(
        err.contains(
            "ultra final acceptance failed after bounded repair: browser_interaction_failed:input_state_change_missing_after_start"
        ),
        "{err}"
    );
    let event_text = std::fs::read_to_string(&events).unwrap();
    assert_eq!(
        event_text
            .matches("\"event\":\"final_acceptance_repair_start\"")
            .count(),
        2,
        "{event_text}"
    );
    assert!(event_text.contains(
        "\"failure_kind\":\"browser_interaction_failed:input_state_change_missing_after_start\""
    ));
    assert!(!event_text.contains("interaction_unverified_probe_unavailable"));
    assert!(!event_text.contains("/setup-interaction-probe"));
    let recovery_plan = assert_single_recovery_ultra_plan(dir.path());
    let recovery_text = render_ultra_plan(&recovery_plan);
    assert!(
        recovery_text.contains("input_state_render_wiring"),
        "{recovery_text}"
    );
    assert!(
        recovery_text.contains(canvas_game_repair_guidance()),
        "{recovery_text}"
    );
    assert!(!recovery_text.contains("/setup-interaction-probe"));
}

#[test]
#[cfg(unix)]
fn focused_behavioral_repair_prompt_and_reprobe_passes() {
    let _probe_guard = dev_server_probe_test_guard();
    let dir = tempfile::tempdir().unwrap();
    let port = free_local_port();
    let events = dir.path().join(".anvil/runs/focused-success/events.jsonl");
    enable_dev_server_probe_test_override(dir.path());
    write_probe_nextjs_workspace(dir.path(), port, interactive_game_page_source());
    interaction_probe::write_test_availability_override(dir.path(), true);
    interaction_probe::write_test_result_overrides(
        dir.path(),
        &[
            interaction_state_missing_probe_result(),
            interaction_state_changed_probe_result(),
        ],
    );
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    let plan = UltraPlan {
        goal: explicit_port_goal("Create an interactive browser game", port),
        profile: "nextjs".to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: vec![UltraPhase {
            id: "final".to_string(),
            prompt: "Final acceptance".to_string(),
        }],
    };
    let initial_report = ultra_final_acceptance_report(&plan, &cfg).unwrap();
    assert!(!initial_report.is_pass(), "{initial_report:?}");
    assert_eq!(
        classify_repair_target(&initial_report),
        RepairTarget::Implementation
    );
    let expected_paths =
        final_acceptance_repair_expected_paths(&plan, &cfg, &initial_report).unwrap();
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
    assert!(
        repair_prompt.contains(
            "ArrowLeft keydown, ArrowRight keydown, Space keydown, canvas/center click"
        )
    );
    assert!(repair_prompt.contains("player=20 score=0 health=3"));
    assert!(repair_prompt.contains(canvas_game_repair_guidance()));
    assert!(
        repair_prompt.contains("probe mode: heuristic"),
        "{repair_prompt}"
    );
    assert!(
        repair_prompt.contains("contract hook status: primary_missing"),
        "{repair_prompt}"
    );
    assert!(repair_prompt.contains("candidate table"), "{repair_prompt}");
    assert!(repair_prompt.contains("rank 2: text=\"Start\" changed=true"));
    assert!(repair_prompt.contains("src/app/page.tsx"));
    let mut fake = FakeClient::new(vec![probe_nextjs_scaffold_reply(
        port,
        contract_interactive_game_page_variant(9),
    )]);
    let mut session = SessionSnapshot::new();
    let outcome = run_final_acceptance_repair_with_ultra_session(
        &mut fake,
        &mut session,
        &repair_prompt,
        &expected_paths,
        &cfg,
        &NOOP_UI,
    )
    .unwrap();
    assert!(!outcome.changed_paths.is_empty(), "{outcome:?}");
    clear_final_acceptance_browser_probe_evidence(&cfg);
    let repaired_report = ultra_final_acceptance_report(&plan, &cfg).unwrap();
    assert!(repaired_report.is_pass(), "{repaired_report:?}");
    let event_text = std::fs::read_to_string(&events).unwrap();
    assert!(
        event_text
            .matches("\"event\":\"browser_interaction_probe\"")
            .count()
            >= 2,
        "{event_text}"
    );
    assert!(!event_text.contains("probe_unavailable"));
}

#[test]
#[cfg(unix)]
fn focused_behavioral_repair_exhaustion_handoff_uses_probe_failure() {
    let _probe_guard = dev_server_probe_test_guard();
    let dir = tempfile::tempdir().unwrap();
    let port = free_local_port();
    let events = dir.path().join(".anvil/runs/focused-exhaust/events.jsonl");
    enable_dev_server_probe_test_override(dir.path());
    write_probe_nextjs_workspace(dir.path(), port, interactive_game_page_source());
    interaction_probe::write_test_availability_override(dir.path(), true);
    interaction_probe::write_test_result_overrides(
        dir.path(),
        &[
            interaction_state_missing_probe_result(),
            interaction_state_missing_probe_result(),
            interaction_state_missing_probe_result(),
        ],
    );
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    let plan = UltraPlan {
        goal: explicit_port_goal("Create an interactive browser game", port),
        profile: "nextjs".to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: vec![UltraPhase {
            id: "final".to_string(),
            prompt: "Final acceptance".to_string(),
        }],
    };
    let mut report = ultra_final_acceptance_report(&plan, &cfg).unwrap();
    assert!(!report.is_pass(), "{report:?}");
    let mut fake = FakeClient::new(vec![
        probe_nextjs_scaffold_reply(port, interactive_game_page_variant(10)),
        probe_nextjs_scaffold_reply(port, interactive_game_page_variant(11)),
    ]);
    let mut session = SessionSnapshot::new();
    for attempt in 1..=FINAL_ACCEPTANCE_REPAIR_MAX_ATTEMPTS {
        let target = classify_repair_target(&report);
        let expected_paths =
            final_acceptance_repair_expected_paths(&plan, &cfg, &report).unwrap();
        let prompt = final_acceptance_repair_prompt(
            &cfg.workspace_root,
            PromptLayout::Stable,
            &plan,
            &report,
            &UltraRunContext::default(),
            target.as_str(),
            &expected_paths,
            &[],
            (attempt, FINAL_ACCEPTANCE_REPAIR_MAX_ATTEMPTS),
            false,
            false,
        );
        let outcome = run_final_acceptance_repair_with_ultra_session(
            &mut fake,
            &mut session,
            &prompt,
            &expected_paths,
            &cfg,
            &NOOP_UI,
        )
        .unwrap();
        assert!(!outcome.changed_paths.is_empty(), "{outcome:?}");
        clear_final_acceptance_browser_probe_evidence(&cfg);
        report = ultra_final_acceptance_report(&plan, &cfg).unwrap();
    }
    assert!(!report.is_pass(), "{report:?}");
    let failure_kind = final_acceptance_app_behavior_failure_kind(&report).unwrap();
    assert_eq!(
        failure_kind,
        "browser_interaction_failed:input_state_change_missing_after_start"
    );
    let reason = final_acceptance_recovery_reason(
        &plan.profile,
        &plan.goal,
        &report,
        &failure_kind,
        "bounded_repair_exhausted",
    );
    let targets =
        final_acceptance_recovery_repair_targets(&report, classify_repair_target(&report));
    assert_eq!(targets, vec!["input_state_render_wiring".to_string()]);
    let missing_signals = verification_missing_signals(&report);
    let phase = plan.phases.last().unwrap();
    let _handoff = save_ultra_phase_recovery_handoff(
        &cfg,
        &plan,
        phase,
        UltraPhaseRecoveryRequest {
            failure_kind: &failure_kind,
            reason: &reason,
            missing_paths: &report.missing_paths,
            missing_signals: &missing_signals,
            repair_targets: &targets,
            verify_commands: &[],
        },
    )
    .expect("handoff saved");
    let recovery_plan = assert_single_recovery_ultra_plan(dir.path());
    let recovery_text = render_ultra_plan(&recovery_plan);
    assert!(
        recovery_text.contains("input_state_render_wiring"),
        "{recovery_text}"
    );
    assert!(
        recovery_text.contains(canvas_game_repair_guidance()),
        "{recovery_text}"
    );
    assert!(!recovery_text.contains("/setup-interaction-probe"));
    let event_text = std::fs::read_to_string(&events).unwrap();
    assert!(!event_text.contains("interaction_unverified_probe_unavailable"));
    assert!(!event_text.contains("/setup-interaction-probe"));
}

#[test]
#[cfg(unix)]
fn overlay_only_restart_after_probe_success_fails_without_reachable_restart_contract() {
    let _probe_guard = dev_server_probe_test_guard();
    let dir = tempfile::tempdir().unwrap();
    let port = free_local_port();
    let events = dir
        .path()
        .join(".anvil/runs/overlay-restart-partial/events.jsonl");
    write_probe_nextjs_workspace(dir.path(), port, overlay_only_restart_game_page_source());
    let run_dir = events.parent().unwrap();
    std::fs::create_dir_all(run_dir).unwrap();
    std::fs::write(
        run_dir.join("browser-readiness.json"),
        r#"{"ok":true,"status":"passed","http_status":200,"route_rendered":true}"#,
    )
    .unwrap();
    std::fs::write(
        run_dir.join("browser-interaction.json"),
        serde_json::to_string_pretty(&recovery_not_observed_probe_result()).unwrap(),
    )
    .unwrap();
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    let plan = UltraPlan {
        goal: explicit_port_goal("Create an interactive browser game", port),
        profile: "nextjs".to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: vec![UltraPhase {
            id: "final".to_string(),
            prompt: "Final acceptance".to_string(),
        }],
    };

    let report = ultra_final_acceptance_report(&plan, &cfg).unwrap();

    assert!(!report.is_pass(), "{report:?}");
    let event_text = std::fs::read_to_string(&events).unwrap();
    assert!(
        event_text.contains(
            "restart_or_recoverable_state_evidence:unverified:terminal_state_not_reached"
        ),
        "{event_text}"
    );
    assert!(
        event_text.contains("interaction_unverified:terminal_state_not_reached"),
        "{event_text}"
    );
    assert!(event_text.contains("contract_instrumentation_missing:restart"));
    assert!(event_text.contains("\"release_gate_status\":\"failed\""));
    let ultra = latest_event(&events, "ultra_final_acceptance");
    let recovery_prompt_path = ultra
        .get("recovery_prompt_path")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let recovery_prompt =
        std::fs::read_to_string(dir.path().join(recovery_prompt_path)).unwrap();
    assert!(
        recovery_prompt.contains(RESTART_PARTIAL_REPAIR_GUIDANCE),
        "{recovery_prompt}"
    );
    assert!(
        !event_text
            .contains("\"missing_evidence\":[\"restart_or_recoverable_state_evidence\"]")
    );
    assert!(!event_text.contains("probe_unavailable"), "{event_text}");
    assert!(
        !event_text.contains("/setup-interaction-probe"),
        "{event_text}"
    );
    assert_eq!(
        ultra
            .get("evidence_tiers")
            .and_then(|tiers| tiers.get("restart_or_recoverable_state_evidence"))
            .and_then(Value::as_str),
        Some("unverified:terminal_state_not_reached")
    );
    assert_eq!(
        ultra
            .get("runtime_acceptance_status")
            .and_then(Value::as_str),
        Some("partial")
    );
    assert_eq!(
        ultra.get("final_acceptance_status").and_then(Value::as_str),
        Some("incomplete")
    );
    assert_eq!(
        ultra
            .get("interaction_evidence_status")
            .and_then(Value::as_str),
        Some("failed:contract_instrumentation_missing:restart")
    );
    assert_eq!(
        ultra
            .get("evidence_arbitration")
            .and_then(|arbitration| arbitration.get("restart_or_recoverable_state_evidence"))
            .and_then(|record| record.get("behavioral_observation"))
            .and_then(Value::as_str),
        Some("terminal_state_not_reached")
    );
}

#[test]
#[cfg(unix)]
fn final_acceptance_repair_cycle_reprobes_restart_hook_recovery_to_pass() {
    let _probe_guard = dev_server_probe_test_guard();
    let dir = tempfile::tempdir().unwrap();
    let port = free_local_port();
    let events = dir.path().join(".anvil/runs/restart-cycle/events.jsonl");
    enable_dev_server_probe_test_override(dir.path());
    interaction_probe::write_test_availability_override(dir.path(), true);
    interaction_probe::write_test_result_overrides(
        dir.path(),
        &[
            recovery_not_observed_probe_result(),
            interaction_state_changed_probe_result(),
        ],
    );
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    let plan = UltraPlan {
        goal: explicit_port_goal("Create an interactive browser game with restart flow", port),
        profile: "nextjs".to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: vec![
            UltraPhase {
                id: "first".to_string(),
                prompt: "Scaffold the app".to_string(),
            },
            UltraPhase {
                id: "final".to_string(),
                prompt: "Final implementation pass".to_string(),
            },
        ],
    };
    let fixed_page = contract_interactive_game_page_source();
    let mut planner = FakeClient::new(vec![
        AssistantReply::text(generated_nextjs_artifact_plan_json_with_build_verify(
            "Create buildable app",
        )),
        AssistantReply::text(static_phase_step_plan_json(true)),
    ]);
    let mut execution = FakeClient::new(vec![
        probe_nextjs_scaffold_reply(
            port,
            contract_interactive_game_page_without_restart_source(),
        ),
        read_static_page_reply(),
        AssistantReply {
            content: String::new(),
            tool_calls: vec![crate::state::ToolCall::new(
                "Write",
                serde_json::json!({"path":"src/app/page.tsx","content":fixed_page}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        },
    ]);

    let result =
        run_ultra_plan(&mut planner, &mut execution, &plan, &cfg).unwrap_or_else(|err| {
            let event_text = std::fs::read_to_string(&events).unwrap_or_default();
            panic!("{err}\nEvents:\n{event_text}");
        });

    assert_eq!(result, "ultra-plan-run complete: 2 phases");
    let event_text = std::fs::read_to_string(&events).unwrap();
    assert_eq!(
        event_text
            .matches("\"event\":\"ultra_final_acceptance\"")
            .count(),
        2,
        "{event_text}"
    );
    assert!(
        event_text
            .matches("\"event\":\"browser_interaction_probe\"")
            .count()
            >= 2,
        "{event_text}"
    );
    let events_json = read_jsonl_events(&events);
    let ultra_cycles = events_json
        .iter()
        .filter(|event| {
            event.get("event").and_then(Value::as_str) == Some("ultra_final_acceptance")
        })
        .filter_map(|event| event.get("cycle_index").and_then(Value::as_u64))
        .collect::<Vec<_>>();
    assert_eq!(ultra_cycles, vec![0, 1], "{event_text}");
    let final_acceptance = latest_event(&events, "ultra_final_acceptance");
    assert_eq!(
        final_acceptance
            .get("runtime_acceptance_status")
            .and_then(Value::as_str),
        Some("pass")
    );
    assert_eq!(
        final_acceptance
            .get("evidence_arbitration")
            .and_then(|arbitration| arbitration.get("restart_or_recoverable_state_evidence"))
            .and_then(|record| record.get("behavioral_observation"))
            .and_then(Value::as_str),
        Some("recovery_transition")
    );
    let cycle = latest_event(&events, "final_acceptance_cycle_complete");
    assert_eq!(cycle.get("cycle_index").and_then(Value::as_u64), Some(1));
    assert!(
        cycle
            .get("resolved_keys")
            .and_then(Value::as_array)
            .is_some_and(|items| items
                .iter()
                .any(|item| item.as_str() == Some("restart_or_recoverable_state_evidence"))),
        "{cycle}"
    );
    assert!(
        cycle
            .get("route_bound_changed_paths")
            .and_then(Value::as_array)
            .is_some_and(|items| items
                .iter()
                .any(|item| item.as_str() == Some("src/app/page.tsx"))),
        "{cycle}"
    );
    let summary = std::fs::read_to_string(events.parent().unwrap().join("summary.md")).unwrap();
    assert!(
        summary.contains("Final acceptance repair cycles:"),
        "{summary}"
    );
    assert!(
        summary.contains("restart_or_recoverable_state_evidence"),
        "{summary}"
    );
}

#[test]
fn final_acceptance_evidence_no_change_uses_reanchor_then_compact_ladder() {
    let (mode, reanchored, compact) = evidence_repair_retry_mode(true, 0);
    assert_eq!(mode, "appended");
    assert!(!reanchored);
    assert!(!compact);

    let (mode, reanchored, compact) = evidence_repair_retry_mode(true, 1);
    assert_eq!(mode, "appended");
    assert!(reanchored);
    assert!(!compact);

    let (mode, reanchored, compact) = evidence_repair_retry_mode(true, 2);
    assert_eq!(mode, "compact");
    assert!(!reanchored);
    assert!(compact);

    let (mode, reanchored, compact) = evidence_repair_retry_mode(false, 3);
    assert_eq!(mode, "appended");
    assert!(!reanchored);
    assert!(!compact);
}

#[test]
#[cfg(unix)]
fn final_acceptance_budget_exhaustion_uses_last_cycle_reason() {
    let _probe_guard = dev_server_probe_test_guard();
    let dir = tempfile::tempdir().unwrap();
    let port = free_local_port();
    let events = dir.path().join(".anvil/runs/last-cycle/events.jsonl");
    enable_dev_server_probe_test_override(dir.path());
    interaction_probe::write_test_availability_override(dir.path(), true);
    interaction_probe::write_test_result_overrides(
        dir.path(),
        &[
            interaction_state_missing_probe_result(),
            recovery_not_observed_probe_result(),
            recovery_not_observed_probe_result(),
        ],
    );
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    let plan = UltraPlan {
        goal: explicit_port_goal("Create an interactive browser game with restart flow", port),
        profile: "nextjs".to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: vec![
            UltraPhase {
                id: "first".to_string(),
                prompt: "Scaffold the app".to_string(),
            },
            UltraPhase {
                id: "final".to_string(),
                prompt: "Final implementation pass".to_string(),
            },
        ],
    };
    let mut planner = FakeClient::new(vec![
        AssistantReply::text(generated_nextjs_artifact_plan_json_with_build_verify(
            "Create buildable app",
        )),
        AssistantReply::text(static_phase_step_plan_json(true)),
    ]);
    let mut execution = FakeClient::new(vec![
        probe_nextjs_scaffold_reply(
            port,
            interactive_game_page_without_restart_source().to_string(),
        ),
        read_static_page_reply(),
        probe_nextjs_scaffold_reply(
            port,
            contract_interactive_game_page_without_restart_variant(101),
        ),
        probe_nextjs_scaffold_reply(
            port,
            contract_interactive_game_page_without_restart_variant(102),
        ),
    ]);

    let err = run_ultra_plan(&mut planner, &mut execution, &plan, &cfg)
        .unwrap_err()
        .to_string();

    assert!(
        err.contains("restart_or_recoverable_state_evidence"),
        "{err}"
    );
    assert!(
        !err.contains("input_state_change_missing_after_start"),
        "{err}"
    );
    let exhausted = latest_event(&events, "final_acceptance_repair_exhausted");
    assert_eq!(
        exhausted.get("cycle_index").and_then(Value::as_u64),
        Some(2)
    );
    assert!(
        exhausted
            .get("primary_reason")
            .and_then(Value::as_str)
            .is_some_and(|reason| reason.contains(
                "capability_evidence_unresolved:restart_or_recoverable_state_evidence"
            )),
        "{exhausted}"
    );
    assert_eq!(
        exhausted.get("failure_kind").and_then(Value::as_str),
        Some("capability_evidence_unresolved:restart_or_recoverable_state_evidence")
    );
    assert_eq!(
        exhausted
            .get("pending_capability_evidence_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let event_text = std::fs::read_to_string(&events).unwrap();
    assert!(
        event_text.contains(
            "restart_or_recoverable_state_evidence: add data-anvil-action=\\\"restart\\\""
        ),
        "{event_text}"
    );
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
        .find(|prompt| prompt.contains("Repair the final acceptance failure"))
        .expect("repair prompt");
    assert!(
        repair_prompt.contains("Pending capability evidence remedies:"),
        "{repair_prompt}"
    );
    assert!(
        repair_prompt.contains(
            "restart_or_recoverable_state_evidence: add data-anvil-action=\"restart\""
        ),
        "{repair_prompt}"
    );
    assert_eq!(
        event_text
            .matches("\"event\":\"final_acceptance_cycle_complete\"")
            .count(),
        2,
        "{event_text}"
    );
}

#[test]
fn route_unbound_runtime_guidance_names_file_and_remedies() {
    let report = RuntimeAcceptanceReport {
        missing_evidence: vec!["user_input_handler_evidence".to_string()],
        diagnostics: vec![
            "route_unbound_capability_artifact:src/components/SpaceInvaders.tsx".to_string(),
        ],
        ..RuntimeAcceptanceReport::default()
    };

    let guidance =
        runtime_acceptance_repair_guidance("nextjs", "Space Invaders game", &report).join("\n");

    assert!(
        guidance.contains("src/components/SpaceInvaders.tsx"),
        "{guidance}"
    );
    assert!(
        guidance.contains("import it from the route page"),
        "{guidance}"
    );
    assert!(
        guidance.contains("consolidate into page.tsx and delete the dead component"),
        "{guidance}"
    );
}

#[test]
fn runtime_guidance_for_restart_terminal_unreached_offers_partial_choice() {
    let report = RuntimeAcceptanceReport {
        unverified_evidence: vec![
            "restart_or_recoverable_state_evidence:unverified:terminal_state_not_reached"
                .to_string(),
        ],
        ..RuntimeAcceptanceReport::default()
    };

    let guidance =
        runtime_acceptance_repair_guidance("nextjs", "Space Invaders game", &report).join("\n");

    assert!(
        guidance.contains(RESTART_PARTIAL_REPAIR_GUIDANCE),
        "{guidance}"
    );
}

#[test]
fn persistence_reset_runtime_guidance_names_reload_persistence_repair() {
    let report = RuntimeAcceptanceReport {
        missing_evidence: vec!["persistence_evidence".to_string()],
        ..RuntimeAcceptanceReport::default()
    };

    let guidance =
        runtime_acceptance_repair_guidance("nextjs", "Create a persistent notes app", &report)
            .join("\n");

    assert!(
        guidance.contains("load persisted state on mount"),
        "{guidance}"
    );
    assert!(
        guidance.contains("read localStorage in initialization"),
        "{guidance}"
    );
    assert!(guidance.contains("write on mutation"), "{guidance}");
}

#[test]
fn token_echo_missing_runtime_guidance_names_live_preview_repair() {
    let report = RuntimeAcceptanceReport {
        missing_evidence: vec!["live_preview_evidence".to_string()],
        ..RuntimeAcceptanceReport::default()
    };

    let guidance =
        runtime_acceptance_repair_guidance("nextjs", "Create a notes app", &report).join("\n");

    assert!(
        guidance.contains("render the input's content reactively"),
        "{guidance}"
    );
    assert!(guidance.contains("no manual rebuild"), "{guidance}");
    assert!(
        guidance.contains("typed text must appear in the preview/list"),
        "{guidance}"
    );
}

#[test]
fn token_echo_after_reload_only_repair_guidance_names_reactivity() {
    let mut report = VerificationReport::pass();
    report.push_profile_failure(
        "browser_interaction_failed:token_echo_after_reload_only".to_string(),
    );

    let guidance = final_acceptance_recovery_reason(
        "nextjs",
        "Create a notes app",
        &report,
        "acceptance failed",
        "repair exhausted",
    );

    assert!(
        guidance.contains("preview renders only after reload"),
        "{guidance}"
    );
    assert!(guidance.contains("make it reactive to input"), "{guidance}");
    assert!(!guidance.contains("token never rendered"), "{guidance}");
}

#[test]
fn browser_interaction_probe_options_require_reload_only_for_persistence_contract() {
    let game = browser_interaction_probe_options(
        &[
            "stateful_interaction".to_string(),
            "player_control".to_string(),
        ],
        &["stateful_update_evidence".to_string()],
    );
    assert!(
        !game.persistence_required,
        "game contracts without persistence must not reload"
    );

    let by_capability = browser_interaction_probe_options(&["persistence".to_string()], &[]);
    assert!(by_capability.persistence_required);

    let by_evidence =
        browser_interaction_probe_options(&[], &["persistence_evidence".to_string()]);
    assert!(by_evidence.persistence_required);
}

#[test]
fn browser_interaction_probe_options_require_text_echo_for_preview_contracts() {
    let game = browser_interaction_probe_options(
        &[
            "stateful_interaction".to_string(),
            "player_control".to_string(),
        ],
        &["stateful_update_evidence".to_string()],
    );
    assert!(!game.text_entry_required);
    assert!(!game.token_echo_required);

    let requested_content =
        browser_interaction_probe_options(&["requested_content".to_string()], &[]);
    assert!(requested_content.text_entry_required);
    assert!(requested_content.token_echo_required);

    let live_preview =
        browser_interaction_probe_options(&[], &["live_preview_evidence".to_string()]);
    assert!(live_preview.text_entry_required);
    assert!(live_preview.token_echo_required);
}

