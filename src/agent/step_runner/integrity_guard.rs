//! Deterministic patch-integrity guards for repair attempts.
#![allow(dead_code)]

use crate::agent::step_runner::profile_artifact::{
    ArtifactKind, ArtifactProvenance, artifact_kind_label, classify_profile_artifact,
};
use crate::agent::step_runner::profiles::ProfileId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PatchValidationOutcome {
    Accepted,
    Malformed,
    Unsafe,
    Noop,
    Duplicate,
    TestWeakening,
    GeneratedTestUnsupportedAssertion,
    UnsupportedContractAssertion,
    SelfReferentialVerifier,
    OutOfScope,
    ProtectedPath,
    GeneratedOrCacheOutput,
    DependencyArtifactMutation,
    WorsenedVerifier,
}

impl PatchValidationOutcome {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Malformed => "malformed",
            Self::Unsafe => "unsafe",
            Self::Noop => "noop",
            Self::Duplicate => "duplicate",
            Self::TestWeakening => "test_weakening",
            Self::GeneratedTestUnsupportedAssertion => "generated_test_unsupported_assertion",
            Self::UnsupportedContractAssertion => "unsupported_contract_assertion",
            Self::SelfReferentialVerifier => "self_referential_verifier",
            Self::OutOfScope => "out_of_scope",
            Self::ProtectedPath => "protected_path",
            Self::GeneratedOrCacheOutput => "generated_or_cache_output",
            Self::DependencyArtifactMutation => "dependency_artifact_mutation",
            Self::WorsenedVerifier => "worsened_verifier",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PatchProposalSource {
    ModelToolEdit,
    MechanicalAdapter,
    RollbackCandidate,
}

impl PatchProposalSource {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ModelToolEdit => "model_tool_edit",
            Self::MechanicalAdapter => "mechanical_adapter",
            Self::RollbackCandidate => "rollback_candidate",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PatchProposal {
    pub(crate) source: PatchProposalSource,
    pub(crate) active_job: String,
    pub(crate) recovery_owner: Option<String>,
    pub(crate) repair_action: Option<String>,
    pub(crate) selected_failure_cluster: Option<String>,
    pub(crate) target_path: Option<String>,
    pub(crate) target_role: Option<String>,
    pub(crate) source_of_truth: Option<String>,
    pub(crate) touched_paths: Vec<String>,
    pub(crate) before_signature: Option<String>,
    pub(crate) after_signature: Option<String>,
    pub(crate) rerun_authority: Vec<String>,
    pub(crate) rollback_snapshot_id: Option<String>,
}

impl PatchProposal {
    pub(crate) fn new(source: PatchProposalSource, touched_paths: Vec<String>) -> Self {
        Self {
            source,
            active_job: "unknown".to_string(),
            recovery_owner: None,
            repair_action: None,
            selected_failure_cluster: None,
            target_path: None,
            target_role: None,
            source_of_truth: None,
            touched_paths,
            before_signature: None,
            after_signature: None,
            rerun_authority: Vec::new(),
            rollback_snapshot_id: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PatchValidationStatus {
    Accepted,
    Rejected,
}

impl PatchValidationStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PatchValidation {
    pub(crate) outcome: PatchValidationOutcome,
    pub(crate) path: Option<String>,
    pub(crate) reason: String,
}

impl PatchValidation {
    pub(crate) fn new(
        outcome: PatchValidationOutcome,
        path: Option<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            outcome,
            path,
            reason: reason.into(),
        }
    }

    pub(crate) fn render_line(&self) -> String {
        format!(
            "outcome={} path={} reason={}",
            self.outcome.as_str(),
            self.path.as_deref().unwrap_or("none"),
            self.reason.split_whitespace().collect::<Vec<_>>().join(" ")
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PatchValidationReport {
    pub(crate) status: PatchValidationStatus,
    pub(crate) proposal_source: PatchProposalSource,
    pub(crate) source_of_truth: Option<String>,
    pub(crate) active_job: String,
    pub(crate) target_path: Option<String>,
    pub(crate) target_role: Option<String>,
    pub(crate) touched_paths: Vec<String>,
    pub(crate) validations: Vec<PatchValidation>,
    pub(crate) rollback_admission: Option<RollbackAdmission>,
}

impl PatchValidationReport {
    pub(crate) fn from_proposal(
        proposal: &PatchProposal,
        validations: Vec<PatchValidation>,
    ) -> Self {
        let status = if validations.is_empty() {
            PatchValidationStatus::Accepted
        } else {
            PatchValidationStatus::Rejected
        };
        Self {
            status,
            proposal_source: proposal.source,
            source_of_truth: proposal.source_of_truth.clone(),
            active_job: proposal.active_job.clone(),
            target_path: proposal.target_path.clone(),
            target_role: proposal.target_role.clone(),
            touched_paths: proposal.touched_paths.clone(),
            validations,
            rollback_admission: None,
        }
    }

    pub(crate) fn is_rejected(&self) -> bool {
        self.status == PatchValidationStatus::Rejected
    }

    pub(crate) fn outcomes(&self) -> Vec<String> {
        if self.validations.is_empty() {
            vec![PatchValidationOutcome::Accepted.as_str().to_string()]
        } else {
            self.validations
                .iter()
                .map(|validation| validation.outcome.as_str().to_string())
                .collect()
        }
    }

    pub(crate) fn rejected_paths(&self) -> Vec<String> {
        self.validations
            .iter()
            .filter_map(|validation| validation.path.clone())
            .collect()
    }

    pub(crate) fn render_lines(&self) -> Vec<String> {
        let mut lines = vec![format!(
            "status={} source={} active_job={} target={} role={} source_of_truth={} touched_paths={} outcomes={}",
            self.status.as_str(),
            self.proposal_source.as_str(),
            compact(&self.active_job),
            self.target_path.as_deref().unwrap_or("none"),
            self.target_role.as_deref().unwrap_or("unknown"),
            self.source_of_truth.as_deref().unwrap_or("unknown"),
            render_list(&self.touched_paths),
            render_list(&self.outcomes())
        )];
        lines.extend(self.validations.iter().map(PatchValidation::render_line));
        if let Some(rollback) = &self.rollback_admission {
            lines.extend(rollback.render_lines());
        }
        lines
    }

    pub(crate) fn eval_report_fields(&self) -> Vec<String> {
        let rejected_paths = self.rejected_paths();
        let mut fields = vec![
            format!("patch_validation_status={}", self.status.as_str()),
            format!("patch_validation_source={}", self.proposal_source.as_str()),
            format!(
                "patch_validation_outcomes={}",
                render_list(&self.outcomes())
            ),
            format!(
                "patch_validation_rejected_paths={}",
                if rejected_paths.is_empty() {
                    "none".to_string()
                } else {
                    render_list(&rejected_paths)
                }
            ),
        ];
        if let Some(rollback) = &self.rollback_admission {
            fields.extend(rollback.eval_report_fields());
        }
        fields
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RollbackAdmissionStatus {
    NotApplicable,
    Admitted,
    Rejected,
}

impl RollbackAdmissionStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::NotApplicable => "not_applicable",
            Self::Admitted => "admitted",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RollbackAdmission {
    pub(crate) status: RollbackAdmissionStatus,
    pub(crate) reason: String,
    pub(crate) touched_paths: Vec<String>,
    pub(crate) snapshot_available: bool,
    pub(crate) verifier_proved_worsened: bool,
}

impl RollbackAdmission {
    pub(crate) fn new(
        status: RollbackAdmissionStatus,
        reason: impl Into<String>,
        touched_paths: Vec<String>,
        snapshot_available: bool,
        verifier_proved_worsened: bool,
    ) -> Self {
        Self {
            status,
            reason: reason.into(),
            touched_paths,
            snapshot_available,
            verifier_proved_worsened,
        }
    }

    pub(crate) fn render_lines(&self) -> Vec<String> {
        vec![format!(
            "rollback_admission_status={} rollback_reason={} snapshot_available={} verifier_proved_worsened={} touched_paths={}",
            self.status.as_str(),
            compact(&self.reason),
            self.snapshot_available,
            self.verifier_proved_worsened,
            render_list(&self.touched_paths)
        )]
    }

    pub(crate) fn eval_report_fields(&self) -> Vec<String> {
        vec![
            format!("rollback_admission_status={}", self.status.as_str()),
            format!(
                "rollback_reason={}",
                compact(&self.reason).replace(' ', "_")
            ),
        ]
    }
}

pub(crate) fn validate_patch_proposal(
    profile: ProfileId,
    proposal: &PatchProposal,
) -> PatchValidationReport {
    let mut validations = Vec::new();
    for path in &proposal.touched_paths {
        if let Some(validation) = validate_patch_path(profile, path) {
            validations.push(validation);
        }
    }
    PatchValidationReport::from_proposal(proposal, validations)
}

pub(crate) fn validate_patch_path(profile: ProfileId, path: &str) -> Option<PatchValidation> {
    let normalized = normalize_path(path);
    if invalid_workspace_path(&normalized) {
        return Some(PatchValidation::new(
            PatchValidationOutcome::OutOfScope,
            Some(path.to_string()),
            "patch attempted to modify a path outside the workspace scope",
        ));
    }
    let artifact = classify_profile_artifact(
        profile,
        &normalized,
        ArtifactProvenance::WorkspaceObservation,
    );
    match artifact.kind {
        ArtifactKind::DependencyCache => Some(PatchValidation::new(
            PatchValidationOutcome::DependencyArtifactMutation,
            Some(normalized),
            "repair attempted to modify dependency cache output",
        )),
        ArtifactKind::BuildOutput | ArtifactKind::GeneratedDeclaration => {
            Some(PatchValidation::new(
                PatchValidationOutcome::GeneratedOrCacheOutput,
                Some(normalized),
                format!(
                    "repair attempted to modify generated artifact kind {}",
                    artifact_kind_label(artifact.kind)
                ),
            ))
        }
        ArtifactKind::RawInput if artifact.eligibility.protected_input => {
            Some(PatchValidation::new(
                PatchValidationOutcome::ProtectedPath,
                Some(normalized),
                "repair attempted to modify protected raw input",
            ))
        }
        _ => None,
    }
}

pub(crate) fn detect_test_weakening(path: &str, content: &str) -> Option<PatchValidation> {
    let test_like =
        path.contains("test") || path.ends_with("_test.rs") || path.ends_with(".spec.ts");
    if !test_like {
        return None;
    }
    let lowered = content.to_ascii_lowercase();
    if lowered.contains("assert!(true)")
        || lowered.contains("it.skip")
        || lowered.contains("describe.skip")
        || lowered.contains("#[ignore]")
    {
        return Some(PatchValidation::new(
            PatchValidationOutcome::TestWeakening,
            Some(path.to_string()),
            "repair attempted to weaken or skip a test",
        ));
    }
    None
}

pub(crate) fn detect_generated_test_assertion_outside_contract(
    path: &str,
    content: &str,
    task_contract_terms: &[&str],
) -> Option<PatchValidation> {
    if !is_test_like(path) {
        return None;
    }
    let lowered = content.to_ascii_lowercase();
    let generated = lowered.contains("generated")
        || lowered.contains("commandagent")
        || lowered.contains("auto-generated");
    if !generated || task_contract_terms.is_empty() {
        return None;
    }
    let has_contract_term = task_contract_terms
        .iter()
        .any(|term| lowered.contains(&term.to_ascii_lowercase()));
    if has_contract_term {
        return None;
    }
    Some(PatchValidation::new(
        PatchValidationOutcome::GeneratedTestUnsupportedAssertion,
        Some(path.to_string()),
        "generated test assertion is not anchored to the task contract",
    ))
}

pub(crate) fn detect_unsupported_contract_assertion(
    path: &str,
    content: &str,
    supported_terms: &[&str],
) -> Option<PatchValidation> {
    if !is_test_like(path) || supported_terms.is_empty() {
        return None;
    }
    let lowered = content.to_ascii_lowercase();
    let asserts = lowered.contains("assert")
        || lowered.contains("expect(")
        || lowered.contains("should")
        || lowered.contains("pytest");
    if !asserts {
        return None;
    }
    let supported = supported_terms
        .iter()
        .any(|term| lowered.contains(&term.to_ascii_lowercase()));
    if supported {
        return None;
    }
    Some(PatchValidation::new(
        PatchValidationOutcome::UnsupportedContractAssertion,
        Some(path.to_string()),
        "test assertion is outside the supported task contract terms",
    ))
}

pub(crate) fn detect_self_referential_verifier(
    verifier_command: &str,
    verifier_artifact_path: &str,
) -> Option<PatchValidation> {
    let command = verifier_command.trim();
    let artifact = verifier_artifact_path.trim();
    if artifact.is_empty() {
        return None;
    }
    if command.contains(artifact) {
        return Some(PatchValidation::new(
            PatchValidationOutcome::SelfReferentialVerifier,
            Some(artifact.to_string()),
            "verifier command depends on the verifier artifact it is meant to validate",
        ));
    }
    None
}

fn is_test_like(path: &str) -> bool {
    path.contains("test")
        || path.ends_with("_test.rs")
        || path.ends_with(".spec.ts")
        || path.ends_with(".test.ts")
        || path.ends_with(".spec.tsx")
        || path.ends_with(".test.tsx")
}

fn invalid_workspace_path(path: &str) -> bool {
    path.is_empty()
        || path.starts_with('/')
        || path == ".."
        || path.starts_with("../")
        || path.contains("/../")
        || path.ends_with("/..")
}

fn normalize_path(path: &str) -> String {
    path.trim().trim_start_matches("./").replace('\\', "/")
}

fn render_list(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values
            .iter()
            .map(|value| compact(value).replace(' ', "_"))
            .collect::<Vec<_>>()
            .join("|")
    }
}

fn compact(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_test_weakening_markers() {
        let validation = detect_test_weakening("tests/app_test.rs", "#[ignore]\nfn test() {}")
            .expect("test weakening should be detected");

        assert_eq!(validation.outcome, PatchValidationOutcome::TestWeakening);
        assert!(validation.render_line().contains("test_weakening"));
    }

    #[test]
    fn generated_test_cannot_assert_behavior_outside_task_contract() {
        let validation = detect_generated_test_assertion_outside_contract(
            "tests/test_app.py",
            "# generated by CommandAgent\nassert dashboard.total == 10",
            &["login"],
        )
        .expect("generated assertion outside task contract should be detected");

        assert_eq!(
            validation.outcome,
            PatchValidationOutcome::GeneratedTestUnsupportedAssertion
        );
    }

    #[test]
    fn unsupported_contract_assertion_is_filtered() {
        let validation = detect_unsupported_contract_assertion(
            "tests/test_app.py",
            "def test_dashboard():\n    assert dashboard.total == 10",
            &["login"],
        )
        .expect("unsupported assertion should be detected");

        assert_eq!(
            validation.outcome,
            PatchValidationOutcome::UnsupportedContractAssertion
        );
    }

    #[test]
    fn detects_self_referential_verifier_commands() {
        let validation = detect_self_referential_verifier(
            "python scripts/verify.py scripts/verify.py",
            "scripts/verify.py",
        )
        .expect("self-referential verifier should be detected");

        assert_eq!(
            validation.outcome,
            PatchValidationOutcome::SelfReferentialVerifier
        );
    }

    #[test]
    fn validates_generated_and_cache_paths() {
        let proposal = PatchProposal::new(
            PatchProposalSource::ModelToolEdit,
            vec![
                ".next/server/app/page.js".to_string(),
                "node_modules/react/index.js".to_string(),
            ],
        );

        let report = validate_patch_proposal(ProfileId::NextJs, &proposal);

        assert!(report.is_rejected());
        assert_eq!(report.status, PatchValidationStatus::Rejected);
        assert!(
            report
                .outcomes()
                .contains(&"generated_or_cache_output".to_string())
        );
        assert!(
            report
                .outcomes()
                .contains(&"dependency_artifact_mutation".to_string())
        );
        assert!(
            report
                .eval_report_fields()
                .iter()
                .any(|line| line == "patch_validation_status=rejected")
        );
    }

    #[test]
    fn validates_raw_inputs_as_protected_for_data_profiles() {
        let validation = validate_patch_path(ProfileId::DataAnalysis, "data/raw/source.csv")
            .expect("raw input should be protected");

        assert_eq!(validation.outcome, PatchValidationOutcome::ProtectedPath);
    }

    #[test]
    fn accepts_in_scope_source_patch() {
        let proposal = PatchProposal::new(
            PatchProposalSource::ModelToolEdit,
            vec!["src/main.rs".to_string()],
        );

        let report = validate_patch_proposal(ProfileId::Rust, &proposal);

        assert!(!report.is_rejected());
        assert_eq!(report.outcomes(), vec!["accepted".to_string()]);
    }

    #[test]
    fn renders_rollback_admission_fields() {
        let admission = RollbackAdmission::new(
            RollbackAdmissionStatus::Rejected,
            "safe rollback data missing",
            vec!["src/main.rs".to_string()],
            false,
            true,
        );

        assert!(
            admission
                .render_lines()
                .iter()
                .any(|line| line.contains("rollback_admission_status=rejected"))
        );
        assert!(
            admission
                .eval_report_fields()
                .contains(&"rollback_reason=safe_rollback_data_missing".to_string())
        );
    }

    #[test]
    fn patch_report_can_reject_noop_duplicate_and_unsafe_outcomes_with_rollback() {
        let mut proposal = PatchProposal::new(
            PatchProposalSource::MechanicalAdapter,
            vec!["src/lib.rs".to_string()],
        );
        proposal.active_job = "source_implementation_repair".to_string();
        proposal.target_path = Some("src/lib.rs".to_string());
        proposal.target_role = Some("implementation".to_string());
        proposal.source_of_truth = Some("original_verifier_diagnostic".to_string());

        let mut report = PatchValidationReport::from_proposal(
            &proposal,
            vec![
                PatchValidation::new(
                    PatchValidationOutcome::Noop,
                    Some("src/lib.rs".to_string()),
                    "patch did not change the target",
                ),
                PatchValidation::new(
                    PatchValidationOutcome::Duplicate,
                    Some("src/lib.rs".to_string()),
                    "patch repeats a prior failed attempt",
                ),
                PatchValidation::new(
                    PatchValidationOutcome::Unsafe,
                    Some("src/lib.rs".to_string()),
                    "patch violates the action envelope",
                ),
            ],
        );
        report.rollback_admission = Some(RollbackAdmission::new(
            RollbackAdmissionStatus::Admitted,
            "verifier proved worsening and safe rollback data is available",
            vec!["src/lib.rs".to_string()],
            true,
            true,
        ));

        assert!(report.is_rejected());
        assert_eq!(report.status, PatchValidationStatus::Rejected);
        assert_eq!(
            report.outcomes(),
            vec![
                "noop".to_string(),
                "duplicate".to_string(),
                "unsafe".to_string()
            ]
        );
        let fields = report.eval_report_fields().join("\n");
        assert!(fields.contains("patch_validation_source=mechanical_adapter"));
        assert!(fields.contains("patch_validation_outcomes=noop|duplicate|unsafe"));
        assert!(fields.contains("rollback_admission_status=admitted"));
    }
}
