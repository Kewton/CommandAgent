//! Which final repair a send is of (Issue #976, G5: N4 and N5).
//!
//! Only the host says a send is a final repair. A session the host opens as one gets a marker of
//! its own, made with its options, and every send of that session carries it. The first send of
//! the session opens one final repair; every turn after it, and every retry, continues that one.
//! Another session, a cycle or a regeneration as much as any, has another marker and opens another.
//! Nothing a model says, no session ID, no feedback, and no call scope decides any of it.
//!
//! Which of the two a send claims is decided where the ledger is locked, from what the marker holds,
//! and the ID the ledger gives back is written into the marker there, once. So a turn, a retry, and
//! a worker that outlived its call's deadline line up behind that lock, and a session opens one
//! final repair however they interleave.
//!
//! A send is made on a worker thread of its call. The marker of the session goes to that worker
//! when the call spawns it, as the redaction scope does, and stays with that worker: a worker
//! still out after its call gave up keeps the marker of its own call, never the next call's. That
//! carrying is the call's, so it lives in `provider_call`: nothing here is held by a thread or by
//! the process.
//!
//! With no wallet nothing reads a marker: the bytes and the purpose of a send are as before.

use std::sync::{Arc, Mutex, PoisonError};

use super::identity::{FinalRepairClaim, FinalRepairId};

/// The host's mark of one final-repair session. Clones are the same session.
#[derive(Debug, Clone, Default)]
pub(crate) struct FinalRepairMarker(Arc<Mutex<Option<FinalRepairId>>>);

impl FinalRepairMarker {
    /// A marker that already holds `id`, for a test of a claim the ledger never issued.
    #[cfg(test)]
    pub(crate) fn holding(id: FinalRepairId) -> Self {
        Self(Arc::new(Mutex::new(Some(id))))
    }

    /// What the next send of the session claims. Called with the ledger locked.
    pub(super) fn claim(&self) -> FinalRepairClaim {
        match *self.0.lock().unwrap_or_else(PoisonError::into_inner) {
            Some(id) => FinalRepairClaim::Continue(id),
            None => FinalRepairClaim::New,
        }
    }

    /// Keeps the ID the ledger issued for the session's first send. Called with the ledger locked;
    /// an ID already held stays.
    pub(super) fn opened(&self, id: Option<FinalRepairId>) {
        let mut held = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if held.is_none() {
            *held = id;
        }
    }

    /// Whether two markers are of one session.
    pub(crate) fn same_session(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
