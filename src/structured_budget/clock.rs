//! The run's wall clock.
//!
//! Inside a process, the monotonic reading keeps counting whatever the wall clock does. Across
//! processes, only the saved start and high-water mark are there, so the time between processes
//! is whatever the wall clock says, and never less than was already counted. Moving the clock back
//! takes nothing off. The start is the first reservation or submission, and it never moves.

use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Where the two readings come from. A test injects its own.
pub(crate) trait BudgetClock: Send + Sync + std::fmt::Debug {
    fn wall_ms(&self) -> u64;
    fn monotonic_ms(&self) -> u64;
}

#[derive(Debug)]
pub(crate) struct SystemBudgetClock {
    origin: Instant,
}

impl SystemBudgetClock {
    pub(crate) fn new() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl BudgetClock for SystemBudgetClock {
    fn wall_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| {
                u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
            })
    }

    fn monotonic_ms(&self) -> u64 {
        u64::try_from(self.origin.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

/// The part of the clock that crosses a process, in the ledger.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ClockRecord {
    pub(crate) started_wall_ms: Option<u64>,
    pub(crate) high_water_elapsed_ms: u64,
    /// Elapsed run time when the first final repair began.
    pub(crate) final_repair_started_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy)]
struct Anchor {
    monotonic_ms: u64,
    elapsed_ms: u64,
}

/// The clock of one process over one wallet.
#[derive(Debug)]
pub(crate) struct RunClock {
    source: Arc<dyn BudgetClock>,
    anchor: Option<Anchor>,
}

impl RunClock {
    pub(crate) fn new(source: Arc<dyn BudgetClock>) -> Self {
        Self {
            source,
            anchor: None,
        }
    }

    /// The elapsed run time, which also raises the record's high-water mark. Zero before the run
    /// has started.
    pub(crate) fn observe(&mut self, record: &mut ClockRecord) -> u64 {
        let Some(started_wall_ms) = record.started_wall_ms else {
            return 0;
        };
        let monotonic_ms = self.source.monotonic_ms();
        let anchor = *self.anchor.get_or_insert(Anchor {
            monotonic_ms,
            elapsed_ms: record.high_water_elapsed_ms,
        });
        let counted_here = anchor
            .elapsed_ms
            .saturating_add(monotonic_ms.saturating_sub(anchor.monotonic_ms));
        let by_the_wall = self.source.wall_ms().saturating_sub(started_wall_ms);
        record.high_water_elapsed_ms = record
            .high_water_elapsed_ms
            .max(counted_here)
            .max(by_the_wall);
        record.high_water_elapsed_ms
    }

    /// The start of the run. Only the first call does anything.
    pub(crate) fn begin(&mut self, record: &mut ClockRecord) {
        if record.started_wall_ms.is_none() {
            record.started_wall_ms = Some(self.source.wall_ms());
            self.anchor = Some(Anchor {
                monotonic_ms: self.source.monotonic_ms(),
                elapsed_ms: record.high_water_elapsed_ms,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{FakeClock, run_clock};
    use super::{BudgetClock, ClockRecord, SystemBudgetClock};

    const START: u64 = 1_000_000;

    fn begun() -> (std::sync::Arc<FakeClock>, super::RunClock, ClockRecord) {
        let source = FakeClock::at(START);
        let mut clock = run_clock(&source);
        let mut record = ClockRecord::default();
        clock.begin(&mut record);
        (source, clock, record)
    }

    #[test]
    fn the_system_clock_moves_forward() {
        let clock = SystemBudgetClock::new();
        let before = clock.monotonic_ms();
        std::thread::sleep(std::time::Duration::from_millis(5));
        assert!(clock.monotonic_ms() >= before + 2);
        // Well after 2020 by any machine that runs this.
        assert!(clock.wall_ms() > 1_577_836_800_000);
    }

    #[test]
    fn nothing_has_elapsed_before_the_first_reservation() {
        let source = FakeClock::at(START);
        let mut clock = run_clock(&source);
        let mut record = ClockRecord::default();
        source.advance(5_000);
        assert_eq!(clock.observe(&mut record), 0);
        assert_eq!(record, ClockRecord::default());
    }

    #[test]
    fn the_start_does_not_move_after_the_first_reservation() {
        let (source, mut clock, mut record) = begun();
        assert_eq!(record.started_wall_ms, Some(START));
        source.advance(700);
        clock.begin(&mut record);
        assert_eq!(record.started_wall_ms, Some(START));
        assert_eq!(clock.observe(&mut record), 700);
    }

    #[test]
    fn a_wall_clock_set_back_inside_a_process_takes_nothing_off() {
        let (source, mut clock, mut record) = begun();
        source.advance(1_000);
        assert_eq!(clock.observe(&mut record), 1_000);
        source.set_wall(START - 500_000);
        assert_eq!(clock.observe(&mut record), 1_000);
        assert_eq!(record.high_water_elapsed_ms, 1_000);
    }

    #[test]
    fn the_monotonic_reading_keeps_counting_while_the_wall_clock_is_behind() {
        let (source, mut clock, mut record) = begun();
        source.advance(1_000);
        source.set_wall(START - 500_000);
        source.advance_monotonic_only(300);
        assert_eq!(clock.observe(&mut record), 1_300);
    }

    #[test]
    fn a_wall_clock_moved_forward_counts_against_the_run() {
        let (source, mut clock, mut record) = begun();
        source.set_wall(START + 9_000);
        assert_eq!(clock.observe(&mut record), 9_000);
    }

    #[test]
    fn a_new_process_counts_the_time_between_processes() {
        let (source, mut clock, mut record) = begun();
        source.advance(1_000);
        clock.observe(&mut record);
        drop(clock);
        source.new_process();
        source.set_wall(START + 40_000);
        let mut next = run_clock(&source);
        assert_eq!(next.observe(&mut record), 40_000);
    }

    #[test]
    fn a_new_process_with_the_clock_set_back_starts_from_the_high_water_mark() {
        let (source, mut clock, mut record) = begun();
        source.advance(2_000);
        clock.observe(&mut record);
        drop(clock);
        source.new_process();
        source.set_wall(START - 900_000);
        let mut next = run_clock(&source);
        assert_eq!(next.observe(&mut record), 2_000);
        source.advance_monotonic_only(5);
        assert_eq!(next.observe(&mut record), 2_005);
    }
}
