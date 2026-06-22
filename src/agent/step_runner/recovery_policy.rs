use crate::agent::step_runner::correction_evidence::ContractEvidence;
use crate::agent::step_runner::profile_artifact::{
    ArtifactProvenance, artifact_kind_label, classify_profile_artifact,
};
use crate::agent::step_runner::profiles::{ProfileId, ProfileVerificationFailure};
use crate::agent::step_runner::recovery_contract;
use crate::agent::step_runner::recovery_orchestration::{
    ActiveJobCandidate, ActiveJobCandidateSeed, RecoveryActionKind, RecoveryJobKind,
};

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActiveJob {
    SetupBootstrap,
    ManifestRepair,
    RouteIntegrationRepair,
    IntegrationArtifactCreation,
    SourceImplementationRepair,
    TestRepair,
    DocsRepair,
    VerifierPolicyRepair,
    ExplicitStop,
}

impl ActiveJob {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::SetupBootstrap => "setup_bootstrap",
            Self::ManifestRepair => "manifest_repair",
            Self::RouteIntegrationRepair => "route_integration_repair",
            Self::IntegrationArtifactCreation => "integration_artifact_creation",
            Self::SourceImplementationRepair => "source_implementation_repair",
            Self::TestRepair => "test_repair",
            Self::DocsRepair => "docs_repair",
            Self::VerifierPolicyRepair => "verifier_policy_repair",
            Self::ExplicitStop => "explicit_stop",
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RepairAction {
    AddManifestDependency,
    RepairBuildScript,
    RepairDevScript,
    RepairTailwindContract,
    RepairTsconfigAlias,
    ConnectArtifactToSelectedRoute,
    CreateMissingIntegrationArtifact,
    RepairSourceError,
    StopWithSetupBlocker,
    StopNoAdmittedTarget,
}

impl RepairAction {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::AddManifestDependency => "add_manifest_dependency",
            Self::RepairBuildScript => "repair_build_script",
            Self::RepairDevScript => "repair_dev_script",
            Self::RepairTailwindContract => "repair_tailwind_contract",
            Self::RepairTsconfigAlias => "repair_tsconfig_alias",
            Self::ConnectArtifactToSelectedRoute => "connect_artifact_to_selected_route",
            Self::CreateMissingIntegrationArtifact => "create_missing_integration_artifact",
            Self::RepairSourceError => "repair_source_error",
            Self::StopWithSetupBlocker => "stop_with_setup_blocker",
            Self::StopNoAdmittedTarget => "stop_no_admitted_target",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepairTargetCandidate {
    pub(crate) path: String,
    pub(crate) artifact_role: String,
    pub(crate) reason: String,
    pub(crate) priority: u8,
}

impl RepairTargetCandidate {
    fn from_path(path: impl Into<String>, reason: impl Into<String>, priority: u8) -> Self {
        let path = path.into();
        let classified =
            classify_profile_artifact(ProfileId::NextJs, &path, ArtifactProvenance::ProfileFact);
        Self {
            path,
            artifact_role: artifact_kind_label(classified.kind).to_string(),
            reason: reason.into(),
            priority,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecoveryPolicyDecision {
    pub(crate) active_job: ActiveJob,
    pub(crate) repair_action: RepairAction,
    pub(crate) repair_kind: String,
    pub(crate) repair_target: Option<String>,
    pub(crate) artifact_role: Option<String>,
    pub(crate) required_action: String,
    pub(crate) disallowed_actions: Vec<String>,
    pub(crate) setup_implication: String,
    pub(crate) rerun_authority: Vec<String>,
    pub(crate) target_candidates: Vec<RepairTargetCandidate>,
}

impl RecoveryPolicyDecision {
    pub(crate) fn active_job_candidate(&self) -> ActiveJobCandidate {
        let job = self.recovery_job_kind();
        let action = self.recovery_action_kind(job);
        ActiveJobCandidate::from_seed(ActiveJobCandidateSeed {
            job,
            action,
            priority: recovery_contract::active_job_priority(job.as_str()),
            reason: "profile_failure_policy".to_string(),
            source_of_truth: "profile_contract".to_string(),
            source_layer: "profile_verification".to_string(),
            target_hint: self.repair_target.clone(),
            artifact_role: self.artifact_role.clone(),
            rerun_authority: self.rerun_authority.clone(),
        })
    }

    pub(crate) fn apply_to_evidence(&self, mut evidence: ContractEvidence) -> ContractEvidence {
        let candidate = self.active_job_candidate();
        evidence = evidence
            .with_active_job(self.active_job.as_str())
            .with_repair_action(self.repair_action.as_str())
            .with_repair_kind(self.repair_kind.clone())
            .with_required_action(self.required_action.clone())
            .with_disallowed_actions(self.disallowed_actions.clone())
            .with_setup_implication(self.setup_implication.clone())
            .with_rerun_authority(self.rerun_authority.clone())
            .with_candidate_jobs(vec![candidate.render_line()]);

        if let Some(target) = self.repair_target.clone() {
            evidence = evidence
                .with_target_path(target.clone())
                .with_repair_target(target);
        }

        if let Some(role) = self.artifact_role.clone() {
            evidence = evidence.with_artifact_role(role);
        } else if let Some(target) = self.repair_target.as_deref() {
            let classified = classify_profile_artifact(
                ProfileId::NextJs,
                target,
                ArtifactProvenance::ProfileFact,
            );
            evidence = evidence.with_artifact_role(artifact_kind_label(classified.kind));
        }

        evidence
    }

    fn recovery_job_kind(&self) -> RecoveryJobKind {
        match self.active_job {
            ActiveJob::SetupBootstrap => RecoveryJobKind::SetupBootstrap,
            ActiveJob::ManifestRepair => RecoveryJobKind::ManifestRepair,
            ActiveJob::RouteIntegrationRepair => RecoveryJobKind::RouteIntegrationRepair,
            ActiveJob::IntegrationArtifactCreation => RecoveryJobKind::ScaffoldMaterialization,
            ActiveJob::SourceImplementationRepair => RecoveryJobKind::SourceImplementationRepair,
            ActiveJob::TestRepair => RecoveryJobKind::TestAlignmentRepair,
            ActiveJob::DocsRepair => RecoveryJobKind::DocumentationRepair,
            ActiveJob::VerifierPolicyRepair => {
                if self.artifact_role.as_deref().is_some_and(|role| {
                    matches!(
                        role,
                        "manifest" | "config" | "setup_manifest" | "setup_config"
                    )
                }) {
                    RecoveryJobKind::ManifestRepair
                } else {
                    RecoveryJobKind::VerifierContractCorrection
                }
            }
            ActiveJob::ExplicitStop => RecoveryJobKind::ExplicitStop,
        }
    }

    fn recovery_action_kind(&self, job: RecoveryJobKind) -> RecoveryActionKind {
        match self.repair_action {
            RepairAction::AddManifestDependency
            | RepairAction::RepairBuildScript
            | RepairAction::RepairDevScript
            | RepairAction::RepairTailwindContract
                if job == RecoveryJobKind::ManifestRepair =>
            {
                RecoveryActionKind::AddMissingManifestDependency
            }
            RepairAction::RepairTailwindContract | RepairAction::RepairTsconfigAlias => {
                RecoveryActionKind::ReplaceInvalidVerifierCommand
            }
            RepairAction::ConnectArtifactToSelectedRoute => {
                RecoveryActionKind::ConnectExistingArtifactToEntrypoint
            }
            RepairAction::CreateMissingIntegrationArtifact => {
                RecoveryActionKind::CreateRequiredArtifact
            }
            RepairAction::RepairSourceError => {
                if job == RecoveryJobKind::ExplicitStop {
                    RecoveryActionKind::StopWithStructuredEvidence
                } else {
                    RecoveryActionKind::EditSourceForDiagnostic
                }
            }
            RepairAction::StopWithSetupBlocker | RepairAction::StopNoAdmittedTarget => {
                RecoveryActionKind::StopWithStructuredEvidence
            }
            RepairAction::AddManifestDependency
            | RepairAction::RepairBuildScript
            | RepairAction::RepairDevScript => RecoveryActionKind::AddMissingManifestDependency,
        }
    }
}

pub(crate) fn profile_failure_policy(
    failure: &ProfileVerificationFailure,
) -> RecoveryPolicyDecision {
    match failure.code.as_str() {
        "nextjs_dependency_version_conflict" => {
            let mut decision = manifest_policy(
                RepairAction::AddManifestDependency,
                "manifest_dependency_repair",
                "package.json",
                "edit package.json so next, react, react-dom, TypeScript, and React type versions use a stable compatible generated-app dependency family; use a stable TypeScript 5.x range such as ^5.4.0 and @types/react 18.x with React 18/Next.js 14; do not switch generated setup repair to latest packages; preserve scripts.build=next build; do not keep exact React pins below 18.2 with Next.js 14, TypeScript 6, exact TypeScript pins such as 5.0.0, or @types/react 19",
            );
            decision.disallowed_actions.extend([
                "Do not keep exact React pins below 18.2 with Next.js 14.".to_string(),
                "Do not keep TypeScript 6 or @types/react 19 in generated Next.js 14/React 18 apps.".to_string(),
                "Do not switch generated setup repair to latest packages as the compatibility strategy.".to_string(),
                "Do not rewrite scripts.build away from next build.".to_string(),
            ]);
            decision
        }
        "nextjs_missing_dependency"
        | "nextjs_dependency_missing"
        | "nextjs_dependency_version_missing" => manifest_policy(
            RepairAction::AddManifestDependency,
            "manifest_dependency_repair",
            "package.json",
            "edit package.json to include required Next.js runtime dependencies without removing build or dev scripts",
        ),
        "nextjs_build_script_drift" => manifest_policy(
            RepairAction::RepairBuildScript,
            "manifest_script_repair",
            "package.json",
            "edit package.json so scripts.build runs next build",
        ),
        "nextjs_dev_port_drift" | "nextjs_dev_script_drift" => manifest_policy(
            RepairAction::RepairDevScript,
            "manifest_script_repair",
            "package.json",
            "edit package.json so scripts.dev runs next dev on the requested port",
        ),
        "nextjs_tailwind_contract"
        | "nextjs_tailwind_missing"
        | "nextjs_tailwind_content_missing"
        | "nextjs_tailwind_css_missing"
        | "nextjs_tailwind_postcss_missing" => tailwind_policy(failure),
        "nextjs_route_not_integrated" => route_integration_policy(failure),
        "nextjs_integration_artifact_missing" => missing_integration_artifact_policy(failure),
        "nextjs_alias_missing" | "nextjs_tsconfig_alias_missing" => config_policy(
            "tsconfig.json",
            "edit tsconfig.json so @/* resolves to the selected source root used by the Next.js app",
        ),
        "nextjs_tsconfig_excludes_route" => config_policy(
            "tsconfig.json",
            "align tsconfig rootDir with the selected Next.js route root",
        ),
        "nextjs_src_app_missing" | "nextjs_root_app_missing" | "nextjs_app_root_ambiguous" => {
            profile_contract_policy(
                None,
                "consolidate Next.js route files under one selected app root",
            )
        }
        _ => source_repair_policy(
            first_path(failure),
            "repair the source implementation so the profile verifier can pass",
        ),
    }
}

fn manifest_policy(
    repair_action: RepairAction,
    repair_kind: &str,
    target: &str,
    required_action: &str,
) -> RecoveryPolicyDecision {
    RecoveryPolicyDecision {
        active_job: ActiveJob::ManifestRepair,
        repair_action,
        repair_kind: repair_kind.to_string(),
        repair_target: Some(target.to_string()),
        artifact_role: Some("manifest".to_string()),
        required_action: required_action.to_string(),
        disallowed_actions: vec![
            "do not edit route or implementation files while repairing manifest setup".to_string(),
            "do not rewrite verifier commands to hide setup failures".to_string(),
        ],
        setup_implication: "setup_after_manifest_repair_required".to_string(),
        rerun_authority: vec![
            "profile_verification".to_string(),
            "npm install".to_string(),
            "npm run build".to_string(),
        ],
        target_candidates: vec![RepairTargetCandidate::from_path(
            target,
            "manifest is the only admitted target for dependency/script contract repair",
            0,
        )],
    }
}

fn tailwind_policy(failure: &ProfileVerificationFailure) -> RecoveryPolicyDecision {
    let target = tailwind_target(failure).unwrap_or_else(|| "package.json".to_string());
    let setup_implication = if target == "package.json" {
        "setup_after_manifest_repair_required".to_string()
    } else {
        "none: rerun verifier after editing the Tailwind/PostCSS/CSS contract".to_string()
    };
    RecoveryPolicyDecision {
        active_job: if target == "package.json" {
            ActiveJob::ManifestRepair
        } else {
            ActiveJob::VerifierPolicyRepair
        },
        repair_action: RepairAction::RepairTailwindContract,
        repair_kind: "tailwind_contract_repair".to_string(),
        repair_target: Some(target.clone()),
        artifact_role: Some(if target == "package.json" {
            "manifest".to_string()
        } else {
            classify_role(&target)
        }),
        required_action: match target.as_str() {
            "package.json" => {
                "edit package.json to include the required Tailwind/PostCSS dependencies without removing Next.js runtime dependencies".to_string()
            }
            "postcss.config.js" => {
                "create or edit postcss.config.js so it uses the required Tailwind/PostCSS plugin configuration".to_string()
            }
            "tailwind.config.js" => {
                "create or edit tailwind.config.js so it covers the selected Next.js app and component paths".to_string()
            }
            _ => format!(
                "edit {target} so the Tailwind profile contract is satisfied without weakening the verifier"
            ),
        },
        disallowed_actions: vec![
            "do not remove Tailwind usage to bypass the contract".to_string(),
            "do not rewrite verifier commands to hide Tailwind failures".to_string(),
        ],
        setup_implication,
        rerun_authority: vec!["profile_verification".to_string(), "npm run build".to_string()],
        target_candidates: vec![RepairTargetCandidate::from_path(
            target,
            "Tailwind failure selected this concrete contract file",
            0,
        )],
    }
}

fn route_integration_policy(failure: &ProfileVerificationFailure) -> RecoveryPolicyDecision {
    let selected_route = failure.paths.first().cloned();
    let disconnected_artifact = failure.paths.get(1).cloned();
    let target = failure
        .paths
        .get(2)
        .filter(|path| is_admissible_repair_target(path))
        .cloned()
        .or_else(|| {
            selected_route
                .clone()
                .filter(|path| is_admissible_repair_target(path))
        })
        .or_else(|| first_path(failure))
        .unwrap_or_else(|| "app/page.tsx".to_string());
    let source = disconnected_artifact
        .as_deref()
        .unwrap_or("the disconnected artifact");
    let route = selected_route
        .as_deref()
        .unwrap_or("the selected Next.js route");

    let mut target_candidates = Vec::new();
    target_candidates.push(RepairTargetCandidate::from_path(
        target.clone(),
        "highest-priority admitted edit target that can connect the disconnected artifact",
        0,
    ));
    if selected_route.as_deref() != Some(target.as_str())
        && let Some(route_path) = selected_route.clone()
    {
        target_candidates.push(RepairTargetCandidate::from_path(
            route_path,
            "selected route is an admitted fallback target for route graph integration",
            1,
        ));
    }

    let required_action = if selected_route.as_deref() == Some(target.as_str()) {
        format!("edit {target} so it imports or references {source}")
    } else {
        format!(
            "edit {target} so {source} is imported, referenced, or otherwise reachable from {route}"
        )
    };

    RecoveryPolicyDecision {
        active_job: ActiveJob::RouteIntegrationRepair,
        repair_action: RepairAction::ConnectArtifactToSelectedRoute,
        repair_kind: "route_integration_repair".to_string(),
        repair_target: Some(target.clone()),
        artifact_role: Some("route_integration".to_string()),
        required_action,
        disallowed_actions: vec![
            "Do not create an unrelated replacement app or route tree".to_string(),
            "Do not delete the disconnected artifact to hide the integration failure".to_string(),
            "Do not satisfy this by editing package.json or dependency setup".to_string(),
        ],
        setup_implication: "none: rerun profile verification after route graph integration"
            .to_string(),
        rerun_authority: vec![
            "profile_verification".to_string(),
            "npm run build".to_string(),
        ],
        target_candidates,
    }
}

fn missing_integration_artifact_policy(
    failure: &ProfileVerificationFailure,
) -> RecoveryPolicyDecision {
    let target = first_path(failure).unwrap_or_else(|| "app/page.tsx".to_string());
    RecoveryPolicyDecision {
        active_job: ActiveJob::IntegrationArtifactCreation,
        repair_action: RepairAction::CreateMissingIntegrationArtifact,
        repair_kind: "integration_artifact_creation".to_string(),
        repair_target: Some(target.clone()),
        artifact_role: Some(classify_role(&target)),
        required_action: format!("create {target} before editing selected route integration"),
        disallowed_actions: vec![
            "do not bypass the required artifact by weakening profile verification".to_string(),
            "do not move the app root while creating the missing artifact".to_string(),
        ],
        setup_implication: "none: rerun profile verification after creating the missing artifact"
            .to_string(),
        rerun_authority: vec!["profile_verification".to_string()],
        target_candidates: vec![RepairTargetCandidate::from_path(
            target,
            "profile verifier reported this required artifact as missing",
            0,
        )],
    }
}

fn config_policy(target: &str, required_action: &str) -> RecoveryPolicyDecision {
    RecoveryPolicyDecision {
        active_job: ActiveJob::VerifierPolicyRepair,
        repair_action: RepairAction::RepairTsconfigAlias,
        repair_kind: "config_contract_repair".to_string(),
        repair_target: Some(target.to_string()),
        artifact_role: Some("config".to_string()),
        required_action: required_action.to_string(),
        disallowed_actions: vec![
            "do not edit source imports to hide a missing profile alias".to_string(),
            "do not rewrite verifier commands to fake success".to_string(),
        ],
        setup_implication: "none: rerun profile verification after config repair".to_string(),
        rerun_authority: vec![
            "profile_verification".to_string(),
            "npm run build".to_string(),
        ],
        target_candidates: vec![RepairTargetCandidate::from_path(
            target,
            "config file is the admitted target for this profile contract",
            0,
        )],
    }
}

fn profile_contract_policy(
    target: Option<String>,
    required_action: &str,
) -> RecoveryPolicyDecision {
    let target_candidates = target
        .clone()
        .map(|path| {
            vec![RepairTargetCandidate::from_path(
                path,
                "profile contract selected this artifact as the admitted repair target",
                0,
            )]
        })
        .unwrap_or_default();
    RecoveryPolicyDecision {
        active_job: if target.is_some() {
            ActiveJob::VerifierPolicyRepair
        } else {
            ActiveJob::ExplicitStop
        },
        repair_action: if target.is_some() {
            RepairAction::RepairSourceError
        } else {
            RepairAction::StopNoAdmittedTarget
        },
        repair_kind: "profile_contract_repair".to_string(),
        repair_target: target.clone(),
        artifact_role: target.as_deref().map(classify_role),
        required_action: required_action.to_string(),
        disallowed_actions: vec![
            "do not create a competing profile structure to bypass the contract".to_string(),
            "do not weaken profile verification".to_string(),
        ],
        setup_implication: "none: rerun profile verification after contract repair".to_string(),
        rerun_authority: vec!["profile_verification".to_string()],
        target_candidates,
    }
}

fn source_repair_policy(target: Option<String>, required_action: &str) -> RecoveryPolicyDecision {
    let target_candidates = target
        .clone()
        .map(|path| {
            vec![RepairTargetCandidate::from_path(
                path,
                "source verifier failure selected this artifact as the repair target",
                0,
            )]
        })
        .unwrap_or_default();
    RecoveryPolicyDecision {
        active_job: ActiveJob::SourceImplementationRepair,
        repair_action: RepairAction::RepairSourceError,
        repair_kind: "profile_contract_repair".to_string(),
        repair_target: target.clone(),
        artifact_role: target.as_deref().map(classify_role),
        required_action: required_action.to_string(),
        disallowed_actions: vec![
            "do not edit manifest or setup files unless the failure is reclassified as setup"
                .to_string(),
            "do not weaken verifier commands".to_string(),
        ],
        setup_implication: "none: rerun the original verifier after source repair".to_string(),
        rerun_authority: vec!["profile_verification".to_string()],
        target_candidates,
    }
}

fn first_path(failure: &ProfileVerificationFailure) -> Option<String> {
    failure.paths.first().cloned()
}

fn classify_role(path: &str) -> String {
    let classified =
        classify_profile_artifact(ProfileId::NextJs, path, ArtifactProvenance::ProfileFact);
    artifact_kind_label(classified.kind).to_string()
}

fn is_admissible_repair_target(path: &str) -> bool {
    classify_profile_artifact(ProfileId::NextJs, path, ArtifactProvenance::ProfileFact)
        .eligibility
        .recovery_target
}

fn tailwind_target(failure: &ProfileVerificationFailure) -> Option<String> {
    failure
        .paths
        .first()
        .cloned()
        .or_else(|| match failure.code.as_str() {
            "nextjs_tailwind_contract"
                if failure
                    .message
                    .to_ascii_lowercase()
                    .contains("postcss.config") =>
            {
                Some("postcss.config.js".to_string())
            }
            "nextjs_tailwind_contract"
                if failure
                    .message
                    .to_ascii_lowercase()
                    .contains("tailwind.config") =>
            {
                Some("tailwind.config.js".to_string())
            }
            "nextjs_tailwind_contract"
                if failure
                    .message
                    .to_ascii_lowercase()
                    .contains("package.json")
                    || failure.message.to_ascii_lowercase().contains("dependency") =>
            {
                Some("package.json".to_string())
            }
            "nextjs_tailwind_missing" => Some("package.json".to_string()),
            "nextjs_tailwind_content_missing" => Some("tailwind.config.js".to_string()),
            "nextjs_tailwind_css_missing" => Some("app/globals.css".to_string()),
            "nextjs_tailwind_postcss_missing" => Some("postcss.config.js".to_string()),
            _ => None,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failure(code: &str, paths: &[&str]) -> ProfileVerificationFailure {
        ProfileVerificationFailure {
            code: code.to_string(),
            message: "test failure".to_string(),
            paths: paths.iter().map(|path| (*path).to_string()).collect(),
        }
    }

    #[test]
    fn active_job_labels_are_stable() {
        assert_eq!(ActiveJob::ManifestRepair.as_str(), "manifest_repair");
        assert_eq!(ActiveJob::SetupBootstrap.as_str(), "setup_bootstrap");
        assert_eq!(
            ActiveJob::RouteIntegrationRepair.as_str(),
            "route_integration_repair"
        );
        assert_eq!(
            ActiveJob::IntegrationArtifactCreation.as_str(),
            "integration_artifact_creation"
        );
        assert_eq!(
            ActiveJob::SourceImplementationRepair.as_str(),
            "source_implementation_repair"
        );
        assert_eq!(ActiveJob::TestRepair.as_str(), "test_repair");
        assert_eq!(ActiveJob::DocsRepair.as_str(), "docs_repair");
        assert_eq!(
            ActiveJob::VerifierPolicyRepair.as_str(),
            "verifier_policy_repair"
        );
        assert_eq!(ActiveJob::ExplicitStop.as_str(), "explicit_stop");
    }

    #[test]
    fn repair_action_labels_are_stable() {
        assert_eq!(
            RepairAction::AddManifestDependency.as_str(),
            "add_manifest_dependency"
        );
        assert_eq!(
            RepairAction::RepairBuildScript.as_str(),
            "repair_build_script"
        );
        assert_eq!(RepairAction::RepairDevScript.as_str(), "repair_dev_script");
        assert_eq!(
            RepairAction::RepairTailwindContract.as_str(),
            "repair_tailwind_contract"
        );
        assert_eq!(
            RepairAction::RepairTsconfigAlias.as_str(),
            "repair_tsconfig_alias"
        );
        assert_eq!(
            RepairAction::CreateMissingIntegrationArtifact.as_str(),
            "create_missing_integration_artifact"
        );
        assert_eq!(
            RepairAction::ConnectArtifactToSelectedRoute.as_str(),
            "connect_artifact_to_selected_route"
        );
        assert_eq!(
            RepairAction::RepairSourceError.as_str(),
            "repair_source_error"
        );
        assert_eq!(
            RepairAction::StopWithSetupBlocker.as_str(),
            "stop_with_setup_blocker"
        );
        assert_eq!(
            RepairAction::StopNoAdmittedTarget.as_str(),
            "stop_no_admitted_target"
        );
    }

    #[test]
    fn route_policy_prefers_connection_target_over_disconnected_artifact() {
        let decision = profile_failure_policy(&failure(
            "nextjs_route_not_integrated",
            &[
                "app/page.tsx",
                "app/game/engine.ts",
                "app/components/GameBoard.tsx",
            ],
        ));

        assert_eq!(decision.active_job, ActiveJob::RouteIntegrationRepair);
        assert_eq!(
            decision.repair_action,
            RepairAction::ConnectArtifactToSelectedRoute
        );
        assert_eq!(
            decision.repair_target.as_deref(),
            Some("app/components/GameBoard.tsx")
        );
        assert!(decision.required_action.contains("app/game/engine.ts"));
        assert!(decision.required_action.contains("app/page.tsx"));
        assert!(
            decision
                .disallowed_actions
                .iter()
                .any(|action| action.contains("package.json"))
        );
    }

    #[test]
    fn dependency_policy_is_manifest_repair_with_setup_implication() {
        let decision = profile_failure_policy(&failure(
            "nextjs_dependency_missing",
            &["dependencies.react-dom"],
        ));

        assert_eq!(decision.active_job, ActiveJob::ManifestRepair);
        assert_eq!(decision.repair_action, RepairAction::AddManifestDependency);
        assert_eq!(decision.repair_target.as_deref(), Some("package.json"));
        assert_eq!(
            decision.setup_implication,
            "setup_after_manifest_repair_required"
        );
        assert!(
            decision
                .rerun_authority
                .iter()
                .any(|authority| authority == "npm install")
        );
    }

    #[test]
    fn profile_policy_emits_canonical_active_job_candidate() {
        let decision = profile_failure_policy(&failure(
            "nextjs_route_not_integrated",
            &[
                "app/page.tsx",
                "components/Game.tsx",
                "components/GameBoard.tsx",
            ],
        ));

        let candidate = decision.active_job_candidate();

        assert_eq!(candidate.job, RecoveryJobKind::RouteIntegrationRepair);
        assert_eq!(
            candidate.action,
            RecoveryActionKind::ConnectExistingArtifactToEntrypoint
        );
        assert_eq!(candidate.source_layer, "profile_verification");
        assert_eq!(candidate.source_of_truth, "profile_contract");
        assert_eq!(
            candidate.target_hint.as_deref(),
            Some("components/GameBoard.tsx")
        );
        assert!(candidate.render_line().contains("owner=route_integration"));
    }

    #[test]
    fn apply_to_evidence_records_profile_policy_candidate() {
        let decision = profile_failure_policy(&failure(
            "nextjs_dependency_missing",
            &["dependencies.react-dom"],
        ));

        let evidence = decision.apply_to_evidence(ContractEvidence::new("profile_verification"));

        assert_eq!(evidence.active_job.as_deref(), Some("manifest_repair"));
        assert_eq!(
            evidence.repair_action.as_deref(),
            Some("add_manifest_dependency")
        );
        assert!(
            evidence
                .candidate_jobs
                .iter()
                .any(|line| line.contains("owner=manifest")
                    && line.contains("source_layer=profile_verification"))
        );
    }
}
