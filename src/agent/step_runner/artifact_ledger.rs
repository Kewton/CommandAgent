#![allow(dead_code)]

use crate::agent::minimal_loop::guards::is_file_change_tool;
use crate::agent::minimal_loop::result::ToolExecutionRecord;
use crate::agent::step_runner::artifact_graph::{
    ArtifactGraph, ArtifactLifecycle, ArtifactRole, role_for_path,
};
use crate::agent::step_runner::artifact_ownership::{
    ArtifactOwnership, classify_artifact_ownership,
};
use crate::agent::step_runner::workspace_scope::WorkspaceScope;

const ARTIFACT_LEDGER_ENTRY_LIMIT: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArtifactFactSource {
    ArtifactGraph,
    ToolRecord,
    WorkspaceObservation,
    VerifierDiagnostic,
}

impl ArtifactFactSource {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ArtifactGraph => "artifact_graph",
            Self::ToolRecord => "tool_record",
            Self::WorkspaceObservation => "workspace_observation",
            Self::VerifierDiagnostic => "verifier_diagnostic",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArtifactLedgerEntry {
    pub(crate) path: String,
    pub(crate) role: ArtifactRole,
    pub(crate) lifecycle: ArtifactLifecycle,
    pub(crate) ownership: ArtifactOwnership,
    pub(crate) origin: String,
    pub(crate) source: ArtifactFactSource,
    pub(crate) source_of_truth: String,
    pub(crate) changed: bool,
    pub(crate) observed: bool,
    pub(crate) required: bool,
    pub(crate) verifier_mentioned: bool,
    pub(crate) generated_or_cache: bool,
    pub(crate) in_scope: bool,
    pub(crate) diagnostic: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ArtifactLedgerSummary {
    pub(crate) entries: Vec<ArtifactLedgerEntry>,
    pub(crate) overflowed: bool,
}

impl ArtifactLedgerSummary {
    pub(crate) fn from_tool_records(
        records: &[ToolExecutionRecord],
        graph: &ArtifactGraph,
        scope: &WorkspaceScope,
    ) -> Self {
        Self::from_graph_and_tool_records(records, graph, scope)
    }

    pub(crate) fn from_graph_and_tool_records(
        records: &[ToolExecutionRecord],
        graph: &ArtifactGraph,
        scope: &WorkspaceScope,
    ) -> Self {
        let changed_paths = records
            .iter()
            .filter(|record| record.ok && is_file_change_tool(&record.name))
            .flat_map(|record| record.target_paths.iter().cloned())
            .collect::<Vec<_>>();
        let mut summary = Self::default();
        for node in graph.nodes() {
            let ownership = classify_artifact_ownership(
                graph,
                scope,
                &node.path,
                node.role,
                &node.source,
                &changed_paths,
            );
            summary.record_entry(ArtifactLedgerEntry {
                path: node.path.clone(),
                role: ownership.role,
                lifecycle: node.lifecycle,
                ownership: ownership.ownership,
                origin: node.source.clone(),
                source: ArtifactFactSource::ArtifactGraph,
                source_of_truth: "artifact_graph".to_string(),
                changed: changed_paths
                    .iter()
                    .any(|changed| normalize_path(changed) == node.path),
                observed: matches!(node.lifecycle, ArtifactLifecycle::Existing),
                required: lifecycle_requires_artifact(node.lifecycle),
                verifier_mentioned: false,
                generated_or_cache: generated_or_cache_role(ownership.role),
                in_scope: scope.contains_path(&node.path),
                diagnostic: None,
            });
        }
        for record in records {
            if !record.ok {
                continue;
            }
            let changed = is_file_change_tool(&record.name);
            for path in &record.target_paths {
                let role = graph
                    .node(path)
                    .map(|node| node.role)
                    .unwrap_or_else(|| role_for_path(path, ArtifactLifecycle::Required));
                let lifecycle = graph
                    .node(path)
                    .map(|node| node.lifecycle)
                    .unwrap_or(ArtifactLifecycle::Existing);
                let ownership = classify_artifact_ownership(
                    graph,
                    scope,
                    path,
                    role,
                    "tool_execution_record",
                    &changed_paths,
                );
                summary.record_entry(ArtifactLedgerEntry {
                    path: normalize_path(path),
                    role: ownership.role,
                    lifecycle,
                    ownership: ownership.ownership,
                    origin: record.name.clone(),
                    source: ArtifactFactSource::ToolRecord,
                    source_of_truth: "tool_execution_record".to_string(),
                    changed,
                    observed: true,
                    required: lifecycle_requires_artifact(lifecycle),
                    verifier_mentioned: false,
                    generated_or_cache: generated_or_cache_role(ownership.role),
                    in_scope: scope.contains_path(path),
                    diagnostic: None,
                });
            }
            if summary.overflowed {
                break;
            }
        }
        summary
    }

    pub(crate) fn record_workspace_observation(
        &mut self,
        path: &str,
        graph: &ArtifactGraph,
        scope: &WorkspaceScope,
    ) {
        let path = normalize_path(path);
        let role = graph
            .node(&path)
            .map(|node| node.role)
            .unwrap_or_else(|| role_for_path(&path, ArtifactLifecycle::Existing));
        let lifecycle = graph
            .node(&path)
            .map(|node| node.lifecycle)
            .unwrap_or(ArtifactLifecycle::Existing);
        let ownership =
            classify_artifact_ownership(graph, scope, &path, role, "workspace_observation", &[]);
        self.record_entry(ArtifactLedgerEntry {
            path,
            role: ownership.role,
            lifecycle,
            ownership: ownership.ownership,
            origin: "workspace_observation".to_string(),
            source: ArtifactFactSource::WorkspaceObservation,
            source_of_truth: "filesystem".to_string(),
            changed: false,
            observed: true,
            required: lifecycle_requires_artifact(lifecycle),
            verifier_mentioned: false,
            generated_or_cache: generated_or_cache_role(ownership.role),
            in_scope: scope.contains_path(&ownership.path),
            diagnostic: None,
        });
    }

    pub(crate) fn record_verifier_mention(
        &mut self,
        path: &str,
        diagnostic: impl Into<String>,
        graph: &ArtifactGraph,
        scope: &WorkspaceScope,
    ) {
        let path = normalize_path(path);
        let role = graph
            .node(&path)
            .map(|node| node.role)
            .unwrap_or_else(|| role_for_path(&path, ArtifactLifecycle::Required));
        let lifecycle = graph
            .node(&path)
            .map(|node| node.lifecycle)
            .unwrap_or(ArtifactLifecycle::Required);
        let ownership =
            classify_artifact_ownership(graph, scope, &path, role, "verifier_diagnostic", &[]);
        self.record_entry(ArtifactLedgerEntry {
            path,
            role: ownership.role,
            lifecycle,
            ownership: ownership.ownership,
            origin: "verifier_diagnostic".to_string(),
            source: ArtifactFactSource::VerifierDiagnostic,
            source_of_truth: "verifier_output".to_string(),
            changed: false,
            observed: matches!(lifecycle, ArtifactLifecycle::Existing),
            required: lifecycle_requires_artifact(lifecycle),
            verifier_mentioned: true,
            generated_or_cache: generated_or_cache_role(ownership.role),
            in_scope: scope.contains_path(&ownership.path),
            diagnostic: Some(diagnostic.into()),
        });
    }

    pub(crate) fn entry(&self, path: &str) -> Option<&ArtifactLedgerEntry> {
        let path = normalize_path(path);
        self.entries.iter().find(|entry| entry.path == path)
    }

    fn record_entry(&mut self, entry: ArtifactLedgerEntry) {
        if let Some(existing) = self.entries.iter_mut().find(|item| item.path == entry.path) {
            existing.lifecycle = stronger_lifecycle(existing.lifecycle, entry.lifecycle);
            existing.role = entry.role;
            existing.ownership = stronger_ownership(existing.ownership, entry.ownership);
            existing.changed |= entry.changed;
            existing.observed |= entry.observed;
            existing.required |= entry.required;
            existing.verifier_mentioned |= entry.verifier_mentioned;
            existing.generated_or_cache |= entry.generated_or_cache;
            existing.in_scope |= entry.in_scope;
            if !existing.source_of_truth.contains(&entry.source_of_truth) {
                existing.source_of_truth =
                    format!("{},{}", existing.source_of_truth, entry.source_of_truth);
            }
            if existing.diagnostic.is_none() {
                existing.diagnostic = entry.diagnostic;
            }
            return;
        }
        if self.entries.len() >= ARTIFACT_LEDGER_ENTRY_LIMIT {
            self.overflowed = true;
            return;
        }
        self.entries.push(entry);
    }

    pub(crate) fn render_lines(&self) -> Vec<String> {
        let mut lines = self
            .entries
            .iter()
            .map(|entry| {
                let diagnostic = entry
                    .diagnostic
                    .as_deref()
                    .map(|value| format!(" diagnostic={}", compact(value)))
                    .unwrap_or_default();
                format!(
                    "{} role={} lifecycle={} ownership={} origin={} source={} source_of_truth={} changed={} observed={} required={} verifier_mentioned={} in_scope={} generated_or_cache={}{}",
                    entry.path,
                    entry.role.as_str(),
                    entry.lifecycle.as_str(),
                    entry.ownership.as_str(),
                    entry.origin,
                    entry.source.as_str(),
                    entry.source_of_truth,
                    entry.changed,
                    entry.observed,
                    entry.required,
                    entry.verifier_mentioned,
                    entry.in_scope,
                    entry.generated_or_cache,
                    diagnostic
                )
            })
            .collect::<Vec<_>>();
        if self.overflowed {
            lines.push("artifact_ledger_overflow=true".to_string());
        }
        lines
    }
}

fn stronger_lifecycle(
    existing: ArtifactLifecycle,
    incoming: ArtifactLifecycle,
) -> ArtifactLifecycle {
    use ArtifactLifecycle as L;
    match (existing, incoming) {
        (L::Existing, _) | (_, L::Existing) => L::Existing,
        (L::SetupManifest, _) | (_, L::SetupManifest) => L::SetupManifest,
        (L::IntegrationTarget, _) | (_, L::IntegrationTarget) => L::IntegrationTarget,
        (L::ToBeCreated, _) | (_, L::ToBeCreated) => L::ToBeCreated,
        (L::Required, _) | (_, L::Required) => L::Required,
        _ => L::GeneratedOutput,
    }
}

fn stronger_ownership(
    existing: ArtifactOwnership,
    incoming: ArtifactOwnership,
) -> ArtifactOwnership {
    use ArtifactOwnership as O;
    match (existing, incoming) {
        (O::Owned, _) | (_, O::Owned) => O::Owned,
        (O::CandidateOnly, _) | (_, O::CandidateOnly) => O::CandidateOnly,
        _ => O::OutOfScope,
    }
}

fn lifecycle_requires_artifact(lifecycle: ArtifactLifecycle) -> bool {
    matches!(
        lifecycle,
        ArtifactLifecycle::Required
            | ArtifactLifecycle::ToBeCreated
            | ArtifactLifecycle::SetupManifest
            | ArtifactLifecycle::IntegrationTarget
    )
}

fn generated_or_cache_role(role: ArtifactRole) -> bool {
    matches!(
        role,
        ArtifactRole::GeneratedOutput | ArtifactRole::DependencyCache
    )
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

    fn record(name: &str, target_paths: Vec<&str>) -> ToolExecutionRecord {
        ToolExecutionRecord {
            name: name.to_string(),
            ok: true,
            output: String::new(),
            output_truncated: false,
            original_output_chars: 0,
            target_paths: target_paths.into_iter().map(str::to_string).collect(),
        }
    }

    #[test]
    fn ledger_records_write_path_without_payload() {
        let graph = ArtifactGraph::new();
        let scope = WorkspaceScope::greenfield();
        let summary = ArtifactLedgerSummary::from_tool_records(
            &[record("Write", vec!["src/main.rs"])],
            &graph,
            &scope,
        );

        assert_eq!(summary.entries.len(), 1);
        assert_eq!(summary.entries[0].path, "src/main.rs");
        assert!(summary.render_lines()[0].contains("changed=true"));
        assert!(summary.render_lines()[0].contains("observed=true"));
    }

    #[test]
    fn ledger_overflow_is_observable() {
        let graph = ArtifactGraph::new();
        let scope = WorkspaceScope::greenfield();
        let targets = (0..40)
            .map(|idx| format!("src/file_{idx}.rs"))
            .collect::<Vec<_>>();
        let record = ToolExecutionRecord {
            name: "Write".to_string(),
            ok: true,
            output: String::new(),
            output_truncated: false,
            original_output_chars: 0,
            target_paths: targets,
        };

        let summary = ArtifactLedgerSummary::from_tool_records(&[record], &graph, &scope);

        assert_eq!(summary.entries.len(), ARTIFACT_LEDGER_ENTRY_LIMIT);
        assert!(summary.overflowed);
        assert!(summary.render_lines().last().unwrap().contains("overflow"));
    }

    #[test]
    fn ledger_records_required_graph_path_without_tool_record() {
        let mut graph = ArtifactGraph::new();
        graph.add_path(
            "app/page.tsx",
            ArtifactLifecycle::Required,
            "plan.required_artifacts",
        );
        let scope = WorkspaceScope::from_graph(&graph);

        let summary = ArtifactLedgerSummary::from_tool_records(&[], &graph, &scope);

        assert_eq!(summary.entries.len(), 1);
        assert_eq!(summary.entries[0].path, "app/page.tsx");
        assert!(summary.entries[0].required);
        assert!(!summary.entries[0].changed);
        assert!(!summary.entries[0].observed);
        assert!(summary.render_lines()[0].contains("source=artifact_graph"));
    }

    #[test]
    fn ledger_keeps_verifier_mentions_distinct_from_changes() {
        let graph = ArtifactGraph::new();
        let scope = WorkspaceScope::greenfield();
        let mut summary = ArtifactLedgerSummary::default();

        summary.record_verifier_mention("src/lib.rs", "compiler points here", &graph, &scope);

        let entry = summary.entry("src/lib.rs").unwrap();
        assert!(entry.verifier_mentioned);
        assert!(!entry.changed);
        assert!(summary.render_lines()[0].contains("source=verifier_diagnostic"));
    }
}
