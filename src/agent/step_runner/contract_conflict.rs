//! Deterministic contract-conflict classification.
//!
//! This module does not execute repair. It separates the authoritative side of
//! a detected contract conflict from the side that may be repaired, then
//! renders that decision into the existing bounded repair machinery.

use crate::agent::step_runner::artifact_graph::{ArtifactLifecycle, ArtifactRole, role_for_path};
use crate::agent::step_runner::correction_evidence::ContractEvidence;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConflictSideKind {
    Implementation,
    Test,
    DocsOrApi,
    VerifierContract,
}

impl ConflictSideKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Implementation => "implementation",
            Self::Test => "test",
            Self::DocsOrApi => "docs_or_api",
            Self::VerifierContract => "verifier_contract",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContractConflictAuthority {
    ImplementationAuthoritative,
    TestAuthoritative,
    DocsOrApiAuthoritative,
    VerifierContractLimited,
    AmbiguousAuthority,
    InsufficientEvidence,
}

impl ContractConflictAuthority {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ImplementationAuthoritative => "implementation_authoritative",
            Self::TestAuthoritative => "test_authoritative",
            Self::DocsOrApiAuthoritative => "docs_or_api_authoritative",
            Self::VerifierContractLimited => "verifier_contract_limited",
            Self::AmbiguousAuthority => "ambiguous_authority",
            Self::InsufficientEvidence => "insufficient_evidence",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContractConflictAction {
    EditSourceForDiagnostic,
    AlignTestAndVerifier,
    ReplaceInvalidVerifierCommand,
    StopWithStructuredEvidence,
}

impl ContractConflictAction {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::EditSourceForDiagnostic => "edit_source_for_diagnostic",
            Self::AlignTestAndVerifier => "align_test_and_verifier",
            Self::ReplaceInvalidVerifierCommand => "replace_invalid_verifier_command",
            Self::StopWithStructuredEvidence => "stop_with_structured_evidence",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContractConflictResolution {
    pub(crate) status: &'static str,
    pub(crate) sides: Vec<ConflictSideKind>,
    pub(crate) authority: ContractConflictAuthority,
    pub(crate) repair_target_side: Option<ConflictSideKind>,
    pub(crate) selected_action: ContractConflictAction,
    pub(crate) safe_stop_reason: Option<&'static str>,
    pub(crate) missing_evidence: Vec<&'static str>,
    pub(crate) source_of_truth: String,
}

impl ContractConflictResolution {
    pub(crate) fn from_evidence(evidence: &ContractEvidence) -> Option<Self> {
        if !is_contract_conflict(evidence) {
            return None;
        }

        let sides = inferred_sides(evidence);
        let text = evidence_text(evidence);
        let source_of_truth = source_of_truth_hint(evidence);
        let authority = classify_authority(evidence, &text, &source_of_truth);
        let (repair_target_side, selected_action, safe_stop_reason, missing_evidence, status) =
            match authority {
                ContractConflictAuthority::ImplementationAuthoritative => (
                    Some(ConflictSideKind::Test),
                    ContractConflictAction::AlignTestAndVerifier,
                    None,
                    Vec::new(),
                    "resolved",
                ),
                ContractConflictAuthority::TestAuthoritative => (
                    Some(ConflictSideKind::Implementation),
                    ContractConflictAction::EditSourceForDiagnostic,
                    None,
                    Vec::new(),
                    "resolved",
                ),
                ContractConflictAuthority::DocsOrApiAuthoritative => (
                    Some(ConflictSideKind::Implementation),
                    ContractConflictAction::EditSourceForDiagnostic,
                    None,
                    Vec::new(),
                    "resolved",
                ),
                ContractConflictAuthority::VerifierContractLimited => (
                    Some(ConflictSideKind::VerifierContract),
                    ContractConflictAction::ReplaceInvalidVerifierCommand,
                    None,
                    Vec::new(),
                    "resolved",
                ),
                ContractConflictAuthority::AmbiguousAuthority => (
                    None,
                    ContractConflictAction::StopWithStructuredEvidence,
                    Some("ambiguous_contract_authority"),
                    Vec::new(),
                    "safe_stop",
                ),
                ContractConflictAuthority::InsufficientEvidence => (
                    None,
                    ContractConflictAction::StopWithStructuredEvidence,
                    Some("contract_conflict_missing_authority_evidence"),
                    vec!["source_of_truth", "conflict_sides"],
                    "safe_stop",
                ),
            };

        Some(Self {
            status,
            sides,
            authority,
            repair_target_side,
            selected_action,
            safe_stop_reason,
            missing_evidence,
            source_of_truth,
        })
    }

    pub(crate) fn is_safe_stop(&self) -> bool {
        self.safe_stop_reason.is_some()
    }

    pub(crate) fn eval_report_fields(&self) -> Vec<String> {
        vec![
            format!("contract_conflict_status={}", self.status),
            format!("contract_conflict_sides={}", self.render_sides()),
            format!("contract_conflict_authority={}", self.authority.as_str()),
            format!(
                "contract_conflict_repair_target_side={}",
                self.repair_target_side
                    .map(ConflictSideKind::as_str)
                    .unwrap_or("none")
            ),
            format!(
                "contract_conflict_selected_action={}",
                self.selected_action.as_str()
            ),
            format!(
                "contract_conflict_safe_stop_reason={}",
                self.safe_stop_reason.unwrap_or("none")
            ),
            format!(
                "contract_conflict_missing_evidence={}",
                if self.missing_evidence.is_empty() {
                    "none".to_string()
                } else {
                    self.missing_evidence.join("|")
                }
            ),
            format!("contract_conflict_source_of_truth={}", self.source_of_truth),
        ]
    }

    pub(crate) fn render_lines(&self) -> Vec<String> {
        vec![format!(
            "contract_conflict status={} sides={} authority={} repair_target_side={} selected_action={} safe_stop_reason={} missing_evidence={} source_of_truth={}",
            self.status,
            self.render_sides(),
            self.authority.as_str(),
            self.repair_target_side
                .map(ConflictSideKind::as_str)
                .unwrap_or("none"),
            self.selected_action.as_str(),
            self.safe_stop_reason.unwrap_or("none"),
            if self.missing_evidence.is_empty() {
                "none".to_string()
            } else {
                self.missing_evidence.join("|")
            },
            self.source_of_truth
        )]
    }

    fn render_sides(&self) -> String {
        if self.sides.is_empty() {
            "unknown".to_string()
        } else {
            self.sides
                .iter()
                .map(|side| side.as_str())
                .collect::<Vec<_>>()
                .join("|")
        }
    }
}

fn is_contract_conflict(evidence: &ContractEvidence) -> bool {
    [
        evidence.diagnostic_code.as_deref(),
        evidence.reason_code.as_deref(),
        evidence.violated_contract.as_deref(),
        evidence.failure_kind.as_deref(),
        evidence.semantic_failure_kind.as_deref(),
        evidence.explicit_stop_reason.as_deref(),
        evidence.no_progress_strategy.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|value| value.contains("contract_conflict"))
        || evidence
            .verifier_diagnostic_payload
            .iter()
            .chain(evidence.semantic_failure_report.iter())
            .chain(evidence.eval_report_fields.iter())
            .any(|line| line.contains("contract_conflict"))
}

fn inferred_sides(evidence: &ContractEvidence) -> Vec<ConflictSideKind> {
    let mut sides = Vec::new();
    if let Some(value) = eval_field(evidence, "contract_conflict_sides") {
        for token in value.split('|') {
            push_side_token(&mut sides, token);
        }
    }
    for path in evidence
        .candidate_artifacts
        .iter()
        .chain(evidence.required_paths.iter())
        .chain(evidence.missing_paths.iter())
        .chain(evidence.target_path.iter())
        .chain(evidence.repair_target.iter())
    {
        push_role_side(&mut sides, role_for_path(path, ArtifactLifecycle::Required));
    }
    if evidence.weak_verifier_reason.is_some()
        || contains_any(
            &evidence_text(evidence),
            &["verifier_contract", "weak_verifier"],
        )
    {
        push_unique_side(&mut sides, ConflictSideKind::VerifierContract);
    }
    sides
}

fn push_side_token(sides: &mut Vec<ConflictSideKind>, token: &str) {
    match compact_token(token).as_str() {
        "implementation" | "source" | "source_impl" => {
            push_unique_side(sides, ConflictSideKind::Implementation)
        }
        "test" | "tests" | "generated_test" | "preexisting_test" => {
            push_unique_side(sides, ConflictSideKind::Test)
        }
        "docs" | "doc" | "api" | "schema" | "docs_or_api" => {
            push_unique_side(sides, ConflictSideKind::DocsOrApi)
        }
        "verifier" | "verifier_contract" | "weak_verifier" => {
            push_unique_side(sides, ConflictSideKind::VerifierContract)
        }
        _ => {}
    }
}

fn push_role_side(sides: &mut Vec<ConflictSideKind>, role: ArtifactRole) {
    match role {
        ArtifactRole::Entrypoint
        | ArtifactRole::IntegrationTarget
        | ArtifactRole::Implementation => push_unique_side(sides, ConflictSideKind::Implementation),
        ArtifactRole::Test => push_unique_side(sides, ConflictSideKind::Test),
        ArtifactRole::Docs => push_unique_side(sides, ConflictSideKind::DocsOrApi),
        _ => {}
    }
}

fn push_unique_side(sides: &mut Vec<ConflictSideKind>, side: ConflictSideKind) {
    if !sides.contains(&side) {
        sides.push(side);
    }
}

fn classify_authority(
    evidence: &ContractEvidence,
    text: &str,
    source_of_truth: &str,
) -> ContractConflictAuthority {
    if let Some(authority) = eval_field(evidence, "contract_conflict_authority") {
        match authority.as_str() {
            "implementation_authoritative" => {
                return ContractConflictAuthority::ImplementationAuthoritative;
            }
            "test_authoritative" => return ContractConflictAuthority::TestAuthoritative,
            "docs_or_api_authoritative" => {
                return ContractConflictAuthority::DocsOrApiAuthoritative;
            }
            "verifier_contract_limited" => {
                return ContractConflictAuthority::VerifierContractLimited;
            }
            "ambiguous_authority" => return ContractConflictAuthority::AmbiguousAuthority,
            "insufficient_evidence" => return ContractConflictAuthority::InsufficientEvidence,
            _ => {}
        }
    }

    if evidence.weak_verifier_reason.is_some()
        || contains_any(
            text,
            &[
                "weak_verifier",
                "self_referential_verifier",
                "verifier_contract_limited",
                "invalid_verifier",
            ],
        )
    {
        return ContractConflictAuthority::VerifierContractLimited;
    }
    if contains_any(
        text,
        &[
            "generated_test_not_authoritative",
            "generated_test",
            "test_generated_from_source",
        ],
    ) {
        return ContractConflictAuthority::ImplementationAuthoritative;
    }
    if contains_any(
        text,
        &[
            "preexisting_test",
            "test_contract",
            "original_verifier",
            "golden_test",
        ],
    ) {
        return ContractConflictAuthority::TestAuthoritative;
    }
    if contains_any(
        text,
        &[
            "docs_api_contract",
            "docs_or_api_authoritative",
            "documentation_contract",
            "api_contract",
            "schema_contract",
            "profile_contract",
            "user_contract",
        ],
    ) {
        return ContractConflictAuthority::DocsOrApiAuthoritative;
    }
    if contains_any(text, &["ambiguous_authority", "conflicting_authority"]) {
        return ContractConflictAuthority::AmbiguousAuthority;
    }
    if source_of_truth != "unknown" {
        return match source_of_truth {
            "verifier_contract" => ContractConflictAuthority::VerifierContractLimited,
            "generated_test_not_authoritative" => {
                ContractConflictAuthority::ImplementationAuthoritative
            }
            "test_contract_and_original_verifier" | "original_verifier_diagnostic" => {
                ContractConflictAuthority::TestAuthoritative
            }
            "documentation_contract" | "profile_contract" => {
                ContractConflictAuthority::DocsOrApiAuthoritative
            }
            _ => ContractConflictAuthority::AmbiguousAuthority,
        };
    }
    ContractConflictAuthority::InsufficientEvidence
}

fn source_of_truth_hint(evidence: &ContractEvidence) -> String {
    if let Some(value) = eval_field(evidence, "contract_conflict_source_of_truth") {
        return value;
    }
    if let Some(source) = &evidence.source_of_truth {
        return source.clone();
    }
    let text = evidence_text(evidence);
    if contains_any(
        &text,
        &["generated_test_not_authoritative", "generated_test"],
    ) {
        "generated_test_not_authoritative".to_string()
    } else if contains_any(&text, &["preexisting_test", "test_contract"]) {
        "test_contract_and_original_verifier".to_string()
    } else if contains_any(
        &text,
        &["docs_api_contract", "api_contract", "schema_contract"],
    ) {
        "docs_api_contract".to_string()
    } else if contains_any(&text, &["weak_verifier", "invalid_verifier"]) {
        "verifier_contract".to_string()
    } else {
        "unknown".to_string()
    }
}

fn evidence_text(evidence: &ContractEvidence) -> String {
    let mut values = Vec::new();
    for value in [
        evidence.guard.as_str(),
        evidence.diagnostic_code.as_deref().unwrap_or(""),
        evidence.reason_code.as_deref().unwrap_or(""),
        evidence.violated_contract.as_deref().unwrap_or(""),
        evidence.failure_kind.as_deref().unwrap_or(""),
        evidence.semantic_failure_kind.as_deref().unwrap_or(""),
        evidence.source_of_truth.as_deref().unwrap_or(""),
        evidence.explicit_stop_reason.as_deref().unwrap_or(""),
        evidence.weak_verifier_reason.as_deref().unwrap_or(""),
        evidence.diagnostic.as_deref().unwrap_or(""),
    ] {
        if !value.is_empty() {
            values.push(value.to_string());
        }
    }
    values.extend(evidence.verifier_diagnostic_payload.iter().cloned());
    values.extend(evidence.semantic_failure_report.iter().cloned());
    values.extend(evidence.eval_report_fields.iter().cloned());
    values
        .join(" ")
        .to_ascii_lowercase()
        .replace([' ', '-'], "_")
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn eval_field(evidence: &ContractEvidence, key: &str) -> Option<String> {
    let marker = format!("{key}=");
    evidence
        .eval_report_fields
        .iter()
        .rev()
        .chain(evidence.verifier_diagnostic_payload.iter().rev())
        .chain(evidence.semantic_failure_report.iter().rev())
        .find_map(|line| {
            let (_, rest) = line.split_once(&marker)?;
            let value = rest
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .trim_matches(|ch| matches!(ch, ',' | ';'))
                .to_string();
            (!value.trim().is_empty()).then_some(value)
        })
}

fn compact_token(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace([' ', '-'], "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conflict_evidence(source: &str) -> ContractEvidence {
        ContractEvidence::new("verifier")
            .with_diagnostic_code("contract_conflict")
            .with_source_of_truth(source)
    }

    #[test]
    fn generated_test_conflict_repairs_test_side() {
        let evidence = conflict_evidence("generated_test_not_authoritative")
            .with_candidate_artifacts(["src/app/page.tsx", "tests/generated.test.ts"]);

        let resolution = ContractConflictResolution::from_evidence(&evidence).unwrap();

        assert_eq!(
            resolution.authority,
            ContractConflictAuthority::ImplementationAuthoritative
        );
        assert_eq!(resolution.repair_target_side, Some(ConflictSideKind::Test));
        assert_eq!(
            resolution.selected_action,
            ContractConflictAction::AlignTestAndVerifier
        );
        assert_eq!(resolution.status, "resolved");
    }

    #[test]
    fn preexisting_test_conflict_repairs_source_side() {
        let evidence = conflict_evidence("test_contract_and_original_verifier")
            .with_candidate_artifacts(["src/lib.rs", "tests/regression_test.rs"]);

        let resolution = ContractConflictResolution::from_evidence(&evidence).unwrap();

        assert_eq!(
            resolution.authority,
            ContractConflictAuthority::TestAuthoritative
        );
        assert_eq!(
            resolution.repair_target_side,
            Some(ConflictSideKind::Implementation)
        );
        assert_eq!(
            resolution.selected_action,
            ContractConflictAction::EditSourceForDiagnostic
        );
    }

    #[test]
    fn docs_api_conflict_repairs_source_side() {
        let evidence = conflict_evidence("docs_api_contract")
            .with_candidate_artifacts(["src/client.ts", "docs/api.md"]);

        let resolution = ContractConflictResolution::from_evidence(&evidence).unwrap();

        assert_eq!(
            resolution.authority,
            ContractConflictAuthority::DocsOrApiAuthoritative
        );
        assert_eq!(
            resolution.repair_target_side,
            Some(ConflictSideKind::Implementation)
        );
    }

    #[test]
    fn weak_verifier_conflict_repairs_verifier_contract() {
        let evidence = ContractEvidence::new("verifier")
            .with_diagnostic_code("contract_conflict")
            .with_weak_verifier_reason("self_referential_verifier");

        let resolution = ContractConflictResolution::from_evidence(&evidence).unwrap();

        assert_eq!(
            resolution.authority,
            ContractConflictAuthority::VerifierContractLimited
        );
        assert_eq!(
            resolution.repair_target_side,
            Some(ConflictSideKind::VerifierContract)
        );
        assert_eq!(
            resolution.selected_action,
            ContractConflictAction::ReplaceInvalidVerifierCommand
        );
    }

    #[test]
    fn ambiguous_conflict_safe_stops() {
        let evidence = ContractEvidence::new("verifier")
            .with_diagnostic_code("contract_conflict")
            .with_eval_report_fields(["contract_conflict_authority=ambiguous_authority"]);

        let resolution = ContractConflictResolution::from_evidence(&evidence).unwrap();

        assert_eq!(
            resolution.authority,
            ContractConflictAuthority::AmbiguousAuthority
        );
        assert!(resolution.is_safe_stop());
        assert_eq!(
            resolution.selected_action,
            ContractConflictAction::StopWithStructuredEvidence
        );
    }
}
