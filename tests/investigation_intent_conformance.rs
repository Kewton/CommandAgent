use commandagent::planner::adjudication::contract::{EvidenceStage, ExpectedOutcome, ProbeOutcome};
use commandagent::planner::adjudication::fix::FixFailureClassification;
use commandagent::planner::adjudication::investigate::{
    DiagnosisClaim, DiagnosisClaimKind, InvestigationAssurance, InvestigationBindingEvidence,
    InvestigationRunEvidence, evaluate_investigation_evidence,
};

fn failed_run() -> InvestigationRunEvidence {
    let mut run =
        InvestigationRunEvidence::new("python3 pipeline/main.py", 1, ProbeOutcome::Failure);
    run.stdout = String::new();
    run.stderr = "ValueError: invalid region".into();
    run
}

#[test]
fn ordinary_investigation_bytes_omit_workflow_only_lineage() {
    let evidence = InvestigationRunEvidence::new("false", 1, ProbeOutcome::Failure);
    let value = serde_json::to_value(evidence).unwrap();
    assert!(value.get("reproducer_lineage").is_none());
}

fn claim(kind: DiagnosisClaimKind, matched: bool) -> DiagnosisClaim {
    DiagnosisClaim {
        kind,
        value: "quoted".into(),
        subject_path: None,
        line: None,
        matched,
        nearest: (!matched).then(|| "nearest".into()),
    }
}

#[test]
fn initially_passing_reproducer_cannot_earn_full() {
    let run = InvestigationRunEvidence::new("true", 1, ProbeOutcome::Success);
    let result = evaluate_investigation_evidence(
        true,
        Some(&run),
        Some(&InvestigationBindingEvidence::new(vec![claim(
            DiagnosisClaimKind::ErrorQuote,
            true,
        )])),
    );
    assert_eq!(result.assurance, InvestigationAssurance::Failed);
    assert_eq!(result.reason, "baseline_not_reproduced");
}

#[test]
fn absent_error_quote_is_diagnosis_unbound() {
    let result = evaluate_investigation_evidence(
        true,
        Some(&failed_run()),
        Some(&InvestigationBindingEvidence::new(vec![claim(
            DiagnosisClaimKind::ErrorQuote,
            false,
        )])),
    );
    assert_eq!(result.assurance, InvestigationAssurance::Failed);
    assert_eq!(result.reason, "diagnosis_unbound");
}

#[test]
fn nonexistent_file_line_and_code_are_rejected() {
    let binding = InvestigationBindingEvidence::new(vec![
        claim(DiagnosisClaimKind::FileLine, false),
        claim(DiagnosisClaimKind::CodeSnippet, false),
    ]);
    let result = evaluate_investigation_evidence(true, Some(&failed_run()), Some(&binding));
    assert_eq!(result.assurance, InvestigationAssurance::Failed);
    assert_eq!(result.reason, "diagnosis_unbound");
}

#[test]
fn reproducer_defect_does_not_establish_i1() {
    let mut run = failed_run();
    run.failure_classification = FixFailureClassification::ReproducerDefect;
    let result = evaluate_investigation_evidence(true, Some(&run), None);
    assert_eq!(result.assurance, InvestigationAssurance::Failed);
    assert_eq!(result.reason, "reproducer_defect");
}

#[test]
fn unexecuted_probe_cannot_earn_assurance() {
    let mut run = failed_run();
    run.executed = false;
    run.outcome = ProbeOutcome::NotExecuted;
    run.stage = EvidenceStage::Diagnosis;
    run.expected = ExpectedOutcome::Failure;
    let result = evaluate_investigation_evidence(true, Some(&run), None);
    assert_eq!(result.assurance, InvestigationAssurance::Failed);
    assert_eq!(result.reason, "investigation_probe_not_executed");
}

#[test]
fn zero_machine_checkable_claims_caps_at_partial() {
    let result = evaluate_investigation_evidence(
        true,
        Some(&failed_run()),
        Some(&InvestigationBindingEvidence::new(Vec::new())),
    );
    assert_eq!(result.assurance, InvestigationAssurance::Partial);
    assert_eq!(result.reason, "diagnosis_claims_absent");
}

#[test]
fn executed_failure_and_all_bound_claims_earn_full() {
    let binding = InvestigationBindingEvidence::new(vec![
        claim(DiagnosisClaimKind::ErrorQuote, true),
        claim(DiagnosisClaimKind::FileLine, true),
        claim(DiagnosisClaimKind::CodeSnippet, true),
    ]);
    let result = evaluate_investigation_evidence(true, Some(&failed_run()), Some(&binding));
    assert_eq!(result.assurance, InvestigationAssurance::Full);
    assert!(result.reason.is_empty());
}
