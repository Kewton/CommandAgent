use std::path::Path;

use serde_json::Value;

pub const INTERACTION_VERIFIED_HEURISTIC_ONLY: &str = "interaction_verified_heuristic_only";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractionFullQualification {
    pub full_eligible: bool,
    pub evidence_status: String,
    pub release_gate_reasons: Vec<String>,
}

pub fn qualify_interaction_evidence(
    value: &Value,
    restart_required: bool,
) -> InteractionFullQualification {
    let details = value
        .get("browser_details")
        .or_else(|| value.get("details"))
        .filter(|details| details.is_object());
    let probe_mode = text_field_deep(value, details, "probe_mode").unwrap_or("heuristic");
    if probe_mode != "contract" {
        return InteractionFullQualification {
            full_eligible: false,
            evidence_status: INTERACTION_VERIFIED_HEURISTIC_ONLY.to_string(),
            release_gate_reasons: vec!["contract_instrumentation_missing:primary".to_string()],
        };
    }

    let contract_hooks = object_field_deep(value, details, "contract_hooks");
    let hook_status = text_field_deep(value, details, "contract_hook_status").unwrap_or("");
    let primary_present = string_array_contains_deep(value, details, "action_hooks", "primary")
        || bool_field_deep(value, details, "primary_present") == Some(true)
        || contract_hooks
            .and_then(|hooks| hooks.get("primary_present"))
            .and_then(Value::as_bool)
            == Some(true);
    let state_changed = nonempty_string_array_deep(value, details, "state_dimensions_changed");
    let restart_present = string_array_contains_deep(value, details, "action_hooks", "restart")
        || bool_field_deep(value, details, "restart_hook_reachable_after_start") == Some(true)
        || numeric_field_deep(value, details, "restart_hook_count_after_start")
            .is_some_and(|count| count > 0)
        || contract_hooks
            .and_then(|hooks| hooks.get("restart_present"))
            .and_then(Value::as_bool)
            == Some(true);

    let mut reasons = Vec::new();
    if hook_status != "usable" || !primary_present {
        reasons.push("contract_instrumentation_missing:primary".to_string());
    }
    if !state_changed {
        reasons.push("contract_instrumentation_missing:state_change".to_string());
    }
    if restart_required && !restart_present {
        reasons.push("contract_instrumentation_missing:restart".to_string());
    }
    if reasons.is_empty() {
        InteractionFullQualification {
            full_eligible: true,
            evidence_status: "passed".to_string(),
            release_gate_reasons: reasons,
        }
    } else {
        InteractionFullQualification {
            full_eligible: false,
            evidence_status: format!("failed:{}", reasons[0]),
            release_gate_reasons: reasons,
        }
    }
}

pub fn contract_requires_restart(
    required_capabilities: &[String],
    required_evidence: &[String],
) -> bool {
    required_capabilities
        .iter()
        .any(|item| item == "start_or_restart_flow")
        || required_evidence
            .iter()
            .any(|item| item == "restart_or_recoverable_state_evidence")
}

pub(crate) fn enforce_release_gate(
    release_gate_status: &mut String,
    release_gate_reasons: &mut Vec<String>,
    interaction_evidence_status: &mut String,
    interaction_evidence_path: &str,
    restart_required: bool,
) {
    if interaction_evidence_status != "passed" {
        return;
    }
    let qualification = read_qualification(interaction_evidence_path, restart_required);
    if qualification.full_eligible {
        return;
    }
    *release_gate_status = "failed".to_string();
    *interaction_evidence_status = qualification.evidence_status;
    for reason in qualification.release_gate_reasons {
        if !release_gate_reasons.contains(&reason) {
            release_gate_reasons.push(reason);
        }
    }
}

fn read_qualification(path: &str, restart_required: bool) -> InteractionFullQualification {
    let value = Path::new(path)
        .is_file()
        .then(|| std::fs::read_to_string(path).ok())
        .flatten()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok());
    value
        .as_ref()
        .map(|value| qualify_interaction_evidence(value, restart_required))
        .unwrap_or_else(|| InteractionFullQualification {
            full_eligible: false,
            evidence_status: "failed:contract_instrumentation_evidence_unreadable".to_string(),
            release_gate_reasons: vec!["contract_instrumentation_evidence_unreadable".to_string()],
        })
}

fn text_field_deep<'a>(value: &'a Value, details: Option<&'a Value>, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str).or_else(|| {
        details
            .and_then(|details| details.get(key))
            .and_then(Value::as_str)
    })
}

fn bool_field_deep(value: &Value, details: Option<&Value>, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool).or_else(|| {
        details
            .and_then(|details| details.get(key))
            .and_then(Value::as_bool)
    })
}

fn numeric_field_deep(value: &Value, details: Option<&Value>, key: &str) -> Option<i64> {
    value.get(key).and_then(Value::as_i64).or_else(|| {
        details
            .and_then(|details| details.get(key))
            .and_then(Value::as_i64)
    })
}

fn object_field_deep<'a>(
    value: &'a Value,
    details: Option<&'a Value>,
    key: &str,
) -> Option<&'a Value> {
    value.get(key).filter(|item| item.is_object()).or_else(|| {
        details
            .and_then(|details| details.get(key))
            .filter(|item| item.is_object())
    })
}

fn array_field_deep<'a>(
    value: &'a Value,
    details: Option<&'a Value>,
    key: &str,
) -> Option<&'a Vec<Value>> {
    value.get(key).and_then(Value::as_array).or_else(|| {
        details
            .and_then(|details| details.get(key))
            .and_then(Value::as_array)
    })
}

fn string_array_contains_deep(
    value: &Value,
    details: Option<&Value>,
    key: &str,
    expected: &str,
) -> bool {
    array_field_deep(value, details, key)
        .is_some_and(|items| items.iter().any(|item| item.as_str() == Some(expected)))
}

fn nonempty_string_array_deep(value: &Value, details: Option<&Value>, key: &str) -> bool {
    array_field_deep(value, details, key).is_some_and(|items| {
        items
            .iter()
            .any(|item| item.as_str().is_some_and(|item| !item.trim().is_empty()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn contract_mode_requires_primary_and_observed_state_dimension() {
        let qualified = qualify_interaction_evidence(
            &json!({
                "probe_mode": "contract",
                "contract_hook_status": "usable",
                "action_hooks": ["primary"],
                "state_dimensions_changed": ["score"]
            }),
            false,
        );

        assert!(qualified.full_eligible, "{qualified:?}");
        assert_eq!(qualified.evidence_status, "passed");
    }

    #[test]
    fn restart_contract_requires_restart_hook_evidence() {
        let qualified = qualify_interaction_evidence(
            &json!({
                "probe_mode": "contract",
                "contract_hook_status": "usable",
                "action_hooks": ["primary"],
                "state_dimensions_changed": ["score"]
            }),
            true,
        );

        assert!(!qualified.full_eligible);
        assert_eq!(
            qualified.release_gate_reasons,
            ["contract_instrumentation_missing:restart"]
        );
    }

    #[test]
    fn heuristic_pass_remains_diagnostic_but_is_not_full_eligible() {
        let qualified = qualify_interaction_evidence(
            &json!({
                "probe_mode": "heuristic",
                "contract_hook_status": "primary_missing",
                "candidate_table": [{"rank": 1, "changed": true}],
                "input_state_change": true
            }),
            false,
        );

        assert!(!qualified.full_eligible);
        assert_eq!(
            qualified.evidence_status,
            INTERACTION_VERIFIED_HEURISTIC_ONLY
        );
        assert_eq!(
            qualified.release_gate_reasons,
            ["contract_instrumentation_missing:primary"]
        );
    }
}
