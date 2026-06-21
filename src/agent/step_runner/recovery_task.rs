//! Clear repair-task contracts derived from deterministic failure evidence.
//!
//! A recovery task contract is rendered into existing bounded repair prompts.
//! It does not grant retry authority, select a new workflow, or execute tools.

use crate::agent::step_runner::correction_evidence::ContractEvidence;
use crate::agent::step_runner::recovery_orchestration::orchestrate_evidence;

const MAX_FIELD_CHARS: usize = 240;
const MAX_LIST_ITEMS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryExecutionEnvelope {
    ReadOnlyEvidence,
    FileMutationRepair,
    SetupConfigMutation,
    ToolProtocolCorrection,
}

impl RecoveryExecutionEnvelope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnlyEvidence => "read_only_evidence",
            Self::FileMutationRepair => "file_mutation_repair",
            Self::SetupConfigMutation => "setup_config_mutation",
            Self::ToolProtocolCorrection => "tool_protocol_correction",
        }
    }

    fn tool_policy(self) -> &'static str {
        match self {
            Self::ReadOnlyEvidence => "read_only",
            Self::FileMutationRepair => "file_mutation_allowed",
            Self::SetupConfigMutation => "setup_config_mutation_only",
            Self::ToolProtocolCorrection => "current_tool_protocol",
        }
    }

    fn evidence_requirement(self) -> &'static str {
        match self {
            Self::ReadOnlyEvidence => "repository_read_evidence",
            Self::FileMutationRepair => "file_change_or_explicit_blocker",
            Self::SetupConfigMutation => "setup_or_config_file_change_or_explicit_blocker",
            Self::ToolProtocolCorrection => "valid_tool_call",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RecoveryTaskContract {
    pub source: String,
    pub failed_step: Option<String>,
    pub contract_code: Option<String>,
    pub active_job: Option<String>,
    pub artifact_role: Option<String>,
    pub blocker: Option<String>,
    pub required_action: Option<String>,
    pub repair_target: Option<String>,
    pub candidate_artifacts: Vec<String>,
    pub allowed_tools: Vec<String>,
    pub disallowed_actions: Vec<String>,
    pub success_check: Option<String>,
    pub evidence_signature: Option<String>,
    pub repair_kind: Option<String>,
    pub repair_action: Option<String>,
    pub semantic_failure_kind: Option<String>,
    pub source_of_truth: Option<String>,
    pub allowed_change_kind: Option<String>,
    pub expected_evidence_delta: Option<String>,
    pub setup_implication: Option<String>,
    pub tool_policy_projection: Option<String>,
    pub target_admission: Option<String>,
    pub target_priority: Option<String>,
    pub workspace_scope: Option<String>,
    pub artifact_ownership: Option<String>,
    pub active_job_priority: Option<String>,
    pub explicit_stop_reason: Option<String>,
    pub artifact_graph_summary: Vec<String>,
    pub rerun_authority: Vec<String>,
    pub repair_attempt_ledger: Vec<String>,
    pub execution_envelope: Option<RecoveryExecutionEnvelope>,
}

impl RecoveryTaskContract {
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            ..Self::default()
        }
    }

    pub fn from_contract_evidence(evidence: &ContractEvidence) -> Option<Self> {
        let orchestrated = orchestrate_evidence(evidence.clone());
        let evidence = &orchestrated;
        if !known_recovery_source(&evidence.guard)
            && evidence.required_action.is_none()
            && evidence.repair_focus.is_none()
        {
            return None;
        }

        let mut task = Self::new(evidence.guard.clone())
            .with_failed_step_opt(evidence.failed_step.clone())
            .with_contract_code_opt(contract_code(evidence))
            .with_active_job_opt(evidence.active_job.clone())
            .with_artifact_role_opt(evidence.artifact_role.clone())
            .with_blocker_opt(blocker(evidence))
            .with_required_action_opt(required_action(evidence))
            .with_repair_target_opt(recovery_repair_target(evidence))
            .with_candidate_artifacts(evidence.candidate_artifacts.clone())
            .with_success_check_opt(success_check(evidence))
            .with_evidence_signature_opt(evidence.failure_signature.clone())
            .with_repair_kind_opt(evidence.repair_kind.clone())
            .with_repair_action_opt(evidence.repair_action.clone())
            .with_semantic_failure_kind_opt(evidence.semantic_failure_kind.clone())
            .with_source_of_truth_opt(evidence.source_of_truth.clone())
            .with_allowed_change_kind_opt(evidence.allowed_change_kind.clone())
            .with_expected_evidence_delta_opt(evidence.expected_evidence_delta.clone())
            .with_setup_implication_opt(evidence.setup_implication.clone())
            .with_tool_policy_projection_opt(evidence.tool_policy_projection.clone())
            .with_target_admission_opt(evidence.target_admission.clone())
            .with_target_priority_opt(evidence.target_priority.clone())
            .with_workspace_scope_opt(evidence.workspace_scope.clone())
            .with_artifact_ownership_opt(evidence.artifact_ownership.clone())
            .with_active_job_priority_opt(evidence.active_job_priority.clone())
            .with_explicit_stop_reason_opt(evidence.explicit_stop_reason.clone())
            .with_artifact_graph_summary(evidence.artifact_graph_summary.clone())
            .with_rerun_authority(evidence.rerun_authority.clone())
            .with_repair_attempt_ledger(evidence.repair_attempt_ledger.clone())
            .with_execution_envelope_opt(execution_envelope(evidence));

        if evidence.guard == "tool_protocol"
            && let Some(tool) = evidence.tool.as_deref()
        {
            task = task.with_allowed_tool(tool);
        }
        for action in disallowed_actions(evidence) {
            task = task.with_disallowed_action(action);
        }

        if task.has_task_detail() {
            Some(task)
        } else {
            None
        }
    }

    pub fn with_failed_step(mut self, failed_step: impl Into<String>) -> Self {
        self.failed_step = Some(failed_step.into());
        self
    }

    pub fn with_contract_code(mut self, contract_code: impl Into<String>) -> Self {
        self.contract_code = Some(contract_code.into());
        self
    }

    pub fn with_active_job(mut self, active_job: impl Into<String>) -> Self {
        self.active_job = Some(active_job.into());
        self
    }

    pub fn with_artifact_role(mut self, artifact_role: impl Into<String>) -> Self {
        self.artifact_role = Some(artifact_role.into());
        self
    }

    pub fn with_blocker(mut self, blocker: impl Into<String>) -> Self {
        self.blocker = Some(blocker.into());
        self
    }

    pub fn with_required_action(mut self, required_action: impl Into<String>) -> Self {
        self.required_action = Some(required_action.into());
        self
    }

    pub fn with_repair_target(mut self, repair_target: impl Into<String>) -> Self {
        self.repair_target = Some(repair_target.into());
        self
    }

    pub fn with_candidate_artifacts<I, S>(mut self, candidate_artifacts: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for artifact in candidate_artifacts {
            self = self.with_candidate_artifact(artifact);
        }
        self
    }

    pub fn with_candidate_artifact(mut self, candidate_artifact: impl Into<String>) -> Self {
        push_unique(&mut self.candidate_artifacts, candidate_artifact.into());
        self
    }

    pub fn with_allowed_tool(mut self, allowed_tool: impl Into<String>) -> Self {
        push_unique(&mut self.allowed_tools, allowed_tool.into());
        self
    }

    pub fn with_disallowed_action(mut self, disallowed_action: impl Into<String>) -> Self {
        push_unique(&mut self.disallowed_actions, disallowed_action.into());
        self
    }

    pub fn with_success_check(mut self, success_check: impl Into<String>) -> Self {
        self.success_check = Some(success_check.into());
        self
    }

    pub fn with_evidence_signature(mut self, evidence_signature: impl Into<String>) -> Self {
        self.evidence_signature = Some(evidence_signature.into());
        self
    }

    pub fn with_repair_kind(mut self, repair_kind: impl Into<String>) -> Self {
        self.repair_kind = Some(repair_kind.into());
        self
    }

    pub fn with_repair_action(mut self, repair_action: impl Into<String>) -> Self {
        self.repair_action = Some(repair_action.into());
        self
    }

    pub fn with_semantic_failure_kind(mut self, semantic_failure_kind: impl Into<String>) -> Self {
        self.semantic_failure_kind = Some(semantic_failure_kind.into());
        self
    }

    pub fn with_source_of_truth(mut self, source_of_truth: impl Into<String>) -> Self {
        self.source_of_truth = Some(source_of_truth.into());
        self
    }

    pub fn with_allowed_change_kind(mut self, allowed_change_kind: impl Into<String>) -> Self {
        self.allowed_change_kind = Some(allowed_change_kind.into());
        self
    }

    pub fn with_expected_evidence_delta(
        mut self,
        expected_evidence_delta: impl Into<String>,
    ) -> Self {
        self.expected_evidence_delta = Some(expected_evidence_delta.into());
        self
    }

    pub fn with_setup_implication(mut self, setup_implication: impl Into<String>) -> Self {
        self.setup_implication = Some(setup_implication.into());
        self
    }

    pub fn with_tool_policy_projection(
        mut self,
        tool_policy_projection: impl Into<String>,
    ) -> Self {
        self.tool_policy_projection = Some(tool_policy_projection.into());
        self
    }

    pub fn with_target_admission(mut self, target_admission: impl Into<String>) -> Self {
        self.target_admission = Some(target_admission.into());
        self
    }

    pub fn with_target_priority(mut self, target_priority: impl Into<String>) -> Self {
        self.target_priority = Some(target_priority.into());
        self
    }

    pub fn with_workspace_scope(mut self, workspace_scope: impl Into<String>) -> Self {
        self.workspace_scope = Some(workspace_scope.into());
        self
    }

    pub fn with_artifact_ownership(mut self, artifact_ownership: impl Into<String>) -> Self {
        self.artifact_ownership = Some(artifact_ownership.into());
        self
    }

    pub fn with_active_job_priority(mut self, active_job_priority: impl Into<String>) -> Self {
        self.active_job_priority = Some(active_job_priority.into());
        self
    }

    pub fn with_explicit_stop_reason(mut self, explicit_stop_reason: impl Into<String>) -> Self {
        self.explicit_stop_reason = Some(explicit_stop_reason.into());
        self
    }

    pub fn with_artifact_graph_summary<I, S>(mut self, artifact_graph_summary: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for summary in artifact_graph_summary {
            self = self.with_artifact_graph_summary_item(summary);
        }
        self
    }

    pub fn with_artifact_graph_summary_item(mut self, summary: impl Into<String>) -> Self {
        push_unique(&mut self.artifact_graph_summary, summary.into());
        self
    }

    pub fn with_rerun_authority<I, S>(mut self, rerun_authority: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for authority in rerun_authority {
            self = self.with_rerun_authority_item(authority);
        }
        self
    }

    pub fn with_rerun_authority_item(mut self, authority: impl Into<String>) -> Self {
        push_unique(&mut self.rerun_authority, authority.into());
        self
    }

    pub fn with_repair_attempt_ledger<I, S>(mut self, repair_attempt_ledger: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for entry in repair_attempt_ledger {
            self = self.with_repair_attempt_ledger_item(entry);
        }
        self
    }

    pub fn with_repair_attempt_ledger_item(mut self, entry: impl Into<String>) -> Self {
        push_unique(&mut self.repair_attempt_ledger, entry.into());
        self
    }

    pub fn with_execution_envelope(mut self, envelope: RecoveryExecutionEnvelope) -> Self {
        self.execution_envelope = Some(envelope);
        self
    }

    pub fn render(&self) -> Option<String> {
        if self.source.trim().is_empty() || !self.has_task_detail() {
            return None;
        }

        let mut lines = Vec::new();
        push_field(&mut lines, "source", Some(&self.source));
        push_field(&mut lines, "failed_step", self.failed_step.as_deref());
        push_field(&mut lines, "contract_code", self.contract_code.as_deref());
        push_field(&mut lines, "active_job", self.active_job.as_deref());
        push_field(&mut lines, "artifact_role", self.artifact_role.as_deref());
        push_field(&mut lines, "blocker", self.blocker.as_deref());
        push_field(
            &mut lines,
            "required_action",
            self.required_action.as_deref(),
        );
        push_field(&mut lines, "repair_target", self.repair_target.as_deref());
        push_list(&mut lines, "candidate_artifacts", &self.candidate_artifacts);
        push_list(&mut lines, "allowed_tools", &self.allowed_tools);
        push_list(&mut lines, "disallowed_actions", &self.disallowed_actions);
        push_field(&mut lines, "success_check", self.success_check.as_deref());
        push_field(
            &mut lines,
            "evidence_signature",
            self.evidence_signature.as_deref(),
        );
        push_field(&mut lines, "repair_kind", self.repair_kind.as_deref());
        push_field(&mut lines, "repair_action", self.repair_action.as_deref());
        push_field(
            &mut lines,
            "semantic_failure_kind",
            self.semantic_failure_kind.as_deref(),
        );
        push_field(
            &mut lines,
            "source_of_truth",
            self.source_of_truth.as_deref(),
        );
        push_field(
            &mut lines,
            "allowed_change_kind",
            self.allowed_change_kind.as_deref(),
        );
        push_field(
            &mut lines,
            "expected_evidence_delta",
            self.expected_evidence_delta.as_deref(),
        );
        push_field(
            &mut lines,
            "setup_implication",
            self.setup_implication.as_deref(),
        );
        push_field(
            &mut lines,
            "tool_policy_projection",
            self.tool_policy_projection.as_deref(),
        );
        push_field(
            &mut lines,
            "target_admission",
            self.target_admission.as_deref(),
        );
        push_field(
            &mut lines,
            "target_priority",
            self.target_priority.as_deref(),
        );
        push_field(
            &mut lines,
            "workspace_scope",
            self.workspace_scope.as_deref(),
        );
        push_field(
            &mut lines,
            "artifact_ownership",
            self.artifact_ownership.as_deref(),
        );
        push_field(
            &mut lines,
            "active_job_priority",
            self.active_job_priority.as_deref(),
        );
        push_field(
            &mut lines,
            "explicit_stop_reason",
            self.explicit_stop_reason.as_deref(),
        );
        push_list(
            &mut lines,
            "artifact_graph_summary",
            &self.artifact_graph_summary,
        );
        push_list(&mut lines, "rerun_authority", &self.rerun_authority);
        push_list(
            &mut lines,
            "repair_attempt_ledger",
            &self.repair_attempt_ledger,
        );
        if let Some(envelope) = self.execution_envelope {
            push_field(&mut lines, "execution_envelope", Some(envelope.as_str()));
            push_field(&mut lines, "tool_policy", Some(envelope.tool_policy()));
            push_field(
                &mut lines,
                "evidence_requirement",
                Some(envelope.evidence_requirement()),
            );
        }
        Some(lines.join("\n"))
    }

    fn with_failed_step_opt(self, failed_step: Option<String>) -> Self {
        match failed_step {
            Some(value) => self.with_failed_step(value),
            None => self,
        }
    }

    fn with_contract_code_opt(self, contract_code: Option<String>) -> Self {
        match contract_code {
            Some(value) => self.with_contract_code(value),
            None => self,
        }
    }

    fn with_active_job_opt(self, active_job: Option<String>) -> Self {
        match active_job {
            Some(value) => self.with_active_job(value),
            None => self,
        }
    }

    fn with_artifact_role_opt(self, artifact_role: Option<String>) -> Self {
        match artifact_role {
            Some(value) => self.with_artifact_role(value),
            None => self,
        }
    }

    fn with_blocker_opt(self, blocker: Option<String>) -> Self {
        match blocker {
            Some(value) => self.with_blocker(value),
            None => self,
        }
    }

    fn with_required_action_opt(self, required_action: Option<String>) -> Self {
        match required_action {
            Some(value) => self.with_required_action(value),
            None => self,
        }
    }

    fn with_repair_target_opt(self, repair_target: Option<String>) -> Self {
        match repair_target {
            Some(value) => self.with_repair_target(value),
            None => self,
        }
    }

    fn with_success_check_opt(self, success_check: Option<String>) -> Self {
        match success_check {
            Some(value) => self.with_success_check(value),
            None => self,
        }
    }

    fn with_evidence_signature_opt(self, evidence_signature: Option<String>) -> Self {
        match evidence_signature {
            Some(value) => self.with_evidence_signature(value),
            None => self,
        }
    }

    fn with_repair_kind_opt(self, repair_kind: Option<String>) -> Self {
        match repair_kind {
            Some(value) => self.with_repair_kind(value),
            None => self,
        }
    }

    fn with_repair_action_opt(self, repair_action: Option<String>) -> Self {
        match repair_action {
            Some(value) => self.with_repair_action(value),
            None => self,
        }
    }

    fn with_semantic_failure_kind_opt(self, semantic_failure_kind: Option<String>) -> Self {
        match semantic_failure_kind {
            Some(value) => self.with_semantic_failure_kind(value),
            None => self,
        }
    }

    fn with_source_of_truth_opt(self, source_of_truth: Option<String>) -> Self {
        match source_of_truth {
            Some(value) => self.with_source_of_truth(value),
            None => self,
        }
    }

    fn with_allowed_change_kind_opt(self, allowed_change_kind: Option<String>) -> Self {
        match allowed_change_kind {
            Some(value) => self.with_allowed_change_kind(value),
            None => self,
        }
    }

    fn with_expected_evidence_delta_opt(self, expected_evidence_delta: Option<String>) -> Self {
        match expected_evidence_delta {
            Some(value) => self.with_expected_evidence_delta(value),
            None => self,
        }
    }

    fn with_setup_implication_opt(self, setup_implication: Option<String>) -> Self {
        match setup_implication {
            Some(value) => self.with_setup_implication(value),
            None => self,
        }
    }

    fn with_tool_policy_projection_opt(self, tool_policy_projection: Option<String>) -> Self {
        match tool_policy_projection {
            Some(value) => self.with_tool_policy_projection(value),
            None => self,
        }
    }

    fn with_target_admission_opt(self, target_admission: Option<String>) -> Self {
        match target_admission {
            Some(value) => self.with_target_admission(value),
            None => self,
        }
    }

    fn with_target_priority_opt(self, target_priority: Option<String>) -> Self {
        match target_priority {
            Some(value) => self.with_target_priority(value),
            None => self,
        }
    }

    fn with_workspace_scope_opt(self, workspace_scope: Option<String>) -> Self {
        match workspace_scope {
            Some(value) => self.with_workspace_scope(value),
            None => self,
        }
    }

    fn with_artifact_ownership_opt(self, artifact_ownership: Option<String>) -> Self {
        match artifact_ownership {
            Some(value) => self.with_artifact_ownership(value),
            None => self,
        }
    }

    fn with_active_job_priority_opt(self, active_job_priority: Option<String>) -> Self {
        match active_job_priority {
            Some(value) => self.with_active_job_priority(value),
            None => self,
        }
    }

    fn with_explicit_stop_reason_opt(self, explicit_stop_reason: Option<String>) -> Self {
        match explicit_stop_reason {
            Some(value) => self.with_explicit_stop_reason(value),
            None => self,
        }
    }

    fn with_execution_envelope_opt(self, envelope: Option<RecoveryExecutionEnvelope>) -> Self {
        match envelope {
            Some(value) => self.with_execution_envelope(value),
            None => self,
        }
    }

    fn has_task_detail(&self) -> bool {
        self.blocker.is_some()
            || self.active_job.is_some()
            || self.artifact_role.is_some()
            || self.required_action.is_some()
            || self.repair_target.is_some()
            || !self.candidate_artifacts.is_empty()
            || !self.allowed_tools.is_empty()
            || !self.disallowed_actions.is_empty()
            || self.success_check.is_some()
            || self.repair_kind.is_some()
            || self.repair_action.is_some()
            || self.semantic_failure_kind.is_some()
            || self.source_of_truth.is_some()
            || self.allowed_change_kind.is_some()
            || self.expected_evidence_delta.is_some()
            || self.setup_implication.is_some()
            || self.tool_policy_projection.is_some()
            || self.target_admission.is_some()
            || self.target_priority.is_some()
            || self.workspace_scope.is_some()
            || self.artifact_ownership.is_some()
            || self.active_job_priority.is_some()
            || self.explicit_stop_reason.is_some()
            || !self.artifact_graph_summary.is_empty()
            || !self.rerun_authority.is_empty()
            || !self.repair_attempt_ledger.is_empty()
            || self.execution_envelope.is_some()
    }
}

pub fn recovery_execution_envelope(
    evidence: &[ContractEvidence],
) -> Option<RecoveryExecutionEnvelope> {
    evidence
        .iter()
        .cloned()
        .map(orchestrate_evidence)
        .filter_map(|item| {
            execution_envelope(&item)
                .map(|envelope| (recovery_priority(&item).unwrap_or(u8::MAX), envelope))
        })
        .min_by_key(|(priority, _)| *priority)
        .map(|(_, envelope)| envelope)
}

fn recovery_priority(evidence: &ContractEvidence) -> Option<u8> {
    evidence
        .active_job_priority
        .as_deref()
        .and_then(|value| value.parse::<u8>().ok())
}

fn known_recovery_source(source: &str) -> bool {
    matches!(
        source,
        "tool_protocol"
            | "step_policy"
            | "provider_transport"
            | "verifier"
            | "profile_verification"
    ) || source.starts_with("plan_lint.")
}

fn contract_code(evidence: &ContractEvidence) -> Option<String> {
    evidence
        .diagnostic_code
        .clone()
        .or_else(|| evidence.reason_code.clone())
        .or_else(|| evidence.violated_contract.clone())
}

fn recovery_repair_target(evidence: &ContractEvidence) -> Option<String> {
    if evidence.guard == "step_policy" {
        return evidence.repair_target.clone();
    }
    evidence
        .repair_target
        .clone()
        .or_else(|| evidence.target_path.clone())
}

fn blocker(evidence: &ContractEvidence) -> Option<String> {
    match evidence.guard.as_str() {
        "tool_protocol" => Some(format!(
            "Tool call violated schema{}",
            evidence
                .tool
                .as_deref()
                .map(|tool| format!(" for {tool}"))
                .unwrap_or_default()
        )),
        "step_policy" => Some(format!(
            "Step tool policy rejected{}",
            evidence
                .tool
                .as_deref()
                .map(|tool| format!(" {tool}"))
                .unwrap_or_default()
        )),
        "provider_transport" => Some(format!(
            "Provider transport could not parse the model response{}",
            evidence
                .diagnostic_code
                .as_deref()
                .map(|code| format!(": {code}"))
                .unwrap_or_default()
        )),
        "verifier" => Some(format!(
            "Verifier command failed{}",
            evidence
                .command
                .as_deref()
                .map(|command| format!(": {command}"))
                .unwrap_or_default()
        )),
        "profile_verification" => {
            let code = contract_code(evidence).unwrap_or_else(|| "profile contract".to_string());
            Some(format!("Profile verification failed: {code}"))
        }
        _ => evidence
            .diagnostic_code
            .as_deref()
            .or(evidence.reason_code.as_deref())
            .or(evidence.violated_contract.as_deref())
            .map(|code| format!("Contract rejected: {code}")),
    }
}

fn required_action(evidence: &ContractEvidence) -> Option<String> {
    if let Some(action) = evidence.required_action.clone() {
        return Some(action);
    }

    match evidence.guard.as_str() {
        "tool_protocol" => Some(format!(
            "Emit exactly one valid {} tool call with the required fields.",
            evidence.tool.as_deref().unwrap_or("tool")
        )),
        "step_policy" => Some(
            "Do not mutate in this step; move file changes into an explicit mutation-allowed create/edit/repair step."
                .to_string(),
        ),
        "verifier" => Some("Fix the original verifier failure before adding feature work.".to_string()),
        "profile_verification" => {
            if let Some(focus) = evidence.repair_focus.clone() {
                Some(focus)
            } else {
                Some("Fix the reported profile contract before adding feature work.".to_string())
            }
        }
        "provider_transport" => Some(
            "Produce a response that satisfies the shared tool-call transport contract; do not rely on provider-specific behavior."
                .to_string(),
        ),
        source if source.starts_with("plan_lint.") => Some(
            "Correct the rejected plan field using the exact missing literals or paths from the contract evidence."
                .to_string(),
        ),
        _ => evidence.repair_focus.clone(),
    }
}

fn success_check(evidence: &ContractEvidence) -> Option<String> {
    match evidence.guard.as_str() {
        "tool_protocol" => Some("tool schema validation".to_string()),
        "step_policy" => Some("step tool policy".to_string()),
        "provider_transport" => Some("provider response parser".to_string()),
        "verifier" => evidence.command.clone(),
        "profile_verification"
            if contract_code(evidence).as_deref()
                == Some("nextjs_integration_artifact_missing") =>
        {
            Some("missing artifact path exists, then profile verification".to_string())
        }
        "profile_verification" => Some("profile verification".to_string()),
        _ => None,
    }
}

fn execution_envelope(evidence: &ContractEvidence) -> Option<RecoveryExecutionEnvelope> {
    match evidence.tool_policy_projection.as_deref() {
        Some("read_only") | Some("verifier_owned_setup_only") | Some("explicit_stop") => {
            return Some(RecoveryExecutionEnvelope::ReadOnlyEvidence);
        }
        Some("setup_config_mutation_only") => {
            return Some(RecoveryExecutionEnvelope::SetupConfigMutation);
        }
        Some("tool_protocol_correction") => {
            return Some(RecoveryExecutionEnvelope::ToolProtocolCorrection);
        }
        Some("file_mutation_repair") => {
            return Some(RecoveryExecutionEnvelope::FileMutationRepair);
        }
        _ => {}
    }
    match evidence.guard.as_str() {
        "step_policy" if contract_code(evidence).as_deref() == Some("read_only_step_mutation") => {
            Some(RecoveryExecutionEnvelope::ReadOnlyEvidence)
        }
        "step_policy"
            if contract_code(evidence).as_deref() == Some("model_issued_dependency_setup") =>
        {
            Some(RecoveryExecutionEnvelope::ReadOnlyEvidence)
        }
        "step_policy"
            if contract_code(evidence).as_deref() == Some("setup_step_source_mutation") =>
        {
            Some(RecoveryExecutionEnvelope::SetupConfigMutation)
        }
        "tool_protocol" => Some(RecoveryExecutionEnvelope::ToolProtocolCorrection),
        "provider_transport" => Some(RecoveryExecutionEnvelope::ToolProtocolCorrection),
        "verifier" => Some(RecoveryExecutionEnvelope::FileMutationRepair),
        "profile_verification"
            if evidence.repair_target.is_some() || evidence.required_action.is_some() =>
        {
            Some(RecoveryExecutionEnvelope::FileMutationRepair)
        }
        _ => None,
    }
}

fn disallowed_actions(evidence: &ContractEvidence) -> Vec<String> {
    let mut actions = match evidence.guard.as_str() {
        "tool_protocol" => vec![
            "Do not answer in prose instead of a tool call.".to_string(),
            "Do not run dependency installation.".to_string(),
        ],
        "step_policy" => match contract_code(evidence).as_deref() {
            Some("read_only_step_mutation") => vec![
                "Do not use Write in a read-only step.".to_string(),
                "Do not use Edit in a read-only step.".to_string(),
                "Do not use mutating Bash in a read-only step.".to_string(),
            ],
            Some("setup_step_source_mutation") => vec![
                "Do not edit source routes or components in a setup step.".to_string(),
                "Do not change the step kind to bypass the setup/source boundary.".to_string(),
            ],
            Some("model_issued_dependency_setup") => vec![
                "Do not run npm install, npm ci, pnpm install, or yarn install from a model tool call.".to_string(),
                "Do not use mutating Bash to perform dependency setup.".to_string(),
            ],
            _ => vec!["Do not bypass the step tool policy.".to_string()],
        },
        "provider_transport" => vec![
            "Do not add provider/model-specific repair behavior.".to_string(),
            "Do not answer with malformed XML or JSON tool-call payloads.".to_string(),
            "Do not run dependency installation.".to_string(),
        ],
        "verifier" => vec![
            "Do not change the verifier command to fake success.".to_string(),
            "Do not rewrite build scripts to bypass errors.".to_string(),
            "Do not run dependency setup except through the existing approved setup path."
                .to_string(),
        ],
        "profile_verification" => vec![
            "Do not add unrelated feature work before fixing the profile contract.".to_string(),
        ]
        .into_iter()
        .chain(match contract_code(evidence).as_deref() {
            Some("nextjs_integration_artifact_missing") => vec![
                "Do not edit selected route integration before the missing artifact exists."
                    .to_string(),
            ],
            Some("nextjs_route_not_integrated") => vec![
                "Do not create placeholder artifacts when the unintegrated artifact already exists."
                    .to_string(),
            ],
            Some("nextjs_dependency_version_conflict") => {
                vec![
                    "Do not keep exact React pins below 18.2 with Next.js 14.".to_string(),
                    "Do not keep TypeScript 6 or @types/react 19 in generated Next.js 14/React 18 apps.".to_string(),
                    "Do not switch generated setup repair to latest packages as the compatibility strategy.".to_string(),
                    "Do not rewrite scripts.build away from next build.".to_string(),
                ]
            }
            Some("nextjs_missing_dependency") => vec![
                "Do not remove Next.js runtime dependencies to silence the profile contract."
                    .to_string(),
            ],
            Some("nextjs_build_script_drift") => vec![
                "Do not rewrite scripts.build away from next build to fake success.".to_string(),
            ],
            _ => Vec::new(),
        })
        .collect(),
        source if source.starts_with("plan_lint.") => vec![
            "Do not weaken or remove the rejected plan obligation.".to_string(),
            "Do not replace the failed check with dependency installation or cache checks."
                .to_string(),
        ],
        _ => Vec::new(),
    };
    for action in &evidence.disallowed_actions {
        push_unique(&mut actions, action.clone());
    }
    if evidence.setup_implication.as_deref() == Some("setup_after_manifest_repair_required") {
        push_unique(
            &mut actions,
            "Do not run npm install, npm ci, pnpm install, or yarn install from a model tool call; verifier-owned setup recovery handles approved setup.".to_string(),
        );
    }
    actions
}

fn push_field(lines: &mut Vec<String>, key: &str, value: Option<&str>) {
    let Some(value) = value else {
        return;
    };
    if value.trim().is_empty() {
        return;
    }
    lines.push(format!("- {key}: {}", bounded_value(value)));
}

fn push_list(lines: &mut Vec<String>, key: &str, values: &[String]) {
    if values.is_empty() {
        return;
    }
    let mut rendered = values
        .iter()
        .take(MAX_LIST_ITEMS)
        .map(|value| bounded_value(value))
        .collect::<Vec<_>>();
    if values.len() > MAX_LIST_ITEMS {
        rendered.push(format!("... ({} more)", values.len() - MAX_LIST_ITEMS));
    }
    lines.push(format!("- {key}: {}", rendered.join(", ")));
}

fn push_unique(values: &mut Vec<String>, value: String) {
    let bounded = bounded_value(&value);
    if !bounded.trim().is_empty() && !values.iter().any(|existing| existing == &bounded) {
        values.push(bounded);
        if values.len() > MAX_LIST_ITEMS {
            values.truncate(MAX_LIST_ITEMS);
        }
    }
}

fn bounded_value(value: &str) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out = String::new();
    for ch in normalized.chars().take(MAX_FIELD_CHARS) {
        out.push(ch);
    }
    if normalized.chars().count() > MAX_FIELD_CHARS {
        out.push_str("...");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_bounded_recovery_task() {
        let task = RecoveryTaskContract::new("verifier")
            .with_failed_step("verify-build")
            .with_contract_code("command_failed:1")
            .with_blocker("Verifier command failed: npm run build")
            .with_required_action("Fix the original verifier failure before adding feature work.")
            .with_repair_target("app/page.tsx")
            .with_candidate_artifacts((0..10).map(|index| format!("app/file-{index}.tsx")))
            .with_disallowed_action("Do not change the verifier command to fake success.")
            .with_success_check("npm run build")
            .with_evidence_signature("verifier|verify-build|npm run build|command_failed:1")
            .with_repair_kind("source_verifier_repair")
            .with_repair_action("repair_source_error")
            .with_setup_implication("none")
            .with_tool_policy_projection("file_mutation_repair")
            .with_target_admission("admitted: target app/page.tsx matches source repair")
            .with_target_priority("priority=0 repair_target from deterministic evidence")
            .with_artifact_graph_summary(vec![
                "app/page.tsx role=implementation lifecycle=required source=contract.repair_target",
            ])
            .with_rerun_authority(vec!["npm run build"])
            .with_execution_envelope(RecoveryExecutionEnvelope::FileMutationRepair);

        let rendered = task.render().unwrap();

        assert!(rendered.contains("- source: verifier"));
        assert!(rendered.contains("- failed_step: verify-build"));
        assert!(rendered.contains("- required_action: Fix the original verifier failure"));
        assert!(rendered.contains("- repair_target: app/page.tsx"));
        assert!(rendered.contains("execution_envelope: file_mutation_repair"));
        assert!(rendered.contains("app/file-0.tsx"));
        assert!(rendered.contains("success_check: npm run build"));
        assert!(rendered.contains("repair_kind: source_verifier_repair"));
        assert!(rendered.contains("repair_action: repair_source_error"));
        assert!(rendered.contains("setup_implication: none"));
        assert!(rendered.contains("tool_policy_projection: file_mutation_repair"));
        assert!(rendered.contains("target_admission: admitted"));
        assert!(rendered.contains("target_priority: priority=0"));
        assert!(rendered.contains("artifact_graph_summary: app/page.tsx"));
        assert!(rendered.contains("rerun_authority: npm run build"));
        assert!(!rendered.contains("app/file-9.tsx"));
    }

    #[test]
    fn empty_task_does_not_render() {
        let task = RecoveryTaskContract::new("verifier");

        assert!(task.render().is_none());
    }

    #[test]
    fn unknown_evidence_without_action_does_not_make_task() {
        let evidence = ContractEvidence::new("unknown_guard").with_diagnostic("something failed");

        assert!(RecoveryTaskContract::from_contract_evidence(&evidence).is_none());
    }

    #[test]
    fn verifier_evidence_becomes_recovery_task() {
        let evidence = ContractEvidence::new("verifier")
            .with_failed_step("verify-build")
            .with_violated_contract("command_failed:1")
            .with_command("npm run build")
            .with_repair_target("app/page.tsx")
            .with_candidate_artifacts(vec!["app/page.tsx"])
            .with_repair_kind("source_verifier_repair")
            .with_repair_action("repair_source_error")
            .with_rerun_authority(vec!["npm run build"])
            .with_failure_signature("verifier|verify-build|npm run build|command_failed:1");

        let rendered = RecoveryTaskContract::from_contract_evidence(&evidence)
            .unwrap()
            .render()
            .unwrap();

        assert!(rendered.contains("blocker: Verifier command failed: npm run build"));
        assert!(rendered.contains("required_action: Fix the original verifier failure"));
        assert!(rendered.contains("repair_target: app/page.tsx"));
        assert!(rendered.contains("success_check: npm run build"));
        assert!(rendered.contains("repair_kind: source_verifier_repair"));
        assert!(rendered.contains("repair_action: repair_source_error"));
        assert!(rendered.contains("rerun_authority: npm run build"));
        assert!(rendered.contains("execution_envelope: file_mutation_repair"));
        assert!(rendered.contains("Do not change the verifier command"));
    }

    #[test]
    fn step_policy_task_does_not_authorize_mutation() {
        let evidence = ContractEvidence::new("step_policy")
            .with_failed_step("inspect-source")
            .with_violated_contract("read_only_step_mutation")
            .with_tool("Write");

        let rendered = RecoveryTaskContract::from_contract_evidence(&evidence)
            .unwrap()
            .render()
            .unwrap();

        assert!(rendered.contains("Step tool policy rejected Write"));
        assert!(rendered.contains("Do not mutate in this step"));
        assert!(rendered.contains("Do not use Write in a read-only step"));
        assert!(rendered.contains("success_check: step tool policy"));
        assert!(rendered.contains("execution_envelope: read_only_evidence"));
        assert!(rendered.contains("evidence_requirement: repository_read_evidence"));
    }

    #[test]
    fn setup_step_source_mutation_uses_setup_config_envelope() {
        let evidence = ContractEvidence::new("step_policy")
            .with_failed_step("setup-dependencies")
            .with_violated_contract("setup_step_source_mutation")
            .with_reason_code("setup_step_source_mutation")
            .with_tool("Write")
            .with_target_path("app/globals.css")
            .with_required_action(
                "do not edit source routes/components in setup steps; move source changes into create/edit/repair steps",
            );

        let rendered = RecoveryTaskContract::from_contract_evidence(&evidence)
            .unwrap()
            .render()
            .unwrap();

        assert!(rendered.contains("Step tool policy rejected Write"));
        assert!(!rendered.contains("repair_target: app/globals.css"));
        assert!(rendered.contains("setup steps"));
        assert!(rendered.contains("Do not edit source routes or components"));
        assert!(rendered.contains("execution_envelope: setup_config_mutation"));
        assert!(rendered.contains("tool_policy: setup_config_mutation_only"));
        assert!(
            rendered
                .contains("evidence_requirement: setup_or_config_file_change_or_explicit_blocker")
        );
    }

    #[test]
    fn model_issued_dependency_setup_uses_read_only_envelope() {
        let evidence = ContractEvidence::new("step_policy")
            .with_failed_step("setup-dependencies")
            .with_violated_contract("model_issued_dependency_setup")
            .with_reason_code("model_issued_dependency_setup")
            .with_tool("Bash")
            .with_required_action(
                "do not run dependency installation from a model tool call; report the setup blocker",
            );

        let rendered = RecoveryTaskContract::from_contract_evidence(&evidence)
            .unwrap()
            .render()
            .unwrap();

        assert!(rendered.contains("Step tool policy rejected Bash"));
        assert!(rendered.contains("model_issued_dependency_setup"));
        assert!(rendered.contains("Do not run npm install"));
        assert!(rendered.contains("execution_envelope: read_only_evidence"));
    }

    #[test]
    fn profile_task_preserves_selected_route_target() {
        let evidence = ContractEvidence::new("profile_verification")
            .with_failed_step("phase-ui")
            .with_violated_contract("nextjs_route_not_integrated")
            .with_repair_target("app/page.tsx")
            .with_candidate_artifacts(vec!["app/page.tsx", "app/hooks/useGame.ts"])
            .with_repair_action("connect_artifact_to_selected_route")
            .with_required_action(
                "edit app/page.tsx so it imports or references app/hooks/useGame.ts",
            );

        let rendered = RecoveryTaskContract::from_contract_evidence(&evidence)
            .unwrap()
            .render()
            .unwrap();

        assert!(rendered.contains("Profile verification failed: nextjs_route_not_integrated"));
        assert!(rendered.contains("repair_target: app/page.tsx"));
        assert!(rendered.contains("repair_action: connect_artifact_to_selected_route"));
        assert!(rendered.contains("candidate_artifacts: app/page.tsx, app/hooks/useGame.ts"));
        assert!(rendered.contains("success_check: profile verification"));
        assert!(rendered.contains("execution_envelope: file_mutation_repair"));
    }

    #[test]
    fn profile_missing_artifact_task_targets_artifact_creation() {
        let evidence = ContractEvidence::new("profile_verification")
            .with_failed_step("phase-ui")
            .with_violated_contract("nextjs_integration_artifact_missing")
            .with_repair_target("components/SpaceInvaders.tsx")
            .with_candidate_artifacts(vec!["components/SpaceInvaders.tsx", "app/page.tsx"])
            .with_repair_action("create_missing_integration_artifact")
            .with_required_action(
                "create components/SpaceInvaders.tsx before editing selected route integration",
            );

        let rendered = RecoveryTaskContract::from_contract_evidence(&evidence)
            .unwrap()
            .render()
            .unwrap();

        assert!(
            rendered.contains("Profile verification failed: nextjs_integration_artifact_missing")
        );
        assert!(rendered.contains("repair_target: components/SpaceInvaders.tsx"));
        assert!(rendered.contains("repair_action: create_missing_integration_artifact"));
        assert!(
            rendered.contains("candidate_artifacts: components/SpaceInvaders.tsx, app/page.tsx")
        );
        assert!(rendered.contains("missing artifact path exists, then profile verification"));
        assert!(
            rendered.contains(
                "Do not edit selected route integration before the missing artifact exists"
            )
        );
        assert!(rendered.contains("execution_envelope: file_mutation_repair"));
    }

    #[test]
    fn missing_test_artifact_task_uses_artifact_completion_contract() {
        let evidence = ContractEvidence::new("profile_verification")
            .with_failed_step("verify-api")
            .with_reason_code("missing_required_artifact")
            .with_missing_paths(vec!["tests/test_app.py"]);

        let rendered = RecoveryTaskContract::from_contract_evidence(&evidence)
            .unwrap()
            .render()
            .unwrap();

        assert!(rendered.contains("active_job: test_artifact_completion"));
        assert!(rendered.contains("artifact_role: test"));
        assert!(rendered.contains("repair_action: create_required_artifact"));
        assert!(rendered.contains("repair_target: tests/test_app.py"));
        assert!(rendered.contains("attempt_limit=4"));
        assert!(rendered.contains("Do not edit implementation source"));
    }

    #[test]
    fn provider_transport_evidence_becomes_tool_protocol_recovery_task() {
        let evidence = ContractEvidence::new("provider_transport")
            .with_failed_step("create-game-component")
            .with_violated_contract("provider_transport_parse_failure")
            .with_reason_code("provider_transport_parse_failure")
            .with_diagnostic_code("xml_tool_call_missing_name")
            .with_diagnostic("Gemini JSON parse failed: tool call is missing a tool name");

        let rendered = RecoveryTaskContract::from_contract_evidence(&evidence)
            .unwrap()
            .render()
            .unwrap();

        assert!(rendered.contains("source: provider_transport"));
        assert!(rendered.contains("Provider transport could not parse the model response"));
        assert!(rendered.contains("provider response parser"));
        assert!(rendered.contains("execution_envelope: tool_protocol_correction"));
        assert!(rendered.contains("Do not add provider/model-specific repair behavior"));
    }

    #[test]
    fn plan_lint_evidence_becomes_plan_correction_recovery_task() {
        let evidence = ContractEvidence::new("plan_lint.profile_obligations")
            .with_failed_step("create-package-json")
            .with_violated_contract("nextjs_dependencies_required")
            .with_target_field("instruction")
            .with_active_job("manifest_repair")
            .with_artifact_role("manifest")
            .with_repair_action("add_manifest_dependency")
            .with_required_literals(vec!["next", "react", "react-dom"])
            .with_missing_literals(vec!["react-dom"])
            .with_disallowed_actions(vec!["Do not run npm install from the plan."]);

        let rendered = RecoveryTaskContract::from_contract_evidence(&evidence)
            .unwrap()
            .render()
            .unwrap();

        assert!(rendered.contains("source: plan_lint.profile_obligations"));
        assert!(rendered.contains("active_job: manifest_repair"));
        assert!(rendered.contains("artifact_role: manifest"));
        assert!(rendered.contains("repair_action: add_manifest_dependency"));
        assert!(rendered.contains("nextjs_dependencies_required"));
        assert!(rendered.contains("exact missing literals or paths"));
        assert!(rendered.contains("Do not run npm install from the plan"));
        assert!(rendered.contains("Do not weaken or remove the rejected plan obligation"));
    }

    #[test]
    fn envelope_selection_prefers_read_only_evidence() {
        let evidence = vec![
            ContractEvidence::new("verifier")
                .with_violated_contract("command_failed:1")
                .with_command("npm run build"),
            ContractEvidence::new("step_policy")
                .with_violated_contract("read_only_step_mutation")
                .with_tool("Write"),
        ];

        assert_eq!(
            recovery_execution_envelope(&evidence),
            Some(RecoveryExecutionEnvelope::ReadOnlyEvidence)
        );
    }
}
