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
    pub output_truncated: bool,
    pub original_output_chars: usize,
    pub target_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MinimalLoopError {
    MaxIterations,
    Model(String),
    ToolArgs(ToolArgError),
    Tool(String),
    FinalAnswerContract(String),
    ActionRequiredNoEvidence(String),
    MissingArtifacts(Vec<String>),
    ProgressBudgetExhausted(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolArgError {
    InvalidJson {
        tool: String,
        message: String,
    },
    MissingRequiredStringField {
        tool: String,
        field: String,
        required_fields: Vec<String>,
    },
}

impl ToolArgError {
    pub fn reason_code(&self) -> &'static str {
        match self {
            Self::InvalidJson { .. } => "tool_args_invalid_json",
            Self::MissingRequiredStringField { .. } => "tool_args_missing_required_field",
        }
    }

    pub fn tool_name(&self) -> &str {
        match self {
            Self::InvalidJson { tool, .. } | Self::MissingRequiredStringField { tool, .. } => tool,
        }
    }

    pub fn missing_field(&self) -> Option<&str> {
        match self {
            Self::MissingRequiredStringField { field, .. } => Some(field),
            Self::InvalidJson { .. } => None,
        }
    }

    pub fn required_fields(&self) -> &[String] {
        match self {
            Self::MissingRequiredStringField {
                required_fields, ..
            } => required_fields,
            Self::InvalidJson { .. } => &[],
        }
    }

    pub fn diagnostic_excerpt(&self) -> String {
        match self {
            Self::InvalidJson { tool, message } => format!(
                "The previous tool call for {tool} had invalid JSON arguments: {message}. Emit exactly one valid tool call with complete JSON for the current step; do not answer in prose."
            ),
            Self::MissingRequiredStringField {
                tool,
                field,
                required_fields,
            } => {
                let required = if required_fields.is_empty() {
                    "unknown".to_string()
                } else {
                    required_fields.join(", ")
                };
                format!(
                    "The previous tool call for {tool} was invalid because required string field `{field}` was missing. {tool} requires: {required}. Emit exactly one valid tool call with complete JSON for the current step; do not answer in prose."
                )
            }
        }
    }
}

impl std::fmt::Display for ToolArgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidJson { tool, message } => {
                write!(f, "{tool} arguments are not valid JSON: {message}")
            }
            Self::MissingRequiredStringField {
                tool,
                field,
                required_fields,
            } => {
                if required_fields.is_empty() {
                    write!(f, "{tool} missing string field `{field}`")
                } else {
                    write!(
                        f,
                        "{tool} missing string field `{field}` (required fields: {})",
                        required_fields.join(", ")
                    )
                }
            }
        }
    }
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
            Self::ProgressBudgetExhausted(message) => {
                write!(f, "minimal loop progress budget exhausted: {}", message)
            }
        }
    }
}

impl std::error::Error for MinimalLoopError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_arg_error_display_missing_required_field() {
        let err = ToolArgError::MissingRequiredStringField {
            tool: "Write".to_string(),
            field: "path".to_string(),
            required_fields: vec!["path".to_string(), "content".to_string()],
        };

        assert_eq!(err.reason_code(), "tool_args_missing_required_field");
        assert_eq!(err.tool_name(), "Write");
        assert_eq!(err.missing_field(), Some("path"));
        assert_eq!(err.required_fields().len(), 2);
        assert_eq!(err.required_fields()[0], "path");
        assert_eq!(err.required_fields()[1], "content");
        assert!(
            err.to_string()
                .contains("Write missing string field `path`")
        );
        assert!(err.to_string().contains("required fields: path, content"));
        assert!(
            err.diagnostic_excerpt()
                .contains("Write requires: path, content")
        );
    }

    #[test]
    fn tool_arg_error_display_invalid_json() {
        let err = ToolArgError::InvalidJson {
            tool: "Write".to_string(),
            message: "expected value".to_string(),
        };

        assert_eq!(err.reason_code(), "tool_args_invalid_json");
        assert_eq!(err.tool_name(), "Write");
        assert_eq!(err.missing_field(), None);
        assert!(err.required_fields().is_empty());
        assert!(
            err.to_string()
                .contains("Write arguments are not valid JSON")
        );
        assert!(
            err.diagnostic_excerpt()
                .contains("invalid JSON arguments: expected value")
        );
    }
}
