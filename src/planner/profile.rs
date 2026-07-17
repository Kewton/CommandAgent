use std::path::Path;

use crate::minimal_loop::build_verifier::{
    BuildVerifierRequirement, CompileError, ForeignToolchainObservation, FullCommandOutput,
};
use crate::minimal_loop::dependency_setup::{
    self, NodeDependencySetupAuthority, NodeDependencySetupRequirement,
};
use crate::minimal_loop::evidence::required_evidence_for_capability;
use crate::planner::adjudication::contract::{ProbeOutcome, is_fix_intent};
use crate::planner::signals;
use crate::planner::step_plan::StepPlan;
use crate::planner::ultra_plan::UltraPlan;
use crate::planner::verify::{NormalizedVerifyCommand, VerificationReport};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfileQualityExpectations {
    pub required_artifacts: Vec<String>,
    pub preferred_verify: Vec<String>,
    pub forbidden_verify: Vec<String>,
    pub dependency_order_hint: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ProfileSnapshot {
    Data(crate::planner::profiles::data::ProfileSnapshot),
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseVerificationMode {
    IntermediateInvariant,
    FinalAcceptance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileHookSnapshotTarget {
    pub relative_path: String,
    pub required_attributes: Vec<ProfileHookAttribute>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ProfileHookAttribute {
    PrimaryAction,
    RestartAction,
    State,
}

impl ProfileHookAttribute {
    pub fn display(self) -> &'static str {
        match self {
            Self::PrimaryAction => "data-anvil-action=\"primary\"",
            Self::RestartAction => "data-anvil-action=\"restart\"",
            Self::State => "data-anvil-state",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileBuildOracle {
    pub command: String,
    pub profile: Option<String>,
    pub requires_dependency_setup: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileBehaviorProbeReport {
    pub status: &'static str,
    pub reasons: Vec<String>,
    pub evidence_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileFixRegressionAdapter {
    VerifyCommand(String),
    ProfileContract,
    DataManifestCheck,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileFixRegressionBinding {
    pub id: String,
    pub adapter: ProfileFixRegressionAdapter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileFixRegressionObservation {
    pub id: String,
    pub outcome: ProbeOutcome,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileFixReproducerSuggestion {
    pub basis: String,
    pub suggestion: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileFixReproducerObservation {
    pub outcome: ProbeOutcome,
    pub reason: String,
}

impl ProfileBehaviorProbeReport {
    pub fn pass() -> Self {
        Self {
            status: "pass",
            reasons: Vec::new(),
            evidence_path: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileDeterministicStepPlan {
    pub template_id: String,
    pub plan: StepPlan,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InteractionRepairContract {
    pub required_capabilities: Vec<String>,
    pub required_evidence: Vec<String>,
}

pub trait DomainProfile: Sync {
    fn id(&self) -> &'static str;

    fn matches(&self, profile: &str) -> bool {
        canonical_profile_name(profile) == self.id()
    }

    fn expected_scaffold_paths(&self, _root: &Path, _goal: &str) -> Vec<String> {
        Vec::new()
    }

    fn setup_scaffold_paths(&self, _root: &Path) -> Vec<String> {
        Vec::new()
    }

    fn complete_scaffold(
        &self,
        _root: &Path,
        _missing_paths: &[String],
    ) -> anyhow::Result<Vec<String>> {
        Ok(Vec::new())
    }

    fn verify_final(&self, _root: &Path, _goal: &str) -> VerificationReport {
        VerificationReport::pass()
    }

    fn verify_invariant(
        &self,
        _root: &Path,
        _goal: &str,
        _snapshot: &ProfileSnapshot,
    ) -> VerificationReport {
        VerificationReport::pass()
    }

    fn before_phase(&self, _root: &Path) -> anyhow::Result<ProfileSnapshot> {
        Ok(ProfileSnapshot::None)
    }

    fn before_fix_phase(&self, _root: &Path) -> anyhow::Result<ProfileSnapshot> {
        Ok(ProfileSnapshot::None)
    }

    fn after_phase(&self, _root: &Path, _snapshot: &ProfileSnapshot) -> VerificationReport {
        VerificationReport::pass()
    }

    fn guidance(&self, _goal: &str) -> Option<String> {
        None
    }

    fn deterministic_step_plan(
        &self,
        _phase_prompt: &str,
        _root: &Path,
        _goal: &str,
    ) -> Option<ProfileDeterministicStepPlan> {
        None
    }

    fn preset_ultra_plan(&self, _goal: &str, _style: &str, _intent: &str) -> Option<UltraPlan> {
        None
    }

    fn runtime_contract(&self, _intent: &str, _goal: &str) -> String {
        "- Keep changes scoped to the current phase and workspace.".to_string()
    }

    fn generation_rules(&self, _intent: &str) -> Option<&'static str> {
        None
    }

    fn hidden_path_continuation(&self) -> Option<&'static str> {
        None
    }

    fn quality_expectations(&self, _root: &Path, _goal: &str) -> ProfileQualityExpectations {
        ProfileQualityExpectations::default()
    }

    fn repair_prompt(
        &self,
        _root: &Path,
        _goal: &str,
        _report: &VerificationReport,
    ) -> Option<String> {
        None
    }

    fn interaction_repair_guidance(
        &self,
        _failure_kind: &str,
        _contract: &InteractionRepairContract,
    ) -> Vec<String> {
        Vec::new()
    }

    fn deterministic_repair(
        &self,
        _root: &Path,
        _goal: &str,
        _report: &VerificationReport,
    ) -> anyhow::Result<bool> {
        Ok(false)
    }

    fn post_step_repair(&self, _root: &Path, _goal: &str) -> anyhow::Result<bool> {
        Ok(false)
    }

    fn build_oracle(&self, _command: &str) -> Option<ProfileBuildOracle> {
        None
    }

    fn dependency_ready(&self, _root: &Path, _command: &str) -> bool {
        true
    }

    fn dependency_missing_reason(&self, _root: &Path, command: &str) -> String {
        format!("dependency setup missing before `{command}`")
    }

    fn dependency_setup_requirement(
        &self,
        _root: &Path,
        _requirement: &BuildVerifierRequirement,
        _setup_authority: NodeDependencySetupAuthority,
    ) -> Option<NodeDependencySetupRequirement> {
        None
    }

    fn dependency_missing_output(&self, _output: &str) -> bool {
        false
    }

    fn parse_compile_errors(&self, _output: &FullCommandOutput) -> Vec<CompileError> {
        Vec::new()
    }

    fn annotate_compile_errors(&self, _root: &Path, _errors: &mut [CompileError]) {}

    fn foreign_toolchain(
        &self,
        _root: &Path,
        _requirement: &BuildVerifierRequirement,
    ) -> Option<ForeignToolchainObservation> {
        None
    }

    fn source_paths(&self, _root: &Path) -> Vec<String> {
        Vec::new()
    }

    fn evidence_repair_target_paths(&self, _root: &Path, _evidence_keys: &[String]) -> Vec<String> {
        Vec::new()
    }

    fn hook_snapshot_targets(&self, _root: &Path, _goal: &str) -> Vec<ProfileHookSnapshotTarget> {
        Vec::new()
    }

    fn infer_required_capabilities(&self, _goal: &str) -> Vec<String> {
        Vec::new()
    }

    fn infer_required_evidence(
        &self,
        _goal: &str,
        _required_capabilities: &[String],
    ) -> Vec<String> {
        Vec::new()
    }

    fn infer_required_obligations(
        &self,
        _goal: &str,
        _required_capabilities: &[String],
    ) -> Vec<String> {
        Vec::new()
    }

    fn completion_contract_required(&self, _goal: &str, _required_capabilities: &[String]) -> bool {
        false
    }

    fn behavior_probe(
        &self,
        _root: &Path,
        _goal: &str,
        _required_capabilities: &[String],
        _offline: bool,
    ) -> anyhow::Result<ProfileBehaviorProbeReport> {
        Ok(ProfileBehaviorProbeReport::pass())
    }

    fn fix_reproducer_suggestion(&self, _goal: &str) -> Option<ProfileFixReproducerSuggestion> {
        None
    }

    fn run_fix_reproducer_catalog_check(
        &self,
        _root: &Path,
        _goal: &str,
        _command: &str,
        _eval_events_path: Option<&Path>,
    ) -> Option<ProfileFixReproducerObservation> {
        None
    }

    fn fix_regression_bindings(&self, root: &Path, goal: &str) -> Vec<ProfileFixRegressionBinding> {
        let mut bindings = vec![ProfileFixRegressionBinding {
            id: "profile_contract".to_string(),
            adapter: ProfileFixRegressionAdapter::ProfileContract,
        }];
        bindings.extend(
            self.quality_expectations(root, goal)
                .preferred_verify
                .into_iter()
                .enumerate()
                .map(|(index, command)| ProfileFixRegressionBinding {
                    id: format!("profile_verify_{}", index + 1),
                    adapter: ProfileFixRegressionAdapter::VerifyCommand(command),
                }),
        );
        bindings
    }

    fn run_fix_regressions(
        &self,
        root: &Path,
        goal: &str,
        bindings: &[ProfileFixRegressionBinding],
        offline: bool,
    ) -> Vec<ProfileFixRegressionObservation> {
        bindings
            .iter()
            .map(|binding| run_profile_fix_regression(self, root, goal, binding, offline))
            .collect()
    }
}

fn run_profile_fix_regression<P: DomainProfile + ?Sized>(
    profile: &P,
    root: &Path,
    goal: &str,
    binding: &ProfileFixRegressionBinding,
    offline: bool,
) -> ProfileFixRegressionObservation {
    let (outcome, reason) = match &binding.adapter {
        ProfileFixRegressionAdapter::ProfileContract => {
            let report = profile.verify_final(root, goal);
            if report.is_pass() {
                (ProbeOutcome::Success, String::new())
            } else {
                (ProbeOutcome::Failure, report.primary_reason())
            }
        }
        ProfileFixRegressionAdapter::VerifyCommand(command) => {
            let normalized: NormalizedVerifyCommand =
                match crate::planner::verify::normalize_verify_command(command) {
                    Ok(normalized) => normalized,
                    Err(err) => {
                        return ProfileFixRegressionObservation {
                            id: binding.id.clone(),
                            outcome: ProbeOutcome::Unavailable,
                            reason: format!("regression_command_rejected:{err}"),
                        };
                    }
                };
            match crate::minimal_loop::verifier_env::run_structured_for_verify_with_profile(
                &normalized,
                root,
                Some(profile.id()),
                offline,
            ) {
                Ok(observation) => match observation.kind {
                    crate::tools::bash::BashOutcomeKind::Success => {
                        (ProbeOutcome::Success, String::new())
                    }
                    crate::tools::bash::BashOutcomeKind::CommandFailed => (
                        ProbeOutcome::Failure,
                        crate::eval_events::body_snippet(
                            &crate::minimal_loop::verifier_env::format_verify_outcome(&observation),
                        ),
                    ),
                    crate::tools::bash::BashOutcomeKind::Blocked
                    | crate::tools::bash::BashOutcomeKind::Timeout
                    | crate::tools::bash::BashOutcomeKind::Cancelled => (
                        ProbeOutcome::Unavailable,
                        crate::eval_events::body_snippet(
                            &crate::minimal_loop::verifier_env::format_verify_outcome(&observation),
                        ),
                    ),
                },
                Err(err) => (
                    ProbeOutcome::Unavailable,
                    format!("regression_probe_error:{err}"),
                ),
            }
        }
        ProfileFixRegressionAdapter::DataManifestCheck => (
            ProbeOutcome::Unavailable,
            "data_manifest_adapter_not_dispatched".to_string(),
        ),
    };
    ProfileFixRegressionObservation {
        id: binding.id.clone(),
        outcome,
        reason,
    }
}

pub struct DataProfile;
pub struct GenericProfile;

pub const GENERIC_INTERACTIVE_CONTRACT_CAPABILITY: &str = "generic_interactive_contract";

static NEXTJS_PROFILE: crate::planner::profiles::nextjs::NextjsProfile =
    crate::planner::profiles::nextjs::NextjsProfile;
static DATA_PROFILE: DataProfile = DataProfile;
static PYTHON_CLI_PROFILE: crate::planner::profiles::python_cli::PythonCliProfile =
    crate::planner::profiles::python_cli::PythonCliProfile;
static GENERIC_PROFILE: GenericProfile = GenericProfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileInferenceSource {
    Goal,
    Workspace,
}

impl ProfileInferenceSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Goal => "goal",
            Self::Workspace => "workspace",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileInference {
    pub profile: &'static str,
    pub source: ProfileInferenceSource,
}

impl ProfileInference {
    pub fn summary_line(self) -> String {
        format!(
            "profile_inferred: {} (from: {})",
            self.profile,
            self.source.as_str()
        )
    }
}

pub fn infer_profile(goal: Option<&str>, workspace_root: &Path) -> Option<ProfileInference> {
    if let Some(goal) = goal {
        if signals::contains_nextjs_goal_token(goal) {
            return Some(ProfileInference {
                profile: NEXTJS_PROFILE.id(),
                source: ProfileInferenceSource::Goal,
            });
        }
        if signals::contains_python_cli_goal_token(goal) {
            return Some(ProfileInference {
                profile: PYTHON_CLI_PROFILE.id(),
                source: ProfileInferenceSource::Goal,
            });
        }
    }
    if package_json_has_dependency(workspace_root, "next") {
        return Some(ProfileInference {
            profile: NEXTJS_PROFILE.id(),
            source: ProfileInferenceSource::Workspace,
        });
    }
    if workspace_root.join("pyproject.toml").is_file() {
        return Some(ProfileInference {
            profile: PYTHON_CLI_PROFILE.id(),
            source: ProfileInferenceSource::Workspace,
        });
    }
    None
}

fn package_json_has_dependency(root: &Path, name: &str) -> bool {
    let Ok(text) = std::fs::read_to_string(root.join("package.json")) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return false;
    };
    ["dependencies", "devDependencies"]
        .iter()
        .filter_map(|key| value.get(*key).and_then(serde_json::Value::as_object))
        .any(|deps| deps.contains_key(name))
}

impl DomainProfile for DataProfile {
    fn id(&self) -> &'static str {
        "data"
    }

    fn matches(&self, profile: &str) -> bool {
        matches!(
            canonical_profile_name(profile).as_str(),
            "data" | "data-analysis" | "data-pipeline"
        )
    }

    fn verify_final(&self, root: &Path, _goal: &str) -> VerificationReport {
        crate::planner::profiles::data::verify(root)
    }

    fn expected_scaffold_paths(&self, _root: &Path, _goal: &str) -> Vec<String> {
        crate::planner::profiles::data::manifest::required_artifacts()
    }

    fn before_phase(&self, root: &Path) -> anyhow::Result<ProfileSnapshot> {
        let scaffold_paths = self.setup_scaffold_paths(root);
        if !scaffold_paths.is_empty() {
            let _ = self.complete_scaffold(root, &scaffold_paths)?;
        }
        Ok(ProfileSnapshot::Data(
            crate::planner::profiles::data::before_phase(root)?,
        ))
    }

    fn before_fix_phase(&self, root: &Path) -> anyhow::Result<ProfileSnapshot> {
        Ok(ProfileSnapshot::Data(
            crate::planner::profiles::data::before_phase(root)?,
        ))
    }

    fn after_phase(&self, root: &Path, snapshot: &ProfileSnapshot) -> VerificationReport {
        match snapshot {
            ProfileSnapshot::Data(snapshot) => {
                crate::planner::profiles::data::after_phase(root, snapshot)
            }
            _ => VerificationReport::pass(),
        }
    }

    fn guidance(&self, _goal: &str) -> Option<String> {
        Some(crate::planner::profiles::data::manifest::guidance())
    }

    fn preset_ultra_plan(&self, goal: &str, style: &str, intent: &str) -> Option<UltraPlan> {
        crate::planner::profiles::data::manifest::preset_ultra_plan(goal, style, intent)
    }

    fn runtime_contract(&self, _intent: &str, _goal: &str) -> String {
        crate::planner::profiles::data::manifest::runtime_contract()
    }

    fn generation_rules(&self, _intent: &str) -> Option<&'static str> {
        Some(crate::planner::profiles::data::manifest::generation_rules())
    }

    fn hidden_path_continuation(&self) -> Option<&'static str> {
        Some(crate::planner::profiles::data::manifest::guidance_message(
            "hidden_path",
            "continuation",
        ))
    }

    fn quality_expectations(&self, _root: &Path, _goal: &str) -> ProfileQualityExpectations {
        ProfileQualityExpectations {
            required_artifacts: crate::planner::profiles::data::manifest::required_artifacts(),
            preferred_verify: Vec::new(),
            forbidden_verify: vec!["pip install".to_string(), "python -m venv".to_string()],
            dependency_order_hint: Some(
                crate::planner::profiles::data::manifest::dependency_order_hint(),
            ),
        }
    }

    fn source_paths(&self, _root: &Path) -> Vec<String> {
        crate::planner::profiles::data::manifest::source_paths()
    }

    fn evidence_repair_target_paths(&self, _root: &Path, evidence_keys: &[String]) -> Vec<String> {
        crate::planner::profiles::data::manifest::evidence_target_paths(evidence_keys)
    }

    fn infer_required_capabilities(&self, _goal: &str) -> Vec<String> {
        crate::planner::profiles::data::manifest::required_capability_ids()
    }

    fn infer_required_obligations(
        &self,
        _goal: &str,
        _required_capabilities: &[String],
    ) -> Vec<String> {
        vec!["implementation".to_string()]
    }

    fn behavior_probe(
        &self,
        root: &Path,
        goal: &str,
        _required_capabilities: &[String],
        _offline: bool,
    ) -> anyhow::Result<ProfileBehaviorProbeReport> {
        let summary = crate::planner::profiles::data::runtime::run_manifest_checks_with_goal(
            root,
            Some(goal),
        )?;
        Ok(ProfileBehaviorProbeReport {
            status: summary.assurance.behavior_status(),
            reasons: summary.reasons,
            evidence_path: Some(
                crate::planner::profiles::data::runtime::DATA_ASSURANCE_EVIDENCE_PATH.to_string(),
            ),
        })
    }

    fn fix_reproducer_suggestion(&self, goal: &str) -> Option<ProfileFixReproducerSuggestion> {
        crate::planner::profiles::data::manifest::fix_reproducer::suggestion_for(goal)
    }

    fn run_fix_reproducer_catalog_check(
        &self,
        root: &Path,
        goal: &str,
        command: &str,
        eval_events_path: Option<&Path>,
    ) -> Option<ProfileFixReproducerObservation> {
        let mut report = VerificationReport::pass();
        let execution = crate::planner::profiles::data::step_policy::execute_catalog_check(
            root,
            command,
            &mut report,
            eval_events_path,
            Some(goal),
        )?;
        Some(match execution {
            Ok(observation) if observation.ok => ProfileFixReproducerObservation {
                outcome: ProbeOutcome::Success,
                reason: "command_succeeded".to_string(),
            },
            Ok(observation) => ProfileFixReproducerObservation {
                outcome: ProbeOutcome::Failure,
                reason: if observation.reasons.is_empty() {
                    format!("{}:check_failed", observation.id)
                } else {
                    observation.reasons.join("; ")
                },
            },
            Err(error) => ProfileFixReproducerObservation {
                outcome: ProbeOutcome::Unavailable,
                reason: format!("catalog_check_error:{error}"),
            },
        })
    }

    fn fix_regression_bindings(
        &self,
        _root: &Path,
        _goal: &str,
    ) -> Vec<ProfileFixRegressionBinding> {
        let mut ids = vec!["pipeline_probe".to_string()];
        ids.extend(crate::planner::profiles::data::manifest::required_capability_ids());
        ids.into_iter()
            .map(|id| ProfileFixRegressionBinding {
                id,
                adapter: ProfileFixRegressionAdapter::DataManifestCheck,
            })
            .collect()
    }

    fn run_fix_regressions(
        &self,
        root: &Path,
        goal: &str,
        bindings: &[ProfileFixRegressionBinding],
        _offline: bool,
    ) -> Vec<ProfileFixRegressionObservation> {
        match crate::planner::profiles::data::runtime::run_manifest_checks_with_goal(
            root,
            Some(goal),
        ) {
            Ok(summary) => bindings
                .iter()
                .map(|binding| {
                    let passed = summary.checks.get(&binding.id).copied();
                    ProfileFixRegressionObservation {
                        id: binding.id.clone(),
                        outcome: match passed {
                            Some(true) => ProbeOutcome::Success,
                            Some(false) => ProbeOutcome::Failure,
                            None => ProbeOutcome::Unavailable,
                        },
                        reason: if passed == Some(true) {
                            String::new()
                        } else {
                            summary
                                .reasons
                                .iter()
                                .find(|reason| reason.contains(&binding.id))
                                .cloned()
                                .unwrap_or_else(|| format!("{}:check_unavailable", binding.id))
                        },
                    }
                })
                .collect(),
            Err(err) => bindings
                .iter()
                .map(|binding| ProfileFixRegressionObservation {
                    id: binding.id.clone(),
                    outcome: ProbeOutcome::Unavailable,
                    reason: format!("data_regression_probe_error:{err}"),
                })
                .collect(),
        }
    }
}

impl DomainProfile for GenericProfile {
    fn id(&self) -> &'static str {
        "generic"
    }

    fn matches(&self, _profile: &str) -> bool {
        true
    }

    fn runtime_contract(&self, _intent: &str, _goal: &str) -> String {
        "- Keep changes scoped to the current phase and workspace.".to_string()
    }

    fn generation_rules(&self, profile: &str) -> Option<&'static str> {
        match profile {
            "rust" => Some(
                "- Profile rust: preserve Cargo project semantics. Keep Cargo.toml before cargo check/test verification, do not weaken scripts or tests to hide failures, and end with cargo check or cargo test when practical.\n",
            ),
            "python" => Some(
                "- Profile python: keep dependency setup separate from deterministic verification. Prefer python -m py_compile, pytest, or unittest checks after source files exist. Do not put package installation in verify commands.\n",
            ),
            _ => None,
        }
    }

    fn run_fix_regressions(
        &self,
        _root: &Path,
        _goal: &str,
        bindings: &[ProfileFixRegressionBinding],
        _offline: bool,
    ) -> Vec<ProfileFixRegressionObservation> {
        bindings
            .iter()
            .map(|binding| ProfileFixRegressionObservation {
                id: binding.id.clone(),
                outcome: ProbeOutcome::Unavailable,
                reason: "generic_profile_regression_contract_unavailable".to_string(),
            })
            .collect()
    }

    fn interaction_repair_guidance(
        &self,
        failure_kind: &str,
        _contract: &InteractionRepairContract,
    ) -> Vec<String> {
        crate::planner::profiles::nextjs::knowledge::generic_interaction_repair_guidance(
            failure_kind,
        )
    }

    fn build_oracle(&self, command: &str) -> Option<ProfileBuildOracle> {
        let lower = command.trim().to_ascii_lowercase();
        let recognized = requires_node_test_runner(command)
            || requires_node_dependency_probe(command)
            || lower == "cargo build"
            || lower.starts_with("cargo build ");
        recognized.then(|| ProfileBuildOracle {
            command: command.to_string(),
            profile: None,
            requires_dependency_setup: requires_node_test_runner(command)
                || requires_node_dependency_probe(command),
        })
    }

    fn dependency_ready(&self, root: &Path, command: &str) -> bool {
        if requires_node_test_runner(command) {
            dependency_setup::node_test_runner_bindable(root)
        } else if dependency_setup::package_json_declares_dependencies(root) {
            dependency_setup::node_declared_dependencies_ready(root)
        } else {
            true
        }
    }

    fn dependency_missing_reason(&self, root: &Path, command: &str) -> String {
        if requires_node_test_runner(command) {
            "package.json scripts.test missing before Node test verifier".to_string()
        } else if dependency_setup::package_json_declares_dependencies(root) {
            dependency_setup::node_declared_dependencies_missing_reason(root)
        } else {
            format!("dependency setup missing before `{command}`")
        }
    }

    fn dependency_setup_requirement(
        &self,
        root: &Path,
        requirement: &BuildVerifierRequirement,
        setup_authority: NodeDependencySetupAuthority,
    ) -> Option<NodeDependencySetupRequirement> {
        if requires_node_test_runner(&requirement.command) {
            return Some(dependency_setup::requirement_for_node_test_runner(
                root,
                requirement.profile.as_deref(),
                &requirement.reason,
                setup_authority,
            ));
        }
        Some(
            dependency_setup::requirement_for_node_declared_dependencies(
                root,
                requirement.profile.as_deref(),
                &requirement.reason,
                setup_authority,
            ),
        )
    }

    fn dependency_missing_output(&self, output: &str) -> bool {
        generic_dependency_missing_output(output)
    }

    fn infer_required_capabilities(&self, goal: &str) -> Vec<String> {
        if generic_app_intent_goal(goal) {
            vec![GENERIC_INTERACTIVE_CONTRACT_CAPABILITY.to_string()]
        } else {
            Vec::new()
        }
    }

    fn infer_required_evidence(&self, goal: &str, required_capabilities: &[String]) -> Vec<String> {
        if generic_app_intent_goal(goal)
            || required_capabilities
                .iter()
                .any(|capability| capability == GENERIC_INTERACTIVE_CONTRACT_CAPABILITY)
        {
            return vec![
                "user_input_handler_evidence".to_string(),
                "stateful_update_evidence".to_string(),
                "visible_interactive_surface_evidence".to_string(),
            ];
        }
        Vec::new()
    }

    fn infer_required_obligations(
        &self,
        goal: &str,
        required_capabilities: &[String],
    ) -> Vec<String> {
        if generic_app_intent_goal(goal)
            || required_capabilities
                .iter()
                .any(|capability| capability == GENERIC_INTERACTIVE_CONTRACT_CAPABILITY)
        {
            return vec!["implementation".to_string()];
        }
        Vec::new()
    }

    fn completion_contract_required(&self, goal: &str, required_capabilities: &[String]) -> bool {
        generic_app_intent_goal(goal)
            || required_capabilities
                .iter()
                .any(|capability| capability == GENERIC_INTERACTIVE_CONTRACT_CAPABILITY)
    }
}

fn generic_app_intent_goal(goal: &str) -> bool {
    let lower = goal.to_ascii_lowercase();
    signals::contains_app_intent_token(goal)
        && !signals::contains_nextjs_goal_token(goal)
        && !signals::contains_python_cli_goal_token(goal)
        && !generic_app_intent_excluded_by_internal_or_artifact_context(&lower)
}

fn generic_app_intent_excluded_by_internal_or_artifact_context(lower: &str) -> bool {
    [
        "app entrypoint",
        "src/app/",
        "required final artifacts:",
        "original ultra goal:",
        "phase id:",
        "phase task:",
        "failed phase:",
        "failed step:",
        "repair target:",
        "recovery handoff",
        "profile:",
    ]
    .iter()
    .any(|token| lower.contains(token))
}

pub fn domain_profile(profile: &str) -> &'static dyn DomainProfile {
    if NEXTJS_PROFILE.matches(profile) {
        &NEXTJS_PROFILE
    } else if PYTHON_CLI_PROFILE.matches(profile) {
        &PYTHON_CLI_PROFILE
    } else if DATA_PROFILE.matches(profile) {
        &DATA_PROFILE
    } else {
        &GENERIC_PROFILE
    }
}

pub fn build_oracle_for_command(
    profile: Option<&str>,
    command: &str,
) -> Option<(&'static dyn DomainProfile, ProfileBuildOracle)> {
    if let Some(profile_name) = profile {
        let profile = domain_profile(profile_name);
        if let Some(oracle) = profile.build_oracle(command) {
            return Some((profile, oracle));
        }
    }
    for profile in [
        &NEXTJS_PROFILE as &'static dyn DomainProfile,
        &GENERIC_PROFILE as &'static dyn DomainProfile,
    ] {
        if let Some(oracle) = profile.build_oracle(command) {
            return Some((profile, oracle));
        }
    }
    None
}

pub fn profile_for_build_requirement(
    requirement: &BuildVerifierRequirement,
) -> &'static dyn DomainProfile {
    if let Some(profile) = requirement.profile.as_deref() {
        return domain_profile(profile);
    }
    build_oracle_for_command(None, &requirement.command)
        .map(|(profile, _)| profile)
        .unwrap_or(&GENERIC_PROFILE)
}

pub fn canonical_profile_name(profile: &str) -> String {
    let normalized = profile.trim().to_ascii_lowercase();
    if let Some(canonical) =
        crate::planner::profiles::nextjs::canonical_profile_alias(normalized.as_str())
    {
        return canonical.to_string();
    }
    match normalized.as_str() {
        "python" | "python-cli" | "py-cli" | "py" => "python-cli".to_string(),
        other => other.to_string(),
    }
}

pub fn is_nextjs_profile(profile: &str) -> bool {
    NEXTJS_PROFILE.matches(profile)
}

pub fn verify_profile(root: &Path, profile: &str, goal: &str) -> VerificationReport {
    verify_profile_final(root, profile, goal)
}

pub fn verify_profile_final(root: &Path, profile: &str, goal: &str) -> VerificationReport {
    domain_profile(profile).verify_final(root, goal)
}

pub fn verify_profile_invariant(
    root: &Path,
    profile: &str,
    goal: &str,
    snapshot: &ProfileSnapshot,
) -> VerificationReport {
    let profile_impl = domain_profile(profile);
    let snapshot_report = profile_impl.after_phase(root, snapshot);
    if !snapshot_report.is_pass() {
        return snapshot_report;
    }
    profile_impl.verify_invariant(root, goal, snapshot)
}

pub fn profile_before_phase(root: &Path, profile: &str) -> anyhow::Result<ProfileSnapshot> {
    domain_profile(profile).before_phase(root)
}

pub fn profile_before_fix_phase(root: &Path, profile: &str) -> anyhow::Result<ProfileSnapshot> {
    domain_profile(profile).before_fix_phase(root)
}

pub fn profile_before_plan(root: &Path, plan: &UltraPlan) -> anyhow::Result<ProfileSnapshot> {
    if is_fix_intent(&plan.intent) {
        profile_before_fix_phase(root, &plan.profile)
    } else {
        profile_before_phase(root, &plan.profile)
    }
}

pub fn profile_fix_regression_bindings(
    root: &Path,
    profile: &str,
    goal: &str,
) -> Vec<ProfileFixRegressionBinding> {
    domain_profile(profile).fix_regression_bindings(root, goal)
}

pub fn run_profile_fix_regressions(
    root: &Path,
    profile: &str,
    goal: &str,
    bindings: &[ProfileFixRegressionBinding],
    offline: bool,
) -> Vec<ProfileFixRegressionObservation> {
    domain_profile(profile).run_fix_regressions(root, goal, bindings, offline)
}

pub fn profile_after_phase(
    root: &Path,
    profile: &str,
    snapshot: &ProfileSnapshot,
) -> VerificationReport {
    domain_profile(profile).after_phase(root, snapshot)
}

pub fn profile_guidance(profile: &str, goal: &str) -> Option<String> {
    domain_profile(profile).guidance(goal)
}

pub fn profile_deterministic_step_plan(
    root: &Path,
    profile: &str,
    phase_prompt: &str,
    goal: &str,
) -> Option<ProfileDeterministicStepPlan> {
    domain_profile(profile).deterministic_step_plan(phase_prompt, root, goal)
}

pub fn profile_preset_ultra_plan(
    profile: &str,
    goal: &str,
    style: &str,
    intent: &str,
) -> Option<UltraPlan> {
    domain_profile(profile).preset_ultra_plan(goal, style, intent)
}

pub fn profile_runtime_contract(profile: &str, intent: &str, goal: &str) -> String {
    match canonical_profile_name(profile).as_str() {
        "rust" => "- Preserve Cargo.toml and crate entrypoints.\n\
- Prefer cargo check or cargo test for deterministic verification.\n\
- Do not weaken tests or public behavior to hide failures."
            .to_string(),
        "python" => "- Preserve the existing Python package/import layout.\n\
- Keep dependency setup separate from deterministic verification.\n\
- Prefer pytest, unittest, or python -m py_compile checks after source files exist."
            .to_string(),
        "docs" | "documentation" => "- Produce or update documentation artifacts.\n\
- Keep claims grounded in inspected files.\n\
- Avoid source-code changes unless the phase explicitly requires them."
            .to_string(),
        _ => domain_profile(profile).runtime_contract(intent, goal),
    }
}

pub fn profile_generation_rules(profile: &str, intent: &str) -> Option<&'static str> {
    match canonical_profile_name(profile).as_str() {
        "rust" => Some(
            "- Profile rust: preserve Cargo project semantics. Keep Cargo.toml before cargo check/test verification, do not weaken scripts or tests to hide failures, and end with cargo check or cargo test when practical.\n",
        ),
        "python" => Some(
            "- Profile python: keep dependency setup separate from deterministic verification. Prefer python -m py_compile, pytest, or unittest checks after source files exist. Do not put package installation in verify commands.\n",
        ),
        _ => domain_profile(profile).generation_rules(intent),
    }
}

pub fn profile_expected_paths(root: &Path, profile: &str, goal: &str) -> Vec<String> {
    domain_profile(profile).expected_scaffold_paths(root, goal)
}

pub fn profile_setup_scaffold_paths(root: &Path, profile: &str) -> Vec<String> {
    domain_profile(profile).setup_scaffold_paths(root)
}

pub fn profile_evidence_repair_target_paths(
    root: &Path,
    profile: &str,
    evidence_keys: &[String],
) -> Vec<String> {
    domain_profile(profile).evidence_repair_target_paths(root, evidence_keys)
}

pub fn profile_hook_snapshot_targets(
    root: &Path,
    profile: &str,
    goal: &str,
) -> Vec<ProfileHookSnapshotTarget> {
    domain_profile(profile).hook_snapshot_targets(root, goal)
}

pub fn profile_complete_scaffold(
    root: &Path,
    profile: &str,
    missing_paths: &[String],
) -> anyhow::Result<Vec<String>> {
    domain_profile(profile).complete_scaffold(root, missing_paths)
}

pub fn profile_quality_expectations(
    root: &Path,
    profile: &str,
    goal: &str,
) -> ProfileQualityExpectations {
    domain_profile(profile).quality_expectations(root, goal)
}

pub fn profile_repair_prompt(
    root: &Path,
    profile: &str,
    goal: &str,
    report: &VerificationReport,
) -> Option<String> {
    domain_profile(profile).repair_prompt(root, goal, report)
}

pub fn interaction_repair_contract(profile: &str, goal: &str) -> InteractionRepairContract {
    let profile = domain_profile(profile);
    let required_capabilities = profile.infer_required_capabilities(goal);
    let mut required_evidence = profile.infer_required_evidence(goal, &required_capabilities);
    for capability in &required_capabilities {
        merge_unique_strings(
            &mut required_evidence,
            &required_evidence_for_capability(capability),
        );
    }
    InteractionRepairContract {
        required_capabilities,
        required_evidence,
    }
}

pub fn profile_interaction_repair_guidance(
    profile: &str,
    failure_kind: &str,
    contract: &InteractionRepairContract,
) -> Vec<String> {
    domain_profile(profile).interaction_repair_guidance(failure_kind, contract)
}

pub fn inferred_profile_interaction_repair_guidance(
    profile: &str,
    goal: &str,
    failure_kind: &str,
) -> Vec<String> {
    let contract = interaction_repair_contract(profile, goal);
    profile_interaction_repair_guidance(profile, failure_kind, &contract)
}

pub fn profile_auto_repair(
    root: &Path,
    profile: &str,
    goal: &str,
    report: &VerificationReport,
) -> anyhow::Result<bool> {
    domain_profile(profile).deterministic_repair(root, goal, report)
}

pub fn profile_post_step_repair(root: &Path, profile: &str, goal: &str) -> anyhow::Result<bool> {
    domain_profile(profile).post_step_repair(root, goal)
}

pub fn profile_failure(reason: impl Into<String>) -> VerificationReport {
    VerificationReport::profile_failed(reason)
}

fn is_nextjs_build_command(command: &str) -> bool {
    let normalized = command.to_ascii_lowercase();
    normalized == "npm run build"
        || normalized == "pnpm build"
        || normalized == "yarn build"
        || normalized.starts_with("npm run build ")
        || normalized.starts_with("pnpm build ")
        || normalized.starts_with("yarn build ")
        || normalized.contains("next build")
}

pub fn requires_next_binary(command: &str) -> bool {
    is_nextjs_build_command(command)
}

fn requires_node_test_runner(command: &str) -> bool {
    let normalized = command.trim().to_ascii_lowercase();
    normalized == "npm test"
        || normalized == "npm run test"
        || normalized == "pnpm test"
        || normalized == "yarn test"
        || normalized.starts_with("npm test ")
        || normalized.starts_with("npm run test ")
        || normalized.starts_with("pnpm test ")
        || normalized.starts_with("yarn test ")
}

fn requires_node_dependency_probe(command: &str) -> bool {
    let lower = command.trim().to_ascii_lowercase();
    (lower.contains("node -e") && lower.contains("require(")) || lower.contains("npx --no-install")
}

pub(crate) fn generic_dependency_missing_output(output: &str) -> bool {
    let lower = output.to_ascii_lowercase();
    lower.contains("command not found")
        || lower.contains("not found")
        || lower.contains("cannot find module")
        || lower.contains("module not found")
        || lower.contains("modulenotfounderror")
        || lower.contains("can't find crate")
        || lower.contains("no such file or directory")
}

pub(crate) fn merge_unique_strings(out: &mut Vec<String>, incoming: &[String]) {
    for item in incoming {
        if !out.contains(item) {
            out.push(item.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FAILURE: &str = "browser_interaction_failed:input_state_change_missing_after_start";

    #[test]
    fn fix_before_hook_never_preprovisions_create_scaffolds() {
        for profile in ["nextjs", "python-cli"] {
            let dir = tempfile::tempdir().unwrap();
            profile_before_fix_phase(dir.path(), profile).unwrap();
            assert!(!dir.path().join("package.json").exists(), "{profile}");
            assert!(!dir.path().join("pyproject.toml").exists(), "{profile}");
        }

        let dir = tempfile::tempdir().unwrap();
        let snapshot = profile_before_fix_phase(dir.path(), "data").unwrap();
        assert!(matches!(snapshot, ProfileSnapshot::Data(_)));
        assert!(!dir.path().join("pipeline/main.py").exists());
    }

    #[test]
    fn nextjs_fix_regressions_bind_profile_contract_and_build() {
        let dir = tempfile::tempdir().unwrap();
        let bindings = profile_fix_regression_bindings(dir.path(), "nextjs", "fix app");

        assert_eq!(
            bindings
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["profile_contract", "profile_verify_1"]
        );
        assert_eq!(
            bindings[0].adapter,
            ProfileFixRegressionAdapter::ProfileContract
        );
        assert_eq!(
            bindings[1].adapter,
            ProfileFixRegressionAdapter::VerifyCommand("npm run build".to_string())
        );
    }

    #[test]
    fn data_fix_regressions_bind_the_full_manifest_runtime_set() {
        let dir = tempfile::tempdir().unwrap();
        let bindings = profile_fix_regression_bindings(dir.path(), "data", "fix pipeline");

        assert_eq!(
            bindings
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            [
                "pipeline_probe",
                "data_reconciliation",
                "data_claims_binding",
                "data_rerun_consistency",
                "data_results_schema",
            ]
        );
        assert!(bindings.iter().all(|binding| matches!(
            binding.adapter,
            ProfileFixRegressionAdapter::DataManifestCheck
        )));
    }

    #[test]
    fn generic_fix_regression_is_unavailable_instead_of_a_noop_pass() {
        let dir = tempfile::tempdir().unwrap();
        let bindings = profile_fix_regression_bindings(dir.path(), "generic", "fix app");
        let observations =
            run_profile_fix_regressions(dir.path(), "generic", "fix app", &bindings, true);

        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].outcome, ProbeOutcome::Unavailable);
        assert_eq!(
            observations[0].reason,
            "generic_profile_regression_contract_unavailable"
        );
    }

    #[test]
    fn quiz_contract_uses_generic_interaction_guidance_only() {
        let contract = interaction_repair_contract(
            "nextjs",
            "Create a Next.js quiz app with answer buttons, score, and retry behavior",
        );

        let guidance = profile_interaction_repair_guidance("nextjs", FAILURE, &contract);

        assert_eq!(
            guidance,
            vec![
                crate::planner::profiles::nextjs::knowledge::get()
                    .repair_guidance
                    .generic_interaction
                    .clone()
            ]
        );
        assert!(guidance.iter().all(|line| !line.contains("projectiles")));
        assert!(guidance.iter().all(|line| !line.contains("rAF loop")));
    }

    #[test]
    fn space_contract_keeps_canvas_game_guidance() {
        let contract = interaction_repair_contract(
            "nextjs",
            "Create a playable Space Invaders game with enemies, collision, and lives",
        );

        let guidance = profile_interaction_repair_guidance("nextjs", FAILURE, &contract);

        let knowledge = &crate::planner::profiles::nextjs::knowledge::get().repair_guidance;
        for expected in [
            &knowledge.generic_interaction,
            &knowledge.canvas_game_interaction,
            &knowledge.canvas_render_loop_checklist,
            &knowledge.canvas_input_wiring_checklist,
        ] {
            assert!(guidance.contains(expected), "{guidance:?}");
        }
    }

    #[test]
    fn persistence_guidance_requires_persistence_evidence() {
        let ordinary = interaction_repair_contract("nextjs", "Create an interactive form app");
        let persistent = interaction_repair_contract(
            "nextjs",
            "Create a notes app saved in localStorage with live preview",
        );
        let persistence = &crate::planner::profiles::nextjs::knowledge::get()
            .repair_guidance
            .persistence;

        assert!(
            !profile_interaction_repair_guidance("nextjs", FAILURE, &ordinary)
                .contains(persistence)
        );
        assert!(
            profile_interaction_repair_guidance("nextjs", FAILURE, &persistent)
                .contains(persistence)
        );
    }

    #[test]
    fn generic_profile_never_adds_canvas_game_guidance() {
        let contract = InteractionRepairContract {
            required_capabilities: vec!["adversary_or_challenge".to_string()],
            required_evidence: vec!["failure_or_collision_evidence".to_string()],
        };

        let guidance = profile_interaction_repair_guidance("generic", FAILURE, &contract);

        assert_eq!(guidance.len(), 1);
        assert_eq!(
            guidance[0],
            crate::planner::profiles::nextjs::knowledge::get()
                .repair_guidance
                .generic_interaction
        );
    }
}
