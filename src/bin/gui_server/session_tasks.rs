use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

const PLAN_STEP_SCHEMA_VERSION: u8 = 1;
const MAX_CHANGED_PATHS: usize = 16;
const MAX_VERIFICATION_FAILURES: usize = 4;

#[derive(Debug, Serialize)]
pub(super) struct TaskProgress {
    status: &'static str,
    executions: Vec<TaskExecution>,
}

#[derive(Debug, Serialize)]
struct TaskExecution {
    execution_index: usize,
    plan_execution_id: String,
    mode: String,
    phase_id: Option<String>,
    total_steps: usize,
    tasks: Vec<TaskStatus>,
}

#[derive(Debug, Serialize)]
struct TaskStatus {
    step_execution_id: String,
    step_index: usize,
    total_steps: usize,
    step_id: String,
    step_kind: String,
    status: &'static str,
    outcome: Option<String>,
    verification_status: Option<String>,
    verification_failure_count: usize,
    verification_failures: Vec<String>,
    verification_failures_truncated: bool,
    changed_path_count: usize,
    changed_paths: Vec<String>,
    changed_paths_truncated: bool,
    repair_attempts: usize,
    failure_summary: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct StepIdentity {
    plan_execution_id: String,
    step_execution_id: String,
    session_id: String,
    mode: String,
    phase_id: Option<String>,
    step_index: usize,
    total_steps: usize,
    step_id: String,
    step_kind: String,
}

#[derive(Debug, Deserialize)]
struct StartedEvent {
    event: String,
    plan_step_schema_version: u8,
    #[serde(flatten)]
    identity: StepIdentity,
}

#[derive(Debug, Deserialize)]
struct TerminalEvent {
    event: String,
    plan_step_schema_version: u8,
    #[serde(flatten)]
    identity: StepIdentity,
    terminal_status: String,
    outcome: String,
    ok: bool,
    completion_count_delta: usize,
    changed_paths: Vec<String>,
    changed_path_count: usize,
    changed_paths_truncated: bool,
    verification_status: String,
    verification_failure_count: usize,
    verification_failures: Vec<String>,
    verification_failures_truncated: bool,
    repair_attempts: usize,
    failure_summary: String,
}

pub(super) fn project(events: &[Value], terminal: bool) -> TaskProgress {
    let mut executions = Vec::<TaskExecution>::new();
    let mut execution_indices = HashMap::<String, usize>::new();
    let mut task_indices = HashMap::<String, (usize, usize, StepIdentity)>::new();
    let mut typed_event_seen = false;

    for value in events {
        let Some(event_name) = value.get("event").and_then(Value::as_str) else {
            continue;
        };
        match event_name {
            "plan_step_started" => {
                typed_event_seen = true;
                let Ok(event) = serde_json::from_value::<StartedEvent>(value.clone()) else {
                    return unsupported();
                };
                if event.event != event_name
                    || event.plan_step_schema_version != PLAN_STEP_SCHEMA_VERSION
                    || event.identity.step_index == 0
                    || event.identity.step_index > event.identity.total_steps
                    || task_indices.contains_key(&event.identity.step_execution_id)
                {
                    return unsupported();
                }
                let execution_index = match execution_indices.get(&event.identity.plan_execution_id)
                {
                    Some(index) => *index,
                    None => {
                        let index = executions.len();
                        execution_indices.insert(event.identity.plan_execution_id.clone(), index);
                        executions.push(TaskExecution {
                            execution_index: index + 1,
                            plan_execution_id: event.identity.plan_execution_id.clone(),
                            mode: event.identity.mode.clone(),
                            phase_id: event.identity.phase_id.clone(),
                            total_steps: event.identity.total_steps,
                            tasks: Vec::new(),
                        });
                        index
                    }
                };
                let execution = &mut executions[execution_index];
                if execution.mode != event.identity.mode
                    || execution.phase_id != event.identity.phase_id
                    || execution.total_steps != event.identity.total_steps
                    || execution
                        .tasks
                        .iter()
                        .any(|task| task.step_index == event.identity.step_index)
                {
                    return unsupported();
                }
                let task_index = execution.tasks.len();
                execution.tasks.push(TaskStatus {
                    step_execution_id: event.identity.step_execution_id.clone(),
                    step_index: event.identity.step_index,
                    total_steps: event.identity.total_steps,
                    step_id: event.identity.step_id.clone(),
                    step_kind: event.identity.step_kind.clone(),
                    status: "running",
                    outcome: None,
                    verification_status: None,
                    verification_failure_count: 0,
                    verification_failures: Vec::new(),
                    verification_failures_truncated: false,
                    changed_path_count: 0,
                    changed_paths: Vec::new(),
                    changed_paths_truncated: false,
                    repair_attempts: 0,
                    failure_summary: None,
                });
                task_indices.insert(
                    event.identity.step_execution_id.clone(),
                    (execution_index, task_index, event.identity),
                );
            }
            "plan_step_completed" | "plan_step_failed" => {
                typed_event_seen = true;
                let Ok(event) = serde_json::from_value::<TerminalEvent>(value.clone()) else {
                    return unsupported();
                };
                let Some((execution_index, task_index, identity)) =
                    task_indices.get(&event.identity.step_execution_id)
                else {
                    return unsupported();
                };
                if event.event != event_name
                    || event.plan_step_schema_version != PLAN_STEP_SCHEMA_VERSION
                    || event.identity != *identity
                    || !terminal_payload_is_bounded(&event)
                {
                    return unsupported();
                }
                let task = &mut executions[*execution_index].tasks[*task_index];
                if task.status != "running" {
                    return unsupported();
                }
                let Some(status) = terminal_status(&event) else {
                    return unsupported();
                };
                task.status = status;
                task.outcome = Some(event.outcome);
                task.verification_status = Some(event.verification_status);
                task.verification_failure_count = event.verification_failure_count;
                task.verification_failures = event.verification_failures;
                task.verification_failures_truncated = event.verification_failures_truncated;
                task.changed_path_count = event.changed_path_count;
                task.changed_paths = event.changed_paths;
                task.changed_paths_truncated = event.changed_paths_truncated;
                task.repair_attempts = event.repair_attempts;
                task.failure_summary =
                    (!event.failure_summary.is_empty()).then_some(event.failure_summary);
            }
            _ => {}
        }
    }

    if !typed_event_seen {
        return TaskProgress {
            status: if terminal { "unsupported" } else { "pending" },
            executions,
        };
    }
    if terminal
        && executions
            .iter()
            .flat_map(|execution| &execution.tasks)
            .any(|task| task.status == "running")
    {
        return unsupported();
    }
    TaskProgress {
        status: "supported",
        executions,
    }
}

fn terminal_payload_is_bounded(event: &TerminalEvent) -> bool {
    bounded_count_is_consistent(
        event.changed_path_count,
        event.changed_paths.len(),
        event.changed_paths_truncated,
        MAX_CHANGED_PATHS,
    ) && bounded_count_is_consistent(
        event.verification_failure_count,
        event.verification_failures.len(),
        event.verification_failures_truncated,
        MAX_VERIFICATION_FAILURES,
    )
}

fn bounded_count_is_consistent(
    count: usize,
    shown: usize,
    truncated: bool,
    maximum: usize,
) -> bool {
    shown <= maximum
        && if truncated {
            count > shown
        } else {
            count == shown
        }
}

fn terminal_status(event: &TerminalEvent) -> Option<&'static str> {
    match (
        event.event.as_str(),
        event.terminal_status.as_str(),
        event.outcome.as_str(),
        event.ok,
        event.completion_count_delta,
    ) {
        ("plan_step_completed", "completed", "completed" | "completed_after_rollback", true, 1) => {
            Some("completed")
        }
        ("plan_step_completed", "skipped", "short_circuited", true, 1) => Some("short_circuited"),
        (
            "plan_step_failed",
            "failed",
            "verification_failed" | "bounded_repair_failed" | "execution_failed",
            false,
            0,
        ) => Some("failed"),
        ("plan_step_failed", "interrupted", "interrupted", false, 0) => Some("interrupted"),
        _ => None,
    }
}

fn unsupported() -> TaskProgress {
    TaskProgress {
        status: "unsupported",
        executions: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_events_keep_duplicate_step_ids_in_separate_execution_intervals() {
        let events = vec![
            started("plan-a", "step-a", 1, 1, "same-id", "phase-a"),
            completed("plan-a", "step-a", 1, 1, "same-id", "phase-a", false),
            started("plan-b", "step-b", 1, 1, "same-id", "phase-b"),
            completed("plan-b", "step-b", 1, 1, "same-id", "phase-b", true),
        ];

        let projection = serde_json::to_value(project(&events, true)).unwrap();

        assert_eq!(projection["status"], "supported");
        assert_eq!(projection["executions"].as_array().unwrap().len(), 2);
        assert_eq!(projection["executions"][0]["execution_index"], 1);
        assert_eq!(projection["executions"][1]["execution_index"], 2);
        assert_eq!(
            projection["executions"][0]["tasks"][0]["step_id"],
            "same-id"
        );
        assert_eq!(
            projection["executions"][0]["tasks"][0]["status"],
            "completed"
        );
        assert_eq!(
            projection["executions"][1]["tasks"][0]["status"],
            "short_circuited"
        );
    }

    #[test]
    fn typed_terminal_fields_are_the_only_task_outcome_authority() {
        let events = vec![
            started("plan", "complete", 1, 4, "complete", "implementation"),
            completed(
                "plan",
                "complete",
                1,
                4,
                "complete",
                "implementation",
                false,
            ),
            started("plan", "short", 2, 4, "short", "implementation"),
            completed("plan", "short", 2, 4, "short", "implementation", true),
            started("plan", "failed", 3, 4, "failed", "implementation"),
            failed("plan", "failed", 3, 4, "failed", "implementation", false),
            started("plan", "interrupted", 4, 4, "interrupted", "implementation"),
            failed(
                "plan",
                "interrupted",
                4,
                4,
                "interrupted",
                "implementation",
                true,
            ),
            serde_json::json!({"event": "ultra_phase_complete", "phase_id": "implementation"}),
        ];

        let projection = serde_json::to_value(project(&events, true)).unwrap();
        let tasks = projection["executions"][0]["tasks"].as_array().unwrap();
        assert_eq!(tasks[0]["status"], "completed");
        assert_eq!(tasks[1]["status"], "short_circuited");
        assert_eq!(tasks[2]["status"], "failed");
        assert_eq!(tasks[2]["failure_summary"], "verification failed");
        assert_eq!(tasks[2]["verification_failures"][0], "missing output");
        assert_eq!(tasks[3]["status"], "interrupted");
    }

    #[test]
    fn active_started_task_is_running_but_terminal_incomplete_contract_is_unsupported() {
        let events = vec![
            started("plan", "step", 1, 1, "one", "implementation"),
            serde_json::json!({"event": "ultra_phase_complete", "phase_id": "implementation"}),
        ];

        let active = serde_json::to_value(project(&events, false)).unwrap();
        assert_eq!(active["status"], "supported");
        assert_eq!(active["executions"][0]["tasks"][0]["status"], "running");

        let terminal = serde_json::to_value(project(&events, true)).unwrap();
        assert_eq!(terminal["status"], "unsupported");
        assert!(terminal["executions"].as_array().unwrap().is_empty());
    }

    #[test]
    fn legacy_terminal_is_unsupported_without_guessing_counts() {
        let terminal = project(
            &[serde_json::json!({
                "event": "tui_command_stop",
                "status": "completed",
                "ok": true
            })],
            true,
        );
        let terminal = serde_json::to_value(terminal).unwrap();
        assert_eq!(terminal["status"], "unsupported");
        assert!(terminal["executions"].as_array().unwrap().is_empty());

        let active = serde_json::to_value(project(&[], false)).unwrap();
        assert_eq!(active["status"], "pending");
    }

    #[test]
    fn oversized_or_inconsistent_typed_payload_is_unsupported() {
        let mut terminal = failed("plan", "step", 1, 1, "one", "implementation", false);
        terminal["verification_failure_count"] = serde_json::json!(5);
        terminal["verification_failures"] = serde_json::json!(["1", "2", "3", "4", "5"]);
        terminal["verification_failures_truncated"] = serde_json::json!(true);

        let projection = serde_json::to_value(project(
            &[
                started("plan", "step", 1, 1, "one", "implementation"),
                terminal,
            ],
            true,
        ))
        .unwrap();

        assert_eq!(projection["status"], "unsupported");
        assert!(projection["executions"].as_array().unwrap().is_empty());
    }

    #[test]
    fn hundred_tasks_project_linearly_without_raw_event_payloads() {
        let mut events = Vec::new();
        for index in 1..=100 {
            let execution = format!("step-{index}");
            let id = format!("task-{index}");
            events.push(started(
                "plan",
                &execution,
                index,
                100,
                &id,
                "implementation",
            ));
            events.push(completed(
                "plan",
                &execution,
                index,
                100,
                &id,
                "implementation",
                false,
            ));
        }

        let projection = serde_json::to_value(project(&events, true)).unwrap();
        let tasks = projection["executions"][0]["tasks"].as_array().unwrap();

        assert_eq!(projection["status"], "supported");
        assert_eq!(tasks.len(), 100);
        assert!(tasks.iter().all(|task| task["status"] == "completed"));
        assert!(serde_json::to_vec(&projection).unwrap().len() < 128 * 1024);
        assert!(!projection.to_string().contains("plan_step_started"));
    }

    fn started(
        plan: &str,
        execution: &str,
        index: usize,
        total: usize,
        id: &str,
        phase: &str,
    ) -> Value {
        serde_json::json!({
            "event": "plan_step_started",
            "plan_step_schema_version": 1,
            "plan_execution_id": plan,
            "step_execution_id": execution,
            "session_id": "session",
            "mode": "ultra-plan",
            "phase_id": phase,
            "step_index": index,
            "total_steps": total,
            "step_id": id,
            "step_kind": "implement"
        })
    }

    fn completed(
        plan: &str,
        execution: &str,
        index: usize,
        total: usize,
        id: &str,
        phase: &str,
        short_circuited: bool,
    ) -> Value {
        terminal(
            plan,
            execution,
            index,
            total,
            id,
            phase,
            if short_circuited {
                "skipped"
            } else {
                "completed"
            },
            if short_circuited {
                "short_circuited"
            } else {
                "completed"
            },
            true,
            if short_circuited {
                "already satisfied"
            } else {
                ""
            },
        )
    }

    fn failed(
        plan: &str,
        execution: &str,
        index: usize,
        total: usize,
        id: &str,
        phase: &str,
        interrupted: bool,
    ) -> Value {
        terminal(
            plan,
            execution,
            index,
            total,
            id,
            phase,
            if interrupted { "interrupted" } else { "failed" },
            if interrupted {
                "interrupted"
            } else {
                "verification_failed"
            },
            false,
            if interrupted {
                "interrupted by user"
            } else {
                "verification failed"
            },
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn terminal(
        plan: &str,
        execution: &str,
        index: usize,
        total: usize,
        id: &str,
        phase: &str,
        terminal_status: &str,
        outcome: &str,
        ok: bool,
        failure_summary: &str,
    ) -> Value {
        serde_json::json!({
            "event": if ok { "plan_step_completed" } else { "plan_step_failed" },
            "plan_step_schema_version": 1,
            "plan_execution_id": plan,
            "step_execution_id": execution,
            "session_id": "session",
            "mode": "ultra-plan",
            "phase_id": phase,
            "step_index": index,
            "total_steps": total,
            "step_id": id,
            "step_kind": "implement",
            "terminal_status": terminal_status,
            "outcome": outcome,
            "ok": ok,
            "completion_count_delta": if ok { 1 } else { 0 },
            "failed_step_id": if ok { Value::Null } else { Value::String(id.to_string()) },
            "changed_paths": if ok { Vec::<String>::new() } else { vec!["src/app.rs".to_string()] },
            "changed_path_count": if ok { 0 } else { 1 },
            "changed_paths_truncated": false,
            "verification_status": if ok { "passed" } else if terminal_status == "interrupted" { "not_run" } else { "failed" },
            "verification_failure_count": if outcome == "verification_failed" { 1 } else { 0 },
            "verification_failures": if outcome == "verification_failed" { vec!["missing output".to_string()] } else { Vec::<String>::new() },
            "verification_failures_truncated": false,
            "repair_attempts": if outcome == "verification_failed" { 2 } else { 0 },
            "failure_summary": failure_summary
        })
    }
}
