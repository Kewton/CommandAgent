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
    pub(crate) source_of_truth: String,
    pub(crate) workspace_scope: String,
    pub(crate) candidate_origin: String,
    pub(crate) admissible_for_repair: bool,
    pub(crate) ownership_subreason: String,
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
        return decision(
            path,
            role,
            ArtifactOwnership::Owned,
            "step_contract_target",
            source,
            scope,
        );
    }
    if !recovery_target_admissible(role) {
        return decision(
            path,
            role,
            ArtifactOwnership::OutOfScope,
            "generated_or_dependency_cache",
            source,
            scope,
        );
    }
    if !scope.contains_path(&path) {
        return decision(
            path,
            role,
            ArtifactOwnership::OutOfScope,
            "outside_workspace_scope",
            source,
            scope,
        );
    }
    if changed_paths
        .iter()
        .any(|changed| normalize_path(changed) == path)
    {
        return decision(
            path,
            role,
            ArtifactOwnership::Owned,
            "changed_by_tool",
            source,
            scope,
        );
    }
    if let Some(node) = graph.node(&path) {
        return decision(
            path,
            node.role,
            ArtifactOwnership::Owned,
            format!("artifact_graph:{}:{}", node.source, node.lifecycle.as_str()),
            source,
            scope,
        );
    }
    if deterministic_source_owns_target(source) {
        return decision(
            path,
            role,
            ArtifactOwnership::Owned,
            format!("deterministic_source:{source}"),
            source,
            scope,
        );
    }
    decision(
        path,
        role,
        ArtifactOwnership::CandidateOnly,
        "candidate_without_ownership",
        source,
        scope,
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
            | "tool_write_record"
            | "tool_edit_record"
            | "tool_execution_record"
            | "scaffold_delta"
            | "setup_delta"
            | "completion_evidence"
            | "evidence_binding"
    ) || source.starts_with("contract.")
        || source.starts_with("plan.")
        || source.starts_with("plan_lint.")
}

fn decision(
    path: String,
    role: ArtifactRole,
    ownership: ArtifactOwnership,
    reason: impl Into<String>,
    source: &str,
    scope: &WorkspaceScope,
) -> ArtifactOwnershipDecision {
    let reason = reason.into();
    let admissible_for_repair =
        ownership == ArtifactOwnership::Owned && recovery_target_admissible(role);
    ArtifactOwnershipDecision {
        path,
        role,
        ownership,
        reason: reason.clone(),
        source_of_truth: source_of_truth_for(source),
        workspace_scope: scope.summary(),
        candidate_origin: source.to_string(),
        admissible_for_repair,
        ownership_subreason: ownership_subreason(role, ownership, &reason, source),
    }
}

fn source_of_truth_for(source: &str) -> String {
    let value = match source {
        "workspace_observation" => "filesystem",
        "verifier_diagnostic" => "verifier_output",
        "tool_read_record" | "tool_write_record" | "tool_edit_record" | "tool_execution_record" => {
            "tool_record"
        }
        "setup_delta" | "setup_manifest" => "setup_contract",
        "scaffold_delta" => "scaffold_contract",
        _ if source.starts_with("contract.") || source.starts_with("plan.") => "artifact_graph",
        _ => source,
    };
    value.to_string()
}

fn ownership_subreason(
    role: ArtifactRole,
    ownership: ArtifactOwnership,
    reason: &str,
    source: &str,
) -> String {
    if role == ArtifactRole::DependencyCache {
        return "dependency_cache".to_string();
    }
    if role == ArtifactRole::GeneratedOutput {
        return "generated_output".to_string();
    }
    if ownership == ArtifactOwnership::OutOfScope && reason.contains("workspace") {
        return "outside_workspace_scope".to_string();
    }
    if reason == "changed_by_tool" {
        return "changed_by_tool".to_string();
    }
    if source == "tool_read_record" {
        return "read_only_observation".to_string();
    }
    if source == "verifier_diagnostic" {
        return "verifier_owned_signal".to_string();
    }
    if source == "scaffold_delta" {
        return "scaffold_delta".to_string();
    }
    if source == "setup_delta" || source == "setup_manifest" {
        return "setup_delta".to_string();
    }
    reason.to_string()
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
        assert!(!decision.admissible_for_repair);
        assert_eq!(decision.ownership_subreason, "dependency_cache");
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
        assert_eq!(decision.source_of_truth, "artifact_graph_relation");
        assert!(decision.admissible_for_repair);
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
        assert_eq!(
            decision.workspace_scope,
            "kind=single_project_root roots=[.]"
        );
    }

    #[test]
    fn read_only_observation_is_not_owned_without_graph_or_change() {
        let graph = ArtifactGraph::new();
        let scope = WorkspaceScope::greenfield();

        let decision = classify_artifact_ownership(
            &graph,
            &scope,
            "src/lib.rs",
            ArtifactRole::Implementation,
            "tool_read_record",
            &[],
        );

        assert_eq!(decision.ownership, ArtifactOwnership::CandidateOnly);
        assert_eq!(decision.source_of_truth, "tool_record");
        assert_eq!(decision.ownership_subreason, "read_only_observation");
    }
}
