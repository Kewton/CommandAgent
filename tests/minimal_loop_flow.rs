mod support;

use commandagent::agent::minimal_loop::config::ActionRequirement;
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
    ]);

    let result = run_session(
        &mut client,
        &root,
        "inspect without changing files",
        MinimalLoopConfig::default(),
    )
    .unwrap();

    assert_eq!(result.tool_call_mode, ToolCallMode::XmlFallback);
    assert_eq!(client.requests().len(), 2);
    assert_eq!(client.requests()[0].tool_call_mode, ToolCallMode::Native);
    assert_eq!(
        client.requests()[1].tool_call_mode,
        ToolCallMode::XmlFallback
    );
    assert!(request_contains_user_message(
        &client.requests()[1],
        "Use XML fallback format"
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
                id: None,
                thought_signature: None,
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
fn xml_fallback_prompt_exposes_tool_argument_shapes() {
    let root = temp_workspace("xml-prompt");
    let mut client = MockChatClient::new(vec![
        ChatResponse {
            content: "No changes needed.".to_string(),
            tool_calls: Vec::new(),
        },
        ChatResponse {
            content: String::new(),
            tool_calls: vec![ToolCall {
                id: None,
                thought_signature: None,
                name: "Write".to_string(),
                args_json: r#"{"path":"out.txt","content":"ok"}"#.to_string(),
            }],
        },
        ChatResponse {
            content: "Created out.txt.".to_string(),
            tool_calls: Vec::new(),
        },
    ]);
    let config = MinimalLoopConfig {
        initial_tool_call_mode: ToolCallMode::XmlFallback,
        action_requirement: ActionRequirement::Required,
        ..MinimalLoopConfig::default()
    };

    let _ = run_session(&mut client, &root, "inspect", config).unwrap();

    let system = client.requests()[0]
        .messages
        .iter()
        .find(|message| message.role == ChatRole::System)
        .unwrap();
    assert!(system.content.contains("commandagent_tool_call"));
    assert!(system.content.contains("\"args\""));
    assert!(
        system
            .content
            .contains("Write: {\"path\":\"README.md\",\"content\":\"text\"}")
    );
    assert!(request_contains_user_message(
        &client.requests()[1],
        "emit one complete XML fallback tool call"
    ));
}

#[test]
fn parsed_tool_calls_do_not_double_execute_xml_content() {
    let root = temp_workspace("no-double-exec");
    let mut client = MockChatClient::new(vec![
        ChatResponse {
            content: r#"<commandagent_tool_call>{"name":"Write","args":{"path":"out.txt","content":"from-content"}}</commandagent_tool_call>"#.to_string(),
            tool_calls: vec![ToolCall {
                id: None,
                thought_signature: None,
                name: "Write".to_string(),
                args_json: r#"{"path":"out.txt","content":"from-tool-call"}"#.to_string(),
            }],
        },
        ChatResponse {
            content: "Created out.txt.".to_string(),
            tool_calls: Vec::new(),
        },
    ]);

    let result = run_session(
        &mut client,
        &root,
        "create out.txt",
        MinimalLoopConfig::default(),
    )
    .unwrap();

    assert_eq!(result.tool_results.len(), 1);
    assert_eq!(
        fs::read_to_string(root.join("out.txt")).unwrap(),
        "from-tool-call"
    );
}

#[test]
fn xml_fallback_tool_calls_are_preserved_in_assistant_history() {
    let root = temp_workspace("xml-history");
    let mut client = MockChatClient::new(vec![
        ChatResponse {
            content: String::new(),
            tool_calls: vec![ToolCall {
                id: None,
                thought_signature: None,
                name: "Write".to_string(),
                args_json: r#"{"path":"out.txt","content":"ok"}"#.to_string(),
            }],
        },
        ChatResponse {
            content: "Created out.txt.".to_string(),
            tool_calls: Vec::new(),
        },
    ]);
    let config = MinimalLoopConfig {
        initial_tool_call_mode: ToolCallMode::XmlFallback,
        ..MinimalLoopConfig::default()
    };

    let _ = run_session(&mut client, &root, "create out.txt", config).unwrap();

    assert!(client.requests()[1].messages.iter().any(|message| {
        message.role == ChatRole::Assistant && message.content.contains("commandagent_tool_call")
    }));
}

#[test]
fn action_required_feedback_fires_once() {
    let root = temp_workspace("action-required-feedback");
    let mut client = MockChatClient::new(vec![
        ChatResponse {
            content: "No changes needed.".to_string(),
            tool_calls: Vec::new(),
        },
        ChatResponse {
            content: String::new(),
            tool_calls: vec![ToolCall {
                id: None,
                thought_signature: None,
                name: "Write".to_string(),
                args_json: r#"{"path":"out.txt","content":"ok"}"#.to_string(),
            }],
        },
        ChatResponse {
            content: "Created out.txt.".to_string(),
            tool_calls: Vec::new(),
        },
    ]);
    let config = MinimalLoopConfig {
        action_requirement: ActionRequirement::Required,
        ..MinimalLoopConfig::default()
    };

    let result = run_session(&mut client, &root, "create out.txt", config).unwrap();

    assert_eq!(result.iterations, 3);
    assert_eq!(result.final_answer, "Created out.txt.");
    assert!(request_contains_user_message(
        &client.requests()[1],
        "concrete repository evidence"
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
