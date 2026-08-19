//! Manifest-driven runtime for externally supplied draft profiles.
//!
//! This is a thin adapter, not a new lifecycle. A standalone draft profile runs
//! the shared generic mechanisms with its manifest's plan preset, artifact
//! obligations, and guidance bound in. An overlay effective profile delegates
//! every base behaviour to the admitted embedded base runtime and only adds the
//! overlay's obligations on top. The runner is untouched.

use std::path::Path;

use crate::minimal_loop::build_verifier::{
    BuildVerifierRequirement, CompileError, ForeignToolchainObservation, FullCommandOutput,
};
use crate::minimal_loop::dependency_setup::{
    NodeDependencySetupAuthority, NodeDependencySetupRequirement,
};
use crate::planner::adjudication::contract::ProbeOutcome;
use crate::planner::capability_catalog::{InternalCapability, ResolvedCapability};
use crate::planner::profile::{
    DomainProfile, InteractionRepairContract, ProfileBehaviorProbeReport, ProfileBuildOracle,
    ProfileDeterministicStepPlan, ProfileFixRegressionAdapter, ProfileFixRegressionBinding,
    ProfileFixRegressionObservation, ProfileFixReproducerObservation,
    ProfileFixReproducerSuggestion, ProfileHookSnapshotTarget, ProfileId,
    ProfileQualityExpectations, ProfileSnapshot,
};
use crate::planner::profile_behavior::ProfileRuntime;
use crate::planner::profile_descriptor::ProfileDescriptor;
use crate::planner::profile_manifest::overlay::LoadedOverlay;
use crate::planner::profile_manifest::{
    ArtifactCardinality, GuidanceTriggerCondition, ManifestV1, ResolvedCheck,
};
use crate::planner::ultra_plan::{UltraPhase, UltraPlan};
use crate::planner::verify::VerificationReport;

/// What the adapter is bound to.
pub enum ManifestBinding {
    /// A standalone external manifest: shared generic mechanisms only.
    Standalone { manifest: &'static ManifestV1 },
    /// An additive overlay on an admitted embedded base.
    Overlay {
        base: &'static ProfileDescriptor,
        overlay: &'static LoadedOverlay,
    },
}

pub struct ManifestDrivenProfile {
    id: &'static str,
    binding: ManifestBinding,
}

impl ManifestDrivenProfile {
    pub const fn standalone(id: &'static str, manifest: &'static ManifestV1) -> Self {
        Self {
            id,
            binding: ManifestBinding::Standalone { manifest },
        }
    }

    pub const fn overlay(
        id: &'static str,
        base: &'static ProfileDescriptor,
        overlay: &'static LoadedOverlay,
    ) -> Self {
        Self {
            id,
            binding: ManifestBinding::Overlay { base, overlay },
        }
    }

    fn base(&self) -> Option<&'static ProfileDescriptor> {
        match &self.binding {
            ManifestBinding::Standalone { .. } => None,
            ManifestBinding::Overlay { base, .. } => Some(base),
        }
    }

    fn base_domain(&self) -> Option<&'static dyn DomainProfile> {
        self.base().map(|base| base.domain)
    }

    fn base_runtime(&self) -> Option<&'static dyn ProfileRuntime> {
        self.base().map(|base| base.runtime)
    }

    fn base_profile_id<'a>(&'a self, requested: &'a ProfileId) -> &'a ProfileId {
        self.base().map(|base| &base.id).unwrap_or(requested)
    }

    /// The declaration that owns artifacts, guidance, and the plan preset.
    fn manifest(&self) -> &'static ManifestV1 {
        match &self.binding {
            ManifestBinding::Standalone { manifest } => manifest,
            ManifestBinding::Overlay { overlay, .. } => overlay.base,
        }
    }

    /// Obligations this adapter adds beyond whatever the base already requires.
    fn added_artifacts(&self) -> Vec<String> {
        match &self.binding {
            ManifestBinding::Standalone { manifest } => manifest.artifacts.preferred_paths(),
            ManifestBinding::Overlay { overlay, .. } => overlay.added_artifacts(),
        }
    }

    fn added_guidance(&self) -> Vec<String> {
        match &self.binding {
            ManifestBinding::Standalone { manifest } => manifest
                .guidance
                .variants
                .values()
                .filter(|variant| {
                    variant
                        .triggers
                        .iter()
                        .any(|trigger| trigger.condition == GuidanceTriggerCondition::Always)
                })
                .flat_map(|variant| variant.messages.values().cloned())
                .collect(),
            ManifestBinding::Overlay { overlay, .. } => overlay.always_guidance(),
        }
    }

    /// Declared artifact obligations, checked deterministically against the
    /// workspace. Group cardinality is enforced exactly as declared.
    fn artifact_failures(&self, root: &Path) -> Vec<String> {
        let mut failures = Vec::new();
        let requirements = match &self.binding {
            ManifestBinding::Standalone { manifest } => Some(&manifest.artifacts),
            ManifestBinding::Overlay { overlay, .. } => overlay.overlay.artifacts.as_ref(),
        };
        let Some(requirements) = requirements else {
            return failures;
        };
        for path in &requirements.required {
            if !root.join(path).is_file() {
                failures.push(format!("{} required artifact missing: {path}", self.id));
            }
        }
        for group in &requirements.groups {
            let present = group
                .paths
                .iter()
                .filter(|path| root.join(path).is_file())
                .count();
            let satisfied = match group.cardinality {
                ArtifactCardinality::EitherOf => present >= 1,
                ArtifactCardinality::ExactlyOneOf => present == 1,
            };
            if !satisfied {
                failures.push(format!(
                    "{} artifact group `{}` is unsatisfied ({} of {} present, cardinality {:?})",
                    self.id,
                    group.id,
                    present,
                    group.paths.len(),
                    group.cardinality
                ));
            }
        }
        failures
    }

    fn resolved_checks(&self) -> Result<Vec<ResolvedCheck>, String> {
        match &self.binding {
            ManifestBinding::Standalone { manifest } => manifest
                .resolve()
                .map(|bindings| bindings.into_values().flatten().collect())
                .map_err(|error| error.to_string()),
            ManifestBinding::Overlay { overlay, .. } => overlay
                .overlay
                .checks
                .as_ref()
                .into_iter()
                .flat_map(|checks| checks.values().flatten())
                .map(|check| {
                    crate::planner::capability_catalog::resolve(&check.id, &check.params)
                        .map(|capability| ResolvedCheck {
                            id: check.id.clone(),
                            phases: check.phases.clone(),
                            capability,
                        })
                        .map_err(|error| error.to_string())
                })
                .collect(),
        }
    }

    fn check_failures(&self, root: &Path, goal: &str) -> Vec<String> {
        let checks = match self.resolved_checks() {
            Ok(checks) => checks,
            Err(error) => return vec![format!("{} manifest checks are invalid: {error}", self.id)],
        };
        let mut failures = Vec::new();
        for check in checks {
            match check.capability {
                ResolvedCapability::ShellCheck(_) => {
                    // Shell checks enter the existing normalized verification
                    // boundary through `quality_expectations` below.
                }
                ResolvedCapability::Internal(InternalCapability::ScaffoldFilesPresent {
                    files,
                }) => {
                    for path in files {
                        if !root.join(&path).is_file() {
                            failures.push(format!(
                                "{} manifest check `{}` failed: missing `{path}`",
                                self.id, check.id
                            ));
                        }
                    }
                }
                ResolvedCapability::Internal(InternalCapability::Data(adapter)) => {
                    match crate::planner::profiles::data::internal_checks::execute(
                        root,
                        adapter,
                        Some(goal),
                    ) {
                        Ok((true, _)) => {}
                        Ok((false, reasons)) => failures.push(format!(
                            "{} manifest check `{}` failed: {}",
                            self.id,
                            check.id,
                            reasons.join("; ")
                        )),
                        Err(error) => failures.push(format!(
                            "{} manifest check `{}` failed to execute: {error:#}",
                            self.id, check.id
                        )),
                    }
                }
                ResolvedCapability::Internal(InternalCapability::Pack(adapter)) => {
                    match crate::planner::pack::checks::execute(root, &adapter) {
                        Ok(result) if result.passed => {}
                        Ok(result) => failures.push(format!(
                            "{} manifest check `{}` failed: {}",
                            self.id,
                            check.id,
                            result.reasons.join("; ")
                        )),
                        Err(error) => failures.push(format!(
                            "{} manifest check `{}` failed to execute: {error:#}",
                            self.id, check.id
                        )),
                    }
                }
                ResolvedCapability::Internal(_) | ResolvedCapability::Probe(_) => {
                    failures.push(format!(
                        "{} manifest check `{}` requires a profile-specific runtime adapter",
                        self.id, check.id
                    ));
                }
            }
        }
        failures
    }
}

impl DomainProfile for ManifestDrivenProfile {
    fn id(&self) -> &'static str {
        self.id
    }

    fn matches(&self, profile: &str) -> bool {
        crate::planner::profile::canonical_profile_name(profile) == self.id
    }

    fn expected_scaffold_paths(&self, root: &Path, goal: &str) -> Vec<String> {
        let mut paths = self
            .base_domain()
            .map(|base| base.expected_scaffold_paths(root, goal))
            .unwrap_or_default();
        for path in self.added_artifacts() {
            if !paths.contains(&path) {
                paths.push(path);
            }
        }
        paths
    }

    fn setup_scaffold_paths(&self, root: &Path) -> Vec<String> {
        self.base_domain()
            .map(|base| base.setup_scaffold_paths(root))
            .unwrap_or_default()
    }

    fn complete_scaffold(
        &self,
        root: &Path,
        missing_paths: &[String],
    ) -> anyhow::Result<Vec<String>> {
        match self.base_domain() {
            Some(base) => base.complete_scaffold(root, missing_paths),
            None => Ok(Vec::new()),
        }
    }

    fn verify_final(&self, root: &Path, goal: &str) -> VerificationReport {
        let mut report = self
            .base_domain()
            .map(|base| base.verify_final(root, goal))
            .unwrap_or_else(VerificationReport::pass);
        for failure in self.artifact_failures(root) {
            report.push_profile_failure(failure);
        }
        for failure in self.check_failures(root, goal) {
            report.push_profile_failure(failure);
        }
        report
    }

    fn verify_invariant(
        &self,
        root: &Path,
        goal: &str,
        snapshot: &ProfileSnapshot,
    ) -> VerificationReport {
        self.base_domain()
            .map(|base| base.verify_invariant(root, goal, snapshot))
            .unwrap_or_else(VerificationReport::pass)
    }

    fn before_phase(&self, root: &Path) -> anyhow::Result<ProfileSnapshot> {
        match self.base_domain() {
            Some(base) => base.before_phase(root),
            None => Ok(ProfileSnapshot::None),
        }
    }

    fn before_fix_phase(&self, root: &Path) -> anyhow::Result<ProfileSnapshot> {
        match self.base_domain() {
            Some(base) => base.before_fix_phase(root),
            None => Ok(ProfileSnapshot::None),
        }
    }

    fn after_phase(&self, root: &Path, snapshot: &ProfileSnapshot) -> VerificationReport {
        self.base_domain()
            .map(|base| base.after_phase(root, snapshot))
            .unwrap_or_else(VerificationReport::pass)
    }

    fn guidance(&self, goal: &str) -> Option<String> {
        let mut lines = self
            .base_domain()
            .and_then(|base| base.guidance(goal))
            .map(|text| vec![text])
            .unwrap_or_default();
        lines.extend(self.added_guidance());
        (!lines.is_empty()).then(|| lines.join("\n"))
    }

    fn deterministic_step_plan(
        &self,
        phase_prompt: &str,
        root: &Path,
        goal: &str,
    ) -> Option<ProfileDeterministicStepPlan> {
        self.base_domain()?
            .deterministic_step_plan(phase_prompt, root, goal)
    }

    fn preset_ultra_plan(&self, goal: &str, style: &str, intent: &str) -> Option<UltraPlan> {
        let manifest = self.manifest();
        if !style.eq_ignore_ascii_case(&manifest.plan.style)
            || !intent.eq_ignore_ascii_case(&manifest.plan.intent)
        {
            return None;
        }
        Some(UltraPlan {
            goal: goal.to_string(),
            profile: self.id.to_string(),
            style: manifest.plan.style.clone(),
            intent: manifest.plan.intent.clone(),
            phases: manifest
                .plan
                .phases
                .iter()
                .map(|phase| UltraPhase {
                    id: phase.id.clone(),
                    prompt: phase.prompt.replace("{goal}", goal),
                })
                .collect(),
        })
    }

    fn runtime_contract(&self, intent: &str, goal: &str) -> String {
        let mut lines = self
            .base_domain()
            .map(|base| base.runtime_contract(intent, goal))
            .unwrap_or_else(|| {
                "- Keep changes scoped to the current phase and workspace.".to_string()
            });
        for path in self.added_artifacts() {
            lines.push_str(&format!("\n- Produce the declared artifact `{path}`."));
        }
        lines
    }

    fn generation_rules(&self, intent: &str) -> Option<&'static str> {
        self.base_domain()?.generation_rules(intent)
    }

    fn hidden_path_continuation(&self) -> Option<&'static str> {
        self.base_domain()?.hidden_path_continuation()
    }

    fn quality_expectations(&self, root: &Path, goal: &str) -> ProfileQualityExpectations {
        let mut expectations = self
            .base_domain()
            .map(|base| base.quality_expectations(root, goal))
            .unwrap_or_default();
        for path in self.added_artifacts() {
            if !expectations.required_artifacts.contains(&path) {
                expectations.required_artifacts.push(path);
            }
        }
        if let Ok(checks) = self.resolved_checks() {
            for command in checks
                .into_iter()
                .filter_map(|check| match check.capability {
                    ResolvedCapability::ShellCheck(command) => Some(command),
                    _ => None,
                })
            {
                if !expectations.preferred_verify.contains(&command) {
                    expectations.preferred_verify.push(command);
                }
            }
        }
        expectations
    }

    fn repair_prompt(
        &self,
        root: &Path,
        goal: &str,
        report: &VerificationReport,
    ) -> Option<String> {
        self.base_domain()?.repair_prompt(root, goal, report)
    }

    fn interaction_repair_guidance(
        &self,
        failure_kind: &str,
        contract: &InteractionRepairContract,
    ) -> Vec<String> {
        self.base_domain()
            .map(|base| base.interaction_repair_guidance(failure_kind, contract))
            .unwrap_or_default()
    }

    fn deterministic_repair(
        &self,
        root: &Path,
        goal: &str,
        report: &VerificationReport,
    ) -> anyhow::Result<bool> {
        match self.base_domain() {
            Some(base) => base.deterministic_repair(root, goal, report),
            None => Ok(false),
        }
    }

    fn post_step_repair(&self, root: &Path, goal: &str) -> anyhow::Result<bool> {
        match self.base_domain() {
            Some(base) => base.post_step_repair(root, goal),
            None => Ok(false),
        }
    }

    fn build_oracle(&self, command: &str) -> Option<ProfileBuildOracle> {
        self.base_domain()?.build_oracle(command)
    }

    fn dependency_ready(&self, root: &Path, command: &str) -> bool {
        self.base_domain()
            .map(|base| base.dependency_ready(root, command))
            .unwrap_or(true)
    }

    fn dependency_missing_reason(&self, root: &Path, command: &str) -> String {
        self.base_domain()
            .map(|base| base.dependency_missing_reason(root, command))
            .unwrap_or_else(|| format!("dependency setup missing before `{command}`"))
    }

    fn dependency_setup_requirement(
        &self,
        root: &Path,
        requirement: &BuildVerifierRequirement,
        setup_authority: NodeDependencySetupAuthority,
    ) -> Option<NodeDependencySetupRequirement> {
        self.base_domain()?
            .dependency_setup_requirement(root, requirement, setup_authority)
    }

    fn dependency_missing_output(&self, output: &str) -> bool {
        self.base_domain()
            .is_some_and(|base| base.dependency_missing_output(output))
    }

    fn parse_compile_errors(&self, output: &FullCommandOutput) -> Vec<CompileError> {
        self.base_domain()
            .map(|base| base.parse_compile_errors(output))
            .unwrap_or_default()
    }

    fn annotate_compile_errors(&self, root: &Path, errors: &mut [CompileError]) {
        if let Some(base) = self.base_domain() {
            base.annotate_compile_errors(root, errors);
        }
    }

    fn foreign_toolchain(
        &self,
        root: &Path,
        requirement: &BuildVerifierRequirement,
    ) -> Option<ForeignToolchainObservation> {
        self.base_domain()?.foreign_toolchain(root, requirement)
    }

    fn source_paths(&self, root: &Path) -> Vec<String> {
        self.base_domain()
            .map(|base| base.source_paths(root))
            .unwrap_or_default()
    }

    fn evidence_repair_target_paths(&self, root: &Path, evidence_keys: &[String]) -> Vec<String> {
        let mut paths = self
            .base_domain()
            .map(|base| base.evidence_repair_target_paths(root, evidence_keys))
            .unwrap_or_default();
        let mappings = match &self.binding {
            ManifestBinding::Standalone { manifest } => Some(&manifest.evidence_targets.mappings),
            ManifestBinding::Overlay { overlay, .. } => overlay
                .overlay
                .evidence_targets
                .as_ref()
                .map(|targets| &targets.mappings),
        };
        if let Some(mappings) = mappings {
            for key in evidence_keys {
                for path in mappings.get(key).into_iter().flatten() {
                    if !paths.contains(path) {
                        paths.push(path.clone());
                    }
                }
            }
        }
        paths
    }

    fn hook_snapshot_targets(&self, root: &Path, goal: &str) -> Vec<ProfileHookSnapshotTarget> {
        self.base_domain()
            .map(|base| base.hook_snapshot_targets(root, goal))
            .unwrap_or_default()
    }

    fn infer_required_capabilities(&self, goal: &str) -> Vec<String> {
        self.base_domain()
            .map(|base| base.infer_required_capabilities(goal))
            .unwrap_or_default()
    }

    fn infer_required_evidence(&self, goal: &str, required_capabilities: &[String]) -> Vec<String> {
        self.base_domain()
            .map(|base| base.infer_required_evidence(goal, required_capabilities))
            .unwrap_or_default()
    }

    fn infer_required_obligations(
        &self,
        goal: &str,
        required_capabilities: &[String],
    ) -> Vec<String> {
        self.base_domain()
            .map(|base| base.infer_required_obligations(goal, required_capabilities))
            .unwrap_or_default()
    }

    fn completion_contract_required(&self, goal: &str, required_capabilities: &[String]) -> bool {
        self.base_domain()
            .is_some_and(|base| base.completion_contract_required(goal, required_capabilities))
    }

    fn behavior_probe(
        &self,
        root: &Path,
        goal: &str,
        required_capabilities: &[String],
        offline: bool,
    ) -> anyhow::Result<ProfileBehaviorProbeReport> {
        match self.base_domain() {
            Some(base) => base.behavior_probe(root, goal, required_capabilities, offline),
            None => Ok(ProfileBehaviorProbeReport::pass()),
        }
    }

    fn fix_reproducer_suggestion(&self, goal: &str) -> Option<ProfileFixReproducerSuggestion> {
        self.base_domain()?.fix_reproducer_suggestion(goal)
    }

    fn run_fix_reproducer_catalog_check(
        &self,
        root: &Path,
        goal: &str,
        command: &str,
        eval_events_path: Option<&Path>,
    ) -> Option<ProfileFixReproducerObservation> {
        self.base_domain()?
            .run_fix_reproducer_catalog_check(root, goal, command, eval_events_path)
    }

    fn fix_regression_bindings(&self, root: &Path, goal: &str) -> Vec<ProfileFixRegressionBinding> {
        self.base_domain()
            .map(|base| base.fix_regression_bindings(root, goal))
            .unwrap_or_else(|| {
                vec![ProfileFixRegressionBinding {
                    id: "profile_contract".to_string(),
                    adapter: ProfileFixRegressionAdapter::ProfileContract,
                }]
            })
    }

    fn run_fix_regressions(
        &self,
        root: &Path,
        goal: &str,
        bindings: &[ProfileFixRegressionBinding],
        offline: bool,
    ) -> Vec<ProfileFixRegressionObservation> {
        match self.base_domain() {
            Some(base) => base.run_fix_regressions(root, goal, bindings, offline),
            None => bindings
                .iter()
                .map(|binding| ProfileFixRegressionObservation {
                    id: binding.id.clone(),
                    outcome: if self.verify_final(root, goal).is_pass() {
                        ProbeOutcome::Success
                    } else {
                        ProbeOutcome::Failure
                    },
                    reason: self.verify_final(root, goal).primary_reason(),
                })
                .collect(),
        }
    }
}

impl ProfileRuntime for ManifestDrivenProfile {
    fn profile_id(&self) -> ProfileId {
        ProfileId::parse(self.id)
    }

    fn required_capabilities(&self, goal: &str) -> Vec<String> {
        match self.base_runtime() {
            Some(base) => base.required_capabilities(goal),
            None => self.infer_required_capabilities(goal),
        }
    }

    fn required_evidence(&self, goal: &str, required_capabilities: &[String]) -> Vec<String> {
        match self.base_runtime() {
            Some(base) => base.required_evidence(goal, required_capabilities),
            None => {
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
        }
    }

    fn interactive_app_capabilities(&self, profile_id: &ProfileId) -> Vec<String> {
        match self.base_runtime() {
            Some(base) => base.interactive_app_capabilities(self.base_profile_id(profile_id)),
            None => Vec::new(),
        }
    }

    fn browser_release_gate_profile(&self) -> bool {
        self.base_runtime()
            .is_some_and(ProfileRuntime::browser_release_gate_profile)
    }

    fn invariant_expected_paths(&self, root: &Path, paths: Vec<String>) -> Vec<String> {
        match self.base_runtime() {
            Some(base) => base.invariant_expected_paths(root, paths),
            None => paths,
        }
    }

    fn invariant_setup_paths(&self, root: &Path) -> Vec<String> {
        self.base_runtime()
            .map(|base| base.invariant_setup_paths(root))
            .unwrap_or_default()
    }

    fn verify_phase_invariant(
        &self,
        root: &Path,
        goal: &str,
        snapshot: &ProfileSnapshot,
    ) -> VerificationReport {
        match self.base_runtime() {
            Some(base) => base.verify_phase_invariant(root, goal, snapshot),
            None => VerificationReport::pass(),
        }
    }

    /// Earned assurance is never invented here. The shared admission gate caps
    /// every externally supplied profile at `static` with `profile_not_admitted`.
    fn assurance_for_completion(
        &self,
        profile_id: &ProfileId,
        required_capabilities: &[String],
    ) -> (&'static str, &'static str) {
        match self.base_runtime() {
            Some(base) => base
                .assurance_for_completion(self.base_profile_id(profile_id), required_capabilities),
            None => ("full", ""),
        }
    }

    fn apply_completion_snapshot(
        &self,
        profile_id: &ProfileId,
        root: &Path,
        snapshot: &mut crate::eval_events::CompletionSnapshot,
    ) {
        match self.base_runtime() {
            Some(base) => {
                base.apply_completion_snapshot(self.base_profile_id(profile_id), root, snapshot)
            }
            None => crate::completion_metadata::apply_full_snapshot(snapshot),
        }
    }

    fn apply_completion_projection(
        &self,
        profile_id: &ProfileId,
        root: &Path,
        projection: &mut crate::eval_events::CompletionProjection,
    ) {
        if let Some(base) = self.base_runtime() {
            base.apply_completion_projection(self.base_profile_id(profile_id), root, projection);
        }
    }

    fn default_requested_port(&self) -> Option<u16> {
        self.base_runtime()?.default_requested_port()
    }

    fn route_bound_closure(&self, root: &Path) -> std::collections::BTreeSet<std::path::PathBuf> {
        match self.base_runtime() {
            Some(base) => base.route_bound_closure(root),
            None => crate::minimal_loop::import_scan::all_route_source_files(root),
        }
    }

    fn run_behavior_probe(
        &self,
        profile_id: &ProfileId,
        root: &Path,
        goal: &str,
        required_capabilities: &[String],
        offline: bool,
    ) -> anyhow::Result<ProfileBehaviorProbeReport> {
        match self.base_runtime() {
            Some(base) => base.run_behavior_probe(
                self.base_profile_id(profile_id),
                root,
                goal,
                required_capabilities,
                offline,
            ),
            None => self.behavior_probe(root, goal, required_capabilities, offline),
        }
    }

    fn run_final_testimony_check(
        &self,
        root: &Path,
        browser_readiness_path: Option<&str>,
        interaction_evidence_path: Option<&str>,
    ) -> anyhow::Result<Option<ProfileBehaviorProbeReport>> {
        match self.base_runtime() {
            Some(base) => base.run_final_testimony_check(
                root,
                browser_readiness_path,
                interaction_evidence_path,
            ),
            None => Ok(None),
        }
    }

    fn canonicalize_create_plan(
        &self,
        plan: &mut crate::planner::step_plan::StepPlan,
        create_intent: bool,
        terminal_plan: bool,
        eval_events_path: Option<&Path>,
    ) -> usize {
        match self.base_runtime() {
            Some(base) => {
                base.canonicalize_create_plan(plan, create_intent, terminal_plan, eval_events_path)
            }
            None => 0,
        }
    }

    fn bind_empty_fix_verify_steps(
        &self,
        plan: &mut crate::planner::step_plan::StepPlan,
        phase_label: Option<&str>,
        eval_events_path: Option<&Path>,
    ) -> usize {
        match self.base_runtime() {
            Some(base) => base.bind_empty_fix_verify_steps(plan, phase_label, eval_events_path),
            None => 0,
        }
    }

    fn convert_preset_phase_setup_steps(
        &self,
        plan: &mut crate::planner::step_plan::StepPlan,
        root: &Path,
        goal: &str,
        phase_scope: Option<(&str, bool)>,
        preset_phase: bool,
        eval_events_path: Option<&Path>,
    ) -> usize {
        match self.base_runtime() {
            Some(base) => base.convert_preset_phase_setup_steps(
                plan,
                root,
                goal,
                phase_scope,
                preset_phase,
                eval_events_path,
            ),
            None => 0,
        }
    }

    fn runtime_step_with_profile_checks(
        &self,
        root: &Path,
        goal: &str,
        step: &crate::planner::step_plan::PlanStep,
        phase_id: Option<&str>,
        eval_events_path: Option<&Path>,
    ) -> (crate::planner::step_plan::PlanStep, bool) {
        match self.base_runtime() {
            Some(base) => {
                base.runtime_step_with_profile_checks(root, goal, step, phase_id, eval_events_path)
            }
            None => (step.clone(), false),
        }
    }

    fn pre_satisfied_verify_first(
        &self,
        root: &Path,
        step: &crate::planner::step_plan::PlanStep,
    ) -> Option<bool> {
        self.base_runtime()?.pre_satisfied_verify_first(root, step)
    }

    fn step_short_circuit_precheck_applicable(
        &self,
        step: &crate::planner::step_plan::PlanStep,
    ) -> bool {
        match self.base_runtime() {
            Some(base) => base.step_short_circuit_precheck_applicable(step),
            None => {
                crate::planner::setup_step_policy::profile_independent_short_circuit_precheck(step)
            }
        }
    }

    fn fallback_setup_plan(
        &self,
        root: &Path,
        goal: &str,
    ) -> Option<crate::planner::step_plan::StepPlan> {
        self.base_runtime()?.fallback_setup_plan(root, goal)
    }

    fn default_plan_preset(
        &self,
        intent: Option<crate::planner::adjudication::contract::IntentId>,
    ) -> Option<(crate::config::PlanPreset, &'static str)> {
        self.base_runtime()
            .and_then(|base| base.default_plan_preset(intent))
            .or(Some((
                crate::config::PlanPreset::Profile,
                "profile:extension-manifest",
            )))
    }

    fn inject_step_material(
        &self,
        config: &crate::config::Config,
        step: &mut crate::planner::step_plan::PlanStep,
    ) -> anyhow::Result<()> {
        match self.base_runtime() {
            Some(base) => base.inject_step_material(config, step),
            None => Ok(()),
        }
    }

    fn synthesizes_fix_plan(&self) -> bool {
        self.base_runtime()
            .is_some_and(ProfileRuntime::synthesizes_fix_plan)
    }

    fn synthesizes_investigation_plan(&self) -> bool {
        self.base_runtime()
            .is_some_and(ProfileRuntime::synthesizes_investigation_plan)
    }

    fn synthesizes_create_plan(&self) -> bool {
        self.base_runtime()
            .is_some_and(ProfileRuntime::synthesizes_create_plan)
    }

    fn enforce_nextjs_plan_shape(&self) -> bool {
        self.base_runtime()
            .is_some_and(ProfileRuntime::enforce_nextjs_plan_shape)
    }

    fn dependency_reconciliation_requirement(
        &self,
        root: &Path,
        profile_id: &ProfileId,
        manifest_changed: bool,
        reason: &str,
        authority: NodeDependencySetupAuthority,
    ) -> Option<NodeDependencySetupRequirement> {
        self.base_runtime()?.dependency_reconciliation_requirement(
            root,
            self.base_profile_id(profile_id),
            manifest_changed,
            reason,
            authority,
        )
    }

    fn release_recovery_verify_commands(
        &self,
        reasons: &[String],
        probe_infrastructure_failure: bool,
    ) -> Vec<String> {
        match self.base_runtime() {
            Some(base) => {
                base.release_recovery_verify_commands(reasons, probe_infrastructure_failure)
            }
            None => vec!["rerun deterministic acceptance checks for the original goal".to_string()],
        }
    }

    fn filter_invariant_expected_paths(&self, root: &Path, paths: Vec<String>) -> Vec<String> {
        match self.base_runtime() {
            Some(base) => base.filter_invariant_expected_paths(root, paths),
            None => paths,
        }
    }

    fn invariant_relevant_paths(&self, root: &Path, reason: &str) -> Vec<std::path::PathBuf> {
        self.base_runtime()
            .map(|base| base.invariant_relevant_paths(root, reason))
            .unwrap_or_default()
    }

    fn styling_choice_rule(&self) -> &'static str {
        self.base_runtime()
            .map(ProfileRuntime::styling_choice_rule)
            .unwrap_or("")
    }

    fn route_bound_constraint(&self) -> &'static str {
        self.base_runtime()
            .map(ProfileRuntime::route_bound_constraint)
            .unwrap_or("")
    }

    fn is_entrypoint_scaffold_path(&self, path: &str) -> bool {
        self.base_runtime()
            .is_some_and(|base| base.is_entrypoint_scaffold_path(path))
    }
}
