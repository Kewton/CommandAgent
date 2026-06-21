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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArtifactLedgerEntry {
    pub(crate) path: String,
    pub(crate) role: ArtifactRole,
    pub(crate) ownership: ArtifactOwnership,
    pub(crate) origin: String,
    pub(crate) changed: bool,
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
        let changed_paths = records
            .iter()
            .filter(|record| record.ok && is_file_change_tool(&record.name))
            .flat_map(|record| record.target_paths.iter().cloned())
            .collect::<Vec<_>>();
        let mut entries = Vec::new();
        let mut overflowed = false;
        for record in records {
            if !record.ok {
                continue;
            }
            let changed = is_file_change_tool(&record.name);
            for path in &record.target_paths {
                if entries.len() >= ARTIFACT_LEDGER_ENTRY_LIMIT {
                    overflowed = true;
                    break;
                }
                let role = graph
                    .node(path)
                    .map(|node| node.role)
                    .unwrap_or_else(|| role_for_path(path, ArtifactLifecycle::Required));
                let ownership = classify_artifact_ownership(
                    graph,
                    scope,
                    path,
                    role,
                    "tool_execution_record",
                    &changed_paths,
                );
                entries.push(ArtifactLedgerEntry {
                    path: path.clone(),
                    role: ownership.role,
                    ownership: ownership.ownership,
                    origin: record.name.clone(),
                    changed,
                });
            }
            if overflowed {
                break;
            }
        }
        Self {
            entries,
            overflowed,
        }
    }

    pub(crate) fn render_lines(&self) -> Vec<String> {
        let mut lines = self
            .entries
            .iter()
            .map(|entry| {
                format!(
                    "{} role={} ownership={} origin={} changed={}",
                    entry.path,
                    entry.role.as_str(),
                    entry.ownership.as_str(),
                    entry.origin,
                    entry.changed
                )
            })
            .collect::<Vec<_>>();
        if self.overflowed {
            lines.push("artifact_ledger_overflow=true".to_string());
        }
        lines
    }
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
}
