use std::path::Path;

use super::checks;
use crate::planner::capability_catalog::DataInternalCheck;

pub(crate) fn execute(
    root: &Path,
    check: DataInternalCheck,
    goal: Option<&str>,
) -> anyhow::Result<(bool, Vec<String>)> {
    Ok(match check {
        DataInternalCheck::InspectionSchema => {
            let evidence = checks::check_inspection_schema_with_goal(root, goal)?;
            (evidence.ok, evidence.failure_kinds)
        }
        DataInternalCheck::ResultsSchema => {
            let evidence = checks::check_results_schema(root)?;
            (evidence.ok, evidence.error.into_iter().collect())
        }
        DataInternalCheck::Reconciliation => {
            let evidence = checks::check_reconciliation(root)?;
            (evidence.ok, evidence.failure_kinds)
        }
        DataInternalCheck::ClaimsBinding => {
            let evidence = checks::check_claims_binding(root)?;
            (evidence.ok, evidence.failure_kinds)
        }
    })
}
