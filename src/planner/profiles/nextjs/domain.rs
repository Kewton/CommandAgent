use std::path::Path;

use crate::minimal_loop::build_verifier::{
    BuildVerifierRequirement, CompileError, ForeignToolchainObservation,
};
use crate::minimal_loop::dependency_setup::{
    self, NodeDependencySetupAuthority, NodeDependencySetupRequirement,
};
use crate::minimal_loop::evidence::required_evidence_for_capability;
use crate::planner::profile::{
    DomainProfile, InteractionRepairContract, ProfileBuildOracle, ProfileDeterministicStepPlan,
    ProfileFixReproducerSuggestion, ProfileHookSnapshotTarget, ProfileQualityExpectations,
    ProfileSnapshot, generic_dependency_missing_output, merge_unique_strings, requires_next_binary,
};
use crate::planner::profile_manifest::{ManifestStatus, nextjs_manifest};
use crate::planner::signals;
use crate::planner::ultra_plan::UltraPlan;
use crate::planner::verify::VerificationReport;

pub const PROFILE_ID: &str = "nextjs";

pub struct NextjsProfile;

pub(crate) fn canonical_profile_alias(normalized: &str) -> Option<&'static str> {
    matches!(normalized, "nextjs" | "next-js" | "next.js").then_some(PROFILE_ID)
}

pub(crate) fn matches_profile(profile: &str) -> bool {
    canonical_profile_alias(profile.trim().to_ascii_lowercase().as_str()).is_some()
}

pub(crate) fn manifest_status(profile: &str) -> Option<ManifestStatus> {
    matches_profile(profile).then(|| nextjs_manifest().metadata.status)
}

impl DomainProfile for NextjsProfile {
    fn id(&self) -> &'static str {
        PROFILE_ID
    }

    fn matches(&self, profile: &str) -> bool {
        matches_profile(profile)
    }

    fn expected_scaffold_paths(&self, root: &Path, goal: &str) -> Vec<String> {
        super::expected_paths(root, goal)
    }

    fn setup_scaffold_paths(&self, root: &Path) -> Vec<String> {
        super::setup_scaffold_paths(root)
    }

    fn before_phase(&self, root: &Path) -> anyhow::Result<ProfileSnapshot> {
        let scaffold_paths = self.setup_scaffold_paths(root);
        if !scaffold_paths.is_empty() {
            let _ = self.complete_scaffold(root, &scaffold_paths)?;
        }
        Ok(ProfileSnapshot::None)
    }

    fn complete_scaffold(
        &self,
        root: &Path,
        missing_paths: &[String],
    ) -> anyhow::Result<Vec<String>> {
        super::complete_scaffold(root, missing_paths)
    }

    fn verify_final(&self, root: &Path, goal: &str) -> VerificationReport {
        super::verify(root, goal)
    }

    fn verify_invariant(
        &self,
        root: &Path,
        goal: &str,
        _snapshot: &ProfileSnapshot,
    ) -> VerificationReport {
        super::verify_invariant(root, goal)
    }

    fn guidance(&self, goal: &str) -> Option<String> {
        Some(super::guidance(goal))
    }

    fn fix_reproducer_suggestion(&self, goal: &str) -> Option<ProfileFixReproducerSuggestion> {
        super::fix_reproducer::suggestion_for(goal)
    }

    fn runtime_contract(&self, intent: &str, goal: &str) -> String {
        super::runtime_contract(intent, goal)
    }

    fn generation_rules(&self, intent: &str) -> Option<&'static str> {
        Some(super::generation_rules(intent))
    }

    fn deterministic_step_plan(
        &self,
        phase_prompt: &str,
        root: &Path,
        goal: &str,
    ) -> Option<ProfileDeterministicStepPlan> {
        super::deterministic_step_plan(phase_prompt, root, goal)
    }

    fn preset_ultra_plan(&self, goal: &str, style: &str, intent: &str) -> Option<UltraPlan> {
        super::preset_ultra_plan(goal, style, intent)
    }

    fn quality_expectations(&self, root: &Path, goal: &str) -> ProfileQualityExpectations {
        super::quality_expectations(root, goal)
    }

    fn repair_prompt(
        &self,
        root: &Path,
        goal: &str,
        report: &VerificationReport,
    ) -> Option<String> {
        Some(super::repair_prompt(root, goal, report))
    }

    fn interaction_repair_guidance(
        &self,
        failure_kind: &str,
        contract: &InteractionRepairContract,
    ) -> Vec<String> {
        super::knowledge::interaction_repair_guidance(
            failure_kind,
            &contract.required_capabilities,
            &contract.required_evidence,
        )
    }

    fn deterministic_repair(
        &self,
        root: &Path,
        goal: &str,
        report: &VerificationReport,
    ) -> anyhow::Result<bool> {
        super::auto_repair(root, goal, report)
    }

    fn post_step_repair(&self, root: &Path, goal: &str) -> anyhow::Result<bool> {
        super::repair_manifest_coherence(root, goal)
    }

    fn build_oracle(&self, command: &str) -> Option<ProfileBuildOracle> {
        requires_next_binary(command).then(|| ProfileBuildOracle {
            command: command.to_string(),
            profile: Some(self.id().to_string()),
            requires_dependency_setup: true,
        })
    }

    fn dependency_ready(&self, root: &Path, command: &str) -> bool {
        if requires_next_binary(command) {
            if requires_package_manifest(command) && !root.join("package.json").is_file() {
                return false;
            }
            dependency_setup::next_build_dependencies_ready(root)
        } else {
            true
        }
    }

    fn dependency_missing_reason(&self, root: &Path, command: &str) -> String {
        if requires_package_manifest(command) && !root.join("package.json").is_file() {
            return "package.json missing before Next.js build verifier".to_string();
        }
        dependency_setup::next_build_missing_dependency_reason(root)
    }

    fn dependency_setup_requirement(
        &self,
        root: &Path,
        requirement: &BuildVerifierRequirement,
        setup_authority: NodeDependencySetupAuthority,
    ) -> Option<NodeDependencySetupRequirement> {
        Some(dependency_setup::requirement_for_next_build(
            root,
            requirement.profile.as_deref(),
            &requirement.reason,
            setup_authority,
        ))
    }

    fn dependency_missing_output(&self, output: &str) -> bool {
        generic_dependency_missing_output(output)
    }

    fn annotate_compile_errors(&self, root: &Path, errors: &mut [CompileError]) {
        let closure = crate::minimal_loop::import_scan::route_bound_closure(root, self.id());
        for error in errors {
            error.route_bound = Some(closure.contains(Path::new(&error.path)));
        }
    }

    fn foreign_toolchain(
        &self,
        root: &Path,
        requirement: &BuildVerifierRequirement,
    ) -> Option<ForeignToolchainObservation> {
        if !requires_next_binary(&requirement.command) {
            return None;
        }
        if dependency_setup::next_package_ready(root) {
            return None;
        }
        let resolved =
            crate::minimal_loop::verifier_env::foreign_node_modules_bin_on_path(root, "next")?;
        Some(ForeignToolchainObservation {
            tool: "next".to_string(),
            resolved_path: resolved.display().to_string(),
            workspace_root: root.display().to_string(),
            reason: format!(
                "foreign_toolchain_detected: workspace node_modules/next missing; PATH would resolve next outside workspace at {}",
                resolved.display()
            ),
        })
    }

    fn source_paths(&self, root: &Path) -> Vec<String> {
        super::app_source_paths(root)
    }

    fn evidence_repair_target_paths(&self, root: &Path, evidence_keys: &[String]) -> Vec<String> {
        super::evidence_repair_target_paths(root, evidence_keys)
    }

    fn hook_snapshot_targets(&self, root: &Path, _goal: &str) -> Vec<ProfileHookSnapshotTarget> {
        super::hook_snapshot_targets(root)
    }

    fn infer_required_capabilities(&self, goal: &str) -> Vec<String> {
        let mut capabilities = Vec::new();
        let game_like = signals::contains_game_token(goal);
        let persistence_like = signals::contains_persistence_token(goal);
        let interactive_app_like = signals::contains_interactive_token(goal);
        if game_like {
            merge_unique_strings(&mut capabilities, &["stateful_interaction".to_string()]);
            merge_unique_strings(&mut capabilities, &["start_or_restart_flow".to_string()]);
            merge_unique_strings(&mut capabilities, &["player_control".to_string()]);
            merge_unique_strings(&mut capabilities, &["adversary_or_challenge".to_string()]);
            merge_unique_strings(&mut capabilities, &["progression_or_score".to_string()]);
            merge_unique_strings(
                &mut capabilities,
                &["failure_or_collision_rule".to_string()],
            );
        } else if interactive_app_like {
            merge_unique_strings(&mut capabilities, &["stateful_interaction".to_string()]);
            merge_unique_strings(&mut capabilities, &["user_input_or_action".to_string()]);
            merge_unique_strings(&mut capabilities, &["visible_state_change".to_string()]);
            if persistence_like {
                merge_unique_strings(&mut capabilities, &["persistence".to_string()]);
            }
        }
        capabilities
    }

    fn infer_required_evidence(
        &self,
        _goal: &str,
        required_capabilities: &[String],
    ) -> Vec<String> {
        let mut evidence = vec![
            "nextjs_route_evidence".to_string(),
            "build_command_or_dependency_missing_boundary".to_string(),
        ];
        for capability in required_capabilities {
            merge_unique_strings(&mut evidence, &required_evidence_for_capability(capability));
        }
        evidence
    }

    fn infer_required_obligations(
        &self,
        goal: &str,
        required_capabilities: &[String],
    ) -> Vec<String> {
        if signals::contains_app_like_token(goal) || !required_capabilities.is_empty() {
            return vec!["implementation".to_string()];
        }
        Vec::new()
    }

    fn completion_contract_required(&self, _goal: &str, _required_capabilities: &[String]) -> bool {
        true
    }
}

fn requires_package_manifest(command: &str) -> bool {
    let normalized = command.to_ascii_lowercase();
    normalized.starts_with("npm ")
        || normalized.starts_with("pnpm ")
        || normalized.starts_with("yarn ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_aliases_and_admission_stay_inside_nextjs_boundary() {
        for profile in ["nextjs", "next-js", "next.js", " Next.JS "] {
            assert!(matches_profile(profile), "profile={profile}");
            assert_eq!(manifest_status(profile), Some(ManifestStatus::Admitted));
        }
        assert!(!matches_profile("data"));
        assert_eq!(manifest_status("data"), None);
    }
}
