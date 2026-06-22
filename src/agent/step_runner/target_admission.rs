#![allow(dead_code)]

use crate::agent::step_runner::artifact_graph::{
    ArtifactGraph, ArtifactRole, recovery_target_admissible, role_for_path,
};
use crate::agent::step_runner::artifact_ownership::{
    ArtifactOwnership, classify_artifact_ownership,
};
use crate::agent::step_runner::workspace_scope::WorkspaceScope;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RepairTargetSource {
    FailureEvidence,
    ArtifactGraphRelation,
    ProfileSelectedRoute,
    VerifierDiagnostic,
    RequiredArtifact,
    SetupManifest,
    ToolReadRecord,
    ToolWriteRecord,
    ToolEditRecord,
    ScaffoldDelta,
    SetupDelta,
    CompletionEvidence,
    EvidenceBinding,
    WorkspaceObservation,
}

impl RepairTargetSource {
    pub(crate) fn priority(self) -> u8 {
        match self {
            Self::FailureEvidence => 0,
            Self::VerifierDiagnostic => 1,
            Self::SetupManifest => 1,
            Self::ToolWriteRecord => 1,
            Self::ToolEditRecord => 1,
            Self::ProfileSelectedRoute => 2,
            Self::ToolReadRecord => 2,
            Self::CompletionEvidence => 2,
            Self::EvidenceBinding => 2,
            Self::RequiredArtifact => 3,
            Self::ScaffoldDelta => 3,
            Self::SetupDelta => 3,
            Self::WorkspaceObservation => 4,
            Self::ArtifactGraphRelation => 5,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::FailureEvidence => "failure_evidence",
            Self::ArtifactGraphRelation => "artifact_graph_relation",
            Self::ProfileSelectedRoute => "profile_selected_route",
            Self::VerifierDiagnostic => "verifier_diagnostic",
            Self::RequiredArtifact => "required_artifact",
            Self::SetupManifest => "setup_manifest",
            Self::ToolReadRecord => "tool_read_record",
            Self::ToolWriteRecord => "tool_write_record",
            Self::ToolEditRecord => "tool_edit_record",
            Self::ScaffoldDelta => "scaffold_delta",
            Self::SetupDelta => "setup_delta",
            Self::CompletionEvidence => "completion_evidence",
            Self::EvidenceBinding => "evidence_binding",
            Self::WorkspaceObservation => "workspace_observation",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TargetEvidenceFreshness {
    Current,
    Unknown,
    Stale,
}

impl TargetEvidenceFreshness {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Unknown => "unknown",
            Self::Stale => "stale",
        }
    }

    fn priority_penalty(self) -> u8 {
        match self {
            Self::Current => 0,
            Self::Unknown => 2,
            Self::Stale => 12,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FocusedEditStatus {
    NotRequired,
    Eligible,
    MissingCurrentExcerpt,
    StaleTarget,
    TargetNotOwned,
}

impl FocusedEditStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::NotRequired => "not_required",
            Self::Eligible => "eligible",
            Self::MissingCurrentExcerpt => "missing_current_excerpt",
            Self::StaleTarget => "stale_target",
            Self::TargetNotOwned => "target_not_owned",
        }
    }

    fn priority_penalty(self) -> u8 {
        match self {
            Self::Eligible => 0,
            Self::NotRequired => 2,
            Self::MissingCurrentExcerpt | Self::StaleTarget | Self::TargetNotOwned => 20,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepairTargetCandidate {
    pub(crate) path: String,
    pub(crate) role: ArtifactRole,
    pub(crate) source: RepairTargetSource,
    pub(crate) source_of_truth: Option<String>,
    pub(crate) evidence_freshness: TargetEvidenceFreshness,
    pub(crate) focused_edit: FocusedEditStatus,
    pub(crate) current_excerpt_available: bool,
}

impl RepairTargetCandidate {
    pub(crate) fn new(
        path: impl Into<String>,
        role: ArtifactRole,
        source: RepairTargetSource,
    ) -> Self {
        Self {
            path: path.into(),
            role,
            source,
            source_of_truth: None,
            evidence_freshness: TargetEvidenceFreshness::Unknown,
            focused_edit: FocusedEditStatus::NotRequired,
            current_excerpt_available: false,
        }
    }

    pub(crate) fn with_source_of_truth(mut self, source_of_truth: impl Into<String>) -> Self {
        self.source_of_truth = Some(source_of_truth.into());
        self
    }

    pub(crate) fn with_evidence_freshness(
        mut self,
        evidence_freshness: TargetEvidenceFreshness,
    ) -> Self {
        self.evidence_freshness = evidence_freshness;
        self
    }

    pub(crate) fn with_focused_edit(mut self, focused_edit: FocusedEditStatus) -> Self {
        self.focused_edit = focused_edit;
        self
    }

    pub(crate) fn with_current_excerpt_available(
        mut self,
        current_excerpt_available: bool,
    ) -> Self {
        self.current_excerpt_available = current_excerpt_available;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TargetAdmissionPolicy {
    pub(crate) active_job: String,
    pub(crate) repair_action: String,
    pub(crate) allowed_roles: Vec<ArtifactRole>,
    pub(crate) requires_target: bool,
    pub(crate) allow_file_target: bool,
    pub(crate) exhausted_targets: Vec<String>,
    pub(crate) exhausted_roles: Vec<ArtifactRole>,
    pub(crate) exhausted_clusters: Vec<String>,
    pub(crate) current_cluster: Option<String>,
}

impl TargetAdmissionPolicy {
    pub(crate) fn new(
        active_job: impl Into<String>,
        repair_action: impl Into<String>,
        allowed_roles: Vec<ArtifactRole>,
        requires_target: bool,
        allow_file_target: bool,
    ) -> Self {
        Self {
            active_job: active_job.into(),
            repair_action: repair_action.into(),
            allowed_roles,
            requires_target,
            allow_file_target,
            exhausted_targets: Vec::new(),
            exhausted_roles: Vec::new(),
            exhausted_clusters: Vec::new(),
            current_cluster: None,
        }
    }

    pub(crate) fn with_exhausted_targets<I, S>(mut self, exhausted_targets: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.exhausted_targets = exhausted_targets
            .into_iter()
            .map(|target| {
                let target = target.into();
                normalize_path(&target)
            })
            .collect();
        self
    }

    pub(crate) fn with_exhausted_roles(mut self, exhausted_roles: Vec<ArtifactRole>) -> Self {
        self.exhausted_roles = exhausted_roles;
        self
    }

    pub(crate) fn with_exhausted_clusters<I, S>(mut self, exhausted_clusters: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.exhausted_clusters = exhausted_clusters
            .into_iter()
            .map(|cluster| cluster.into())
            .filter(|cluster| !cluster.trim().is_empty())
            .collect();
        self
    }

    pub(crate) fn with_current_cluster(mut self, cluster: Option<String>) -> Self {
        self.current_cluster = cluster;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TargetAdmissionStatus {
    Proposed,
    Admitted,
    Rejected,
}

impl TargetAdmissionStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Admitted => "admitted",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TargetAdmissionRecord {
    pub(crate) status: TargetAdmissionStatus,
    pub(crate) path: String,
    pub(crate) role: ArtifactRole,
    pub(crate) priority: u8,
    pub(crate) source: RepairTargetSource,
    pub(crate) ownership: ArtifactOwnership,
    pub(crate) source_of_truth: String,
    pub(crate) ownership_source: String,
    pub(crate) workspace_scope: String,
    pub(crate) evidence_freshness: TargetEvidenceFreshness,
    pub(crate) focused_edit: FocusedEditStatus,
    pub(crate) current_excerpt_available: bool,
    pub(crate) priority_components: String,
    pub(crate) reason: String,
}

impl TargetAdmissionRecord {
    pub(crate) fn render_line(&self) -> String {
        format!(
            "status={} path={} role={} priority={} source={} source_of_truth={} ownership={} ownership_source={} workspace_scope={} freshness={} focused_edit={} current_excerpt={} priority_components={} reason={}",
            self.status.as_str(),
            self.path,
            self.role.as_str(),
            self.priority,
            self.source.as_str(),
            self.source_of_truth,
            self.ownership.as_str(),
            self.ownership_source,
            self.workspace_scope,
            self.evidence_freshness.as_str(),
            self.focused_edit.as_str(),
            self.current_excerpt_available,
            compact(&self.priority_components),
            compact(&self.reason)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct TargetAdmissionDecision {
    pub(crate) proposed_targets: Vec<TargetAdmissionRecord>,
    pub(crate) admitted_targets: Vec<TargetAdmissionRecord>,
    pub(crate) rejected_targets: Vec<TargetAdmissionRecord>,
    pub(crate) selected_target: Option<String>,
    pub(crate) selected_role: Option<ArtifactRole>,
    pub(crate) selected_priority: Option<u8>,
    pub(crate) explicit_stop_reason: Option<String>,
}

impl TargetAdmissionDecision {
    pub(crate) fn selected_record(&self) -> Option<&TargetAdmissionRecord> {
        self.admitted_targets
            .iter()
            .find(|record| Some(record.path.as_str()) == self.selected_target.as_deref())
    }

    pub(crate) fn target_admission_line(&self) -> String {
        if let Some(record) = self.selected_record() {
            return format!(
                "admitted: target {} role={} source={} reason={}",
                record.path,
                record.role.as_str(),
                record.source.as_str(),
                compact(&record.reason)
            );
        }
        if let Some(reason) = &self.explicit_stop_reason {
            return format!("rejected: {reason}");
        }
        "none: no deterministic target admitted".to_string()
    }

    pub(crate) fn target_priority_line(&self) -> String {
        if let Some(record) = self.selected_record() {
            return format!(
                "priority={} selected by {} for role {}",
                record.priority,
                record.source.as_str(),
                record.role.as_str()
            );
        }
        "none: no deterministic priority winner".to_string()
    }

    pub(crate) fn proposed_lines(&self) -> Vec<String> {
        self.proposed_targets
            .iter()
            .map(TargetAdmissionRecord::render_line)
            .collect()
    }

    pub(crate) fn admitted_lines(&self) -> Vec<String> {
        self.admitted_targets
            .iter()
            .map(TargetAdmissionRecord::render_line)
            .collect()
    }

    pub(crate) fn rejected_lines(&self) -> Vec<String> {
        self.rejected_targets
            .iter()
            .map(TargetAdmissionRecord::render_line)
            .collect()
    }

    pub(crate) fn eval_report_fields(&self) -> Vec<String> {
        let selected = self.selected_record();
        vec![
            format!("target_candidate_count={}", self.proposed_targets.len()),
            format!("target_admitted_count={}", self.admitted_targets.len()),
            format!("target_rejected_count={}", self.rejected_targets.len()),
            format!(
                "selected_target={}",
                self.selected_target.as_deref().unwrap_or("none")
            ),
            format!(
                "selected_target_role={}",
                self.selected_role
                    .map(ArtifactRole::as_str)
                    .unwrap_or("unknown")
            ),
            format!(
                "target_rejection_reasons={}",
                self.rejected_targets
                    .iter()
                    .map(|record| compact(&record.reason))
                    .collect::<Vec<_>>()
                    .join("|")
            ),
            format!(
                "target_source_of_truth={}",
                selected
                    .map(|record| record.source_of_truth.as_str())
                    .unwrap_or("none")
            ),
            format!(
                "target_ownership_source={}",
                selected
                    .map(|record| record.ownership_source.as_str())
                    .unwrap_or("none")
            ),
            format!(
                "target_workspace_scope={}",
                selected
                    .map(|record| record.workspace_scope.as_str())
                    .unwrap_or("none")
            ),
            format!(
                "target_evidence_freshness={}",
                selected
                    .map(|record| record.evidence_freshness.as_str())
                    .unwrap_or("none")
            ),
            format!(
                "focused_edit_status={}",
                selected
                    .map(|record| record.focused_edit.as_str())
                    .unwrap_or_else(|| {
                        self.rejected_targets
                            .first()
                            .map(|record| record.focused_edit.as_str())
                            .unwrap_or("none")
                    })
            ),
            format!(
                "current_excerpt_available={}",
                selected
                    .map(|record| record.current_excerpt_available.to_string())
                    .unwrap_or_else(|| "false".to_string())
            ),
            format!(
                "target_priority_components={}",
                selected
                    .map(|record| compact(&record.priority_components))
                    .unwrap_or_else(|| "none".to_string())
            ),
            format!(
                "target_conflict_reason={}",
                self.explicit_stop_reason.as_deref().unwrap_or("none")
            ),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TargetAdmission {
    Admitted {
        path: String,
        role: ArtifactRole,
        priority: u8,
        reason: String,
    },
    Rejected {
        path: String,
        role: ArtifactRole,
        reason: String,
    },
}

impl TargetAdmission {
    pub(crate) fn is_admitted(&self) -> bool {
        matches!(self, Self::Admitted { .. })
    }

    pub(crate) fn reason(&self) -> &str {
        match self {
            Self::Admitted { reason, .. } | Self::Rejected { reason, .. } => reason,
        }
    }
}

pub(crate) fn admit_repair_target(
    candidate: RepairTargetCandidate,
    graph: &ArtifactGraph,
    allowed_roles: &[ArtifactRole],
) -> TargetAdmission {
    let scope = WorkspaceScope::from_graph(graph);
    admit_repair_target_with_scope(candidate, graph, allowed_roles, &scope, &[])
}

pub(crate) fn admit_repair_target_with_scope(
    candidate: RepairTargetCandidate,
    graph: &ArtifactGraph,
    allowed_roles: &[ArtifactRole],
    scope: &WorkspaceScope,
    changed_paths: &[String],
) -> TargetAdmission {
    let role = graph
        .node(&candidate.path)
        .map(|node| node.role)
        .unwrap_or_else(|| {
            role_for_path(
                &candidate.path,
                crate::agent::step_runner::artifact_graph::ArtifactLifecycle::Required,
            )
        });
    if !recovery_target_admissible(role) {
        return TargetAdmission::Rejected {
            path: candidate.path,
            role,
            reason: "generated_or_dependency_cache_target".to_string(),
        };
    }
    let ownership = classify_artifact_ownership(
        graph,
        scope,
        &candidate.path,
        role,
        candidate.source.as_str(),
        changed_paths,
    );
    if ownership.ownership == ArtifactOwnership::OutOfScope {
        return TargetAdmission::Rejected {
            path: candidate.path,
            role,
            reason: format!("artifact_out_of_scope:{}", ownership.reason),
        };
    }
    if ownership.ownership == ArtifactOwnership::CandidateOnly {
        if candidate.focused_edit == FocusedEditStatus::Eligible
            && candidate.current_excerpt_available
            && ownership.ownership_subreason == "read_only_observation"
        {
            let priority = target_priority(&candidate, role, &ownership);
            let path = candidate.path;
            return TargetAdmission::Admitted {
                path,
                role,
                priority,
                reason: format!(
                    "source={} ownership={} focused_edit={} scope={}",
                    candidate.source.as_str(),
                    ownership.ownership.as_str(),
                    candidate.focused_edit.as_str(),
                    scope.summary()
                ),
            };
        }
        return TargetAdmission::Rejected {
            path: candidate.path,
            role,
            reason: format!("candidate_without_artifact_ownership:{}", ownership.reason),
        };
    }
    if !allowed_roles.is_empty() && !allowed_roles.contains(&role) {
        return TargetAdmission::Rejected {
            path: candidate.path,
            role,
            reason: format!("role_not_allowed_for_current_job:{}", role.as_str()),
        };
    }
    TargetAdmission::Admitted {
        path: candidate.path,
        role,
        priority: candidate.source.priority(),
        reason: format!(
            "source={} ownership={} scope={}",
            candidate.source.as_str(),
            ownership.ownership.as_str(),
            scope.summary()
        ),
    }
}

pub(crate) fn decide_repair_target_with_scope(
    candidates: impl IntoIterator<Item = RepairTargetCandidate>,
    graph: &ArtifactGraph,
    policy: &TargetAdmissionPolicy,
    scope: &WorkspaceScope,
    changed_paths: &[String],
) -> TargetAdmissionDecision {
    let mut decision = TargetAdmissionDecision::default();
    for candidate in candidates {
        if candidate.path.trim().is_empty()
            || decision
                .proposed_targets
                .iter()
                .any(|record| record.path == normalize_path(&candidate.path))
        {
            continue;
        }
        let record = proposed_record(&candidate, graph, scope, changed_paths);
        decision.proposed_targets.push(record.clone());

        let rejection_reason = policy_rejection_reason(&record, policy).or_else(|| {
            admission_rejection_reason(
                candidate.clone(),
                graph,
                &policy.allowed_roles,
                scope,
                changed_paths,
            )
        });
        if let Some(reason) = rejection_reason {
            decision.rejected_targets.push(TargetAdmissionRecord {
                status: TargetAdmissionStatus::Rejected,
                reason,
                ..record
            });
        } else {
            decision.admitted_targets.push(TargetAdmissionRecord {
                status: TargetAdmissionStatus::Admitted,
                ..record
            });
        }
    }
    decision.admitted_targets.sort_by(|left, right| {
        left.priority
            .cmp(&right.priority)
            .then(left.path.cmp(&right.path))
    });
    if let Some(selected) = decision.admitted_targets.first() {
        let best_priority = selected.priority;
        let same_priority_paths = decision
            .admitted_targets
            .iter()
            .filter(|record| record.priority == best_priority)
            .map(|record| record.path.as_str())
            .collect::<Vec<_>>();
        if same_priority_paths.len() > 1 {
            decision.explicit_stop_reason = Some(format!(
                "ambiguous_recovery_target_tie:{}",
                same_priority_paths.join("|")
            ));
        } else {
            decision.selected_target = Some(selected.path.clone());
            decision.selected_role = Some(selected.role);
            decision.selected_priority = Some(selected.priority);
        }
    } else if policy.requires_target {
        decision.explicit_stop_reason = Some(if current_cluster_exhausted(policy) {
            "failure_cluster_exhausted_no_admitted_target".to_string()
        } else {
            "no_admitted_recovery_target".to_string()
        });
    }
    decision
}

pub(crate) fn select_first_admitted_target(
    candidates: impl IntoIterator<Item = RepairTargetCandidate>,
    graph: &ArtifactGraph,
    allowed_roles: &[ArtifactRole],
) -> Option<TargetAdmission> {
    let mut admitted = candidates
        .into_iter()
        .map(|candidate| admit_repair_target(candidate, graph, allowed_roles))
        .filter(TargetAdmission::is_admitted)
        .collect::<Vec<_>>();
    admitted.sort_by_key(|admission| match admission {
        TargetAdmission::Admitted { priority, .. } => *priority,
        TargetAdmission::Rejected { .. } => u8::MAX,
    });
    admitted.into_iter().next()
}

fn proposed_record(
    candidate: &RepairTargetCandidate,
    graph: &ArtifactGraph,
    scope: &WorkspaceScope,
    changed_paths: &[String],
) -> TargetAdmissionRecord {
    let path = normalize_path(&candidate.path);
    let role = graph.node(&path).map(|node| node.role).unwrap_or_else(|| {
        role_for_path(
            &path,
            crate::agent::step_runner::artifact_graph::ArtifactLifecycle::Required,
        )
    });
    let ownership = classify_artifact_ownership(
        graph,
        scope,
        &path,
        role,
        candidate.source.as_str(),
        changed_paths,
    );
    let priority = target_priority(candidate, role, &ownership);
    let source_of_truth = candidate
        .source_of_truth
        .clone()
        .unwrap_or_else(|| ownership.source_of_truth.clone());
    TargetAdmissionRecord {
        status: TargetAdmissionStatus::Proposed,
        path,
        role,
        priority,
        source: candidate.source,
        ownership: ownership.ownership,
        source_of_truth,
        ownership_source: ownership.ownership_subreason.clone(),
        workspace_scope: ownership.workspace_scope.clone(),
        evidence_freshness: candidate.evidence_freshness,
        focused_edit: candidate.focused_edit,
        current_excerpt_available: candidate.current_excerpt_available,
        priority_components: target_priority_components(candidate, role, &ownership, priority),
        reason: format!(
            "source={} ownership={} ownership_source={} scope={}",
            candidate.source.as_str(),
            ownership.ownership.as_str(),
            ownership.ownership_subreason,
            scope.summary()
        ),
    }
}

fn policy_rejection_reason(
    record: &TargetAdmissionRecord,
    policy: &TargetAdmissionPolicy,
) -> Option<String> {
    if current_cluster_exhausted(policy) {
        return Some("failure_cluster_exhausted_for_current_cluster".to_string());
    }
    if !policy.allow_file_target {
        return Some(format!(
            "file_target_not_allowed_for_current_job:{}",
            policy.active_job
        ));
    }
    if policy
        .exhausted_targets
        .iter()
        .any(|target| target == &record.path)
    {
        return Some("target_exhausted_for_same_failure_cluster".to_string());
    }
    if policy.exhausted_roles.contains(&record.role) {
        return Some(format!(
            "role_exhausted_for_same_failure_cluster:{}",
            record.role.as_str()
        ));
    }
    if record.evidence_freshness == TargetEvidenceFreshness::Stale {
        return Some("stale_target_evidence".to_string());
    }
    match record.focused_edit {
        FocusedEditStatus::MissingCurrentExcerpt => {
            return Some("focused_edit_missing_current_excerpt".to_string());
        }
        FocusedEditStatus::StaleTarget => return Some("focused_edit_stale_target".to_string()),
        FocusedEditStatus::TargetNotOwned => {
            return Some("focused_edit_target_not_owned".to_string());
        }
        FocusedEditStatus::NotRequired | FocusedEditStatus::Eligible => {}
    }
    None
}

fn current_cluster_exhausted(policy: &TargetAdmissionPolicy) -> bool {
    let Some(cluster) = policy.current_cluster.as_deref() else {
        return false;
    };
    policy
        .exhausted_clusters
        .iter()
        .any(|exhausted| exhausted == cluster)
}

fn admission_rejection_reason(
    candidate: RepairTargetCandidate,
    graph: &ArtifactGraph,
    allowed_roles: &[ArtifactRole],
    scope: &WorkspaceScope,
    changed_paths: &[String],
) -> Option<String> {
    match admit_repair_target_with_scope(candidate, graph, allowed_roles, scope, changed_paths) {
        TargetAdmission::Admitted { .. } => None,
        TargetAdmission::Rejected { reason, .. } => Some(reason),
    }
}

fn normalize_path(path: &str) -> String {
    path.trim().trim_start_matches("./").replace('\\', "/")
}

fn compact(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn target_priority(
    candidate: &RepairTargetCandidate,
    role: ArtifactRole,
    ownership: &crate::agent::step_runner::artifact_ownership::ArtifactOwnershipDecision,
) -> u8 {
    let source = candidate.source.priority().saturating_mul(24);
    let ownership_penalty = match ownership.ownership {
        ArtifactOwnership::Owned => 0,
        ArtifactOwnership::CandidateOnly => 18,
        ArtifactOwnership::OutOfScope => 48,
    };
    let role_penalty = match role {
        ArtifactRole::SetupManifest | ArtifactRole::Entrypoint | ArtifactRole::Implementation => 0,
        ArtifactRole::IntegrationTarget | ArtifactRole::Test | ArtifactRole::Docs => 2,
        ArtifactRole::SetupConfig => 4,
        ArtifactRole::Unknown => 8,
        ArtifactRole::DependencyCache | ArtifactRole::GeneratedOutput => 48,
    };
    source
        .saturating_add(ownership_penalty)
        .saturating_add(role_penalty)
        .saturating_add(candidate.evidence_freshness.priority_penalty())
        .saturating_add(candidate.focused_edit.priority_penalty())
}

fn target_priority_components(
    candidate: &RepairTargetCandidate,
    role: ArtifactRole,
    ownership: &crate::agent::step_runner::artifact_ownership::ArtifactOwnershipDecision,
    priority: u8,
) -> String {
    format!(
        "priority={priority};source={}:{};role={};ownership={};freshness={};focused_edit={};current_excerpt={}",
        candidate.source.as_str(),
        candidate.source.priority(),
        role.as_str(),
        ownership.ownership.as_str(),
        candidate.evidence_freshness.as_str(),
        candidate.focused_edit.as_str(),
        candidate.current_excerpt_available
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_dependency_cache_target() {
        let candidate = RepairTargetCandidate::new(
            "node_modules/react/index.js",
            ArtifactRole::DependencyCache,
            RepairTargetSource::FailureEvidence,
        );

        let admission = admit_repair_target(candidate, &ArtifactGraph::new(), &[]);

        assert!(!admission.is_admitted());
        assert!(admission.reason().contains("generated_or_dependency_cache"));
    }

    #[test]
    fn rejects_role_mismatch() {
        let candidate = RepairTargetCandidate::new(
            "README.md",
            ArtifactRole::Docs,
            RepairTargetSource::RequiredArtifact,
        );

        let admission =
            admit_repair_target(candidate, &ArtifactGraph::new(), &[ArtifactRole::Test]);

        assert!(!admission.is_admitted());
        assert!(admission.reason().contains("role_not_allowed"));
    }

    #[test]
    fn selects_lowest_priority_admitted_target() {
        let candidates = vec![
            RepairTargetCandidate::new(
                "README.md",
                ArtifactRole::Docs,
                RepairTargetSource::RequiredArtifact,
            ),
            RepairTargetCandidate::new(
                "tests/test_app.py",
                ArtifactRole::Test,
                RepairTargetSource::FailureEvidence,
            ),
        ];

        let admission =
            select_first_admitted_target(candidates, &ArtifactGraph::new(), &[ArtifactRole::Test])
                .unwrap();

        match admission {
            TargetAdmission::Admitted { path, priority, .. } => {
                assert_eq!(path, "tests/test_app.py");
                assert_eq!(priority, 0);
            }
            TargetAdmission::Rejected { .. } => panic!("expected admitted target"),
        }
    }

    #[test]
    fn rejects_candidate_outside_workspace_scope() {
        let mut graph = ArtifactGraph::new();
        graph.add_path(
            "apps/web/app/page.tsx",
            crate::agent::step_runner::artifact_graph::ArtifactLifecycle::Required,
            "contract.required_paths",
        );
        let scope = WorkspaceScope::from_graph(&graph);
        let candidate = RepairTargetCandidate::new(
            "apps/admin/app/page.tsx",
            ArtifactRole::Entrypoint,
            RepairTargetSource::FailureEvidence,
        );

        let admission = admit_repair_target_with_scope(
            candidate,
            &graph,
            &[ArtifactRole::Entrypoint],
            &scope,
            &[],
        );

        assert!(!admission.is_admitted());
        assert!(admission.reason().contains("artifact_out_of_scope"));
    }

    #[test]
    fn decision_records_proposed_admitted_and_rejected_targets() {
        let candidates = vec![
            RepairTargetCandidate::new(
                "node_modules/react/index.js",
                ArtifactRole::DependencyCache,
                RepairTargetSource::FailureEvidence,
            ),
            RepairTargetCandidate::new(
                "app/page.tsx",
                ArtifactRole::Entrypoint,
                RepairTargetSource::VerifierDiagnostic,
            ),
        ];
        let policy = TargetAdmissionPolicy::new(
            "source_implementation_repair",
            "edit_source_for_diagnostic",
            vec![ArtifactRole::Entrypoint, ArtifactRole::Implementation],
            true,
            true,
        );
        let graph = ArtifactGraph::new();
        let scope = WorkspaceScope::greenfield();

        let decision = decide_repair_target_with_scope(candidates, &graph, &policy, &scope, &[]);

        assert_eq!(decision.proposed_targets.len(), 2);
        assert_eq!(decision.admitted_targets.len(), 1);
        assert_eq!(decision.rejected_targets.len(), 1);
        assert_eq!(decision.selected_target.as_deref(), Some("app/page.tsx"));
        assert!(
            decision
                .rejected_lines()
                .iter()
                .any(|line| line.contains("generated_or_dependency_cache"))
        );
    }

    #[test]
    fn decision_stops_when_target_required_but_none_admitted() {
        let candidates = vec![RepairTargetCandidate::new(
            "node_modules/react/index.js",
            ArtifactRole::DependencyCache,
            RepairTargetSource::FailureEvidence,
        )];
        let policy = TargetAdmissionPolicy::new(
            "source_implementation_repair",
            "edit_source_for_diagnostic",
            vec![ArtifactRole::Implementation],
            true,
            true,
        );

        let decision = decide_repair_target_with_scope(
            candidates,
            &ArtifactGraph::new(),
            &policy,
            &WorkspaceScope::greenfield(),
            &[],
        );

        assert!(decision.selected_target.is_none());
        assert_eq!(
            decision.explicit_stop_reason.as_deref(),
            Some("no_admitted_recovery_target")
        );
    }

    #[test]
    fn decision_rejects_file_target_for_tool_protocol() {
        let candidates = vec![RepairTargetCandidate::new(
            "app/page.tsx",
            ArtifactRole::Entrypoint,
            RepairTargetSource::FailureEvidence,
        )];
        let policy = TargetAdmissionPolicy::new(
            "tool_protocol_correction",
            "correct_tool_protocol",
            Vec::new(),
            false,
            false,
        );

        let decision = decide_repair_target_with_scope(
            candidates,
            &ArtifactGraph::new(),
            &policy,
            &WorkspaceScope::greenfield(),
            &[],
        );

        assert!(decision.selected_target.is_none());
        assert!(
            decision
                .rejected_lines()
                .iter()
                .any(|line| line.contains("file_target_not_allowed"))
        );
    }

    #[test]
    fn decision_prefers_current_focused_edit_signal() {
        let candidates = vec![
            RepairTargetCandidate::new(
                "app/page.tsx",
                ArtifactRole::Entrypoint,
                RepairTargetSource::ArtifactGraphRelation,
            ),
            RepairTargetCandidate::new(
                "components/Game.tsx",
                ArtifactRole::Implementation,
                RepairTargetSource::ToolReadRecord,
            )
            .with_evidence_freshness(TargetEvidenceFreshness::Current)
            .with_focused_edit(FocusedEditStatus::Eligible)
            .with_current_excerpt_available(true),
        ];
        let policy = TargetAdmissionPolicy::new(
            "source_implementation_repair",
            "focused_edit_repair",
            vec![ArtifactRole::Entrypoint, ArtifactRole::Implementation],
            true,
            true,
        );

        let decision = decide_repair_target_with_scope(
            candidates,
            &ArtifactGraph::new(),
            &policy,
            &WorkspaceScope::greenfield(),
            &[],
        );

        assert_eq!(
            decision.selected_target.as_deref(),
            Some("components/Game.tsx")
        );
        let fields = decision.eval_report_fields();
        assert!(
            fields
                .iter()
                .any(|field| field.starts_with("target_priority_components="))
        );
        assert!(
            fields
                .iter()
                .any(|field| field.starts_with("target_source_of_truth="))
        );
        assert!(
            fields
                .iter()
                .any(|field| field == "focused_edit_status=eligible")
        );
    }

    #[test]
    fn decision_rejects_focused_edit_without_current_excerpt() {
        let candidates = vec![
            RepairTargetCandidate::new(
                "components/Game.tsx",
                ArtifactRole::Implementation,
                RepairTargetSource::ToolEditRecord,
            )
            .with_evidence_freshness(TargetEvidenceFreshness::Current)
            .with_focused_edit(FocusedEditStatus::MissingCurrentExcerpt)
            .with_current_excerpt_available(false),
        ];
        let policy = TargetAdmissionPolicy::new(
            "source_implementation_repair",
            "focused_edit_repair",
            vec![ArtifactRole::Implementation],
            true,
            true,
        );

        let decision = decide_repair_target_with_scope(
            candidates,
            &ArtifactGraph::new(),
            &policy,
            &WorkspaceScope::greenfield(),
            &[],
        );

        assert!(decision.selected_target.is_none());
        assert!(
            decision
                .rejected_lines()
                .iter()
                .any(|line| line.contains("focused_edit_missing_current_excerpt"))
        );
        assert!(
            decision
                .eval_report_fields()
                .iter()
                .any(|field| field == "focused_edit_status=missing_current_excerpt")
        );
    }

    #[test]
    fn decision_stops_on_ambiguous_same_priority_targets() {
        let candidates = vec![
            RepairTargetCandidate::new(
                "app/a.tsx",
                ArtifactRole::Implementation,
                RepairTargetSource::VerifierDiagnostic,
            ),
            RepairTargetCandidate::new(
                "app/b.tsx",
                ArtifactRole::Implementation,
                RepairTargetSource::VerifierDiagnostic,
            ),
        ];
        let policy = TargetAdmissionPolicy::new(
            "source_implementation_repair",
            "edit_source_for_diagnostic",
            vec![ArtifactRole::Implementation],
            true,
            true,
        );

        let decision = decide_repair_target_with_scope(
            candidates,
            &ArtifactGraph::new(),
            &policy,
            &WorkspaceScope::greenfield(),
            &[],
        );

        assert!(decision.selected_target.is_none());
        assert!(
            decision
                .explicit_stop_reason
                .as_deref()
                .is_some_and(|reason| reason.starts_with("ambiguous_recovery_target_tie:"))
        );
    }

    #[test]
    fn decision_rejects_exhausted_failure_cluster() {
        let candidates = vec![RepairTargetCandidate::new(
            "app/page.tsx",
            ArtifactRole::Entrypoint,
            RepairTargetSource::FailureEvidence,
        )];
        let policy = TargetAdmissionPolicy::new(
            "source_implementation_repair",
            "edit_source_for_diagnostic",
            vec![ArtifactRole::Entrypoint],
            true,
            true,
        )
        .with_current_cluster(Some("build_failure".to_string()))
        .with_exhausted_clusters(["build_failure"]);

        let decision = decide_repair_target_with_scope(
            candidates,
            &ArtifactGraph::new(),
            &policy,
            &WorkspaceScope::greenfield(),
            &[],
        );

        assert!(decision.selected_target.is_none());
        assert_eq!(
            decision.explicit_stop_reason.as_deref(),
            Some("failure_cluster_exhausted_no_admitted_target")
        );
        assert!(
            decision
                .rejected_lines()
                .iter()
                .any(|line| line.contains("failure_cluster_exhausted_for_current_cluster"))
        );
    }
}
