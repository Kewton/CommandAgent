//! Whole-record appends to event files (Issue #987).
//!
//! Every writer of an events file in this process builds its record (the JSON
//! text and its `\n`) before it gets here, then writes it under one
//! process-wide lock. While a record, or a provider turn's event line and its
//! spooled tail, is being written, no other writer in this process can put
//! bytes into the file, whatever spelling of the path it opened. Partial writes
//! and `Interrupted` are retried to the end of the record; any other error is
//! returned to the caller, which reports it. Writers in other processes are
//! outside this guarantee (see `docs/dev/maintainability-architecture.md`).
//!
//! Nothing called under the lock emits, redacts, publishes, or touches the
//! provider-turn spool registry, so a flush can never re-enter the lock.
use std::fs::OpenOptions;
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde_json::Value;

/// One lock for every events file: a per-path lock would let two spellings of
/// one file (relative, `..`, a symlink) write at the same time.
static WRITE_LOCK: Mutex<()> = Mutex::new(());

/// Append `event` as one line, creating the parent directory first.
pub(crate) fn append_event(path: &Path, event: &Value) -> anyhow::Result<()> {
    append_event_with_tail(path, event, &mut io::empty())
}

/// Append `event` as one line and then `tail` (whole lines) under one lock, so
/// no other line of this process lands between them or inside them.
pub(crate) fn append_event_with_tail(
    path: &Path,
    event: &Value,
    tail: &mut dyn Read,
) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut record = serde_json::to_vec(event)?;
    record.push(b'\n');
    append_locked(path, &record, tail)?;
    Ok(())
}

/// Append a line its caller already built, ending in `\n`. The parent
/// directory must exist.
pub(crate) fn append_line(path: &Path, line: &[u8]) -> io::Result<()> {
    append_locked(path, line, &mut io::empty())
}

fn append_locked(path: &Path, record: &[u8], tail: &mut dyn Read) -> io::Result<()> {
    let _guard = lock();
    let file = OpenOptions::new().create(true).append(true).open(path)?;
    #[cfg(test)]
    let file = test_seam::PauseAt::for_this_thread(file);
    write_record(file, record, tail)
}

fn write_record(mut out: impl Write, record: &[u8], tail: &mut dyn Read) -> io::Result<()> {
    out.write_all(record)?;
    #[cfg(test)]
    test_seam::before_tail();
    io::copy(tail, &mut out)?;
    out.flush()
}

fn lock() -> MutexGuard<'static, ()> {
    #[cfg(test)]
    test_seam::observe_lock(&WRITE_LOCK);
    // The guard protects no data, so a panic while it was held leaves nothing
    // half-updated; later writers keep going.
    WRITE_LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Test-only seams: stop a writer at a chosen point of its bytes, and see when
/// another writer has to wait for the lock. Both are thread-local scopes.
#[cfg(test)]
pub(crate) mod test_seam {
    use std::cell::{Cell, RefCell};
    use std::io::{self, Write};
    use std::sync::Mutex;
    use std::sync::mpsc::{Receiver, Sender, channel};
    use std::time::Duration;

    /// Long enough never to fire on a healthy run; it only turns a hang into a
    /// failure.
    pub(crate) const HANG_GUARD: Duration = Duration::from_secs(60);

    /// Where a writer stops inside its bytes: after `newlines` line ends, then
    /// `bytes` more.
    #[derive(Debug, Clone, Copy)]
    pub(crate) struct Point {
        pub(crate) newlines: usize,
        pub(crate) bytes: usize,
    }

    pub(crate) struct Plan {
        /// `None`: once the record is written, before the tail is copied.
        point: Option<Point>,
        paused: Sender<()>,
        resume: Receiver<()>,
    }

    /// The test's side of a plan. Dropping it lets the stopped writer go on,
    /// so a failing assertion never leaves a writer holding the lock.
    pub(crate) struct Control {
        paused: Receiver<()>,
        resume: Option<Sender<()>>,
    }

    impl Control {
        pub(crate) fn wait_paused(&self) {
            self.paused
                .recv_timeout(HANG_GUARD)
                .expect("the writer reaches its pause point");
        }

        pub(crate) fn resume(&mut self) {
            self.resume.take();
        }
    }

    pub(crate) fn plan(point: Point) -> (Plan, Control) {
        plan_at(Some(point))
    }

    /// Stop after the record's write returned and before the tail is copied.
    pub(crate) fn plan_before_tail() -> (Plan, Control) {
        plan_at(None)
    }

    fn plan_at(point: Option<Point>) -> (Plan, Control) {
        let (paused_tx, paused_rx) = channel();
        let (resume_tx, resume_rx) = channel();
        (
            Plan {
                point,
                paused: paused_tx,
                resume: resume_rx,
            },
            Control {
                paused: paused_rx,
                resume: Some(resume_tx),
            },
        )
    }

    thread_local! {
        static PAUSE: RefCell<Option<Plan>> = const { RefCell::new(None) };
        static PROBE: RefCell<Option<Sender<Seen>>> = const { RefCell::new(None) };
        static LOCK_CALLS: Cell<usize> = const { Cell::new(0) };
    }

    /// The next record this thread writes stops at the plan's point.
    pub(crate) struct PauseScope;

    impl PauseScope {
        pub(crate) fn enter(plan: Plan) -> Self {
            PAUSE.with_borrow_mut(|slot| *slot = Some(plan));
            Self
        }
    }

    impl Drop for PauseScope {
        fn drop(&mut self) {
            PAUSE.with_borrow_mut(|slot| *slot = None);
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum Seen {
        /// The writer found the lock held and waits for it.
        Contended,
        /// The writer's call returned (sent by the test).
        Wrote,
    }

    /// While entered, this thread reports `Contended` when it finds the write
    /// lock held.
    pub(crate) struct ContentionProbe;

    impl ContentionProbe {
        pub(crate) fn enter(sender: Sender<Seen>) -> Self {
            PROBE.with_borrow_mut(|slot| *slot = Some(sender));
            Self
        }
    }

    impl Drop for ContentionProbe {
        fn drop(&mut self) {
            PROBE.with_borrow_mut(|slot| *slot = None);
        }
    }

    impl Plan {
        fn stop(self) {
            let _ = self.paused.send(());
            // A dropped `Control` also ends the wait.
            let _ = self.resume.recv();
        }
    }

    pub(super) fn before_tail() {
        if let Some(plan) = PAUSE.with_borrow_mut(Option::take) {
            plan.stop();
        }
    }

    /// How many times this thread has taken the write lock.
    pub(crate) fn lock_calls() -> usize {
        LOCK_CALLS.get()
    }

    pub(super) fn observe_lock(lock: &Mutex<()>) {
        LOCK_CALLS.set(LOCK_CALLS.get() + 1);
        PROBE.with_borrow(|probe| {
            if let Some(sender) = probe
                && matches!(lock.try_lock(), Err(std::sync::TryLockError::WouldBlock))
            {
                let _ = sender.send(Seen::Contended);
            }
        });
    }

    /// A writer that stops once at a point of the bytes written through it,
    /// whatever the chunking of the calls.
    pub(crate) struct PauseAt<W> {
        inner: W,
        plan: Option<Plan>,
        newlines: usize,
        since_newline: usize,
    }

    impl<W: Write> PauseAt<W> {
        pub(crate) fn new(inner: W, plan: Option<Plan>) -> Self {
            Self {
                inner,
                plan,
                newlines: 0,
                since_newline: 0,
            }
        }

        pub(super) fn for_this_thread(inner: W) -> Self {
            let plan = PAUSE.with_borrow_mut(|slot| slot.take_if(|plan| plan.point.is_some()));
            Self::new(inner, plan)
        }

        fn stop_index(&self, point: Point, buf: &[u8]) -> Option<usize> {
            let (mut newlines, mut since) = (self.newlines, self.since_newline);
            for (index, byte) in buf.iter().enumerate() {
                if newlines == point.newlines && since == point.bytes {
                    return Some(index);
                }
                if *byte == b'\n' {
                    newlines += 1;
                    since = 0;
                } else {
                    since += 1;
                }
            }
            None
        }

        fn count(&mut self, written: &[u8]) {
            for byte in written {
                if *byte == b'\n' {
                    self.newlines += 1;
                    self.since_newline = 0;
                } else {
                    self.since_newline += 1;
                }
            }
        }
    }

    impl<W: Write> Write for PauseAt<W> {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let mut limit = buf.len();
            if let Some(point) = self.plan.as_ref().and_then(|plan| plan.point) {
                match self.stop_index(point, buf) {
                    Some(0) => self.plan.take().unwrap().stop(),
                    Some(index) => limit = index,
                    None => {}
                }
            }
            let written = self.inner.write(&buf[..limit])?;
            self.count(&buf[..written]);
            Ok(written)
        }

        fn flush(&mut self) -> io::Result<()> {
            self.inner.flush()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs::File;
    use std::path::PathBuf;
    use std::sync::mpsc::channel;
    use std::sync::{Arc, Barrier};

    use serde_json::json;
    use sha2::Digest;

    use super::test_seam::{self, ContentionProbe, PauseAt, PauseScope, Plan, Point, Seen};
    use super::*;
    use crate::eval_events::provider_turn::ExecutionTelemetry;
    use crate::eval_events::{append_event_failsafe, emit};

    const CORPUS_CASES: &str =
        include_str!("../../tests/corpus/apps/event-line-atomicity/fixtures/line-cases.json");
    const CORPUS_MANIFEST: &str =
        include_str!("../../tests/corpus/apps/event-line-atomicity/source-manifest.json");

    /// Every physical line is one JSON object and the file ends with LF. Read
    /// as bytes: neither `str::lines()` nor a stream parser is used, and no
    /// line is skipped.
    fn strict_lines(bytes: &[u8]) -> Result<Vec<Value>, String> {
        if bytes.is_empty() {
            return Ok(Vec::new());
        }
        let body = bytes.strip_suffix(b"\n").ok_or("the last line has no LF")?;
        let mut events = Vec::new();
        for (index, line) in body.split(|byte| *byte == b'\n').enumerate() {
            let shown = String::from_utf8_lossy(line);
            match serde_json::from_slice::<Value>(line) {
                Ok(event @ Value::Object(_)) => events.push(event),
                Ok(_) => return Err(format!("line {} is not an object: {shown}", index + 1)),
                Err(err) => return Err(format!("line {}: {err}: {shown}", index + 1)),
            }
        }
        Ok(events)
    }

    fn ids(events: &[Value]) -> Vec<String> {
        events
            .iter()
            .map(|event| event["id"].as_str().unwrap_or("<no id>").to_string())
            .collect()
    }

    /// The strict lines hold exactly `expected` (by id, once each, each with
    /// the fields it was given).
    fn check_exact(bytes: &[u8], expected: &[Value]) -> Result<Vec<Value>, String> {
        let events = strict_lines(bytes)?;
        let mut got = ids(&events);
        let mut want = ids(expected);
        got.sort();
        want.sort();
        if got != want {
            return Err(format!("ids differ: got {got:?}, want {want:?}"));
        }
        for want in expected {
            let event = events
                .iter()
                .find(|event| event["id"] == want["id"])
                .unwrap();
            for (key, value) in want.as_object().unwrap() {
                if &event[key] != value {
                    return Err(format!("{} changed field {key}", want["id"]));
                }
            }
        }
        Ok(events)
    }

    /// The turn's event line is followed at once by its whole tail, in order.
    fn check_turn(events: &[Value], turn: &str, tail: usize) -> Result<(), String> {
        let ids = ids(events);
        let at = ids
            .iter()
            .position(|id| id == turn)
            .ok_or(format!("{turn} is missing"))?;
        let want = (0..tail).map(|index| format!("{turn}-tail-{index}"));
        let got = ids.iter().skip(at + 1).take(tail).cloned();
        if !got.eq(want) {
            return Err(format!(
                "{turn}: another line sits inside its tail: {ids:?}"
            ));
        }
        Ok(())
    }

    fn read(path: &Path) -> Vec<u8> {
        std::fs::read(path).unwrap()
    }

    fn fixture(id: &str) -> Value {
        json!({"event":"fixture", "id":id, "text":"改行\\n と \"引用\" と 🧪\n", "pad":"x".repeat(300)})
    }

    fn duration(id: &str) -> Value {
        json!({"event":"provider_turn_duration", "id":id, "caller_scope":"executor",
            "tool_calls_in_response":0, "write_or_edit_succeeded":false})
    }

    fn tail_event(turn: &str, index: usize) -> Value {
        json!({"event":"fixture", "id":format!("{turn}-tail-{index}"), "turn":turn,
            "seq":index, "payload":"ü".repeat(1_024)})
    }

    /// One provider turn: its duration event, then a tail of `tail` events the
    /// scope spools until `finish` flushes them.
    fn spool_turn(path: &Path, turn: &str, tail: usize) -> ExecutionTelemetry {
        let scope = ExecutionTelemetry::new(Some(path));
        emit(Some(path), duration(turn));
        for index in 0..tail {
            append_event_failsafe(Some(path), tail_event(turn, index)).unwrap();
        }
        scope
    }

    fn turn_events(turn: &str, tail: usize) -> Vec<Value> {
        std::iter::once(duration(turn))
            .chain((0..tail).map(|index| tail_event(turn, index)))
            .collect()
    }

    /// The writers at develop fc903248: `writeln!` into an append handle, and a
    /// provider turn's tail copied through a second handle after its event.
    mod legacy {
        use super::*;

        pub(super) fn open(path: &Path) -> File {
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .unwrap()
        }

        pub(super) fn append(mut out: impl Write, event: &Value) {
            writeln!(out, "{}", serde_json::to_string(event).unwrap()).unwrap();
        }

        pub(super) fn flush(path: &Path, events: &[Value], plan: Plan) {
            append(open(path), &events[0]);
            let mut tail = Vec::new();
            for event in &events[1..] {
                serde_json::to_writer(&mut tail, event).unwrap();
                tail.push(b'\n');
            }
            let file = OpenOptions::new().append(true).open(path).unwrap();
            io::copy(&mut tail.as_slice(), &mut PauseAt::new(file, Some(plan))).unwrap();
        }
    }

    /// Writer A runs until it stops at `point`; then writer B runs on another
    /// thread until its first report (`Contended` from the lock, or `Wrote`
    /// once its call returned); then A finishes. Returns B's first report.
    fn interleave<A, B>(point: Point, a: A, b: B) -> Seen
    where
        A: FnOnce(Plan) + Send + 'static,
        B: FnOnce() + Send + 'static,
    {
        interleave_with(test_seam::plan(point), a, b)
    }

    fn interleave_with<A, B>((plan, mut control): (Plan, test_seam::Control), a: A, b: B) -> Seen
    where
        A: FnOnce(Plan) + Send + 'static,
        B: FnOnce() + Send + 'static,
    {
        let writer_a = std::thread::spawn(move || a(plan));
        control.wait_paused();
        let (seen_tx, seen_rx) = channel();
        let writer_b = std::thread::spawn(move || {
            let _probe = ContentionProbe::enter(seen_tx.clone());
            b();
            let _ = seen_tx.send(Seen::Wrote);
        });
        let first = seen_rx
            .recv_timeout(test_seam::HANG_GUARD)
            .expect("writer B reports");
        control.resume();
        writer_a.join().unwrap();
        writer_b.join().unwrap();
        first
    }

    const MID_RECORD: Point = Point {
        newlines: 0,
        bytes: 10,
    };

    #[test]
    fn legacy_writeln_lets_another_record_into_an_unfinished_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let (a_path, b_path) = (path.clone(), path.clone());
        let seen = interleave(
            MID_RECORD,
            move |plan| {
                legacy::append(
                    PauseAt::new(legacy::open(&a_path), Some(plan)),
                    &fixture("a"),
                )
            },
            move || legacy::append(legacy::open(&b_path), &fixture("b")),
        );
        assert_eq!(seen, Seen::Wrote);
        let check = check_exact(&read(&path), &[fixture("a"), fixture("b")]);
        assert!(
            check.is_err(),
            "the old writer kept the lines whole: {check:?}"
        );
    }

    #[test]
    fn a_record_in_progress_holds_back_another_writer_until_its_line_ends() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let (a_path, b_path) = (path.clone(), path.clone());
        let seen = interleave(
            MID_RECORD,
            move |plan| {
                let _pause = PauseScope::enter(plan);
                emit(Some(&a_path), fixture("a"));
            },
            move || emit(Some(&b_path), fixture("b")),
        );
        assert_eq!(seen, Seen::Contended);
        check_exact(&read(&path), &[fixture("a"), fixture("b")]).unwrap();
    }

    #[test]
    fn other_spellings_of_the_same_file_share_the_lock() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        let path = dir.path().join("events.jsonl");
        let mut spellings = vec![dir.path().join("sub/../events.jsonl")];
        #[cfg(unix)]
        {
            let link = dir.path().join("link.jsonl");
            std::os::unix::fs::symlink(&path, &link).unwrap();
            spellings.push(link);
        }
        let mut expected = Vec::new();
        for (index, other) in spellings.into_iter().enumerate() {
            let (a, b) = (format!("a{index}"), format!("b{index}"));
            let (a_path, a_id, b_id) = (path.clone(), a.clone(), b.clone());
            let seen = interleave(
                MID_RECORD,
                move |plan| {
                    let _pause = PauseScope::enter(plan);
                    append_event_failsafe(Some(&a_path), fixture(&a_id)).unwrap();
                },
                move || emit(Some(&other), fixture(&b_id)),
            );
            assert_eq!(seen, Seen::Contended, "spelling {index}");
            expected.extend([fixture(&a), fixture(&b)]);
            check_exact(&read(&path), &expected).unwrap();
        }
    }

    #[test]
    fn legacy_flush_lets_a_line_between_the_turn_event_and_its_tail() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let (a_path, b_path) = (path.clone(), path.clone());
        let turn = turn_events("turn", 3);
        let seen = interleave(
            Point {
                newlines: 0,
                bytes: 0,
            },
            move |plan| legacy::flush(&a_path, &turn, plan),
            move || legacy::append(legacy::open(&b_path), &fixture("b")),
        );
        assert_eq!(seen, Seen::Wrote);
        let mut expected = turn_events("turn", 3);
        expected.push(fixture("b"));
        let events = check_exact(&read(&path), &expected).unwrap();
        let check = check_turn(&events, "turn", 3);
        assert!(
            check.is_err(),
            "the old flush kept the tail next to its event"
        );
    }

    #[test]
    fn a_flushing_turn_takes_the_lock_once_for_its_event_and_tail() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let (a_path, b_path) = (path.clone(), path.clone());
        let (locks_tx, locks_rx) = channel();
        let seen = interleave_with(
            test_seam::plan_before_tail(),
            move |plan| {
                let mut scope = spool_turn(&a_path, "turn", 3);
                let before = test_seam::lock_calls();
                let _pause = PauseScope::enter(plan);
                scope.finish().unwrap();
                locks_tx.send(test_seam::lock_calls() - before).unwrap();
            },
            move || emit(Some(&b_path), fixture("b")),
        );
        assert_eq!(seen, Seen::Contended);
        assert_eq!(locks_rx.recv().unwrap(), 1, "one lock per flush");
        let mut expected = turn_events("turn", 3);
        expected.push(fixture("b"));
        let events = check_exact(&read(&path), &expected).unwrap();
        check_turn(&events, "turn", 3).unwrap();
    }

    /// Past the 64 KiB spool threshold, stopped seven bytes into its sixth
    /// tail line.
    const TAIL: usize = 80;

    #[test]
    fn legacy_tail_copy_lets_a_record_into_a_tail_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let (a_path, b_path) = (path.clone(), path.clone());
        let turn = turn_events("turn", TAIL);
        let seen = interleave(
            Point {
                newlines: 5,
                bytes: 7,
            },
            move |plan| legacy::flush(&a_path, &turn, plan),
            move || legacy::append(legacy::open(&b_path), &fixture("b")),
        );
        assert_eq!(seen, Seen::Wrote);
        let mut expected = turn_events("turn", TAIL);
        expected.push(fixture("b"));
        let check = check_exact(&read(&path), &expected);
        assert!(check.is_err(), "the old tail copy kept the lines whole");
    }

    #[test]
    fn a_large_tail_is_written_whole_under_the_lock() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let (a_path, b_path) = (path.clone(), path.clone());
        let seen = interleave(
            Point {
                newlines: 6,
                bytes: 7,
            },
            move |plan| {
                let mut scope = spool_turn(&a_path, "turn", TAIL);
                let _pause = PauseScope::enter(plan);
                scope.finish().unwrap();
            },
            move || emit(Some(&b_path), fixture("b")),
        );
        assert_eq!(seen, Seen::Contended);
        let mut expected = turn_events("turn", TAIL);
        expected.push(fixture("b"));
        let events = check_exact(&read(&path), &expected).unwrap();
        check_turn(&events, "turn", TAIL).unwrap();
    }

    /// Normal emits, failsafe appends, and provider-turn flushes from threads
    /// that start together. Supporting evidence only: the lock itself is
    /// measured by the stopped-writer tests above. A failing round fails the
    /// test at once; no round is retried.
    #[test]
    fn concurrent_writers_of_every_kind_keep_every_line_whole() {
        for round in 0..6 {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("events.jsonl");
            let start = Arc::new(Barrier::new(3));
            let small = |index: usize| {
                json!({"event":"fixture", "id":format!("emit-{index}"), "seq":index,
                    "text":format!("行{index}\\n\"q\"\u{0301}\n\t")})
            };
            let large = |index: usize| {
                json!({"event":"fixture", "id":format!("failsafe-{index}"), "seq":index,
                    "payload":format!("{index}{}", "大\n".repeat(40_000))})
            };
            let mut threads = Vec::new();
            for kind in 0..3 {
                let (path, start) = (path.clone(), start.clone());
                threads.push(std::thread::spawn(move || {
                    start.wait();
                    match kind {
                        0 => (0..40).for_each(|index| emit(Some(&path), small(index))),
                        1 => (0..4).for_each(|index| {
                            append_event_failsafe(Some(&path), large(index)).unwrap()
                        }),
                        _ => (0..3).for_each(|index| {
                            spool_turn(&path, &format!("turn-{index}"), TAIL)
                                .finish()
                                .unwrap()
                        }),
                    }
                }));
            }
            threads
                .into_iter()
                .for_each(|thread| thread.join().unwrap());
            let mut expected = (0..40).map(small).collect::<Vec<_>>();
            expected.extend((0..4).map(large));
            for index in 0..3 {
                expected.extend(turn_events(&format!("turn-{index}"), TAIL));
            }
            let events = check_exact(&read(&path), &expected)
                .unwrap_or_else(|err| panic!("round {round}: {err}"));
            for index in 0..3 {
                check_turn(&events, &format!("turn-{index}"), TAIL).unwrap();
            }
            for producer in ["emit-", "failsafe-"] {
                let seqs = events
                    .iter()
                    .filter(|event| event["id"].as_str().unwrap().starts_with(producer))
                    .map(|event| event["seq"].as_u64().unwrap())
                    .collect::<Vec<_>>();
                assert!(seqs.is_sorted(), "round {round}: {producer} order {seqs:?}");
            }
            assert!(events.iter().all(|event| event["schema_version"] == "1"));
        }
    }

    /// A nested turn flushes into its parent and the parent into the file. Run
    /// in its own process, so a deadlock that outlived the guard could not keep
    /// the lock from the other tests.
    #[test]
    fn nested_turn_flushes_finish_in_their_own_process() {
        crate::cli_pack::tests::own_process::run(
            "eval_events::append_writer::tests::nested_turn_flushes_finish_in_their_own_process",
            || {
                let dir = tempfile::tempdir().unwrap();
                let path = dir.path().join("events.jsonl");
                let (done_tx, done_rx) = channel();
                let thread_path = path.clone();
                std::thread::spawn(move || {
                    let mut outer = spool_turn(&thread_path, "outer", TAIL);
                    spool_turn(&thread_path, "inner", TAIL).finish().unwrap();
                    outer.finish().unwrap();
                    drop(outer);
                    let _ = done_tx.send(());
                });
                done_rx
                    .recv_timeout(test_seam::HANG_GUARD)
                    .expect("nested flushes finish without a deadlock");
                let mut expected = turn_events("outer", TAIL);
                expected.extend(turn_events("inner", TAIL));
                let events = check_exact(&read(&path), &expected).unwrap();
                check_turn(&events, "outer", TAIL).unwrap();
                check_turn(&events, "inner", TAIL).unwrap();
            },
        );
    }

    #[test]
    fn existing_readers_take_every_line_and_keep_refusing_broken_ones() {
        let root = tempfile::tempdir().unwrap();
        let path = root
            .path()
            .join(".commandagent/runs/atomicity-run/events.jsonl");
        let writers = (0..3)
            .map(|kind| {
                let path = path.clone();
                std::thread::spawn(move || match kind {
                    0 => (0..20).for_each(|index| emit(Some(&path), fixture(&format!("e{index}")))),
                    1 => (0..20).for_each(|index| {
                        append_event_failsafe(Some(&path), fixture(&format!("f{index}"))).unwrap()
                    }),
                    _ => spool_turn(&path, "turn", TAIL).finish().unwrap(),
                })
            })
            .collect::<Vec<_>>();
        writers
            .into_iter()
            .for_each(|writer| writer.join().unwrap());
        emit(
            Some(&path),
            json!({"event":"run_stop", "id":"stop", "profile":"generic",
                "final_acceptance_status":"passed"}),
        );
        let total = 20 + 20 + 1 + TAIL + 1;
        assert_eq!(strict_lines(&read(&path)).unwrap().len(), total);

        let request = crate::config::RunsRequest {
            id: Some("atomicity-run".to_string()),
            events: true,
            json: true,
            ..crate::config::RunsRequest::default()
        };
        let rendered = crate::runs::render_runs_request(root.path(), &request).unwrap();
        let rendered: Value = serde_json::from_str(&rendered).unwrap();
        assert_eq!(rendered["total"], total);
        assert_eq!(
            crate::eval_events::terminal_report::read_events(Some(&path)).len(),
            total
        );
        let snapshot = crate::eval_events::latest_completion_snapshot(Some(&path));
        assert_eq!(snapshot.profile, "generic");
        assert_eq!(snapshot.final_acceptance_status, "passed");

        // The torn line the old writer could leave: the strict reader still
        // refuses it, and the lenient one still drops only that line.
        legacy::open(&path)
            .write_all(b"{\"id\":\"torn\"{\"id\":\"b\"}\n")
            .unwrap();
        let error = crate::runs::render_runs_request(root.path(), &request).unwrap_err();
        assert!(
            error.to_string().contains("failed to parse run event"),
            "{error}"
        );
        assert_eq!(
            crate::eval_events::terminal_report::read_events(Some(&path)).len(),
            total
        );
    }

    #[test]
    fn a_synthetic_secret_spooled_in_a_turn_never_reaches_the_events_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let secret = "SYNTHETIC_ISSUE987_SECRET_value_42";
        let mut catalog = crate::sensitive_data::SecretCatalog::new();
        catalog.register(secret);
        crate::sensitive_data::install_scope(catalog, None, Some(&path));
        let mut scope = ExecutionTelemetry::new(Some(&path));
        emit(
            Some(&path),
            json!({"event":"provider_turn_duration", "id":"turn", "note":secret}),
        );
        append_event_failsafe(
            Some(&path),
            json!({"event":"fixture", "id":"tail", "note":secret}),
        )
        .unwrap();
        scope.finish().unwrap();
        drop(scope);
        emit(
            Some(&path),
            json!({"event":"fixture", "id":"plain", "note":secret}),
        );
        crate::sensitive_data::set_current(None);
        let bytes = read(&path);
        assert!(!String::from_utf8_lossy(&bytes).contains(secret));
        assert_eq!(
            ids(&strict_lines(&bytes).unwrap()),
            ["turn", "tail", "plain"]
        );
    }

    /// The corpus cases: the strict line check gives each its recorded
    /// verdict, and the writer reproduces every valid case byte for byte.
    #[test]
    fn corpus_line_cases_get_their_recorded_verdicts() {
        let manifest: Value = serde_json::from_str(CORPUS_MANIFEST).unwrap();
        assert_eq!(
            manifest["fixture_sha256"]["line-cases.json"],
            format!("{:x}", sha2::Sha256::digest(CORPUS_CASES.as_bytes()))
        );
        let corpus: Value = serde_json::from_str(CORPUS_CASES).unwrap();
        assert_eq!(corpus["kind"], "synthetic");
        let dir = tempfile::tempdir().unwrap();
        let mut names = BTreeSet::new();
        for case in corpus["cases"].as_array().unwrap() {
            let name = case["id"].as_str().unwrap();
            assert!(names.insert(name), "{name} twice");
            let bytes = case["bytes"].as_str().unwrap().as_bytes();
            let verdict = strict_lines(bytes);
            match case["verdict"].as_str().unwrap() {
                "valid" => {
                    let events = verdict.unwrap_or_else(|err| panic!("{name}: {err}"));
                    assert_eq!(ids(&events), json_strings(&case["event_ids"]), "{name}");
                    let path: PathBuf = dir.path().join(format!("{name}.jsonl"));
                    for event in &events {
                        append_event(&path, event).unwrap();
                    }
                    assert_eq!(read(&path), bytes, "{name}: rewritten bytes");
                }
                "invalid" => assert!(verdict.is_err(), "{name} passed the strict check"),
                other => panic!("{name}: unknown verdict {other}"),
            }
        }
    }

    fn json_strings(value: &Value) -> Vec<String> {
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item.as_str().unwrap().to_string())
            .collect()
    }
}
