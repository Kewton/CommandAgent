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

    fn required_capabilities(&self, goal: &str) -> Vec<String> {
        self.infer_required_capabilities(goal)
    }

    fn required_evidence(&self, goal: &str, required_capabilities: &[String]) -> Vec<String> {
        let mut evidence = self.infer_required_evidence(goal, required_capabilities);
        for capability in required_capabilities {
            for required in
                crate::minimal_loop::evidence::required_evidence_for_capability(capability)
            {
                if !evidence.contains(&required) {
                    evidence.push(required);
                }
            }
        }
        evidence
    }

    fn required_obligations(
        &self,
        profile_id: &ProfileId,
        goal: &str,
        required_capabilities: &[String],
    ) -> Vec<String> {
        let obligations = self.infer_required_obligations(goal, required_capabilities);
        if !obligations.is_empty() {
            return obligations;
        }
        let app_profile = matches!(profile_id, ProfileId::Web | ProfileId::Vite);
        if app_profile
            && (crate::planner::signals::contains_app_like_token(goal)
                || !required_capabilities.is_empty())
        {
            return vec!["implementation".to_string()];
        }
        if required_capabilities.iter().any(|capability| {
            matches!(
                capability.as_str(),
                "implementation"
                    | "entrypoint"
                    | "input_output_contract"
                    | "player_control"
                    | "stateful_interaction"
                    | "user_input_or_action"
                    | "visible_state_change"
                    | "persistence"
                    | "adversary_or_challenge"
                    | "progression_or_score"
                    | "failure_or_collision_rule"
            )
        }) {
            return vec!["implementation".to_string()];
        }
        Vec::new()
    }

    fn requires_completion_contract(
        &self,
        profile_id: &ProfileId,
        goal: &str,
        required_capabilities: &[String],
    ) -> bool {
        if self.completion_contract_required(goal, required_capabilities) {
            return true;
        }
        let web_or_app_profile = matches!(
            profile_id,
            ProfileId::Vite | ProfileId::React | ProfileId::Web
        );
        let interactive_goal = crate::planner::signals::contains_app_like_token(goal)
            || crate::planner::signals::contains_browser_probe_token(goal)
            || crate::planner::signals::contains_canvas_token(goal);
        let interactive_capability = required_capabilities.iter().any(|capability| {
            matches!(
                capability.as_str(),
                "stateful_interaction"
                    | "start_or_restart_flow"
                    | "player_control"
                    | "adversary_or_challenge"
                    | "progression_or_score"
                    | "failure_or_collision_rule"
                    | "persistence"
                    | "browser_interaction"
                    | "playable_ui"
            )
        });
        interactive_capability || (web_or_app_profile && interactive_goal)
    }

    fn interactive_app_capabilities(&self, profile_id: &ProfileId) -> Vec<String> {
        if matches!(
            profile_id,
            ProfileId::Nextjs | ProfileId::React | ProfileId::Vite | ProfileId::Web
        ) {
            vec![
                "stateful_interaction".to_string(),
                "user_input_or_action".to_string(),
                "visible_state_change".to_string(),
            ]
        } else {
            Vec::new()
        }
    }

    fn browser_release_gate_profile(&self) -> bool {
        false
    }

    fn invariant_expected_paths(&self, root: &Path, paths: Vec<String>) -> Vec<String> {
        self.filter_invariant_expected_paths(root, paths)
    }

    fn invariant_setup_paths(&self, root: &Path) -> Vec<String> {
        self.setup_scaffold_paths(root)
    }

    fn verify_phase_invariant(
        &self,
        root: &Path,
        goal: &str,
        snapshot: &crate::planner::profile::ProfileSnapshot,
    ) -> crate::planner::verify::VerificationReport {
        let snapshot_report = self.after_phase(root, snapshot);
        if !snapshot_report.is_pass() {
            return snapshot_report;
        }
        self.verify_invariant(root, goal, snapshot)
    }

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

    fn plan_final_behavior_probe_required(&self, _profile_id: &ProfileId) -> bool {
        false
    }

    fn run_final_testimony_check(
        &self,
        _root: &Path,
        _browser_readiness_path: Option<&str>,
        _interaction_evidence_path: Option<&str>,
    ) -> anyhow::Result<Option<ProfileBehaviorProbeReport>> {
        Ok(None)
    }

    fn canonicalize_create_plan(
        &self,
        _plan: &mut crate::planner::step_plan::StepPlan,
        _create_intent: bool,
        _terminal_plan: bool,
        _eval_events_path: Option<&Path>,
    ) -> usize {
        0
    }

    fn bind_empty_fix_verify_steps(
        &self,
        _plan: &mut crate::planner::step_plan::StepPlan,
        _phase_label: Option<&str>,
        _eval_events_path: Option<&Path>,
    ) -> usize {
        0
    }

    fn convert_preset_phase_setup_steps(
        &self,
        _plan: &mut crate::planner::step_plan::StepPlan,
        _root: &Path,
        _goal: &str,
        _phase_scope: Option<(&str, bool)>,
        _preset_phase: bool,
        _eval_events_path: Option<&Path>,
    ) -> usize {
        0
    }

    fn runtime_step_with_profile_checks(
        &self,
        _root: &Path,
        _goal: &str,
        step: &crate::planner::step_plan::PlanStep,
        _phase_id: Option<&str>,
        _eval_events_path: Option<&Path>,
    ) -> (crate::planner::step_plan::PlanStep, bool) {
        (step.clone(), false)
    }

    fn pre_satisfied_verify_first(
        &self,
        _root: &Path,
        _step: &crate::planner::step_plan::PlanStep,
    ) -> Option<bool> {
        None
    }

    fn step_short_circuit_precheck_applicable(
        &self,
        step: &crate::planner::step_plan::PlanStep,
    ) -> bool {
        crate::planner::setup_step_policy::profile_independent_short_circuit_precheck(step)
    }

    fn fallback_setup_plan(
        &self,
        _root: &Path,
        _goal: &str,
    ) -> Option<crate::planner::step_plan::StepPlan> {
        None
    }

    fn default_plan_preset(
        &self,
        _intent: Option<crate::planner::adjudication::contract::IntentId>,
    ) -> Option<(crate::config::PlanPreset, &'static str)> {
        None
    }

    fn inject_step_material(
        &self,
        _config: &crate::config::Config,
        _step: &mut crate::planner::step_plan::PlanStep,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn synthesizes_fix_plan(&self) -> bool {
        false
    }

    fn synthesizes_investigation_plan(&self) -> bool {
        false
    }

    fn synthesizes_create_plan(&self) -> bool {
        false
    }

    fn enforce_nextjs_plan_shape(&self) -> bool {
        false
    }

    fn dependency_reconciliation_requirement(
        &self,
        root: &Path,
        profile_id: &ProfileId,
        manifest_changed: bool,
        reason: &str,
        authority: crate::minimal_loop::dependency_setup::NodeDependencySetupAuthority,
    ) -> Option<crate::minimal_loop::dependency_setup::NodeDependencySetupRequirement> {
        if crate::minimal_loop::dependency_setup::package_json_declares_dependencies(root)
            && (manifest_changed
                || !crate::minimal_loop::dependency_setup::node_declared_dependencies_ready(root))
        {
            Some(
                crate::minimal_loop::dependency_setup::requirement_for_node_declared_dependencies(
                    root,
                    Some(profile_id.as_str()),
                    reason,
                    authority,
                ),
            )
        } else {
            None
        }
    }

    fn release_recovery_verify_commands(
        &self,
        _reasons: &[String],
        _probe_infrastructure_failure: bool,
    ) -> Vec<String> {
        vec!["rerun deterministic acceptance checks for the original goal".to_string()]
    }

    fn filter_invariant_expected_paths(&self, _root: &Path, paths: Vec<String>) -> Vec<String> {
        paths
    }

    fn invariant_relevant_paths(&self, _root: &Path, _reason: &str) -> Vec<std::path::PathBuf> {
        Vec::new()
    }

    fn styling_choice_rule(&self) -> &'static str {
        ""
    }

    fn route_bound_constraint(&self) -> &'static str {
        ""
    }

    fn is_entrypoint_scaffold_path(&self, _path: &str) -> bool {
        false
    }
}
