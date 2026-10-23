use crate::config::Config;
use crate::planner::lint::PlanLintReport;
use crate::planner::step_plan::{StepPlan, repair_generated_step_plan_contract};

pub use crate::planner::lint::lint_step_plan_report_with_workspace as validate_step_plan_contract;

pub(crate) fn finalize_step_plan_for_execution(
    plan: &mut StepPlan,
    config: &Config,
) -> PlanLintReport {
    repair_generated_step_plan_contract(plan);
    crate::planner::verify_producer_lint::lint_step_plan_report_for_host(plan, config)
}
