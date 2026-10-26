//! Release-gate attribution of the restart requirement.
//!
//! A restart hook that was only seen before the game started (or counted while
//! hidden after start) does not show that restart works. The gate is full only
//! when the probe reached a restart hook after start, or attributed an
//! observed recovery to a restart control. A declared but unexercised restart
//! is partial, never full, and this module never raises a gate status.

use serde_json::Value;

use super::{
    InteractionFullQualification, bool_field_deep, numeric_field_deep, string_array_contains_deep,
    text_field_deep,
};

pub(super) const RESTART_NOT_EXERCISED_REASON: &str =
    "interaction_unverified:restart_not_exercised";
const RESTART_NOT_EXERCISED_STATUS: &str = "unverified:restart_not_exercised";

/// How far the restart requirement is backed by the probe evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RestartAttribution {
    /// Reached after start, or an observed recovery attributed to a restart control.
    Exercised,
    /// A hook was declared (before start or hidden after start) but never exercised.
    Declared,
    /// No restart hook anywhere in the evidence.
    Missing,
}

pub(super) fn attribute_restart(
    value: &Value,
    details: Option<&Value>,
    contract_hooks: Option<&Value>,
) -> RestartAttribution {
    let reachable =
        bool_field_deep(value, details, "restart_hook_reachable_after_start") == Some(true);
    let recovered_by_restart = text_field_deep(value, details, "recovery_transition_status")
        == Some("observed")
        && text_field_deep(value, details, "recovery_control") == Some("restart");
    if reachable || recovered_by_restart {
        return RestartAttribution::Exercised;
    }
    let declared = string_array_contains_deep(value, details, "action_hooks", "restart")
        || numeric_field_deep(value, details, "restart_hook_count_after_start")
            .is_some_and(|count| count > 0)
        || contract_hooks
            .and_then(|hooks| hooks.get("restart_present"))
            .and_then(Value::as_bool)
            == Some(true);
    if declared {
        RestartAttribution::Declared
    } else {
        RestartAttribution::Missing
    }
}

pub(super) fn restart_not_exercised_partial() -> InteractionFullQualification {
    InteractionFullQualification {
        full_eligible: false,
        evidence_status: RESTART_NOT_EXERCISED_STATUS.to_string(),
        release_gate_reasons: vec![RESTART_NOT_EXERCISED_REASON.to_string()],
    }
}

pub(super) fn is_restart_not_exercised(qualification: &InteractionFullQualification) -> bool {
    !qualification.full_eligible
        && qualification.release_gate_reasons == [RESTART_NOT_EXERCISED_REASON.to_string()]
}

#[cfg(test)]
#[path = "restart_attribution_tests.rs"]
mod tests;
