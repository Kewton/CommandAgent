#![allow(dead_code)]

use crate::agent::step_runner::artifact_graph::{ArtifactGraph, ArtifactRole, role_for_path};
use crate::agent::step_runner::profile_artifact::{is_build_output_path, is_dependency_cache_path};
use crate::agent::step_runner::workspace_snapshot::WorkspaceSnapshot;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkspaceScopeKind {
    Explicit,
    SingleProjectRoot,
    AmbiguousParent,
    Greenfield,
}

impl WorkspaceScopeKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Explicit => "explicit",
            Self::SingleProjectRoot => "single_project_root",
            Self::AmbiguousParent => "ambiguous_parent",
            Self::Greenfield => "greenfield",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceScope {
    pub(crate) kind: WorkspaceScopeKind,
    roots: Vec<String>,
    excluded_paths: Vec<String>,
}

impl WorkspaceScope {
    pub(crate) fn greenfield() -> Self {
        Self {
            kind: WorkspaceScopeKind::Greenfield,
            roots: Vec::new(),
            excluded_paths: Vec::new(),
        }
    }

    pub(crate) fn from_graph(graph: &ArtifactGraph) -> Self {
        Self::from_paths(graph.nodes().iter().map(|node| node.path.as_str()))
    }

    pub(crate) fn from_snapshot_and_graph(
        snapshot: &WorkspaceSnapshot,
        graph: &ArtifactGraph,
    ) -> Self {
        Self::from_paths(
            graph.nodes().iter().map(|node| node.path.as_str()).chain(
                snapshot
                    .observed_paths
                    .iter()
                    .map(|item| item.path.as_str()),
            ),
        )
    }

    pub(crate) fn from_paths<'a>(paths: impl IntoIterator<Item = &'a str>) -> Self {
        let mut roots = BTreeSet::new();
        let mut excluded_paths = Vec::new();
        for path in paths {
            let path = normalize_path(path);
            if path.is_empty() || path.starts_with("step:") {
                continue;
            }
            if is_excluded_path(&path) {
                excluded_paths.push(path);
                continue;
            }
            roots.insert(project_root_for(&path));
        }
        let roots = roots.into_iter().collect::<Vec<_>>();
        let kind = match roots.as_slice() {
            [] => WorkspaceScopeKind::Greenfield,
            [root] if root == "." => WorkspaceScopeKind::SingleProjectRoot,
            [_] => WorkspaceScopeKind::Explicit,
            _ => WorkspaceScopeKind::AmbiguousParent,
        };
        Self {
            kind,
            roots,
            excluded_paths,
        }
    }

    pub(crate) fn roots(&self) -> &[String] {
        &self.roots
    }

    pub(crate) fn contains_path(&self, path: &str) -> bool {
        let path = normalize_path(path);
        if path.is_empty() {
            return false;
        }
        if path.starts_with("step:") {
            return true;
        }
        if is_excluded_path(&path) {
            return false;
        }
        match self.kind {
            WorkspaceScopeKind::Greenfield | WorkspaceScopeKind::SingleProjectRoot => true,
            WorkspaceScopeKind::Explicit | WorkspaceScopeKind::AmbiguousParent => self
                .roots
                .iter()
                .any(|root| root == "." || path == *root || path.starts_with(&format!("{root}/"))),
        }
    }

    pub(crate) fn summary(&self) -> String {
        let roots = if self.roots.is_empty() {
            "[]".to_string()
        } else {
            format!("[{}]", self.roots.join(", "))
        };
        let excluded = if self.excluded_paths.is_empty() {
            String::new()
        } else {
            format!(" excluded=[{}]", self.excluded_paths.join(", "))
        };
        format!("kind={} roots={}{}", self.kind.as_str(), roots, excluded)
    }
}

fn project_root_for(path: &str) -> String {
    let parts = path.split('/').collect::<Vec<_>>();
    if parts.len() >= 2
        && matches!(
            parts[0],
            "apps" | "packages" | "crates" | "services" | "examples"
        )
    {
        format!("{}/{}", parts[0], parts[1])
    } else {
        ".".to_string()
    }
}

fn is_excluded_path(path: &str) -> bool {
    if is_dependency_cache_path(path) || is_build_output_path(path) {
        return true;
    }
    matches!(
        role_for_path(
            path,
            crate::agent::step_runner::artifact_graph::ArtifactLifecycle::Required,
        ),
        ArtifactRole::GeneratedOutput | ArtifactRole::DependencyCache
    )
}

fn normalize_path(path: &str) -> String {
    path.trim().trim_start_matches("./").replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::step_runner::artifact_graph::{ArtifactGraph, ArtifactLifecycle};

    #[test]
    fn workspace_scope_detects_greenfield() {
        let scope = WorkspaceScope::from_graph(&ArtifactGraph::new());

        assert_eq!(scope.kind, WorkspaceScopeKind::Greenfield);
        assert!(scope.contains_path("package.json"));
        assert!(!scope.contains_path("node_modules/react/index.js"));
    }

    #[test]
    fn workspace_scope_detects_single_project_root() {
        let mut graph = ArtifactGraph::new();
        graph.add_path("app/page.tsx", ArtifactLifecycle::Required, "test");
        graph.add_path("components/Game.tsx", ArtifactLifecycle::Required, "test");

        let scope = WorkspaceScope::from_graph(&graph);

        assert_eq!(scope.kind, WorkspaceScopeKind::SingleProjectRoot);
        assert_eq!(scope.roots(), &[".".to_string()]);
        assert!(scope.contains_path("app/page.tsx"));
    }

    #[test]
    fn workspace_scope_detects_ambiguous_parent() {
        let mut graph = ArtifactGraph::new();
        graph.add_path("apps/web/app/page.tsx", ArtifactLifecycle::Required, "test");
        graph.add_path(
            "packages/ui/src/lib.ts",
            ArtifactLifecycle::Required,
            "test",
        );

        let scope = WorkspaceScope::from_graph(&graph);

        assert_eq!(scope.kind, WorkspaceScopeKind::AmbiguousParent);
        assert!(scope.contains_path("apps/web/app/page.tsx"));
        assert!(scope.contains_path("packages/ui/src/lib.ts"));
        assert!(!scope.contains_path("apps/admin/app/page.tsx"));
    }
}
