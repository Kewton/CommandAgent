//! Post-hoc shadow projection of investigate VerificationSpec claims.
//!
//! The existing I1/I2 evaluator remains the only verdict authority. This
//! module does not execute provider candidates or mutate investigation
//! evidence, diagnosis output, events, repair, or acceptance state.

use serde::Serialize;

use super::{
    AcceptanceClaim, ClaimKind, ClaimOrigin, ExpectedPolarity, OracleLifecycle, OracleObservation,
    OracleResult, OracleStrategy, ShadowGeneration, VerificationSpec,
};
use crate::planner::adjudication::contract::{EvidenceStage, ExpectedOutcome, ProbeOutcome};
use crate::planner::adjudication::investigate::{
    DIAGNOSIS_BOUND_ID, DiagnosisClaim, DiagnosisClaimKind, InvestigationAdjudication,
    InvestigationBindingEvidence, InvestigationRunEvidence, REPRODUCER_FAILS_ID,
    evaluate_investigation_evidence,
};
use crate::tools::path_guard::validate_workspace_relative;

pub const COVERAGE_SCHEMA_VERSION: &str =
    "commandagent.verification_spec.investigate_shadow_coverage.v0";
pub const REPRODUCER_BINDING_ID: &str = "reproducer";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvestigationEvidenceArtifacts {
    pub run_path: String,
    pub binding_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InvestigationShadowCoverageReport {
    pub schema_version: &'static str,
    pub shadow_only: bool,
    pub candidate_execution_authorized: bool,
    pub authoritative_verdict_changed: bool,
    pub authoritative: InvestigationAdjudication,
    pub all_observed_claims_covered: bool,
    pub observations: Vec<InvestigationObservationProjection>,
    pub causal_hypotheses: Vec<CausalHypothesisProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InvestigationObservationProjection {
    pub claim_id: Option<String>,
    pub requirement_id: String,
    pub binding_id: String,
    pub artifact_path: Option<String>,
    pub stage: EvidenceStage,
    pub lineage: String,
    pub epoch: Option<u64>,
    pub diagnosis_claim_kind: Option<DiagnosisClaimKind>,
    pub observed_value: Option<String>,
    pub evidence_matched: Option<bool>,
    pub covered: bool,
    pub unverified_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CausalHypothesisProjection {
    pub claim_id: String,
    pub statement: String,
    pub claim_kind: ClaimKind,
    pub observed_fact: bool,
    pub authoritative: bool,
    pub critic_asserted_observed: bool,
}

pub fn evaluate_investigation_shadow(
    report_written: bool,
    run: Option<&InvestigationRunEvidence>,
    binding: Option<&InvestigationBindingEvidence>,
    artifacts: &InvestigationEvidenceArtifacts,
    generation: &ShadowGeneration,
) -> InvestigationShadowCoverageReport {
    let authoritative = evaluate_investigation_evidence(report_written, run, binding);
    let mut observations = vec![project_reproducer(run, artifacts, generation)];
    if let (Some(run), Some(binding)) = (run, binding) {
        observations.extend(binding.claims.iter().enumerate().map(|(index, claim)| {
            project_diagnosis_claim(run, claim, index, artifacts, generation)
        }));
    }
    let binding_has_claims = binding.is_some_and(|item| !item.claims.is_empty());
    let all_observed_claims_covered =
        binding_has_claims && observations.iter().all(|observation| observation.covered);
    InvestigationShadowCoverageReport {
        schema_version: COVERAGE_SCHEMA_VERSION,
        shadow_only: true,
        candidate_execution_authorized: false,
        authoritative_verdict_changed: false,
        authoritative,
        all_observed_claims_covered,
        observations,
        causal_hypotheses: causal_hypotheses(generation),
    }
}

fn project_reproducer(
    run: Option<&InvestigationRunEvidence>,
    artifacts: &InvestigationEvidenceArtifacts,
    generation: &ShadowGeneration,
) -> InvestigationObservationProjection {
    let Some(run) = run else {
        return unverified_observation(
            REPRODUCER_FAILS_ID,
            REPRODUCER_BINDING_ID,
            "authoritative_evidence_missing",
        );
    };
    if validate_workspace_relative(&artifacts.run_path).is_err() {
        return observation_from_run(run, None, None, false, "authoritative_artifact_invalid");
    }
    let ShadowGeneration::Generated(spec) = generation else {
        return observation_from_run(
            run,
            Some(&artifacts.run_path),
            None,
            false,
            "generation_rejected",
        );
    };
    let matches = spec
        .claims
        .iter()
        .filter(|claim| reproducer_claim_matches(claim, spec, run, &artifacts.run_path))
        .collect::<Vec<_>>();
    let (claim_id, covered, reason) = unique_match(&matches);
    observation_from_run(run, Some(&artifacts.run_path), claim_id, covered, reason)
}

fn project_diagnosis_claim(
    run: &InvestigationRunEvidence,
    diagnosis: &DiagnosisClaim,
    index: usize,
    artifacts: &InvestigationEvidenceArtifacts,
    generation: &ShadowGeneration,
) -> InvestigationObservationProjection {
    let binding_id = diagnosis_binding_id(diagnosis.kind, index);
    if validate_workspace_relative(&artifacts.binding_path).is_err() {
        return observation_from_diagnosis(
            run,
            diagnosis,
            binding_id,
            None,
            None,
            false,
            "authoritative_artifact_invalid",
        );
    }
    let ShadowGeneration::Generated(spec) = generation else {
        return observation_from_diagnosis(
            run,
            diagnosis,
            binding_id,
            Some(&artifacts.binding_path),
            None,
            false,
            "generation_rejected",
        );
    };
    let matches = spec
        .claims
        .iter()
        .filter(|claim| {
            diagnosis_claim_matches(claim, spec, run, &binding_id, &artifacts.binding_path)
        })
        .collect::<Vec<_>>();
    let (claim_id, covered, reason) = unique_match(&matches);
    observation_from_diagnosis(
        run,
        diagnosis,
        binding_id,
        Some(&artifacts.binding_path),
        claim_id,
        covered,
        reason,
    )
}

fn reproducer_claim_matches(
    claim: &AcceptanceClaim,
    spec: &VerificationSpec,
    run: &InvestigationRunEvidence,
    artifact_path: &str,
) -> bool {
    claim.kind == ClaimKind::ReproducerObservation
        && origin_matches(
            claim,
            artifact_path,
            REPRODUCER_FAILS_ID,
            REPRODUCER_BINDING_ID,
            &run.reproducer_lineage,
            run.epoch,
        )
        && existing_oracle_matches(claim, spec, artifact_path, ExpectedPolarity::Failure)
}

fn diagnosis_claim_matches(
    claim: &AcceptanceClaim,
    spec: &VerificationSpec,
    run: &InvestigationRunEvidence,
    binding_id: &str,
    artifact_path: &str,
) -> bool {
    claim.kind == ClaimKind::DiagnosisBinding
        && origin_matches(
            claim,
            artifact_path,
            DIAGNOSIS_BOUND_ID,
            binding_id,
            &run.reproducer_lineage,
            run.epoch,
        )
        && existing_oracle_matches(claim, spec, artifact_path, ExpectedPolarity::Present)
}

fn origin_matches(
    claim: &AcceptanceClaim,
    artifact_path: &str,
    expected_requirement: &str,
    expected_binding: &str,
    expected_lineage: &str,
    expected_epoch: u64,
) -> bool {
    let ClaimOrigin::InvestigationRequirement {
        artifact_path: origin_path,
        requirement_id,
        binding_id,
        stage,
        lineage,
        epoch,
    } = &claim.origin
    else {
        return false;
    };
    origin_path == artifact_path
        && requirement_id == expected_requirement
        && binding_id == expected_binding
        && stage == "diagnosis"
        && lineage == expected_lineage
        && *epoch == expected_epoch
}

fn existing_oracle_matches(
    claim: &AcceptanceClaim,
    spec: &VerificationSpec,
    artifact_path: &str,
    expected_polarity: ExpectedPolarity,
) -> bool {
    claim.oracle_ids.iter().any(|oracle_id| {
        spec.oracles.iter().any(|oracle| {
            oracle.id == *oracle_id
                && oracle.claim_id == claim.id
                && oracle.strategy == OracleStrategy::ExistingInvestigationBinding
                && oracle.expected_polarity == expected_polarity
                && matches!(
                    &oracle.observation,
                    OracleObservation::ExistingBinding { artifact_path: path }
                        if path == artifact_path
                )
        })
    })
}

fn unique_match(matches: &[&AcceptanceClaim]) -> (Option<String>, bool, &'static str) {
    match matches {
        [claim] => (Some(claim.id.clone()), true, ""),
        [] => (None, false, "matching_claim_missing"),
        _ => (None, false, "matching_claim_duplicate"),
    }
}

fn observation_from_run(
    run: &InvestigationRunEvidence,
    artifact_path: Option<&str>,
    claim_id: Option<String>,
    covered: bool,
    reason: &str,
) -> InvestigationObservationProjection {
    InvestigationObservationProjection {
        claim_id,
        requirement_id: REPRODUCER_FAILS_ID.to_string(),
        binding_id: REPRODUCER_BINDING_ID.to_string(),
        artifact_path: artifact_path.map(str::to_string),
        stage: run.stage,
        lineage: run.reproducer_lineage.clone(),
        epoch: Some(run.epoch),
        diagnosis_claim_kind: None,
        observed_value: Some(format!("{}\n{}", run.stdout, run.stderr)),
        evidence_matched: Some(
            run.executed
                && run.stage == EvidenceStage::Diagnosis
                && run.expected == ExpectedOutcome::Failure
                && run.outcome == ProbeOutcome::Failure
                && !run.failure_classification.is_reproducer_defect(),
        ),
        covered,
        unverified_reason: (!covered).then(|| reason.to_string()),
    }
}

#[allow(clippy::too_many_arguments)]
fn observation_from_diagnosis(
    run: &InvestigationRunEvidence,
    diagnosis: &DiagnosisClaim,
    binding_id: String,
    artifact_path: Option<&str>,
    claim_id: Option<String>,
    covered: bool,
    reason: &str,
) -> InvestigationObservationProjection {
    InvestigationObservationProjection {
        claim_id,
        requirement_id: DIAGNOSIS_BOUND_ID.to_string(),
        binding_id,
        artifact_path: artifact_path.map(str::to_string),
        stage: EvidenceStage::Diagnosis,
        lineage: run.reproducer_lineage.clone(),
        epoch: Some(run.epoch),
        diagnosis_claim_kind: Some(diagnosis.kind),
        observed_value: Some(diagnosis.value.clone()),
        evidence_matched: Some(diagnosis.matched),
        covered,
        unverified_reason: (!covered).then(|| reason.to_string()),
    }
}

fn unverified_observation(
    requirement_id: &str,
    binding_id: &str,
    reason: &str,
) -> InvestigationObservationProjection {
    InvestigationObservationProjection {
        claim_id: None,
        requirement_id: requirement_id.to_string(),
        binding_id: binding_id.to_string(),
        artifact_path: None,
        stage: EvidenceStage::Diagnosis,
        lineage: String::new(),
        epoch: None,
        diagnosis_claim_kind: None,
        observed_value: None,
        evidence_matched: None,
        covered: false,
        unverified_reason: Some(reason.to_string()),
    }
}

fn causal_hypotheses(generation: &ShadowGeneration) -> Vec<CausalHypothesisProjection> {
    let ShadowGeneration::Generated(spec) = generation else {
        return Vec::new();
    };
    spec.claims
        .iter()
        .filter(|claim| {
            matches!(claim.origin, ClaimOrigin::InvestigationRequirement { .. })
                && !matches!(
                    claim.kind,
                    ClaimKind::ReproducerObservation | ClaimKind::DiagnosisBinding
                )
        })
        .map(|claim| CausalHypothesisProjection {
            claim_id: claim.id.clone(),
            statement: claim.normalized_requirement.clone(),
            claim_kind: claim.kind,
            observed_fact: false,
            authoritative: false,
            critic_asserted_observed: claim.oracle_ids.iter().any(|id| {
                spec.oracles.iter().any(|oracle| {
                    oracle.id == *id
                        && (oracle.lifecycle == OracleLifecycle::Executed
                            || oracle.result == OracleResult::Pass)
                })
            }),
        })
        .collect()
}

pub fn diagnosis_binding_id(kind: DiagnosisClaimKind, index: usize) -> String {
    let prefix = match kind {
        DiagnosisClaimKind::ErrorQuote => "error_quote",
        DiagnosisClaimKind::FileLine => "file_line",
        DiagnosisClaimKind::CodeSnippet => "code_snippet",
    };
    format!("{prefix}:{index}")
}
