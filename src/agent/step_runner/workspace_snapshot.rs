#![allow(dead_code)]

use crate::agent::step_runner::artifact_graph::{ArtifactRole, role_for_artifact_kind};
use crate::agent::step_runner::profile_artifact::{
    ArtifactKind, ArtifactProvenance, classify_profile_artifact, is_build_output_path,
    is_dependency_cache_path,
};
use crate::agent::step_runner::profiles::ProfileId;
use crate::agent::step_runner::workspace_scope::{WorkspaceScope, WorkspaceScopeKind};
use std::fs;
use std::path::{Path, PathBuf};

const WORKSPACE_SNAPSHOT_SCHEMA_VERSION: &str = "workspace_snapshot.v1";
const DEFAULT_MAX_PATHS: usize = 64;
const MAX_DEPTH: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceObservedPath {
    pub(crate) path: String,
    pub(crate) role: ArtifactRole,
    pub(crate) kind: ArtifactKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceIgnoredPath {
    pub(crate) path: String,
    pub(crate) reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceCandidatePath {
    pub(crate) path: String,
    pub(crate) role: ArtifactRole,
    pub(crate) status: String,
    pub(crate) reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceSnapshot {
    pub(crate) schema_version: String,
    pub(crate) scope_kind: WorkspaceScopeKind,
    pub(crate) roots: Vec<String>,
    pub(crate) observed_paths: Vec<WorkspaceObservedPath>,
    pub(crate) excluded_paths: Vec<WorkspaceIgnoredPath>,
    pub(crate) candidate_paths: Vec<WorkspaceCandidatePath>,
    pub(crate) lockfiles: Vec<String>,
    pub(crate) manifests: Vec<String>,
    pub(crate) selected_profile: String,
    pub(crate) selected_profile_root: Option<String>,
    pub(crate) overflowed: bool,
}

impl WorkspaceSnapshot {
    pub(crate) fn collect(cwd: &Path, profile: &str) -> Self {
        Self::collect_with_limit(cwd, profile, DEFAULT_MAX_PATHS)
    }

    pub(crate) fn collect_with_limit(cwd: &Path, profile: &str, max_paths: usize) -> Self {
        let profile_id = ProfileId::parse(profile).unwrap_or(ProfileId::Generic);
        let mut builder = WorkspaceSnapshotBuilder {
            cwd: cwd.to_path_buf(),
            profile_id,
            max_paths,
            observed_paths: Vec::new(),
            excluded_paths: Vec::new(),
            candidate_paths: Vec::new(),
            overflowed: false,
        };
        builder.walk(cwd, 0);
        builder.finish()
    }

    pub(crate) fn render_lines(&self) -> Vec<String> {
        let mut lines = vec![format!(
            "workspace_snapshot schema={} scope_kind={} roots=[{}] profile={}",
            self.schema_version,
            self.scope_kind.as_str(),
            self.roots.join(","),
            self.selected_profile
        )];
        for path in self.observed_paths.iter().take(8) {
            lines.push(format!(
                "{} role={} kind={:?}",
                path.path,
                path.role.as_str(),
                path.kind
            ));
        }
        for path in self.excluded_paths.iter().take(8) {
            lines.push(format!("{} ignored_reason={}", path.path, path.reason));
        }
        if self.overflowed {
            lines.push("workspace_snapshot_overflow=true".to_string());
        }
        lines.extend(self.candidate_report_fields());
        lines
    }

    pub(crate) fn candidate_report_fields(&self) -> Vec<String> {
        let observed = self
            .candidate_paths
            .iter()
            .filter(|path| path.status == "observed")
            .count();
        let excluded = self
            .candidate_paths
            .iter()
            .filter(|path| path.status == "excluded")
            .count();
        let ignored_reasons = self
            .excluded_paths
            .iter()
            .map(|path| path.reason.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join("|");
        vec![
            format!("workspace_candidate_status=observed:{observed}|excluded:{excluded}"),
            "workspace_ignored_dir_policy=single_source_of_truth".to_string(),
            format!(
                "workspace_candidate_ignored_reasons={}",
                if ignored_reasons.is_empty() {
                    "none".to_string()
                } else {
                    ignored_reasons
                }
            ),
        ]
    }
}

struct WorkspaceSnapshotBuilder {
    cwd: PathBuf,
    profile_id: ProfileId,
    max_paths: usize,
    observed_paths: Vec<WorkspaceObservedPath>,
    excluded_paths: Vec<WorkspaceIgnoredPath>,
    candidate_paths: Vec<WorkspaceCandidatePath>,
    overflowed: bool,
}

impl WorkspaceSnapshotBuilder {
    fn walk(&mut self, dir: &Path, depth: usize) {
        if depth > MAX_DEPTH || self.overflowed {
            return;
        }
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        let mut entries = entries.filter_map(Result::ok).collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.path());
        for entry in entries {
            if self.overflowed {
                return;
            }
            let path = entry.path();
            let relative = self.relative_path(&path);
            if relative.is_empty() {
                continue;
            }
            if entry
                .file_type()
                .map(|kind| kind.is_symlink())
                .unwrap_or(false)
            {
                self.record_ignored(relative, "symlink");
                continue;
            }
            if let Some(reason) = ignored_reason(&relative) {
                self.record_ignored(relative, reason);
                continue;
            }
            if path.is_dir() {
                self.walk(&path, depth + 1);
                continue;
            }
            self.record_observed(relative);
        }
    }

    fn record_observed(&mut self, path: String) {
        if self.observed_paths.len() >= self.max_paths {
            self.overflowed = true;
            return;
        }
        let classified = classify_profile_artifact(
            self.profile_id,
            &path,
            ArtifactProvenance::WorkspaceObservation,
        );
        let role = role_for_artifact_kind(classified.kind);
        self.observed_paths.push(WorkspaceObservedPath {
            role,
            kind: classified.kind,
            path: path.clone(),
        });
        self.candidate_paths.push(WorkspaceCandidatePath {
            path,
            role,
            status: "observed".to_string(),
            reason: "workspace_observation".to_string(),
        });
    }

    fn record_ignored(&mut self, path: String, reason: impl Into<String>) {
        let reason = reason.into();
        if self.excluded_paths.len() < self.max_paths {
            self.excluded_paths.push(WorkspaceIgnoredPath {
                path: path.clone(),
                reason: reason.clone(),
            });
        }
        if self.candidate_paths.len() < self.max_paths {
            self.candidate_paths.push(WorkspaceCandidatePath {
                path,
                role: ArtifactRole::Unknown,
                status: "excluded".to_string(),
                reason,
            });
        }
    }

    fn finish(self) -> WorkspaceSnapshot {
        let paths = self
            .observed_paths
            .iter()
            .map(|item| item.path.as_str())
            .collect::<Vec<_>>();
        let scope = WorkspaceScope::from_paths(paths);
        let manifests = self
            .observed_paths
            .iter()
            .filter(|item| item.role == ArtifactRole::SetupManifest)
            .map(|item| item.path.clone())
            .collect::<Vec<_>>();
        let lockfiles = self
            .observed_paths
            .iter()
            .filter(|item| is_lockfile(&item.path))
            .map(|item| item.path.clone())
            .collect::<Vec<_>>();
        let selected_profile_root = manifests.first().map(|path| {
            Path::new(path)
                .parent()
                .and_then(|parent| parent.to_str())
                .filter(|parent| !parent.is_empty())
                .unwrap_or(".")
                .to_string()
        });
        WorkspaceSnapshot {
            schema_version: WORKSPACE_SNAPSHOT_SCHEMA_VERSION.to_string(),
            scope_kind: scope.kind,
            roots: scope.roots().to_vec(),
            observed_paths: self.observed_paths,
            excluded_paths: self.excluded_paths,
            candidate_paths: self.candidate_paths,
            lockfiles,
            manifests,
            selected_profile: self.profile_id.as_str().to_string(),
            selected_profile_root,
            overflowed: self.overflowed,
        }
    }

    fn relative_path(&self, path: &Path) -> String {
        path.strip_prefix(&self.cwd)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    }
}

fn ignored_reason(path: &str) -> Option<&'static str> {
    if is_dependency_cache_path(path) {
        Some("dependency_cache")
    } else if is_build_output_path(path) {
        Some("build_output")
    } else if is_hidden_state_path(path) {
        Some("agent_or_vcs_state")
    } else if path.contains("__pycache__/") || path.starts_with("__pycache__/") {
        Some("python_cache")
    } else {
        None
    }
}

fn is_hidden_state_path(path: &str) -> bool {
    path == ".git"
        || path.starts_with(".git/")
        || path == ".commandagent"
        || path.starts_with(".commandagent/")
        || path == ".pytest_cache"
        || path.starts_with(".pytest_cache/")
}

fn is_lockfile(path: &str) -> bool {
    matches!(
        Path::new(path).file_name().and_then(|name| name.to_str()),
        Some("package-lock.json" | "pnpm-lock.yaml" | "yarn.lock" | "Cargo.lock" | "poetry.lock")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_workspace(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "commandagent-workspace-snapshot-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn snapshot_records_greenfield_workspace() {
        let root = temp_workspace("greenfield");

        let snapshot = WorkspaceSnapshot::collect(&root, "nextjs");

        assert_eq!(snapshot.scope_kind, WorkspaceScopeKind::Greenfield);
        assert!(snapshot.observed_paths.is_empty());
    }

    #[test]
    fn snapshot_records_single_project_root_and_manifests() {
        let root = temp_workspace("single-root");
        fs::write(root.join("package.json"), "{}").unwrap();
        fs::write(root.join("package-lock.json"), "{}").unwrap();
        fs::create_dir_all(root.join("app")).unwrap();
        fs::write(
            root.join("app/page.tsx"),
            "export default function Page() { return null }",
        )
        .unwrap();

        let snapshot = WorkspaceSnapshot::collect(&root, "nextjs");

        assert_eq!(snapshot.scope_kind, WorkspaceScopeKind::SingleProjectRoot);
        assert!(snapshot.manifests.contains(&"package.json".to_string()));
        assert!(
            snapshot
                .lockfiles
                .contains(&"package-lock.json".to_string())
        );
        assert!(
            snapshot
                .observed_paths
                .iter()
                .any(|item| item.path == "app/page.tsx" && item.role == ArtifactRole::Entrypoint)
        );
    }

    #[test]
    fn snapshot_records_ambiguous_parent_scope() {
        let root = temp_workspace("ambiguous");
        fs::create_dir_all(root.join("apps/web")).unwrap();
        fs::create_dir_all(root.join("packages/ui/src")).unwrap();
        fs::write(root.join("apps/web/package.json"), "{}").unwrap();
        fs::write(root.join("packages/ui/src/lib.ts"), "export const x = 1").unwrap();

        let snapshot = WorkspaceSnapshot::collect(&root, "nextjs");

        assert_eq!(snapshot.scope_kind, WorkspaceScopeKind::AmbiguousParent);
        assert!(snapshot.roots.contains(&"apps/web".to_string()));
        assert!(snapshot.roots.contains(&"packages/ui".to_string()));
    }

    #[test]
    fn snapshot_records_ignored_dependency_and_build_outputs() {
        let root = temp_workspace("ignored");
        fs::create_dir_all(root.join("node_modules/react")).unwrap();
        fs::create_dir_all(root.join(".next/server")).unwrap();
        fs::create_dir_all(root.join("target/debug")).unwrap();
        fs::write(root.join("node_modules/react/index.js"), "").unwrap();
        fs::write(root.join(".next/server/app.js"), "").unwrap();
        fs::write(root.join("target/debug/app"), "").unwrap();

        let snapshot = WorkspaceSnapshot::collect(&root, "nextjs");

        assert!(snapshot.observed_paths.is_empty());
        assert!(
            snapshot
                .excluded_paths
                .iter()
                .any(|item| item.reason == "dependency_cache")
        );
        assert!(
            snapshot
                .excluded_paths
                .iter()
                .any(|item| item.reason == "build_output")
        );
        assert!(
            snapshot
                .candidate_paths
                .iter()
                .any(|item| item.status == "excluded" && item.reason == "dependency_cache")
        );
        assert!(
            snapshot
                .candidate_report_fields()
                .contains(&"workspace_ignored_dir_policy=single_source_of_truth".to_string())
        );
        assert!(
            snapshot.candidate_report_fields().iter().any(|line| {
                line.starts_with("workspace_candidate_status=observed:0|excluded:")
            })
        );
    }
}
