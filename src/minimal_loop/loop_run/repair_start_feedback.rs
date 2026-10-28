//! Queues a short write advisory on the first request of a repair turn whose
//! seeded read-only streak already sits one step below write_required.
//!
//! A step repair with a short budget starts with its streak pre-advanced next
//! to the write_required threshold. The write_required feedback only appears
//! after the first read-only batch, so the model spent its first turn reading
//! and then hit the rejection limit. This advisory only adds prompt text: it
//! never activates write_required enforcement, never rejects a tool, and
//! never touches the seed, counters, budget, or carryover. A stronger
//! feedback already queued for the first request always wins.

use std::path::Path;

use serde_json::json;

use super::{RunSessionOptions, RunSessionStepKind, repair_turn_observation};
use crate::eval_events;
use crate::minimal_loop::repair_pressure::READ_ONLY_STAGNATION_WRITE_REQUIRED_THRESHOLD;

pub(super) const REPAIR_START_WRITE_GUIDANCE: &str = "Make the concrete repair with Write/Edit now using the reported failure and available source context. Keep further inspection minimal. Preserve the requested behavior and verification; a write alone is not success.";

/// A carry-bearing repair turn whose seeded streak (after the pre-advance
/// correction) is at least the write_required threshold minus one.
pub(super) fn repair_start_needs_write_guidance(
    options: &RunSessionOptions,
    initial_read_only_streak: usize,
) -> bool {
    repair_turn_observation::is_step_repair_turn(options)
        && initial_read_only_streak
            >= READ_ONLY_STAGNATION_WRITE_REQUIRED_THRESHOLD.saturating_sub(1)
}

/// Returns the advisory for the first request and records that it was put on
/// the send path. The record does not claim the model received or followed it.
pub(super) fn repair_start_write_guidance(
    eval_events_path: Option<&Path>,
    options: &RunSessionOptions,
    initial_read_only_streak: usize,
    repair_turn_budget: usize,
) -> Option<String> {
    if !repair_start_needs_write_guidance(options, initial_read_only_streak) {
        return None;
    }
    eval_events::emit(
        eval_events_path,
        json!({
            "event": "repair_start_write_guidance_queued",
            "initial_read_only_streak": initial_read_only_streak,
            "write_required_threshold": READ_ONLY_STAGNATION_WRITE_REQUIRED_THRESHOLD,
            "repair_turn_budget": repair_turn_budget,
            "enforcement_changed": false,
            "guidance": eval_events::body_snippet(REPAIR_START_WRITE_GUIDANCE),
            "session_scope": options.scope.as_str(),
            "step_kind": options.step_kind.map(RunSessionStepKind::as_str).unwrap_or(""),
            "phase_scope": options.phase_scope.as_deref().unwrap_or(""),
        }),
    );
    Some(REPAIR_START_WRITE_GUIDANCE.to_string())
}

#[cfg(test)]
mod tests;
