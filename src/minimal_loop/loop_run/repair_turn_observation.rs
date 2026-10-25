//! Keeps a step repair session alive after a batch that changed nothing.
//!
//! A verify step may finish after one tool batch because its first
//! observation only runs commands. A repair session inherits the step's
//! options, so the same rule used to end it after a read-only batch, before
//! the model could act on what it read. Host verification still judges the
//! repair afterwards; only the early completion is deferred here.

use super::{RunSessionOptions, RunSessionScope};

/// A step repair session is the only `PlanRunStep` session that carries
/// escalation state; the planner attaches it when it starts a repair turn.
pub(super) fn is_step_repair_turn(options: &RunSessionOptions) -> bool {
    options.scope == RunSessionScope::PlanRunStep && options.escalation_carryover.is_some()
}

/// True when a repair turn must continue instead of completing on the batch
/// it just ran: the batch had no Write or Edit and recorded no changed path.
pub(super) fn defers_tool_observation_completion(
    options: &RunSessionOptions,
    batch_had_edit: bool,
    batch_changed_paths: &[String],
) -> bool {
    is_step_repair_turn(options) && !batch_had_edit && batch_changed_paths.is_empty()
}

#[cfg(test)]
#[path = "verify_repair_observation_tests.rs"]
mod verify_repair_observation_tests;
