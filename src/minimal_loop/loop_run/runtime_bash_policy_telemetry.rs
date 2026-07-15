use std::path::Path;

use serde_json::json;

use super::RuntimeBashPolicyDecision;
use crate::eval_events;

pub(super) fn emit_policy(
    path: Option<&Path>,
    decision: &RuntimeBashPolicyDecision,
    command: &str,
) {
    let mut event = json!({
        "event": "runtime_bash_policy",
        "tool_name": "Bash",
        "step_kind": decision.step_kind,
        "bash_policy_purpose": decision.bash_policy_purpose,
        "verifier_policy_checked": decision.verifier_policy_checked,
        "verifier_policy_ok": decision.verifier_policy_ok,
        "deterministic_verifier_evidence": decision.deterministic_verifier_evidence,
        "blocked": decision.blocked,
        "policy_error_kind": decision.policy_error_kind,
        "verify_command_violation_kind": decision.violation_kind,
        "normalization_kind": decision.normalization_kind,
        "normalized_command_summary": decision.normalized_command.as_deref().map(eval_events::body_snippet).unwrap_or_default(),
        "reason": eval_events::body_snippet(&decision.reason),
        "command_summary": eval_events::body_snippet(command),
    });
    if decision.verifier_policy_checked
        && (decision.blocked || !decision.normalization_kind.is_empty())
        && let Some(fields) = event.as_object_mut()
    {
        fields.insert("original_command".to_string(), json!(command));
        fields.insert(
            "violation_kind".to_string(),
            json!(if decision.blocked {
                decision.violation_kind
            } else {
                decision.normalization_kind
            }),
        );
        fields.insert(
            "normalized_commands".to_string(),
            json!(normalized_commands(decision)),
        );
    }
    eval_events::emit(path, event);
}

pub(super) fn emit_normalization(
    path: Option<&Path>,
    decision: &RuntimeBashPolicyDecision,
    command: &str,
    normalized_command: &str,
) {
    eval_events::emit(
        path,
        json!({
            "event": "verify_command_normalized_at_runtime",
            "classification": "deterministic_verify_policy",
            "kind": decision.normalization_kind,
            "normalization_kind": decision.normalization_kind,
            "normalization_source": decision.normalization_kind,
            "original": eval_events::body_snippet(command),
            "original_command": command,
            "repaired": eval_events::body_snippet(normalized_command),
            "normalized_commands": normalized_commands(decision),
            "reason": eval_events::body_snippet(&decision.normalization_reason),
        }),
    );
}

fn normalized_commands(decision: &RuntimeBashPolicyDecision) -> Vec<&str> {
    decision
        .split_segments
        .iter()
        .map(|segment| segment.command.as_str())
        .collect()
}
