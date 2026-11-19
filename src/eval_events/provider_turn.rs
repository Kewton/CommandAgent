//! Finalize per-response execution facts without reordering persisted events.
//!
//! UI projections remain immediate. Only this caller thread's event-file writes,
//! from the provider result through its tool batch, wait for the outcome.
use std::cell::RefCell;
use std::io::{Seek, SeekFrom};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyhow::Context;
use serde_json::{Value, json};

use super::append_writer::{Progress, copy_exact, record, spool_out, write_counted};
use crate::providers::AssistantReply;

thread_local! {
    static PENDING: RefCell<Vec<PendingTurn>> = const { RefCell::new(Vec::new()) };
}

struct PendingTurn {
    path: Option<PathBuf>,
    event: Option<Value>,
    tail: tempfile::SpooledTempFile,
    /// The end of the last whole line in `tail`; bytes past it are a failed
    /// write's fragment and are never read.
    committed: u64,
}

pub(crate) struct ExecutionTelemetry {
    // A scope must finish on the same thread and in stack order.
    _thread: PhantomData<Rc<()>>,
}

impl ExecutionTelemetry {
    pub(crate) fn new(path: Option<&Path>) -> Self {
        PENDING.with_borrow_mut(|pending| {
            pending.push(PendingTurn {
                path: path.map(Path::to_path_buf),
                event: None,
                tail: tempfile::spooled_tempfile(64 * 1024),
                committed: 0,
            });
        });
        Self {
            _thread: PhantomData,
        }
    }

    pub(crate) fn response(&mut self, reply: &AssistantReply, num_predict: usize) {
        self.update(|event| {
            event["tool_calls_in_response"] = json!(reply.tool_calls.len());
            event["num_predict"] = json!(num_predict);
            event["output_limit_reached"] = json!(
                num_predict > 0
                    && reply
                        .completion_tokens
                        .is_some_and(|count| count >= num_predict as u64)
            );
        });
    }

    pub(crate) fn write_or_edit_succeeded(&mut self) {
        self.update(|event| event["write_or_edit_succeeded"] = json!(true));
    }

    pub(crate) fn finish(&mut self) -> anyhow::Result<()> {
        let turn = PENDING.with_borrow_mut(|pending| {
            pending.last_mut().map(|turn| {
                std::mem::replace(
                    turn,
                    PendingTurn {
                        path: None,
                        event: None,
                        tail: tempfile::spooled_tempfile(64 * 1024),
                        committed: 0,
                    },
                )
            })
        });
        if let Some(turn) = turn {
            turn.flush()?;
        }
        Ok(())
    }

    fn update(&mut self, update: impl FnOnce(&mut Value)) {
        PENDING.with_borrow_mut(|pending| {
            if let Some(event) = pending.last_mut().and_then(|turn| turn.event.as_mut()) {
                update(event);
            }
        });
    }
}

impl Drop for ExecutionTelemetry {
    fn drop(&mut self) {
        let turn = PENDING.with_borrow_mut(|pending| pending.pop());
        if let Some(turn) = turn
            && let Err(err) = turn.flush()
        {
            eprintln!("warning: failed to write COMMANDAGENT_EVAL_EVENTS: {err}");
        }
    }
}

impl PendingTurn {
    fn flush(mut self) -> anyhow::Result<()> {
        let (Some(path), Some(event)) = (self.path, self.event) else {
            return Ok(());
        };
        self.tail.rewind()?;
        let event = protect(&path, &event);
        let (tail, len) = (&mut self.tail, self.committed);
        let merged = PENDING.with_borrow_mut(|pending| -> anyhow::Result<bool> {
            let Some(parent) = pending
                .iter_mut()
                .rev()
                .find(|turn| turn.path.as_deref() == Some(path.as_path()))
            else {
                return Ok(false);
            };
            // The nested turn's event and tail reach the parent together or
            // not at all (Issue #987).
            let head = match parent.event {
                Some(_) => record(&event)?,
                None => Vec::new(),
            };
            parent.tail.seek(SeekFrom::Start(parent.committed))?;
            let mut out = spool_out(&mut parent.tail, true);
            let mut progress = Progress::default();
            write_counted(&mut out, &head, &mut progress)
                .and_then(|()| copy_exact(tail, len, &mut out, &mut progress))
                .with_context(|| {
                    format!("nested provider turn lost: its event and {len} spooled bytes were not merged")
                })?;
            parent.committed += (head.len() as u64) + len;
            parent.event.get_or_insert_with(|| event.clone());
            Ok(true)
        })?;
        if !merged {
            // One lock for the event line and its tail (Issue #987).
            super::append_writer::append_event_with_tail(&path, &event, (tail, len))?;
        }
        Ok(())
    }
}

pub(super) fn buffer(path: &Path, event: &Value) -> anyhow::Result<bool> {
    // The caller (emit/append_event_failsafe) already scrubbed this payload;
    // re-applying the idempotent catalog scrub here guarantees the spool file
    // only ever receives protected bytes, including provider-thread events.
    let event = &protect(path, event);
    PENDING.with_borrow_mut(|pending| {
        let Some(turn) = pending
            .iter_mut()
            .rev()
            .find(|turn| turn.path.as_deref() == Some(path))
        else {
            return Ok(false);
        };
        if turn.event.is_none() {
            if event["event"] != "provider_turn_duration" {
                return Ok(false);
            }
            turn.event = Some(event.clone());
        } else {
            // From the last whole line: a failed write's fragment is overwritten.
            let line = record(event)?;
            turn.tail.seek(SeekFrom::Start(turn.committed))?;
            write_counted(
                &mut spool_out(&mut turn.tail, false),
                &line,
                &mut Progress::default(),
            )
            .context("failed to spool a provider-turn event")?;
            turn.committed += line.len() as u64;
        }
        Ok(true)
    })
}

fn protect(path: &Path, event: &Value) -> Value {
    let mut protected = event.clone();
    if let Some(context) = crate::sensitive_data::active_for(Some(path)) {
        context.scrub_value_lenient(&mut protected);
    }
    protected
}

#[cfg(test)]
mod issue440_tests {
    use super::*;
    use crate::eval_events::{append_event_failsafe, emit};

    fn duration(scope: &str) -> Value {
        json!({"event":"provider_turn_duration", "caller_scope":scope,
            "tools":9, "tool_calls_in_response":0, "write_or_edit_succeeded":false,
            "eval_count":8192,"duration_ms":120000})
    }

    fn events(path: &Path) -> Vec<Value> {
        std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    #[test]
    fn both_writers_keep_order_and_nested_provider_does_not_inherit_outer_write() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let mut scope = ExecutionTelemetry::new(Some(&path));
        emit(Some(&path), duration("executor"));
        let reply = AssistantReply {
            content: String::new(),
            tool_calls: vec![crate::state::ToolCall::new(
                "Write",
                json!({"path":"page.tsx", "content":"ok"}),
            )],
            prompt_tokens: None,
            completion_tokens: Some(8192),
        };
        scope.response(&reply, 8192);
        emit(
            Some(&path),
            json!({"event":"tool_call_raw", "name":"Write"}),
        );
        append_event_failsafe(Some(&path), duration("classifier")).unwrap();
        scope.write_or_edit_succeeded();
        append_event_failsafe(Some(&path), json!({"event":"tool_execute", "status":"ok"})).unwrap();
        scope.finish().unwrap();
        drop(scope);
        let events = events(&path);
        assert_eq!(
            events
                .iter()
                .map(|event| event["event"].as_str().unwrap())
                .collect::<Vec<_>>(),
            [
                "provider_turn_duration",
                "tool_call_raw",
                "provider_turn_duration",
                "tool_execute"
            ]
        );
        assert_eq!(events[0]["write_or_edit_succeeded"], true);
        assert_eq!(events[0]["tool_calls_in_response"], 1);
        assert_eq!(events[0]["tools"], 9);
        assert_eq!(events[2]["write_or_edit_succeeded"], false);
        assert_eq!(events[2]["tool_calls_in_response"], 0);
        assert!(events.iter().all(|event| event["schema_version"] == "1"));
    }

    #[test]
    fn scope_flushes_once_on_continue_error_and_interruption_unwind() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        for _ in 0..2 {
            let _scope = ExecutionTelemetry::new(Some(&path));
            emit(Some(&path), duration("repair"));
            continue;
        }
        let early_error = || -> anyhow::Result<()> {
            let _scope = ExecutionTelemetry::new(Some(&path));
            emit(Some(&path), duration("repair"));
            anyhow::bail!("interrupted by user")
        };
        assert!(early_error().is_err());
        assert!(
            std::panic::catch_unwind(|| {
                let _scope = ExecutionTelemetry::new(Some(&path));
                emit(Some(&path), duration("repair"));
                panic!("fixture panic");
            })
            .is_err()
        );
        append_event_failsafe(Some(&path), json!({"event":"run_stop"})).unwrap();
        assert_eq!(events(&path).len(), 5);
        assert!(PENDING.with_borrow(|pending| pending.is_empty()));
    }

    #[test]
    fn sessions_paths_nested_scopes_and_threads_are_isolated() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("outer.jsonl");
        let other = dir.path().join("other.jsonl");
        let mut outer = ExecutionTelemetry::new(Some(&path));
        emit(Some(&path), duration("executor"));
        {
            let _inner = ExecutionTelemetry::new(Some(&path));
            emit(Some(&path), duration("repair"));
        }
        let thread_path = other.clone();
        std::thread::spawn(move || {
            let _scope = ExecutionTelemetry::new(Some(&thread_path));
            emit(Some(&thread_path), duration("executor"));
        })
        .join()
        .unwrap();
        emit(Some(&other), json!({"event":"other_path"}));
        outer.write_or_edit_succeeded();
        drop(outer);
        {
            let _scope = ExecutionTelemetry::new(Some(&path));
            emit(Some(&path), duration("executor"));
        }
        let events = events(&path);
        assert_eq!(events.len(), 3);
        assert_eq!(events[0]["write_or_edit_succeeded"], true);
        assert_eq!(events[1]["write_or_edit_succeeded"], false);
        assert_eq!(events[2]["write_or_edit_succeeded"], false);
        assert_eq!(std::fs::read_to_string(other).unwrap().lines().count(), 2);
    }

    #[test]
    fn large_batch_spills_after_64_kib_and_flushes_without_loss() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let mut scope = ExecutionTelemetry::new(Some(&path));
        emit(Some(&path), duration("executor"));
        let payload = "x".repeat(2048);
        for index in 0..100 {
            append_event_failsafe(
                Some(&path),
                json!({"event":"fixture", "index":index,"payload":payload}),
            )
            .unwrap();
        }
        assert!(PENDING.with_borrow(|pending| pending.last().unwrap().tail.is_rolled()));
        scope.finish().unwrap();
        drop(scope);
        let events = events(&path);
        assert_eq!(events.len(), 101);
        assert_eq!(events[100]["index"], 99);
    }

    fn ids(path: &Path) -> Vec<String> {
        let bytes = std::fs::read(path).unwrap();
        let body = bytes
            .strip_suffix(b"\n")
            .expect("the last line ends with LF");
        body.split(|byte| *byte == b'\n')
            .map(|line| serde_json::from_slice::<Value>(line).unwrap()["id"].to_string())
            .map(|id| id.trim_matches('"').to_string())
            .collect()
    }

    fn fixture(id: &str, payload: usize) -> Value {
        json!({"event":"fixture", "id":id, "payload":"y".repeat(payload)})
    }

    /// Issue #987: a spool write that fails part way leaves a fragment longer
    /// than the next line; neither that fragment nor any of it is flushed.
    #[test]
    fn a_failed_spool_write_leaves_no_fragment_in_the_flushed_tail() {
        use super::super::append_writer::test_seam::{Fault, FaultScope, Site};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let mut scope = ExecutionTelemetry::new(Some(&path));
        let mut duration = duration("executor");
        duration["id"] = json!("turn");
        emit(Some(&path), duration);
        for index in 0..40 {
            append_event_failsafe(Some(&path), fixture(&format!("t{index}"), 2_048)).unwrap();
        }
        assert!(PENDING.with_borrow(|pending| pending.last().unwrap().tail.is_rolled()));
        let faults = [
            (Site::SpoolAppend, Fault::Short(3_000)),
            (Site::SpoolAppend, Fault::Fail),
        ];
        let fault = FaultScope::enter(&faults);
        assert!(append_event_failsafe(Some(&path), fixture("lost", 5_000)).is_err());
        drop(fault);
        append_event_failsafe(Some(&path), fixture("short", 10)).unwrap();
        scope.finish().unwrap();
        let mut want = vec!["turn".to_string()];
        want.extend((0..40).map(|index| format!("t{index}")));
        want.push("short".to_string());
        assert_eq!(ids(&path), want);
    }

    /// Issue #987: a nested turn whose merge fails (a write part way, or its
    /// tail ending before its length) is lost whole, by finish or by drop,
    /// and the parent keeps its own lines and its event slot.
    #[test]
    fn a_failed_merge_loses_the_whole_nested_turn_and_keeps_the_parent() {
        use super::super::append_writer::test_seam::{Fault, FaultScope, Site};
        let write = [
            (Site::SpoolMerge, Fault::Short(50)),
            (Site::SpoolMerge, Fault::Fail),
        ];
        let early_eof = [
            (Site::TailRead, Fault::Short(50)),
            (Site::TailRead, Fault::Zero),
        ];
        let cases = [(write, true), (early_eof, true), (write, false)];
        for (index, (faults, finish)) in cases.into_iter().enumerate() {
            for parent_event in [true, false] {
                let dir = tempfile::tempdir().unwrap();
                let path = dir.path().join("events.jsonl");
                let mut outer = ExecutionTelemetry::new(Some(&path));
                if parent_event {
                    let mut event = duration("executor");
                    event["id"] = json!("outer");
                    emit(Some(&path), event);
                    append_event_failsafe(Some(&path), fixture("outer-0", 10)).unwrap();
                }
                {
                    // Entered first, so it outlives the scope's drop.
                    let _faults = FaultScope::enter(&faults);
                    let mut inner = ExecutionTelemetry::new(Some(&path));
                    let mut event = duration("repair");
                    event["id"] = json!("inner");
                    emit(Some(&path), event);
                    for tail in 0..3 {
                        let id = format!("inner-{tail}");
                        append_event_failsafe(Some(&path), fixture(&id, 100)).unwrap();
                    }
                    if finish {
                        let err = inner.finish().unwrap_err();
                        let text = format!("{err:#}");
                        assert!(text.contains("nested provider turn lost"), "{text}");
                        let eof = faults[1].0 == Site::TailRead;
                        assert_eq!(text.contains("ended"), eof, "{text}");
                    }
                }
                append_event_failsafe(Some(&path), fixture("after", 10)).unwrap();
                outer.finish().unwrap();
                let want: &[&str] = if parent_event {
                    &["outer", "outer-0", "after"]
                } else {
                    &["after"]
                };
                assert_eq!(ids(&path), want, "case {index}, parent {parent_event}");
            }
        }
    }
}
