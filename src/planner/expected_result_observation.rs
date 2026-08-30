use std::path::Path;

use crate::minimal_loop::python_traceback::PythonTraceback;
use crate::planner::step_plan::{ExpectedResult, PlanStep};
use crate::planner::verify::VerificationReport;

pub(crate) fn record_python_traceback(
    report: &mut VerificationReport,
    eval_events_path: Option<&Path>,
    step: &PlanStep,
    command: &str,
    traceback: Option<PythonTraceback>,
) {
    let Some(traceback) = traceback else {
        return;
    };
    if step.expected_result_kind() != ExpectedResult::Fail {
        report.push_python_traceback(traceback);
        return;
    }
    crate::eval_events::emit(
        eval_events_path,
        serde_json::json!({
            "event": "expected_fail_python_traceback_observed",
            "step_id": step.id,
            "command": command,
            "exception_type": traceback.exception_type,
            "target_path": traceback.target_path,
            "expected_result": "fail",
            "polarity_satisfied": true,
        }),
    );
}
