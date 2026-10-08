//! Private wait-loop boundary for the provider call chokepoint.
//!
//! The provider wait loop checks cancellation, checks the configured deadline,
//! and blocks for one bounded slice on each iteration. Production drives that
//! loop with a real monotonic `Instant` and a blocking channel receive. Tests
//! inject a virtual clock and a boundary that never blocks, so the timeout and
//! wait-slice semantics can be verified without taking a real wait slice.
//!
//! This module is a leaf: it owns only the loop control flow. It does not know
//! about provider messages, telemetry, or event schemas.

use std::time::Duration;

/// Monotonic elapsed-time source for the provider wait loop.
pub(super) trait WaitClock {
    fn elapsed(&mut self) -> Duration;
}

/// Result of blocking one slice at the loop boundary.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum WaitSignal<M> {
    /// A worker message is ready to consume.
    Ready(M),
    /// The slice elapsed with nothing to consume.
    Elapsed,
    /// The worker side is gone and no further message can arrive.
    Disconnected,
}

/// Blocking boundary for one provider wait slice.
pub(super) trait WaitBoundary<M> {
    fn wait(&mut self, slice: Duration) -> WaitSignal<M>;
}

/// Control outcome of one provider wait-loop iteration.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum WaitStep<M> {
    /// The caller asked to cancel.
    Cancelled,
    /// The configured deadline was reached.
    Deadline,
    /// A worker message is ready to consume.
    Message(M),
    /// The slice elapsed with no message; loop again.
    Sliced,
    /// The worker side is gone.
    Disconnected,
}

/// The next slice the loop may block for, or `None` once the deadline has
/// passed. The slice never overshoots the remaining time to the deadline, so a
/// clock that advances only by explicit slices reaches `None` exactly at the
/// timeout.
pub(super) fn next_wait_slice(
    elapsed: Duration,
    timeout: Duration,
    slice: Duration,
) -> Option<Duration> {
    if elapsed >= timeout {
        None
    } else {
        Some(slice.min(timeout - elapsed))
    }
}

/// One iteration of the provider wait-loop control flow.
///
/// Both the clock and the boundary are injected. `on_slice` runs with the
/// elapsed time only when the loop is about to block, matching the production
/// progress-emission point.
pub(super) fn wait_step<C, B, F, M>(
    clock: &mut C,
    boundary: &mut B,
    timeout: Duration,
    slice: Duration,
    is_cancelled: F,
    mut on_slice: impl FnMut(Duration),
) -> WaitStep<M>
where
    C: WaitClock,
    B: WaitBoundary<M>,
    F: FnOnce() -> bool,
{
    if is_cancelled() {
        return WaitStep::Cancelled;
    }
    let elapsed = clock.elapsed();
    let Some(next) = next_wait_slice(elapsed, timeout, slice) else {
        return WaitStep::Deadline;
    };
    on_slice(elapsed);
    match boundary.wait(next) {
        WaitSignal::Ready(message) => WaitStep::Message(message),
        WaitSignal::Elapsed => WaitStep::Sliced,
        WaitSignal::Disconnected => WaitStep::Disconnected,
    }
}
