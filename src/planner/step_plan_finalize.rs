use crate::config::Config;
use crate::planner::lint::{PlanLintReport, lint_step_plan_report_with_workspace};
use crate::planner::step_plan::{StepPlan, repair_generated_step_plan_contract};

pub use crate::planner::lint::lint_step_plan_report_with_workspace as validate_step_plan_contract;

pub(crate) fn finalize_step_plan_for_execution(
    plan: &mut StepPlan,
    config: &Config,
) -> PlanLintReport {
    repair_generated_step_plan_contract(plan);
    lint_step_plan_report_with_workspace(plan, Some(&config.workspace_root))
}
