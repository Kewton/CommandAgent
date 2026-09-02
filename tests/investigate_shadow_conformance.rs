use commandagent::planner::adjudication::contract::ProbeOutcome;
use commandagent::planner::adjudication::fix::FixFailureClassification;
use commandagent::planner::adjudication::investigate::{
    DiagnosisClaim, DiagnosisClaimKind, InvestigationAssurance, InvestigationBindingEvidence,
    InvestigationRunEvidence,
};
use commandagent::verification_spec::investigate_shadow::{
    InvestigationEvidenceArtifacts, evaluate_investigation_shadow,
};
use commandagent::verification_spec::{VerificationIntent, parse_provider_spec};
use serde_json::{Value, json};

const GOAL: &str =
    "Investigate the parser failure and distinguish observations from possible causes.";
const INVESTIGATE_SHADOW: &str =
    include_str!("fixtures/verification_spec_v0/investigate-shadow-full.json");

fn run(outcome: ProbeOutcome) -> InvestigationRunEvidence {
    let mut run = InvestigationRunEvidence::new("cargo test parser", 7, outcome);
    run.reproducer_lineage = "reproducer:parser".to_string();
    run.stderr = "ValueError: invalid token at src/parser.rs:9".to_string();
    run
}

fn diagnosis_claim(kind: DiagnosisClaimKind, value: &str, matched: bool) -> DiagnosisClaim {
    DiagnosisClaim {
        kind,
        value: value.to_string(),
        subject_path: (kind != DiagnosisClaimKind::ErrorQuote).then(|| "src/parser.rs".to_string()),
        line: (kind == DiagnosisClaimKind::FileLine).then_some(9),
        matched,
        nearest: (!matched).then(|| "nearest measured value".to_string()),
    }
}

fn binding() -> InvestigationBindingEvidence {
    InvestigationBindingEvidence::new(vec![
        diagnosis_claim(
            DiagnosisClaimKind::ErrorQuote,
            "ValueError: invalid token",
            true,
        ),
        diagnosis_claim(DiagnosisClaimKind::FileLine, "src/parser.rs:9", true),
        diagnosis_claim(
            DiagnosisClaimKind::CodeSnippet,
            "return parse_error(token);",
            true,
        ),
    ])
}

fn artifacts() -> InvestigationEvidenceArtifacts {
    InvestigationEvidenceArtifacts {
        run_path: "evidence/investigation-run.json".to_string(),
        binding_path: "evidence/investigation-binding.json".to_string(),
    }
}

fn generation(raw: &str) -> commandagent::verification_spec::ShadowGeneration {
    commandagent::verification_spec::ShadowGeneration::Generated(Box::new(
        parse_provider_spec(GOAL, VerificationIntent::Investigate, raw).unwrap(),
    ))
}

#[test]
fn full_fixture_projects_i1_and_every_i2_binding_without_authority() {
    let run = run(ProbeOutcome::Failure);
    let binding = binding();
    let report = evaluate_investigation_shadow(
        true,
        Some(&run),
        Some(&binding),
        &artifacts(),
        &generation(INVESTIGATE_SHADOW),
    );

    assert_eq!(report.authoritative.assurance, InvestigationAssurance::Full);
    assert!(report.all_observed_claims_covered);
    assert!(report.shadow_only);
    assert!(!report.authoritative_verdict_changed);
    assert!(!report.candidate_execution_authorized);
    assert_eq!(report.observations.len(), 4);
    assert_eq!(report.observations[0].binding_id, "reproducer");
    assert_eq!(report.observations[1].binding_id, "error_quote:0");
    assert_eq!(report.observations[2].binding_id, "file_line:1");
    assert_eq!(report.observations[3].binding_id, "code_snippet:2");
    assert!(report.observations.iter().all(|claim| claim.covered));
}

#[test]
fn critic_cannot_promote_a_causal_hypothesis_to_observed_fact() {
    let run = run(ProbeOutcome::Failure);
    let binding = binding();
    let report = evaluate_investigation_shadow(
        true,
        Some(&run),
        Some(&binding),
        &artifacts(),
        &generation(INVESTIGATE_SHADOW),
    );

    assert_eq!(report.causal_hypotheses.len(), 1);
    let hypothesis = &report.causal_hypotheses[0];
    assert_eq!(hypothesis.claim_id, "possible-root-cause");
    assert!(hypothesis.critic_asserted_observed);
    assert!(!hypothesis.observed_fact);
    assert!(!hypothesis.authoritative);
}

#[test]
fn fabricated_evidence_and_duplicate_shadow_claims_never_rewrite_i2() {
    let run = run(ProbeOutcome::Failure);
    let mut fabricated = binding();
    fabricated.claims[0].matched = false;
    let report = evaluate_investigation_shadow(
        true,
        Some(&run),
        Some(&fabricated),
        &artifacts(),
        &generation(INVESTIGATE_SHADOW),
    );
    assert_eq!(
        report.authoritative.assurance,
        InvestigationAssurance::Failed
    );
    assert_eq!(report.authoritative.reason, "diagnosis_unbound");
    assert!(!report.authoritative_verdict_changed);

    let mut raw: Value = serde_json::from_str(INVESTIGATE_SHADOW).unwrap();
    let mut duplicate_claim = raw["claims"][0].clone();
    duplicate_claim["id"] = json!("i1-reproducer-duplicate");
    duplicate_claim["oracle_ids"] = json!(["i1-existing-duplicate"]);
    raw["claims"].as_array_mut().unwrap().push(duplicate_claim);
    let mut duplicate_oracle = raw["oracles"][0].clone();
    duplicate_oracle["id"] = json!("i1-existing-duplicate");
    duplicate_oracle["claim_id"] = json!("i1-reproducer-duplicate");
    raw["oracles"]
        .as_array_mut()
        .unwrap()
        .push(duplicate_oracle);
    let binding = binding();
    let report = evaluate_investigation_shadow(
        true,
        Some(&run),
        Some(&binding),
        &artifacts(),
        &generation(&raw.to_string()),
    );
    assert_eq!(report.authoritative.assurance, InvestigationAssurance::Full);
    assert!(!report.all_observed_claims_covered);
    assert_eq!(
        report.observations[0].unverified_reason.as_deref(),
        Some("matching_claim_duplicate")
    );
}

#[test]
fn claims_absent_reproducer_defect_and_passing_baseline_keep_existing_caps() {
    let failed = run(ProbeOutcome::Failure);
    let empty = InvestigationBindingEvidence::new(Vec::new());
    let report = evaluate_investigation_shadow(
        true,
        Some(&failed),
        Some(&empty),
        &artifacts(),
        &generation(INVESTIGATE_SHADOW),
    );
    assert_eq!(
        report.authoritative.assurance,
        InvestigationAssurance::Partial
    );
    assert_eq!(report.authoritative.reason, "diagnosis_claims_absent");
    assert!(!report.all_observed_claims_covered);

    let mut defect = run(ProbeOutcome::Failure);
    defect.failure_classification = FixFailureClassification::ReproducerDefect;
    let binding = binding();
    let report = evaluate_investigation_shadow(
        true,
        Some(&defect),
        Some(&binding),
        &artifacts(),
        &generation(INVESTIGATE_SHADOW),
    );
    assert_eq!(
        report.authoritative.assurance,
        InvestigationAssurance::Failed
    );
    assert_eq!(report.authoritative.reason, "reproducer_defect");

    let passing = run(ProbeOutcome::Success);
    let report = evaluate_investigation_shadow(
        true,
        Some(&passing),
        Some(&binding),
        &artifacts(),
        &generation(INVESTIGATE_SHADOW),
    );
    assert_eq!(
        report.authoritative.assurance,
        InvestigationAssurance::Failed
    );
    assert_eq!(report.authoritative.reason, "baseline_not_reproduced");
}

#[test]
fn observation_kind_substitution_is_a_hypothesis_not_i2_coverage() {
    let mut raw: Value = serde_json::from_str(INVESTIGATE_SHADOW).unwrap();
    raw["claims"][1]["kind"] = json!("state");
    raw["oracles"][1]["observed_strength"] = json!("runtime");
    raw["oracles"][1]["lifecycle"] = json!("executed");
    raw["oracles"][1]["result"] = json!("pass");
    let run = run(ProbeOutcome::Failure);
    let binding = binding();
    let report = evaluate_investigation_shadow(
        true,
        Some(&run),
        Some(&binding),
        &artifacts(),
        &generation(&raw.to_string()),
    );

    assert_eq!(report.authoritative.assurance, InvestigationAssurance::Full);
    assert!(!report.all_observed_claims_covered);
    assert_eq!(
        report.observations[1].unverified_reason.as_deref(),
        Some("matching_claim_missing")
    );
    assert!(
        report
            .causal_hypotheses
            .iter()
            .any(|claim| claim.claim_id == "i2-error" && !claim.observed_fact)
    );
}
