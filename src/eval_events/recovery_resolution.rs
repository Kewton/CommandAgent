//! Resolve Recovery handoffs across isolated treatment transactions.

use serde_json::Value;

const RECOVERY_FIELDS: &[&str] = &[
    "recovery_prompt_path",
    "recovery_ultra_plan_path",
    "suggested_recovery_command",
    "suggested_recovery_yaml_command",
];

/// Return events from the Recovery lineage retained by the transaction log.
///
/// A treatment can emit a newer handoff while it runs, but that handoff is not
/// authoritative until the treatment is promoted. Rejected and unresolved
/// treatments therefore leave the pre-treatment control lineage intact.
pub(super) fn resolved_recovery_events(events: &[Value]) -> Vec<&Value> {
    let mut control = Vec::new();
    let mut treatment: Option<Vec<&Value>> = None;
    let mut terminal_fallback = Vec::new();
    let mut saw_source_handoff = false;

    for event in events {
        let name = event.get("event").and_then(Value::as_str);
        if name == Some("recovery_plan_auto_run_complete")
            && event
                .get("recovery_plan_auto_run_stop_reason")
                .and_then(Value::as_str)
                == Some("recovery_succeeded")
        {
            control.clear();
            treatment = None;
            terminal_fallback.clear();
            saw_source_handoff = false;
            continue;
        }

        match name {
            Some("tui_command_stop" | "run_stop") if has_recovery_fields(event) => {
                // Terminal events project an already selected handoff. Keep
                // them only for legacy streams that have no source record.
                terminal_fallback.push(event);
            }
            Some("recovery_plan_auto_run_start") => {
                // The start names the control candidate being attempted, not a
                // handoff produced by the treatment that follows it.
                if has_recovery_fields(event) {
                    saw_source_handoff = true;
                    control.push(event);
                }
                treatment = Some(Vec::new());
            }
            Some("recovery_treatment_promoted") => {
                promote(&mut control, &mut treatment);
            }
            Some("recovery_control_retained") => {
                treatment = None;
            }
            Some("recovery_promotion_decision") => {
                match event.get("decision").and_then(Value::as_str) {
                    Some("promoted") => promote(&mut control, &mut treatment),
                    Some("rejected") => treatment = None,
                    _ => {}
                }
            }
            _ if has_recovery_fields(event) => {
                saw_source_handoff = true;
                match treatment.as_mut() {
                    Some(staged) => staged.push(event),
                    None => control.push(event),
                }
            }
            _ => {}
        }
    }

    if control.is_empty() && !saw_source_handoff {
        terminal_fallback
    } else {
        control
    }
}

fn promote<'a>(control: &mut Vec<&'a Value>, treatment: &mut Option<Vec<&'a Value>>) {
    if let Some(staged) = treatment.take() {
        control.extend(staged);
    }
}

fn has_recovery_fields(event: &Value) -> bool {
    RECOVERY_FIELDS.iter().any(|field| {
        event
            .get(*field)
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn plan_paths(events: &[Value]) -> Vec<&str> {
        resolved_recovery_events(events)
            .into_iter()
            .filter_map(|event| {
                event
                    .get("recovery_ultra_plan_path")
                    .and_then(Value::as_str)
            })
            .collect()
    }

    fn control() -> Value {
        json!({
            "event": "recovery_prompt_saved",
            "recovery_ultra_plan_path": ".commandagent/plans/control.yaml"
        })
    }

    fn start() -> Value {
        json!({
            "event": "recovery_plan_auto_run_start",
            "recovery_ultra_plan_path": ".commandagent/plans/control.yaml",
            "recovery_treatment_path": ".commandagent/recovery-treatments/attempt-1/workspace"
        })
    }

    fn treatment() -> Value {
        json!({
            "event": "recovery_prompt_saved",
            "recovery_ultra_plan_path": ".commandagent/recovery-treatments/attempt-1/workspace/.commandagent/plans/treatment.yaml"
        })
    }

    #[test]
    fn rejected_treatment_keeps_the_control_recovery_lineage() {
        let events = vec![
            control(),
            start(),
            treatment(),
            json!({"event": "recovery_control_retained"}),
            json!({"event": "recovery_promotion_decision", "decision": "rejected"}),
            json!({
                "event": "tui_command_stop",
                "recovery_ultra_plan_path": ".commandagent/recovery-treatments/attempt-1/workspace/.commandagent/plans/treatment.yaml"
            }),
        ];

        assert_eq!(
            plan_paths(&events),
            [
                ".commandagent/plans/control.yaml",
                ".commandagent/plans/control.yaml"
            ]
        );
    }

    #[test]
    fn promoted_treatment_commits_its_recovery_lineage() {
        let events = vec![
            control(),
            start(),
            treatment(),
            json!({"event": "recovery_treatment_promoted"}),
            json!({"event": "recovery_promotion_decision", "decision": "promoted"}),
            json!({
                "event": "tui_command_stop",
                "recovery_ultra_plan_path": ".commandagent/plans/control.yaml"
            }),
        ];

        assert_eq!(
            plan_paths(&events).last().copied(),
            Some(
                ".commandagent/recovery-treatments/attempt-1/workspace/.commandagent/plans/treatment.yaml"
            )
        );
    }

    #[test]
    fn unresolved_treatment_does_not_replace_control_and_success_clears_it() {
        let unresolved = vec![control(), start(), treatment()];
        assert_eq!(
            plan_paths(&unresolved).last().copied(),
            Some(".commandagent/plans/control.yaml")
        );

        let succeeded = vec![
            control(),
            json!({
                "event": "recovery_plan_auto_run_complete",
                "recovery_plan_auto_run_stop_reason": "recovery_succeeded"
            }),
        ];
        assert!(plan_paths(&succeeded).is_empty());
    }

    #[test]
    fn legacy_terminal_only_handoff_remains_available() {
        let events = vec![json!({
            "event": "tui_command_stop",
            "recovery_ultra_plan_path": ".anvil/plans/legacy.yaml"
        })];

        assert_eq!(plan_paths(&events), [".anvil/plans/legacy.yaml"]);
    }
}
