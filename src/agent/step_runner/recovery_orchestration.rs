#![allow(dead_code)]

use crate::agent::step_runner::active_job::RecoveryOwner;
use crate::agent::step_runner::artifact_completion::ArtifactCompletionJob;
use crate::agent::step_runner::artifact_graph::{
    ArtifactGraph, ArtifactRole, recovery_target_admissible, role_for_path,
};
use crate::agent::step_runner::correction_evidence::ContractEvidence;
use crate::agent::step_runner::recovery_contract;
use crate::agent::step_runner::repair_action_plan::{
    AllowedToolCategory, RepairActionPlan, RepairActionStatus,
};
use crate::agent::step_runner::repair_brief::{
    ActionEnvelopeStatus, RepairBrief, RepairBriefStatus,
};
use crate::agent::step_runner::semantic_failure::{SemanticFailureReport, SemanticRepairPlan};
use crate::agent::step_runner::target_admission::{
    RepairTargetCandidate, RepairTargetSource, TargetAdmissionPolicy,
    decide_repair_target_with_scope,
};
use crate::agent::step_runner::workspace_scope::WorkspaceScope;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecoveryJobKind {
    SetupBootstrap,
    ManifestRepair,
    ScaffoldMaterialization,
    RouteIntegrationRepair,
    SourceImplementationRepair,
    DevServerSmoke,
    TestArtifactCompletion,
    TestAlignmentRepair,
    DocumentationRepair,
    EvidenceBindingRepair,
    VerifierContractCorrection,
    ToolProtocolCorrection,
    ContractConflict,
    ExplicitStop,
}

impl RecoveryJobKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::SetupBootstrap => "setup_bootstrap",
            Self::ManifestRepair => "manifest_repair",
            Self::ScaffoldMaterialization => "scaffold_materialization",
            Self::RouteIntegrationRepair => "route_integration_repair",
            Self::SourceImplementationRepair => "source_implementation_repair",
            Self::DevServerSmoke => "dev_server_smoke",
            Self::TestArtifactCompletion => "test_artifact_completion",
            Self::TestAlignmentRepair => "test_alignment_repair",
            Self::DocumentationRepair => "documentation_repair",
            Self::EvidenceBindingRepair => "evidence_binding_repair",
            Self::VerifierContractCorrection => "verifier_contract_correction",
            Self::ToolProtocolCorrection => "tool_protocol_correction",
            Self::ContractConflict => "contract_conflict",
            Self::ExplicitStop => "explicit_stop",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecoveryActionKind {
    InstallOrPrepareDependencies,
    AddMissingManifestDependency,
    ResolveManifestConflict,
    CreateRequiredArtifact,
    ConnectExistingArtifactToEntrypoint,
    EditSourceForDiagnostic,
    RunDevServerSmoke,
    AlignTestAndVerifier,
    UpdateDocsLiteral,
    RepairEvidenceBinding,
    ReplaceInvalidVerifierCommand,
    CorrectToolProtocol,
    StopWithStructuredEvidence,
}

impl RecoveryActionKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::InstallOrPrepareDependencies => "install_or_prepare_dependencies",
            Self::AddMissingManifestDependency => "add_missing_manifest_dependency",
            Self::ResolveManifestConflict => "resolve_manifest_conflict",
            Self::CreateRequiredArtifact => "create_required_artifact",
            Self::ConnectExistingArtifactToEntrypoint => "connect_existing_artifact_to_entrypoint",
            Self::EditSourceForDiagnostic => "edit_source_for_diagnostic",
            Self::RunDevServerSmoke => "run_dev_server_smoke",
            Self::AlignTestAndVerifier => "align_test_and_verifier",
            Self::UpdateDocsLiteral => "update_docs_literal",
            Self::RepairEvidenceBinding => "repair_evidence_binding",
            Self::ReplaceInvalidVerifierCommand => "replace_invalid_verifier_command",
            Self::CorrectToolProtocol => "correct_tool_protocol",
            Self::StopWithStructuredEvidence => "stop_with_structured_evidence",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolPolicyProjection {
    ReadOnly,
    FileMutationRepair,
    SetupConfigMutationOnly,
    VerifierOwnedSetupOnly,
    ToolProtocolCorrection,
    ExplicitStop,
}

impl ToolPolicyProjection {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read_only",
            Self::FileMutationRepair => "file_mutation_repair",
            Self::SetupConfigMutationOnly => "setup_config_mutation_only",
            Self::VerifierOwnedSetupOnly => "verifier_owned_setup_only",
            Self::ToolProtocolCorrection => "tool_protocol_correction",
            Self::ExplicitStop => "explicit_stop",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LoopControlAction {
    RunBoundedRepairTask,
    RunVerifierOwnedSetup,
    RunDevServerSmoke,
    RunToolProtocolCorrection,
    RenderExplicitStop,
}

impl LoopControlAction {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::RunBoundedRepairTask => "run_bounded_repair_task",
            Self::RunVerifierOwnedSetup => "run_verifier_owned_setup",
            Self::RunDevServerSmoke => "run_dev_server_smoke",
            Self::RunToolProtocolCorrection => "run_tool_protocol_correction",
            Self::RenderExplicitStop => "render_explicit_stop",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DispatchStatus {
    Selected,
    ExplicitStop,
    AmbiguousTie,
    NoOwner,
}

impl DispatchStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Selected => "selected",
            Self::ExplicitStop => "explicit_stop",
            Self::AmbiguousTie => "ambiguous_tie",
            Self::NoOwner => "no_owner",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActiveJobCandidate {
    pub(crate) job: RecoveryJobKind,
    pub(crate) action: RecoveryActionKind,
    pub(crate) priority: u8,
    pub(crate) reason: String,
    pub(crate) source_of_truth: String,
    pub(crate) target_hint: Option<String>,
    pub(crate) artifact_role: Option<String>,
    pub(crate) rerun_authority: Vec<String>,
}

impl ActiveJobCandidate {
    fn render_line(&self) -> String {
        format!(
            "job={} action={} priority={} reason={}",
            self.job.as_str(),
            self.action.as_str(),
            self.priority,
            self.reason.split_whitespace().collect::<Vec<_>>().join(" ")
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActiveJobArbitration {
    pub(crate) selected_job: RecoveryJobKind,
    pub(crate) selected_action: RecoveryActionKind,
    pub(crate) selected_priority: u8,
    pub(crate) loop_control_action: LoopControlAction,
    pub(crate) dispatch_status: DispatchStatus,
    pub(crate) dispatch_reason: String,
    pub(crate) candidate_jobs: Vec<String>,
    pub(crate) tie_break_reason: Option<String>,
    pub(crate) explicit_stop_reason: Option<String>,
    pub(crate) rerun_authority: Vec<String>,
    pub(crate) tool_policy_projection: ToolPolicyProjection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecoveryOrchestrationDecision {
    pub(crate) job: RecoveryJobKind,
    pub(crate) action: RecoveryActionKind,
    pub(crate) target: Option<String>,
    pub(crate) artifact_role: Option<String>,
    pub(crate) target_admission: String,
    pub(crate) target_priority: String,
    pub(crate) required_action: String,
    pub(crate) disallowed_actions: Vec<String>,
    pub(crate) semantic_failure_kind: String,
    pub(crate) source_of_truth: String,
    pub(crate) allowed_change_kind: String,
    pub(crate) expected_evidence_delta: String,
    pub(crate) workspace_scope: String,
    pub(crate) artifact_ownership: String,
    pub(crate) active_job_priority: u8,
    pub(crate) loop_control_action: LoopControlAction,
    pub(crate) dispatch_status: DispatchStatus,
    pub(crate) dispatch_reason: String,
    pub(crate) candidate_jobs: Vec<String>,
    pub(crate) tie_break_reason: Option<String>,
    pub(crate) recovery_owner: String,
    pub(crate) tool_policy: ToolPolicyProjection,
    pub(crate) rerun_authority: Vec<String>,
    pub(crate) explicit_stop_reason: Option<String>,
    pub(crate) repair_action_plan: Vec<String>,
    pub(crate) semantic_failure_report: Vec<String>,
    pub(crate) eval_report_fields: Vec<String>,
    pub(crate) artifact_graph_summary: Vec<String>,
    pub(crate) proposed_targets: Vec<String>,
    pub(crate) admitted_targets: Vec<String>,
    pub(crate) rejected_targets: Vec<String>,
    pub(crate) repair_brief: Vec<String>,
    pub(crate) selected_failure_cluster: Option<String>,
    pub(crate) repair_brief_status: Option<String>,
    pub(crate) action_envelope_status: Option<String>,
    pub(crate) exhausted_clusters: Vec<String>,
    pub(crate) no_progress_strategy: Option<String>,
    pub(crate) repair_state_status: Option<String>,
}

impl RecoveryOrchestrationDecision {
    pub(crate) fn apply_to_evidence(&self, mut evidence: ContractEvidence) -> ContractEvidence {
        if evidence.active_job.is_none() {
            evidence = evidence.with_active_job(self.job.as_str());
        }
        if evidence.repair_action.is_none() {
            evidence = evidence.with_repair_action(self.action.as_str());
        }
        let disallowed_actions =
            merge_lists(&evidence.disallowed_actions, &self.disallowed_actions);
        evidence = evidence
            .with_disallowed_actions(disallowed_actions)
            .with_tool_policy_projection(self.tool_policy.as_str())
            .with_target_admission(self.target_admission.clone())
            .with_target_priority(self.target_priority.clone())
            .with_semantic_failure_kind(self.semantic_failure_kind.clone())
            .with_source_of_truth(self.source_of_truth.clone())
            .with_allowed_change_kind(self.allowed_change_kind.clone())
            .with_expected_evidence_delta(self.expected_evidence_delta.clone())
            .with_workspace_scope(self.workspace_scope.clone())
            .with_artifact_ownership(self.artifact_ownership.clone())
            .with_active_job_priority(self.active_job_priority.to_string())
            .with_loop_control_action(self.loop_control_action.as_str())
            .with_dispatch_status(self.dispatch_status.as_str())
            .with_dispatch_reason(self.dispatch_reason.clone())
            .with_candidate_jobs(self.candidate_jobs.clone())
            .with_recovery_owner(self.recovery_owner.clone())
            .with_artifact_graph_summary(self.artifact_graph_summary.clone());
        if let Some(reason) = &self.tie_break_reason {
            evidence = evidence.with_tie_break_reason(reason.clone());
        }
        if !self.repair_action_plan.is_empty() {
            let repair_action_plan =
                merge_lists(&evidence.repair_action_plan, &self.repair_action_plan);
            evidence = evidence.with_repair_action_plan(repair_action_plan);
        }
        if !self.semantic_failure_report.is_empty() {
            let semantic_failure_report = merge_lists(
                &evidence.semantic_failure_report,
                &self.semantic_failure_report,
            );
            evidence = evidence.with_semantic_failure_report(semantic_failure_report);
        }
        if !self.eval_report_fields.is_empty() {
            let eval_report_fields =
                merge_lists(&evidence.eval_report_fields, &self.eval_report_fields);
            evidence = evidence.with_eval_report_fields(eval_report_fields);
        }

        if evidence.repair_kind.is_none() {
            evidence = evidence.with_repair_kind(self.job.as_str());
        }
        if let Some(target) = &self.target {
            evidence = evidence
                .with_target_path(target.clone())
                .with_repair_target(target.clone());
        }
        if evidence.artifact_role.is_none()
            && let Some(role) = &self.artifact_role
        {
            evidence = evidence.with_artifact_role(role.clone());
        }
        if !self.rerun_authority.is_empty() {
            let rerun_authority = merge_lists(&evidence.rerun_authority, &self.rerun_authority);
            evidence = evidence.with_rerun_authority(rerun_authority);
        }
        if !self.proposed_targets.is_empty() {
            let proposed_targets = merge_lists(&evidence.proposed_targets, &self.proposed_targets);
            evidence = evidence.with_proposed_targets(proposed_targets);
        }
        if !self.admitted_targets.is_empty() {
            let admitted_targets = merge_lists(&evidence.admitted_targets, &self.admitted_targets);
            evidence = evidence.with_admitted_targets(admitted_targets);
        }
        if !self.rejected_targets.is_empty() {
            let rejected_targets = merge_lists(&evidence.rejected_targets, &self.rejected_targets);
            evidence = evidence.with_rejected_targets(rejected_targets);
        }
        if !self.repair_brief.is_empty() {
            let repair_brief = merge_lists(&evidence.repair_brief, &self.repair_brief);
            evidence = evidence.with_repair_brief(repair_brief);
        }
        if evidence.selected_failure_cluster.is_none()
            && let Some(cluster) = &self.selected_failure_cluster
        {
            evidence = evidence.with_selected_failure_cluster(cluster.clone());
        }
        if evidence.repair_brief_status.is_none()
            && let Some(status) = &self.repair_brief_status
        {
            evidence = evidence.with_repair_brief_status(status.clone());
        }
        if evidence.action_envelope_status.is_none()
            && let Some(status) = &self.action_envelope_status
        {
            evidence = evidence.with_action_envelope_status(status.clone());
        }
        if !self.exhausted_clusters.is_empty() {
            let exhausted_clusters =
                merge_lists(&evidence.exhausted_clusters, &self.exhausted_clusters);
            evidence = evidence.with_exhausted_clusters(exhausted_clusters);
        }
        if evidence.no_progress_strategy.is_none()
            && let Some(strategy) = &self.no_progress_strategy
        {
            evidence = evidence.with_no_progress_strategy(strategy.clone());
        }
        if evidence.repair_state_status.is_none()
            && let Some(status) = &self.repair_state_status
        {
            evidence = evidence.with_repair_state_status(status.clone());
        }
        if evidence.explicit_stop_reason.is_none()
            && let Some(reason) = &self.explicit_stop_reason
        {
            evidence = evidence.with_explicit_stop_reason(reason.clone());
        }
        evidence
    }
}

pub(crate) fn orchestrate_contract_evidence(
    evidence: &ContractEvidence,
) -> Option<RecoveryOrchestrationDecision> {
    if !known_orchestration_source(evidence) {
        return None;
    }
    let graph = ArtifactGraph::from_contract_evidence(evidence);
    let arbitration = arbitrate_active_job(active_job_candidates(evidence, &graph));
    let job = arbitration.selected_job;
    if job == RecoveryJobKind::ExplicitStop && evidence.guard.trim().is_empty() {
        return None;
    }
    let action = arbitration.selected_action;
    let scope = WorkspaceScope::from_graph(&graph);
    let target_policy = target_admission_policy(evidence, job, action);
    let target_decision = decide_repair_target_with_scope(
        target_candidates_for_job(evidence, &graph, job),
        &graph,
        &target_policy,
        &scope,
        &[],
    );
    let target = target_decision.selected_target.clone();
    let target_role = target_decision.selected_role;
    let role = target_role.map(|role| role.as_str().to_string());
    let target_admission = target_decision.target_admission_line();
    let explicit_stop_reason = if let Some(reason) = arbitration.explicit_stop_reason.clone() {
        Some(reason)
    } else if let Some(reason) = target_decision.explicit_stop_reason.clone() {
        Some(reason)
    } else if matches!(
        job,
        RecoveryJobKind::ExplicitStop | RecoveryJobKind::ContractConflict
    ) {
        Some("explicit_stop_from_deterministic_contract".to_string())
    } else {
        None
    };
    let target_priority = target_decision.target_priority_line();
    let tool_policy = arbitration.tool_policy_projection;
    let semantic_failure_kind = recovery_contract::semantic_failure_kind(
        evidence,
        job.as_str(),
        action.as_str(),
        target_role,
    )
    .to_string();
    let source_of_truth = recovery_contract::source_of_truth(evidence, job.as_str()).to_string();
    let allowed_change_kind =
        recovery_contract::allowed_change_kind(job.as_str(), action.as_str(), target_role)
            .to_string();
    let expected_evidence_delta =
        recovery_contract::expected_evidence_delta(evidence, job.as_str(), action.as_str());
    let workspace_scope =
        recovery_contract::workspace_scope(target.as_deref(), target_role).to_string();
    let artifact_ownership =
        recovery_contract::artifact_ownership(&graph, target.as_deref(), target_role);
    let active_job_priority = arbitration.selected_priority;
    let recovery_owner = RecoveryOwner::for_active_job(job.as_str())
        .as_str()
        .to_string();
    let required_action = required_action(evidence, job, action);
    let disallowed_actions = disallowed_actions(evidence, job, action);
    let rerun_authority = arbitration.rerun_authority.clone();
    let repair_action_plan = vec![
        RepairActionPlan {
            status: repair_action_status(&target_admission, explicit_stop_reason.as_deref()),
            target_role: target_role.map(|role| role.as_str().to_string()),
            target_path: target.clone(),
            allowed_change_kind: allowed_change_kind.clone(),
            allowed_tool_category: AllowedToolCategory::from_projection(tool_policy.as_str()),
            expected_delta: expected_evidence_delta.clone(),
            rejection_reason: explicit_stop_reason.clone().or_else(|| {
                target_admission
                    .starts_with("rejected")
                    .then(|| target_admission.clone())
            }),
            source_of_truth: source_of_truth.clone(),
        }
        .render_line(),
    ];
    let mut semantic_source = evidence
        .clone()
        .with_semantic_failure_kind(semantic_failure_kind.clone())
        .with_source_of_truth(source_of_truth.clone())
        .with_target_admission(target_admission.clone());
    if let Some(target) = &target {
        semantic_source = semantic_source.with_repair_target(target.clone());
    }
    if let Some(role) = target_role {
        semantic_source = semantic_source.with_artifact_role(role.as_str());
    }
    let semantic_plan = SemanticRepairPlan::from_contract_evidence(&semantic_source);
    let selected_failure_cluster = semantic_plan.selected_cluster_label();
    let semantic_failure_report = merge_lists(
        &SemanticFailureReport::from_contract_evidence(&semantic_source).render_lines(),
        &semantic_plan.render_lines(),
    );
    let repair_brief = RepairBrief {
        status: repair_brief_status(explicit_stop_reason.as_deref()),
        active_job: job.as_str().to_string(),
        recovery_owner: recovery_owner.clone(),
        repair_action: action.as_str().to_string(),
        selected_failure_cluster: selected_failure_cluster.clone(),
        selected_target: target.clone(),
        allowed_change_kind: allowed_change_kind.clone(),
        allowed_tool_category: AllowedToolCategory::from_projection(tool_policy.as_str()),
        disallowed_actions: disallowed_actions.clone(),
        must_preserve: repair_brief_must_preserve(evidence, &rerun_authority),
        success_check: repair_brief_success_check(evidence, &rerun_authority),
        rerun_authority: rerun_authority.clone(),
        explicit_stop_reason: explicit_stop_reason.clone(),
        action_envelope_status: action_envelope_status(explicit_stop_reason.as_deref()),
    };
    let repair_brief_lines = repair_brief.render_lines();
    let exhausted_clusters = exhausted_clusters_from_evidence(evidence);
    let no_progress_strategy = no_progress_strategy_from_evidence(evidence);
    let repair_state_status = repair_state_status_from_evidence(evidence);
    let eval_report_fields = merge_lists(
        &target_decision.eval_report_fields(),
        &repair_brief.eval_report_fields(),
    );
    let repair_state_eval_fields = repair_state_eval_fields(
        evidence,
        &exhausted_clusters,
        no_progress_strategy.as_deref(),
    );
    let eval_report_fields = merge_lists(&eval_report_fields, &repair_state_eval_fields);
    let eval_report_fields = merge_lists(
        &eval_report_fields,
        &[
            format!("active_job={}", job.as_str()),
            format!("recovery_owner={recovery_owner}"),
            format!("target_path={}", target.as_deref().unwrap_or("none")),
            format!(
                "target_role={}",
                target_role.map(|role| role.as_str()).unwrap_or("unknown")
            ),
            format!("repair_action={}", action.as_str()),
            format!("tool_policy={}", tool_policy.as_str()),
            format!("target_admission={target_admission}"),
            format!(
                "attempt_outcome={}",
                if explicit_stop_reason.is_some() {
                    "explicit_stop"
                } else {
                    "not_attempted"
                }
            ),
            format!(
                "evidence_binding_status={}",
                evidence_binding_status(evidence)
            ),
            format!(
                "completion_evidence_status={}",
                completion_evidence_status(evidence)
            ),
        ],
    );
    Some(RecoveryOrchestrationDecision {
        job,
        action,
        target,
        artifact_role: role,
        target_admission,
        target_priority,
        required_action,
        disallowed_actions,
        semantic_failure_kind,
        source_of_truth,
        allowed_change_kind,
        expected_evidence_delta,
        workspace_scope,
        artifact_ownership,
        active_job_priority,
        loop_control_action: arbitration.loop_control_action,
        dispatch_status: arbitration.dispatch_status,
        dispatch_reason: arbitration.dispatch_reason,
        candidate_jobs: arbitration.candidate_jobs,
        tie_break_reason: arbitration.tie_break_reason,
        recovery_owner,
        tool_policy,
        rerun_authority,
        explicit_stop_reason,
        repair_action_plan,
        semantic_failure_report,
        eval_report_fields,
        artifact_graph_summary: graph.summary(),
        proposed_targets: target_decision.proposed_lines(),
        admitted_targets: target_decision.admitted_lines(),
        rejected_targets: target_decision.rejected_lines(),
        repair_brief: repair_brief_lines,
        selected_failure_cluster: Some(selected_failure_cluster),
        repair_brief_status: Some(repair_brief.status.as_str().to_string()),
        action_envelope_status: Some(repair_brief.action_envelope_status.as_str().to_string()),
        exhausted_clusters,
        no_progress_strategy,
        repair_state_status,
    })
}

fn evidence_binding_status(evidence: &ContractEvidence) -> &'static str {
    if evidence
        .evidence_binding
        .iter()
        .any(|line| line.contains("status=missing"))
    {
        "missing"
    } else if evidence
        .evidence_binding
        .iter()
        .any(|line| line.contains("status=failed"))
    {
        "failed"
    } else if evidence
        .evidence_binding
        .iter()
        .any(|line| line.contains("status=unbound"))
    {
        "unbound"
    } else if evidence
        .evidence_binding
        .iter()
        .any(|line| line.contains("status=bound"))
    {
        "bound"
    } else {
        "unknown"
    }
}

fn completion_evidence_status(evidence: &ContractEvidence) -> &'static str {
    if evidence
        .completion_evidence
        .iter()
        .any(|line| line.contains("status=failed"))
    {
        "failed"
    } else if evidence
        .completion_evidence
        .iter()
        .any(|line| line.contains("status=missing"))
    {
        "missing"
    } else if evidence
        .completion_evidence
        .iter()
        .any(|line| line.contains("status=unbound"))
    {
        "unbound"
    } else if evidence
        .completion_evidence
        .iter()
        .any(|line| line.contains("status=passed"))
    {
        "passed"
    } else {
        "unknown"
    }
}

fn active_job_candidates(
    evidence: &ContractEvidence,
    graph: &ArtifactGraph,
) -> Vec<ActiveJobCandidate> {
    let mut candidates = Vec::new();
    if let Some(job) = evidence
        .active_job
        .as_deref()
        .and_then(|job| parse_active_job_for_evidence(evidence, job))
    {
        push_active_candidate(
            &mut candidates,
            evidence,
            graph,
            job,
            classify_action(evidence, job),
            dispatch_priority(job),
            "provided_active_job",
        );
    }

    let has_explicit_repair_policy =
        evidence.active_job.is_some() && evidence.repair_action.is_some();
    let code = primary_code(evidence);
    match code.as_deref() {
        Some("dependency_missing") | Some("nextjs_dependency_missing") => push_active_candidate(
            &mut candidates,
            evidence,
            graph,
            RecoveryJobKind::SetupBootstrap,
            RecoveryActionKind::InstallOrPrepareDependencies,
            dispatch_priority(RecoveryJobKind::SetupBootstrap),
            "dependency_missing",
        ),
        Some("model_issued_dependency_setup") => push_active_candidate(
            &mut candidates,
            evidence,
            graph,
            RecoveryJobKind::SetupBootstrap,
            RecoveryActionKind::InstallOrPrepareDependencies,
            dispatch_priority(RecoveryJobKind::SetupBootstrap),
            "model_issued_dependency_setup",
        ),
        Some("setup_step_source_mutation") => push_active_candidate(
            &mut candidates,
            evidence,
            graph,
            RecoveryJobKind::ManifestRepair,
            RecoveryActionKind::AddMissingManifestDependency,
            dispatch_priority(RecoveryJobKind::ManifestRepair),
            "setup_step_source_mutation",
        ),
        Some("read_only_step_mutation") => push_active_candidate(
            &mut candidates,
            evidence,
            graph,
            RecoveryJobKind::ExplicitStop,
            RecoveryActionKind::StopWithStructuredEvidence,
            dispatch_priority(RecoveryJobKind::ExplicitStop),
            "read_only_step_mutation",
        ),
        Some("tool_args_missing_required_field")
        | Some("tool_args_invalid_json")
        | Some("provider_transport_parse_failure") => push_active_candidate(
            &mut candidates,
            evidence,
            graph,
            RecoveryJobKind::ToolProtocolCorrection,
            RecoveryActionKind::CorrectToolProtocol,
            dispatch_priority(RecoveryJobKind::ToolProtocolCorrection),
            "tool_or_provider_protocol_failure",
        ),
        Some("nextjs_route_not_integrated") => push_active_candidate(
            &mut candidates,
            evidence,
            graph,
            RecoveryJobKind::RouteIntegrationRepair,
            RecoveryActionKind::ConnectExistingArtifactToEntrypoint,
            dispatch_priority(RecoveryJobKind::RouteIntegrationRepair),
            "profile_route_integration_failure",
        ),
        Some("nextjs_integration_artifact_missing") => push_active_candidate(
            &mut candidates,
            evidence,
            graph,
            RecoveryJobKind::ScaffoldMaterialization,
            RecoveryActionKind::CreateRequiredArtifact,
            dispatch_priority(RecoveryJobKind::ScaffoldMaterialization),
            "profile_missing_integration_artifact",
        ),
        Some("port_in_use")
        | Some("nextjs_dev_server_port_in_use")
        | Some("nextjs_dev_server_smoke_failed") => push_active_candidate(
            &mut candidates,
            evidence,
            graph,
            RecoveryJobKind::DevServerSmoke,
            RecoveryActionKind::RunDevServerSmoke,
            dispatch_priority(RecoveryJobKind::DevServerSmoke),
            "dev_server_smoke_failure",
        ),
        Some(code)
            if code.contains("missing")
                && code.contains("artifact")
                && evidence_has_missing_role(evidence, ArtifactRole::Test) =>
        {
            push_active_candidate(
                &mut candidates,
                evidence,
                graph,
                RecoveryJobKind::TestArtifactCompletion,
                RecoveryActionKind::CreateRequiredArtifact,
                30,
                "missing_test_artifact",
            )
        }
        Some(code)
            if code.contains("missing")
                && code.contains("artifact")
                && evidence_has_missing_role(evidence, ArtifactRole::Docs) =>
        {
            push_active_candidate(
                &mut candidates,
                evidence,
                graph,
                RecoveryJobKind::DocumentationRepair,
                RecoveryActionKind::UpdateDocsLiteral,
                30,
                "missing_documentation_artifact",
            )
        }
        Some(code) if code.starts_with("evidence_binding_") => push_active_candidate(
            &mut candidates,
            evidence,
            graph,
            RecoveryJobKind::EvidenceBindingRepair,
            RecoveryActionKind::RepairEvidenceBinding,
            dispatch_priority(RecoveryJobKind::EvidenceBindingRepair),
            "evidence_binding_failure",
        ),
        Some(code) if code.contains("contract_conflict") => push_active_candidate(
            &mut candidates,
            evidence,
            graph,
            RecoveryJobKind::ContractConflict,
            RecoveryActionKind::StopWithStructuredEvidence,
            dispatch_priority(RecoveryJobKind::ContractConflict),
            "contract_conflict",
        ),
        Some(code)
            if !has_explicit_repair_policy
                && (code.contains("dependency") || code.contains("manifest")) =>
        {
            let action = if code.contains("conflict") {
                RecoveryActionKind::ResolveManifestConflict
            } else {
                RecoveryActionKind::AddMissingManifestDependency
            };
            push_active_candidate(
                &mut candidates,
                evidence,
                graph,
                RecoveryJobKind::ManifestRepair,
                action,
                dispatch_priority(RecoveryJobKind::ManifestRepair),
                "manifest_or_dependency_contract_failure",
            );
        }
        Some(code) if code.contains("verifier_contract") => push_active_candidate(
            &mut candidates,
            evidence,
            graph,
            RecoveryJobKind::VerifierContractCorrection,
            RecoveryActionKind::ReplaceInvalidVerifierCommand,
            dispatch_priority(RecoveryJobKind::VerifierContractCorrection),
            "verifier_contract_failure",
        ),
        Some(code) if code.contains("test") => push_active_candidate(
            &mut candidates,
            evidence,
            graph,
            RecoveryJobKind::TestAlignmentRepair,
            RecoveryActionKind::AlignTestAndVerifier,
            dispatch_priority(RecoveryJobKind::TestAlignmentRepair),
            "test_alignment_failure",
        ),
        Some(code) if code.contains("docs") || code.contains("literal") => push_active_candidate(
            &mut candidates,
            evidence,
            graph,
            RecoveryJobKind::DocumentationRepair,
            RecoveryActionKind::UpdateDocsLiteral,
            dispatch_priority(RecoveryJobKind::DocumentationRepair),
            "documentation_contract_failure",
        ),
        _ => {}
    }

    match evidence.guard.as_str() {
        "profile_verification" => push_active_candidate(
            &mut candidates,
            evidence,
            graph,
            RecoveryJobKind::SourceImplementationRepair,
            RecoveryActionKind::EditSourceForDiagnostic,
            dispatch_priority(RecoveryJobKind::SourceImplementationRepair),
            "profile_verification_fallback",
        ),
        "verifier" => push_active_candidate(
            &mut candidates,
            evidence,
            graph,
            RecoveryJobKind::SourceImplementationRepair,
            RecoveryActionKind::EditSourceForDiagnostic,
            dispatch_priority(RecoveryJobKind::SourceImplementationRepair),
            "verifier_failure_fallback",
        ),
        "evidence_binding" => push_active_candidate(
            &mut candidates,
            evidence,
            graph,
            RecoveryJobKind::EvidenceBindingRepair,
            RecoveryActionKind::RepairEvidenceBinding,
            dispatch_priority(RecoveryJobKind::EvidenceBindingRepair),
            "evidence_binding_guard",
        ),
        "tool_protocol" | "provider_transport" => push_active_candidate(
            &mut candidates,
            evidence,
            graph,
            RecoveryJobKind::ToolProtocolCorrection,
            RecoveryActionKind::CorrectToolProtocol,
            dispatch_priority(RecoveryJobKind::ToolProtocolCorrection),
            "tool_protocol_guard",
        ),
        _ if evidence.guard.starts_with("plan_lint.") => push_active_candidate(
            &mut candidates,
            evidence,
            graph,
            RecoveryJobKind::VerifierContractCorrection,
            RecoveryActionKind::ReplaceInvalidVerifierCommand,
            dispatch_priority(RecoveryJobKind::VerifierContractCorrection),
            "plan_lint_guard",
        ),
        _ => {}
    }

    if candidates.is_empty() && known_orchestration_source(evidence) {
        push_active_candidate(
            &mut candidates,
            evidence,
            graph,
            RecoveryJobKind::ExplicitStop,
            RecoveryActionKind::StopWithStructuredEvidence,
            200,
            "no_classified_active_job",
        );
    }
    candidates
}

fn push_active_candidate(
    candidates: &mut Vec<ActiveJobCandidate>,
    evidence: &ContractEvidence,
    graph: &ArtifactGraph,
    job: RecoveryJobKind,
    action: RecoveryActionKind,
    priority: u8,
    reason: &'static str,
) {
    let target_hint = first_target_hint(evidence, job);
    let artifact_role = target_hint
        .as_deref()
        .map(|target| target_role_for(graph, target).as_str().to_string());
    let candidate = ActiveJobCandidate {
        job,
        action,
        priority,
        reason: reason.to_string(),
        source_of_truth: recovery_contract::source_of_truth(evidence, job.as_str()).to_string(),
        target_hint,
        artifact_role,
        rerun_authority: rerun_authority(evidence, job),
    };
    if let Some(existing) = candidates.iter_mut().find(|existing| {
        existing.job == candidate.job
            && existing.action == candidate.action
            && existing.target_hint == candidate.target_hint
    }) {
        if candidate.priority < existing.priority {
            *existing = candidate;
        }
        return;
    }
    candidates.push(candidate);
}

fn first_target_hint(evidence: &ContractEvidence, job: RecoveryJobKind) -> Option<String> {
    if matches!(
        job,
        RecoveryJobKind::SetupBootstrap | RecoveryJobKind::ManifestRepair
    ) {
        return Some("package.json".to_string());
    }
    evidence
        .repair_target
        .clone()
        .or_else(|| evidence.target_path.clone())
        .or_else(|| evidence.candidate_artifacts.first().cloned())
        .or_else(|| evidence.missing_paths.first().cloned())
        .or_else(|| evidence.required_paths.first().cloned())
}

fn arbitrate_active_job(candidates: Vec<ActiveJobCandidate>) -> ActiveJobArbitration {
    if candidates.is_empty() {
        return explicit_arbitration(
            RecoveryJobKind::ExplicitStop,
            RecoveryActionKind::StopWithStructuredEvidence,
            200,
            DispatchStatus::NoOwner,
            "no active job candidate was produced",
            "no_active_job_candidate",
            Vec::new(),
        );
    }

    let mut ordered = candidates.clone();
    ordered.sort_by_key(|candidate| candidate.priority);
    let selected_priority = ordered[0].priority;
    let top = ordered
        .iter()
        .filter(|candidate| candidate.priority == selected_priority)
        .collect::<Vec<_>>();
    let candidate_jobs = ordered
        .iter()
        .map(ActiveJobCandidate::render_line)
        .collect::<Vec<_>>();
    let first = top[0];
    let ambiguous = top.iter().any(|candidate| {
        candidate.job != first.job
            || candidate.action != first.action
            || candidate.target_hint != first.target_hint
    });
    if ambiguous {
        return explicit_arbitration(
            RecoveryJobKind::ContractConflict,
            RecoveryActionKind::StopWithStructuredEvidence,
            selected_priority,
            DispatchStatus::AmbiguousTie,
            "top priority active job candidates conflict",
            "active_job_tie",
            candidate_jobs,
        );
    }

    let status = if matches!(
        first.job,
        RecoveryJobKind::ExplicitStop | RecoveryJobKind::ContractConflict
    ) {
        DispatchStatus::ExplicitStop
    } else {
        DispatchStatus::Selected
    };
    let explicit_stop_reason = (status != DispatchStatus::Selected)
        .then(|| "explicit_stop_from_deterministic_contract".to_string());
    let action = first.action;
    let job = first.job;
    ActiveJobArbitration {
        selected_job: job,
        selected_action: action,
        selected_priority,
        loop_control_action: loop_control_action_for(job),
        dispatch_status: status,
        dispatch_reason: first.reason.clone(),
        candidate_jobs,
        tie_break_reason: None,
        explicit_stop_reason,
        rerun_authority: first.rerun_authority.clone(),
        tool_policy_projection: tool_policy_for(job, action),
    }
}

fn explicit_arbitration(
    job: RecoveryJobKind,
    action: RecoveryActionKind,
    selected_priority: u8,
    dispatch_status: DispatchStatus,
    dispatch_reason: &'static str,
    explicit_stop_reason: &'static str,
    candidate_jobs: Vec<String>,
) -> ActiveJobArbitration {
    ActiveJobArbitration {
        selected_job: job,
        selected_action: action,
        selected_priority,
        loop_control_action: LoopControlAction::RenderExplicitStop,
        dispatch_status,
        dispatch_reason: dispatch_reason.to_string(),
        candidate_jobs,
        tie_break_reason: (dispatch_status == DispatchStatus::AmbiguousTie)
            .then(|| explicit_stop_reason.to_string()),
        explicit_stop_reason: Some(explicit_stop_reason.to_string()),
        rerun_authority: Vec::new(),
        tool_policy_projection: ToolPolicyProjection::ExplicitStop,
    }
}

fn loop_control_action_for(job: RecoveryJobKind) -> LoopControlAction {
    match job {
        RecoveryJobKind::SetupBootstrap => LoopControlAction::RunVerifierOwnedSetup,
        RecoveryJobKind::DevServerSmoke => LoopControlAction::RunDevServerSmoke,
        RecoveryJobKind::ToolProtocolCorrection => LoopControlAction::RunToolProtocolCorrection,
        RecoveryJobKind::ExplicitStop | RecoveryJobKind::ContractConflict => {
            LoopControlAction::RenderExplicitStop
        }
        _ => LoopControlAction::RunBoundedRepairTask,
    }
}

fn dispatch_priority(job: RecoveryJobKind) -> u8 {
    recovery_contract::active_job_priority(job.as_str())
}

pub(crate) fn orchestrate_evidence(evidence: ContractEvidence) -> ContractEvidence {
    let Some(decision) = orchestrate_contract_evidence(&evidence) else {
        return evidence;
    };
    let enriched = decision.apply_to_evidence(evidence.clone());
    if decision.action == RecoveryActionKind::CreateRequiredArtifact
        && let Some(job) = ArtifactCompletionJob::from_contract_evidence(&enriched)
    {
        return job.apply_to_evidence(enriched);
    }
    enriched
}

fn classify_job(evidence: &ContractEvidence) -> RecoveryJobKind {
    if let Some(job) = evidence.active_job.as_deref().and_then(parse_job) {
        return job;
    }
    let code = primary_code(evidence);
    match code.as_deref() {
        Some("dependency_missing") | Some("nextjs_dependency_missing") => {
            RecoveryJobKind::SetupBootstrap
        }
        Some("model_issued_dependency_setup") => RecoveryJobKind::SetupBootstrap,
        Some("setup_step_source_mutation") => RecoveryJobKind::ManifestRepair,
        Some(code) if code.starts_with("evidence_binding_") => {
            RecoveryJobKind::EvidenceBindingRepair
        }
        Some(code) if code.contains("dependency") || code.contains("manifest") => {
            RecoveryJobKind::ManifestRepair
        }
        Some("nextjs_route_not_integrated") => RecoveryJobKind::RouteIntegrationRepair,
        Some("nextjs_integration_artifact_missing") => RecoveryJobKind::ScaffoldMaterialization,
        Some("port_in_use")
        | Some("nextjs_dev_server_port_in_use")
        | Some("nextjs_dev_server_smoke_failed") => RecoveryJobKind::DevServerSmoke,
        Some(code)
            if code.contains("missing")
                && code.contains("artifact")
                && evidence_has_missing_role(evidence, ArtifactRole::Test) =>
        {
            RecoveryJobKind::TestArtifactCompletion
        }
        Some(code)
            if code.contains("missing")
                && code.contains("artifact")
                && evidence_has_missing_role(evidence, ArtifactRole::Docs) =>
        {
            RecoveryJobKind::DocumentationRepair
        }
        Some("read_only_step_mutation") => RecoveryJobKind::ExplicitStop,
        Some("tool_args_missing_required_field")
        | Some("tool_args_invalid_json")
        | Some("provider_transport_parse_failure") => RecoveryJobKind::ToolProtocolCorrection,
        Some(code) if code.contains("test") || code.contains("verifier_contract") => {
            RecoveryJobKind::TestAlignmentRepair
        }
        Some(code) if code.contains("docs") || code.contains("literal") => {
            RecoveryJobKind::DocumentationRepair
        }
        _ if evidence.guard == "profile_verification" => {
            RecoveryJobKind::SourceImplementationRepair
        }
        _ if evidence.guard == "verifier" => RecoveryJobKind::SourceImplementationRepair,
        _ if evidence.guard == "evidence_binding" => RecoveryJobKind::EvidenceBindingRepair,
        _ if evidence.guard == "tool_protocol" || evidence.guard == "provider_transport" => {
            RecoveryJobKind::ToolProtocolCorrection
        }
        _ if evidence.guard.starts_with("plan_lint.") => {
            RecoveryJobKind::VerifierContractCorrection
        }
        _ => RecoveryJobKind::ExplicitStop,
    }
}

fn known_orchestration_source(evidence: &ContractEvidence) -> bool {
    matches!(
        evidence.guard.as_str(),
        "tool_protocol"
            | "step_policy"
            | "provider_transport"
            | "verifier"
            | "profile_verification"
            | "evidence_binding"
            | "setup"
            | "recovery"
            | "repair"
    ) || evidence.guard.starts_with("plan_lint.")
        || evidence.active_job.is_some()
        || evidence.repair_action.is_some()
        || evidence.repair_kind.is_some()
        || evidence.required_action.is_some()
        || evidence.repair_focus.is_some()
}

fn classify_action(evidence: &ContractEvidence, job: RecoveryJobKind) -> RecoveryActionKind {
    if let Some(action) = evidence.repair_action.as_deref().and_then(parse_action) {
        return action;
    }
    match job {
        RecoveryJobKind::SetupBootstrap => RecoveryActionKind::InstallOrPrepareDependencies,
        RecoveryJobKind::DevServerSmoke => RecoveryActionKind::RunDevServerSmoke,
        RecoveryJobKind::ManifestRepair => {
            let code = primary_code(evidence);
            if code
                .as_deref()
                .is_some_and(|code| code.contains("conflict"))
            {
                RecoveryActionKind::ResolveManifestConflict
            } else {
                RecoveryActionKind::AddMissingManifestDependency
            }
        }
        RecoveryJobKind::ScaffoldMaterialization => RecoveryActionKind::CreateRequiredArtifact,
        RecoveryJobKind::TestArtifactCompletion => RecoveryActionKind::CreateRequiredArtifact,
        RecoveryJobKind::RouteIntegrationRepair => {
            RecoveryActionKind::ConnectExistingArtifactToEntrypoint
        }
        RecoveryJobKind::SourceImplementationRepair => RecoveryActionKind::EditSourceForDiagnostic,
        RecoveryJobKind::TestAlignmentRepair => RecoveryActionKind::AlignTestAndVerifier,
        RecoveryJobKind::DocumentationRepair => RecoveryActionKind::UpdateDocsLiteral,
        RecoveryJobKind::EvidenceBindingRepair => RecoveryActionKind::RepairEvidenceBinding,
        RecoveryJobKind::VerifierContractCorrection => {
            RecoveryActionKind::ReplaceInvalidVerifierCommand
        }
        RecoveryJobKind::ToolProtocolCorrection => RecoveryActionKind::CorrectToolProtocol,
        RecoveryJobKind::ContractConflict => RecoveryActionKind::StopWithStructuredEvidence,
        RecoveryJobKind::ExplicitStop => RecoveryActionKind::StopWithStructuredEvidence,
    }
}

fn target_admission_policy(
    evidence: &ContractEvidence,
    job: RecoveryJobKind,
    action: RecoveryActionKind,
) -> TargetAdmissionPolicy {
    TargetAdmissionPolicy::new(
        job.as_str(),
        action.as_str(),
        allowed_target_roles(job),
        job_requires_target(job),
        job_allows_file_target(job),
    )
    .with_exhausted_targets(exhausted_targets_from_evidence(evidence))
    .with_exhausted_roles(exhausted_roles_from_evidence(evidence))
    .with_exhausted_clusters(exhausted_clusters_from_evidence(evidence))
    .with_current_cluster(evidence.selected_failure_cluster.clone())
}

fn target_candidates_for_job(
    evidence: &ContractEvidence,
    graph: &ArtifactGraph,
    job: RecoveryJobKind,
) -> Vec<RepairTargetCandidate> {
    let mut candidates = Vec::new();
    push_repair_target_candidate(
        &mut candidates,
        graph,
        evidence.repair_target.clone(),
        RepairTargetSource::FailureEvidence,
    );
    push_repair_target_candidate(
        &mut candidates,
        graph,
        evidence.target_path.clone(),
        RepairTargetSource::FailureEvidence,
    );
    if matches!(
        job,
        RecoveryJobKind::SetupBootstrap | RecoveryJobKind::ManifestRepair
    ) {
        push_repair_target_candidate(
            &mut candidates,
            graph,
            Some("package.json".to_string()),
            RepairTargetSource::SetupManifest,
        );
    }
    for path in &evidence.candidate_artifacts {
        push_repair_target_candidate(
            &mut candidates,
            graph,
            Some(path.clone()),
            candidate_source_for_job(job),
        );
        if job == RecoveryJobKind::RouteIntegrationRepair {
            for route in graph.integration_targets_for(path) {
                push_repair_target_candidate(
                    &mut candidates,
                    graph,
                    Some(route),
                    RepairTargetSource::ArtifactGraphRelation,
                );
            }
        }
        if matches!(
            job,
            RecoveryJobKind::SetupBootstrap | RecoveryJobKind::ManifestRepair
        ) && let Some(manifest) = graph.setup_manifest_for(path)
        {
            push_repair_target_candidate(
                &mut candidates,
                graph,
                Some(manifest),
                RepairTargetSource::SetupManifest,
            );
        }
    }
    for path in &evidence.missing_paths {
        push_repair_target_candidate(
            &mut candidates,
            graph,
            Some(path.clone()),
            RepairTargetSource::RequiredArtifact,
        );
    }
    for path in &evidence.required_paths {
        push_repair_target_candidate(
            &mut candidates,
            graph,
            Some(path.clone()),
            RepairTargetSource::RequiredArtifact,
        );
    }
    candidates
}

fn push_repair_target_candidate(
    candidates: &mut Vec<RepairTargetCandidate>,
    graph: &ArtifactGraph,
    path: Option<String>,
    source: RepairTargetSource,
) {
    let Some(path) = path else {
        return;
    };
    let path = path.trim().trim_start_matches("./").replace('\\', "/");
    if path.is_empty() || candidates.iter().any(|candidate| candidate.path == path) {
        return;
    }
    let role = graph.node(&path).map(|node| node.role).unwrap_or_else(|| {
        role_for_path(
            &path,
            crate::agent::step_runner::artifact_graph::ArtifactLifecycle::Required,
        )
    });
    candidates.push(RepairTargetCandidate { path, role, source });
}

fn candidate_source_for_job(job: RecoveryJobKind) -> RepairTargetSource {
    match job {
        RecoveryJobKind::RouteIntegrationRepair => RepairTargetSource::ProfileSelectedRoute,
        RecoveryJobKind::SourceImplementationRepair => RepairTargetSource::VerifierDiagnostic,
        RecoveryJobKind::DevServerSmoke => RepairTargetSource::VerifierDiagnostic,
        RecoveryJobKind::SetupBootstrap | RecoveryJobKind::ManifestRepair => {
            RepairTargetSource::SetupManifest
        }
        _ => RepairTargetSource::RequiredArtifact,
    }
}

fn allowed_target_roles(job: RecoveryJobKind) -> Vec<ArtifactRole> {
    match job {
        RecoveryJobKind::SetupBootstrap | RecoveryJobKind::ManifestRepair => {
            vec![ArtifactRole::SetupManifest, ArtifactRole::SetupConfig]
        }
        RecoveryJobKind::RouteIntegrationRepair => vec![
            ArtifactRole::Entrypoint,
            ArtifactRole::IntegrationTarget,
            ArtifactRole::Implementation,
            ArtifactRole::Unknown,
        ],
        RecoveryJobKind::ScaffoldMaterialization => vec![
            ArtifactRole::Entrypoint,
            ArtifactRole::IntegrationTarget,
            ArtifactRole::Implementation,
            ArtifactRole::Test,
            ArtifactRole::Docs,
            ArtifactRole::Unknown,
        ],
        RecoveryJobKind::SourceImplementationRepair => vec![
            ArtifactRole::Entrypoint,
            ArtifactRole::IntegrationTarget,
            ArtifactRole::Implementation,
        ],
        RecoveryJobKind::DevServerSmoke => Vec::new(),
        RecoveryJobKind::TestArtifactCompletion | RecoveryJobKind::TestAlignmentRepair => {
            vec![ArtifactRole::Test]
        }
        RecoveryJobKind::DocumentationRepair => vec![ArtifactRole::Docs],
        RecoveryJobKind::EvidenceBindingRepair => vec![
            ArtifactRole::SetupManifest,
            ArtifactRole::SetupConfig,
            ArtifactRole::Entrypoint,
            ArtifactRole::IntegrationTarget,
            ArtifactRole::Implementation,
            ArtifactRole::Test,
            ArtifactRole::Docs,
            ArtifactRole::Unknown,
        ],
        RecoveryJobKind::VerifierContractCorrection => vec![ArtifactRole::Unknown],
        RecoveryJobKind::ToolProtocolCorrection
        | RecoveryJobKind::ContractConflict
        | RecoveryJobKind::ExplicitStop => Vec::new(),
    }
}

fn job_allows_file_target(job: RecoveryJobKind) -> bool {
    !matches!(
        job,
        RecoveryJobKind::SetupBootstrap
            | RecoveryJobKind::DevServerSmoke
            | RecoveryJobKind::ToolProtocolCorrection
            | RecoveryJobKind::ContractConflict
            | RecoveryJobKind::ExplicitStop
    )
}

fn exhausted_targets_from_evidence(evidence: &ContractEvidence) -> Vec<String> {
    evidence
        .repair_job_state
        .iter()
        .filter_map(|line| line.strip_prefix("exhausted_targets="))
        .flat_map(|targets| targets.split('|').map(str::to_string).collect::<Vec<_>>())
        .collect()
}

fn exhausted_roles_from_evidence(evidence: &ContractEvidence) -> Vec<ArtifactRole> {
    evidence
        .repair_job_state
        .iter()
        .filter_map(|line| line.strip_prefix("exhausted_roles="))
        .flat_map(|roles| {
            roles
                .split('|')
                .filter_map(parse_artifact_role)
                .collect::<Vec<_>>()
        })
        .collect()
}

fn exhausted_clusters_from_evidence(evidence: &ContractEvidence) -> Vec<String> {
    let mut clusters = Vec::new();
    clusters.extend(evidence.exhausted_clusters.clone());
    clusters.extend(
        evidence
            .repair_job_state
            .iter()
            .filter_map(|line| line.strip_prefix("exhausted_clusters="))
            .flat_map(|clusters| clusters.split('|').map(str::to_string).collect::<Vec<_>>()),
    );
    clusters
        .into_iter()
        .filter(|cluster| !cluster.trim().is_empty() && cluster != "none")
        .collect()
}

fn no_progress_strategy_from_evidence(evidence: &ContractEvidence) -> Option<String> {
    evidence.no_progress_strategy.clone().or_else(|| {
        evidence.repair_job_state.iter().find_map(|line| {
            line.strip_prefix("no_progress_strategy=")
                .map(str::to_string)
        })
    })
}

fn repair_state_status_from_evidence(evidence: &ContractEvidence) -> Option<String> {
    evidence.repair_state_status.clone().or_else(|| {
        evidence
            .repair_job_state
            .iter()
            .find_map(|line| {
                line.strip_prefix("repair_state_status=")
                    .map(str::to_string)
            })
            .or_else(|| (!evidence.repair_job_state.is_empty()).then(|| "attempted".to_string()))
    })
}

fn repair_state_eval_fields(
    evidence: &ContractEvidence,
    exhausted_clusters: &[String],
    no_progress_strategy: Option<&str>,
) -> Vec<String> {
    let attempt_count = evidence
        .repair_attempt_ledger
        .len()
        .max(evidence.attempt_outcomes.len());
    let mut fields = vec![
        format!("repair_attempt_count={attempt_count}"),
        format!(
            "exhausted_clusters={}",
            if exhausted_clusters.is_empty() {
                "none".to_string()
            } else {
                exhausted_clusters.join("|")
            }
        ),
        format!(
            "no_progress_strategy={}",
            no_progress_strategy.unwrap_or("none")
        ),
    ];
    for key in [
        "attempt_outcome",
        "attempt_outcome_reason",
        "before_signature",
        "after_signature",
        "repair_state_status",
    ] {
        if let Some(value) = latest_eval_field_value(evidence, key) {
            fields.push(format!("{key}={value}"));
        }
    }
    fields
}

fn latest_eval_field_value(evidence: &ContractEvidence, key: &str) -> Option<String> {
    let marker = format!("{key}=");
    evidence
        .eval_report_fields
        .iter()
        .rev()
        .chain(evidence.attempt_outcomes.iter().rev())
        .find_map(|line| extract_eval_field(line, &marker))
}

fn extract_eval_field(line: &str, marker: &str) -> Option<String> {
    let (_, rest) = line.split_once(marker)?;
    let value = rest
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim_matches(|ch| matches!(ch, ',' | ';'))
        .to_string();
    (!value.trim().is_empty()).then_some(value)
}

fn parse_artifact_role(value: &str) -> Option<ArtifactRole> {
    match value {
        "setup_manifest" => Some(ArtifactRole::SetupManifest),
        "setup_config" => Some(ArtifactRole::SetupConfig),
        "entrypoint" => Some(ArtifactRole::Entrypoint),
        "integration_target" => Some(ArtifactRole::IntegrationTarget),
        "implementation" => Some(ArtifactRole::Implementation),
        "test" => Some(ArtifactRole::Test),
        "docs" => Some(ArtifactRole::Docs),
        "generated_output" => Some(ArtifactRole::GeneratedOutput),
        "dependency_cache" => Some(ArtifactRole::DependencyCache),
        "unknown" => Some(ArtifactRole::Unknown),
        _ => None,
    }
}

fn repair_brief_status(explicit_stop_reason: Option<&str>) -> RepairBriefStatus {
    if explicit_stop_reason.is_some() {
        RepairBriefStatus::ExplicitStop
    } else {
        RepairBriefStatus::Admitted
    }
}

fn action_envelope_status(explicit_stop_reason: Option<&str>) -> ActionEnvelopeStatus {
    if explicit_stop_reason.is_some() {
        ActionEnvelopeStatus::ExplicitStop
    } else {
        ActionEnvelopeStatus::Admitted
    }
}

fn repair_brief_must_preserve(
    evidence: &ContractEvidence,
    rerun_authority: &[String],
) -> Vec<String> {
    let mut values = Vec::new();
    if let Some(source) = evidence.source_of_truth.as_deref() {
        push_unique(&mut values, format!("source_of_truth={source}"));
    }
    for authority in rerun_authority {
        push_unique(&mut values, format!("rerun_authority={authority}"));
    }
    values
}

fn repair_brief_success_check(evidence: &ContractEvidence, rerun_authority: &[String]) -> String {
    if !rerun_authority.is_empty() {
        return rerun_authority.join(" plus ");
    }
    evidence
        .command
        .clone()
        .unwrap_or_else(|| "original guard or verifier".to_string())
}

fn select_target(
    evidence: &ContractEvidence,
    graph: &ArtifactGraph,
    job: RecoveryJobKind,
) -> Option<String> {
    let mut candidates = Vec::new();
    push_candidate(&mut candidates, evidence.repair_target.clone(), 0);
    push_candidate(&mut candidates, evidence.target_path.clone(), 1);
    for path in &evidence.candidate_artifacts {
        push_candidate(&mut candidates, Some(path.clone()), 2);
    }
    for path in &evidence.missing_paths {
        push_candidate(&mut candidates, Some(path.clone()), 3);
    }
    for path in &evidence.required_paths {
        push_candidate(&mut candidates, Some(path.clone()), 4);
    }
    if matches!(
        job,
        RecoveryJobKind::SetupBootstrap | RecoveryJobKind::ManifestRepair
    ) {
        push_candidate(&mut candidates, Some("package.json".to_string()), 0);
    }
    candidates.sort_by_key(|(_, priority)| *priority);
    candidates
        .into_iter()
        .map(|(path, _)| path)
        .find(|path| target_admitted(path, graph, job))
}

fn target_admitted(path: &str, graph: &ArtifactGraph, job: RecoveryJobKind) -> bool {
    if path.starts_with("step:") {
        return matches!(job, RecoveryJobKind::VerifierContractCorrection);
    }
    let role = graph.node(path).map(|node| node.role).unwrap_or_else(|| {
        role_for_path(
            path,
            crate::agent::step_runner::artifact_graph::ArtifactLifecycle::Required,
        )
    });
    if !recovery_target_admissible(role) {
        return false;
    }
    match job {
        RecoveryJobKind::ManifestRepair | RecoveryJobKind::SetupBootstrap => {
            matches!(
                role,
                ArtifactRole::SetupManifest | ArtifactRole::SetupConfig
            )
        }
        RecoveryJobKind::RouteIntegrationRepair => matches!(
            role,
            ArtifactRole::Entrypoint
                | ArtifactRole::IntegrationTarget
                | ArtifactRole::Implementation
                | ArtifactRole::Unknown
        ),
        RecoveryJobKind::DocumentationRepair => role == ArtifactRole::Docs,
        RecoveryJobKind::TestArtifactCompletion => role == ArtifactRole::Test,
        RecoveryJobKind::ScaffoldMaterialization => matches!(
            role,
            ArtifactRole::Entrypoint
                | ArtifactRole::IntegrationTarget
                | ArtifactRole::Implementation
                | ArtifactRole::Test
                | ArtifactRole::Docs
        ),
        RecoveryJobKind::SourceImplementationRepair => matches!(
            role,
            ArtifactRole::Entrypoint
                | ArtifactRole::IntegrationTarget
                | ArtifactRole::Implementation
        ),
        RecoveryJobKind::TestAlignmentRepair => role == ArtifactRole::Test,
        RecoveryJobKind::EvidenceBindingRepair => matches!(
            role,
            ArtifactRole::SetupManifest
                | ArtifactRole::SetupConfig
                | ArtifactRole::Entrypoint
                | ArtifactRole::IntegrationTarget
                | ArtifactRole::Implementation
                | ArtifactRole::Test
                | ArtifactRole::Docs
                | ArtifactRole::Unknown
        ),
        RecoveryJobKind::VerifierContractCorrection => path.starts_with("step:"),
        RecoveryJobKind::ContractConflict => false,
        RecoveryJobKind::ExplicitStop => false,
        _ => true,
    }
}

fn target_role_for(graph: &ArtifactGraph, path: &str) -> ArtifactRole {
    graph.node(path).map(|node| node.role).unwrap_or_else(|| {
        role_for_path(
            path,
            crate::agent::step_runner::artifact_graph::ArtifactLifecycle::Required,
        )
    })
}

fn target_admission(
    target: &Option<String>,
    graph: &ArtifactGraph,
    job: RecoveryJobKind,
) -> String {
    match target {
        Some(target) if target_admitted(target, graph, job) => {
            format!("admitted: target {target} matches {}", job.as_str())
        }
        Some(target) => format!(
            "rejected: target {target} is not admitted for {}",
            job.as_str()
        ),
        None => "none: no deterministic target admitted".to_string(),
    }
}

fn target_priority(
    target: &Option<String>,
    evidence: &ContractEvidence,
    job: RecoveryJobKind,
) -> String {
    let Some(target) = target else {
        return "none: no deterministic priority winner".to_string();
    };
    if evidence.repair_target.as_deref() == Some(target) {
        "priority=0 repair_target from deterministic evidence".to_string()
    } else if evidence.target_path.as_deref() == Some(target) {
        "priority=1 target_path from deterministic evidence".to_string()
    } else if matches!(
        job,
        RecoveryJobKind::SetupBootstrap | RecoveryJobKind::ManifestRepair
    ) && target == "package.json"
    {
        "priority=0 setup manifest owner".to_string()
    } else {
        format!("priority=2 first admitted candidate for {}", job.as_str())
    }
}

fn tool_policy_for(job: RecoveryJobKind, action: RecoveryActionKind) -> ToolPolicyProjection {
    match (job, action) {
        (RecoveryJobKind::SetupBootstrap, _) => ToolPolicyProjection::VerifierOwnedSetupOnly,
        (RecoveryJobKind::DevServerSmoke, _) => ToolPolicyProjection::VerifierOwnedSetupOnly,
        (RecoveryJobKind::ManifestRepair, _) => ToolPolicyProjection::SetupConfigMutationOnly,
        (RecoveryJobKind::ToolProtocolCorrection, _) => {
            ToolPolicyProjection::ToolProtocolCorrection
        }
        (RecoveryJobKind::VerifierContractCorrection, _) => ToolPolicyProjection::ReadOnly,
        (RecoveryJobKind::ContractConflict, _) => ToolPolicyProjection::ExplicitStop,
        (RecoveryJobKind::ExplicitStop, _) => ToolPolicyProjection::ExplicitStop,
        _ => ToolPolicyProjection::FileMutationRepair,
    }
}

fn required_action(
    evidence: &ContractEvidence,
    job: RecoveryJobKind,
    action: RecoveryActionKind,
) -> String {
    if let Some(action) = &evidence.required_action {
        return action.clone();
    }
    match (job, action) {
        (RecoveryJobKind::SetupBootstrap, _) => {
            "use verifier-owned setup recovery when allowed; do not run dependency installation from a model repair turn".to_string()
        }
        (RecoveryJobKind::DevServerSmoke, _) => {
            "run the bounded dev-server smoke contract; do not repair source or setup from this job".to_string()
        }
        (RecoveryJobKind::ManifestRepair, RecoveryActionKind::ResolveManifestConflict) => {
            "edit the setup manifest to resolve the deterministic dependency conflict before rerunning setup/build".to_string()
        }
        (RecoveryJobKind::ManifestRepair, _) => {
            "edit the setup manifest to satisfy the missing dependency or script contract".to_string()
        }
        (RecoveryJobKind::RouteIntegrationRepair, _) => {
            "connect the existing artifact to the selected entrypoint or route graph without creating unrelated placeholders".to_string()
        }
        (RecoveryJobKind::ScaffoldMaterialization, _) => {
            "create the missing required artifact before attempting integration repair".to_string()
        }
        (RecoveryJobKind::TestArtifactCompletion, _) => {
            "create the missing required test artifact before attempting source or verifier repair".to_string()
        }
        (RecoveryJobKind::TestAlignmentRepair, _) => {
            "align the test contract and verifier target without weakening the verifier".to_string()
        }
        (RecoveryJobKind::VerifierContractCorrection, _) => {
            "replace or replan the invalid verifier command; do not edit source to satisfy a rejected verifier contract".to_string()
        }
        (RecoveryJobKind::DocumentationRepair, _) => {
            "update the documentation literal or observed/expected mismatch exactly".to_string()
        }
        (RecoveryJobKind::EvidenceBindingRepair, _) => {
            "bind the required evidence to the target artifact without changing unrelated artifacts".to_string()
        }
        (RecoveryJobKind::ToolProtocolCorrection, _) => {
            "produce exactly one valid tool call that satisfies the shared tool protocol".to_string()
        }
        (RecoveryJobKind::ContractConflict, _) => {
            "stop with structured evidence because deterministic recovery candidates conflict".to_string()
        }
        (RecoveryJobKind::ExplicitStop, _) => {
            "stop with structured evidence because no deterministic recovery action is admitted".to_string()
        }
        _ => "fix the classified deterministic blocker before adding feature work".to_string(),
    }
}

fn disallowed_actions(
    evidence: &ContractEvidence,
    job: RecoveryJobKind,
    action: RecoveryActionKind,
) -> Vec<String> {
    let mut actions = evidence.disallowed_actions.clone();
    match job {
        RecoveryJobKind::SetupBootstrap => {
            push_unique(
                &mut actions,
                "Do not edit source files while resolving dependency setup.".to_string(),
            );
            push_unique(
                &mut actions,
                "Do not run dependency installation from a model tool call; use verifier-owned setup recovery only.".to_string(),
            );
        }
        RecoveryJobKind::DevServerSmoke => {
            push_unique(
                &mut actions,
                "Do not edit files from the dev-server smoke job; classify the launchability result first.".to_string(),
            );
            push_unique(
                &mut actions,
                "Do not treat npm run build success as proof that the requested port is launchable.".to_string(),
            );
        }
        RecoveryJobKind::ManifestRepair => {
            push_unique(
                &mut actions,
                "Do not edit route or implementation files while repairing manifest setup."
                    .to_string(),
            );
            push_unique(
                &mut actions,
                "Do not run npm install, npm ci, pnpm install, or yarn install from this repair task.".to_string(),
            );
        }
        RecoveryJobKind::RouteIntegrationRepair => {
            push_unique(
                &mut actions,
                "Do not create unrelated placeholder artifacts for route integration.".to_string(),
            );
            push_unique(
                &mut actions,
                "Do not satisfy route integration by editing package.json or dependency setup."
                    .to_string(),
            );
        }
        RecoveryJobKind::TestArtifactCompletion => {
            push_unique(
                &mut actions,
                "Do not edit implementation source until the required test artifact exists."
                    .to_string(),
            );
            push_unique(
                &mut actions,
                "Do not weaken verifier expectations while creating the test artifact.".to_string(),
            );
        }
        RecoveryJobKind::SourceImplementationRepair => {
            push_unique(
                &mut actions,
                "Do not change manifest or setup files unless the failure is reclassified as setup.".to_string(),
            );
            push_unique(
                &mut actions,
                "Do not change verifier commands to fake success.".to_string(),
            );
        }
        RecoveryJobKind::ToolProtocolCorrection => {
            push_unique(
                &mut actions,
                "Do not answer in prose instead of a tool call.".to_string(),
            );
            push_unique(
                &mut actions,
                "Do not run dependency installation.".to_string(),
            );
        }
        RecoveryJobKind::VerifierContractCorrection => {
            push_unique(
                &mut actions,
                "Do not edit implementation source to hide an invalid verifier command."
                    .to_string(),
            );
            push_unique(
                &mut actions,
                "Do not run dependency installation from verifier contract correction.".to_string(),
            );
        }
        RecoveryJobKind::EvidenceBindingRepair => {
            push_unique(
                &mut actions,
                "Do not create a new feature artifact when the blocker is only evidence binding."
                    .to_string(),
            );
            push_unique(
                &mut actions,
                "Do not edit dependency setup unless the evidence target is a setup artifact."
                    .to_string(),
            );
        }
        RecoveryJobKind::ExplicitStop => {
            push_unique(
                &mut actions,
                "Do not continue hidden repair without an admitted deterministic target."
                    .to_string(),
            );
        }
        RecoveryJobKind::ContractConflict => {
            push_unique(
                &mut actions,
                "Do not choose one conflicting recovery path without a deterministic owner."
                    .to_string(),
            );
            push_unique(
                &mut actions,
                "Do not mutate files until the active job conflict is resolved.".to_string(),
            );
        }
        _ => {}
    }
    if action == RecoveryActionKind::AlignTestAndVerifier {
        push_unique(
            &mut actions,
            "Do not weaken the verifier; align the target contract honestly.".to_string(),
        );
    }
    actions
}

fn rerun_authority(evidence: &ContractEvidence, job: RecoveryJobKind) -> Vec<String> {
    if !evidence.rerun_authority.is_empty() {
        return evidence.rerun_authority.clone();
    }
    match job {
        RecoveryJobKind::SetupBootstrap => vec![
            "dependency setup".to_string(),
            "original verifier".to_string(),
        ],
        RecoveryJobKind::DevServerSmoke => vec![
            "dev-server port preflight".to_string(),
            "localhost endpoint smoke".to_string(),
        ],
        RecoveryJobKind::ManifestRepair => vec![
            "profile verification".to_string(),
            "original verifier".to_string(),
        ],
        RecoveryJobKind::ToolProtocolCorrection => vec!["tool schema validation".to_string()],
        RecoveryJobKind::ContractConflict => Vec::new(),
        RecoveryJobKind::ExplicitStop => Vec::new(),
        _ => evidence
            .command
            .clone()
            .map(|command| vec![command])
            .unwrap_or_else(|| vec!["original verifier/guard".to_string()]),
    }
}

fn parse_job(value: &str) -> Option<RecoveryJobKind> {
    match value {
        "setup_bootstrap" => Some(RecoveryJobKind::SetupBootstrap),
        "manifest_repair" => Some(RecoveryJobKind::ManifestRepair),
        "integration_artifact_creation" | "scaffold_materialization" => {
            Some(RecoveryJobKind::ScaffoldMaterialization)
        }
        "route_integration_repair" => Some(RecoveryJobKind::RouteIntegrationRepair),
        "source_implementation_repair" => Some(RecoveryJobKind::SourceImplementationRepair),
        "dev_server_smoke" => Some(RecoveryJobKind::DevServerSmoke),
        "test_artifact_completion" => Some(RecoveryJobKind::TestArtifactCompletion),
        "test_repair" | "test_alignment_repair" => Some(RecoveryJobKind::TestAlignmentRepair),
        "docs_repair" | "documentation_repair" => Some(RecoveryJobKind::DocumentationRepair),
        "evidence_binding_repair" => Some(RecoveryJobKind::EvidenceBindingRepair),
        "verifier_policy_repair" | "verifier_contract_correction" => {
            Some(RecoveryJobKind::VerifierContractCorrection)
        }
        "tool_protocol_correction" => Some(RecoveryJobKind::ToolProtocolCorrection),
        "contract_conflict" => Some(RecoveryJobKind::ContractConflict),
        "explicit_stop" => Some(RecoveryJobKind::ExplicitStop),
        _ => None,
    }
}

fn parse_active_job_for_evidence(
    evidence: &ContractEvidence,
    value: &str,
) -> Option<RecoveryJobKind> {
    if value == "verifier_policy_repair" && policy_repair_targets_setup_artifact(evidence) {
        return Some(RecoveryJobKind::ManifestRepair);
    }
    parse_job(value)
}

fn policy_repair_targets_setup_artifact(evidence: &ContractEvidence) -> bool {
    if evidence.artifact_role.as_deref().is_some_and(|role| {
        matches!(
            role,
            "manifest" | "config" | "setup_manifest" | "setup_config"
        )
    }) {
        return true;
    }
    let target = evidence
        .repair_target
        .as_deref()
        .or(evidence.target_path.as_deref());
    target.is_some_and(|target| {
        matches!(
            role_for_path(
                target,
                crate::agent::step_runner::artifact_graph::ArtifactLifecycle::Required
            ),
            ArtifactRole::SetupManifest | ArtifactRole::SetupConfig
        )
    })
}

fn parse_action(value: &str) -> Option<RecoveryActionKind> {
    match value {
        "install_or_prepare_dependencies" => Some(RecoveryActionKind::InstallOrPrepareDependencies),
        "add_manifest_dependency" | "add_missing_manifest_dependency" => {
            Some(RecoveryActionKind::AddMissingManifestDependency)
        }
        "resolve_manifest_conflict" => Some(RecoveryActionKind::ResolveManifestConflict),
        "create_missing_integration_artifact" | "create_required_artifact" => {
            Some(RecoveryActionKind::CreateRequiredArtifact)
        }
        "connect_artifact_to_selected_route" | "connect_existing_artifact_to_entrypoint" => {
            Some(RecoveryActionKind::ConnectExistingArtifactToEntrypoint)
        }
        "repair_source_error" | "edit_source_for_diagnostic" => {
            Some(RecoveryActionKind::EditSourceForDiagnostic)
        }
        "run_dev_server_smoke" => Some(RecoveryActionKind::RunDevServerSmoke),
        "align_test_and_verifier" => Some(RecoveryActionKind::AlignTestAndVerifier),
        "update_docs_literal" => Some(RecoveryActionKind::UpdateDocsLiteral),
        "repair_evidence_binding" => Some(RecoveryActionKind::RepairEvidenceBinding),
        "replace_invalid_verifier_command" => {
            Some(RecoveryActionKind::ReplaceInvalidVerifierCommand)
        }
        "correct_tool_protocol" => Some(RecoveryActionKind::CorrectToolProtocol),
        "stop_with_setup_blocker" | "stop_no_admitted_target" | "stop_with_structured_evidence" => {
            Some(RecoveryActionKind::StopWithStructuredEvidence)
        }
        _ => None,
    }
}

fn primary_code(evidence: &ContractEvidence) -> Option<String> {
    evidence
        .diagnostic_code
        .clone()
        .or_else(|| evidence.reason_code.clone())
        .or_else(|| evidence.violated_contract.clone())
}

fn job_requires_target(job: RecoveryJobKind) -> bool {
    !matches!(
        job,
        RecoveryJobKind::SetupBootstrap
            | RecoveryJobKind::ToolProtocolCorrection
            | RecoveryJobKind::ContractConflict
            | RecoveryJobKind::ExplicitStop
    )
}

fn repair_action_status(
    target_admission: &str,
    explicit_stop_reason: Option<&str>,
) -> RepairActionStatus {
    if explicit_stop_reason.is_some() {
        RepairActionStatus::ExplicitStop
    } else if target_admission.starts_with("rejected") {
        RepairActionStatus::Rejected
    } else {
        RepairActionStatus::Admitted
    }
}

fn evidence_has_missing_role(evidence: &ContractEvidence, role: ArtifactRole) -> bool {
    evidence
        .missing_paths
        .iter()
        .chain(evidence.required_paths.iter())
        .chain(evidence.candidate_artifacts.iter())
        .any(|path| {
            role_for_path(
                path,
                crate::agent::step_runner::artifact_graph::ArtifactLifecycle::Required,
            ) == role
        })
}

fn push_candidate(candidates: &mut Vec<(String, u8)>, candidate: Option<String>, priority: u8) {
    let Some(candidate) = candidate else {
        return;
    };
    if candidate.trim().is_empty() {
        return;
    }
    if !candidates
        .iter()
        .any(|(existing, _)| existing == &candidate)
    {
        candidates.push((candidate, priority));
    }
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn merge_lists(left: &[String], right: &[String]) -> Vec<String> {
    let mut merged = Vec::new();
    for value in left.iter().chain(right.iter()) {
        push_unique(&mut merged, value.clone());
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_integration_selects_route_job_and_admitted_target() {
        let evidence = ContractEvidence::new("profile_verification")
            .with_reason_code("nextjs_route_not_integrated")
            .with_active_job("route_integration_repair")
            .with_candidate_artifacts(vec![
                "app/page.tsx",
                "components/Game.tsx",
                "components/GameBoard.tsx",
            ])
            .with_repair_target("components/GameBoard.tsx");

        let decision = orchestrate_contract_evidence(&evidence).unwrap();

        assert_eq!(decision.job, RecoveryJobKind::RouteIntegrationRepair);
        assert_eq!(
            decision.action,
            RecoveryActionKind::ConnectExistingArtifactToEntrypoint
        );
        assert_eq!(decision.target.as_deref(), Some("components/GameBoard.tsx"));
        assert!(decision.target_admission.starts_with("admitted"));
        assert_eq!(decision.source_of_truth, "profile_contract");
        assert_eq!(
            decision.allowed_change_kind,
            "route_or_integration_target_only"
        );
        assert_eq!(
            decision.tool_policy,
            ToolPolicyProjection::FileMutationRepair
        );
    }

    #[test]
    fn dependency_missing_uses_setup_job_without_model_install_policy() {
        let evidence = ContractEvidence::new("verifier")
            .with_reason_code("dependency_missing")
            .with_command("npm run build");

        let decision = orchestrate_contract_evidence(&evidence).unwrap();

        assert_eq!(decision.job, RecoveryJobKind::SetupBootstrap);
        assert_eq!(
            decision.action,
            RecoveryActionKind::InstallOrPrepareDependencies
        );
        assert_eq!(
            decision.tool_policy,
            ToolPolicyProjection::VerifierOwnedSetupOnly
        );
        assert!(
            decision
                .disallowed_actions
                .iter()
                .any(|action| action.contains("model tool call"))
        );
    }

    #[test]
    fn manifest_conflict_targets_package_json() {
        let evidence = ContractEvidence::new("profile_verification")
            .with_reason_code("nextjs_dependency_version_conflict")
            .with_active_job("manifest_repair");

        let decision = orchestrate_contract_evidence(&evidence).unwrap();

        assert_eq!(decision.job, RecoveryJobKind::ManifestRepair);
        assert_eq!(decision.target.as_deref(), Some("package.json"));
        assert_eq!(
            decision.tool_policy,
            ToolPolicyProjection::SetupConfigMutationOnly
        );
    }

    #[test]
    fn generated_output_candidate_is_rejected() {
        let evidence = ContractEvidence::new("verifier")
            .with_reason_code("command_failed:1")
            .with_candidate_artifacts(vec!["node_modules/react/index.js"]);

        let decision = orchestrate_contract_evidence(&evidence).unwrap();

        assert_eq!(decision.job, RecoveryJobKind::SourceImplementationRepair);
        assert_eq!(decision.target, None);
        assert_eq!(
            decision.explicit_stop_reason.as_deref(),
            Some("no_admitted_recovery_target")
        );
    }

    #[test]
    fn source_implementation_repair_rejects_test_target_and_chooses_source() {
        let evidence = ContractEvidence::new("verifier")
            .with_reason_code("command_failed:1")
            .with_active_job("source_implementation_repair")
            .with_repair_target("tests/test_app.py")
            .with_candidate_artifacts(vec!["tests/test_app.py", "app/main.py"]);

        let decision = orchestrate_contract_evidence(&evidence).unwrap();

        assert_eq!(decision.job, RecoveryJobKind::SourceImplementationRepair);
        assert_eq!(decision.target.as_deref(), Some("app/main.py"));
        assert_eq!(decision.artifact_role.as_deref(), Some("entrypoint"));
        assert_eq!(decision.allowed_change_kind, "entrypoint_source_only");
        assert!(decision.target_admission.starts_with("admitted"));
    }

    #[test]
    fn missing_test_artifact_selects_artifact_completion_job() {
        let evidence = ContractEvidence::new("profile_verification")
            .with_reason_code("missing_required_artifact")
            .with_missing_paths(vec!["tests/test_app.py"]);

        let decision = orchestrate_contract_evidence(&evidence).unwrap();

        assert_eq!(decision.job, RecoveryJobKind::TestArtifactCompletion);
        assert_eq!(decision.action, RecoveryActionKind::CreateRequiredArtifact);
        assert_eq!(decision.target.as_deref(), Some("tests/test_app.py"));
        assert!(
            decision
                .disallowed_actions
                .iter()
                .any(|action| action.contains("implementation source"))
        );
    }

    #[test]
    fn verifier_contract_correction_is_read_only_policy() {
        let evidence = ContractEvidence::new("verifier")
            .with_reason_code("blocked:Unknown: compound command")
            .with_active_job("verifier_contract_correction")
            .with_repair_action("replace_invalid_verifier_command")
            .with_repair_target("step:verify-build");

        let decision = orchestrate_contract_evidence(&evidence).unwrap();

        assert_eq!(decision.job, RecoveryJobKind::VerifierContractCorrection);
        assert_eq!(
            decision.action,
            RecoveryActionKind::ReplaceInvalidVerifierCommand
        );
        assert_eq!(decision.target.as_deref(), Some("step:verify-build"));
        assert_eq!(decision.tool_policy, ToolPolicyProjection::ReadOnly);
    }

    #[test]
    fn tool_protocol_candidate_wins_over_source_fallback() {
        let evidence = ContractEvidence::new("profile_verification")
            .with_reason_code("tool_args_missing_required_field")
            .with_tool("Write")
            .with_target_field("path");

        let decision = orchestrate_contract_evidence(&evidence).unwrap();

        assert_eq!(decision.job, RecoveryJobKind::ToolProtocolCorrection);
        assert_eq!(decision.action, RecoveryActionKind::CorrectToolProtocol);
        assert_eq!(
            decision.loop_control_action,
            LoopControlAction::RunToolProtocolCorrection
        );
        assert_eq!(decision.dispatch_status, DispatchStatus::Selected);
        assert!(
            decision
                .candidate_jobs
                .iter()
                .any(|candidate| candidate.contains("source_implementation_repair"))
        );
    }

    #[test]
    fn read_only_step_mutation_dispatches_explicit_stop() {
        let evidence = ContractEvidence::new("step_policy")
            .with_reason_code("read_only_step_mutation")
            .with_failed_step("inspect-files");

        let decision = orchestrate_contract_evidence(&evidence).unwrap();

        assert_eq!(decision.job, RecoveryJobKind::ExplicitStop);
        assert_eq!(
            decision.loop_control_action,
            LoopControlAction::RenderExplicitStop
        );
        assert_eq!(decision.dispatch_status, DispatchStatus::ExplicitStop);
        assert_eq!(
            decision.explicit_stop_reason.as_deref(),
            Some("explicit_stop_from_deterministic_contract")
        );
    }

    #[test]
    fn ambiguous_top_priority_dispatches_contract_conflict_stop() {
        let candidates = vec![
            ActiveJobCandidate {
                job: RecoveryJobKind::ManifestRepair,
                action: RecoveryActionKind::AddMissingManifestDependency,
                priority: 20,
                reason: "manifest".to_string(),
                source_of_truth: "test".to_string(),
                target_hint: Some("package.json".to_string()),
                artifact_role: Some("setup_manifest".to_string()),
                rerun_authority: vec!["profile_verification".to_string()],
            },
            ActiveJobCandidate {
                job: RecoveryJobKind::RouteIntegrationRepair,
                action: RecoveryActionKind::ConnectExistingArtifactToEntrypoint,
                priority: 20,
                reason: "route".to_string(),
                source_of_truth: "test".to_string(),
                target_hint: Some("app/page.tsx".to_string()),
                artifact_role: Some("entrypoint".to_string()),
                rerun_authority: vec!["profile_verification".to_string()],
            },
        ];

        let arbitration = arbitrate_active_job(candidates);

        assert_eq!(arbitration.selected_job, RecoveryJobKind::ContractConflict);
        assert_eq!(arbitration.dispatch_status, DispatchStatus::AmbiguousTie);
        assert_eq!(
            arbitration.loop_control_action,
            LoopControlAction::RenderExplicitStop
        );
        assert_eq!(
            arbitration.tie_break_reason.as_deref(),
            Some("active_job_tie")
        );
    }

    #[test]
    fn dispatch_fields_are_applied_to_contract_evidence() {
        let evidence = ContractEvidence::new("tool_protocol")
            .with_reason_code("tool_args_invalid_json")
            .with_tool("Write");

        let enriched = orchestrate_evidence(evidence);

        assert_eq!(
            enriched.loop_control_action.as_deref(),
            Some("run_tool_protocol_correction")
        );
        assert_eq!(enriched.dispatch_status.as_deref(), Some("selected"));
        assert_eq!(
            enriched.dispatch_reason.as_deref(),
            Some("tool_or_provider_protocol_failure")
        );
        assert!(
            enriched
                .candidate_jobs
                .iter()
                .any(|candidate| candidate.contains("tool_protocol_correction"))
        );
    }
}
