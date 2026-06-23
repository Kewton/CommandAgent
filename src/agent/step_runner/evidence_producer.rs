//! Deterministic completion-evidence producers.
//!
//! Producers convert already observed runtime facts into completion evidence
//! and evidence bindings. They do not run tools, repair files, or select the
//! next recovery job.
#![allow(dead_code)]

use crate::agent::step_runner::artifact_ledger::ArtifactLedgerSummary;
use crate::agent::step_runner::completion_evidence::{
    CompletionEvidence, CompletionEvidenceKind, CompletionEvidenceStatus, verifier_completion,
};
use crate::agent::step_runner::evidence_authority::{
    file_layout_binding, file_layout_completion_evidence,
};
use crate::agent::step_runner::evidence_binding::{EvidenceBindingPlan, EvidenceBindingStatus};
use crate::agent::step_runner::verify::VerificationFailure;

#[derive(Debug, Clone)]
pub(crate) struct EvidenceProducerInput<'a> {
    pub(crate) step_id: &'a str,
    pub(crate) profile: &'a str,
    pub(crate) required_paths: &'a [String],
    pub(crate) verifier_commands: &'a [String],
    pub(crate) verifier_failures: &'a [VerificationFailure],
    pub(crate) ledger: &'a ArtifactLedgerSummary,
    pub(crate) observed_completion_facts: &'a [ObservedCompletionFact<'a>],
    pub(crate) observed_bindings: &'a [EvidenceBindingPlan],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ObservedCompletionFact<'a> {
    pub(crate) kind: CompletionEvidenceKind,
    pub(crate) target: &'a str,
    pub(crate) passed: bool,
    pub(crate) source: &'a str,
}

impl<'a> ObservedCompletionFact<'a> {
    pub(crate) fn new(
        kind: CompletionEvidenceKind,
        target: &'a str,
        passed: bool,
        source: &'a str,
    ) -> Self {
        Self {
            kind,
            target,
            passed,
            source,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct EvidenceProducerOutput {
    pub(crate) completion_evidence: Vec<CompletionEvidence>,
    pub(crate) evidence_bindings: Vec<EvidenceBindingPlan>,
    pub(crate) producer_lines: Vec<String>,
}

pub(crate) fn produce_completion_evidence(
    input: &EvidenceProducerInput<'_>,
) -> EvidenceProducerOutput {
    let mut output = EvidenceProducerOutput::default();
    produce_file_layout_evidence(input, &mut output);
    produce_verifier_evidence(input, &mut output);
    produce_observed_completion_facts(input, &mut output);
    produce_observed_bindings(input, &mut output);
    output
}

fn produce_file_layout_evidence(
    input: &EvidenceProducerInput<'_>,
    output: &mut EvidenceProducerOutput,
) {
    for path in input.required_paths {
        let present = ledger_path_present(input.ledger, path);
        output
            .completion_evidence
            .push(file_layout_completion_evidence(path, present));
        output
            .evidence_bindings
            .push(file_layout_binding(path, present));
        output.producer_lines.push(format!(
            "producer=file_layout step={} profile={} target={} status={}",
            compact(input.step_id),
            compact(input.profile),
            compact(path),
            if present { "bound" } else { "missing" }
        ));
    }
}

fn produce_observed_completion_facts(
    input: &EvidenceProducerInput<'_>,
    output: &mut EvidenceProducerOutput,
) {
    for fact in input.observed_completion_facts {
        let status = if fact.passed {
            CompletionEvidenceStatus::Passed
        } else {
            CompletionEvidenceStatus::Failed
        };
        output.completion_evidence.push(CompletionEvidence::new(
            fact.kind,
            fact.target,
            status,
            fact.source,
        ));
        output.producer_lines.push(format!(
            "producer=observed_completion step={} profile={} kind={} target={} status={} source={}",
            compact(input.step_id),
            compact(input.profile),
            fact.kind.as_str(),
            compact(fact.target),
            status.as_str(),
            compact(fact.source)
        ));
    }
}

fn produce_observed_bindings(
    input: &EvidenceProducerInput<'_>,
    output: &mut EvidenceProducerOutput,
) {
    for binding in input.observed_bindings {
        output.evidence_bindings.push(binding.clone());
        output.producer_lines.push(format!(
            "producer=evidence_binding step={} profile={} kind={} target={} status={}",
            compact(input.step_id),
            compact(input.profile),
            binding.kind.as_str(),
            compact(&binding.target),
            binding.status.as_str()
        ));
    }
}

fn produce_verifier_evidence(
    input: &EvidenceProducerInput<'_>,
    output: &mut EvidenceProducerOutput,
) {
    if input.verifier_commands.is_empty() {
        return;
    }
    if input.verifier_failures.is_empty() {
        for command in input.verifier_commands {
            output
                .completion_evidence
                .push(verifier_completion(command, true));
            output.producer_lines.push(format!(
                "producer=verifier step={} profile={} target={} status=passed",
                compact(input.step_id),
                compact(input.profile),
                compact(command)
            ));
        }
        return;
    }
    for failure in input.verifier_failures {
        output.completion_evidence.push(
            verifier_completion(&failure.command, false).with_diagnostic(
                if failure.diagnostic_excerpt.is_empty() {
                    failure.reason.clone()
                } else {
                    failure.diagnostic_excerpt.clone()
                },
            ),
        );
        output.producer_lines.push(format!(
            "producer=verifier step={} profile={} target={} status=failed",
            compact(input.step_id),
            compact(input.profile),
            compact(&failure.command)
        ));
    }
}

pub(crate) fn missing_evidence_for_path(path: &str, source: &str) -> CompletionEvidence {
    CompletionEvidence::new(
        CompletionEvidenceKind::RepoEdit,
        path,
        CompletionEvidenceStatus::Missing,
        source,
    )
}

pub(crate) fn binding_status_for_presence(present: bool) -> EvidenceBindingStatus {
    if present {
        EvidenceBindingStatus::Bound
    } else {
        EvidenceBindingStatus::Missing
    }
}

fn ledger_path_present(ledger: &ArtifactLedgerSummary, path: &str) -> bool {
    ledger.entry(path).is_some_and(|entry| {
        entry.observed
            || entry.changed
            || matches!(
                entry.lifecycle,
                crate::agent::step_runner::artifact_graph::ArtifactLifecycle::Existing
            )
    })
}

fn compact(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::step_runner::artifact_graph::{ArtifactGraph, ArtifactLifecycle};
    use crate::agent::step_runner::workspace_scope::WorkspaceScope;

    fn ledger_with_path(path: &str, exists: bool) -> ArtifactLedgerSummary {
        let mut graph = ArtifactGraph::new();
        graph.add_path(
            path,
            if exists {
                ArtifactLifecycle::Existing
            } else {
                ArtifactLifecycle::Required
            },
            "test",
        );
        let scope = WorkspaceScope::from_graph(&graph);
        ArtifactLedgerSummary::from_tool_records(&[], &graph, &scope)
    }

    #[test]
    fn file_layout_producer_binds_existing_required_path() {
        let required = vec!["README.md".to_string()];
        let ledger = ledger_with_path("README.md", true);

        let output = produce_completion_evidence(&EvidenceProducerInput {
            step_id: "write-readme",
            profile: "docs",
            required_paths: &required,
            verifier_commands: &[],
            verifier_failures: &[],
            ledger: &ledger,
            observed_completion_facts: &[],
            observed_bindings: &[],
        });

        assert_eq!(
            output.completion_evidence[0].status,
            CompletionEvidenceStatus::Passed
        );
        assert_eq!(
            output.evidence_bindings[0].status,
            EvidenceBindingStatus::Bound
        );
        assert!(
            output.producer_lines[0].contains("producer=file_layout"),
            "{:?}",
            output.producer_lines
        );
    }

    #[test]
    fn verifier_producer_records_passed_commands() {
        let ledger = ArtifactLedgerSummary::default();
        let commands = vec!["cargo test".to_string()];

        let output = produce_completion_evidence(&EvidenceProducerInput {
            step_id: "verify",
            profile: "rust",
            required_paths: &[],
            verifier_commands: &commands,
            verifier_failures: &[],
            ledger: &ledger,
            observed_completion_facts: &[],
            observed_bindings: &[],
        });

        assert_eq!(
            output.completion_evidence[0].status,
            CompletionEvidenceStatus::Passed
        );
        assert_eq!(output.completion_evidence[0].target, "cargo test");
    }

    #[test]
    fn verifier_producer_records_failed_commands() {
        let ledger = ArtifactLedgerSummary::default();
        let commands = vec!["npm run build".to_string()];
        let failures = vec![VerificationFailure {
            command: "npm run build".to_string(),
            reason: "command_failed".to_string(),
            stdout_excerpt: String::new(),
            stderr_excerpt: String::new(),
            diagnostic_excerpt: "build failed".to_string(),
            source_excerpt: None,
        }];

        let output = produce_completion_evidence(&EvidenceProducerInput {
            step_id: "verify-build",
            profile: "nextjs",
            required_paths: &[],
            verifier_commands: &commands,
            verifier_failures: &failures,
            ledger: &ledger,
            observed_completion_facts: &[],
            observed_bindings: &[],
        });

        assert_eq!(
            output.completion_evidence[0].status,
            CompletionEvidenceStatus::Failed
        );
        assert!(
            output.completion_evidence[0]
                .render_line()
                .contains("diagnostic=build failed")
        );
    }

    #[test]
    fn observed_completion_and_binding_facts_are_projected() {
        use crate::agent::step_runner::evidence_binding::{
            EvidenceBindingStatus, required_section_binding,
        };

        let ledger = ArtifactLedgerSummary::default();
        let observed = vec![
            ObservedCompletionFact::new(
                CompletionEvidenceKind::DocsSectionPass,
                "README.md#Usage",
                true,
                "docs_section_check",
            ),
            ObservedCompletionFact::new(
                CompletionEvidenceKind::ReportCompletenessPass,
                "workspace/mvp/report.md",
                false,
                "report_completeness_check",
            ),
        ];
        let bindings = vec![required_section_binding(
            "README.md",
            "Usage",
            EvidenceBindingStatus::Bound,
        )];

        let output = produce_completion_evidence(&EvidenceProducerInput {
            step_id: "docs-proof",
            profile: "docs",
            required_paths: &[],
            verifier_commands: &[],
            verifier_failures: &[],
            ledger: &ledger,
            observed_completion_facts: &observed,
            observed_bindings: &bindings,
        });

        assert!(
            output
                .completion_evidence
                .iter()
                .any(|evidence| evidence.kind == CompletionEvidenceKind::DocsSectionPass)
        );
        assert!(
            output
                .completion_evidence
                .iter()
                .any(|evidence| evidence.status == CompletionEvidenceStatus::Failed)
        );
        assert_eq!(
            output.evidence_bindings[0].status,
            EvidenceBindingStatus::Bound
        );
        assert!(
            output
                .producer_lines
                .iter()
                .any(|line| line.contains("producer=observed_completion"))
        );
    }
}
