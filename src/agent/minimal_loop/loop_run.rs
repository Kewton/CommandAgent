use crate::agent::events::{
    ArtifactScope, ArtifactStatus, GuardFeedbackKind, NoopRuntimeObserver, RuntimeEvent,
    RuntimeObserver, bounded_event_text,
};
use crate::agent::minimal_loop::config::{ActionRequirement, StepToolPolicy};
use crate::agent::minimal_loop::guards::{
    action_required_feedback, future_action_feedback, is_file_change_tool, missing_artifacts,
    repository_evidence_required_feedback, requested_artifact_feedback,
};
use crate::agent::minimal_loop::prompt::{
    parser_failure_feedback, system_prompt, violates_final_answer_contract,
};
use crate::providers::xml_fallback::{
    extract_tool_calls, mode_after_parse_failure, render_tool_calls,
};
use crate::providers::{
    ChatMessage, ChatRequest, ChatResponse, ChatRole, ToolCall, ToolCallMode,
    tool_call_parse_error_from_content,
};
use crate::safety::path_guard::PathGuard;
use crate::tools::registry::file_tool_specs;
use std::path::Path;
use std::time::Instant;

pub use super::client::ChatClient;
pub use super::config::MinimalLoopConfig;
pub use super::result::{MinimalLoopError, RunResult, ToolExecutionRecord};
use super::tool_executor::ToolExecutor;

pub fn run_session<C>(
    client: &mut C,
    cwd: impl AsRef<Path>,
    user_prompt: &str,
    config: MinimalLoopConfig,
) -> Result<RunResult, MinimalLoopError>
where
    C: ChatClient,
{
    let mut observer = NoopRuntimeObserver;
    run_session_with_observer(client, cwd, user_prompt, config, &mut observer)
}

pub fn run_session_with_observer<C, O>(
    client: &mut C,
    cwd: impl AsRef<Path>,
    user_prompt: &str,
    config: MinimalLoopConfig,
    observer: &mut O,
) -> Result<RunResult, MinimalLoopError>
where
    C: ChatClient,
    O: RuntimeObserver + ?Sized,
{
    let guard = match PathGuard::new(cwd.as_ref()) {
        Ok(guard) => guard,
        Err(err) => {
            let err = MinimalLoopError::Tool(err.to_string());
            emit_session_error(observer, &err);
            return Err(err);
        }
    };
    let executor = ToolExecutor::new(
        &guard,
        config.dependency_setup_policy,
        config.step_tool_policy,
    );
    let mut mode = config.initial_tool_call_mode;
    let tools = file_tool_specs();
    let mut messages = vec![
        ChatMessage::new(ChatRole::System, system_prompt(mode, &tools)),
        ChatMessage::new(ChatRole::User, user_prompt),
    ];
    let mut tool_results = Vec::new();
    let mut file_change_count = 0usize;
    let mut repository_evidence_count = 0usize;
    let mut future_action_feedback_sent = false;
    let mut action_required_feedback_sent = false;
    let mut requested_artifact_feedback_sent = false;

    for iteration in 1..=config.max_iterations {
        let request = ChatRequest {
            model: config.model.clone(),
            messages: messages.clone(),
            tools: tools.clone(),
            tool_call_mode: mode,
        };
        observer.on_event(RuntimeEvent::ModelRequestStarted {
            iteration,
            model: request.model.clone(),
            tool_call_mode: mode,
        });
        let started = Instant::now();
        let response = match client.chat(&request) {
            Ok(response) => response,
            Err(err) => {
                let err = MinimalLoopError::Model(err);
                emit_session_error(observer, &err);
                return Err(err);
            }
        };
        let elapsed_ms = started.elapsed().as_millis();
        observer.on_event(RuntimeEvent::ModelResponseReceived {
            iteration,
            tool_call_mode: mode,
            tool_call_count: response.tool_calls.len(),
            content_chars: response.content.chars().count(),
            elapsed_ms,
            usage: response.usage.clone().with_latency(elapsed_ms as u64),
        });

        let calls = tool_calls_from_response(&response, mode);
        match calls {
            Ok(calls) if !calls.is_empty() => {
                messages.push(ChatMessage::assistant_with_tool_calls(
                    assistant_history_content(&response, &calls, mode),
                    calls.clone(),
                ));
                for call in calls {
                    observer.on_event(RuntimeEvent::ToolCallStarted {
                        iteration,
                        tool_name: call.name.clone(),
                        args_summary: compact_tool_args_summary(&call),
                    });
                    let record = match executor.execute(&call) {
                        Ok(record) => record,
                        Err(err) => {
                            observer.on_event(RuntimeEvent::ToolCallFinished {
                                iteration,
                                tool_name: call.name.clone(),
                                ok: false,
                                output_chars: 0,
                                error: Some(bounded_event_text(err.to_string())),
                            });
                            emit_session_error(observer, &err);
                            return Err(err);
                        }
                    };
                    observer.on_event(RuntimeEvent::ToolCallFinished {
                        iteration,
                        tool_name: record.name.clone(),
                        ok: record.ok,
                        output_chars: record.output.chars().count(),
                        error: None,
                    });
                    if record.output_truncated {
                        observer.on_event(RuntimeEvent::ToolResultTruncated {
                            iteration,
                            tool_name: record.name.clone(),
                            original_chars: record.original_output_chars,
                            returned_chars: record.output.chars().count(),
                            reason: "max_output_chars".to_string(),
                        });
                    }
                    if record.ok && is_file_change_tool(&record.name) {
                        file_change_count += 1;
                    }
                    if record.ok
                        && is_repository_evidence_tool(&record.name, config.step_tool_policy)
                    {
                        repository_evidence_count += 1;
                    }
                    messages.push(ChatMessage::tool_result(
                        record.output.clone(),
                        call.name.clone(),
                        call.id.clone(),
                    ));
                    tool_results.push(record);
                }
            }
            Ok(_) => {
                messages.push(ChatMessage::new(
                    ChatRole::Assistant,
                    response.content.clone(),
                ));
                if violates_final_answer_contract(&response.content) {
                    if config.enable_future_action_feedback && !future_action_feedback_sent {
                        future_action_feedback_sent = true;
                        observer.on_event(RuntimeEvent::GuardFeedbackSent {
                            iteration,
                            kind: GuardFeedbackKind::FutureAction,
                            tool_call_mode: mode,
                            missing_artifacts: Vec::new(),
                        });
                        messages.push(ChatMessage::new(ChatRole::User, future_action_feedback()));
                        continue;
                    }
                    let err = MinimalLoopError::FinalAnswerContract(response.content);
                    emit_session_error(observer, &err);
                    return Err(err);
                }

                let missing = missing_artifacts(&guard, &config.expected_artifacts);
                emit_artifact_statuses(observer, &config.expected_artifacts, &missing);
                if config.enable_requested_artifact_feedback && !missing.is_empty() {
                    if !requested_artifact_feedback_sent {
                        requested_artifact_feedback_sent = true;
                        observer.on_event(RuntimeEvent::GuardFeedbackSent {
                            iteration,
                            kind: GuardFeedbackKind::RequestedArtifacts,
                            tool_call_mode: mode,
                            missing_artifacts: missing.clone(),
                        });
                        messages.push(ChatMessage::new(
                            ChatRole::User,
                            requested_artifact_feedback(&missing, mode),
                        ));
                        continue;
                    }
                    let err = MinimalLoopError::MissingArtifacts(missing);
                    emit_session_error(observer, &err);
                    return Err(err);
                }

                if config.enable_completion_without_write_feedback
                    && !action_requirement_satisfied(
                        config.action_requirement,
                        file_change_count,
                        repository_evidence_count,
                    )
                    && !action_required_feedback_sent
                {
                    action_required_feedback_sent = true;
                    observer.on_event(RuntimeEvent::GuardFeedbackSent {
                        iteration,
                        kind: GuardFeedbackKind::ActionRequired,
                        tool_call_mode: mode,
                        missing_artifacts: Vec::new(),
                    });
                    let feedback = match config.action_requirement {
                        ActionRequirement::RepositoryEvidenceRequired => {
                            repository_evidence_required_feedback(mode)
                        }
                        ActionRequirement::Optional | ActionRequirement::Required => {
                            action_required_feedback(mode)
                        }
                    };
                    messages.push(ChatMessage::new(ChatRole::User, feedback));
                    continue;
                } else if config.enable_completion_without_write_feedback
                    && !action_requirement_satisfied(
                        config.action_requirement,
                        file_change_count,
                        repository_evidence_count,
                    )
                    && action_required_feedback_sent
                {
                    let err = MinimalLoopError::ActionRequiredNoEvidence(response.content);
                    emit_session_error(observer, &err);
                    return Err(err);
                }

                observer.on_event(RuntimeEvent::FinalAnswerAccepted {
                    iteration,
                    answer_chars: response.content.chars().count(),
                });
                return Ok(RunResult {
                    final_answer: response.content,
                    iterations: iteration,
                    tool_call_mode: mode,
                    tool_results,
                    messages,
                });
            }
            Err(err) => {
                messages.push(ChatMessage::new(
                    ChatRole::Assistant,
                    response.content.clone(),
                ));
                let previous_mode = mode;
                mode = mode_after_parse_failure(mode);
                observer.on_event(RuntimeEvent::ParserFeedbackSent {
                    iteration,
                    previous_tool_call_mode: previous_mode,
                    next_tool_call_mode: mode,
                    error: bounded_event_text(&err),
                });
                messages.push(ChatMessage::new(
                    ChatRole::User,
                    parser_failure_feedback(&err),
                ));
            }
        }
    }

    let err = MinimalLoopError::MaxIterations;
    emit_session_error(observer, &err);
    Err(err)
}

fn action_requirement_satisfied(
    requirement: ActionRequirement,
    file_change_count: usize,
    repository_evidence_count: usize,
) -> bool {
    match requirement {
        ActionRequirement::Optional => true,
        ActionRequirement::Required => file_change_count > 0,
        ActionRequirement::RepositoryEvidenceRequired => repository_evidence_count > 0,
    }
}

fn is_repository_evidence_tool(name: &str, policy: StepToolPolicy) -> bool {
    matches!(name, "Read" | "Glob" | "Grep")
        || (name == "Bash" && matches!(policy, StepToolPolicy::ReadOnly))
}

fn tool_calls_from_response(
    response: &ChatResponse,
    mode: ToolCallMode,
) -> Result<Vec<ToolCall>, String> {
    if let Some(error) = tool_call_parse_error_from_content(&response.content) {
        return Err(error);
    }

    if !response.tool_calls.is_empty() {
        return Ok(response.tool_calls.clone());
    }

    match extract_tool_calls(&response.content) {
        Ok(calls) if !calls.is_empty() => Ok(calls),
        Ok(_) => Ok(Vec::new()),
        Err(err) if mode == ToolCallMode::Native => Err(err.to_string()),
        Err(err) => Err(err.to_string()),
    }
}

fn assistant_history_content(
    response: &ChatResponse,
    calls: &[ToolCall],
    mode: ToolCallMode,
) -> String {
    if mode != ToolCallMode::XmlFallback && calls.iter().any(|call| call.id.is_some()) {
        return response.content.clone();
    }

    let rendered_calls = render_tool_calls(calls);
    if response.content.trim().is_empty() {
        rendered_calls
    } else {
        format!("{}\n{}", response.content, rendered_calls)
    }
}

fn emit_session_error(observer: &mut (impl RuntimeObserver + ?Sized), err: &MinimalLoopError) {
    observer.on_event(RuntimeEvent::SessionError {
        message: bounded_event_text(err.to_string()),
    });
}

fn emit_artifact_statuses(
    observer: &mut (impl RuntimeObserver + ?Sized),
    expected_artifacts: &[String],
    missing: &[String],
) {
    for path in expected_artifacts {
        let status = if missing.iter().any(|missing_path| missing_path == path) {
            ArtifactStatus::Missing
        } else {
            ArtifactStatus::Ok
        };
        observer.on_event(RuntimeEvent::ArtifactStatus {
            scope: ArtifactScope::StepExpectedPath,
            path: bounded_event_text(path),
            status,
        });
    }
}

fn compact_tool_args_summary(call: &ToolCall) -> String {
    let Ok(args) = serde_json::from_str::<serde_json::Value>(&call.args_json) else {
        return "invalid args".to_string();
    };
    let field = |key: &str| {
        args.get(key)
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
    };
    let summary = match call.name.as_str() {
        "Read" => field("path").to_string(),
        "Write" => {
            let content_bytes = field("content").len();
            format!("{} {content_bytes}B", field("path"))
        }
        "Edit" => {
            let new_bytes = field("new").len();
            format!("{} {new_bytes}B", field("path"))
        }
        "Bash" => field("command").to_string(),
        "Glob" | "Grep" => field("pattern").to_string(),
        _ => call.args_json.clone(),
    };
    bounded_event_text(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::events::{
        ArtifactScope, ArtifactStatus, CaptureObserver, GuardFeedbackKind, RuntimeEvent,
    };
    use crate::agent::minimal_loop::config::{ActionRequirement, StepToolPolicy};
    use std::collections::VecDeque;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn one_shot_write_executes_then_finishes() {
        let root = temp_workspace("write");
        let mut client = MockClient::new(vec![
            ChatResponse {
                content: String::new(),
                tool_calls: vec![ToolCall {
                    id: None,
                    thought_signature: None,
                    name: "Write".to_string(),
                    args_json: r#"{"path":"nested/hello.txt","content":"hello"}"#.to_string(),
                }],

                usage: Default::default(),
            },
            ChatResponse {
                content: "Created nested/hello.txt.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
        ]);

        let result = run_session(
            &mut client,
            &root,
            "create a file",
            MinimalLoopConfig::default(),
        )
        .unwrap();

        assert_eq!(result.final_answer, "Created nested/hello.txt.");
        assert_eq!(
            fs::read_to_string(root.join("nested/hello.txt")).unwrap(),
            "hello"
        );
        assert_eq!(result.tool_results.len(), 1);
    }

    #[test]
    fn observer_captures_bounded_tool_event_order() {
        let root = temp_workspace("observer-write");
        let mut client = MockClient::new(vec![
            ChatResponse {
                content: String::new(),
                tool_calls: vec![ToolCall {
                    id: None,
                    thought_signature: None,
                    name: "Write".to_string(),
                    args_json: r#"{"path":"nested/file.txt","content":"secret"}"#.to_string(),
                }],

                usage: Default::default(),
            },
            ChatResponse {
                content: "Created nested/file.txt.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
        ]);
        let mut observer = CaptureObserver::default();

        let result = run_session_with_observer(
            &mut client,
            &root,
            "create a file",
            MinimalLoopConfig::default(),
            &mut observer,
        )
        .unwrap();

        assert_eq!(result.final_answer, "Created nested/file.txt.");
        assert!(matches!(
            observer.events()[0],
            RuntimeEvent::ModelRequestStarted { iteration: 1, .. }
        ));
        assert!(observer.events().iter().any(|event| matches!(
            event,
            RuntimeEvent::ToolCallStarted {
                iteration: 1,
                tool_name,
                args_summary
            } if tool_name == "Write"
                && args_summary == "nested/file.txt 6B"
                && !args_summary.contains("secret")
        )));
        assert!(observer.events().iter().any(|event| matches!(
            event,
            RuntimeEvent::ToolCallFinished {
                iteration: 1,
                tool_name,
                ok: true,
                error: None,
                ..
            } if tool_name == "Write"
        )));
        assert!(observer.events().iter().any(|event| matches!(
            event,
            RuntimeEvent::FinalAnswerAccepted { iteration: 2, .. }
        )));
    }

    #[test]
    fn parse_failure_downgrades_next_request_to_xml_fallback() {
        let root = temp_workspace("downgrade");
        let mut client = MockClient::new(vec![
            ChatResponse {
                content: "<commandagent_tool_call>{\"name\":\"Write\"".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
            ChatResponse {
                content: "Recovered.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
        ]);

        let result = run_session(
            &mut client,
            &root,
            "create a file",
            MinimalLoopConfig::default(),
        )
        .unwrap();

        assert_eq!(result.tool_call_mode, ToolCallMode::XmlFallback);
        assert_eq!(
            client.modes,
            vec![ToolCallMode::Native, ToolCallMode::XmlFallback]
        );
    }

    #[test]
    fn provider_parse_evidence_downgrades_next_request_to_xml_fallback() {
        let root = temp_workspace("provider-parse-evidence");
        let mut client = MockClient::new(vec![
            ChatResponse {
                content: crate::providers::tool_call_parse_error_content(
                    "gemini_native_function_call_missing_name",
                ),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
            ChatResponse {
                content: "Recovered.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
        ]);

        let result = run_session(
            &mut client,
            &root,
            "create a file",
            MinimalLoopConfig::default(),
        )
        .unwrap();

        assert_eq!(result.tool_call_mode, ToolCallMode::XmlFallback);
        assert_eq!(
            client.modes,
            vec![ToolCallMode::Native, ToolCallMode::XmlFallback]
        );
    }

    #[test]
    fn observer_reports_parser_feedback_without_raw_tool_xml() {
        let root = temp_workspace("observer-parser");
        let mut client = MockClient::new(vec![
            ChatResponse {
                content: "<commandagent_tool_call>{\"name\":\"Write\"".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
            ChatResponse {
                content: "No changes needed.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
        ]);
        let mut observer = CaptureObserver::default();

        run_session_with_observer(
            &mut client,
            &root,
            "inspect",
            MinimalLoopConfig {
                enable_completion_without_write_feedback: false,
                ..MinimalLoopConfig::default()
            },
            &mut observer,
        )
        .unwrap();

        assert!(observer.events().iter().any(|event| matches!(
            event,
            RuntimeEvent::ParserFeedbackSent {
                previous_tool_call_mode: ToolCallMode::Native,
                next_tool_call_mode: ToolCallMode::XmlFallback,
                error,
                ..
            } if error == "unclosed <commandagent_tool_call> block"
        )));
    }

    #[test]
    fn future_action_feedback_prompts_for_tool_call() {
        let root = temp_workspace("future-feedback");
        let mut client = MockClient::new(vec![
            ChatResponse {
                content: "Now I'll create the files.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
            ChatResponse {
                content: String::new(),
                tool_calls: vec![ToolCall {
                    id: None,
                    thought_signature: None,
                    name: "Write".to_string(),
                    args_json: r#"{"path":"created.txt","content":"ok"}"#.to_string(),
                }],

                usage: Default::default(),
            },
            ChatResponse {
                content: "Created created.txt.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
        ]);

        let result = run_session(
            &mut client,
            &root,
            "create files",
            MinimalLoopConfig::default(),
        )
        .unwrap();

        assert_eq!(result.final_answer, "Created created.txt.");
        assert_eq!(fs::read_to_string(root.join("created.txt")).unwrap(), "ok");
        assert!(result.messages.iter().any(|message| {
            message.role == ChatRole::User
                && message
                    .content
                    .contains("You described a future tool action")
        }));
    }

    #[test]
    fn repeated_future_action_after_feedback_is_rejected() {
        let root = temp_workspace("final-contract");
        let mut client = MockClient::new(vec![
            ChatResponse {
                content: "Now I'll create the files.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
            ChatResponse {
                content: "Let me write the file now.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
        ]);

        let err = run_session(
            &mut client,
            &root,
            "create files",
            MinimalLoopConfig::default(),
        )
        .unwrap_err();

        assert!(matches!(err, MinimalLoopError::FinalAnswerContract(_)));
    }

    #[test]
    fn executes_xml_fallback_tool_call() {
        let root = temp_workspace("xml");
        let mut client = MockClient::new(vec![
            ChatResponse {
                content: r#"<commandagent_tool_call>{"name":"Write","args":{"path":"a.txt","content":"x"}}</commandagent_tool_call>"#.to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),},
            ChatResponse {
                content: "Created a.txt.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),},
        ]);

        run_session(
            &mut client,
            &root,
            "create a file",
            MinimalLoopConfig {
                initial_tool_call_mode: ToolCallMode::XmlFallback,
                ..MinimalLoopConfig::default()
            },
        )
        .unwrap();

        assert_eq!(fs::read_to_string(root.join("a.txt")).unwrap(), "x");
    }

    #[test]
    fn direct_conversation_accepts_prose_without_action_required() {
        let root = temp_workspace("conversation-prose");
        let mut client = MockClient::new(vec![ChatResponse {
            content: "Done.".to_string(),
            tool_calls: Vec::new(),

            usage: Default::default(),
        }]);

        let result = run_session(
            &mut client,
            &root,
            "answer a question",
            MinimalLoopConfig::default(),
        )
        .unwrap();

        assert_eq!(result.final_answer, "Done.");
        assert_eq!(result.messages.len(), 3);
        assert!(
            !result
                .messages
                .iter()
                .any(|message| message.content.contains("concrete repository evidence"))
        );
    }

    #[test]
    fn action_required_feedback_prompts_for_tool_call() {
        let root = temp_workspace("action-required-feedback");
        let mut client = MockClient::new(vec![
            ChatResponse {
                content: "Done.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
            ChatResponse {
                content: String::new(),
                tool_calls: vec![ToolCall {
                    id: None,
                    thought_signature: None,
                    name: "Write".to_string(),
                    args_json: r#"{"path":"created.txt","content":"ok"}"#.to_string(),
                }],

                usage: Default::default(),
            },
            ChatResponse {
                content: "Created created.txt.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
        ]);
        let mut observer = CaptureObserver::default();

        let result = run_session_with_observer(
            &mut client,
            &root,
            "create a file",
            MinimalLoopConfig {
                action_requirement: ActionRequirement::Required,
                ..MinimalLoopConfig::default()
            },
            &mut observer,
        )
        .unwrap();

        assert_eq!(result.final_answer, "Created created.txt.");
        assert_eq!(fs::read_to_string(root.join("created.txt")).unwrap(), "ok");
        assert!(result.messages.iter().any(|message| {
            message.role == ChatRole::User
                && message
                    .content
                    .contains("requires concrete repository evidence")
        }));
        assert!(observer.events().iter().any(|event| matches!(
            event,
            RuntimeEvent::GuardFeedbackSent {
                kind: GuardFeedbackKind::ActionRequired,
                ..
            }
        )));
    }

    #[test]
    fn repeated_action_required_prose_returns_explicit_error() {
        let root = temp_workspace("action-required-error");
        let mut client = MockClient::new(vec![
            ChatResponse {
                content: "Done.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
            ChatResponse {
                content: "Still done.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
        ]);

        let err = run_session(
            &mut client,
            &root,
            "create a file",
            MinimalLoopConfig {
                action_requirement: ActionRequirement::Required,
                ..MinimalLoopConfig::default()
            },
        )
        .unwrap_err();

        assert!(matches!(err, MinimalLoopError::ActionRequiredNoEvidence(_)));
    }

    #[test]
    fn repository_evidence_required_accepts_read_tool() {
        let root = temp_workspace("repository-evidence-read");
        fs::write(
            root.join("package.json"),
            r#"{"scripts":{"build":"next build"}}"#,
        )
        .unwrap();
        let mut client = MockClient::new(vec![
            ChatResponse {
                content: "Already checked package.json.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
            ChatResponse {
                content: String::new(),
                tool_calls: vec![ToolCall {
                    id: None,
                    thought_signature: None,
                    name: "Read".to_string(),
                    args_json: r#"{"path":"package.json"}"#.to_string(),
                }],

                usage: Default::default(),
            },
            ChatResponse {
                content: "Read package.json and confirmed the build script.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
        ]);

        let result = run_session(
            &mut client,
            &root,
            "inspect package.json",
            MinimalLoopConfig {
                action_requirement: ActionRequirement::RepositoryEvidenceRequired,
                step_tool_policy: StepToolPolicy::ReadOnly,
                ..MinimalLoopConfig::default()
            },
        )
        .unwrap();

        assert_eq!(
            result.final_answer,
            "Read package.json and confirmed the build script."
        );
        assert!(result.messages.iter().any(|message| {
            message.role == ChatRole::User
                && message
                    .content
                    .contains("requires concrete repository read evidence")
        }));
        assert_eq!(result.tool_results[0].name, "Read");
    }

    #[test]
    fn repository_evidence_required_rejects_repeated_prose() {
        let root = temp_workspace("repository-evidence-prose");
        let mut client = MockClient::new(vec![
            ChatResponse {
                content: "package.json looks fine.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
            ChatResponse {
                content: "I already inspected package.json.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
        ]);

        let err = run_session(
            &mut client,
            &root,
            "inspect package.json",
            MinimalLoopConfig {
                action_requirement: ActionRequirement::RepositoryEvidenceRequired,
                step_tool_policy: StepToolPolicy::ReadOnly,
                ..MinimalLoopConfig::default()
            },
        )
        .unwrap_err();

        assert!(matches!(err, MinimalLoopError::ActionRequiredNoEvidence(_)));
    }

    #[test]
    fn requested_artifact_feedback_prompts_for_missing_path() {
        let root = temp_workspace("missing-artifact");
        let mut client = MockClient::new(vec![
            ChatResponse {
                content: "Done.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
            ChatResponse {
                content: String::new(),
                tool_calls: vec![ToolCall {
                    id: None,
                    thought_signature: None,
                    name: "Write".to_string(),
                    args_json: r#"{"path":"dist/report.md","content":"ok"}"#.to_string(),
                }],

                usage: Default::default(),
            },
            ChatResponse {
                content: "Created dist/report.md.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
        ]);

        let result = run_session(
            &mut client,
            &root,
            "create a report",
            MinimalLoopConfig {
                expected_artifacts: vec!["dist/report.md".to_string()],
                ..MinimalLoopConfig::default()
            },
        )
        .unwrap();

        assert_eq!(result.final_answer, "Created dist/report.md.");
        assert_eq!(
            fs::read_to_string(root.join("dist/report.md")).unwrap(),
            "ok"
        );
        assert!(result.messages.iter().any(|message| {
            message.role == ChatRole::User
                && message
                    .content
                    .contains("requested artifact paths are still missing")
        }));
    }

    #[test]
    fn requested_artifact_missing_after_feedback_is_error() {
        let root = temp_workspace("missing-artifact-error");
        let mut client = MockClient::new(vec![
            ChatResponse {
                content: "Done.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
            ChatResponse {
                content: "No changes needed.".to_string(),
                tool_calls: Vec::new(),

                usage: Default::default(),
            },
        ]);

        let err = run_session(
            &mut client,
            &root,
            "create a report",
            MinimalLoopConfig {
                expected_artifacts: vec!["dist/report.md".to_string()],
                ..MinimalLoopConfig::default()
            },
        )
        .unwrap_err();

        assert_eq!(
            err,
            MinimalLoopError::MissingArtifacts(vec!["dist/report.md".to_string()])
        );
    }

    #[test]
    fn observer_reports_step_expected_artifact_status_and_guard_feedback() {
        let root = temp_workspace("observer-artifact");
        let mut client = MockClient::new(vec![
            ChatResponse {
                content: "Done.".to_string(),
                tool_calls: Vec::new(),
                usage: Default::default(),
            },
            ChatResponse {
                content: "Still done.".to_string(),
                tool_calls: Vec::new(),
                usage: Default::default(),
            },
        ]);
        let mut observer = CaptureObserver::default();

        let err = run_session_with_observer(
            &mut client,
            &root,
            "create a report",
            MinimalLoopConfig {
                expected_artifacts: vec!["dist/report.md".to_string()],
                ..MinimalLoopConfig::default()
            },
            &mut observer,
        )
        .unwrap_err();

        assert_eq!(
            err,
            MinimalLoopError::MissingArtifacts(vec!["dist/report.md".to_string()])
        );
        assert!(observer.events().iter().any(|event| matches!(
            event,
            RuntimeEvent::ArtifactStatus {
                scope: ArtifactScope::StepExpectedPath,
                path,
                status: ArtifactStatus::Missing,
            } if path == "dist/report.md"
        )));
        assert!(observer.events().iter().any(|event| matches!(
            event,
            RuntimeEvent::GuardFeedbackSent {
                kind: GuardFeedbackKind::RequestedArtifacts,
                missing_artifacts,
                ..
            } if missing_artifacts == &vec!["dist/report.md".to_string()]
        )));
        assert!(
            observer
                .events()
                .iter()
                .any(|event| matches!(event, RuntimeEvent::SessionError { .. }))
        );
    }

    struct MockClient {
        responses: VecDeque<ChatResponse>,
        modes: Vec<ToolCallMode>,
    }

    impl MockClient {
        fn new(responses: Vec<ChatResponse>) -> Self {
            Self {
                responses: VecDeque::from(responses),
                modes: Vec::new(),
            }
        }
    }

    impl ChatClient for MockClient {
        fn chat(&mut self, request: &ChatRequest) -> Result<ChatResponse, String> {
            self.modes.push(request.tool_call_mode);
            self.responses
                .pop_front()
                .ok_or_else(|| "no mock response".to_string())
        }
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "commandagent-minimal-loop-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
}
