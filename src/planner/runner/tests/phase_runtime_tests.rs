#[test]
#[cfg(unix)]
fn manifest_changed_reconciliation_installs_before_next_build_verification() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    write_fake_npm_dependency_installer(dir.path());
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"scripts":{"build":"next build"},"dependencies":{"next":"^14.2.0","react":"^18.3.0","react-dom":"^18.3.0"}}"#,
    )
    .unwrap();
    let mut setup_authority = UltraRunSetupAuthorityState::default();
    setup_authority.grant("phase_setup_step");
    reconcile_run_dependency_setup(
        &cfg,
        "nextjs",
        DependencyReconciliationTrigger::DeclaredDependenciesNotReady,
        &setup_authority,
    )
    .unwrap();
    assert!(!dependency_setup::node_dependency_declarations_fingerprint_mismatch(dir.path()));

    std::fs::write(
        dir.path().join("package.json"),
        r#"{"scripts":{"build":"next build"},"dependencies":{"next":"^14.2.0","react":"^18.3.0","react-dom":"^18.3.0"},"devDependencies":{"tailwindcss":"^3.4.19","postcss":"^8.5.15","autoprefixer":"^10.4.20"}}"#,
    )
    .unwrap();
    let changed =
        reconcile_manifest_changed_dependencies_if_needed(&cfg, "nextjs", &mut setup_authority)
            .unwrap();

    assert!(changed.is_some());
    assert!(
        dir.path()
            .join("node_modules/tailwindcss/package.json")
            .is_file()
    );
    assert!(
        dir.path()
            .join("node_modules/postcss/package.json")
            .is_file()
    );
    assert!(
        dir.path()
            .join("node_modules/autoprefixer/package.json")
            .is_file()
    );
    assert!(!dependency_setup::node_dependency_declarations_fingerprint_mismatch(dir.path()));
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains(r#""trigger":"manifest_changed""#));
}

#[test]
#[cfg(unix)]
fn scripts_only_manifest_edit_does_not_reconcile_dependencies() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    write_fake_npm_dependency_installer(dir.path());
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"scripts":{"build":"next build"},"dependencies":{"next":"^14.2.0","react":"^18.3.0","react-dom":"^18.3.0"}}"#,
    )
    .unwrap();
    let mut setup_authority = UltraRunSetupAuthorityState::default();
    setup_authority.grant("phase_setup_step");
    reconcile_run_dependency_setup(
        &cfg,
        "nextjs",
        DependencyReconciliationTrigger::DeclaredDependenciesNotReady,
        &setup_authority,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"scripts":{"build":"next build --turbo"},"dependencies":{"next":"^14.2.0","react":"^18.3.0","react-dom":"^18.3.0"}}"#,
    )
    .unwrap();

    let changed =
        reconcile_manifest_changed_dependencies_if_needed(&cfg, "nextjs", &mut setup_authority)
            .unwrap();

    assert!(changed.is_none());
}

#[test]
fn python_cli_manifest_changed_reconciliation_is_noop_without_node_manifest() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "python-cli".to_string();
    std::fs::write(
        dir.path().join("pyproject.toml"),
        r#"[project]
name = "demo-cli"
version = "0.1.0"
dependencies = ["requests"]
"#,
    )
    .unwrap();
    let mut setup_authority = UltraRunSetupAuthorityState::default();
    setup_authority.grant("phase_setup_step");

    let changed = reconcile_manifest_changed_dependencies_if_needed(
        &cfg,
        "python-cli",
        &mut setup_authority,
    )
    .unwrap();

    assert!(changed.is_none());
}

#[test]
#[cfg(unix)]
fn final_acceptance_applies_deterministic_dependency_repair_before_compile_targeting() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    let contract_path = dir.path().join("completion-contract.json");
    cfg.completion_contract_path = Some(contract_path.clone());
    write_compile_error_fake_npm(dir.path());
    write_nextjs_dual_blocker_workspace(dir.path());
    std::fs::write(
        &contract_path,
        r#"{
  "required_paths": ["src/app/page.tsx"],
  "verify_commands": ["npm run build"],
  "profile": "nextjs",
  "goal": "Create a Next.js route",
  "required_capabilities": [],
  "required_evidence": [],
  "required_obligations": []
}
"#,
    )
    .unwrap();
    let plan =
        UltraPlan::deterministic("Create a Next.js route", "nextjs", "default", "create");
    let mut setup_authority = UltraRunSetupAuthorityState::default();

    let (report, deterministic_remedies) =
        ultra_final_acceptance_report_with_deterministic_remedies(
            &plan,
            &cfg,
            0,
            &mut setup_authority,
        )
        .unwrap();

    assert!(
        deterministic_remedies.contains(&"declared_dependencies_not_ready_install".to_string())
    );
    assert!(
        report
            .compile_errors
            .iter()
            .any(|error| error.message.contains("defined multiple times")),
        "{report:?}"
    );
    assert_eq!(classify_repair_target(&report).as_str(), "implementation");
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains(
        r#""deterministic_remedies_applied":["declared_dependencies_not_ready_install"]"#
    ));
    assert!(event_text.contains(r#""trigger":"declared_dependencies_not_ready""#));
}

#[cfg(unix)]
#[test]
fn ultra_final_acceptance_uses_effective_profile_over_stale_config_profile() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "generic".to_string();
    cfg.eval_events_path = Some(events.clone());
    let port = 3011;
    write_probe_nextjs_workspace(dir.path(), port, &contract_interactive_game_page_source());
    std::fs::write(
        dir.path().join("browser-readiness.json"),
        r#"{"ok":true,"http_status":200,"route_rendered":true}"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("browser-interaction.json"),
        contract_interaction_pass_json(),
    )
    .unwrap();
    let plan = UltraPlan::deterministic(
        &explicit_port_goal("Create an interactive browser game", port),
        "next-js",
        "default",
        "create",
    );

    let report = ultra_final_acceptance_report(&plan, &cfg).unwrap();

    assert!(report.is_pass(), "{report:?}");
    let final_acceptance = latest_event(&events, "ultra_final_acceptance");
    assert_eq!(
        final_acceptance.get("profile").and_then(Value::as_str),
        Some("nextjs")
    );
    assert_eq!(
        final_acceptance
            .get("effective_profile")
            .and_then(Value::as_str),
        Some("nextjs")
    );
    assert_eq!(
        final_acceptance
            .get("browser_readiness_status")
            .and_then(Value::as_str),
        Some("passed")
    );
    assert_eq!(
        final_acceptance
            .get("interaction_evidence_status")
            .and_then(Value::as_str),
        Some("passed")
    );
}

#[test]
fn explicit_generic_profile_does_not_promote_from_workspace_manifest() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "generic".to_string();
    cfg.profile_explicit = true;
    cfg.eval_events_path = Some(events.clone());
    let goal = "Build an interactive memo app with add and delete actions";
    let plan = two_phase_ultra_plan(goal, "generic");
    let mut planner = FakeClient::new(vec![
        AssistantReply::text(single_write_step_plan_json(
            "Create a package manifest",
            "package.json",
        )),
        AssistantReply::text(single_write_step_plan_json(
            "Create generic app source",
            "memo.jsx",
        )),
    ]);
    let mut execution = FakeClient::new(vec![
        AssistantReply {
            content: String::new(),
            tool_calls: vec![crate::state::ToolCall::new(
                "Write",
                serde_json::json!({
                    "path": "package.json",
                    "content": r#"{"dependencies":{"next":"^14.2.0"}}"#
                }),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        },
        AssistantReply {
            content: String::new(),
            tool_calls: vec![crate::state::ToolCall::new(
                "Write",
                serde_json::json!({"path":"memo.jsx","content":generic_interactive_source()}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        },
    ]);

    let result = run_ultra_plan(&mut planner, &mut execution, &plan, &cfg).unwrap();

    assert_eq!(result, "ultra-plan-run complete: 2 phases");
    let event_text = std::fs::read_to_string(&events).unwrap();
    assert!(!event_text.contains("\"event\":\"profile_reinferred\""));
    let phase_two_prompt = planner_request_text(&planner, 1);
    assert!(phase_two_prompt.contains("Profile: generic"));
    assert!(!phase_two_prompt.contains("Profile: nextjs"));
    let final_acceptance = latest_event(&events, "ultra_final_acceptance");
    assert_eq!(
        final_acceptance.get("profile").and_then(Value::as_str),
        Some("generic")
    );
    assert_eq!(
        final_acceptance
            .get("assurance_level")
            .and_then(Value::as_str),
        Some("static")
    );
}

#[test]
fn profile_promotion_occurs_once_and_ignores_later_manifests() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events.clone());
    write_fake_npm_dependency_installer(dir.path());
    let goal = "Build an interactive browser game on port 3011";
    let plan = two_phase_ultra_plan(goal, "generic");
    let mut planner = FakeClient::new(vec![
        AssistantReply::text(single_write_step_plan_json(
            "Create a package manifest",
            "package.json",
        )),
        AssistantReply::text(generated_nextjs_artifact_plan_json(
            "Complete the promoted app and add another manifest",
        )),
    ]);
    let contract_page = contract_interactive_game_page_source();
    let mut final_calls = nextjs_interactive_app_tool_calls(&contract_page);
    final_calls.remove(0);
    final_calls.push(crate::state::ToolCall::new(
        "Write",
        serde_json::json!({"path":"pyproject.toml","content":"[project]\nname = \"late-python\"\nversion = \"0.1.0\"\n"}),
    ));
    final_calls.extend(browser_release_evidence_tool_calls());
    let mut execution = FakeClient::new(vec![
        AssistantReply {
            content: String::new(),
            tool_calls: vec![crate::state::ToolCall::new(
                "Write",
                serde_json::json!({
                    "path": "package.json",
                    "content": nextjs_complete_package_json()
                }),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        },
        AssistantReply {
            content: String::new(),
            tool_calls: final_calls,
            prompt_tokens: None,
            completion_tokens: None,
        },
    ]);

    let result = run_ultra_plan(&mut planner, &mut execution, &plan, &cfg).unwrap();

    assert_eq!(result, "ultra-plan-run complete: 2 phases");
    let events_json = events_with_name(&events, "profile_reinferred");
    assert_eq!(events_json.len(), 1, "{events_json:#?}");
    assert_eq!(
        events_json[0].get("id").and_then(Value::as_str),
        Some("nextjs")
    );
    assert_ne!(
        latest_event(&events, "ultra_final_acceptance")
            .get("profile")
            .and_then(Value::as_str),
        Some("python-cli")
    );
}

#[test]
#[cfg(unix)]
fn promoted_manifest_repair_reconciles_dependencies_before_later_build_verify() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events.clone());
    write_fake_npm_dependency_installer(dir.path());
    let goal = "Build a static product page on port 3011";
    let plan = two_phase_ultra_plan(goal, "generic");
    let mut planner = FakeClient::new(vec![
        AssistantReply::text(single_write_step_plan_json(
            "Create a lean package manifest",
            "package.json",
        )),
        AssistantReply::text(generated_nextjs_artifact_plan_json_with_build_verify(
            "Complete the promoted Next.js product page",
        )),
    ]);
    let page = "export default function Page(){return <main className=\"min-h-screen\"><h1>Product Page</h1><p>Ready on port 3011</p></main>;}\n";
    let mut final_calls = nextjs_interactive_app_tool_calls(page);
    final_calls.remove(0);
    let mut execution = FakeClient::new(vec![
        AssistantReply {
            content: String::new(),
            tool_calls: vec![crate::state::ToolCall::new(
                "Write",
                serde_json::json!({
                    "path": "package.json",
                    "content": nextjs_lean_package_json()
                }),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        },
        AssistantReply {
            content: String::new(),
            tool_calls: final_calls,
            prompt_tokens: None,
            completion_tokens: None,
        },
    ]);

    let result = run_ultra_plan(&mut planner, &mut execution, &plan, &cfg).unwrap();

    assert_eq!(result, "ultra-plan-run complete: 2 phases");
    let package = std::fs::read_to_string(dir.path().join("package.json")).unwrap();
    assert!(package.contains("\"autoprefixer\""), "{package}");
    assert!(dir.path().join("node_modules/autoprefixer").is_dir());
    let reconciliations = events_with_name(&events, "dependency_setup_reconciliation");
    assert!(
        reconciliations.iter().any(|event| {
            event.get("trigger").and_then(Value::as_str) == Some("promotion")
                && event.get("status").and_then(Value::as_str) == Some("passed")
        }),
        "{reconciliations:#?}"
    );
    assert!(
        reconciliations.iter().any(|event| {
            event.get("trigger").and_then(Value::as_str) == Some("manifest_repair")
                && event.get("status").and_then(Value::as_str) == Some("passed")
                && event_array_contains(event, "added", "node_modules/autoprefixer")
        }),
        "{reconciliations:#?}"
    );
    let build_lifecycles = events_with_name(&events, "dependency_build_lifecycle");
    assert!(
        build_lifecycles.iter().any(|event| {
            event.get("step_id").and_then(Value::as_str) == Some("create-nextjs-artifacts")
                && event.get("setup_status").and_then(Value::as_str) == Some("not_required")
                && event.get("final_status").and_then(Value::as_str) == Some("passed")
        }),
        "{build_lifecycles:#?}"
    );
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(!event_text.contains("dependency_setup_authority_required"));
}

#[test]
#[cfg(unix)]
fn side_effect_expected_paths_are_sanitized_before_dependency_lifecycle_setup() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    let setup_plan = serde_json::to_string(&StepPlan {
        goal: "Create package manifest and install dependencies".to_string(),
        steps: vec![PlanStep {
            id: "setup-nextjs".to_string(),
            kind: "setup".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Create package.json and install dependencies for the Next.js app."
                .to_string(),
            expected_paths: vec!["package.json".to_string(), "node_modules".to_string()],
            verify: Vec::new(),
        }],
    })
    .unwrap();
    let mut planner = FakeClient::new(vec![AssistantReply::text(setup_plan)]);
    let generated = generate_step_plan(
        &mut planner,
        "Create package manifest and install dependencies",
        &cfg,
    )
    .unwrap();
    assert_eq!(
        generated.steps[0].expected_paths,
        vec!["package.json".to_string()]
    );
    let drops = events_with_name(&events, "side_effect_path_dropped");
    assert!(
        drops.iter().any(|event| {
            event.get("path").and_then(Value::as_str) == Some("node_modules")
                && event.get("tier").and_then(Value::as_str) == Some("unambiguous")
        }),
        "{drops:#?}"
    );
    std::fs::write(
        dir.path().join("package.json"),
        nextjs_complete_package_json(),
    )
    .unwrap();
    let fake_npm = dir.path().join("fake-npm.sh");
    std::fs::write(
        &fake_npm,
        "#!/bin/sh\nmkdir -p node_modules/next node_modules/react node_modules/react-dom\necho '{\"version\":\"14.2.0\"}' > node_modules/next/package.json\ntouch package-lock.json\nexit 0\n",
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&fake_npm).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    std::fs::set_permissions(&fake_npm, permissions).unwrap();
    let (dependency_report, build_lifecycles) =
        verify_setup_dependency_state_with_setup_observed_with_options(
            dir.path(),
            NodeDependencySetupAuthority::PlanSetupStep,
            &fake_npm,
            false,
        );

    assert!(dependency_report.is_pass(), "{dependency_report:?}");
    assert!(dir.path().join("node_modules/next").is_dir());
    assert!(dir.path().join("package-lock.json").is_file());
    assert!(
        build_lifecycles.iter().any(|event| {
            event.setup_status() == "passed"
                && event.setup.as_ref().is_some_and(|setup| {
                    setup
                        .changed_paths
                        .iter()
                        .any(|path| path == "node_modules")
                })
        }),
        "{build_lifecycles:#?}"
    );
}

#[test]
fn plan_run_without_setup_step_keeps_dependency_setup_authority_none() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    write_nextjs_profile_workspace(
        dir.path(),
        Some(nextjs_globals_css()),
        Some(nextjs_postcss_config()),
        Some(nextjs_tsconfig_json()),
    );
    let plan = StepPlan {
        goal: "Verify Next.js app without setup authority".to_string(),
        steps: vec![PlanStep {
            id: "plain-build".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Verify the existing app build".to_string(),
            expected_paths: vec!["src/app/page.tsx".to_string()],
            verify: vec!["npm run build".to_string()],
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
        AssistantReply::text("No source changes needed."),
    ]);

    let err = run_step_plan(&mut fake, &plan, &cfg)
        .unwrap_err()
        .to_string();

    assert!(err.contains("dependency_setup_authority_required"), "{err}");
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("\"setup_authority\":\"none\""));
    assert!(!event_text.contains("\"event\":\"dependency_setup_reconciliation\""));
}

#[test]
fn offline_promotion_dependency_reconciliation_stops_honestly() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events.clone());
    cfg.offline = true;
    let goal = "Build an interactive browser game on port 3011";
    let plan = two_phase_ultra_plan(goal, "generic");
    let mut planner = FakeClient::new(vec![AssistantReply::text(single_write_step_plan_json(
        "Create a package manifest",
        "package.json",
    ))]);
    let mut execution = FakeClient::new(vec![AssistantReply {
        content: String::new(),
        tool_calls: vec![crate::state::ToolCall::new(
            "Write",
            serde_json::json!({
                "path": "package.json",
                "content": nextjs_lean_package_json()
            }),
        )],
        prompt_tokens: None,
        completion_tokens: None,
    }]);

    let err = run_ultra_plan(&mut planner, &mut execution, &plan, &cfg)
        .unwrap_err()
        .to_string();

    assert!(err.contains("dependency_setup_blocked_offline"), "{err}");
    let reconciliations = events_with_name(&events, "dependency_setup_reconciliation");
    assert!(
        reconciliations.iter().any(|event| {
            event.get("trigger").and_then(Value::as_str) == Some("promotion")
                && event.get("status").and_then(Value::as_str) == Some("blocked")
                && event.get("primary_reason").and_then(Value::as_str)
                    == Some("dependency_setup_blocked_offline")
        }),
        "{reconciliations:#?}"
    );
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(!event_text.contains("\"event\":\"ultra_plan_complete\""));
}

#[test]
fn known_profile_run_never_reinfers_profile() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    write_fake_npm_dependency_installer(dir.path());
    let goal = "Build an interactive browser game on port 3011";
    let plan = two_phase_ultra_plan(goal, "nextjs");
    let mut planner = FakeClient::new(vec![
        AssistantReply::text(generated_nextjs_artifact_plan_json(
            "Create the Next.js app",
        )),
        AssistantReply::text(generated_nextjs_artifact_plan_json(
            "Finish the Next.js app",
        )),
    ]);
    let contract_page = contract_interactive_game_page_source();
    let mut final_calls = nextjs_interactive_app_tool_calls(&contract_page);
    final_calls.push(crate::state::ToolCall::new(
        "Write",
        serde_json::json!({"path":"pyproject.toml","content":"[project]\nname = \"ignored\"\nversion = \"0.1.0\"\n"}),
    ));
    final_calls.extend(browser_release_evidence_tool_calls());
    let mut execution = FakeClient::new(vec![
        AssistantReply {
            content: String::new(),
            tool_calls: nextjs_interactive_app_tool_calls(interactive_game_page_source()),
            prompt_tokens: None,
            completion_tokens: None,
        },
        AssistantReply {
            content: String::new(),
            tool_calls: final_calls,
            prompt_tokens: None,
            completion_tokens: None,
        },
    ]);

    let result = run_ultra_plan(&mut planner, &mut execution, &plan, &cfg).unwrap();

    assert_eq!(result, "ultra-plan-run complete: 2 phases");
    let event_text = std::fs::read_to_string(&events).unwrap();
    assert!(!event_text.contains("\"event\":\"profile_reinferred\""));
    let final_acceptance = latest_event(&events, "ultra_final_acceptance");
    assert_eq!(
        final_acceptance.get("profile").and_then(Value::as_str),
        Some("nextjs")
    );
    assert_eq!(
        final_acceptance
            .get("assurance_level")
            .and_then(Value::as_str),
        Some("full")
    );
}

#[test]
fn plan_adherence_missing_tokens_do_not_change_acceptance_outcome() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "generic".to_string();
    std::fs::create_dir_all(dir.path().join("src/app")).unwrap();
    std::fs::write(
        dir.path().join("src/app/page.tsx"),
        "export default function Page(){ return <main><p>Ready</p></main>; }",
    )
    .unwrap();
    let with_adherence_drift = UltraPlan {
        goal: "Build a simple page".to_string(),
        profile: "generic".to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: vec![UltraPhase {
            id: "final".to_string(),
            prompt: "Implement keyboard controls, lives counter, and pause mode".to_string(),
        }],
    };
    let without_adherence_drift = UltraPlan {
        phases: vec![UltraPhase {
            id: "final".to_string(),
            prompt: "Finalize the page".to_string(),
        }],
        ..with_adherence_drift.clone()
    };

    let with_report = ultra_final_acceptance_report(&with_adherence_drift, &cfg).unwrap();
    let without_report = ultra_final_acceptance_report(&without_adherence_drift, &cfg).unwrap();

    assert_eq!(with_report, without_report);
    assert!(with_report.is_pass(), "{with_report:?}");
}

#[test]
fn non_ultra_plan_run_still_enforces_completion_contract() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events.clone());
    cfg.completion_contract_path = Some(write_challenge_contract(dir.path()));
    let plan = StepPlan {
        goal: "Create the page".to_string(),
        steps: vec![PlanStep {
            id: "page".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Create src/app/page.tsx".to_string(),
            expected_paths: vec!["src/app/page.tsx".to_string()],
            verify: Vec::new(),
        }],
    };
    let mut fake = FakeClient::new(vec![AssistantReply {
        content: String::new(),
        tool_calls: vec![crate::state::ToolCall::new(
            "Write",
            serde_json::json!({
                "path":"src/app/page.tsx",
                "content":"export default function Page(){ return <main><canvas>ready</canvas></main>; }",
            }),
        )],
        prompt_tokens: None,
        completion_tokens: None,
    }]);

    let err = run_step_plan(&mut fake, &plan, &cfg)
        .unwrap_err()
        .to_string();

    assert!(err.contains("completion contract verify failed"), "{err}");
    assert!(err.contains("challenge_or_adversary_evidence"), "{err}");
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("\"contract_enforcement\":\"enforce\""));
    assert!(!event_text.contains("\"event\":\"contract_observation_incomplete\""));
}

#[test]
fn exhaustion_with_pending_contract_state_names_capability_keys() {
    let reason = exhaustion_reason_with_pending_contract_state(
        "loop_progress_exhausted: no concrete blocker recorded",
        &["restart_or_recoverable_state_evidence".to_string()],
    );

    assert_eq!(
        reason,
        "capability_evidence_unresolved:restart_or_recoverable_state_evidence"
    );
}

#[test]
fn restart_hook_attachment_guidance_cites_route_bound_file_line() {
    let dir = tempfile::tempdir().unwrap();
    let app = dir.path().join("src/app");
    std::fs::create_dir_all(&app).unwrap();
    std::fs::write(
        app.join("page.tsx"),
        r#""use client";
export default function Page() {
  const restartGame = () => {};
  return <main>
<button onClick={restartGame}>TRY AGAIN</button>
  </main>;
}
"#,
    )
    .unwrap();

    let guidance = restart_hook_attachment_guidance(dir.path(), "nextjs").join("\n");

    assert!(
        guidance.contains("add data-anvil-action=\"restart\""),
        "{guidance}"
    );
    assert!(guidance.contains("the TRY AGAIN button"), "{guidance}");
    assert!(guidance.contains("src/app/page.tsx:5"), "{guidance}");
}

#[test]
fn capability_evidence_failure_evidence_leads_with_remedies() {
    let dir = tempfile::tempdir().unwrap();
    let app = dir.path().join("src/app");
    std::fs::create_dir_all(&app).unwrap();
    std::fs::write(
        app.join("page.tsx"),
        r#""use client";
export default function Page() {
  const playAgain = () => {};
  return <button onClick={playAgain}>PLAY AGAIN</button>;
}
"#,
    )
    .unwrap();
    let evidence = capability_evidence_failure_evidence(
        dir.path(),
        "nextjs",
        &["restart_or_recoverable_state_evidence".to_string()],
        "capability_evidence_unresolved:restart_or_recoverable_state_evidence",
    );

    assert!(
        evidence
            .first()
            .is_some_and(|line| line.contains("data-anvil-action=\"restart\"")),
        "{evidence:#?}"
    );
    assert!(
        evidence
            .iter()
            .any(|line| line.contains("src/app/page.tsx:4")),
        "{evidence:#?}"
    );
    assert!(
        evidence
            .iter()
            .any(|line| line.contains("exhaustion classification")),
        "{evidence:#?}"
    );
}

#[test]
fn session_error_text_missing_evidence_populates_step_outcome() {
    let err = anyhow::anyhow!(
        "completion contract verify failed after 1 attempts: \
         missing_required_capabilities:stateful_interaction \
         missing_required_evidence:challenge_or_adversary_evidence"
    );

    let outcome = step_run_outcome_from_session_error(&err, "initial_turn_error");

    assert_eq!(
        outcome.observed_missing_capabilities,
        vec!["stateful_interaction".to_string()]
    );
    assert_eq!(
        outcome.observed_missing_evidence,
        vec!["challenge_or_adversary_evidence".to_string()]
    );
    assert_eq!(
        outcome.repair_targets,
        vec!["required_evidence_missing".to_string()]
    );
    assert_eq!(outcome.stop_reason.as_deref(), Some("initial_turn_error"));
    assert!(outcome.partial);
}

#[test]
fn final_phase_profile_dependency_repair_runs_in_final_acceptance() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events.clone());
    let scaffold_plan = generated_nextjs_fixture_plan_json_with_kind(
        "Scaffold interactive app",
        "check_scaffold.py",
        "setup",
    );
    let finish_plan = generated_nextjs_fixture_plan_json_with_kind(
        "Create interactive app",
        "check_finish.py",
        "setup",
    );
    let mut planner = FakeClient::new(vec![
        AssistantReply::text(scaffold_plan),
        AssistantReply::text(finish_plan.clone()),
        AssistantReply::text(finish_plan),
    ]);
    let package = nextjs_complete_package_json();
    let bad_package =
        r#"{"dependencies":{},"scripts":{"build":"next build","dev":"next dev -p 3011"}}"#;
    let mut first_phase_calls =
        nextjs_interactive_app_tool_calls(interactive_game_page_source());
    first_phase_calls.push(crate::state::ToolCall::new(
        "Write",
        serde_json::json!({"path":"package.json","content":package}),
    ));
    first_phase_calls.push(crate::state::ToolCall::new(
        "Write",
        serde_json::json!({"path":"check_scaffold.py","content":"x = 1\n"}),
    ));
    let mut execution = FakeClient::new(vec![
        AssistantReply {
            content: String::new(),
            tool_calls: first_phase_calls,
            prompt_tokens: None,
            completion_tokens: None,
        },
        AssistantReply {
            content: String::new(),
            tool_calls: vec![
                crate::state::ToolCall::new(
                    "Write",
                    serde_json::json!({"path":"package.json","content":bad_package}),
                ),
                crate::state::ToolCall::new(
                    "Write",
                    serde_json::json!({"path":"check_finish.py","content":"x = 2\n"}),
                ),
            ],
            prompt_tokens: None,
            completion_tokens: None,
        },
        AssistantReply {
            content: String::new(),
            tool_calls: vec![
                crate::state::ToolCall::new(
                    "Write",
                    serde_json::json!({"path":"README.md","content":"# Recovery note\nThe scaffold exists but implementation still needs task-specific gameplay."}),
                ),
                crate::state::ToolCall::new(
                    "Write",
                    serde_json::json!({"path":"package.json","content":package}),
                ),
            ],
            prompt_tokens: None,
            completion_tokens: None,
        },
    ]);
    let plan = UltraPlan {
        goal: "Create an interactive browser game".to_string(),
        profile: "nextjs".to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: vec![
            crate::planner::ultra_plan::UltraPhase {
                id: "scaffold".to_string(),
                prompt: "Scaffold the app".to_string(),
            },
            crate::planner::ultra_plan::UltraPhase {
                id: "finish".to_string(),
                prompt: "Finish the interactive app".to_string(),
            },
        ],
    };
    let result = run_ultra_plan(&mut planner, &mut execution, &plan, &cfg).unwrap();
    assert_eq!(result, "ultra-plan-run complete: 2 phases");
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("\"event\":\"ultra_phase_complete\""));
    assert!(event_text.contains("\"phase_id\":\"finish\""));
    assert!(event_text.contains("\"event\":\"ultra_final_acceptance_failed\""));
    assert!(event_text.contains("dependency missing: next"));
    assert!(event_text.contains("\"event\":\"final_acceptance_repair_start\""));
    assert!(event_text.contains("\"event\":\"final_acceptance_repair_complete\""));
    assert!(!event_text.contains("\"event\":\"profile_repair_complete\""));
    assert!(event_text.contains("\"event\":\"ultra_plan_complete\""));
}

#[test]
fn profile_invariant_handoff_uses_final_import_evidence_not_stale_postcss() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events);
    write_nextjs_profile_workspace(dir.path(), None, Some(nextjs_postcss_config()), None);
    let plan = UltraPlan {
        goal: "3011 port app".to_string(),
        profile: "nextjs".to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: vec![crate::planner::ultra_plan::UltraPhase {
            id: "scaffold".to_string(),
            prompt: "Scaffold project".to_string(),
        }],
    };
    let phase = &plan.phases[0];
    let evidence = fresh_profile_invariant_failure_evidence(
        &cfg,
        &plan,
        &ProfileSnapshot::None,
        &nextjs_scaffold_expected_paths(),
    );
    let reason = evidence.report.primary_reason();

    assert!(reason.contains("missing relative imports"), "{reason}");
    assert!(reason.contains("src/app/globals.css"), "{reason}");
    assert!(!reason.contains("PostCSS config file missing"), "{reason}");
    assert_eq!(
        reason,
        verify_profile_invariant(dir.path(), "nextjs", &plan.goal, &ProfileSnapshot::None)
            .primary_reason()
    );
    assert!(
        evidence
            .missing_paths
            .contains(&"src/app/globals.css".to_string()),
        "{:?}",
        evidence.missing_paths
    );

    let prompt = profile_invariant_model_repair_prompt(
        &plan,
        phase,
        &evidence.report,
        &UltraRunContext::default(),
        &nextjs_scaffold_expected_paths(),
        &cfg,
        None,
    );
    assert!(
        prompt.contains(
            "src/app/layout.tsx imports ./globals.css which does not exist - create src/app/globals.css"
        ),
        "{prompt}"
    );

    let _handoff = save_ultra_phase_recovery_handoff_with_evidence(
        &cfg,
        &plan,
        phase,
        UltraPhaseRecoveryRequest {
            failure_kind: "profile_invariant_failure",
            reason: &reason,
            missing_paths: &evidence.missing_paths,
            missing_signals: &[],
            repair_targets: &["profile_contract".to_string()],
            verify_commands: &[],
        },
        &evidence.failure_evidence,
    );
    let repair_text = std::fs::read_dir(dir.path().join(".commandagent/repairs"))
        .unwrap()
        .map(|entry| std::fs::read_to_string(entry.unwrap().path()).unwrap())
        .find(|text| text.contains("profile_invariant_failure"))
        .expect("profile invariant recovery prompt");
    assert!(
        repair_text.contains("Missing paths:\n- src/app/globals.css"),
        "{repair_text}"
    );
    assert!(
        repair_text.contains("src/app/layout.tsx imports ./globals.css which does not exist"),
        "{repair_text}"
    );
    assert!(
        !repair_text.contains("PostCSS config file missing"),
        "{repair_text}"
    );
}

#[test]
fn profile_invariant_fresh_evidence_reason_matches_final_reverification() {
    for outcome in [
        "valid-postcss-missing-globals",
        "missing-postcss",
        "bad-tsconfig",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let cfg = config(dir.path().to_path_buf());
        let postcss = (outcome != "missing-postcss").then_some(nextjs_postcss_config());
        let tsconfig = (outcome == "bad-tsconfig")
            .then_some("{\"compilerOptions\":{\"rootDir\":\"src\"}}\n");
        let globals =
            (outcome != "valid-postcss-missing-globals").then_some(nextjs_globals_css());
        write_nextjs_profile_workspace(dir.path(), globals, postcss, tsconfig);
        let plan = UltraPlan {
            goal: "3011 port app".to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            intent: "create".to_string(),
            phases: vec![crate::planner::ultra_plan::UltraPhase {
                id: outcome.to_string(),
                prompt: "Simulated repair outcome".to_string(),
            }],
        };

        let evidence = fresh_profile_invariant_failure_evidence(
            &cfg,
            &plan,
            &ProfileSnapshot::None,
            &nextjs_scaffold_expected_paths(),
        );
        let reverified =
            verify_profile_invariant(dir.path(), "nextjs", &plan.goal, &ProfileSnapshot::None);

        assert_eq!(
            evidence.report.primary_reason(),
            reverified.primary_reason(),
            "outcome {outcome}"
        );
    }
}

#[test]
fn plan_run_final_contract_fails_when_required_final_artifact_missing() {
    let dir = tempfile::tempdir().unwrap();
    let plan = StepPlan::single("Update docs\n\nRequired final artifacts:\n- README.md");
    let mut fake = FakeClient::new(vec![]);
    let err = run_step_plan(&mut fake, &plan, &config(dir.path().to_path_buf()))
        .unwrap_err()
        .to_string();
    assert!(err.contains("plan final contract failed"));
    assert!(err.contains("README.md"));
}

#[test]
fn plan_run_final_contract_passes_after_step_artifacts_created() {
    let dir = tempfile::tempdir().unwrap();
    let plan = StepPlan {
        goal: "Create a.txt\n\nRequired final artifacts:\n- a.txt".to_string(),
        steps: vec![PlanStep {
            id: "code".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Create a.txt".to_string(),
            expected_paths: vec!["a.txt".to_string()],
            verify: Vec::new(),
        }],
    };
    let mut fake = FakeClient::new(vec![AssistantReply {
        content: String::new(),
        tool_calls: vec![crate::state::ToolCall::new(
            "Write",
            serde_json::json!({"path":"a.txt","content":"ok"}),
        )],
        prompt_tokens: None,
        completion_tokens: None,
    }]);
    let result = run_step_plan(&mut fake, &plan, &config(dir.path().to_path_buf())).unwrap();
    assert_eq!(result, "plan-run complete: 1 steps");
    assert!(dir.path().join("a.txt").is_file());
}

#[test]
fn plan_run_nextjs_game_setup_only_fails_inferred_obligation() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    let plan = StepPlan {
        goal: "Create an interactive browser game\n\nRequired final artifacts:\n- package.json"
            .to_string(),
        steps: vec![PlanStep {
            id: "setup".to_string(),
            kind: "setup".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Create package.json".to_string(),
            expected_paths: vec!["package.json".to_string()],
            verify: Vec::new(),
        }],
    };
    let mut fake = FakeClient::new(vec![AssistantReply {
        content: String::new(),
        tool_calls: vec![crate::state::ToolCall::new(
            "Write",
            serde_json::json!({"path":"package.json","content":"{\"scripts\":{\"build\":\"next build\"}}"}),
        )],
        prompt_tokens: None,
        completion_tokens: None,
    }]);
    let err = run_step_plan(&mut fake, &plan, &cfg)
        .unwrap_err()
        .to_string();
    assert!(err.contains("plan final contract failed"), "{err}");
    assert!(err.contains("missing_required_evidence"), "{err}");
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("\"required_obligations\":[\"implementation\"]"));
    assert!(event_text.contains("\"missing_obligations\":[\"implementation\"]"));
    assert!(event_text.contains("\"obligation_repair_targets\""));
    assert!(event_text.contains("\"target_path\":\"src/app/page.tsx\""));
}

#[test]
fn plan_run_nextjs_game_scaffold_only_fails_inferred_capabilities() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    let plan = StepPlan {
        goal: "Create an interactive browser game\n\nRequired final artifacts:\n- src/app/page.tsx"
            .to_string(),
        steps: vec![PlanStep {
            id: "page".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Create src/app/page.tsx".to_string(),
            expected_paths: vec!["src/app/page.tsx".to_string()],
            verify: Vec::new(),
        }],
    };
    let mut fake = FakeClient::new(vec![
        AssistantReply {
            content: String::new(),
            tool_calls: vec![crate::state::ToolCall::new(
                "Write",
                serde_json::json!({"path":"src/app/page.tsx","content":"export default function Page(){ return <main>Press any key to start</main>; }"}),
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
    assert!(event_text.contains("\"runtime_acceptance_inconclusive\":false"));
    assert!(event_text.contains("\"missing_evidence\""));
    assert!(event_text.contains("\"capability_evidence_bindings\""));
    assert!(event_text.contains("\"role\":\"scaffold\""));
    assert!(event_text.contains("\"artifact_paths\":[]"));
    assert!(event_text.contains("\"event\":\"step_obligation_scope\""));
    assert!(event_text.contains("\"step_kind\":\"implement\""));
    assert!(event_text.contains("\"completion_contract_path_merge_enabled\":true"));
    assert!(event_text.contains("\"completion_contract_verification_enabled\":true"));
    assert!(event_text.contains("\"contract_paths_merged\":true"));
    assert!(event_text.contains("\"event\":\"completion_contract_bound\""));
    assert!(event_text.contains("\"session_scope\":\"plan-run\""));
    assert!(event_text.contains("\"completion_contract_verification_enabled\":true"));
    assert!(event_text.contains("\"external_contract_checked\":true"));
    assert!(event_text.contains("\"completion_contract_generated\":true"));
    assert!(event_text.contains("\"step_prompt_contract\""));
    assert!(event_text.contains("\"has_required_final_evidence\":true"));
    assert!(event_text.contains("\"visible_interactive_surface_evidence\""));
    assert!(event_text.contains("\"user_input_handler_evidence\""));
    assert!(event_text.contains("\"restart_or_recoverable_state_evidence\""));
}

#[test]
fn plan_run_nextjs_game_docs_only_fails_inferred_capabilities() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    let plan = StepPlan {
        goal: "Create an interactive browser game\n\nRequired final artifacts:\n- README.md"
            .to_string(),
        steps: vec![PlanStep {
            id: "docs".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Create README.md".to_string(),
            expected_paths: vec!["README.md".to_string()],
            verify: Vec::new(),
        }],
    };
    let mut fake = FakeClient::new(vec![
        AssistantReply {
            content: String::new(),
            tool_calls: vec![crate::state::ToolCall::new(
                "Write",
                serde_json::json!({"path":"README.md","content":"# Game\nUse arrow keys."}),
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
    assert!(event_text.contains("\"missing_evidence\""));
    assert!(event_text.contains("\"role\":\"acceptance_evidence\""));
}
