//! Evidence-binding facts.
//!
//! Evidence binding records whether a required deliverable has an observable
//! proof path, such as a manifest identity, route import, test script, or docs
//! section. Binding failure is recovery data, not a retry trigger.
#![allow(dead_code)]

use crate::agent::step_runner::correction_evidence::ContractEvidence;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EvidenceBindingKind {
    ManifestIdentity,
    ImportSymbol,
    ExecutableHandle,
    TestScript,
    RequiredSection,
    SchemaColumn,
    Citation,
    FileLayout,
}

impl EvidenceBindingKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ManifestIdentity => "manifest_identity",
            Self::ImportSymbol => "import_symbol",
            Self::ExecutableHandle => "executable_handle",
            Self::TestScript => "test_script",
            Self::RequiredSection => "required_section",
            Self::SchemaColumn => "schema_column",
            Self::Citation => "citation",
            Self::FileLayout => "file_layout",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EvidenceBindingStatus {
    Bound,
    Missing,
    Failed,
    Unbound,
}

impl EvidenceBindingStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Bound => "bound",
            Self::Missing => "missing",
            Self::Failed => "failed",
            Self::Unbound => "unbound",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EvidenceBindingPlan {
    pub(crate) kind: EvidenceBindingKind,
    pub(crate) target: String,
    pub(crate) expected_binding: String,
    pub(crate) status: EvidenceBindingStatus,
    pub(crate) reason: Option<String>,
}

impl EvidenceBindingPlan {
    pub(crate) fn new(
        kind: EvidenceBindingKind,
        target: impl Into<String>,
        expected_binding: impl Into<String>,
        status: EvidenceBindingStatus,
    ) -> Self {
        Self {
            kind,
            target: target.into(),
            expected_binding: expected_binding.into(),
            status,
            reason: None,
        }
    }

    pub(crate) fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    pub(crate) fn render_line(&self) -> String {
        let reason = self
            .reason
            .as_deref()
            .map(|value| format!(" reason={}", compact(value)))
            .unwrap_or_default();
        format!(
            "kind={} target={} expected={} status={}{}",
            self.kind.as_str(),
            compact(&self.target),
            compact(&self.expected_binding),
            self.status.as_str(),
            reason
        )
    }

    pub(crate) fn to_contract_evidence(
        &self,
        failed_step: Option<&str>,
    ) -> Option<ContractEvidence> {
        if self.status == EvidenceBindingStatus::Bound {
            return None;
        }
        let mut evidence = ContractEvidence::new("evidence_binding")
            .with_reason_code(format!("evidence_binding_{}", self.status.as_str()))
            .with_violated_contract(self.kind.as_str())
            .with_target_path(self.target.clone())
            .with_repair_target(self.target.clone())
            .with_required_literals([self.expected_binding.clone()])
            .with_evidence_binding([self.render_line()])
            .with_active_job("evidence_binding_repair")
            .with_repair_action("repair_evidence_binding");
        if let Some(step) = failed_step {
            evidence = evidence.with_failed_step(step.to_string());
        }
        if let Some(reason) = &self.reason {
            evidence = evidence.with_diagnostic(reason.clone());
        }
        Some(evidence)
    }
}

pub(crate) fn manifest_identity_binding(
    target: &str,
    expected_identity: &str,
    status: EvidenceBindingStatus,
) -> EvidenceBindingPlan {
    EvidenceBindingPlan::new(
        EvidenceBindingKind::ManifestIdentity,
        target,
        expected_identity,
        status,
    )
}

pub(crate) fn executable_handle_binding(
    target: &str,
    executable: &str,
    status: EvidenceBindingStatus,
) -> EvidenceBindingPlan {
    EvidenceBindingPlan::new(
        EvidenceBindingKind::ExecutableHandle,
        target,
        executable,
        status,
    )
}

pub(crate) fn test_script_binding(
    target: &str,
    script_name: &str,
    status: EvidenceBindingStatus,
) -> EvidenceBindingPlan {
    EvidenceBindingPlan::new(EvidenceBindingKind::TestScript, target, script_name, status)
}

pub(crate) fn required_section_binding(
    target: &str,
    section: &str,
    status: EvidenceBindingStatus,
) -> EvidenceBindingPlan {
    EvidenceBindingPlan::new(
        EvidenceBindingKind::RequiredSection,
        target,
        section,
        status,
    )
}

pub(crate) fn schema_column_binding(
    target: &str,
    column: &str,
    status: EvidenceBindingStatus,
) -> EvidenceBindingPlan {
    EvidenceBindingPlan::new(EvidenceBindingKind::SchemaColumn, target, column, status)
}

pub(crate) fn citation_binding(
    target: &str,
    citation: &str,
    status: EvidenceBindingStatus,
) -> EvidenceBindingPlan {
    EvidenceBindingPlan::new(EvidenceBindingKind::Citation, target, citation, status)
}

fn compact(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_binding_becomes_contract_evidence() {
        let plan = EvidenceBindingPlan::new(
            EvidenceBindingKind::ImportSymbol,
            "app/page.tsx",
            "components/Game",
            EvidenceBindingStatus::Missing,
        );

        let evidence = plan.to_contract_evidence(Some("verify-route")).unwrap();

        assert_eq!(evidence.guard, "evidence_binding");
        assert_eq!(
            evidence.active_job.as_deref(),
            Some("evidence_binding_repair")
        );
        assert_eq!(
            evidence.repair_action.as_deref(),
            Some("repair_evidence_binding")
        );
        assert_eq!(evidence.failed_step.as_deref(), Some("verify-route"));
    }

    #[test]
    fn missing_package_test_script_is_binding_failure() {
        let evidence = test_script_binding(
            "package.json",
            "scripts.test",
            EvidenceBindingStatus::Missing,
        )
        .to_contract_evidence(Some("verify-tests"))
        .unwrap();

        assert_eq!(evidence.violated_contract.as_deref(), Some("test_script"));
        assert_eq!(evidence.target_path.as_deref(), Some("package.json"));
        assert_eq!(
            evidence.active_job.as_deref(),
            Some("evidence_binding_repair")
        );
    }

    #[test]
    fn docs_section_check_binds_to_target_document() {
        let plan =
            required_section_binding("docs/usage.md", "Usage", EvidenceBindingStatus::Failed);
        let rendered = plan.render_line();

        assert!(rendered.contains("kind=required_section"));
        assert!(rendered.contains("target=docs/usage.md"));
        assert!(rendered.contains("expected=Usage"));
        assert!(rendered.contains("status=failed"));
    }

    #[test]
    fn data_schema_check_binds_to_output_artifact() {
        let evidence =
            schema_column_binding("output/report.csv", "email", EvidenceBindingStatus::Missing)
                .to_contract_evidence(Some("verify-schema"))
                .unwrap();

        assert_eq!(evidence.violated_contract.as_deref(), Some("schema_column"));
        assert_eq!(evidence.repair_target.as_deref(), Some("output/report.csv"));
    }

    #[test]
    fn rust_binary_and_test_bindings_are_separate_contracts() {
        let binary = executable_handle_binding(
            "Cargo.toml",
            "bin:commandagent",
            EvidenceBindingStatus::Missing,
        );
        let test = test_script_binding("Cargo.toml", "cargo test", EvidenceBindingStatus::Missing);

        assert!(binary.render_line().contains("kind=executable_handle"));
        assert!(test.render_line().contains("kind=test_script"));
        assert_ne!(binary.kind, test.kind);
    }

    #[test]
    fn manifest_identity_and_citation_bindings_are_rendered() {
        let manifest = manifest_identity_binding(
            "pyproject.toml",
            "project.name",
            EvidenceBindingStatus::Bound,
        );
        let citation = citation_binding(
            "docs/report.md",
            "source-url",
            EvidenceBindingStatus::Unbound,
        );

        assert!(manifest.render_line().contains("kind=manifest_identity"));
        assert!(citation.render_line().contains("kind=citation"));
    }
}
