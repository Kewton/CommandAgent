use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PromptArtifactExtraction {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompletionContractPathMerge {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompletionContractVerification {
    Enabled,
    DisabledDuringStep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContractEnforcement {
    Enforce,
    Observe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActionNoToolPolicy {
    RequireWriteForActionPrompt,
    RequireToolOnlyIfNoToolSeen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RunSessionScope {
    MinimalLoop,
    PlanRunStep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RunSessionStepKind {
    Inspect,
    Setup,
    Implement,
    Verify,
    Report,
    Unknown,
}

#[derive(Debug, Clone)]
pub(crate) struct RunSessionOptions {
    pub prompt_artifact_extraction: PromptArtifactExtraction,
    pub completion_contract_path_merge: CompletionContractPathMerge,
    pub completion_contract_verification: CompletionContractVerification,
    pub contract_enforcement: ContractEnforcement,
    pub phase_scope: Option<String>,
    pub action_no_tool_policy: ActionNoToolPolicy,
    pub scope: RunSessionScope,
    pub step_kind: Option<RunSessionStepKind>,
    pub dependency_setup_authority: NodeDependencySetupAuthority,
    pub step_wall_clock_cap: Option<Duration>,
    pub path_fallback_candidates: Vec<String>,
    pub repair_target_priority: crate::planner::repair_targeting::RepairTargetPriority,
    pub require_mutation_before_contract_short_circuit: bool,
    pub escalation_carryover: Option<EscalationCarryoverHandle>,
}

impl Default for RunSessionOptions {
    fn default() -> Self {
        Self {
            prompt_artifact_extraction: PromptArtifactExtraction::Enabled,
            completion_contract_path_merge: CompletionContractPathMerge::Enabled,
            completion_contract_verification: CompletionContractVerification::Enabled,
            contract_enforcement: ContractEnforcement::Enforce,
            phase_scope: None,
            action_no_tool_policy: ActionNoToolPolicy::RequireWriteForActionPrompt,
            scope: RunSessionScope::MinimalLoop,
            step_kind: None,
            dependency_setup_authority: NodeDependencySetupAuthority::None,
            step_wall_clock_cap: None,
            path_fallback_candidates: Vec::new(),
            repair_target_priority: Default::default(),
            require_mutation_before_contract_short_circuit: false,
            escalation_carryover: None,
        }
    }
}

impl RunSessionOptions {
    pub(crate) fn plan_step(step_kind: RunSessionStepKind) -> Self {
        Self::plan_step_with_enforcement(step_kind, ContractEnforcement::Enforce, None)
    }

    pub(crate) fn final_acceptance_repair() -> Self {
        let mut options = Self::plan_step(RunSessionStepKind::Implement);
        options.require_mutation_before_contract_short_circuit = true;
        options
    }

    pub(crate) fn plan_step_with_enforcement(
        step_kind: RunSessionStepKind,
        enforcement: ContractEnforcement,
        phase_scope: Option<String>,
    ) -> Self {
        let completion_contract_enabled = step_kind == RunSessionStepKind::Implement;
        let contract_path_merge_enabled =
            completion_contract_enabled && enforcement == ContractEnforcement::Enforce;
        Self {
            prompt_artifact_extraction: PromptArtifactExtraction::Disabled,
            completion_contract_path_merge: if contract_path_merge_enabled {
                CompletionContractPathMerge::Enabled
            } else {
                CompletionContractPathMerge::Disabled
            },
            completion_contract_verification: if completion_contract_enabled {
                CompletionContractVerification::Enabled
            } else {
                CompletionContractVerification::DisabledDuringStep
            },
            contract_enforcement: enforcement,
            phase_scope,
            action_no_tool_policy: ActionNoToolPolicy::RequireToolOnlyIfNoToolSeen,
            scope: RunSessionScope::PlanRunStep,
            step_kind: Some(step_kind),
            ..Self::default()
        }
    }

    pub(crate) fn with_dependency_setup_authority(
        mut self,
        authority: NodeDependencySetupAuthority,
    ) -> Self {
        self.dependency_setup_authority = authority;
        self
    }

    pub(crate) fn with_path_fallback_candidates(mut self, candidates: Vec<String>) -> Self {
        self.path_fallback_candidates = candidates;
        self
    }

    pub(crate) fn with_repair_target_priority(
        mut self,
        priority: crate::planner::repair_targeting::RepairTargetPriority,
    ) -> Self {
        self.repair_target_priority = priority;
        self
    }

    pub(crate) fn with_required_mutation_before_short_circuit(mut self, required: bool) -> Self {
        self.require_mutation_before_contract_short_circuit |= required;
        self
    }

    pub(super) fn contract_runtime_enabled(&self) -> bool {
        self.completion_contract_verification == CompletionContractVerification::Enabled
    }

    pub(super) fn contract_path_merge_enabled(&self) -> bool {
        self.completion_contract_path_merge == CompletionContractPathMerge::Enabled
    }

    pub(super) fn prompt_artifact_extraction_enabled(&self) -> bool {
        self.prompt_artifact_extraction == PromptArtifactExtraction::Enabled
    }

    pub(super) fn requires_action_tool_feedback(
        &self,
        write_or_edit_seen: bool,
        tool_call_count: usize,
    ) -> bool {
        match self.action_no_tool_policy {
            ActionNoToolPolicy::RequireWriteForActionPrompt => !write_or_edit_seen,
            ActionNoToolPolicy::RequireToolOnlyIfNoToolSeen => tool_call_count == 0,
        }
    }

    pub(super) fn allows_tool_only_step_completion(&self) -> bool {
        self.scope == RunSessionScope::PlanRunStep
            && matches!(
                self.step_kind,
                Some(
                    RunSessionStepKind::Inspect
                        | RunSessionStepKind::Setup
                        | RunSessionStepKind::Verify
                )
            )
    }

    pub(super) fn contract_enforcement_label(&self) -> &'static str {
        self.contract_enforcement.as_str()
    }
}

impl ContractEnforcement {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            ContractEnforcement::Enforce => "enforce",
            ContractEnforcement::Observe => "observe",
        }
    }
}

impl RunSessionScope {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            RunSessionScope::MinimalLoop => "minimal-loop",
            RunSessionScope::PlanRunStep => "plan-run-step",
        }
    }
}

impl RunSessionStepKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            RunSessionStepKind::Inspect => "inspect",
            RunSessionStepKind::Setup => "setup",
            RunSessionStepKind::Implement => "implement",
            RunSessionStepKind::Verify => "verify",
            RunSessionStepKind::Report => "report",
            RunSessionStepKind::Unknown => "unknown",
        }
    }

    pub(super) fn bash_policy_purpose(self) -> &'static str {
        match self {
            RunSessionStepKind::Inspect => "runtime_inspection",
            RunSessionStepKind::Setup => "runtime_setup",
            RunSessionStepKind::Implement => "runtime_implementation",
            RunSessionStepKind::Verify | RunSessionStepKind::Report => {
                "deterministic_verifier_evidence"
            }
            RunSessionStepKind::Unknown => "runtime_unknown",
        }
    }

    pub(super) fn requires_verifier_bash_policy(self) -> bool {
        matches!(
            self,
            RunSessionStepKind::Verify | RunSessionStepKind::Report
        )
    }
}

pub(super) fn provider_call_scope_for_options(
    options: &RunSessionOptions,
    pending_feedback: Option<&str>,
) -> ProviderCallScope {
    if pending_feedback.is_some() {
        return ProviderCallScope::Repair;
    }
    match options.scope {
        RunSessionScope::MinimalLoop | RunSessionScope::PlanRunStep => ProviderCallScope::Executor,
    }
}

pub(super) fn step_wall_clock_cap(options: &RunSessionOptions) -> Duration {
    options
        .step_wall_clock_cap
        .or_else(step_wall_clock_cap_from_env)
        .unwrap_or(DEFAULT_STEP_WALL_CLOCK_CAP)
}

fn step_wall_clock_cap_from_env() -> Option<Duration> {
    let value = crate::env_compat::var("COMMANDAGENT_STEP_WALL_CLOCK_CAP_MS").ok()?;
    let millis = value.trim().parse::<u64>().ok()?;
    Some(Duration::from_millis(millis))
}
