#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RequirementStatus {
    Pass,
    Failed,
    Unverified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RequirementKind {
    Capability,
    Evidence,
    Obligation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RequirementOutcome {
    pub(crate) kind: RequirementKind,
    pub(crate) requirement: String,
    pub(crate) status: RequirementStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RequirementEvaluation {
    pub(crate) outcomes: Vec<RequirementOutcome>,
    pub(crate) passed: bool,
    pub(crate) inconclusive: bool,
    pub(crate) primary_reason: String,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate_requirements(
    required_capabilities: &[String],
    required_evidence: &[String],
    required_obligations: &[String],
    missing_capabilities: &[String],
    missing_evidence: &[String],
    missing_obligations: &[String],
    unverified_evidence: &[String],
    inconclusive_reasons: &[String],
    weak_evidence: &[String],
    weak_evidence_blocks_completion: bool,
) -> RequirementEvaluation {
    let mut outcomes = required_capabilities
        .iter()
        .map(|requirement| RequirementOutcome {
            kind: RequirementKind::Capability,
            requirement: requirement.clone(),
            status: if missing_capabilities.contains(requirement) {
                RequirementStatus::Failed
            } else {
                RequirementStatus::Pass
            },
        })
        .collect::<Vec<_>>();
    outcomes.extend(required_evidence.iter().map(|requirement| {
        let unverified_prefix = format!("{requirement}:");
        RequirementOutcome {
            kind: RequirementKind::Evidence,
            requirement: requirement.clone(),
            status: if missing_evidence.contains(requirement) {
                RequirementStatus::Failed
            } else if unverified_evidence
                .iter()
                .any(|evidence| evidence.starts_with(&unverified_prefix))
            {
                RequirementStatus::Unverified
            } else {
                RequirementStatus::Pass
            },
        }
    }));
    outcomes.extend(required_obligations.iter().map(|requirement| {
        let normalized = requirement.trim().to_ascii_lowercase().replace('-', "_");
        RequirementOutcome {
            kind: RequirementKind::Obligation,
            requirement: requirement.clone(),
            status: if missing_obligations.contains(&normalized) {
                RequirementStatus::Failed
            } else {
                RequirementStatus::Pass
            },
        }
    }));
    let inconclusive = !inconclusive_reasons.is_empty();
    let passed = missing_capabilities.is_empty()
        && missing_evidence.is_empty()
        && missing_obligations.is_empty()
        && !inconclusive
        && !weak_evidence_blocks_completion;
    let primary_reason = if let Some(reason) = missing_capabilities.first() {
        format!("missing_required_capabilities:{reason}")
    } else if let Some(reason) = missing_evidence.first() {
        format!("missing_required_evidence:{reason}")
    } else if let Some(reason) = missing_obligations.first() {
        format!("missing_required_obligations:{reason}")
    } else if let Some(reason) = inconclusive_reasons.first() {
        format!("inconclusive_acceptance:{reason}")
    } else if let Some(reason) = weak_evidence.first() {
        format!("weak_verification_evidence:{reason}")
    } else {
        "pass".to_string()
    };
    RequirementEvaluation {
        outcomes,
        passed,
        inconclusive,
        primary_reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declared_requirements_keep_pass_failed_and_unverified_outcomes() {
        let evaluation = evaluate_requirements(
            &["browser_interaction".to_string()],
            &["build_oracle".to_string(), "interaction_probe".to_string()],
            &["implementation".to_string()],
            &[],
            &["build_oracle".to_string()],
            &[],
            &["interaction_probe:unverified:probe_unavailable".to_string()],
            &[],
            &[],
            false,
        );

        assert_eq!(
            evaluation.outcomes,
            vec![
                RequirementOutcome {
                    kind: RequirementKind::Capability,
                    requirement: "browser_interaction".to_string(),
                    status: RequirementStatus::Pass,
                },
                RequirementOutcome {
                    kind: RequirementKind::Evidence,
                    requirement: "build_oracle".to_string(),
                    status: RequirementStatus::Failed,
                },
                RequirementOutcome {
                    kind: RequirementKind::Evidence,
                    requirement: "interaction_probe".to_string(),
                    status: RequirementStatus::Unverified,
                },
                RequirementOutcome {
                    kind: RequirementKind::Obligation,
                    requirement: "implementation".to_string(),
                    status: RequirementStatus::Pass,
                },
            ]
        );
        assert!(!evaluation.passed);
        assert_eq!(
            evaluation.primary_reason,
            "missing_required_evidence:build_oracle"
        );
    }

    #[test]
    fn requirement_aggregation_preserves_primary_reason_precedence() {
        let evaluation = evaluate_requirements(
            &[],
            &[],
            &[],
            &["capability".to_string()],
            &["evidence".to_string()],
            &["obligation".to_string()],
            &[],
            &["probe unavailable".to_string()],
            &["weak source".to_string()],
            true,
        );

        assert!(!evaluation.passed);
        assert!(evaluation.inconclusive);
        assert_eq!(
            evaluation.primary_reason,
            "missing_required_capabilities:capability"
        );
    }
}
