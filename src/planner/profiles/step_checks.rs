use std::path::Path;

use crate::planner::step_plan::PlanStep;
use crate::planner::verify::VerificationReport;

pub(crate) fn is_internal_command(command: &str) -> bool {
    crate::planner::profiles::data::step_policy::catalog_check_id(command).is_some()
        || crate::planner::profiles::python_cli::readme_verify::is_check_command(command)
        || crate::planner::profiles::ingest::phase_verify::is_check_command(command)
        || crate::planner::profiles::nextjs::package_script_check::is_step_check_command(command)
}

const HOST_CHECK_NOTE: &str =
    "  (host check: the host judges it after the step; do not run it in Bash)\n";

/// The step prompt's verification list. A host check token gets a note on the next,
/// un-bulleted line, so a reader that takes each `- ` line as a command is unaffected.
pub(crate) fn verify_prompt_bullets(verify: &[String]) -> String {
    if verify.is_empty() {
        return "- none\n".to_string();
    }
    let mut out = String::new();
    for item in verify {
        out.push_str(&format!("- {item}\n"));
        let host_checked = is_internal_command(item)
            || crate::planner::profiles::nextjs::package_script_check::is_check_command(item);
        if host_checked {
            out.push_str(HOST_CHECK_NOTE);
        }
    }
    out
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
    crate::planner::profiles::nextjs::package_script_check::run_step_check(
        root, profile, step, report,
    );
}
