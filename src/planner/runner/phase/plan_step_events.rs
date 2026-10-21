use super::*;

use serde::Serialize;

const PLAN_STEP_SCHEMA_VERSION: u8 = 1;
const MAX_CHANGED_PATHS: usize = 16;
const MAX_VERIFICATION_FAILURES: usize = 4;

pub(super) struct PlanStepEvents {
    path: Option<PathBuf>,
    plan_execution_id: String,
    session_id: String,
    mode: String,
    phase_id: Option<String>,
    total_steps: usize,
}

pub(super) struct PlanStepEvent {
    path: Option<PathBuf>,
    identity: PlanStepIdentity,
}

#[derive(Clone, Serialize)]
struct PlanStepIdentity {
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

#[derive(Serialize)]
struct PlanStepStarted {
    event: &'static str,
    plan_step_schema_version: u8,
    #[serde(flatten)]
    identity: PlanStepIdentity,
}

#[derive(Serialize)]
struct PlanStepTerminal {
    event: &'static str,
    plan_step_schema_version: u8,
    #[serde(flatten)]
    identity: PlanStepIdentity,
    terminal_status: &'static str,
    outcome: &'static str,
    ok: bool,
    rollback_applied: bool,
    completion_count_delta: usize,
    failed_step_id: Option<String>,
    changed_paths: Vec<String>,
    changed_path_count: usize,
    changed_paths_truncated: bool,
    verification_status: &'static str,
    verification_failure_count: usize,
    verification_failures: Vec<String>,
    verification_failures_truncated: bool,
    repair_attempts: usize,
    failure_summary: String,
}

impl PlanStepEvents {
    pub(super) fn new(
        plan: &StepPlan,
        config: &Config,
        session: &SessionSnapshot,
        mode: &str,
        phase_id: Option<&str>,
    ) -> Self {
        Self {
            path: config.eval_events_path.clone(),
            plan_execution_id: uuid::Uuid::now_v7().to_string(),
            session_id: eval_events::body_snippet(&session.id),
            mode: eval_events::body_snippet(mode),
            phase_id: phase_id.map(eval_events::body_snippet),
            total_steps: plan.steps.len(),
        }
    }

    pub(super) fn start(&self, step: &PlanStep, index: usize) -> PlanStepEvent {
        let identity = PlanStepIdentity {
            plan_execution_id: self.plan_execution_id.clone(),
            step_execution_id: uuid::Uuid::now_v7().to_string(),
            session_id: self.session_id.clone(),
            mode: self.mode.clone(),
            phase_id: self.phase_id.clone(),
            step_index: index + 1,
            total_steps: self.total_steps,
            step_id: eval_events::body_snippet(&step.id),
            step_kind: run_session_step_kind(step).as_str().to_string(),
        };
        emit(
            self.path.as_deref(),
            &PlanStepStarted {
                event: "plan_step_started",
                plan_step_schema_version: PLAN_STEP_SCHEMA_VERSION,
                identity: identity.clone(),
            },
        );
        PlanStepEvent {
            path: self.path.clone(),
            identity,
        }
    }
}

impl PlanStepEvent {
    pub(super) fn interrupted(&self) {
        self.emit_terminal(TerminalFields {
            terminal_status: "interrupted",
            outcome: "interrupted",
            failed: true,
            verification_status: "not_run",
            failure_summary: "interrupted by user",
            ..TerminalFields::default()
        });
    }

    pub(super) fn finish(&self, result: &Result<StepRunOutcome, StepRunError>) {
        match result {
            Ok(outcome) => self.finish_ok(outcome),
            Err(error) => self.finish_error(error),
        }
    }

    fn finish_ok(&self, result: &StepRunOutcome) {
        let short_circuited = result.stop_reason.as_deref() == Some("StepShortCircuited");
        let rolled_back = result.stop_reason.as_deref() == Some("compile_rollback_applied");
        self.emit_terminal(TerminalFields {
            terminal_status: if short_circuited {
                "skipped"
            } else {
                "completed"
            },
            outcome: if short_circuited {
                "short_circuited"
            } else if rolled_back {
                "completed_after_rollback"
            } else {
                "completed"
            },
            ok: true,
            rollback_applied: rolled_back,
            completion_count_delta: 1,
            changed_paths: &result.changed_paths,
            verification_status: if rolled_back {
                "failed_recovered"
            } else {
                "passed"
            },
            verification_failures: &result.verify_failures,
            repair_attempts: result.repair_attempts,
            ..TerminalFields::default()
        });
    }

    fn finish_error(&self, error: &StepRunError) {
        let interrupted = is_interrupted(&error.message);
        let verification_failed = !error.outcome.verify_failures.is_empty();
        let bounded_repair_failed = verification_failed && error.outcome.repair_attempts > 0;
        self.emit_terminal(TerminalFields {
            terminal_status: if interrupted { "interrupted" } else { "failed" },
            outcome: if interrupted {
                "interrupted"
            } else if bounded_repair_failed {
                "bounded_repair_failed"
            } else if verification_failed {
                "verification_failed"
            } else {
                "execution_failed"
            },
            failed: true,
            changed_paths: &error.outcome.changed_paths,
            verification_status: if interrupted {
                "interrupted"
            } else if verification_failed {
                "failed"
            } else {
                "not_run"
            },
            verification_failures: &error.outcome.verify_failures,
            repair_attempts: error.outcome.repair_attempts,
            failure_summary: &error.message,
            ..TerminalFields::default()
        });
    }

    fn emit_terminal(&self, fields: TerminalFields<'_>) {
        let changed_path_count = fields.changed_paths.len();
        let verification_failure_count = fields.verification_failures.len();
        emit(
            self.path.as_deref(),
            &PlanStepTerminal {
                event: if fields.ok {
                    "plan_step_completed"
                } else {
                    "plan_step_failed"
                },
                plan_step_schema_version: PLAN_STEP_SCHEMA_VERSION,
                identity: self.identity.clone(),
                terminal_status: fields.terminal_status,
                outcome: fields.outcome,
                ok: fields.ok,
                rollback_applied: fields.rollback_applied,
                completion_count_delta: fields.completion_count_delta,
                failed_step_id: fields.failed.then(|| self.identity.step_id.clone()),
                changed_paths: bounded_snippets(fields.changed_paths, MAX_CHANGED_PATHS),
                changed_path_count,
                changed_paths_truncated: changed_path_count > MAX_CHANGED_PATHS,
                verification_status: fields.verification_status,
                verification_failure_count,
                verification_failures: bounded_snippets(
                    fields.verification_failures,
                    MAX_VERIFICATION_FAILURES,
                ),
                verification_failures_truncated: verification_failure_count
                    > MAX_VERIFICATION_FAILURES,
                repair_attempts: fields.repair_attempts,
                failure_summary: eval_events::body_snippet(fields.failure_summary),
            },
        );
    }
}

struct TerminalFields<'a> {
    terminal_status: &'static str,
    outcome: &'static str,
    ok: bool,
    rollback_applied: bool,
    completion_count_delta: usize,
    failed: bool,
    changed_paths: &'a [String],
    verification_status: &'static str,
    verification_failures: &'a [String],
    repair_attempts: usize,
    failure_summary: &'a str,
}

impl Default for TerminalFields<'_> {
    fn default() -> Self {
        Self {
            terminal_status: "failed",
            outcome: "execution_failed",
            ok: false,
            rollback_applied: false,
            completion_count_delta: 0,
            failed: false,
            changed_paths: &[],
            verification_status: "not_run",
            verification_failures: &[],
            repair_attempts: 0,
            failure_summary: "",
        }
    }
}

fn bounded_snippets(values: &[String], limit: usize) -> Vec<String> {
    values
        .iter()
        .take(limit)
        .map(|value| eval_events::body_snippet(value))
        .collect()
}

fn is_interrupted(message: &str) -> bool {
    message.contains("interrupted by user") || message.contains("aborted_by_user")
}

fn emit(path: Option<&Path>, event: &impl Serialize) {
    match serde_json::to_value(event) {
        Ok(value) => eval_events::emit(path, value),
        Err(error) => eprintln!("warning: failed to serialize plan step event: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emitter(path: &Path, total_steps: usize) -> PlanStepEvents {
        PlanStepEvents {
            path: Some(path.to_path_buf()),
            plan_execution_id: uuid::Uuid::now_v7().to_string(),
            session_id: "session".to_string(),
            mode: "ultra-plan".to_string(),
            phase_id: Some("implementation".to_string()),
            total_steps,
        }
    }

    fn step(id: &str) -> PlanStep {
        PlanStep {
            id: id.to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: "secret instruction must not be emitted".to_string(),
            expected_paths: vec!["secret-expected-path".to_string()],
            verify: vec!["secret verify command".to_string()],
        }
    }

    fn read_events(path: &Path) -> Vec<Value> {
        std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    #[test]
    fn paired_events_keep_duplicate_step_ids_distinct() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let events = emitter(&path, 2);
        let retry = emitter(&path, 2);
        assert_eq!(events.session_id, retry.session_id);
        assert_ne!(events.plan_execution_id, retry.plan_execution_id);
        for index in 0..2 {
            let task = events.start(&step("same-id"), index);
            task.finish(&Ok(StepRunOutcome::default()));
        }

        let records = read_events(&path);
        assert_eq!(records.len(), 4);
        for pair in records.as_chunks::<2>().0 {
            assert_eq!(pair[0]["event"], "plan_step_started");
            assert_eq!(pair[1]["event"], "plan_step_completed");
            assert_eq!(pair[0]["schema_version"], "1");
            assert_eq!(pair[0]["plan_step_schema_version"], 1);
            assert_eq!(pair[0]["step_execution_id"], pair[1]["step_execution_id"]);
            assert_eq!(pair[0]["plan_execution_id"], pair[1]["plan_execution_id"]);
            assert_eq!(pair[0]["session_id"], "session");
            assert_eq!(pair[0]["phase_id"], "implementation");
            assert_eq!(pair[1]["completion_count_delta"], 1);
            assert_eq!(pair[1]["failed_step_id"], Value::Null);
            assert_eq!(pair[1]["rollback_applied"], false);
        }
        assert_ne!(
            records[0]["step_execution_id"],
            records[2]["step_execution_id"]
        );
        assert_eq!(records[0]["step_index"], 1);
        assert_eq!(records[2]["step_index"], 2);
    }

    #[test]
    fn terminal_outcomes_distinguish_skip_verify_repair_and_interrupt() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let events = emitter(&path, 4);

        let skipped = events.start(&step("skip"), 0);
        skipped.finish(&Ok(StepRunOutcome {
            stop_reason: Some("StepShortCircuited".to_string()),
            ..StepRunOutcome::default()
        }));
        let verify = events.start(&step("verify"), 1);
        verify.finish(&Err(StepRunError {
            message: "verify failed".to_string(),
            outcome: StepRunOutcome {
                verify_failures: vec!["verify failed".to_string()],
                ..StepRunOutcome::default()
            },
        }));
        let repair = events.start(&step("repair"), 2);
        repair.finish(&Err(StepRunError {
            message: "repair exhausted".to_string(),
            outcome: StepRunOutcome {
                verify_failures: vec!["still failing".to_string()],
                repair_attempts: 4,
                ..StepRunOutcome::default()
            },
        }));
        events.start(&step("interrupt"), 3).interrupted();

        let terminal = read_events(&path)
            .into_iter()
            .filter(|event| {
                matches!(
                    event["event"].as_str(),
                    Some("plan_step_completed" | "plan_step_failed")
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(terminal[0]["event"], "plan_step_completed");
        assert_eq!(terminal[0]["outcome"], "short_circuited");
        assert_eq!(terminal[0]["terminal_status"], "skipped");
        assert_eq!(terminal[1]["outcome"], "verification_failed");
        assert_eq!(terminal[1]["event"], "plan_step_failed");
        assert_eq!(terminal[1]["failed_step_id"], "verify");
        assert_eq!(terminal[2]["outcome"], "bounded_repair_failed");
        assert_eq!(terminal[3]["outcome"], "interrupted");
        assert_eq!(terminal[3]["verification_status"], "not_run");
        assert_eq!(terminal[3]["failed_step_id"], "interrupt");
    }

    #[test]
    fn terminal_payload_is_bounded_redacted_and_omits_step_bodies() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let events = emitter(&path, 1);
        let task = events.start(&step("bounded"), 0);
        let sensitive = "OPENAI_API_KEY=super-secret ".repeat(80);
        task.finish(&Err(StepRunError {
            message: sensitive.clone(),
            outcome: StepRunOutcome {
                changed_paths: (0..40)
                    .map(|index| format!("/Users/private/project/{index}-{sensitive}"))
                    .collect(),
                verify_failures: (0..20).map(|_| sensitive.clone()).collect(),
                ..StepRunOutcome::default()
            },
        }));

        let text = std::fs::read_to_string(&path).unwrap();
        let terminal = read_events(&path).pop().unwrap();
        assert!(!text.contains("super-secret"));
        assert!(!text.contains("secret instruction must not be emitted"));
        assert!(!text.contains("secret verify command"));
        assert_eq!(terminal["changed_paths"].as_array().unwrap().len(), 16);
        assert_eq!(
            terminal["verification_failures"].as_array().unwrap().len(),
            4
        );
        assert_eq!(terminal["changed_paths_truncated"], true);
        assert_eq!(terminal["changed_path_count"], 40);
        assert_eq!(terminal["verification_failures_truncated"], true);
        assert_eq!(terminal["verification_failure_count"], 20);
        assert!(terminal.to_string().len() < 16 * 1024);
    }
}
