use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

pub use super::contract::ProbeOutcome;
use super::contract::{EvidenceStage, ExpectedOutcome, FIX_CONTRACT_REF, FIX_CONTRACT_VERSION};

mod failure;
pub use failure::FixFailureClassification;

pub const BEFORE_FAILS_ID: &str = "before_fails";
pub const AFTER_PASSES_ID: &str = "after_passes";
pub const NO_REGRESSION_ID: &str = "no_regression";
pub const BASELINE_NOT_REPRODUCED: &str = "baseline_not_reproduced";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixEvidenceObservation {
    pub schema_version: String,
    pub intent: String,
    pub contract_version: String,
    pub contract_ref: String,
    pub requirement_id: String,
    pub binding_id: String,
    pub stage: EvidenceStage,
    pub expected: ExpectedOutcome,
    pub lineage: String,
    pub epoch: u64,
    pub run_id: String,
    pub executed: bool,
    pub outcome: ProbeOutcome,
    pub reason: String,
    #[serde(default, skip_serializing_if = "FixFailureClassification::is_subject")]
    pub failure_classification: FixFailureClassification,
}

impl FixEvidenceObservation {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        requirement_id: &str,
        binding_id: &str,
        stage: EvidenceStage,
        expected: ExpectedOutcome,
        lineage: &str,
        epoch: u64,
        run_id: &str,
        outcome: ProbeOutcome,
        reason: &str,
    ) -> Self {
        Self {
            schema_version: "1".to_string(),
            intent: "fix".to_string(),
            contract_version: FIX_CONTRACT_VERSION.to_string(),
            contract_ref: FIX_CONTRACT_REF.to_string(),
            requirement_id: requirement_id.to_string(),
            binding_id: binding_id.to_string(),
            stage,
            expected,
            lineage: lineage.to_string(),
            epoch,
            run_id: run_id.to_string(),
            executed: outcome.was_executed(),
            outcome,
            reason: reason.to_string(),
            failure_classification: FixFailureClassification::SubjectFailure,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixEvidenceBundle {
    pub run_id: String,
    pub fix_written: bool,
    pub bound_regression_ids: Vec<String>,
    pub bound_regression_lineages: BTreeMap<String, String>,
    pub before: Option<FixEvidenceObservation>,
    pub after: Option<FixEvidenceObservation>,
    pub regressions: Vec<FixEvidenceObservation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FixAssurance {
    Full,
    Partial,
    Static,
    Failed,
}

impl FixAssurance {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Partial => "partial",
            Self::Static => "static",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixAdjudication {
    pub assurance: FixAssurance,
    pub reason: String,
    pub requirement_statuses: BTreeMap<String, String>,
}

impl FixAdjudication {
    fn new(
        assurance: FixAssurance,
        reason: impl Into<String>,
        requirement_statuses: BTreeMap<String, String>,
    ) -> Self {
        Self {
            assurance,
            reason: reason.into(),
            requirement_statuses,
        }
    }
}

pub fn evaluate_fix_evidence(bundle: &FixEvidenceBundle) -> FixAdjudication {
    let mut statuses = BTreeMap::from([
        (BEFORE_FAILS_ID.to_string(), "not_executed".to_string()),
        (AFTER_PASSES_ID.to_string(), "not_executed".to_string()),
        (NO_REGRESSION_ID.to_string(), "not_executed".to_string()),
    ]);
    let any_executed = bundle
        .before
        .iter()
        .chain(bundle.after.iter())
        .chain(bundle.regressions.iter())
        .any(|observation| observation.executed);
    if bundle.fix_written && !any_executed {
        return FixAdjudication::new(FixAssurance::Static, "fix_probes_not_executed", statuses);
    }

    let Some(before) = bundle.before.as_ref() else {
        return FixAdjudication::new(FixAssurance::Failed, "before_not_executed", statuses);
    };
    if let Some(reason) = invalid_observation(
        before,
        &bundle.run_id,
        BEFORE_FAILS_ID,
        EvidenceStage::Before,
        ExpectedOutcome::Failure,
    ) {
        return FixAdjudication::new(FixAssurance::Failed, reason, statuses);
    }
    match before.outcome {
        ProbeOutcome::Success => {
            statuses.insert(BEFORE_FAILS_ID.to_string(), "failed".to_string());
            return FixAdjudication::new(FixAssurance::Failed, BASELINE_NOT_REPRODUCED, statuses);
        }
        ProbeOutcome::Failure => {
            statuses.insert(BEFORE_FAILS_ID.to_string(), "passed".to_string());
        }
        ProbeOutcome::Inconclusive | ProbeOutcome::Unavailable | ProbeOutcome::NotExecuted => {
            statuses.insert(BEFORE_FAILS_ID.to_string(), "unverified".to_string());
            return FixAdjudication::new(
                FixAssurance::Failed,
                "before_probe_unavailable",
                statuses,
            );
        }
    }

    let Some(after) = bundle.after.as_ref() else {
        return FixAdjudication::new(FixAssurance::Failed, "after_not_executed", statuses);
    };
    if let Some(reason) = invalid_observation(
        after,
        &bundle.run_id,
        AFTER_PASSES_ID,
        EvidenceStage::After,
        ExpectedOutcome::Success,
    ) {
        return FixAdjudication::new(FixAssurance::Failed, reason, statuses);
    }
    if before.lineage.is_empty()
        || after.lineage.is_empty()
        || before.lineage != after.lineage
        || before.binding_id != after.binding_id
    {
        return FixAdjudication::new(
            FixAssurance::Failed,
            "reproducer_lineage_mismatch",
            statuses,
        );
    }
    if after.epoch <= before.epoch {
        return FixAdjudication::new(FixAssurance::Failed, "after_epoch_not_newer", statuses);
    }
    match after.outcome {
        ProbeOutcome::Success => {
            statuses.insert(AFTER_PASSES_ID.to_string(), "passed".to_string());
        }
        ProbeOutcome::Failure => {
            statuses.insert(AFTER_PASSES_ID.to_string(), "failed".to_string());
            return FixAdjudication::new(FixAssurance::Failed, "after_reproducer_failed", statuses);
        }
        ProbeOutcome::Inconclusive | ProbeOutcome::Unavailable | ProbeOutcome::NotExecuted => {
            statuses.insert(AFTER_PASSES_ID.to_string(), "unverified".to_string());
            return FixAdjudication::new(FixAssurance::Failed, "after_probe_unavailable", statuses);
        }
    }

    let bound = bundle
        .bound_regression_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if bound.len() != bundle.bound_regression_ids.len() {
        return FixAdjudication::new(
            FixAssurance::Failed,
            "regression_binding_duplicate",
            statuses,
        );
    }
    let lineage_ids = bundle
        .bound_regression_lineages
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    if lineage_ids != bound
        || bundle
            .bound_regression_lineages
            .values()
            .any(|lineage| lineage.is_empty())
    {
        return FixAdjudication::new(
            FixAssurance::Failed,
            "regression_binding_lineage_invalid",
            statuses,
        );
    }
    let observed = bundle
        .regressions
        .iter()
        .map(|observation| observation.binding_id.clone())
        .collect::<BTreeSet<_>>();
    if observed.len() != bundle.regressions.len() || observed != bound {
        return FixAdjudication::new(FixAssurance::Failed, "regression_set_mismatch", statuses);
    }

    let mut regression_inconclusive = None;
    for observation in &bundle.regressions {
        if let Some(reason) = invalid_observation(
            observation,
            &bundle.run_id,
            NO_REGRESSION_ID,
            EvidenceStage::After,
            ExpectedOutcome::Success,
        ) {
            return FixAdjudication::new(FixAssurance::Failed, reason, statuses);
        }
        if bundle
            .bound_regression_lineages
            .get(&observation.binding_id)
            != Some(&observation.lineage)
        {
            return FixAdjudication::new(
                FixAssurance::Failed,
                format!("regression_lineage_mismatch:{}", observation.binding_id),
                statuses,
            );
        }
        if observation.epoch <= before.epoch {
            return FixAdjudication::new(
                FixAssurance::Failed,
                "regression_epoch_not_after_before",
                statuses,
            );
        }
        match observation.outcome {
            ProbeOutcome::Success => {}
            ProbeOutcome::Failure => {
                statuses.insert(NO_REGRESSION_ID.to_string(), "failed".to_string());
                return FixAdjudication::new(
                    FixAssurance::Failed,
                    format!("regression_failed:{}", observation.binding_id),
                    statuses,
                );
            }
            ProbeOutcome::Inconclusive | ProbeOutcome::Unavailable => {
                regression_inconclusive.get_or_insert_with(|| observation.binding_id.clone());
            }
            ProbeOutcome::NotExecuted => {
                return FixAdjudication::new(
                    FixAssurance::Failed,
                    format!("regression_not_executed:{}", observation.binding_id),
                    statuses,
                );
            }
        }
    }
    if let Some(binding) = regression_inconclusive {
        statuses.insert(NO_REGRESSION_ID.to_string(), "inconclusive".to_string());
        return FixAdjudication::new(
            FixAssurance::Partial,
            format!("regression_inconclusive:{binding}"),
            statuses,
        );
    }
    statuses.insert(NO_REGRESSION_ID.to_string(), "passed".to_string());
    FixAdjudication::new(FixAssurance::Full, "", statuses)
}

fn invalid_observation(
    observation: &FixEvidenceObservation,
    run_id: &str,
    requirement_id: &str,
    stage: EvidenceStage,
    expected: ExpectedOutcome,
) -> Option<String> {
    if observation.schema_version != "1"
        || observation.intent != "fix"
        || observation.contract_version != FIX_CONTRACT_VERSION
        || observation.contract_ref != FIX_CONTRACT_REF
    {
        return Some(format!("contract_provenance_invalid:{requirement_id}"));
    }
    if run_id.trim().is_empty() || observation.run_id != run_id {
        return Some(format!("run_provenance_mismatch:{requirement_id}"));
    }
    if observation.requirement_id != requirement_id
        || observation.stage != stage
        || observation.expected != expected
    {
        return Some(format!("requirement_binding_mismatch:{requirement_id}"));
    }
    if observation.epoch == 0 {
        return Some(format!("evidence_epoch_invalid:{requirement_id}"));
    }
    if observation.executed != observation.outcome.was_executed() {
        return Some(format!("execution_provenance_invalid:{requirement_id}"));
    }
    None
}

pub fn evidence_lineage(namespace: &str, normalized_binding: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in namespace
        .as_bytes()
        .iter()
        .chain(std::iter::once(&0))
        .chain(normalized_binding.as_bytes())
    {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{namespace}:{hash:016x}")
}

pub fn reproducer_lineage(normalized_binding: &str) -> String {
    evidence_lineage("reproducer", normalized_binding)
}

#[cfg(test)]
mod tests {
    use super::*;

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
            "run-1",
            outcome,
            "test",
        )
    }

    fn full_bundle() -> FixEvidenceBundle {
        FixEvidenceBundle {
            run_id: "run-1".to_string(),
            fix_written: true,
            bound_regression_ids: vec!["cargo-test".to_string()],
            bound_regression_lineages: BTreeMap::from([(
                "cargo-test".to_string(),
                "regression:cargo-test".to_string(),
            )]),
            before: Some(observation(
                BEFORE_FAILS_ID,
                "cargo test parser",
                EvidenceStage::Before,
                ExpectedOutcome::Failure,
                "reproducer:1",
                1,
                ProbeOutcome::Failure,
            )),
            after: Some(observation(
                AFTER_PASSES_ID,
                "cargo test parser",
                EvidenceStage::After,
                ExpectedOutcome::Success,
                "reproducer:1",
                2,
                ProbeOutcome::Success,
            )),
            regressions: vec![observation(
                NO_REGRESSION_ID,
                "cargo-test",
                EvidenceStage::After,
                ExpectedOutcome::Success,
                "regression:cargo-test",
                3,
                ProbeOutcome::Success,
            )],
        }
    }

    #[test]
    fn full_requires_f1_f2_and_the_complete_bound_regression_set() {
        let adjudication = evaluate_fix_evidence(&full_bundle());
        assert_eq!(adjudication.assurance, FixAssurance::Full);
        assert!(adjudication.reason.is_empty());
    }

    #[test]
    fn initially_passing_reproducer_is_baseline_not_reproduced() {
        let mut bundle = full_bundle();
        bundle.before.as_mut().unwrap().outcome = ProbeOutcome::Success;
        let adjudication = evaluate_fix_evidence(&bundle);
        assert_eq!(adjudication.assurance, FixAssurance::Failed);
        assert_eq!(adjudication.reason, BASELINE_NOT_REPRODUCED);
    }

    #[test]
    fn switched_after_reproducer_is_rejected_by_lineage() {
        let mut bundle = full_bundle();
        bundle.after.as_mut().unwrap().lineage = "reproducer:2".to_string();
        let adjudication = evaluate_fix_evidence(&bundle);
        assert_eq!(adjudication.assurance, FixAssurance::Failed);
        assert_eq!(adjudication.reason, "reproducer_lineage_mismatch");
    }

    #[test]
    fn shrunken_regression_set_cannot_earn_full() {
        let mut bundle = full_bundle();
        bundle.regressions.clear();
        let adjudication = evaluate_fix_evidence(&bundle);
        assert_eq!(adjudication.assurance, FixAssurance::Failed);
        assert_eq!(adjudication.reason, "regression_set_mismatch");
    }

    #[test]
    fn stale_after_epoch_is_rejected() {
        let mut bundle = full_bundle();
        bundle.after.as_mut().unwrap().epoch = 1;
        let adjudication = evaluate_fix_evidence(&bundle);
        assert_eq!(adjudication.assurance, FixAssurance::Failed);
        assert_eq!(adjudication.reason, "after_epoch_not_newer");
    }

    #[test]
    fn changed_regression_binding_is_rejected_by_lineage() {
        let mut bundle = full_bundle();
        bundle.regressions[0].lineage = "regression:changed".to_string();
        let adjudication = evaluate_fix_evidence(&bundle);
        assert_eq!(adjudication.assurance, FixAssurance::Failed);
        assert_eq!(
            adjudication.reason,
            "regression_lineage_mismatch:cargo-test"
        );
    }

    #[test]
    fn forged_execution_provenance_is_rejected() {
        let mut bundle = full_bundle();
        bundle.before.as_mut().unwrap().executed = false;
        let adjudication = evaluate_fix_evidence(&bundle);
        assert_eq!(adjudication.assurance, FixAssurance::Failed);
        assert_eq!(
            adjudication.reason,
            "execution_provenance_invalid:before_fails"
        );
    }

    #[test]
    fn unavailable_bound_regression_maps_to_partial() {
        let mut bundle = full_bundle();
        bundle.regressions[0].outcome = ProbeOutcome::Unavailable;
        bundle.regressions[0].executed = false;
        let adjudication = evaluate_fix_evidence(&bundle);
        assert_eq!(adjudication.assurance, FixAssurance::Partial);
        assert_eq!(adjudication.reason, "regression_inconclusive:cargo-test");
    }

    #[test]
    fn written_fix_with_no_executed_f_evidence_maps_to_static() {
        let bundle = FixEvidenceBundle {
            run_id: "run-1".to_string(),
            fix_written: true,
            bound_regression_ids: vec![],
            bound_regression_lineages: BTreeMap::new(),
            before: None,
            after: None,
            regressions: vec![],
        };
        let adjudication = evaluate_fix_evidence(&bundle);
        assert_eq!(adjudication.assurance, FixAssurance::Static);
        assert_eq!(adjudication.reason, "fix_probes_not_executed");
    }

    #[test]
    fn lineage_is_stable_and_sensitive_to_the_bound_command() {
        assert_eq!(
            reproducer_lineage("cargo test parser"),
            reproducer_lineage("cargo test parser")
        );
        assert_ne!(
            reproducer_lineage("cargo test parser"),
            reproducer_lineage("cargo test lexer")
        );
    }
}
