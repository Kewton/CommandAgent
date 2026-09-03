//! Bounded, display-safe projection of one terminal execution interval.
//!
//! The projection consumes existing event schemas without changing them. Callers
//! must select one continuation interval before calling [`project`].

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

const PLAN_STEP_SCHEMA_VERSION: u8 = 1;
const MAX_TEXT_CHARS: usize = 512;
const MAX_COMMAND_CHARS: usize = 2_048;
const MAX_OUTPUT_CHARS: usize = 1_024;
const MAX_LIST_ITEMS: usize = 16;
const MAX_OBSERVATIONS: usize = 16;
const MAX_CHANGED_PATHS: usize = 16;
const MAX_VERIFICATION_FAILURES: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureCategory {
    Planning,
    Execution,
    Verification,
    ReleaseGate,
    Infrastructure,
    Interrupted,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionStatus {
    Supported,
    Fallback,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceState {
    Available,
    Missing,
    #[default]
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PartialArtifactState {
    Observed,
    WorkspaceAvailable,
    WorkspaceMissing,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BoundedText {
    pub value: String,
    pub truncated: bool,
}

impl BoundedText {
    fn map(&mut self, transform: &mut impl FnMut(&str) -> String) {
        self.value = transform(&self.value);
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct BoundedTextList {
    pub items: Vec<BoundedText>,
    pub total_count: usize,
    pub truncated: bool,
}

impl BoundedTextList {
    fn map(&mut self, transform: &mut impl FnMut(&str) -> String) {
        for item in &mut self.items {
            item.map(transform);
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FailureExplanation {
    pub projection_status: ProjectionStatus,
    pub category: FailureCategory,
    pub location: FailureLocation,
    pub primary: PrimaryExplanation,
    pub evidence: FailureEvidence,
    pub progress: FailureProgress,
    pub recovery: FailureRecovery,
    pub technical: TechnicalDetails,
}

impl FailureExplanation {
    /// Applies a caller-owned redaction to every projected string without
    /// rebuilding a long failure message.
    pub fn transform_text(&mut self, mut transform: impl FnMut(&str) -> String) {
        self.location.map(&mut transform);
        self.primary.map(&mut transform);
        self.evidence.map(&mut transform);
        self.recovery.map(&mut transform);
        self.technical.machine_codes.map(&mut transform);
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FailureLocation {
    pub interval_index: usize,
    pub plan_execution_id: Option<BoundedText>,
    pub phase: Option<PhaseLocation>,
    pub step: Option<StepLocation>,
}

impl FailureLocation {
    fn map(&mut self, transform: &mut impl FnMut(&str) -> String) {
        map_option(&mut self.plan_execution_id, transform);
        if let Some(phase) = &mut self.phase {
            phase.id.map(transform);
        }
        if let Some(step) = &mut self.step {
            step.execution_id.map(transform);
            step.id.map(transform);
            step.kind.map(transform);
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PhaseLocation {
    pub id: BoundedText,
    pub index: Option<usize>,
    pub total: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StepLocation {
    pub execution_id: BoundedText,
    pub id: BoundedText,
    pub kind: BoundedText,
    pub index: usize,
    pub total: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PrimaryExplanation {
    pub summary: BoundedText,
    pub failure_kind: Option<BoundedText>,
    pub reason_code: Option<BoundedText>,
}

impl PrimaryExplanation {
    fn map(&mut self, transform: &mut impl FnMut(&str) -> String) {
        self.summary.map(transform);
        map_option(&mut self.failure_kind, transform);
        map_option(&mut self.reason_code, transform);
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct FailureEvidence {
    pub command: Option<BoundedText>,
    pub exit_code: Option<i64>,
    pub stdout: Option<BoundedText>,
    pub stderr: Option<BoundedText>,
    pub verification_status: Option<BoundedText>,
    pub acceptance_status: Option<BoundedText>,
    pub release_gate_status: Option<BoundedText>,
    pub observations: Vec<EvidenceObservation>,
    pub observation_count: usize,
    pub observations_truncated: bool,
    pub missing_paths: BoundedTextList,
    pub changed_paths: BoundedTextList,
    pub evidence_paths: BoundedTextList,
}

impl FailureEvidence {
    fn map(&mut self, transform: &mut impl FnMut(&str) -> String) {
        map_option(&mut self.command, transform);
        map_option(&mut self.stdout, transform);
        map_option(&mut self.stderr, transform);
        map_option(&mut self.verification_status, transform);
        map_option(&mut self.acceptance_status, transform);
        map_option(&mut self.release_gate_status, transform);
        for observation in &mut self.observations {
            observation.map(transform);
        }
        self.missing_paths.map(transform);
        self.changed_paths.map(transform);
        self.evidence_paths.map(transform);
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EvidenceObservation {
    pub kind: BoundedText,
    pub status: Option<BoundedText>,
    pub detail: Option<BoundedText>,
    pub path: Option<BoundedText>,
}

impl EvidenceObservation {
    fn map(&mut self, transform: &mut impl FnMut(&str) -> String) {
        self.kind.map(transform);
        map_option(&mut self.status, transform);
        map_option(&mut self.detail, transform);
        map_option(&mut self.path, transform);
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FailureProgress {
    pub completed_phases: usize,
    pub total_phases: usize,
    pub completed_tasks: usize,
    pub total_tasks: usize,
    pub repair_attempts: usize,
    pub workspace_state: WorkspaceState,
    pub partial_artifact_state: PartialArtifactState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FailureRecovery {
    pub next_action_code: Option<BoundedText>,
    pub explanation: BoundedText,
    pub viable_actions: BoundedTextList,
    pub repair_prompt_path: Option<BoundedText>,
    pub recovery_plan_path: Option<BoundedText>,
    pub suggested_command: Option<BoundedText>,
    pub suggested_yaml_command: Option<BoundedText>,
    pub continuation_eligible: bool,
    pub continuation_reason: BoundedText,
}

impl FailureRecovery {
    fn map(&mut self, transform: &mut impl FnMut(&str) -> String) {
        map_option(&mut self.next_action_code, transform);
        self.explanation.map(transform);
        self.viable_actions.map(transform);
        map_option(&mut self.repair_prompt_path, transform);
        map_option(&mut self.recovery_plan_path, transform);
        map_option(&mut self.suggested_command, transform);
        map_option(&mut self.suggested_yaml_command, transform);
        self.continuation_reason.map(transform);
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct TechnicalDetails {
    pub machine_codes: BoundedTextList,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectionContext {
    pub interval_index: usize,
    pub workspace_state: WorkspaceState,
}

impl ProjectionContext {
    pub fn new(interval_index: usize, workspace_state: WorkspaceState) -> Self {
        Self {
            interval_index: interval_index.max(1),
            workspace_state,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
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

#[derive(Clone, Debug, Deserialize)]
struct TerminalStepEvent {
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

#[derive(Clone, Debug)]
struct ValidatedTerminalStep {
    event: TerminalStepEvent,
    start_index: usize,
    terminal_index: usize,
}

/// Projects a failed terminal interval. A successful terminal interval returns
/// `None`, even if the caller's full session contains an earlier failure.
pub fn project(events: &[Value], context: ProjectionContext) -> Option<FailureExplanation> {
    let (terminal_name, terminal) = latest_terminal(events)?;
    if !terminal_failed(terminal) {
        return None;
    }

    let failed_step = latest_failed_step(events);
    let last_step = failed_step
        .clone()
        .or_else(|| latest_validated_terminal_step(events));
    let exact_step_events = failed_step
        .as_ref()
        .map(|step| &events[step.start_index..=step.terminal_index]);
    let verify_failure = failed_step.as_ref().and_then(|step| {
        latest_matching_step_event(exact_step_events?, "step_verify_failure", step)
    });
    let phase_failure = latest_event(events, "ultra_phase_failed");
    let release_failure = latest_release_failure(events);
    let resolved_recovery_events = super::recovery_resolution::resolved_recovery_events(events);
    let recovery_event = latest_recovery_event(
        &resolved_recovery_events,
        failed_step.as_ref(),
        phase_failure,
    );
    let failed_command =
        latest_failed_command(exact_step_events.unwrap_or(events), failed_step.as_ref());
    let category = classify(
        terminal,
        failed_step.as_ref(),
        verify_failure,
        phase_failure,
        release_failure,
        recovery_event,
    );
    let projection_status = if failed_step.is_some()
        || verify_failure.is_some()
        || phase_failure.is_some()
        || release_failure.is_some()
        || recovery_event.is_some()
        || category != FailureCategory::Unknown
    {
        ProjectionStatus::Supported
    } else {
        ProjectionStatus::Fallback
    };

    let plan_execution_id = last_step
        .as_ref()
        .and_then(|step| bounded(&step.event.identity.plan_execution_id, MAX_TEXT_CHARS));
    let phase_id = failed_step
        .as_ref()
        .and_then(|step| step.event.identity.phase_id.as_deref())
        .or_else(|| phase_failure.and_then(|event| text(event, "phase_id")));
    let phase = phase_id.and_then(|phase_id| {
        bounded(phase_id, MAX_TEXT_CHARS).map(|id| PhaseLocation {
            id,
            index: latest_phase_number(events, phase_id, "phase_index"),
            total: latest_phase_number(events, phase_id, "total_phases"),
        })
    });
    let step = failed_step.as_ref().and_then(|step| {
        let identity = &step.event.identity;
        Some(StepLocation {
            execution_id: bounded(&identity.step_execution_id, MAX_TEXT_CHARS)?,
            id: bounded(&identity.step_id, MAX_TEXT_CHARS)?,
            kind: bounded(&identity.step_kind, MAX_TEXT_CHARS)?,
            index: identity.step_index,
            total: identity.total_steps,
        })
    });

    let failure_kind = first_text(
        [
            text(terminal, "failure_kind"),
            recovery_event.and_then(|event| text(event, "failure_kind")),
            recovery_event.and_then(|event| text(event, "recovery_handoff_kind")),
        ]
        .into_iter(),
        MAX_TEXT_CHARS,
    );
    let reason_code = first_text(
        [
            failed_step.as_ref().map(|step| step.event.outcome.as_str()),
            release_failure.map(|_| "release_gate_failed"),
            text(terminal, "primary_reason"),
            text(terminal, "stop_reason").filter(|value| is_machine_code(value)),
        ]
        .into_iter(),
        MAX_TEXT_CHARS,
    );
    let primary_summary = primary_summary(
        terminal,
        failed_step.as_ref(),
        verify_failure,
        phase_failure,
        release_failure,
        failed_command,
        projection_status,
    );

    let selected_plan = last_step
        .as_ref()
        .map(|step| step.event.identity.plan_execution_id.as_str());
    let (completed_tasks, total_tasks, changed_paths) = task_progress(events, selected_plan);
    let (completed_phases, total_phases) = phase_progress(events);
    let repair_attempts = failed_step
        .as_ref()
        .map(|step| step.event.repair_attempts)
        .unwrap_or_else(|| matching_repair_count(events, failed_step.as_ref()));
    let partial_artifact_state = match context.workspace_state {
        WorkspaceState::Available if !changed_paths.is_empty() => PartialArtifactState::Observed,
        WorkspaceState::Available => PartialArtifactState::WorkspaceAvailable,
        WorkspaceState::Missing => PartialArtifactState::WorkspaceMissing,
        WorkspaceState::Unknown => PartialArtifactState::Unknown,
    };

    let evidence = project_evidence(
        events,
        failed_step.as_ref(),
        verify_failure,
        release_failure,
        failed_command,
        changed_paths,
    );
    let recovery = project_recovery(
        terminal,
        recovery_event,
        verify_failure,
        category,
        context.workspace_state,
    );
    let technical = project_technical(
        terminal_name,
        failure_kind.as_ref(),
        reason_code.as_ref(),
        &recovery,
        failed_step.is_some(),
        verify_failure.is_some(),
        phase_failure.is_some(),
        recovery_event.is_some(),
    );

    Some(FailureExplanation {
        projection_status,
        category,
        location: FailureLocation {
            interval_index: context.interval_index.max(1),
            plan_execution_id,
            phase,
            step,
        },
        primary: PrimaryExplanation {
            summary: primary_summary,
            failure_kind,
            reason_code,
        },
        evidence,
        progress: FailureProgress {
            completed_phases,
            total_phases,
            completed_tasks,
            total_tasks,
            repair_attempts,
            workspace_state: context.workspace_state,
            partial_artifact_state,
        },
        recovery,
        technical,
    })
}

fn latest_terminal(events: &[Value]) -> Option<(&'static str, &Value)> {
    latest_event(events, "tui_command_stop")
        .map(|event| ("tui_command_stop", event))
        .or_else(|| latest_event(events, "run_stop").map(|event| ("run_stop", event)))
}

fn terminal_failed(event: &Value) -> bool {
    matches!(
        text(event, "status"),
        Some("failed" | "incomplete" | "interrupted" | "aborted")
    ) || event.get("ok").and_then(Value::as_bool) == Some(false)
}

fn latest_failed_step(events: &[Value]) -> Option<ValidatedTerminalStep> {
    events.iter().enumerate().rev().find_map(|(index, value)| {
        (text(value, "event") == Some("plan_step_failed"))
            .then(|| validated_terminal_step(events, index))
            .flatten()
    })
}

fn latest_validated_terminal_step(events: &[Value]) -> Option<ValidatedTerminalStep> {
    events.iter().enumerate().rev().find_map(|(index, value)| {
        matches!(
            text(value, "event"),
            Some("plan_step_completed" | "plan_step_failed")
        )
        .then(|| validated_terminal_step(events, index))
        .flatten()
    })
}

fn validated_terminal_step(events: &[Value], index: usize) -> Option<ValidatedTerminalStep> {
    let terminal = serde_json::from_value::<TerminalStepEvent>(events.get(index)?.clone()).ok()?;
    if terminal.plan_step_schema_version != PLAN_STEP_SCHEMA_VERSION
        || !matches!(
            terminal.event.as_str(),
            "plan_step_completed" | "plan_step_failed"
        )
        || terminal.identity.step_index == 0
        || terminal.identity.step_index > terminal.identity.total_steps
        || !bounded_terminal_contract(&terminal)
    {
        return None;
    }
    let start_index =
        events[..index]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(start_index, value)| {
                let started = serde_json::from_value::<StartedEvent>(value.clone()).ok()?;
                (started.event == "plan_step_started"
                    && started.plan_step_schema_version == PLAN_STEP_SCHEMA_VERSION
                    && started.identity == terminal.identity)
                    .then_some(start_index)
            })?;
    Some(ValidatedTerminalStep {
        event: terminal,
        start_index,
        terminal_index: index,
    })
}

fn bounded_terminal_contract(event: &TerminalStepEvent) -> bool {
    event.changed_paths.len() <= MAX_CHANGED_PATHS
        && event.verification_failures.len() <= MAX_VERIFICATION_FAILURES
        && bounded_count_matches(
            event.changed_path_count,
            event.changed_paths.len(),
            event.changed_paths_truncated,
        )
        && bounded_count_matches(
            event.verification_failure_count,
            event.verification_failures.len(),
            event.verification_failures_truncated,
        )
        && matches!(
            (
                event.event.as_str(),
                event.terminal_status.as_str(),
                event.ok,
                event.completion_count_delta,
            ),
            ("plan_step_completed", "completed" | "skipped", true, 1)
                | ("plan_step_failed", "failed" | "interrupted", false, 0)
        )
}

fn bounded_count_matches(total: usize, shown: usize, truncated: bool) -> bool {
    if truncated {
        total > shown
    } else {
        total == shown
    }
}

fn latest_matching_step_event<'a>(
    events: &'a [Value],
    name: &str,
    step: &ValidatedTerminalStep,
) -> Option<&'a Value> {
    events.iter().rev().find(|event| {
        text(event, "event") == Some(name)
            && text(event, "step_id") == Some(step.event.identity.step_id.as_str())
    })
}

fn latest_recovery_event<'a>(
    events: &[&'a Value],
    step: Option<&ValidatedTerminalStep>,
    phase_failure: Option<&Value>,
) -> Option<&'a Value> {
    let step_id = step.map(|step| step.event.identity.step_id.as_str());
    let phase_id = step
        .and_then(|step| step.event.identity.phase_id.as_deref())
        .or_else(|| phase_failure.and_then(|event| text(event, "phase_id")));
    events
        .iter()
        .rev()
        .filter(|event| text(event, "event") == Some("recovery_prompt_saved"))
        .copied()
        .find(
            |event| match (text(event, "step_id"), text(event, "phase_id")) {
                (Some(event_step), _) => step_id == Some(event_step),
                (None, Some(event_phase)) => phase_id == Some(event_phase),
                (None, None) => step_id.is_none() && phase_id.is_none(),
            },
        )
}

fn latest_release_failure(events: &[Value]) -> Option<&Value> {
    events.iter().rev().find(|event| {
        text(event, "release_gate_status").is_some_and(is_failed_status)
            || non_empty_values(event.get("release_gate_reasons"))
            || (text(event, "acceptance_layer") == Some("release_gate")
                && event.get("ok").and_then(Value::as_bool) == Some(false))
    })
}

fn latest_failed_command<'a>(
    events: &'a [Value],
    step: Option<&ValidatedTerminalStep>,
) -> Option<&'a Value> {
    events.iter().rev().find(|event| {
        let step_matches = step.is_none_or(|step| {
            text(event, "step_id").is_none()
                || text(event, "step_id") == Some(step.event.identity.step_id.as_str())
        });
        step_matches
            && first_non_empty(event, &["command", "failed_command"]).is_some()
            && event_failed(event)
    })
}

fn event_failed(event: &Value) -> bool {
    event.get("ok").and_then(Value::as_bool) == Some(false)
        || ["status", "final_status", "verification_status"]
            .iter()
            .filter_map(|key| text(event, key))
            .any(is_failed_status)
}

fn classify(
    terminal: &Value,
    step: Option<&ValidatedTerminalStep>,
    verify_failure: Option<&Value>,
    phase_failure: Option<&Value>,
    release_failure: Option<&Value>,
    recovery: Option<&Value>,
) -> FailureCategory {
    if matches!(text(terminal, "status"), Some("interrupted" | "aborted"))
        || step.is_some_and(|step| {
            step.event.terminal_status == "interrupted" || step.event.outcome == "interrupted"
        })
    {
        return FailureCategory::Interrupted;
    }
    if release_failure.is_some() {
        return FailureCategory::ReleaseGate;
    }
    if let Some(step) = step {
        match step.event.outcome.as_str() {
            "verification_failed" | "bounded_repair_failed" => {
                return FailureCategory::Verification;
            }
            "execution_failed" => return FailureCategory::Execution,
            _ => {}
        }
    }
    if verify_failure.is_some() {
        return FailureCategory::Verification;
    }

    let signals = [
        text(terminal, "failure_kind"),
        text(terminal, "stop_reason"),
        phase_failure.and_then(|event| text(event, "stage")),
        phase_failure.and_then(|event| text(event, "reason")),
        recovery.and_then(|event| text(event, "failure_kind")),
        recovery.and_then(|event| text(event, "recovery_handoff_kind")),
    ];
    if signals.iter().flatten().any(|value| {
        contains_any(
            value,
            &[
                "spawn",
                "preflight",
                "infrastructure",
                "provider_unavailable",
            ],
        )
    }) {
        return FailureCategory::Infrastructure;
    }
    if signals.iter().flatten().any(|value| {
        contains_any(
            value,
            &["planning", "planner", "plan_generation", "phase_scaffold"],
        )
    }) {
        return FailureCategory::Planning;
    }
    if signals
        .iter()
        .flatten()
        .any(|value| contains_any(value, &["verify", "verification", "acceptance", "contract"]))
    {
        return FailureCategory::Verification;
    }
    if phase_failure.is_some() {
        return FailureCategory::Execution;
    }
    FailureCategory::Unknown
}

fn primary_summary(
    terminal: &Value,
    step: Option<&ValidatedTerminalStep>,
    verify_failure: Option<&Value>,
    phase_failure: Option<&Value>,
    release_failure: Option<&Value>,
    command: Option<&Value>,
    projection_status: ProjectionStatus,
) -> BoundedText {
    let value = verify_failure
        .and_then(|event| first_non_empty(event, &["primary_reason", "reason"]))
        .or_else(|| {
            step.map(|step| step.event.failure_summary.as_str())
                .filter(|value| !value.trim().is_empty())
        })
        .or_else(|| phase_failure.and_then(|event| first_non_empty(event, &["reason"])))
        .or_else(|| release_failure.and_then(first_release_reason))
        .or_else(|| {
            command.and_then(|event| {
                first_non_empty(event, &["final_reason", "reason", "output_excerpt"])
            })
        })
        .or_else(|| {
            first_non_empty(terminal, &["primary_reason", "stop_reason", "failure_kind"])
                .map(first_line)
        })
        .filter(|value| !value.trim().is_empty());
    bounded(
        value.unwrap_or(match projection_status {
            ProjectionStatus::Supported => "The recorded execution interval failed.",
            ProjectionStatus::Fallback => {
                "Structured failure details are unavailable for this legacy execution interval."
            }
        }),
        MAX_TEXT_CHARS,
    )
    .expect("fallback explanation is non-empty")
}

fn project_evidence(
    events: &[Value],
    step: Option<&ValidatedTerminalStep>,
    verify_failure: Option<&Value>,
    release_failure: Option<&Value>,
    command_event: Option<&Value>,
    changed_paths: Vec<String>,
) -> FailureEvidence {
    let mut observations = Vec::new();
    if let Some(step) = step {
        for failure in &step.event.verification_failures {
            push_observation(
                &mut observations,
                "verification",
                Some(&step.event.verification_status),
                Some(failure),
                None,
            );
        }
    }
    if let Some(event) = verify_failure
        && let Some(reason) = first_non_empty(event, &["primary_reason", "reason"])
    {
        push_observation(
            &mut observations,
            "step_verify_failure",
            Some("failed"),
            Some(reason),
            None,
        );
    }
    if let Some(event) = command_event
        && let Some(reason) = first_non_empty(event, &["final_reason", "reason", "output_excerpt"])
    {
        push_observation(
            &mut observations,
            text(event, "event").unwrap_or("command"),
            first_non_empty(event, &["final_status", "status", "verification_status"]),
            Some(reason),
            None,
        );
    }
    if let Some(event) = release_failure {
        let status = text(event, "release_gate_status").unwrap_or("failed");
        let reasons = string_values(event.get("release_gate_reasons"));
        if reasons.is_empty() {
            push_observation(&mut observations, "release_gate", Some(status), None, None);
        } else {
            for reason in reasons {
                push_observation(
                    &mut observations,
                    "release_gate",
                    Some(status),
                    Some(&reason),
                    None,
                );
            }
        }
    }
    append_probe_observations(events, &mut observations);
    let observation_count = observations.len();
    observations.truncate(MAX_OBSERVATIONS);

    let mut missing_paths = verify_failure
        .map(|event| string_values(event.get("missing_paths")))
        .unwrap_or_default();
    for event in events {
        missing_paths.extend(string_values(event.get("missing_paths")));
    }
    let mut evidence_paths = vec!["summary.md".to_string(), "events.jsonl".to_string()];
    for event in events {
        let Some(object) = event.as_object() else {
            continue;
        };
        for (key, value) in object {
            if key.ends_with("_evidence_path") || key == "completion_contract_path" {
                evidence_paths.extend(string_values(Some(value)));
            }
        }
    }

    FailureEvidence {
        command: command_event.and_then(|event| {
            first_non_empty(event, &["command", "failed_command"])
                .and_then(|value| bounded(value, MAX_COMMAND_CHARS))
        }),
        exit_code: command_event.and_then(|event| integer(event, &["exit_code", "status_code"])),
        stdout: command_event.and_then(|event| {
            first_non_empty(event, &["stdout", "stdout_excerpt"])
                .and_then(|value| bounded(value, MAX_OUTPUT_CHARS))
        }),
        stderr: command_event.and_then(|event| {
            first_non_empty(event, &["stderr", "stderr_excerpt"])
                .and_then(|value| bounded(value, MAX_OUTPUT_CHARS))
        }),
        verification_status: step
            .and_then(|step| bounded(&step.event.verification_status, MAX_TEXT_CHARS))
            .or_else(|| {
                command_event.and_then(|event| {
                    first_non_empty(event, &["verification_status", "final_status"])
                        .and_then(|value| bounded(value, MAX_TEXT_CHARS))
                })
            }),
        acceptance_status: latest_non_empty(events, "final_acceptance_status")
            .and_then(|value| bounded(value, MAX_TEXT_CHARS)),
        release_gate_status: latest_non_empty(events, "release_gate_status")
            .and_then(|value| bounded(value, MAX_TEXT_CHARS)),
        observations_truncated: observation_count > observations.len(),
        observation_count,
        observations,
        missing_paths: bounded_list(missing_paths, MAX_TEXT_CHARS),
        changed_paths: bounded_list(changed_paths, MAX_TEXT_CHARS),
        evidence_paths: bounded_list(evidence_paths, MAX_TEXT_CHARS),
    }
}

fn append_probe_observations(events: &[Value], observations: &mut Vec<EvidenceObservation>) {
    let mut latest = BTreeMap::<String, (&str, &Value)>::new();
    for event in events.iter().rev() {
        let Some(object) = event.as_object() else {
            continue;
        };
        for (key, status) in object {
            let Some(prefix) = key.strip_suffix("_status") else {
                continue;
            };
            if latest.contains_key(prefix)
                || !(prefix.contains("probe")
                    || matches!(prefix, "browser_readiness" | "interaction_evidence"))
            {
                continue;
            }
            let Some(status) = status.as_str() else {
                continue;
            };
            latest.insert(prefix.to_string(), (status, event));
        }
    }
    for (name, (status, event)) in latest {
        if is_success_status(status) || is_not_applicable(status) {
            continue;
        }
        let reasons = string_values(event.get(format!("{name}_reasons")));
        let reason = reasons
            .first()
            .map(String::as_str)
            .or_else(|| text(event, &format!("{name}_reason")));
        let path = text(event, &format!("{name}_evidence_path"));
        push_observation(observations, &name, Some(status), reason, path);
    }
}

fn push_observation(
    target: &mut Vec<EvidenceObservation>,
    kind: &str,
    status: Option<&str>,
    detail: Option<&str>,
    path: Option<&str>,
) {
    let Some(kind) = bounded(kind, MAX_TEXT_CHARS) else {
        return;
    };
    let observation = EvidenceObservation {
        kind,
        status: status.and_then(|value| bounded(value, MAX_TEXT_CHARS)),
        detail: detail.and_then(|value| bounded(value, MAX_TEXT_CHARS)),
        path: path.and_then(|value| bounded(value, MAX_TEXT_CHARS)),
    };
    if !target.contains(&observation) {
        target.push(observation);
    }
}

fn project_recovery(
    terminal: &Value,
    recovery: Option<&Value>,
    verify_failure: Option<&Value>,
    category: FailureCategory,
    workspace_state: WorkspaceState,
) -> FailureRecovery {
    let next_action = first_text(
        [
            text(terminal, "next_action"),
            text(terminal, "recovery_next_action"),
        ]
        .into_iter(),
        MAX_TEXT_CHARS,
    );
    let repair_prompt_path = first_text(
        [
            recovery.and_then(|event| text(event, "recovery_prompt_path")),
            text(terminal, "recovery_prompt_path"),
        ]
        .into_iter(),
        MAX_TEXT_CHARS,
    );
    let recovery_plan_path = first_text(
        [
            recovery.and_then(|event| text(event, "recovery_ultra_plan_path")),
            text(terminal, "recovery_ultra_plan_path"),
        ]
        .into_iter(),
        MAX_TEXT_CHARS,
    );
    let suggested_command = first_text(
        [
            recovery.and_then(|event| text(event, "suggested_recovery_command")),
            text(terminal, "suggested_recovery_command"),
        ]
        .into_iter(),
        MAX_COMMAND_CHARS,
    );
    let suggested_yaml_command = first_text(
        [
            recovery.and_then(|event| text(event, "suggested_recovery_yaml_command")),
            text(terminal, "suggested_recovery_yaml_command"),
        ]
        .into_iter(),
        MAX_COMMAND_CHARS,
    );
    let viable_actions = bounded_list(
        verify_failure
            .map(|event| string_values(event.get("viable_actions")))
            .unwrap_or_default(),
        MAX_TEXT_CHARS,
    );
    let has_handoff = repair_prompt_path.is_some()
        || recovery_plan_path.is_some()
        || suggested_command.is_some()
        || suggested_yaml_command.is_some()
        || !viable_actions.items.is_empty();
    let recovery_artifacts_invalid = recovery.is_some_and(|event| {
        [
            "recovery_prompt_exists",
            "recovery_prompt_parse_ok",
            "recovery_yaml_exists",
            "recovery_yaml_parse_ok",
            "recovery_command_targets_valid",
        ]
        .iter()
        .any(|key| event.get(key).and_then(Value::as_bool) == Some(false))
            || event.get("recovery_yaml_missing").and_then(Value::as_bool) == Some(true)
    });
    let continuation_eligible = workspace_state == WorkspaceState::Available
        && has_handoff
        && !recovery_artifacts_invalid
        && !matches!(
            category,
            FailureCategory::Interrupted | FailureCategory::Infrastructure
        );
    let continuation_reason = match (
        continuation_eligible,
        workspace_state,
        has_handoff,
        category,
    ) {
        (true, _, _, _) => "structured_recovery_available",
        (_, WorkspaceState::Missing, _, _) => "workspace_missing",
        (_, WorkspaceState::Unknown, _, _) => "workspace_state_unknown",
        (_, _, _, _) if recovery_artifacts_invalid => "recovery_artifacts_invalid",
        (_, _, _, FailureCategory::Interrupted) => "interrupted_run_requires_review",
        (_, _, _, FailureCategory::Infrastructure) => "infrastructure_recovery_not_continuable",
        (_, _, false, _) => "no_structured_recovery",
        _ => "continuation_not_eligible",
    };
    let explanation = if repair_prompt_path.is_some() || recovery_plan_path.is_some() {
        "Review the saved repair prompt and Recovery Plan, then use the confirmed continuation flow."
    } else if !viable_actions.items.is_empty() {
        "Review the viable repair actions and apply one through the confirmed continuation flow."
    } else if category == FailureCategory::ReleaseGate {
        "Repair the recorded release-gate evidence before running verification again."
    } else {
        "Inspect summary.md and events.jsonl before deciding whether to continue or rerun."
    };

    FailureRecovery {
        next_action_code: next_action,
        explanation: bounded(explanation, MAX_TEXT_CHARS)
            .expect("recovery explanation is non-empty"),
        viable_actions,
        repair_prompt_path,
        recovery_plan_path,
        suggested_command,
        suggested_yaml_command,
        continuation_eligible,
        continuation_reason: bounded(continuation_reason, MAX_TEXT_CHARS)
            .expect("continuation reason is non-empty"),
    }
}

#[allow(clippy::too_many_arguments)]
fn project_technical(
    terminal_name: &str,
    failure_kind: Option<&BoundedText>,
    reason_code: Option<&BoundedText>,
    recovery: &FailureRecovery,
    has_step: bool,
    has_verify: bool,
    has_phase: bool,
    has_recovery: bool,
) -> TechnicalDetails {
    let mut values = vec![terminal_name.to_string()];
    if has_step {
        values.push("plan_step_failed".to_string());
    }
    if has_verify {
        values.push("step_verify_failure".to_string());
    }
    if has_phase {
        values.push("ultra_phase_failed".to_string());
    }
    if has_recovery {
        values.push("recovery_prompt_saved".to_string());
    }
    values.extend(
        failure_kind
            .into_iter()
            .chain(reason_code)
            .map(|value| value.value.clone()),
    );
    if let Some(next_action) = &recovery.next_action_code {
        values.push(next_action.value.clone());
    }
    TechnicalDetails {
        machine_codes: bounded_list(values, MAX_TEXT_CHARS),
    }
}

fn task_progress(events: &[Value], plan_execution_id: Option<&str>) -> (usize, usize, Vec<String>) {
    let Some(plan_execution_id) = plan_execution_id else {
        return (0, 0, Vec::new());
    };
    let mut completed = 0usize;
    let mut total = 0usize;
    let mut changed_paths = Vec::new();
    let mut seen = BTreeSet::new();
    for index in 0..events.len() {
        let Some(step) = validated_terminal_step(events, index) else {
            continue;
        };
        if step.event.identity.plan_execution_id != plan_execution_id
            || !seen.insert(step.event.identity.step_execution_id.clone())
        {
            continue;
        }
        total = total.max(step.event.identity.total_steps);
        if step.event.event == "plan_step_completed" {
            completed += step.event.completion_count_delta;
        }
        changed_paths.extend(step.event.changed_paths);
    }
    (completed, total, changed_paths)
}

fn phase_progress(events: &[Value]) -> (usize, usize) {
    let mut completed = BTreeSet::new();
    let mut total = 0usize;
    for event in events {
        total = total.max(
            event
                .get("total_phases")
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .unwrap_or(0),
        );
        if text(event, "event") == Some("ultra_phase_complete")
            && event.get("ok").and_then(Value::as_bool) != Some(false)
            && let Some(id) = text(event, "phase_id")
        {
            let index = event
                .get("phase_index")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            completed.insert((index, id.to_string()));
        }
    }
    (completed.len(), total)
}

fn matching_repair_count(events: &[Value], step: Option<&ValidatedTerminalStep>) -> usize {
    events
        .iter()
        .filter(|event| {
            text(event, "event") == Some("step_verify_repair")
                && step.is_none_or(|step| {
                    text(event, "step_id") == Some(step.event.identity.step_id.as_str())
                })
        })
        .count()
}

fn latest_phase_number(events: &[Value], phase_id: &str, key: &str) -> Option<usize> {
    events.iter().rev().find_map(|event| {
        (text(event, "phase_id") == Some(phase_id))
            .then(|| event.get(key).and_then(Value::as_u64))
            .flatten()
            .and_then(|value| usize::try_from(value).ok())
    })
}

fn latest_non_empty<'a>(events: &'a [Value], key: &str) -> Option<&'a str> {
    events.iter().rev().find_map(|event| text(event, key))
}

fn latest_event<'a>(events: &'a [Value], name: &str) -> Option<&'a Value> {
    events
        .iter()
        .rev()
        .find(|event| text(event, "event") == Some(name))
}

fn first_release_reason(event: &Value) -> Option<&str> {
    event
        .get("release_gate_reasons")
        .and_then(Value::as_array)
        .and_then(|values| values.iter().find_map(Value::as_str))
}

fn first_non_empty<'a>(event: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| text(event, key))
}

fn first_text<'a>(
    values: impl Iterator<Item = Option<&'a str>>,
    maximum: usize,
) -> Option<BoundedText> {
    values.flatten().find_map(|value| bounded(value, maximum))
}

fn string_values(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::String(value)) if !value.trim().is_empty() => vec![value.trim().to_string()],
        Some(Value::Array(values)) => values
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

fn non_empty_values(value: Option<&Value>) -> bool {
    !string_values(value).is_empty()
}

fn bounded_list(values: Vec<String>, maximum_chars: usize) -> BoundedTextList {
    let mut unique = Vec::new();
    for value in values {
        let value = value.trim();
        if !value.is_empty() && !unique.iter().any(|current: &String| current == value) {
            unique.push(value.to_string());
        }
    }
    let total_count = unique.len();
    let items = unique
        .into_iter()
        .take(MAX_LIST_ITEMS)
        .filter_map(|value| bounded(&value, maximum_chars))
        .collect::<Vec<_>>();
    BoundedTextList {
        truncated: total_count > items.len(),
        total_count,
        items,
    }
}

fn bounded(value: &str, maximum: usize) -> Option<BoundedText> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let truncated = value.chars().count() > maximum;
    Some(BoundedText {
        value: value.chars().take(maximum).collect(),
        truncated,
    })
}

fn map_option(value: &mut Option<BoundedText>, transform: &mut impl FnMut(&str) -> String) {
    if let Some(value) = value {
        value.map(transform);
    }
}

fn integer(event: &Value, keys: &[&str]) -> Option<i64> {
    keys.iter()
        .find_map(|key| event.get(key).and_then(Value::as_i64))
}

fn text<'a>(event: &'a Value, key: &str) -> Option<&'a str> {
    event
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn first_line(value: &str) -> &str {
    value.lines().next().unwrap_or(value).trim()
}

fn is_machine_code(value: &str) -> bool {
    !value.contains(char::is_whitespace) && value.len() <= MAX_TEXT_CHARS
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    let value = value.to_ascii_lowercase();
    needles.iter().any(|needle| value.contains(needle))
}

fn is_failed_status(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "fail" | "failed" | "error" | "incomplete" | "partial" | "timed_out" | "timeout"
    )
}

fn is_success_status(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "ok" | "pass" | "passed" | "ready" | "completed" | "full" | "full_success"
    )
}

fn is_not_applicable(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "not_applicable" | "not_required" | "not_run"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE_ROOT: &str = "tests/corpus/apps/issue377-gui-failure-explanations/fixtures";

    #[test]
    fn correlates_exact_failed_step_progress_evidence_and_recovery() {
        let events = fixture("failure.jsonl");
        let projection = project(
            &events,
            ProjectionContext::new(3, WorkspaceState::Available),
        )
        .expect("failed interval projection");

        assert_eq!(projection.projection_status, ProjectionStatus::Supported);
        assert_eq!(projection.category, FailureCategory::Verification);
        assert_eq!(projection.location.interval_index, 3);
        assert_eq!(
            projection
                .location
                .plan_execution_id
                .as_ref()
                .map(|value| value.value.as_str()),
            Some("final-plan-execution")
        );
        let phase = projection.location.phase.as_ref().unwrap();
        assert_eq!(phase.id.value, "build");
        assert_eq!(phase.index, Some(2));
        assert_eq!(phase.total, Some(2));
        let step = projection.location.step.as_ref().unwrap();
        assert_eq!(step.execution_id.value, "build-step-execution");
        assert_eq!(step.id.value, "verify-build");
        assert_eq!((step.index, step.total), (2, 2));
        assert_eq!(
            projection.primary.summary.value,
            "implementation_compile_error: src/app/page.tsx:42:7 Property 'score' does not exist"
        );
        assert_eq!(
            projection
                .primary
                .reason_code
                .as_ref()
                .map(|value| value.value.as_str()),
            Some("bounded_repair_failed")
        );
        assert_eq!(
            projection
                .evidence
                .command
                .as_ref()
                .map(|value| value.value.as_str()),
            Some("npm run build")
        );
        assert_eq!(projection.evidence.exit_code, Some(1));
        assert_eq!(
            projection
                .evidence
                .verification_status
                .as_ref()
                .unwrap()
                .value,
            "failed"
        );
        assert_eq!(
            values(&projection.evidence.missing_paths),
            vec!["src/app/game.ts"]
        );
        assert_eq!(
            values(&projection.evidence.changed_paths),
            vec!["src/app/page.tsx"]
        );
        assert_eq!(projection.progress.completed_phases, 1);
        assert_eq!(projection.progress.total_phases, 2);
        assert_eq!(projection.progress.completed_tasks, 1);
        assert_eq!(projection.progress.total_tasks, 2);
        assert_eq!(projection.progress.repair_attempts, 2);
        assert_eq!(
            projection.progress.partial_artifact_state,
            PartialArtifactState::Observed
        );
        assert_eq!(
            values(&projection.recovery.viable_actions),
            vec!["edit_source_artifact", "rerun_verification"]
        );
        assert_eq!(
            projection
                .recovery
                .repair_prompt_path
                .as_ref()
                .map(|value| value.value.as_str()),
            Some(".anvil/repairs/repair-build.md")
        );
        assert_eq!(
            projection
                .recovery
                .recovery_plan_path
                .as_ref()
                .map(|value| value.value.as_str()),
            Some(".anvil/plans/recovery-build.yaml")
        );
        assert!(projection.recovery.suggested_command.is_some());
        assert!(projection.recovery.suggested_yaml_command.is_some());
        assert!(projection.recovery.continuation_eligible);
    }

    #[test]
    fn successful_continuation_does_not_mix_the_prior_failed_interval() {
        let events = fixture("continuation-success.jsonl");
        let continuation = events
            .iter()
            .rposition(|event| text(event, "event") == Some("human_directive_continuation_started"))
            .unwrap();

        assert_eq!(
            project(
                &events[continuation + 1..],
                ProjectionContext::new(2, WorkspaceState::Available)
            ),
            None
        );
        assert_eq!(
            project(
                &events,
                ProjectionContext::new(2, WorkspaceState::Available)
            ),
            None,
            "the latest terminal remains authoritative even for a full-session caller"
        );
    }

    #[test]
    fn recovery_handoff_must_match_the_failed_phase() {
        let mut events = fixture("failure.jsonl");
        let terminal = events.pop().unwrap();
        events.push(serde_json::json!({
            "event": "recovery_prompt_saved",
            "phase_id": "unrelated-prior-phase",
            "recovery_prompt_path": ".anvil/repairs/wrong.md",
            "recovery_ultra_plan_path": ".anvil/plans/wrong.yaml",
            "schema_version": "1"
        }));
        events.push(terminal);

        let projection = project(
            &events,
            ProjectionContext::new(1, WorkspaceState::Available),
        )
        .unwrap();
        assert_eq!(
            projection.recovery.repair_prompt_path.unwrap().value,
            ".anvil/repairs/repair-build.md"
        );
        assert_eq!(
            projection.recovery.recovery_plan_path.unwrap().value,
            ".anvil/plans/recovery-build.yaml"
        );
    }

    #[test]
    fn rejected_treatment_does_not_replace_gui_recovery_handoff() {
        let events = vec![
            serde_json::json!({
                "event": "recovery_prompt_saved",
                "phase_id": "verify-recovery",
                "recovery_prompt_path": ".commandagent/repairs/control.md",
                "recovery_ultra_plan_path": ".commandagent/plans/control.yaml",
                "suggested_recovery_yaml_command": "/run-ultra-plan .commandagent/plans/control.yaml",
            }),
            serde_json::json!({
                "event": "recovery_plan_auto_run_start",
                "recovery_plan_auto_run_current": 1,
                "recovery_ultra_plan_path": ".commandagent/plans/control.yaml",
                "recovery_treatment_path": ".commandagent/recovery-treatments/attempt-1/workspace",
            }),
            serde_json::json!({
                "event": "recovery_prompt_saved",
                "phase_id": "verify-recovery",
                "recovery_prompt_path": ".commandagent/recovery-treatments/attempt-1/workspace/.commandagent/repairs/treatment.md",
                "recovery_ultra_plan_path": ".commandagent/recovery-treatments/attempt-1/workspace/.commandagent/plans/treatment.yaml",
                "suggested_recovery_yaml_command": "/run-ultra-plan .commandagent/recovery-treatments/attempt-1/workspace/.commandagent/plans/treatment.yaml",
            }),
            serde_json::json!({
                "event": "ultra_phase_failed",
                "phase_id": "verify-recovery",
                "stage": "verify",
                "reason": "treatment verification failed",
            }),
            serde_json::json!({"event": "recovery_control_retained"}),
            serde_json::json!({
                "event": "recovery_promotion_decision",
                "decision": "rejected",
            }),
            serde_json::json!({
                "event": "tui_command_stop",
                "status": "failed",
                "ok": false,
                "stop_reason": "automatic Recovery treatment rejected",
                "recovery_prompt_path": ".commandagent/recovery-treatments/attempt-1/workspace/.commandagent/repairs/treatment.md",
                "recovery_ultra_plan_path": ".commandagent/recovery-treatments/attempt-1/workspace/.commandagent/plans/treatment.yaml",
                "suggested_recovery_yaml_command": "/run-ultra-plan .commandagent/recovery-treatments/attempt-1/workspace/.commandagent/plans/treatment.yaml",
            }),
        ];

        let projection = project(
            &events,
            ProjectionContext::new(1, WorkspaceState::Available),
        )
        .unwrap();

        assert_eq!(
            projection.recovery.repair_prompt_path.unwrap().value,
            ".commandagent/repairs/control.md"
        );
        assert_eq!(
            projection.recovery.recovery_plan_path.unwrap().value,
            ".commandagent/plans/control.yaml"
        );
        assert_eq!(
            projection.recovery.suggested_yaml_command.unwrap().value,
            "/run-ultra-plan .commandagent/plans/control.yaml"
        );
    }

    #[test]
    fn explicitly_invalid_recovery_artifacts_are_not_continuation_eligible() {
        let mut events = fixture("failure.jsonl");
        let recovery = events
            .iter_mut()
            .find(|event| text(event, "event") == Some("recovery_prompt_saved"))
            .unwrap();
        recovery["recovery_command_targets_valid"] = Value::Bool(false);

        let projection = project(
            &events,
            ProjectionContext::new(1, WorkspaceState::Available),
        )
        .unwrap();
        assert!(!projection.recovery.continuation_eligible);
        assert_eq!(
            projection.recovery.continuation_reason.value,
            "recovery_artifacts_invalid"
        );
    }

    #[test]
    fn legacy_failure_is_explicit_fallback_with_direct_evidence_links() {
        let projection = project(
            &fixture("legacy.jsonl"),
            ProjectionContext::new(1, WorkspaceState::Unknown),
        )
        .unwrap();

        assert_eq!(projection.projection_status, ProjectionStatus::Fallback);
        assert_eq!(projection.category, FailureCategory::Unknown);
        assert_eq!(projection.location.step, None);
        assert_eq!(
            values(&projection.evidence.evidence_paths),
            vec!["summary.md", "events.jsonl"]
        );
        assert!(!projection.recovery.continuation_eligible);
        assert_eq!(
            projection.recovery.continuation_reason.value,
            "workspace_state_unknown"
        );
    }

    #[test]
    fn distinguishes_all_failure_categories_without_relaxing_terminal_failure() {
        let cases = [
            (
                FailureCategory::Planning,
                serde_json::json!({
                    "event": "tui_command_stop", "status": "failed", "ok": false,
                    "failure_kind": "planner_generation_failed"
                }),
                None,
            ),
            (
                FailureCategory::Execution,
                failed_terminal("phase execution failed"),
                Some(serde_json::json!({
                    "event": "ultra_phase_failed", "phase_id": "build", "stage": "execute",
                    "reason": "tool execution failed", "ok": false
                })),
            ),
            (
                FailureCategory::Verification,
                failed_terminal("completion_contract workspace boundary violation"),
                None,
            ),
            (
                FailureCategory::ReleaseGate,
                serde_json::json!({
                    "event": "tui_command_stop", "status": "failed", "ok": false,
                    "release_gate_status": "failed",
                    "release_gate_reasons": ["browser_route_unavailable"]
                }),
                None,
            ),
            (
                FailureCategory::Infrastructure,
                failed_terminal("delegated CLI spawn preflight failed"),
                None,
            ),
            (
                FailureCategory::Interrupted,
                serde_json::json!({
                    "event": "tui_command_stop", "status": "interrupted", "ok": false,
                    "stop_reason": "interrupted by user"
                }),
                None,
            ),
            (
                FailureCategory::Unknown,
                failed_terminal("legacy unclassified failure"),
                None,
            ),
        ];

        for (expected, terminal, precursor) in cases {
            let mut events = precursor.into_iter().collect::<Vec<_>>();
            events.push(terminal);
            let actual = project(
                &events,
                ProjectionContext::new(1, WorkspaceState::Available),
            )
            .unwrap();
            assert_eq!(actual.category, expected, "{events:?}");
        }
    }

    #[test]
    fn every_text_and_list_is_bounded_and_redactable() {
        let long = format!("/private/secret/{}", "x".repeat(MAX_TEXT_CHARS + 100));
        let missing = (0..32)
            .map(|index| format!("/private/secret/missing-{index}"))
            .collect::<Vec<_>>();
        let events = vec![
            serde_json::json!({ "event": "legacy_detail", "missing_paths": missing }),
            serde_json::json!({
                "event": "tui_command_stop", "status": "failed", "ok": false,
                "failure_kind": "tui_command_failed", "stop_reason": long
            }),
        ];
        let mut projection = project(
            &events,
            ProjectionContext::new(1, WorkspaceState::Available),
        )
        .unwrap();

        assert!(projection.primary.summary.truncated);
        assert_eq!(
            projection.evidence.missing_paths.items.len(),
            MAX_LIST_ITEMS
        );
        assert!(projection.evidence.missing_paths.truncated);
        projection.transform_text(|value| value.replace("/private/secret", "<execution-root>"));
        let serialized = serde_json::to_string(&projection).unwrap();
        assert!(!serialized.contains("/private/secret"));
        assert!(serialized.contains("<execution-root>"));
    }

    fn fixture(name: &str) -> Vec<Value> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(FIXTURE_ROOT)
            .join(name);
        std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    fn failed_terminal(reason: &str) -> Value {
        serde_json::json!({
            "event": "tui_command_stop",
            "status": "failed",
            "ok": false,
            "failure_kind": "tui_command_failed",
            "stop_reason": reason
        })
    }

    fn values(list: &BoundedTextList) -> Vec<&str> {
        list.items
            .iter()
            .map(|value| value.value.as_str())
            .collect()
    }
}
