use crate::agent::minimal_loop::guards::{
    completion_without_write_feedback, future_action_feedback, is_file_change_tool,
    missing_artifacts, requested_artifact_feedback,
};
use crate::agent::minimal_loop::prompt::{
    parser_failure_feedback, system_prompt, violates_final_answer_contract,
};
use crate::providers::gemini::{GeminiClient, GeminiTransport};
use crate::providers::ollama::{OllamaClient, OllamaTransport};
use crate::providers::openai::{OpenAiClient, OpenAiTransport};
use crate::providers::xml_fallback::{extract_tool_calls, mode_after_parse_failure};
use crate::providers::{ChatMessage, ChatRequest, ChatResponse, ChatRole, ToolCall, ToolCallMode};
use crate::safety::path_guard::PathGuard;
use crate::tools::bash::BashTool;
use crate::tools::edit::EditTool;
use crate::tools::glob::{GlobTool, SearchOptions};
use crate::tools::grep::GrepTool;
use crate::tools::read::ReadTool;
use crate::tools::registry::file_tool_specs;
use crate::tools::write::WriteTool;
use serde_json::Value;
use std::path::Path;

pub trait ChatClient {
    fn chat(&mut self, request: &ChatRequest) -> Result<ChatResponse, String>;
}

impl<T> ChatClient for OllamaClient<T>
where
    T: OllamaTransport,
{
    fn chat(&mut self, request: &ChatRequest) -> Result<ChatResponse, String> {
        OllamaClient::chat(self, request).map_err(|err| err.to_string())
    }
}

impl<T> ChatClient for GeminiClient<T>
where
    T: GeminiTransport,
{
    fn chat(&mut self, request: &ChatRequest) -> Result<ChatResponse, String> {
        GeminiClient::chat(self, request).map_err(|err| err.to_string())
    }
}

impl<T> ChatClient for OpenAiClient<T>
where
    T: OpenAiTransport,
{
    fn chat(&mut self, request: &ChatRequest) -> Result<ChatResponse, String> {
        OpenAiClient::chat(self, request).map_err(|err| err.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct MinimalLoopConfig {
    pub model: String,
    pub max_iterations: usize,
    pub initial_tool_call_mode: ToolCallMode,
    pub expected_artifacts: Vec<String>,
    pub enable_completion_without_write_feedback: bool,
    pub enable_requested_artifact_feedback: bool,
    pub enable_future_action_feedback: bool,
}

impl Default for MinimalLoopConfig {
    fn default() -> Self {
        Self {
            model: "default".to_string(),
            max_iterations: 8,
            initial_tool_call_mode: ToolCallMode::Native,
            expected_artifacts: Vec::new(),
            enable_completion_without_write_feedback: true,
            enable_requested_artifact_feedback: true,
            enable_future_action_feedback: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunResult {
    pub final_answer: String,
    pub iterations: usize,
    pub tool_call_mode: ToolCallMode,
    pub tool_results: Vec<ToolExecutionRecord>,
    pub messages: Vec<ChatMessage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolExecutionRecord {
    pub name: String,
    pub ok: bool,
    pub output: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MinimalLoopError {
    MaxIterations,
    Model(String),
    ToolArgs(String),
    Tool(String),
    FinalAnswerContract(String),
    MissingArtifacts(Vec<String>),
}

impl std::fmt::Display for MinimalLoopError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MaxIterations => write!(f, "minimal loop reached max iterations"),
            Self::Model(message) => write!(f, "model error: {}", message),
            Self::ToolArgs(message) => write!(f, "invalid tool arguments: {}", message),
            Self::Tool(message) => write!(f, "tool error: {}", message),
            Self::FinalAnswerContract(message) => {
                write!(f, "assistant violated final answer contract: {}", message)
            }
            Self::MissingArtifacts(paths) => {
                write!(f, "missing expected artifacts: {}", paths.join(", "))
            }
        }
    }
}

impl std::error::Error for MinimalLoopError {}

pub fn run_session<C>(
    client: &mut C,
    cwd: impl AsRef<Path>,
    user_prompt: &str,
    config: MinimalLoopConfig,
) -> Result<RunResult, MinimalLoopError>
where
    C: ChatClient,
{
    let guard =
        PathGuard::new(cwd.as_ref()).map_err(|err| MinimalLoopError::Tool(err.to_string()))?;
    let executor = ToolExecutor::new(&guard);
    let mut messages = vec![
        ChatMessage {
            role: ChatRole::System,
            content: system_prompt().to_string(),
        },
        ChatMessage {
            role: ChatRole::User,
            content: user_prompt.to_string(),
        },
    ];
    let mut tool_results = Vec::new();
    let mut mode = config.initial_tool_call_mode;
    let mut file_change_count = 0usize;
    let mut future_action_feedback_sent = false;
    let mut completion_without_write_feedback_sent = false;
    let mut requested_artifact_feedback_sent = false;

    for iteration in 1..=config.max_iterations {
        let request = ChatRequest {
            model: config.model.clone(),
            messages: messages.clone(),
            tools: file_tool_specs(),
            tool_call_mode: mode,
        };
        let response = client.chat(&request).map_err(MinimalLoopError::Model)?;

        messages.push(ChatMessage {
            role: ChatRole::Assistant,
            content: response.content.clone(),
        });

        let calls = tool_calls_from_response(&response, mode);
        match calls {
            Ok(calls) if !calls.is_empty() => {
                for call in calls {
                    let record = executor.execute(&call)?;
                    if record.ok && is_file_change_tool(&record.name) {
                        file_change_count += 1;
                    }
                    messages.push(ChatMessage {
                        role: ChatRole::Tool,
                        content: record.output.clone(),
                    });
                    tool_results.push(record);
                }
            }
            Ok(_) => {
                if violates_final_answer_contract(&response.content) {
                    if config.enable_future_action_feedback && !future_action_feedback_sent {
                        future_action_feedback_sent = true;
                        messages.push(ChatMessage {
                            role: ChatRole::User,
                            content: future_action_feedback(),
                        });
                        continue;
                    }
                    return Err(MinimalLoopError::FinalAnswerContract(response.content));
                }

                let missing = missing_artifacts(&guard, &config.expected_artifacts);
                if config.enable_requested_artifact_feedback && !missing.is_empty() {
                    if !requested_artifact_feedback_sent {
                        requested_artifact_feedback_sent = true;
                        messages.push(ChatMessage {
                            role: ChatRole::User,
                            content: requested_artifact_feedback(&missing),
                        });
                        continue;
                    }
                    return Err(MinimalLoopError::MissingArtifacts(missing));
                }

                if config.enable_completion_without_write_feedback
                    && file_change_count == 0
                    && !completion_without_write_feedback_sent
                {
                    completion_without_write_feedback_sent = true;
                    messages.push(ChatMessage {
                        role: ChatRole::User,
                        content: completion_without_write_feedback(),
                    });
                    continue;
                }

                return Ok(RunResult {
                    final_answer: response.content,
                    iterations: iteration,
                    tool_call_mode: mode,
                    tool_results,
                    messages,
                });
            }
            Err(err) => {
                mode = mode_after_parse_failure(mode);
                messages.push(ChatMessage {
                    role: ChatRole::User,
                    content: parser_failure_feedback(&err),
                });
            }
        }
    }

    Err(MinimalLoopError::MaxIterations)
}

fn tool_calls_from_response(
    response: &ChatResponse,
    mode: ToolCallMode,
) -> Result<Vec<ToolCall>, String> {
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

struct ToolExecutor<'a> {
    read: ReadTool<'a>,
    write: WriteTool<'a>,
    edit: EditTool<'a>,
    bash: BashTool<'a>,
    glob: GlobTool<'a>,
    grep: GrepTool<'a>,
}

impl<'a> ToolExecutor<'a> {
    fn new(guard: &'a PathGuard) -> Self {
        Self {
            read: ReadTool::new(guard),
            write: WriteTool::new(guard),
            edit: EditTool::new(guard),
            bash: BashTool::new(guard),
            glob: GlobTool::new(guard),
            grep: GrepTool::new(guard),
        }
    }

    fn execute(&self, call: &ToolCall) -> Result<ToolExecutionRecord, MinimalLoopError> {
        let args: Value = serde_json::from_str(&call.args_json)
            .map_err(|err| MinimalLoopError::ToolArgs(err.to_string()))?;
        let output = match call.name.as_str() {
            "Read" => self
                .read
                .read(required_str(&args, "path")?)
                .map_err(tool_err)?,
            "Write" => {
                self.write
                    .write(
                        required_str(&args, "path")?,
                        required_str(&args, "content")?,
                    )
                    .map_err(tool_err)?;
                "wrote file".to_string()
            }
            "Edit" => {
                self.edit
                    .replace_once(
                        required_str(&args, "path")?,
                        required_str(&args, "old")?,
                        required_str(&args, "new")?,
                    )
                    .map_err(tool_err)?;
                "edited file".to_string()
            }
            "Bash" => {
                let output = self
                    .bash
                    .run(required_str(&args, "command")?)
                    .map_err(tool_err)?;
                format!(
                    "status: {}\nstdout:\n{}\nstderr:\n{}",
                    output.status, output.stdout, output.stderr
                )
            }
            "Glob" => self
                .glob
                .glob(required_str(&args, "pattern")?, SearchOptions::default())
                .map_err(tool_err)?
                .into_iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join("\n"),
            "Grep" => self
                .grep
                .grep(required_str(&args, "pattern")?, SearchOptions::default())
                .map_err(tool_err)?
                .into_iter()
                .map(|m| format!("{}:{}:{}", m.path.display(), m.line_number, m.line))
                .collect::<Vec<_>>()
                .join("\n"),
            other => return Err(MinimalLoopError::Tool(format!("unknown tool: {}", other))),
        };

        Ok(ToolExecutionRecord {
            name: call.name.clone(),
            ok: true,
            output,
        })
    }
}

fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, MinimalLoopError> {
    args.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| MinimalLoopError::ToolArgs(format!("missing string field `{}`", key)))
}

fn tool_err(err: impl std::fmt::Display) -> MinimalLoopError {
    MinimalLoopError::Tool(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
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
                    name: "Write".to_string(),
                    args_json: r#"{"path":"nested/hello.txt","content":"hello"}"#.to_string(),
                }],
            },
            ChatResponse {
                content: "Created nested/hello.txt.".to_string(),
                tool_calls: Vec::new(),
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
    fn parse_failure_downgrades_next_request_to_xml_fallback() {
        let root = temp_workspace("downgrade");
        let mut client = MockClient::new(vec![
            ChatResponse {
                content: "<commandagent_tool_call>{\"name\":\"Write\"".to_string(),
                tool_calls: Vec::new(),
            },
            ChatResponse {
                content: "Recovered.".to_string(),
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
            "create a file",
            MinimalLoopConfig::default(),
        )
        .unwrap();

        assert_eq!(result.tool_call_mode, ToolCallMode::XmlFallback);
        assert_eq!(
            client.modes,
            vec![
                ToolCallMode::Native,
                ToolCallMode::XmlFallback,
                ToolCallMode::XmlFallback
            ]
        );
    }

    #[test]
    fn future_action_feedback_prompts_for_tool_call() {
        let root = temp_workspace("future-feedback");
        let mut client = MockClient::new(vec![
            ChatResponse {
                content: "Now I'll create the files.".to_string(),
                tool_calls: Vec::new(),
            },
            ChatResponse {
                content: String::new(),
                tool_calls: vec![ToolCall {
                    name: "Write".to_string(),
                    args_json: r#"{"path":"created.txt","content":"ok"}"#.to_string(),
                }],
            },
            ChatResponse {
                content: "Created created.txt.".to_string(),
                tool_calls: Vec::new(),
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
            },
            ChatResponse {
                content: "Let me write the file now.".to_string(),
                tool_calls: Vec::new(),
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
            },
            ChatResponse {
                content: "Created a.txt.".to_string(),
                tool_calls: Vec::new(),
            },
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
    fn completion_without_write_feedback_fires_once_then_accepts_no_change_completion() {
        let root = temp_workspace("completion-no-write");
        let mut client = MockClient::new(vec![
            ChatResponse {
                content: "Done.".to_string(),
                tool_calls: Vec::new(),
            },
            ChatResponse {
                content: "No file changes were needed for this task.".to_string(),
                tool_calls: Vec::new(),
            },
        ]);

        let result = run_session(
            &mut client,
            &root,
            "answer a question",
            MinimalLoopConfig::default(),
        )
        .unwrap();

        assert_eq!(
            result.final_answer,
            "No file changes were needed for this task."
        );
        assert!(result.messages.iter().any(|message| {
            message.role == ChatRole::User
                && message.content.contains("No file changes have been made")
        }));
    }

    #[test]
    fn requested_artifact_feedback_prompts_for_missing_path() {
        let root = temp_workspace("missing-artifact");
        let mut client = MockClient::new(vec![
            ChatResponse {
                content: "Done.".to_string(),
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
            },
            ChatResponse {
                content: "No changes needed.".to_string(),
                tool_calls: Vec::new(),
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
