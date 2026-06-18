use crate::providers::ToolCallMode;

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
