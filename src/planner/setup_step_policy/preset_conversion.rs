use std::path::Path;

use serde_json::json;

use crate::eval_events;
use crate::planner::profiles::data::step_policy::{
    canonicalize_step_plan, verify_default::bind_empty_fix_verify_steps,
};
use crate::planner::step_plan::StepPlan;

use super::{
    is_data_profile, is_implementation_phase, merge_unique_paths, profile_owns_declared_paths,
    profile_setup_checks, references_template_owned_artifacts,
};

pub(crate) fn convert_preset_phase_setup_steps(
    plan: &mut StepPlan,
    root: &Path,
    profile: &str,
    goal: &str,
    phase_scope: Option<(&str, bool)>,
    preset_phase: bool,
    eval_events_path: Option<&Path>,
) -> usize {
    let phase_id = phase_scope.map(|(id, _)| id);
    let mut converted = if is_data_profile(profile) {
        if phase_scope.is_some_and(|(id, _)| {
            matches!(
                id,
                "reproduce-before" | "isolate-cause" | "repair" | "verify-regressions"
            )
        }) {
            bind_empty_fix_verify_steps(plan, phase_id, eval_events_path)
        } else {
            canonicalize_step_plan(plan, phase_scope, eval_events_path)
        }
    } else {
        0
    };
    let phase_supports_conversion = phase_id.is_some_and(|phase_id| {
        if is_data_profile(profile) {
            crate::planner::profiles::data::step_policy::preset_phase_supports_conversion(phase_id)
        } else {
            is_implementation_phase(phase_id)
        }
    });
    if !preset_phase || !phase_supports_conversion {
        return converted;
    }
    for step in &mut plan.steps {
        if is_data_profile(profile)
            && !crate::planner::profiles::data::step_policy::supports_verify_conversion(step)
        {
            continue;
        }
        if !references_template_owned_artifacts(profile, step) {
            continue;
        }
        let Some(checks) = profile_setup_checks(root, profile, goal, step, phase_id) else {
            continue;
        };
        if !profile_owns_declared_paths(root, profile, step) {
            continue;
        }
        step.kind = "verify".to_string();
        step.expected_result = "pass".to_string();
        step.instruction = format!(
            "Verify the profile-owned {} contract by running every declared check and report any exact failure.",
            checks.ownership
        );
        if is_data_profile(profile) {
            step.expected_paths = checks.expected_paths;
        } else {
            merge_unique_paths(&mut step.expected_paths, checks.expected_paths);
        }
        step.verify = checks.verify_commands;
        eval_events::emit(
            eval_events_path,
            json!({
                "event": "preset_step_converted",
                "phase_id": phase_id,
                "step_id": step.id,
                "ownership": checks.ownership,
            }),
        );
        converted += 1;
    }
    converted
}

#[cfg(test)]
use crate::planner::step_plan::PlanStep;

#[cfg(test)]
pub(super) fn setup_scripts_step() -> PlanStep {
    PlanStep {
        id: "setup-scripts".to_string(),
        kind: "setup".to_string(),
        expected_result: "pass".to_string(),
        instruction: "Confirm the package is ready.".to_string(),
        expected_paths: Vec::new(),
        verify: Vec::new(),
    }
}

#[cfg(test)]
pub(super) fn ensure_port_scripts_implement_step() -> PlanStep {
    PlanStep {
        id: "ensure-port-scripts".to_string(),
        kind: "implement".to_string(),
        expected_result: "pass".to_string(),
        instruction: "Update package.json scripts to use port 3011.".to_string(),
        expected_paths: Vec::new(),
        verify: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_implementation_setup_is_converted_and_emitted() {
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join("events.jsonl");
        let mut plan = StepPlan {
            goal: "Implement the app".to_string(),
            steps: vec![setup_scripts_step()],
        };

        let count = convert_preset_phase_setup_steps(
            &mut plan,
            dir.path(),
            "nextjs",
            "Build a Next.js app",
            Some(("core-implementation", false)),
            true,
            Some(&events),
        );

        assert_eq!(count, 1);
        assert_eq!(plan.steps[0].kind, "verify");
        assert!(!plan.steps[0].verify.is_empty());
        let event = std::fs::read_to_string(events).unwrap();
        assert_eq!(
            event,
            "{\"event\":\"preset_step_converted\",\"ownership\":\"package_manifest\",\"phase_id\":\"core-implementation\",\"schema_version\":\"1\",\"step_id\":\"setup-scripts\"}\n"
        );
    }

    #[test]
    fn preset_implementation_port_step_is_converted_independent_of_kind() {
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join("events.jsonl");
        let mut plan = StepPlan {
            goal: "Implement the app".to_string(),
            steps: vec![ensure_port_scripts_implement_step()],
        };

        let count = convert_preset_phase_setup_steps(
            &mut plan,
            dir.path(),
            "nextjs",
            "Build a Next.js app on port 3011",
            Some(("core-implementation", false)),
            true,
            Some(&events),
        );

        assert_eq!(count, 1);
        assert_eq!(plan.steps[0].kind, "verify");
        assert_eq!(plan.steps[0].expected_paths, ["package.json"]);
        assert!(!plan.steps[0].verify.is_empty());
        let event = std::fs::read_to_string(events).unwrap();
        assert!(event.contains("\"event\":\"preset_step_converted\""));
        assert!(event.contains("\"step_id\":\"ensure-port-scripts\""));
    }

    #[test]
    fn data_inspection_invention_is_canonicalized_then_converted() {
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join("events.jsonl");
        let mut plan = StepPlan {
            goal: "Inspect sales".to_string(),
            steps: vec![PlanStep {
                id: "inspect-data".to_string(),
                kind: "inspect".to_string(),
                expected_result: "pass".to_string(),
                instruction: "Write data/inspection_report.md and verify its literal headings."
                    .to_string(),
                expected_paths: vec!["data/inspection_report.md".to_string()],
                verify: vec!["grep -q 'Rows inspected' data/inspection_report.md".to_string()],
            }],
        };

        let changes = convert_preset_phase_setup_steps(
            &mut plan,
            dir.path(),
            "data",
            "Inspect sales",
            Some(("data-inspection", false)),
            true,
            Some(&events),
        );

        assert!(changes >= 2);
        assert_eq!(plan.steps[0].kind, "verify");
        assert_eq!(plan.steps[0].expected_paths, ["output/inspection.json"]);
        assert_eq!(plan.steps[0].verify.len(), 2);
        assert!(plan.steps[0].verify[0].ends_with(":data_inspection_schema"));
        assert_eq!(plan.steps[0].verify[1], "test -f output/inspection.json");
        let event = std::fs::read_to_string(events).unwrap();
        assert!(event.contains("\"event\":\"verify_canonicalized\""));
        assert!(event.contains("\"replacement\":\"advisory\""));
        assert!(event.contains("\"event\":\"preset_step_converted\""));
        assert!(event.contains("\"ownership\":\"data_manifest_artifact\""));
    }

    #[test]
    fn data_reporting_step_is_canonicalized_without_verify_conversion() {
        let dir = tempfile::tempdir().unwrap();
        let mut plan = StepPlan {
            goal: "Report sales".to_string(),
            steps: vec![PlanStep {
                id: "write-report".to_string(),
                kind: "report".to_string(),
                expected_result: "pass".to_string(),
                instruction: "Write output/sales_summary_report.md from results.".to_string(),
                expected_paths: vec!["output/sales_summary_report.md".to_string()],
                verify: Vec::new(),
            }],
        };

        assert_eq!(
            convert_preset_phase_setup_steps(
                &mut plan,
                dir.path(),
                "data",
                "Report sales",
                Some(("data-reporting", false)),
                true,
                None,
            ),
            1
        );
        assert_eq!(plan.steps[0].kind, "report");
        assert_eq!(plan.steps[0].expected_paths, ["output/report.md"]);
    }

    #[test]
    fn preset_game_logic_implement_step_is_not_converted() {
        let dir = tempfile::tempdir().unwrap();
        let mut plan = StepPlan {
            goal: "Implement Breakout".to_string(),
            steps: vec![PlanStep {
                id: "implement-gameplay".to_string(),
                kind: "implement".to_string(),
                expected_result: "pass".to_string(),
                instruction: "Implement paddle movement, ball collision, and scoring.\n\nProfile contract:\nKeep package scripts on port 3011.".to_string(),
                expected_paths: vec!["src/app/page.tsx".to_string()],
                verify: vec!["npm run build".to_string()],
            }],
        };

        assert!(!references_template_owned_artifacts(
            "nextjs",
            &plan.steps[0]
        ));
        assert_eq!(
            convert_preset_phase_setup_steps(
                &mut plan,
                dir.path(),
                "nextjs",
                "Build a Next.js app on port 3011",
                Some(("core-implementation", false)),
                true,
                None,
            ),
            0
        );
        assert_eq!(plan.steps[0].kind, "implement");
    }

    #[test]
    fn setup_step_with_non_template_path_is_not_converted() {
        let dir = tempfile::tempdir().unwrap();
        let mut plan = StepPlan {
            goal: "Implement a game".to_string(),
            steps: vec![PlanStep {
                id: "setup-scripts".to_string(),
                kind: "setup".to_string(),
                expected_result: "pass".to_string(),
                instruction: "Configure package scripts and implement the game.".to_string(),
                expected_paths: vec!["src/app/game.tsx".to_string()],
                verify: Vec::new(),
            }],
        };

        assert_eq!(
            convert_preset_phase_setup_steps(
                &mut plan,
                dir.path(),
                "nextjs",
                "Build a Next.js app",
                Some(("core-implementation", false)),
                true,
                None,
            ),
            0
        );
        assert_eq!(plan.steps[0].kind, "setup");
    }
}
