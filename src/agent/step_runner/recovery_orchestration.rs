#![allow(dead_code)]

use crate::agent::step_runner::artifact_completion::ArtifactCompletionJob;
use crate::agent::step_runner::artifact_graph::{
    ArtifactGraph, ArtifactRole, recovery_target_admissible, role_for_path,
};
use crate::agent::step_runner::correction_evidence::ContractEvidence;
use crate::agent::step_runner::recovery_contract;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecoveryJobKind {
    SetupBootstrap,
    ManifestRepair,
    ScaffoldMaterialization,
    RouteIntegrationRepair,
    SourceImplementationRepair,
    TestArtifactCompletion,
    TestAlignmentRepair,
    DocumentationRepair,
    VerifierContractCorrection,
    ToolProtocolCorrection,
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
            Self::TestArtifactCompletion => "test_artifact_completion",
            Self::TestAlignmentRepair => "test_alignment_repair",
            Self::DocumentationRepair => "documentation_repair",
            Self::VerifierContractCorrection => "verifier_contract_correction",
            Self::ToolProtocolCorrection => "tool_protocol_correction",
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
    AlignTestAndVerifier,
    UpdateDocsLiteral,
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
            Self::AlignTestAndVerifier => "align_test_and_verifier",
            Self::UpdateDocsLiteral => "update_docs_literal",
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
    pub(crate) tool_policy: ToolPolicyProjection,
    pub(crate) rerun_authority: Vec<String>,
    pub(crate) explicit_stop_reason: Option<String>,
    pub(crate) artifact_graph_summary: Vec<String>,
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
            .with_artifact_graph_summary(self.artifact_graph_summary.clone());

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
        if let Some(reason) = &self.explicit_stop_reason {
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
    let job = classify_job(evidence);
    if job == RecoveryJobKind::ExplicitStop && evidence.guard.trim().is_empty() {
        return None;
    }
    let action = classify_action(evidence, job);
    let target = select_target(evidence, &graph, job);
    let role = target
        .as_deref()
        .map(|target| {
            role_for_path(
                target,
                crate::agent::step_runner::artifact_graph::ArtifactLifecycle::Required,
            )
        })
        .map(|role| role.as_str().to_string());
    let target_role = target
        .as_deref()
        .map(|target| target_role_for(&graph, target));
    let target_admission = target_admission(&target, &graph, job);
    let explicit_stop_reason = if target.is_none() && job_requires_target(job) {
        Some("no_admitted_recovery_target".to_string())
    } else if job == RecoveryJobKind::ExplicitStop {
        Some("explicit_stop_from_deterministic_contract".to_string())
    } else {
        None
    };
    let target_priority = target_priority(&target, evidence, job);
    let tool_policy = tool_policy_for(job, action);
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
    let active_job_priority = recovery_contract::active_job_priority(job.as_str());
    Some(RecoveryOrchestrationDecision {
        job,
        action,
        target,
        artifact_role: role,
        target_admission,
        target_priority,
        required_action: required_action(evidence, job, action),
        disallowed_actions: disallowed_actions(evidence, job, action),
        semantic_failure_kind,
        source_of_truth,
        allowed_change_kind,
        expected_evidence_delta,
        workspace_scope,
        artifact_ownership,
        active_job_priority,
        tool_policy,
        rerun_authority: rerun_authority(evidence, job),
        explicit_stop_reason,
        artifact_graph_summary: graph.summary(),
    })
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
        Some(code) if code.contains("dependency") || code.contains("manifest") => {
            RecoveryJobKind::ManifestRepair
        }
        Some("nextjs_route_not_integrated") => RecoveryJobKind::RouteIntegrationRepair,
        Some("nextjs_integration_artifact_missing") => RecoveryJobKind::ScaffoldMaterialization,
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
        RecoveryJobKind::VerifierContractCorrection => {
            RecoveryActionKind::ReplaceInvalidVerifierCommand
        }
        RecoveryJobKind::ToolProtocolCorrection => RecoveryActionKind::CorrectToolProtocol,
        RecoveryJobKind::ExplicitStop => RecoveryActionKind::StopWithStructuredEvidence,
    }
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
        RecoveryJobKind::VerifierContractCorrection => path.starts_with("step:"),
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
        (RecoveryJobKind::ManifestRepair, _) => ToolPolicyProjection::SetupConfigMutationOnly,
        (RecoveryJobKind::ToolProtocolCorrection, _) => {
            ToolPolicyProjection::ToolProtocolCorrection
        }
        (RecoveryJobKind::VerifierContractCorrection, _) => ToolPolicyProjection::ReadOnly,
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
        (RecoveryJobKind::ToolProtocolCorrection, _) => {
            "produce exactly one valid tool call that satisfies the shared tool protocol".to_string()
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
        RecoveryJobKind::ExplicitStop => {
            push_unique(
                &mut actions,
                "Do not continue hidden repair without an admitted deterministic target."
                    .to_string(),
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
        RecoveryJobKind::ManifestRepair => vec![
            "profile verification".to_string(),
            "original verifier".to_string(),
        ],
        RecoveryJobKind::ToolProtocolCorrection => vec!["tool schema validation".to_string()],
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
        "test_artifact_completion" => Some(RecoveryJobKind::TestArtifactCompletion),
        "test_repair" | "test_alignment_repair" => Some(RecoveryJobKind::TestAlignmentRepair),
        "docs_repair" | "documentation_repair" => Some(RecoveryJobKind::DocumentationRepair),
        "verifier_policy_repair" | "verifier_contract_correction" => {
            Some(RecoveryJobKind::VerifierContractCorrection)
        }
        "tool_protocol_correction" => Some(RecoveryJobKind::ToolProtocolCorrection),
        "explicit_stop" => Some(RecoveryJobKind::ExplicitStop),
        _ => None,
    }
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
        "align_test_and_verifier" => Some(RecoveryActionKind::AlignTestAndVerifier),
        "update_docs_literal" => Some(RecoveryActionKind::UpdateDocsLiteral),
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
            | RecoveryJobKind::ExplicitStop
    )
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
}
