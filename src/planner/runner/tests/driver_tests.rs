#[test]
fn plan_artifact_saved() {
    let dir = tempfile::tempdir().unwrap();
    let plan = StepPlan::single("goal");
    let path = save_step_plan(dir.path(), &plan).unwrap();
    assert!(path.exists());
}

#[test]
fn implement_contract_setup_authority_requires_setup_purpose_step_or_phase() {
    let implement = PlanStep {
        id: "app".to_string(),
        kind: "implement".to_string(),
        expected_result: "pass".to_string(),
        instruction: "Build the app".to_string(),
        expected_paths: Vec::new(),
        verify: Vec::new(),
    };
    let setup_implement = PlanStep {
        id: "workspace-and-dependencies-setup".to_string(),
        kind: "implement".to_string(),
        expected_result: "pass".to_string(),
        instruction: "Install dependencies".to_string(),
        expected_paths: Vec::new(),
        verify: Vec::new(),
    };
    let without_setup = StepPlan {
        goal: "Build app".to_string(),
        steps: vec![implement.clone()],
    };
    let with_setup_implement = StepPlan {
        goal: "Build app".to_string(),
        steps: vec![setup_implement.clone()],
    };

    assert_eq!(
        step_contract_setup_authority(
            &without_setup,
            &implement,
            None,
            NodeDependencySetupAuthority::None
        ),
        NodeDependencySetupAuthority::None
    );
    assert_eq!(
        step_contract_setup_authority(
            &with_setup_implement,
            &setup_implement,
            None,
            NodeDependencySetupAuthority::None
        ),
        NodeDependencySetupAuthority::PlanSetupStep
    );
    assert_eq!(
        step_contract_setup_authority(
            &without_setup,
            &implement,
            Some("workspace-and-dependencies-setup"),
            NodeDependencySetupAuthority::None,
        ),
        NodeDependencySetupAuthority::PlanSetupStep
    );
}

#[test]
fn run_plan_accepts_absolute_path_inside_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let plan = StepPlan::single("goal");
    let path = save_step_plan(dir.path(), &plan).unwrap();
    let mut fake = FakeClient::new(vec![AssistantReply::text("done")]);
    let result = run_plan_file(&mut fake, &path, &config(dir.path().to_path_buf())).unwrap();
    assert_eq!(result, "plan-run complete: 1 steps");
}

#[test]
fn verify_step_short_circuits_when_expected_path_and_verify_already_pass() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    std::fs::write(dir.path().join("done.txt"), "ok").unwrap();
    let plan = StepPlan {
        goal: "Verify existing artifact".to_string(),
        steps: vec![PlanStep {
            id: "verify-existing".to_string(),
            kind: "verify".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Verify done.txt exists".to_string(),
            expected_paths: vec!["done.txt".to_string()],
            verify: vec!["test -f done.txt".to_string()],
        }],
    };
    let path = save_step_plan(dir.path(), &plan).unwrap();
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events.clone());
    let mut fake = FakeClient::new(Vec::new());

    let result = run_plan_file(&mut fake, &path, &cfg).unwrap();

    assert_eq!(result, "plan-run complete: 1 steps");
    assert_eq!(fake.messages().len(), 0);
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("\"event\":\"step_short_circuited\""));
    assert!(event_text.contains("\"at\":\"start\""));
    assert!(event_text.contains("\"step_id\":\"verify-existing\""));
}

#[test]
fn run_plan_path_confinement_rejects_absolute_escape() {
    let workspace = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let path = outside.path().join("plan.yaml");
    std::fs::write(&path, render_step_plan(&StepPlan::single("goal"))).unwrap();
    let mut fake = FakeClient::new(vec![AssistantReply::text("done")]);
    let err = run_plan_file(&mut fake, &path, &config(workspace.path().to_path_buf()))
        .unwrap_err()
        .to_string();
    assert!(err.contains("escapes workspace"));
}

#[test]
fn run_plan_rejects_invalid_yaml_without_repair() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.yaml");
    std::fs::write(
        &path,
        r#"steps:
  - id: "s1"
instruction: "do it"
"#,
    )
    .unwrap();
    let mut fake = FakeClient::new(vec![AssistantReply::text("done")]);
    let err = run_plan_file(&mut fake, &path, &config(dir.path().to_path_buf()))
        .unwrap_err()
        .to_string();
    assert!(err.contains("StepPlan missing goal"));
}

#[test]
fn source_generated_json_saves_yaml_readable_by_run_plan() {
    let dir = tempfile::tempdir().unwrap();
    let json = include_str!("../../../../eval/fixtures/plans/source-step-plan.json");
    let plan = parse_generated_step_plan_json(
        json,
        "Create a Next.js Space Invaders app on port 3011.",
    )
    .unwrap();
    let path = save_step_plan(dir.path(), &plan).unwrap();
    let parsed = parse_step_plan(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(parsed, plan);
    assert!(parsed.steps.iter().any(|step| step.kind == "implement"));
    assert!(parsed.steps.iter().any(|step| step.kind == "verify"));
}

#[test]
fn run_plan_accepts_existing_and_generated_yaml() {
    let existing = include_str!("../../../../eval/fixtures/plans/existing-mvp-step-plan.yaml");
    let parsed_existing = parse_step_plan(existing).unwrap();
    assert_eq!(
        parsed_existing.goal,
        "Create a small markdown heading linter."
    );

    let generated = include_str!("../../../../eval/fixtures/plans/source-step-plan.expected.yaml");
    let parsed_generated = parse_step_plan(generated).unwrap();
    assert_eq!(
        parsed_generated.goal,
        "Create a Next.js Space Invaders app on port 3011."
    );
    assert_eq!(parsed_generated.steps.len(), 5);
}

#[test]
fn invalid_planner_output_gets_corrective_retry() {
    let dir = tempfile::tempdir().unwrap();
    let valid = generated_step_plan_json("goal");
    let mut planner = FakeClient::new(vec![
        AssistantReply::text("not json"),
        AssistantReply::text(valid),
    ]);
    let plan =
        generate_step_plan(&mut planner, "goal", &config(dir.path().to_path_buf())).unwrap();
    assert_eq!(plan.goal, "goal");
    assert_eq!(plan.steps.len(), 1);
}

#[test]
fn empty_planner_output_uses_compact_ladder_then_accepts_valid_plan() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events.clone());
    let valid = generated_step_plan_json("goal");
    let mut planner = FakeClient::new(vec![
        AssistantReply::text(""),
        AssistantReply::text(""),
        AssistantReply::text(valid),
    ]);

    let plan = generate_step_plan(&mut planner, "goal", &cfg).unwrap();

    assert_eq!(plan.goal, "goal");
    assert_eq!(planner.messages().len(), 3);
    let compact_prompt = planner.messages()[1]
        .iter()
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(compact_prompt.contains("Compact StepPlan recovery"));
    assert!(compact_prompt.contains("Required JSON shape"));
    assert!(compact_prompt.contains("Minimal phase context"));
    let fresh_prompt = planner.messages()[2]
        .iter()
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(fresh_prompt.contains("Compact StepPlan recovery"));
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("\"planner_session_mode\":\"standard\""));
    assert!(event_text.contains("\"planner_session_mode\":\"compact_retry\""));
    assert!(event_text.contains("\"planner_session_mode\":\"fresh_compact\""));
    assert!(event_text.contains("\"content_len\":0"));
    assert!(event_text.contains("\"planner_error_kind\":\"planner_empty_response\""));
}

#[test]
fn all_empty_planner_output_reports_precise_classification() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events.clone());
    let mut planner = FakeClient::new(vec![
        AssistantReply::text(""),
        AssistantReply::text(""),
        AssistantReply::text(""),
    ]);

    let err = generate_step_plan(&mut planner, "goal", &cfg)
        .unwrap_err()
        .to_string();

    assert!(err.contains("planner_empty_response"), "{err}");
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("\"planner_error_kind\":\"planner_empty_response\""));
    assert!(
        !event_text.contains("\"planner_error_kind\":\"planner_schema_error\""),
        "{event_text}"
    );
}

#[test]
fn missing_goal_gets_corrective_retry_and_recorded() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events.clone());
    let mut planner = FakeClient::new(vec![
        AssistantReply::text(r#"{"steps":[{"id":"s1","instruction":"Create file"}]}"#),
        AssistantReply::text(generated_step_plan_json("goal")),
    ]);
    let plan = generate_step_plan(&mut planner, "goal", &cfg).unwrap();
    assert_eq!(plan.goal, "goal");
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("planner_error"));
    assert!(event_text.contains("planner_raw_output_shape"));
}

#[test]
fn missing_descriptive_expected_result_is_defaulted_and_recorded() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events.clone());
    let generated = r#"{
      "goal": "Implement the core Space Invaders game engine within app/page.tsx using an HTML5 canvas.",
      "steps": [
        {
          "id": "implement-game-engine",
          "kind": "implement",
          "instruction": "Create `src/app/page.tsx` as a route-bound Space Invaders game engine with a canvas render loop, keyboard input, synchronized enemy grid, projectile firing, collision detection, score tracking, lives, and game-over state.",
          "expected_paths": ["src/app/page.tsx"],
          "verify": ["test -f src/app/page.tsx"]
        }
      ]
    }"#;
    let mut planner = FakeClient::new(vec![AssistantReply::text(generated)]);

    let plan = generate_step_plan(&mut planner, "goal", &cfg).unwrap();

    assert_eq!(plan.steps[0].expected_result, "pass");
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("\"event\":\"planner_plan_sanitized\""));
    assert!(event_text.contains("\"kind\":\"schema_field_defaulted\""));
    assert!(event_text.contains("\"field\":\"expected_result\""));
    assert!(!event_text.contains("\"event\":\"planner_error\""));
}

#[test]
fn verify_policy_error_gets_corrective_retry() {
    let dir = tempfile::tempdir().unwrap();
    let invalid = r#"{"goal":"goal","steps":[{"id":"s1","kind":"implement","expected_result":"pass","instruction":"Create app","expected_paths":["package.json"],"verify":["node check.js | node check2.js"]}]}"#;
    let valid = r#"{"goal":"goal","steps":[{"id":"s1","kind":"implement","expected_result":"pass","instruction":"Create app","expected_paths":["package.json"],"verify":["node check.js","node check2.js"]}]}"#;
    let mut planner = FakeClient::new(vec![
        AssistantReply::text(invalid),
        AssistantReply::text(valid),
    ]);
    let plan =
        generate_step_plan(&mut planner, "goal", &config(dir.path().to_path_buf())).unwrap();
    assert_eq!(
        plan.steps[0].verify,
        vec!["node check.js".to_string(), "node check2.js".to_string()]
    );
}

#[test]
fn safe_and_verify_policy_is_normalized_without_corrective_retry() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("node_modules")).unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events.clone());
    let generated = r#"{"goal":"goal","steps":[{"id":"s1","kind":"implement","expected_result":"pass","instruction":"Create package.json for the app","expected_paths":["package.json"],"verify":["npm test && test -f package.json"]}]}"#;
    let mut planner = FakeClient::new(vec![AssistantReply::text(generated)]);
    let plan = generate_step_plan(&mut planner, "goal", &cfg).unwrap();
    assert_eq!(
        plan.steps[0].verify,
        vec!["npm test".to_string(), "test -f package.json".to_string()]
    );
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("planner_verify_command_normalized"));
    assert!(event_text.contains("\"normalization_source\":\"deterministic_verify_policy\""));
    assert!(event_text.contains("\"original_command_hash\""));
    assert!(
        event_text
            .contains("\"original_command_summary\":\"npm test && test -f package.json\"")
    );
    assert!(
        event_text.contains("\"normalized_commands\":[\"npm test\",\"test -f package.json\"]")
    );
    assert!(!event_text.contains("\"event\":\"planner_error\""));
}

#[test]
fn status_echo_verify_is_normalized_before_plan_time_policy_diagnosis() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events.clone());
    let generated = r#"{"goal":"goal","steps":[{"id":"s1","kind":"implement","expected_result":"pass","instruction":"Create src/app/page.tsx for the app route.","expected_paths":["src/app/page.tsx"],"verify":["test -f src/app/page.tsx && echo \"pass\" || echo \"fail\""]}]}"#;
    let mut planner = FakeClient::new(vec![AssistantReply::text(generated)]);

    let plan = generate_step_plan(&mut planner, "goal", &cfg).unwrap();

    assert_eq!(plan.steps[0].verify, vec!["test -f src/app/page.tsx"]);
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("\"event\":\"planner_verify_command_normalized\""));
    assert!(event_text.contains("\"normalized_commands\":[\"test -f src/app/page.tsx\"]"));
    assert!(!event_text.contains("\"event\":\"planner_error\""));
}

#[test]
fn setup_phase_lint_exhaustion_uses_known_profile_fallback_plan() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.profile_explicit = true;
    cfg.eval_events_path = Some(events.clone());
    let goal = "Phase id: project-setup\nPhase task: Scaffold a Next.js application with Tailwind CSS configuration and port 3011 scripts.";
    let invalid = r#"{
      "goal": "Phase id: project-setup\nPhase task: Scaffold a Next.js application with Tailwind CSS configuration and port 3011 scripts.",
      "steps": [
        {
          "id": "setup-nextjs",
          "kind": "setup",
          "expected_result": "pass",
          "instruction": "Create the package and Tailwind scaffold.",
          "expected_paths": ["package.json", "tailwind.config.js", "postcss.config.js", "app/layout.js", "app/page.js"],
          "verify": ["cat package.json | grep -q '\"next\"' && test -f tailwind.config.js && test -f app/layout.js"]
        }
      ]
    }"#;
    let mut planner = FakeClient::new(vec![
        AssistantReply::text(invalid),
        AssistantReply::text(invalid),
        AssistantReply::text(invalid),
    ]);

    let plan = generate_step_plan(&mut planner, goal, &cfg).unwrap();

    assert_eq!(plan.steps[0].id, "fallback-setup");
    assert_eq!(plan.steps[0].step_kind(), StepKind::Setup);
    assert!(
        plan.steps[0]
            .expected_paths
            .iter()
            .any(|path| path == "src/app/page.tsx"),
        "{plan:?}"
    );
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("\"planner_error_kind\":\"verify_command_policy_error\""));
    assert!(event_text.contains("\"event\":\"planner_fallback_plan\""));
    assert!(event_text.contains("\"profile\":\"nextjs\""));
}

#[test]
fn nextjs_profile_strengthening_does_not_reintroduce_duplicate_package_owner() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    let generated = r#"{
      "goal":"Scaffold a Next.js app",
      "steps":[
        {
          "id":"setup-manifests",
          "kind":"setup",
          "expected_result":"pass",
          "instruction":"Create package.json and tsconfig.json",
          "expected_paths":["package.json","tsconfig.json"],
          "verify":[]
        },
        {
          "id":"implement-game-page",
          "kind":"implement",
          "expected_result":"pass",
          "instruction":"Create package.json and the app entrypoint",
          "expected_paths":["package.json","src/app/page.tsx"],
          "verify":[]
        }
      ]
    }"#;
    let mut plan =
        parse_generated_step_plan_json(generated, "Scaffold a Next.js Space Invaders app")
            .unwrap();
    repair_generated_step_plan_contract(&mut plan);
    strengthen_step_plan_for_profile(&mut plan, &cfg);
    repair_generated_step_plan_contract(&mut plan);
    crate::planner::lint::lint_step_plan(&plan).unwrap();
    let package_owners = plan
        .steps
        .iter()
        .filter(|step| {
            step.expected_paths
                .iter()
                .any(|path| path == "package.json")
        })
        .count();
    assert_eq!(package_owners, 1);
}

#[test]
fn retry_prompt_accumulates_lint_categories() {
    let mut report = PlanLintReport::pass();
    report.push(
        "verify_policy",
        "verify command may not use shell control syntax",
    );
    let mut categories = BTreeSet::new();
    categories.insert("dependency_order".to_string());
    categories.insert("path_ownership".to_string());
    let prompt = build_lint_retry_prompt("goal", &report, 2, &categories);
    assert!(prompt.contains("without &&, ||, |, ;"));
    assert!(prompt.contains("Preserve the verification meaning"));
    assert!(prompt.contains("dependency installation"));
    assert!(prompt.contains("smoke-check.js"));
    assert!(prompt.contains("grep -q"));
    assert!(prompt.contains("Do not duplicate expected_paths"));
    assert!(prompt.contains("Python stdlib unittest does not require dependency setup"));
    assert!(prompt.contains("Keep the original top-level goal unchanged"));
    for provider in ["OpenAI", "Gemini", "Ollama"] {
        assert!(!prompt.contains(provider), "{provider}: {prompt}");
    }
}

#[test]
fn goal_length_retry_prompt_is_exact_and_step_preserving() {
    let mut report = PlanLintReport::pass();
    report.push("contract", "StepPlan goal is too long");
    let categories = BTreeSet::new();

    let prompt = build_lint_retry_prompt("goal", &report, 2, &categories);

    assert_eq!(prompt, "shorten goal to one sentence; keep steps unchanged");
}

#[test]
fn dependency_order_lint_maps_to_specific_planner_failure_kind() {
    let mut report = PlanLintReport::pass();
    report.push(
        "dependency_order",
        "verify command requires dependency setup or package manifest first",
    );
    let (stage, kind) = planner_stage_and_kind_for_lint(&report);
    assert_eq!(stage, "dependency_order");
    assert_eq!(kind, "verify_dependency_order_error");
}

#[test]
fn schema_retry_prompt_reports_missing_goal() {
    let prompt = build_schema_retry_prompt("Build app", "StepPlan missing goal", 1);
    assert!(prompt.contains("Detected schema issues:"));
    assert!(prompt.contains("Add a top-level goal field"));
    assert!(prompt.contains("\"goal\": \"Build app\""));
    assert!(prompt.contains("Return only one JSON object"));
}

#[test]
fn schema_retry_prompt_reports_invalid_step_id_type() {
    let prompt =
        build_schema_retry_prompt("Build app", "step id must be string, not number", 2);
    assert!(prompt.contains("Use quoted string step ids"));
    assert!(prompt.contains("Step id must be a quoted string"));
}

#[test]
fn required_final_artifacts_are_preserved_in_step_prompt() {
    let prompt = prompt_with_required_paths(
        "Create the app",
        &["package.json".to_string(), "src/app/page.tsx".to_string()],
    );
    assert!(prompt.contains("Required final artifacts:"));
    assert!(prompt.contains("- package.json"));
    assert!(prompt.contains("- src/app/page.tsx"));
}

#[test]
fn step_execution_prompt_includes_source_contract() {
    let plan = StepPlan {
        goal: "Build a game".to_string(),
        steps: vec![PlanStep {
            id: "create-page".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Create the page".to_string(),
            expected_paths: vec!["src/app/page.tsx".to_string()],
            verify: vec!["npm run build".to_string()],
        }],
    };
    let context = StepPromptContext {
        overall_goal: plan.goal.clone(),
        required_final_artifacts: vec!["src/app/page.tsx".to_string()],
        prior_expected_paths: vec!["package.json".to_string()],
        final_required_capabilities: vec!["player_control".to_string()],
        final_required_evidence: vec!["interactive_ui_source_evidence".to_string()],
        completion_contract_path: None,
    };
    let prompt = build_step_prompt(&plan, &plan.steps[0], &context, PromptLayout::Stable);
    assert!(prompt.contains("Overall goal:"));
    assert!(prompt.contains("Build a game"));
    assert!(prompt.contains("Current step id:"));
    assert!(prompt.contains("create-page"));
    assert!(prompt.contains("Verification commands for this step:"));
    assert!(prompt.contains("npm run build"));
    assert!(prompt.contains("Required final capabilities:"));
    assert!(prompt.contains("player_control"));
    assert!(prompt.contains("Required final evidence:"));
    assert!(prompt.contains("interactive_ui_source_evidence"));
    assert!(prompt.contains("Expected verification result:"));
    assert!(prompt.contains("Artifacts available from previous steps:"));
    assert!(prompt.contains("bounded step-local repair"));
}

#[test]
fn step_plan_prompt_prefix_keeps_profile_guidance_before_phase_goal() {
    let mut cfg = config(PathBuf::from("/tmp/work"));
    cfg.profile = "nextjs".to_string();
    let first = build_step_plan_user_prompt(
        "Original ultra goal: Build a game on port 3011\nProfile: nextjs\nStyle: default\nIntent: create\nPhase id: setup\nPhase task: Scaffold the app",
        &cfg,
    );
    let second = build_step_plan_user_prompt(
        "Original ultra goal: Build a game on port 3011\nProfile: nextjs\nStyle: default\nIntent: create\nPhase id: gameplay\nPhase task: Implement the gameplay",
        &cfg,
    );

    let prefix = common_prefix(&first, &second);

    assert!(prefix.contains("Required final artifacts:"), "{prefix}");
    assert!(
        prefix.contains("Profile verification expectations:"),
        "{prefix}"
    );
    assert!(prefix.contains("Ultra phase hard constraints:"), "{prefix}");
    assert!(prefix.ends_with("Phase id: "), "{prefix}");
}

#[test]
fn step_execution_prompt_prefix_covers_stable_contract_sections() {
    let plan = StepPlan {
        goal: "Build a game".to_string(),
        steps: vec![
            PlanStep {
                id: "create-page".to_string(),
                kind: "implement".to_string(),
                expected_result: "pass".to_string(),
                instruction: "Create the page".to_string(),
                expected_paths: vec!["src/app/page.tsx".to_string()],
                verify: Vec::new(),
            },
            PlanStep {
                id: "wire-input".to_string(),
                kind: "implement".to_string(),
                expected_result: "pass".to_string(),
                instruction: "Wire keyboard input".to_string(),
                expected_paths: vec!["src/app/page.tsx".to_string()],
                verify: Vec::new(),
            },
        ],
    };
    let context = StepPromptContext {
        overall_goal: plan.goal.clone(),
        required_final_artifacts: vec!["src/app/page.tsx".to_string()],
        prior_expected_paths: vec!["package.json".to_string()],
        final_required_capabilities: vec!["player_control".to_string()],
        final_required_evidence: vec!["interactive_ui_source_evidence".to_string()],
        completion_contract_path: None,
    };
    let first = build_step_prompt(&plan, &plan.steps[0], &context, PromptLayout::Stable);
    let second = build_step_prompt(&plan, &plan.steps[1], &context, PromptLayout::Stable);

    let prefix = common_prefix(&first, &second);

    assert!(prefix.contains("Required final artifacts:"), "{prefix}");
    assert!(prefix.contains("Required final capabilities:"), "{prefix}");
    assert!(prefix.contains("Required final evidence:"), "{prefix}");
    assert!(prefix.contains("Step execution rules:"), "{prefix}");
    assert!(prefix.ends_with("Current objective: "), "{prefix}");
}

#[test]
fn prompt_layout_legacy_restores_phase_first_order() {
    let mut cfg = config(PathBuf::from("/tmp/work"));
    cfg.profile = "nextjs".to_string();
    cfg.prompt_layout = PromptLayout::Legacy;
    let prompt = build_step_plan_user_prompt(
        "Original ultra goal: Build a game\nPhase id: setup\nPhase task: Scaffold",
        &cfg,
    );

    assert!(
        prompt.starts_with("Create a step plan for this task:"),
        "{prompt}"
    );
    assert!(
        prompt.find("Create a step plan for this task:").unwrap()
            < prompt.find("Required final artifacts:").unwrap(),
        "{prompt}"
    );
}

#[test]
fn stable_step_prompt_tail_opens_with_current_objective() {
    let plan = StepPlan {
        goal: "Build a game".to_string(),
        steps: vec![PlanStep {
            id: "create-page".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Create src/app/page.tsx for the game".to_string(),
            expected_paths: vec!["src/app/page.tsx".to_string()],
            verify: Vec::new(),
        }],
    };
    let context = StepPromptContext {
        overall_goal: plan.goal.clone(),
        required_final_artifacts: vec!["src/app/page.tsx".to_string()],
        prior_expected_paths: Vec::new(),
        final_required_capabilities: Vec::new(),
        final_required_evidence: Vec::new(),
        completion_contract_path: None,
    };

    let prompt = build_step_prompt(&plan, &plan.steps[0], &context, PromptLayout::Stable);
    let current_objective = prompt.find("Current objective:").unwrap();
    let current_step = prompt.find("Current step id:").unwrap();

    assert!(current_objective < current_step, "{prompt}");
    assert!(
        prompt.contains("Current objective: Create src/app/page.tsx for the game"),
        "{prompt}"
    );
}

#[test]
fn repair_prompt_prefix_is_shared_across_anchored_and_compact_rungs() {
    let report = VerificationReport::missing_path("src/app/page.tsx");
    let context = RepairContext {
        prompt_layout: crate::config::PromptLayout::Stable,
        overall_goal: Some("Build app".to_string()),
        required_final_artifacts: vec!["src/app/page.tsx".to_string()],
        ..RepairContext::default()
    };
    let anchored = build_repair_prompt_with_context("create-page", &report, &context);
    let compact = build_compact_compile_repair_prompt_with_context(
        "create-page",
        &VerificationReport::pass(),
        &context,
    );

    let prefix = common_prefix(&anchored, &compact);

    assert!(prefix.contains("Repair rules:"), "{prefix}");
    assert!(
        prefix.contains("Stop after the smallest bounded repair."),
        "{prefix}"
    );
}

#[test]
fn run_plan_passes_step_contract_to_execution_client() {
    let dir = tempfile::tempdir().unwrap();
    let plan = StepPlan {
        goal: "Create app.py".to_string(),
        steps: vec![PlanStep {
            id: "code".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Create app.py".to_string(),
            expected_paths: vec!["app.py".to_string()],
            verify: Vec::new(),
        }],
    };
    let mut fake = FakeClient::new(vec![AssistantReply {
        content: String::new(),
        tool_calls: vec![crate::state::ToolCall::new(
            "Write",
            serde_json::json!({"path":"app.py","content":"print('ok')"}),
        )],
        prompt_tokens: None,
        completion_tokens: None,
    }]);
    let result = run_step_plan(&mut fake, &plan, &config(dir.path().to_path_buf())).unwrap();
    assert_eq!(result, "plan-run complete: 1 steps");
    let recorded_messages = fake.messages();
    let messages = recorded_messages.first().expect("execution prompt");
    let prompt = messages
        .iter()
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(prompt.contains("Overall goal:"));
    assert!(prompt.contains("Current step id:"));
    assert!(prompt.contains("Expected paths after this step:"));
    assert!(prompt.contains("Verification commands for this step:"));
}

#[test]
fn verifier_command_false_negative_does_not_start_implementation_repair_turn() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    std::fs::write(
        dir.path().join("usage_error.py"),
        "import sys\nsys.stderr.write('usage: fake\\ninvalid option\\n')\nsys.exit(2)\n",
    )
    .unwrap();
    let plan = StepPlan {
        goal: "Run deterministic verification".to_string(),
        steps: vec![PlanStep {
            id: "verify-usage-error".to_string(),
            kind: "verify".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Run the verifier command".to_string(),
            expected_paths: Vec::new(),
            verify: vec!["python3 usage_error.py".to_string()],
        }],
    };
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events.clone());
    let mut fake = FakeClient::new(vec![
        AssistantReply {
            content: String::new(),
            tool_calls: vec![crate::state::ToolCall::new(
                "Bash",
                serde_json::json!({"command":"test -f usage_error.py"}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        },
        AssistantReply::text("Verification step complete."),
    ]);

    let err = run_step_plan(&mut fake, &plan, &cfg)
        .unwrap_err()
        .to_string();

    assert_eq!(fake.messages().len(), 1, "{err}");
    assert!(
        fake.messages().iter().all(|messages| {
            !messages
                .iter()
                .any(|message| message.content.contains("Repair step `verify-usage-error`"))
        }),
        "{:#?}",
        fake.messages()
    );
    assert!(err.contains("deterministic_verify_command_bug"), "{err}");
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("\"reason\":\"deterministic_verify_command_bug\""));
    assert!(event_text.contains("\"repair_target\":\"verifier_command\""));
    assert!(!event_text.contains("\"reason\":\"bounded_repair_exhausted\""));
}

#[test]
fn planner_prompt_report_is_blocker_not_success() {
    let prompt = plan_generation_system_prompt();
    assert!(prompt.contains("Report is not success"));
    assert!(prompt.contains("explicit blockers"));
    assert!(!prompt.contains("Use report only for final summary"));
}

#[test]
fn invalid_planner_json_does_not_save_plan_file() {
    let dir = tempfile::tempdir().unwrap();
    let mut planner = FakeClient::new(vec![
        AssistantReply::text("not json"),
        AssistantReply::text("still not json"),
        AssistantReply::text("nope"),
    ]);
    let err = generate_step_plan(&mut planner, "goal", &config(dir.path().to_path_buf()))
        .unwrap_err()
        .to_string();
    assert!(err.contains("invalid StepPlan after corrective retries"));
    assert!(!dir.path().join(".anvil/plans").exists());
}

#[test]
fn invalid_planner_lint_does_not_save_plan_file() {
    let dir = tempfile::tempdir().unwrap();
    let invalid = r#"{"goal":"goal","steps":[{"id":"s1","kind":"implement","expected_result":"pass","instruction":"Create app","expected_paths":["package.json"],"verify":["node check.js | node check2.js"]}]}"#;
    let mut planner = FakeClient::new(vec![
        AssistantReply::text(invalid),
        AssistantReply::text(invalid),
        AssistantReply::text(invalid),
    ]);
    let err = generate_step_plan(&mut planner, "goal", &config(dir.path().to_path_buf()))
        .unwrap_err()
        .to_string();
    assert!(err.contains("verify command"));
    assert!(!dir.path().join(".anvil/plans").exists());
}

#[test]
fn sanitizer_repairs_setup_dev_verify_without_retry() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    let plan_json = serde_json::to_string(&StepPlan {
        goal: "Project setup phase".to_string(),
        steps: vec![
            PlanStep {
                id: "create-manifest".to_string(),
                kind: "implement".to_string(),
                expected_result: "pass".to_string(),
                instruction: "Create package.json".to_string(),
                expected_paths: vec!["package.json".to_string()],
                verify: vec!["npm install".to_string()],
            },
            PlanStep {
                id: "create-page".to_string(),
                kind: "implement".to_string(),
                expected_result: "pass".to_string(),
                instruction: "Create src/app/page.tsx".to_string(),
                expected_paths: vec!["src/app/page.tsx".to_string()],
                verify: vec!["npm run dev & curl http://localhost:3011".to_string()],
            },
        ],
    })
    .unwrap();
    let mut planner = FakeClient::new(vec![AssistantReply::text(plan_json)]);

    let plan = generate_step_plan(&mut planner, "Project setup phase", &cfg).unwrap();

    assert_eq!(planner.messages().len(), 1);
    assert_eq!(plan.steps[0].kind, "setup");
    assert_eq!(plan.steps[0].verify, vec!["npm install"]);
    assert!(plan.steps[1].verify.is_empty());
    assert!(
        plan.steps[1]
            .instruction
            .contains("Browser readiness is verified by the runtime")
    );
    assert!(
        lint_step_plan_report_with_workspace(&plan, Some(dir.path())).is_pass(),
        "{plan:?}"
    );
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("\"event\":\"planner_plan_sanitized\""));
    assert!(!event_text.contains("\"event\":\"planner_error\""));
}

#[test]
fn sanitizer_emits_qwen_lint_shape_repairs() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events.clone());
    let plan_json = serde_json::to_string(&StepPlan {
        goal: "Create a Rust helper".to_string(),
        steps: vec![
            PlanStep {
                id: "setup-project".to_string(),
                kind: "setup".to_string(),
                expected_result: "pass".to_string(),
                instruction: "Create Cargo.toml for the helper crate".to_string(),
                expected_paths: vec!["Cargo.toml".to_string()],
                verify: vec!["cargo test".to_string()],
            },
            PlanStep {
                id: "create-helper".to_string(),
                kind: "implement".to_string(),
                expected_result: "pass".to_string(),
                instruction: format!("Create src/lib.rs. {}", "日本語".repeat(1_000)),
                expected_paths: vec!["src/lib.rs".to_string()],
                verify: Vec::new(),
            },
        ],
    })
    .unwrap();
    let mut planner = FakeClient::new(vec![AssistantReply::text(plan_json)]);

    let plan = generate_step_plan(&mut planner, "Create a Rust helper", &cfg).unwrap();

    assert_eq!(planner.messages().len(), 1);
    assert!(plan.steps[0].verify.is_empty());
    assert_eq!(plan.steps[1].verify, vec!["cargo test"]);
    assert_eq!(plan.steps[1].instruction, "Create src/lib.rs.");
    assert!(
        lint_step_plan_report_with_workspace(&plan, Some(dir.path())).is_pass(),
        "{plan:?}"
    );
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("\"event\":\"planner_plan_sanitized\""));
    assert!(event_text.contains("\"kind\":\"setup_verify_relocated\""));
    assert!(event_text.contains("\"kind\":\"instruction_truncated\""));
    assert!(!event_text.contains("\"event\":\"planner_error\""));
}

#[test]
fn generated_step_plan_drops_side_effect_expected_paths_before_lint() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    let plan_json = serde_json::to_string(&StepPlan {
        goal: "Set up the Next.js app dependencies".to_string(),
        steps: vec![PlanStep {
            id: "setup".to_string(),
            kind: "setup".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Create package.json and install dependencies.".to_string(),
            expected_paths: vec!["package.json".to_string(), "node_modules".to_string()],
            verify: vec!["test -d node_modules/next".to_string()],
        }],
    })
    .unwrap();
    let mut planner = FakeClient::new(vec![AssistantReply::text(plan_json)]);

    let plan = generate_step_plan(&mut planner, "Set up the Next.js app", &cfg).unwrap();

    assert_eq!(
        plan.steps[0].expected_paths,
        vec!["package.json".to_string()]
    );
    assert_eq!(
        plan.steps[0].verify,
        vec!["test -d node_modules/next".to_string()]
    );
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("\"event\":\"side_effect_path_dropped\""));
    assert!(event_text.contains("\"tier\":\"unambiguous\""));
    assert!(event_text.contains("\"path\":\"node_modules\""));
    assert!(!event_text.contains("\"event\":\"planner_error\""));
}

#[test]
fn deterministic_nextjs_scaffold_skips_planner_and_emits_event() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    let goal = "Original ultra goal: Build a browser game on port 3011\n\
Profile: nextjs\n\
Intent: create\n\
Phase id: project-setup\n\
Phase task: Scaffold and initialize the Next.js project shell";
    let mut planner = FakeClient::new(Vec::new());

    let plan = generate_step_plan_with_ui_for_phase(
        &mut planner,
        goal,
        &cfg,
        &NOOP_UI,
        Some("project-setup"),
        false,
        false,
    )
    .unwrap();

    assert!(planner.messages().is_empty());
    assert_eq!(plan.steps[0].id, "nextjs-scaffold");
    assert!(
        lint_step_plan_report_with_workspace(&plan, Some(dir.path())).is_pass(),
        "{plan:?}"
    );
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("\"event\":\"deterministic_step_plan_used\""));
    assert!(event_text.contains("\"phase_id\":\"project-setup\""));
    assert!(event_text.contains("\"template_id\":\"nextjs-scaffold\""));
    assert!(!event_text.contains("\"event\":\"planner_raw_output_shape\""));
}

#[test]
fn setup_phase_fallback_generates_profile_scaffold_after_invalid_attempts() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    let goal = "Original ultra goal: Build an interactive app\n\
Profile: nextjs\n\
Style: default\n\
Intent: create\n\
Phase id: project-setup\n\
Phase task: Scaffold and initialize the Next.js project shell on port 3011";
    let mut planner = FakeClient::new(vec![
        AssistantReply::text("not a step plan"),
        AssistantReply::text("still not a step plan"),
        AssistantReply::text("no valid step plan"),
    ]);

    let plan = generate_step_plan(&mut planner, goal, &cfg).unwrap();

    let expected_paths = crate::planner::profiles::nextjs::setup_scaffold_paths(dir.path());
    assert_eq!(planner.messages().len(), 3);
    assert_eq!(plan.steps.len(), 1);
    assert_eq!(plan.steps[0].kind, "setup");
    assert_eq!(plan.steps[0].expected_paths, expected_paths);
    assert_eq!(
        plan.steps[0].verify,
        expected_paths
            .iter()
            .map(|path| format!("test -f {path}"))
            .collect::<Vec<_>>()
    );
    for path in crate::planner::profiles::nextjs::setup_invariant_required_paths(dir.path()) {
        assert!(
            plan.steps[0].expected_paths.contains(&path),
            "fallback expected_paths must include invariant-required {path}"
        );
    }
    let instruction = &plan.steps[0].instruction;
    assert!(instruction.contains("@tailwind base"), "{instruction}");
    assert!(
        instruction.contains("@tailwind components"),
        "{instruction}"
    );
    assert!(instruction.contains("@tailwind utilities"), "{instruction}");
    assert!(instruction.contains("./globals.css"), "{instruction}");
    assert!(
        instruction.contains("exactly one Tailwind config file"),
        "{instruction}"
    );
    assert!(instruction.contains("scripts.dev"), "{instruction}");
    assert!(instruction.contains("scripts.start"), "{instruction}");
    assert!(instruction.contains("goal's port"), "{instruction}");
    assert!(
        lint_step_plan_report_with_workspace(&plan, Some(dir.path())).is_pass(),
        "{plan:?}"
    );
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("\"event\":\"planner_fallback_plan\""));
}

#[test]
fn setup_phase_fallback_generates_python_cli_scaffold_after_lint_exhaustion() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "python-cli".to_string();
    cfg.eval_events_path = Some(events.clone());
    let goal = "Original ultra goal: CSV processor CLI\n\
Profile: python-cli\n\
Style: default\n\
Intent: create\n\
Phase id: cli-setup\n\
Phase task: Set up the Python CLI package scaffold";
    let bad_plan = r#"{"goal":"bad setup","steps":[{"id":"bad","kind":"setup","expected_result":"pass","instruction":"Create pyproject.toml","expected_paths":["pyproject.toml"],"verify":["npm run build | grep error"]}]}"#;
    let mut planner = FakeClient::new(vec![
        AssistantReply::text(bad_plan),
        AssistantReply::text(bad_plan),
        AssistantReply::text(bad_plan),
    ]);

    let plan = generate_step_plan(&mut planner, goal, &cfg).unwrap();

    let expected_paths = profile_setup_scaffold_paths(dir.path(), "python-cli");
    assert_eq!(planner.messages().len(), 3);
    assert_eq!(plan.steps.len(), 1);
    assert_eq!(plan.steps[0].kind, "setup");
    assert_eq!(plan.steps[0].expected_paths, expected_paths);
    assert_eq!(
        plan.steps[0].verify,
        plan.steps[0]
            .expected_paths
            .iter()
            .map(|path| format!("test -f {path}"))
            .collect::<Vec<_>>()
    );
    let instruction = &plan.steps[0].instruction;
    assert!(
        instruction.contains("python-cli package scaffold"),
        "{instruction}"
    );
    assert!(instruction.contains("pyproject.toml"), "{instruction}");
    assert!(
        instruction.contains("python -m compileall -q src"),
        "{instruction}"
    );
    assert!(
        lint_step_plan_report_with_workspace(&plan, Some(dir.path())).is_pass(),
        "{plan:?}"
    );
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("\"event\":\"planner_fallback_plan\""));
    assert!(event_text.contains("\"profile\":\"python-cli\""));
}

#[test]
fn planner_prompt_provider_request_contract() {
    let dir = tempfile::tempdir().unwrap();
    let mut planner =
        FakeClient::new(vec![AssistantReply::text(generated_step_plan_json("goal"))]);
    let _ =
        generate_step_plan(&mut planner, "goal", &config(dir.path().to_path_buf())).unwrap();
    let recorded_messages = planner.messages();
    let messages = recorded_messages.first().expect("messages");
    assert_eq!(messages[0].role, "system");
    assert!(messages[0].content.contains("Return only one JSON object"));
    assert_eq!(messages[1].role, "user");
    assert!(messages[1].content.contains("Create a step plan"));
}

#[test]
fn step_plan_generation_retries_transient_provider_request_error() {
    let dir = tempfile::tempdir().unwrap();
    let mut planner = FlakyClient::new(
        1,
        "transient provider unavailable",
        vec![AssistantReply::text(generated_step_plan_json("goal"))],
    );

    let plan =
        generate_step_plan(&mut planner, "goal", &config(dir.path().to_path_buf())).unwrap();

    assert_eq!(planner.messages().len(), 2);
    assert_eq!(plan.goal, "goal");
}

#[test]
fn planner_prompt_ollama_request_contract() {
    let messages = step_plan_messages(&build_step_plan_user_prompt(
        "goal",
        &config(PathBuf::from("/tmp/work")),
    ));
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, "system");
    assert!(messages[0].content.contains("Allowed step kinds"));
    assert_eq!(messages[1].role, "user");
}

#[test]
fn generated_final_verify_uses_existing_workspace_nextjs_artifacts() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    std::fs::create_dir_all(dir.path().join("src/app")).unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"scripts":{"build":"next build"}}"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("src/app/page.tsx"),
        "export default function Page() { return null; }\n",
    )
    .unwrap();
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    let plan_json = r#"{"goal":"Verify the existing Next.js app","steps":[{"id":"final-verify","kind":"verify","expected_result":"pass","instruction":"Run deterministic Next.js build verification","expected_paths":[],"verify":["npm run build"]}]}"#;
    let mut planner = FakeClient::new(vec![AssistantReply::text(plan_json)]);

    let plan = generate_step_plan(&mut planner, "Verify the existing Next.js app", &cfg)
        .expect("workspace entrypoint and manifest should satisfy generation lint");

    assert_eq!(planner.messages().len(), 1);
    assert_eq!(plan.steps[0].id, "final-verify");
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(!event_text.contains("\"event\":\"planner_error\""));
}

#[test]
fn step_plan_quality_warning_does_not_change_exit_status() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events.clone());
    let plan_json = r#"{"goal":"Build a Next.js game app","steps":[{"id":"s1","kind":"implement","expected_result":"pass","instruction":"Create the app","expected_paths":["package.json","src/app/page.tsx"],"verify":[]}]}"#;
    let mut planner = FakeClient::new(vec![AssistantReply::text(plan_json)]);
    let plan = generate_step_plan(&mut planner, "Build a Next.js game app", &cfg).unwrap();
    assert_eq!(plan.steps.len(), 1);
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("planner_quality_warning"));
}

#[test]
fn retryable_quality_issue_gets_corrective_retry() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    let weak = r#"{"goal":"Build a Next.js game app","steps":[{"id":"make-app","kind":"implement","expected_result":"pass","instruction":"Create package.json and src/app/page.tsx for the game app","expected_paths":["package.json","src/app/page.tsx"],"verify":[]}]}"#;
    let strong = r#"{"goal":"Build a Next.js game app","steps":[{"id":"setup","kind":"setup","expected_result":"pass","instruction":"Create package.json with next, react, and react-dom dependencies","expected_paths":["package.json"],"verify":[]},{"id":"page","kind":"implement","expected_result":"pass","instruction":"Create src/app/page.tsx game page","expected_paths":["src/app/page.tsx"],"verify":[]},{"id":"build","kind":"verify","expected_result":"pass","instruction":"Run deterministic Next.js build","expected_paths":[],"verify":["npm run build"]}]}"#;
    let mut planner = FakeClient::new(vec![
        AssistantReply::text(weak),
        AssistantReply::text(strong),
    ]);
    let plan = generate_step_plan(&mut planner, "Build a Next.js game app", &cfg).unwrap();
    assert_eq!(planner.messages().len(), 2);
    assert!(
        plan.steps
            .iter()
            .flat_map(|step| step.verify.iter())
            .any(|command| command == "npm run build")
    );
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("planner_quality_issue"));
    assert!(event_text.contains("planner_quality_retry"));
}

#[test]
fn community_quality_retry_exhaustion_is_terminal_and_classified() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "community-mini-app".to_string();
    cfg.eval_events_path = Some(events.clone());
    let weak = r#"{"goal":"Create a Community Mini App","steps":[{"id":"spec","kind":"implement","expected_result":"pass","instruction":"Write app.spec.yaml","expected_paths":["app.spec.yaml"],"verify":["test -f app.spec.yaml"]}]}"#;
    let mut planner = FakeClient::new(vec![
        AssistantReply::text(weak),
        AssistantReply::text(weak),
        AssistantReply::text(weak),
    ]);
    let error = generate_step_plan(&mut planner, "Create a Community Mini App", &cfg)
        .expect_err("community quality exhaustion must stop");
    assert!(error.to_string().contains("planner_quality_exhausted"));
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("planner_quality_retry_exhausted"));
    assert!(event_text.contains("planner_quality_exhausted"));
}

#[test]
fn quality_retry_degradation_keeps_last_valid_plan() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    let weak = r#"{"goal":"Build a Next.js game app","steps":[{"id":"make-app","kind":"implement","expected_result":"pass","instruction":"Create package.json and src/app/page.tsx for the game app","expected_paths":["package.json","src/app/page.tsx"],"verify":[]}]}"#;
    let degraded = r#"{"goal":"Build a Next.js game app","steps":[{"id":"bad","kind":"implement","expected_result":"pass","instruction":"Create app","expected_paths":["package.json"],"verify":["node check.js | node check2.js"]}]}"#;
    let mut planner = FakeClient::new(vec![
        AssistantReply::text(weak),
        AssistantReply::text(degraded),
        AssistantReply::text(degraded),
    ]);
    let plan = generate_step_plan(&mut planner, "Build a Next.js game app", &cfg).unwrap();
    assert_eq!(planner.messages().len(), 3);
    assert_eq!(plan.steps[0].id, "make-app");
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains("planner_quality_retry_degraded"));
}

#[test]
fn advisory_quality_issue_does_not_retry() {
    let dir = tempfile::tempdir().unwrap();
    let plan_json = r#"{"goal":"Update README heading","steps":[{"id":"docs","kind":"implement","expected_result":"pass","instruction":"Update README.md","expected_paths":["README.md"],"verify":["test -f README.md"]}]}"#;
    let mut planner = FakeClient::new(vec![AssistantReply::text(plan_json)]);
    let plan = generate_step_plan(
        &mut planner,
        "Update README heading",
        &config(dir.path().to_path_buf()),
    )
    .unwrap();
    assert_eq!(planner.messages().len(), 1);
    assert_eq!(plan.steps[0].id, "docs");
}

#[test]
fn uat_scenario_goal_capability_goldens_guard_distribution_drift() {
    let scenarios = [
        (
            "game",
            "あなたが考える最高に面白くかっこいいスペースインベーダーゲームを3011ポートで起動可能なnext.jsアプリとして開発してください。",
            vec![
                "stateful_interaction",
                "start_or_restart_flow",
                "player_control",
                "adversary_or_challenge",
                "progression_or_score",
                "failure_or_collision_rule",
            ],
        ),
        (
            "tool",
            "ローカルストレージに保存されるTodoアプリ(追加・完了・削除・フィルタ)をNext.jsアプリとして3011ポートで起動可能に開発してください。",
            vec![
                "stateful_interaction",
                "user_input_or_action",
                "visible_state_change",
                "persistence",
            ],
        ),
        (
            "content",
            "Markdownをリアルタイムプレビューできるノートアプリ(編集・保存・一覧)をNext.jsアプリとして3011ポートで開発してください。",
            vec![
                "stateful_interaction",
                "user_input_or_action",
                "visible_state_change",
                "persistence",
            ],
        ),
    ];

    for (name, goal, expected) in scenarios {
        let actual = inferred_required_capabilities("nextjs", goal);
        let expected = expected
            .into_iter()
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        assert_eq!(actual, expected, "scenario={name}");
        if name != "game" {
            assert!(
                !actual.contains(&"adversary_or_challenge".to_string()),
                "scenario={name}: {actual:?}"
            );
            assert!(
                !actual.contains(&"failure_or_collision_rule".to_string()),
                "scenario={name}: {actual:?}"
            );
        }
    }
}

#[test]
fn todo_and_notes_capabilities_bind_to_generic_interactive_evidence() {
    for goal in [
        "ローカルストレージに保存されるTodoアプリ(追加・完了・削除・フィルタ)をNext.jsアプリとして3011ポートで起動可能に開発してください。",
        "Markdownをリアルタイムプレビューできるノートアプリ(編集・保存・一覧)をNext.jsアプリとして3011ポートで開発してください。",
    ] {
        let capabilities = inferred_required_capabilities("nextjs", goal);
        let evidence = inferred_required_evidence("nextjs", goal, &capabilities);
        assert!(evidence.contains(&"nextjs_route_evidence".to_string()));
        assert!(evidence.contains(&"visible_interactive_surface_evidence".to_string()));
        assert!(evidence.contains(&"user_input_handler_evidence".to_string()));
        assert!(evidence.contains(&"stateful_update_evidence".to_string()));
        assert!(evidence.contains(&"persistence_evidence".to_string()));
        assert!(!evidence.contains(&"challenge_or_adversary_evidence".to_string()));
        assert!(!evidence.contains(&"failure_or_collision_evidence".to_string()));
    }
}

#[test]
fn nextjs_route_goal_binds_basic_profile_contract() {
    let goal = "Create a route page";
    let capabilities = inferred_required_capabilities("nextjs", goal);
    let evidence = inferred_required_evidence("nextjs", goal, &capabilities);

    assert!(capabilities.is_empty(), "{capabilities:?}");
    let nextjs_id = ProfileId::Nextjs;
    assert!(
        ProfileRuntimeRegistry::resolve(&nextjs_id).requires_completion_contract(
            &nextjs_id,
            goal,
            &capabilities,
        )
    );
    assert!(evidence.contains(&"nextjs_route_evidence".to_string()));
    assert!(evidence.contains(&"build_command_or_dependency_missing_boundary".to_string()));
}

#[test]
fn generic_app_intent_binds_minimal_static_contract() {
    let goal = "ちょっとしたメモアプリを作って";
    let capabilities = inferred_required_capabilities("generic", goal);
    let evidence = inferred_required_evidence("generic", goal, &capabilities);
    let obligations = inferred_required_obligations("generic", goal, &capabilities);

    assert_eq!(
        capabilities,
        vec![GENERIC_INTERACTIVE_CONTRACT_CAPABILITY.to_string()]
    );
    assert_eq!(
        evidence,
        vec![
            "user_input_handler_evidence".to_string(),
            "stateful_update_evidence".to_string(),
            "visible_interactive_surface_evidence".to_string(),
        ]
    );
    assert_eq!(obligations, vec!["implementation".to_string()]);
}

#[test]
fn generic_script_goal_keeps_empty_contract() {
    let goal = "READMEを整形するスクリプト";
    let capabilities = inferred_required_capabilities("generic", goal);
    let evidence = inferred_required_evidence("generic", goal, &capabilities);
    let obligations = inferred_required_obligations("generic", goal, &capabilities);

    assert!(capabilities.is_empty());
    assert!(evidence.is_empty());
    assert!(obligations.is_empty());
}

#[test]
fn generic_filename_app_py_does_not_bind_static_contract() {
    let goal = "Create app.py";
    let capabilities = inferred_required_capabilities("generic", goal);
    let evidence = inferred_required_evidence("generic", goal, &capabilities);
    let obligations = inferred_required_obligations("generic", goal, &capabilities);

    assert!(capabilities.is_empty());
    assert!(evidence.is_empty());
    assert!(obligations.is_empty());
}

#[test]
fn generic_app_entrypoint_artifact_goal_does_not_bind_static_contract() {
    let goal = "Create app entrypoint\n\nRequired final artifacts:\n- src/app/page.tsx";
    let capabilities = inferred_required_capabilities("generic", goal);
    let evidence = inferred_required_evidence("generic", goal, &capabilities);
    let obligations = inferred_required_obligations("generic", goal, &capabilities);

    assert!(capabilities.is_empty());
    assert!(evidence.is_empty());
    assert!(obligations.is_empty());
}

#[test]
fn generic_profile_ignores_known_profile_phase_prompt_for_static_contract() {
    let goal =
        "Original ultra goal: 3011 port app\nProfile: nextjs\nPhase task: Scaffold project";
    let capabilities = inferred_required_capabilities("generic", goal);
    let evidence = inferred_required_evidence("generic", goal, &capabilities);
    let obligations = inferred_required_obligations("generic", goal, &capabilities);

    assert!(capabilities.is_empty());
    assert!(evidence.is_empty());
    assert!(obligations.is_empty());
}

#[test]
fn generic_contract_binding_emits_matched_intent_token() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events.clone());
    cfg.profile = "generic".to_string();
    let goal = "ちょっとしたメモアプリを作って";
    let capabilities = inferred_required_capabilities("generic", goal);
    let evidence = inferred_required_evidence("generic", goal, &capabilities);
    let obligations = inferred_required_obligations("generic", goal, &capabilities);

    let bound = bind_completion_contract_for_acceptance(
        &cfg,
        "ultra-plan-run",
        "generic",
        goal,
        &[],
        &capabilities,
        &evidence,
        &obligations,
    )
    .unwrap()
    .expect("generic app contract should bind");

    assert!(bound.generated);
    let event_text = std::fs::read_to_string(events).unwrap();
    assert!(event_text.contains(r#""event":"generic_contract_bound""#));
    assert!(event_text.contains(r#""matched_intent_token":"アプリ""#));
    assert!(event_text.contains(r#""inferred_keys":["user_input_handler_evidence","stateful_update_evidence","visible_interactive_surface_evidence"]"#));
}

#[test]
fn requested_port_overrides_dev_server_probe_spec() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"scripts":{"dev":"next dev","build":"next build"},"dependencies":{"next":"x","react":"x","react-dom":"x"}}"#,
    )
    .unwrap();

    let spec = load_nextjs_dev_server_probe_spec(dir.path(), Some(4000)).unwrap();

    assert_eq!(spec.port, 4000);
}

#[test]
fn dev_server_probe_spec_defaults_to_3011_without_request() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"scripts":{"dev":"next dev -p 4010","build":"next build"},"dependencies":{"next":"x","react":"x","react-dom":"x"}}"#,
    )
    .unwrap();

    let spec = load_nextjs_dev_server_probe_spec(dir.path(), None).unwrap();

    assert_eq!(spec.port, 3011);
}

#[test]
fn requested_port_is_bound_before_browser_stage() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    let plan = UltraPlan {
        goal: "Build a Next.js game on port 4022".to_string(),
        profile: "nextjs".to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: vec![UltraPhase {
            id: "core-game-engine".to_string(),
            prompt: "Create the route-bound game and run npm run build".to_string(),
        }],
    };
    let context = UltraRunContext::new(Vec::new());

    emit_ultra_context_initialized(&cfg, &plan, &context, 0);
    eval_events::emit(
        cfg.eval_events_path.as_deref(),
        json!({
            "event": "run_stop",
            "ok": false,
            "reason": "implementation_compile_error",
        }),
    );

    let event_text = std::fs::read_to_string(&events).unwrap();
    assert!(
        event_text.contains("\"event\":\"ultra_context_initialized\""),
        "{event_text}"
    );
    assert!(
        event_text.contains("\"requested_port\":\"4022 (goal)\""),
        "{event_text}"
    );
    let snapshot = eval_events::latest_completion_snapshot(Some(&events));
    assert_eq!(snapshot.requested_port, "4022 (goal)");
}

#[test]
fn omitted_intent_preserves_legacy_ultra_prompt_bytes() {
    let cfg = config(PathBuf::from("/tmp/work"));
    let goal = "parserを修正して";
    let intent = crate::planner::intent::detect_intent(goal);
    let expected = vec![
        crate::state::ConversationMessage::system(ultra_plan_generation_system_prompt(
            &cfg.profile,
            &cfg.style,
            intent,
        )),
        crate::state::ConversationMessage::user(ultra_plan_generation_user_prompt(
            goal,
            &cfg.profile,
            &cfg.style,
            intent,
        )),
    ];

    assert_eq!(
        serde_json::to_vec(&ultra_plan_generation_messages(goal, &cfg)).unwrap(),
        serde_json::to_vec(&expected).unwrap()
    );
}

#[test]
fn fix_contract_freezes_the_run_start_profile_binding() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "generic".to_string();
    cfg.profile_explicit = false;
    let fix = UltraPlan::deterministic("fix parser", "generic", "default", "fix");
    let create = UltraPlan::deterministic("create parser", "generic", "default", "create");

    assert!(!ProfilePromotionState::for_run(&fix, &cfg).eligible);
    assert!(ProfilePromotionState::for_run(&create, &cfg).eligible);
}

fn config(root: PathBuf) -> Config {
    Config {
        workspace_root: root,
        state_dir: PathBuf::from("state"),
        eval_events_path: None,
        completion_contract_path: None,
        yes: true,
        offline: false,
        context_budget: 1000,
        model: "m".to_string(),
        provider: crate::config::Provider::Ollama,
        tool_protocol: None,
        openai_api: crate::config::OpenAiApi::ChatCompletions,
        prompt_layout: crate::config::PromptLayout::Stable,
        plan_preset: crate::config::PlanPreset::None,
        intent_override: None,
        planner_model: "m".to_string(),
        planner_provider: crate::config::Provider::Ollama,
        ollama_host: "http://localhost:11434".to_string(),
        lm_studio_host: "http://localhost:1234".to_string(),
        num_predict: 100,
        max_iterations: 4,
        chat_timeout_secs: 1,
        chat_timeout_source: "override:test".to_string(),
        field_sources: crate::config::ConfigFieldSources::default(),
        chat_retries: 1,
        stream: false,
        resume: None,
        fresh_session: false,
        no_footer: false,
        narration: crate::config::NarrationMode::Normal,
        profile: "generic".to_string(),
        profile_explicit: false,
        profile_inference: None,
        style: "default".to_string(),
        action: crate::config::Action::Repl,
    }
}
