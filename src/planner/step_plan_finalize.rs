use crate::config::Config;
use crate::planner::lint::{PlanLintReport, lint_step_plan_report_with_workspace};
use crate::planner::step_plan::StepPlan;

pub(crate) fn finalize_step_plan_for_execution(
    plan: &mut StepPlan,
    config: &Config,
) -> PlanLintReport {
    lint_step_plan_report_with_workspace(plan, Some(&config.workspace_root))
}
