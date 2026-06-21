#![allow(dead_code)]

use crate::agent::step_runner::artifact_graph::{
    ArtifactGraph, ArtifactRole, recovery_target_admissible,
};
use crate::agent::step_runner::workspace_scope::WorkspaceScope;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArtifactOwnership {
    Owned,
    CandidateOnly,
    OutOfScope,
}

impl ArtifactOwnership {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Owned => "owned",
            Self::CandidateOnly => "candidate_only",
            Self::OutOfScope => "out_of_scope",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArtifactOwnershipDecision {
    pub(crate) path: String,
    pub(crate) role: ArtifactRole,
    pub(crate) ownership: ArtifactOwnership,
    pub(crate) reason: String,
}

pub(crate) fn classify_artifact_ownership(
    graph: &ArtifactGraph,
    scope: &WorkspaceScope,
    path: &str,
    role: ArtifactRole,
    source: &str,
    changed_paths: &[String],
) -> ArtifactOwnershipDecision {
    let path = normalize_path(path);
    if path.starts_with("step:") {
        return decision(path, role, ArtifactOwnership::Owned, "step_contract_target");
    }
    if !recovery_target_admissible(role) {
        return decision(
            path,
            role,
            ArtifactOwnership::OutOfScope,
            "generated_or_dependency_cache",
        );
    }
    if !scope.contains_path(&path) {
        return decision(
            path,
            role,
            ArtifactOwnership::OutOfScope,
            "outside_workspace_scope",
        );
    }
    if changed_paths
        .iter()
        .any(|changed| normalize_path(changed) == path)
    {
        return decision(path, role, ArtifactOwnership::Owned, "changed_by_tool");
    }
    if let Some(node) = graph.node(&path) {
        return decision(
            path,
            node.role,
            ArtifactOwnership::Owned,
            format!("artifact_graph:{}:{}", node.source, node.lifecycle.as_str()),
        );
    }
    if deterministic_source_owns_target(source) {
        return decision(
            path,
            role,
            ArtifactOwnership::Owned,
            format!("deterministic_source:{source}"),
        );
    }
    decision(
        path,
        role,
        ArtifactOwnership::CandidateOnly,
        "candidate_without_ownership",
    )
}

fn deterministic_source_owns_target(source: &str) -> bool {
    matches!(
        source,
        "failure_evidence"
            | "profile_selected_route"
            | "verifier_diagnostic"
            | "required_artifact"
            | "setup_manifest"
    ) || source.starts_with("contract.")
        || source.starts_with("plan.")
        || source.starts_with("plan_lint.")
}

fn decision(
    path: String,
    role: ArtifactRole,
    ownership: ArtifactOwnership,
    reason: impl Into<String>,
) -> ArtifactOwnershipDecision {
    ArtifactOwnershipDecision {
        path,
        role,
        ownership,
        reason: reason.into(),
    }
}

fn normalize_path(path: &str) -> String {
    path.trim().trim_start_matches("./").replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::step_runner::artifact_graph::{ArtifactGraph, ArtifactLifecycle};

    #[test]
    fn artifact_ownership_rejects_dependency_cache() {
        let graph = ArtifactGraph::new();
        let scope = WorkspaceScope::greenfield();

        let decision = classify_artifact_ownership(
            &graph,
            &scope,
            "node_modules/react/index.js",
            ArtifactRole::DependencyCache,
            "failure_evidence",
            &[],
        );

        assert_eq!(decision.ownership, ArtifactOwnership::OutOfScope);
        assert!(decision.reason.contains("dependency_cache"));
    }

    #[test]
    fn artifact_ownership_promotes_edited_path() {
        let graph = ArtifactGraph::new();
        let scope = WorkspaceScope::greenfield();

        let decision = classify_artifact_ownership(
            &graph,
            &scope,
            "src/main.rs",
            ArtifactRole::Implementation,
            "artifact_graph_relation",
            &["src/main.rs".to_string()],
        );

        assert_eq!(decision.ownership, ArtifactOwnership::Owned);
        assert_eq!(decision.reason, "changed_by_tool");
    }

    #[test]
    fn graph_node_owns_required_artifact() {
        let mut graph = ArtifactGraph::new();
        graph.add_path(
            "app/page.tsx",
            ArtifactLifecycle::Required,
            "contract.required_paths",
        );
        let scope = WorkspaceScope::from_graph(&graph);

        let decision = classify_artifact_ownership(
            &graph,
            &scope,
            "app/page.tsx",
            ArtifactRole::Entrypoint,
            "artifact_graph_relation",
            &[],
        );

        assert_eq!(decision.ownership, ArtifactOwnership::Owned);
        assert!(decision.reason.contains("contract.required_paths"));
    }
}
