mod support;

use commandagent::agent::minimal_loop::loop_run::{
    MinimalLoopConfig, MinimalLoopError, run_session,
};
use commandagent::providers::{ChatResponse, ChatRole, ToolCall, ToolCallMode};
use std::fs;
use support::{MockChatClient, temp_workspace};

#[test]
fn xml_parse_failure_downgrades_public_session() {
    let root = temp_workspace("xml-downgrade");
    let mut client = MockChatClient::new(vec![
        ChatResponse {
            content: "<commandagent_tool_call>{\"name\":\"Write\"".to_string(),
            tool_calls: Vec::new(),
        },
        ChatResponse {
            content: "No changes needed.".to_string(),
            tool_calls: Vec::new(),
        },
        ChatResponse {
            content: "No file changes were needed.".to_string(),
            tool_calls: Vec::new(),
        },
    ]);

    let result = run_session(
        &mut client,
        &root,
        "inspect without changing files",
        MinimalLoopConfig::default(),
    )
    .unwrap();

    assert_eq!(result.tool_call_mode, ToolCallMode::XmlFallback);
    assert_eq!(client.requests().len(), 3);
    assert_eq!(client.requests()[0].tool_call_mode, ToolCallMode::Native);
    assert_eq!(
        client.requests()[1].tool_call_mode,
        ToolCallMode::XmlFallback
    );
    assert_eq!(
        client.requests()[2].tool_call_mode,
        ToolCallMode::XmlFallback
    );
    assert!(request_contains_user_message(
        &client.requests()[1],
        "Use XML fallback format"
    ));
    assert!(request_contains_user_message(
        &client.requests()[2],
        "No file changes have been made"
    ));
}

#[test]
fn requested_artifact_feedback_is_public_behavior() {
    let root = temp_workspace("artifact-feedback");
    let mut client = MockChatClient::new(vec![
        ChatResponse {
            content: "Created the requested artifact.".to_string(),
            tool_calls: Vec::new(),
        },
        ChatResponse {
            content: String::new(),
            tool_calls: vec![ToolCall {
                name: "Write".to_string(),
                args_json: r#"{"path":"dist/report.md","content":"ok"}"#.to_string(),
            }],
        },
        ChatResponse {
            content: "Created dist/report.md.".to_string(),
            tool_calls: Vec::new(),
        },
    ]);
    let config = MinimalLoopConfig {
        expected_artifacts: vec!["dist/report.md".to_string()],
        ..MinimalLoopConfig::default()
    };

    let result = run_session(&mut client, &root, "create report", config).unwrap();

    assert_eq!(result.final_answer, "Created dist/report.md.");
    assert_eq!(
        fs::read_to_string(root.join("dist/report.md")).unwrap(),
        "ok"
    );
    assert!(request_contains_user_message(
        &client.requests()[1],
        "requested artifact paths are still missing"
    ));
}

#[test]
fn completion_without_write_feedback_fires_once() {
    let root = temp_workspace("no-write-feedback");
    let mut client = MockChatClient::new(vec![
        ChatResponse {
            content: "No changes needed.".to_string(),
            tool_calls: Vec::new(),
        },
        ChatResponse {
            content: "No file changes were needed.".to_string(),
            tool_calls: Vec::new(),
        },
    ]);

    let result = run_session(
        &mut client,
        &root,
        "summarize current state",
        MinimalLoopConfig::default(),
    )
    .unwrap();

    assert_eq!(result.iterations, 2);
    assert_eq!(result.final_answer, "No file changes were needed.");
    assert!(request_contains_user_message(
        &client.requests()[1],
        "No file changes have been made"
    ));
}

#[test]
fn missing_artifact_after_feedback_is_public_error() {
    let root = temp_workspace("artifact-error");
    let mut client = MockChatClient::new(vec![
        ChatResponse {
            content: "Created the requested artifact.".to_string(),
            tool_calls: Vec::new(),
        },
        ChatResponse {
            content: "The requested artifact is complete.".to_string(),
            tool_calls: Vec::new(),
        },
    ]);
    let config = MinimalLoopConfig {
        expected_artifacts: vec!["missing.md".to_string()],
        ..MinimalLoopConfig::default()
    };

    let err = run_session(&mut client, &root, "create missing artifact", config).unwrap_err();

    assert_eq!(
        err,
        MinimalLoopError::MissingArtifacts(vec!["missing.md".to_string()])
    );
}

fn request_contains_user_message(
    request: &commandagent::providers::ChatRequest,
    needle: &str,
) -> bool {
    request
        .messages
        .iter()
        .any(|message| message.role == ChatRole::User && message.content.contains(needle))
}
