mod support;

use commandagent::agent::minimal_loop::loop_run::MinimalLoopConfig;
use commandagent::agent::slash_command::{SlashCommand, SlashCommandKind};
use commandagent::agent::step_runner::runtime::{PlannerRuntimeConfig, SlashRuntime};
use commandagent::providers::{ChatResponse, ToolCall, ToolCallMode};
use std::fs;
use support::{MockChatClient, temp_workspace};

#[test]
fn plan_run_creates_expected_file() {
    let root = temp_workspace("plan-run-create");
    let mut planner = MockChatClient::new(vec![text_response(readme_plan("Create README"))]);
    let mut executor =
        MockChatClient::new(write_responses("README.md", "ok", "Created README.md."));
    let mut runtime = runtime(&mut executor, &mut planner, &root);

    let output = runtime
        .run(plan_run_command("Create README", "docs", vec![]))
        .unwrap();

    assert_eq!(fs::read_to_string(root.join("README.md")).unwrap(), "ok");
    assert!(output.contains("step write-readme: ok"));
}

#[test]
fn invalid_plan_is_corrected_before_execution() {
    let root = temp_workspace("invalid-plan-correction");
    let mut planner = MockChatClient::new(vec![
        text_response(invalid_expected_path_plan()),
        text_response(readme_plan("Create README")),
    ]);
    let mut executor =
        MockChatClient::new(write_responses("README.md", "ok", "Created README.md."));
    let mut runtime = runtime(&mut executor, &mut planner, &root);

    let output = runtime
        .run(plan_run_command("Create README", "docs", vec![]))
        .unwrap();

    assert!(output.contains("step write-readme: ok"));
    assert_eq!(fs::read_to_string(root.join("README.md")).unwrap(), "ok");
    let invalid_plans = fs::read_dir(root.join(".commandagent/plans"))
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("invalid-step-plan-")
        })
        .count();
    assert_eq!(invalid_plans, 1);
}

#[test]
fn repair_packet_is_saved_after_bounded_failure() {
    let root = temp_workspace("repair-packet");
    let mut planner = MockChatClient::new(vec![text_response(failing_verify_plan())]);
    let mut executor = MockChatClient::new(vec![
        no_change_response(),
        no_change_response(),
        no_change_response(),
        no_change_response(),
        no_change_response(),
        no_change_response(),
    ]);
    let mut runtime = runtime(&mut executor, &mut planner, &root);

    let err = runtime
        .run(plan_run_command("Verify missing file", "generic", vec![]))
        .unwrap_err();

    assert!(err.contains("repair prompt saved"));
    assert!(err.contains(".commandagent/repairs/repair-verify-missing-"));
    let repair_count = fs::read_dir(root.join(".commandagent/repairs"))
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("md"))
        .count();
    assert_eq!(repair_count, 1);
}

#[test]
fn ultra_plan_allows_final_artifact_to_be_created_in_later_phase() {
    let root = temp_workspace("ultra-final-boundary");
    let mut planner = MockChatClient::new(vec![
        text_response(ultra_plan()),
        text_response(phase_plan("phase-one-file", "phase1.txt")),
        text_response(phase_plan("phase-two-final", "FINAL.md")),
    ]);
    let mut executor = MockChatClient::new(
        [
            write_responses("phase1.txt", "phase one", "Created phase1.txt."),
            write_responses("FINAL.md", "done", "Created FINAL.md."),
        ]
        .concat(),
    );
    let mut runtime = runtime(&mut executor, &mut planner, &root);

    let output = runtime
        .run(ultra_plan_run_command(
            "Create final artifact in phases",
            "docs",
            vec!["FINAL.md".to_string()],
        ))
        .unwrap();

    assert_eq!(
        fs::read_to_string(root.join("phase1.txt")).unwrap(),
        "phase one"
    );
    assert_eq!(fs::read_to_string(root.join("FINAL.md")).unwrap(), "done");
    assert!(output.contains("phase phase-one: ok"));
    assert!(output.contains("phase phase-two: ok"));
}

fn runtime<'a>(
    executor: &'a mut MockChatClient,
    planner: &'a mut MockChatClient,
    cwd: &'a std::path::Path,
) -> SlashRuntime<'a, MockChatClient, MockChatClient> {
    SlashRuntime {
        executor,
        planner,
        cwd,
        loop_config: MinimalLoopConfig::default(),
        planner_config: PlannerRuntimeConfig {
            model: "planner".to_string(),
            tool_call_mode: ToolCallMode::XmlFallback,
        },
    }
}

fn plan_run_command(argument: &str, profile: &str, artifacts: Vec<String>) -> SlashCommand {
    SlashCommand {
        kind: SlashCommandKind::PlanRun,
        profile: Some(profile.to_string()),
        style: Some("default".to_string()),
        intent: Some("document".to_string()),
        artifacts,
        argument: argument.to_string(),
    }
}

fn ultra_plan_run_command(argument: &str, profile: &str, artifacts: Vec<String>) -> SlashCommand {
    SlashCommand {
        kind: SlashCommandKind::UltraPlanRun,
        profile: Some(profile.to_string()),
        style: Some("default".to_string()),
        intent: Some("document".to_string()),
        artifacts,
        argument: argument.to_string(),
    }
}

fn text_response(content: impl Into<String>) -> ChatResponse {
    ChatResponse {
        content: content.into(),
        tool_calls: Vec::new(),
    }
}

fn no_change_response() -> ChatResponse {
    text_response("No file changes were needed.")
}

fn write_responses(path: &str, content: &str, final_answer: &str) -> Vec<ChatResponse> {
    vec![
        ChatResponse {
            content: String::new(),
            tool_calls: vec![ToolCall {
                name: "Write".to_string(),
                args_json: format!(
                    r#"{{"path":"{}","content":"{}"}}"#,
                    json_escape(path),
                    json_escape(content)
                ),
            }],
        },
        text_response(final_answer),
    ]
}

fn readme_plan(goal: &str) -> String {
    format!(
        r#"goal: "{goal}"
profile: "docs"
style: "default"
intent: "document"
required_artifacts: []
steps:
  - id: "write-readme"
    kind: "create"
    instruction: "Create README.md."
    expected_result: "pass"
    expected_paths:
      - "README.md"
    verify:
      - "test -f README.md"
"#
    )
}

fn invalid_expected_path_plan() -> &'static str {
    r#"goal: "Create README"
profile: "docs"
style: "default"
intent: "document"
required_artifacts: []
steps:
  - id: "bad-path"
    kind: "create"
    instruction: "Create an invalid expected path."
    expected_result: "pass"
    expected_paths:
      - "1.0"
    verify: []
"#
}

fn failing_verify_plan() -> &'static str {
    r#"goal: "Verify missing file"
profile: "generic"
style: "default"
intent: "unknown"
required_artifacts: []
steps:
  - id: "verify-missing"
    kind: "verify"
    instruction: "Verify that missing.txt exists."
    expected_result: "pass"
    expected_paths: []
    verify:
      - "test -f missing.txt"
"#
}

fn ultra_plan() -> &'static str {
    r#"goal: "Create final artifact in phases"
profile: "docs"
style: "default"
intent: "document"
required_artifacts:
  - "FINAL.md"
phases:
  - id: "phase-one"
    goal: "Create an intermediate file."
  - id: "phase-two"
    goal: "Create the final artifact."
"#
}

fn phase_plan(step_id: &str, path: &str) -> String {
    format!(
        r#"goal: "phase"
profile: "docs"
style: "default"
intent: "document"
required_artifacts: []
steps:
  - id: "{step_id}"
    kind: "create"
    instruction: "Create {path}."
    expected_result: "pass"
    expected_paths:
      - "{path}"
    verify:
      - "test -f {path}"
"#
    )
}

fn json_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
