use std::path::Path;

use crate::planner::profile::{DomainProfile, ProfileBehaviorProbeReport, ProfileId};

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

    fn assurance_for_completion(
        &self,
        _profile_id: &ProfileId,
        _required_capabilities: &[String],
    ) -> (&'static str, &'static str) {
        ("full", "")
    }

    fn apply_completion_snapshot(
        &self,
        _profile_id: &ProfileId,
        _root: &Path,
        snapshot: &mut crate::eval_events::CompletionSnapshot,
    ) {
        crate::completion_metadata::apply_full_snapshot(snapshot);
    }

    fn apply_completion_projection(
        &self,
        _profile_id: &ProfileId,
        _root: &Path,
        _projection: &mut crate::eval_events::CompletionProjection,
    ) {
    }

    fn default_requested_port(&self) -> Option<u16> {
        None
    }

    fn route_bound_closure(&self, root: &Path) -> std::collections::BTreeSet<std::path::PathBuf> {
        crate::minimal_loop::import_scan::all_route_source_files(root)
    }

    fn run_behavior_probe(
        &self,
        _profile_id: &ProfileId,
        root: &Path,
        goal: &str,
        required_capabilities: &[String],
        offline: bool,
    ) -> anyhow::Result<ProfileBehaviorProbeReport> {
        self.behavior_probe(root, goal, required_capabilities, offline)
    }
}
