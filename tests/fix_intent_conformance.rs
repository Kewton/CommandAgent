use std::collections::BTreeMap;

use commandagent::planner::adjudication::contract::{EvidenceStage, ExpectedOutcome};
use commandagent::planner::adjudication::fix::{
    AFTER_PASSES_ID, BASELINE_NOT_REPRODUCED, BEFORE_FAILS_ID, FixAssurance, FixEvidenceBundle,
    FixEvidenceObservation, NO_REGRESSION_ID, ProbeOutcome, evaluate_fix_evidence,
};

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
        "conformance-run",
        outcome,
        "fixture",
    )
}

fn full_bundle() -> FixEvidenceBundle {
    FixEvidenceBundle {
        run_id: "conformance-run".to_string(),
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

#[test]
fn initially_passing_reproducer_cannot_earn_full() {
    let mut bundle = full_bundle();
    bundle.before.as_mut().unwrap().outcome = ProbeOutcome::Success;

    let result = evaluate_fix_evidence(&bundle);

    assert_eq!(result.assurance, FixAssurance::Failed);
    assert_eq!(result.reason, BASELINE_NOT_REPRODUCED);
}

#[test]
fn switched_reproducer_lineage_cannot_earn_full() {
    let mut bundle = full_bundle();
    bundle.after.as_mut().unwrap().lineage = "reproducer:easier-check".to_string();

    let result = evaluate_fix_evidence(&bundle);

    assert_eq!(result.assurance, FixAssurance::Failed);
    assert_eq!(result.reason, "reproducer_lineage_mismatch");
}

#[test]
fn before_after_requirement_swap_cannot_earn_full() {
    let mut bundle = full_bundle();
    let before = bundle.before.as_mut().unwrap();
    before.requirement_id = AFTER_PASSES_ID.to_string();
    before.stage = EvidenceStage::After;
    before.expected = ExpectedOutcome::Success;

    let result = evaluate_fix_evidence(&bundle);

    assert_eq!(result.assurance, FixAssurance::Failed);
    assert_eq!(result.reason, "requirement_binding_mismatch:before_fails");
}

#[test]
fn after_only_execution_cannot_earn_full() {
    let mut bundle = full_bundle();
    bundle.before = None;

    let result = evaluate_fix_evidence(&bundle);

    assert_eq!(result.assurance, FixAssurance::Failed);
    assert_eq!(result.reason, "before_not_executed");
}

#[test]
fn shrunken_regression_binding_set_cannot_earn_full() {
    let mut bundle = full_bundle();
    bundle.regressions.pop();

    let result = evaluate_fix_evidence(&bundle);

    assert_eq!(result.assurance, FixAssurance::Failed);
    assert_eq!(result.reason, "regression_set_mismatch");
}

#[test]
fn stale_after_epoch_cannot_earn_full() {
    let mut bundle = full_bundle();
    bundle.after.as_mut().unwrap().epoch = 1;

    let result = evaluate_fix_evidence(&bundle);

    assert_eq!(result.assurance, FixAssurance::Failed);
    assert_eq!(result.reason, "after_epoch_not_newer");
}

#[test]
fn failing_after_reproducer_is_a_failed_verdict() {
    let mut bundle = full_bundle();
    bundle.after.as_mut().unwrap().outcome = ProbeOutcome::Failure;

    let result = evaluate_fix_evidence(&bundle);

    assert_eq!(result.assurance, FixAssurance::Failed);
    assert_eq!(result.reason, "after_reproducer_failed");
}

#[test]
fn executed_regression_failure_is_a_failed_verdict() {
    let mut bundle = full_bundle();
    bundle.regressions[1].outcome = ProbeOutcome::Failure;

    let result = evaluate_fix_evidence(&bundle);

    assert_eq!(result.assurance, FixAssurance::Failed);
    assert_eq!(result.reason, "regression_failed:cargo-test");
}

#[test]
fn changed_regression_binding_cannot_earn_full() {
    let mut bundle = full_bundle();
    bundle.regressions[1].lineage = "regression:substitute".to_string();

    let result = evaluate_fix_evidence(&bundle);

    assert_eq!(result.assurance, FixAssurance::Failed);
    assert_eq!(result.reason, "regression_lineage_mismatch:cargo-test");
}

#[test]
fn unexecuted_probe_provenance_cannot_earn_full() {
    let mut bundle = full_bundle();
    bundle.after.as_mut().unwrap().executed = false;

    let result = evaluate_fix_evidence(&bundle);

    assert_eq!(result.assurance, FixAssurance::Failed);
    assert_eq!(result.reason, "execution_provenance_invalid:after_passes");
}

#[test]
fn evidence_schema_carries_stage_polarity_lineage_and_epoch() {
    let evidence = full_bundle().before.unwrap();
    let value = serde_json::to_value(evidence).unwrap();

    assert_eq!(value["stage"], "before");
    assert_eq!(value["expected"], "failure");
    assert_eq!(value["lineage"], "reproducer:parser");
    assert_eq!(value["epoch"], 1);
    assert_eq!(value["executed"], true);
    assert!(value.get("failure_classification").is_none());
}
