//! Repair job state and no-progress classification.
#![allow(dead_code)]

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
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepairAttemptRecord {
    pub(crate) verifier_command: String,
    pub(crate) failure_signature: String,
    pub(crate) target: Option<String>,
    pub(crate) target_role: Option<String>,
    pub(crate) outcome: RepairAttemptOutcomeKind,
}

impl RepairAttemptRecord {
    pub(crate) fn render_line(&self) -> String {
        format!(
            "verifier={} signature={} target={} role={} outcome={}",
            compact(&self.verifier_command),
            compact(&self.failure_signature),
            self.target.as_deref().unwrap_or("none"),
            self.target_role.as_deref().unwrap_or("unknown"),
            self.outcome.as_str()
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepairJobState {
    pub(crate) active_job: String,
    pub(crate) current_target: Option<String>,
    pub(crate) exhausted_targets: Vec<String>,
    pub(crate) exhausted_roles: Vec<String>,
    pub(crate) attempt_ledger: Vec<RepairAttemptRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NoProgressStrategy {
    RetryDeterministicOperatorOnce,
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
            active_job: active_job.into(),
            current_target: None,
            exhausted_targets: Vec::new(),
            exhausted_roles: Vec::new(),
            attempt_ledger: Vec::new(),
        }
    }

    pub(crate) fn with_current_target(mut self, target: impl Into<String>) -> Self {
        self.current_target = Some(target.into());
        self
    }

    pub(crate) fn with_attempt(mut self, attempt: RepairAttemptRecord) -> Self {
        if matches!(
            attempt.outcome,
            RepairAttemptOutcomeKind::NoProgress
                | RepairAttemptOutcomeKind::Duplicate
                | RepairAttemptOutcomeKind::Worsened
        ) {
            if let Some(target) = &attempt.target {
                push_unique(&mut self.exhausted_targets, target.clone());
            }
            if let Some(role) = &attempt.target_role {
                push_unique(&mut self.exhausted_roles, role.clone());
            }
        }
        self.attempt_ledger.push(attempt);
        self
    }

    pub(crate) fn render_lines(&self) -> Vec<String> {
        let mut lines = vec![format!("active_job={}", self.active_job)];
        if let Some(target) = &self.current_target {
            lines.push(format!("current_target={target}"));
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
        for attempt in &self.attempt_ledger {
            lines.push(format!("attempt={}", attempt.render_line()));
        }
        lines
    }
}

pub(crate) fn select_no_progress_strategy(
    state: &RepairJobState,
    current_role: Option<&str>,
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
    if verifier_passed {
        return RepairAttemptOutcomeKind::Passed;
    }
    if changed_files.is_empty() {
        return RepairAttemptOutcomeKind::Noop;
    }
    if before_signature == after_signature {
        RepairAttemptOutcomeKind::NoProgress
    } else {
        RepairAttemptOutcomeKind::ImprovedStillFailing
    }
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

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.contains(&value) {
        values.push(value);
    }
}

fn compact(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_progress_attempt_exhausts_target_and_role() {
        let state = RepairJobState::new("source_implementation_repair")
            .with_current_target("src/lib.rs")
            .with_attempt(RepairAttemptRecord {
                verifier_command: "cargo test".to_string(),
                failure_signature: "E0425".to_string(),
                target: Some("src/lib.rs".to_string()),
                target_role: Some("implementation".to_string()),
                outcome: RepairAttemptOutcomeKind::NoProgress,
            });

        assert_eq!(state.exhausted_targets, vec!["src/lib.rs"]);
        assert!(
            state
                .render_lines()
                .iter()
                .any(|line| line.contains("outcome=no_progress"))
        );
    }

    #[test]
    fn same_role_no_progress_forces_role_switch_when_candidate_exists() {
        let state =
            RepairJobState::new("source_implementation_repair").with_attempt(RepairAttemptRecord {
                verifier_command: "npm run build".to_string(),
                failure_signature: "route-missing".to_string(),
                target: Some("components/Game.tsx".to_string()),
                target_role: Some("implementation".to_string()),
                outcome: RepairAttemptOutcomeKind::NoProgress,
            });
        let candidates = vec!["implementation".to_string(), "entrypoint".to_string()];

        let strategy =
            select_no_progress_strategy(&state, Some("implementation"), &candidates, false, false);

        assert_eq!(strategy, NoProgressStrategy::SwitchTargetRole);
        assert_eq!(strategy.as_str(), "switch_target_role");
    }

    #[test]
    fn no_progress_strategy_prefers_binding_before_scaffold_rebuild() {
        let state =
            RepairJobState::new("source_implementation_repair").with_attempt(RepairAttemptRecord {
                verifier_command: "npm run build".to_string(),
                failure_signature: "missing import".to_string(),
                target: Some("app/page.tsx".to_string()),
                target_role: Some("entrypoint".to_string()),
                outcome: RepairAttemptOutcomeKind::NoProgress,
            });

        let strategy = select_no_progress_strategy(&state, Some("entrypoint"), &[], true, true);

        assert_eq!(strategy, NoProgressStrategy::RouteToEvidenceBinding);
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
}
