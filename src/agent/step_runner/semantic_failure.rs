//! Semantic failure reports derived from deterministic facts.
#![allow(dead_code)]

use crate::agent::step_runner::correction_evidence::ContractEvidence;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SemanticFailureReport {
    pub(crate) kind: String,
    pub(crate) clusters: Vec<String>,
    pub(crate) observed_expected: Vec<String>,
    pub(crate) affected_cases: Vec<String>,
    pub(crate) contract_conflict: Option<String>,
    pub(crate) preferred_repair_role: Option<String>,
    pub(crate) proposed_targets: Vec<String>,
    pub(crate) admitted_target: Option<String>,
}

impl SemanticFailureReport {
    pub(crate) fn from_contract_evidence(evidence: &ContractEvidence) -> Self {
        let kind = evidence
            .semantic_failure_kind
            .clone()
            .or_else(|| evidence.failure_kind.clone())
            .or_else(|| evidence.reason_code.clone())
            .unwrap_or_else(|| "contract_failure".to_string());
        let mut proposed_targets = Vec::new();
        push_unique_opt(&mut proposed_targets, evidence.repair_target.clone());
        push_unique_opt(&mut proposed_targets, evidence.target_path.clone());
        for target in evidence
            .candidate_artifacts
            .iter()
            .chain(evidence.required_paths.iter())
            .chain(evidence.missing_paths.iter())
        {
            push_unique(&mut proposed_targets, target.clone());
        }
        Self {
            kind,
            clusters: cluster_labels(evidence),
            observed_expected: evidence.observed_expected_pairs.clone(),
            affected_cases: evidence.affected_cases.clone(),
            contract_conflict: contract_conflict(evidence),
            preferred_repair_role: preferred_repair_role(evidence),
            proposed_targets,
            admitted_target: evidence
                .repair_target
                .clone()
                .or_else(|| evidence.target_path.clone()),
        }
    }

    pub(crate) fn render_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push(format!("kind={}", compact(&self.kind)));
        if !self.clusters.is_empty() {
            lines.push(format!("clusters={}", self.clusters.join("|")));
        }
        if !self.observed_expected.is_empty() {
            lines.push(format!(
                "observed_expected={}",
                self.observed_expected.join("|")
            ));
        }
        if !self.affected_cases.is_empty() {
            lines.push(format!("affected_cases={}", self.affected_cases.join("|")));
        }
        if let Some(conflict) = &self.contract_conflict {
            lines.push(format!("contract_conflict={}", compact(conflict)));
        }
        if let Some(role) = &self.preferred_repair_role {
            lines.push(format!("preferred_repair_role={}", compact(role)));
        }
        if !self.proposed_targets.is_empty() {
            lines.push(format!(
                "proposed_targets={}",
                self.proposed_targets.join("|")
            ));
        }
        if let Some(target) = &self.admitted_target {
            lines.push(format!("admitted_target={}", compact(target)));
        }
        lines
    }
}

fn cluster_labels(evidence: &ContractEvidence) -> Vec<String> {
    let mut labels = Vec::new();
    if evidence.command.is_some() {
        push_unique(&mut labels, "verifier_command".to_string());
    }
    if !evidence.missing_paths.is_empty() || !evidence.required_paths.is_empty() {
        push_unique(&mut labels, "artifact_path".to_string());
    }
    if !evidence.missing_literals.is_empty() || !evidence.required_literals.is_empty() {
        push_unique(&mut labels, "literal_contract".to_string());
    }
    if evidence
        .target_admission
        .as_deref()
        .is_some_and(|value| value.starts_with("rejected"))
    {
        push_unique(&mut labels, "target_admission".to_string());
    }
    labels
}

fn contract_conflict(evidence: &ContractEvidence) -> Option<String> {
    evidence
        .target_admission
        .as_deref()
        .filter(|value| value.starts_with("rejected"))
        .map(str::to_string)
        .or_else(|| {
            evidence
                .explicit_stop_reason
                .as_deref()
                .map(|reason| format!("explicit_stop: {reason}"))
        })
}

fn preferred_repair_role(evidence: &ContractEvidence) -> Option<String> {
    let code = evidence
        .semantic_failure_kind
        .as_deref()
        .or(evidence.failure_kind.as_deref())
        .or(evidence.reason_code.as_deref())
        .unwrap_or_default();
    if code.contains("generated_test")
        || code.contains("test_assertion")
        || evidence.source_of_truth.as_deref() == Some("generated_test")
    {
        return Some("test".to_string());
    }
    if code.contains("assertion_mismatch")
        && matches!(
            evidence.source_of_truth.as_deref(),
            Some("user_contract") | Some("profile_contract")
        )
    {
        return Some("implementation".to_string());
    }
    evidence.artifact_role.clone()
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !value.trim().is_empty() && !values.contains(&value) {
        values.push(value);
    }
}

fn push_unique_opt(values: &mut Vec<String>, value: Option<String>) {
    if let Some(value) = value {
        push_unique(values, value);
    }
}

fn compact(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_report_prefers_repair_target_and_role() {
        let evidence = ContractEvidence::new("profile_verification")
            .with_semantic_failure_kind("route_integration_failure")
            .with_repair_target("app/page.tsx")
            .with_artifact_role("entrypoint")
            .with_observed_expected_pairs(["observed=no import expected=Game import"]);

        let report = SemanticFailureReport::from_contract_evidence(&evidence);

        assert_eq!(report.admitted_target.as_deref(), Some("app/page.tsx"));
        assert_eq!(report.preferred_repair_role.as_deref(), Some("entrypoint"));
        assert!(
            report
                .render_lines()
                .iter()
                .any(|line| line.contains("route_integration_failure"))
        );
    }

    #[test]
    fn assertion_mismatch_prefers_implementation_when_user_contract_is_truth() {
        let evidence = ContractEvidence::new("verifier")
            .with_semantic_failure_kind("assertion_mismatch")
            .with_source_of_truth("user_contract")
            .with_artifact_role("test")
            .with_observed_expected_pairs(["observed=4 expected=5"]);

        let report = SemanticFailureReport::from_contract_evidence(&evidence);

        assert_eq!(
            report.preferred_repair_role.as_deref(),
            Some("implementation")
        );
    }

    #[test]
    fn generated_test_bug_prefers_test_target() {
        let evidence = ContractEvidence::new("verifier")
            .with_semantic_failure_kind("generated_test_assertion_mismatch")
            .with_source_of_truth("generated_test")
            .with_artifact_role("implementation")
            .with_target_path("tests/test_app.py");

        let report = SemanticFailureReport::from_contract_evidence(&evidence);

        assert_eq!(report.preferred_repair_role.as_deref(), Some("test"));
        assert!(
            report
                .render_lines()
                .iter()
                .any(|line| line.contains("preferred_repair_role=test"))
        );
    }
}
