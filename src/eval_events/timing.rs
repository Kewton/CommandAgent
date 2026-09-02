use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

const TIMED_EVENTS: &[&str] = &[
    "ultra_plan_generation_attempt",
    "ultra_plan_generation_succeeded",
    "ultra_plan_generation_failed",
    "ultra_phase_start",
    "ultra_phase_complete",
    "ultra_phase_failed",
    "tui_command_stop",
    "run_stop",
];

pub(super) fn stamp_phase_boundary(event: &mut Value) {
    let Value::Object(object) = event else {
        return;
    };
    let timed = object
        .get("event")
        .and_then(Value::as_str)
        .is_some_and(|name| TIMED_EVENTS.contains(&name));
    if !timed || object.contains_key("occurred_at_epoch_ms") {
        return;
    }
    object.insert(
        "occurred_at_epoch_ms".to_string(),
        Value::from(current_epoch_millis()),
    );
}

fn current_epoch_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn stamps_only_phase_boundaries_and_preserves_recorded_values() {
        let mut boundary = json!({"event": "ultra_phase_start"});
        stamp_phase_boundary(&mut boundary);
        assert!(boundary["occurred_at_epoch_ms"].as_u64().is_some());

        let mut recorded = json!({
            "event": "ultra_phase_complete",
            "occurred_at_epoch_ms": 42,
        });
        stamp_phase_boundary(&mut recorded);
        assert_eq!(recorded["occurred_at_epoch_ms"], 42);

        let mut unrelated = Value::Object(serde_json::Map::from_iter([(
            "event".to_string(),
            Value::String("provider_turn_duration".to_string()),
        )]));
        stamp_phase_boundary(&mut unrelated);
        assert!(unrelated.get("occurred_at_epoch_ms").is_none());
    }
}
