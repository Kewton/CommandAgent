//! Structured facts from deterministic guards.
//!
//! Evidence is rendered into existing bounded correction or repair paths. It
//! must not carry retry state, target authority, semantic guesses, sidecar
//! results, memory references, or provider policy.

const MAX_FIELD_CHARS: usize = 240;
const MAX_LIST_ITEMS: usize = 8;

pub type PlanCorrectionEvidence = ContractEvidence;

/// Bounded data produced by a deterministic contract guard.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContractEvidence {
    pub guard: String,
    pub failed_step: Option<String>,
    pub violated_contract: Option<String>,
    pub reason_code: Option<String>,
    pub failure_signature: Option<String>,
    pub failure_kind: Option<String>,
    pub diagnostic_code: Option<String>,
    pub command: Option<String>,
    pub tool: Option<String>,
    pub target_field: Option<String>,
    pub target_path: Option<String>,
    pub candidate_artifacts: Vec<String>,
    pub repair_target: Option<String>,
    pub required_fields: Vec<String>,
    pub required_literals: Vec<String>,
    pub missing_literals: Vec<String>,
    pub required_paths: Vec<String>,
    pub missing_paths: Vec<String>,
    pub affected_cases: Vec<String>,
    pub observed_expected_pairs: Vec<String>,
    pub rejected_value: Option<String>,
    pub active_job: Option<String>,
    pub artifact_role: Option<String>,
    pub required_action: Option<String>,
    pub disallowed_actions: Vec<String>,
    pub related_source_excerpt: Option<String>,
    pub prior_attempts: Vec<String>,
    pub repair_attempt_ledger: Vec<String>,
    pub repair_focus: Option<String>,
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
    pub loop_control_action: Option<String>,
    pub dispatch_status: Option<String>,
    pub dispatch_reason: Option<String>,
    pub candidate_jobs: Vec<String>,
    pub tie_break_reason: Option<String>,
    pub explicit_stop_reason: Option<String>,
    pub recovery_owner: Option<String>,
    pub completion_evidence: Vec<String>,
    pub evidence_binding: Vec<String>,
    pub deliverable_obligations: Vec<String>,
    pub repair_action_plan: Vec<String>,
    pub semantic_failure_report: Vec<String>,
    pub repair_job_state: Vec<String>,
    pub attempt_outcomes: Vec<String>,
    pub patch_validation: Vec<String>,
    pub eval_report_fields: Vec<String>,
    pub artifact_graph_summary: Vec<String>,
    pub rerun_authority: Vec<String>,
    pub proposed_targets: Vec<String>,
    pub admitted_targets: Vec<String>,
    pub rejected_targets: Vec<String>,
    pub repair_brief: Vec<String>,
    pub selected_failure_cluster: Option<String>,
    pub repair_brief_status: Option<String>,
    pub action_envelope_status: Option<String>,
    pub diagnostic: Option<String>,
}

impl ContractEvidence {
    pub fn new(guard: impl Into<String>) -> Self {
        Self {
            guard: guard.into(),
            ..Self::default()
        }
    }

    pub fn with_failed_step(mut self, failed_step: impl Into<String>) -> Self {
        self.failed_step = Some(failed_step.into());
        self
    }

    pub fn with_violated_contract(mut self, violated_contract: impl Into<String>) -> Self {
        self.violated_contract = Some(violated_contract.into());
        self
    }

    pub fn with_reason_code(mut self, reason_code: impl Into<String>) -> Self {
        self.reason_code = Some(reason_code.into());
        self
    }

    pub fn with_failure_signature(mut self, failure_signature: impl Into<String>) -> Self {
        self.failure_signature = Some(failure_signature.into());
        self
    }

    pub fn with_failure_kind(mut self, failure_kind: impl Into<String>) -> Self {
        self.failure_kind = Some(failure_kind.into());
        self
    }

    pub fn with_diagnostic_code(mut self, diagnostic_code: impl Into<String>) -> Self {
        self.diagnostic_code = Some(diagnostic_code.into());
        self
    }

    pub fn with_command(mut self, command: impl Into<String>) -> Self {
        self.command = Some(command.into());
        self
    }

    pub fn with_tool(mut self, tool: impl Into<String>) -> Self {
        self.tool = Some(tool.into());
        self
    }

    pub fn with_target_field(mut self, target_field: impl Into<String>) -> Self {
        self.target_field = Some(target_field.into());
        self
    }

    pub fn with_target_path(mut self, target_path: impl Into<String>) -> Self {
        self.target_path = Some(target_path.into());
        self
    }

    pub fn with_candidate_artifacts<I, S>(mut self, candidate_artifacts: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.candidate_artifacts = collect_values(candidate_artifacts);
        self
    }

    pub fn with_repair_target(mut self, repair_target: impl Into<String>) -> Self {
        self.repair_target = Some(repair_target.into());
        self
    }

    pub fn with_required_fields<I, S>(mut self, required_fields: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.required_fields = collect_values(required_fields);
        self
    }

    pub fn with_required_literals<I, S>(mut self, required_literals: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.required_literals = collect_values(required_literals);
        self
    }

    pub fn with_missing_literals<I, S>(mut self, missing_literals: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.missing_literals = collect_values(missing_literals);
        self
    }

    pub fn with_required_paths<I, S>(mut self, required_paths: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.required_paths = collect_values(required_paths);
        self
    }

    pub fn with_missing_paths<I, S>(mut self, missing_paths: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.missing_paths = collect_values(missing_paths);
        self
    }

    pub fn with_affected_cases<I, S>(mut self, affected_cases: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.affected_cases = collect_values(affected_cases);
        self
    }

    pub fn with_observed_expected_pairs<I, S>(mut self, observed_expected_pairs: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.observed_expected_pairs = collect_values(observed_expected_pairs);
        self
    }

    pub fn with_rejected_value(mut self, rejected_value: impl Into<String>) -> Self {
        self.rejected_value = Some(rejected_value.into());
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

    pub fn with_required_action(mut self, required_action: impl Into<String>) -> Self {
        self.required_action = Some(required_action.into());
        self
    }

    pub fn with_disallowed_actions<I, S>(mut self, disallowed_actions: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.disallowed_actions = collect_values(disallowed_actions);
        self
    }

    pub fn with_repair_attempt(mut self, repair_attempt: impl Into<String>) -> Self {
        self.repair_attempt_ledger.push(repair_attempt.into());
        self
    }

    pub fn with_related_source_excerpt(
        mut self,
        related_source_excerpt: impl Into<String>,
    ) -> Self {
        self.related_source_excerpt = Some(related_source_excerpt.into());
        self
    }

    pub fn with_prior_attempts<I, S>(mut self, prior_attempts: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.prior_attempts = collect_values(prior_attempts);
        self
    }

    pub fn with_repair_attempt_ledger<I, S>(mut self, repair_attempt_ledger: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.repair_attempt_ledger = collect_values(repair_attempt_ledger);
        self
    }

    pub fn with_repair_focus(mut self, repair_focus: impl Into<String>) -> Self {
        self.repair_focus = Some(repair_focus.into());
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

    pub fn with_loop_control_action(mut self, loop_control_action: impl Into<String>) -> Self {
        self.loop_control_action = Some(loop_control_action.into());
        self
    }

    pub fn with_dispatch_status(mut self, dispatch_status: impl Into<String>) -> Self {
        self.dispatch_status = Some(dispatch_status.into());
        self
    }

    pub fn with_dispatch_reason(mut self, dispatch_reason: impl Into<String>) -> Self {
        self.dispatch_reason = Some(dispatch_reason.into());
        self
    }

    pub fn with_candidate_jobs<I, S>(mut self, candidate_jobs: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.candidate_jobs = collect_values(candidate_jobs);
        self
    }

    pub fn with_tie_break_reason(mut self, tie_break_reason: impl Into<String>) -> Self {
        self.tie_break_reason = Some(tie_break_reason.into());
        self
    }

    pub fn with_explicit_stop_reason(mut self, explicit_stop_reason: impl Into<String>) -> Self {
        self.explicit_stop_reason = Some(explicit_stop_reason.into());
        self
    }

    pub fn with_recovery_owner(mut self, recovery_owner: impl Into<String>) -> Self {
        self.recovery_owner = Some(recovery_owner.into());
        self
    }

    pub fn with_completion_evidence<I, S>(mut self, completion_evidence: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.completion_evidence = collect_values(completion_evidence);
        self
    }

    pub fn with_evidence_binding<I, S>(mut self, evidence_binding: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.evidence_binding = collect_values(evidence_binding);
        self
    }

    pub fn with_deliverable_obligations<I, S>(mut self, deliverable_obligations: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.deliverable_obligations = collect_values(deliverable_obligations);
        self
    }

    pub fn with_repair_action_plan<I, S>(mut self, repair_action_plan: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.repair_action_plan = collect_values(repair_action_plan);
        self
    }

    pub fn with_semantic_failure_report<I, S>(mut self, semantic_failure_report: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.semantic_failure_report = collect_values(semantic_failure_report);
        self
    }

    pub fn with_repair_job_state<I, S>(mut self, repair_job_state: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.repair_job_state = collect_values(repair_job_state);
        self
    }

    pub fn with_attempt_outcomes<I, S>(mut self, attempt_outcomes: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.attempt_outcomes = collect_values(attempt_outcomes);
        self
    }

    pub fn with_patch_validation<I, S>(mut self, patch_validation: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.patch_validation = collect_values(patch_validation);
        self
    }

    pub fn with_eval_report_fields<I, S>(mut self, eval_report_fields: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.eval_report_fields = collect_values(eval_report_fields);
        self
    }

    pub fn with_artifact_graph_summary<I, S>(mut self, artifact_graph_summary: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.artifact_graph_summary = collect_values(artifact_graph_summary);
        self
    }

    pub fn with_rerun_authority<I, S>(mut self, rerun_authority: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.rerun_authority = collect_values(rerun_authority);
        self
    }

    pub fn with_proposed_targets<I, S>(mut self, proposed_targets: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.proposed_targets = collect_values(proposed_targets);
        self
    }

    pub fn with_admitted_targets<I, S>(mut self, admitted_targets: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.admitted_targets = collect_values(admitted_targets);
        self
    }

    pub fn with_rejected_targets<I, S>(mut self, rejected_targets: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.rejected_targets = collect_values(rejected_targets);
        self
    }

    pub fn with_repair_brief<I, S>(mut self, repair_brief: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.repair_brief = collect_values(repair_brief);
        self
    }

    pub fn with_selected_failure_cluster(
        mut self,
        selected_failure_cluster: impl Into<String>,
    ) -> Self {
        self.selected_failure_cluster = Some(selected_failure_cluster.into());
        self
    }

    pub fn with_repair_brief_status(mut self, repair_brief_status: impl Into<String>) -> Self {
        self.repair_brief_status = Some(repair_brief_status.into());
        self
    }

    pub fn with_action_envelope_status(
        mut self,
        action_envelope_status: impl Into<String>,
    ) -> Self {
        self.action_envelope_status = Some(action_envelope_status.into());
        self
    }

    pub fn with_diagnostic(mut self, diagnostic: impl Into<String>) -> Self {
        self.diagnostic = Some(diagnostic.into());
        self
    }

    pub fn render(&self) -> Option<String> {
        if self.guard.trim().is_empty() && self.is_empty_without_guard() {
            return None;
        }

        let mut lines = vec!["Contract correction evidence:".to_string()];
        push_field(&mut lines, "guard", Some(&self.guard));
        push_field(&mut lines, "failed_step", self.failed_step.as_deref());
        push_field(
            &mut lines,
            "violated_contract",
            self.violated_contract.as_deref(),
        );
        push_field(&mut lines, "reason_code", self.reason_code.as_deref());
        push_field(
            &mut lines,
            "failure_signature",
            self.failure_signature.as_deref(),
        );
        push_field(&mut lines, "failure_kind", self.failure_kind.as_deref());
        push_field(
            &mut lines,
            "diagnostic_code",
            self.diagnostic_code.as_deref(),
        );
        push_field(&mut lines, "command", self.command.as_deref());
        push_field(&mut lines, "tool", self.tool.as_deref());
        push_field(&mut lines, "target_field", self.target_field.as_deref());
        push_field(&mut lines, "target_path", self.target_path.as_deref());
        push_list(&mut lines, "candidate_artifacts", &self.candidate_artifacts);
        push_field(&mut lines, "repair_target", self.repair_target.as_deref());
        push_list(&mut lines, "required_fields", &self.required_fields);
        push_list(&mut lines, "required_literals", &self.required_literals);
        push_list(&mut lines, "missing_literals", &self.missing_literals);
        push_list(&mut lines, "required_paths", &self.required_paths);
        push_list(&mut lines, "missing_paths", &self.missing_paths);
        push_list(&mut lines, "affected_cases", &self.affected_cases);
        push_list(
            &mut lines,
            "observed_expected_pairs",
            &self.observed_expected_pairs,
        );
        push_field(&mut lines, "rejected_value", self.rejected_value.as_deref());
        push_field(&mut lines, "active_job", self.active_job.as_deref());
        push_field(&mut lines, "artifact_role", self.artifact_role.as_deref());
        push_field(
            &mut lines,
            "required_action",
            self.required_action.as_deref(),
        );
        push_list(&mut lines, "disallowed_actions", &self.disallowed_actions);
        push_field(
            &mut lines,
            "related_source_excerpt",
            self.related_source_excerpt.as_deref(),
        );
        push_list(&mut lines, "prior_attempts", &self.prior_attempts);
        push_list(
            &mut lines,
            "repair_attempt_ledger",
            &self.repair_attempt_ledger,
        );
        push_field(&mut lines, "repair_focus", self.repair_focus.as_deref());
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
        push_list(&mut lines, "rerun_authority", &self.rerun_authority);
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
            "loop_control_action",
            self.loop_control_action.as_deref(),
        );
        push_field(
            &mut lines,
            "dispatch_status",
            self.dispatch_status.as_deref(),
        );
        push_field(
            &mut lines,
            "dispatch_reason",
            self.dispatch_reason.as_deref(),
        );
        push_list(&mut lines, "candidate_jobs", &self.candidate_jobs);
        push_field(
            &mut lines,
            "tie_break_reason",
            self.tie_break_reason.as_deref(),
        );
        push_field(
            &mut lines,
            "explicit_stop_reason",
            self.explicit_stop_reason.as_deref(),
        );
        push_field(&mut lines, "recovery_owner", self.recovery_owner.as_deref());
        push_list(&mut lines, "completion_evidence", &self.completion_evidence);
        push_list(&mut lines, "evidence_binding", &self.evidence_binding);
        push_list(
            &mut lines,
            "deliverable_obligations",
            &self.deliverable_obligations,
        );
        push_list(&mut lines, "repair_action_plan", &self.repair_action_plan);
        push_list(
            &mut lines,
            "semantic_failure_report",
            &self.semantic_failure_report,
        );
        push_list(&mut lines, "repair_job_state", &self.repair_job_state);
        push_list(&mut lines, "attempt_outcomes", &self.attempt_outcomes);
        push_list(&mut lines, "patch_validation", &self.patch_validation);
        push_list(&mut lines, "eval_report_fields", &self.eval_report_fields);
        push_list(
            &mut lines,
            "artifact_graph_summary",
            &self.artifact_graph_summary,
        );
        push_list(&mut lines, "proposed_targets", &self.proposed_targets);
        push_list(&mut lines, "admitted_targets", &self.admitted_targets);
        push_list(&mut lines, "rejected_targets", &self.rejected_targets);
        push_field(
            &mut lines,
            "selected_failure_cluster",
            self.selected_failure_cluster.as_deref(),
        );
        push_field(
            &mut lines,
            "repair_brief_status",
            self.repair_brief_status.as_deref(),
        );
        push_field(
            &mut lines,
            "action_envelope_status",
            self.action_envelope_status.as_deref(),
        );
        push_field(&mut lines, "diagnostic", self.diagnostic.as_deref());
        Some(lines.join("\n"))
    }

    fn is_empty_without_guard(&self) -> bool {
        self.failed_step.is_none()
            && self.violated_contract.is_none()
            && self.reason_code.is_none()
            && self.failure_signature.is_none()
            && self.failure_kind.is_none()
            && self.diagnostic_code.is_none()
            && self.command.is_none()
            && self.tool.is_none()
            && self.target_field.is_none()
            && self.target_path.is_none()
            && self.candidate_artifacts.is_empty()
            && self.repair_target.is_none()
            && self.required_fields.is_empty()
            && self.required_literals.is_empty()
            && self.missing_literals.is_empty()
            && self.required_paths.is_empty()
            && self.missing_paths.is_empty()
            && self.affected_cases.is_empty()
            && self.observed_expected_pairs.is_empty()
            && self.rejected_value.is_none()
            && self.active_job.is_none()
            && self.artifact_role.is_none()
            && self.required_action.is_none()
            && self.disallowed_actions.is_empty()
            && self.related_source_excerpt.is_none()
            && self.prior_attempts.is_empty()
            && self.repair_attempt_ledger.is_empty()
            && self.repair_focus.is_none()
            && self.repair_kind.is_none()
            && self.repair_action.is_none()
            && self.semantic_failure_kind.is_none()
            && self.source_of_truth.is_none()
            && self.allowed_change_kind.is_none()
            && self.expected_evidence_delta.is_none()
            && self.setup_implication.is_none()
            && self.tool_policy_projection.is_none()
            && self.target_admission.is_none()
            && self.target_priority.is_none()
            && self.workspace_scope.is_none()
            && self.artifact_ownership.is_none()
            && self.active_job_priority.is_none()
            && self.loop_control_action.is_none()
            && self.dispatch_status.is_none()
            && self.dispatch_reason.is_none()
            && self.candidate_jobs.is_empty()
            && self.tie_break_reason.is_none()
            && self.explicit_stop_reason.is_none()
            && self.recovery_owner.is_none()
            && self.completion_evidence.is_empty()
            && self.evidence_binding.is_empty()
            && self.deliverable_obligations.is_empty()
            && self.repair_action_plan.is_empty()
            && self.semantic_failure_report.is_empty()
            && self.repair_job_state.is_empty()
            && self.attempt_outcomes.is_empty()
            && self.patch_validation.is_empty()
            && self.eval_report_fields.is_empty()
            && self.artifact_graph_summary.is_empty()
            && self.rerun_authority.is_empty()
            && self.proposed_targets.is_empty()
            && self.admitted_targets.is_empty()
            && self.rejected_targets.is_empty()
            && self.repair_brief.is_empty()
            && self.selected_failure_cluster.is_none()
            && self.repair_brief_status.is_none()
            && self.action_envelope_status.is_none()
            && self.diagnostic.is_none()
    }
}

pub fn failure_signature<I, S>(parts: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let normalized = parts
        .into_iter()
        .filter_map(|part| {
            let value = bounded_value(part.as_ref());
            if value.trim().is_empty() {
                None
            } else {
                Some(value)
            }
        })
        .collect::<Vec<_>>()
        .join("|");
    bounded_value(&normalized)
}

fn collect_values<I, S>(values: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    values.into_iter().map(Into::into).collect()
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
    fn plan_correction_evidence_renders_missing_literals() {
        let evidence = ContractEvidence::new("plan_lint.profile_obligations")
            .with_failed_step("create-package-json")
            .with_violated_contract("nextjs_dependencies_required")
            .with_target_field("instruction")
            .with_required_literals(vec![
                "next".to_string(),
                "react".to_string(),
                "react-dom".to_string(),
            ])
            .with_missing_literals(vec!["react-dom".to_string()])
            .with_required_action(
                "include these exact literals in the corrected package.json step instruction"
                    .to_string(),
            );

        let rendered = evidence.render().unwrap();

        assert!(rendered.contains("Contract correction evidence"));
        assert!(rendered.contains("- guard: plan_lint.profile_obligations"));
        assert!(rendered.contains("- failed_step: create-package-json"));
        assert!(rendered.contains("- violated_contract: nextjs_dependencies_required"));
        assert!(rendered.contains("- required_literals: next, react, react-dom"));
        assert!(rendered.contains("- missing_literals: react-dom"));
    }

    #[test]
    fn contract_evidence_alias_keeps_plan_correction_name_usable() {
        let evidence = PlanCorrectionEvidence::new("plan_lint.profile_obligations")
            .with_failed_step("create-package-json")
            .with_required_paths(vec!["package.json"])
            .with_missing_paths(vec!["package.json"])
            .with_diagnostic("missing expected path");

        let rendered = evidence.render().unwrap();

        assert!(rendered.contains("- failed_step: create-package-json"));
        assert!(rendered.contains("- required_paths: package.json"));
        assert!(rendered.contains("- missing_paths: package.json"));
        assert!(rendered.contains("- diagnostic: missing expected path"));
    }

    #[test]
    fn contract_evidence_renders_runtime_fields() {
        let evidence = ContractEvidence::new("tool_protocol")
            .with_failed_step("create-game-canvas")
            .with_violated_contract("tool_args_missing_required_field")
            .with_reason_code("tool_args_missing_required_field")
            .with_failure_signature("tool_protocol|create-game-canvas|Write|path")
            .with_failure_kind("tool_protocol_error")
            .with_diagnostic_code("tool_args_missing_required_field")
            .with_command("npm run build")
            .with_tool("Write")
            .with_target_field("path")
            .with_target_path("src/components/GameCanvas.tsx")
            .with_candidate_artifacts(vec!["src/components/GameCanvas.tsx"])
            .with_repair_target("src/components/GameCanvas.tsx")
            .with_required_fields(vec!["path", "content"])
            .with_observed_expected_pairs(vec![
                "observed=missing path; expected=Write.path".to_string(),
            ])
            .with_active_job("source_implementation_repair")
            .with_artifact_role("source")
            .with_related_source_excerpt("src/components/GameCanvas.tsx:1\n>1: broken")
            .with_disallowed_actions(vec!["do not run dependency setup"])
            .with_prior_attempts(vec!["attempt 1: same signature"])
            .with_repair_attempt_ledger(vec![
                "repair attempt 1: tool_protocol|create-game-canvas|Write|path",
            ])
            .with_repair_focus("emit valid Write call for src/components/GameCanvas.tsx")
            .with_repair_kind("tool_protocol_correction")
            .with_repair_action("repair_source_error")
            .with_setup_implication("none")
            .with_tool_policy_projection("file_mutation_repair")
            .with_target_admission("admitted: target src/components/GameCanvas.tsx")
            .with_target_priority("priority=0 repair_target from deterministic evidence")
            .with_artifact_graph_summary(vec![
                "src/components/GameCanvas.tsx role=implementation lifecycle=required source=contract.repair_target",
            ])
            .with_rerun_authority(vec!["tool schema validation"])
            .with_diagnostic("Write missing path");

        let rendered = evidence.render().unwrap();

        assert!(rendered.contains("- guard: tool_protocol"));
        assert!(rendered.contains("- failed_step: create-game-canvas"));
        assert!(rendered.contains("- violated_contract: tool_args_missing_required_field"));
        assert!(rendered.contains("- reason_code: tool_args_missing_required_field"));
        assert!(
            rendered.contains("- failure_signature: tool_protocol|create-game-canvas|Write|path")
        );
        assert!(rendered.contains("- failure_kind: tool_protocol_error"));
        assert!(rendered.contains("- diagnostic_code: tool_args_missing_required_field"));
        assert!(rendered.contains("- command: npm run build"));
        assert!(rendered.contains("- tool: Write"));
        assert!(rendered.contains("- target_field: path"));
        assert!(rendered.contains("- target_path: src/components/GameCanvas.tsx"));
        assert!(rendered.contains("- candidate_artifacts: src/components/GameCanvas.tsx"));
        assert!(rendered.contains("- repair_target: src/components/GameCanvas.tsx"));
        assert!(rendered.contains("- required_fields: path, content"));
        assert!(
            rendered
                .contains("- observed_expected_pairs: observed=missing path; expected=Write.path")
        );
        assert!(rendered.contains("- active_job: source_implementation_repair"));
        assert!(rendered.contains("- artifact_role: source"));
        assert!(rendered.contains("- disallowed_actions: do not run dependency setup"));
        assert!(
            rendered
                .contains("- related_source_excerpt: src/components/GameCanvas.tsx:1 >1: broken")
        );
        assert!(rendered.contains("- prior_attempts: attempt 1: same signature"));
        assert!(rendered.contains(
            "- repair_attempt_ledger: repair attempt 1: tool_protocol|create-game-canvas|Write|path"
        ));
        assert!(rendered.contains("- repair_focus: emit valid Write call"));
        assert!(rendered.contains("- repair_kind: tool_protocol_correction"));
        assert!(rendered.contains("- repair_action: repair_source_error"));
        assert!(rendered.contains("- setup_implication: none"));
        assert!(rendered.contains("- tool_policy_projection: file_mutation_repair"));
        assert!(rendered.contains("- target_admission: admitted"));
        assert!(rendered.contains("- target_priority: priority=0"));
        assert!(rendered.contains("- artifact_graph_summary: src/components/GameCanvas.tsx"));
        assert!(rendered.contains("- rerun_authority: tool schema validation"));
    }

    #[test]
    fn plan_correction_evidence_bounds_long_values() {
        let evidence = ContractEvidence::new("plan_lint.profile_obligations")
            .with_diagnostic("x".repeat(MAX_FIELD_CHARS + 20))
            .with_required_literals(
                (0..(MAX_LIST_ITEMS + 2)).map(|index| format!("literal-{index}")),
            );

        let rendered = evidence.render().unwrap();

        assert!(rendered.contains("..."));
        assert!(rendered.contains("2 more"));
        assert!(rendered.len() < 800);
    }

    #[test]
    fn failure_signature_is_stable_and_bounded() {
        let signature = failure_signature([
            " verifier ",
            "verify-build",
            "npm   run   build",
            "command_failed:1",
            "app/page.tsx",
        ]);

        assert_eq!(
            signature,
            "verifier|verify-build|npm run build|command_failed:1|app/page.tsx"
        );
        assert!(signature.len() <= MAX_FIELD_CHARS);
    }
}
