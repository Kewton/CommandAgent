//! Whole-record appends to event files (Issue #987).
//!
//! Every writer of an events file in this process builds its record (the JSON
//! text and its `\n`) before it gets here, then writes it under one
//! process-wide lock. While a record, or a provider turn's event line and its
//! spooled tail, is being written, no other writer in this process can put
//! bytes into the file, whatever spelling of the path it opened. Partial writes
//! and `Interrupted` are retried to the end of the record; any other error is
//! returned to the caller, which reports it. Writers in other processes, and
//! paths that are not regular files (a FIFO, a device, written with the calls
//! of fc903248 and no lock), are outside this guarantee (see
//! `docs/dev/maintainability-architecture.md`).
//!
//! Nothing called under the lock emits, redacts, publishes, or touches the
//! provider-turn spool registry, so a flush can never re-enter the lock.
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde_json::Value;

/// One lock for every events file: a per-path lock would let two spellings of
/// one file (relative, `..`, a symlink) write at the same time.
static WRITE_LOCK: Mutex<()> = Mutex::new(());

/// Append `event` as one line, creating the parent directory first.
pub(crate) fn append_event(path: &Path, event: &Value) -> anyhow::Result<()> {
    append_event_and(path, event, None)
}

/// Append `event` as one line and then `tail` (whole lines) under one lock, so
/// no other line of this process lands between them or inside them.
pub(crate) fn append_event_with_tail(
    path: &Path,
    event: &Value,
    tail: &mut dyn Read,
) -> anyhow::Result<()> {
    append_event_and(path, event, Some(tail))
}

fn append_event_and(path: &Path, event: &Value, tail: Option<&mut dyn Read>) -> anyhow::Result<()> {
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
    append_locked(path, line, None)
}

fn append_locked(path: &Path, record: &[u8], tail: Option<&mut dyn Read>) -> io::Result<()> {
    // Opened before the lock: opening a FIFO with no reader waits, and must
    // not hold up writers of other files. A handle that cannot be classified
    // counts as a regular file.
    let file = open_append(path)?;
    if !takes_lock(&file) {
        return append_as_before(file, path, record, tail);
    }
    let _guard = lock();
    #[cfg(test)]
    let file = test_seam::PauseAt::for_this_thread(file);
    write_record(file, record, tail.unwrap_or(&mut io::empty()))
}

/// A FIFO or device, written with fc903248's calls and no lock: the body and
/// its LF as `writeln!` wrote them, then a provider turn's tail through a
/// second append open, which reaches a FIFO reader that came after the first.
fn append_as_before(
    mut file: File,
    path: &Path,
    record: &[u8],
    tail: Option<&mut dyn Read>,
) -> io::Result<()> {
    match record.strip_suffix(b"\n") {
        Some(body) => {
            file.write_all(body)?;
            file.write_all(b"\n")?;
        }
        None => file.write_all(record)?,
    }
    let Some(tail) = tail else {
        return Ok(());
    };
    drop(file);
    #[cfg(test)]
    test_seam::before_tail();
    #[cfg(test)]
    test_seam::before_open();
    let mut file = OpenOptions::new().append(true).open(path)?;
    // The path may name a regular file by now; its tail then takes the lock.
    let _guard = takes_lock(&file).then(lock);
    io::copy(tail, &mut file)?;
    Ok(())
}

/// Whether writes through `file` take the lock: a regular file, or a handle
/// that cannot be classified.
fn takes_lock(file: &File) -> bool {
    file.metadata().map_or(true, |meta| meta.is_file())
}

/// The open itself. Tests are told here, at the open and not before it, so
/// whatever the caller holds when it opens is already held when they hear.
fn open_append(path: &Path) -> io::Result<File> {
    #[cfg(test)]
    test_seam::before_open();
    OpenOptions::new().create(true).append(true).open(path)
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
        static OPENING: RefCell<Option<Sender<usize>>> = const { RefCell::new(None) };
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

    /// While entered, this thread reports each open it is about to make, with
    /// how many times it has taken the write lock by then.
    pub(crate) struct OpenProbe;

    impl OpenProbe {
        pub(crate) fn enter(sender: Sender<usize>) -> Self {
            OPENING.with_borrow_mut(|slot| *slot = Some(sender));
            Self
        }
    }

    impl Drop for OpenProbe {
        fn drop(&mut self) {
            OPENING.with_borrow_mut(|slot| *slot = None);
        }
    }

    pub(super) fn before_open() {
        OPENING.with_borrow(|probe| {
            if let Some(sender) = probe {
                let _ = sender.send(LOCK_CALLS.get());
            }
        });
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
            use std::io::Seek;
            append(open(path), &events[0]);
            // The spool and the copy as they were: a non-buffered reader, so
            // `io::copy` writes in 8 KiB chunks.
            let mut tail = tempfile::spooled_tempfile(64 * 1024);
            tail.write_all(&tail_bytes(&events[1..])).unwrap();
            tail.rewind().unwrap();
            let file = OpenOptions::new().append(true).open(path).unwrap();
            io::copy(&mut tail, &mut PauseAt::new(file, Some(plan))).unwrap();
        }

        pub(super) fn tail_bytes(events: &[Value]) -> Vec<u8> {
            let mut tail = Vec::new();
            for event in events {
                serde_json::to_writer(&mut tail, event).unwrap();
                writeln!(tail).unwrap();
            }
            tail
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
        // Where `writeln!` itself splits: after the body's write, before the
        // LF's own write. No write is shortened.
        let body = serde_json::to_string(&fixture("a")).unwrap().len();
        let seen = interleave(
            Point {
                newlines: 0,
                bytes: body,
            },
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
        // Where `io::copy` itself splits: at its 8 KiB chunk end, which falls
        // inside a line. No write is shortened.
        let tail = legacy::tail_bytes(&turn[1..]);
        let chunk_end = &tail[..8 * 1024];
        let line_start = chunk_end.iter().rposition(|byte| *byte == b'\n').unwrap() + 1;
        assert!(line_start < chunk_end.len(), "the chunk ends inside a line");
        let point = Point {
            newlines: chunk_end.iter().filter(|byte| **byte == b'\n').count(),
            bytes: chunk_end.len() - line_start,
        };
        let seen = interleave(
            point,
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

    /// Runs `work` on its own thread; its result comes back on the receiver.
    #[cfg(unix)]
    fn spawn_with_result<T: Send + 'static>(
        work: impl FnOnce() -> T + Send + 'static,
    ) -> (std::thread::JoinHandle<()>, std::sync::mpsc::Receiver<T>) {
        let (tx, rx) = channel();
        let handle = std::thread::spawn(move || {
            let _ = tx.send(work());
        });
        (handle, rx)
    }

    /// The result of a thread from `spawn_with_result`, which is then joined.
    /// A thread still running after `limit` fails the test by name instead of
    /// being left behind unnoticed.
    #[cfg(unix)]
    fn collect<T>(
        (handle, rx): (std::thread::JoinHandle<()>, std::sync::mpsc::Receiver<T>),
        limit: std::time::Duration,
        what: &str,
    ) -> T {
        match rx.recv_timeout(limit) {
            Ok(result) => {
                handle.join().unwrap();
                result
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                handle.join().unwrap();
                panic!("{what} ended without a result")
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                panic!("{what} did not finish within {limit:?}; its thread is still running")
            }
        }
    }

    #[cfg(unix)]
    fn mkfifo(path: &Path) {
        let made = std::process::Command::new("mkfifo")
            .arg(path)
            .status()
            .unwrap();
        assert!(made.success());
    }

    /// Reads a FIFO from a blocking open (which waits for a writer) to its end.
    #[cfg(unix)]
    fn read_fifo(path: PathBuf) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::new();
        File::open(path)
            .and_then(|mut reader| reader.read_to_end(&mut bytes))
            .map(|_| bytes)
            .map_err(|err| err.to_string())
    }

    /// A writer waiting to open a FIFO with no reader holds no lock, so a
    /// writer of an unrelated regular file finishes meanwhile. Every thread is
    /// collected before any result is asserted.
    #[cfg(unix)]
    #[test]
    fn a_fifo_waiting_for_a_reader_does_not_hold_up_other_files() {
        in_own_process_within(
            "eval_events::append_writer::tests::a_fifo_waiting_for_a_reader_does_not_hold_up_other_files",
            fifo_waiting_for_a_reader,
        );
    }

    #[cfg(unix)]
    fn fifo_waiting_for_a_reader() {
        let dir = tempfile::tempdir().unwrap();
        let fifo = dir.path().join("events.fifo");
        let path = dir.path().join("events.jsonl");
        mkfifo(&fifo);
        let (opening_tx, opening_rx) = channel();
        let fifo_path = fifo.clone();
        let fifo_writer = spawn_with_result(move || {
            let _probe = test_seam::OpenProbe::enter(opening_tx);
            append_event(&fifo_path, &fixture("fifo")).map_err(|err| err.to_string())
        });
        // Heard at the open itself: a lock taken before the open is held by
        // now, whatever the order of the two threads.
        let locks_before_open = opening_rx
            .recv_timeout(test_seam::HANG_GUARD)
            .expect("the FIFO writer reaches its open");
        let (file_handle, file_rx) = spawn_with_result(move || {
            append_event(&path, &fixture("file"))
                .map(|()| read(&path))
                .map_err(|err| err.to_string())
        });
        let file_in_time = file_rx.recv_timeout(std::time::Duration::from_secs(30));
        // The reader is the peer the FIFO writer's open waits for; it stays
        // open until that writer is done.
        let reader = spawn_with_result({
            let fifo = fifo.clone();
            move || read_fifo(fifo)
        });
        let fifo_result = collect(fifo_writer, test_seam::HANG_GUARD, "the FIFO writer");
        let fifo_bytes = collect(reader, test_seam::HANG_GUARD, "the FIFO reader");
        let written = match file_in_time {
            Ok(written) => {
                file_handle.join().unwrap();
                written
            }
            Err(_) => {
                let late = collect(
                    (file_handle, file_rx),
                    test_seam::HANG_GUARD,
                    "the file writer",
                );
                panic!(
                    "the regular file's append waited for the FIFO's reader (then: {:?})",
                    late.map(|bytes| bytes.len())
                )
            }
        };
        check_exact(&written.unwrap(), &[fixture("file")]).unwrap();
        assert_eq!(
            locks_before_open, 0,
            "the FIFO writer opens holding no lock"
        );
        fifo_result.unwrap();
        assert_eq!(fifo_bytes.unwrap(), record_of(&fixture("fifo")));
    }

    /// fc903248 gave a provider turn's tail an append open of its own, so a
    /// FIFO reader that comes after the reader of the event line still gets
    /// the tail. Non-regular files keep that second open.
    #[cfg(unix)]
    #[test]
    fn a_fifo_reader_after_the_event_line_still_gets_the_tail() {
        in_own_process_within(
            "eval_events::append_writer::tests::a_fifo_reader_after_the_event_line_still_gets_the_tail",
            fifo_reader_after_the_event_line,
        );
    }

    #[cfg(unix)]
    fn fifo_reader_after_the_event_line() {
        use std::io::BufRead;
        let dir = tempfile::tempdir().unwrap();
        let fifo = dir.path().join("events.fifo");
        mkfifo(&fifo);
        let (plan, mut control) = test_seam::plan_before_tail();
        let (opening_tx, opening_rx) = channel();
        let done_tx = opening_tx.clone();
        let turn_path = fifo.clone();
        let turn = spawn_with_result(move || {
            let mut scope = spool_turn(&turn_path, "turn", 3);
            let _probe = test_seam::OpenProbe::enter(opening_tx);
            let _pause = PauseScope::enter(plan);
            let result = scope.finish().map_err(|err| format!("{err:#}"));
            let _ = done_tx.send(usize::MAX);
            result
        });
        opening_rx
            .recv_timeout(test_seam::HANG_GUARD)
            .expect("the turn opens the FIFO for its event line");
        let mut first = std::io::BufReader::new(File::open(&fifo).unwrap());
        let mut line = Vec::new();
        first.read_until(b'\n', &mut line).unwrap();
        control.wait_paused();
        // Reader 1 leaves after the event line, before the tail is written.
        drop(first);
        control.resume();
        let next = opening_rx
            .recv_timeout(test_seam::HANG_GUARD)
            .expect("the turn opens again or ends");
        // Reader 2 comes only for a second open; a writer that kept its first
        // handle has already ended.
        let second = (next != usize::MAX).then(|| {
            let fifo = fifo.clone();
            spawn_with_result(move || read_fifo(fifo))
        });
        let finished = collect(turn, test_seam::HANG_GUARD, "the provider turn");
        let tail = second.map(|reader| collect(reader, test_seam::HANG_GUARD, "reader 2"));
        assert_eq!(
            strict_lines(&line).map(|events| ids(&events)),
            Ok(vec!["turn".to_string()])
        );
        finished.unwrap();
        let mut expected = Vec::new();
        for index in 0..3 {
            let mut event = tail_event("turn", index);
            event["schema_version"] = json!("1");
            expected.extend(record_of(&event));
        }
        assert_eq!(tail.expect("reader 2 came").unwrap(), expected);
    }

    /// The tail's second open is classified on its own: when the path names a
    /// regular file by then, the tail is copied under the lock.
    #[cfg(unix)]
    #[test]
    fn a_tail_reopened_onto_a_regular_file_takes_the_lock() {
        in_own_process_within(
            "eval_events::append_writer::tests::a_tail_reopened_onto_a_regular_file_takes_the_lock",
            tail_reopened_onto_a_regular_file,
        );
    }

    #[cfg(unix)]
    fn tail_reopened_onto_a_regular_file() {
        use std::io::BufRead;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.fifo");
        mkfifo(&path);
        let (plan, mut control) = test_seam::plan_before_tail();
        let turn_path = path.clone();
        let turn = spawn_with_result(move || {
            let mut scope = spool_turn(&turn_path, "turn", 3);
            let _pause = PauseScope::enter(plan);
            let before = test_seam::lock_calls();
            let result = scope.finish().map_err(|err| format!("{err:#}"));
            (result, test_seam::lock_calls() - before)
        });
        let mut first = std::io::BufReader::new(File::open(&path).unwrap());
        first.read_until(b'\n', &mut Vec::new()).unwrap();
        control.wait_paused();
        drop(first);
        // Between the two opens the path becomes a regular file.
        std::fs::remove_file(&path).unwrap();
        std::fs::write(&path, b"").unwrap();
        control.resume();
        let (finished, locks) = collect(turn, test_seam::HANG_GUARD, "the provider turn");
        finished.unwrap();
        assert_eq!(locks, 1, "the tail copied to a regular file takes the lock");
        let events = strict_lines(&read(&path)).unwrap();
        assert_eq!(ids(&events), ["turn-tail-0", "turn-tail-1", "turn-tail-2"]);
    }

    /// Runs `body` alone in a child test process (`test` is this test's full
    /// name), so a FIFO peer that never comes cannot leave a thread waiting in
    /// this process. A child still running after its limit is killed and the
    /// test fails with that reason.
    #[cfg(unix)]
    fn in_own_process_within(test: &str, body: fn()) {
        const RAN: &str = "COMMANDAGENT_TEST_ISSUE987_CHILD_RAN";
        if let Some(ran) = std::env::var_os(RAN) {
            body();
            std::fs::write(ran, test).unwrap();
            return;
        }
        let limit = std::time::Duration::from_secs(180);
        let scratch = tempfile::tempdir().unwrap();
        let ran = scratch.path().join("ran");
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", test, "--nocapture"])
            .env(RAN, &ran)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        // The child's stdout ends when the child does; its text passes through.
        let mut stdout = child.stdout.take().unwrap();
        let ended = spawn_with_result(move || {
            let mut text = String::new();
            let read = stdout.read_to_string(&mut text);
            print!("{text}");
            read.map(|_| ())
        });
        if ended.1.recv_timeout(limit).is_err() {
            let _ = child.kill();
            let _ = child.wait();
            let _ = collect(ended, test_seam::HANG_GUARD, "the killed child's output");
            panic!("{test} did not finish within {limit:?} in its own process; it was killed")
        }
        ended.0.join().unwrap();
        let status = child.wait().unwrap();
        assert!(
            status.success(),
            "{test} failed in its own process: {status}"
        );
        assert_eq!(
            std::fs::read_to_string(&ran).unwrap(),
            test,
            "the child ran {test}"
        );
    }

    fn record_of(event: &Value) -> Vec<u8> {
        let mut bytes = serde_json::to_vec(event).unwrap();
        bytes.push(b'\n');
        bytes
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
        // This thread's scope only: nothing is left in the shared registry.
        crate::sensitive_data::install_scope(catalog, None, None);
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
