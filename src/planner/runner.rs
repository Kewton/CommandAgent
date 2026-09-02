use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

use crate::bounded_process;
use crate::config::{Config, PromptLayout};
use crate::eval_events;
use crate::minimal_loop::behavior_evidence::{self, EvidenceArbitrationReport};
use crate::minimal_loop::browser_probe::{
    BrowserReadinessObservation, html_surface_markers_json,
    probe_browser_readiness_with_offline_and_interaction_options,
};
use crate::minimal_loop::build_verifier::{
    self, BuildVerifierLifecycleObservation, BuildVerifierObservation, BuildVerifierRequirement,
    BuildVerifierStatus, CompileError, emit_dependency_build_lifecycle,
};
use crate::minimal_loop::completion::{
    CompileRepairPromptProtection, CompletionContract, compile_error_repair_guidance,
    compile_repair_prompt_section_with_root, evidence_hint_tokens_for_goal,
};
use crate::minimal_loop::dependency_setup::{
    self, NodeDependencySetupAuthority, NodeDependencySetupRequirement, NodeDependencySetupStatus,
};
use crate::minimal_loop::evidence::{
    RuntimeAcceptanceReport, comment_stripped_source_corpus,
    verify_runtime_acceptance_with_browser_dirs_and_hints,
};
use crate::minimal_loop::feedback::{
    capability_evidence_remedy_lines, capability_evidence_unresolved_reason,
};
use crate::minimal_loop::import_scan::{
    MissingImport, UnattachedRefDiagnostic, format_missing_import_findings,
    route_bound_unattached_ref_diagnostics, scan_relative_imports,
};
use crate::minimal_loop::interaction_probe::{
    self, BrowserInteractionProbeOptions, InteractionProbeOutcome,
};
use crate::minimal_loop::loop_run::{
    ContractEnforcement, RunSessionError, RunSessionOptions, RunSessionOutcome, RunSessionStepKind,
    RunStopReason, extract_requested_artifact_paths, run_session_with_outcome_with_options,
};
use crate::minimal_loop::reachability::{
    RepairReachability, reachability_failure_kind, reachability_recovery_reason,
};
use crate::minimal_loop::repair_pressure::CarriedPressure;
use crate::minimal_loop::repair_target::{
    RepairFollowThrough, RepairTarget, classify_repair_follow_through, classify_repair_target,
};
use crate::minimal_loop::stagnation_carryover::{
    EscalationCarryoverHandle, attach_to_options, run_final_acceptance_repair_with_carryover,
};
use crate::minimal_loop::verifier_env;
use crate::planner::adjudication::contract::IntentId;
use crate::planner::adjudication::*;
use crate::planner::lint::{
    PlanLintReport, PlanQualityContext, PlanQualityReport, lint_ultra_plan_report,
    step_plan_quality_report, step_plan_quality_warnings,
};
use crate::planner::profile::{
    GENERIC_INTERACTIVE_CONTRACT_CAPABILITY, PhaseVerificationMode, ProfileBehaviorProbeReport,
    ProfileId, ProfileInferenceSource, ProfileRuntimeRegistry, ProfileSnapshot, infer_profile,
    profile_before_plan, resolve_profile_runtime,
};
#[cfg(test)]
use crate::planner::profile::{
    domain_profile, profile_setup_scaffold_paths, verify_profile_invariant,
};
use crate::planner::profile_behavior::ProfileRuntime;
use crate::planner::repair::{
    RecoveryHandoff, RepairContext, build_compact_compile_repair_prompt_with_context,
    build_compile_regeneration_prompt_with_context, build_repair_prompt_with_context,
    save_recovery_ultra_plan, save_repair_report_with_context, save_ultra_recovery_prompt,
    suggested_recovery_ultra_plan_command, suggested_ultra_recovery_command,
    workspace_relative_handoff_path,
};
use crate::planner::sanitizer::{SanitizerReport, sanitize_step_plan_against_policy};
#[cfg(test)]
use crate::planner::setup_step_policy;
#[cfg(test)]
use crate::planner::step_plan::parse_generated_step_plan_json;
use crate::planner::step_plan::{
    GeneratedStepPlanFieldDefault, PlanStep, StepKind, StepPlan, extract_json_object,
    parse_generated_step_plan_json_with_report, parse_step_plan, render_step_plan,
    repair_generated_step_plan_contract,
};
use crate::planner::ultra_plan::{UltraPhase, UltraPlan, parse_ultra_plan, render_ultra_plan};
#[cfg(test)]
use crate::planner::verify::verify_setup_dependency_state_with_setup_observed_with_options;
use crate::planner::verify::{
    VerificationReport, verify_setup_dependency_state_with_setup_observed_with_offline,
    verify_step_with_context, verify_step_with_profile_setup_observed_with_offline,
};
use crate::planner::{
    contract_attribute_repair::merge_repair_target_paths, hook_snapshot, repair_targeting, signals,
};
use crate::provider_call::{self, ProviderCallScope};
use crate::providers::{AssistantReply, ChatClient, model_for};
use crate::state::SessionSnapshot;
use crate::tools::path_guard::resolve_existing;
use crate::tui::status::UiStatus;
use crate::tui::{InteractionUi, NOOP_UI};
use serde_json::{Value, json};

#[path = "final_acceptance.rs"]
mod final_acceptance;
use final_acceptance::*;

#[path = "adjudication/create.rs"]
mod adjudication_create;
use adjudication_create::*;

#[path = "runner/recovery_acceptance.rs"]
pub(crate) mod recovery_acceptance;

#[path = "assurance.rs"]
mod assurance;
use assurance::*;

#[path = "runner/phase.rs"]
mod phase;
pub(crate) use phase::StepRunOutcome;
use phase::*;
pub use phase::{
    generate_and_run_ultra_plan, generate_and_run_ultra_plan_with_ui, generate_ultra_plan,
    generate_ultra_plan_with_ui, run_ultra_plan, run_ultra_plan_file, run_ultra_plan_file_with_ui,
    run_ultra_plan_with_ui, save_ultra_plan,
};

#[path = "runner/acceptance.rs"]
mod acceptance;
use acceptance::*;

#[path = "runner/driver.rs"]
mod driver;
use driver::*;
pub use driver::{
    generate_and_run_step_plan, generate_and_run_step_plan_with_ui, generate_step_plan,
    generate_step_plan_with_ui, run_plan_file, run_plan_file_with_ui, run_step_plan,
    run_step_plan_with_ui, save_step_plan,
};

#[cfg(test)]
#[path = "runner/tests/mod.rs"]
mod tests;
