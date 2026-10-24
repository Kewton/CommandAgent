//! Final-acceptance status that follows the outcome of the final report.

/// Keeps the gate-derived status only when the final report passed; a failed
/// report is never reported as accepted.
pub(crate) fn final_acceptance_status_for_outcome(
    gate_derived_status: &'static str,
    acceptance_ok: bool,
) -> &'static str {
    if acceptance_ok {
        gate_derived_status
    } else {
        "incomplete"
    }
}

pub(crate) fn final_acceptance_unmet(final_acceptance_status: &str) -> bool {
    matches!(final_acceptance_status, "incomplete" | "failed")
}

#[cfg(test)]
mod tests;
