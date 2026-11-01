//! Terminal observation for a run that used up its iteration budget
//! (Issue #959).
//!
//! When the minimal loop leaves its iteration loop because the budget is
//! spent, the `loop_stop` it emits (`model_stagnation:no_progress_recorded`,
//! `loop_progress_exhausted`, and the other exhaustion reasons) says nothing
//! about the budget or about what the run last did. This leaf keeps a small
//! in-memory record of the run and adds nine fields to that one event:
//!
//! - `iteration_limit`, `iterations_used`, `iteration_budget_exhausted`;
//! - `successful_write_count` and `last_successful_write_iteration` (1-based)
//!   for Write/Edit tool calls that executed successfully;
//! - `bash_results_after_last_write`, `bash_failures_after_last_write`, and
//!   `last_bash_after_last_write_succeeded` for the Bash results that reached
//!   the execution-progress observation after the last successful write;
//! - `write_failure_after_last_write` for a Write/Edit recoverable error or
//!   rejection after the last successful write.
//!
//! Every value is a number, a bool, or null. No path, command, tool output,
//! or model text is copied. Each successful Write/Edit resets the three Bash
//! fields and the write-failure flag; a run without a write counts them from
//! its start. Bash that was rejected before execution or replaced by a policy
//! substitute never reaches the observation point and is not counted.
//!
//! Limits of meaning: these fields describe the run only at the moment the
//! budget ran out. They are not a ledger of unresolved obligations, not a
//! record of which checks ran, and not execution evidence. A successful Bash
//! result means only that the command exited successfully (a repeated command
//! counts as successful too); it says nothing about whether the command
//! verified anything. Host-side writes and files changed through Bash are not
//! Write/Edit calls and are not counted. The `loop_stop` reason and every
//! existing field are unchanged; other `loop_stop` sites never carry these
//! fields.

use serde_json::Value;

use crate::minimal_loop::execution_progress::ExecutionProgress;

/// The observation one run keeps in memory until its budget runs out.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct BudgetObservation {
    iterations_used: usize,
    successful_write_count: usize,
    last_successful_write_iteration: Option<usize>,
    bash_results_after_last_write: usize,
    bash_failures_after_last_write: usize,
    last_bash_after_last_write_succeeded: Option<bool>,
    write_failure_after_last_write: bool,
}

impl BudgetObservation {
    /// A new iteration started; `iterations_used` is 1-based.
    pub(super) fn note_iteration(&mut self, iterations_used: usize) {
        self.iterations_used = iterations_used;
    }

    /// A Write or Edit tool call executed successfully.
    pub(super) fn note_successful_write(&mut self) {
        self.successful_write_count += 1;
        self.last_successful_write_iteration = Some(self.iterations_used);
        self.bash_results_after_last_write = 0;
        self.bash_failures_after_last_write = 0;
        self.last_bash_after_last_write_succeeded = None;
        self.write_failure_after_last_write = false;
    }

    /// A tool call failed with a recoverable error or was rejected. Only a
    /// Write or Edit counts; any other tool is ignored.
    pub(super) fn note_tool_error(&mut self, tool_name: &str) {
        if matches!(tool_name, "Write" | "Edit") {
            self.write_failure_after_last_write = true;
        }
    }

    /// A Bash result reached the execution-progress observation point. New
    /// and Repeated are successes; NotSuccessful is not.
    pub(super) fn note_bash(&mut self, progress: ExecutionProgress) {
        let succeeded = progress != ExecutionProgress::NotSuccessful;
        self.bash_results_after_last_write += 1;
        if !succeeded {
            self.bash_failures_after_last_write += 1;
        }
        self.last_bash_after_last_write_succeeded = Some(succeeded);
    }

    /// Adds the nine fields to the budget-exhaustion `loop_stop`. Existing
    /// fields are never overwritten; a non-object event is returned as is.
    pub(super) fn with_fields(&self, mut event: Value, iteration_limit: usize) -> Value {
        if let Value::Object(object) = &mut event {
            for (key, value) in [
                ("iteration_limit", Value::from(iteration_limit)),
                ("iterations_used", Value::from(self.iterations_used)),
                ("iteration_budget_exhausted", Value::Bool(true)),
                (
                    "successful_write_count",
                    Value::from(self.successful_write_count),
                ),
                (
                    "last_successful_write_iteration",
                    Value::from(self.last_successful_write_iteration),
                ),
                (
                    "bash_results_after_last_write",
                    Value::from(self.bash_results_after_last_write),
                ),
                (
                    "bash_failures_after_last_write",
                    Value::from(self.bash_failures_after_last_write),
                ),
                (
                    "last_bash_after_last_write_succeeded",
                    Value::from(self.last_bash_after_last_write_succeeded),
                ),
                (
                    "write_failure_after_last_write",
                    Value::Bool(self.write_failure_after_last_write),
                ),
            ] {
                object.entry(key).or_insert(value);
            }
        }
        event
    }
}

#[cfg(test)]
mod tests;
