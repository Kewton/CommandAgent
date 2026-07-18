use std::path::Path;

use serde_json::json;

use crate::eval_events;
use crate::planner::step_plan::{StepKind, StepPlan};

/// Bind safe defaults for fix phases after planner sanitization removes every
/// generated verify command.  This is deliberately separate from the
/// data-create phase table: fix phases need F1/F2/F3 semantics even though
/// they are not declared in the data manifest's create-phase scopes.
pub(crate) fn bind_empty_fix_verify_steps(
    plan: &mut StepPlan,
    phase_id: Option<&str>,
    eval_events_path: Option<&Path>,
) -> usize {
    let Some(phase_id) = phase_id else { return 0 };
    let checks: &[&str] = match phase_id {
        "reproduce-before" => &["pipeline_probe", "test -f pipeline/main.py"],
        // Cause isolation is read-only: profile checks are observations only.
        "isolate-cause" => &[
            "data_results_schema",
            "data_reconciliation",
            "data_claims_binding",
        ],
        // Repair must prove the subject exists and remains contract-valid.
        "repair" => &[
            "test -f pipeline/main.py",
            "data_results_schema",
            "data_reconciliation",
            "data_claims_binding",
        ],
        "verify-regressions" => &[
            "pipeline_probe",
            "data_results_schema",
            "data_reconciliation",
            "data_claims_binding",
            "data_rerun_consistency",
        ],
        _ => return 0,
    };
    plan.steps
        .iter_mut()
        .filter(|step| step.step_kind() == StepKind::Verify && step.verify.is_empty())
        .map(|step| {
            step.verify = checks
                .iter()
                .map(|check| {
                    if check.starts_with("data_") || *check == "pipeline_probe" {
                        format!("anvil-catalog-check:{check}")
                    } else {
                        (*check).to_string()
                    }
                })
                .collect();
            eval_events::emit(
                eval_events_path,
                json!({
                    "event": "verify_default_bound",
                    "phase_id": phase_id,
                    "step_id": step.id,
                    "bound_checks": step.verify,
                    "intent": "fix",
                }),
            );
            1
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::step_plan::PlanStep;

    #[test]
    fn fix_phase_empty_verify_steps_get_contract_defaults() {
        let mut plan = StepPlan {
            goal: "fix pipeline failure".to_string(),
            steps: vec![PlanStep {
                id: "verify-regressions".to_string(),
                kind: "verify".to_string(),
                expected_result: "pass".to_string(),
                instruction: "Verify the repaired data pipeline".to_string(),
                expected_paths: Vec::new(),
                verify: Vec::new(),
            }],
        };
        assert_eq!(
            bind_empty_fix_verify_steps(&mut plan, Some("verify-regressions"), None),
            1
        );
        assert!(
            plan.steps[0]
                .verify
                .iter()
                .any(|check| check == "anvil-catalog-check:pipeline_probe")
        );
        assert!(
            plan.steps[0]
                .verify
                .iter()
                .any(|check| check == "anvil-catalog-check:data_rerun_consistency")
        );
    }
}
