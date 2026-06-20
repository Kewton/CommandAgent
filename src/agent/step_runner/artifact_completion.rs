#![allow(dead_code)]

use crate::agent::step_runner::artifact_graph::{ArtifactRole, role_for_path};
use crate::agent::step_runner::correction_evidence::ContractEvidence;

pub(crate) const ARTIFACT_COMPLETION_ATTEMPT_LIMIT: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArtifactCompletionMode {
    Create,
    Modify,
}

impl ArtifactCompletionMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Modify => "modify",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArtifactAttemptOutcomeKind {
    WrongTarget,
    NoTool,
    ProseOnly,
    PolicyViolation,
    EvidenceFailed,
    NoProgress,
}

impl ArtifactAttemptOutcomeKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::WrongTarget => "wrong_target",
            Self::NoTool => "no_tool",
            Self::ProseOnly => "prose_only",
            Self::PolicyViolation => "policy_violation",
            Self::EvidenceFailed => "evidence_failed",
            Self::NoProgress => "no_progress",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArtifactCompletionJob {
    pub(crate) target_path: String,
    pub(crate) role: ArtifactRole,
    pub(crate) mode: ArtifactCompletionMode,
    pub(crate) reason: String,
    pub(crate) attempt_limit: usize,
}

impl ArtifactCompletionJob {
    pub(crate) fn from_contract_evidence(evidence: &ContractEvidence) -> Option<Self> {
        let target = evidence
            .repair_target
            .as_deref()
            .or(evidence.target_path.as_deref())
            .or_else(|| evidence.missing_paths.first().map(String::as_str))
            .or_else(|| evidence.required_paths.first().map(String::as_str))?;
        let role = role_for_path(
            target,
            crate::agent::step_runner::artifact_graph::ArtifactLifecycle::Required,
        );
        if matches!(
            role,
            ArtifactRole::GeneratedOutput | ArtifactRole::DependencyCache | ArtifactRole::Unknown
        ) {
            return None;
        }
        let mode = if evidence.missing_paths.iter().any(|path| path == target)
            || evidence
                .reason_code
                .as_deref()
                .is_some_and(|code| code.contains("missing"))
        {
            ArtifactCompletionMode::Create
        } else {
            ArtifactCompletionMode::Modify
        };
        Some(Self {
            target_path: target.to_string(),
            role,
            mode,
            reason: evidence
                .reason_code
                .clone()
                .or_else(|| evidence.violated_contract.clone())
                .unwrap_or_else(|| "required_artifact_completion".to_string()),
            attempt_limit: ARTIFACT_COMPLETION_ATTEMPT_LIMIT,
        })
    }

    pub(crate) fn target_only_policy_summary(&self) -> Vec<String> {
        match self.mode {
            ArtifactCompletionMode::Create => vec![
                format!("read_scope=target_or_parent_context:{}", self.target_path),
                format!("write_scope=create_target_only:{}", self.target_path),
            ],
            ArtifactCompletionMode::Modify => vec![
                format!("read_scope=target_only:{}", self.target_path),
                format!("write_scope=modify_target_only:{}", self.target_path),
            ],
        }
    }

    pub(crate) fn apply_to_evidence(&self, mut evidence: ContractEvidence) -> ContractEvidence {
        let active_job = match self.role {
            ArtifactRole::Test => "test_artifact_completion",
            ArtifactRole::Docs => "documentation_repair",
            ArtifactRole::SetupManifest | ArtifactRole::SetupConfig => "manifest_repair",
            _ => "scaffold_materialization",
        };
        if evidence.active_job.is_none() {
            evidence = evidence.with_active_job(active_job);
        }
        if evidence.repair_action.is_none() {
            evidence = evidence.with_repair_action("create_required_artifact");
        }
        if evidence.repair_kind.is_none() {
            evidence = evidence.with_repair_kind(active_job);
        }
        if evidence.target_path.is_none() {
            evidence = evidence.with_target_path(self.target_path.clone());
        }
        if evidence.repair_target.is_none() {
            evidence = evidence.with_repair_target(self.target_path.clone());
        }
        if evidence.artifact_role.is_none() {
            evidence = evidence.with_artifact_role(self.role.as_str());
        }
        if evidence.required_action.is_none() {
            evidence = evidence.with_required_action(format!(
                "{} the required {} artifact at {}",
                self.mode.as_str(),
                self.role.as_str(),
                self.target_path
            ));
        }
        let mut disallowed_actions = evidence.disallowed_actions.clone();
        push_unique(
            &mut disallowed_actions,
            "do not satisfy the contract by editing unrelated artifacts".to_string(),
        );
        push_unique(
            &mut disallowed_actions,
            "do not answer in prose without creating or modifying the target artifact".to_string(),
        );
        let mut repair_attempt_ledger = evidence.repair_attempt_ledger.clone();
        push_unique(
            &mut repair_attempt_ledger,
            format!(
                "artifact_completion_job: target={}; role={}; mode={}; attempt_limit={}",
                self.target_path,
                self.role.as_str(),
                self.mode.as_str(),
                self.attempt_limit
            ),
        );
        evidence
            .with_disallowed_actions(disallowed_actions)
            .with_repair_attempt_ledger(repair_attempt_ledger)
    }
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_job_for_missing_test_artifact() {
        let evidence = ContractEvidence::new("profile_verification")
            .with_reason_code("missing_required_artifact")
            .with_missing_paths(vec!["tests/test_app.py"]);

        let job = ArtifactCompletionJob::from_contract_evidence(&evidence).unwrap();

        assert_eq!(job.target_path, "tests/test_app.py");
        assert_eq!(job.role, ArtifactRole::Test);
        assert_eq!(job.mode, ArtifactCompletionMode::Create);
        assert_eq!(job.attempt_limit, ARTIFACT_COMPLETION_ATTEMPT_LIMIT);
    }

    #[test]
    fn rejects_generated_output_completion_target() {
        let evidence = ContractEvidence::new("verifier")
            .with_reason_code("missing_required_artifact")
            .with_missing_paths(vec!["target/debug/app"]);

        assert!(ArtifactCompletionJob::from_contract_evidence(&evidence).is_none());
    }

    #[test]
    fn applies_target_only_policy_to_evidence() {
        let evidence = ContractEvidence::new("profile_verification")
            .with_reason_code("missing_required_artifact")
            .with_missing_paths(vec!["tests/test_app.py"]);
        let job = ArtifactCompletionJob::from_contract_evidence(&evidence).unwrap();

        let enriched = job.apply_to_evidence(evidence);

        assert_eq!(
            enriched.active_job.as_deref(),
            Some("test_artifact_completion")
        );
        assert_eq!(enriched.repair_target.as_deref(), Some("tests/test_app.py"));
        assert!(
            enriched
                .repair_attempt_ledger
                .iter()
                .any(|entry| entry.contains("attempt_limit=4"))
        );
    }
}
