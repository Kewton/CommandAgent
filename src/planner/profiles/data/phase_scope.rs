use super::{manifest, step_policy};
use crate::planner::step_plan::PlanStep;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DataSetupStepChecks {
    pub expected_paths: Vec<String>,
    pub verify_commands: Vec<String>,
}

pub(crate) fn setup_step_checks(phase_id: &str) -> Option<DataSetupStepChecks> {
    let expected_paths = canonical_artifacts(phase_id)?
        .iter()
        .map(|path| (*path).to_string())
        .collect::<Vec<_>>();
    let mut verify_commands =
        crate::planner::profile_manifest::check_phase_scope::check_ids_for_phase(
            manifest::get(),
            phase_id,
            phase_id == "data-validation",
        )
        .into_iter()
        .map(step_policy::catalog_check_command)
        .collect::<Vec<_>>();
    verify_commands.extend(expected_paths.iter().map(|path| format!("test -f {path}")));
    Some(DataSetupStepChecks {
        expected_paths,
        verify_commands,
    })
}

pub(crate) fn setup_step_checks_for_phase(
    step: &PlanStep,
    phase_id: Option<&str>,
) -> Option<DataSetupStepChecks> {
    phase_id.map_or_else(|| step_policy::setup_step_checks(step), setup_step_checks)
}

fn canonical_artifacts(phase_id: &str) -> Option<&'static [&'static str]> {
    match phase_id {
        "data-inspection" => Some(&["output/inspection.json"]),
        "data-cleaning" => Some(&["pipeline/main.py"]),
        "data-aggregation" => Some(&["pipeline/main.py", "output/results.json"]),
        "data-reporting" => Some(&["output/report.md"]),
        "data-validation" => Some(&[
            "pipeline/main.py",
            "output/inspection.json",
            "output/results.json",
            "output/report.md",
        ]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::planner::setup_step_policy::convert_preset_phase_setup_steps;
    use crate::planner::step_plan::{PlanStep, StepPlan};

    const RUN1: &str = include_str!(
        "../../../../tests/corpus/apps/test0714_data10_phase_scope/fixtures/run1-step.json"
    );
    const RUN2: &str = include_str!(
        "../../../../tests/corpus/apps/test0714_data10_phase_scope/fixtures/run2-m4001-step.json"
    );
    const SNAPSHOT: &str = include_str!(
        "../../../../tests/corpus/apps/test0714_data10_phase_scope/expected/converted-steps.json"
    );

    #[test]
    fn converted_inspection_and_final_steps_match_phase_scope_snapshot() {
        let inspection: PlanStep = serde_json::from_str(RUN1).unwrap();
        let mut final_step = inspection.clone();
        final_step.id = "validate-data-contract".to_string();
        let converted = [
            convert(inspection, "data-inspection"),
            convert(final_step, "data-validation"),
        ];
        assert_eq!(
            format!("{}\n", serde_json::to_string_pretty(&converted).unwrap()),
            SNAPSHOT
        );
    }

    #[test]
    fn both_observed_data10_steps_drop_later_phase_obligations() {
        for raw in [RUN1, RUN2] {
            let converted = convert(serde_json::from_str(raw).unwrap(), "data-inspection");
            assert_eq!(converted.expected_paths, ["output/inspection.json"]);
            assert_eq!(converted.verify.len(), 2);
            assert!(converted.verify[0].ends_with(":data_inspection_schema"));
            assert_eq!(converted.verify[1], "test -f output/inspection.json");
        }
    }
    fn convert(step: PlanStep, phase_id: &str) -> PlanStep {
        let dir = tempfile::tempdir().unwrap();
        let mut plan = StepPlan {
            goal: "Inspect sales".to_string(),
            steps: vec![step],
        };
        convert_preset_phase_setup_steps(
            &mut plan,
            dir.path(),
            "data",
            "Inspect sales",
            Some((phase_id, phase_id == "data-validation")),
            true,
            None,
        );
        plan.steps.remove(0)
    }
}
