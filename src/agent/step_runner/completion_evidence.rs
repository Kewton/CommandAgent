//! Completion-evidence facts for deterministic recovery.
//!
//! This module only classifies observed evidence. It does not decide whether to
//! continue or retry a step.
#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompletionEvidenceKind {
    RepoEdit,
    VerifierExitZero,
    DocsSectionPass,
    StructuredDataPass,
    ReportCompletenessPass,
    CommandObservation,
}

impl CompletionEvidenceKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::RepoEdit => "repo_edit",
            Self::VerifierExitZero => "verifier_exit_zero",
            Self::DocsSectionPass => "docs_section_pass",
            Self::StructuredDataPass => "structured_data_pass",
            Self::ReportCompletenessPass => "report_completeness_pass",
            Self::CommandObservation => "command_observation",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompletionEvidenceStatus {
    Passed,
    Failed,
    Missing,
    Unbound,
    Stale,
}

impl CompletionEvidenceStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Missing => "missing",
            Self::Unbound => "unbound",
            Self::Stale => "stale",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompletionEvidence {
    pub(crate) kind: CompletionEvidenceKind,
    pub(crate) target: String,
    pub(crate) status: CompletionEvidenceStatus,
    pub(crate) source: String,
    pub(crate) diagnostic: Option<String>,
}

impl CompletionEvidence {
    pub(crate) fn new(
        kind: CompletionEvidenceKind,
        target: impl Into<String>,
        status: CompletionEvidenceStatus,
        source: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            target: target.into(),
            status,
            source: source.into(),
            diagnostic: None,
        }
    }

    pub(crate) fn with_diagnostic(mut self, diagnostic: impl Into<String>) -> Self {
        self.diagnostic = Some(diagnostic.into());
        self
    }

    pub(crate) fn render_line(&self) -> String {
        let diagnostic = self
            .diagnostic
            .as_deref()
            .map(|value| format!(" diagnostic={}", compact(value)))
            .unwrap_or_default();
        format!(
            "kind={} target={} status={} source={}{}",
            self.kind.as_str(),
            compact(&self.target),
            self.status.as_str(),
            compact(&self.source),
            diagnostic
        )
    }
}

pub(crate) fn verifier_completion(command: &str, passed: bool) -> CompletionEvidence {
    let status = if passed {
        CompletionEvidenceStatus::Passed
    } else {
        CompletionEvidenceStatus::Failed
    };
    CompletionEvidence::new(
        CompletionEvidenceKind::VerifierExitZero,
        command,
        status,
        "original_verifier",
    )
}

pub(crate) fn command_observation(command: &str, exit_ok: bool) -> CompletionEvidence {
    let status = if exit_ok {
        CompletionEvidenceStatus::Passed
    } else {
        CompletionEvidenceStatus::Failed
    };
    CompletionEvidence::new(
        CompletionEvidenceKind::CommandObservation,
        command,
        status,
        "runtime_command_observation",
    )
}

pub(crate) fn docs_section_pass(target: &str, passed: bool) -> CompletionEvidence {
    pass_fail_evidence(
        CompletionEvidenceKind::DocsSectionPass,
        target,
        passed,
        "docs_section_check",
    )
}

pub(crate) fn structured_data_pass(target: &str, passed: bool) -> CompletionEvidence {
    pass_fail_evidence(
        CompletionEvidenceKind::StructuredDataPass,
        target,
        passed,
        "structured_data_check",
    )
}

pub(crate) fn report_completeness_pass(target: &str, passed: bool) -> CompletionEvidence {
    pass_fail_evidence(
        CompletionEvidenceKind::ReportCompletenessPass,
        target,
        passed,
        "report_completeness_check",
    )
}

fn pass_fail_evidence(
    kind: CompletionEvidenceKind,
    target: &str,
    passed: bool,
    source: &str,
) -> CompletionEvidence {
    let status = if passed {
        CompletionEvidenceStatus::Passed
    } else {
        CompletionEvidenceStatus::Failed
    };
    CompletionEvidence::new(kind, target, status, source)
}

fn compact(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifier_completion_distinguishes_pass_and_fail() {
        let pass = verifier_completion("cargo test", true);
        let fail = verifier_completion("cargo test", false);

        assert!(pass.render_line().contains("status=passed"));
        assert!(fail.render_line().contains("status=failed"));
        assert!(fail.render_line().contains("target=cargo test"));
    }

    #[test]
    fn pass_side_completion_evidence_has_distinct_kinds() {
        let docs = docs_section_pass("README.md#usage", true);
        let data = structured_data_pass("report.csv:email", true);
        let report = report_completeness_pass("workspace/mvp/report.md", false);

        assert!(docs.render_line().contains("kind=docs_section_pass"));
        assert!(data.render_line().contains("kind=structured_data_pass"));
        assert!(
            report
                .render_line()
                .contains("kind=report_completeness_pass")
        );
        assert!(report.render_line().contains("status=failed"));
    }
}
