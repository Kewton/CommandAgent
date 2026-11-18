//! The dispatch state of one provider call's request (Issue #965).
//!
//! The Issue #957 notice and the Issue #958 sentence each guard the request
//! that carries them. When one request carries both, the two guards share one
//! [`RequestDispatch`], so whether the request reached the client is decided
//! once and both read the same answer. The state moves once, either
//! `Pending → Dispatched` (a watch, right before it asks the client) or
//! `Pending → Closed` (the run, once the provider call returned).

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

const PENDING: u8 = 0;
const DISPATCHED: u8 = 1;
const CLOSED: u8 = 2;

/// The error a watch returns instead of asking the client once the run has
/// closed the request; it is the provider call's own interruption text.
pub(crate) const CLOSED_ERROR: &str = "aborted_by_user: interrupted by user";

/// One request's dispatch state. Clones share it.
#[derive(Debug, Clone)]
pub(crate) struct RequestDispatch(Arc<AtomicU8>);

impl RequestDispatch {
    pub(crate) fn new() -> Self {
        Self(Arc::new(AtomicU8::new(PENDING)))
    }

    /// The worker side: takes the state to dispatched, or finds it already
    /// dispatched (a second call of the same request). False when the run
    /// closed it; the worker must then not ask the client.
    pub(crate) fn dispatch(&self) -> bool {
        match self
            .0
            .compare_exchange(PENDING, DISPATCHED, Ordering::SeqCst, Ordering::SeqCst)
        {
            Ok(_) => true,
            Err(current) => current == DISPATCHED,
        }
    }

    /// The run side, after the provider call returned: closes a request that
    /// is still pending. Returns whether the request was dispatched; every
    /// call gives the same answer.
    pub(crate) fn close(&self) -> bool {
        match self
            .0
            .compare_exchange(PENDING, CLOSED, Ordering::SeqCst, Ordering::SeqCst)
        {
            Ok(_) => false,
            Err(current) => current == DISPATCHED,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RequestDispatch;

    #[test]
    fn the_state_moves_once_and_every_reader_sees_the_same_answer() {
        let dispatched = RequestDispatch::new();
        assert!(dispatched.dispatch());
        assert!(dispatched.clone().dispatch());
        assert!(dispatched.close());
        assert!(dispatched.close());
        assert!(dispatched.dispatch());

        let closed = RequestDispatch::new();
        assert!(!closed.close());
        assert!(!closed.clone().dispatch());
        assert!(!closed.close());
    }
}
