use crate::planner::adjudication::investigate::{
    InvestigationAssurance, evaluate_investigation_evidence,
};

const RUN1_DIAGNOSIS: &str = include_str!(
    "../../../tests/corpus/apps/test0718_d3b_investigation_intent/fixtures/elev003-run1-diagnosis.md"
);
const RUN1_EVIDENCE: &str = include_str!(
    "../../../tests/corpus/apps/test0718_d3b_investigation_intent/fixtures/elev003-run1-investigation-run.json"
);
const RUN2_DIAGNOSIS: &str = include_str!(
    "../../../tests/corpus/apps/test0718_d3b_investigation_intent/fixtures/elev003-run2-diagnosis.md"
);
const RUN2_EVIDENCE: &str = include_str!(
    "../../../tests/corpus/apps/test0718_d3b_investigation_intent/fixtures/elev003-run2-investigation-run.json"
);
const RUN3_DIAGNOSIS: &str = include_str!(
    "../../../tests/corpus/apps/test0718_d3b_investigation_intent/fixtures/elev003-run3-diagnosis.md"
);
const RUN3_EVIDENCE: &str = include_str!(
    "../../../tests/corpus/apps/test0718_d3b_investigation_intent/fixtures/elev003-run3-investigation-run.json"
);

fn recorded_run(raw: &str) -> InvestigationRunEvidence {
    serde_json::from_str(raw).unwrap()
}

#[test]
fn prose_only_has_no_machine_checkable_claims() {
    let root = tempfile::tempdir().unwrap();
    let run = recorded_run(RUN2_EVIDENCE);
    let evidence = bind_diagnosis(root.path(), "The parser appears inconsistent.", &run);
    assert!(evidence.claims.is_empty());
}

#[test]
fn output_anchoring_binds_recorded_command_failed_and_schema_violation_quotes() {
    let root = tempfile::tempdir().unwrap();
    for (diagnosis, raw_run) in [
        (RUN1_DIAGNOSIS, RUN1_EVIDENCE),
        (RUN2_DIAGNOSIS, RUN2_EVIDENCE),
        (RUN3_DIAGNOSIS, RUN3_EVIDENCE),
    ] {
        let evidence = bind_diagnosis(root.path(), diagnosis, &recorded_run(raw_run));
        assert!(!evidence.claims.is_empty());
        assert!(evidence.claims.iter().all(|claim| claim.matched));
        assert!(
            evidence
                .claims
                .iter()
                .any(|claim| claim.kind == DiagnosisClaimKind::ErrorQuote)
        );
    }
}

#[test]
fn elev003_run1_false_violation_recalibrates_to_full() {
    let root = tempfile::tempdir().unwrap();
    let run = recorded_run(RUN1_EVIDENCE);
    let binding = bind_diagnosis(root.path(), RUN1_DIAGNOSIS, &run);
    let adjudication = evaluate_investigation_evidence(true, Some(&run), Some(&binding));
    assert_eq!(adjudication.assurance, InvestigationAssurance::Full);
    assert!(binding.claims.iter().all(|claim| claim.nearest.is_none()));
}

#[test]
fn explicit_fabricated_quote_without_keyword_remains_a_violation() {
    let root = tempfile::tempdir().unwrap();
    let run = recorded_run(RUN2_EVIDENCE);
    let diagnosis = "エラー引用: inspection_schema_violation:fabricated_claim";
    let evidence = bind_diagnosis(root.path(), diagnosis, &run);
    assert_eq!(evidence.claims.len(), 1);
    assert!(!evidence.claims[0].matched);
    assert!(evidence.claims[0].nearest.is_some());
}
