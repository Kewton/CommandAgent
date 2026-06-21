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
}
