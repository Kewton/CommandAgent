#![allow(dead_code)]

use crate::agent::step_runner::artifact_graph::{
    ArtifactGraph, ArtifactRole, recovery_target_admissible, role_for_path,
};

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
        reason: format!("source={}", candidate.source.as_str()),
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
}
