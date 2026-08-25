//! Claim-level shadow evaluation for create-oracle proposals.
//!
//! This module compares a proposal with reviewed gold bindings. It never
//! executes a proposed oracle and never contributes to an authoritative
//! verification or release decision.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{
    EvidenceStrength, ExpectedPolarity, Oracle, OracleInput, OracleObservation, OracleResult,
    OracleStrategy, ShadowGeneration,
};
use crate::tools::path_guard::validate_workspace_relative;

pub const COVERAGE_SCHEMA_VERSION: &str =
    "commandagent.verification_spec.create_shadow_coverage.v0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateGoldClaim {
    pub id: String,
    pub required: bool,
    pub minimum_strength: EvidenceStrength,
    pub bindings: Vec<CreateGoldBinding>,
    pub unsupported_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateGoldBinding {
    pub accepted_strategies: Vec<OracleStrategy>,
    pub expected_polarity: ExpectedPolarity,
    pub input: OracleInput,
    pub observation: OracleObservation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OracleExecutionEvidence {
    pub oracle_id: String,
    pub observed_strength: EvidenceStrength,
    pub outcome: OracleResult,
    pub evidence_path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageGenerationStatus {
    Generated,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CreateShadowCoverageReport {
    pub schema_version: &'static str,
    pub shadow_only: bool,
    pub authoritative_verdict_changed: bool,
    pub generation_status: CoverageGenerationStatus,
    pub all_required_passed: bool,
    pub claims: Vec<CreateClaimCoverage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CreateClaimCoverage {
    pub claim_id: String,
    pub required: bool,
    pub strategy: Vec<OracleStrategy>,
    pub strength: Option<EvidenceStrength>,
    pub executed: bool,
    pub outcome: OracleResult,
    pub evidence_path: Vec<String>,
    pub oracle_ids: Vec<String>,
    pub unverified_reason: Option<String>,
}

/// Compare a create proposal with reviewed gold bindings and caller-supplied
/// execution evidence. Provider lifecycle/result fields are deliberately not
/// used as proof that an oracle ran.
pub fn evaluate_create_shadow(
    gold_claims: &[CreateGoldClaim],
    generation: &ShadowGeneration,
    execution: &[OracleExecutionEvidence],
) -> CreateShadowCoverageReport {
    let generation_status = match generation {
        ShadowGeneration::Generated(_) => CoverageGenerationStatus::Generated,
        ShadowGeneration::Rejected(_) => CoverageGenerationStatus::Rejected,
    };
    let evidence = execution_by_oracle(execution);
    let claims = gold_claims
        .iter()
        .map(|gold| evaluate_claim(gold, generation, &evidence))
        .collect::<Vec<_>>();
    let required_claims = claims.iter().filter(|claim| claim.required);
    let all_required_passed = required_claims.clone().next().is_some()
        && required_claims.clone().all(|claim| {
            claim.executed
                && claim.outcome == OracleResult::Pass
                && claim.unverified_reason.is_none()
        });
    CreateShadowCoverageReport {
        schema_version: COVERAGE_SCHEMA_VERSION,
        shadow_only: true,
        authoritative_verdict_changed: false,
        generation_status,
        all_required_passed,
        claims,
    }
}

fn evaluate_claim(
    gold: &CreateGoldClaim,
    generation: &ShadowGeneration,
    evidence: &BTreeMap<&str, Option<&OracleExecutionEvidence>>,
) -> CreateClaimCoverage {
    let unverified = |reason: String| CreateClaimCoverage {
        claim_id: gold.id.clone(),
        required: gold.required,
        strategy: Vec::new(),
        strength: None,
        executed: false,
        outcome: OracleResult::Unverified,
        evidence_path: Vec::new(),
        oracle_ids: Vec::new(),
        unverified_reason: Some(reason),
    };
    if let Some(reason) = &gold.unsupported_reason {
        return unverified(format!("unsupported:{reason}"));
    }
    if gold.bindings.is_empty() {
        return unverified("gold_bindings_absent".to_string());
    }
    let ShadowGeneration::Generated(spec) = generation else {
        let ShadowGeneration::Rejected(failure) = generation else {
            unreachable!()
        };
        return unverified(format!(
            "generation_rejected:{}:{}",
            failure_kind_name(failure.kind),
            failure.error
        ));
    };
    let Some(claim) = spec.claims.iter().find(|claim| claim.id == gold.id) else {
        return unverified("claim_missing".to_string());
    };
    let candidates = claim
        .oracle_ids
        .iter()
        .filter_map(|id| spec.oracles.iter().find(|oracle| &oracle.id == id))
        .collect::<Vec<_>>();
    let mut used = BTreeSet::new();
    let mut matched = Vec::with_capacity(gold.bindings.len());
    for (index, binding) in gold.bindings.iter().enumerate() {
        let shape_matches = candidates.iter().copied().filter(|oracle| {
            !used.contains(oracle.id.as_str()) && oracle_matches_binding(oracle, binding)
        });
        let mut policy_rejection = None;
        let mut selected = None;
        for oracle in shape_matches {
            match validate_candidate_policy(oracle) {
                Ok(()) => {
                    selected = Some(oracle);
                    break;
                }
                Err(reason) => policy_rejection = Some(reason),
            }
        }
        let Some(oracle) = selected else {
            return unverified(policy_rejection.map_or_else(
                || format!("binding_missing:{index}"),
                |reason| format!("policy_rejected:{reason}"),
            ));
        };
        if strength_rank(oracle.minimum_strength) < strength_rank(gold.minimum_strength) {
            return unverified(format!("proposal_under_strength:{}", oracle.id));
        }
        used.insert(oracle.id.as_str());
        matched.push(oracle);
    }

    let strategy = matched.iter().map(|oracle| oracle.strategy).collect();
    let oracle_ids = matched
        .iter()
        .map(|oracle| oracle.id.clone())
        .collect::<Vec<_>>();
    let mut paths = Vec::with_capacity(matched.len());
    let mut strengths = Vec::with_capacity(matched.len());
    let mut outcomes = Vec::with_capacity(matched.len());
    for oracle in &matched {
        let Some(Some(observed)) = evidence.get(oracle.id.as_str()) else {
            return CreateClaimCoverage {
                claim_id: gold.id.clone(),
                required: gold.required,
                strategy,
                strength: None,
                executed: false,
                outcome: OracleResult::Unverified,
                evidence_path: paths,
                oracle_ids,
                unverified_reason: Some(if evidence.contains_key(oracle.id.as_str()) {
                    format!("execution_evidence_duplicate:{}", oracle.id)
                } else {
                    format!("execution_evidence_missing:{}", oracle.id)
                }),
            };
        };
        if validate_workspace_relative(&observed.evidence_path).is_err() {
            return CreateClaimCoverage {
                claim_id: gold.id.clone(),
                required: gold.required,
                strategy,
                strength: None,
                executed: false,
                outcome: OracleResult::Unverified,
                evidence_path: paths,
                oracle_ids,
                unverified_reason: Some(format!("evidence_path_unsafe:{}", oracle.id)),
            };
        }
        paths.push(observed.evidence_path.clone());
        strengths.push(observed.observed_strength);
        outcomes.push(observed.outcome);
    }
    let strength = strengths
        .into_iter()
        .min_by_key(|value| strength_rank(*value));
    if strength.is_some_and(|value| strength_rank(value) < strength_rank(gold.minimum_strength)) {
        return CreateClaimCoverage {
            claim_id: gold.id.clone(),
            required: gold.required,
            strategy,
            strength,
            executed: true,
            outcome: OracleResult::Unverified,
            evidence_path: paths,
            oracle_ids,
            unverified_reason: Some("execution_under_strength".to_string()),
        };
    }
    let outcome = combined_outcome(&outcomes);
    let unverified_reason = (outcome != OracleResult::Pass && outcome != OracleResult::Fail)
        .then(|| format!("execution_outcome:{outcome:?}"));
    CreateClaimCoverage {
        claim_id: gold.id.clone(),
        required: gold.required,
        strategy,
        strength,
        executed: true,
        outcome,
        evidence_path: paths,
        oracle_ids,
        unverified_reason,
    }
}

fn execution_by_oracle(
    execution: &[OracleExecutionEvidence],
) -> BTreeMap<&str, Option<&OracleExecutionEvidence>> {
    let mut indexed = BTreeMap::new();
    for item in execution {
        indexed
            .entry(item.oracle_id.as_str())
            .and_modify(|entry| *entry = None)
            .or_insert(Some(item));
    }
    indexed
}

fn oracle_matches_binding(oracle: &Oracle, binding: &CreateGoldBinding) -> bool {
    binding.accepted_strategies.contains(&oracle.strategy)
        && oracle.expected_polarity == binding.expected_polarity
        && oracle.input == binding.input
        && oracle.observation == binding.observation
}

fn validate_candidate_policy(oracle: &Oracle) -> Result<(), String> {
    if oracle.setup.argv.is_empty() {
        return Ok(());
    }
    crate::planner::declarative_command_checks::validate_shadow_argv(&oracle.setup.argv)
}

fn strength_rank(strength: EvidenceStrength) -> u8 {
    match strength {
        EvidenceStrength::Weak => 0,
        EvidenceStrength::Deterministic => 1,
        EvidenceStrength::Runtime => 2,
    }
}

fn combined_outcome(outcomes: &[OracleResult]) -> OracleResult {
    if outcomes.is_empty() {
        OracleResult::Unverified
    } else if outcomes
        .iter()
        .all(|outcome| *outcome == OracleResult::Pass)
    {
        OracleResult::Pass
    } else if outcomes.contains(&OracleResult::Fail) {
        OracleResult::Fail
    } else if outcomes.contains(&OracleResult::OracleError) {
        OracleResult::OracleError
    } else if outcomes.contains(&OracleResult::Blocked) {
        OracleResult::Blocked
    } else if outcomes.contains(&OracleResult::Partial) {
        OracleResult::Partial
    } else {
        OracleResult::Unverified
    }
}

fn failure_kind_name(kind: super::ShadowFailureKind) -> &'static str {
    match kind {
        super::ShadowFailureKind::SchemaInvalid => "schema_invalid",
        super::ShadowFailureKind::Timeout => "timeout",
        super::ShadowFailureKind::EmptyClaims => "empty_claims",
        super::ShadowFailureKind::ProviderUnavailable => "provider_unavailable",
        super::ShadowFailureKind::PolicyRejected => "policy_rejected",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregate_outcome_fails_closed() {
        assert_eq!(combined_outcome(&[]), OracleResult::Unverified);
        assert_eq!(
            combined_outcome(&[OracleResult::Pass, OracleResult::Fail]),
            OracleResult::Fail
        );
        assert_eq!(
            combined_outcome(&[OracleResult::Pass, OracleResult::OracleError]),
            OracleResult::OracleError
        );
    }

    #[test]
    fn no_required_gold_claims_cannot_report_full_coverage() {
        let generation = super::super::provider_failure(
            super::super::ShadowFailureKind::ProviderUnavailable,
            "offline",
        );
        assert!(!evaluate_create_shadow(&[], &generation, &[]).all_required_passed);
    }
}
