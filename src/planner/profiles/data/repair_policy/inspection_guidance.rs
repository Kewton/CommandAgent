use crate::planner::verify::VerificationReport;

pub(super) fn repair_text(report: &VerificationReport) -> &'static str {
    let guidance = &super::super::manifest::get().guidance;
    if report.profile_failures.iter().any(|reason| {
        reason == "data_inspection_schema" || reason.starts_with("data_inspection_schema:")
    }) {
        &guidance.canvas_game.canvas_input_wiring_checklist
    } else {
        &guidance.contracts.contract_attribute_guidance
    }
}
