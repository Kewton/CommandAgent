use std::path::Path;

use serde_json::json;

use crate::eval_events;
use crate::planner::profiles::data::phase_scope::{self, DataSetupStepChecks};
use crate::planner::step_plan::{StepKind, StepPlan};

mod fix;
pub(crate) use fix::bind_empty_fix_verify_steps;

pub(crate) fn bind_empty_verify_steps(
    plan: &mut StepPlan,
    phase_scope: Option<(&str, bool)>,
    eval_events_path: Option<&Path>,
) -> usize {
    let Some((phase_id, final_phase)) = phase_scope else {
        return 0;
    };
    let Some(binding_phase) = binding_phase_id(phase_id, final_phase) else {
        return 0;
    };
    let Some(checks) = phase_scope::setup_step_checks(binding_phase) else {
        return 0;
    };
    if checks.verify_commands.is_empty() {
        return 0;
    }
    plan.steps
        .iter_mut()
        .filter(|step| step.step_kind() == StepKind::Verify && step.verify.is_empty())
        .map(|step| bind_step(step, phase_id, &checks, eval_events_path))
        .sum()
}

fn binding_phase_id(phase_id: &str, final_phase: bool) -> Option<&'static str> {
    if final_phase {
        return Some("data-validation");
    }
    match phase_id {
        "data-inspection" => return Some("data-inspection"),
        "data-cleaning" => return Some("data-cleaning"),
        "data-aggregation" => return Some("data-aggregation"),
        "data-reporting" => return Some("data-reporting"),
        "data-validation" => return Some("data-validation"),
        _ => {}
    }
    let tokens = phase_id
        .to_ascii_lowercase()
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if tokens
        .iter()
        .any(|token| matches!(token.as_str(), "inspect" | "inspection" | "load"))
    {
        Some("data-inspection")
    } else if tokens.iter().any(|token| {
        matches!(
            token.as_str(),
            "clean" | "cleaning" | "filter" | "filtering"
        )
    }) {
        Some("data-cleaning")
    } else if tokens.iter().any(|token| {
        matches!(
            token.as_str(),
            "aggregate" | "aggregation" | "aggregations" | "compute" | "calculate"
        )
    }) {
        Some("data-aggregation")
    } else if tokens.iter().any(|token| {
        matches!(
            token.as_str(),
            "report" | "reporting" | "summary" | "summarize" | "generate"
        )
    }) {
        Some("data-reporting")
    } else {
        None
    }
}

fn bind_step(
    step: &mut crate::planner::step_plan::PlanStep,
    phase_id: &str,
    checks: &DataSetupStepChecks,
    eval_events_path: Option<&Path>,
) -> usize {
    step.expected_paths = checks.expected_paths.clone();
    step.verify = checks.verify_commands.clone();
    eval_events::emit(
        eval_events_path,
        json!({
            "event": "verify_default_bound",
            "phase_id": phase_id,
            "step_id": step.id,
            "bound_checks": step.verify,
        }),
    );
    1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::minimal_loop::dependency_setup::NodeDependencySetupAuthority;
    use crate::planner::lint::lint_step_plan_report;
    use crate::planner::profiles::data::step_policy::canonicalize_step_plan;
    use crate::planner::step_plan::PlanStep;
    use crate::planner::verify::verify_step_with_profile_setup_observed_with_offline;

    const RUN3_EVENTS: &str = include_str!(
        "../../../../../tests/corpus/apps/test0715_data12_verify_default/fixtures/data6_qwen35_none_001/verify-canonicalized-events.jsonl"
    );
    const RUN1_FIXTURE_ROOT: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/corpus/apps/test0715_data12_pre_satisfied/fixtures/data6_qwen35_profile_001"
    );

    fn measured_run3_plan() -> StepPlan {
        StepPlan {
            goal: "Load and inspect the measured sales data".to_string(),
            steps: RUN3_EVENTS
                .lines()
                .filter_map(|line| {
                    let event: serde_json::Value = serde_json::from_str(line).unwrap();
                    (event["event"] == "verify_canonicalized").then(|| PlanStep {
                        id: event["step_id"].as_str().unwrap().to_string(),
                        kind: "verify".to_string(),
                        expected_result: "pass".to_string(),
                        instruction: "Verify the generated results contract".to_string(),
                        expected_paths: vec!["output/results.json".to_string()],
                        verify: vec![event["original"].as_str().unwrap().to_string()],
                    })
                })
                .collect(),
        }
    }

    #[test]
    fn measured_run3_zero_verify_steps_bind_and_execute_inspection_checks() {
        let dir = tempfile::tempdir().unwrap();
        for relative in ["data/sales.csv", "output/inspection.json"] {
            let target = dir.path().join(relative);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::copy(Path::new(RUN1_FIXTURE_ROOT).join(relative), target).unwrap();
        }
        let events = dir.path().join("events.jsonl");
        let mut plan = measured_run3_plan();
        let measured = RUN3_EVENTS
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .collect::<Vec<_>>();

        assert!(measured[..2].iter().all(|event| {
            event["event"] == "verify_canonicalized" && event["replacement"] == "advisory"
        }));
        assert_eq!(
            measured[2]["planner_error_message"],
            "verify step requires at least one verify command"
        );
        assert_eq!(
            canonicalize_step_plan(
                &mut plan,
                Some(("load-and-inspect-data", false)),
                Some(&events),
            ),
            4
        );
        for step in &plan.steps {
            assert_eq!(step.expected_paths, ["output/inspection.json"]);
            assert_eq!(
                step.verify,
                [
                    "anvil-catalog-check:data_inspection_schema",
                    "test -f output/inspection.json",
                ]
            );
            let (report, _) = verify_step_with_profile_setup_observed_with_offline(
                dir.path(),
                step,
                Some("data"),
                NodeDependencySetupAuthority::None,
                true,
            );
            assert!(report.is_pass(), "{}", report.primary_reason());
        }
        assert!(lint_step_plan_report(&plan).is_pass());
        let bound = std::fs::read_to_string(events)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .filter(|event| event["event"] == "verify_default_bound")
            .collect::<Vec<_>>();
        assert_eq!(bound.len(), 2);
        assert_eq!(bound[0]["step_id"], "verify-results-schema");
        assert_eq!(
            bound[0]["bound_checks"],
            serde_json::json!([
                "anvil-catalog-check:data_inspection_schema",
                "test -f output/inspection.json"
            ])
        );
    }

    #[test]
    fn phase_without_a_binding_keeps_the_existing_empty_verify_error() {
        let mut plan = StepPlan {
            goal: "Unknown phase".to_string(),
            steps: vec![PlanStep {
                id: "verify-unknown".to_string(),
                kind: "verify".to_string(),
                expected_result: "pass".to_string(),
                instruction: "Verify an unknown phase".to_string(),
                expected_paths: Vec::new(),
                verify: Vec::new(),
            }],
        };

        assert_eq!(
            bind_empty_verify_steps(&mut plan, Some(("unmapped-phase", false)), None),
            0
        );
        let report = lint_step_plan_report(&plan);
        assert_eq!(
            report.primary_message(),
            "verify step requires at least one verify command"
        );
    }

    #[test]
    fn dynamic_final_uses_only_final_default_checks() {
        let mut plan = StepPlan {
            goal: "Final verification".to_string(),
            steps: vec![PlanStep {
                id: "verify-final".to_string(),
                kind: "verify".to_string(),
                expected_result: "pass".to_string(),
                instruction: "Verify final data outputs".to_string(),
                expected_paths: Vec::new(),
                verify: Vec::new(),
            }],
        };

        assert_eq!(
            bind_empty_verify_steps(&mut plan, Some(("generate-report-and-verify", true)), None,),
            1
        );
        assert!(
            plan.steps[0]
                .verify
                .iter()
                .any(|command| command.ends_with(":data_rerun_consistency"))
        );
        assert!(
            plan.steps[0]
                .verify
                .iter()
                .all(|command| !command.ends_with(":data_inspection_schema"))
        );
    }
}
