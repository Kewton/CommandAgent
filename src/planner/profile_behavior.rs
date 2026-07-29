use std::path::Path;

use crate::planner::profile::{
    DomainProfile, ProfileBehaviorProbeReport, ProfileId, canonical_profile_name, domain_profile,
};
use crate::planner::profiles::python_cli::runtime;

/// Typed runtime surface derived from the E-5b dispatch inventory.
///
/// The responsibility mapping in
/// `workspace/management/runs/e5b-dispatch-audit.md` is:
///
/// - acceptance runtime -> `DomainProfile::{verify_final, verify_invariant,
///   infer_required_capabilities, infer_required_evidence,
///   infer_required_obligations, completion_contract_required}`;
/// - repair boundary -> `DomainProfile::{repair_prompt, deterministic_repair,
///   post_step_repair, evidence_repair_target_paths, hook_snapshot_targets}`;
/// - preset selection -> `DomainProfile::{deterministic_step_plan,
///   preset_ultra_plan, expected_scaffold_paths, setup_scaffold_paths}`;
/// - guidance injection -> `DomainProfile::{guidance, runtime_contract,
///   generation_rules, quality_expectations, interaction_repair_guidance}`;
/// - probe selection -> `DomainProfile::{behavior_probe, build_oracle,
///   dependency_ready, dependency_setup_requirement, parse_compile_errors}`.
///
/// Batch 0 establishes only the typed identity and registry boundary. Existing
/// string dispatch remains in place until its responsibility batch migrates;
/// inherited methods are the implementation surface, not adapter completion.
pub trait ProfileRuntime: DomainProfile {
    fn profile_id(&self) -> ProfileId;
}

pub(crate) fn run(
    root: &Path,
    profile: &str,
    goal: &str,
    required_capabilities: &[String],
    offline: bool,
) -> anyhow::Result<ProfileBehaviorProbeReport> {
    if canonical_profile_name(profile) == "cli" {
        let summary = runtime::run_manifest_checks(root)?;
        return Ok(ProfileBehaviorProbeReport {
            status: summary.assurance.behavior_status(),
            reasons: summary.reasons,
            evidence_path: Some(runtime::EVIDENCE_PATH.to_string()),
        });
    }
    domain_profile(profile).behavior_probe(root, goal, required_capabilities, offline)
}
