//! Where the sandbox reports a downgrade, scoped to one run (Issue #773).
//!
//! The destination is process-wide, but a workflow child run replaces it with
//! its own events file. [`EventsScope`] puts the previous destination back when
//! the child ends, fails or unwinds, so a later downgrade in the parent is not
//! written to the child's file. Only the destination is saved and restored; the
//! installed policy and its downgrade state are never touched, so restoring can
//! not make `required` pass again.

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
pub(super) fn current_path() -> Option<PathBuf> {
    EVENTS_PATH
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

/// Announce the status event. Without a path nothing is written to disk, but the
/// event still goes through `eval_events::emit`, which projects it to the TUI.
#[cfg(any(target_os = "linux", test))]
pub(super) fn announce(path: Option<&std::path::Path>, event: serde_json::Value) {
    #[cfg(test)]
    tests::record(path, &event);
    crate::eval_events::emit(path, event);
}

#[cfg(test)]
mod tests;
