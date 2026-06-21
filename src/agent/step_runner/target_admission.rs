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
}

impl RepairTargetSource {
    pub(crate) fn priority(self) -> u8 {
        match self {
            Self::FailureEvidence => 0,
            Self::VerifierDiagnostic => 1,
            Self::ProfileSelectedRoute => 1,
            Self::SetupManifest => 1,
            Self::RequiredArtifact => 2,
            Self::ArtifactGraphRelation => 3,
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
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepairTargetCandidate {
    pub(crate) path: String,
    pub(crate) role: ArtifactRole,
    pub(crate) source: RepairTargetSource,
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
    pub(crate) reason: String,
}

impl TargetAdmissionRecord {
    pub(crate) fn render_line(&self) -> String {
        format!(
            "status={} path={} role={} priority={} source={} ownership={} reason={}",
            self.status.as_str(),
            self.path,
            self.role.as_str(),
            self.priority,
            self.source.as_str(),
            self.ownership.as_str(),
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
        decision.selected_target = Some(selected.path.clone());
        decision.selected_role = Some(selected.role);
        decision.selected_priority = Some(selected.priority);
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
    TargetAdmissionRecord {
        status: TargetAdmissionStatus::Proposed,
        path,
        role,
        priority: candidate.source.priority(),
        source: candidate.source,
        ownership: ownership.ownership,
        reason: format!(
            "source={} ownership={} scope={}",
            candidate.source.as_str(),
            ownership.ownership.as_str(),
            scope.summary()
        ),
    }
}

fn policy_rejection_reason(
    record: &TargetAdmissionRecord,
    policy: &TargetAdmissionPolicy,
) -> Option<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_dependency_cache_target() {
        let candidate = RepairTargetCandidate {
            path: "node_modules/react/index.js".to_string(),
            role: ArtifactRole::DependencyCache,
            source: RepairTargetSource::FailureEvidence,
        };

        let admission = admit_repair_target(candidate, &ArtifactGraph::new(), &[]);

        assert!(!admission.is_admitted());
        assert!(admission.reason().contains("generated_or_dependency_cache"));
    }

    #[test]
    fn rejects_role_mismatch() {
        let candidate = RepairTargetCandidate {
            path: "README.md".to_string(),
            role: ArtifactRole::Docs,
            source: RepairTargetSource::RequiredArtifact,
        };

        let admission =
            admit_repair_target(candidate, &ArtifactGraph::new(), &[ArtifactRole::Test]);

        assert!(!admission.is_admitted());
        assert!(admission.reason().contains("role_not_allowed"));
    }

    #[test]
    fn selects_lowest_priority_admitted_target() {
        let candidates = vec![
            RepairTargetCandidate {
                path: "README.md".to_string(),
                role: ArtifactRole::Docs,
                source: RepairTargetSource::RequiredArtifact,
            },
            RepairTargetCandidate {
                path: "tests/test_app.py".to_string(),
                role: ArtifactRole::Test,
                source: RepairTargetSource::FailureEvidence,
            },
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
        let candidate = RepairTargetCandidate {
            path: "apps/admin/app/page.tsx".to_string(),
            role: ArtifactRole::Entrypoint,
            source: RepairTargetSource::FailureEvidence,
        };

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
            RepairTargetCandidate {
                path: "node_modules/react/index.js".to_string(),
                role: ArtifactRole::DependencyCache,
                source: RepairTargetSource::FailureEvidence,
            },
            RepairTargetCandidate {
                path: "app/page.tsx".to_string(),
                role: ArtifactRole::Entrypoint,
                source: RepairTargetSource::VerifierDiagnostic,
            },
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
        let candidates = vec![RepairTargetCandidate {
            path: "node_modules/react/index.js".to_string(),
            role: ArtifactRole::DependencyCache,
            source: RepairTargetSource::FailureEvidence,
        }];
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
        let candidates = vec![RepairTargetCandidate {
            path: "app/page.tsx".to_string(),
            role: ArtifactRole::Entrypoint,
            source: RepairTargetSource::FailureEvidence,
        }];
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
}
