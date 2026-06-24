use crate::agent::events::{
    ArtifactScope, ArtifactStatus, GuardFeedbackKind, PlanKind, RuntimeEvent, RuntimeObserver,
};
use crate::providers::usage::ModelUsage;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub const EVENT_SCHEMA_VERSION: &str = "1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventSource {
    pub component: String,
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl EventSource {
    pub fn commandagent(
        role: impl Into<String>,
        provider: Option<String>,
        model: Option<String>,
    ) -> Self {
        Self {
            component: "commandagent".to_string(),
            role: role.into(),
            provider,
            model,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VersionedEvent {
    pub schema_version: String,
    pub event_id: String,
    pub sequence: u64,
    pub timestamp: String,
    pub run_id: String,
    pub job_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt_id: Option<String>,
    pub event_type: String,
    pub source: EventSource,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventProtocolContext {
    run_id: String,
    job_id: String,
    sequence: u64,
    source: EventSource,
}

impl EventProtocolContext {
    pub fn new(run_id: impl Into<String>, job_id: impl Into<String>, source: EventSource) -> Self {
        Self {
            run_id: run_id.into(),
            job_id: job_id.into(),
            sequence: 0,
            source,
        }
    }

    pub fn next_runtime_event(&mut self, event: &RuntimeEvent) -> VersionedEvent {
        self.sequence += 1;
        let sequence = self.sequence;
        let (event_type, phase_id, step_id, attempt_id, payload) = runtime_event_payload(event);
        VersionedEvent {
            schema_version: EVENT_SCHEMA_VERSION.to_string(),
            event_id: format!("evt_{}_{}", self.run_id, sequence),
            sequence,
            timestamp: unix_timestamp_ms().to_string(),
            run_id: self.run_id.clone(),
            job_id: self.job_id.clone(),
            phase_id,
            step_id,
            attempt_id,
            event_type,
            source: self.source.clone(),
            payload,
        }
    }
}

pub struct JsonlEventObserver<O> {
    inner: O,
    context: EventProtocolContext,
    file: File,
    last_error: Option<String>,
}

impl<O> JsonlEventObserver<O> {
    pub fn new(
        inner: O,
        path: impl AsRef<Path>,
        context: EventProtocolContext,
    ) -> io::Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self {
            inner,
            context,
            file,
            last_error: None,
        })
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub fn into_inner(self) -> O {
        self.inner
    }
}

impl<O> RuntimeObserver for JsonlEventObserver<O>
where
    O: RuntimeObserver,
{
    fn on_event(&mut self, event: RuntimeEvent) {
        let versioned = self.context.next_runtime_event(&event);
        self.inner.on_event(event);
        match serde_json::to_string(&versioned) {
            Ok(line) => {
                if let Err(err) = writeln!(self.file, "{line}") {
                    self.last_error = Some(err.to_string());
                }
            }
            Err(err) => self.last_error = Some(err.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobManifest {
    pub schema_version: String,
    pub job_id: String,
    pub goal: String,
    pub workspace: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planner_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_model: Option<String>,
    #[serde(default)]
    pub execution_policy: Vec<String>,
    #[serde(default)]
    pub approval_policy: Vec<String>,
    #[serde(default)]
    pub budget_policy: Vec<String>,
    #[serde(default)]
    pub plan_references: Vec<String>,
    #[serde(default)]
    pub artifact_references: Vec<String>,
    #[serde(default)]
    pub diff_references: Vec<String>,
    #[serde(default)]
    pub failure_evidence_references: Vec<String>,
}

impl JobManifest {
    pub fn new(
        job_id: impl Into<String>,
        goal: impl Into<String>,
        workspace: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: EVENT_SCHEMA_VERSION.to_string(),
            job_id: job_id.into(),
            goal: goal.into(),
            workspace: workspace.into(),
            worktree: None,
            profile: None,
            planner_model: None,
            worker_model: None,
            execution_policy: Vec::new(),
            approval_policy: Vec::new(),
            budget_policy: Vec::new(),
            plan_references: Vec::new(),
            artifact_references: Vec::new(),
            diff_references: Vec::new(),
            failure_evidence_references: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Planning,
    WaitingApproval,
    Running,
    Verifying,
    Repairing,
    Blocked,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobState {
    pub schema_version: String,
    pub job_id: String,
    pub status: JobStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_phase: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_step: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_failure: Option<String>,
    #[serde(default)]
    pub token_cost_summary: Value,
    #[serde(default)]
    pub artifact_references: Vec<String>,
    #[serde(default)]
    pub diff_references: Vec<String>,
}

impl JobState {
    pub fn new(job_id: impl Into<String>) -> Self {
        Self {
            schema_version: EVENT_SCHEMA_VERSION.to_string(),
            job_id: job_id.into(),
            status: JobStatus::Queued,
            active_phase: None,
            active_step: None,
            last_failure: None,
            token_cost_summary: json!({}),
            artifact_references: Vec::new(),
            diff_references: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeJobLifecycleStage {
    Planning,
    Running,
    Setup,
    Verifying,
    Repairing,
    Rechecking,
    Completed,
    Failed,
    Blocked,
    ExplicitStop,
    DryRunPlaceholder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeJobCompletionSource {
    RuntimeSuccess,
    ExistingSuccess,
    DryRunPlaceholderSuccess,
    EvidenceOnlySuccess,
    RecheckSuccess,
    RecheckFailure,
    None,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeJobReport {
    pub schema_version: String,
    pub job_id: String,
    pub lifecycle_stage: RuntimeJobLifecycleStage,
    pub active_owner: String,
    pub selected_action: String,
    pub target_admission_status: String,
    pub repair_action_plan_status: String,
    pub attempt_outcome: String,
    pub evidence_runner_status: String,
    pub verifier_rerun_result: String,
    pub explicit_stop_reason: String,
    pub completion_source: RuntimeJobCompletionSource,
}

impl RuntimeJobReport {
    pub fn new(job_id: impl Into<String>, lifecycle_stage: RuntimeJobLifecycleStage) -> Self {
        Self {
            schema_version: EVENT_SCHEMA_VERSION.to_string(),
            job_id: job_id.into(),
            lifecycle_stage,
            active_owner: "unknown".to_string(),
            selected_action: "unknown".to_string(),
            target_admission_status: "unknown".to_string(),
            repair_action_plan_status: "unknown".to_string(),
            attempt_outcome: "unknown".to_string(),
            evidence_runner_status: "unknown".to_string(),
            verifier_rerun_result: "unknown".to_string(),
            explicit_stop_reason: String::new(),
            completion_source: RuntimeJobCompletionSource::Unknown,
        }
    }
}

pub fn project_job_state(events: &[VersionedEvent]) -> Option<JobState> {
    let first = events.first()?;
    let mut state = JobState::new(first.job_id.clone());
    for event in events {
        if event.job_id != state.job_id {
            continue;
        }
        if let Some(phase_id) = event.phase_id.as_ref() {
            state.active_phase = Some(phase_id.clone());
        }
        if let Some(step_id) = event.step_id.as_ref() {
            state.active_step = Some(step_id.clone());
        }
        match event.event_type.as_str() {
            "plan_generation.started" => state.status = JobStatus::Planning,
            "ultra_phase.started" | "step.started" => state.status = JobStatus::Running,
            "verifier.started" => state.status = JobStatus::Verifying,
            "repair_attempt.started" | "recovery_task.started" => {
                state.status = JobStatus::Repairing
            }
            "dependency_setup.finished" if !payload_bool(&event.payload, "ok").unwrap_or(true) => {
                state.status = JobStatus::Blocked;
                state.last_failure = payload_string(&event.payload, "status");
            }
            "profile_verification.failed" => {
                state.status = JobStatus::Blocked;
                state.last_failure = Some("profile_verification_failed".to_string());
            }
            "ultra_phase.failed" | "step.failed" | "session.error" => {
                state.status = JobStatus::Failed;
                state.last_failure = payload_string(&event.payload, "error")
                    .or_else(|| payload_string(&event.payload, "message"));
            }
            "job.cancelled" => state.status = JobStatus::Cancelled,
            "final_answer.accepted" => state.status = JobStatus::Completed,
            _ => {}
        }
    }
    Some(state)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VersionedCommand {
    pub schema_version: String,
    pub command_id: String,
    pub job_id: String,
    pub command_type: String,
    #[serde(default)]
    pub payload: Value,
}

pub fn command_response_event(
    command: &VersionedCommand,
    accepted: bool,
    reason: impl Into<String>,
) -> VersionedEvent {
    let event_type = if accepted {
        "command.accepted"
    } else {
        "command.rejected"
    };
    VersionedEvent {
        schema_version: EVENT_SCHEMA_VERSION.to_string(),
        event_id: format!(
            "evt_{}_{}",
            command.command_id,
            event_type.replace('.', "_")
        ),
        sequence: 0,
        timestamp: unix_timestamp_ms().to_string(),
        run_id: command.job_id.clone(),
        job_id: command.job_id.clone(),
        phase_id: None,
        step_id: None,
        attempt_id: None,
        event_type: event_type.to_string(),
        source: EventSource::commandagent("runtime", None, None),
        payload: json!({
            "command_id": command.command_id,
            "command_type": command.command_type,
            "accepted": accepted,
            "reason": reason.into(),
        }),
    }
}

fn runtime_event_payload(
    event: &RuntimeEvent,
) -> (
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Value,
) {
    match event {
        RuntimeEvent::PlanGenerationStarted {
            kind,
            goal,
            profile,
        } => (
            "plan_generation.started".to_string(),
            None,
            None,
            None,
            json!({"kind": plan_kind(*kind), "goal": goal, "profile": profile}),
        ),
        RuntimeEvent::PlanGenerationFinished { kind, item_count } => (
            "plan_generation.finished".to_string(),
            None,
            None,
            None,
            json!({"kind": plan_kind(*kind), "item_count": item_count}),
        ),
        RuntimeEvent::PlanSaved {
            kind,
            path,
            item_ids,
        } => (
            "plan.saved".to_string(),
            None,
            None,
            None,
            json!({"kind": plan_kind(*kind), "path": path, "item_ids": item_ids}),
        ),
        RuntimeEvent::UltraPhaseStarted {
            index,
            total,
            phase_id,
        } => (
            "ultra_phase.started".to_string(),
            Some(phase_id.clone()),
            None,
            None,
            json!({"index": index, "total": total}),
        ),
        RuntimeEvent::UltraPhaseFinished {
            index,
            total,
            phase_id,
        } => (
            "ultra_phase.finished".to_string(),
            Some(phase_id.clone()),
            None,
            None,
            json!({"index": index, "total": total}),
        ),
        RuntimeEvent::UltraPhaseFailed {
            index,
            total,
            phase_id,
            error,
        } => (
            "ultra_phase.failed".to_string(),
            Some(phase_id.clone()),
            None,
            None,
            json!({"index": index, "total": total, "error": error}),
        ),
        RuntimeEvent::ProfileVerificationFailed { profile, failures } => (
            "profile_verification.failed".to_string(),
            None,
            None,
            None,
            json!({"profile": profile, "failures": failures}),
        ),
        RuntimeEvent::StepStarted {
            index,
            total,
            step_id,
        } => (
            "step.started".to_string(),
            None,
            Some(step_id.clone()),
            None,
            json!({"index": index, "total": total}),
        ),
        RuntimeEvent::StepFinished {
            index,
            total,
            step_id,
        } => (
            "step.finished".to_string(),
            None,
            Some(step_id.clone()),
            None,
            json!({"index": index, "total": total}),
        ),
        RuntimeEvent::StepFailed {
            index,
            total,
            step_id,
            error,
            missing_expected_paths,
        } => (
            "step.failed".to_string(),
            None,
            Some(step_id.clone()),
            None,
            json!({"index": index, "total": total, "error": error, "missing_expected_paths": missing_expected_paths}),
        ),
        RuntimeEvent::VerifierStarted { step_id, command } => (
            "verifier.started".to_string(),
            None,
            Some(step_id.clone()),
            None,
            json!({"command": command}),
        ),
        RuntimeEvent::VerifierFinished {
            step_id,
            command,
            ok,
            failure_count,
        } => (
            "verifier.finished".to_string(),
            None,
            Some(step_id.clone()),
            None,
            json!({"command": command, "ok": ok, "failure_count": failure_count}),
        ),
        RuntimeEvent::DependencySetupStarted { step_id, command } => (
            "dependency_setup.started".to_string(),
            None,
            Some(step_id.clone()),
            None,
            json!({"command": command}),
        ),
        RuntimeEvent::DependencySetupFinished {
            step_id,
            command,
            ok,
            elapsed_ms,
            status,
        } => (
            "dependency_setup.finished".to_string(),
            None,
            Some(step_id.clone()),
            None,
            json!({"command": command, "ok": ok, "elapsed_ms": elapsed_ms, "status": status}),
        ),
        RuntimeEvent::RecoveryTaskStarted {
            step_id,
            attempt,
            active_job,
            dispatch_status,
            execution_envelope,
            target_path,
        } => (
            "recovery_task.started".to_string(),
            None,
            Some(step_id.clone()),
            Some(format!("attempt_{attempt}")),
            json!({
                "attempt": attempt,
                "active_job": active_job,
                "dispatch_status": dispatch_status,
                "execution_envelope": execution_envelope,
                "target_path": target_path,
            }),
        ),
        RuntimeEvent::RepairAttemptStarted {
            step_id,
            attempt,
            max_attempts,
            missing_expected_paths,
        } => (
            "repair_attempt.started".to_string(),
            None,
            Some(step_id.clone()),
            Some(format!("attempt_{attempt}")),
            json!({"attempt": attempt, "max_attempts": max_attempts, "missing_expected_paths": missing_expected_paths}),
        ),
        RuntimeEvent::RepairExhausted {
            step_id,
            repair_path,
            suggested_command,
            missing_expected_paths,
        } => (
            "repair.exhausted".to_string(),
            None,
            Some(step_id.clone()),
            None,
            json!({"repair_path": repair_path, "suggested_command": suggested_command, "missing_expected_paths": missing_expected_paths}),
        ),
        RuntimeEvent::ModelRequestStarted {
            iteration,
            model,
            tool_call_mode,
        } => (
            "model_request.started".to_string(),
            None,
            None,
            Some(format!("iteration_{iteration}")),
            json!({"iteration": iteration, "model": model, "tool_call_mode": tool_call_mode_text(*tool_call_mode)}),
        ),
        RuntimeEvent::ModelResponseReceived {
            iteration,
            tool_call_mode,
            tool_call_count,
            content_chars,
            elapsed_ms,
            usage,
        } => (
            "model_response.received".to_string(),
            None,
            None,
            Some(format!("iteration_{iteration}")),
            json!({"iteration": iteration, "tool_call_mode": tool_call_mode_text(*tool_call_mode), "tool_call_count": tool_call_count, "content_chars": content_chars, "elapsed_ms": elapsed_ms, "usage": usage_payload(usage)}),
        ),
        RuntimeEvent::ParserFeedbackSent {
            iteration,
            previous_tool_call_mode,
            next_tool_call_mode,
            error,
        } => (
            "parser_feedback.sent".to_string(),
            None,
            None,
            Some(format!("iteration_{iteration}")),
            json!({"iteration": iteration, "previous_tool_call_mode": tool_call_mode_text(*previous_tool_call_mode), "next_tool_call_mode": tool_call_mode_text(*next_tool_call_mode), "error": error}),
        ),
        RuntimeEvent::GuardFeedbackSent {
            iteration,
            kind,
            tool_call_mode,
            missing_artifacts,
        } => (
            "guard_feedback.sent".to_string(),
            None,
            None,
            Some(format!("iteration_{iteration}")),
            json!({"iteration": iteration, "kind": guard_feedback_kind(*kind), "tool_call_mode": tool_call_mode_text(*tool_call_mode), "missing_artifacts": missing_artifacts}),
        ),
        RuntimeEvent::ArtifactStatus {
            scope,
            path,
            status,
        } => (
            "artifact.status".to_string(),
            None,
            None,
            None,
            json!({"scope": artifact_scope(*scope), "path": path, "status": artifact_status(*status)}),
        ),
        RuntimeEvent::ToolCallStarted {
            iteration,
            tool_name,
            args_summary,
        } => (
            "tool_call.started".to_string(),
            None,
            None,
            Some(format!("iteration_{iteration}")),
            json!({"iteration": iteration, "tool_name": tool_name, "args_summary": args_summary}),
        ),
        RuntimeEvent::ToolCallFinished {
            iteration,
            tool_name,
            ok,
            output_chars,
            error,
        } => (
            "tool_call.finished".to_string(),
            None,
            None,
            Some(format!("iteration_{iteration}")),
            json!({"iteration": iteration, "tool_name": tool_name, "ok": ok, "output_chars": output_chars, "error": error}),
        ),
        RuntimeEvent::ToolResultTruncated {
            iteration,
            tool_name,
            original_chars,
            returned_chars,
            reason,
        } => (
            "tool_result.truncated".to_string(),
            None,
            None,
            Some(format!("iteration_{iteration}")),
            json!({"iteration": iteration, "tool_name": tool_name, "truncated": true, "original_chars": original_chars, "returned_chars": returned_chars, "reason": reason}),
        ),
        RuntimeEvent::FinalAnswerAccepted {
            iteration,
            answer_chars,
        } => (
            "final_answer.accepted".to_string(),
            None,
            None,
            Some(format!("iteration_{iteration}")),
            json!({"iteration": iteration, "answer_chars": answer_chars}),
        ),
        RuntimeEvent::SessionError { message } => (
            "session.error".to_string(),
            None,
            None,
            None,
            json!({"message": message}),
        ),
    }
}

fn usage_payload(usage: &ModelUsage) -> Value {
    let mut value = serde_json::to_value(usage).unwrap_or_else(|_| json!({}));
    let has_token_metadata = usage.input_tokens.is_some()
        || usage.cached_input_tokens.is_some()
        || usage.output_tokens.is_some()
        || usage.reasoning_tokens.is_some()
        || usage.total_tokens.is_some();
    let available = usage.unavailable_reason.is_none() && has_token_metadata;
    if let Value::Object(map) = &mut value {
        map.insert("available".to_string(), json!(available));
        if !available {
            map.insert(
                "reason".to_string(),
                json!(
                    usage
                        .unavailable_reason
                        .as_deref()
                        .unwrap_or("usage_metadata_missing")
                ),
            );
        }
    }
    value
}

fn plan_kind(kind: PlanKind) -> &'static str {
    match kind {
        PlanKind::StepPlan => "step_plan",
        PlanKind::UltraPlan => "ultra_plan",
        PlanKind::PhaseStepPlan => "phase_step_plan",
    }
}

fn tool_call_mode_text(mode: crate::providers::ToolCallMode) -> &'static str {
    match mode {
        crate::providers::ToolCallMode::Native => "native",
        crate::providers::ToolCallMode::XmlFallback => "xml_fallback",
    }
}

fn guard_feedback_kind(kind: GuardFeedbackKind) -> &'static str {
    match kind {
        GuardFeedbackKind::FutureAction => "future_action",
        GuardFeedbackKind::RequestedArtifacts => "requested_artifacts",
        GuardFeedbackKind::ActionRequired => "action_required",
    }
}

fn artifact_scope(scope: ArtifactScope) -> &'static str {
    match scope {
        ArtifactScope::StepExpectedPath => "step_expected_path",
        ArtifactScope::FinalRequiredArtifact => "final_required_artifact",
    }
}

fn artifact_status(status: ArtifactStatus) -> &'static str {
    match status {
        ArtifactStatus::Ok => "ok",
        ArtifactStatus::Missing => "missing",
        ArtifactStatus::Unchecked => "unchecked",
    }
}

fn payload_bool(payload: &Value, key: &str) -> Option<bool> {
    payload.get(key).and_then(Value::as_bool)
}

fn payload_string(payload: &Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn unix_timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::events::RuntimeEvent;
    use crate::providers::ToolCallMode;

    #[test]
    fn versioned_event_adapter_adds_identity_and_payload() {
        let mut context = EventProtocolContext::new(
            "run1",
            "job1",
            EventSource::commandagent(
                "worker",
                Some("ollama".to_string()),
                Some("qwen".to_string()),
            ),
        );

        let event = context.next_runtime_event(&RuntimeEvent::VerifierFinished {
            step_id: "verify-build".to_string(),
            command: "npm run build".to_string(),
            ok: false,
            failure_count: 1,
        });

        assert_eq!(event.schema_version, EVENT_SCHEMA_VERSION);
        assert_eq!(event.sequence, 1);
        assert_eq!(event.job_id, "job1");
        assert_eq!(event.step_id.as_deref(), Some("verify-build"));
        assert_eq!(event.event_type, "verifier.finished");
        assert_eq!(event.payload["command"], "npm run build");
    }

    #[test]
    fn job_state_projector_replays_core_statuses() {
        let mut context = EventProtocolContext::new(
            "run1",
            "job1",
            EventSource::commandagent("worker", None, None),
        );
        let events = vec![
            context.next_runtime_event(&RuntimeEvent::PlanGenerationStarted {
                kind: PlanKind::StepPlan,
                goal: "goal".to_string(),
                profile: "generic".to_string(),
            }),
            context.next_runtime_event(&RuntimeEvent::StepStarted {
                index: 1,
                total: 1,
                step_id: "write-readme".to_string(),
            }),
            context.next_runtime_event(&RuntimeEvent::FinalAnswerAccepted {
                iteration: 1,
                answer_chars: 4,
            }),
        ];

        let state = project_job_state(&events).unwrap();

        assert_eq!(state.status, JobStatus::Completed);
        assert_eq!(state.active_step.as_deref(), Some("write-readme"));
    }

    #[test]
    fn recovery_task_started_projects_repairing_state() {
        let mut context = EventProtocolContext::new(
            "run1",
            "job1",
            EventSource::commandagent("worker", None, None),
        );

        let event = context.next_runtime_event(&RuntimeEvent::RecoveryTaskStarted {
            step_id: "write-readme".to_string(),
            attempt: 1,
            active_job: "scaffold_materialization".to_string(),
            dispatch_status: "selected".to_string(),
            execution_envelope: Some("file_mutation_repair".to_string()),
            target_path: Some("README.md".to_string()),
        });
        let state = project_job_state(std::slice::from_ref(&event)).unwrap();

        assert_eq!(event.event_type, "recovery_task.started");
        assert_eq!(event.step_id.as_deref(), Some("write-readme"));
        assert_eq!(event.attempt_id.as_deref(), Some("attempt_1"));
        assert_eq!(event.payload["active_job"], "scaffold_materialization");
        assert_eq!(event.payload["dispatch_status"], "selected");
        assert_eq!(event.payload["execution_envelope"], "file_mutation_repair");
        assert_eq!(event.payload["target_path"], "README.md");
        assert_eq!(state.status, JobStatus::Repairing);
        assert_eq!(state.active_step.as_deref(), Some("write-readme"));
    }

    #[test]
    fn runtime_job_report_serializes_lifecycle_projection() {
        let mut report = RuntimeJobReport::new("job1", RuntimeJobLifecycleStage::Rechecking);
        report.active_owner = "setup".to_string();
        report.selected_action = "run_verifier_owned_setup".to_string();
        report.completion_source = RuntimeJobCompletionSource::RecheckSuccess;

        let value = serde_json::to_value(&report).unwrap();

        assert_eq!(value["schema_version"], EVENT_SCHEMA_VERSION);
        assert_eq!(value["job_id"], "job1");
        assert_eq!(value["lifecycle_stage"], "rechecking");
        assert_eq!(value["active_owner"], "setup");
        assert_eq!(value["completion_source"], "recheck_success");
    }

    #[test]
    fn model_response_event_records_usage_unavailable_boundary() {
        let mut context = EventProtocolContext::new(
            "run1",
            "job1",
            EventSource::commandagent("worker", None, None),
        );

        let event = context.next_runtime_event(&RuntimeEvent::ModelResponseReceived {
            iteration: 1,
            tool_call_mode: ToolCallMode::Native,
            tool_call_count: 0,
            content_chars: 12,
            elapsed_ms: 30,
            usage: ModelUsage::unavailable("provider_usage_missing"),
        });

        assert_eq!(event.event_type, "model_response.received");
        assert_eq!(event.payload["usage"]["available"], false);
        assert_eq!(event.payload["usage"]["reason"], "provider_usage_missing");
    }

    #[test]
    fn model_response_event_records_attached_usage() {
        let mut context = EventProtocolContext::new(
            "run1",
            "job1",
            EventSource::commandagent("worker", Some("gemini".to_string()), None),
        );

        let event = context.next_runtime_event(&RuntimeEvent::ModelResponseReceived {
            iteration: 1,
            tool_call_mode: ToolCallMode::Native,
            tool_call_count: 0,
            content_chars: 12,
            elapsed_ms: 30,
            usage: ModelUsage {
                input_tokens: Some(10),
                output_tokens: Some(5),
                total_tokens: Some(15),
                request_count: 1,
                ..ModelUsage::default()
            },
        });

        assert_eq!(event.event_type, "model_response.received");
        assert_eq!(event.payload["usage"]["available"], true);
        assert_eq!(event.payload["usage"]["input_tokens"], 10);
        assert_eq!(event.payload["usage"]["output_tokens"], 5);
        assert_eq!(event.payload["usage"]["total_tokens"], 15);
    }

    #[test]
    fn command_response_event_is_versioned() {
        let command = VersionedCommand {
            schema_version: EVENT_SCHEMA_VERSION.to_string(),
            command_id: "cmd1".to_string(),
            job_id: "job1".to_string(),
            command_type: "job.retry".to_string(),
            payload: json!({}),
        };

        let event = command_response_event(&command, false, "unsupported");

        assert_eq!(event.event_type, "command.rejected");
        assert_eq!(event.payload["reason"], "unsupported");
    }
}
