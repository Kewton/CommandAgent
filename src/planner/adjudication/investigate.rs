use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::contract::{EvidenceStage, ExpectedOutcome, ProbeOutcome};
use super::fix::FixFailureClassification;

mod evidence;
pub use evidence::write_investigation_evidence;
pub(crate) use evidence::{write_investigation_binding, write_investigation_run};

pub const INVESTIGATION_CONTRACT_VERSION: &str = "v0";
pub const INVESTIGATION_CONTRACT_REF: &str = "docs/investigation-intent-contract.md";
pub const REPRODUCER_FAILS_ID: &str = "reproducer_fails";
pub const DIAGNOSIS_BOUND_ID: &str = "diagnosis_bound";
pub const BASELINE_NOT_REPRODUCED: &str = "baseline_not_reproduced";
pub const DIAGNOSIS_UNBOUND: &str = "diagnosis_unbound";
pub const DIAGNOSIS_CLAIMS_ABSENT: &str = "diagnosis_claims_absent";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvestigationRunEvidence {
    pub schema_version: String,
    pub intent: String,
    pub contract_version: String,
    pub contract_ref: String,
    pub requirement_id: String,
    pub reproducer: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reproducer_lineage: String,
    pub stage: EvidenceStage,
    pub expected: ExpectedOutcome,
    pub epoch: u64,
    pub executed: bool,
    pub outcome: ProbeOutcome,
    pub stdout: String,
    pub stderr: String,
    #[serde(default, skip_serializing_if = "FixFailureClassification::is_subject")]
    pub failure_classification: FixFailureClassification,
}

impl InvestigationRunEvidence {
    pub fn new(reproducer: impl Into<String>, epoch: u64, outcome: ProbeOutcome) -> Self {
        Self {
            schema_version: "1".into(),
            intent: "investigate".into(),
            contract_version: INVESTIGATION_CONTRACT_VERSION.into(),
            contract_ref: INVESTIGATION_CONTRACT_REF.into(),
            requirement_id: REPRODUCER_FAILS_ID.into(),
            reproducer: reproducer.into(),
            reproducer_lineage: String::new(),
            stage: EvidenceStage::Diagnosis,
            expected: ExpectedOutcome::Failure,
            epoch,
            executed: outcome.was_executed(),
            outcome,
            stdout: String::new(),
            stderr: String::new(),
            failure_classification: FixFailureClassification::SubjectFailure,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosisClaimKind {
    ErrorQuote,
    FileLine,
    CodeSnippet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosisClaim {
    pub kind: DiagnosisClaimKind,
    pub value: String,
    pub subject_path: Option<String>,
    pub line: Option<usize>,
    pub matched: bool,
    pub nearest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvestigationBindingEvidence {
    pub schema_version: String,
    pub intent: String,
    pub contract_version: String,
    pub contract_ref: String,
    pub requirement_id: String,
    pub claims: Vec<DiagnosisClaim>,
}

impl InvestigationBindingEvidence {
    pub fn new(claims: Vec<DiagnosisClaim>) -> Self {
        Self {
            schema_version: "1".into(),
            intent: "investigate".into(),
            contract_version: INVESTIGATION_CONTRACT_VERSION.into(),
            contract_ref: INVESTIGATION_CONTRACT_REF.into(),
            requirement_id: DIAGNOSIS_BOUND_ID.into(),
            claims,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InvestigationAssurance {
    Full,
    Partial,
    Static,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvestigationAdjudication {
    pub assurance: InvestigationAssurance,
    pub reason: String,
    pub requirement_statuses: BTreeMap<String, String>,
}

pub fn evaluate_investigation_evidence(
    report_written: bool,
    run: Option<&InvestigationRunEvidence>,
    binding: Option<&InvestigationBindingEvidence>,
) -> InvestigationAdjudication {
    let mut statuses = BTreeMap::from([
        (REPRODUCER_FAILS_ID.into(), "not_executed".into()),
        (DIAGNOSIS_BOUND_ID.into(), "not_executed".into()),
    ]);
    let Some(run) = run else {
        return adjudication(
            if report_written {
                InvestigationAssurance::Static
            } else {
                InvestigationAssurance::Failed
            },
            if report_written {
                "investigation_probe_not_executed"
            } else {
                "diagnosis_not_written"
            },
            statuses,
        );
    };
    if run.stage != EvidenceStage::Diagnosis
        || run.expected != ExpectedOutcome::Failure
        || !run.executed
    {
        return adjudication(
            InvestigationAssurance::Failed,
            "investigation_probe_not_executed",
            statuses,
        );
    }
    if run.failure_classification.is_reproducer_defect() {
        return adjudication(
            InvestigationAssurance::Failed,
            "reproducer_defect",
            statuses,
        );
    }
    if run.outcome != ProbeOutcome::Failure {
        statuses.insert(REPRODUCER_FAILS_ID.into(), "failed".into());
        return adjudication(
            InvestigationAssurance::Failed,
            BASELINE_NOT_REPRODUCED,
            statuses,
        );
    }
    statuses.insert(REPRODUCER_FAILS_ID.into(), "passed".into());
    let Some(binding) = binding else {
        return adjudication(
            InvestigationAssurance::Failed,
            "diagnosis_binding_not_executed",
            statuses,
        );
    };
    if binding.claims.is_empty() {
        statuses.insert(DIAGNOSIS_BOUND_ID.into(), "claims_absent".into());
        return adjudication(
            InvestigationAssurance::Partial,
            DIAGNOSIS_CLAIMS_ABSENT,
            statuses,
        );
    }
    if binding.claims.iter().any(|claim| !claim.matched) {
        statuses.insert(DIAGNOSIS_BOUND_ID.into(), "failed".into());
        return adjudication(InvestigationAssurance::Failed, DIAGNOSIS_UNBOUND, statuses);
    }
    statuses.insert(DIAGNOSIS_BOUND_ID.into(), "passed".into());
    adjudication(InvestigationAssurance::Full, "", statuses)
}

fn adjudication(
    assurance: InvestigationAssurance,
    reason: impl Into<String>,
    requirement_statuses: BTreeMap<String, String>,
) -> InvestigationAdjudication {
    InvestigationAdjudication {
        assurance,
        reason: reason.into(),
        requirement_statuses,
    }
}
