use crate::providers::{ChatMessage, ToolCallMode};

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
    ActionRequiredNoEvidence(String),
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
            Self::ActionRequiredNoEvidence(message) => {
                write!(
                    f,
                    "assistant did not provide required repository evidence: {}",
                    message
                )
            }
            Self::MissingArtifacts(paths) => {
                write!(f, "missing expected artifacts: {}", paths.join(", "))
            }
        }
    }
}

impl std::error::Error for MinimalLoopError {}
