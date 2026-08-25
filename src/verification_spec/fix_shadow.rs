//! Post-hoc shadow projection of fix VerificationSpec claims.
//!
//! The existing fix evidence evaluator remains the only verdict authority.
//! This module neither executes provider candidates nor mutates fix evidence.

use serde::Serialize;

use super::{ClaimOrigin, ExpectedPolarity, OracleObservation, OracleStrategy, ShadowGeneration};
use crate::planner::adjudication::contract::{EvidenceStage, ExpectedOutcome};
use crate::planner::adjudication::fix::{
    AFTER_PASSES_ID, BEFORE_FAILS_ID, FixAdjudication, FixEvidenceBundle, FixEvidenceObservation,
    NO_REGRESSION_ID, evaluate_fix_evidence,
};
use crate::tools::path_guard::validate_workspace_relative;

pub const COVERAGE_SCHEMA_VERSION: &str = "commandagent.verification_spec.fix_shadow_coverage.v0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixEvidenceArtifact {
    pub evidence_path: String,
    pub observation: FixEvidenceObservation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FixShadowCoverageReport {
    pub schema_version: &'static str,
    pub shadow_only: bool,
    pub candidate_execution_authorized: bool,
    pub authoritative_verdict_changed: bool,
    pub authoritative: FixAdjudication,
    pub all_required_claims_covered: bool,
    pub candidates: Vec<FixVerificationCandidate>,
    pub claims: Vec<FixClaimProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FixVerificationCandidate {
    pub claim_id: String,
    pub oracle_id: String,
    pub requirement_id: String,
    pub argv: Vec<String>,
    pub cwd: String,
    pub fixture_paths: Vec<String>,
    pub proposed_lineage: String,
    pub proposed_epoch: u64,
    pub execution_authorized: bool,
    pub execution_boundary: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FixClaimProjection {
    pub claim_id: Option<String>,
    pub requirement_id: String,
    pub binding_id: String,
    pub artifact_path: Option<String>,
    pub stage: EvidenceStage,
    pub expected_polarity: ExpectedOutcome,
    pub lineage: String,
    pub epoch: Option<u64>,
    pub executed: bool,
    pub outcome: Option<crate::planner::adjudication::fix::ProbeOutcome>,
    pub covered: bool,
    pub unverified_reason: Option<String>,
}

/// Compare a provider proposal with already-authoritative F1/F2/F3 evidence.
///
/// `artifacts` supplies paths only for post-hoc correlation. An artifact must
/// exactly equal an observation already present in `bundle`; it cannot add or
/// replace authoritative evidence.
pub fn evaluate_fix_shadow(
    bundle: &FixEvidenceBundle,
    artifacts: &[FixEvidenceArtifact],
    generation: &ShadowGeneration,
) -> FixShadowCoverageReport {
    let authoritative = evaluate_fix_evidence(bundle);
    let mut claims = Vec::new();
    claims.push(project_slot(
        BEFORE_FAILS_ID,
        bundle.before.as_ref(),
        "",
        EvidenceStage::Before,
        ExpectedOutcome::Failure,
        generation,
        artifacts,
    ));
    claims.push(project_slot(
        AFTER_PASSES_ID,
        bundle.after.as_ref(),
        "",
        EvidenceStage::After,
        ExpectedOutcome::Success,
        generation,
        artifacts,
    ));
    for binding_id in &bundle.bound_regression_ids {
        let observation = bundle
            .regressions
            .iter()
            .find(|item| item.binding_id == *binding_id);
        let lineage = bundle
            .bound_regression_lineages
            .get(binding_id)
            .map(String::as_str)
            .unwrap_or("");
        claims.push(project_slot(
            NO_REGRESSION_ID,
            observation,
            lineage,
            EvidenceStage::After,
            ExpectedOutcome::Success,
            generation,
            artifacts,
        ));
    }
    let all_required_claims_covered = claims.iter().all(|claim| claim.covered);
    FixShadowCoverageReport {
        schema_version: COVERAGE_SCHEMA_VERSION,
        shadow_only: true,
        candidate_execution_authorized: false,
        authoritative_verdict_changed: false,
        authoritative,
        all_required_claims_covered,
        candidates: extract_candidates(generation),
        claims,
    }
}

fn project_slot(
    requirement_id: &str,
    observation: Option<&FixEvidenceObservation>,
    missing_lineage: &str,
    stage: EvidenceStage,
    expected: ExpectedOutcome,
    generation: &ShadowGeneration,
    artifacts: &[FixEvidenceArtifact],
) -> FixClaimProjection {
    let binding_id = observation.map_or("", |item| item.binding_id.as_str());
    let lineage = observation.map_or(missing_lineage, |item| item.lineage.as_str());
    let Some(observation) = observation else {
        return unverified_projection(
            requirement_id,
            binding_id,
            stage,
            expected,
            lineage,
            "authoritative_evidence_missing",
        );
    };
    let artifact_paths = artifacts
        .iter()
        .filter(|artifact| {
            artifact.observation == *observation
                && validate_workspace_relative(&artifact.evidence_path).is_ok()
        })
        .map(|artifact| artifact.evidence_path.as_str())
        .collect::<Vec<_>>();
    let [artifact_path] = artifact_paths.as_slice() else {
        return FixClaimProjection {
            claim_id: None,
            requirement_id: requirement_id.to_string(),
            binding_id: observation.binding_id.clone(),
            artifact_path: None,
            stage,
            expected_polarity: expected,
            lineage: observation.lineage.clone(),
            epoch: Some(observation.epoch),
            executed: observation.executed,
            outcome: Some(observation.outcome),
            covered: false,
            unverified_reason: Some(if artifact_paths.len() > 1 {
                "authoritative_artifact_duplicate".to_string()
            } else {
                "authoritative_artifact_missing".to_string()
            }),
        };
    };
    let ShadowGeneration::Generated(spec) = generation else {
        return projection_from_observation(
            observation,
            artifact_path,
            None,
            false,
            Some("generation_rejected".to_string()),
        );
    };
    let matches = spec
        .claims
        .iter()
        .filter(|claim| {
            claim_matches_observation(claim, spec, observation, artifact_path, expected)
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [claim] => projection_from_observation(
            observation,
            artifact_path,
            Some(claim.id.clone()),
            true,
            None,
        ),
        [] => projection_from_observation(
            observation,
            artifact_path,
            None,
            false,
            Some("matching_claim_missing".to_string()),
        ),
        _ => projection_from_observation(
            observation,
            artifact_path,
            None,
            false,
            Some("matching_claim_duplicate".to_string()),
        ),
    }
}

fn claim_matches_observation(
    claim: &super::AcceptanceClaim,
    spec: &super::VerificationSpec,
    observation: &FixEvidenceObservation,
    artifact_path: &str,
    expected: ExpectedOutcome,
) -> bool {
    let ClaimOrigin::FixRequirement {
        artifact_path: origin_path,
        requirement_id,
        stage,
        expected_polarity,
        lineage,
        epoch,
    } = &claim.origin
    else {
        return false;
    };
    origin_path == artifact_path
        && requirement_id == &observation.requirement_id
        && stage == stage_name(observation.stage)
        && *expected_polarity == polarity(expected)
        && lineage == &observation.lineage
        && *epoch == observation.epoch
        && claim.oracle_ids.iter().any(|oracle_id| {
            spec.oracles.iter().any(|oracle| {
                oracle.id == *oracle_id
                    && oracle.claim_id == claim.id
                    && oracle.strategy == OracleStrategy::ExistingFixEvidence
                    && oracle.expected_polarity == polarity(expected)
                    && matches!(
                        &oracle.observation,
                        OracleObservation::ExistingBinding { artifact_path: path }
                            if path == artifact_path
                    )
            })
        })
}

fn extract_candidates(generation: &ShadowGeneration) -> Vec<FixVerificationCandidate> {
    let ShadowGeneration::Generated(spec) = generation else {
        return Vec::new();
    };
    let mut candidates = Vec::new();
    for claim in &spec.claims {
        let ClaimOrigin::FixRequirement {
            requirement_id,
            stage,
            expected_polarity,
            lineage,
            epoch,
            ..
        } = &claim.origin
        else {
            continue;
        };
        let candidate_role = (requirement_id == BEFORE_FAILS_ID
            && stage == "before"
            && *expected_polarity == ExpectedPolarity::Failure)
            || (requirement_id == NO_REGRESSION_ID
                && stage == "after"
                && *expected_polarity == ExpectedPolarity::Success);
        if !candidate_role {
            continue;
        }
        for oracle_id in &claim.oracle_ids {
            let Some(oracle) = spec.oracles.iter().find(|item| &item.id == oracle_id) else {
                continue;
            };
            if oracle.setup.argv.is_empty()
                || oracle.expected_polarity != *expected_polarity
                || !matches!(
                    oracle.strategy,
                    OracleStrategy::Command
                        | OracleStrategy::ExitCode
                        | OracleStrategy::Stdout
                        | OracleStrategy::Stderr
                        | OracleStrategy::ExistingFixEvidence
                )
                || crate::planner::declarative_command_checks::validate_shadow_argv(
                    &oracle.setup.argv,
                )
                .is_err()
            {
                continue;
            }
            candidates.push(FixVerificationCandidate {
                claim_id: claim.id.clone(),
                oracle_id: oracle.id.clone(),
                requirement_id: requirement_id.clone(),
                argv: oracle.setup.argv.clone(),
                cwd: oracle.setup.cwd.clone(),
                fixture_paths: oracle.setup.fixture_paths.clone(),
                proposed_lineage: lineage.clone(),
                proposed_epoch: *epoch,
                execution_authorized: false,
                execution_boundary: "isolated_copy_or_after_authoritative_f1_f2_f3",
            });
        }
    }
    candidates
}

fn unverified_projection(
    requirement_id: &str,
    binding_id: &str,
    stage: EvidenceStage,
    expected: ExpectedOutcome,
    lineage: &str,
    reason: &str,
) -> FixClaimProjection {
    FixClaimProjection {
        claim_id: None,
        requirement_id: requirement_id.to_string(),
        binding_id: binding_id.to_string(),
        artifact_path: None,
        stage,
        expected_polarity: expected,
        lineage: lineage.to_string(),
        epoch: None,
        executed: false,
        outcome: None,
        covered: false,
        unverified_reason: Some(reason.to_string()),
    }
}

fn projection_from_observation(
    observation: &FixEvidenceObservation,
    artifact_path: &str,
    claim_id: Option<String>,
    covered: bool,
    unverified_reason: Option<String>,
) -> FixClaimProjection {
    FixClaimProjection {
        claim_id,
        requirement_id: observation.requirement_id.clone(),
        binding_id: observation.binding_id.clone(),
        artifact_path: Some(artifact_path.to_string()),
        stage: observation.stage,
        expected_polarity: observation.expected,
        lineage: observation.lineage.clone(),
        epoch: Some(observation.epoch),
        executed: observation.executed,
        outcome: Some(observation.outcome),
        covered,
        unverified_reason,
    }
}

fn stage_name(stage: EvidenceStage) -> &'static str {
    match stage {
        EvidenceStage::Before => "before",
        EvidenceStage::After => "after",
        EvidenceStage::Unstaged => "unstaged",
        EvidenceStage::Diagnosis => "diagnosis",
    }
}

fn polarity(expected: ExpectedOutcome) -> ExpectedPolarity {
    match expected {
        ExpectedOutcome::Success => ExpectedPolarity::Success,
        ExpectedOutcome::Failure => ExpectedPolarity::Failure,
        ExpectedOutcome::Observation => ExpectedPolarity::Present,
    }
}
