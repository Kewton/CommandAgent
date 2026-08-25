use std::collections::BTreeMap;

use commandagent::planner::adjudication::contract::{EvidenceStage, ExpectedOutcome};
use commandagent::planner::adjudication::fix::{
    AFTER_PASSES_ID, BEFORE_FAILS_ID, FixAssurance, FixEvidenceBundle, FixEvidenceObservation,
    NO_REGRESSION_ID, ProbeOutcome,
};
use commandagent::verification_spec::fix_shadow::{FixEvidenceArtifact, evaluate_fix_shadow};
use commandagent::verification_spec::{VerificationIntent, parse_provider_spec};
use serde_json::{Value, json};

const GOAL: &str = "Fix the parser crash reproduced by cargo test parser.";
const FIX_SHADOW: &str = include_str!("fixtures/verification_spec_v0/fix-shadow-full.json");

fn observation(
    requirement: &str,
    binding: &str,
    stage: EvidenceStage,
    expected: ExpectedOutcome,
    lineage: &str,
    epoch: u64,
    outcome: ProbeOutcome,
) -> FixEvidenceObservation {
    FixEvidenceObservation::new(
        requirement,
        binding,
        stage,
        expected,
        lineage,
        epoch,
        "run",
        outcome,
        "fixture",
    )
}

fn full_bundle() -> FixEvidenceBundle {
    FixEvidenceBundle {
        run_id: "run".to_string(),
        fix_written: true,
        bound_regression_ids: vec!["profile-contract".to_string(), "cargo-test".to_string()],
        bound_regression_lineages: BTreeMap::from([
            (
                "profile-contract".to_string(),
                "regression:profile-contract".to_string(),
            ),
            (
                "cargo-test".to_string(),
                "regression:cargo-test".to_string(),
            ),
        ]),
        before: Some(observation(
            BEFORE_FAILS_ID,
            "cargo test parser",
            EvidenceStage::Before,
            ExpectedOutcome::Failure,
            "reproducer:parser",
            1,
            ProbeOutcome::Failure,
        )),
        after: Some(observation(
            AFTER_PASSES_ID,
            "cargo test parser",
            EvidenceStage::After,
            ExpectedOutcome::Success,
            "reproducer:parser",
            2,
            ProbeOutcome::Success,
        )),
        regressions: vec![
            observation(
                NO_REGRESSION_ID,
                "profile-contract",
                EvidenceStage::After,
                ExpectedOutcome::Success,
                "regression:profile-contract",
                3,
                ProbeOutcome::Success,
            ),
            observation(
                NO_REGRESSION_ID,
                "cargo-test",
                EvidenceStage::After,
                ExpectedOutcome::Success,
                "regression:cargo-test",
                4,
                ProbeOutcome::Success,
            ),
        ],
    }
}

fn artifacts(bundle: &FixEvidenceBundle) -> Vec<FixEvidenceArtifact> {
    vec![
        (
            "evidence/fix-run-before.json",
            bundle.before.as_ref().unwrap(),
        ),
        (
            "evidence/fix-run-after.json",
            bundle.after.as_ref().unwrap(),
        ),
        (
            "evidence/fix-run-regression-profile-contract.json",
            &bundle.regressions[0],
        ),
        (
            "evidence/fix-run-regression-cargo-test.json",
            &bundle.regressions[1],
        ),
    ]
    .into_iter()
    .map(|(path, observation)| FixEvidenceArtifact {
        evidence_path: path.to_string(),
        observation: observation.clone(),
    })
    .collect()
}

fn generation(raw: &str) -> commandagent::verification_spec::ShadowGeneration {
    commandagent::verification_spec::ShadowGeneration::Generated(Box::new(
        parse_provider_spec(GOAL, VerificationIntent::Fix, raw).unwrap(),
    ))
}

#[test]
fn full_fixture_projects_every_f1_f2_and_frozen_f3_field_without_authority() {
    let bundle = full_bundle();
    let report = evaluate_fix_shadow(&bundle, &artifacts(&bundle), &generation(FIX_SHADOW));

    assert_eq!(report.authoritative.assurance, FixAssurance::Full);
    assert!(report.all_required_claims_covered);
    assert!(report.shadow_only);
    assert!(!report.authoritative_verdict_changed);
    assert!(!report.candidate_execution_authorized);
    assert_eq!(report.claims.len(), 4);
    assert_eq!(report.claims[0].requirement_id, BEFORE_FAILS_ID);
    assert_eq!(report.claims[0].stage, EvidenceStage::Before);
    assert_eq!(report.claims[0].expected_polarity, ExpectedOutcome::Failure);
    assert_eq!(report.claims[0].lineage, "reproducer:parser");
    assert_eq!(report.claims[0].epoch, Some(1));
    assert_eq!(report.claims[1].requirement_id, AFTER_PASSES_ID);
    assert_eq!(report.claims[2].binding_id, "profile-contract");
    assert_eq!(report.claims[3].binding_id, "cargo-test");
    assert_eq!(report.candidates.len(), 2);
    assert_eq!(report.candidates[0].argv, ["cargo", "test", "parser"]);
    assert_eq!(report.candidates[0].requirement_id, BEFORE_FAILS_ID);
    assert_eq!(report.candidates[1].requirement_id, NO_REGRESSION_ID);
    assert!(!report.candidates[0].execution_authorized);
}

#[test]
fn switched_before_after_and_stale_epoch_are_uncovered_not_reinterpreted() {
    for (pointer, replacement) in [
        ("/claims/0/origin/stage", json!("after")),
        ("/claims/0/origin/expected_polarity", json!("success")),
        ("/claims/1/origin/lineage", json!("reproducer:weaker")),
        ("/claims/1/origin/epoch", json!(1)),
    ] {
        let mut raw: Value = serde_json::from_str(FIX_SHADOW).unwrap();
        *raw.pointer_mut(pointer).unwrap() = replacement;
        let bundle = full_bundle();
        let report =
            evaluate_fix_shadow(&bundle, &artifacts(&bundle), &generation(&raw.to_string()));
        assert_eq!(report.authoritative.assurance, FixAssurance::Full);
        assert!(!report.all_required_claims_covered, "pointer={pointer}");
        assert!(report.claims.iter().any(|claim| !claim.covered));
    }
}

#[test]
fn after_only_and_changed_frozen_set_cannot_manufacture_full() {
    let mut after_only = full_bundle();
    after_only.before = None;
    let report = evaluate_fix_shadow(
        &after_only,
        &artifacts(&full_bundle()),
        &generation(FIX_SHADOW),
    );
    assert_eq!(report.authoritative.assurance, FixAssurance::Failed);
    assert_eq!(report.authoritative.reason, "before_not_executed");
    assert!(!report.all_required_claims_covered);

    let mut changed = full_bundle();
    changed.regressions.pop();
    let report = evaluate_fix_shadow(
        &changed,
        &artifacts(&full_bundle()),
        &generation(FIX_SHADOW),
    );
    assert_eq!(report.authoritative.assurance, FixAssurance::Failed);
    assert_eq!(report.authoritative.reason, "regression_set_mismatch");
    assert!(!report.all_required_claims_covered);
}

#[test]
fn model_declared_execution_cannot_override_partial_or_static_caps() {
    let mut raw: Value = serde_json::from_str(FIX_SHADOW).unwrap();
    for oracle in raw["oracles"].as_array_mut().unwrap() {
        oracle["lifecycle"] = json!("executed");
        oracle["result"] = json!("pass");
        oracle["observed_strength"] = json!("runtime");
    }

    let mut partial = full_bundle();
    partial.regressions[0].outcome = ProbeOutcome::Unavailable;
    partial.regressions[0].executed = false;
    let report = evaluate_fix_shadow(
        &partial,
        &artifacts(&partial),
        &generation(&raw.to_string()),
    );
    assert_eq!(report.authoritative.assurance, FixAssurance::Partial);

    let static_bundle = FixEvidenceBundle {
        run_id: "run".to_string(),
        fix_written: true,
        bound_regression_ids: Vec::new(),
        bound_regression_lineages: BTreeMap::new(),
        before: None,
        after: None,
        regressions: Vec::new(),
    };
    let report = evaluate_fix_shadow(&static_bundle, &[], &generation(&raw.to_string()));
    assert_eq!(report.authoritative.assurance, FixAssurance::Static);
    assert!(!report.all_required_claims_covered);
}
