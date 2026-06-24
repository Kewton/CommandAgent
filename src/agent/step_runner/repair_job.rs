//! Repair job state and no-progress classification.
#![allow(dead_code)]

use crate::agent::step_runner::correction_evidence::{ContractEvidence, failure_signature};
use crate::agent::step_runner::integrity_guard::{RollbackAdmission, RollbackAdmissionStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RepairAttemptOutcomeKind {
    Unsafe,
    Malformed,
    Noop,
    Duplicate,
    ImprovedStillFailing,
    NoProgress,
    Worsened,
    Passed,
    ExplicitStop,
}

impl RepairAttemptOutcomeKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Unsafe => "unsafe",
            Self::Malformed => "malformed",
            Self::Noop => "noop",
            Self::Duplicate => "duplicate",
            Self::ImprovedStillFailing => "improved_still_failing",
            Self::NoProgress => "no_progress",
            Self::Worsened => "worsened",
            Self::Passed => "passed",
            Self::ExplicitStop => "explicit_stop",
        }
    }

    fn exhausts_target(self) -> bool {
        matches!(self, Self::NoProgress | Self::Duplicate | Self::Worsened)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepairAttemptRecord {
    pub(crate) attempt_number: usize,
    pub(crate) step_id: String,
    pub(crate) active_job: String,
    pub(crate) recovery_owner: Option<String>,
    pub(crate) repair_action: Option<String>,
    pub(crate) selected_failure_cluster: Option<String>,
    pub(crate) verifier_command: String,
    pub(crate) failure_signature: String,
    pub(crate) before_signature: String,
    pub(crate) after_signature: String,
    pub(crate) target: Option<String>,
    pub(crate) target_role: Option<String>,
    pub(crate) changed_files: Vec<String>,
    pub(crate) outcome: RepairAttemptOutcomeKind,
    pub(crate) outcome_reason: String,
}

impl RepairAttemptRecord {
    pub(crate) fn render_line(&self) -> String {
        let changed = if self.changed_files.is_empty() {
            "none".to_string()
        } else {
            self.changed_files.join("|")
        };
        format!(
            "attempt={} step={} active_job={} owner={} action={} cluster={} verifier={} signature={} before={} after={} target={} role={} changed_files={} outcome={} reason={}",
            self.attempt_number,
            compact(&self.step_id),
            compact(&self.active_job),
            self.recovery_owner.as_deref().unwrap_or("unknown"),
            self.repair_action.as_deref().unwrap_or("unknown"),
            self.selected_failure_cluster
                .as_deref()
                .unwrap_or("unknown"),
            compact(&self.verifier_command),
            compact(&self.failure_signature),
            compact(&self.before_signature),
            compact(&self.after_signature),
            self.target.as_deref().unwrap_or("none"),
            self.target_role.as_deref().unwrap_or("unknown"),
            compact(&changed),
            self.outcome.as_str(),
            compact(&self.outcome_reason)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepairJobState {
    pub(crate) step_id: Option<String>,
    pub(crate) active_job: String,
    pub(crate) recovery_owner: Option<String>,
    pub(crate) repair_action: Option<String>,
    pub(crate) recovery_task_started: bool,
    pub(crate) dispatch_status: Option<String>,
    pub(crate) execution_envelope: Option<String>,
    pub(crate) verifier_command: Option<String>,
    pub(crate) previous_signature: Option<String>,
    pub(crate) current_signature: Option<String>,
    pub(crate) selected_failure_cluster: Option<String>,
    pub(crate) current_target: Option<String>,
    pub(crate) current_target_role: Option<String>,
    pub(crate) exhausted_targets: Vec<String>,
    pub(crate) exhausted_roles: Vec<String>,
    pub(crate) exhausted_clusters: Vec<String>,
    pub(crate) attempt_ledger: Vec<RepairAttemptRecord>,
    pub(crate) no_progress_strategy: Option<NoProgressStrategy>,
    pub(crate) explicit_stop_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NoProgressStrategy {
    RetryDeterministicOperatorOnce,
    SwitchTarget,
    SwitchTargetRole,
    RouteToEvidenceBinding,
    EscalateToContractConflict,
    ScaffoldRebuild,
    ExplicitStop,
}

impl NoProgressStrategy {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::RetryDeterministicOperatorOnce => "retry_deterministic_operator_once",
            Self::SwitchTarget => "switch_target",
            Self::SwitchTargetRole => "switch_target_role",
            Self::RouteToEvidenceBinding => "route_to_evidence_binding",
            Self::EscalateToContractConflict => "escalate_to_contract_conflict",
            Self::ScaffoldRebuild => "scaffold_rebuild",
            Self::ExplicitStop => "explicit_stop",
        }
    }
}

impl RepairJobState {
    pub(crate) fn new(active_job: impl Into<String>) -> Self {
        Self {
            step_id: None,
            active_job: active_job.into(),
            recovery_owner: None,
            repair_action: None,
            recovery_task_started: false,
            dispatch_status: None,
            execution_envelope: None,
            verifier_command: None,
            previous_signature: None,
            current_signature: None,
            selected_failure_cluster: None,
            current_target: None,
            current_target_role: None,
            exhausted_targets: Vec::new(),
            exhausted_roles: Vec::new(),
            exhausted_clusters: Vec::new(),
            attempt_ledger: Vec::new(),
            no_progress_strategy: None,
            explicit_stop_reason: None,
        }
    }

    pub(crate) fn with_step_id(mut self, step_id: impl Into<String>) -> Self {
        self.step_id = Some(step_id.into());
        self
    }

    pub(crate) fn with_active_job(mut self, active_job: impl Into<String>) -> Self {
        self.active_job = active_job.into();
        self
    }

    pub(crate) fn with_recovery_owner(mut self, owner: Option<String>) -> Self {
        self.recovery_owner = owner;
        self
    }

    pub(crate) fn with_repair_action(mut self, action: Option<String>) -> Self {
        self.repair_action = action;
        self
    }

    pub(crate) fn with_recovery_task_started(
        mut self,
        dispatch_status: Option<String>,
        execution_envelope: Option<String>,
    ) -> Self {
        self.recovery_task_started = true;
        self.dispatch_status = dispatch_status;
        self.execution_envelope = execution_envelope;
        self
    }

    pub(crate) fn with_verifier_command(mut self, command: Option<String>) -> Self {
        self.verifier_command = command;
        self
    }

    pub(crate) fn with_signatures(
        mut self,
        previous_signature: Option<String>,
        current_signature: Option<String>,
    ) -> Self {
        self.previous_signature = previous_signature;
        self.current_signature = current_signature;
        self
    }

    pub(crate) fn with_selected_failure_cluster(mut self, cluster: Option<String>) -> Self {
        self.selected_failure_cluster = cluster;
        self
    }

    pub(crate) fn with_current_target(mut self, target: impl Into<String>) -> Self {
        self.current_target = Some(target.into());
        self
    }

    pub(crate) fn with_current_target_opt(mut self, target: Option<String>) -> Self {
        self.current_target = target;
        self
    }

    pub(crate) fn with_current_target_role(mut self, role: Option<String>) -> Self {
        self.current_target_role = role;
        self
    }

    pub(crate) fn with_no_progress_strategy(mut self, strategy: NoProgressStrategy) -> Self {
        self.no_progress_strategy = Some(strategy);
        self
    }

    pub(crate) fn with_explicit_stop_reason(mut self, reason: impl Into<String>) -> Self {
        self.explicit_stop_reason = Some(reason.into());
        self
    }

    pub(crate) fn with_attempt(mut self, attempt: RepairAttemptRecord) -> Self {
        self.previous_signature = Some(attempt.before_signature.clone());
        self.current_signature = Some(attempt.after_signature.clone());
        self.active_job = attempt.active_job.clone();
        self.step_id = Some(attempt.step_id.clone());
        self.recovery_owner = attempt.recovery_owner.clone();
        self.repair_action = attempt.repair_action.clone();
        self.verifier_command = Some(attempt.verifier_command.clone());
        self.selected_failure_cluster = attempt.selected_failure_cluster.clone();
        self.current_target = attempt.target.clone();
        self.current_target_role = attempt.target_role.clone();
        if attempt.outcome.exhausts_target() {
            if let Some(target) = &attempt.target {
                push_unique(&mut self.exhausted_targets, target.clone());
            }
            if let Some(role) = &attempt.target_role {
                push_unique(&mut self.exhausted_roles, role.clone());
            }
            if let Some(cluster) = &attempt.selected_failure_cluster {
                push_unique(&mut self.exhausted_clusters, cluster.clone());
            }
        }
        self.attempt_ledger.push(attempt);
        const MAX_ATTEMPT_LEDGER_ENTRIES: usize = 8;
        if self.attempt_ledger.len() > MAX_ATTEMPT_LEDGER_ENTRIES {
            let drop_count = self.attempt_ledger.len() - MAX_ATTEMPT_LEDGER_ENTRIES;
            self.attempt_ledger.drain(0..drop_count);
        }
        self
    }

    pub(crate) fn render_lines(&self) -> Vec<String> {
        let mut lines = vec![format!("active_job={}", self.active_job)];
        if let Some(step_id) = &self.step_id {
            lines.push(format!("step_id={step_id}"));
        }
        if let Some(owner) = &self.recovery_owner {
            lines.push(format!("recovery_owner={owner}"));
        }
        if let Some(action) = &self.repair_action {
            lines.push(format!("repair_action={action}"));
        }
        lines.push(format!(
            "recovery_task_started={}",
            self.recovery_task_started
        ));
        if let Some(status) = &self.dispatch_status {
            lines.push(format!("dispatch_status={status}"));
        }
        if let Some(envelope) = &self.execution_envelope {
            lines.push(format!("execution_envelope={envelope}"));
        }
        if let Some(command) = &self.verifier_command {
            lines.push(format!("verifier_command={}", compact(command)));
        }
        if let Some(signature) = &self.previous_signature {
            lines.push(format!("previous_signature={}", compact(signature)));
        }
        if let Some(signature) = &self.current_signature {
            lines.push(format!("current_signature={}", compact(signature)));
        }
        if let Some(cluster) = &self.selected_failure_cluster {
            lines.push(format!("selected_failure_cluster={}", compact(cluster)));
        }
        if let Some(target) = &self.current_target {
            lines.push(format!("current_target={target}"));
        }
        if let Some(role) = &self.current_target_role {
            lines.push(format!("current_target_role={role}"));
        }
        if !self.exhausted_targets.is_empty() {
            lines.push(format!(
                "exhausted_targets={}",
                self.exhausted_targets.join("|")
            ));
        }
        if !self.exhausted_roles.is_empty() {
            lines.push(format!(
                "exhausted_roles={}",
                self.exhausted_roles.join("|")
            ));
        }
        if !self.exhausted_clusters.is_empty() {
            lines.push(format!(
                "exhausted_clusters={}",
                self.exhausted_clusters.join("|")
            ));
        }
        if let Some(strategy) = self.no_progress_strategy {
            lines.push(format!("no_progress_strategy={}", strategy.as_str()));
        }
        if let Some(reason) = &self.explicit_stop_reason {
            lines.push(format!("explicit_stop_reason={}", compact(reason)));
        }
        for attempt in &self.attempt_ledger {
            lines.push(format!("attempt={}", attempt.render_line()));
        }
        lines
    }

    pub(crate) fn attempt_ledger_lines(&self) -> Vec<String> {
        self.attempt_ledger
            .iter()
            .map(RepairAttemptRecord::render_line)
            .collect()
    }

    pub(crate) fn attempt_outcome_lines(&self) -> Vec<String> {
        self.attempt_ledger
            .iter()
            .map(|attempt| {
                format!(
                    "attempt={} outcome={} reason={} before_signature={} after_signature={} target={} role={} cluster={}",
                    attempt.attempt_number,
                    attempt.outcome.as_str(),
                    compact(&attempt.outcome_reason),
                    compact(&attempt.before_signature),
                    compact(&attempt.after_signature),
                    attempt.target.as_deref().unwrap_or("none"),
                    attempt.target_role.as_deref().unwrap_or("unknown"),
                    attempt
                        .selected_failure_cluster
                        .as_deref()
                        .unwrap_or("unknown")
                )
            })
            .collect()
    }

    pub(crate) fn eval_report_fields(&self) -> Vec<String> {
        let latest = self.attempt_ledger.last();
        let mut fields = vec![
            format!("recovery_task_started={}", self.recovery_task_started),
            format!("repair_attempt_count={}", self.attempt_ledger.len()),
            format!(
                "repair_state_status={}",
                if !self.attempt_ledger.is_empty() {
                    "attempted"
                } else if self.recovery_task_started {
                    "started"
                } else {
                    "not_attempted"
                }
            ),
            format!(
                "attempt_outcome={}",
                latest
                    .map(|attempt| attempt.outcome.as_str())
                    .unwrap_or("not_attempted")
            ),
            format!(
                "attempt_outcome_reason={}",
                latest
                    .map(|attempt| compact(&attempt.outcome_reason))
                    .unwrap_or_else(|| "none".to_string())
            ),
            format!(
                "before_signature={}",
                latest
                    .map(|attempt| compact(&attempt.before_signature))
                    .unwrap_or_else(|| "none".to_string())
            ),
            format!(
                "after_signature={}",
                latest
                    .map(|attempt| compact(&attempt.after_signature))
                    .unwrap_or_else(|| "none".to_string())
            ),
            format!(
                "exhausted_targets={}",
                if self.exhausted_targets.is_empty() {
                    "none".to_string()
                } else {
                    self.exhausted_targets.join("|")
                }
            ),
            format!(
                "exhausted_roles={}",
                if self.exhausted_roles.is_empty() {
                    "none".to_string()
                } else {
                    self.exhausted_roles.join("|")
                }
            ),
            format!(
                "exhausted_clusters={}",
                if self.exhausted_clusters.is_empty() {
                    "none".to_string()
                } else {
                    self.exhausted_clusters.join("|")
                }
            ),
            format!(
                "no_progress_strategy={}",
                self.no_progress_strategy
                    .map(NoProgressStrategy::as_str)
                    .unwrap_or("none")
            ),
        ];
        if let Some(reason) = &self.explicit_stop_reason {
            fields.push(format!("explicit_stop_reason={}", compact(reason)));
            fields.push(format!(
                "safe_stop_payload={}",
                self.safe_stop_payload_inline(reason)
            ));
        }
        let rollback = latest
            .map(|attempt| {
                rollback_admission_for_attempt(
                    attempt.outcome,
                    attempt.changed_files.clone(),
                    attempt.outcome == RepairAttemptOutcomeKind::Worsened,
                    false,
                )
            })
            .unwrap_or_else(|| {
                rollback_admission_for_attempt(
                    RepairAttemptOutcomeKind::ExplicitStop,
                    Vec::new(),
                    false,
                    false,
                )
            });
        fields.extend(rollback.eval_report_fields());
        fields
    }

    pub(crate) fn safe_stop_payload_lines(&self) -> Vec<String> {
        let Some(reason) = self.explicit_stop_reason.as_deref() else {
            return Vec::new();
        };
        vec![self.safe_stop_payload_inline(reason)]
    }

    fn safe_stop_payload_inline(&self, reason: &str) -> String {
        let outcomes = self
            .attempt_ledger
            .iter()
            .map(|attempt| attempt.outcome.as_str())
            .collect::<Vec<_>>()
            .join("|");
        format!(
            "owner={};job={};action={};target={};role={};cluster={};reason={};attempts={};outcomes={};exhausted_targets={};exhausted_roles={};exhausted_clusters={}",
            eval_value(self.recovery_owner.as_deref().unwrap_or("unknown")),
            eval_value(&self.active_job),
            eval_value(self.repair_action.as_deref().unwrap_or("unknown")),
            eval_value(self.current_target.as_deref().unwrap_or("none")),
            eval_value(self.current_target_role.as_deref().unwrap_or("unknown")),
            eval_value(
                self.selected_failure_cluster
                    .as_deref()
                    .unwrap_or("unknown")
            ),
            eval_value(reason),
            self.attempt_ledger.len(),
            if outcomes.is_empty() {
                "none".to_string()
            } else {
                eval_value(&outcomes)
            },
            eval_list_value(&self.exhausted_targets),
            eval_list_value(&self.exhausted_roles),
            eval_list_value(&self.exhausted_clusters)
        )
    }
}

pub(crate) fn select_no_progress_strategy(
    state: &RepairJobState,
    current_target: Option<&str>,
    current_role: Option<&str>,
    candidate_targets: &[String],
    candidate_roles: &[String],
    evidence_binding_available: bool,
    scaffold_rebuild_admitted: bool,
) -> NoProgressStrategy {
    if state.attempt_ledger.is_empty() {
        return NoProgressStrategy::RetryDeterministicOperatorOnce;
    }
    if evidence_binding_available {
        return NoProgressStrategy::RouteToEvidenceBinding;
    }
    if let Some(current_target) = current_target
        && candidate_targets
            .iter()
            .any(|target| target != current_target && !state.exhausted_targets.contains(target))
    {
        return NoProgressStrategy::SwitchTarget;
    }
    if let Some(current_role) = current_role
        && candidate_roles
            .iter()
            .any(|role| role != current_role && !state.exhausted_roles.contains(role))
    {
        return NoProgressStrategy::SwitchTargetRole;
    }
    if scaffold_rebuild_admitted {
        return NoProgressStrategy::ScaffoldRebuild;
    }
    if state.exhausted_roles.len() > 1 || state.exhausted_targets.len() > 1 {
        return NoProgressStrategy::EscalateToContractConflict;
    }
    NoProgressStrategy::ExplicitStop
}

pub(crate) fn classify_attempt_outcome(
    before_signature: &str,
    after_signature: &str,
    changed_files: &[String],
    verifier_passed: bool,
) -> RepairAttemptOutcomeKind {
    classify_attempt_outcome_with_history(AttemptOutcomeInput {
        before_signature,
        after_signature,
        changed_files,
        verifier_passed,
        target: None,
        selected_failure_cluster: None,
        prior_attempts: &[],
    })
}

pub(crate) struct AttemptOutcomeInput<'a> {
    pub(crate) before_signature: &'a str,
    pub(crate) after_signature: &'a str,
    pub(crate) changed_files: &'a [String],
    pub(crate) verifier_passed: bool,
    pub(crate) target: Option<&'a str>,
    pub(crate) selected_failure_cluster: Option<&'a str>,
    pub(crate) prior_attempts: &'a [RepairAttemptRecord],
}

pub(crate) fn classify_attempt_outcome_with_history(
    input: AttemptOutcomeInput<'_>,
) -> RepairAttemptOutcomeKind {
    if input.verifier_passed {
        return RepairAttemptOutcomeKind::Passed;
    }
    if input.changed_files.is_empty() {
        return RepairAttemptOutcomeKind::Noop;
    }
    if duplicate_attempt(
        input.prior_attempts,
        input.target,
        input.selected_failure_cluster,
        input.after_signature,
    ) {
        return RepairAttemptOutcomeKind::Duplicate;
    }
    if diagnostic_severity_rank(input.after_signature)
        > diagnostic_severity_rank(input.before_signature)
    {
        return RepairAttemptOutcomeKind::Worsened;
    }
    if input.before_signature == input.after_signature {
        return RepairAttemptOutcomeKind::NoProgress;
    }
    RepairAttemptOutcomeKind::ImprovedStillFailing
}

pub(crate) fn attempt_outcome_reason(
    outcome: RepairAttemptOutcomeKind,
    before_signature: &str,
    after_signature: &str,
    changed_files: &[String],
) -> String {
    match outcome {
        RepairAttemptOutcomeKind::Passed => "original verifier/profile/guard passed".to_string(),
        RepairAttemptOutcomeKind::Noop => {
            "repair attempt made no file-changing progress before rerun".to_string()
        }
        RepairAttemptOutcomeKind::Malformed => {
            "repair attempt failed the tool or provider protocol".to_string()
        }
        RepairAttemptOutcomeKind::Unsafe => {
            "repair attempt violated patch validation or integrity guard".to_string()
        }
        RepairAttemptOutcomeKind::NoProgress => format!(
            "same failure signature after changed_files={}",
            if changed_files.is_empty() {
                "none".to_string()
            } else {
                changed_files.join("|")
            }
        ),
        RepairAttemptOutcomeKind::Duplicate => {
            "duplicate repair attempt for the same target and signature".to_string()
        }
        RepairAttemptOutcomeKind::ImprovedStillFailing => format!(
            "failure signature changed from {} to {} but still fails",
            compact(before_signature),
            compact(after_signature)
        ),
        RepairAttemptOutcomeKind::Worsened => {
            "repair attempt produced a stronger or less recoverable contract violation".to_string()
        }
        RepairAttemptOutcomeKind::ExplicitStop => {
            "no admitted bounded repair action remained".to_string()
        }
    }
}

pub(crate) fn repair_signature_from_contract_evidence(evidence: &[ContractEvidence]) -> String {
    if evidence.is_empty() {
        return "no_contract_evidence".to_string();
    }
    let mut parts = Vec::new();
    for item in evidence.iter().take(4) {
        if let Some(signature) = &item.failure_signature {
            parts.push(format!("signature={}", compact(signature)));
        } else {
            parts.push(format!("guard={}", compact(&item.guard)));
            push_part(&mut parts, "contract", item.violated_contract.as_deref());
            push_part(&mut parts, "reason", item.reason_code.as_deref());
            push_part(&mut parts, "diagnostic", item.diagnostic_code.as_deref());
            push_part(&mut parts, "command", item.command.as_deref());
            push_part(
                &mut parts,
                "cluster",
                item.selected_failure_cluster.as_deref(),
            );
            push_part(&mut parts, "target", item.repair_target.as_deref());
            push_part(&mut parts, "target_path", item.target_path.as_deref());
        }
    }
    failure_signature(parts.iter().map(String::as_str))
}

pub(crate) fn rollback_allowed(
    outcome: RepairAttemptOutcomeKind,
    verifier_rerun_proved_worsened: bool,
    safe_rollback_data_available: bool,
) -> bool {
    outcome == RepairAttemptOutcomeKind::Worsened
        && verifier_rerun_proved_worsened
        && safe_rollback_data_available
}

pub(crate) fn rollback_admission_for_attempt(
    outcome: RepairAttemptOutcomeKind,
    touched_paths: Vec<String>,
    verifier_rerun_proved_worsened: bool,
    safe_rollback_data_available: bool,
) -> RollbackAdmission {
    if outcome != RepairAttemptOutcomeKind::Worsened {
        return RollbackAdmission::new(
            RollbackAdmissionStatus::NotApplicable,
            "latest attempt did not worsen the verifier",
            touched_paths,
            safe_rollback_data_available,
            verifier_rerun_proved_worsened,
        );
    }
    if rollback_allowed(
        outcome,
        verifier_rerun_proved_worsened,
        safe_rollback_data_available,
    ) {
        return RollbackAdmission::new(
            RollbackAdmissionStatus::Admitted,
            "verifier proved worsening and safe rollback data is available",
            touched_paths,
            true,
            true,
        );
    }
    let reason = if !verifier_rerun_proved_worsened {
        "verifier did not prove worsened outcome"
    } else {
        "safe rollback data missing"
    };
    RollbackAdmission::new(
        RollbackAdmissionStatus::Rejected,
        reason,
        touched_paths,
        safe_rollback_data_available,
        verifier_rerun_proved_worsened,
    )
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.contains(&value) {
        values.push(value);
    }
}

fn push_part(parts: &mut Vec<String>, key: &str, value: Option<&str>) {
    if let Some(value) = value
        && !value.trim().is_empty()
    {
        parts.push(format!("{key}={}", compact(value)));
    }
}

fn compact(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn duplicate_attempt(
    prior_attempts: &[RepairAttemptRecord],
    target: Option<&str>,
    selected_failure_cluster: Option<&str>,
    after_signature: &str,
) -> bool {
    prior_attempts.iter().any(|attempt| {
        attempt.after_signature == after_signature
            && option_matches(attempt.target.as_deref(), target)
            && option_matches(
                attempt.selected_failure_cluster.as_deref(),
                selected_failure_cluster,
            )
            && matches!(
                attempt.outcome,
                RepairAttemptOutcomeKind::NoProgress
                    | RepairAttemptOutcomeKind::Duplicate
                    | RepairAttemptOutcomeKind::Worsened
                    | RepairAttemptOutcomeKind::ImprovedStillFailing
            )
    })
}

fn option_matches(left: Option<&str>, right: Option<&str>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left == right,
        (None, None) => true,
        _ => false,
    }
}

fn diagnostic_severity_rank(signature: &str) -> u8 {
    let value = signature.to_ascii_lowercase();
    if value.contains("patch_validation")
        || value.contains("unsafe")
        || value.contains("test_weakening")
    {
        return 90;
    }
    if value.contains("contract_conflict") || value.contains("explicit_stop") {
        return 80;
    }
    if value.contains("provider_transport") || value.contains("tool_protocol") {
        return 70;
    }
    if value.contains("dependency_missing")
        || value.contains("setup_failed")
        || value.contains("manifest_invalid")
    {
        return 60;
    }
    if value.contains("profile_verification") || value.contains("profile") {
        return 50;
    }
    if value.contains("verifier") || value.contains("command_failed") {
        return 40;
    }
    if value.contains("missing_required_artifact") || value.contains("missing_deliverable") {
        return 30;
    }
    10
}

fn eval_value(value: &str) -> String {
    let compacted = compact(value);
    if compacted.trim().is_empty() {
        "none".to_string()
    } else {
        compacted
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | '|' | ':') {
                    ch
                } else {
                    '_'
                }
            })
            .collect()
    }
}

fn eval_list_value(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        eval_value(&values.join("|"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_progress_attempt_exhausts_target_and_role() {
        let state = RepairJobState::new("source_implementation_repair")
            .with_current_target("src/lib.rs")
            .with_attempt(attempt(
                "cargo test",
                "E0425",
                Some("src/lib.rs"),
                Some("implementation"),
                RepairAttemptOutcomeKind::NoProgress,
            ));

        assert_eq!(state.exhausted_targets, vec!["src/lib.rs"]);
        assert_eq!(state.exhausted_roles, vec!["implementation"]);
        assert_eq!(state.exhausted_clusters, vec!["verifier_failure"]);
        assert!(
            state
                .render_lines()
                .iter()
                .any(|line| line.contains("outcome=no_progress"))
        );
    }

    #[test]
    fn improved_still_failing_does_not_exhaust_target_role_or_cluster() {
        let state = RepairJobState::new("source_implementation_repair").with_attempt(attempt(
            "cargo test",
            "E0425",
            Some("src/lib.rs"),
            Some("implementation"),
            RepairAttemptOutcomeKind::ImprovedStillFailing,
        ));

        assert!(state.exhausted_targets.is_empty());
        assert!(state.exhausted_roles.is_empty());
        assert!(state.exhausted_clusters.is_empty());
    }

    #[test]
    fn noop_is_distinct_from_no_progress() {
        assert_eq!(
            classify_attempt_outcome("sig-a", "sig-a", &[], false),
            RepairAttemptOutcomeKind::Noop
        );
        assert_eq!(
            classify_attempt_outcome("sig-a", "sig-a", &["src/lib.rs".to_string()], false),
            RepairAttemptOutcomeKind::NoProgress
        );
    }

    #[test]
    fn same_role_no_progress_forces_role_switch_when_candidate_exists() {
        let state = RepairJobState::new("source_implementation_repair").with_attempt(attempt(
            "npm run build",
            "route-missing",
            Some("components/Game.tsx"),
            Some("implementation"),
            RepairAttemptOutcomeKind::NoProgress,
        ));
        let candidates = vec!["implementation".to_string(), "entrypoint".to_string()];

        let strategy = select_no_progress_strategy(
            &state,
            Some("components/Game.tsx"),
            Some("implementation"),
            &[],
            &candidates,
            false,
            false,
        );

        assert_eq!(strategy, NoProgressStrategy::SwitchTargetRole);
        assert_eq!(strategy.as_str(), "switch_target_role");
    }

    #[test]
    fn no_progress_strategy_switches_target_before_role() {
        let state = RepairJobState::new("route_integration_repair").with_attempt(attempt(
            "npm run build",
            "route-missing",
            Some("components/Game.tsx"),
            Some("implementation"),
            RepairAttemptOutcomeKind::NoProgress,
        ));
        let candidate_targets = vec![
            "components/Game.tsx".to_string(),
            "app/page.tsx".to_string(),
        ];
        let candidate_roles = vec!["implementation".to_string(), "entrypoint".to_string()];

        let strategy = select_no_progress_strategy(
            &state,
            Some("components/Game.tsx"),
            Some("implementation"),
            &candidate_targets,
            &candidate_roles,
            false,
            false,
        );

        assert_eq!(strategy, NoProgressStrategy::SwitchTarget);
        assert_eq!(strategy.as_str(), "switch_target");
    }

    #[test]
    fn no_progress_strategy_prefers_binding_before_scaffold_rebuild() {
        let state = RepairJobState::new("source_implementation_repair").with_attempt(attempt(
            "npm run build",
            "missing import",
            Some("app/page.tsx"),
            Some("entrypoint"),
            RepairAttemptOutcomeKind::NoProgress,
        ));

        let strategy = select_no_progress_strategy(
            &state,
            Some("app/page.tsx"),
            Some("entrypoint"),
            &[],
            &[],
            true,
            true,
        );

        assert_eq!(strategy, NoProgressStrategy::RouteToEvidenceBinding);
    }

    #[test]
    fn no_progress_strategy_defers_contract_conflict_after_multiple_exhausted_surfaces() {
        let state = RepairJobState::new("source_implementation_repair")
            .with_attempt(attempt(
                "cargo test",
                "assertion_mismatch",
                Some("src/lib.rs"),
                Some("implementation"),
                RepairAttemptOutcomeKind::NoProgress,
            ))
            .with_attempt(RepairAttemptRecord {
                attempt_number: 2,
                target: Some("tests/lib_test.rs".to_string()),
                target_role: Some("test".to_string()),
                selected_failure_cluster: Some("verifier_failure".to_string()),
                outcome: RepairAttemptOutcomeKind::NoProgress,
                ..attempt(
                    "cargo test",
                    "assertion_mismatch",
                    Some("tests/lib_test.rs"),
                    Some("test"),
                    RepairAttemptOutcomeKind::NoProgress,
                )
            });

        let strategy = select_no_progress_strategy(
            &state,
            Some("tests/lib_test.rs"),
            Some("test"),
            &["src/lib.rs".to_string(), "tests/lib_test.rs".to_string()],
            &["implementation".to_string(), "test".to_string()],
            false,
            false,
        );

        assert_eq!(strategy, NoProgressStrategy::EscalateToContractConflict);
        assert_eq!(strategy.as_str(), "escalate_to_contract_conflict");
    }

    #[test]
    fn duplicate_attempt_repeats_prior_target_cluster_and_after_signature() {
        let prior = attempt(
            "cargo test",
            "signature-a",
            Some("src/lib.rs"),
            Some("implementation"),
            RepairAttemptOutcomeKind::NoProgress,
        );
        let changed = vec!["src/lib.rs".to_string()];

        let outcome = classify_attempt_outcome_with_history(AttemptOutcomeInput {
            before_signature: "signature-b",
            after_signature: "signature-a",
            changed_files: &changed,
            verifier_passed: false,
            target: Some("src/lib.rs"),
            selected_failure_cluster: Some("verifier_failure"),
            prior_attempts: &[prior],
        });

        assert_eq!(outcome, RepairAttemptOutcomeKind::Duplicate);
    }

    #[test]
    fn worsened_attempt_is_detected_from_more_severe_signature() {
        let changed = vec!["src/lib.rs".to_string()];
        let outcome = classify_attempt_outcome_with_history(AttemptOutcomeInput {
            before_signature: "verifier|step|cargo test",
            after_signature: "patch_validation|step|test_weakening",
            changed_files: &changed,
            verifier_passed: false,
            target: Some("src/lib.rs"),
            selected_failure_cluster: Some("verifier_failure"),
            prior_attempts: &[],
        });

        assert_eq!(outcome, RepairAttemptOutcomeKind::Worsened);
    }

    #[test]
    fn safe_stop_payload_is_structured_and_eval_safe() {
        let state = RepairJobState::new("source_implementation_repair")
            .with_attempt(attempt(
                "cargo test",
                "E0425",
                Some("src/lib.rs"),
                Some("implementation"),
                RepairAttemptOutcomeKind::NoProgress,
            ))
            .with_no_progress_strategy(NoProgressStrategy::ExplicitStop)
            .with_explicit_stop_reason("no progress remained");

        let payload = state.safe_stop_payload_lines();

        assert_eq!(payload.len(), 1);
        assert!(payload[0].contains("job=source_implementation_repair"));
        assert!(payload[0].contains("target=src/lib.rs"));
        assert!(payload[0].contains("reason=no_progress_remained"));
        assert!(!payload[0].contains(' '));
    }

    #[test]
    fn rollback_requires_worsened_verifier_and_safe_data() {
        assert!(rollback_allowed(
            RepairAttemptOutcomeKind::Worsened,
            true,
            true
        ));
        assert!(!rollback_allowed(
            RepairAttemptOutcomeKind::ImprovedStillFailing,
            true,
            true
        ));
        assert!(!rollback_allowed(
            RepairAttemptOutcomeKind::Worsened,
            false,
            true
        ));
        assert!(!rollback_allowed(
            RepairAttemptOutcomeKind::Worsened,
            true,
            false
        ));
    }

    #[test]
    fn rollback_admission_reports_rejected_without_safe_data() {
        let admission = rollback_admission_for_attempt(
            RepairAttemptOutcomeKind::Worsened,
            vec!["src/lib.rs".to_string()],
            true,
            false,
        );

        assert_eq!(admission.status, RollbackAdmissionStatus::Rejected);
        assert!(
            admission
                .eval_report_fields()
                .contains(&"rollback_admission_status=rejected".to_string())
        );
        assert!(
            admission
                .eval_report_fields()
                .contains(&"rollback_reason=safe_rollback_data_missing".to_string())
        );
    }

    fn attempt(
        verifier_command: &str,
        failure_signature: &str,
        target: Option<&str>,
        target_role: Option<&str>,
        outcome: RepairAttemptOutcomeKind,
    ) -> RepairAttemptRecord {
        RepairAttemptRecord {
            attempt_number: 1,
            step_id: "step".to_string(),
            active_job: "source_implementation_repair".to_string(),
            recovery_owner: Some("minimal_loop".to_string()),
            repair_action: Some("repair_source_error".to_string()),
            selected_failure_cluster: Some("verifier_failure".to_string()),
            verifier_command: verifier_command.to_string(),
            failure_signature: failure_signature.to_string(),
            before_signature: failure_signature.to_string(),
            after_signature: failure_signature.to_string(),
            target: target.map(str::to_string),
            target_role: target_role.map(str::to_string),
            changed_files: vec!["src/lib.rs".to_string()],
            outcome,
            outcome_reason: "test outcome".to_string(),
        }
    }
}
