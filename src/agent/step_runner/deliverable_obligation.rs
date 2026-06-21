//! Deliverable-obligation records.
//!
//! Obligations distinguish missing deliverables from missing evidence for an
//! existing deliverable. They are data projected into repair packets.
#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeliverableKind {
    Source,
    SetupManifest,
    Test,
    Docs,
    StructuredData,
    Report,
}

impl DeliverableKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::SetupManifest => "setup_manifest",
            Self::Test => "test",
            Self::Docs => "docs",
            Self::StructuredData => "structured_data",
            Self::Report => "report",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FreshnessRule {
    MustExist,
    MustBeEditedThisSession,
    MustMatchCurrentPlan,
    MustHaveVerifierEvidence,
}

impl FreshnessRule {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::MustExist => "must_exist",
            Self::MustBeEditedThisSession => "must_be_edited_this_session",
            Self::MustMatchCurrentPlan => "must_match_current_plan",
            Self::MustHaveVerifierEvidence => "must_have_verifier_evidence",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DeliverableObligation {
    pub(crate) kind: DeliverableKind,
    pub(crate) path: String,
    pub(crate) required_evidence: Vec<String>,
    pub(crate) freshness: Vec<FreshnessRule>,
}

impl DeliverableObligation {
    pub(crate) fn new(kind: DeliverableKind, path: impl Into<String>) -> Self {
        Self {
            kind,
            path: path.into(),
            required_evidence: Vec::new(),
            freshness: vec![FreshnessRule::MustExist],
        }
    }

    pub(crate) fn with_required_evidence(mut self, evidence: impl Into<String>) -> Self {
        self.required_evidence.push(evidence.into());
        self
    }

    pub(crate) fn with_freshness(mut self, freshness: FreshnessRule) -> Self {
        if !self.freshness.contains(&freshness) {
            self.freshness.push(freshness);
        }
        self
    }

    pub(crate) fn render_line(&self) -> String {
        let required_evidence = if self.required_evidence.is_empty() {
            "none".to_string()
        } else {
            self.required_evidence.join("|")
        };
        let freshness = self
            .freshness
            .iter()
            .map(|rule| rule.as_str())
            .collect::<Vec<_>>()
            .join("|");
        format!(
            "kind={} path={} evidence={} freshness={}",
            self.kind.as_str(),
            self.path,
            required_evidence,
            freshness
        )
    }
}

pub(crate) fn obligation_kind_for_path(path: &str) -> DeliverableKind {
    if path == "package.json" || path.ends_with("/package.json") || path.ends_with("Cargo.toml") {
        DeliverableKind::SetupManifest
    } else if path.contains("/test") || path.ends_with("_test.rs") || path.ends_with(".test.ts") {
        DeliverableKind::Test
    } else if path.ends_with(".md") {
        DeliverableKind::Docs
    } else if path.ends_with(".json") || path.ends_with(".csv") || path.ends_with(".yaml") {
        DeliverableKind::StructuredData
    } else {
        DeliverableKind::Source
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obligation_rendering_keeps_freshness_visible() {
        let obligation = DeliverableObligation::new(DeliverableKind::Source, "app/page.tsx")
            .with_required_evidence("route imports Game")
            .with_freshness(FreshnessRule::MustHaveVerifierEvidence);

        let rendered = obligation.render_line();

        assert!(rendered.contains("kind=source"));
        assert!(rendered.contains("route imports Game"));
        assert!(rendered.contains("must_have_verifier_evidence"));
    }

    #[test]
    fn coding_task_can_require_fresh_source_and_verifier_evidence() {
        let obligation = DeliverableObligation::new(DeliverableKind::Source, "src/lib.rs")
            .with_freshness(FreshnessRule::MustBeEditedThisSession)
            .with_freshness(FreshnessRule::MustHaveVerifierEvidence);

        assert!(obligation.freshness.contains(&FreshnessRule::MustExist));
        assert!(
            obligation
                .freshness
                .contains(&FreshnessRule::MustBeEditedThisSession)
        );
        assert!(
            obligation
                .freshness
                .contains(&FreshnessRule::MustHaveVerifierEvidence)
        );
    }

    #[test]
    fn docs_and_data_tasks_do_not_require_fake_source_edits() {
        let docs = DeliverableObligation::new(obligation_kind_for_path("README.md"), "README.md");
        let data = DeliverableObligation::new(
            obligation_kind_for_path("reports/summary.csv"),
            "reports/summary.csv",
        );

        assert_eq!(docs.kind, DeliverableKind::Docs);
        assert_eq!(data.kind, DeliverableKind::StructuredData);
        assert!(
            !docs
                .freshness
                .contains(&FreshnessRule::MustBeEditedThisSession)
        );
        assert!(
            !data
                .freshness
                .contains(&FreshnessRule::MustBeEditedThisSession)
        );
    }
}
