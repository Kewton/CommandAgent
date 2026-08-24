use crate::planner::verify::VerificationReport;

pub(super) fn repair_text(report: &VerificationReport) -> &'static str {
    if report.profile_failures.iter().any(|reason| {
        reason == "data_inspection_schema" || reason.starts_with("data_inspection_schema:")
    }) {
        super::super::manifest::guidance_message("inspection", "canvas_input_wiring_checklist")
    } else {
        super::super::manifest::guidance_message("contracts", "contract_attribute_guidance")
    }
}
