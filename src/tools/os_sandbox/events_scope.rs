//! Where the sandbox reports a downgrade, scoped to one run (Issue #773).
//!
//! The destination is process-wide, but a workflow child run replaces it with
//! its own events file. [`EventsScope`] puts the previous destination back when
//! the child ends, fails or unwinds, so a later downgrade in the parent is not
//! written to the child's file. Only the destination is saved and restored; the
//! installed policy and its downgrade state are never touched, so restoring can
//! not make `required` pass again.
//!
//! A workflow parent run enters the scope too (Issue #804): its destination is
//! `evidence/workflow-events.jsonl`, so a downgrade in the parent's own stage is
//! written there once and never to a child's file. Nothing is written for a
//! status that has not been downgraded.

use std::path::PathBuf;

use super::EVENTS_PATH;

/// Holds a run's events destination and puts the previous one back on drop.
#[must_use = "the previous destination is restored when the scope is dropped"]
pub struct EventsScope {
    previous: Option<PathBuf>,
}

impl EventsScope {
    /// Make `path` the destination until the returned guard is dropped.
    pub fn enter(path: Option<PathBuf>) -> Self {
        let previous = replace(path);
        Self { previous }
    }
}

impl Drop for EventsScope {
    fn drop(&mut self) {
        replace(self.previous.take());
    }
}

/// Swap the destination, tolerating a lock poisoned by an unwinding test or run.
fn replace(path: Option<PathBuf>) -> Option<PathBuf> {
    let mut slot = EVENTS_PATH
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    std::mem::replace(&mut *slot, path)
}

/// The current destination.
#[cfg(any(target_os = "linux", test))]
pub(crate) fn current_path() -> Option<PathBuf> {
    EVENTS_PATH
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

/// Downgrade the installed backend as a failed wrap would, for tests outside
/// this module that need a deterministic downgrade without a real backend.
#[cfg(test)]
pub(crate) fn downgrade_for_test(reason: &str) {
    super::mark_backend_unavailable(reason.to_string());
}

/// Announce the status event. Without a path nothing is written to disk. The event
/// still goes through `eval_events::emit`, but the status bus and presentation
/// have no branch for `bash_sandbox_status` yet, so it is not shown in the TUI (a
/// separate Issue).
#[cfg(any(target_os = "linux", test))]
pub(super) fn announce(path: Option<&std::path::Path>, event: serde_json::Value) {
    #[cfg(test)]
    tests::record(path, &event);
    crate::eval_events::emit(path, event);
}

#[cfg(test)]
mod tests;
