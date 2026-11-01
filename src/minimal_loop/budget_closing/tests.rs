//! Leaf tests for the Issue #957 closing notice and unresolved-failure guard.

use std::borrow::Cow;
use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use serde_json::{Value, json};

use super::{
    BudgetClosing, CLOSING_REMAINING_CALLS, ClosingMode, DEFERRAL_FEEDBACK, ENV_NAME, NOTICE,
    PROMPT_VERSION, TEST_ENV, in_scope, parse_mode,
};
use crate::config::Config;
use crate::minimal_loop::execution_progress::ExecutionProgress;
use crate::minimal_loop::loop_run::{
    RunSessionOptions, RunSessionStepKind, run_session_with_outcome_with_options,
};
use crate::planner::profile_descriptor::{
    DATA_PROFILE_ID, INGEST_PROFILE_ID, NEXTJS_PROFILE_ID, PYTHON_CLI_PROFILE_ID,
};
use crate::providers::{AssistantReply, ChatClient};
use crate::state::{ConversationMessage, SessionSnapshot, ToolCall};
use crate::tools::hidden_path;
use crate::tools::registry::ToolSpec;
use crate::tui::NOOP_UI;

const FIXED_KEYS: [&str; 13] = [
    "event",
    "schema_version",
    "phase",
    "guard_reason",
    "prompt_version",
    "iteration",
    "iteration_limit",
    "remaining_calls",
    "unresolved_bash_failures",
    "unresolved_write_failures",
    "resolved_failures",
    "policy_rejected_bash",
    "final_deferrals",
];

const FIXED_STRINGS: [&str; 10] = [
    "budget_closing",
    "1",
    "notice",
    "final_accepted",
    "final_deferred",
    "exhausted_after_notice",
    "remaining_calls_reached",
    "no_unresolved_failure",
    "unresolved_failure",
    "iteration_budget_exhausted",
];

fn read_events(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect()
}

/// An enabled closing state writing to a temporary events file.
fn closing(enabled: bool) -> (tempfile::TempDir, BudgetClosing) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let closing = BudgetClosing::new(enabled, Some(&root.join("e.jsonl")), &root);
    (dir, closing)
}

fn events(dir: &tempfile::TempDir) -> Vec<Value> {
    read_events(&dir.path().canonicalize().unwrap().join("e.jsonl"))
}

fn phases(dir: &tempfile::TempDir) -> Vec<String> {
    events(dir)
        .iter()
        .map(|event| event["phase"].as_str().unwrap().to_string())
        .collect()
}

/// Composes and records the notice for a run with `limit` iterations.
fn send_notice(closing: &mut BudgetClosing, limit: usize) {
    let feedback = closing
        .request_feedback(CLOSING_REMAINING_CALLS - 1, limit, None)
        .map(Cow::into_owned);
    assert_eq!(feedback.as_deref(), Some(NOTICE));
    closing.note_request(&[ConversationMessage::user(feedback.unwrap())]);
    closing.note_provider_call(false);
}

fn edit(path: &str, old: &str, new: &str) -> Value {
    json!({"path": path, "old_string": old, "new_string": new})
}

fn write(path: &str, content: &str) -> Value {
    json!({"path": path, "content": content})
}

/// An event carries only the fixed keys, numbers, and fixed words.
fn assert_fixed_shape(event: &Value) {
    for (key, value) in event.as_object().unwrap() {
        assert!(FIXED_KEYS.contains(&key.as_str()), "key {key}: {event}");
        match value {
            Value::Number(_) => {}
            Value::String(text) => assert!(
                FIXED_STRINGS.contains(&text.as_str()),
                "free text {text:?} in {event}"
            ),
            other => panic!("unexpected value {other} under {key}: {event}"),
        }
    }
}

#[test]
fn mode_accepts_only_unset_empty_off_and_once() {
    for (raw, expected) in [
        (None, ClosingMode::Off),
        (Some(""), ClosingMode::Off),
        (Some("  "), ClosingMode::Off),
        (Some("off"), ClosingMode::Off),
        (Some("once"), ClosingMode::Once),
        (Some(" once\n"), ClosingMode::Once),
    ] {
        assert_eq!(
            parse_mode(raw.map(OsStr::new)).unwrap(),
            expected,
            "{raw:?}"
        );
    }
    let secret = "sk-test-957-NOT-A-KEY";
    for value in ["ONCE", "Off", "on", "true", "1", "twice", secret] {
        let error = parse_mode(Some(OsStr::new(value))).unwrap_err().to_string();
        assert_eq!(
            error,
            format!("invalid {ENV_NAME}: expected unset, empty, `off`, or `once`")
        );
        assert!(!error.contains(secret), "{error}");
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let raw = OsStr::from_bytes(&[0x6f, 0x6e, 0xff]);
        let error = parse_mode(Some(raw)).unwrap_err().to_string();
        assert!(error.contains(ENV_NAME), "{error}");
    }
}

#[test]
fn disabled_state_never_changes_a_request_records_or_emits() {
    let (dir, mut closing) = closing(false);
    for remaining in (0..10).rev() {
        let pending = Some("existing feedback");
        let feedback = closing.request_feedback(remaining, 10, pending);
        assert!(matches!(feedback, Some(Cow::Borrowed("existing feedback"))));
        assert_eq!(closing.request_feedback(remaining, 10, None), None);
        closing.note_request(&[ConversationMessage::user(NOTICE)]);
        closing.note_provider_call(false);
    }
    closing.note_bash(Some("false"), ExecutionProgress::NotSuccessful);
    closing.note_bash_rejected("Bash");
    closing.note_write_failure("Edit", &edit("a.txt", "x", "y"));
    for iteration in 1..5 {
        assert_eq!(closing.before_normal_final(iteration, 1), None);
    }
    closing.note_budget_exhausted();
    assert!(events(&dir).is_empty());
    assert_eq!(closing.ledger.unresolved(), 0);
    assert_eq!(closing.ledger.rejected_bash, 0);
}

#[test]
fn notice_is_composed_only_when_three_calls_remain_and_follows_feedback() {
    let (dir, mut closing) = closing(true);
    let limit = 9;
    for remaining in [8, 7, 6, 5, 4, 3] {
        assert_eq!(closing.request_feedback(remaining, limit, None), None);
        assert!(matches!(
            closing.request_feedback(remaining, limit, Some("kept")),
            Some(Cow::Borrowed("kept"))
        ));
    }
    let composed = closing
        .request_feedback(2, limit, Some("existing feedback"))
        .unwrap()
        .into_owned();
    assert_eq!(composed, format!("existing feedback\n\n{NOTICE}"));
    closing.note_request(&[ConversationMessage::user(composed)]);
    // Built but not yet returned from the provider call: nothing recorded.
    assert!(events(&dir).is_empty());
    closing.note_provider_call(false);
    for remaining in [1, 0] {
        assert_eq!(closing.request_feedback(remaining, limit, None), None);
    }
    let events = events(&dir);
    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(events[0]["phase"], "notice");
    assert_eq!(events[0]["guard_reason"], "remaining_calls_reached");
    assert_eq!(events[0]["iteration"], 7);
    assert_eq!(events[0]["iteration_limit"], 9);
    assert_eq!(events[0]["remaining_calls"], 3);
    assert_eq!(events[0]["prompt_version"], PROMPT_VERSION);
    assert_fixed_shape(&events[0]);
}

#[test]
fn a_limit_of_three_or_less_never_composes_the_notice() {
    for limit in 0..=CLOSING_REMAINING_CALLS {
        let (dir, mut closing) = closing(true);
        for remaining in (0..limit).rev() {
            assert_eq!(closing.request_feedback(remaining, limit, None), None);
        }
        assert!(events(&dir).is_empty(), "{limit}");
    }
}

#[test]
fn a_composed_notice_that_is_not_in_the_sent_request_is_not_recorded() {
    let (dir, mut closing) = closing(true);
    assert!(closing.request_feedback(2, 6, None).is_some());
    closing.note_request(&[ConversationMessage::user("something else")]);
    closing.note_provider_call(false);
    // A later request never carries it, and nothing was recorded.
    closing.note_request(&[ConversationMessage::user(NOTICE)]);
    closing.note_provider_call(false);
    assert_eq!(closing.request_feedback(1, 6, None), None);
    assert!(events(&dir).is_empty());
    // Without a recorded notice the guard does not apply.
    closing.note_bash(Some("false"), ExecutionProgress::NotSuccessful);
    assert_eq!(closing.before_normal_final(5, 1), None);
    assert!(events(&dir).is_empty());
}

#[test]
fn a_notice_whose_provider_call_was_aborted_is_not_recorded() {
    let (dir, mut closing) = closing(true);
    let feedback = closing
        .request_feedback(2, 6, None)
        .map(Cow::into_owned)
        .unwrap();
    closing.note_request(&[ConversationMessage::user(feedback)]);
    closing.note_provider_call(true);
    // A later call does not record it either; the guard never applies.
    closing.note_provider_call(false);
    assert_eq!(closing.request_feedback(1, 6, None), None);
    closing.note_bash(Some("false"), ExecutionProgress::NotSuccessful);
    assert_eq!(closing.before_normal_final(5, 1), None);
    assert_eq!(closing.before_post_write_read_completion(5, 1), None);
    closing.note_budget_exhausted();
    assert!(events(&dir).is_empty());
}

#[test]
fn a_bash_refused_before_it_ran_is_counted_and_not_a_failure() {
    let (_dir, mut closing) = closing(true);
    let root = closing.root.clone();
    let refusals = [
        hidden_path::path_error(&root, &root.join(".anvil/missing")),
        anyhow!("missing string argument `command`"),
        anyhow!("bash_path_confinement_error: command remains blocked"),
    ];
    for error in &refusals {
        closing.note_tool_error("Bash", &json!({"command": "cat .anvil/missing"}), error);
        // The observation that follows for the same call is not a result.
        closing.note_bash(Some("cat .anvil/missing"), ExecutionProgress::NotSuccessful);
    }
    assert_eq!(closing.ledger.rejected_bash, 3);
    assert_eq!(closing.ledger.unresolved_bash(), 0);
    // The next call's observation counts again.
    closing.note_bash(Some("false"), ExecutionProgress::NotSuccessful);
    assert_eq!(closing.ledger.unresolved_bash(), 1);
}

#[test]
fn a_bash_error_that_does_not_prove_a_refusal_stays_a_failure() {
    let (_dir, mut closing) = closing(true);
    for error in [
        anyhow!("command_timeout: sleep 99"),
        anyhow!("something unexpected"),
    ] {
        closing.note_tool_error("Bash", &json!({"command": "sleep 99"}), &error);
        closing.note_bash(Some("sleep 99"), ExecutionProgress::NotSuccessful);
    }
    assert_eq!(closing.ledger.rejected_bash, 0);
    assert_eq!(closing.ledger.unresolved_bash(), 1);
    // A Write error through the same entry is a write failure.
    closing.note_tool_error(
        "Write",
        &write("notes.txt", "v1\n"),
        &anyhow!("path_not_found_recoverable"),
    );
    assert_eq!(closing.ledger.unresolved_writes(), 1);
}

#[test]
fn bash_failure_is_resolved_only_by_the_same_command_succeeding() {
    let (_dir, mut closing) = closing(true);
    closing.note_bash(
        Some("grep -q v2 notes.txt"),
        ExecutionProgress::NotSuccessful,
    );
    closing.note_bash(Some("true"), ExecutionProgress::New);
    closing.note_bash(Some("grep -q v2 other.txt"), ExecutionProgress::New);
    closing.note_bash(
        Some("grep -q v2 notes.txt"),
        ExecutionProgress::NotSuccessful,
    );
    assert_eq!(closing.ledger.unresolved_bash(), 1);
    closing.note_bash(Some(" grep -q v2 notes.txt "), ExecutionProgress::Repeated);
    assert_eq!(closing.ledger.unresolved_bash(), 0);
    assert_eq!(closing.ledger.resolved, 1);
}

#[test]
fn bash_failure_without_a_command_cannot_be_resolved() {
    let (_dir, mut closing) = closing(true);
    closing.note_bash(None, ExecutionProgress::NotSuccessful);
    closing.note_bash(Some("  "), ExecutionProgress::NotSuccessful);
    closing.note_bash(Some("true"), ExecutionProgress::New);
    assert_eq!(closing.ledger.unresolved_bash(), 2);
}

#[test]
fn rejected_bash_is_counted_but_never_a_failure() {
    let (_dir, mut closing) = closing(true);
    closing.note_bash_rejected("Bash");
    closing.note_bash_rejected("Write");
    assert_eq!(closing.ledger.rejected_bash, 1);
    assert_eq!(closing.ledger.unresolved(), 0);
}

#[test]
fn failed_edit_is_resolved_only_by_the_same_edit_succeeding() {
    let (_dir, mut closing) = closing(true);
    closing.note_write_failure("Edit", &edit("notes.txt", "A", "B"));
    // Another anchor in the same file, another file, and the same edit
    // with replace_all are different operations.
    closing.note_write_success("Edit", &edit("notes.txt", "C", "D"));
    closing.note_write_success("Edit", &edit("other.txt", "A", "B"));
    let mut replace_all = edit("notes.txt", "A", "B");
    replace_all["replace_all"] = json!(true);
    closing.note_write_success("Edit", &replace_all);
    // A Write to another path or one that does not show the edit's effect.
    closing.note_write_success("Write", &write("other.txt", "B\n"));
    closing.note_write_success("Write", &write("notes.txt", "A\n"));
    closing.note_write_success("Write", &write("notes.txt", "A and B\n"));
    // Read, Glob, and Grep are not writes.
    closing.note_write_success("Read", &json!({"path": "notes.txt"}));
    assert_eq!(closing.ledger.unresolved_writes(), 1);
    closing.note_write_success("Edit", &edit("./notes.txt", "A", "B"));
    assert_eq!(closing.ledger.unresolved_writes(), 0);
    assert_eq!(closing.ledger.resolved, 1);
}

#[test]
fn two_failed_edits_need_two_matching_successes() {
    let (_dir, mut closing) = closing(true);
    closing.note_write_failure("Edit", &edit("notes.txt", "A", "B"));
    closing.note_write_failure("Edit", &edit("notes.txt", "Z", "Y"));
    closing.note_write_success("Edit", &edit("notes.txt", "A", "B"));
    assert_eq!(closing.ledger.unresolved_writes(), 1);
    closing.note_write_success("Edit", &edit("notes.txt", "Z", "Y"));
    assert_eq!(closing.ledger.unresolved_writes(), 0);
}

#[test]
fn a_whole_file_write_resolves_an_edit_only_when_it_shows_the_effect() {
    let (_dir, mut closing) = closing(true);
    closing.note_write_failure("Edit", &edit("notes.txt", "A", "B"));
    closing.note_write_failure("Edit", &edit("notes.txt", "", "X"));
    closing.note_write_success("Write", &write("notes.txt", "B\n"));
    // The edit without replaced text has no defined effect and stays.
    assert_eq!(closing.ledger.unresolved_writes(), 1);
    assert_eq!(closing.ledger.resolved, 1);
}

#[test]
fn an_unchanged_write_never_resolves_an_edit_whose_replacement_contains_its_anchor() {
    let (_dir, mut closing) = closing(true);
    // The ambiguous edit `foo` → `foobar` failed on `foobar foo`.
    closing.note_write_failure("Edit", &edit("notes.txt", "foo", "foobar"));
    closing.note_write_success("Write", &write("notes.txt", "foobar foo"));
    // Even content without a second `foo` cannot show the edit was made.
    closing.note_write_success("Write", &write("notes.txt", "foobar"));
    assert_eq!(closing.ledger.unresolved_writes(), 1);
    // replace_all is part of the operation.
    let mut replace_all = edit("notes.txt", "foo", "foobar");
    replace_all["replace_all"] = json!(true);
    closing.note_write_success("Edit", &replace_all);
    assert_eq!(closing.ledger.unresolved_writes(), 1);
    closing.note_write_success("Edit", &edit("notes.txt", "foo", "foobar"));
    assert_eq!(closing.ledger.unresolved_writes(), 0);
    // A failed replace_all edit needs a replace_all success.
    closing.note_write_failure("Edit", &replace_all);
    closing.note_write_success("Edit", &edit("notes.txt", "foo", "foobar"));
    assert_eq!(closing.ledger.unresolved_writes(), 1);
    closing.note_write_success("Edit", &replace_all);
    assert_eq!(closing.ledger.unresolved_writes(), 0);
}

#[test]
fn repeated_separators_and_dot_components_name_the_same_target() {
    let (_dir, mut closing) = closing(true);
    closing.note_write_failure("Write", &write("a//b.txt", "v1\n"));
    closing.note_write_failure("Write", &write("a/./b.txt", "v1\n"));
    closing.note_write_success("Write", &write("a/b.txt", "v1\n"));
    assert_eq!(closing.ledger.unresolved_writes(), 0);
    // `..` is not resolved: `a/../b.txt` stays another target than `b.txt`.
    closing.note_write_failure("Write", &write("a/../b.txt", "v1\n"));
    closing.note_write_success("Write", &write("b.txt", "v1\n"));
    assert_eq!(closing.ledger.unresolved_writes(), 1);
}

#[test]
fn the_post_write_read_completion_follows_the_same_guard() {
    let (dir, mut closing) = closing(true);
    closing.note_write_failure("Write", &write(".env", "v1\n"));
    closing.note_write_success("Write", &write("notes.txt", "v1\n"));
    // Before the notice the existing completion is left alone.
    assert_eq!(closing.before_post_write_read_completion(2, 3), None);
    assert!(events(&dir).is_empty());
    send_notice(&mut closing, 5);
    for iteration in 3..6 {
        assert_eq!(
            closing.before_post_write_read_completion(iteration, 5 - iteration),
            Some(DEFERRAL_FEEDBACK)
        );
    }
    closing.note_budget_exhausted();
    assert_eq!(
        phases(&dir),
        [
            "notice",
            "final_deferred",
            "final_deferred",
            "final_deferred",
            "exhausted_after_notice"
        ]
    );
    // Without an unresolved failure it is accepted like a normal final.
    let (dir, mut closing) = self::closing(true);
    closing.note_write_success("Write", &write("notes.txt", "v1\n"));
    send_notice(&mut closing, 5);
    assert_eq!(closing.before_post_write_read_completion(3, 2), None);
    assert_eq!(phases(&dir), ["notice", "final_accepted"]);
}

#[test]
fn failed_write_is_resolved_only_by_the_same_write_succeeding() {
    let (_dir, mut closing) = closing(true);
    closing.note_write_failure("Write", &write("notes.txt", "v1\n"));
    closing.note_write_success("Write", &write("notes.txt", "v2\n"));
    closing.note_write_success("Write", &write("other.txt", "v1\n"));
    closing.note_write_success("Edit", &edit("notes.txt", "v2", "v1"));
    assert_eq!(closing.ledger.unresolved_writes(), 1);
    closing.note_write_success("Write", &write("notes.txt", "v1\n"));
    assert_eq!(closing.ledger.unresolved_writes(), 0);
}

#[test]
fn a_failure_without_identifying_arguments_stays_unresolved() {
    let (_dir, mut closing) = closing(true);
    closing.note_write_failure("Write", &json!({"content": "v1\n"}));
    closing.note_write_failure("Edit", &json!({"path": "notes.txt", "old_string": "A"}));
    closing.note_write_failure("Bash", &json!({"command": "false"}));
    closing.note_write_success("Write", &write("notes.txt", "v1\n"));
    closing.note_write_success("Edit", &edit("notes.txt", "A", "B"));
    assert_eq!(closing.ledger.unresolved_writes(), 2);
    assert_eq!(closing.ledger.unresolved_bash(), 0);
}

#[test]
fn finals_before_the_notice_are_left_to_the_existing_checks() {
    let (dir, mut closing) = closing(true);
    closing.note_bash(Some("false"), ExecutionProgress::NotSuccessful);
    for iteration in 1..5 {
        assert_eq!(closing.before_normal_final(iteration, 6), None);
    }
    closing.note_budget_exhausted();
    assert!(events(&dir).is_empty());
}

#[test]
fn every_final_after_the_notice_is_deferred_while_a_failure_remains() {
    let (dir, mut closing) = closing(true);
    closing.note_bash(
        Some("grep -q v2 notes.txt"),
        ExecutionProgress::NotSuccessful,
    );
    closing.note_write_failure("Edit", &edit("notes.txt", "A", "B"));
    send_notice(&mut closing, 20);
    for iteration in 19..30 {
        assert_eq!(
            closing.before_normal_final(iteration, 1),
            Some(DEFERRAL_FEEDBACK),
            "{iteration}"
        );
    }
    closing.note_budget_exhausted();
    closing.note_budget_exhausted();
    let events = events(&dir);
    let mut expected = vec!["notice".to_string()];
    expected.extend(std::iter::repeat_n("final_deferred".to_string(), 11));
    expected.push("exhausted_after_notice".to_string());
    assert_eq!(phases_of(&events), expected);
    let deferred = &events[11];
    assert_eq!(deferred["guard_reason"], "unresolved_failure");
    assert_eq!(deferred["unresolved_bash_failures"], 1);
    assert_eq!(deferred["unresolved_write_failures"], 1);
    assert_eq!(deferred["final_deferrals"], 11);
    let exhausted = events.last().unwrap();
    assert_eq!(exhausted["iteration"], 20);
    assert_eq!(exhausted["remaining_calls"], 0);
    assert_eq!(exhausted["final_deferrals"], 11);
    for event in &events {
        assert_fixed_shape(event);
    }
}

fn phases_of(events: &[Value]) -> Vec<String> {
    events
        .iter()
        .map(|event| event["phase"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn a_final_after_the_failures_are_resolved_is_accepted_once() {
    let (dir, mut closing) = closing(true);
    closing.note_bash(
        Some("grep -q v2 notes.txt"),
        ExecutionProgress::NotSuccessful,
    );
    send_notice(&mut closing, 6);
    assert_eq!(closing.before_normal_final(4, 2), Some(DEFERRAL_FEEDBACK));
    closing.note_bash(Some("grep -q v2 notes.txt"), ExecutionProgress::New);
    assert_eq!(closing.before_normal_final(5, 1), None);
    // The run returns there; nothing later is recorded.
    closing.note_budget_exhausted();
    assert_eq!(phases(&dir), ["notice", "final_deferred", "final_accepted"]);
    let accepted = &events(&dir)[2];
    assert_eq!(accepted["guard_reason"], "no_unresolved_failure");
    assert_eq!(accepted["resolved_failures"], 1);
    assert_eq!(accepted["remaining_calls"], 2);
}

#[test]
fn self_reports_and_unrelated_tools_never_release_the_guard() {
    let (dir, mut closing) = closing(true);
    closing.note_bash(
        Some("grep -q v2 notes.txt"),
        ExecutionProgress::NotSuccessful,
    );
    send_notice(&mut closing, 6);
    closing.note_write_success("Write", &write("notes.txt", "v2\n"));
    closing.note_write_success("Read", &json!({"path": "notes.txt"}));
    closing.note_bash(Some("true"), ExecutionProgress::New);
    closing.note_bash_rejected("Bash");
    for iteration in 4..7 {
        assert_eq!(
            closing.before_normal_final(iteration, 6 - iteration),
            Some(DEFERRAL_FEEDBACK)
        );
    }
    assert_eq!(phases(&dir).len(), 4);
}

/// The message text is never copied into an event.
#[test]
fn events_copy_no_command_path_or_content() {
    let (dir, mut closing) = closing(true);
    let secret = "sk-test-957-NOT-A-KEY";
    closing.note_bash(
        Some(&format!("echo {secret}")),
        ExecutionProgress::NotSuccessful,
    );
    closing.note_write_failure("Write", &write(&format!("{secret}.txt"), secret));
    send_notice(&mut closing, 6);
    let _ = closing.before_normal_final(4, 2);
    closing.note_budget_exhausted();
    let text = std::fs::read_to_string(dir.path().canonicalize().unwrap().join("e.jsonl")).unwrap();
    assert!(!text.contains(secret), "{text}");
    assert!(!text.contains("echo"), "{text}");
    for event in events(&dir) {
        assert_fixed_shape(&event);
    }
}

#[test]
fn scope_is_a_generic_direct_run_without_a_contract() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = config(dir.path());
    assert!(in_scope(&config, &RunSessionOptions::default(), false));
    assert!(!in_scope(&config, &RunSessionOptions::default(), true));
    for options in [
        RunSessionOptions::plan_step(RunSessionStepKind::Implement),
        RunSessionOptions::plan_step(RunSessionStepKind::Unknown),
        RunSessionOptions::final_acceptance_repair(),
        RunSessionOptions {
            step_kind: Some(RunSessionStepKind::Implement),
            ..RunSessionOptions::default()
        },
        RunSessionOptions {
            phase_scope: Some("recovery".to_string()),
            ..RunSessionOptions::default()
        },
    ] {
        assert!(!in_scope(&config, &options, false), "{options:?}");
    }
    for profile in [
        PYTHON_CLI_PROFILE_ID,
        NEXTJS_PROFILE_ID,
        DATA_PROFILE_ID,
        INGEST_PROFILE_ID,
    ] {
        config.profile = profile.to_string();
        assert!(
            !in_scope(&config, &RunSessionOptions::default(), false),
            "{profile}"
        );
    }
}

#[derive(Clone)]
struct Scripted {
    replies: Arc<Mutex<Vec<AssistantReply>>>,
    requests: Arc<Mutex<Vec<Vec<ConversationMessage>>>>,
}

impl ChatClient for Scripted {
    fn label(&self) -> &str {
        "scripted"
    }
    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }
    fn supports_native_tools(&self, _model: &str) -> bool {
        true
    }
    fn chat(
        &mut self,
        _model: &str,
        messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        self.requests.lock().unwrap().push(messages.to_vec());
        let mut replies = self.replies.lock().unwrap();
        if replies.is_empty() {
            anyhow::bail!("scripted client exhausted");
        }
        Ok(replies.remove(0))
    }
}

fn reply(content: &str, tool_calls: Vec<ToolCall>) -> AssistantReply {
    AssistantReply {
        content: content.to_string(),
        tool_calls,
        prompt_tokens: None,
        completion_tokens: None,
    }
}

fn call(name: &str, arguments: Value) -> Vec<ToolCall> {
    vec![ToolCall {
        id: format!("call-{name}"),
        name: name.to_string(),
        arguments,
    }]
}

fn config(root: &Path) -> Config {
    Config {
        workspace_root: root.to_path_buf(),
        state_dir: root.join("state"),
        eval_events_path: Some(root.join("events.jsonl")),
        completion_contract_path: None,
        yes: true,
        offline: false,
        context_budget: 100_000,
        model: "m".to_string(),
        provider: crate::config::Provider::Ollama,
        tool_protocol: None,
        openai_api: crate::config::OpenAiApi::ChatCompletions,
        prompt_layout: crate::config::PromptLayout::Stable,
        plan_preset: crate::config::PlanPreset::None,
        intent_override: None,
        planner_model: "m".to_string(),
        planner_provider: crate::config::Provider::Ollama,
        planner_think: Some(crate::config::OllamaThink::False),
        classifier_model: "m".to_string(),
        classifier_provider: crate::config::Provider::Ollama,
        openai_compatible: None,
        ollama_host: "http://localhost:11434".to_string(),
        ollama_think: None,
        lm_studio_host: "http://localhost:1234".to_string(),
        num_predict: 100,
        max_iterations: 2,
        recovery_plan_auto_runs: 0,
        chat_timeout_secs: 30,
        chat_timeout_source: "override:test".to_string(),
        field_sources: crate::config::ConfigFieldSources::default(),
        chat_retries: 1,
        stream: false,
        resume: None,
        fresh_session: false,
        no_footer: false,
        narration: crate::config::NarrationMode::Normal,
        profile: "generic".to_string(),
        profile_explicit: true,
        profile_inference: None,
        style: "default".to_string(),
        action: crate::config::Action::Repl,
    }
}

/// What a run produced, with the workspace root normalized.
#[derive(Debug, PartialEq)]
struct RunTrace {
    outcome: String,
    requests: Vec<String>,
    events: Vec<String>,
    closing_events: usize,
}

/// Runs write → failed check → finals with the mode set for this thread.
fn run_with_options(setting: Option<&str>, options: RunSessionOptions) -> RunTrace {
    TEST_ENV.with(|value| *value.borrow_mut() = setting.map(OsString::from));
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let mut client = Scripted {
        replies: Arc::new(Mutex::new(vec![
            reply("", call("Write", write("notes.txt", "v1\n"))),
            reply("", call("Bash", json!({"command": "grep -q v2 notes.txt"}))),
            reply("Done.", Vec::new()),
            reply("Done.", Vec::new()),
            reply("Done.", Vec::new()),
            reply("Done.", Vec::new()),
            reply("Done.", Vec::new()),
        ])),
        requests: Arc::new(Mutex::new(Vec::new())),
    };
    let result = run_session_with_outcome_with_options(
        &mut client,
        &mut SessionSnapshot::new(),
        "Write the notes file and check it with shell commands.",
        &[],
        &config(&root),
        &NOOP_UI,
        options,
    );
    TEST_ENV.with(|value| *value.borrow_mut() = None);
    let root_text = root.display().to_string();
    let normalize = |text: String| {
        let text = text.replace(&root_text, "<ROOT>");
        regex::Regex::new(r"elapsed_ms: [0-9]+")
            .unwrap()
            .replace_all(&text, "elapsed_ms: <N>")
            .into_owned()
    };
    let outcome = normalize(match result {
        Ok(outcome) => format!("{:?} {}", outcome.stop_reason, outcome.final_text),
        Err(error) => format!("{error:#}"),
    });
    let requests = client
        .requests
        .lock()
        .unwrap()
        .iter()
        .map(|messages| normalize(serde_json::to_string(messages).unwrap()))
        .collect();
    let events = read_events(&root.join("events.jsonl"));
    RunTrace {
        outcome,
        closing_events: events
            .iter()
            .filter(|event| event["event"] == "budget_closing")
            .count(),
        events: events
            .iter()
            .map(|event| event["event"].as_str().unwrap_or_default().to_string())
            .collect(),
        requests,
    }
}

#[test]
fn plan_steps_and_final_acceptance_repair_behave_exactly_as_off() {
    let cases: [fn() -> RunSessionOptions; 4] = [
        || RunSessionOptions::plan_step(RunSessionStepKind::Implement),
        || RunSessionOptions::plan_step(RunSessionStepKind::Unknown),
        RunSessionOptions::final_acceptance_repair,
        || RunSessionOptions {
            phase_scope: Some("recovery".to_string()),
            ..RunSessionOptions::default()
        },
    ];
    for options in cases {
        let off = run_with_options(None, options());
        let once = run_with_options(Some("once"), options());
        assert_eq!(once, off);
        assert_eq!(once.closing_events, 0);
        assert!(once.requests.len() >= CLOSING_REMAINING_CALLS, "{once:?}");
        for request in &once.requests {
            assert!(!request.contains(NOTICE), "{request}");
        }
    }
}

#[test]
fn the_direct_run_with_the_same_script_is_guarded() {
    let off = run_with_options(None, RunSessionOptions::default());
    let once = run_with_options(Some("once"), RunSessionOptions::default());
    assert_eq!(off.outcome, "AssistantFinal Done.");
    assert_ne!(once.outcome, off.outcome);
    assert!(once.closing_events >= 2, "{once:?}");
    assert!(once.requests[2].contains(NOTICE));
}
