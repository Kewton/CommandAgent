//! Semantic failure reports derived from deterministic facts.
#![allow(dead_code)]

use crate::agent::step_runner::correction_evidence::ContractEvidence;

const DEFAULT_CONFIDENCE: &str = "deterministic";

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SemanticFailureCluster {
    pub(crate) cluster_id: String,
    pub(crate) failure_kind: String,
    pub(crate) observed_expected: Vec<String>,
    pub(crate) affected_cases: Vec<String>,
    pub(crate) source_of_truth: String,
    pub(crate) contract_conflict: Option<String>,
    pub(crate) preferred_repair_role: Option<String>,
    pub(crate) candidate_targets: Vec<String>,
}

impl SemanticFailureCluster {
    pub(crate) fn render_line(&self) -> String {
        let conflict = self
            .contract_conflict
            .as_deref()
            .map(|value| format!(" conflict={}", compact(value)))
            .unwrap_or_default();
        format!(
            "cluster={} kind={} source_of_truth={} preferred_role={} targets={}{}",
            compact(&self.cluster_id),
            compact(&self.failure_kind),
            compact(&self.source_of_truth),
            self.preferred_repair_role.as_deref().unwrap_or("unknown"),
            self.candidate_targets.join("|"),
            conflict
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SemanticRepairPlan {
    pub(crate) selected_cluster: SemanticFailureCluster,
    pub(crate) selected_target: Option<String>,
    pub(crate) repair_hypothesis: String,
    pub(crate) expected_improvement: String,
    pub(crate) confidence: String,
}

impl SemanticRepairPlan {
    pub(crate) fn from_contract_evidence(evidence: &ContractEvidence) -> Self {
        let report = SemanticFailureReport::from_contract_evidence(evidence);
        let cluster = SemanticFailureCluster {
            cluster_id: selected_cluster_id(&report),
            failure_kind: report.kind.clone(),
            observed_expected: report.observed_expected.clone(),
            affected_cases: report.affected_cases.clone(),
            source_of_truth: evidence
                .source_of_truth
                .clone()
                .unwrap_or_else(|| "deterministic_contract".to_string()),
            contract_conflict: report.contract_conflict.clone(),
            preferred_repair_role: report.preferred_repair_role.clone(),
            candidate_targets: report.proposed_targets.clone(),
        };
        let selected_target = report.admitted_target.clone();
        let repair_hypothesis = repair_hypothesis(&cluster, selected_target.as_deref());
        let expected_improvement = expected_improvement(&cluster);
        Self {
            selected_cluster: cluster,
            selected_target,
            repair_hypothesis,
            expected_improvement,
            confidence: DEFAULT_CONFIDENCE.to_string(),
        }
    }

    pub(crate) fn selected_cluster_label(&self) -> String {
        format!(
            "{}:{}",
            compact(&self.selected_cluster.cluster_id),
            compact(&self.selected_cluster.failure_kind)
        )
    }

    pub(crate) fn render_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("selected_cluster={}", self.selected_cluster_label()),
            self.selected_cluster.render_line(),
            format!("repair_hypothesis={}", compact(&self.repair_hypothesis)),
            format!(
                "expected_improvement={}",
                compact(&self.expected_improvement)
            ),
            format!("confidence={}", compact(&self.confidence)),
        ];
        if let Some(target) = &self.selected_target {
            lines.push(format!("selected_target={}", compact(target)));
        }
        lines
    }
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

fn selected_cluster_id(report: &SemanticFailureReport) -> String {
    report
        .clusters
        .first()
        .cloned()
        .unwrap_or_else(|| "deterministic_failure".to_string())
}

fn repair_hypothesis(cluster: &SemanticFailureCluster, selected_target: Option<&str>) -> String {
    let target = selected_target.unwrap_or("no admitted target");
    if cluster.contract_conflict.is_some() {
        return format!("stop and preserve conflict evidence for {target}");
    }
    match cluster.preferred_repair_role.as_deref() {
        Some("test") => format!("repair test artifact or assertion contract at {target}"),
        Some("implementation") => format!("repair implementation behavior at {target}"),
        Some(role) => format!("repair {role} target {target}"),
        None => format!("repair selected deterministic target {target}"),
    }
}

fn expected_improvement(cluster: &SemanticFailureCluster) -> String {
    if !cluster.observed_expected.is_empty() {
        return "observed/expected pair moves toward expected value".to_string();
    }
    if !cluster.affected_cases.is_empty() {
        return "affected case should pass original guard or verifier".to_string();
    }
    if cluster.contract_conflict.is_some() {
        return "runtime stops with structured conflict instead of wrong-target repair".to_string();
    }
    "original guard or verifier should pass after bounded repair".to_string()
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

    #[test]
    fn semantic_repair_plan_renders_selected_cluster_and_target() {
        let evidence = ContractEvidence::new("profile_verification")
            .with_semantic_failure_kind("nextjs_route_not_integrated")
            .with_source_of_truth("profile_contract")
            .with_repair_target("app/page.tsx")
            .with_candidate_artifacts(["app/page.tsx", "components/Game.tsx"])
            .with_artifact_role("entrypoint");

        let plan = SemanticRepairPlan::from_contract_evidence(&evidence);

        assert!(
            plan.render_lines()
                .iter()
                .any(|line| line.contains("selected_cluster"))
        );
        assert_eq!(plan.selected_target.as_deref(), Some("app/page.tsx"));
        assert_eq!(plan.confidence, "deterministic");
    }
}
