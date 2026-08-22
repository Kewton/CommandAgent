use crate::planner::step_plan::{StepPlan, render_step_plan};
use crate::planner::ultra_plan::{UltraPlan, render_ultra_plan};

const EDIT_HEADER: &str = "# CommandAgent editable plan YAML. Comments are ignored.\n\
# Edit values, add or remove list items, then validate before execution.\n\
# Validation never executes the plan: commandagent --validate-plan <path>\n";

pub fn render_editable_step_plan(plan: &StepPlan) -> String {
    format!(
        "{EDIT_HEADER}\
# goal: overall outcome; steps run in order.\n\
# kind: inspect, setup, implement, verify, or report.\n\
# expected_result: pass or fail; paths and verify commands are optional lists.\n\
# Run only after validation: commandagent --run-plan <path>\n\
{}",
        render_step_plan(plan)
    )
}

pub fn render_editable_ultra_plan(plan: &UltraPlan) -> String {
    format!(
        "{EDIT_HEADER}\
# profile/style/intent preserve the generated execution context.\n\
# phases run in order; each id must be unique and each prompt a focused task.\n\
# Run only after validation: commandagent --run-ultra-plan <path>\n\
{}",
        render_ultra_plan(plan)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::step_plan::{PlanStep, parse_step_plan};
    use crate::planner::ultra_plan::parse_ultra_plan;

    #[test]
    fn editable_step_template_is_commented_and_loadable() {
        let plan = StepPlan {
            goal: "edit the docs".to_string(),
            steps: vec![PlanStep {
                id: "update-docs".to_string(),
                kind: "implement".to_string(),
                expected_result: "pass".to_string(),
                instruction: "Update docs/guide.md.".to_string(),
                expected_paths: vec!["docs/guide.md".to_string()],
                verify: Vec::new(),
            }],
        };
        let rendered = render_editable_step_plan(&plan);
        assert!(rendered.contains("# kind: inspect, setup, implement, verify, or report."));
        assert!(rendered.contains("commandagent --validate-plan <path>"));
        assert_eq!(parse_step_plan(&rendered).unwrap(), plan);
    }

    #[test]
    fn editable_ultra_template_is_commented_and_loadable() {
        let plan = UltraPlan::deterministic("goal", "generic", "default", "create");
        let rendered = render_editable_ultra_plan(&plan);
        assert!(rendered.contains("# phases run in order"));
        assert!(rendered.contains("commandagent --run-ultra-plan <path>"));
        assert_eq!(parse_ultra_plan(&rendered).unwrap(), plan);
    }
}
