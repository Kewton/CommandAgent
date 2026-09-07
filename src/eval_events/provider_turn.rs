//! Finalize per-response execution facts without reordering persisted events.
//!
//! UI projections remain immediate. Only this caller thread's event-file writes,
//! from the provider result through its tool batch, wait for the outcome.
use std::cell::RefCell;
use std::io::{Seek, Write};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use serde_json::{Value, json};

use crate::providers::AssistantReply;

thread_local! {
    static PENDING: RefCell<Vec<PendingTurn>> = const { RefCell::new(Vec::new()) };
}

struct PendingTurn {
    path: Option<PathBuf>,
    event: Option<Value>,
    tail: tempfile::SpooledTempFile,
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
        if buffer(&path, &event)? {
            PENDING.with_borrow_mut(|pending| -> anyhow::Result<()> {
                let parent = pending
                    .iter_mut()
                    .rev()
                    .find(|turn| turn.path.as_deref() == Some(path.as_path()))
                    .expect("buffer found enclosing scope");
                std::io::copy(&mut self.tail, &mut parent.tail)?;
                Ok(())
            })?;
        } else {
            super::append(&path, &event)?;
            let mut file = std::fs::OpenOptions::new().append(true).open(path)?;
            std::io::copy(&mut self.tail, &mut file)?;
        }
        Ok(())
    }
}

pub(super) fn buffer(path: &Path, event: &Value) -> anyhow::Result<bool> {
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
            serde_json::to_writer(&mut turn.tail, event)?;
            writeln!(turn.tail)?;
        }
        Ok(true)
    })
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
}
