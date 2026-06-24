use crate::providers::ToolCallMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DependencySetupPolicy {
    pub auto_approve: bool,
    pub offline: bool,
    pub timeout_secs: u64,
}

impl Default for DependencySetupPolicy {
    fn default() -> Self {
        Self {
            auto_approve: false,
            offline: false,
            timeout_secs: 600,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionRequirement {
    Optional,
    Required,
    RepositoryEvidenceRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepToolPolicy {
    ReadOnly,
    NoMutation,
    FileMutationAllowed,
    SetupMutationOnly,
}

#[derive(Debug, Clone)]
pub struct MinimalLoopConfig {
    pub model: String,
    pub max_iterations: usize,
    pub initial_tool_call_mode: ToolCallMode,
    pub expected_artifacts: Vec<String>,
    pub dependency_setup_policy: DependencySetupPolicy,
    pub action_requirement: ActionRequirement,
    pub step_tool_policy: StepToolPolicy,
    pub allowed_tools: Vec<String>,
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
            dependency_setup_policy: DependencySetupPolicy::default(),
            action_requirement: ActionRequirement::Optional,
            step_tool_policy: StepToolPolicy::FileMutationAllowed,
            allowed_tools: Vec::new(),
            enable_completion_without_write_feedback: true,
            enable_requested_artifact_feedback: true,
            enable_future_action_feedback: true,
        }
    }
}
