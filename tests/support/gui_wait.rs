//! Bounded state-wait helpers for the `gui_server` integration tests.
//!
//! Every waiter here is driven by a monotonic deadline and a bounded poll
//! interval. None of them waits forever, re-runs a failing case, or treats an
//! unobserved state as success. A waiter returns the value that satisfied its
//! predicate, and on timeout or an unsatisfiable observation it fails with the
//! last observed state so the failure names what never happened.

use std::cell::Cell;
use std::time::{Duration, Instant};

/// Explicit test-only poll interval shared by every GUI waiter.
pub const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// Explicit test-only upper bound for bringing up the `gui_server` child and
/// waiting for its stdout ready line. This is a bootstrap bound and is kept
/// separate from any product stop grace: it never extends a product timeout.
pub const SERVER_READY_TIMEOUT: Duration = Duration::from_secs(30);

/// Explicit test-only upper bound for waiting on a delegated CLI pid file.
pub const DESCENDANT_READY_TIMEOUT: Duration = Duration::from_secs(5);

/// Explicit test-only upper bound for waiting on generation-bound stop evidence.
pub const STOP_EVIDENCE_TIMEOUT: Duration = Duration::from_secs(8);

/// Explicit test-only upper bound for waiting on the trial workspace lease.
pub const IDLE_LEASE_TIMEOUT: Duration = Duration::from_secs(5);

/// Outcome of a single observation while waiting for a condition.
pub enum Poll<T> {
    /// The condition holds; the waiter returns this value.
    Ready(T),
    /// The condition does not hold yet; the payload is a short snapshot of the
    /// current state, used in the timeout message.
    Pending(String),
    /// The condition can never hold (for example the child exited early); the
    /// waiter fails immediately with this reason.
    Failed(String),
}

/// Monotonic clock used for the waiter deadlines.
pub trait Clock {
    /// Current monotonic instant.
    fn now(&self) -> Instant;
    /// Wait for `duration`, advancing the clock.
    fn sleep(&self, duration: Duration);
}

/// Wall clock backed by [`Instant`].
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl SystemClock {
    /// Create a system clock.
    pub fn new() -> Self {
        Self
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

/// Deterministic clock with a manually advanced instant, so waiter behavior can
/// be exercised without depending on real time.
#[derive(Debug, Clone)]
pub struct FakeClock {
    now: Cell<Instant>,
}

impl FakeClock {
    /// Create a fake clock starting at the current instant.
    pub fn new() -> Self {
        Self {
            now: Cell::new(Instant::now()),
        }
    }
}

impl Default for FakeClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Instant {
        self.now.get()
    }

    fn sleep(&self, duration: Duration) {
        self.now.set(self.now.get() + duration);
    }
}

/// Poll `observe` until it is ready, fails it, or the monotonic deadline passes.
///
/// A [`Poll::Failed`] observation is terminal: the waiter stops immediately
/// instead of retrying. A timeout fails with the last [`Poll::Pending`] state.
pub fn wait_for<T>(
    clock: &impl Clock,
    description: &str,
    timeout: Duration,
    mut observe: impl FnMut() -> Poll<T>,
) -> T {
    let deadline = clock.now() + timeout;
    loop {
        match observe() {
            Poll::Ready(value) => return value,
            Poll::Failed(reason) => {
                panic!("{description} can no longer be satisfied: {reason}");
            }
            Poll::Pending(state) => {
                if clock.now() >= deadline {
                    panic!(
                        "timed out after {timeout:?} waiting for {description}; last state: {state}"
                    );
                }
                clock.sleep(POLL_INTERVAL);
            }
        }
    }
}

/// Wait until `path` exists and its contents satisfy `accept`, and return them.
///
/// The predicate receives the file contents, so a caller can wait for completed
/// content instead of mere existence.
pub fn wait_for_file_content(
    clock: &impl Clock,
    path: &std::path::Path,
    description: &str,
    timeout: Duration,
    accept: impl Fn(&str) -> bool,
) -> String {
    wait_for(
        clock,
        description,
        timeout,
        || match std::fs::read_to_string(path) {
            Ok(text) if accept(&text) => Poll::Ready(text),
            Ok(text) => Poll::Pending(format!(
                "{} exists but its content is not complete yet: {:?}",
                path.display(),
                text.trim()
            )),
            Err(error) => Poll::Pending(format!("{} is not readable yet: {error}", path.display())),
        },
    )
}

/// Wait for a delegated CLI pid file and return the completed pid it records.
///
/// A pid is accepted only once the file holds a complete, newline-terminated
/// integer, so a file that exists but is empty or only partially written cannot
/// be mistaken for a ready descendant.
pub fn wait_for_descendant_pid(
    clock: &impl Clock,
    path: &std::path::Path,
    description: &str,
    timeout: Duration,
) -> i32 {
    let text = wait_for_file_content(clock, path, description, timeout, |text| {
        let Some((first_line, _)) = text.split_once('\n') else {
            return false;
        };
        let first_line = first_line.trim();
        !first_line.is_empty() && first_line.bytes().all(|byte| byte.is_ascii_digit())
    });
    text.lines()
        .next()
        .unwrap_or_default()
        .trim()
        .parse()
        .unwrap_or_else(|error| panic!("{description} recorded an unparsable pid: {error}"))
}

/// Evidence that a session's delegated process generation finished stopping.
pub struct StopEvidence {
    /// The `gui_trial_stop_completed` event bound to the requested session and
    /// generation.
    pub event: serde_json::Value,
    /// The full events log snapshot the event was read from.
    pub events_log: String,
}

/// Wait for the `gui_trial_stop_completed` event bound to `session_id` and
/// `process_generation`.
///
/// Evidence emitted for a different generation or session is never accepted, so
/// a stale stop event cannot make this waiter succeed.
pub fn wait_for_stop_evidence(
    clock: &impl Clock,
    events_path: &std::path::Path,
    session_id: &str,
    generation: &str,
    timeout: Duration,
) -> StopEvidence {
    let description =
        format!("gui_trial_stop_completed for session {session_id} generation {generation}");
    wait_for(clock, &description, timeout, || {
        let events_log = match std::fs::read_to_string(events_path) {
            Ok(text) => text,
            Err(error) => {
                return Poll::Pending(format!(
                    "{} is not readable yet: {error}",
                    events_path.display()
                ));
            }
        };
        match matching_stop_event(&events_log, session_id, generation) {
            Some(event) => Poll::Ready(StopEvidence { event, events_log }),
            None => Poll::Pending(format!(
                "{} has no gui_trial_stop_completed bound to session {session_id} generation {generation}",
                events_path.display()
            )),
        }
    })
}

fn matching_stop_event(
    events_log: &str,
    session_id: &str,
    generation: &str,
) -> Option<serde_json::Value> {
    events_log
        .lines()
        .filter(|line| !line.trim().is_empty())
        .find_map(|line| {
            let event = serde_json::from_str::<serde_json::Value>(line).ok()?;
            if event["event"] == "gui_trial_stop_completed"
                && event["session_id"] == session_id
                && event["process_generation"] == generation
            {
                Some(event)
            } else {
                None
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a scratch directory under Cargo's test target dir, never under a
    /// system prefix such as `/tmp` or `/usr`.
    fn temp_dir() -> tempfile::TempDir {
        tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap()
    }

    #[test]
    fn ready_returns_the_first_satisfying_observation() {
        let clock = FakeClock::new();
        let attempts = Cell::new(0_u32);
        let value = wait_for(&clock, "value", Duration::from_millis(100), || {
            attempts.set(attempts.get() + 1);
            if attempts.get() < 3 {
                Poll::Pending("not ready".to_string())
            } else {
                Poll::Ready(7_u32)
            }
        });
        assert_eq!(value, 7);
        assert_eq!(attempts.get(), 3);
    }

    #[test]
    #[should_panic(expected = "timed out after")]
    fn pending_past_the_deadline_fails_with_the_last_state() {
        let clock = FakeClock::new();
        let _: () = wait_for(
            &clock,
            "a never-ready state",
            Duration::from_millis(1),
            || Poll::Pending("still waiting".to_string()),
        );
    }

    #[test]
    #[should_panic(expected = "the trial workspace lease is busy")]
    fn a_lease_that_never_returns_to_idle_times_out_with_its_last_status() {
        let clock = FakeClock::new();
        let _: () = wait_for(
            &clock,
            "the trial workspace lease to return to idle",
            Duration::from_millis(1),
            || Poll::Pending("the trial workspace lease is busy".to_string()),
        );
    }

    #[test]
    #[should_panic(expected = "can no longer be satisfied")]
    fn failed_observation_stops_immediately_instead_of_retrying() {
        let clock = FakeClock::new();
        let _: () = wait_for(&clock, "a ready child", Duration::from_secs(600), || {
            Poll::Failed("the child exited early with status 1".to_string())
        });
    }

    #[test]
    fn file_waiter_accepts_only_completed_content() {
        let temp = temp_dir();
        let path = temp.path().join("descendant.pid");
        std::fs::write(&path, "1234\n").unwrap();
        let clock = FakeClock::new();
        let pid =
            wait_for_descendant_pid(&clock, &path, "the delegated pid", Duration::from_millis(1));
        assert_eq!(pid, 1234);
    }

    #[test]
    #[should_panic(expected = "content is not complete")]
    fn file_waiter_rejects_partial_content() {
        let temp = temp_dir();
        let path = temp.path().join("descendant.pid");
        std::fs::write(&path, "12").unwrap();
        let clock = FakeClock::new();
        let _ =
            wait_for_descendant_pid(&clock, &path, "the delegated pid", Duration::from_millis(1));
    }

    #[test]
    fn stop_evidence_is_bound_to_the_requested_generation() {
        let temp = temp_dir();
        let path = temp.path().join("events.jsonl");
        std::fs::write(
            &path,
            concat!(
                "{\"event\":\"gui_trial_stop_completed\",\"session_id\":\"s1\",\"process_generation\":\"g1\"}\n",
                "{\"event\":\"gui_trial_stop_completed\",\"session_id\":\"s1\",\"process_generation\":\"g2\"}\n",
            ),
        )
        .unwrap();
        let clock = FakeClock::new();
        let evidence = wait_for_stop_evidence(&clock, &path, "s1", "g2", Duration::from_millis(1));
        assert_eq!(evidence.event["process_generation"], "g2");
    }

    #[test]
    #[should_panic(expected = "no gui_trial_stop_completed bound")]
    fn stop_evidence_from_a_different_generation_is_not_success() {
        let temp = temp_dir();
        let path = temp.path().join("events.jsonl");
        std::fs::write(
            &path,
            "{\"event\":\"gui_trial_stop_completed\",\"session_id\":\"s1\",\"process_generation\":\"other\"}\n",
        )
        .unwrap();
        let clock = FakeClock::new();
        let _ = wait_for_stop_evidence(&clock, &path, "s1", "expected", Duration::from_millis(1));
    }
}
