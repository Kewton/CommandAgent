use super::*;
use crate::planner::lint::lint_step_plan_report_with_workspace;
use crate::providers::{AssistantReply, ChatClient};
use crate::state::ConversationMessage;
use crate::tools::registry::ToolSpec;
use std::sync::{Arc, Mutex};

#[path = "final_acceptance_tests.rs"]
mod final_acceptance_tests;

#[path = "ultra_plan_flow_tests.rs"]
mod ultra_plan_flow_tests;

#[path = "data_pre_satisfied_tests.rs"]
mod data_pre_satisfied_tests;

#[path = "assurance_tests.rs"]
mod assurance_tests;

#[path = "cli_runtime_dispatch_tests.rs"]
mod cli_runtime_dispatch_tests;

#[path = "requested_port_tests.rs"]
mod requested_port_tests;

#[path = "profile_runtime_tests.rs"]
mod profile_runtime_tests;

fn common_prefix(left: &str, right: &str) -> String {
    left.chars()
        .zip(right.chars())
        .take_while(|(left, right)| left == right)
        .map(|(left, _)| left)
        .collect()
}

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
        dev_server_output_excerpt(&kind, output).contains(verifier_env::ENV_NODE_ENV_REMEDIATION)
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
    let plan =
        parse_generated_step_plan_json(json, "Create a Next.js Space Invaders app on port 3011.")
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
    let plan = generate_step_plan(&mut planner, "goal", &config(dir.path().to_path_buf())).unwrap();
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
    let plan = generate_step_plan(&mut planner, "goal", &config(dir.path().to_path_buf())).unwrap();
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
        event_text.contains("\"original_command_summary\":\"npm test && test -f package.json\"")
    );
    assert!(event_text.contains("\"normalized_commands\":[\"npm test\",\"test -f package.json\"]"));
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
        parse_generated_step_plan_json(generated, "Scaffold a Next.js Space Invaders app").unwrap();
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
    let prompt = build_schema_retry_prompt("Build app", "step id must be string, not number", 2);
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
    let mut planner = FakeClient::new(vec![AssistantReply::text(generated_step_plan_json("goal"))]);
    let _ = generate_step_plan(&mut planner, "goal", &config(dir.path().to_path_buf())).unwrap();
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

    let plan = generate_step_plan(&mut planner, "goal", &config(dir.path().to_path_buf())).unwrap();

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
    let goal = "Original ultra goal: 3011 port app\nProfile: nextjs\nPhase task: Scaffold project";
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

    let changed =
        reconcile_manifest_changed_dependencies_if_needed(&cfg, "python-cli", &mut setup_authority)
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
    let plan = UltraPlan::deterministic("Create a Next.js route", "nextjs", "default", "create");
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
    let mut first_phase_calls = nextjs_interactive_app_tool_calls(interactive_game_page_source());
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
    let repair_text = std::fs::read_dir(dir.path().join(".anvil/repairs"))
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
        let tsconfig =
            (outcome == "bad-tsconfig").then_some("{\"compilerOptions\":{\"rootDir\":\"src\"}}\n");
        let globals = (outcome != "valid-postcss-missing-globals").then_some(nextjs_globals_css());
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
        repair_prompt
            .contains("ArrowLeft keydown, ArrowRight keydown, Space keydown, canvas/center click")
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
        repair_prompt
            .contains("ArrowLeft keydown, ArrowRight keydown, Space keydown, canvas/center click")
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
        let expected_paths = final_acceptance_repair_expected_paths(&plan, &cfg, &report).unwrap();
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
    let recovery_prompt = std::fs::read_to_string(dir.path().join(recovery_prompt_path)).unwrap();
    assert!(
        recovery_prompt.contains(RESTART_PARTIAL_REPAIR_GUIDANCE),
        "{recovery_prompt}"
    );
    assert!(
        !event_text.contains("\"missing_evidence\":[\"restart_or_recoverable_state_evidence\"]")
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

    let result = run_ultra_plan(&mut planner, &mut execution, &plan, &cfg).unwrap_or_else(|err| {
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
            .is_some_and(|reason| reason
                .contains("capability_evidence_unresolved:restart_or_recoverable_state_evidence")),
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
        repair_prompt
            .contains("restart_or_recoverable_state_evidence: add data-anvil-action=\"restart\""),
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

    let by_evidence = browser_interaction_probe_options(&[], &["persistence_evidence".to_string()]);
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
            instruction: "Create package.json and src/app/page.tsx then verify build".to_string(),
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
        goal: "Create app entrypoint\n\nRequired final artifacts:\n- src/app/page.tsx".to_string(),
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
        goal: "Create app entrypoint\n\nRequired final artifacts:\n- src/app/page.tsx".to_string(),
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
    let repair_dir = dir.path().join(".anvil/repairs");
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
        event_text.contains("\"repair_attempts\":0") || !event_text.contains("step_verify_repair")
    );
    let repair_dir = dir.path().join(".anvil/repairs");
    let prompt = std::fs::read_dir(repair_dir)
        .unwrap()
        .flatten()
        .filter_map(|entry| std::fs::read_to_string(entry.path()).ok())
        .find(|text| text.contains("requires a Setup-authority step running dependency install"))
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
        goal: "Create app entrypoint\n\nRequired final artifacts:\n- src/app/page.tsx".to_string(),
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

fn assert_single_recovery_ultra_plan(root: &Path) -> UltraPlan {
    let plans_dir = root.join(".anvil/plans");
    assert!(
        plans_dir.is_dir(),
        "missing plans dir: {}",
        plans_dir.display()
    );
    let mut paths = std::fs::read_dir(&plans_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with("recovery-ultra-plan-") && name.ends_with(".yaml")
                })
        })
        .collect::<Vec<_>>();
    paths.sort();
    assert_eq!(paths.len(), 1, "recovery plan paths: {paths:#?}");
    parse_ultra_plan(&std::fs::read_to_string(&paths[0]).unwrap()).unwrap()
}

#[derive(Clone)]
struct FakeClient {
    state: Arc<Mutex<FakeClientState>>,
}

struct FakeClientState {
    replies: Vec<AssistantReply>,
    messages: Vec<Vec<ConversationMessage>>,
}

impl FakeClient {
    fn new(replies: Vec<AssistantReply>) -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeClientState {
                replies,
                messages: Vec::new(),
            })),
        }
    }

    fn messages(&self) -> Vec<Vec<ConversationMessage>> {
        self.state.lock().unwrap().messages.clone()
    }
}

impl ChatClient for FakeClient {
    fn label(&self) -> &str {
        "fake"
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn chat(
        &mut self,
        _model: &str,
        messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        let mut state = self.state.lock().unwrap();
        state.messages.push(messages.to_vec());
        if state.replies.is_empty() {
            anyhow::bail!("fake client exhausted")
        }
        Ok(state.replies.remove(0))
    }
}

#[derive(Clone)]
struct CompactAwareCompileRepairClient {
    state: Arc<Mutex<CompactAwareCompileRepairState>>,
}

struct CompactAwareCompileRepairState {
    messages: Vec<Vec<ConversationMessage>>,
    initial_done: bool,
    appended_repair_calls: usize,
    compact_repair_calls: usize,
}

impl CompactAwareCompileRepairClient {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(CompactAwareCompileRepairState {
                messages: Vec::new(),
                initial_done: false,
                appended_repair_calls: 0,
                compact_repair_calls: 0,
            })),
        }
    }

    fn messages(&self) -> Vec<Vec<ConversationMessage>> {
        self.state.lock().unwrap().messages.clone()
    }

    fn appended_repair_calls(&self) -> usize {
        self.state.lock().unwrap().appended_repair_calls
    }

    fn compact_repair_calls(&self) -> usize {
        self.state.lock().unwrap().compact_repair_calls
    }
}

impl ChatClient for CompactAwareCompileRepairClient {
    fn label(&self) -> &str {
        "compact-aware"
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn chat(
        &mut self,
        _model: &str,
        messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        let mut state = self.state.lock().unwrap();
        state.messages.push(messages.to_vec());
        let prompt = messages
            .iter()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if !state.initial_done {
            state.initial_done = true;
            return Ok(api_mismatch_initial_reply(3011));
        }
        if prompt.contains("Repair session mode: compact") {
            state.compact_repair_calls += 1;
            return Ok(api_mismatch_poll_fix_reply());
        }
        if prompt.contains("Property 'onStateChange'") {
            state.appended_repair_calls += 1;
            if state.appended_repair_calls == 1 {
                return Ok(api_mismatch_read_only_reply());
            }
            return Ok(AssistantReply::text(
                "The failing call is engine.onStateChange, but no edit is needed.",
            ));
        }
        anyhow::bail!("compact-aware fake client received unexpected prompt")
    }
}

#[derive(Clone)]
struct RegenerationCompileRepairClient {
    state: Arc<Mutex<RegenerationCompileRepairState>>,
}

struct RegenerationCompileRepairState {
    messages: Vec<Vec<ConversationMessage>>,
    initial_done: bool,
    appended_repair_calls: usize,
    compact_repair_calls: usize,
    regeneration_calls: usize,
    regeneration_reply: AssistantReply,
}

impl RegenerationCompileRepairClient {
    fn new(regeneration_reply: AssistantReply) -> Self {
        Self {
            state: Arc::new(Mutex::new(RegenerationCompileRepairState {
                messages: Vec::new(),
                initial_done: false,
                appended_repair_calls: 0,
                compact_repair_calls: 0,
                regeneration_calls: 0,
                regeneration_reply,
            })),
        }
    }

    fn messages(&self) -> Vec<Vec<ConversationMessage>> {
        self.state.lock().unwrap().messages.clone()
    }

    fn regeneration_calls(&self) -> usize {
        self.state.lock().unwrap().regeneration_calls
    }
}

impl ChatClient for RegenerationCompileRepairClient {
    fn label(&self) -> &str {
        "regeneration-aware"
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn chat(
        &mut self,
        _model: &str,
        messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        let mut state = self.state.lock().unwrap();
        state.messages.push(messages.to_vec());
        let prompt = messages
            .iter()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if !state.initial_done {
            state.initial_done = true;
            return Ok(api_mismatch_initial_reply(3011));
        }
        if prompt.contains("Repair session mode: compact regeneration") {
            state.regeneration_calls += 1;
            return Ok(state.regeneration_reply.clone());
        }
        if prompt.contains("Repair session mode: compact") {
            state.compact_repair_calls += 1;
            return Ok(AssistantReply::text(
                "I understand the compile frame, but no edit is needed.",
            ));
        }
        if prompt.contains("Property 'onStateChange'") {
            state.appended_repair_calls += 1;
            if state.appended_repair_calls == 1 {
                return Ok(api_mismatch_read_only_reply());
            }
            return Ok(AssistantReply::text(
                "The failing source was inspected, but no edit is needed.",
            ));
        }
        anyhow::bail!("regeneration-aware fake client received unexpected prompt")
    }
}

#[derive(Clone)]
struct EditThenRegenerationCompileRepairClient {
    state: Arc<Mutex<EditThenRegenerationCompileRepairState>>,
}

struct EditThenRegenerationCompileRepairState {
    messages: Vec<Vec<ConversationMessage>>,
    initial_done: bool,
    read_followup_pending: bool,
    appended_repair_calls: usize,
    compact_repair_calls: usize,
    regeneration_calls: usize,
}

impl EditThenRegenerationCompileRepairClient {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(EditThenRegenerationCompileRepairState {
                messages: Vec::new(),
                initial_done: false,
                read_followup_pending: false,
                appended_repair_calls: 0,
                compact_repair_calls: 0,
                regeneration_calls: 0,
            })),
        }
    }

    fn appended_repair_calls(&self) -> usize {
        self.state.lock().unwrap().appended_repair_calls
    }

    fn compact_repair_calls(&self) -> usize {
        self.state.lock().unwrap().compact_repair_calls
    }

    fn regeneration_calls(&self) -> usize {
        self.state.lock().unwrap().regeneration_calls
    }
}

impl ChatClient for EditThenRegenerationCompileRepairClient {
    fn label(&self) -> &str {
        "edit-then-regeneration-aware"
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn chat(
        &mut self,
        _model: &str,
        messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        let mut state = self.state.lock().unwrap();
        state.messages.push(messages.to_vec());
        let prompt = messages
            .iter()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if !state.initial_done {
            state.initial_done = true;
            return Ok(api_mismatch_initial_reply(3011));
        }
        if state.read_followup_pending {
            state.read_followup_pending = false;
            return Ok(AssistantReply::text(
                "The file was inspected, but no source behavior changed.",
            ));
        }
        if prompt.contains("Repair session mode: compact regeneration") {
            state.regeneration_calls += 1;
            return Ok(api_mismatch_poll_fix_reply());
        }
        if prompt.contains("Repair session mode: compact") {
            state.compact_repair_calls += 1;
            state.read_followup_pending = true;
            return Ok(api_mismatch_read_only_reply());
        }
        if prompt.contains("Compile error frames and remedies")
            || prompt.contains("implementation_compile_error")
            || prompt.contains("Type error:")
        {
            state.appended_repair_calls += 1;
            if state.appended_repair_calls == 1 {
                return Ok(AssistantReply {
                    content: String::new(),
                    tool_calls: vec![crate::state::ToolCall::new(
                        "Write",
                        serde_json::json!({"path":"src/app/SpaceInvadersGame.tsx","content":api_mismatch_insufficient_game_source()}),
                    )],
                    prompt_tokens: None,
                    completion_tokens: None,
                });
            }
            state.read_followup_pending = true;
            return Ok(api_mismatch_read_only_reply());
        }
        anyhow::bail!("edit-then-regeneration fake client received unexpected prompt")
    }
}

#[derive(Clone)]
struct FlakyClient {
    state: Arc<Mutex<FlakyClientState>>,
}

struct FlakyClientState {
    replies: Vec<AssistantReply>,
    messages: Vec<Vec<ConversationMessage>>,
    failures_remaining: usize,
    failure_message: String,
}

impl FlakyClient {
    fn new(
        failures_remaining: usize,
        failure_message: impl Into<String>,
        replies: Vec<AssistantReply>,
    ) -> Self {
        Self {
            state: Arc::new(Mutex::new(FlakyClientState {
                replies,
                messages: Vec::new(),
                failures_remaining,
                failure_message: failure_message.into(),
            })),
        }
    }

    fn messages(&self) -> Vec<Vec<ConversationMessage>> {
        self.state.lock().unwrap().messages.clone()
    }
}

impl ChatClient for FlakyClient {
    fn label(&self) -> &str {
        "flaky"
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn chat(
        &mut self,
        _model: &str,
        messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        let mut state = self.state.lock().unwrap();
        state.messages.push(messages.to_vec());
        if state.failures_remaining > 0 {
            state.failures_remaining -= 1;
            anyhow::bail!("{}", state.failure_message);
        }
        if state.replies.is_empty() {
            anyhow::bail!("flaky client exhausted")
        }
        Ok(state.replies.remove(0))
    }
}

#[derive(Clone)]
struct EchoGoalPlanner {
    state: Arc<Mutex<EchoGoalPlannerState>>,
}

struct EchoGoalPlannerState {
    messages: Vec<Vec<ConversationMessage>>,
    calls: usize,
}

impl EchoGoalPlanner {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(EchoGoalPlannerState {
                messages: Vec::new(),
                calls: 0,
            })),
        }
    }

    fn messages(&self) -> Vec<Vec<ConversationMessage>> {
        self.state.lock().unwrap().messages.clone()
    }
}

impl ChatClient for EchoGoalPlanner {
    fn label(&self) -> &str {
        "echo-planner"
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn chat(
        &mut self,
        _model: &str,
        messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        let mut state = self.state.lock().unwrap();
        state.messages.push(messages.to_vec());
        state.calls += 1;
        let echoed_goal = messages
            .iter()
            .rev()
            .find(|message| message.role == "user")
            .map(|message| message.content.clone())
            .unwrap_or_default();
        let path = format!("phase-{}.txt", state.calls);
        let plan = StepPlan {
            goal: echoed_goal,
            steps: vec![PlanStep {
                id: format!("phase-{}", state.calls),
                kind: "implement".to_string(),
                expected_result: "pass".to_string(),
                instruction: format!("Create {path} for this phase."),
                expected_paths: vec![path],
                verify: Vec::new(),
            }],
        };
        Ok(AssistantReply::text(serde_json::to_string(&plan).unwrap()))
    }
}

fn generated_step_plan_json(goal: &str) -> String {
    serde_json::to_string(&StepPlan::single(goal)).unwrap()
}

fn generated_ultra_plan_yaml(goal: &str) -> String {
    render_ultra_plan(&UltraPlan {
        goal: goal.to_string(),
        profile: "generic".to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: vec![
            crate::planner::ultra_plan::UltraPhase {
                id: "scaffold".to_string(),
                prompt: format!("Create the required project artifacts for {goal}."),
            },
            crate::planner::ultra_plan::UltraPhase {
                id: "verify".to_string(),
                prompt: format!("Run deterministic verification for {goal} and repair failures."),
            },
        ],
    })
}

fn two_phase_ultra_plan(goal: &str, profile: &str) -> UltraPlan {
    UltraPlan {
        goal: goal.to_string(),
        profile: profile.to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: vec![
            UltraPhase {
                id: "scaffold".to_string(),
                prompt: "Create the initial scaffold.".to_string(),
            },
            UltraPhase {
                id: "finish".to_string(),
                prompt: "Complete the final behavior and verification evidence.".to_string(),
            },
        ],
    }
}

fn single_write_step_plan_json(goal: &str, path: &str) -> String {
    serde_json::to_string(&StepPlan {
        goal: goal.to_string(),
        steps: vec![PlanStep {
            id: "write-artifact".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: format!("Create {path}."),
            expected_paths: vec![path.to_string()],
            verify: Vec::new(),
        }],
    })
    .unwrap()
}

fn browser_release_evidence_tool_calls() -> Vec<crate::state::ToolCall> {
    vec![
        crate::state::ToolCall::new(
            "Write",
            serde_json::json!({
                "path": "browser-readiness.json",
                "content": r#"{"ok":true,"http_status":200,"route_rendered":true}"#
            }),
        ),
        crate::state::ToolCall::new(
            "Write",
            serde_json::json!({
                "path": "browser-interaction.json",
                "content": contract_interaction_pass_json()
            }),
        ),
    ]
}

fn generic_interactive_source() -> &'static str {
    r#"import { useState } from "react";
export default function Memo(){
  const [items, setItems] = useState([]);
  return <form onSubmit={(event) => { event.preventDefault(); setItems([...items, "note"]); }}>
    <input onChange={() => setItems([...items, "draft"])} />
    <button type="submit">Add</button>
    <ul>{items.map((item, index) => <li key={index}>{item}</li>)}</ul>
  </form>;
}
"#
}

fn challenge_ultra_plan() -> UltraPlan {
    UltraPlan {
        goal: "Create a browser challenge screen".to_string(),
        profile: "generic".to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: vec![
            crate::planner::ultra_plan::UltraPhase {
                id: "phase-one".to_string(),
                prompt: "Create the first page artifact.".to_string(),
            },
            crate::planner::ultra_plan::UltraPhase {
                id: "phase-two".to_string(),
                prompt: "Close remaining final requirements.".to_string(),
            },
        ],
    }
}

fn challenge_implement_step_plan_json() -> String {
    serde_json::to_string(&StepPlan {
        goal: "Create src/app/page.tsx".to_string(),
        steps: vec![PlanStep {
            id: "page".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Create or update src/app/page.tsx".to_string(),
            expected_paths: vec!["src/app/page.tsx".to_string()],
            verify: Vec::new(),
        }],
    })
    .unwrap()
}

fn challenge_setup_step_plan_json() -> String {
    serde_json::to_string(&StepPlan {
        goal: "Record phase two setup completion".to_string(),
        steps: vec![PlanStep {
            id: "phase-two-marker".to_string(),
            kind: "setup".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Create phase-two.txt".to_string(),
            expected_paths: vec!["phase-two.txt".to_string()],
            verify: Vec::new(),
        }],
    })
    .unwrap()
}

fn final_marker_implement_step_plan_json() -> String {
    serde_json::to_string(&StepPlan {
        goal: "Run the final implementation pass".to_string(),
        steps: vec![PlanStep {
            id: "final-page-pass".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Update src/app/page.tsx as the final implementation artifact."
                .to_string(),
            expected_paths: vec!["src/app/page.tsx".to_string()],
            verify: Vec::new(),
        }],
    })
    .unwrap()
}

fn write_challenge_contract(root: &Path) -> PathBuf {
    write_challenge_contract_with_cap(root, 1)
}

fn write_challenge_contract_with_cap(root: &Path, verify_repair_cap: usize) -> PathBuf {
    let path = root.join("challenge-contract.json");
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&serde_json::json!({
            "required_paths": ["src/app/page.tsx"],
            "required_evidence": ["challenge_or_adversary_evidence"],
            "verify_repair_cap": verify_repair_cap
        }))
        .unwrap(),
    )
    .unwrap();
    path
}

fn latest_event(path: &Path, event: &str) -> Value {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .rfind(|value| value.get("event").and_then(Value::as_str) == Some(event))
        .unwrap_or_else(|| panic!("missing event {event} in {}", path.display()))
}

fn events_with_name(path: &Path, event: &str) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|value| value.get("event").and_then(Value::as_str) == Some(event))
        .collect()
}

fn event_array_contains(value: &Value, key: &str, needle: &str) -> bool {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|item| item.as_str() == Some(needle))
}

fn planner_request_text(client: &FakeClient, index: usize) -> String {
    client.messages()[index]
        .iter()
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn prompt_section_lines(prompt: &str, header: &str) -> Vec<String> {
    let mut lines = prompt.lines().skip_while(|line| *line != header);
    let Some(first) = lines.next() else {
        panic!("missing prompt section {header:?} in {prompt}");
    };
    std::iter::once(first.to_string())
        .chain(
            lines
                .take_while(|line| !line.trim().is_empty())
                .map(str::to_string),
        )
        .collect()
}

fn nextjs_scaffold_expected_paths() -> Vec<String> {
    vec![
        "package.json".to_string(),
        "tsconfig.json".to_string(),
        "postcss.config.js".to_string(),
        "tailwind.config.ts".to_string(),
        "src/app/layout.tsx".to_string(),
        "src/app/page.tsx".to_string(),
        "src/app/globals.css".to_string(),
        "src/app/global.d.ts".to_string(),
    ]
}

fn nextjs_complete_package_json() -> &'static str {
    r#"{"dependencies":{"next":"^14.2.0","react":"^18.3.0","react-dom":"^18.3.0"},"devDependencies":{"typescript":"^5.5.0","@types/node":"^20.14.0","@types/react":"^18.3.0","@types/react-dom":"^18.3.0","tailwindcss":"^3.4.19","postcss":"^8.5.15","autoprefixer":"^10.4.20"},"scripts":{"build":"next build","dev":"next dev -p 3011","start":"next start -p 3011"}}"#
}

fn canvas_game_repair_guidance() -> &'static str {
    &crate::planner::profiles::nextjs::knowledge::get()
        .repair_guidance
        .canvas_game_interaction
}

fn nextjs_lean_package_json() -> &'static str {
    r#"{"dependencies":{"next":"^14.2.0","react":"^18.3.0","react-dom":"^18.3.0"},"scripts":{"build":"next build","dev":"next dev -p 3011","start":"next start -p 3011"}}"#
}

fn explicit_port_goal(goal: &str, port: u16) -> String {
    format!("{goal} on port {port}")
}

fn nextjs_tsconfig_json() -> &'static str {
    r#"{"compilerOptions":{"target":"ES2017","lib":["dom","dom.iterable","esnext"],"allowJs":true,"skipLibCheck":true,"strict":true,"noEmit":true,"esModuleInterop":true,"module":"esnext","moduleResolution":"bundler","resolveJsonModule":true,"isolatedModules":true,"jsx":"preserve","incremental":true,"plugins":[{"name":"next"}],"baseUrl":".","paths":{"@/*":["./src/*"]}},"include":["next-env.d.ts","**/*.ts","**/*.tsx",".next/types/**/*.ts"],"exclude":["node_modules"]}"#
}

fn nextjs_layout_source() -> &'static str {
    "import './globals.css';\nexport default function Layout({children}:{children:React.ReactNode}){return <html><body>{children}</body></html>;}"
}

fn nextjs_globals_css() -> &'static str {
    "@tailwind base;\n@tailwind components;\n@tailwind utilities;\n"
}

fn nextjs_tailwind_config_ts() -> &'static str {
    "import type { Config } from 'tailwindcss';\nconst config: Config = { content: ['./src/pages/**/*.{js,ts,jsx,tsx,mdx}', './src/components/**/*.{js,ts,jsx,tsx,mdx}', './src/app/**/*.{js,ts,jsx,tsx,mdx}'], theme: { extend: {} }, plugins: [] };\nexport default config;\n"
}

fn nextjs_postcss_config() -> &'static str {
    "module.exports = { plugins: { tailwindcss: {}, autoprefixer: {} } };\n"
}

fn write_nextjs_profile_workspace(
    root: &Path,
    globals_css: Option<&str>,
    postcss_config: Option<&str>,
    tsconfig_json: Option<&str>,
) {
    std::fs::create_dir_all(root.join("src/app")).unwrap();
    std::fs::write(root.join("package.json"), nextjs_complete_package_json()).unwrap();
    let tsconfig_json = tsconfig_json.unwrap_or(nextjs_tsconfig_json());
    std::fs::write(root.join("tsconfig.json"), tsconfig_json).unwrap();
    std::fs::write(root.join("tailwind.config.ts"), nextjs_tailwind_config_ts()).unwrap();
    if let Some(postcss_config) = postcss_config {
        std::fs::write(root.join("postcss.config.js"), postcss_config).unwrap();
    }
    std::fs::write(
        root.join("src/app/page.tsx"),
        "export default function Page(){return <main className=\"min-h-screen\">App</main>;}",
    )
    .unwrap();
    std::fs::write(root.join("src/app/layout.tsx"), nextjs_layout_source()).unwrap();
    std::fs::write(
        root.join("src/app/global.d.ts"),
        "declare module \"*.css\";\n",
    )
    .unwrap();
    if let Some(globals_css) = globals_css {
        std::fs::write(root.join("src/app/globals.css"), globals_css).unwrap();
    }
}

fn generated_nextjs_artifact_plan_json(goal: &str) -> String {
    let expected_paths = nextjs_scaffold_expected_paths();
    serde_json::to_string(&StepPlan {
        goal: goal.to_string(),
        steps: vec![PlanStep {
            id: "create-nextjs-artifacts".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: format!(
                "Create a coherent Next.js scaffold with {}",
                expected_paths.join(", ")
            ),
            expected_paths,
            verify: Vec::new(),
        }],
    })
    .unwrap()
}

fn generated_nextjs_fixture_plan_json_with_kind(
    goal: &str,
    check_path: &str,
    kind: &str,
) -> String {
    let mut expected_paths = vec![check_path.to_string()];
    if check_path.contains("scaffold") {
        expected_paths = nextjs_scaffold_expected_paths();
        expected_paths.push(check_path.to_string());
    }
    let verify = if kind == "setup" {
        Vec::new()
    } else {
        vec![format!("python3 -m py_compile {check_path}")]
    };
    serde_json::to_string(&StepPlan {
            goal: goal.to_string(),
            steps: vec![PlanStep {
                id: "create-and-check-artifacts".to_string(),
                kind: kind.to_string(),
                expected_result: "pass".to_string(),
                instruction: format!(
                    "Create the declared artifacts including {check_path} and keep the Next.js files coherent"
                ),
                expected_paths,
                verify,
            }],
        })
        .unwrap()
}

fn interactive_game_page_source() -> &'static str {
    r#""use client";
import { useEffect, useState } from "react";
export default function Page(){
  const [score, setScore] = useState(0);
  const [gameOver, setGameOver] = useState(false);
  const [bullets, setBullets] = useState<{ x: number; y: number }[]>([]);
  const [enemies, setEnemies] = useState([{ x: 10, y: 20 }]);
  const fireBullet = () => setBullets((items) => [...items, { x: 10, y: 90 }]);
  const restart = () => {
    setGameOver(false);
    setScore(0);
    setBullets([]);
    setEnemies([{ x: 10, y: 20 }]);
  };
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "ArrowLeft") {
        fireBullet();
      }
    };
    const frame = requestAnimationFrame(() => {
      bullets.forEach((bullet) => {
        enemies.forEach((enemy) => {
          if (Math.abs(bullet.x - enemy.x) < 12 && Math.abs(bullet.y - enemy.y) < 12) {
            setGameOver(true);
            setScore((value) => value + 10);
          }
        });
      });
      setEnemies((items) => items.map((enemy) => ({ ...enemy, x: enemy.x + 1 })));
    });
    window.addEventListener("keydown", onKeyDown);
    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [bullets, enemies]);
  return <main><button onClick={fireBullet}>Start</button><button onClick={restart}>Restart</button><canvas /><p>score {score} enemy collision {gameOver ? "game over" : "playing"}</p></main>;
}
"#
}

fn cross_file_weak_restart_interactive_game_page_source() -> &'static str {
    r#""use client";
import { useEffect, useRef, useState } from "react";
import { GameEngine } from "./gameEngine";
export default function Page(){
  const engineRef = useRef(new GameEngine());
  const [score, setScore] = useState(0);
  const [gameOver, setGameOver] = useState(false);
  const [screen, setScreen] = useState("gameOver");
  const [bullets, setBullets] = useState<{ x: number; y: number }[]>([]);
  const [enemies, setEnemies] = useState([{ x: 10, y: 20 }]);
  const fireBullet = () => {
    setBullets((items) => [...items, { x: 10, y: 90 }]);
    setScore((value) => value + 10);
  };
  const startGame = () => {
    engineRef.current?.reset();
    setScreen("playing");
    setGameOver(false);
  };
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "ArrowLeft") {
        fireBullet();
      }
    };
    const frame = requestAnimationFrame(() => {
      bullets.forEach((bullet) => {
        enemies.forEach((enemy) => {
          if (Math.abs(bullet.x - enemy.x) < 12 && Math.abs(bullet.y - enemy.y) < 12) {
            setGameOver(true);
            setScore((value) => value + 25);
          }
        });
      });
      setEnemies((items) => items.map((enemy) => ({ ...enemy, x: enemy.x + 1 })));
    });
    window.addEventListener("keydown", onKeyDown);
    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [bullets, enemies]);
  return <main><button onClick={startGame}>Restart</button><button onClick={fireBullet}>Fire</button><canvas /><p>score {score} enemy collision {gameOver ? "game over" : screen}</p></main>;
}
"#
}

fn cross_file_weak_restart_game_engine_source() -> &'static str {
    r#"export class GameEngine {
  score = 10;
  actors = [{ x: 1, y: 2 }];
  reset() {
    this.score = 0;
    this.actors = [{ x: 1, y: 2 }];
  }
}
"#
}

fn interactive_game_page_without_restart_source() -> &'static str {
    r#""use client";
import { useEffect, useState } from "react";
export default function Page(){
  const [score, setScore] = useState(0);
  const [gameOver, setGameOver] = useState(false);
  const [bullets, setBullets] = useState<{ x: number; y: number }[]>([]);
  const [enemies, setEnemies] = useState([{ x: 10, y: 20 }]);
  const fireBullet = () => setBullets((items) => [...items, { x: 10, y: 90 }]);
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "ArrowLeft") {
        fireBullet();
      }
    };
    const frame = requestAnimationFrame(() => {
      bullets.forEach((bullet) => {
        enemies.forEach((enemy) => {
          if (Math.abs(bullet.x - enemy.x) < 12 && Math.abs(bullet.y - enemy.y) < 12) {
            setGameOver(true);
            setScore((value) => value + 10);
          }
        });
      });
      setEnemies((items) => items.map((enemy) => ({ ...enemy, x: enemy.x + 1 })));
    });
    window.addEventListener("keydown", onKeyDown);
    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [bullets, enemies]);
  return <main><button onClick={fireBullet}>Fire</button><canvas /><p>score {score} enemy collision {gameOver ? "game over" : "playing"}</p></main>;
}
"#
}

fn contract_interactive_game_page_source() -> String {
    interactive_game_page_source()
            .replace(
                "<main>",
                r#"<main data-anvil-state={JSON.stringify({ score, gameOver, bulletCount: bullets.length, enemyCount: enemies.length })}>"#,
            )
            .replace(
                "<button onClick={fireBullet}>Start</button>",
                r#"<button data-anvil-action="primary" onClick={fireBullet}>Start</button>"#,
            )
            .replace(
                "<button onClick={restart}>Restart</button>",
                r#"<button data-anvil-action="restart" onClick={restart}>Restart</button>"#,
            )
}

fn contract_interactive_game_page_without_restart_source() -> String {
    interactive_game_page_without_restart_source()
            .replace(
                "<main>",
                r#"<main data-anvil-state={JSON.stringify({ score, gameOver, bulletCount: bullets.length, enemyCount: enemies.length })}>"#,
            )
            .replace(
                "<button onClick={fireBullet}>Fire</button>",
                r#"<button data-anvil-action="primary" onClick={fireBullet}>Fire</button>"#,
            )
}

fn overlay_only_restart_game_page_source() -> &'static str {
    r#""use client";
import { useEffect, useState } from "react";
export default function Page(){
  const [score, setScore] = useState(0);
  const [gameOver, setGameOver] = useState(false);
  const [screen, setScreen] = useState("menu");
  const [bullets, setBullets] = useState<{ x: number; y: number }[]>([]);
  const [enemies, setEnemies] = useState([{ x: 10, y: 20 }]);
  const fireBullet = () => {
    setScreen("playing");
    setBullets((items) => [...items, { x: 10, y: 90 }]);
    setScore((value) => value + 1);
  };
  const restart = () => {
    setGameOver(false);
    setScreen("menu");
    setScore(0);
    setBullets([]);
    setEnemies([{ x: 10, y: 20 }]);
  };
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "ArrowLeft" || event.key === " ") fireBullet();
    };
    const frame = requestAnimationFrame(() => {
      bullets.forEach((bullet) => {
        enemies.forEach((enemy) => {
          if (Math.abs(bullet.x - enemy.x) < 12 && Math.abs(bullet.y - enemy.y) < 12) {
            setGameOver(true);
            setScore((value) => value + 10);
          }
        });
      });
      setEnemies((items) => items.map((enemy) => ({ ...enemy, x: enemy.x + 1 })));
    });
    window.addEventListener("keydown", onKeyDown);
    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [bullets, enemies]);
  return (
    <main data-anvil-state={JSON.stringify({ screen, score, gameOver, bullets, enemies })}>
      <button data-anvil-action="primary" onClick={fireBullet}>Start</button>
      <canvas />
      <p>score {score} enemy collision {gameOver ? "game over" : screen}</p>
      {gameOver ? <button data-anvil-action="restart" onClick={restart}>Restart</button> : null}
    </main>
  );
}
"#
}

fn generated_data_mutation_plan_json(goal: &str) -> String {
    serde_json::to_string(&StepPlan {
        goal: goal.to_string(),
        steps: vec![PlanStep {
            id: "mutate-input".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Mutate input/source.csv".to_string(),
            expected_paths: vec!["input/source.csv".to_string()],
            verify: Vec::new(),
        }],
    })
    .unwrap()
}

fn enable_browser_probe_test_override(root: &Path) {
    std::fs::create_dir_all(root.join(".anvil")).unwrap();
    std::fs::write(root.join(".anvil/enable-browser-probe-tests"), "1").unwrap();
}

fn write_browser_probe_mock_command(root: &Path, status: &str) -> u16 {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let dir = root.join(".anvil/evidence");
    std::fs::create_dir_all(&dir).unwrap();
    let exe = std::env::current_exe().unwrap();
    let command = serde_json::json!({
        "program": exe.display().to_string(),
        "args": [
            "--ignored",
            "--exact",
            "minimal_loop::browser_probe::tests::browser_probe_mock_server_child",
            "--nocapture"
        ],
        "env": {
            "COMMANDAGENT_BROWSER_PROBE_MOCK_CHILD": "1",
            "COMMANDAGENT_BROWSER_PROBE_MOCK_PORT": port.to_string(),
            "COMMANDAGENT_BROWSER_PROBE_MOCK_STATUS": status,
            "COMMANDAGENT_BROWSER_PROBE_MOCK_DELAY_MS": "0"
        },
        "port": port,
        "require_build": false,
        "display": "mock browser probe child"
    });
    std::fs::write(
        dir.join("browser-probe-command.json"),
        serde_json::to_string_pretty(&command).unwrap(),
    )
    .unwrap();
    port
}

fn run_ignored_runner_harness(test_name: &str) -> std::process::ExitStatus {
    let exe = std::env::current_exe().unwrap();
    std::process::Command::new(exe)
        .args(["--ignored", "--exact", test_name, "--nocapture"])
        .env("NODE_ENV", "production")
        .status()
        .unwrap()
}

#[cfg(unix)]
fn forced_cleanup_timeout_after_real_cleanup(
    child: Child,
    logs: &DevServerLogPaths,
) -> DevServerCleanup {
    let _ = cleanup_dev_server_child(child, logs);
    DevServerCleanup {
        ok: false,
        failure_kind: Some("dev_server_cleanup_timeout".to_string()),
        output_excerpt: cleanup_timeout_excerpt(
            logs,
            &["forced cleanup timeout for test".to_string()],
        ),
    }
}

#[cfg(unix)]
fn enable_dev_server_probe_test_override(root: &Path) {
    std::fs::create_dir_all(root.join(".anvil")).unwrap();
    std::fs::write(root.join(".anvil/enable-dev-server-probe-tests"), "1").unwrap();
}

#[cfg(unix)]
fn write_fake_nextjs_dev_workspace(root: &Path, port: u16, spawn_grandchild: bool) {
    std::fs::write(
            root.join("package.json"),
            format!(
                r#"{{"scripts":{{"dev":"next dev -p {port}","build":"next build"}},"dependencies":{{"next":"^14.2.0","react":"^18.3.0","react-dom":"^18.3.0"}}}}"#
            ),
        )
        .unwrap();
    write_fake_nextjs_package_manager(root, spawn_grandchild);
}

#[cfg(unix)]
fn write_fake_nextjs_package_manager(root: &Path, spawn_grandchild: bool) {
    let bin = root.join("node_modules/.bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(root.join("node_modules/next")).unwrap();
    std::fs::create_dir_all(root.join("node_modules/tailwindcss")).unwrap();
    std::fs::create_dir_all(root.join("node_modules/postcss")).unwrap();
    std::fs::create_dir_all(root.join("node_modules/autoprefixer")).unwrap();
    let exe = shell_quote(&std::env::current_exe().unwrap().display().to_string());
    let grandchild = if spawn_grandchild { "1" } else { "0" };
    let script = format!(
        "#!/bin/sh\n\
if [ \"$1\" = \"run\" ] && [ \"$2\" = \"build\" ]; then\n\
  echo \"fake build ok\"\n\
  exit 0\n\
fi\n\
if [ \"$1\" = \"run\" ] && [ \"$2\" = \"dev\" ]; then\n\
  COMMANDAGENT_FAKE_DEV_SERVER_CHILD=1 COMMANDAGENT_FAKE_DEV_SERVER_GRANDCHILD={grandchild} exec {exe} --ignored --exact planner::runner::tests::fake_dev_server_package_manager_child --nocapture\n\
fi\n\
echo \"unexpected fake npm args: $*\" >&2\n\
exit 2\n"
    );
    let path = bin.join("npm");
    std::fs::write(&path, script).unwrap();
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    std::fs::set_permissions(&path, permissions).unwrap();
    let next_path = bin.join("next");
    std::fs::write(&next_path, "#!/bin/sh\nexit 0\n").unwrap();
    let mut permissions = std::fs::metadata(&next_path).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    std::fs::set_permissions(next_path, permissions).unwrap();
}

#[cfg(unix)]
fn write_fake_npm_dependency_installer(root: &Path) {
    let bin = root.join("node_modules/.bin");
    std::fs::create_dir_all(&bin).unwrap();
    let script = r#"#!/bin/sh
set -eu
install_pkg() {
  name="$1"
  if grep -q "\"$name\"" package.json 2>/dev/null; then
    mkdir -p "node_modules/$name"
    printf '{"name":"%s"}\n' "$name" > "node_modules/$name/package.json"
  fi
}
if [ "$1" = "install" ]; then
  mkdir -p node_modules/.bin
  install_pkg next
  install_pkg react
  install_pkg react-dom
  install_pkg typescript
  install_pkg @types/node
  install_pkg @types/react
  install_pkg @types/react-dom
  install_pkg tailwindcss
  install_pkg postcss
  install_pkg autoprefixer
  if [ -d node_modules/next ]; then
    printf '#!/bin/sh\nexit 0\n' > node_modules/.bin/next
    chmod +x node_modules/.bin/next
  fi
  printf '{"lockfileVersion":3}\n' > package-lock.json
  exit 0
fi
if [ "$1" = "run" ] && [ "$2" = "build" ]; then
  test -x node_modules/.bin/next || { echo "next missing" >&2; exit 1; }
  if grep -q "\"tailwindcss\"" package.json 2>/dev/null; then
    test -d node_modules/tailwindcss || { echo "tailwindcss missing" >&2; exit 1; }
    test -d node_modules/postcss || { echo "postcss missing" >&2; exit 1; }
    test -d node_modules/autoprefixer || { echo "autoprefixer missing" >&2; exit 1; }
  fi
  echo "fake build ok"
  exit 0
fi
echo "unexpected fake npm args: $*" >&2
exit 2
"#;
    let path = bin.join("npm");
    std::fs::write(&path, script).unwrap();
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    std::fs::set_permissions(path, permissions).unwrap();
}

#[cfg(unix)]
fn write_compile_error_fake_npm(root: &Path) {
    let bin = root.join("node_modules/.bin");
    std::fs::create_dir_all(&bin).unwrap();
    let script = r#"#!/bin/sh
set -eu
install_pkg() {
  name="$1"
  if grep -q "\"$name\"" package.json 2>/dev/null; then
    mkdir -p "node_modules/$name"
    printf '{"name":"%s"}\n' "$name" > "node_modules/$name/package.json"
  fi
}
if [ "$1" = "install" ]; then
  mkdir -p node_modules/.bin
  install_pkg next
  install_pkg react
  install_pkg react-dom
  install_pkg typescript
  install_pkg @types/node
  install_pkg @types/react
  install_pkg @types/react-dom
  install_pkg tailwindcss
  install_pkg postcss
  install_pkg autoprefixer
  if [ -d node_modules/next ]; then
    printf '#!/bin/sh\nexit 0\n' > node_modules/.bin/next
    chmod +x node_modules/.bin/next
  fi
  printf '{"lockfileVersion":3}\n' > package-lock.json
  exit 0
fi
if [ "$1" = "run" ] && [ "$2" = "build" ]; then
  cat >&2 <<'OUT'
./src/app/page.tsx
Error:
  x the name `player` is defined multiple times

   ,-[./src/app/page.tsx:479:1]
359 |       const player = playerRef.current;
    :             ------ previous definition of `player` here
479 |       const player = playerRef.current;
    :             ------ `player` redefined here
   `----
> Build failed because of webpack errors
OUT
  exit 1
fi
echo "unexpected fake npm args: $*" >&2
exit 2
"#;
    let path = bin.join("npm");
    std::fs::write(&path, script).unwrap();
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    std::fs::set_permissions(path, permissions).unwrap();
}

fn write_nextjs_dual_blocker_workspace(root: &Path) {
    std::fs::create_dir_all(root.join("src/app")).unwrap();
    std::fs::write(
            root.join("package.json"),
            r#"{"scripts":{"build":"next build"},"dependencies":{"next":"^14.2.0","react":"^18.3.0","react-dom":"^18.3.0"},"devDependencies":{"typescript":"^5.5.0","@types/node":"^20.14.0","@types/react":"^18.3.0","@types/react-dom":"^18.3.0","tailwindcss":"^3.4.19","postcss":"^8.5.15","autoprefixer":"^10.4.20"}}"#,
        )
        .unwrap();
    std::fs::write(root.join("tsconfig.json"), nextjs_tsconfig_json()).unwrap();
    std::fs::write(root.join("postcss.config.js"), nextjs_postcss_config()).unwrap();
    std::fs::write(root.join("tailwind.config.ts"), nextjs_tailwind_config_ts()).unwrap();
    std::fs::write(root.join("src/app/layout.tsx"), nextjs_layout_source()).unwrap();
    std::fs::write(root.join("src/app/globals.css"), nextjs_globals_css()).unwrap();
    std::fs::write(
        root.join("src/app/global.d.ts"),
        "declare module \"*.css\";\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/app/page.tsx"),
        r#""use client";
export default function Page() {
  if (true) {
    const player = { lives: 3 };
    const enemyBullets = [{ active: true }];
    const player = { lives: 2 };
    return <main><canvas data-anvil-primary-action />{enemyBullets.length}{player.lives}</main>;
  }
  return <main />;
}
"#,
    )
    .unwrap();
}

#[cfg(not(unix))]
fn write_fake_npm_dependency_installer(root: &Path) {
    let bin = root.join("node_modules/.bin");
    std::fs::create_dir_all(&bin).unwrap();
    let script = r#"@echo off
setlocal
if "%1"=="install" (
  if exist package.json (
    findstr /c:"\"next\"" package.json >nul && mkdir node_modules\next 2>nul
    findstr /c:"\"tailwindcss\"" package.json >nul && mkdir node_modules\tailwindcss 2>nul
    findstr /c:"\"postcss\"" package.json >nul && mkdir node_modules\postcss 2>nul
    findstr /c:"\"autoprefixer\"" package.json >nul && mkdir node_modules\autoprefixer 2>nul
    if exist node_modules\next (
      echo @echo off> node_modules\.bin\next.cmd
      echo exit /b 0>> node_modules\.bin\next.cmd
      echo {"name":"next"}> node_modules\next\package.json
    )
    if exist node_modules\tailwindcss echo {"name":"tailwindcss"}> node_modules\tailwindcss\package.json
    if exist node_modules\postcss echo {"name":"postcss"}> node_modules\postcss\package.json
    if exist node_modules\autoprefixer echo {"name":"autoprefixer"}> node_modules\autoprefixer\package.json
  )
  echo {"lockfileVersion":3}> package-lock.json
  exit /b 0
)
if "%1"=="run" if "%2"=="build" (
  if not exist node_modules\.bin\next.cmd exit /b 1
  if exist node_modules\tailwindcss (
    if not exist node_modules\postcss exit /b 1
    if not exist node_modules\autoprefixer exit /b 1
  )
  echo fake build ok
  exit /b 0
)
echo unexpected fake npm args: %*
exit /b 2
"#;
    std::fs::write(bin.join("npm.cmd"), script).unwrap();
}

#[cfg(unix)]
fn write_probe_nextjs_workspace(root: &Path, port: u16, page: &str) {
    std::fs::write(
            root.join("package.json"),
            format!(
                r#"{{"scripts":{{"build":"next build","dev":"next dev -p {port}","start":"next start -p {port}"}},"dependencies":{{"next":"^14.2.0","react":"^18.3.0","react-dom":"^18.3.0"}},"devDependencies":{{"typescript":"^5.5.0","@types/node":"^20.14.0","@types/react":"^18.3.0","@types/react-dom":"^18.3.0","tailwindcss":"^3.4.19","postcss":"^8.5.15","autoprefixer":"^10.4.20"}}}}"#
            ),
        )
        .unwrap();
    write_fake_nextjs_package_manager(root, false);
    std::fs::create_dir_all(root.join("src/app")).unwrap();
    std::fs::write(root.join("tsconfig.json"), nextjs_tsconfig_json()).unwrap();
    std::fs::write(root.join("postcss.config.js"), nextjs_postcss_config()).unwrap();
    std::fs::write(root.join("tailwind.config.ts"), nextjs_tailwind_config_ts()).unwrap();
    std::fs::write(root.join("src/app/page.tsx"), page).unwrap();
    std::fs::write(root.join("src/app/layout.tsx"), nextjs_layout_source()).unwrap();
    std::fs::write(root.join("src/app/globals.css"), nextjs_globals_css()).unwrap();
    std::fs::write(
        root.join("src/app/global.d.ts"),
        "declare module \"*.css\";\n",
    )
    .unwrap();
}

fn hollow_canvas_game_page_source() -> &'static str {
    r#""use client";
import { useState } from "react";
export default function Page(){
  const [mode, setMode] = useState("menu");
  return <main><button onClick={() => setMode("playing")}>Start</button><canvas /><p>score 0 health 3 {mode}</p></main>;
}
"#
}

fn unattached_canvas_ref_game_page_source() -> &'static str {
    r#""use client";
import { useEffect, useRef, useState } from "react";
import { useGame } from "./useGame";

export default function Page() {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const [score, setScore] = useState(0);
  const [gameOver, setGameOver] = useState(false);
  const [screen, setScreen] = useState("menu");
  const [bullets, setBullets] = useState<{ x: number; y: number }[]>([]);
  const [enemies, setEnemies] = useState([{ x: 10, y: 20 }]);
  useGame(canvasRef);
  const fireBullet = () => {
    setScreen("playing");
    setBullets((items) => [...items, { x: 10, y: 90 }]);
    setScore((value) => value + 1);
  };
  const restart = () => {
    setScreen("menu");
    setGameOver(false);
    setScore(0);
    setBullets([]);
    setEnemies([{ x: 10, y: 20 }]);
  };
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "ArrowLeft" || event.key === " ") {
        fireBullet();
      }
    };
    const frame = requestAnimationFrame(() => {
      bullets.forEach((bullet) => {
        enemies.forEach((enemy) => {
          if (Math.abs(bullet.x - enemy.x) < 12 && Math.abs(bullet.y - enemy.y) < 12) {
            setGameOver(true);
            setScore((value) => value + 10);
          }
        });
      });
      setEnemies((items) => items.map((enemy) => ({ ...enemy, x: enemy.x + 1 })));
    });
    window.addEventListener("keydown", onKeyDown);
    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [bullets, enemies]);
  return (
    <main data-anvil-state={JSON.stringify({ screen, score, gameOver, bullets, enemies })}>
      <button data-anvil-action="primary" onClick={fireBullet}>Start</button>
      <button data-anvil-action="restart" onClick={restart}>Restart</button>
      <canvas width={800} height={600} />
      <p>score {score} enemy collision {gameOver ? "game over" : screen}</p>
    </main>
  );
}
"#
}

fn attached_canvas_ref_game_page_source() -> String {
    unattached_canvas_ref_game_page_source().replace(
        "<canvas width={800} height={600} />",
        "<canvas ref={canvasRef} width={800} height={600} />",
    )
}

fn canvas_ref_game_hook_source() -> &'static str {
    r##"import { useEffect, type RefObject } from "react";

export function useGame(canvasRef: RefObject<HTMLCanvasElement | null>) {
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    let frame = 0;
    const draw = () => {
      frame += 1;
      ctx.fillStyle = "#111827";
      ctx.fillRect(0, 0, canvas.width, canvas.height);
      ctx.fillStyle = "#22c55e";
      ctx.fillRect(40 + frame, 500, 60, 20);
      requestAnimationFrame(draw);
    };
    const id = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(id);
  }, [canvasRef]);
}
"##
}

fn interaction_state_missing_probe_result() -> Value {
    serde_json::json!({
        "ok": false,
        "status": "failed",
        "interaction_success": false,
        "interaction_performed": false,
        "input_event_observed": true,
        "start_transition": true,
        "input_state_evaluated_after_start": true,
        "input_state_change": false,
        "state_changed": false,
        "visible_state_changed": false,
        "probe_mode": "heuristic",
        "contract_hook_status": "primary_missing",
        "candidate_table": [
            {"rank": 1, "index": 0, "text_excerpt": "", "changed": false},
            {"rank": 2, "index": 1, "text_excerpt": "Start", "changed": true}
        ],
        "input_dispatches": [
            "ArrowLeft keydown",
            "ArrowRight keydown",
            "Space keydown",
            "canvas/center click"
        ],
        "informational_failure_kinds": ["primary_start_transition_missing"],
        "steps": ["surface_visible", "start_transition", "control_input_dispatched", "input_state_evaluated_after_start"],
        "before_marker": "screen=menu score=0 health=3",
        "after_marker": "screen=playing score=0 health=3",
        "input_before_marker": "player=20 score=0 health=3",
        "input_after_marker": "player=20 score=0 health=3",
        "recovery_transition": true,
        "recovery_transition_status": "observed",
        "failure_kind": "input_state_change_missing_after_start",
        "duration_ms": 17
    })
}

fn interaction_state_changed_probe_result() -> Value {
    serde_json::json!({
        "ok": true,
        "status": "passed",
        "probe_mode": "contract",
        "contract_hook_status": "usable",
        "contract_hooks": {
            "usable": true,
            "primary_present": true,
            "restart_present": true,
            "valid_state_count": 1
        },
        "action_hooks": ["primary", "restart"],
        "state_dimensions_changed": ["playerX", "score"],
        "restart_hook_reachable_after_start": true,
        "restart_hook_count_after_start": 1,
        "interaction_success": true,
        "interaction_performed": true,
        "input_event_observed": true,
        "start_transition": true,
        "input_state_evaluated_after_start": true,
        "input_state_change": true,
        "state_changed": true,
        "visible_state_changed": true,
        "steps": [
            "surface_visible",
            "start_transition",
            "control_input_dispatched",
            "input_state_evaluated_after_start",
            "input_state_change",
            "recovery_transition"
        ],
        "before_marker": "screen=menu score=0 health=3",
        "after_marker": "screen=playing score=0 health=3",
        "input_before_marker": "player=20 score=0 health=3",
        "input_after_marker": "player=15 score=1 health=3",
        "recovery_transition": true,
        "recovery_transition_status": "observed",
        "duration_ms": 19
    })
}

fn recovery_not_observed_probe_result() -> Value {
    serde_json::json!({
        "ok": true,
        "status": "passed",
        "probe_mode": "contract",
        "contract_hook_status": "usable",
        "contract_hooks": {
            "usable": true,
            "primary_present": true,
            "restart_present": false,
            "valid_state_count": 1
        },
        "action_hooks": ["primary"],
        "state_dimensions_changed": ["playerX", "score"],
        "restart_hook_reachable_after_start": false,
        "restart_hook_count_after_start": 0,
        "interaction_success": true,
        "interaction_performed": true,
        "input_event_observed": true,
        "start_transition": true,
        "input_state_evaluated_after_start": true,
        "input_state_change": true,
        "state_changed": true,
        "visible_state_changed": true,
        "steps": [
            "surface_visible",
            "start_transition",
            "control_input_dispatched",
            "input_state_evaluated_after_start",
            "input_state_change",
            "recovery_transition:not_observed"
        ],
        "before_marker": "screen=menu",
        "after_marker": "screen=playing",
        "input_before_marker": "player=20 score=0",
        "input_after_marker": "player=15 score=1",
        "recovery_before_marker": "screen=playing",
        "recovery_after_marker": "screen=playing",
        "recovery_transition": false,
        "recovery_transition_status": "not_observed",
        "duration_ms": 23
    })
}

fn contract_interaction_pass_json() -> String {
    serde_json::to_string(&interaction_state_changed_probe_result()).unwrap()
}

#[cfg(unix)]
fn probe_nextjs_scaffold_tool_calls(
    port: u16,
    page: &str,
    check_path: &str,
) -> Vec<crate::state::ToolCall> {
    vec![
        crate::state::ToolCall::new(
            "Write",
            serde_json::json!({
                "path":"package.json",
                "content": format!(
                    r#"{{"scripts":{{"build":"next build","dev":"next dev -p {port}","start":"next start -p {port}"}},"dependencies":{{"next":"^14.2.0","react":"^18.3.0","react-dom":"^18.3.0"}},"devDependencies":{{"typescript":"^5.5.0","@types/node":"^20.14.0","@types/react":"^18.3.0","@types/react-dom":"^18.3.0","tailwindcss":"^3.4.19","postcss":"^8.5.15","autoprefixer":"^10.4.20"}}}}"#
                )
            }),
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
        crate::state::ToolCall::new(
            "Write",
            serde_json::json!({"path":check_path,"content":"x = 1\n"}),
        ),
    ]
}

#[cfg(unix)]
fn probe_nextjs_scaffold_reply(port: u16, page: String) -> AssistantReply {
    AssistantReply {
        content: String::new(),
        tool_calls: probe_nextjs_scaffold_tool_calls(port, &page, "check_scaffold.py"),
        prompt_tokens: None,
        completion_tokens: None,
    }
}

fn interactive_game_page_variant(label: usize) -> String {
    interactive_game_page_source()
        .replace("score {score}", &format!("score {{score}} health {label}"))
}

fn contract_interactive_game_page_variant(label: usize) -> String {
    contract_interactive_game_page_source()
        .replace("score {score}", &format!("score {{score}} health {label}"))
}

fn contract_interactive_game_page_without_restart_variant(label: usize) -> String {
    contract_interactive_game_page_without_restart_source()
        .replace("score {score}", &format!("score {{score}} health {label}"))
}

#[cfg(unix)]
fn write_compile_error_nextjs_workspace(root: &Path, port: u16) -> PathBuf {
    std::fs::create_dir_all(root.join("src/app")).unwrap();
    std::fs::create_dir_all(root.join("src/components")).unwrap();
    std::fs::create_dir_all(root.join(".anvil")).unwrap();
    std::fs::create_dir_all(root.join("node_modules/.bin")).unwrap();
    std::fs::create_dir_all(root.join("node_modules/next")).unwrap();
    std::fs::create_dir_all(root.join("node_modules/tailwindcss")).unwrap();
    std::fs::create_dir_all(root.join("node_modules/postcss")).unwrap();
    std::fs::create_dir_all(root.join("node_modules/autoprefixer")).unwrap();
    std::fs::write(root.join(".anvil/enable-browser-probe-tests"), "1").unwrap();
    std::fs::write(
            root.join("package.json"),
            format!(
                r#"{{"scripts":{{"build":"next build","dev":"next dev -p {port}","start":"next start -p {port}"}},"dependencies":{{"next":"^14.2.0","react":"^18.3.0","react-dom":"^18.3.0"}},"devDependencies":{{"typescript":"^5.5.0","@types/node":"^20.14.0","@types/react":"^18.3.0","@types/react-dom":"^18.3.0","tailwindcss":"^3.4.19","postcss":"^8.5.15","autoprefixer":"^10.4.20"}}}}"#
            ),
        )
        .unwrap();
    std::fs::write(root.join("node_modules/next/package.json"), "{}").unwrap();
    std::fs::write(root.join("node_modules/tailwindcss/package.json"), "{}").unwrap();
    std::fs::write(root.join("node_modules/postcss/package.json"), "{}").unwrap();
    std::fs::write(root.join("node_modules/autoprefixer/package.json"), "{}").unwrap();
    let component = r#""use client";
import { useState } from "react";
export function SpaceInvaders(){
  const [score, setScore] = useState(0);
  const fire = () => setScore((value) => value + 1);
  return <main><button onClick={fire}>Fire</button><button onClick={reset}>Restart</button><canvas /><p>score {score}</p></main>;
}
"#;
    std::fs::write(root.join("src/components/SpaceInvaders.tsx"), component).unwrap();
    std::fs::write(
        root.join("src/app/page.tsx"),
        "export default function Page(){return <main><button>Plain</button></main>;}\n",
    )
    .unwrap();
    std::fs::write(root.join("src/app/layout.tsx"), nextjs_layout_source()).unwrap();
    std::fs::write(root.join("src/app/globals.css"), nextjs_globals_css()).unwrap();
    std::fs::write(
        root.join("src/app/global.d.ts"),
        "declare module \"*.css\";\n",
    )
    .unwrap();
    std::fs::write(root.join("tsconfig.json"), nextjs_tsconfig_json()).unwrap();
    std::fs::write(root.join("postcss.config.js"), nextjs_postcss_config()).unwrap();
    std::fs::write(root.join("tailwind.config.ts"), nextjs_tailwind_config_ts()).unwrap();
    let exe = shell_quote(&std::env::current_exe().unwrap().display().to_string());
    let script = format!(
        "#!/bin/sh\n\
if [ \"$1\" = \"run\" ] && [ \"$2\" = \"build\" ]; then\n\
  if grep -q 'onClick={{reset}}' src/components/SpaceInvaders.tsx && ! grep -q 'const reset' src/components/SpaceInvaders.tsx; then\n\
    echo './src/components/SpaceInvaders.tsx:137:28' >&2\n\
    echo \"Type error: Cannot find name 'reset'.\" >&2\n\
    exit 1\n\
  fi\n\
  echo 'fake build ok'\n\
  exit 0\n\
fi\n\
if [ \"$1\" = \"run\" ] && {{ [ \"$2\" = \"dev\" ] || [ \"$2\" = \"start\" ]; }}; then\n\
  COMMANDAGENT_FAKE_DEV_SERVER_CHILD=1 COMMANDAGENT_FAKE_DEV_SERVER_GRANDCHILD=0 exec {exe} --ignored --exact planner::runner::tests::fake_dev_server_package_manager_child --nocapture\n\
fi\n\
echo \"unexpected fake npm args: $*\" >&2\n\
exit 2\n"
    );
    let npm = root.join("node_modules/.bin/npm");
    std::fs::write(&npm, script).unwrap();
    let mut permissions = std::fs::metadata(&npm).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    std::fs::set_permissions(&npm, permissions).unwrap();
    let next_path = root.join("node_modules/.bin/next");
    std::fs::write(&next_path, "#!/bin/sh\nexit 0\n").unwrap();
    let mut permissions = std::fs::metadata(&next_path).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    std::fs::set_permissions(next_path, permissions).unwrap();
    let contract_path = root.join("completion-contract.json");
    std::fs::write(
        &contract_path,
        serde_json::to_string_pretty(&serde_json::json!({
            "required_paths": ["src/app/page.tsx", "src/components/SpaceInvaders.tsx"],
            "verify_commands": ["npm run build"],
            "profile": "nextjs",
            "goal": explicit_port_goal("Create an interactive browser app", port),
            "required_capabilities": ["playable_ui", "stateful_interaction"],
            "verify_repair_cap": 2
        }))
        .unwrap(),
    )
    .unwrap();
    contract_path
}

#[cfg(unix)]
fn write_api_mismatch_build_shim(root: &Path) {
    std::fs::create_dir_all(root.join("node_modules/.bin")).unwrap();
    std::fs::create_dir_all(root.join("node_modules/next")).unwrap();
    let script = "#!/bin/sh\n\
if [ \"$1\" = \"run\" ] && [ \"$2\" = \"build\" ]; then\n\
  if grep -q 'onStateChange' src/app/SpaceInvadersGame.tsx 2>/dev/null; then\n\
    echo './src/app/SpaceInvadersGame.tsx:30:12' >&2\n\
    echo \"Type error: Property 'onStateChange' does not exist on type 'SpaceInvadersEngine'.\" >&2\n\
    exit 1\n\
  fi\n\
  if ! grep -q 'getState' src/app/SpaceInvadersGame.tsx 2>/dev/null; then\n\
    echo './src/app/SpaceInvadersGame.tsx:30:12' >&2\n\
    echo \"Type error: expected poll-based getState repair.\" >&2\n\
    exit 1\n\
  fi\n\
  echo 'fake build ok'\n\
  exit 0\n\
fi\n\
echo \"unexpected fake npm args: $*\" >&2\n\
exit 2\n";
    let npm = root.join("node_modules/.bin/npm");
    std::fs::write(&npm, script).unwrap();
    let mut permissions = std::fs::metadata(&npm).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    std::fs::set_permissions(&npm, permissions).unwrap();
    let next_path = root.join("node_modules/.bin/next");
    std::fs::write(&next_path, "#!/bin/sh\nexit 0\n").unwrap();
    let mut permissions = std::fs::metadata(&next_path).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    std::fs::set_permissions(next_path, permissions).unwrap();
}

fn api_mismatch_step_plan() -> StepPlan {
    StepPlan {
        goal: "Fix a Next.js TypeScript API mismatch on port 3011".to_string(),
        steps: vec![PlanStep {
            id: "verify-build".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Create the route-bound game component and engine, then build."
                .to_string(),
            expected_paths: vec![
                "src/app/page.tsx".to_string(),
                "src/app/SpaceInvadersGame.tsx".to_string(),
                "src/lib/game-engine.ts".to_string(),
                "package.json".to_string(),
            ],
            verify: vec!["npm run build".to_string()],
        }],
    }
}

fn api_mismatch_initial_reply(port: u16) -> AssistantReply {
    AssistantReply {
        content: String::new(),
        tool_calls: vec![
            crate::state::ToolCall::new(
                "Write",
                serde_json::json!({
                    "path": "package.json",
                    "content": format!(
                        r#"{{"scripts":{{"build":"next build","dev":"next dev -p {port}","start":"next start -p {port}"}},"dependencies":{{"next":"^14.2.0","react":"^18.3.0","react-dom":"^18.3.0"}}}}"#
                    )
                }),
            ),
            crate::state::ToolCall::new(
                "Write",
                serde_json::json!({"path":"src/app/page.tsx","content":"import SpaceInvadersGame from \"./SpaceInvadersGame\";\n\nexport default function Page() {\n  return <SpaceInvadersGame />;\n}\n"}),
            ),
            crate::state::ToolCall::new(
                "Write",
                serde_json::json!({"path":"src/app/SpaceInvadersGame.tsx","content":api_mismatch_broken_game_source()}),
            ),
            crate::state::ToolCall::new(
                "Write",
                serde_json::json!({"path":"src/lib/game-engine.ts","content":api_mismatch_engine_source()}),
            ),
        ],
        prompt_tokens: None,
        completion_tokens: None,
    }
}

fn api_mismatch_poll_fix_reply() -> AssistantReply {
    AssistantReply {
        content: String::new(),
        tool_calls: vec![crate::state::ToolCall::new(
            "Write",
            serde_json::json!({"path":"src/app/SpaceInvadersGame.tsx","content":api_mismatch_poll_fixed_game_source()}),
        )],
        prompt_tokens: None,
        completion_tokens: None,
    }
}

fn api_mismatch_read_only_reply() -> AssistantReply {
    AssistantReply {
        content: String::new(),
        tool_calls: vec![crate::state::ToolCall::new(
            "Read",
            serde_json::json!({"path":"src/app/SpaceInvadersGame.tsx"}),
        )],
        prompt_tokens: None,
        completion_tokens: None,
    }
}

fn api_mismatch_insufficient_game_source() -> &'static str {
    r#""use client";
import { useRef, useState } from "react";
import { SpaceInvadersEngine, type GameState } from "../lib/game-engine";

export default function SpaceInvadersGame() {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const [gameState] = useState<GameState>({ score: 0, status: "ready" });
  void SpaceInvadersEngine;
  return <main data-anvil-state={JSON.stringify(gameState)}><canvas ref={canvasRef} /></main>;
}
"#
}

fn api_mismatch_broken_game_source() -> &'static str {
    r#""use client";
import { useEffect, useRef, useState } from "react";
import { SpaceInvadersEngine, type GameState } from "../lib/game-engine";

export default function SpaceInvadersGame() {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const [gameState, setGameState] = useState<GameState>({ score: 0, status: "ready" });
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const engine = new SpaceInvadersEngine(canvas);
    engine.onStateChange((state) => {
      setGameState({ ...state });
    });
    engine.start();
    return () => engine.destroy();
  }, []);
  return <main data-anvil-state={JSON.stringify(gameState)}><canvas ref={canvasRef} /></main>;
}
"#
}

fn api_mismatch_poll_fixed_game_source() -> &'static str {
    r#""use client";
import { useEffect, useRef, useState } from "react";
import { SpaceInvadersEngine, type GameState } from "../lib/game-engine";

export default function SpaceInvadersGame() {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const [gameState, setGameState] = useState<GameState>({ score: 0, status: "ready" });
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const engine = new SpaceInvadersEngine(canvas);
    let raf = 0;
    const tick = () => {
      setGameState({ ...engine.getState() });
      raf = requestAnimationFrame(tick);
    };
    engine.start();
    raf = requestAnimationFrame(tick);
    return () => {
      cancelAnimationFrame(raf);
      engine.destroy();
    };
  }, []);
  return <main data-anvil-state={JSON.stringify(gameState)}><canvas ref={canvasRef} /></main>;
}
"#
}

fn api_mismatch_engine_source() -> &'static str {
    r#"export interface GameState {
  score: number;
  status: string;
}

export class SpaceInvadersEngine {
  private state: GameState = { score: 0, status: "ready" };
  public start() { this.state = { ...this.state, status: "playing" }; }
  public pause() { this.state = { ...this.state, status: "paused" }; }
  public reset() { this.state = { score: 0, status: "ready" }; }
  public setKey(key: string, pressed: boolean) { void key; void pressed; }
  public getState(): GameState { return this.state; }
  public destroy() { this.state = { ...this.state, status: "destroyed" }; }
}
"#
}

fn generated_nextjs_artifact_plan_json_with_build_verify(goal: &str) -> String {
    let expected_paths = nextjs_scaffold_expected_paths();
    serde_json::to_string(&StepPlan {
        goal: goal.to_string(),
        steps: vec![PlanStep {
            id: "create-nextjs-artifacts".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: format!(
                "Create a coherent Next.js scaffold with {}",
                expected_paths.join(", ")
            ),
            expected_paths,
            verify: vec!["npm run build".to_string()],
        }],
    })
    .unwrap()
}

#[cfg(unix)]
fn static_good_page_source() -> &'static str {
    "export default function Page(){return <main>Recovered static app</main>;}\n"
}

#[cfg(unix)]
fn static_broken_page_source() -> &'static str {
    "export default function Page(){\n  return (\n    <main>\n      <p>Broken</p>\nBROKEN_SYNTAX\n    </main>\n  );\n}\n"
}

#[cfg(unix)]
fn write_static_compile_repair_workspace(root: &Path, page: &str) {
    std::fs::create_dir_all(root.join("src/app")).unwrap();
    std::fs::create_dir_all(root.join("node_modules/.bin")).unwrap();
    for package in ["next", "tailwindcss", "postcss", "autoprefixer"] {
        let dir = root.join("node_modules").join(package);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("package.json"), "{}").unwrap();
    }
    std::fs::write(root.join("package.json"), nextjs_complete_package_json()).unwrap();
    std::fs::write(root.join("tsconfig.json"), nextjs_tsconfig_json()).unwrap();
    std::fs::write(root.join("postcss.config.js"), nextjs_postcss_config()).unwrap();
    std::fs::write(root.join("tailwind.config.ts"), nextjs_tailwind_config_ts()).unwrap();
    std::fs::write(root.join("src/app/page.tsx"), page).unwrap();
    std::fs::write(root.join("src/app/layout.tsx"), nextjs_layout_source()).unwrap();
    std::fs::write(root.join("src/app/globals.css"), nextjs_globals_css()).unwrap();
    std::fs::write(
        root.join("src/app/global.d.ts"),
        "declare module \"*.css\";\n",
    )
    .unwrap();
    let page_path = root.join("src/app/page.tsx");
    let script = format!(
        "#!/bin/sh\n\
if [ \"$1\" = \"run\" ] && [ \"$2\" = \"build\" ]; then\n\
  if grep -q 'BROKEN_SYNTAX' src/app/page.tsx; then\n\
    echo 'Failed to compile.' >&2\n\
    echo './src/app/page.tsx' >&2\n\
    echo 'Error:' >&2\n\
    echo \"  x Expected ';', '}}' or <eof>\" >&2\n\
    echo '   ,-[{}:12:1]' >&2\n\
    echo ' 9 |   return (' >&2\n\
    echo '10 |     <main>' >&2\n\
    echo '11 |       <p>Broken</p>' >&2\n\
    echo '12 | BROKEN_SYNTAX' >&2\n\
    echo '   | ^' >&2\n\
    exit 1\n\
  fi\n\
  echo 'fake build ok'\n\
  exit 0\n\
fi\n\
echo \"unexpected fake npm args: $*\" >&2\n\
exit 2\n",
        page_path.display()
    );
    let npm = root.join("node_modules/.bin/npm");
    std::fs::write(&npm, script).unwrap();
    let mut permissions = std::fs::metadata(&npm).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    std::fs::set_permissions(&npm, permissions).unwrap();
    let next = root.join("node_modules/.bin/next");
    std::fs::write(&next, "#!/bin/sh\nexit 0\n").unwrap();
    let mut permissions = std::fs::metadata(&next).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    std::fs::set_permissions(next, permissions).unwrap();
}

fn write_static_build_contract(root: &Path) -> PathBuf {
    let path = root.join("completion-contract.json");
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&serde_json::json!({
            "required_paths": ["src/app/page.tsx"],
            "verify_commands": ["npm run build"],
            "profile": "nextjs",
            "goal": "Create a static Next.js page",
            "required_capabilities": [],
            "required_evidence": ["implementation_artifact"],
            "verify_repair_cap": 2
        }))
        .unwrap(),
    )
    .unwrap();
    path
}

fn static_compile_repair_plan() -> UltraPlan {
    UltraPlan {
        goal: "Create a static Next.js page".to_string(),
        profile: "nextjs".to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: vec![
            UltraPhase {
                id: "phase-one".to_string(),
                prompt: "Keep the current compiling static page.".to_string(),
            },
            UltraPhase {
                id: "phase-two".to_string(),
                prompt: "Update the page copy for the final app.".to_string(),
            },
        ],
    }
}

fn static_phase_step_plan_json(verify_build: bool) -> String {
    serde_json::to_string(&StepPlan {
        goal: "Create a static Next.js page".to_string(),
        steps: vec![PlanStep {
            id: if verify_build {
                "verify-static-build".to_string()
            } else {
                "update-static-page".to_string()
            },
            kind: if verify_build {
                "verify".to_string()
            } else {
                "setup".to_string()
            },
            expected_result: "static page is present".to_string(),
            instruction: if verify_build {
                "Verify the current static Next.js page build".to_string()
            } else {
                "Update src/app/page.tsx for the static app".to_string()
            },
            expected_paths: if verify_build {
                Vec::new()
            } else {
                vec!["src/app/page.tsx".to_string()]
            },
            verify: if verify_build {
                vec!["npm run build".to_string()]
            } else {
                Vec::new()
            },
        }],
    })
    .unwrap()
}

#[cfg(unix)]
fn static_breaking_build_step_plan() -> StepPlan {
    StepPlan {
        goal: "Create a static Next.js page".to_string(),
        steps: vec![PlanStep {
            id: "break-then-verify-build".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Update src/app/page.tsx and verify the build.".to_string(),
            expected_paths: vec!["src/app/page.tsx".to_string()],
            verify: vec!["npm run build".to_string()],
        }],
    }
}

#[cfg(unix)]
fn static_breaking_build_step_plan_json() -> String {
    serde_json::to_string(&static_breaking_build_step_plan()).unwrap()
}

fn write_static_page_reply(content: &str) -> AssistantReply {
    AssistantReply {
        content: String::new(),
        tool_calls: vec![crate::state::ToolCall::new(
            "Write",
            serde_json::json!({"path":"src/app/page.tsx","content":content}),
        )],
        prompt_tokens: None,
        completion_tokens: None,
    }
}

fn read_static_page_reply() -> AssistantReply {
    AssistantReply {
        content: String::new(),
        tool_calls: vec![crate::state::ToolCall::new(
            "Read",
            serde_json::json!({"path":"src/app/page.tsx"}),
        )],
        prompt_tokens: None,
        completion_tokens: None,
    }
}

#[cfg(unix)]
fn bash_true_reply() -> AssistantReply {
    AssistantReply {
        content: String::new(),
        tool_calls: vec![crate::state::ToolCall::new(
            "Bash",
            serde_json::json!({"command":"true"}),
        )],
        prompt_tokens: None,
        completion_tokens: None,
    }
}

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

    let result = run_ultra_plan(&mut planner, &mut execution, &plan, &cfg).unwrap_or_else(|err| {
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
        phase_three_prompt.contains("phase phase-two changes to src/app/page.tsx were rolled back"),
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

    let result = run_ultra_plan(&mut planner, &mut execution, &plan, &cfg).unwrap_or_else(|err| {
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
    let targets = final_acceptance_recovery_repair_targets(&report, RepairTarget::Implementation);
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

    let repair_text = std::fs::read_dir(dir.path().join(".anvil/repairs"))
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

#[cfg(unix)]
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(unix)]
static DEV_SERVER_PROBE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(unix)]
fn dev_server_probe_test_guard() -> std::sync::MutexGuard<'static, ()> {
    DEV_SERVER_PROBE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(unix)]
static TEST_DEV_SERVER_PORT: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(34_011);

#[cfg(unix)]
fn free_local_port() -> u16 {
    for _ in 0..2_000 {
        let port = TEST_DEV_SERVER_PORT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if port > u16::MAX as usize {
            break;
        }
        let port = port as u16;
        if port == NEXTJS_DEV_SERVER_DEFAULT_PORT {
            continue;
        }
        if test_dev_server_port_is_available(port) {
            return port;
        }
    }
    loop {
        match std::net::TcpListener::bind(("127.0.0.1", 0)) {
            Ok(listener) => {
                let port = listener.local_addr().unwrap().port();
                drop(listener);
                if port != NEXTJS_DEV_SERVER_DEFAULT_PORT && test_dev_server_port_is_available(port)
                {
                    return port;
                }
            }
            Err(_) => return NEXTJS_DEV_SERVER_DEFAULT_PORT + 1,
        }
    }
}

#[cfg(unix)]
fn test_dev_server_port_is_available(port: u16) -> bool {
    let Ok(listener) = std::net::TcpListener::bind(("127.0.0.1", port)) else {
        return false;
    };
    drop(listener);
    !localhost_port_accepts_connection(port)
}

fn read_jsonl_events(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect()
}

fn dev_server_stage_names(events: &[Value]) -> Vec<&str> {
    events
        .iter()
        .filter(|event| event.get("event").and_then(Value::as_str) == Some("dev_server_lifecycle"))
        .filter_map(|event| event.get("stage").and_then(Value::as_str))
        .collect()
}

#[cfg(unix)]
fn wait_until_process_group_gone(pgid: u32, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if !process_group_exists(pgid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    !process_group_exists(pgid)
}

#[cfg(unix)]
fn process_group_exists(pgid: u32) -> bool {
    let Ok(pgid) = i32::try_from(pgid) else {
        return false;
    };
    // SAFETY: signal 0 performs existence/permission checking only, using
    // a process-group id originally emitted from a spawned child pid.
    let rc = unsafe { libc::kill(-pgid, 0) };
    if rc == 0 {
        return true;
    }
    let err = std::io::Error::last_os_error();
    err.raw_os_error() != Some(libc::ESRCH)
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
        prompt_layout: crate::config::PromptLayout::Stable,
        plan_preset: crate::config::PlanPreset::None,
        intent_override: None,
        planner_model: "m".to_string(),
        planner_provider: crate::config::Provider::Ollama,
        ollama_host: "http://localhost:11434".to_string(),
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
