use std::path::Path;

use crate::planner::step_plan::{PlanStep, StepPlan};
use crate::planner::verify::VerificationReport;

pub(crate) fn canonicalize_create_plan(
    plan: &mut StepPlan,
    profile: &str,
    create_intent: bool,
    terminal_plan: bool,
    eval_events_path: Option<&Path>,
) -> usize {
    crate::planner::profiles::python_cli::readme_verify::canonicalize_step_plan(
        plan,
        profile,
        create_intent,
        eval_events_path,
    ) + crate::planner::profiles::ingest::phase_verify::canonicalize_step_plan(
        plan,
        profile,
        create_intent,
        terminal_plan,
        eval_events_path,
    )
}

pub(crate) fn is_internal_command(command: &str) -> bool {
    crate::planner::profiles::data::step_policy::catalog_check_id(command).is_some()
        || crate::planner::profiles::python_cli::readme_verify::is_check_command(command)
        || crate::planner::profiles::ingest::phase_verify::is_check_command(command)
}

pub(crate) fn run(
    root: &Path,
    profile: Option<&str>,
    goal: Option<&str>,
    step: &PlanStep,
    eval_events_path: Option<&Path>,
    report: &mut VerificationReport,
) {
    crate::planner::profiles::data::step_policy::run_step_catalog_checks(
        root,
        profile,
        goal,
        step,
        eval_events_path,
        report,
    );
    crate::planner::profiles::python_cli::readme_verify::run_step_check(
        root, profile, step, report,
    );
    crate::planner::profiles::ingest::phase_verify::run_step_check(root, profile, step, report);
}
