use super::FixFailureDiagnostic;
use crate::planner::ultra_plan::UltraPhase;

pub(super) fn diagnostic_phase(phase: &UltraPhase) -> bool {
    matches!(phase.id.as_str(), "isolate-cause" | "repair")
}

pub(super) fn render_guidance(diagnostic: &FixFailureDiagnostic) -> String {
    let location = if diagnostic.line > 0 && diagnostic.column > 0 {
        format!(
            "{}:{}:{}",
            diagnostic.target_path, diagnostic.line, diagnostic.column
        )
    } else if diagnostic.line > 0 {
        format!("{}:{}", diagnostic.target_path, diagnostic.line)
    } else {
        diagnostic.target_path.clone()
    };
    let mut guidance = format!(
        "Fix F1 failure diagnostic (runtime-derived):\n- location: {location}\n- error kind: {}\n- message: {}\n- write-pressure target: {} (selection_reason={})",
        diagnostic.error_kind,
        crate::eval_events::body_snippet(&diagnostic.message),
        diagnostic.target_path,
        diagnostic.selection_reason.as_str(),
    );
    if !diagnostic.excerpt.trim().is_empty() {
        guidance.push_str("\n- excerpt: ");
        guidance.push_str(&crate::eval_events::body_snippet(&diagnostic.excerpt));
    }
    guidance
}
