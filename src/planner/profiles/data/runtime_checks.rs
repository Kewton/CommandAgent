use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use super::{checks, internal_checks};
use crate::planner::capability_catalog::{
    DataInternalCheck, InternalCapability, ResolvedCapability,
};

pub(crate) struct InternalCheckSummary {
    pub statuses: BTreeMap<String, bool>,
    pub reasons: Vec<String>,
}

pub(crate) fn run(root: &Path, goal: Option<&str>) -> anyhow::Result<InternalCheckSummary> {
    let mut summary = InternalCheckSummary {
        statuses: BTreeMap::new(),
        reasons: Vec::new(),
    };
    for check in all() {
        let (ok, failures) = internal_checks::execute(root, check, goal)?;
        summary.statuses.insert(id(check).to_string(), ok);
        if check == DataInternalCheck::ResultsSchema {
            summary.reasons.extend(
                failures
                    .into_iter()
                    .map(|failure| format!("data_results_schema:{failure}")),
            );
        } else {
            summary.reasons.extend(failures);
        }
    }
    Ok(summary)
}

pub(crate) fn observed(root: &Path) -> BTreeMap<String, bool> {
    BTreeMap::from([
        (
            id(DataInternalCheck::InspectionSchema).to_string(),
            read::<checks::InspectionSchemaEvidence>(root, checks::INSPECTION_SCHEMA_EVIDENCE_PATH)
                .is_some_and(|evidence| evidence.ok),
        ),
        (
            id(DataInternalCheck::ResultsSchema).to_string(),
            read::<checks::ResultsSchemaEvidence>(root, checks::RESULTS_SCHEMA_EVIDENCE_PATH)
                .is_some_and(|evidence| evidence.ok),
        ),
        (
            id(DataInternalCheck::Reconciliation).to_string(),
            read::<checks::ReconciliationEvidence>(root, checks::RECONCILIATION_EVIDENCE_PATH)
                .is_some_and(|evidence| evidence.ok),
        ),
        (
            id(DataInternalCheck::ClaimsBinding).to_string(),
            read::<checks::ClaimsBindingEvidence>(root, checks::CLAIMS_BINDING_EVIDENCE_PATH)
                .is_some_and(|evidence| evidence.ok),
        ),
    ])
}

pub(crate) fn adapters_complete(adapters: &[ResolvedCapability]) -> bool {
    all().into_iter().all(|check| {
        adapters.contains(&ResolvedCapability::Internal(InternalCapability::Data(
            check,
        )))
    })
}

fn all() -> [DataInternalCheck; 4] {
    [
        DataInternalCheck::InspectionSchema,
        DataInternalCheck::ResultsSchema,
        DataInternalCheck::Reconciliation,
        DataInternalCheck::ClaimsBinding,
    ]
}

fn id(check: DataInternalCheck) -> &'static str {
    match check {
        DataInternalCheck::InspectionSchema => "data_inspection_schema",
        DataInternalCheck::ResultsSchema => "data_results_schema",
        DataInternalCheck::Reconciliation => "data_reconciliation",
        DataInternalCheck::ClaimsBinding => "data_claims_binding",
    }
}

fn read<T: for<'de> Deserialize<'de>>(root: &Path, relative: &str) -> Option<T> {
    let text =
        std::fs::read_to_string(crate::tools::path_guard::resolve_existing(root, relative).ok()?)
            .ok()?;
    serde_json::from_str(&text).ok()
}

#[cfg(test)]
pub(crate) fn write_valid_inspection(root: &Path) {
    for directory in ["pipeline", "input", "output"] {
        std::fs::create_dir_all(root.join(directory)).unwrap();
    }
    std::fs::write(
        root.join("input/source.csv"),
        "category,amount\nA,10\nB,2.5\nC,\n",
    )
    .unwrap();
    std::fs::write(root.join("output/inspection.json"), r#"{"column_names":["category","amount"],"input_row_count":3,"type_summaries":{"category":"string","amount":"numeric"},"distinct_values":{"category":["A","B","C"]},"sample_rows":[{"category":"A","amount":"10"}]}"#).unwrap();
}
