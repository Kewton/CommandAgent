//! Completion authority for deliverables and evidence.
//!
//! This module does not retry, repair, or choose the next job. It classifies
//! deterministic ledger, completion-evidence, and binding facts into a visible
//! terminal state that runtime and eval can report consistently.
#![allow(dead_code)]

use crate::agent::step_runner::artifact_ledger::ArtifactLedgerSummary;
use crate::agent::step_runner::completion_evidence::{
    CompletionEvidence, CompletionEvidenceKind, CompletionEvidenceStatus,
};
use crate::agent::step_runner::evidence_binding::{
    EvidenceBindingKind, EvidenceBindingPlan, EvidenceBindingStatus,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EvidenceRunnerStatus {
    NotRequired,
    Missing,
    Executed,
}

impl EvidenceRunnerStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::NotRequired => "not_required",
            Self::Missing => "missing",
            Self::Executed => "executed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompletionAuthorityStatus {
    Eligible,
    MissingDeliverable,
    MissingEvidence,
    CompletionEvidenceFailed,
    EvidenceBindingFailed,
}

impl CompletionAuthorityStatus {
    pub(crate) fn terminal_state(self) -> &'static str {
        match self {
            Self::Eligible => "ok",
            Self::MissingDeliverable => "missing_deliverable",
            Self::MissingEvidence => "missing_evidence",
            Self::CompletionEvidenceFailed => "completion_evidence_failed",
            Self::EvidenceBindingFailed => "evidence_binding_failed",
        }
    }

    pub(crate) fn diagnostic_code(self) -> &'static str {
        match self {
            Self::Eligible => "ok",
            Self::MissingDeliverable => "missing_deliverable",
            Self::MissingEvidence => "missing_evidence",
            Self::CompletionEvidenceFailed => "completion_evidence_failed",
            Self::EvidenceBindingFailed => "evidence_binding_failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompletionAuthorityResult {
    pub(crate) status: CompletionAuthorityStatus,
    pub(crate) evidence_runner_status: EvidenceRunnerStatus,
    pub(crate) completion_evidence_status: CompletionEvidenceStatus,
    pub(crate) evidence_binding_status: EvidenceBindingStatus,
    pub(crate) artifact_ledger_status: String,
    pub(crate) source_of_truth: String,
    pub(crate) missing_deliverables: Vec<String>,
    pub(crate) missing_evidence: Vec<String>,
    pub(crate) failed_evidence: Vec<String>,
    pub(crate) failed_bindings: Vec<String>,
}

impl CompletionAuthorityResult {
    pub(crate) fn success_eligible(&self) -> bool {
        self.status == CompletionAuthorityStatus::Eligible
    }

    pub(crate) fn terminal_state(&self) -> &'static str {
        self.status.terminal_state()
    }

    pub(crate) fn diagnostic_code(&self) -> &'static str {
        self.status.diagnostic_code()
    }

    pub(crate) fn render_eval_fields(&self) -> Vec<String> {
        vec![
            format!("terminal_state={}", self.terminal_state()),
            format!("diagnostic_code={}", self.diagnostic_code()),
            format!(
                "evidence_runner_status={}",
                self.evidence_runner_status.as_str()
            ),
            format!(
                "completion_evidence_status={}",
                self.completion_evidence_status.as_str()
            ),
            format!(
                "evidence_binding_status={}",
                self.evidence_binding_status.as_str()
            ),
            format!("artifact_ledger_status={}", self.artifact_ledger_status),
            format!("source_of_truth={}", self.source_of_truth),
        ]
    }

    pub(crate) fn render_contract_lines(&self) -> Vec<String> {
        let mut lines = self
            .render_eval_fields()
            .into_iter()
            .map(|line| format!("- {line}"))
            .collect::<Vec<_>>();
        if !self.missing_deliverables.is_empty() {
            lines.push(format!(
                "- missing_deliverables: {}",
                self.missing_deliverables.join(",")
            ));
        }
        if !self.missing_evidence.is_empty() {
            lines.push(format!(
                "- missing_evidence: {}",
                self.missing_evidence.join(",")
            ));
        }
        if !self.failed_evidence.is_empty() {
            lines.push(format!(
                "- failed_evidence: {}",
                self.failed_evidence.join(",")
            ));
        }
        if !self.failed_bindings.is_empty() {
            lines.push(format!(
                "- failed_bindings: {}",
                self.failed_bindings.join(",")
            ));
        }
        lines
    }
}

pub(crate) fn evaluate_completion_authority(
    required_paths: &[String],
    ledger: &ArtifactLedgerSummary,
    completion_evidence: &[CompletionEvidence],
    evidence_bindings: &[EvidenceBindingPlan],
) -> CompletionAuthorityResult {
    if required_paths.is_empty() && completion_evidence.is_empty() && evidence_bindings.is_empty() {
        return CompletionAuthorityResult {
            status: CompletionAuthorityStatus::Eligible,
            evidence_runner_status: EvidenceRunnerStatus::NotRequired,
            completion_evidence_status: CompletionEvidenceStatus::Passed,
            evidence_binding_status: EvidenceBindingStatus::Bound,
            artifact_ledger_status: "not_required".to_string(),
            source_of_truth: "no_required_deliverables".to_string(),
            missing_deliverables: Vec::new(),
            missing_evidence: Vec::new(),
            failed_evidence: Vec::new(),
            failed_bindings: Vec::new(),
        };
    }

    let missing_deliverables = required_paths
        .iter()
        .filter(|path| !ledger_entry_present(ledger, path))
        .cloned()
        .collect::<Vec<_>>();
    if !missing_deliverables.is_empty() {
        return CompletionAuthorityResult {
            status: CompletionAuthorityStatus::MissingDeliverable,
            evidence_runner_status: EvidenceRunnerStatus::Missing,
            completion_evidence_status: CompletionEvidenceStatus::Missing,
            evidence_binding_status: EvidenceBindingStatus::Missing,
            artifact_ledger_status: "missing_required".to_string(),
            source_of_truth: "artifact_ledger".to_string(),
            missing_deliverables,
            missing_evidence: Vec::new(),
            failed_evidence: Vec::new(),
            failed_bindings: Vec::new(),
        };
    }

    if completion_evidence.is_empty() {
        return CompletionAuthorityResult {
            status: CompletionAuthorityStatus::MissingEvidence,
            evidence_runner_status: EvidenceRunnerStatus::Missing,
            completion_evidence_status: CompletionEvidenceStatus::Missing,
            evidence_binding_status: aggregate_binding_status(evidence_bindings),
            artifact_ledger_status: "complete".to_string(),
            source_of_truth: "completion_evidence".to_string(),
            missing_deliverables: Vec::new(),
            missing_evidence: required_paths.to_vec(),
            failed_evidence: Vec::new(),
            failed_bindings: Vec::new(),
        };
    }

    let failed_evidence = completion_evidence
        .iter()
        .filter(|evidence| evidence.status == CompletionEvidenceStatus::Failed)
        .map(CompletionEvidence::render_line)
        .collect::<Vec<_>>();
    if !failed_evidence.is_empty() {
        return CompletionAuthorityResult {
            status: CompletionAuthorityStatus::CompletionEvidenceFailed,
            evidence_runner_status: EvidenceRunnerStatus::Executed,
            completion_evidence_status: CompletionEvidenceStatus::Failed,
            evidence_binding_status: aggregate_binding_status(evidence_bindings),
            artifact_ledger_status: "complete".to_string(),
            source_of_truth: "completion_evidence".to_string(),
            missing_deliverables: Vec::new(),
            missing_evidence: Vec::new(),
            failed_evidence,
            failed_bindings: Vec::new(),
        };
    }

    let missing_evidence = completion_evidence
        .iter()
        .filter(|evidence| {
            matches!(
                evidence.status,
                CompletionEvidenceStatus::Missing | CompletionEvidenceStatus::Unbound
            )
        })
        .map(CompletionEvidence::render_line)
        .collect::<Vec<_>>();
    if !missing_evidence.is_empty() {
        return CompletionAuthorityResult {
            status: CompletionAuthorityStatus::MissingEvidence,
            evidence_runner_status: EvidenceRunnerStatus::Missing,
            completion_evidence_status: CompletionEvidenceStatus::Missing,
            evidence_binding_status: aggregate_binding_status(evidence_bindings),
            artifact_ledger_status: "complete".to_string(),
            source_of_truth: "completion_evidence".to_string(),
            missing_deliverables: Vec::new(),
            missing_evidence,
            failed_evidence: Vec::new(),
            failed_bindings: Vec::new(),
        };
    }

    let failed_bindings = evidence_bindings
        .iter()
        .filter(|binding| binding.status != EvidenceBindingStatus::Bound)
        .map(EvidenceBindingPlan::render_line)
        .collect::<Vec<_>>();
    if !failed_bindings.is_empty() {
        return CompletionAuthorityResult {
            status: CompletionAuthorityStatus::EvidenceBindingFailed,
            evidence_runner_status: EvidenceRunnerStatus::Executed,
            completion_evidence_status: CompletionEvidenceStatus::Passed,
            evidence_binding_status: aggregate_binding_status(evidence_bindings),
            artifact_ledger_status: "complete".to_string(),
            source_of_truth: "evidence_binding".to_string(),
            missing_deliverables: Vec::new(),
            missing_evidence: Vec::new(),
            failed_evidence: Vec::new(),
            failed_bindings,
        };
    }

    CompletionAuthorityResult {
        status: CompletionAuthorityStatus::Eligible,
        evidence_runner_status: if completion_evidence.is_empty() {
            EvidenceRunnerStatus::NotRequired
        } else {
            EvidenceRunnerStatus::Executed
        },
        completion_evidence_status: CompletionEvidenceStatus::Passed,
        evidence_binding_status: EvidenceBindingStatus::Bound,
        artifact_ledger_status: "complete".to_string(),
        source_of_truth: "artifact_ledger_and_completion_evidence".to_string(),
        missing_deliverables: Vec::new(),
        missing_evidence: Vec::new(),
        failed_evidence: Vec::new(),
        failed_bindings: Vec::new(),
    }
}

pub(crate) fn file_layout_completion_evidence(path: &str, present: bool) -> CompletionEvidence {
    CompletionEvidence::new(
        CompletionEvidenceKind::RepoEdit,
        path,
        if present {
            CompletionEvidenceStatus::Passed
        } else {
            CompletionEvidenceStatus::Missing
        },
        "artifact_ledger_file_layout",
    )
}

pub(crate) fn file_layout_binding(path: &str, present: bool) -> EvidenceBindingPlan {
    EvidenceBindingPlan::new(
        EvidenceBindingKind::FileLayout,
        path,
        "filesystem path exists",
        if present {
            EvidenceBindingStatus::Bound
        } else {
            EvidenceBindingStatus::Missing
        },
    )
}

fn ledger_entry_present(ledger: &ArtifactLedgerSummary, path: &str) -> bool {
    ledger.entry(path).is_some_and(|entry| {
        entry.observed
            || entry.changed
            || matches!(
                entry.lifecycle,
                crate::agent::step_runner::artifact_graph::ArtifactLifecycle::Existing
            )
    })
}

fn aggregate_binding_status(bindings: &[EvidenceBindingPlan]) -> EvidenceBindingStatus {
    if bindings.iter().any(|binding| {
        matches!(
            binding.status,
            EvidenceBindingStatus::Failed | EvidenceBindingStatus::Unbound
        )
    }) {
        return EvidenceBindingStatus::Failed;
    }
    if bindings
        .iter()
        .any(|binding| binding.status == EvidenceBindingStatus::Missing)
    {
        return EvidenceBindingStatus::Missing;
    }
    EvidenceBindingStatus::Bound
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::step_runner::artifact_graph::{ArtifactGraph, ArtifactLifecycle};
    use crate::agent::step_runner::workspace_scope::WorkspaceScope;

    fn ledger_with_required(path: &str, exists: bool) -> ArtifactLedgerSummary {
        let mut graph = ArtifactGraph::new();
        graph.add_path(
            path,
            if exists {
                ArtifactLifecycle::Existing
            } else {
                ArtifactLifecycle::Required
            },
            "test.required",
        );
        let scope = WorkspaceScope::from_graph(&graph);
        ArtifactLedgerSummary::from_tool_records(&[], &graph, &scope)
    }

    #[test]
    fn missing_required_deliverable_is_not_evidence_failure() {
        let required = vec!["app/page.tsx".to_string()];
        let ledger = ledger_with_required("app/page.tsx", false);

        let result = evaluate_completion_authority(&required, &ledger, &[], &[]);

        assert_eq!(result.status, CompletionAuthorityStatus::MissingDeliverable);
        assert_eq!(result.terminal_state(), "missing_deliverable");
        assert_eq!(result.artifact_ledger_status, "missing_required");
    }

    #[test]
    fn existing_path_without_evidence_is_missing_evidence() {
        let required = vec!["README.md".to_string()];
        let ledger = ledger_with_required("README.md", true);

        let result = evaluate_completion_authority(&required, &ledger, &[], &[]);

        assert_eq!(result.status, CompletionAuthorityStatus::MissingEvidence);
        assert_eq!(result.terminal_state(), "missing_evidence");
    }

    #[test]
    fn failed_completion_evidence_is_distinct() {
        let required = vec!["src/lib.rs".to_string()];
        let ledger = ledger_with_required("src/lib.rs", true);
        let completion = vec![CompletionEvidence::new(
            CompletionEvidenceKind::VerifierExitZero,
            "cargo test",
            CompletionEvidenceStatus::Failed,
            "original_verifier",
        )];

        let result = evaluate_completion_authority(&required, &ledger, &completion, &[]);

        assert_eq!(
            result.status,
            CompletionAuthorityStatus::CompletionEvidenceFailed
        );
        assert_eq!(result.terminal_state(), "completion_evidence_failed");
    }

    #[test]
    fn unbound_binding_is_distinct_from_missing_artifact() {
        let required = vec!["app/page.tsx".to_string()];
        let ledger = ledger_with_required("app/page.tsx", true);
        let completion = vec![file_layout_completion_evidence("app/page.tsx", true)];
        let bindings = vec![EvidenceBindingPlan::new(
            EvidenceBindingKind::ImportSymbol,
            "app/page.tsx",
            "components/Game",
            EvidenceBindingStatus::Unbound,
        )];

        let result = evaluate_completion_authority(&required, &ledger, &completion, &bindings);

        assert_eq!(
            result.status,
            CompletionAuthorityStatus::EvidenceBindingFailed
        );
        assert_eq!(result.terminal_state(), "evidence_binding_failed");
    }

    #[test]
    fn bound_passing_evidence_is_eligible() {
        let required = vec!["app/page.tsx".to_string()];
        let ledger = ledger_with_required("app/page.tsx", true);
        let completion = vec![file_layout_completion_evidence("app/page.tsx", true)];
        let bindings = vec![file_layout_binding("app/page.tsx", true)];

        let result = evaluate_completion_authority(&required, &ledger, &completion, &bindings);

        assert!(result.success_eligible());
        assert_eq!(result.terminal_state(), "ok");
    }
}
