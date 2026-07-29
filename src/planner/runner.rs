use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
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

#[path = "assurance.rs"]
mod assurance;
use assurance::*;

#[path = "ultra_plan_flow.rs"]
mod ultra_plan_flow;
pub use ultra_plan_flow::{
    generate_and_run_ultra_plan, generate_and_run_ultra_plan_with_ui, generate_ultra_plan,
    generate_ultra_plan_with_ui, run_ultra_plan, run_ultra_plan_file, run_ultra_plan_file_with_ui,
    run_ultra_plan_with_ui, save_ultra_plan,
};

const STEP_TURN_MAX_ITERATIONS: usize = 8;
const STEP_REPAIR_MAX_ITERATIONS: usize = 6;
const STEP_REPAIR_MAX_TURNS: usize = 4;
const STEP_REPAIR_IDENTICAL_NO_CHANGE_LIMIT: usize = 2;
const PLANNER_PROVIDER_REQUEST_ATTEMPTS: usize = 2;
const PLANNER_PROVIDER_REQUEST_RETRY_DELAY: Duration = Duration::from_millis(80);
const ULTRA_PLAN_GENERATION_ATTEMPTS: usize = 3;
const NEXTJS_DEV_SERVER_DEFAULT_PORT: u16 = 3011;
const NEXTJS_DEV_SERVER_READY_TIMEOUT: Duration = Duration::from_secs(8);
const NEXTJS_DEV_SERVER_CONNECT_TIMEOUT: Duration = Duration::from_millis(500);
const NEXTJS_DEV_SERVER_WAIT_INTERVAL: Duration = Duration::from_millis(250);
const DEV_SERVER_CLEANUP_TERM_TIMEOUT: Duration = Duration::from_secs(5);
const DEV_SERVER_CLEANUP_KILL_TIMEOUT: Duration = Duration::from_secs(1);
const DEV_SERVER_LOG_EXCERPT_BYTES: usize = 24_000;
const DEV_SERVER_ROUTE: &str = "/";
const DEV_SERVER_LIFECYCLE_STAGES: [&str; 4] = ["start", "wait", "probe", "cleanup"];
const PROFILE_REPAIR_FILE_EXCERPT_MAX_CHARS: usize = 2_400;
const TEXT_ECHO_REPAIR_REQUIREMENT: &str = "token never rendered; render the input's content reactively (no manual rebuild) - the typed text must appear in the preview/list";
const TEXT_ECHO_AFTER_RELOAD_REPAIR_REQUIREMENT: &str =
    "preview renders only after reload - make it reactive to input";
const RESTART_PARTIAL_REPAIR_GUIDANCE: &str = "either expose an in-play restart control, or accept the partial classification (the restart exists but cannot be behaviorally verified by the generic probe)";
const APP_BEHAVIOR_PROBE_FAILURE_KINDS: [&str; 15] = [
    "canvas_blank",
    "interaction_state_change_missing",
    "input_state_change_missing_after_start",
    "input_state_change_not_evaluated_after_start",
    "persistence_after_reload_reset",
    "primary_start_transition_missing",
    "start_transition_missing",
    "surface_missing",
    "text_entry_missing",
    "text_input_state_change_missing",
    "token_echo_after_reload_only",
    "token_echo_missing",
    "surface_visible_missing",
    "interactive_surface_missing",
    "canvas_unavailable",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlannerSessionMode {
    Standard,
    CompactRetry,
    FreshCompact,
}

impl PlannerSessionMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::CompactRetry => "compact_retry",
            Self::FreshCompact => "fresh_compact",
        }
    }
}

const GENERIC_INTERACTIVE_EVIDENCE_KEYS: [&str; 3] = [
    "user_input_handler_evidence",
    "stateful_update_evidence",
    "visible_interactive_surface_evidence",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RepairSessionMode {
    Appended,
    Compact,
}

impl RepairSessionMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Appended => "appended",
            Self::Compact => "compact",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EffectiveRequestedPort {
    port: u16,
    telemetry: String,
}

fn effective_requested_port(
    runtime: &dyn ProfileRuntime,
    goal: &str,
    plan_text: Option<&str>,
) -> Option<EffectiveRequestedPort> {
    if let Some(requested) = signals::requested_port(goal, plan_text) {
        return Some(EffectiveRequestedPort {
            port: requested.port,
            telemetry: format!("{} ({})", requested.port, requested.source.as_str()),
        });
    }
    runtime
        .default_requested_port()
        .map(|port| EffectiveRequestedPort {
            port,
            telemetry: format!("{port} (default)"),
        })
}

#[derive(Debug, Clone)]
struct RecoveryArtifactValidation {
    prompt_exists: bool,
    prompt_parse_ok: bool,
    prompt_parse_error: Option<String>,
    yaml_exists: bool,
    yaml_parse_ok: bool,
    yaml_parse_error: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct ReleaseRecoveryHandoffSummary {
    recovery_handoff_kind: String,
    acceptance_layer: String,
    recovery_prompt_path: String,
    recovery_ultra_plan_path: String,
    suggested_recovery_command: String,
    suggested_recovery_yaml_command: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct PlanAdherenceReport {
    present: Vec<String>,
    missing: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct DepthProfile {
    route_bound_source_line_count: usize,
    state_dimensions_count: usize,
    data_anvil_action_kind_count: usize,
    input_types_with_observed_state_change_count: usize,
    summary: String,
}

impl ReleaseRecoveryHandoffSummary {
    fn has_artifact(&self) -> bool {
        !self.recovery_prompt_path.is_empty() || !self.recovery_ultra_plan_path.is_empty()
    }
}

#[derive(Debug, Clone)]
struct BoundCompletionContract {
    contract: CompletionContract,
    path: String,
    fs_path: Option<PathBuf>,
    generated: bool,
    required: bool,
}

impl RecoveryArtifactValidation {
    fn prompt_command_available(&self) -> bool {
        self.prompt_exists && self.prompt_parse_ok
    }

    fn yaml_command_available(&self) -> bool {
        self.yaml_exists && self.yaml_parse_ok
    }

    fn command_targets_valid(&self) -> bool {
        self.prompt_command_available() && self.yaml_command_available()
    }
}

fn validate_recovery_artifacts(
    prompt_path: &Path,
    recovery_plan_path: Option<&Path>,
) -> RecoveryArtifactValidation {
    let prompt_result = validate_recovery_prompt(prompt_path);
    let (yaml_exists, yaml_parse_ok, yaml_parse_error) = match recovery_plan_path {
        Some(path) => {
            let exists = path.is_file();
            match validate_recovery_yaml(path) {
                Ok(()) => (exists, true, None),
                Err(err) => (exists, false, Some(err)),
            }
        }
        None => (false, false, Some("recovery_yaml_missing".to_string())),
    };
    RecoveryArtifactValidation {
        prompt_exists: prompt_path.is_file(),
        prompt_parse_ok: prompt_result.is_ok(),
        prompt_parse_error: prompt_result.err(),
        yaml_exists,
        yaml_parse_ok,
        yaml_parse_error,
    }
}

fn validate_recovery_prompt(path: &Path) -> Result<(), String> {
    if !path.is_file() {
        return Err("recovery_prompt_missing".to_string());
    }
    let text = std::fs::read_to_string(path)
        .map_err(|err| format!("recovery_prompt_unreadable: {}", err))?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("recovery_prompt_empty".to_string());
    }
    if trimmed.contains("Required recovery action:")
        && (trimmed.contains("Failure evidence:") || trimmed.contains("Primary failure:"))
    {
        return Ok(());
    }
    Err("recovery_prompt_missing_recovery_sections".to_string())
}

fn validate_recovery_yaml(path: &Path) -> Result<(), String> {
    if !path.is_file() {
        return Err("recovery_yaml_missing".to_string());
    }
    let text = std::fs::read_to_string(path)
        .map_err(|err| format!("recovery_yaml_unreadable: {}", err))?;
    let parsed =
        parse_ultra_plan(&text).map_err(|err| format!("recovery_yaml_parse_failed: {}", err))?;
    let rendered = render_ultra_plan(&parsed);
    let reparsed = parse_ultra_plan(&rendered)
        .map_err(|err| format!("recovery_yaml_roundtrip_parse_failed: {}", err))?;
    if reparsed != parsed {
        return Err("recovery_yaml_roundtrip_mismatch".to_string());
    }
    if let Some(reason) = recovery_yaml_needs_review_reason(&text) {
        return Err(format!("recovery_yaml_needs_review: {reason}"));
    }
    Ok(())
}

fn recovery_yaml_needs_review_reason(text: &str) -> Option<String> {
    let mut needs_review = false;
    let mut reason = String::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "recovery_needs_review: true" {
            needs_review = true;
        } else if let Some(value) = trimmed.strip_prefix("recovery_needs_review_reason:") {
            reason = parse_recovery_metadata_string(value.trim());
        }
    }
    needs_review.then(|| {
        if reason.is_empty() {
            "needs_review".to_string()
        } else {
            reason
        }
    })
}

fn parse_recovery_metadata_string(value: &str) -> String {
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        serde_json::from_str(value).unwrap_or_else(|_| value.trim_matches('"').to_string())
    } else {
        value.trim_matches('"').trim_matches('\'').to_string()
    }
}

fn recovery_artifact_check_summary(validation: &RecoveryArtifactValidation) -> String {
    format!(
        "Recovery artifact check: prompt_parse_ok={}, yaml_parse_ok={}, command_targets_valid={}",
        validation.prompt_parse_ok,
        validation.yaml_parse_ok,
        validation.command_targets_valid()
    )
}

pub fn generate_step_plan(
    client: &mut dyn ChatClient,
    goal: &str,
    config: &Config,
) -> anyhow::Result<StepPlan> {
    generate_step_plan_with_ui(client, goal, config, &NOOP_UI)
}

fn planner_chat_with_request_retry(
    client: &mut dyn ChatClient,
    config: &Config,
    scope: ProviderCallScope,
    model: &str,
    messages: &[crate::state::ConversationMessage],
    ui: &dyn InteractionUi,
) -> anyhow::Result<AssistantReply> {
    let mut last_error = None;
    for request_attempt in 1..=PLANNER_PROVIDER_REQUEST_ATTEMPTS {
        let result = {
            let mut guard = ui.before_model_call(&format!("planner {} {model}", client.label()));
            let mut outcome = provider_call::chat_with_cancel_and_stream(
                client,
                config,
                provider_call::ProviderChatRequest {
                    scope,
                    model,
                    messages,
                    tools: &[],
                    native_tools_enabled: false,
                },
                || ui.interrupted(),
                &mut |chunk| guard.push_assistant_chunk(chunk),
            );
            if let Err(err) = guard.finish_assistant_stream()
                && outcome.result.is_ok()
            {
                outcome.result = Err(anyhow::anyhow!("failed to finish provider stream: {err:#}"));
            }
            outcome.result
        };
        match result {
            Ok(reply) => return Ok(reply),
            Err(err) => {
                last_error = Some(err.to_string());
                if last_error
                    .as_deref()
                    .is_some_and(provider_call::is_aborted_by_user)
                    || last_error
                        .as_deref()
                        .is_some_and(crate::providers::streaming::is_after_first_chunk_error)
                {
                    break;
                }
                if request_attempt < PLANNER_PROVIDER_REQUEST_ATTEMPTS {
                    std::thread::sleep(PLANNER_PROVIDER_REQUEST_RETRY_DELAY);
                }
            }
        }
    }
    let last_error = last_error.unwrap_or_else(|| "unknown provider error".to_string());
    if provider_call::is_aborted_by_user(&last_error) {
        anyhow::bail!("{last_error}");
    }
    if provider_call::is_scoped_timeout(scope, &last_error) {
        anyhow::bail!("{last_error}");
    }
    anyhow::bail!(
        "provider request failed after {} attempts: {}",
        PLANNER_PROVIDER_REQUEST_ATTEMPTS,
        last_error
    )
}

fn planner_chat_for_step_plan_attempt(
    client: &mut dyn ChatClient,
    config: &Config,
    model: &str,
    messages: &[crate::state::ConversationMessage],
    ui: &dyn InteractionUi,
    session_mode: PlannerSessionMode,
) -> anyhow::Result<AssistantReply> {
    if session_mode == PlannerSessionMode::FreshCompact {
        let mut fresh_client = client.boxed_clone();
        return planner_chat_with_request_retry(
            fresh_client.as_mut(),
            config,
            ProviderCallScope::PlannerStep,
            model,
            messages,
            ui,
        );
    }
    planner_chat_with_request_retry(
        client,
        config,
        ProviderCallScope::PlannerStep,
        model,
        messages,
        ui,
    )
}

pub fn generate_step_plan_with_ui(
    client: &mut dyn ChatClient,
    goal: &str,
    config: &Config,
    ui: &dyn InteractionUi,
) -> anyhow::Result<StepPlan> {
    generate_step_plan_with_ui_for_phase(client, goal, config, ui, None, false, false)
}

fn generate_step_plan_with_ui_for_phase(
    client: &mut dyn ChatClient,
    goal: &str,
    config: &Config,
    ui: &dyn InteractionUi,
    phase_label: Option<&str>,
    preset_phase: bool,
    final_phase: bool,
) -> anyhow::Result<StepPlan> {
    if ui.interrupted() {
        anyhow::bail!("interrupted by user");
    }
    let fix_before = crate::planner::fix_runtime::is_before_prompt(goal);
    let model = model_for(config, true);
    if phase_label.is_some()
        && let Some(plan) = deterministic_step_plan_for_phase(
            client.label(),
            model,
            goal,
            config,
            phase_label,
            final_phase,
        )?
    {
        return Ok(plan);
    }
    let mut prompt = build_step_plan_user_prompt(goal, config);
    if let Some(guidance) = resolve_profile_runtime(&config.profile).guidance(goal) {
        prompt.push_str("\n\nProfile contract:\n");
        prompt.push_str(&guidance);
        prompt.push_str(
            "\nInclude expected_paths on the final step so deterministic verification can catch missing artifacts.",
        );
    }
    let mut last_error = None;
    let mut last_lint_report = None;
    let mut last_valid_plan: Option<StepPlan> = None;
    let mut lint_categories_seen = BTreeSet::new();
    let mut empty_response_count = 0usize;
    let mut session_mode = PlannerSessionMode::Standard;
    for attempt in 1..=3 {
        let messages = step_plan_messages(&prompt);
        let reply =
            planner_chat_for_step_plan_attempt(client, config, model, &messages, ui, session_mode)?;
        ui.publish_status(UiStatus::for_model_reply(
            config,
            model,
            client.label(),
            reply.prompt_tokens,
            reply.completion_tokens,
        ));
        emit_planner_raw_output_shape(
            config,
            client.label(),
            model,
            attempt,
            &reply.content,
            session_mode,
        );
        if reply.content.trim().is_empty() {
            last_lint_report = None;
            empty_response_count += 1;
            let message = format!(
                "planner_empty_response: planner returned empty content on attempt {attempt}/3"
            );
            last_error = Some(message.clone());
            emit_planner_error(
                config,
                client.label(),
                model,
                "empty_response",
                "planner_empty_response",
                &message,
                attempt,
            );
            if attempt < 3 {
                prompt = build_empty_step_plan_compact_prompt(goal, attempt);
                session_mode = if empty_response_count >= 2 {
                    PlannerSessionMode::FreshCompact
                } else {
                    PlannerSessionMode::CompactRetry
                };
            }
            continue;
        }
        match parse_generated_step_plan_json_with_report(&reply.content, goal) {
            Ok((mut plan, generated_sanitization)) => {
                emit_planner_schema_field_defaults(
                    config,
                    client.label(),
                    model,
                    attempt,
                    &generated_sanitization.field_defaults,
                );
                let verify_before_repair = collect_step_verify_commands(&plan);
                repair_generated_step_plan_contract(&mut plan);
                let verify_after_repair = collect_step_verify_commands(&plan);
                let verify_was_normalized = verify_before_repair != verify_after_repair;
                emit_planner_verify_command_normalized(
                    config,
                    client.label(),
                    model,
                    attempt,
                    &verify_before_repair,
                    &verify_after_repair,
                );
                strengthen_step_plan_for_profile(&mut plan, config);
                repair_generated_step_plan_contract(&mut plan);
                let runtime = resolve_profile_runtime(&config.profile);
                let step_checks_converted = runtime.canonicalize_create_plan(
                    &mut plan,
                    config.resolved_run_intent() == IntentId::Create,
                    phase_label.is_none() || final_phase,
                    config.eval_events_path.as_deref(),
                );
                let sanitizer_report =
                    sanitize_step_plan_against_policy(&mut plan, Some(&config.workspace_root));
                let preset_converted = if fix_before {
                    runtime.bind_empty_fix_verify_steps(
                        &mut plan,
                        phase_label,
                        config.eval_events_path.as_deref(),
                    )
                } else {
                    runtime.convert_preset_phase_setup_steps(
                        &mut plan,
                        &config.workspace_root,
                        goal,
                        phase_label.map(|id| (id, final_phase)),
                        preset_phase,
                        config.eval_events_path.as_deref(),
                    )
                };
                let plan_was_sanitized = verify_was_normalized
                    || !generated_sanitization.is_empty()
                    || !sanitizer_report.is_empty()
                    || step_checks_converted > 0
                    || preset_converted > 0;
                emit_planner_plan_sanitized(
                    config,
                    client.label(),
                    model,
                    attempt,
                    &sanitizer_report,
                );
                let lint_report = crate::planner::lint::lint_template_contract(
                    &plan,
                    Some(&config.workspace_root),
                );
                if lint_report.is_pass() {
                    let quality_context = plan_quality_context(config, goal);
                    let quality_report = step_plan_quality_report(&plan, &quality_context);
                    emit_planner_quality_warnings(config, client.label(), model, attempt, &plan);
                    emit_planner_quality_issues(
                        config,
                        client.label(),
                        model,
                        attempt,
                        &quality_report,
                    );
                    if quality_report.has_retryable_quality() && !plan_was_sanitized && !fix_before
                    {
                        last_valid_plan = Some(plan.clone());
                        if attempt < 3 {
                            emit_planner_quality_retry(
                                config,
                                client.label(),
                                model,
                                attempt,
                                &quality_report,
                            );
                            prompt = build_quality_retry_prompt(goal, &quality_report, attempt);
                            session_mode = PlannerSessionMode::Standard;
                            continue;
                        }
                        emit_planner_quality_retry_exhausted(
                            config,
                            client.label(),
                            model,
                            attempt,
                            &quality_report,
                        );
                    }
                    emit_step_plan_presentation(phase_label, &plan, Some(&sanitizer_report));
                    return Ok(plan);
                }
                if let Some(plan) = last_valid_plan.clone() {
                    emit_planner_quality_retry_degraded(
                        config,
                        client.label(),
                        model,
                        attempt,
                        "lint",
                        &lint_report.primary_message(),
                    );
                    if attempt >= 3 {
                        emit_step_plan_presentation(phase_label, &plan, None);
                        return Ok(plan);
                    }
                    let message = lint_report.primary_message();
                    last_error = Some(message.clone());
                    for err in &lint_report.errors {
                        lint_categories_seen.insert(err.category.clone());
                    }
                    prompt =
                        build_lint_retry_prompt(goal, &lint_report, attempt, &lint_categories_seen);
                    session_mode = PlannerSessionMode::Standard;
                    continue;
                }
                let message = lint_report.primary_message();
                last_lint_report = Some(lint_report.clone());
                emit_planner_error_for_lint(config, client.label(), model, &lint_report, attempt);
                last_error = Some(message.clone());
                for err in &lint_report.errors {
                    lint_categories_seen.insert(err.category.clone());
                }
                prompt =
                    build_lint_retry_prompt(goal, &lint_report, attempt, &lint_categories_seen);
                session_mode = PlannerSessionMode::Standard;
            }
            Err(err) => {
                last_lint_report = None;
                if let Some(plan) = last_valid_plan.clone() {
                    emit_planner_quality_retry_degraded(
                        config,
                        client.label(),
                        model,
                        attempt,
                        "schema",
                        &err.to_string(),
                    );
                    if attempt >= 3 {
                        return Ok(plan);
                    }
                    last_error = Some(err.to_string());
                    prompt = build_schema_retry_prompt(goal, &err.to_string(), attempt);
                    session_mode = PlannerSessionMode::Standard;
                    continue;
                }
                last_error = Some(err.to_string());
                emit_planner_error(
                    config,
                    client.label(),
                    model,
                    "schema",
                    "planner_schema_error",
                    &err.to_string(),
                    attempt,
                );
                prompt = build_schema_retry_prompt(goal, &err.to_string(), attempt);
                session_mode = PlannerSessionMode::Standard;
            }
        }
    }
    if let Some(plan) = last_valid_plan {
        emit_step_plan_presentation(phase_label, &plan, None);
        return Ok(plan);
    }
    if empty_response_count == 3 {
        anyhow::bail!(
            "invalid StepPlan after corrective retries: planner_empty_response: planner returned empty content on all attempts"
        );
    }
    if let Some(plan) = fallback_step_plan_for_setup_phase(goal, config) {
        emit_planner_fallback_plan(
            config,
            client.label(),
            model,
            goal,
            &plan,
            last_error.as_deref().unwrap_or("unknown parse error"),
        );
        emit_step_plan_presentation(phase_label, &plan, None);
        return Ok(plan);
    }
    if let Some(report) = last_lint_report {
        return Err(anyhow::Error::new(
            crate::planner::lint_rejection::PlanLintExhausted { report },
        ));
    }
    anyhow::bail!(
        "invalid StepPlan after corrective retries: {}",
        last_error.unwrap_or_else(|| "unknown parse error".to_string())
    )
}

fn deterministic_step_plan_for_phase(
    provider: &str,
    model: &str,
    phase_prompt: &str,
    config: &Config,
    phase_label: Option<&str>,
    final_phase: bool,
) -> anyhow::Result<Option<StepPlan>> {
    if crate::planner::fix_runtime::is_before_prompt(phase_prompt) {
        return Ok(None);
    }
    if crate::planner::fix_diagnostics::prompt_has_diagnostic(phase_prompt) {
        return Ok(None);
    }
    let Some(template) = resolve_profile_runtime(&config.profile).deterministic_step_plan(
        phase_prompt,
        &config.workspace_root,
        phase_prompt,
    ) else {
        return Ok(None);
    };
    let template_id = template.template_id;
    let mut plan = template.plan;
    let verify_before_repair = collect_step_verify_commands(&plan);
    repair_generated_step_plan_contract(&mut plan);
    let verify_after_repair = collect_step_verify_commands(&plan);
    emit_planner_verify_command_normalized(
        config,
        provider,
        model,
        1,
        &verify_before_repair,
        &verify_after_repair,
    );
    strengthen_step_plan_for_profile(&mut plan, config);
    repair_generated_step_plan_contract(&mut plan);
    let _ = resolve_profile_runtime(&config.profile).canonicalize_create_plan(
        &mut plan,
        config.resolved_run_intent() == IntentId::Create,
        phase_label.is_none() || final_phase,
        config.eval_events_path.as_deref(),
    );
    let sanitizer_report =
        sanitize_step_plan_against_policy(&mut plan, Some(&config.workspace_root));
    emit_planner_plan_sanitized(config, provider, model, 1, &sanitizer_report);
    let lint_report =
        crate::planner::lint::lint_template_contract(&plan, Some(&config.workspace_root));
    if !lint_report.is_pass() {
        emit_planner_error_for_lint(config, provider, model, &lint_report, 1);
        anyhow::bail!(
            "deterministic StepPlan template `{}` failed lint: {}",
            template_id,
            lint_report.primary_message()
        );
    }
    emit_deterministic_step_plan_used(config, phase_label, &template_id, &plan);
    emit_step_plan_presentation(phase_label, &plan, Some(&sanitizer_report));
    Ok(Some(plan))
}

fn emit_deterministic_step_plan_used(
    config: &Config,
    phase_label: Option<&str>,
    template_id: &str,
    plan: &StepPlan,
) {
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "deterministic_step_plan_used",
            "phase_id": phase_label.unwrap_or(""),
            "template_id": template_id,
            "profile": config.profile,
            "step_count": plan.steps.len(),
            "expected_paths": plan.steps.iter().flat_map(|step| step.expected_paths.iter()).cloned().collect::<Vec<_>>(),
            "verify": collect_step_verify_commands(plan),
        }),
    );
}

fn emit_step_plan_presentation(
    phase_label: Option<&str>,
    plan: &StepPlan,
    sanitizer_report: Option<&SanitizerReport>,
) {
    let phase = phase_label.unwrap_or(&plan.goal);
    crate::tui::presentation::emit_step_plan_block(phase, plan, sanitizer_report);
}

pub fn save_step_plan(root: &Path, plan: &StepPlan) -> anyhow::Result<PathBuf> {
    let dir = root.join(".anvil").join("plans");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("plan-{}.yaml", uuid::Uuid::now_v7()));
    std::fs::write(&path, render_step_plan(plan))?;
    Ok(path)
}

pub fn run_plan_file(
    client: &mut dyn ChatClient,
    path: &Path,
    config: &Config,
) -> anyhow::Result<String> {
    run_plan_file_with_ui(client, path, config, &NOOP_UI)
}

pub fn run_plan_file_with_ui(
    client: &mut dyn ChatClient,
    path: &Path,
    config: &Config,
    ui: &dyn InteractionUi,
) -> anyhow::Result<String> {
    let path = resolve_plan_file_path(&config.workspace_root, path)?;
    let text = std::fs::read_to_string(path)?;
    let plan = parse_step_plan(&text)?;
    run_step_plan_with_ui(client, &plan, config, ui)
}

pub fn generate_and_run_step_plan(
    planner: &mut dyn ChatClient,
    execution: &mut dyn ChatClient,
    goal: &str,
    config: &Config,
) -> anyhow::Result<String> {
    generate_and_run_step_plan_with_ui(planner, execution, goal, config, &NOOP_UI)
}

pub fn generate_and_run_step_plan_with_ui(
    planner: &mut dyn ChatClient,
    execution: &mut dyn ChatClient,
    goal: &str,
    config: &Config,
    ui: &dyn InteractionUi,
) -> anyhow::Result<String> {
    let plan = generate_step_plan_with_ui(planner, goal, config, ui)?;
    save_step_plan(&config.workspace_root, &plan)?;
    run_step_plan_with_ui(execution, &plan, config, ui)
}

pub fn run_step_plan(
    client: &mut dyn ChatClient,
    plan: &StepPlan,
    config: &Config,
) -> anyhow::Result<String> {
    run_step_plan_with_ui(client, plan, config, &NOOP_UI)
}

pub fn run_step_plan_with_ui(
    client: &mut dyn ChatClient,
    plan: &StepPlan,
    config: &Config,
    ui: &dyn InteractionUi,
) -> anyhow::Result<String> {
    let mut session = SessionSnapshot::new();
    run_step_plan_with_session_with_ui(
        client,
        &mut session,
        plan,
        config,
        ui,
        true,
        "plan-run",
        None,
        None,
    )
    .map(|outcome| outcome.summary)
    .map_err(|err| anyhow::anyhow!("{}", err.message))
}

#[derive(Debug, Clone, Default)]
struct StepPlanRunOutcome {
    summary: String,
    completed_steps: usize,
    total_steps: usize,
    changed_paths: Vec<String>,
    observed_missing_capabilities: Vec<String>,
    observed_missing_evidence: Vec<String>,
    observed_missing_obligations: Vec<String>,
    verify_failures: Vec<String>,
    primary_failure: Option<String>,
    repair_targets: Vec<String>,
    command_failures: Vec<String>,
    repair_attempts: usize,
    repair_changed_paths: Vec<String>,
    compile_rollbacks: Vec<CompileRollbackOutcome>,
    stop_reason: Option<String>,
    partial: bool,
}

#[derive(Debug, Clone, Default)]
struct UltraRunContext {
    completed_phases: Vec<String>,
    created_or_changed_paths: Vec<String>,
    last_failed_phase: Option<String>,
    last_verify_failures: Vec<String>,
    last_repair_changed_paths: Vec<String>,
    pending_final_artifacts: Vec<String>,
    pending_capability_evidence: Vec<String>,
    unresolved_repair_targets: Vec<String>,
    carry_forward_guidance: Vec<String>,
    truncated: bool,
}

#[derive(Debug, Clone)]
struct ProfilePromotionState {
    eligible: bool,
    promoted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProfilePromotion {
    id: String,
    at_phase: usize,
    phase_id: String,
    requested_port: Option<String>,
    contract_origin: String,
    delta_capabilities: Vec<String>,
    delta_requirements: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ContractRequirements {
    capabilities: Vec<String>,
    evidence: Vec<String>,
    obligations: Vec<String>,
}

const ULTRA_CONTEXT_MAX_PHASES: usize = 12;
const ULTRA_CONTEXT_MAX_PATHS: usize = 24;
const ULTRA_CONTEXT_MAX_MESSAGES: usize = 10;
const ULTRA_PROMPT_GUIDANCE_MAX_LINES: usize = 8;

impl ProfilePromotionState {
    fn for_run(plan: &UltraPlan, config: &Config) -> Self {
        Self {
            eligible: !crate::planner::fix_runtime::applies(plan)
                && ProfileId::parse(&plan.profile) == ProfileId::Generic
                && !config.profile_explicit,
            promoted: false,
        }
    }

    fn can_promote(&self, plan: &UltraPlan) -> bool {
        self.eligible && !self.promoted && ProfileId::parse(&plan.profile) == ProfileId::Generic
    }
}

impl UltraRunContext {
    fn for_run(root: &Path, expected_paths: &[String]) -> Self {
        Self::new(missing_final_artifacts(root, expected_paths))
    }

    fn new(pending_final_artifacts: Vec<String>) -> Self {
        Self {
            pending_final_artifacts,
            ..Self::default()
        }
    }

    fn emit_attached(
        &self,
        config: &Config,
        plan: &UltraPlan,
        phase: &UltraPhase,
        index: usize,
        session: &SessionSnapshot,
    ) {
        emit_ultra_phase_context_attached(config, plan, phase, index, self, session.messages.len());
    }

    fn update_after_phase(
        &mut self,
        phase: &UltraPhase,
        outcome: &StepPlanRunOutcome,
        pending_final_artifacts: Vec<String>,
    ) {
        self.last_failed_phase = None;
        self.pending_final_artifacts = pending_final_artifacts;
        push_context_unique_capped(
            &mut self.completed_phases,
            format!(
                "{} ({}/{})",
                phase.id, outcome.completed_steps, outcome.total_steps
            ),
            ULTRA_CONTEXT_MAX_PHASES,
            &mut self.truncated,
        );
        push_context_items_capped(
            &mut self.created_or_changed_paths,
            &outcome.changed_paths,
            ULTRA_CONTEXT_MAX_PATHS,
            &mut self.truncated,
        );
        self.merge_observed_contract_debt(outcome);
        self.last_verify_failures.clear();
        push_context_items_capped(
            &mut self.last_repair_changed_paths,
            &outcome.repair_changed_paths,
            ULTRA_CONTEXT_MAX_PATHS,
            &mut self.truncated,
        );
        self.unresolved_repair_targets.clear();
        push_context_items_capped(
            &mut self.unresolved_repair_targets,
            &outcome.repair_targets,
            ULTRA_CONTEXT_MAX_MESSAGES,
            &mut self.truncated,
        );
        for rollback in &outcome.compile_rollbacks {
            push_context_items_capped(
                &mut self.carry_forward_guidance,
                &rollback.carry_forward_guidance,
                ULTRA_CONTEXT_MAX_MESSAGES,
                &mut self.truncated,
            );
        }
    }

    fn merge_observed_contract_debt(&mut self, outcome: &StepPlanRunOutcome) {
        push_context_items_capped(
            &mut self.pending_capability_evidence,
            &outcome.observed_contract_keys(),
            ULTRA_CONTEXT_MAX_MESSAGES,
            &mut self.truncated,
        );
    }

    fn refresh_pending_capability_evidence(&mut self, report: &RuntimeAcceptanceReport) {
        let still_missing = runtime_missing_signals(report);
        self.pending_capability_evidence
            .retain(|item| still_missing.contains(item));
        push_context_items_capped(
            &mut self.pending_capability_evidence,
            &report.diagnostics,
            ULTRA_CONTEXT_MAX_MESSAGES,
            &mut self.truncated,
        );
    }

    fn refresh_intent_acceptance(
        &mut self,
        plan: &UltraPlan,
        config: &Config,
    ) -> anyhow::Result<()> {
        if !crate::planner::fix_runtime::applies(plan) {
            let acceptance = ultra_contract_runtime_acceptance_report(plan, config)?;
            self.refresh_pending_capability_evidence(&acceptance);
        }
        Ok(())
    }

    fn render_unmet_final_requirements_section(&self) -> String {
        if self.pending_capability_evidence.is_empty() {
            return "Unmet final requirements from earlier phases:\n- none".to_string();
        }
        let pending = pending_capability_context_items(&self.pending_capability_evidence);
        render_bounded_prompt_section(
            "Unmet final requirements from earlier phases:",
            &pending,
            Some("Close these requirements when they are in scope for this phase."),
            ULTRA_PROMPT_GUIDANCE_MAX_LINES,
        )
    }

    fn update_after_failure(
        &mut self,
        phase: &UltraPhase,
        outcome: &StepPlanRunOutcome,
        pending_final_artifacts: Vec<String>,
    ) {
        self.last_failed_phase = Some(phase.id.clone());
        self.pending_final_artifacts = pending_final_artifacts;
        push_context_items_capped(
            &mut self.created_or_changed_paths,
            &outcome.changed_paths,
            ULTRA_CONTEXT_MAX_PATHS,
            &mut self.truncated,
        );
        push_context_items_capped(
            &mut self.last_verify_failures,
            &outcome.verify_failures,
            ULTRA_CONTEXT_MAX_MESSAGES,
            &mut self.truncated,
        );
        if let Some(primary) = &outcome.primary_failure {
            push_context_unique_capped(
                &mut self.last_verify_failures,
                primary.clone(),
                ULTRA_CONTEXT_MAX_MESSAGES,
                &mut self.truncated,
            );
        }
        push_context_items_capped(
            &mut self.last_repair_changed_paths,
            &outcome.repair_changed_paths,
            ULTRA_CONTEXT_MAX_PATHS,
            &mut self.truncated,
        );
        push_context_items_capped(
            &mut self.unresolved_repair_targets,
            &outcome.repair_targets,
            ULTRA_CONTEXT_MAX_MESSAGES,
            &mut self.truncated,
        );
        for rollback in &outcome.compile_rollbacks {
            push_context_items_capped(
                &mut self.carry_forward_guidance,
                &rollback.carry_forward_guidance,
                ULTRA_CONTEXT_MAX_MESSAGES,
                &mut self.truncated,
            );
        }
    }

    fn update_after_profile_failure(
        &mut self,
        phase: &UltraPhase,
        reason: &str,
        pending_final_artifacts: Vec<String>,
    ) {
        self.last_failed_phase = Some(phase.id.clone());
        self.pending_final_artifacts = pending_final_artifacts;
        push_context_unique_capped(
            &mut self.last_verify_failures,
            eval_events::body_snippet(reason),
            ULTRA_CONTEXT_MAX_MESSAGES,
            &mut self.truncated,
        );
        push_context_unique_capped(
            &mut self.unresolved_repair_targets,
            "profile_contract".to_string(),
            ULTRA_CONTEXT_MAX_MESSAGES,
            &mut self.truncated,
        );
    }

    fn render_prompt_section(&self) -> String {
        if self.completed_phases.is_empty()
            && self.created_or_changed_paths.is_empty()
            && self.last_failed_phase.is_none()
            && self.pending_final_artifacts.is_empty()
            && self.pending_capability_evidence.is_empty()
            && self.carry_forward_guidance.is_empty()
        {
            return "Prior ultra context:\n- none yet".to_string();
        }
        let mut lines = vec!["Prior ultra context:".to_string()];
        append_context_list(&mut lines, "Completed phases", &self.completed_phases);
        append_context_list(
            &mut lines,
            "Created or changed paths",
            &self.created_or_changed_paths,
        );
        if let Some(phase) = &self.last_failed_phase {
            lines.push(format!("- Last failed phase: {phase}"));
        }
        append_context_list(
            &mut lines,
            "Recent verify failures",
            &self.last_verify_failures,
        );
        append_context_list(
            &mut lines,
            "Recent repair changed paths",
            &self.last_repair_changed_paths,
        );
        append_context_list(
            &mut lines,
            "Pending final artifacts",
            &self.pending_final_artifacts,
        );
        append_context_list(
            &mut lines,
            "Pending capability/evidence",
            &pending_capability_context_items(&self.pending_capability_evidence),
        );
        append_context_list(
            &mut lines,
            "Unresolved repair targets",
            &self.unresolved_repair_targets,
        );
        append_context_list(
            &mut lines,
            "Carry-forward guidance",
            &self.carry_forward_guidance,
        );
        if self.truncated {
            lines.push("- Context was truncated to bounded path/failure summaries".to_string());
        }
        lines.join("\n")
    }
}

fn try_promote_profile_at_phase_boundary(
    config: &mut Config,
    plan: &mut UltraPlan,
    context: &mut UltraRunContext,
    final_expected_paths: &mut Vec<String>,
    promotion_state: &mut ProfilePromotionState,
    phase: &UltraPhase,
    index: usize,
) -> anyhow::Result<Option<ProfilePromotion>> {
    if !promotion_state.can_promote(plan) {
        return Ok(None);
    }
    // E5B_PROFILE_DISPATCH_ALLOW: inference-boundary
    let Some(inference) = infer_profile(None, &config.workspace_root) else {
        return Ok(None);
    };
    if inference.source != ProfileInferenceSource::Workspace {
        return Ok(None);
    }
    let promoted_id = ProfileId::parse(inference.profile);
    if promoted_id == ProfileId::Generic || promoted_id == ProfileId::parse(&plan.profile) {
        return Ok(None);
    }
    let promoted = promoted_id.to_string();
    let promoted_runtime = ProfileRuntimeRegistry::resolve(&promoted_id);
    let generic_id = ProfileId::Generic;
    let generic_runtime = ProfileRuntimeRegistry::resolve(&generic_id);

    let generic_paths = generic_runtime.expected_scaffold_paths(&config.workspace_root, &plan.goal);
    let mut generic_requirements =
        runtime_contract_requirements(generic_runtime, &generic_id, &plan.goal);
    let pre_promotion_contract = bind_completion_contract_for_acceptance(
        config,
        "ultra-plan-run-pre-promotion",
        "generic",
        &plan.goal,
        &generic_paths,
        &generic_requirements.capabilities,
        &generic_requirements.evidence,
        &generic_requirements.obligations,
    )?;
    if let Some(contract) = pre_promotion_contract.as_ref().map(|bound| &bound.contract) {
        merge_unique_strings(
            &mut generic_requirements.capabilities,
            &contract.required_capabilities,
        );
        merge_unique_strings(
            &mut generic_requirements.evidence,
            &contract.required_evidence,
        );
        merge_unique_strings(
            &mut generic_requirements.obligations,
            &contract.required_obligations,
        );
    }

    let mut promoted_requirements =
        runtime_contract_requirements(promoted_runtime, &promoted_id, &plan.goal);
    carry_pre_promotion_contract_requirements_with_runtime(
        promoted_runtime,
        &promoted_id,
        &plan.goal,
        &generic_requirements,
        &mut promoted_requirements,
    );
    merge_unique_strings(
        &mut promoted_requirements.evidence,
        &promoted_runtime.required_evidence(&plan.goal, &promoted_requirements.capabilities),
    );
    let promoted_paths =
        promoted_runtime.expected_scaffold_paths(&config.workspace_root, &plan.goal);
    let bound_contract = bind_completion_contract_for_acceptance(
        config,
        "ultra-plan-run",
        &promoted,
        &plan.goal,
        &promoted_paths,
        &promoted_requirements.capabilities,
        &promoted_requirements.evidence,
        &promoted_requirements.obligations,
    )?;
    if let Some(contract) = bound_contract.as_ref().map(|bound| &bound.contract) {
        merge_unique_strings(
            &mut promoted_requirements.capabilities,
            &contract.required_capabilities,
        );
        merge_unique_strings(
            &mut promoted_requirements.evidence,
            &contract.required_evidence,
        );
        merge_unique_strings(
            &mut promoted_requirements.obligations,
            &contract.required_obligations,
        );
    }
    merge_unique_strings(
        &mut promoted_requirements.evidence,
        &promoted_runtime.required_evidence(&plan.goal, &promoted_requirements.capabilities),
    );

    let mapped_generic_capabilities = mapped_pre_promotion_capabilities_for_profile(
        promoted_runtime,
        &promoted_id,
        &generic_requirements.capabilities,
    );
    debug_assert!(
        mapped_generic_capabilities
            .iter()
            .all(|capability| promoted_requirements.capabilities.contains(capability)),
        "profile promotion dropped a pre-promotion capability"
    );
    debug_assert!(
        generic_requirements
            .evidence
            .iter()
            .all(|evidence| promoted_requirements.evidence.contains(evidence)),
        "profile promotion dropped pre-promotion evidence"
    );
    debug_assert!(
        generic_requirements
            .obligations
            .iter()
            .all(|obligation| promoted_requirements.obligations.contains(obligation)),
        "profile promotion dropped pre-promotion obligations"
    );

    let delta_capabilities = ordered_string_difference(
        &promoted_requirements.capabilities,
        &generic_requirements.capabilities,
    );
    let mut generic_requirement_keys = generic_requirements.capabilities.clone();
    merge_unique_strings(
        &mut generic_requirement_keys,
        &generic_requirements.evidence,
    );
    merge_unique_strings(
        &mut generic_requirement_keys,
        &generic_requirements.obligations,
    );
    let mut promoted_requirement_keys = promoted_requirements.capabilities.clone();
    merge_unique_strings(
        &mut promoted_requirement_keys,
        &promoted_requirements.evidence,
    );
    merge_unique_strings(
        &mut promoted_requirement_keys,
        &promoted_requirements.obligations,
    );
    let delta_requirements =
        ordered_string_difference(&promoted_requirement_keys, &generic_requirement_keys);
    push_context_items_capped(
        &mut context.pending_capability_evidence,
        &delta_requirements,
        ULTRA_CONTEXT_MAX_MESSAGES,
        &mut context.truncated,
    );

    plan.profile = promoted.clone();
    config.profile = promoted.clone();
    config.profile_inference = Some(inference);
    *final_expected_paths =
        promoted_runtime.expected_scaffold_paths(&config.workspace_root, &plan.goal);
    context.pending_final_artifacts =
        missing_final_artifacts(&config.workspace_root, final_expected_paths);
    promotion_state.promoted = true;

    let promotion = ProfilePromotion {
        id: promoted,
        at_phase: index + 1,
        phase_id: phase.id.clone(),
        requested_port: effective_requested_port(
            resolve_profile_runtime(&plan.profile),
            &plan.goal,
            Some(&ultra_plan_phase_signal_text(plan)),
        )
        .map(|requested| requested.telemetry),
        contract_origin: "promoted_union".to_string(),
        delta_capabilities,
        delta_requirements,
    };
    emit_profile_reinferred(config, &promotion);
    Ok(Some(promotion))
}

fn ordered_string_difference(values: &[String], baseline: &[String]) -> Vec<String> {
    let baseline = baseline.iter().map(String::as_str).collect::<BTreeSet<_>>();
    values
        .iter()
        .filter(|value| !baseline.contains(value.as_str()))
        .cloned()
        .collect()
}

fn runtime_contract_requirements(
    runtime: &dyn ProfileRuntime,
    profile_id: &ProfileId,
    goal: &str,
) -> ContractRequirements {
    let capabilities = runtime.required_capabilities(goal);
    ContractRequirements {
        evidence: runtime.required_evidence(goal, &capabilities),
        obligations: runtime.required_obligations(profile_id, goal, &capabilities),
        capabilities,
    }
}

fn carry_pre_promotion_contract_requirements_with_runtime(
    promoted_runtime: &dyn ProfileRuntime,
    promoted_id: &ProfileId,
    goal: &str,
    pre_promotion: &ContractRequirements,
    promoted: &mut ContractRequirements,
) {
    merge_unique_strings(
        &mut promoted.capabilities,
        &mapped_pre_promotion_capabilities_for_profile(
            promoted_runtime,
            promoted_id,
            &pre_promotion.capabilities,
        ),
    );
    if pre_promotion
        .capabilities
        .iter()
        .any(|capability| capability == GENERIC_INTERACTIVE_CONTRACT_CAPABILITY)
        && signals::contains_app_intent_token(goal)
    {
        merge_unique_strings(
            &mut promoted.capabilities,
            &promoted_runtime.interactive_app_capabilities(promoted_id),
        );
    }
    merge_unique_strings(&mut promoted.evidence, &pre_promotion.evidence);
    merge_unique_strings(&mut promoted.obligations, &pre_promotion.obligations);
}

fn mapped_pre_promotion_capabilities_for_profile(
    promoted_runtime: &dyn ProfileRuntime,
    promoted_id: &ProfileId,
    capabilities: &[String],
) -> Vec<String> {
    let mut mapped = Vec::new();
    for capability in capabilities {
        if capability == GENERIC_INTERACTIVE_CONTRACT_CAPABILITY {
            let equivalents = promoted_runtime.interactive_app_capabilities(promoted_id);
            if equivalents.is_empty() {
                merge_unique_strings(&mut mapped, std::slice::from_ref(capability));
            } else {
                merge_unique_strings(&mut mapped, &equivalents);
            }
        } else {
            merge_unique_strings(&mut mapped, std::slice::from_ref(capability));
        }
    }
    mapped
}

fn emit_profile_reinferred(config: &Config, promotion: &ProfilePromotion) {
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "profile_reinferred",
            "id": promotion.id,
            "profile": promotion.id,
            "at_phase": promotion.at_phase,
            "phase_id": promotion.phase_id,
            "from": "workspace",
            "from_profile": "generic",
            "to_profile": promotion.id,
            "requested_port": promotion.requested_port.clone(),
            "contract_origin": promotion.contract_origin,
            "delta_capabilities": promotion.delta_capabilities.clone(),
            "delta_requirements": promotion.delta_requirements.clone(),
        }),
    );
    let line = format!(
        "Profile promoted: generic -> {} (workspace evidence, phase {})",
        promotion.id, promotion.at_phase
    );
    eprintln!("{line}");
    eval_events::write_run_summary(config.eval_events_path.as_deref(), &line);
}

fn carry_recorded_promotion_contract_requirements(
    config: &Config,
    effective_profile: &str,
    goal: &str,
    requirements: &mut ContractRequirements,
) {
    let effective_profile_id = ProfileId::parse(effective_profile);
    if contract_origin_for_acceptance(config) != "promoted_union"
        || effective_profile_id == ProfileId::Generic
    {
        return;
    }
    let generic_id = ProfileId::Generic;
    let generic_runtime = ProfileRuntimeRegistry::resolve(&generic_id);
    let generic_requirements = runtime_contract_requirements(generic_runtime, &generic_id, goal);
    let effective_runtime = ProfileRuntimeRegistry::resolve(&effective_profile_id);
    carry_pre_promotion_contract_requirements_with_runtime(
        effective_runtime,
        &effective_profile_id,
        goal,
        &generic_requirements,
        requirements,
    );
    merge_unique_strings(
        &mut requirements.evidence,
        &effective_runtime.required_evidence(goal, &requirements.capabilities),
    );
}

impl StepPlanRunOutcome {
    fn for_plan(plan: &StepPlan) -> Self {
        Self {
            total_steps: plan.steps.len(),
            summary: format!("plan-run complete: {} steps", plan.steps.len()),
            ..Self::default()
        }
    }

    fn merge_step(&mut self, step: &StepRunOutcome) {
        merge_unique_strings(&mut self.changed_paths, &step.changed_paths);
        merge_unique_strings(
            &mut self.observed_missing_capabilities,
            &step.observed_missing_capabilities,
        );
        merge_unique_strings(
            &mut self.observed_missing_evidence,
            &step.observed_missing_evidence,
        );
        merge_unique_strings(
            &mut self.observed_missing_obligations,
            &step.observed_missing_obligations,
        );
        merge_unique_strings(&mut self.verify_failures, &step.verify_failures);
        merge_unique_strings(&mut self.repair_targets, &step.repair_targets);
        merge_unique_strings(&mut self.command_failures, &step.command_failures);
        merge_unique_strings(&mut self.repair_changed_paths, &step.repair_changed_paths);
        self.compile_rollbacks
            .extend(step.compile_rollbacks.iter().cloned());
        self.repair_attempts += step.repair_attempts;
        if let Some(primary) = &step.primary_failure {
            self.primary_failure = Some(primary.clone());
        }
        if let Some(stop) = &step.stop_reason {
            self.stop_reason = Some(stop.clone());
        }
        self.partial |= step.partial;
    }

    fn mark_failure(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.primary_failure = Some(message.clone());
        self.stop_reason = Some(message);
        self.partial = true;
    }

    fn observed_contract_keys(&self) -> Vec<String> {
        let mut out = Vec::new();
        merge_unique_strings(&mut out, &self.observed_missing_capabilities);
        merge_unique_strings(&mut out, &self.observed_missing_evidence);
        merge_unique_strings(&mut out, &self.observed_missing_obligations);
        out
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct StepRunOutcome {
    changed_paths: Vec<String>,
    pub(crate) observed_missing_capabilities: Vec<String>,
    pub(crate) observed_missing_evidence: Vec<String>,
    pub(crate) observed_missing_obligations: Vec<String>,
    verify_failures: Vec<String>,
    primary_failure: Option<String>,
    pub(crate) repair_targets: Vec<String>,
    command_failures: Vec<String>,
    repair_attempts: usize,
    repair_changed_paths: Vec<String>,
    compile_rollbacks: Vec<CompileRollbackOutcome>,
    stop_reason: Option<String>,
    partial: bool,
}

#[derive(Debug, Clone)]
struct StepRunError {
    message: String,
    outcome: StepRunOutcome,
}

#[derive(Debug, Clone)]
struct StepPlanRunError {
    message: String,
    partial_outcome: StepPlanRunOutcome,
}

impl StepPlanRunError {
    fn from_error(message: impl Into<String>, mut partial_outcome: StepPlanRunOutcome) -> Self {
        let message = message.into();
        partial_outcome.mark_failure(message.clone());
        Self {
            message,
            partial_outcome,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct UltraRunSetupAuthorityState {
    reasons: Vec<String>,
}

impl UltraRunSetupAuthorityState {
    fn authority(&self) -> NodeDependencySetupAuthority {
        if self.reasons.is_empty() {
            NodeDependencySetupAuthority::None
        } else {
            NodeDependencySetupAuthority::PlanSetupStep
        }
    }

    fn grant(&mut self, reason: &str) {
        if !self.reasons.iter().any(|existing| existing == reason) {
            self.reasons.push(reason.to_string());
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DependencyReconciliationTrigger {
    Promotion,
    ManifestRepair,
    ManifestChanged,
    DeclaredDependenciesNotReady,
}

impl DependencyReconciliationTrigger {
    fn as_str(self) -> &'static str {
        match self {
            Self::Promotion => "promotion",
            Self::ManifestRepair => "manifest_repair",
            Self::ManifestChanged => "manifest_changed",
            Self::DeclaredDependenciesNotReady => "declared_dependencies_not_ready",
        }
    }
}

fn reconcile_run_dependency_setup(
    config: &Config,
    profile: &str,
    trigger: DependencyReconciliationTrigger,
    setup_authority: &UltraRunSetupAuthorityState,
) -> anyhow::Result<Option<Vec<String>>> {
    let authority = setup_authority.authority();
    let Some(requirement) =
        dependency_reconciliation_requirement(&config.workspace_root, profile, trigger, authority)
    else {
        return Ok(None);
    };
    let setup = dependency_setup::run_node_dependency_setup_with_program_and_offline(
        &config.workspace_root,
        &requirement,
        Path::new("npm"),
        config.offline,
    );
    let lifecycle = dependency_reconciliation_lifecycle(&requirement, setup.clone());
    emit_dependency_build_lifecycle(
        config.eval_events_path.as_deref(),
        "ultra-plan-run",
        Some(trigger.as_str()),
        &lifecycle,
    );
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "dependency_setup_reconciliation",
            "trigger": trigger.as_str(),
            // E5B_PROFILE_DISPATCH_ALLOW: telemetry-profile
            "profile": ProfileId::parse(profile).to_string(),
            "authority": authority.as_str(),
            "setup_kind": setup.setup_kind.as_str(),
            "status": setup.status.as_str(),
            "attempted": setup.attempted,
            "command": setup.command.clone(),
            "added": setup.changed_paths.clone(),
            "primary_reason": eval_events::body_snippet(&setup.primary_reason),
            "duration_ms": setup.duration_ms,
            "timeout_ms": setup.timeout_ms,
            "classification": if setup.status == NodeDependencySetupStatus::TimedOut {
                "dependency_setup_timeout"
            } else {
                ""
            },
            "offline": config.offline,
        }),
    );
    match setup.status {
        NodeDependencySetupStatus::Passed | NodeDependencySetupStatus::NotRequired => {
            Ok(Some(setup.changed_paths))
        }
        NodeDependencySetupStatus::Blocked
            if setup.primary_reason == "dependency_setup_blocked_offline" =>
        {
            anyhow::bail!("dependency_setup_blocked_offline")
        }
        NodeDependencySetupStatus::Blocked => {
            anyhow::bail!("dependency_setup_blocked: {}", setup.primary_reason)
        }
        NodeDependencySetupStatus::Failed | NodeDependencySetupStatus::TimedOut => {
            anyhow::bail!(
                "dependency_setup_lifecycle_failed: {}",
                setup.primary_reason
            )
        }
        NodeDependencySetupStatus::Attempted => {
            anyhow::bail!("dependency_setup_lifecycle_failed: dependency setup did not finish")
        }
    }
}

fn dependency_reconciliation_requirement(
    root: &Path,
    profile: &str,
    trigger: DependencyReconciliationTrigger,
    authority: NodeDependencySetupAuthority,
) -> Option<NodeDependencySetupRequirement> {
    let reason = format!("{} dependency reconciliation", trigger.as_str());
    let profile_id = ProfileId::parse(profile);
    resolve_profile_runtime(profile).dependency_reconciliation_requirement(
        root,
        &profile_id,
        trigger == DependencyReconciliationTrigger::ManifestChanged,
        &reason,
        authority,
    )
}

fn reconcile_manifest_changed_dependencies_if_needed(
    config: &Config,
    profile: &str,
    setup_authority: &mut UltraRunSetupAuthorityState,
) -> anyhow::Result<Option<Vec<String>>> {
    if !dependency_setup::node_dependency_declarations_fingerprint_mismatch(&config.workspace_root)
    {
        return Ok(None);
    }
    setup_authority.grant("manifest_changed");
    reconcile_run_dependency_setup(
        config,
        profile,
        DependencyReconciliationTrigger::ManifestChanged,
        setup_authority,
    )
}

fn verification_report_mentions_dependency_setup_missing(report: &VerificationReport) -> bool {
    report
        .dependency_missing
        .iter()
        .chain(report.profile_failures.iter())
        .any(|reason| text_mentions_dependency_setup_missing(reason))
        || report
            .command_failures
            .iter()
            .any(|failure| text_mentions_dependency_setup_missing(&failure.reason))
}

fn text_mentions_dependency_setup_missing(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("dependency setup missing")
        || lower.contains("build dependency setup missing")
        || lower.contains("node_modules missing")
}

fn push_unique_label(labels: &mut Vec<String>, label: &str) {
    if !labels.iter().any(|existing| existing == label) {
        labels.push(label.to_string());
    }
}

fn dependency_reconciliation_lifecycle(
    requirement: &NodeDependencySetupRequirement,
    setup: dependency_setup::NodeDependencySetupObservation,
) -> BuildVerifierLifecycleObservation {
    let before = BuildVerifierObservation {
        command: "dependency reconciliation".to_string(),
        profile: requirement.profile.clone(),
        authority: requirement.setup_authority.as_str().to_string(),
        required_for_completion: true,
        requires_dependency_setup: true,
        dependency_ready: false,
        attempted: false,
        status: BuildVerifierStatus::DependencyMissing,
        primary_reason: requirement.reason.clone(),
        output_snippet: String::new(),
        output_path: String::new(),
        compile_errors: Vec::new(),
        foreign_toolchain: None,
    };
    let after_status = match setup.status {
        NodeDependencySetupStatus::Passed | NodeDependencySetupStatus::NotRequired => {
            BuildVerifierStatus::Passed
        }
        NodeDependencySetupStatus::Blocked | NodeDependencySetupStatus::TimedOut => {
            BuildVerifierStatus::Blocked
        }
        NodeDependencySetupStatus::Failed | NodeDependencySetupStatus::Attempted => {
            BuildVerifierStatus::Failed
        }
    };
    let after = (after_status == BuildVerifierStatus::Passed).then(|| BuildVerifierObservation {
        command: "dependency reconciliation".to_string(),
        profile: requirement.profile.clone(),
        authority: requirement.setup_authority.as_str().to_string(),
        required_for_completion: true,
        requires_dependency_setup: true,
        dependency_ready: true,
        attempted: false,
        status: BuildVerifierStatus::Passed,
        primary_reason: "dependency setup reconciliation passed".to_string(),
        output_snippet: String::new(),
        output_path: String::new(),
        compile_errors: Vec::new(),
        foreign_toolchain: None,
    });
    let final_reason = after
        .as_ref()
        .map(|observation| observation.primary_reason.clone())
        .unwrap_or_else(|| setup.primary_reason.clone());
    BuildVerifierLifecycleObservation {
        requirement: BuildVerifierRequirement {
            command: "dependency reconciliation".to_string(),
            profile: requirement.profile.clone(),
            reason: requirement.reason.clone(),
            authority: requirement.setup_authority.as_str().to_string(),
            status: "required".to_string(),
            requires_dependency_setup: true,
            required_for_completion: true,
        },
        before_setup: before,
        setup: Some(setup),
        after_setup: after,
        final_status: after_status,
        final_reason,
    }
}

#[allow(clippy::result_large_err, clippy::too_many_arguments)]
fn run_step_plan_with_session_with_ui(
    client: &mut dyn ChatClient,
    session: &mut SessionSnapshot,
    plan: &StepPlan,
    config: &Config,
    ui: &dyn InteractionUi,
    verify_final_contract: bool,
    mode: &'static str,
    phase_scope: Option<&str>,
    overall_goal_override: Option<&str>,
) -> Result<StepPlanRunOutcome, StepPlanRunError> {
    run_step_plan_with_session_with_ui_and_run_authority(
        client,
        session,
        plan,
        config,
        ui,
        verify_final_contract,
        mode,
        phase_scope,
        overall_goal_override,
        None,
    )
}

#[allow(clippy::result_large_err, clippy::too_many_arguments)]
fn run_step_plan_with_session_with_ui_and_run_authority(
    client: &mut dyn ChatClient,
    session: &mut SessionSnapshot,
    plan: &StepPlan,
    config: &Config,
    ui: &dyn InteractionUi,
    verify_final_contract: bool,
    mode: &'static str,
    phase_scope: Option<&str>,
    overall_goal_override: Option<&str>,
    mut run_setup_authority: Option<&mut UltraRunSetupAuthorityState>,
) -> Result<StepPlanRunOutcome, StepPlanRunError> {
    let mut outcome = StepPlanRunOutcome::for_plan(plan);
    let overall_goal = overall_goal_override.unwrap_or(&plan.goal);
    let report = crate::planner::lint::lint_plan_for_execution(plan, Some(&config.workspace_root));
    if !report.is_pass() {
        emit_planner_error_for_lint(config, "plan-file", &config.planner_model, &report, 0);
        if !lint_report_is_runtime_repairable_verifier_command(&report) {
            return Err(StepPlanRunError::from_error(
                report.primary_message(),
                outcome,
            ));
        }
    }
    let required_final_artifacts = required_final_artifacts(plan, &config.workspace_root);
    let profile_id = ProfileId::parse(&config.profile);
    let runtime = ProfileRuntimeRegistry::resolve(&profile_id);
    let mut final_required_capabilities = runtime.required_capabilities(&plan.goal);
    let final_required_obligations =
        runtime.required_obligations(&profile_id, &plan.goal, &final_required_capabilities);
    let initial_required_evidence =
        runtime.required_evidence(&plan.goal, &final_required_capabilities);
    let bound_contract = bind_completion_contract_for_acceptance(
        config,
        "plan-run",
        &config.profile,
        &plan.goal,
        &required_final_artifacts,
        &final_required_capabilities,
        &initial_required_evidence,
        &final_required_obligations,
    )
    .map_err(|err| StepPlanRunError::from_error(err.to_string(), outcome.clone()))?;
    if let Some(contract) = bound_contract.as_ref().map(|bound| &bound.contract) {
        merge_unique_strings(
            &mut final_required_capabilities,
            &contract.required_capabilities,
        );
    }
    let mut final_required_evidence =
        runtime.required_evidence(&plan.goal, &final_required_capabilities);
    if let Some(contract) = bound_contract.as_ref().map(|bound| &bound.contract) {
        merge_unique_strings(
            &mut final_required_capabilities,
            &contract.required_capabilities,
        );
        merge_unique_strings(&mut final_required_evidence, &contract.required_evidence);
    }
    let contract_enforcement = if verify_final_contract {
        ContractEnforcement::Enforce
    } else {
        ContractEnforcement::Observe
    };
    let mut prior_expected_paths = Vec::new();
    for step in &plan.steps {
        if ui.interrupted() {
            return Err(StepPlanRunError::from_error("interrupted by user", outcome));
        }
        let prompt_context = StepPromptContext {
            overall_goal: overall_goal.to_string(),
            required_final_artifacts: required_final_artifacts.clone(),
            prior_expected_paths: prior_expected_paths.clone(),
            final_required_capabilities: final_required_capabilities.clone(),
            final_required_evidence: final_required_evidence.clone(),
            completion_contract_path: bound_contract
                .as_ref()
                .and_then(|bound| bound.fs_path.clone()),
        };
        match run_step(
            client,
            session,
            plan,
            step,
            &prompt_context,
            config,
            ui,
            mode,
            contract_enforcement,
            phase_scope,
            run_setup_authority.as_deref_mut(),
        ) {
            Ok(step_outcome) => {
                outcome.completed_steps += 1;
                outcome.merge_step(&step_outcome);
            }
            Err(err) => {
                outcome.merge_step(&err.outcome);
                return Err(StepPlanRunError::from_error(err.message, outcome));
            }
        }
        if let Some(state) = run_setup_authority.as_deref_mut()
            && step_carries_setup_authority(plan, step, phase_scope)
        {
            state.grant("phase_setup_step");
        }
        merge_unique_strings(&mut prior_expected_paths, &step.expected_paths);
    }
    if verify_final_contract
        && let Err(err) = verify_plan_final_contract(
            plan,
            &required_final_artifacts,
            config,
            bound_contract.as_ref(),
        )
    {
        return Err(StepPlanRunError::from_error(err.to_string(), outcome));
    }
    outcome.summary = format!("plan-run complete: {} steps", plan.steps.len());
    Ok(outcome)
}

fn lint_report_is_runtime_repairable_verifier_command(report: &PlanLintReport) -> bool {
    !report.errors.is_empty()
        && report.errors.iter().all(|error| {
            error.category == "verify_policy"
                && (error
                    .message
                    .contains("grep pattern begins with '-' but command lacks `--` or `-e`")
                    || error.message.contains(
                        "grep package.json script assertion replaced with JSON parser check",
                    ))
        })
}

#[derive(Debug, Clone, Default)]
struct StepPromptContext {
    overall_goal: String,
    required_final_artifacts: Vec<String>,
    prior_expected_paths: Vec<String>,
    final_required_capabilities: Vec<String>,
    final_required_evidence: Vec<String>,
    completion_contract_path: Option<PathBuf>,
}

#[allow(clippy::result_large_err, clippy::too_many_arguments)]
fn run_step(
    client: &mut dyn ChatClient,
    session: &mut SessionSnapshot,
    plan: &StepPlan,
    step: &PlanStep,
    prompt_context: &StepPromptContext,
    config: &Config,
    ui: &dyn InteractionUi,
    mode: &'static str,
    contract_enforcement: ContractEnforcement,
    phase_scope: Option<&str>,
    mut run_setup_authority: Option<&mut UltraRunSetupAuthorityState>,
) -> Result<StepRunOutcome, StepRunError> {
    let runtime = resolve_profile_runtime(&config.profile);
    let (mut runtime_step, synthesized_precheck) = runtime.runtime_step_with_profile_checks(
        &config.workspace_root,
        &prompt_context.overall_goal,
        step,
        phase_scope,
        config.eval_events_path.as_deref(),
    );
    runtime
        .inject_step_material(config, &mut runtime_step)
        .map_err(|err| StepRunError {
            message: format!("step source material injection failed: {err}"),
            outcome: StepRunOutcome::default(),
        })?;
    let instruction = build_step_prompt(plan, &runtime_step, prompt_context, config.prompt_layout);
    emit_step_prompt_contract(config, &runtime_step, prompt_context, &instruction);
    if step.step_kind() == StepKind::Report
        && step.expected_paths.is_empty()
        && step.verify.is_empty()
    {
        return Ok(StepRunOutcome::default());
    }
    let mut step_config = capped_config(config, STEP_TURN_MAX_ITERATIONS);
    if step.step_kind() == StepKind::Implement
        && let Some(path) = prompt_context.completion_contract_path.clone()
    {
        step_config.completion_contract_path = Some(path);
    }
    let run_authority = run_setup_authority
        .as_deref()
        .map(UltraRunSetupAuthorityState::authority)
        .unwrap_or(NodeDependencySetupAuthority::None);
    let setup_authority = step_verify_setup_authority(plan, step, run_authority);
    let contract_setup_authority =
        step_contract_setup_authority(plan, step, phase_scope, run_authority);
    let step_options = step_run_session_options(
        plan,
        step,
        contract_enforcement,
        phase_scope,
        contract_setup_authority,
    )
    .with_repair_target_priority(repair_targeting::RepairTargetPriority::for_intent(
        config.resolved_intent(&prompt_context.overall_goal),
    ))
    .with_required_mutation_before_short_circuit(synthesized_precheck);
    let data_pre_satisfied =
        runtime.pre_satisfied_verify_first(&config.workspace_root, &runtime_step);
    let verify_first_applicable = data_pre_satisfied
        .unwrap_or_else(|| runtime.step_short_circuit_precheck_applicable(&runtime_step));
    if verify_first_applicable {
        let (report, build_lifecycles) = verify_step_completion_observed(
            config,
            &prompt_context.overall_goal,
            &runtime_step,
            step,
            phase_scope,
            setup_authority,
        );
        apply_runtime_command_normalizations(&mut runtime_step, &report);
        for lifecycle in &build_lifecycles {
            emit_dependency_build_lifecycle(
                config.eval_events_path.as_deref(),
                mode,
                Some(&step.id),
                lifecycle,
            );
        }
        if report.is_pass() {
            if data_pre_satisfied.is_some() {
                crate::planner::profiles::data::pre_satisfied::emit_short_circuited(
                    config.eval_events_path.as_deref(),
                    &runtime_step,
                    phase_scope,
                    &report,
                );
            } else {
                emit_runner_step_short_circuited(
                    config,
                    &runtime_step,
                    phase_scope,
                    &runtime_step.expected_paths,
                    "start",
                );
            }
            if production_build_lifecycle_passed(&build_lifecycles) {
                snapshot_last_known_good_sources(
                    config,
                    mode,
                    Some(&step.id),
                    &config.profile,
                    prompt_context.overall_goal.as_str(),
                    &runtime_step.expected_paths,
                );
            }
            return Ok(StepRunOutcome {
                stop_reason: Some("StepShortCircuited".to_string()),
                ..StepRunOutcome::default()
            });
        }
    }
    let initial = run_session_with_outcome_with_options(
        client,
        session,
        &instruction,
        &runtime_step.expected_paths,
        &step_config,
        ui,
        step_options.clone(),
    )
    .map_err(|err| {
        let message = err.to_string();
        StepRunError {
            outcome: step_run_outcome_from_session_error(&err, "initial_turn_error"),
            message,
        }
    })?;
    let mut outcome = StepRunOutcome {
        changed_paths: initial.changed_paths.clone(),
        observed_missing_capabilities: initial.missing_capabilities.clone(),
        observed_missing_evidence: initial.missing_evidence.clone(),
        observed_missing_obligations: initial.missing_obligations.clone(),
        stop_reason: Some(format!("{:?}", initial.stop_reason)),
        ..StepRunOutcome::default()
    };
    let overall_goal = prompt_context.overall_goal.as_str();
    match runtime.post_step_repair(&config.workspace_root, overall_goal) {
        Ok(true) => {
            if let Some(state) = run_setup_authority.as_deref_mut() {
                state.grant("manifest_repair");
                if let Err(err) = reconcile_run_dependency_setup(
                    config,
                    &config.profile,
                    DependencyReconciliationTrigger::ManifestRepair,
                    state,
                ) {
                    let message = err.to_string();
                    outcome.primary_failure = Some(message.clone());
                    outcome.stop_reason =
                        Some("dependency_setup_reconciliation_failed".to_string());
                    outcome.partial = true;
                    return Err(StepRunError { message, outcome });
                }
            }
        }
        Ok(false) => {}
        Err(err) => {
            outcome.primary_failure = Some(err.to_string());
            outcome.stop_reason = Some("profile_post_step_repair_error".to_string());
            outcome.partial = true;
            return Err(StepRunError {
                message: err.to_string(),
                outcome,
            });
        }
    }
    if let Some(state) = run_setup_authority.as_deref_mut()
        && let Err(err) =
            reconcile_manifest_changed_dependencies_if_needed(config, &config.profile, state)
    {
        let message = err.to_string();
        outcome.primary_failure = Some(message.clone());
        outcome.stop_reason = Some("dependency_setup_reconciliation_failed".to_string());
        outcome.partial = true;
        return Err(StepRunError { message, outcome });
    }
    let (report, build_lifecycles) = verify_step_completion_observed(
        config,
        &prompt_context.overall_goal,
        &runtime_step,
        step,
        phase_scope,
        setup_authority,
    );
    apply_runtime_command_normalizations(&mut runtime_step, &report);
    for lifecycle in &build_lifecycles {
        emit_dependency_build_lifecycle(
            config.eval_events_path.as_deref(),
            mode,
            Some(&step.id),
            lifecycle,
        );
    }
    if report.is_pass() {
        if production_build_lifecycle_passed(&build_lifecycles) {
            snapshot_last_known_good_sources(
                config,
                mode,
                Some(&step.id),
                &config.profile,
                overall_goal,
                &runtime_step.expected_paths,
            );
        }
        return Ok(outcome);
    }
    let first_target = classify_repair_target(&report).as_str().to_string();
    let repair_policy = crate::planner::profiles::data::repair_policy::StepRepairPolicy::new(
        &config.profile,
        &step.id,
        STEP_REPAIR_MAX_TURNS,
        config.eval_events_path.as_deref(),
    );
    let mut current_reachability =
        repair_policy.assess(&report, setup_authority, config.offline, 0);
    outcome.primary_failure = Some(report.primary_reason());
    outcome.verify_failures.push(report.primary_reason());
    outcome.repair_targets.push(first_target.clone());
    outcome
        .command_failures
        .extend(command_failure_summaries(&report));
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "step_verify_failure",
            "step_id": step.id,
            "repair_target": first_target,
            "primary_reason": eval_events::body_snippet(&report.primary_reason()),
            "missing_paths": report.missing_paths.clone(),
            "command_failures": report.command_failures.len(),
            "dependency_missing": report.dependency_missing.clone(),
            "profile_failures": report.profile_failures.clone(),
            "dependency_setup_authority": setup_authority.as_str(),
            "repair_reachable": current_reachability.reachable,
            "reachable": current_reachability.reachable,
            "viable_actions": reachability_action_labels(&current_reachability),
            "blocked_requirements": current_reachability.blocked_requirements.clone(),
        }),
    );
    let mut context = RepairContext {
        profile: Some(config.profile.clone()),
        overall_goal: Some(overall_goal.to_string()),
        required_final_artifacts: prompt_context.required_final_artifacts.clone(),
        step_instruction: Some(step.instruction.clone()),
        expected_paths: merge_repair_target_paths(&report, &runtime_step.expected_paths),
        verify_commands: runtime_step.verify.clone(),
        expected_result: Some(step_expected_result(step).to_string()),
        max_repair_turns: Some(STEP_REPAIR_MAX_TURNS),
        missing_paths: report.missing_paths.clone(),
        changed_files: initial.changed_paths.clone(),
        initial_stop_reason: Some(format!("{:?}", initial.stop_reason)),
        workspace_root: Some(config.workspace_root.clone()),
        eval_events_path: config.eval_events_path.clone(),
        prompt_layout: config.prompt_layout,
        ..RepairContext::default()
    };
    let mut current_report = report;
    let mut previous_missing = current_report.missing_paths.len();
    let mut repair_stop_reason = None;
    let mut terminal_repair_failure_kind: Option<String> = None;
    let mut terminal_blocked_requirements: Vec<String> = Vec::new();
    let mut no_change_repairs = 0usize;
    let mut target_not_followed_repairs = 0usize;
    let mut identical_no_change_repairs = 0usize;
    let mut hook_snapshot_feedback_given = false;
    let mut hook_snapshot_restore_used = false;
    let mut current_report_signature = verification_report_signature(&current_report);
    let repair_config = capped_config(config, STEP_REPAIR_MAX_ITERATIONS);
    let escalation_carryover = EscalationCarryoverHandle::from_pressure(CarriedPressure::default());
    if !current_reachability.reachable {
        terminal_repair_failure_kind =
            Some(reachability_failure_kind(&current_reachability).to_string());
        terminal_blocked_requirements = current_reachability.blocked_requirements.clone();
        context.progress_warning = Some(reachability_recovery_reason(&current_reachability));
        emit_repair_unreachable(
            config,
            mode,
            &step.id,
            classify_repair_target(&current_report).as_str(),
            &current_report.primary_reason(),
            &current_reachability,
        );
    }
    if current_reachability.reachable {
        for attempt in 1..=STEP_REPAIR_MAX_TURNS {
            let repair_session_mode =
                if no_change_repairs > 0 && !current_report.compile_errors.is_empty() {
                    RepairSessionMode::Compact
                } else {
                    RepairSessionMode::Appended
                };
            context.repair_attempt = Some(attempt);
            context.compile_reanchored_retry = repair_session_mode == RepairSessionMode::Compact;
            context.compile_narrow_no_snapshot_retry = false;
            let repair_options =
                attach_to_options(step_options.clone(), escalation_carryover.clone());
            let mut repair_prompt = if repair_session_mode == RepairSessionMode::Compact {
                build_compact_compile_repair_prompt_with_context(
                    &step.id,
                    &current_report,
                    &context,
                )
            } else {
                build_repair_prompt_with_context(&step.id, &current_report, &context)
            };
            repair_prompt = hook_snapshot::prefix_feedback_if_missing_with_runtime(
                config,
                runtime,
                overall_goal,
                "step_verify_repair",
                Some(&step.id),
                &mut hook_snapshot_feedback_given,
                repair_prompt,
            );
            let repair_result = match repair_session_mode {
                RepairSessionMode::Appended => run_session_with_outcome_with_options(
                    client,
                    session,
                    &repair_prompt,
                    &context.expected_paths,
                    &repair_config,
                    ui,
                    repair_options.clone(),
                ),
                RepairSessionMode::Compact => {
                    let mut compact_session = SessionSnapshot::new();
                    run_session_with_outcome_with_options(
                        client,
                        &mut compact_session,
                        &repair_prompt,
                        &context.expected_paths,
                        &repair_config,
                        ui,
                        repair_options.clone(),
                    )
                }
            };
            let repair = match repair_result {
                Ok(repair) => repair,
                Err(err)
                    if repair_session_mode == RepairSessionMode::Compact
                        && !current_report.compile_errors.is_empty()
                        && err
                            .to_string()
                            .contains("missing tool call for action prompt") =>
                {
                    RunSessionOutcome {
                        final_text: err.to_string(),
                        stop_reason: RunStopReason::AssistantFinal,
                        changed_paths: Vec::new(),
                        iterations: 0,
                        tool_calls: 0,
                        missing_required_paths: Vec::new(),
                        missing_capabilities: Vec::new(),
                        missing_evidence: Vec::new(),
                        missing_obligations: Vec::new(),
                        verify_attempts: 0,
                        last_blocking_reason: Some(err.to_string()),
                        last_provider_error: None,
                    }
                }
                Err(err) => {
                    let message = err.to_string();
                    apply_session_error_observations(&mut outcome, &err, &message);
                    outcome.primary_failure = Some(message.clone());
                    outcome.stop_reason = Some("repair_turn_error".to_string());
                    outcome.repair_attempts = attempt;
                    outcome.partial = true;
                    return Err(StepRunError {
                        message,
                        outcome: outcome.clone(),
                    });
                }
            };
            outcome.repair_attempts = attempt;
            repair_stop_reason = Some(format!("{:?}", repair.stop_reason));
            let changed_paths_before_repair = context.changed_files.clone();
            let mut repair_turn_changed_paths = repair.changed_paths.clone();
            merge_changed_files(&mut context, &repair.changed_paths);
            merge_unique_strings(&mut outcome.changed_paths, &repair.changed_paths);
            merge_unique_strings(
                &mut outcome.observed_missing_capabilities,
                &repair.missing_capabilities,
            );
            merge_unique_strings(
                &mut outcome.observed_missing_evidence,
                &repair.missing_evidence,
            );
            merge_unique_strings(
                &mut outcome.observed_missing_obligations,
                &repair.missing_obligations,
            );
            merge_unique_strings(&mut outcome.repair_changed_paths, &repair.changed_paths);
            match runtime.post_step_repair(&config.workspace_root, overall_goal) {
                Ok(true) => {
                    if let Some(state) = run_setup_authority.as_deref_mut() {
                        state.grant("manifest_repair");
                        if let Err(err) = reconcile_run_dependency_setup(
                            config,
                            &config.profile,
                            DependencyReconciliationTrigger::ManifestRepair,
                            state,
                        ) {
                            let message = err.to_string();
                            outcome.primary_failure = Some(message.clone());
                            outcome.stop_reason =
                                Some("dependency_setup_reconciliation_failed".to_string());
                            outcome.partial = true;
                            return Err(StepRunError { message, outcome });
                        }
                    }
                    let package_path = "package.json".to_string();
                    merge_changed_files(&mut context, std::slice::from_ref(&package_path));
                    merge_unique_strings(
                        &mut outcome.changed_paths,
                        std::slice::from_ref(&package_path),
                    );
                    merge_unique_strings(&mut outcome.repair_changed_paths, &[package_path]);
                    merge_unique_strings(
                        &mut repair_turn_changed_paths,
                        &["package.json".to_string()],
                    );
                }
                Ok(false) => {}
                Err(err) => {
                    outcome.primary_failure = Some(err.to_string());
                    outcome.stop_reason = Some("profile_post_step_repair_error".to_string());
                    outcome.partial = true;
                    return Err(StepRunError {
                        message: err.to_string(),
                        outcome,
                    });
                }
            }
            if let Some(state) = run_setup_authority.as_deref_mut()
                && let Err(err) = reconcile_manifest_changed_dependencies_if_needed(
                    config,
                    &config.profile,
                    state,
                )
            {
                let message = err.to_string();
                outcome.primary_failure = Some(message.clone());
                outcome.stop_reason = Some("dependency_setup_reconciliation_failed".to_string());
                outcome.partial = true;
                return Err(StepRunError { message, outcome });
            }
            let (retry, retry_lifecycles) = verify_step_with_context(
                &config.workspace_root,
                &runtime_step,
                Some(&config.profile),
                Some(overall_goal),
                setup_authority,
                config.offline,
                config.eval_events_path.as_deref(),
            );
            apply_runtime_command_normalizations(&mut runtime_step, &retry);
            context.verify_commands = runtime_step.verify.clone();
            for lifecycle in &retry_lifecycles {
                emit_dependency_build_lifecycle(
                    config.eval_events_path.as_deref(),
                    mode,
                    Some(&step.id),
                    lifecycle,
                );
            }
            let retry_target = classify_repair_target(&retry);
            let retry_reachability =
                repair_policy.assess(&retry, setup_authority, config.offline, attempt);
            let previous_target = classify_repair_target(&current_report);
            let repair_follow_through =
                classify_repair_follow_through(previous_target, &repair_turn_changed_paths);
            let repair_target_followed = repair_follow_through.followed();
            match repair_follow_through {
                RepairFollowThrough::NoChange => {
                    no_change_repairs += 1;
                }
                RepairFollowThrough::TargetNotFollowed | RepairFollowThrough::UnrelatedChange => {
                    target_not_followed_repairs += 1;
                    no_change_repairs = 0;
                }
                RepairFollowThrough::TargetMatched => {
                    no_change_repairs = 0;
                    target_not_followed_repairs = 0;
                }
            }
            let compile_repair_no_change = !current_report.compile_errors.is_empty()
                && matches!(repair_follow_through, RepairFollowThrough::NoChange);
            let repair_failure_kind = if compile_repair_no_change {
                "compile_repair_no_source_change"
            } else {
                repair_follow_through.failure_kind().unwrap_or("")
            };
            let retry_signature = verification_report_signature(&retry);
            let report_signature_unchanged = retry_signature == current_report_signature;
            if report_signature_unchanged && repair_turn_changed_paths.is_empty() {
                identical_no_change_repairs += 1;
            } else {
                identical_no_change_repairs = 0;
            }
            merge_unique_strings(
                &mut outcome.repair_targets,
                &[retry_target.as_str().to_string()],
            );
            if !retry.is_pass() {
                merge_unique_strings(&mut outcome.verify_failures, &[retry.primary_reason()]);
                merge_unique_strings(
                    &mut outcome.command_failures,
                    &command_failure_summaries(&retry),
                );
                outcome.primary_failure = Some(retry.primary_reason());
            }
            eval_events::emit(
                config.eval_events_path.as_deref(),
                json!({
                    "event": "step_verify_repair",
                    "step_id": step.id,
                    "attempt": attempt,
                    "ok": retry.is_pass(),
                    "repair_target": retry_target.as_str(),
                    "previous_repair_target": previous_target.as_str(),
                    "repair_target_followed": repair_target_followed,
                    "target_relation": repair_follow_through.as_str(),
                    "repair_follow_through": repair_follow_through.as_str(),
                    "failure_kind": repair_failure_kind,
                    "primary_reason": eval_events::body_snippet(&retry.primary_reason()),
                    "changed_paths": repair_turn_changed_paths.clone(),
                    "changed_paths_before": changed_paths_before_repair,
                    "changed_paths_after": context.changed_files.clone(),
                    "repair_turn_changed_paths": repair_turn_changed_paths.clone(),
                    "no_change_repairs": no_change_repairs,
                    "target_not_followed_repairs": target_not_followed_repairs,
                    "identical_no_change_repairs": identical_no_change_repairs,
                    "report_signature_unchanged": report_signature_unchanged,
                    "allowed_action": previous_target.allowed_action(),
                    "repair_stop_reason": repair_stop_reason.clone().unwrap_or_default(),
                    "repair_session_mode": repair_session_mode.as_str(),
                    "compile_reanchored_retry": context.compile_reanchored_retry,
                    "compile_repair_no_source_change": compile_repair_no_change,
                    "dependency_setup_authority": setup_authority.as_str(),
                    "repair_reachable": retry_reachability.reachable,
                    "reachable": retry_reachability.reachable,
                    "viable_actions": reachability_action_labels(&retry_reachability),
                    "blocked_requirements": retry_reachability.blocked_requirements.clone(),
                }),
            );
            if retry.is_pass() {
                if production_build_lifecycle_passed(&retry_lifecycles) {
                    snapshot_last_known_good_sources(
                        config,
                        mode,
                        Some(&step.id),
                        &config.profile,
                        &plan.goal,
                        &runtime_step.expected_paths,
                    );
                }
                outcome.primary_failure = None;
                outcome.stop_reason = repair_stop_reason.clone();
                return Ok(outcome);
            }
            if hook_snapshot_feedback_given && !hook_snapshot_restore_used {
                match hook_snapshot::restore_first_missing_with_runtime(
                    config,
                    runtime,
                    overall_goal,
                ) {
                    Ok(Some(restored)) => {
                        hook_snapshot_restore_used = true;
                        merge_changed_files(
                            &mut context,
                            std::slice::from_ref(&restored.restored_path),
                        );
                        merge_unique_strings(
                            &mut outcome.changed_paths,
                            std::slice::from_ref(&restored.restored_path),
                        );
                        merge_unique_strings(
                            &mut outcome.repair_changed_paths,
                            std::slice::from_ref(&restored.restored_path),
                        );
                        let (restored_retry, restored_lifecycles) = verify_step_with_context(
                            &config.workspace_root,
                            &runtime_step,
                            Some(&config.profile),
                            Some(overall_goal),
                            setup_authority,
                            config.offline,
                            config.eval_events_path.as_deref(),
                        );
                        apply_runtime_command_normalizations(&mut runtime_step, &restored_retry);
                        context.verify_commands = runtime_step.verify.clone();
                        for lifecycle in &restored_lifecycles {
                            emit_dependency_build_lifecycle(
                                config.eval_events_path.as_deref(),
                                mode,
                                Some(&step.id),
                                lifecycle,
                            );
                        }
                        eval_events::emit(
                            config.eval_events_path.as_deref(),
                            json!({
                                "event": "step_verify_repair",
                                "step_id": step.id,
                                "attempt": attempt,
                                "ok": restored_retry.is_pass(),
                                "repair_target": classify_repair_target(&restored_retry).as_str(),
                                "previous_repair_target": retry_target.as_str(),
                                "repair_target_followed": true,
                                "target_relation": "hook_snapshot_restore",
                                "repair_follow_through": "hook_snapshot_restore",
                                "failure_kind": if restored_retry.is_pass() { "" } else { "hook_snapshot_restore_unresolved" },
                                "primary_reason": eval_events::body_snippet(&restored_retry.primary_reason()),
                                "changed_paths": [restored.restored_path.clone()],
                                "repair_turn_changed_paths": [restored.restored_path.clone()],
                                "repair_session_mode": "hook_snapshot_restore",
                                "dependency_setup_authority": setup_authority.as_str(),
                                "repair_reachable": true,
                                "reachable": true,
                            }),
                        );
                        if restored_retry.is_pass() {
                            outcome.primary_failure = None;
                            outcome.stop_reason = Some("hook_snapshot_restore_applied".to_string());
                            return Ok(outcome);
                        }
                        let restored_reachability = repair_policy.assess(
                            &restored_retry,
                            setup_authority,
                            config.offline,
                            attempt,
                        );
                        if !restored_reachability.reachable {
                            terminal_repair_failure_kind =
                                Some(reachability_failure_kind(&restored_reachability).to_string());
                            terminal_blocked_requirements =
                                restored_reachability.blocked_requirements.clone();
                            context.progress_warning =
                                Some(reachability_recovery_reason(&restored_reachability));
                            current_report = restored_retry;
                            emit_repair_unreachable(
                                config,
                                mode,
                                &step.id,
                                classify_repair_target(&current_report).as_str(),
                                &current_report.primary_reason(),
                                &restored_reachability,
                            );
                            break;
                        }
                        previous_missing = restored_retry.missing_paths.len();
                        current_report_signature = verification_report_signature(&restored_retry);
                        current_report = restored_retry;
                        continue;
                    }
                    Ok(None) => {}
                    Err(err) => {
                        context.progress_warning =
                            Some(format!("Hook snapshot restore failed: {err}"));
                    }
                }
            }
            if compile_repair_no_change && repair_session_mode == RepairSessionMode::Compact {
                match single_compile_regeneration_target(&retry) {
                    Ok(target_path) => {
                        let Some(target_abs) =
                            writable_workspace_source_path(&config.workspace_root, &target_path)
                        else {
                            emit_compile_regeneration_event(
                                config,
                                Some(&step.id),
                                "step_repair",
                                false,
                                false,
                                0,
                                Some(&target_path),
                                "target_path_rejected",
                                retry.compile_errors.len(),
                                retry.compile_errors.len(),
                                &[],
                            );
                            current_report = retry;
                            terminal_repair_failure_kind =
                                Some("compile_repair_no_source_change".to_string());
                            break;
                        };
                        let before_content = match std::fs::read(&target_abs) {
                            Ok(content) => content,
                            Err(err) => {
                                emit_compile_regeneration_event(
                                    config,
                                    Some(&step.id),
                                    "step_repair",
                                    false,
                                    false,
                                    0,
                                    Some(&target_path),
                                    &format!("snapshot_read_error:{err}"),
                                    retry.compile_errors.len(),
                                    retry.compile_errors.len(),
                                    &[],
                                );
                                current_report = retry;
                                terminal_repair_failure_kind =
                                    Some("compile_repair_no_source_change".to_string());
                                break;
                            }
                        };
                        let before_error_count = retry.compile_errors.len();
                        let regeneration_prompt = build_compile_regeneration_prompt_with_context(
                            &step.id,
                            &retry,
                            &context,
                            &target_path,
                        );
                        let mut regeneration_session = SessionSnapshot::new();
                        let regeneration = run_session_with_outcome_with_options(
                            client,
                            &mut regeneration_session,
                            &regeneration_prompt,
                            std::slice::from_ref(&target_path),
                            &repair_config,
                            ui,
                            repair_options.clone(),
                        );
                        let regeneration = match regeneration {
                            Ok(regeneration) => regeneration,
                            Err(err) => {
                                let _ = std::fs::write(&target_abs, &before_content);
                                emit_compile_regeneration_event(
                                    config,
                                    Some(&step.id),
                                    "step_repair",
                                    true,
                                    false,
                                    0,
                                    Some(&target_path),
                                    &format!(
                                        "regeneration_turn_error:{}",
                                        eval_events::body_snippet(&err.to_string())
                                    ),
                                    before_error_count,
                                    before_error_count,
                                    &[],
                                );
                                current_report = retry;
                                if compile_repair_no_change && no_change_repairs >= 2 {
                                    terminal_repair_failure_kind =
                                        Some("compile_repair_no_source_change".to_string());
                                    break;
                                }
                                current_report_signature = retry_signature;
                                continue;
                            }
                        };
                        let mut regeneration_changed_paths = regeneration.changed_paths.clone();
                        regeneration_changed_paths.sort();
                        regeneration_changed_paths.dedup();
                        let one_file_write =
                            changed_paths_only_target(&regeneration_changed_paths, &target_path);
                        let (regenerated_report, regeneration_lifecycles) =
                            verify_step_with_context(
                                &config.workspace_root,
                                &runtime_step,
                                Some(&config.profile),
                                Some(overall_goal),
                                setup_authority,
                                config.offline,
                                config.eval_events_path.as_deref(),
                            );
                        apply_runtime_command_normalizations(
                            &mut runtime_step,
                            &regenerated_report,
                        );
                        context.verify_commands = runtime_step.verify.clone();
                        for lifecycle in &regeneration_lifecycles {
                            emit_dependency_build_lifecycle(
                                config.eval_events_path.as_deref(),
                                mode,
                                Some(&step.id),
                                lifecycle,
                            );
                        }
                        let after_error_count = regenerated_report.compile_errors.len();
                        let error_delta = before_error_count as i64 - after_error_count as i64;
                        if one_file_write && error_delta > 0 {
                            emit_compile_regeneration_event(
                                config,
                                Some(&step.id),
                                "step_repair",
                                true,
                                true,
                                error_delta,
                                Some(&target_path),
                                "accepted",
                                before_error_count,
                                after_error_count,
                                &regeneration_changed_paths,
                            );
                            merge_changed_files(&mut context, std::slice::from_ref(&target_path));
                            merge_unique_strings(
                                &mut outcome.changed_paths,
                                std::slice::from_ref(&target_path),
                            );
                            merge_unique_strings(
                                &mut outcome.repair_changed_paths,
                                std::slice::from_ref(&target_path),
                            );
                            if regenerated_report.is_pass() {
                                if production_build_lifecycle_passed(&regeneration_lifecycles) {
                                    snapshot_last_known_good_sources(
                                        config,
                                        mode,
                                        Some(&step.id),
                                        &config.profile,
                                        &plan.goal,
                                        &runtime_step.expected_paths,
                                    );
                                }
                                outcome.primary_failure = None;
                                outcome.stop_reason =
                                    Some("compile_regeneration_applied".to_string());
                                return Ok(outcome);
                            }
                            let regenerated_reachability = repair_policy.assess(
                                &regenerated_report,
                                setup_authority,
                                config.offline,
                                attempt,
                            );
                            if !regenerated_reachability.reachable {
                                terminal_repair_failure_kind = Some(
                                    reachability_failure_kind(&regenerated_reachability)
                                        .to_string(),
                                );
                                terminal_blocked_requirements =
                                    regenerated_reachability.blocked_requirements.clone();
                                context.progress_warning =
                                    Some(reachability_recovery_reason(&regenerated_reachability));
                                current_report = regenerated_report;
                                emit_repair_unreachable(
                                    config,
                                    mode,
                                    &step.id,
                                    classify_repair_target(&current_report).as_str(),
                                    &current_report.primary_reason(),
                                    &regenerated_reachability,
                                );
                                break;
                            }
                            current_report_signature =
                                verification_report_signature(&regenerated_report);
                            previous_missing = regenerated_report.missing_paths.len();
                            current_report = regenerated_report;
                            continue;
                        }
                        let _ = std::fs::write(&target_abs, &before_content);
                        emit_compile_regeneration_event(
                            config,
                            Some(&step.id),
                            "step_repair",
                            true,
                            false,
                            error_delta,
                            Some(&target_path),
                            if one_file_write {
                                "compile_error_count_not_decreased"
                            } else {
                                "changed_paths_not_single_target"
                            },
                            before_error_count,
                            after_error_count,
                            &regeneration_changed_paths,
                        );
                    }
                    Err(reason) => emit_compile_regeneration_event(
                        config,
                        Some(&step.id),
                        "step_repair",
                        false,
                        false,
                        0,
                        None,
                        &reason,
                        retry.compile_errors.len(),
                        retry.compile_errors.len(),
                        &[],
                    ),
                }
            }
            current_reachability = retry_reachability;
            if !current_reachability.reachable {
                terminal_repair_failure_kind =
                    Some(reachability_failure_kind(&current_reachability).to_string());
                terminal_blocked_requirements = current_reachability.blocked_requirements.clone();
                context.progress_warning =
                    Some(reachability_recovery_reason(&current_reachability));
                current_report = retry;
                emit_repair_unreachable(
                    config,
                    mode,
                    &step.id,
                    classify_repair_target(&current_report).as_str(),
                    &current_report.primary_reason(),
                    &current_reachability,
                );
                break;
            }
            let next_missing = retry.missing_paths.len();
            if next_missing >= previous_missing {
                context.progress_warning = Some(format!(
                    "Missing expected paths did not decrease after repair. Remaining: {}",
                    if retry.missing_paths.is_empty() {
                        "none".to_string()
                    } else {
                        retry.missing_paths.join(", ")
                    }
                ));
            }
            previous_missing = next_missing;
            current_report = retry;
            current_report_signature = retry_signature;
            if compile_repair_no_change && no_change_repairs >= 2 {
                terminal_repair_failure_kind = Some("compile_repair_no_source_change".to_string());
                break;
            }
            if identical_no_change_repairs >= STEP_REPAIR_IDENTICAL_NO_CHANGE_LIMIT {
                terminal_repair_failure_kind = Some(if !current_report.compile_errors.is_empty() {
                    "compile_repair_no_source_change".to_string()
                } else {
                    "verify_repair_progress_unchanged".to_string()
                });
                break;
            }
        }
    }
    let final_failure_kind =
        terminal_repair_failure_kind.unwrap_or_else(|| "bounded_repair_exhausted".to_string());
    context.repair_stop_reason = Some(final_failure_kind.to_string());
    let final_repair_target = classify_repair_target(&current_report);
    if !current_report.compile_errors.is_empty() {
        match try_compile_rollback_after_repair_exhaustion(
            config,
            &config.profile,
            overall_goal,
            phase_scope.unwrap_or(&step.id),
            &step.instruction,
            &current_report,
            &final_failure_kind,
        ) {
            Ok(Some(rollback)) => {
                outcome.primary_failure = None;
                outcome.stop_reason = Some("compile_rollback_applied".to_string());
                outcome.partial = true;
                outcome.compile_rollbacks.push(rollback);
                return Ok(outcome);
            }
            Ok(None) => {}
            Err(err) => {
                let message = err.to_string();
                outcome.primary_failure = Some(message.clone());
                outcome.stop_reason = Some("compile_rollback_error".to_string());
                outcome.partial = true;
                return Err(StepRunError { message, outcome });
            }
        }
    }
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "loop_stop",
            "reason": final_failure_kind,
            "step_id": step.id,
            "repair_target": final_repair_target.as_str(),
            "repair_stop_reason": repair_stop_reason.clone().unwrap_or_default(),
            "local_repair_exhausted": true,
        }),
    );
    let repair_report_path = match save_repair_report_with_context(
        &config.workspace_root,
        &step.id,
        &current_report,
        &context,
    ) {
        Ok(path) => path,
        Err(err) => {
            outcome.primary_failure = Some(err.to_string());
            outcome.stop_reason = Some("repair_report_save_error".to_string());
            outcome.partial = true;
            return Err(StepRunError {
                message: err.to_string(),
                outcome,
            });
        }
    };
    let step_handoff = RecoveryHandoff {
        profile: config.profile.clone(),
        original_goal: context
            .overall_goal
            .clone()
            .unwrap_or_else(|| plan.goal.clone()),
        failed_phase: None,
        failed_step: Some(step.id.clone()),
        failure_kind: final_failure_kind.to_string(),
        failure_evidence: std::iter::once(current_report.primary_reason())
            .chain(reachability_blocked_evidence(
                &terminal_blocked_requirements,
            ))
            .chain(
                current_report
                    .command_failures
                    .iter()
                    .map(|failure| format!("{}: {}", failure.command, failure.reason)),
            )
            .chain(
                current_report
                    .verifier_command_false_negatives
                    .iter()
                    .map(|failure| {
                        format!(
                            "deterministic_verify_command_bug: {}: {}",
                            failure.command, failure.reason
                        )
                    }),
            )
            .collect(),
        missing_paths: current_report.missing_paths.clone(),
        missing_capabilities: vec![final_repair_target.as_str().to_string()],
        verify_commands: context.verify_commands.clone(),
        changed_paths: context.changed_files.clone(),
        repair_targets: vec![final_repair_target.as_str().to_string()],
    };
    let recovery_plan_path =
        match save_recovery_ultra_plan(&config.workspace_root, &step.id, &step_handoff) {
            Ok(path) => Some(path),
            Err(err) => {
                let repair_report_display = handoff_path(&repair_report_path);
                eval_events::emit(
                    config.eval_events_path.as_deref(),
                    json!({
                        "event": "recovery_ultra_plan_save_failed",
                        "recovery_handoff_kind": final_failure_kind,
                        "step_id": step.id,
                        "recovery_prompt_path": repair_report_display,
                        "reason": eval_events::body_snippet(&err.to_string()),
                        "recovery_yaml_missing": true,
                    }),
                );
                None
            }
        };
    let validation =
        validate_recovery_artifacts(&repair_report_path, recovery_plan_path.as_deref());
    let raw_suggested_command =
        suggested_ultra_recovery_command(&repair_report_path, &config.profile);
    let suggested_command = if validation.prompt_command_available() {
        raw_suggested_command
    } else {
        String::new()
    };
    let suggested_yaml_command = recovery_plan_path
        .as_ref()
        .filter(|_| validation.yaml_command_available())
        .map(|path| suggested_recovery_ultra_plan_command(path));
    let repair_report_display = handoff_path(&repair_report_path);
    let recovery_plan_display = optional_handoff_path(recovery_plan_path.as_ref());
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "recovery_prompt_saved",
            "recovery_handoff_kind": final_failure_kind,
            "step_id": step.id,
            "recovery_prompt_path": &repair_report_display,
            "recovery_ultra_plan_path": &recovery_plan_display,
            "recovery_yaml_missing": recovery_plan_path.is_none(),
            "recovery_prompt_exists": validation.prompt_exists,
            "recovery_prompt_parse_ok": validation.prompt_parse_ok,
            "recovery_prompt_parse_error": validation.prompt_parse_error.as_deref().unwrap_or_default(),
            "recovery_yaml_exists": validation.yaml_exists,
            "recovery_yaml_parse_ok": validation.yaml_parse_ok,
            "recovery_yaml_parse_error": validation.yaml_parse_error.as_deref().unwrap_or_default(),
            "recovery_command_targets_valid": validation.command_targets_valid(),
            "suggested_recovery_command": suggested_command.clone(),
            "suggested_recovery_yaml_command": suggested_yaml_command.clone().unwrap_or_default(),
            "recovery_profile": config.profile,
            "local_repair_exhausted": true,
            "failure_kind": final_failure_kind,
            "status": "incomplete",
        }),
    );
    let yaml_summary = recovery_plan_path
        .as_ref()
        .map(|path| {
            let display = handoff_path(path);
            if validation.yaml_parse_ok {
                format!("Recovery UltraPlan YAML saved: {display}")
            } else {
                format!(
                    "Recovery UltraPlan YAML invalid: {} ({})",
                    display,
                    validation
                        .yaml_parse_error
                        .as_deref()
                        .unwrap_or("recovery_yaml_invalid")
                )
            }
        })
        .unwrap_or_else(|| {
            "Recovery UltraPlan YAML missing: failed to save valid recovery plan".to_string()
        });
    let prompt_command_summary = if validation.prompt_command_available() {
        format!("Suggested command: {suggested_command}")
    } else {
        format!(
            "Suggested command: unavailable because recovery prompt validation failed ({})",
            validation
                .prompt_parse_error
                .as_deref()
                .unwrap_or("recovery_prompt_invalid")
        )
    };
    let yaml_command_summary = suggested_yaml_command
        .as_ref()
        .map(|command| format!("Suggested YAML command: {command}"))
        .unwrap_or_else(|| {
            "Suggested YAML command: unavailable because recovery YAML is missing".to_string()
        });
    let artifact_check_summary = recovery_artifact_check_summary(&validation);
    eval_events::write_run_summary(
        config.eval_events_path.as_deref(),
        &format!(
            "Status: incomplete\n{}\n{}\nRecovery prompt saved: {}\n{}\n{}\nFailure: {}",
            yaml_summary,
            yaml_command_summary,
            repair_report_display,
            prompt_command_summary,
            artifact_check_summary,
            eval_events::body_snippet(&current_report.primary_reason())
        ),
    );
    let prompt_message = if validation.prompt_command_available() {
        format!("suggested command: {suggested_command}")
    } else {
        "suggested command unavailable because recovery prompt validation failed".to_string()
    };
    let yaml_message = suggested_yaml_command
        .as_ref()
        .map(|command| format!("suggested YAML command: {command}"))
        .unwrap_or_else(|| {
            "suggested YAML command unavailable because recovery YAML is missing".to_string()
        });
    let message = eval_events::render_stop_reason(&eval_events::StopReasonParts {
        free_text: format!(
            "step {} failed verification after bounded repair: {}; failure_kind={}; incomplete; {}",
            step.id,
            current_report.primary_reason(),
            final_failure_kind,
            artifact_check_summary
        ),
        paths: vec![
            format!("repair prompt saved: {repair_report_display}"),
            yaml_summary,
        ],
        commands: vec![prompt_message, yaml_message],
    });
    outcome.primary_failure = Some(current_report.primary_reason());
    outcome.stop_reason = Some(final_failure_kind.to_string());
    outcome.partial = true;
    Err(StepRunError { message, outcome })
}

fn step_verify_setup_authority(
    plan: &StepPlan,
    step: &PlanStep,
    fallback: NodeDependencySetupAuthority,
) -> NodeDependencySetupAuthority {
    if step.step_kind() == StepKind::Setup {
        return NodeDependencySetupAuthority::PlanSetupStep;
    }
    if step.step_kind() != StepKind::Verify {
        return fallback;
    }
    let prior_setup_exists = plan
        .steps
        .iter()
        .take_while(|candidate| candidate.id != step.id)
        .any(|candidate| candidate.step_kind() == StepKind::Setup);
    if prior_setup_exists {
        NodeDependencySetupAuthority::PlanSetupStep
    } else {
        fallback
    }
}

fn step_contract_setup_authority(
    _plan: &StepPlan,
    step: &PlanStep,
    phase_scope: Option<&str>,
    fallback: NodeDependencySetupAuthority,
) -> NodeDependencySetupAuthority {
    if step.step_kind() != StepKind::Implement {
        return NodeDependencySetupAuthority::None;
    }
    if step_or_phase_is_dependency_setup_purpose(step, phase_scope) {
        NodeDependencySetupAuthority::PlanSetupStep
    } else {
        fallback
    }
}

fn step_carries_setup_authority(
    plan: &StepPlan,
    step: &PlanStep,
    phase_scope: Option<&str>,
) -> bool {
    step_verify_setup_authority(plan, step, NodeDependencySetupAuthority::None).allows_setup()
        || step_contract_setup_authority(
            plan,
            step,
            phase_scope,
            NodeDependencySetupAuthority::None,
        )
        .allows_setup()
}

fn step_or_phase_is_dependency_setup_purpose(step: &PlanStep, phase_scope: Option<&str>) -> bool {
    [
        step.id.as_str(),
        step.kind.as_str(),
        step.instruction.as_str(),
    ]
    .into_iter()
    .chain(phase_scope)
    .any(text_mentions_dependency_setup)
}

fn text_mentions_dependency_setup(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    signals::contains_setup_token(text)
        && (lower.contains("depend")
            || lower.contains("workspace")
            || lower.contains("package")
            || lower.contains("npm")
            || text.contains("依存"))
}

fn should_run_setup_step_dependency_state_lifecycle(
    root: &Path,
    step: &PlanStep,
    phase_scope: Option<&str>,
    setup_authority: NodeDependencySetupAuthority,
) -> bool {
    step.step_kind() == StepKind::Setup
        && setup_authority == NodeDependencySetupAuthority::PlanSetupStep
        && step_or_phase_is_dependency_setup_purpose(step, phase_scope)
        && dependency_setup::package_json_declares_dependencies(root)
        && !dependency_setup::node_declared_dependencies_ready(root)
}

fn merge_verification_report(report: &mut VerificationReport, extra: VerificationReport) {
    for path in extra.missing_paths {
        report.push_missing_path(path);
    }
    for reason in extra.dependency_missing {
        report.push_dependency_missing(reason);
    }
    for failure in extra.command_failures {
        report.push_command_failure(failure.command, failure.reason);
    }
    for failure in extra.verifier_command_false_negatives {
        report.push_verifier_command_false_negative(failure.command, failure.reason);
    }
    for normalization in extra.runtime_command_normalizations {
        report.runtime_command_normalizations.push(normalization);
    }
    for error in extra.compile_errors {
        if !report.compile_errors.contains(&error) {
            report.compile_errors.push(error);
        }
    }
    for traceback in extra.python_tracebacks {
        report.push_python_traceback(traceback);
    }
    for reason in extra.profile_failures {
        report.push_profile_failure(reason);
    }
    report.refresh_status();
}

fn apply_runtime_command_normalizations(step: &mut PlanStep, report: &VerificationReport) {
    for normalization in &report.runtime_command_normalizations {
        for command in &mut step.verify {
            if command == &normalization.original {
                *command = normalization.repaired.clone();
            }
        }
    }
}

fn step_run_session_options(
    plan: &StepPlan,
    step: &PlanStep,
    contract_enforcement: ContractEnforcement,
    phase_scope: Option<&str>,
    setup_authority: NodeDependencySetupAuthority,
) -> RunSessionOptions {
    RunSessionOptions::plan_step_with_enforcement(
        run_session_step_kind(step),
        contract_enforcement,
        phase_scope.map(str::to_string),
    )
    .with_dependency_setup_authority(setup_authority)
    .with_path_fallback_candidates(plan_expected_paths(plan))
}

fn plan_expected_paths(plan: &StepPlan) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for path in plan
        .steps
        .iter()
        .flat_map(|step| step.expected_paths.iter())
    {
        if seen.insert(path.clone()) {
            out.push(path.clone());
        }
    }
    out
}

fn verify_step_completion_observed(
    config: &Config,
    goal: &str,
    runtime_step: &PlanStep,
    original_step: &PlanStep,
    phase_scope: Option<&str>,
    setup_authority: NodeDependencySetupAuthority,
) -> (VerificationReport, Vec<BuildVerifierLifecycleObservation>) {
    let (mut report, mut build_lifecycles) = verify_step_with_context(
        &config.workspace_root,
        runtime_step,
        Some(&config.profile),
        Some(goal),
        setup_authority,
        config.offline,
        config.eval_events_path.as_deref(),
    );
    if should_run_setup_step_dependency_state_lifecycle(
        &config.workspace_root,
        original_step,
        phase_scope,
        setup_authority,
    ) {
        let (dependency_report, mut dependency_lifecycles) =
            verify_setup_dependency_state_with_setup_observed_with_offline(
                &config.workspace_root,
                setup_authority,
                config.offline,
            );
        merge_verification_report(&mut report, dependency_report);
        build_lifecycles.append(&mut dependency_lifecycles);
    }
    (report, build_lifecycles)
}

fn emit_runner_step_short_circuited(
    config: &Config,
    step: &PlanStep,
    phase_scope: Option<&str>,
    required_paths: &[String],
    at: &str,
) {
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "step_short_circuited",
            "at": at,
            "step_id": step.id,
            "step_kind": run_session_step_kind(step).as_str(),
            "phase_scope": phase_scope.unwrap_or(""),
            "required_paths": required_paths,
            "verify_commands": step.verify.clone(),
            "session_scope": "plan-run-step",
        }),
    );
}

fn run_session_step_kind(step: &PlanStep) -> RunSessionStepKind {
    match step.step_kind() {
        StepKind::Inspect => RunSessionStepKind::Inspect,
        StepKind::Setup => RunSessionStepKind::Setup,
        StepKind::Implement => RunSessionStepKind::Implement,
        StepKind::Verify => RunSessionStepKind::Verify,
        StepKind::Report => RunSessionStepKind::Report,
        StepKind::Unknown(_) => RunSessionStepKind::Unknown,
    }
}

fn capped_config(config: &Config, cap: usize) -> Config {
    let mut out = config.clone();
    out.max_iterations = out.max_iterations.min(cap);
    out
}

fn required_final_artifacts(plan: &StepPlan, root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    merge_unique_strings(
        &mut out,
        &extract_requested_artifact_paths(root, &plan.goal),
    );
    for step in &plan.steps {
        merge_unique_strings(&mut out, &step.expected_paths);
    }
    out
}

fn explicit_completion_contract_path(config: &Config) -> Option<PathBuf> {
    config.completion_contract_path.clone().or_else(|| {
        crate::env_compat::var_os("COMMANDAGENT_COMPLETION_CONTRACT").map(PathBuf::from)
    })
}

fn generated_completion_contract_path(config: &Config, scope: &str) -> PathBuf {
    let filename = format!(
        "completion-contract-{}.json",
        scope
            .chars()
            .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
            .collect::<String>()
    );
    if let Some(parent) = config
        .eval_events_path
        .as_ref()
        .and_then(|path| path.parent())
    {
        return parent.join(filename);
    }
    config.workspace_root.join(".anvil").join(filename)
}

fn display_path_for_event(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

fn emit_completion_contract_bound(
    config: &Config,
    scope: &str,
    profile: &str,
    goal: &str,
    bound: &BoundCompletionContract,
) {
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "completion_contract_bound",
            "session_scope": scope,
            "completion_contract_verification_enabled": true,
            "completion_contract_path_merge_enabled": true,
            "external_contract_checked": true,
            "external_contract_required": bound.required,
            "completion_contract_generated": bound.generated,
            "completion_contract_path": bound.path,
            "required_paths": bound.contract.required_paths.clone(),
            "required_capabilities": bound.contract.required_capabilities.clone(),
            "required_evidence": bound.contract.required_evidence.clone(),
            "evidence_hint_tokens": bound.contract.evidence_hint_tokens.clone(),
            "required_obligations": bound.contract.required_obligations.clone(),
        }),
    );
    // E5B_PROFILE_DISPATCH_ALLOW: telemetry-generic-contract
    if ProfileId::parse(profile) == ProfileId::Generic
        && bound
            .contract
            .required_capabilities
            .iter()
            .any(|capability| capability == GENERIC_INTERACTIVE_CONTRACT_CAPABILITY)
        && let Some(matched_intent_token) = signals::matched_app_intent_token(goal)
    {
        eval_events::emit(
            config.eval_events_path.as_deref(),
            json!({
                "event": "generic_contract_bound",
                "session_scope": scope,
                "matched_intent_token": matched_intent_token,
                "inferred_keys": GENERIC_INTERACTIVE_EVIDENCE_KEYS,
                "required_capabilities": bound.contract.required_capabilities.clone(),
                "required_evidence": bound.contract.required_evidence.clone(),
                "required_obligations": bound.contract.required_obligations.clone(),
                "completion_contract_path": bound.path,
            }),
        );
    }
}

fn verify_plan_final_contract(
    plan: &StepPlan,
    required_final_artifacts: &[String],
    config: &Config,
    bound_contract: Option<&BoundCompletionContract>,
) -> anyhow::Result<()> {
    let mut required_paths = required_final_artifacts.to_vec();
    let profile_id = ProfileId::parse(&config.profile);
    let runtime = ProfileRuntimeRegistry::resolve(&profile_id);
    let mut required_capabilities = runtime.required_capabilities(&plan.goal);
    let mut required_evidence = runtime.required_evidence(&plan.goal, &required_capabilities);
    let mut required_obligations =
        runtime.required_obligations(&profile_id, &plan.goal, &required_capabilities);
    let mut evidence_hint_tokens = evidence_hint_tokens_for_goal(&plan.goal);
    let owned_bound_contract;
    let bound_contract = if let Some(bound_contract) = bound_contract {
        Some(bound_contract)
    } else {
        owned_bound_contract = bind_completion_contract_for_acceptance(
            config,
            "plan-run",
            &config.profile,
            &plan.goal,
            &required_paths,
            &required_capabilities,
            &required_evidence,
            &required_obligations,
        )?;
        owned_bound_contract.as_ref()
    };
    let mut verify_commands = Vec::new();
    let mut deferred_commands = Vec::new();
    if let Some(bound) = bound_contract {
        let contract = &bound.contract;
        merge_unique_strings(&mut required_paths, &contract.required_paths);
        merge_unique_strings(&mut required_capabilities, &contract.required_capabilities);
        merge_unique_strings(&mut required_evidence, &contract.required_evidence);
        merge_unique_strings(&mut required_obligations, &contract.required_obligations);
        merge_unique_strings(&mut verify_commands, &contract.verify_commands);
        merge_unique_strings(&mut evidence_hint_tokens, &contract.evidence_hint_tokens);
        deferred_commands.extend(
            contract
                .deferred_verify_requirements
                .iter()
                .map(|requirement| requirement.command.clone()),
        );
    }
    merge_unique_strings(
        &mut required_evidence,
        &runtime.required_evidence(&plan.goal, &required_capabilities),
    );
    let missing_final_artifacts = missing_final_artifacts(&config.workspace_root, &required_paths);
    let external_report = bound_contract.map(|bound| {
        bound
            .contract
            .verify_with_goal(&config.workspace_root, &plan.goal)
    });
    let runtime_acceptance_required = !required_capabilities.is_empty()
        || !required_evidence.is_empty()
        || !required_obligations.is_empty();
    let mut runtime_acceptance = runtime_acceptance_required.then(|| {
        verify_runtime_acceptance_with_browser_dirs_and_hints(
            &config.workspace_root,
            &required_paths,
            &verify_commands,
            &required_capabilities,
            &required_evidence,
            &required_obligations,
            &deferred_commands,
            &release_evidence_extra_dirs(config),
            &evidence_hint_tokens,
        )
    });
    let evidence_arbitration = runtime_acceptance.as_mut().map(|report| {
        final_acceptance_evidence_arbitration(
            config,
            report,
            &required_capabilities,
            &required_evidence,
            &required_obligations,
        )
    });
    let mut release_gate = final_acceptance_release_gate_with_runtime(
        config,
        runtime,
        &plan.goal,
        &required_capabilities,
        runtime_acceptance.as_ref(),
        false,
    );
    crate::planner::interaction_qualification::enforce_release_gate(
        &mut release_gate.status,
        &mut release_gate.reasons,
        &mut release_gate.interaction_evidence_status,
        &release_gate.interaction_evidence_path,
        crate::planner::interaction_qualification::contract_requires_restart(
            &required_capabilities,
            &required_evidence,
        ),
    );
    let contract_required =
        runtime.requires_completion_contract(&profile_id, &plan.goal, &required_capabilities)
            || bound_contract.is_some_and(|bound| bound.required);
    let external_contract_checked = bound_contract.is_some();
    let contract_binding_missing = contract_required && !external_contract_checked;
    let external_ok = !contract_binding_missing
        && external_contract_ok_after_runtime_arbitration(
            external_report.as_ref(),
            runtime_acceptance.as_ref(),
        );
    let runtime_ok = runtime_acceptance
        .as_ref()
        .is_none_or(|report| report.passed);
    let release_gate_failed = release_gate.status == "failed";
    let ok =
        missing_final_artifacts.is_empty() && external_ok && runtime_ok && !release_gate_failed;
    let final_acceptance_status = release_gate_final_acceptance_status(&release_gate);
    let runtime_acceptance_status =
        runtime_acceptance_status(runtime_ok, runtime_acceptance.as_ref());
    let (assurance_level, assurance_reason) =
        runtime.assurance_for_completion(&profile_id, &required_capabilities);
    let release_quality_completion =
        release_quality_completion_status(&release_gate, final_acceptance_status);
    let next_action = release_gate_next_action(&release_gate, final_acceptance_status);
    let state_dimensions_changed =
        interaction_state_dimensions_changed_from_path(&release_gate.interaction_evidence_path);
    let action_hooks = interaction_action_hooks_from_path(&release_gate.interaction_evidence_path);
    let surface_fit = interaction_surface_fit_from_path(&release_gate.interaction_evidence_path);
    let text_telemetry =
        interaction_text_telemetry_from_path(&release_gate.interaction_evidence_path);
    let depth_profile = depth_profile(
        &config.workspace_root,
        &config.profile,
        &state_dimensions_changed,
        &action_hooks,
        &release_gate.interaction_evidence_path,
        &text_telemetry,
    );
    let requested_port =
        effective_requested_port(resolve_profile_runtime(&config.profile), &plan.goal, None);
    let primary_reason = if !missing_final_artifacts.is_empty() {
        format!(
            "missing final artifacts: {}",
            missing_final_artifacts.join(", ")
        )
    } else if contract_binding_missing {
        "completion contract binding required but missing".to_string()
    } else if let Some(report) = runtime_acceptance.as_ref().filter(|report| !report.passed) {
        report.primary_reason.clone()
    } else if let Some(report) = external_report.as_ref().filter(|report| {
        !external_contract_ok_after_runtime_arbitration(Some(*report), runtime_acceptance.as_ref())
    }) {
        report.primary_reason()
    } else if release_gate_failed {
        format!("release gate failed: {}", release_gate.reasons.join("; "))
    } else {
        "ok".to_string()
    };
    let recovery_handoff = if !ok || release_recovery_needed(&release_gate, final_acceptance_status)
    {
        let acceptance_layer =
            release_recovery_acceptance_layer(&release_gate, final_acceptance_status);
        let failure_kind =
            release_recovery_failure_kind(&release_gate, final_acceptance_status, &primary_reason);
        let scope = format!("release-{}", recovery_scope_token(acceptance_layer));
        save_release_recovery_handoff(
            config,
            &config.profile,
            &plan.goal,
            &scope,
            acceptance_layer,
            &failure_kind,
            release_recovery_failure_evidence(
                &config.profile,
                &plan.goal,
                &release_gate,
                final_acceptance_status,
                &primary_reason,
                runtime_acceptance.as_ref(),
            ),
            missing_final_artifacts.clone(),
            release_recovery_missing_capabilities(runtime_acceptance.as_ref()),
            release_recovery_repair_targets(&release_gate, runtime_acceptance.as_ref()),
            release_recovery_verify_commands(&config.profile, &release_gate),
        )
    } else {
        None
    };
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "plan_final_contract",
            "profile": config.profile,
            "requested_port": requested_port.as_ref().map(|requested| requested.telemetry.clone()),
            "required_final_artifacts": required_paths,
            "missing_final_artifacts": missing_final_artifacts,
            "completion_contract_verification_enabled": external_contract_checked,
            "completion_contract_path_merge_enabled": external_contract_checked,
            "completion_contract_path": bound_contract
                .map(|bound| bound.path.clone())
                .unwrap_or_default(),
            "completion_contract_generated": bound_contract
                .map(|bound| bound.generated)
                .unwrap_or(false),
            "external_contract_checked": external_contract_checked,
            "external_contract_required": contract_required,
            "external_contract_ok": external_ok,
            "required_capabilities": required_capabilities,
            "required_evidence": required_evidence,
            "required_obligations": required_obligations,
            "missing_capabilities": runtime_acceptance
                .as_ref()
                .map(|report| report.missing_capabilities.clone())
                .unwrap_or_default(),
            "missing_evidence": runtime_acceptance
                .as_ref()
                .map(|report| report.missing_evidence.clone())
                .unwrap_or_default(),
            "missing_obligations": runtime_acceptance
                .as_ref()
                .map(|report| report.missing_obligations.clone())
                .unwrap_or_default(),
            "weak_evidence": runtime_acceptance
                .as_ref()
                .map(|report| report.weak_evidence.clone())
                .unwrap_or_default(),
            "runtime_acceptance_diagnostics": runtime_acceptance
                .as_ref()
                .map(|report| report.diagnostics.clone())
                .unwrap_or_default(),
            "unverified_evidence": runtime_acceptance
                .as_ref()
                .map(|report| report.unverified_evidence.clone())
                .unwrap_or_default(),
            "evidence_tiers": runtime_acceptance
                .as_ref()
                .map(|report| report.evidence_tiers.clone())
                .unwrap_or_default(),
            "evidence_arbitration": evidence_arbitration
                .as_ref()
                .map(|report| report.records.clone())
                .unwrap_or_default(),
            "evidence_arbitration_summary": evidence_arbitration
                .as_ref()
                .map(|report| report.summary.clone())
                .unwrap_or_default(),
            "artifact_obligations": runtime_acceptance
                .as_ref()
                .map(|report| report.artifact_obligations.clone())
                .unwrap_or_default(),
            "capability_evidence_bindings": runtime_acceptance
                .as_ref()
                .map(|report| report.capability_evidence_bindings.clone())
                .unwrap_or_default(),
            "obligation_repair_targets": runtime_acceptance
                .as_ref()
                .map(|report| report.obligation_repair_targets.clone())
                .unwrap_or_default(),
            "inconclusive_reasons": runtime_acceptance
                .as_ref()
                .map(|report| report.inconclusive_reasons.clone())
                .unwrap_or_default(),
            "runtime_acceptance_inconclusive": runtime_acceptance
                .as_ref()
                .map(|report| report.inconclusive)
                .unwrap_or(false),
            "runtime_acceptance_passed": runtime_ok,
            "runtime_acceptance_status": runtime_acceptance_status,
            "final_acceptance_status": final_acceptance_status,
            "assurance_level": assurance_level,
            "assurance_reason": assurance_reason,
            "release_quality_completion": release_quality_completion,
            "release_gate_status": release_gate.status.clone(),
            "release_gate_reasons": release_gate.reasons.clone(),
            "browser_readiness_status": release_gate.browser_readiness_status.clone(),
            "browser_readiness_evidence_path": release_gate.browser_readiness_evidence_path.clone(),
            "interaction_evidence_status": release_gate.interaction_evidence_status.clone(),
            "interaction_evidence_path": release_gate.interaction_evidence_path.clone(),
            "state_dimensions_changed": state_dimensions_changed,
            "action_hooks": action_hooks,
            "surface_fit": surface_fit.raw,
            "surface_fit_summary": surface_fit.summary,
            "surface_fit_guidance": surface_fit.guidance,
            "text_entry": text_telemetry.text_entry,
            "text_entry_target": text_telemetry.text_entry_target,
            "typed_token": text_telemetry.typed_token,
            "token_echoed": text_telemetry.token_echoed,
            "echo_latency_ms": text_telemetry.echo_latency_ms,
            "token_echoed_after_reload": text_telemetry.token_echoed_after_reload,
            "token_echo_after_reload_latency_ms": text_telemetry.token_echo_after_reload_latency_ms,
            "text_input_state_change": text_telemetry.text_input_state_change,
            "next_action": next_action,
            "recovery_handoff_kind": recovery_handoff
                .as_ref()
                .map(|handoff| handoff.recovery_handoff_kind.as_str())
                .unwrap_or_default(),
            "acceptance_layer": recovery_handoff
                .as_ref()
                .map(|handoff| handoff.acceptance_layer.as_str())
                .unwrap_or_default(),
            "recovery_prompt_path": recovery_handoff
                .as_ref()
                .map(|handoff| handoff.recovery_prompt_path.as_str())
                .unwrap_or_default(),
            "recovery_ultra_plan_path": recovery_handoff
                .as_ref()
                .map(|handoff| handoff.recovery_ultra_plan_path.as_str())
                .unwrap_or_default(),
            "suggested_recovery_command": recovery_handoff
                .as_ref()
                .map(|handoff| handoff.suggested_recovery_command.as_str())
                .unwrap_or_default(),
            "suggested_recovery_yaml_command": recovery_handoff
                .as_ref()
                .map(|handoff| handoff.suggested_recovery_yaml_command.as_str())
                .unwrap_or_default(),
            "recovery_handoff_saved": recovery_handoff
                .as_ref()
                .is_some_and(ReleaseRecoveryHandoffSummary::has_artifact),
            "handoff_saved_not_success": recovery_handoff.is_some(),
            "ok": ok,
            "primary_reason": eval_events::body_snippet(&primary_reason),
        }),
    );
    emit_depth_profile(
        config.eval_events_path.as_deref(),
        "plan_final_contract",
        &depth_profile,
    );
    if ok {
        return Ok(());
    }
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "loop_stop",
            "reason": "plan_final_contract_failure",
            "primary_reason": eval_events::body_snippet(&primary_reason),
        }),
    );
    anyhow::bail!("plan final contract failed: {primary_reason}")
}

fn missing_final_artifacts(root: &Path, required_paths: &[String]) -> Vec<String> {
    required_paths
        .iter()
        .filter(|path| resolve_existing(root, path).is_err())
        .cloned()
        .collect()
}

#[derive(Debug, Clone)]
struct ProfileInvariantFailureEvidence {
    report: VerificationReport,
    missing_paths: Vec<String>,
    failure_evidence: Vec<String>,
}

fn fresh_profile_invariant_failure_evidence(
    config: &Config,
    plan: &UltraPlan,
    snapshot: &ProfileSnapshot,
    required_paths: &[String],
) -> ProfileInvariantFailureEvidence {
    let runtime = resolve_profile_runtime(&plan.profile);
    let report = verify_invariant_with_hooks(config, runtime, plan, snapshot);
    let mut required =
        runtime.invariant_expected_paths(&config.workspace_root, required_paths.to_vec());
    merge_unique_strings(
        &mut required,
        &runtime.invariant_setup_paths(&config.workspace_root),
    );
    let mut missing_paths = missing_final_artifacts(&config.workspace_root, &required);
    merge_unique_strings(&mut missing_paths, &report.missing_paths);

    let missing_imports = profile_missing_relative_imports(&config.workspace_root, &plan.profile);
    let import_findings = format_missing_import_findings(&config.workspace_root, &missing_imports);
    merge_unique_strings(
        &mut missing_paths,
        &repair_targeting::missing_import_target_paths(&config.workspace_root, &missing_imports),
    );
    if missing_paths.is_empty() && !missing_imports.is_empty() {
        merge_unique_strings(&mut missing_paths, &import_findings);
    }

    let mut failure_evidence = vec![report.primary_reason()];
    if !import_findings.is_empty() {
        failure_evidence.push(format!(
            "Missing relative imports:\n{}",
            render_prompt_bullets(&import_findings)
        ));
    }

    ProfileInvariantFailureEvidence {
        report,
        missing_paths,
        failure_evidence,
    }
}

fn verify_invariant_with_hooks(
    config: &Config,
    runtime: &dyn ProfileRuntime,
    plan: &UltraPlan,
    snapshot: &ProfileSnapshot,
) -> VerificationReport {
    let report = runtime.verify_phase_invariant(&config.workspace_root, &plan.goal, snapshot);
    hook_snapshot::report_missing_as_profile_failure_with_runtime(
        config, runtime, &plan.goal, report,
    )
}

fn profile_missing_relative_imports(root: &Path, profile: &str) -> Vec<MissingImport> {
    let paths = resolve_profile_runtime(profile).source_paths(root);
    if paths.is_empty() {
        return Vec::new();
    }
    scan_relative_imports(root, &paths).unwrap_or_default()
}

fn merge_unique_strings(out: &mut Vec<String>, incoming: &[String]) {
    for item in incoming {
        if !out.contains(item) {
            out.push(item.clone());
        }
    }
}

fn step_run_outcome_from_session_error(err: &anyhow::Error, stop_reason: &str) -> StepRunOutcome {
    let message = err.to_string();
    let mut outcome = StepRunOutcome {
        primary_failure: Some(message.clone()),
        stop_reason: Some(stop_reason.to_string()),
        partial: true,
        ..StepRunOutcome::default()
    };
    apply_session_error_observations(&mut outcome, err, &message);
    outcome
}

fn apply_session_error_observations(
    outcome: &mut StepRunOutcome,
    err: &anyhow::Error,
    message: &str,
) {
    if let Some(session_error) = err.downcast_ref::<RunSessionError>() {
        merge_unique_strings(
            &mut outcome.observed_missing_capabilities,
            &session_error.context.missing_capabilities,
        );
        merge_unique_strings(
            &mut outcome.observed_missing_evidence,
            &session_error.context.missing_evidence,
        );
        merge_unique_strings(
            &mut outcome.observed_missing_obligations,
            &session_error.context.missing_obligations,
        );
        if let Some(repair_target) = session_error.context.repair_target.as_ref() {
            merge_unique_strings(
                &mut outcome.repair_targets,
                std::slice::from_ref(repair_target),
            );
        }
    }

    let missing_capabilities =
        missing_signal_values_after_prefix(message, "missing_required_capabilities:");
    let missing_evidence =
        missing_signal_values_after_prefix(message, "missing_required_evidence:");
    let missing_obligations =
        missing_signal_values_after_prefix(message, "missing_required_obligations:");
    merge_unique_strings(
        &mut outcome.observed_missing_capabilities,
        &missing_capabilities,
    );
    merge_unique_strings(&mut outcome.observed_missing_evidence, &missing_evidence);
    merge_unique_strings(
        &mut outcome.observed_missing_obligations,
        &missing_obligations,
    );
    repair_targeting::ensure_session_error_repair_target(outcome);
}

fn exhaustion_reason_with_pending_contract_state(message: &str, pending_keys: &[String]) -> String {
    if !is_exhaustion_message(message) {
        return message.to_string();
    }
    capability_evidence_unresolved_reason(pending_keys).unwrap_or_else(|| message.to_string())
}

fn is_exhaustion_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("loop_progress_exhausted")
        || lower.contains("progress_exhausted")
        || lower.contains("iteration")
        || lower.contains("exhausted")
}

fn capability_evidence_failure_evidence(
    root: &Path,
    profile: &str,
    pending_keys: &[String],
    reason: &str,
) -> Vec<String> {
    let mut evidence = capability_evidence_remedy_lines(pending_keys);
    if pending_keys
        .iter()
        .any(|key| key == "restart_or_recoverable_state_evidence")
    {
        merge_unique_strings(
            &mut evidence,
            &restart_hook_attachment_guidance(root, profile),
        );
    }
    if evidence.is_empty() {
        evidence.push(reason.to_string());
    } else {
        evidence.push(format!("exhaustion classification: {reason}"));
    }
    evidence
}

fn restart_hook_attachment_guidance(root: &Path, profile: &str) -> Vec<String> {
    let mut out = Vec::new();
    for rel in resolve_profile_runtime(profile).route_bound_closure(root) {
        if !restart_hook_scan_candidate(&rel) {
            continue;
        }
        let full = root.join(&rel);
        let Ok(content) = std::fs::read_to_string(&full) else {
            continue;
        };
        let lines = content.lines().collect::<Vec<_>>();
        for (index, line) in lines.iter().enumerate() {
            if !restart_attachment_candidate_line(line) {
                continue;
            }
            let block = restart_attachment_block(&lines, index);
            if has_restart_action_attribute(&block) || restart_block_is_initial_primary(&block) {
                continue;
            }
            let label = restart_attachment_label(&block);
            let line_number = restart_attachment_line_number(&lines, index);
            out.push(format!(
                "restart hook attachment point: add data-anvil-action=\"restart\" to {label} at {}:{}",
                rel.display(),
                line_number
            ));
        }
    }
    dedup_strings(out)
}

fn restart_hook_scan_candidate(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("tsx" | "jsx" | "ts" | "js")
    )
}

fn restart_attachment_candidate_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    let restart_named_click = lower.contains("onclick")
        && (lower.contains("restart") || lower.contains("reset") || lower.contains("again"));
    let restart_key_handler = (lower.contains("onkeydown")
        || lower.contains("keyup")
        || lower.contains("keydown")
        || lower.contains("addeventlistener"))
        && (line.contains("'r'")
            || line.contains("\"r\"")
            || line.contains("`r`")
            || lower.contains("keyr"));
    restart_named_click || restart_key_handler
}

fn has_restart_action_attribute(line: &str) -> bool {
    line.contains("data-anvil-action=\"restart\"")
        || line.contains("data-anvil-action='restart'")
        || line.contains("data-anvil-action={`restart`}")
}

fn restart_attachment_line_number(lines: &[&str], index: usize) -> usize {
    let start = index.saturating_sub(4);
    for candidate in (start..=index).rev() {
        if lines
            .get(candidate)
            .is_some_and(|line| line.to_ascii_lowercase().contains("<button"))
        {
            return candidate + 1;
        }
    }
    index + 1
}

fn restart_attachment_block(lines: &[&str], index: usize) -> String {
    let mut block = String::new();
    for line in lines.iter().skip(index).take(8) {
        block.push_str(line);
        block.push('\n');
        if line.to_ascii_lowercase().contains("</button>") {
            break;
        }
    }
    block
}

fn restart_block_is_initial_primary(block: &str) -> bool {
    let lower = block.to_ascii_lowercase();
    lower.contains("data-anvil-action=\"primary\"")
        || lower.contains("data-anvil-action='primary'")
        || lower.contains("start game")
        || lower.contains(">start<")
}

fn restart_attachment_label(text: &str) -> &'static str {
    let lower = text.to_ascii_lowercase();
    if lower.contains("try again") {
        "the TRY AGAIN button"
    } else if lower.contains("restart") {
        "the restart button"
    } else if lower.contains("new game") {
        "the NEW GAME button"
    } else if lower.contains("play again") || lower.contains("again") {
        "the PLAY AGAIN button"
    } else if lower.contains("reset") {
        "the reset button"
    } else if lower.contains("keydown") || lower.contains("keyr") {
        "the R-key restart handler's visible restart control"
    } else {
        "the restart-shaped control"
    }
}

fn verification_missing_signals(report: &VerificationReport) -> Vec<String> {
    let mut out = Vec::new();
    merge_unique_strings(
        &mut out,
        &missing_signals_from_text(&report.primary_reason()),
    );
    for failure in &report.profile_failures {
        merge_unique_strings(&mut out, &missing_signals_from_text(failure));
    }
    out
}

fn missing_signals_from_text(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    merge_unique_strings(
        &mut out,
        &missing_signal_values_after_prefix(text, "missing_required_capabilities:"),
    );
    merge_unique_strings(
        &mut out,
        &missing_signal_values_after_prefix(text, "missing_required_evidence:"),
    );
    merge_unique_strings(
        &mut out,
        &missing_signal_values_after_prefix(text, "missing_required_obligations:"),
    );
    merge_unique_strings(
        &mut out,
        &repair_targeting::missing_obligation_targets_from_text(text),
    );
    out
}

fn missing_signal_values_after_prefix(text: &str, prefix: &str) -> Vec<String> {
    let Some((_, rest)) = text.split_once(prefix) else {
        return Vec::new();
    };
    let end = rest
        .find(|ch: char| ch.is_whitespace() || matches!(ch, ';' | ')' | ']'))
        .unwrap_or(rest.len());
    rest[..end]
        .split(',')
        .filter_map(|value| {
            let value = value
                .trim()
                .trim_matches(|ch: char| matches!(ch, '.' | ':'));
            let value = value
                .strip_prefix("required_obligation:")
                .unwrap_or(value)
                .trim();
            (!value.is_empty()).then(|| value.to_string())
        })
        .collect()
}

fn handoff_path(path: &Path) -> String {
    workspace_relative_handoff_path(path)
}

fn optional_handoff_path(path: Option<&PathBuf>) -> String {
    path.map(|path| handoff_path(path.as_path()))
        .unwrap_or_default()
}

fn command_failure_summaries(report: &VerificationReport) -> Vec<String> {
    let mut summaries = report
        .command_failures
        .iter()
        .map(|failure| {
            format!(
                "{}: {}",
                failure.command,
                eval_events::body_snippet(&failure.reason)
            )
        })
        .collect::<Vec<_>>();
    summaries.extend(
        report
            .verifier_command_false_negatives
            .iter()
            .map(|failure| {
                format!(
                    "deterministic_verify_command_bug: {}: {}",
                    failure.command,
                    eval_events::body_snippet(&failure.reason)
                )
            }),
    );
    summaries
}

fn reachability_action_labels(reachability: &RepairReachability) -> Vec<&'static str> {
    reachability
        .viable_actions
        .iter()
        .map(|action| action.as_str())
        .collect()
}

fn reachability_blocked_evidence(blocked_requirements: &[String]) -> Vec<String> {
    blocked_requirements
        .iter()
        .map(|requirement| match requirement.as_str() {
            "dependency_setup_authority_required" => {
                "dependency_setup_authority_required: requires a Setup-authority step running dependency install before verification can pass".to_string()
            }
            "dependency_setup_blocked_offline" => {
                "dependency_setup_blocked_offline: dependency verification requires dependency setup lifecycle, but offline mode blocks install".to_string()
            }
            "deterministic_verify_command_bug" => {
                "deterministic_verify_command_bug: the verify command is malformed; the artifact may already satisfy the requirement".to_string()
            }
            other => format!("repair_unreachable: {other}"),
        })
        .collect()
}

fn emit_repair_unreachable(
    config: &Config,
    mode: &str,
    step_id: &str,
    repair_target: &str,
    primary_reason: &str,
    reachability: &RepairReachability,
) {
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "repair_unreachable",
            "mode": mode,
            "step_id": step_id,
            "reason": reachability_failure_kind(reachability),
            "blocked_requirements": reachability.blocked_requirements.clone(),
            "viable_actions": reachability_action_labels(reachability),
            "repair_target": repair_target,
            "primary_reason": eval_events::body_snippet(primary_reason),
        }),
    );
}

fn verification_report_signature(report: &VerificationReport) -> Vec<String> {
    let mut signature = Vec::new();
    signature.extend(
        report
            .missing_paths
            .iter()
            .map(|path| format!("missing:{path}")),
    );
    signature.extend(
        report
            .dependency_missing
            .iter()
            .map(|reason| format!("dependency:{reason}")),
    );
    signature.extend(report.command_failures.iter().map(|failure| {
        format!(
            "command:{}:{}",
            failure.command,
            normalize_report_reason_for_signature(&failure.reason)
        )
    }));
    signature.extend(
        report
            .verifier_command_false_negatives
            .iter()
            .map(|failure| {
                format!(
                    "verifier_command:{}:{}",
                    failure.command,
                    normalize_report_reason_for_signature(&failure.reason)
                )
            }),
    );
    signature.extend(
        report
            .profile_failures
            .iter()
            .map(|reason| format!("profile:{reason}")),
    );
    signature.extend(
        report
            .python_tracebacks
            .iter()
            .map(|value| value.signature()),
    );
    signature.sort();
    signature
}

fn normalize_report_reason_for_signature(reason: &str) -> String {
    let mut normalized = Vec::new();
    let mut parts = reason.split_whitespace();
    while let Some(part) = parts.next() {
        if part == "elapsed_ms:" {
            let _ = parts.next();
            normalized.push("elapsed_ms:<n>".to_string());
        } else {
            normalized.push(part.to_string());
        }
    }
    normalized.join(" ")
}

fn push_context_items_capped(
    out: &mut Vec<String>,
    incoming: &[String],
    cap: usize,
    truncated: &mut bool,
) {
    for item in incoming {
        push_context_unique_capped(out, item.clone(), cap, truncated);
    }
}

fn push_context_unique_capped(
    out: &mut Vec<String>,
    item: String,
    cap: usize,
    truncated: &mut bool,
) {
    if out.contains(&item) {
        return;
    }
    if out.len() >= cap {
        *truncated = true;
        return;
    }
    out.push(item);
}

fn append_context_list(lines: &mut Vec<String>, label: &str, values: &[String]) {
    if values.is_empty() {
        return;
    }
    lines.push(format!("- {label}:"));
    for value in values {
        lines.push(format!("  - {value}"));
    }
}

fn pending_capability_context_items(keys: &[String]) -> Vec<String> {
    let mut out = keys.to_vec();
    for remedy in capability_evidence_remedy_lines(keys) {
        let line = format!("remedy: {remedy}");
        if !out.contains(&line) {
            out.push(line);
        }
    }
    out
}

fn render_bounded_prompt_section(
    header: &str,
    items: &[String],
    footer: Option<&str>,
    max_lines: usize,
) -> String {
    let footer_lines = usize::from(footer.is_some());
    let available_item_lines = max_lines.saturating_sub(1 + footer_lines).max(1);
    let mut lines = vec![header.to_string()];
    if items.len() > available_item_lines {
        let shown = available_item_lines.saturating_sub(1);
        for item in items.iter().take(shown) {
            lines.push(format!("- {item}"));
        }
        lines.push(format!(
            "- … and {} more",
            items.len().saturating_sub(shown)
        ));
    } else {
        for item in items {
            lines.push(format!("- {item}"));
        }
    }
    if let Some(footer) = footer {
        lines.push(format!("- {footer}"));
    }
    lines.join("\n")
}

fn merge_changed_files(context: &mut RepairContext, incoming: &[String]) {
    for path in incoming {
        if context.changed_files.contains(path) {
            if !context.repeated_changed_files.contains(path) {
                context.repeated_changed_files.push(path.clone());
            }
        } else {
            context.changed_files.push(path.clone());
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn repair_intermediate_profile_invariant(
    execution: &mut dyn ChatClient,
    ultra_session: &mut SessionSnapshot,
    config: &Config,
    plan: &UltraPlan,
    phase: &UltraPhase,
    index: usize,
    profile_snapshot: &ProfileSnapshot,
    step_plan: &StepPlan,
    ultra_context: &mut UltraRunContext,
    final_expected_paths: &[String],
    ui: &dyn InteractionUi,
    failed_report: VerificationReport,
    setup_authority_state: &mut UltraRunSetupAuthorityState,
) -> anyhow::Result<VerificationReport> {
    let mut retry = failed_report.clone();
    let runtime = resolve_profile_runtime(&plan.profile);
    let deterministic_error =
        match runtime.deterministic_repair(&config.workspace_root, &plan.goal, &failed_report) {
            Ok(changed) => {
                if changed {
                    setup_authority_state.grant("manifest_repair");
                    reconcile_run_dependency_setup(
                        config,
                        &plan.profile,
                        DependencyReconciliationTrigger::ManifestRepair,
                        setup_authority_state,
                    )?;
                }
                retry = verify_invariant_with_hooks(
                    config,
                    resolve_profile_runtime(&plan.profile),
                    plan,
                    profile_snapshot,
                );
                emit_profile_invariant_repair_event(
                    config,
                    plan,
                    phase,
                    index,
                    "deterministic",
                    changed,
                    retry.is_pass(),
                    &retry.primary_reason(),
                );
                None
            }
            Err(err) => {
                let message = err.to_string();
                emit_profile_invariant_repair_event(
                    config,
                    plan,
                    phase,
                    index,
                    "deterministic",
                    false,
                    false,
                    &message,
                );
                Some(message)
            }
        };
    if retry.is_pass() {
        return Ok(confirm_phase_build_after_profile_repair(
            config, plan, phase, index, step_plan, retry,
        ));
    }

    let expected_paths = runtime.filter_invariant_expected_paths(
        &config.workspace_root,
        runtime.expected_scaffold_paths(&config.workspace_root, &plan.goal),
    );
    let mut hook_snapshot_feedback_given = false;
    let mut repair_prompt = profile_invariant_model_repair_prompt(
        plan,
        phase,
        &retry,
        ultra_context,
        &expected_paths,
        config,
        deterministic_error.as_deref(),
    );
    repair_prompt = hook_snapshot::prefix_feedback_if_missing_with_runtime(
        config,
        runtime,
        &plan.goal,
        "profile_invariant_repair",
        Some(&phase.id),
        &mut hook_snapshot_feedback_given,
        repair_prompt,
    );
    let repair_config = capped_config(config, STEP_REPAIR_MAX_ITERATIONS);
    match run_profile_repair_with_ultra_session(
        execution,
        ultra_session,
        &repair_prompt,
        &plan.intent,
        &expected_paths,
        &repair_config,
        ui,
    ) {
        Ok(repair_outcome) => {
            push_context_items_capped(
                &mut ultra_context.created_or_changed_paths,
                &repair_outcome.changed_paths,
                ULTRA_CONTEXT_MAX_PATHS,
                &mut ultra_context.truncated,
            );
            push_context_items_capped(
                &mut ultra_context.last_repair_changed_paths,
                &repair_outcome.changed_paths,
                ULTRA_CONTEXT_MAX_PATHS,
                &mut ultra_context.truncated,
            );
            emit_ultra_phase_context_updated(
                config,
                plan,
                phase,
                index,
                ultra_context,
                ultra_session.messages.len(),
                true,
            );
            retry = verify_invariant_with_hooks(
                config,
                resolve_profile_runtime(&plan.profile),
                plan,
                profile_snapshot,
            );
            emit_profile_invariant_repair_event(
                config,
                plan,
                phase,
                index,
                "model",
                !repair_outcome.changed_paths.is_empty(),
                retry.is_pass(),
                &retry.primary_reason(),
            );
            if retry.is_pass() {
                return Ok(confirm_phase_build_after_profile_repair(
                    config, plan, phase, index, step_plan, retry,
                ));
            }
            if hook_snapshot_feedback_given
                && let Some(restored) =
                    hook_snapshot::restore_first_missing_with_runtime(config, runtime, &plan.goal)?
            {
                push_context_items_capped(
                    &mut ultra_context.created_or_changed_paths,
                    std::slice::from_ref(&restored.restored_path),
                    ULTRA_CONTEXT_MAX_PATHS,
                    &mut ultra_context.truncated,
                );
                push_context_items_capped(
                    &mut ultra_context.last_repair_changed_paths,
                    std::slice::from_ref(&restored.restored_path),
                    ULTRA_CONTEXT_MAX_PATHS,
                    &mut ultra_context.truncated,
                );
                retry = verify_invariant_with_hooks(
                    config,
                    resolve_profile_runtime(&plan.profile),
                    plan,
                    profile_snapshot,
                );
                emit_profile_invariant_repair_event(
                    config,
                    plan,
                    phase,
                    index,
                    "hook_snapshot_restore",
                    true,
                    retry.is_pass(),
                    &retry.primary_reason(),
                );
                if retry.is_pass() {
                    return Ok(confirm_phase_build_after_profile_repair(
                        config, plan, phase, index, step_plan, retry,
                    ));
                }
            }
        }
        Err(err) => {
            emit_profile_invariant_repair_event(
                config,
                plan,
                phase,
                index,
                "model",
                false,
                false,
                &err.to_string(),
            );
        }
    }
    let fresh_evidence = fresh_profile_invariant_failure_evidence(
        config,
        plan,
        profile_snapshot,
        final_expected_paths,
    );
    retry = fresh_evidence.report.clone();
    if retry.is_pass() {
        return Ok(confirm_phase_build_after_profile_repair(
            config, plan, phase, index, step_plan, retry,
        ));
    }
    ultra_context.update_after_profile_failure(
        phase,
        &retry.primary_reason(),
        fresh_evidence.missing_paths,
    );
    emit_ultra_phase_context_updated(
        config,
        plan,
        phase,
        index,
        ultra_context,
        ultra_session.messages.len(),
        true,
    );
    Ok(retry)
}

#[allow(clippy::too_many_arguments)]
fn emit_profile_invariant_repair_event(
    config: &Config,
    plan: &UltraPlan,
    phase: &UltraPhase,
    index: usize,
    method: &str,
    changed: bool,
    ok: bool,
    reason: &str,
) {
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "profile_invariant_repair",
            "phase_id": phase.id,
            "phase_index": index + 1,
            "total_phases": plan.phases.len(),
            "final_phase": false,
            "method": method,
            "changed": changed,
            "ok": ok,
            "reason": eval_events::body_snippet(reason),
            "bounded_repair": method == "model",
        }),
    );
}

fn confirm_phase_build_after_profile_repair(
    config: &Config,
    plan: &UltraPlan,
    phase: &UltraPhase,
    index: usize,
    step_plan: &StepPlan,
    profile_report: VerificationReport,
) -> VerificationReport {
    let build_commands = phase_build_verify_commands(step_plan);
    if build_commands.is_empty() {
        return profile_report;
    }
    let build_step = PlanStep {
        id: "profile-repair-build".to_string(),
        kind: "verify".to_string(),
        expected_result: "pass".to_string(),
        instruction: "Re-run the phase build verification after profile repair".to_string(),
        expected_paths: Vec::new(),
        verify: build_commands.clone(),
    };
    let (build_report, build_lifecycles) = verify_step_with_profile_setup_observed_with_offline(
        &config.workspace_root,
        &build_step,
        Some(&plan.profile),
        NodeDependencySetupAuthority::None,
        config.offline,
    );
    for lifecycle in &build_lifecycles {
        emit_dependency_build_lifecycle(
            config.eval_events_path.as_deref(),
            "ultra-plan-run",
            Some(&phase.id),
            lifecycle,
        );
    }
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "profile_invariant_repair_build_verify",
            "phase_id": phase.id,
            "phase_index": index + 1,
            "total_phases": plan.phases.len(),
            "commands": build_commands,
            "ok": build_report.is_pass(),
            "reason": eval_events::body_snippet(&build_report.primary_reason()),
        }),
    );
    if build_report.is_pass() {
        let expected_paths = step_plan
            .steps
            .iter()
            .flat_map(|step| step.expected_paths.iter().cloned())
            .collect::<Vec<_>>();
        snapshot_last_known_good_sources(
            config,
            "profile_invariant_repair",
            Some(&phase.id),
            &plan.profile,
            &plan.goal,
            &expected_paths,
        );
        profile_report
    } else {
        build_report
    }
}

fn phase_build_verify_commands(plan: &StepPlan) -> Vec<String> {
    let mut commands = Vec::new();
    for command in plan.steps.iter().flat_map(|step| step.verify.iter()) {
        if is_nextjs_build_verify_command_like(command) && !commands.contains(command) {
            commands.push(command.clone());
        }
    }
    commands
}

fn production_build_lifecycle_passed(lifecycles: &[BuildVerifierLifecycleObservation]) -> bool {
    lifecycles
        .iter()
        .any(|lifecycle| lifecycle.final_status == BuildVerifierStatus::Passed)
}

fn route_bound_source_paths(root: &Path, profile: &str) -> Vec<String> {
    resolve_profile_runtime(profile)
        .route_bound_closure(root)
        .into_iter()
        .filter_map(|path| {
            path.to_str()
                .and_then(safe_source_rel_path)
                .map(|path| path.replace('\\', "/"))
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn depth_profile(
    root: &Path,
    profile: &str,
    state_dimensions_changed: &[String],
    action_hooks: &[String],
    interaction_evidence_path: &str,
    text_telemetry: &InteractionTextTelemetry,
) -> DepthProfile {
    let route_bound_source_line_count = route_bound_source_paths(root, profile)
        .iter()
        .map(|path| source_line_count(&root.join(path)))
        .sum();
    let state_dimensions_count = state_dimensions_changed
        .iter()
        .filter(|value| !value.trim().is_empty())
        .collect::<BTreeSet<_>>()
        .len();
    let source_action_kinds = route_bound_data_anvil_action_kinds(root, profile);
    let data_anvil_action_kind_count = source_action_kinds
        .iter()
        .chain(action_hooks.iter())
        .filter(|value| !value.trim().is_empty())
        .collect::<BTreeSet<_>>()
        .len();
    let input_types_with_observed_state_change_count =
        input_types_with_observed_state_change(interaction_evidence_path, text_telemetry).len();
    let summary = format!(
        "route_bound_source_lines={} state_dimensions={} data_anvil_action_kinds={} input_types_with_observed_state_change={}",
        route_bound_source_line_count,
        state_dimensions_count,
        data_anvil_action_kind_count,
        input_types_with_observed_state_change_count
    );
    DepthProfile {
        route_bound_source_line_count,
        state_dimensions_count,
        data_anvil_action_kind_count,
        input_types_with_observed_state_change_count,
        summary,
    }
}

fn source_line_count(path: &Path) -> usize {
    std::fs::read_to_string(path)
        .map(|content| content.lines().count())
        .unwrap_or(0)
}

fn route_bound_data_anvil_action_kinds(root: &Path, profile: &str) -> Vec<String> {
    route_bound_source_paths(root, profile)
        .iter()
        .filter_map(|path| std::fs::read_to_string(root.join(path)).ok())
        .flat_map(|content| data_anvil_action_kinds_from_source(&content))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn data_anvil_action_kinds_from_source(source: &str) -> Vec<String> {
    ["data-anvil-action=\"", "data-anvil-action='"]
        .into_iter()
        .flat_map(|needle| {
            let quote = needle.chars().last().unwrap_or('"');
            let mut out = Vec::new();
            let mut rest = source;
            while let Some(index) = rest.find(needle) {
                let after = &rest[index + needle.len()..];
                let Some(end) = after.find(quote) else {
                    break;
                };
                let value = after[..end].trim();
                if !value.is_empty() {
                    out.push(value.to_string());
                }
                rest = &after[end + quote.len_utf8()..];
            }
            out
        })
        .collect()
}

fn input_types_with_observed_state_change(
    interaction_evidence_path: &str,
    text_telemetry: &InteractionTextTelemetry,
) -> BTreeSet<String> {
    let mut types = BTreeSet::new();
    if text_telemetry.text_input_state_change == Some(true)
        && let Some(input_type) =
            input_type_from_text_entry_target(&text_telemetry.text_entry_target)
    {
        types.insert(input_type);
    }
    if let Some(value) = read_json_file(interaction_evidence_path)
        && raw_bool_field_deep(&value, "input_state_change") == Some(true)
        && types.is_empty()
    {
        types.insert("control".to_string());
    }
    types
}

fn input_type_from_text_entry_target(target: &str) -> Option<String> {
    let input_type = target.split(':').next()?.trim();
    (!input_type.is_empty()).then(|| input_type.to_string())
}

fn read_json_file(path: &str) -> Option<Value> {
    if path.trim().is_empty() {
        return None;
    }
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn emit_depth_profile(path: Option<&Path>, source_event: &str, profile: &DepthProfile) {
    eval_events::emit(
        path,
        json!({
            "event": "depth_profile",
            "source_event": source_event,
            "depth_profile_summary": profile.summary.as_str(),
            "route_bound_source_line_count": profile.route_bound_source_line_count,
            "state_dimensions_count": profile.state_dimensions_count,
            "data_anvil_action_kind_count": profile.data_anvil_action_kind_count,
            "input_types_with_observed_state_change_count": profile.input_types_with_observed_state_change_count,
        }),
    );
}

fn is_nextjs_build_verify_command_like(command: &str) -> bool {
    let lower = command.trim().to_ascii_lowercase();
    lower == "npm run build"
        || lower.starts_with("npm run build ")
        || lower == "pnpm build"
        || lower.starts_with("pnpm build ")
        || lower == "yarn build"
        || lower.starts_with("yarn build ")
        || lower == "next build"
        || lower.starts_with("next build ")
}

fn profile_invariant_model_repair_prompt(
    plan: &UltraPlan,
    phase: &UltraPhase,
    report: &VerificationReport,
    context: &UltraRunContext,
    expected_paths: &[String],
    config: &Config,
    deterministic_error: Option<&str>,
) -> String {
    let exact_reason = report.primary_reason();
    let expected = render_prompt_bullets(expected_paths);
    let file_excerpts = profile_invariant_offending_file_excerpts(
        &config.workspace_root,
        resolve_profile_runtime(&plan.profile),
        &exact_reason,
    );
    let missing_imports = profile_missing_relative_imports(&config.workspace_root, &plan.profile);
    let import_findings = format_missing_import_findings(&config.workspace_root, &missing_imports);
    let fix_target_guidance = repair_targeting::fix_profile_invariant_target_guidance(
        &config.workspace_root,
        &plan.profile,
        &plan.intent,
        &missing_imports,
    );
    let import_scan_section = if import_findings.is_empty() {
        "Current missing relative imports:\n- none".to_string()
    } else {
        format!(
            "Current missing relative imports:\n{}",
            render_prompt_bullets(&import_findings)
        )
    };
    let deterministic_note = deterministic_error
        .map(|err| format!("\nDeterministic repair error:\n{err}\n"))
        .unwrap_or_default();
    format!(
        "Repair this intermediate profile invariant failure in one bounded turn.\n\n\
Original ultra goal:\n{goal}\n\n\
Profile: {profile}\nIntent: {intent}\nPhase id: {phase_id}\nPhase task:\n{phase_task}\n\n\
Exact invariant reason:\n\"{exact_reason}\"\n{deterministic_note}\n\
{fix_target_guidance}\
Offending file contents:\n{file_excerpts}\n\n\
{import_scan_section}\n\n\
Expected profile artifacts:\n{expected}\n\n\
{prior_context}\n\n\
Bounded repair rules:\n\
- Repair only the quoted invariant reason without weakening scripts, dependencies, tests, or profile checks.\n\
- Prefer the smallest edit to the offending file shown above.\n\
- For Tailwind, package.json must include tailwindcss/postcss/autoprefixer and postcss.config plugins must include BOTH tailwindcss and autoprefixer.\n\
- Stop after one repair pass; do not start a new planning cycle.",
        goal = plan.goal,
        profile = plan.profile,
        intent = plan.intent,
        phase_id = phase.id,
        phase_task = phase.prompt,
        exact_reason = exact_reason,
        deterministic_note = deterministic_note,
        fix_target_guidance = fix_target_guidance,
        file_excerpts = file_excerpts,
        import_scan_section = import_scan_section,
        expected = expected,
        prior_context = context.render_prompt_section(),
    )
}

fn plan_adherence_report(plan: &UltraPlan, root: &Path) -> PlanAdherenceReport {
    let tokens = ultra_plan_requested_feature_tokens(plan);
    if tokens.is_empty() {
        return PlanAdherenceReport::default();
    }
    let corpus = comment_stripped_source_corpus(root);
    let corpus_lower = corpus.to_ascii_lowercase();
    let mut report = PlanAdherenceReport::default();
    for token in tokens {
        let present = if token.is_ascii() {
            corpus_lower.contains(&token)
        } else {
            corpus.contains(&token)
        };
        if present {
            report.present.push(token);
        } else {
            report.missing.push(token);
        }
    }
    report
}

fn ultra_plan_requested_feature_tokens(plan: &UltraPlan) -> Vec<String> {
    let mut tokens = BTreeSet::new();
    for phase in &plan.phases {
        collect_plan_feature_tokens(&phase.prompt, &mut tokens);
    }
    tokens.into_iter().collect()
}

fn collect_plan_feature_tokens(text: &str, tokens: &mut BTreeSet<String>) {
    let mut ascii = String::new();
    let mut katakana = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            flush_katakana_token(&mut katakana, tokens);
            ascii.push(ch.to_ascii_lowercase());
        } else if is_katakana(ch) {
            flush_ascii_token(&mut ascii, tokens);
            katakana.push(ch);
        } else {
            flush_ascii_token(&mut ascii, tokens);
            flush_katakana_token(&mut katakana, tokens);
        }
    }
    flush_ascii_token(&mut ascii, tokens);
    flush_katakana_token(&mut katakana, tokens);
}

fn flush_ascii_token(token: &mut String, tokens: &mut BTreeSet<String>) {
    if token.len() >= 3
        && !token.chars().all(|ch| ch.is_ascii_digit())
        && !plan_adherence_stopword(token)
    {
        tokens.insert(token.clone());
    }
    token.clear();
}

fn flush_katakana_token(token: &mut String, tokens: &mut BTreeSet<String>) {
    if token.chars().count() >= 2 {
        tokens.insert(token.clone());
    }
    token.clear();
}

fn is_katakana(ch: char) -> bool {
    matches!(ch, '\u{30A0}'..='\u{30FF}' | '\u{31F0}'..='\u{31FF}')
}

fn plan_adherence_stopword(token: &str) -> bool {
    signals::plan_adherence_stopword(token)
}

fn ultra_plan_phase_signal_text(plan: &UltraPlan) -> String {
    plan.phases
        .iter()
        .flat_map(|phase| [phase.id.as_str(), phase.prompt.as_str()])
        .collect::<Vec<_>>()
        .join("\n")
}

fn ultra_plan_signal_text(plan: &UltraPlan) -> String {
    let phase_text = ultra_plan_phase_signal_text(plan);
    if phase_text.is_empty() {
        plan.goal.clone()
    } else {
        format!("{}\n{}", plan.goal, phase_text)
    }
}

fn emit_browser_probe_event(
    config: &Config,
    observation: &BrowserReadinessObservation,
    requested_port: Option<String>,
) {
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "browser_probe",
            "cycle_index": current_final_acceptance_cycle_index(),
            "profile": observation.profile,
            "status": observation.status,
            "ok": observation.ok,
            "requested_port": requested_port,
            "port": observation.port,
            "route": observation.route,
            "command": observation.command,
            "http_status": observation.http_status,
            "failure_kind": observation.failure_kind,
            "elapsed_ms": observation.elapsed_ms,
            "evidence_path": observation.evidence_path.display().to_string(),
            "output_excerpt": eval_events::body_snippet(&observation.output_excerpt),
            "child_spawned": observation.child_spawned,
            "child_reaped": observation.child_reaped,
            "ssr_has_canvas": observation.has_canvas,
            "ssr_interactive_control_count": observation.interactive_control_count,
            "has_canvas": observation.has_canvas,
            "interactive_control_count": observation.interactive_control_count,
            "title_text_excerpt": observation.title_text_excerpt,
        }),
    );
}

fn emit_browser_interaction_probe_event(config: &Config, outcome: &InteractionProbeOutcome) {
    let source_diagnostics = interaction_source_diagnostics(config);
    let source_diagnostic_labels = unattached_ref_diagnostic_labels(&source_diagnostics);
    match outcome {
        InteractionProbeOutcome::Unavailable(reason) => {
            eval_events::emit(
                config.eval_events_path.as_deref(),
                json!({
                    "event": "browser_interaction_probe",
                    "cycle_index": current_final_acceptance_cycle_index(),
                    "status": "unavailable",
                    "ok": false,
                    "failure_kind": reason,
                    "evidence_path": "",
                    "source_diagnostics": &source_diagnostic_labels,
                    "unattached_ref_diagnostics": &source_diagnostics,
                    "playwright_resolution_location": "",
                    "playwright_version": "",
                }),
            );
        }
        InteractionProbeOutcome::Observation(observation) => {
            annotate_interaction_evidence_with_source_diagnostics(
                &observation.evidence_path,
                &source_diagnostics,
            );
            let workspace_evidence =
                interaction_probe::browser_interaction_evidence_path(&config.workspace_root);
            if workspace_evidence != observation.evidence_path {
                annotate_interaction_evidence_with_source_diagnostics(
                    &workspace_evidence,
                    &source_diagnostics,
                );
            }
            let resolution = observation.playwright_resolution.as_ref();
            eval_events::emit(
                config.eval_events_path.as_deref(),
                json!({
                    "event": "browser_interaction_probe",
                    "cycle_index": current_final_acceptance_cycle_index(),
                    "status": observation.status,
                    "ok": observation.ok,
                    "failure_kind": observation.failure_kind,
                    "failure_category": if observation.failure_kind.starts_with("probe_dependency_missing")
                        || observation.failure_kind.starts_with("probe_infrastructure_failed")
                    {
                        "infrastructure"
                    } else if observation.failure_kind.is_empty() {
                        ""
                    } else {
                        "app"
                    },
                    "stage": observation.stage,
                    "error": observation.error,
                    "stderr_excerpt": observation.stderr_excerpt,
                    "server_http_status": observation.server_http_status,
                    "server_http_error": observation.server_http_error,
                    "navigation_failure_kind": observation.navigation_failure_kind,
                    "cold_start_ms": observation.cold_start_ms,
                    "measured_navigation_ms": observation.measured_navigation_ms,
                    "has_canvas": observation.has_canvas,
                    "interactive_control_count": observation.interactive_control_count,
                    "steps": observation.steps,
                    "probe_mode": observation.probe_mode.as_str(),
                    "contract_hook_status": observation.contract_hook_status.as_str(),
                    "candidate_table": &observation.candidate_table,
                    "input_dispatches": &observation.input_dispatches,
                    "canvas_snapshots": &observation.canvas_snapshots,
                    "canvas_blank_before_start": observation.canvas_blank_before_start,
                    "canvas_blank_after_start": observation.canvas_blank_after_start,
                    "canvas_blank_after_inputs": observation.canvas_blank_after_inputs,
                    "source_diagnostics": &source_diagnostic_labels,
                    "unattached_ref_diagnostics": &source_diagnostics,
                    "state_dimensions_changed": &observation.state_dimensions_changed,
                    "surface_fit": &observation.surface_fit,
                    "restart_hook_reachable_after_start": observation.restart_hook_reachable_after_start,
                    "restart_hook_count_after_start": observation.restart_hook_count_after_start,
                    "persistence_after_reload": observation.persistence_after_reload.as_str(),
                    "persistence_after_reload_reason": observation.persistence_after_reload_reason.as_str(),
                    "persistence_changed_dimensions": &observation.persistence_changed_dimensions,
                    "action_hooks": &observation.action_hooks,
                    "text_entry": observation.text_entry.as_str(),
                    "text_entry_target": observation.text_entry_target.as_str(),
                    "typed_token": observation.typed_token.as_str(),
                    "token_echoed": observation.token_echoed,
                    "echo_latency_ms": observation.echo_latency_ms,
                    "token_echoed_after_reload": observation.token_echoed_after_reload,
                    "token_echo_after_reload_latency_ms": observation.token_echo_after_reload_latency_ms,
                    "text_input_state_change": observation.text_input_state_change,
                    "input_state_evaluated_after_start": observation.input_state_evaluated_after_start,
                    "primary_start_transition": observation.primary_transition_observed,
                    "informational_failure_kinds": &observation.informational_failure_kinds,
                    "duration_ms": observation.duration_ms,
                    "evidence_path": observation.evidence_path.display().to_string(),
                    "script_path": observation.script_path.display().to_string(),
                    "output_excerpt": eval_events::body_snippet(&observation.output_excerpt),
                    "child_spawned": observation.child_spawned,
                    "child_reaped": observation.child_reaped,
                    "playwright_resolution_location": resolution
                        .map(|resolution| resolution.location.as_str())
                        .unwrap_or(""),
                    "playwright_module_path": resolution
                        .map(|resolution| resolution.module_path.as_str())
                        .unwrap_or(""),
                    "playwright_node_path": resolution
                        .and_then(|resolution| resolution.node_path.as_deref())
                        .unwrap_or(""),
                    "playwright_version": resolution
                        .map(|resolution| resolution.version.as_str())
                        .unwrap_or(""),
                }),
            );
        }
    }
}

fn interaction_source_diagnostics(config: &Config) -> Vec<UnattachedRefDiagnostic> {
    let profile = config
        .profile_inference
        .map(|inference| inference.profile.to_string())
        .unwrap_or_else(|| config.profile.clone());
    route_bound_unattached_ref_diagnostics(
        &config.workspace_root,
        resolve_profile_runtime(&profile),
    )
}

fn unattached_ref_diagnostic_labels(diagnostics: &[UnattachedRefDiagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.diagnostic.clone())
        .collect()
}

fn annotate_interaction_evidence_with_source_diagnostics(
    path: &Path,
    diagnostics: &[UnattachedRefDiagnostic],
) {
    if diagnostics.is_empty() || !path.is_file() {
        return;
    }
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let Ok(mut value) = serde_json::from_str::<Value>(&text) else {
        return;
    };
    if !value.is_object() {
        return;
    }
    value["source_diagnostics"] = json!(unattached_ref_diagnostic_labels(diagnostics));
    value["unattached_ref_diagnostics"] = json!(diagnostics);
    if let Ok(text) = serde_json::to_string_pretty(&value) {
        let _ = std::fs::write(path, format!("{text}\n"));
    }
}

#[cfg(test)]
fn inferred_required_capabilities(profile: &str, goal: &str) -> Vec<String> {
    resolve_profile_runtime(profile).required_capabilities(goal)
}

#[cfg(test)]
fn inferred_required_evidence(
    profile: &str,
    goal: &str,
    required_capabilities: &[String],
) -> Vec<String> {
    resolve_profile_runtime(profile).required_evidence(goal, required_capabilities)
}

fn runtime_required_evidence(
    runtime: &dyn ProfileRuntime,
    goal: &str,
    required_capabilities: &[String],
) -> Vec<String> {
    runtime.required_evidence(goal, required_capabilities)
}

#[cfg(test)]
fn inferred_required_obligations(
    profile: &str,
    goal: &str,
    required_capabilities: &[String],
) -> Vec<String> {
    let profile_id = ProfileId::parse(profile);
    ProfileRuntimeRegistry::resolve(&profile_id).required_obligations(
        &profile_id,
        goal,
        required_capabilities,
    )
}

fn run_profile_behavior_probe(
    config: &Config,
    profile: &str,
    goal: &str,
    required_capabilities: &[String],
    profile_report: &VerificationReport,
) -> ProfileBehaviorProbeReport {
    if !profile_report.is_pass() {
        return ProfileBehaviorProbeReport::pass();
    }
    let profile_id = ProfileId::parse(profile);
    match resolve_profile_runtime(profile).run_behavior_probe(
        &profile_id,
        &config.workspace_root,
        goal,
        required_capabilities,
        config.offline,
    ) {
        Ok(report) => {
            emit_profile_behavior_probe_event(config, profile, &report);
            report
        }
        Err(err) => {
            let report = ProfileBehaviorProbeReport {
                status: "failed",
                reasons: vec![format!("profile_behavior_probe_error: {err}")],
                evidence_path: None,
            };
            emit_profile_behavior_probe_event(config, profile, &report);
            report
        }
    }
}

fn emit_profile_behavior_probe_event(
    config: &Config,
    profile: &str,
    report: &ProfileBehaviorProbeReport,
) {
    if report.status == "pass" && report.reasons.is_empty() && report.evidence_path.is_none() {
        return;
    }
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "profile_behavior_probe",
            "cycle_index": current_final_acceptance_cycle_index(),
            "profile": profile,
            "status": report.status,
            "ok": report.status == "pass",
            "reasons": report.reasons.clone(),
            "evidence_path": report.evidence_path.clone().unwrap_or_default(),
        }),
    );
}

fn runtime_acceptance_repair_guidance(
    profile: &str,
    goal: &str,
    acceptance: &crate::minimal_loop::evidence::RuntimeAcceptanceReport,
) -> Vec<String> {
    let mut guidance = Vec::new();
    for evidence in &acceptance.missing_evidence {
        match evidence.as_str() {
            "restart_or_recoverable_state_evidence" => {
                guidance.push(crate::minimal_loop::feedback::capability_evidence_remedy_line(
                    evidence,
                ));
            }
            "persistence_evidence" => {
                let contract = crate::planner::profile::interaction_repair_contract(profile, goal);
                guidance.extend(resolve_profile_runtime(profile).interaction_repair_guidance(
                    "browser_interaction_failed:persistence_after_reload_reset",
                    &contract,
                ));
            }
            "live_preview_evidence" | "requested_content_evidence" => {
                guidance.push(TEXT_ECHO_REPAIR_REQUIREMENT.to_string())
            }
            "challenge_or_adversary_evidence" => guidance.push(
                "wire a reachable challenge/adversary entity into state evolution, not only a static label"
                    .to_string(),
            ),
            "failure_or_collision_evidence" => guidance.push(
                "wire a collision/failure conditional that transitions to a reachable failure state"
                    .to_string(),
            ),
            "score_or_progression_evidence" => guidance.push(
                "wire score/progression updates to meaningful state transitions, not only an isolated counter"
                    .to_string(),
            ),
            "stateful_update_evidence" => guidance.push(
                "mutate application state over time or in response to input"
                    .to_string(),
            ),
            "user_input_handler_evidence" => guidance.push(
                "wire keyboard, pointer, click, touch, or form handlers to gameplay state changes"
                    .to_string(),
            ),
            _ => {}
        }
    }
    for evidence in &acceptance.unverified_evidence {
        if evidence == "restart_or_recoverable_state_evidence:unverified:terminal_state_not_reached"
        {
            guidance.push(RESTART_PARTIAL_REPAIR_GUIDANCE.to_string());
        }
    }
    for weak in &acceptance.weak_evidence {
        if let Some(reason) = weak.split(':').next_back()
            && !reason.trim().is_empty()
        {
            guidance.push(reason.trim().to_string());
        }
    }
    for diagnostic in &acceptance.diagnostics {
        if let Some(path) = diagnostic.strip_prefix("route_unbound_capability_artifact:") {
            for evidence in &acceptance.missing_evidence {
                guidance.push(format!(
                    "For missing evidence {evidence}, {path} contains capability code but is not route-bound; import it from the route page, or consolidate into page.tsx and delete the dead component"
                ));
            }
        }
    }
    dedup_strings(guidance)
}

#[derive(Debug, Clone)]
struct NextjsDevServerProbeSpec {
    package_manager: String,
    args: Vec<String>,
    command_display: String,
    port: u16,
    route: String,
}

#[derive(Debug, Clone)]
struct HttpProbeResult {
    status: i64,
    body_excerpt: String,
}

#[cfg(test)]
fn run_nextjs_dev_route_probe(config: &Config, evidence_path: &Path) -> Value {
    run_nextjs_dev_route_probe_with_interaction_options(
        config,
        evidence_path,
        BrowserInteractionProbeOptions::default(),
        None,
    )
}

fn run_nextjs_dev_route_probe_with_interaction_options(
    config: &Config,
    evidence_path: &Path,
    interaction_options: BrowserInteractionProbeOptions,
    requested_port: Option<u16>,
) -> Value {
    run_nextjs_dev_route_probe_with_runtime(
        config,
        evidence_path,
        dev_server_probe_runtime_enabled(config),
        cleanup_dev_server_child,
        interaction_options,
        requested_port,
    )
}

type DevServerCleanupFn = fn(Child, &DevServerLogPaths) -> DevServerCleanup;

fn run_nextjs_dev_route_probe_with_runtime(
    config: &Config,
    evidence_path: &Path,
    runtime_enabled: bool,
    cleanup_fn: DevServerCleanupFn,
    interaction_options: BrowserInteractionProbeOptions,
    requested_port: Option<u16>,
) -> Value {
    if !runtime_enabled {
        let failure_kind = if cfg!(test) {
            "browser_unavailable:dev_server_probe_disabled_in_tests"
        } else {
            "browser_unavailable:dev_server_probe_disabled"
        };
        emit_dev_server_unavailable_lifecycle(
            config,
            NEXTJS_DEV_SERVER_DEFAULT_PORT,
            DEV_SERVER_ROUTE,
            "",
            failure_kind,
            evidence_path,
        );
        return dev_server_unavailable_evidence(
            NEXTJS_DEV_SERVER_DEFAULT_PORT,
            DEV_SERVER_ROUTE,
            "",
            failure_kind,
            "",
        );
    }

    let spec = match load_nextjs_dev_server_probe_spec(&config.workspace_root, requested_port) {
        Ok(spec) => spec,
        Err(failure_kind) => {
            emit_dev_server_unavailable_lifecycle(
                config,
                NEXTJS_DEV_SERVER_DEFAULT_PORT,
                DEV_SERVER_ROUTE,
                "",
                &failure_kind,
                evidence_path,
            );
            return dev_server_unavailable_evidence(
                NEXTJS_DEV_SERVER_DEFAULT_PORT,
                DEV_SERVER_ROUTE,
                "",
                &failure_kind,
                "",
            );
        }
    };

    if localhost_port_accepts_connection(spec.port) {
        let owner = dev_server_port_owner(spec.port);
        if let Some(owner) = &owner
            && owner
                .pid
                .and_then(bounded_process::registered_server_child)
                .is_some()
        {
            let reaped = owner.pid.is_some_and(|pid| {
                bounded_process::reap_registered_server_child(
                    pid,
                    config.eval_events_path.as_deref(),
                    "readiness_port_in_use_retry",
                )
            });
            eval_events::emit(
                config.eval_events_path.as_deref(),
                json!({
                    "event": "dev_server_port_in_use_retry",
                    "port": spec.port,
                    "owner_pid": owner.pid,
                    "owner_command": owner.command,
                    "registered_child": true,
                    "reaped": reaped,
                }),
            );
            std::thread::sleep(Duration::from_millis(150));
        }
    }

    if localhost_port_accepts_connection(spec.port) {
        let owner = dev_server_port_owner(spec.port);
        let failure_kind = "port_in_use";
        let owner_text = owner
            .as_ref()
            .map(DevServerPortOwner::display)
            .unwrap_or_else(|| "unknown owner".to_string());
        let output_excerpt =
            dev_server_output_excerpt_for_port(failure_kind, &owner_text, spec.port);
        emit_dev_server_unavailable_lifecycle(
            config,
            spec.port,
            &spec.route,
            &spec.command_display,
            failure_kind,
            evidence_path,
        );
        return dev_server_unavailable_evidence(
            spec.port,
            &spec.route,
            &spec.command_display,
            failure_kind,
            &output_excerpt,
        );
    }

    let (logs, stdout_log, stderr_log) = match open_dev_server_log_files(evidence_path) {
        Ok(logs) => logs,
        Err(err) => {
            let failure_kind = "browser_unavailable:dev_server_log_open_failed";
            emit_dev_server_unavailable_lifecycle(
                config,
                spec.port,
                &spec.route,
                &spec.command_display,
                failure_kind,
                evidence_path,
            );
            return dev_server_unavailable_evidence(
                spec.port,
                &spec.route,
                &spec.command_display,
                failure_kind,
                &err.to_string(),
            );
        }
    };

    let mut command =
        verifier_env::normalized_command_at_root(&spec.package_manager, &config.workspace_root);
    command
        .args(&spec.args)
        .current_dir(&config.workspace_root)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout_log))
        .stderr(Stdio::from(stderr_log))
        .env("PORT", spec.port.to_string());
    let mut child = match bounded_process::spawn_child(&mut command) {
        Ok(child) => child,
        Err(err) => {
            let failure_kind = dev_server_spawn_failure_kind(&err);
            emit_dev_server_lifecycle_stage(
                config,
                "start",
                false,
                spec.port,
                &spec.route,
                &spec.command_display,
                Some(&failure_kind),
                None,
                evidence_path,
                None,
            );
            emit_dev_server_lifecycle_stage(
                config,
                "wait",
                false,
                spec.port,
                &spec.route,
                &spec.command_display,
                Some(&failure_kind),
                None,
                evidence_path,
                None,
            );
            emit_dev_server_lifecycle_stage(
                config,
                "probe",
                false,
                spec.port,
                &spec.route,
                &spec.command_display,
                Some(&failure_kind),
                None,
                evidence_path,
                None,
            );
            emit_dev_server_lifecycle_stage(
                config,
                "cleanup",
                true,
                spec.port,
                &spec.route,
                &spec.command_display,
                Some(&failure_kind),
                None,
                evidence_path,
                None,
            );
            return dev_server_unavailable_evidence(
                spec.port,
                &spec.route,
                &spec.command_display,
                &failure_kind,
                &err.to_string(),
            );
        }
    };

    let pid = child.id();
    bounded_process::register_server_child(
        &child,
        spec.command_display.clone(),
        format!(
            "final_acceptance_cycle_{}",
            current_final_acceptance_cycle_index()
        ),
        &config.workspace_root,
    );
    emit_dev_server_lifecycle_stage(
        config,
        "start",
        true,
        spec.port,
        &spec.route,
        &spec.command_display,
        None,
        None,
        evidence_path,
        Some(pid),
    );

    let deadline = Instant::now() + NEXTJS_DEV_SERVER_READY_TIMEOUT;
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(_status)) => {
                let output_excerpt = dev_server_logs_excerpt(&logs)
                    .unwrap_or_else(|| "dev server exited before readiness".to_string());
                let failure_kind = classify_dev_server_startup_failure(&output_excerpt)
                    .unwrap_or_else(|| "browser_unavailable:dev_server_exited".to_string());
                let failure_kind = classify_dev_server_env_conflict(&failure_kind, &output_excerpt);
                let output_excerpt =
                    dev_server_output_excerpt_for_port(&failure_kind, &output_excerpt, spec.port);
                emit_dev_server_lifecycle_stage(
                    config,
                    "wait",
                    false,
                    spec.port,
                    &spec.route,
                    &spec.command_display,
                    Some(&failure_kind),
                    None,
                    evidence_path,
                    Some(pid),
                );
                emit_dev_server_lifecycle_stage(
                    config,
                    "probe",
                    false,
                    spec.port,
                    &spec.route,
                    &spec.command_display,
                    Some(&failure_kind),
                    None,
                    evidence_path,
                    Some(pid),
                );
                let evidence = dev_server_unavailable_evidence(
                    spec.port,
                    &spec.route,
                    &spec.command_display,
                    &failure_kind,
                    &output_excerpt,
                );
                write_release_evidence_json(evidence_path, &evidence);
                let cleanup = cleanup_registered_dev_server_child(cleanup_fn, child, &logs);
                emit_dev_server_cleanup_lifecycle_stage(
                    config,
                    cleanup.ok,
                    spec.port,
                    &spec.route,
                    &spec.command_display,
                    Some(&failure_kind),
                    None,
                    evidence_path,
                    Some(pid),
                    &cleanup,
                );
                return evidence;
            }
            Ok(None) => {}
            Err(err) => {
                let failure_kind = "browser_unavailable:dev_server_status_unreadable";
                let log_excerpt = dev_server_logs_excerpt(&logs).unwrap_or_default();
                let combined = format!("{} {}", err, log_excerpt);
                let failure_kind = classify_dev_server_env_conflict(failure_kind, &combined);
                let output_excerpt =
                    dev_server_output_excerpt_for_port(&failure_kind, &combined, spec.port);
                emit_dev_server_lifecycle_stage(
                    config,
                    "wait",
                    false,
                    spec.port,
                    &spec.route,
                    &spec.command_display,
                    Some(&failure_kind),
                    None,
                    evidence_path,
                    Some(pid),
                );
                emit_dev_server_lifecycle_stage(
                    config,
                    "probe",
                    false,
                    spec.port,
                    &spec.route,
                    &spec.command_display,
                    Some(&failure_kind),
                    None,
                    evidence_path,
                    Some(pid),
                );
                let evidence = dev_server_unavailable_evidence(
                    spec.port,
                    &spec.route,
                    &spec.command_display,
                    &failure_kind,
                    &output_excerpt,
                );
                write_release_evidence_json(evidence_path, &evidence);
                let cleanup = cleanup_registered_dev_server_child(cleanup_fn, child, &logs);
                emit_dev_server_cleanup_lifecycle_stage(
                    config,
                    cleanup.ok,
                    spec.port,
                    &spec.route,
                    &spec.command_display,
                    Some(&failure_kind),
                    None,
                    evidence_path,
                    Some(pid),
                    &cleanup,
                );
                return evidence;
            }
        }

        match http_get_local_route(spec.port, &spec.route) {
            Ok(response) => {
                emit_dev_server_lifecycle_stage(
                    config,
                    "wait",
                    true,
                    spec.port,
                    &spec.route,
                    &spec.command_display,
                    None,
                    Some(response.status),
                    evidence_path,
                    Some(pid),
                );
                let failure_kind =
                    classify_dev_route_failure_kind(response.status, &response.body_excerpt);
                let probe_ok = failure_kind.is_none();
                let log_excerpt = dev_server_logs_excerpt(&logs).unwrap_or_default();
                let failure_kind = failure_kind.map(|kind| {
                    let combined = format!("{}\n{}", response.body_excerpt, log_excerpt);
                    classify_dev_server_env_conflict(&kind, &combined)
                });
                let body_excerpt = failure_kind
                    .as_deref()
                    .map(|kind| {
                        dev_server_output_excerpt_for_port(kind, &response.body_excerpt, spec.port)
                    })
                    .unwrap_or_else(|| response.body_excerpt.clone());
                let output_excerpt = failure_kind
                    .as_deref()
                    .map(|kind| dev_server_output_excerpt_for_port(kind, &log_excerpt, spec.port))
                    .unwrap_or_else(|| log_excerpt.clone());
                emit_dev_server_lifecycle_stage(
                    config,
                    "probe",
                    probe_ok && failure_kind.is_none(),
                    spec.port,
                    &spec.route,
                    &spec.command_display,
                    failure_kind.as_deref(),
                    Some(response.status),
                    evidence_path,
                    Some(pid),
                );
                let evidence = if let Some(failure_kind) = failure_kind.as_deref() {
                    dev_server_failed_evidence(
                        spec.port,
                        &spec.route,
                        &spec.command_display,
                        response.status,
                        failure_kind,
                        &body_excerpt,
                        &output_excerpt,
                    )
                } else {
                    dev_server_passed_evidence(
                        spec.port,
                        &spec.route,
                        &spec.command_display,
                        response.status,
                        &response.body_excerpt,
                    )
                };
                write_release_evidence_json(evidence_path, &evidence);
                if failure_kind.is_none() {
                    let interaction_path = evidence_path.with_file_name("browser-interaction.json");
                    let run_dir = evidence_path.parent().unwrap_or(&config.workspace_root);
                    let interaction =
                        interaction_probe::probe_browser_interaction_against_running_server_with_options(
                            &config.workspace_root,
                            spec.port,
                            run_dir,
                            &interaction_path,
                            Duration::from_secs(120),
                            interaction_options,
                        );
                    emit_browser_interaction_probe_event(config, &interaction);
                }
                let cleanup = cleanup_registered_dev_server_child(cleanup_fn, child, &logs);
                emit_dev_server_cleanup_lifecycle_stage(
                    config,
                    cleanup.ok,
                    spec.port,
                    &spec.route,
                    &spec.command_display,
                    failure_kind.as_deref(),
                    Some(response.status),
                    evidence_path,
                    Some(pid),
                    &cleanup,
                );
                return evidence;
            }
            Err(_) => {
                std::thread::sleep(NEXTJS_DEV_SERVER_WAIT_INTERVAL);
            }
        }
    }

    let failure_kind = "startup_timeout";
    let log_excerpt = dev_server_logs_excerpt(&logs).unwrap_or_default();
    let failure_kind = classify_dev_server_env_conflict(failure_kind, &log_excerpt);
    let output_excerpt = dev_server_output_excerpt_for_port(&failure_kind, &log_excerpt, spec.port);
    emit_dev_server_lifecycle_stage(
        config,
        "wait",
        false,
        spec.port,
        &spec.route,
        &spec.command_display,
        Some(&failure_kind),
        None,
        evidence_path,
        Some(pid),
    );
    emit_dev_server_lifecycle_stage(
        config,
        "probe",
        false,
        spec.port,
        &spec.route,
        &spec.command_display,
        Some(&failure_kind),
        None,
        evidence_path,
        Some(pid),
    );
    let evidence = dev_server_unavailable_evidence(
        spec.port,
        &spec.route,
        &spec.command_display,
        &failure_kind,
        &output_excerpt,
    );
    write_release_evidence_json(evidence_path, &evidence);
    let cleanup = cleanup_registered_dev_server_child(cleanup_fn, child, &logs);
    emit_dev_server_cleanup_lifecycle_stage(
        config,
        cleanup.ok,
        spec.port,
        &spec.route,
        &spec.command_display,
        Some(&failure_kind),
        None,
        evidence_path,
        Some(pid),
        &cleanup,
    );
    evidence
}

fn dev_server_probe_runtime_enabled(config: &Config) -> bool {
    if env_flag_is_false("COMMANDAGENT_DEV_SERVER_PROBE") {
        return false;
    }
    if cfg!(test)
        && !env_flag_is_true("COMMANDAGENT_TEST_DEV_SERVER_PROBE")
        && !config
            .workspace_root
            .join(".anvil")
            .join("enable-dev-server-probe-tests")
            .is_file()
    {
        return false;
    }
    true
}

fn env_flag_is_false(name: &str) -> bool {
    crate::env_compat::var(name)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "0" | "false" | "no" | "off"
            )
        })
        .unwrap_or(false)
}

fn env_flag_is_true(name: &str) -> bool {
    crate::env_compat::var(name)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn load_nextjs_dev_server_probe_spec(
    root: &Path,
    requested_port: Option<u16>,
) -> Result<NextjsDevServerProbeSpec, String> {
    let manifest_path = root.join("package.json");
    let text = std::fs::read_to_string(&manifest_path)
        .map_err(|_| "browser_unavailable:package_json_missing".to_string())?;
    let value = serde_json::from_str::<Value>(&text)
        .map_err(|_| "browser_unavailable:package_json_invalid".to_string())?;
    let script = value
        .get("scripts")
        .and_then(Value::as_object)
        .and_then(|scripts| scripts.get("dev"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|script| !script.is_empty())
        .ok_or_else(|| "browser_unavailable:dev_script_missing".to_string())?;
    if !script_contains_next_dev(script) {
        return Err("browser_unavailable:dev_script_not_next_dev".to_string());
    }
    let port = requested_port.unwrap_or(NEXTJS_DEV_SERVER_DEFAULT_PORT);
    let (package_manager, args) = package_manager_dev_command(root);
    let command_display = std::iter::once(package_manager.as_str())
        .chain(args.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ");
    Ok(NextjsDevServerProbeSpec {
        package_manager,
        args,
        command_display,
        port,
        route: DEV_SERVER_ROUTE.to_string(),
    })
}

fn script_contains_next_dev(script: &str) -> bool {
    let lower = script.to_ascii_lowercase();
    lower.contains("next") && lower.contains("dev")
}

fn package_manager_dev_command(root: &Path) -> (String, Vec<String>) {
    if root.join("pnpm-lock.yaml").is_file() {
        return (
            "pnpm".to_string(),
            vec!["run".to_string(), "dev".to_string()],
        );
    }
    if root.join("yarn.lock").is_file() {
        return ("yarn".to_string(), vec!["dev".to_string()]);
    }
    (
        "npm".to_string(),
        vec!["run".to_string(), "dev".to_string()],
    )
}

fn localhost_port_accepts_connection(port: u16) -> bool {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    TcpStream::connect_timeout(&addr, Duration::from_millis(150)).is_ok()
}

fn http_get_local_route(port: u16, route: &str) -> Result<HttpProbeResult, String> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = TcpStream::connect_timeout(&addr, NEXTJS_DEV_SERVER_CONNECT_TIMEOUT)
        .map_err(|err| err.to_string())?;
    let _ = stream.set_read_timeout(Some(NEXTJS_DEV_SERVER_CONNECT_TIMEOUT));
    let _ = stream.set_write_timeout(Some(NEXTJS_DEV_SERVER_CONNECT_TIMEOUT));
    let path = if route.starts_with('/') {
        route.to_string()
    } else {
        format!("/{route}")
    };
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\nUser-Agent: commandagent-dev-server-probe\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|err| err.to_string())?;
    let mut response = Vec::new();
    let mut buffer = [0u8; 4096];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                response.extend_from_slice(&buffer[..n]);
                if response.len() >= 32_768 {
                    break;
                }
            }
            Err(err)
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                break;
            }
            Err(err) => return Err(err.to_string()),
        }
    }
    let response_text = String::from_utf8_lossy(&response).to_string();
    let status_line = response_text
        .lines()
        .next()
        .ok_or_else(|| "empty_http_response".to_string())?;
    let status = status_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| "http_status_missing".to_string())?
        .parse::<i64>()
        .map_err(|_| "http_status_invalid".to_string())?;
    let body = response_text
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .unwrap_or(&response_text);
    Ok(HttpProbeResult {
        status,
        body_excerpt: eval_events::body_snippet(body),
    })
}

fn dev_server_spawn_failure_kind(err: &std::io::Error) -> String {
    match err.kind() {
        std::io::ErrorKind::NotFound => "browser_unavailable:dev_server_command_missing",
        std::io::ErrorKind::PermissionDenied => "browser_unavailable:dev_server_command_denied",
        _ => "browser_unavailable:dev_server_spawn_failed",
    }
    .to_string()
}

fn classify_dev_server_startup_failure(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    if lower.contains("eaddrinuse") || lower.contains("address already in use") {
        return Some("port_in_use".to_string());
    }
    if lower.contains("eacces")
        || lower.contains("permission denied")
        || lower.contains("operation not permitted")
    {
        return Some("bind_denied".to_string());
    }
    if tailwind_dev_pipeline_failure(&lower) {
        return Some("tailwind_dev_pipeline_failure".to_string());
    }
    None
}

fn classify_dev_route_failure_kind(status: i64, body_excerpt: &str) -> Option<String> {
    if status < 400 {
        return None;
    }
    let lower = body_excerpt.to_ascii_lowercase();
    if tailwind_dev_pipeline_failure(&lower) {
        return Some("tailwind_dev_pipeline_failure".to_string());
    }
    Some(format!("http_{status}"))
}

fn classify_dev_server_env_conflict(failure_kind: &str, output: &str) -> String {
    if verifier_env::is_env_node_env_conflict_output(output) {
        verifier_env::ENV_NODE_ENV_CONFLICT_KIND.to_string()
    } else {
        failure_kind.to_string()
    }
}

#[cfg(test)]
fn dev_server_output_excerpt(failure_kind: &str, output: &str) -> String {
    dev_server_output_excerpt_for_port(failure_kind, output, NEXTJS_DEV_SERVER_DEFAULT_PORT)
}

fn dev_server_output_excerpt_for_port(failure_kind: &str, output: &str, port: u16) -> String {
    if failure_kind == verifier_env::ENV_NODE_ENV_CONFLICT_KIND {
        verifier_env::with_env_node_env_remediation(output)
    } else if failure_kind == "port_in_use" {
        port_in_use_remediation(output, port)
    } else {
        output.to_string()
    }
}

fn port_in_use_remediation(output: &str, port: u16) -> String {
    let remediation = format!(
        "Port {port} is already accepting connections. This may be a leftover dev server from a previous run. Inspect it with `lsof -nP -iTCP:{port} -sTCP:LISTEN` and stop the stale process before retrying."
    );
    if output.trim().is_empty() {
        remediation
    } else {
        format!("{output}\n{remediation}")
    }
}

#[derive(Debug, Clone)]
struct DevServerPortOwner {
    pid: Option<u32>,
    command: String,
}

impl DevServerPortOwner {
    fn display(&self) -> String {
        match (self.pid, self.command.trim()) {
            (Some(pid), command) if !command.is_empty() => format!("pid {pid} ({command})"),
            (Some(pid), _) => format!("pid {pid}"),
            (None, command) if !command.is_empty() => command.to_string(),
            (None, _) => "unknown owner".to_string(),
        }
    }
}

fn dev_server_port_owner(port: u16) -> Option<DevServerPortOwner> {
    let port_spec = format!("-iTCP:{port}");
    let mut command = std::process::Command::new("lsof");
    command
        .args(["-nP", &port_spec, "-sTCP:LISTEN", "-F", "pc"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = bounded_process::run_with_timeout(&mut command, Duration::from_secs(2)).ok()?;
    if !output.success() {
        return None;
    }
    parse_dev_server_port_owner(&String::from_utf8(output.stdout).ok()?)
}

fn parse_dev_server_port_owner(text: &str) -> Option<DevServerPortOwner> {
    let mut pid = None;
    let mut command = None;
    for line in text.lines() {
        if let Some(raw) = line.strip_prefix('p') {
            pid = raw.parse::<u32>().ok();
        } else if let Some(raw) = line.strip_prefix('c') {
            command = Some(raw.to_string());
        }
        if pid.is_some() && command.is_some() {
            break;
        }
    }
    pid.or_else(|| command.as_ref().map(|_| 0))
        .map(|pid_value| DevServerPortOwner {
            pid: (pid_value != 0).then_some(pid_value),
            command: command.unwrap_or_default(),
        })
}

fn tailwind_dev_pipeline_failure(lower_text: &str) -> bool {
    lower_text.contains("@tailwind")
        && (lower_text.contains("module parse failed")
            || lower_text.contains("unexpected character")
            || lower_text.contains("postcss")
            || lower_text.contains("tailwind"))
}

#[derive(Debug, Clone)]
struct DevServerLogPaths {
    stdout: PathBuf,
    stderr: PathBuf,
}

fn open_dev_server_log_files(
    evidence_path: &Path,
) -> std::io::Result<(DevServerLogPaths, std::fs::File, std::fs::File)> {
    let dir = evidence_path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;
    let paths = DevServerLogPaths {
        stdout: dir.join("dev-server.out"),
        stderr: dir.join("dev-server.err"),
    };
    let stdout = std::fs::File::create(&paths.stdout)?;
    let stderr = std::fs::File::create(&paths.stderr)?;
    Ok((paths, stdout, stderr))
}

fn dev_server_logs_excerpt(paths: &DevServerLogPaths) -> Option<String> {
    let stdout = read_dev_server_log_excerpt(&paths.stdout).unwrap_or_default();
    let stderr = read_dev_server_log_excerpt(&paths.stderr).unwrap_or_default();
    let combined = format!("{stdout}\n{stderr}");
    let excerpt = eval_events::body_snippet(combined.trim());
    if excerpt.trim().is_empty() {
        None
    } else {
        Some(excerpt)
    }
}

fn read_dev_server_log_excerpt(path: &Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    let start = bytes.len().saturating_sub(DEV_SERVER_LOG_EXCERPT_BYTES);
    Ok(String::from_utf8_lossy(&bytes[start..]).to_string())
}

#[derive(Debug)]
struct DevServerCleanup {
    ok: bool,
    failure_kind: Option<String>,
    output_excerpt: String,
}

fn cleanup_dev_server_child(mut child: Child, logs: &DevServerLogPaths) -> DevServerCleanup {
    #[cfg(unix)]
    {
        cleanup_dev_server_child_unix(&mut child, logs)
    }
    #[cfg(not(unix))]
    {
        cleanup_dev_server_child_non_unix(&mut child, logs)
    }
}

fn cleanup_registered_dev_server_child(
    cleanup_fn: DevServerCleanupFn,
    child: Child,
    logs: &DevServerLogPaths,
) -> DevServerCleanup {
    let pid = child.id();
    let cleanup = cleanup_fn(child, logs);
    bounded_process::unregister_server_child(pid);
    cleanup
}

#[cfg(unix)]
fn cleanup_dev_server_child_unix(child: &mut Child, logs: &DevServerLogPaths) -> DevServerCleanup {
    let pid = child.id();
    let mut notes = Vec::new();
    if let Err(err) = signal_dev_server_process_group(pid, libc::SIGTERM) {
        notes.push(format!("SIGTERM process group failed: {err}"));
    }
    match wait_for_dev_server_process_group_exit(
        child,
        pid,
        Instant::now() + DEV_SERVER_CLEANUP_TERM_TIMEOUT,
    ) {
        Ok(true) => {
            return DevServerCleanup {
                ok: true,
                failure_kind: None,
                output_excerpt: dev_server_logs_excerpt(logs).unwrap_or_default(),
            };
        }
        Ok(false) => {}
        Err(err) => notes.push(format!("wait after SIGTERM failed: {err}")),
    }

    if let Err(err) = signal_dev_server_process_group(pid, libc::SIGKILL) {
        notes.push(format!("SIGKILL process group failed: {err}"));
    }
    match wait_for_dev_server_process_group_exit(
        child,
        pid,
        Instant::now() + DEV_SERVER_CLEANUP_KILL_TIMEOUT,
    ) {
        Ok(true) => DevServerCleanup {
            ok: true,
            failure_kind: None,
            output_excerpt: dev_server_logs_excerpt(logs).unwrap_or_default(),
        },
        Ok(false) => DevServerCleanup {
            ok: false,
            failure_kind: Some("dev_server_cleanup_timeout".to_string()),
            output_excerpt: cleanup_timeout_excerpt(logs, &notes),
        },
        Err(err) => {
            notes.push(format!("wait after SIGKILL failed: {err}"));
            DevServerCleanup {
                ok: false,
                failure_kind: Some("dev_server_cleanup_timeout".to_string()),
                output_excerpt: cleanup_timeout_excerpt(logs, &notes),
            }
        }
    }
}

#[cfg(unix)]
fn signal_dev_server_process_group(pid: u32, signal: libc::c_int) -> std::io::Result<()> {
    let pid =
        i32::try_from(pid).map_err(|_| std::io::Error::other("child pid does not fit pid_t"))?;
    // SAFETY: `kill` is called with a process-group id derived from the child
    // pid returned by `std::process::Child` and a libc signal constant.
    let rc = unsafe { libc::kill(-pid, signal) };
    if rc == 0 {
        return Ok(());
    }
    let err = std::io::Error::last_os_error();
    if err.raw_os_error() == Some(libc::ESRCH) {
        Ok(())
    } else {
        Err(err)
    }
}

#[cfg(unix)]
fn wait_for_dev_server_process_group_exit(
    child: &mut Child,
    pid: u32,
    deadline: Instant,
) -> std::io::Result<bool> {
    let mut child_exited = false;
    loop {
        if !child_exited && child.try_wait()?.is_some() {
            let _ = child.wait();
            child_exited = true;
        }
        if child_exited && !dev_server_process_group_exists(pid) {
            return Ok(true);
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(unix)]
fn dev_server_process_group_exists(pgid: u32) -> bool {
    let Ok(pgid) = i32::try_from(pgid) else {
        return false;
    };
    // SAFETY: signal 0 performs existence/permission checking only, using
    // a process-group id derived from a child process spawned by this probe.
    let rc = unsafe { libc::kill(-pgid, 0) };
    if rc == 0 {
        return true;
    }
    let err = std::io::Error::last_os_error();
    err.raw_os_error() != Some(libc::ESRCH)
}

#[cfg(not(unix))]
fn cleanup_dev_server_child_non_unix(
    child: &mut Child,
    logs: &DevServerLogPaths,
) -> DevServerCleanup {
    let _ = child.kill();
    match wait_for_dev_server_child_exit(child, Instant::now() + DEV_SERVER_CLEANUP_TERM_TIMEOUT) {
        Ok(true) => DevServerCleanup {
            ok: true,
            failure_kind: None,
            output_excerpt: dev_server_logs_excerpt(logs).unwrap_or_default(),
        },
        Ok(false) | Err(_) => DevServerCleanup {
            ok: false,
            failure_kind: Some("dev_server_cleanup_timeout".to_string()),
            output_excerpt: cleanup_timeout_excerpt(logs, &[]),
        },
    }
}

#[cfg(not(unix))]
fn wait_for_dev_server_child_exit(child: &mut Child, deadline: Instant) -> std::io::Result<bool> {
    loop {
        if child.try_wait()?.is_some() {
            let _ = child.wait();
            return Ok(true);
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn cleanup_timeout_excerpt(logs: &DevServerLogPaths, notes: &[String]) -> String {
    let mut parts = notes.to_vec();
    if let Some(log_excerpt) = dev_server_logs_excerpt(logs) {
        parts.push(log_excerpt);
    }
    if parts.is_empty() {
        "dev server cleanup timed out".to_string()
    } else {
        eval_events::body_snippet(&parts.join("\n"))
    }
}

fn cleanup_stage_failure_kind<'a>(
    original_failure_kind: Option<&'a str>,
    cleanup: &'a DevServerCleanup,
) -> Option<&'a str> {
    if !cleanup.ok {
        cleanup.failure_kind.as_deref().or(original_failure_kind)
    } else {
        original_failure_kind
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_dev_server_cleanup_lifecycle_stage(
    config: &Config,
    ok: bool,
    port: u16,
    route: &str,
    command: &str,
    failure_kind: Option<&str>,
    http_status: Option<i64>,
    evidence_path: &Path,
    pid: Option<u32>,
    cleanup: &DevServerCleanup,
) {
    let stage_failure_kind = cleanup_stage_failure_kind(failure_kind, cleanup);
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "dev_server_lifecycle",
            "cycle_index": current_final_acceptance_cycle_index(),
            "profile": "nextjs",
            "stage": "cleanup",
            "ok": ok,
            "port": port,
            "route": route,
            "command": command,
            "failure_kind": stage_failure_kind.unwrap_or(""),
            "http_status": http_status,
            "pid": pid,
            "evidence_path": evidence_path.display().to_string(),
            "output_excerpt": eval_events::body_snippet(&cleanup.output_excerpt),
            "lifecycle_stages": DEV_SERVER_LIFECYCLE_STAGES,
            "probe_environment": dev_server_probe_environment(port),
        }),
    );
}

fn emit_dev_server_unavailable_lifecycle(
    config: &Config,
    port: u16,
    route: &str,
    command: &str,
    failure_kind: &str,
    evidence_path: &Path,
) {
    emit_dev_server_lifecycle_stage(
        config,
        "start",
        false,
        port,
        route,
        command,
        Some(failure_kind),
        None,
        evidence_path,
        None,
    );
    emit_dev_server_lifecycle_stage(
        config,
        "wait",
        false,
        port,
        route,
        command,
        Some(failure_kind),
        None,
        evidence_path,
        None,
    );
    emit_dev_server_lifecycle_stage(
        config,
        "probe",
        false,
        port,
        route,
        command,
        Some(failure_kind),
        None,
        evidence_path,
        None,
    );
    emit_dev_server_lifecycle_stage(
        config,
        "cleanup",
        true,
        port,
        route,
        command,
        Some(failure_kind),
        None,
        evidence_path,
        None,
    );
}

#[allow(clippy::too_many_arguments)]
fn emit_dev_server_lifecycle_stage(
    config: &Config,
    stage: &str,
    ok: bool,
    port: u16,
    route: &str,
    command: &str,
    failure_kind: Option<&str>,
    http_status: Option<i64>,
    evidence_path: &Path,
    pid: Option<u32>,
) {
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "dev_server_lifecycle",
            "cycle_index": current_final_acceptance_cycle_index(),
            "profile": "nextjs",
            "stage": stage,
            "ok": ok,
            "port": port,
            "route": route,
            "command": command,
            "failure_kind": failure_kind.unwrap_or(""),
            "http_status": http_status,
            "pid": pid,
            "evidence_path": evidence_path.display().to_string(),
            "lifecycle_stages": DEV_SERVER_LIFECYCLE_STAGES,
            "probe_environment": dev_server_probe_environment(port),
        }),
    );
}

fn dev_server_probe_environment(port: u16) -> Value {
    json!({
        "NODE_ENV": "",
        "NODE_OPTIONS": "",
        "NEXT_TELEMETRY_DISABLED": "1",
        "PORT": port.to_string(),
        "host_env_contamination": verifier_env::host_env_contamination(),
        "COMMANDAGENT_DEV_SERVER_PROBE": crate::env_compat::var("COMMANDAGENT_DEV_SERVER_PROBE").unwrap_or_default(),
        "COMMANDAGENT_TEST_DEV_SERVER_PROBE": crate::env_compat::var("COMMANDAGENT_TEST_DEV_SERVER_PROBE").unwrap_or_default(),
    })
}

fn dev_server_unavailable_evidence(
    port: u16,
    route: &str,
    command: &str,
    failure_kind: &str,
    output_excerpt: &str,
) -> Value {
    json!({
        "status": "unavailable",
        "browser_failure_kind": failure_kind,
        "failure_kind": failure_kind,
        "dev_server": {
            "profile": "nextjs",
            "port": port,
            "route": route,
            "command": command,
            "failure_kind": failure_kind,
            "output_excerpt": eval_events::body_snippet(output_excerpt),
            "lifecycle_stages": DEV_SERVER_LIFECYCLE_STAGES,
            "probe_environment": dev_server_probe_environment(port),
        }
    })
}

fn dev_server_failed_evidence(
    port: u16,
    route: &str,
    command: &str,
    http_status: i64,
    failure_kind: &str,
    body_excerpt: &str,
    output_excerpt: &str,
) -> Value {
    let mut value = json!({
        "status": "failed",
        "ok": false,
        "http_status": http_status,
        "route_rendered": false,
        "browser_failure_kind": failure_kind,
        "failure_kind": failure_kind,
        "body_excerpt": eval_events::body_snippet(body_excerpt),
        "dev_server": {
            "profile": "nextjs",
            "port": port,
            "route": route,
            "command": command,
            "failure_kind": failure_kind,
            "output_excerpt": eval_events::body_snippet(output_excerpt),
            "lifecycle_stages": DEV_SERVER_LIFECYCLE_STAGES,
            "probe_environment": dev_server_probe_environment(port),
        }
    });
    add_surface_markers_to_evidence(&mut value, body_excerpt);
    value
}

fn dev_server_passed_evidence(
    port: u16,
    route: &str,
    command: &str,
    http_status: i64,
    body_excerpt: &str,
) -> Value {
    let mut value = json!({
        "status": "ready",
        "ok": true,
        "http_status": http_status,
        "route_rendered": true,
        "dev_server": {
            "profile": "nextjs",
            "port": port,
            "route": route,
            "command": command,
            "body_excerpt": eval_events::body_snippet(body_excerpt),
            "lifecycle_stages": DEV_SERVER_LIFECYCLE_STAGES,
            "probe_environment": dev_server_probe_environment(port),
        }
    });
    add_surface_markers_to_evidence(&mut value, body_excerpt);
    value
}

fn add_surface_markers_to_evidence(value: &mut Value, body_excerpt: &str) {
    let markers = html_surface_markers_json(body_excerpt);
    for key in [
        "ssr_has_canvas",
        "ssr_interactive_control_count",
        "has_canvas",
        "interactive_control_count",
        "title_text_excerpt",
        "surface_marker_authority",
        "route_rendered_quality",
    ] {
        value[key] = markers.get(key).cloned().unwrap_or(Value::Null);
    }
}

fn release_recovery_failure_kind(
    release_gate: &ReleaseGateSummary,
    final_acceptance_status: &str,
    primary_reason: &str,
) -> String {
    if release_gate.status == "partial" {
        if release_gate
            .reasons
            .iter()
            .any(|reason| reason.contains("interaction_unverified:terminal_state_not_reached"))
        {
            return "interaction_unverified_terminal_state_not_reached".to_string();
        }
        if release_gate
            .reasons
            .iter()
            .any(|reason| reason.contains("interaction_unverified:probe_unavailable"))
        {
            return "interaction_unverified_probe_unavailable".to_string();
        }
        if release_gate
            .reasons
            .iter()
            .any(|reason| reason.contains("browser_readiness_or_interaction_evidence_required"))
            || release_gate
                .browser_readiness_status
                .starts_with("unavailable:")
            || release_gate
                .browser_readiness_status
                .contains("browser_readiness_evidence_missing")
            || release_gate
                .browser_readiness_status
                .contains("browser_render_evidence_missing")
        {
            return "browser_readiness_missing".to_string();
        }
        if release_gate
            .interaction_evidence_status
            .contains("interaction_evidence_missing")
        {
            return "browser_interaction_evidence_missing".to_string();
        }
        return "release_gate_partial".to_string();
    }
    if release_gate.status == "failed" {
        if release_gate
            .browser_readiness_status
            .contains(verifier_env::ENV_NODE_ENV_CONFLICT_KIND)
            || release_gate
                .reasons
                .iter()
                .any(|reason| reason.contains(verifier_env::ENV_NODE_ENV_CONFLICT_KIND))
        {
            return verifier_env::ENV_NODE_ENV_CONFLICT_KIND.to_string();
        }
        if release_gate
            .browser_readiness_status
            .contains("tailwind_dev_pipeline_failure")
        {
            return "tailwind_dev_pipeline_failure".to_string();
        }
        if release_gate.browser_readiness_status.starts_with("failed:")
            || release_gate
                .reasons
                .iter()
                .any(|reason| reason.contains("browser_readiness_failed"))
        {
            return "browser_readiness_failed".to_string();
        }
        if release_gate
            .interaction_evidence_status
            .starts_with("failed:")
            || release_gate
                .reasons
                .iter()
                .any(|reason| reason.contains("browser_interaction_failed"))
        {
            return "browser_interaction_failed".to_string();
        }
        return "release_gate_failed".to_string();
    }
    if final_acceptance_status == "partial" {
        "final_acceptance_partial".to_string()
    } else if primary_reason == "ok" {
        "final_acceptance_recovery_required".to_string()
    } else {
        "final_acceptance_failed".to_string()
    }
}

fn app_behavior_probe_failure_kind(reason: &str) -> Option<String> {
    let lower = reason.to_ascii_lowercase();
    APP_BEHAVIOR_PROBE_FAILURE_KINDS
        .iter()
        .find(|kind| lower.contains(**kind))
        .map(|kind| format!("browser_interaction_failed:{kind}"))
}

fn release_recovery_failure_evidence(
    profile: &str,
    goal: &str,
    release_gate: &ReleaseGateSummary,
    final_acceptance_status: &str,
    primary_reason: &str,
    runtime_acceptance: Option<&crate::minimal_loop::evidence::RuntimeAcceptanceReport>,
) -> Vec<String> {
    let mut evidence = Vec::new();
    evidence.push(format!(
        "failed acceptance layer: {}",
        release_recovery_acceptance_layer(release_gate, final_acceptance_status)
    ));
    evidence.push(format!(
        "final acceptance status: {final_acceptance_status}"
    ));
    evidence.push(format!("release gate status: {}", release_gate.status));
    if primary_reason != "ok" {
        evidence.push(format!("primary reason: {primary_reason}"));
    }
    evidence.extend(
        release_gate
            .reasons
            .iter()
            .map(|reason| format!("release gate reason: {reason}")),
    );
    evidence.push(format!(
        "browser readiness: {}",
        release_gate.browser_readiness_status
    ));
    if release_gate
        .browser_readiness_status
        .contains(verifier_env::ENV_NODE_ENV_CONFLICT_KIND)
        || release_gate
            .reasons
            .iter()
            .any(|reason| reason.contains(verifier_env::ENV_NODE_ENV_CONFLICT_KIND))
    {
        evidence.push(format!(
            "host environment remediation: {}",
            verifier_env::ENV_NODE_ENV_REMEDIATION
        ));
    }
    if !release_gate.browser_readiness_evidence_path.is_empty() {
        evidence.push(format!(
            "browser readiness evidence: {}",
            release_gate.browser_readiness_evidence_path
        ));
        evidence.extend(
            compile_errors_from_release_evidence_path(
                &release_gate.browser_readiness_evidence_path,
            )
            .into_iter()
            .flat_map(|error| compile_error_repair_guidance(&[error]))
            .map(|line| format!("fix_compile_error: {line}")),
        );
    }
    evidence.push(format!(
        "interaction evidence: {}",
        release_gate.interaction_evidence_status
    ));
    if !release_gate.interaction_evidence_path.is_empty() {
        evidence.push(format!(
            "interaction evidence path: {}",
            release_gate.interaction_evidence_path
        ));
        evidence.extend(interaction_probe_failure_evidence_lines(
            profile,
            goal,
            &release_gate.interaction_evidence_path,
        ));
    }
    if let Some(report) = runtime_acceptance {
        evidence.extend(
            report
                .missing_evidence
                .iter()
                .map(|item| format!("missing runtime evidence: {item}")),
        );
        evidence.extend(
            runtime_acceptance_repair_guidance(profile, goal, report)
                .into_iter()
                .map(|item| format!("runtime repair guidance: {item}")),
        );
        evidence.extend(
            report
                .unverified_evidence
                .iter()
                .map(|item| format!("unverified runtime evidence: {item}")),
        );
        evidence.extend(
            report
                .missing_obligations
                .iter()
                .map(|item| format!("missing runtime obligation: {item}")),
        );
        evidence.extend(
            report
                .inconclusive_reasons
                .iter()
                .map(|item| format!("runtime acceptance inconclusive: {item}")),
        );
    }
    dedup_strings(evidence)
}

fn interaction_probe_failure_evidence_lines(profile: &str, goal: &str, path: &str) -> Vec<String> {
    let Some(value) = std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .filter(Value::is_object)
    else {
        return Vec::new();
    };
    let mut lines = Vec::new();
    let failure_kind = raw_text_field_deep(&value, &["failure_kind", "browser_failure_kind"])
        .map(|kind| format!("browser_interaction_failed:{kind}"))
        .unwrap_or_default();
    lines.extend(interaction_root_cause_repair_guidance(
        profile,
        goal,
        &failure_kind,
        Some(&value),
    ));
    if let Some(cold_start_ms) = raw_u64_field_deep(&value, "cold_start_ms")
        && cold_start_ms > 10_000
    {
        let seconds = (cold_start_ms + 500) / 1000;
        lines.push(format!(
            "Note: first page load took {seconds}s (cold start; excluded from assertions)"
        ));
    }
    lines.extend(
        surface_fit_guidance_lines_from_value(&value)
            .into_iter()
            .map(|line| format!("interaction surface fit: {line}")),
    );
    if let Some(mode) = raw_text_field_deep(&value, &["probe_mode"]).filter(|mode| !mode.is_empty())
    {
        lines.push(format!("interaction probe mode: {mode}"));
    }
    if let Some(status) =
        raw_text_field_deep(&value, &["contract_hook_status"]).filter(|status| !status.is_empty())
    {
        lines.push(format!("interaction contract hook status: {status}"));
    }
    if let Some(restart_present) = raw_contract_hook_bool(&value, "restart_present") {
        lines.push(format!(
            "interaction restart hook present: {restart_present}"
        ));
    }
    if let Some(restart_reachable) =
        raw_bool_field_deep(&value, "restart_hook_reachable_after_start")
    {
        lines.push(format!(
            "interaction restart hook reachable after start: {restart_reachable}"
        ));
    }
    let inputs = raw_string_array_field_deep(&value, "input_dispatches");
    if !inputs.is_empty() {
        lines.push(format!(
            "interaction redispatched inputs: {}",
            inputs.join(", ")
        ));
    }
    let state_dimensions = raw_string_array_field_deep(&value, "state_dimensions_changed");
    if !state_dimensions.is_empty() {
        lines.push(format!(
            "interaction state dimensions changed: {}",
            state_dimensions.join(", ")
        ));
    }
    let info = raw_string_array_field_deep(&value, "informational_failure_kinds");
    if !info.is_empty() {
        lines.push(format!(
            "interaction informational findings: {}",
            info.join(", ")
        ));
    }
    lines.extend(
        interaction_candidate_prompt_lines(&value)
            .into_iter()
            .map(|line| format!("interaction candidate table: {line}")),
    );
    lines
}

fn release_recovery_missing_capabilities(
    runtime_acceptance: Option<&crate::minimal_loop::evidence::RuntimeAcceptanceReport>,
) -> Vec<String> {
    runtime_acceptance
        .map(|report| report.missing_capabilities.clone())
        .unwrap_or_default()
}

fn release_recovery_repair_targets(
    release_gate: &ReleaseGateSummary,
    runtime_acceptance: Option<&crate::minimal_loop::evidence::RuntimeAcceptanceReport>,
) -> Vec<String> {
    let mut targets = Vec::new();
    let browser_status = release_gate.browser_readiness_status.to_ascii_lowercase();
    let interaction_status = release_gate
        .interaction_evidence_status
        .to_ascii_lowercase();
    let interaction_probe_unavailable = release_gate
        .reasons
        .iter()
        .any(|reason| reason.contains("interaction_unverified:probe_unavailable"));
    let restart_terminal_unreached = release_gate
        .reasons
        .iter()
        .any(|reason| reason.contains("interaction_unverified:terminal_state_not_reached"));
    let interaction_probe_infrastructure =
        release_gate_has_interaction_probe_infrastructure_failure(release_gate);
    let build_verifier_compile_errors = release_gate
        .browser_readiness_status
        .contains("build_verifier_failed")
        && !compile_errors_from_release_evidence_path(
            &release_gate.browser_readiness_evidence_path,
        )
        .is_empty();
    if build_verifier_compile_errors {
        targets.push("fix_compile_error".to_string());
        targets.push("implementation".to_string());
    }
    if browser_status.contains("tailwind_dev_pipeline_failure")
        || browser_status.contains("css")
        || browser_status.contains("http_500")
    {
        targets.push("framework_config".to_string());
    }
    if browser_status.starts_with("unavailable:")
        || browser_status.contains("evidence_missing")
        || (!interaction_probe_unavailable
            && (interaction_status.starts_with("unavailable:")
                || interaction_status.contains("evidence_missing")))
    {
        targets.push("required_evidence_missing".to_string());
    }
    if browser_status.starts_with("failed:") && !build_verifier_compile_errors {
        targets.push("test_or_evidence".to_string());
    }
    if interaction_status.starts_with("failed:") && !interaction_probe_infrastructure {
        targets.extend(interaction_repair_targets_for_reason(&interaction_status));
    }
    if restart_terminal_unreached {
        targets.push("restart_reachability_or_accept_partial".to_string());
    }
    if let Some(report) = runtime_acceptance {
        targets.extend(
            report
                .missing_evidence
                .iter()
                .filter(|evidence| behavior_depth_evidence_key(evidence))
                .map(|evidence| format!("implementation:{evidence}")),
        );
        targets.extend(
            report
                .obligation_repair_targets
                .iter()
                .map(|target| format!("{}:{}", target.obligation, target.target_path)),
        );
    }
    if targets.is_empty() {
        targets.push("release_acceptance".to_string());
    }
    dedup_strings(targets)
}

fn compile_errors_from_release_evidence_path(path: &str) -> Vec<CompileError> {
    if path.trim().is_empty() {
        return Vec::new();
    }
    let evidence_path = Path::new(path);
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return build_verifier::FullCommandOutput::read_from_path(evidence_path)
            .map(|output| build_verifier::parse_compile_errors(&output))
            .unwrap_or_default();
    };
    let mut errors = Vec::new();
    for output in release_evidence_compile_output_path_fields(&value, evidence_path) {
        for error in build_verifier::parse_compile_errors(&output) {
            if !errors.contains(&error) {
                errors.push(error);
            }
        }
    }
    errors
}

fn release_evidence_compile_output_path_fields(
    value: &Value,
    evidence_path: &Path,
) -> Vec<build_verifier::FullCommandOutput> {
    let mut out: Vec<build_verifier::FullCommandOutput> = Vec::new();
    let base_dir = evidence_path.parent().unwrap_or_else(|| Path::new("."));
    for scope in raw_value_scopes(value) {
        for key in [
            "build_output_path",
            "full_output_path",
            "output_path",
            "stdout_path",
            "stderr_path",
        ] {
            if let Some(raw_path) = scope.get(key).and_then(Value::as_str) {
                let path = Path::new(raw_path);
                let path = if path.is_absolute() {
                    path.to_path_buf()
                } else {
                    base_dir.join(path)
                };
                if let Ok(output) = build_verifier::FullCommandOutput::read_from_path(path)
                    && !output.as_str().trim().is_empty()
                    && !out
                        .iter()
                        .any(|existing| existing.as_str() == output.as_str())
                {
                    out.push(output);
                }
            }
        }
    }
    out
}

fn release_recovery_verify_commands(
    profile: &str,
    release_gate: &ReleaseGateSummary,
) -> Vec<String> {
    let mut commands = resolve_profile_runtime(profile).release_recovery_verify_commands(
        &release_gate.reasons,
        release_gate_has_interaction_probe_infrastructure_failure(release_gate),
    );
    if release_gate.status == "partial" {
        commands.push("do not claim release_ready until release gate evidence passes".to_string());
    }
    dedup_strings(commands)
}

fn interaction_repair_targets_for_reason(reason: &str) -> Vec<String> {
    let lower = reason.to_ascii_lowercase();
    if lower.contains("input_state_change_missing_after_start")
        || lower.contains("input_state_change_not_evaluated_after_start")
        || lower.contains("interaction_state_change_missing")
        || lower.contains("canvas_blank")
        || lower.contains("text_input_state_change_missing")
    {
        vec!["input_state_render_wiring".to_string()]
    } else if lower.contains("token_echo_after_reload_only") || lower.contains("token_echo_missing")
    {
        vec!["live_preview_render_wiring".to_string()]
    } else if lower.contains("text_entry_missing") {
        vec!["text_input_wiring".to_string()]
    } else if lower.contains("persistence_after_reload_reset") {
        vec!["persistence_state_wiring".to_string()]
    } else if lower.contains("start_transition_missing")
        || lower.contains("primary_start_transition_missing")
    {
        vec!["start_control_wiring".to_string()]
    } else {
        vec!["capability_implementation".to_string()]
    }
}

fn behavior_depth_evidence_key(evidence: &str) -> bool {
    matches!(
        evidence,
        "challenge_or_adversary_evidence"
            | "failure_or_collision_evidence"
            | "score_or_progression_evidence"
            | "restart_or_recoverable_state_evidence"
            | "persistence_evidence"
            | "live_preview_evidence"
    )
}

#[allow(clippy::too_many_arguments)]
fn emit_ultra_phase_event(
    config: &Config,
    event: &str,
    plan: &UltraPlan,
    phase: &UltraPhase,
    index: usize,
    stage: &str,
    ok: Option<bool>,
    reason: Option<&str>,
    step_count: Option<usize>,
) {
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": event,
            "phase_id": phase.id,
            "phase_index": index + 1,
            "total_phases": plan.phases.len(),
            "final_phase": index + 1 == plan.phases.len(),
            "stage": stage,
            "ok": ok,
            "reason": reason.map(eval_events::body_snippet).unwrap_or_default(),
            "step_count": step_count,
        }),
    );
}

fn emit_phase_verification_event(
    config: &Config,
    plan: &UltraPlan,
    phase: &UltraPhase,
    index: usize,
    mode: PhaseVerificationMode,
    ok: bool,
    reason: Option<&str>,
) {
    let mode = match mode {
        PhaseVerificationMode::IntermediateInvariant => "intermediate_invariant",
        PhaseVerificationMode::FinalAcceptance => "final_acceptance",
    };
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "phase_verification_result",
            "phase_id": phase.id,
            "phase_index": index + 1,
            "total_phases": plan.phases.len(),
            "phase_verification_mode": mode,
            "ok": ok,
            "reason": reason.map(eval_events::body_snippet).unwrap_or_default(),
        }),
    );
}

struct UltraPhaseRecoveryRequest<'a> {
    failure_kind: &'a str,
    reason: &'a str,
    missing_paths: &'a [String],
    missing_signals: &'a [String],
    repair_targets: &'a [String],
    verify_commands: &'a [String],
}

fn save_ultra_phase_recovery_handoff(
    config: &Config,
    plan: &UltraPlan,
    phase: &UltraPhase,
    request: UltraPhaseRecoveryRequest<'_>,
) -> Option<eval_events::StopReasonParts> {
    save_ultra_phase_recovery_handoff_with_evidence(config, plan, phase, request, &[])
}

fn save_ultra_phase_recovery_handoff_with_evidence(
    config: &Config,
    plan: &UltraPlan,
    phase: &UltraPhase,
    request: UltraPhaseRecoveryRequest<'_>,
    failure_evidence: &[String],
) -> Option<eval_events::StopReasonParts> {
    let handoff = RecoveryHandoff {
        profile: plan.profile.clone(),
        original_goal: plan.goal.clone(),
        failed_phase: Some(phase.id.clone()),
        failed_step: None,
        failure_kind: request.failure_kind.to_string(),
        failure_evidence: if failure_evidence.is_empty() {
            vec![request.reason.to_string()]
        } else {
            failure_evidence.to_vec()
        },
        missing_paths: request.missing_paths.to_vec(),
        missing_capabilities: request.missing_signals.to_vec(),
        verify_commands: request.verify_commands.to_vec(),
        changed_paths: Vec::new(),
        repair_targets: request.repair_targets.to_vec(),
    };
    let failure_kind = request.failure_kind;
    let reason = request.reason;
    let scope = format!("phase-{}", recovery_scope_token(&phase.id));
    let path = match save_ultra_recovery_prompt(&config.workspace_root, &scope, &handoff) {
        Ok(path) => path,
        Err(err) => {
            eval_events::emit(
                config.eval_events_path.as_deref(),
                json!({
                    "event": "recovery_prompt_save_failed",
                    "recovery_handoff_kind": failure_kind,
                    "phase_id": phase.id,
                    "reason": eval_events::body_snippet(&err.to_string()),
                }),
            );
            return Some(eval_events::StopReasonParts::free_text(format!(
                "recovery prompt save failed: {err}"
            )));
        }
    };
    let recovery_plan = match save_recovery_ultra_plan(&config.workspace_root, &scope, &handoff) {
        Ok(path) => Some(path),
        Err(err) => {
            let prompt_path = handoff_path(&path);
            eval_events::emit(
                config.eval_events_path.as_deref(),
                json!({
                    "event": "recovery_ultra_plan_save_failed",
                    "recovery_handoff_kind": failure_kind,
                    "phase_id": phase.id,
                    "recovery_prompt_path": prompt_path,
                    "reason": eval_events::body_snippet(&err.to_string()),
                    "recovery_yaml_missing": true,
                }),
            );
            None
        }
    };
    let validation = validate_recovery_artifacts(&path, recovery_plan.as_deref());
    let raw_prompt_command = suggested_ultra_recovery_command(&path, &plan.profile);
    let prompt_command = if validation.prompt_command_available() {
        raw_prompt_command
    } else {
        String::new()
    };
    let recovery_plan_command = recovery_plan
        .as_ref()
        .filter(|_| validation.yaml_command_available())
        .map(|path| suggested_recovery_ultra_plan_command(path));
    let prompt_path = handoff_path(&path);
    let recovery_plan_path = optional_handoff_path(recovery_plan.as_ref());
    let (completed_phases, pending_phases) = ultra_phase_status(plan, phase);
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "recovery_prompt_saved",
            "recovery_handoff_kind": failure_kind,
            "phase_id": phase.id,
            "recovery_prompt_path": &prompt_path,
            "recovery_ultra_plan_path": &recovery_plan_path,
            "recovery_yaml_missing": recovery_plan.is_none(),
            "recovery_prompt_exists": validation.prompt_exists,
            "recovery_prompt_parse_ok": validation.prompt_parse_ok,
            "recovery_prompt_parse_error": validation.prompt_parse_error.as_deref().unwrap_or_default(),
            "recovery_yaml_exists": validation.yaml_exists,
            "recovery_yaml_parse_ok": validation.yaml_parse_ok,
            "recovery_yaml_parse_error": validation.yaml_parse_error.as_deref().unwrap_or_default(),
            "recovery_command_targets_valid": validation.command_targets_valid(),
            "suggested_recovery_command": prompt_command.clone(),
            "suggested_recovery_yaml_command": recovery_plan_command.clone().unwrap_or_default(),
            "recovery_profile": plan.profile,
            "local_repair_exhausted": true,
            "status": "incomplete",
        }),
    );
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "ultra_partial_artifact_summary",
            "status": "incomplete",
            "completed_phase_ids": completed_phases.clone(),
            "failed_phase_id": phase.id,
            "pending_phase_ids": pending_phases.clone(),
            "failure_kind": failure_kind,
            "recovery_prompt_path": &prompt_path,
            "recovery_ultra_plan_path": &recovery_plan_path,
            "recovery_yaml_missing": recovery_plan.is_none(),
            "recovery_prompt_exists": validation.prompt_exists,
            "recovery_prompt_parse_ok": validation.prompt_parse_ok,
            "recovery_yaml_exists": validation.yaml_exists,
            "recovery_yaml_parse_ok": validation.yaml_parse_ok,
            "recovery_command_targets_valid": validation.command_targets_valid(),
            "suggested_recovery_command": prompt_command.clone(),
            "suggested_recovery_yaml_command": recovery_plan_command.clone().unwrap_or_default(),
        }),
    );
    let recovery_yaml_summary = recovery_plan
        .as_ref()
        .map(|path| {
            let display = handoff_path(path);
            if validation.yaml_parse_ok {
                format!("Recovery UltraPlan YAML saved: {display}")
            } else {
                format!(
                    "Recovery UltraPlan YAML invalid: {} ({})",
                    display,
                    validation
                        .yaml_parse_error
                        .as_deref()
                        .unwrap_or("recovery_yaml_invalid")
                )
            }
        })
        .unwrap_or_else(|| {
            "Recovery UltraPlan YAML missing: failed to save valid recovery plan".to_string()
        });
    let prompt_command_summary = if validation.prompt_command_available() {
        format!("Suggested command: {prompt_command}")
    } else {
        format!(
            "Suggested command: unavailable because recovery prompt validation failed ({})",
            validation
                .prompt_parse_error
                .as_deref()
                .unwrap_or("recovery_prompt_invalid")
        )
    };
    let recovery_yaml_command_summary = recovery_plan_command
        .as_ref()
        .map(|command| format!("Suggested YAML command: {command}"))
        .unwrap_or_else(|| {
            "Suggested YAML command: unavailable because recovery YAML is missing".to_string()
        });
    let artifact_check_summary = recovery_artifact_check_summary(&validation);
    eval_events::write_run_summary(
        config.eval_events_path.as_deref(),
        &render_ultra_partial_run_summary(UltraPartialRunSummary {
            completed_phases: &completed_phases,
            failed_phase: &phase.id,
            pending_phases: &pending_phases,
            failure_kind,
            reason,
            recovery_prompt_path: &prompt_path,
            recovery_yaml_summary: &recovery_yaml_summary,
            prompt_command_summary: &prompt_command_summary,
            recovery_yaml_command_summary: &recovery_yaml_command_summary,
            recovery_artifact_check: &artifact_check_summary,
            browser_evidence_missing_note: browser_evidence_missing_before_final_acceptance_note(
                config,
            ),
        }),
    );
    let prompt_message = if validation.prompt_command_available() {
        format!("suggested command: {prompt_command}")
    } else {
        "suggested command unavailable because recovery prompt validation failed".to_string()
    };
    let recovery_yaml_command_message = recovery_plan_command
        .as_ref()
        .map(|command| format!("suggested YAML command: {command}"))
        .unwrap_or_else(|| {
            "suggested YAML command unavailable because recovery YAML is missing".to_string()
        });
    Some(eval_events::StopReasonParts {
        free_text: format!("incomplete; {artifact_check_summary}"),
        paths: vec![
            format!("repair prompt saved: {prompt_path}"),
            recovery_yaml_summary,
        ],
        commands: vec![prompt_message, recovery_yaml_command_message],
    })
}

fn render_failure_stop_reason(
    free_text: impl Into<String>,
    handoff: Option<eval_events::StopReasonParts>,
) -> String {
    let free_text = free_text.into();
    let mut parts = handoff.unwrap_or_default();
    parts.free_text = if parts.free_text.trim().is_empty() {
        free_text
    } else {
        format!("{free_text}; {}", parts.free_text)
    };
    eval_events::render_stop_reason(&parts)
}

#[allow(clippy::too_many_arguments)]
fn save_release_recovery_handoff(
    config: &Config,
    profile: &str,
    original_goal: &str,
    scope: &str,
    acceptance_layer: &str,
    failure_kind: &str,
    failure_evidence: Vec<String>,
    missing_paths: Vec<String>,
    missing_capabilities: Vec<String>,
    repair_targets: Vec<String>,
    verify_commands: Vec<String>,
) -> Option<ReleaseRecoveryHandoffSummary> {
    let handoff = RecoveryHandoff {
        profile: profile.to_string(),
        original_goal: original_goal.to_string(),
        failed_phase: Some(acceptance_layer.to_string()),
        failed_step: None,
        failure_kind: failure_kind.to_string(),
        failure_evidence,
        missing_paths,
        missing_capabilities,
        verify_commands,
        changed_paths: Vec::new(),
        repair_targets,
    };
    let path = match save_ultra_recovery_prompt(&config.workspace_root, scope, &handoff) {
        Ok(path) => path,
        Err(err) => {
            eval_events::emit(
                config.eval_events_path.as_deref(),
                json!({
                    "event": "recovery_prompt_save_failed",
                    "recovery_handoff_kind": failure_kind,
                    "acceptance_layer": acceptance_layer,
                    "reason": eval_events::body_snippet(&err.to_string()),
                    "status": "incomplete",
                }),
            );
            return None;
        }
    };
    let recovery_plan = match save_recovery_ultra_plan(&config.workspace_root, scope, &handoff) {
        Ok(path) => Some(path),
        Err(err) => {
            let prompt_path = handoff_path(&path);
            eval_events::emit(
                config.eval_events_path.as_deref(),
                json!({
                    "event": "recovery_ultra_plan_save_failed",
                    "recovery_handoff_kind": failure_kind,
                    "acceptance_layer": acceptance_layer,
                    "recovery_prompt_path": prompt_path,
                    "reason": eval_events::body_snippet(&err.to_string()),
                    "recovery_yaml_missing": true,
                    "status": "incomplete",
                }),
            );
            None
        }
    };
    let validation = validate_recovery_artifacts(&path, recovery_plan.as_deref());
    let raw_prompt_command = suggested_ultra_recovery_command(&path, profile);
    let prompt_command = if validation.prompt_command_available() {
        raw_prompt_command
    } else {
        String::new()
    };
    let recovery_plan_command = recovery_plan
        .as_ref()
        .filter(|_| validation.yaml_command_available())
        .map(|path| suggested_recovery_ultra_plan_command(path));
    let prompt_path = handoff_path(&path);
    let recovery_plan_path = optional_handoff_path(recovery_plan.as_ref());
    let summary = ReleaseRecoveryHandoffSummary {
        recovery_handoff_kind: failure_kind.to_string(),
        acceptance_layer: acceptance_layer.to_string(),
        recovery_prompt_path: prompt_path,
        recovery_ultra_plan_path: recovery_plan_path,
        suggested_recovery_command: prompt_command.clone(),
        suggested_recovery_yaml_command: recovery_plan_command.clone().unwrap_or_default(),
    };
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "recovery_prompt_saved",
            "recovery_handoff_kind": failure_kind,
            "acceptance_layer": acceptance_layer,
            "recovery_scope": scope,
            "recovery_prompt_path": &summary.recovery_prompt_path,
            "recovery_ultra_plan_path": &summary.recovery_ultra_plan_path,
            "recovery_yaml_missing": recovery_plan.is_none(),
            "recovery_prompt_exists": validation.prompt_exists,
            "recovery_prompt_parse_ok": validation.prompt_parse_ok,
            "recovery_prompt_parse_error": validation.prompt_parse_error.as_deref().unwrap_or_default(),
            "recovery_yaml_exists": validation.yaml_exists,
            "recovery_yaml_parse_ok": validation.yaml_parse_ok,
            "recovery_yaml_parse_error": validation.yaml_parse_error.as_deref().unwrap_or_default(),
            "recovery_command_targets_valid": validation.command_targets_valid(),
            "suggested_recovery_command": &summary.suggested_recovery_command,
            "suggested_recovery_yaml_command": &summary.suggested_recovery_yaml_command,
            "recovery_profile": profile,
            "release_acceptance_handoff": true,
            "handoff_saved_not_success": true,
            "status": "incomplete",
        }),
    );
    eval_events::append_run_summary(
        config.eval_events_path.as_deref(),
        &render_release_recovery_handoff_summary(&summary, &validation),
    );
    Some(summary)
}

fn render_release_recovery_handoff_summary(
    summary: &ReleaseRecoveryHandoffSummary,
    validation: &RecoveryArtifactValidation,
) -> String {
    format!(
        "Recovery next action:\n\
- Status: incomplete_release_acceptance\n\
- Failed acceptance layer: {}\n\
- Recovery handoff kind: {}\n\
- Recovery prompt saved: {}\n\
- Recovery UltraPlan YAML saved: {}\n\
- Suggested command: {}\n\
- Suggested YAML command: {}\n\
- Recovery artifact check: {}",
        summary.acceptance_layer,
        summary.recovery_handoff_kind,
        missing_if_empty(&summary.recovery_prompt_path),
        missing_if_empty(&summary.recovery_ultra_plan_path),
        missing_if_empty(&summary.suggested_recovery_command),
        missing_if_empty(&summary.suggested_recovery_yaml_command),
        recovery_artifact_check_summary(validation),
    )
}

fn missing_if_empty(value: &str) -> &str {
    if value.is_empty() { "missing" } else { value }
}

struct UltraPartialRunSummary<'a> {
    completed_phases: &'a [String],
    failed_phase: &'a str,
    pending_phases: &'a [String],
    failure_kind: &'a str,
    reason: &'a str,
    recovery_prompt_path: &'a str,
    recovery_yaml_summary: &'a str,
    prompt_command_summary: &'a str,
    recovery_yaml_command_summary: &'a str,
    recovery_artifact_check: &'a str,
    browser_evidence_missing_note: Option<&'a str>,
}

const BROWSER_EVIDENCE_MISSING_BEFORE_FINAL_ACCEPTANCE: &str = "Browser evidence missing: run failed before final acceptance (interaction probe installed but not exercised).";

fn render_ultra_partial_run_summary(summary: UltraPartialRunSummary<'_>) -> String {
    let browser_evidence_missing = summary
        .browser_evidence_missing_note
        .map(|note| format!("\n\n{note}"))
        .unwrap_or_default();
    format!(
        "Status: incomplete\n\n\
Completed phases:\n{}\n\n\
Failed phase:\n- {} ({})\n\n\
Pending phases:\n{}\n\n\
Recovery next action:\n- {}\n- Recovery prompt saved: {}\n- {}\n- {}\n- {}\n\n\
Failure:\n{}{}",
        render_summary_bullets(summary.completed_phases),
        summary.failed_phase,
        summary.failure_kind,
        render_summary_bullets(summary.pending_phases),
        summary.recovery_yaml_summary,
        summary.recovery_prompt_path,
        summary.prompt_command_summary,
        summary.recovery_yaml_command_summary,
        summary.recovery_artifact_check,
        eval_events::render_stop_reason_text(summary.reason),
        browser_evidence_missing,
    )
}

fn browser_evidence_missing_before_final_acceptance_note(config: &Config) -> Option<&'static str> {
    if interaction_probe_performed_for_run(config) {
        return None;
    }
    match interaction_probe::playwright_availability(&config.workspace_root) {
        interaction_probe::ProbeAvailability::Available(_) => {
            Some(BROWSER_EVIDENCE_MISSING_BEFORE_FINAL_ACCEPTANCE)
        }
        interaction_probe::ProbeAvailability::Unavailable(_) => None,
    }
}

fn render_summary_bullets(items: &[String]) -> String {
    if items.is_empty() {
        "- none".to_string()
    } else {
        items
            .iter()
            .map(|item| format!("- {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn ultra_phase_status(plan: &UltraPlan, failed_phase: &UltraPhase) -> (Vec<String>, Vec<String>) {
    let failed_index = plan
        .phases
        .iter()
        .position(|phase| phase.id == failed_phase.id)
        .unwrap_or(0);
    let completed = plan
        .phases
        .iter()
        .take(failed_index)
        .map(|phase| phase.id.clone())
        .collect();
    let pending = plan
        .phases
        .iter()
        .skip(failed_index + 1)
        .map(|phase| phase.id.clone())
        .collect();
    (completed, pending)
}

fn recovery_scope_token(value: &str) -> String {
    let token = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    if token.trim_matches('-').is_empty() {
        "phase".to_string()
    } else {
        token
    }
}

fn emit_ultra_context_initialized(
    config: &Config,
    plan: &UltraPlan,
    context: &UltraRunContext,
    session_message_count: usize,
) {
    let phase_signal_text = ultra_plan_phase_signal_text(plan);
    let requested_port = effective_requested_port(
        resolve_profile_runtime(&plan.profile),
        &plan.goal,
        Some(&phase_signal_text),
    );
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "ultra_context_initialized",
            "total_phases": plan.phases.len(),
            "profile": plan.profile.clone(),
            "requested_port": requested_port.as_ref().map(|requested| requested.telemetry.clone()),
            "requested_port_value": requested_port.as_ref().map(|requested| requested.port),
            "shared_execution_session": true,
            "session_message_count": session_message_count,
            "pending_final_artifacts_count": context.pending_final_artifacts.len(),
            "pending_capability_evidence": context.pending_capability_evidence.clone(),
            "pending_capability_evidence_count": context.pending_capability_evidence.len(),
            "context_truncated": context.truncated,
        }),
    );
}

fn emit_ultra_phase_context_attached(
    config: &Config,
    plan: &UltraPlan,
    phase: &UltraPhase,
    index: usize,
    context: &UltraRunContext,
    session_message_count: usize,
) {
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "ultra_phase_context_attached",
            "phase_id": phase.id,
            "phase_index": index + 1,
            "total_phases": plan.phases.len(),
            "shared_execution_session": true,
            "session_message_count": session_message_count,
            "completed_phase_count": context.completed_phases.len(),
            "changed_path_count": context.created_or_changed_paths.len(),
            "pending_final_artifacts_count": context.pending_final_artifacts.len(),
            "pending_capability_evidence": context.pending_capability_evidence.clone(),
            "pending_capability_evidence_count": context.pending_capability_evidence.len(),
            "unresolved_repair_target_count": context.unresolved_repair_targets.len(),
            "has_previous_context": index > 0
                && (!context.completed_phases.is_empty()
                    || !context.created_or_changed_paths.is_empty()
                    || context.last_failed_phase.is_some()),
            "context_truncated": context.truncated,
        }),
    );
}

fn emit_ultra_phase_context_updated(
    config: &Config,
    plan: &UltraPlan,
    phase: &UltraPhase,
    index: usize,
    context: &UltraRunContext,
    session_message_count: usize,
    partial_outcome_recorded: bool,
) {
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "ultra_phase_context_updated",
            "phase_id": phase.id,
            "phase_index": index + 1,
            "total_phases": plan.phases.len(),
            "shared_execution_session": true,
            "session_message_count": session_message_count,
            "completed_phase_count": context.completed_phases.len(),
            "changed_path_count": context.created_or_changed_paths.len(),
            "pending_final_artifacts_count": context.pending_final_artifacts.len(),
            "pending_capability_evidence": context.pending_capability_evidence.clone(),
            "pending_capability_evidence_count": context.pending_capability_evidence.len(),
            "recent_verify_failure_count": context.last_verify_failures.len(),
            "recent_repair_changed_path_count": context.last_repair_changed_paths.len(),
            "unresolved_repair_target_count": context.unresolved_repair_targets.len(),
            "partial_outcome_recorded": partial_outcome_recorded,
            "context_truncated": context.truncated,
        }),
    );
}

fn emit_planner_error_for_lint(
    config: &Config,
    provider: &str,
    model: &str,
    report: &PlanLintReport,
    attempt: usize,
) {
    let (stage, kind) = planner_stage_and_kind_for_lint(report);
    crate::planner::lint_rejection::emit_planner_error(
        config.eval_events_path.as_deref(),
        provider,
        model,
        stage,
        kind,
        report,
        attempt,
    );
}

fn emit_planner_error(
    config: &Config,
    provider: &str,
    model: &str,
    stage: &str,
    kind: &str,
    message: &str,
    attempt: usize,
) {
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "planner_error",
            "planner_stage": stage,
            "planner_error_kind": kind,
            "planner_error_message": eval_events::body_snippet(message),
            "planner_provider": provider,
            "planner_model": model,
            "repair_attempt": attempt,
        }),
    );
}

fn emit_planner_raw_output_shape(
    config: &Config,
    provider: &str,
    model: &str,
    attempt: usize,
    content: &str,
    session_mode: PlannerSessionMode,
) {
    let json_extract_status = match extract_json_object(content) {
        Ok(_) => "ok",
        Err(_) => "missing",
    };
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "planner_raw_output_shape",
            "planner_provider": provider,
            "planner_model": model,
            "attempt": attempt,
            "planner_session_mode": session_mode.as_str(),
            "content_len": content.chars().count(),
            "has_json_object": json_extract_status == "ok",
            "has_yaml_fence": content.contains("```yaml") || content.contains("```yml"),
            "contains_goal_key": content.contains("\"goal\"") || content.contains("goal:"),
            "contains_steps_key": content.contains("\"steps\"") || content.contains("steps:"),
            "json_extract_status": json_extract_status,
            "preview": eval_events::body_snippet(content),
        }),
    );
}

fn emit_planner_schema_field_defaults(
    config: &Config,
    provider: &str,
    model: &str,
    attempt: usize,
    field_defaults: &[GeneratedStepPlanFieldDefault],
) {
    if field_defaults.is_empty() {
        return;
    }
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "planner_plan_sanitized",
            "planner_stage": "sanitize",
            "planner_provider": provider,
            "planner_model": model,
            "repair_attempt": attempt,
            "actions": field_defaults.iter().map(|record| json!({
                "kind": "schema_field_defaulted",
                "field": &record.field,
                "step_index": record.step_index,
                "step_id": &record.step_id,
                "default_value": &record.default_value,
                "source_excerpt": eval_events::body_snippet(&record.source_excerpt),
            })).collect::<Vec<_>>(),
            "schema_field_defaults": field_defaults.iter().map(|record| json!({
                "field": &record.field,
                "step_index": record.step_index,
                "step_id": &record.step_id,
                "default_value": &record.default_value,
                "source_excerpt": eval_events::body_snippet(&record.source_excerpt),
            })).collect::<Vec<_>>(),
        }),
    );
}

fn collect_step_verify_commands(plan: &StepPlan) -> Vec<String> {
    plan.steps
        .iter()
        .flat_map(|step| step.verify.iter().cloned())
        .collect()
}

fn emit_planner_verify_command_normalized(
    config: &Config,
    provider: &str,
    model: &str,
    attempt: usize,
    before: &[String],
    after: &[String],
) {
    if before == after {
        return;
    }
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "planner_verify_command_normalized",
            "planner_stage": "verify_policy",
            "planner_provider": provider,
            "planner_model": model,
            "repair_attempt": attempt,
            "before_count": before.len(),
            "after_count": after.len(),
            "contains_safe_shell_split": before.iter().any(|command| command.contains("&&")),
            "normalization_source": "deterministic_verify_policy",
            "original_command_hash": stable_command_list_hash(before),
            "original_command_summary": command_list_summary(before),
            "normalized_command_hash": stable_command_list_hash(after),
            "normalized_commands": after.iter().map(|command| eval_events::body_snippet(command)).collect::<Vec<_>>(),
            "before_preview": eval_events::body_snippet(&before.join(" | ")),
            "after_preview": eval_events::body_snippet(&after.join(" | ")),
        }),
    );
}

fn emit_planner_plan_sanitized(
    config: &Config,
    provider: &str,
    model: &str,
    attempt: usize,
    report: &SanitizerReport,
) {
    if report.is_empty() {
        return;
    }
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "planner_plan_sanitized",
            "planner_stage": "sanitize",
            "planner_provider": provider,
            "planner_model": model,
            "repair_attempt": attempt,
            "actions": report.goal_truncations.iter().map(|record| json!({
                "kind": &record.kind,
                "original_len": record.original_len,
                "new_len": record.new_len,
            })).chain(report.dropped_expected_paths.iter().map(|record| json!({
                "kind": "side_effect_path_dropped",
                "step_id": &record.step_id,
                "path": &record.path,
                "tier": &record.tier,
                "side_effect_token": &record.token,
                "reason": eval_events::body_snippet(&record.reason),
            }))).chain(report.shell_control_splits.iter().map(|record| json!({
                "kind": &record.kind,
                "step_id": &record.step_id,
                "original_command": eval_events::body_snippet(&record.original_command),
                "fragments": record.fragments.iter().map(|fragment| eval_events::body_snippet(fragment)).collect::<Vec<_>>(),
                "dropped_fallback": record.dropped_fallback.as_deref().map(eval_events::body_snippet),
            }))).chain(report.setup_verify_relocations.iter().map(|record| json!({
                "kind": "setup_verify_relocated",
                "from_step_id": &record.from_step_id,
                "to_step_id": &record.to_step_id,
                "command": eval_events::body_snippet(&record.command),
            }))).chain(report.instruction_truncations.iter().map(|record| json!({
                "kind": &record.kind,
                "step_id": &record.step_id,
                "original_len": record.original_len,
                "new_len": record.new_len,
            }))).collect::<Vec<_>>(),
            "goal_truncations": report.goal_truncations.iter().map(|record| json!({
                "kind": &record.kind,
                "original_len": record.original_len,
                "new_len": record.new_len,
            })).collect::<Vec<_>>(),
            "normalized_commands": report.normalized_commands.iter().map(|record| json!({
                "kind": &record.kind,
                "step_id": &record.step_id,
                "original_command": eval_events::body_snippet(&record.original_command),
                "normalized_command": eval_events::body_snippet(&record.normalized_command),
                "reason": eval_events::body_snippet(&record.reason),
            })).collect::<Vec<_>>(),
            "shell_control_splits": report.shell_control_splits.iter().map(|record| json!({
                "kind": &record.kind,
                "step_id": &record.step_id,
                "original_command": eval_events::body_snippet(&record.original_command),
                "fragments": record.fragments.iter().map(|fragment| eval_events::body_snippet(fragment)).collect::<Vec<_>>(),
                "dropped_fallback": record.dropped_fallback.as_deref().map(eval_events::body_snippet),
            })).collect::<Vec<_>>(),
            "removed_commands": report.removed_commands.iter().map(|record| json!({
                "step_id": &record.step_id,
                "command": eval_events::body_snippet(&record.command),
                "reason": eval_events::body_snippet(&record.reason),
            })).collect::<Vec<_>>(),
            "substituted_commands": report.substituted_commands.iter().map(|record| json!({
                "step_id": &record.step_id,
                "removed_command": eval_events::body_snippet(&record.removed_command),
                "substituted_command": eval_events::body_snippet(&record.substituted_command),
            })).collect::<Vec<_>>(),
            "moved_commands": report.moved_commands.iter().map(|record| json!({
                "from_step_id": &record.from_step_id,
                "to_step_id": &record.to_step_id,
                "command": eval_events::body_snippet(&record.command),
                "reason": eval_events::body_snippet(&record.reason),
            })).collect::<Vec<_>>(),
            "setup_verify_relocations": report.setup_verify_relocations.iter().map(|record| json!({
                "from_step_id": &record.from_step_id,
                "to_step_id": &record.to_step_id,
                "command": eval_events::body_snippet(&record.command),
                "reason": eval_events::body_snippet(&record.reason),
            })).collect::<Vec<_>>(),
            "dropped_expected_paths": report.dropped_expected_paths.iter().map(|record| json!({
                "step_id": &record.step_id,
                "path": &record.path,
                "tier": &record.tier,
                "side_effect_token": &record.token,
                "reason": eval_events::body_snippet(&record.reason),
            })).collect::<Vec<_>>(),
            "dropped_commands": report.dropped_commands.iter().map(|record| json!({
                "step_id": &record.step_id,
                "command": eval_events::body_snippet(&record.command),
                "reason": eval_events::body_snippet(&record.reason),
            })).collect::<Vec<_>>(),
            "retyped_steps": report.retyped_steps.iter().map(|record| json!({
                "step_id": &record.step_id,
                "from_kind": &record.from_kind,
                "to_kind": &record.to_kind,
                "reason": eval_events::body_snippet(&record.reason),
            })).collect::<Vec<_>>(),
            "instruction_truncations": report.instruction_truncations.iter().map(|record| json!({
                "kind": &record.kind,
                "step_id": &record.step_id,
                "original_len": record.original_len,
                "new_len": record.new_len,
            })).collect::<Vec<_>>(),
            "instruction_notes": report.instruction_notes.iter().map(|record| json!({
                "step_id": &record.step_id,
                "note": eval_events::body_snippet(&record.note),
            })).collect::<Vec<_>>(),
        }),
    );
    for record in &report.dropped_expected_paths {
        eval_events::emit(
            config.eval_events_path.as_deref(),
            json!({
                "event": "side_effect_path_dropped",
                "planner_stage": "sanitize",
                "planner_provider": provider,
                "planner_model": model,
                "repair_attempt": attempt,
                "step_id": &record.step_id,
                "path": &record.path,
                "tier": &record.tier,
                "side_effect_token": &record.token,
                "reason": eval_events::body_snippet(&record.reason),
            }),
        );
    }
}

fn emit_planner_fallback_plan(
    config: &Config,
    provider: &str,
    model: &str,
    goal: &str,
    plan: &StepPlan,
    reason: &str,
) {
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "planner_fallback_plan",
            "planner_stage": "fallback",
            "planner_provider": provider,
            "planner_model": model,
            "planner_error_message": eval_events::body_snippet(reason),
            "profile": config.profile,
            "goal": eval_events::body_snippet(goal),
            "step_count": plan.steps.len(),
            "expected_paths": plan.steps.iter().flat_map(|step| step.expected_paths.iter()).cloned().collect::<Vec<_>>(),
            "verify": collect_step_verify_commands(plan),
        }),
    );
}

fn stable_command_list_hash(commands: &[String]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for command in commands {
        for byte in eval_events::body_snippet(command).bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn command_list_summary(commands: &[String]) -> String {
    eval_events::body_snippet(&commands.join(" | "))
}

fn fallback_step_plan_for_setup_phase(goal: &str, config: &Config) -> Option<StepPlan> {
    let plan = resolve_profile_runtime(&config.profile)
        .fallback_setup_plan(&config.workspace_root, goal)?;
    crate::planner::lint::lint_template_contract(&plan, Some(&config.workspace_root))
        .is_pass()
        .then_some(plan)
}

fn phase_id_and_task_text(goal: &str) -> Option<String> {
    let mut out = Vec::new();
    for line in goal.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("Phase id:") || trimmed.starts_with("Phase task:") {
            out.push(trimmed.to_string());
        }
    }
    (!out.is_empty()).then(|| out.join("\n"))
}

fn compact_single_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn emit_ultra_plan_raw_output_shape(
    config: &Config,
    provider: &str,
    model: &str,
    attempt: usize,
    content: &str,
) {
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "ultra_plan_raw_output_shape",
            "planner_provider": provider,
            "planner_model": model,
            "attempt": attempt,
            "content_len": content.chars().count(),
            "has_yaml_fence": content.contains("```yaml") || content.contains("```yml"),
            "contains_goal_key": content.contains("goal:"),
            "contains_profile_key": content.contains("profile:"),
            "contains_style_key": content.contains("style:"),
            "contains_intent_key": content.contains("intent:"),
            "contains_phases_key": content.contains("phases:"),
            "preview": eval_events::body_snippet(content),
        }),
    );
}

fn emit_ultra_plan_generation_attempt(
    config: &Config,
    provider: &str,
    model: &str,
    attempt: usize,
    profile: &str,
    style: &str,
    intent: &str,
) {
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "ultra_plan_generation_attempt",
            "planner_provider": provider,
            "planner_model": model,
            "repair_attempt": attempt,
            "profile": profile,
            "style": style,
            "intent": intent,
            "degraded": false,
        }),
    );
}

fn emit_ultra_plan_generation_retry(
    config: &Config,
    provider: &str,
    model: &str,
    attempt: usize,
    failure_kind: &str,
    message: &str,
) {
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "ultra_plan_generation_retry",
            "planner_provider": provider,
            "planner_model": model,
            "repair_attempt": attempt,
            "planner_error_kind": failure_kind,
            "planner_error_message": eval_events::body_snippet(message),
            "degraded": false,
        }),
    );
}

fn emit_ultra_plan_generation_failed(config: &Config, provider: &str, model: &str, message: &str) {
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "ultra_plan_generation_failed",
            "planner_provider": provider,
            "planner_model": model,
            "planner_error_kind": "planner_schema_error",
            "planner_error_message": eval_events::body_snippet(message),
            "degraded": false,
        }),
    );
}

fn emit_ultra_plan_generation_succeeded(
    config: &Config,
    provider: &str,
    model: &str,
    attempt: usize,
    phase_count: usize,
) {
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "ultra_plan_generation_succeeded",
            "planner_provider": provider,
            "planner_model": model,
            "repair_attempt": attempt,
            "phase_count": phase_count,
            "degraded": false,
        }),
    );
}

fn emit_ultra_plan_generation_tool_call_rejected(
    config: &Config,
    provider: &str,
    model: &str,
    attempt: usize,
    message: &str,
) {
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "ultra_plan_generation_tool_call_rejected",
            "planner_provider": provider,
            "planner_model": model,
            "repair_attempt": attempt,
            "planner_error_kind": "planner_schema_error",
            "planner_error_message": eval_events::body_snippet(message),
            "degraded": false,
        }),
    );
}

fn emit_ultra_plan_generation_metadata_normalized(
    config: &Config,
    provider: &str,
    model: &str,
    attempt: usize,
    fields: &[String],
) {
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "ultra_plan_generation_metadata_normalized",
            "planner_provider": provider,
            "planner_model": model,
            "repair_attempt": attempt,
            "fields": fields,
            "degraded": false,
        }),
    );
}

fn emit_planner_quality_warnings(
    config: &Config,
    provider: &str,
    model: &str,
    attempt: usize,
    plan: &StepPlan,
) {
    for message in step_plan_quality_warnings(plan) {
        eval_events::emit(
            config.eval_events_path.as_deref(),
            json!({
                "event": "planner_quality_warning",
                "planner_stage": "quality",
                "planner_error_kind": "planner_quality_warning",
                "planner_error_message": eval_events::body_snippet(&message),
                "planner_provider": provider,
                "planner_model": model,
                "repair_attempt": attempt,
            }),
        );
    }
}

fn emit_planner_quality_issues(
    config: &Config,
    provider: &str,
    model: &str,
    attempt: usize,
    report: &PlanQualityReport,
) {
    for issue in &report.issues {
        eval_events::emit(
            config.eval_events_path.as_deref(),
            json!({
                "event": "planner_quality_issue",
                "planner_stage": "quality",
                "planner_error_kind": "planner_quality_issue",
                "planner_quality_category": issue.category,
                "planner_quality_severity": issue.severity.as_str(),
                "planner_error_message": eval_events::body_snippet(&issue.message),
                "planner_quality_step_id": issue.step_id,
                "planner_quality_evidence": issue.evidence,
                "planner_provider": provider,
                "planner_model": model,
                "repair_attempt": attempt,
            }),
        );
    }
}

fn emit_planner_quality_retry(
    config: &Config,
    provider: &str,
    model: &str,
    attempt: usize,
    report: &PlanQualityReport,
) {
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "planner_quality_retry",
            "planner_stage": "quality",
            "planner_error_kind": "planner_quality_retry",
            "planner_error_message": eval_events::body_snippet(&report.primary_message()),
            "planner_provider": provider,
            "planner_model": model,
            "repair_attempt": attempt,
            "planner_quality_issue_count": report.issues.len(),
        }),
    );
}

fn emit_planner_quality_retry_degraded(
    config: &Config,
    provider: &str,
    model: &str,
    attempt: usize,
    stage: &str,
    message: &str,
) {
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "planner_quality_retry_degraded",
            "planner_stage": stage,
            "planner_error_kind": "planner_quality_retry_degraded",
            "planner_error_message": eval_events::body_snippet(message),
            "planner_provider": provider,
            "planner_model": model,
            "repair_attempt": attempt,
        }),
    );
}

fn emit_planner_quality_retry_exhausted(
    config: &Config,
    provider: &str,
    model: &str,
    attempt: usize,
    report: &PlanQualityReport,
) {
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "planner_quality_retry_exhausted",
            "planner_stage": "quality",
            "planner_error_kind": "planner_quality_retry_exhausted",
            "planner_error_message": eval_events::body_snippet(&report.primary_message()),
            "planner_provider": provider,
            "planner_model": model,
            "repair_attempt": attempt,
            "planner_quality_issue_count": report.issues.len(),
        }),
    );
}

fn planner_stage_and_kind_for_lint(report: &PlanLintReport) -> (&'static str, &'static str) {
    if report.has_category("verify_policy") {
        ("verify_policy", "verify_command_policy_error")
    } else if report.has_category("dependency_order") {
        ("dependency_order", "verify_dependency_order_error")
    } else if report.has_category("scaffold") {
        ("scaffold", "phase_scaffold_error")
    } else {
        ("lint", "planner_lint_error")
    }
}

fn build_schema_retry_prompt(goal: &str, error: &str, attempt: usize) -> String {
    let issue_hints = schema_retry_issue_hints(error)
        .into_iter()
        .map(|hint| format!("- {hint}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Your previous StepPlan output failed schema validation on attempt {attempt}/3: {error}.\n\
Return only one JSON object and no markdown fences.\n\
Detected schema issues:\n{issue_hints}\n\n\
Required JSON shape:\n\
{{\n  \"goal\": \"{goal}\",\n  \"steps\": [\n    {{\n      \"id\": \"kebab-id\",\n      \"kind\": \"implement\",\n      \"expected_result\": \"pass\",\n      \"instruction\": \"Create the required files for the goal.\",\n      \"expected_paths\": [\"relative/path\"],\n      \"verify\": [\"command\"]\n    }}\n  ]\n}}\n\n\
Rules:\n- Include top-level goal and non-empty steps.\n- Step id must be a quoted string, not a number.\n- expected_result must be exactly \"pass\" or \"fail\", not prose.\n- Keep expected_paths workspace-relative.\n- Use deterministic verify commands only.\n\nGoal: {goal}"
    )
}

fn schema_retry_issue_hints(error: &str) -> Vec<&'static str> {
    let lower = error.to_ascii_lowercase();
    let mut hints = Vec::new();
    if lower.contains("missing goal") || lower.contains("top-level goal") {
        hints.push("Add a top-level goal field equal to the original goal.");
    }
    if lower.contains("missing steps")
        || lower.contains("non-empty steps")
        || lower.contains("at least one step")
    {
        hints.push("Add a non-empty steps array.");
    }
    if lower.contains("id") && (lower.contains("number") || lower.contains("string")) {
        hints.push("Use quoted string step ids such as \"setup-project\".");
    }
    if lower.contains("expected_result") || lower.contains("expected result") {
        hints.push("Set expected_result to exactly \"pass\" or \"fail\".");
    }
    if lower.contains("expected_paths") || lower.contains("workspace-relative") {
        hints.push("Use workspace-relative expected_paths and avoid absolute paths.");
    }
    if hints.is_empty() {
        hints.push("Return the canonical StepPlan JSON object with goal and steps.");
    }
    hints
}

fn build_empty_step_plan_compact_prompt(goal: &str, attempt: usize) -> String {
    let minimal_context = phase_id_and_task_text(goal).unwrap_or_else(|| {
        compact_single_line(goal)
            .chars()
            .take(600)
            .collect::<String>()
    });
    format!(
        "Compact StepPlan recovery after empty planner output on attempt {attempt}/3.\n\
Do not use prior chat history. Return only one JSON object and no markdown fences.\n\n\
Required JSON shape:\n\
{{\n  \"goal\": \"short phase goal\",\n  \"steps\": [\n    {{\n      \"id\": \"kebab-id\",\n      \"kind\": \"implement\",\n      \"expected_result\": \"pass\",\n      \"instruction\": \"Create the required files for the phase.\",\n      \"expected_paths\": [\"relative/path\"],\n      \"verify\": [\"command\"]\n    }}\n  ]\n}}\n\n\
Rules:\n\
- Include top-level goal and non-empty steps.\n\
- Step kind is required and must be inspect, setup, implement, verify, or report.\n\
- expected_paths and verify are semantic contracts; include arrays, even if empty.\n\
- expected_result is descriptive and may be pass or fail.\n\
- Keep paths workspace-relative and verify commands deterministic.\n\n\
Minimal phase context:\n{minimal_context}"
    )
}

fn build_lint_retry_prompt(
    goal: &str,
    report: &PlanLintReport,
    attempt: usize,
    categories_seen: &BTreeSet<String>,
) -> String {
    if is_only_goal_length_lint(report) {
        return "shorten goal to one sentence; keep steps unchanged".to_string();
    }
    let guidance = lint_retry_hard_constraints(report, categories_seen).join("\n");
    let errors = report
        .errors
        .iter()
        .map(crate::planner::lint_rejection::retry_error_line)
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Your previous StepPlan failed deterministic lint on attempt {attempt}/3.\n\
Fix these issues without weakening safety rules:\n{errors}\n\n\
{guidance}\n\n\
Return only one JSON object with top-level goal and steps. Do not use markdown fences.\n\
Step id must be a quoted string. expected_result must be exactly \"pass\" or \"fail\".\n\
Goal: {goal}"
    )
}

fn is_only_goal_length_lint(report: &PlanLintReport) -> bool {
    report.errors.len() == 1
        && report.errors[0].category == "contract"
        && report.errors[0].message == "StepPlan goal is too long"
}

fn build_quality_retry_prompt(goal: &str, report: &PlanQualityReport, attempt: usize) -> String {
    let issues = report
        .issues
        .iter()
        .filter(|issue| issue.severity.as_str() == "retryable_quality")
        .map(|issue| {
            let step = issue
                .step_id
                .as_ref()
                .map(|value| format!(" step={value}"))
                .unwrap_or_default();
            let evidence = issue
                .evidence
                .as_ref()
                .map(|value| format!(" evidence={value}"))
                .unwrap_or_default();
            format!(
                "- [{}{}] {}{}",
                issue.category, step, issue.message, evidence
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Your previous StepPlan was schema-valid and passed safety lint, but it had retryable quality issues on attempt {attempt}/3.\n\
Improve the plan without weakening safety rules:\n{issues}\n\n\
Hard constraints:\n\
- Keep the original top-level goal unchanged.\n\
- Keep expected_paths workspace-relative and owned by one setup/implement step.\n\
- Keep verify commands deterministic, single commands, and free of shell control syntax.\n\
- Do not start dev servers or perform dependency installation in verify.\n\
- Prefer tests, builds, smoke checks, or content assertions over file existence checks.\n\
- For Next.js, put package.json and app entrypoints before npm run build.\n\n\
- Do not add a final report step for normal success; report is only for explicit blockers.\n\
- In a fresh create/scaffold workspace, start with setup/implement work that owns an artifact instead of an empty inspect wrapper.\n\
- Keep verify-only steps connected to prior artifacts through the verify command or step instruction.\n\n\
Return only one JSON object with top-level goal and steps. Do not use markdown fences.\n\
Goal: {goal}"
    )
}

fn lint_retry_hard_constraints(
    report: &PlanLintReport,
    categories_seen: &BTreeSet<String>,
) -> Vec<&'static str> {
    let mut categories = categories_seen.clone();
    for err in &report.errors {
        categories.insert(err.category.clone());
    }
    let mut out = vec![
        "Hard constraints to preserve in the corrected plan:",
        "- Keep the original top-level goal unchanged.",
        "- Use setup, implement, verify, and report responsibilities separately.",
        "- Implement/setup steps own expected_paths; verify-only steps should normally have empty expected_paths.",
        "- Each implementation-owned expected path must have exactly one owner step.",
    ];
    if categories.contains("verify_policy") {
        out.push("- Verify commands must be single commands without &&, ||, |, ;, backticks, or command substitution.");
        out.push(
            "- Split multiple checks into multiple verify steps or multiple verify list items.",
        );
        out.push("- Preserve the verification meaning; do not replace a real check with a weak file-existence check.");
        out.push("- Move setup, dependency installation, and dev-server readiness into setup/implement instructions or external postcheck, not verify.");
        out.push("- If a Node smoke check needs multiple statements, create a smoke-check.js artifact in an implement step and verify with a single command such as node smoke-check.js.");
        out.push("- If documentation needs multiple assertions, prefer separate grep -q commands in the verify list, each checking one phrase in one file.");
    }
    if categories.contains("path_ownership") {
        out.push("- Do not duplicate expected_paths across steps; move shared validation into verify commands instead.");
    }
    if categories.contains("dependency_order") {
        out.push("- Put package manifest and dependency setup before npm/pnpm/yarn build or test verification.");
        out.push("- Python stdlib unittest does not require dependency setup by itself.");
    }
    if categories.contains("side_effect_expected_path") {
        out.push("- Do not put dependency/build side-effect directories such as node_modules, .next, __pycache__, .venv, venv, dist, build, target, coverage, or out in expected_paths unless the user goal explicitly asks for that artifact path.");
    }
    if categories.contains("contract") {
        out.push("- Implement steps must declare concrete workspace-relative expected_paths.");
        out.push("- Inspect and report steps must not declare expected_paths or verify commands.");
        out.push("- Verify steps must not request file changes in their instruction.");
    }
    out
}

fn step_plan_messages(prompt: &str) -> Vec<crate::state::ConversationMessage> {
    vec![
        crate::state::ConversationMessage::system(plan_generation_system_prompt()),
        crate::state::ConversationMessage::user(prompt.to_string()),
    ]
}

fn ultra_plan_generation_messages(
    goal: &str,
    config: &Config,
) -> Vec<crate::state::ConversationMessage> {
    let intent = config.resolved_intent(goal);
    vec![
        crate::state::ConversationMessage::system(ultra_plan_generation_system_prompt(
            &config.profile,
            &config.style,
            intent,
        )),
        crate::state::ConversationMessage::user(ultra_plan_generation_user_prompt(
            goal,
            &config.profile,
            &config.style,
            intent,
        )),
    ]
}

fn ultra_plan_generation_system_prompt(profile: &str, style: &str, intent: &str) -> String {
    let runtime = resolve_profile_runtime(profile);
    let intent_rules = crate::planner::fix_runtime::generation_rules(intent);
    let style_rules = match style {
        "tdd" => {
            "- Style tdd: use phases for inspect, failing test, implementation, focused verification, and cleanup. The failing-test phase should ask /plan-run to create an expected_result:\"fail\" red verification step.\n"
        }
        "test-hardening" => {
            "- Style test-hardening: inspect existing tests, identify uncovered behavior, add focused tests, make only necessary fixes, then run broader verification.\n"
        }
        _ => {
            "- Style default: use ordinary phased delivery unless the user explicitly asks for TDD or test hardening.\n"
        }
    };
    let profile_rules = runtime.generation_rules(intent).unwrap_or(
        "- Profile generic: keep phases concrete, local, deterministic, and safe. Separate setup, implementation, and verification responsibilities.\n",
    );
    let styling_choice_rule = runtime.styling_choice_rule();
    format!(
        "You are CommandAgent's ultra planner. You do not execute tools or emit tool calls. Produce a top-level phase plan whose phases will each be executed by /plan-run.\n\
Output YAML only, with this exact shape:\n\
goal: \"...\"\n\
profile: \"{profile}\"\n\
style: \"{style}\"\n\
intent: \"{intent}\"\n\
phases:\n\
  - id: \"kebab-id\"\n\
    prompt: \"focused natural-language /plan-run goal\"\n\
Rules:\n\
- Return 2 to 6 phases for most tasks, never more than 8.\n\
- Each phase prompt must be a focused natural-language task that can be handled by one /plan-run.\n\
- Phase prompts should name the concrete outcome and the verification expectation when practical.\n\
- If the user goal contains a Required final artifacts list, preserve those exact repository-relative paths across phases. Do not rename or relocate them.\n\
- Do not make a phase prompt a shell command or a REPL command.\n\
- Do not include long-running dev servers, network setup, or package installation as a phase unless the user explicitly requires it.\n\
- Phase descriptions must not request dev-server startup or page-load/browser-route verification outside the final phase.\n\
- Browser readiness and page-route acceptance are verified by the runtime at final acceptance.\n\
- Stop at a clean final verification or cleanup phase.\n\
{styling_choice_rule}{profile_rules}{style_rules}{intent_rules}"
    )
}

fn ultra_plan_generation_user_prompt(
    goal: &str,
    profile: &str,
    style: &str,
    intent: &str,
) -> String {
    format!(
        "Create an UltraPlan YAML for this task using profile `{profile}`, style `{style}`, and work intent `{intent}`.\n\
Use the exact YAML shape from the system message and return YAML only.\n\n\
Task:\n{goal}"
    )
}

fn build_ultra_plan_schema_retry_prompt(goal: &str, error: &str, attempt: usize) -> String {
    format!(
        "Your previous UltraPlan output failed schema parsing on attempt {attempt}/{ULTRA_PLAN_GENERATION_ATTEMPTS}: {error}.\n\
Return corrected YAML only, no markdown fences, no prose, and no tool calls.\n\
Required YAML shape:\n\
goal: \"{goal}\"\n\
profile: \"...\"\n\
style: \"...\"\n\
intent: \"...\"\n\
phases:\n\
  - id: \"kebab-id\"\n\
    prompt: \"focused natural-language /plan-run goal\"\n\
Rules:\n\
- Include top-level goal and 2-8 phases.\n\
- Each phase must have id and prompt.\n\
- Phase prompts must be natural-language tasks, not shell commands.\n\n\
Goal: {goal}"
    )
}

fn build_ultra_plan_lint_retry_prompt(
    goal: &str,
    report: &PlanLintReport,
    attempt: usize,
) -> String {
    let errors = report
        .errors
        .iter()
        .map(|err| format!("- [{}] {}", err.category, err.message))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Your previous UltraPlan YAML failed deterministic lint on attempt {attempt}/{ULTRA_PLAN_GENERATION_ATTEMPTS}.\n\
Fix these issues without weakening safety rules:\n{errors}\n\n\
Return corrected YAML only, no markdown fences, no prose, and no tool calls.\n\
Hard constraints:\n\
- Keep 2-8 phases.\n\
- Use unique kebab-case phase ids.\n\
- Phase prompts must be natural-language /plan-run goals, not shell commands or REPL commands.\n\
- Keep concrete outcomes and verification expectations in phase prompts.\n\
- Preserve any Required final artifacts from the user goal.\n\n\
Goal: {goal}"
    )
}

fn build_ultra_plan_tool_call_retry_prompt(goal: &str, attempt: usize) -> String {
    format!(
        "Your previous UltraPlan generation attempted to emit tool calls on attempt {attempt}/{ULTRA_PLAN_GENERATION_ATTEMPTS}.\n\
Do not call tools. Return corrected UltraPlan YAML only.\n\
Use natural-language phase prompts for later /plan-run execution.\n\n\
Goal: {goal}"
    )
}

fn normalize_ultra_plan_metadata(plan: &mut UltraPlan, goal: &str, config: &Config) -> Vec<String> {
    let mut normalized = Vec::new();
    if plan.goal != goal {
        plan.goal = goal.to_string();
        normalized.push("goal".to_string());
    }
    if plan.profile != config.profile {
        plan.profile = config.profile.clone();
        normalized.push("profile".to_string());
    }
    if plan.style != config.style {
        plan.style = config.style.clone();
        normalized.push("style".to_string());
    }
    if plan.intent != config.resolved_intent(goal) {
        plan.intent = config.resolved_intent(goal).to_string();
        normalized.push("intent".to_string());
    }
    normalized
}

fn tool_call_names(tool_calls: &[crate::state::ToolCall]) -> String {
    let names = tool_calls
        .iter()
        .map(|call| call.name.as_str())
        .collect::<Vec<_>>()
        .join(",");
    if names.is_empty() {
        "<unknown>".to_string()
    } else {
        names
    }
}

fn plan_generation_system_prompt() -> String {
    [
        "You are a deterministic planning component for a local coding agent.",
        "Return only one JSON object. Do not include markdown fences, prose, comments, or XML.",
        "The JSON object must have top-level keys: goal, steps.",
        "Each step must include id, kind, instruction, expected_paths, verify, and expected_result.",
        "Step id must be a quoted string. Use kebab-case such as inspect-workspace.",
        "expected_result must be exactly \"pass\" or \"fail\". Do not put prose in expected_result.",
        "Use 2 to 8 steps for normal implementation tasks, with a maximum of 12 steps.",
        "Allowed step kinds: inspect, setup, implement, verify, report.",
        "Use inspect only for reading or assessing state; it must not declare expected_paths or verify commands.",
        "Use setup for dependency manifests or setup files; do not run build/test verification in setup.",
        "Use implement/create/edit/work/repair semantics as implement steps, with concrete expected_paths.",
        "Use verify only for deterministic checks. Do not request file changes in verify steps.",
        "Use report only for explicit blockers such as dependency_missing, unavailable external service, required user input, or unfixable local blocker. Report is not success.",
        "Normal success plans must end with a verify step or an artifact-owning setup/implement step, not a final summary report.",
        "Expected paths must be workspace-relative, exact, and owned by one implement/setup step.",
        "Do not put dependency/build side-effect directories such as node_modules, .next, __pycache__, .venv, venv, dist, build, target, coverage, or out in expected_paths unless the user goal explicitly asks for that artifact path.",
        "Prefer tests, builds, smoke checks, or content assertions over file existence checks.",
        "Use file existence checks only as a fallback when no stronger deterministic verification fits.",
        "Implement/setup instructions should say what each expected artifact will contain.",
        "Verify commands must not use shell control syntax such as &&, ||, |, or ;.",
        "For multi-statement Node checks, create a smoke-check.js artifact and verify with node smoke-check.js.",
        "Do not put dev server startup or network setup in verify.",
        "For Next.js, create package.json and app entrypoints before npm run build.",
        "Do not use node --check for .ts or .tsx files.",
    ]
    .join("\n")
}

fn build_step_plan_user_prompt(goal: &str, config: &Config) -> String {
    match config.prompt_layout {
        PromptLayout::Stable => build_step_plan_user_prompt_stable(goal, config),
        PromptLayout::Legacy => build_step_plan_user_prompt_legacy(goal, config),
    }
}

fn build_step_plan_user_prompt_stable(goal: &str, config: &Config) -> String {
    let mut prompt = String::new();
    let runtime = resolve_profile_runtime(&config.profile);
    let expected_paths = runtime.expected_scaffold_paths(&config.workspace_root, goal);
    if !expected_paths.is_empty() {
        prompt.push_str("Required final artifacts:\n");
        for path in expected_paths {
            prompt.push_str("- ");
            prompt.push_str(&path);
            prompt.push('\n');
        }
    }
    let expectations = runtime.quality_expectations(&config.workspace_root, goal);
    if !expectations.preferred_verify.is_empty()
        || !expectations
            .dependency_order_hint
            .as_deref()
            .unwrap_or("")
            .is_empty()
    {
        if !prompt.is_empty() {
            prompt.push('\n');
        }
        prompt.push_str("Profile verification expectations:\n");
        if !expectations.preferred_verify.is_empty() {
            prompt.push_str("- Preferred deterministic verify commands:\n");
            for command in expectations.preferred_verify {
                prompt.push_str("  - ");
                prompt.push_str(&command);
                prompt.push('\n');
            }
        }
        if let Some(hint) = expectations.dependency_order_hint {
            prompt.push_str("- Order: ");
            prompt.push_str(&hint);
            prompt.push('\n');
        }
        if !expectations.forbidden_verify.is_empty() {
            prompt.push_str("- Do not use these in verify:\n");
            for command in expectations.forbidden_verify {
                prompt.push_str("  - ");
                prompt.push_str(&command);
                prompt.push('\n');
            }
        }
    }
    if let Some(note) = preprovisioned_scaffold_note(&config.workspace_root, runtime) {
        if !prompt.is_empty() {
            prompt.push('\n');
        }
        prompt.push_str("Pre-provisioned scaffold note:\n");
        prompt.push_str("- ");
        prompt.push_str(&note);
        prompt.push('\n');
    }
    if is_ultra_phase_step_goal(goal) {
        if !prompt.is_empty() {
            prompt.push('\n');
        }
        prompt.push_str("Ultra phase hard constraints:\n");
        prompt.push_str("- StepPlan.goal must be ONE short sentence naming the phase outcome; never copy the phase context, unmet-requirements, or adherence lists into goal -- details belong in step instructions.\n");
        prompt.push_str("- Do not put dev-server startup, page-load probes, curl localhost, or dependency installation in verify.\n");
        prompt.push_str("- Browser readiness is verified by the runtime at final acceptance.\n");
        prompt.push_str("- GOOD verify examples:\n");
        prompt.push_str("  - test -f package.json\n");
        prompt.push_str("  - node -p \"require('./package.json').scripts.dev\"\n");
        prompt.push_str("- BAD verify examples:\n");
        prompt.push_str("  - npm install\n");
        prompt.push_str("  - npm run dev\n");
        prompt.push_str("  - curl http://localhost:3011\n");
    }
    if !prompt.is_empty() {
        prompt.push('\n');
    }
    prompt.push_str("Create a step plan for this task:\n");
    prompt.push_str(goal);
    prompt
}

fn build_step_plan_user_prompt_legacy(goal: &str, config: &Config) -> String {
    let mut prompt = format!("Create a step plan for this task:\n{goal}");
    let runtime = resolve_profile_runtime(&config.profile);
    let expected_paths = runtime.expected_scaffold_paths(&config.workspace_root, goal);
    if !expected_paths.is_empty() {
        prompt.push_str("\n\nRequired final artifacts:\n");
        for path in expected_paths {
            prompt.push_str("- ");
            prompt.push_str(&path);
            prompt.push('\n');
        }
    }
    let expectations = runtime.quality_expectations(&config.workspace_root, goal);
    if !expectations.preferred_verify.is_empty()
        || !expectations
            .dependency_order_hint
            .as_deref()
            .unwrap_or("")
            .is_empty()
    {
        prompt.push_str("\nProfile verification expectations:\n");
        if !expectations.preferred_verify.is_empty() {
            prompt.push_str("- Preferred deterministic verify commands:\n");
            for command in expectations.preferred_verify {
                prompt.push_str("  - ");
                prompt.push_str(&command);
                prompt.push('\n');
            }
        }
        if let Some(hint) = expectations.dependency_order_hint {
            prompt.push_str("- Order: ");
            prompt.push_str(&hint);
            prompt.push('\n');
        }
        if !expectations.forbidden_verify.is_empty() {
            prompt.push_str("- Do not use these in verify:\n");
            for command in expectations.forbidden_verify {
                prompt.push_str("  - ");
                prompt.push_str(&command);
                prompt.push('\n');
            }
        }
    }
    if let Some(note) = preprovisioned_scaffold_note(&config.workspace_root, runtime) {
        prompt.push_str("\n\nPre-provisioned scaffold note:\n");
        prompt.push_str("- ");
        prompt.push_str(&note);
        prompt.push('\n');
    }
    if is_ultra_phase_step_goal(goal) {
        prompt.push_str("\nUltra phase hard constraints:\n");
        prompt.push_str("- StepPlan.goal must be ONE short sentence naming the phase outcome; never copy the phase context, unmet-requirements, or adherence lists into goal -- details belong in step instructions.\n");
        prompt.push_str("- Do not put dev-server startup, page-load probes, curl localhost, or dependency installation in verify.\n");
        prompt.push_str("- Browser readiness is verified by the runtime at final acceptance.\n");
        prompt.push_str("- GOOD verify examples:\n");
        prompt.push_str("  - test -f package.json\n");
        prompt.push_str("  - node -p \"require('./package.json').scripts.dev\"\n");
        prompt.push_str("- BAD verify examples:\n");
        prompt.push_str("  - npm install\n");
        prompt.push_str("  - npm run dev\n");
        prompt.push_str("  - curl http://localhost:3011\n");
    }
    prompt
}

fn preprovisioned_scaffold_note(root: &Path, runtime: &dyn ProfileRuntime) -> Option<String> {
    let scaffold_paths = runtime.setup_scaffold_paths(root);
    (!scaffold_paths.is_empty()).then(|| {
        "Required scaffold files are authored before phase 1 when absent; verify or extend the scaffold rather than re-planning file creation.".to_string()
    })
}

fn is_ultra_phase_step_goal(goal: &str) -> bool {
    goal.contains("Original ultra goal:")
        && goal.contains("Phase id:")
        && goal.contains("Phase task:")
}

fn plan_quality_context(config: &Config, goal: &str) -> PlanQualityContext {
    let expectations =
        resolve_profile_runtime(&config.profile).quality_expectations(&config.workspace_root, goal);
    let workspace = workspace_quality_snapshot(&config.workspace_root);
    PlanQualityContext {
        profile: config.profile.clone(),
        required_artifacts: expectations.required_artifacts,
        preferred_verify: expectations.preferred_verify,
        dependency_order_hint: expectations.dependency_order_hint,
        task_intent: config.resolved_intent(goal).to_string(),
        workspace_context_known: workspace.context_known,
        workspace_snapshot_class: workspace.snapshot_class,
        has_user_seed_files: workspace.has_user_seed_files,
        has_only_agent_metadata: workspace.has_only_agent_metadata,
    }
}

#[derive(Debug, Clone)]
struct WorkspaceQualitySnapshot {
    context_known: bool,
    snapshot_class: String,
    has_user_seed_files: bool,
    has_only_agent_metadata: bool,
}

fn workspace_quality_snapshot(root: &Path) -> WorkspaceQualitySnapshot {
    let Ok(entries) = std::fs::read_dir(root) else {
        return WorkspaceQualitySnapshot {
            context_known: false,
            snapshot_class: "unknown".to_string(),
            has_user_seed_files: false,
            has_only_agent_metadata: false,
        };
    };
    let mut user_entries = 0usize;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if is_agent_metadata_entry(&name) {
            continue;
        }
        user_entries += 1;
    }
    let has_only_agent_metadata = user_entries == 0;
    WorkspaceQualitySnapshot {
        context_known: true,
        snapshot_class: if has_only_agent_metadata {
            "metadata_only".to_string()
        } else {
            "user_files".to_string()
        },
        has_user_seed_files: user_entries > 0,
        has_only_agent_metadata,
    }
}

fn is_agent_metadata_entry(name: &str) -> bool {
    matches!(
        name,
        ".git" | ".anvil" | ".codex" | ".agents" | "target" | ".DS_Store"
    ) || name.starts_with("commandagent-eval-")
}

fn strengthen_step_plan_for_profile(plan: &mut StepPlan, config: &Config) {
    let runtime = resolve_profile_runtime(&config.profile);
    let is_scaffold = plan.goal.to_ascii_lowercase().contains("scaffold");
    let Some(target_index) = plan
        .steps
        .iter()
        .rposition(|step| matches!(step.step_kind(), StepKind::Setup | StepKind::Implement))
        .or_else(|| {
            is_scaffold.then(|| {
                plan.steps
                    .iter()
                    .rposition(|step| step.step_kind() == StepKind::Report)
            })?
        })
    else {
        return;
    };
    let target = &mut plan.steps[target_index];
    if is_scaffold {
        for path in runtime.expected_scaffold_paths(&config.workspace_root, &plan.goal) {
            if path.ends_with("package.json") && !target.expected_paths.contains(&path) {
                target.expected_paths.push(path);
            }
        }
        if !target.expected_paths.is_empty() && target.kind == "report" {
            target.kind = "implement".to_string();
        }
    }
    if let Some(guidance) = runtime.guidance(&plan.goal) {
        target.instruction = format!("{}\n\nProfile contract:\n{}", target.instruction, guidance);
    }
}

fn build_step_prompt(
    plan: &StepPlan,
    step: &PlanStep,
    context: &StepPromptContext,
    layout: PromptLayout,
) -> String {
    match layout {
        PromptLayout::Stable => build_step_prompt_stable(plan, step, context),
        PromptLayout::Legacy => build_step_prompt_legacy(plan, step, context),
    }
}

fn build_step_prompt_stable(
    _plan: &StepPlan,
    step: &PlanStep,
    context: &StepPromptContext,
) -> String {
    let mut prompt = String::new();
    prompt.push_str("Execute exactly one StepPlan step.\n\n");
    prompt.push_str("Overall goal:\n");
    prompt.push_str(&context.overall_goal);

    prompt.push_str("\n\nRequired final artifacts:\n");
    append_bullets_or_none(&mut prompt, &context.required_final_artifacts);

    prompt.push_str("\n\nRequired final capabilities:\n");
    append_bullets_or_none(&mut prompt, &context.final_required_capabilities);

    prompt.push_str("\n\nRequired final evidence:\n");
    append_bullets_or_none(&mut prompt, &context.final_required_evidence);

    prompt.push_str(
        "\n\nStep execution rules:\n\
- Work only on the current step unless a prior artifact is required to verify this step.\n\
- Prefer Write/Edit/MultiEdit for declared expected paths.\n\
- If this step has verification commands, use them as the deterministic success contract.\n\
- If verification fails, make a bounded step-local repair and re-check the declared contract.\n\
- For Next.js/App Router work, keep a single route-bound implementation; do not leave capability components unimported.\n\
- Do not claim the plan is complete until this step's expected paths and verification contract are satisfied.\n\
- Report an explicit blocker only when the blocker cannot be resolved locally.",
    );

    prompt.push_str("\n\nCurrent objective: ");
    prompt.push_str(&eval_events::body_snippet(&step.instruction));

    prompt.push_str("\n\nCurrent step id:\n");
    prompt.push_str(&step.id);
    prompt.push_str("\n\nCurrent step kind:\n");
    prompt.push_str(&step.kind);
    prompt.push_str("\n\nCurrent step instruction:\n");
    prompt.push_str(&step.instruction);

    prompt.push_str("\n\nArtifacts available from previous steps:\n");
    append_bullets_or_none(&mut prompt, &context.prior_expected_paths);

    prompt.push_str("\n\nExpected paths after this step:\n");
    append_bullets_or_none(&mut prompt, &step.expected_paths);

    prompt.push_str("\n\nVerification commands for this step:\n");
    append_bullets_or_none(&mut prompt, &step.verify);

    prompt.push_str("\n\nExpected verification result:\n");
    prompt.push_str(step_expected_result(step));
    prompt
}

fn build_step_prompt_legacy(
    _plan: &StepPlan,
    step: &PlanStep,
    context: &StepPromptContext,
) -> String {
    let mut prompt = String::new();
    prompt.push_str("Execute exactly one StepPlan step.\n\n");
    prompt.push_str("Overall goal:\n");
    prompt.push_str(&context.overall_goal);
    prompt.push_str("\n\nCurrent step id:\n");
    prompt.push_str(&step.id);
    prompt.push_str("\n\nCurrent step kind:\n");
    prompt.push_str(&step.kind);
    prompt.push_str("\n\nCurrent step instruction:\n");
    prompt.push_str(&step.instruction);

    prompt.push_str("\n\nRequired final artifacts:\n");
    append_bullets_or_none(&mut prompt, &context.required_final_artifacts);

    prompt.push_str("\n\nRequired final capabilities:\n");
    append_bullets_or_none(&mut prompt, &context.final_required_capabilities);

    prompt.push_str("\n\nRequired final evidence:\n");
    append_bullets_or_none(&mut prompt, &context.final_required_evidence);

    prompt.push_str("\n\nArtifacts available from previous steps:\n");
    append_bullets_or_none(&mut prompt, &context.prior_expected_paths);

    prompt.push_str("\n\nExpected paths after this step:\n");
    append_bullets_or_none(&mut prompt, &step.expected_paths);

    prompt.push_str("\n\nVerification commands for this step:\n");
    append_bullets_or_none(&mut prompt, &step.verify);

    prompt.push_str("\n\nExpected verification result:\n");
    prompt.push_str(step_expected_result(step));

    prompt.push_str(
        "\n\nStep execution rules:\n\
- Work only on the current step unless a prior artifact is required to verify this step.\n\
- Prefer Write/Edit/MultiEdit for declared expected paths.\n\
- If this step has verification commands, use them as the deterministic success contract.\n\
- If verification fails, make a bounded step-local repair and re-check the declared contract.\n\
- For Next.js/App Router work, keep a single route-bound implementation; do not leave capability components unimported.\n\
- Do not claim the plan is complete until this step's expected paths and verification contract are satisfied.\n\
- Report an explicit blocker only when the blocker cannot be resolved locally.",
    );
    prompt
}

fn append_bullets_or_none(prompt: &mut String, items: &[String]) {
    if items.is_empty() {
        prompt.push_str("- none\n");
        return;
    }
    for item in items {
        prompt.push_str("- ");
        prompt.push_str(item);
        prompt.push('\n');
    }
}

fn step_expected_result(step: &PlanStep) -> &str {
    let trimmed = step.expected_result.trim();
    if trimmed.is_empty() { "pass" } else { trimmed }
}

fn emit_step_prompt_contract(
    config: &Config,
    step: &PlanStep,
    context: &StepPromptContext,
    prompt: &str,
) {
    let prior_context_applicable =
        step.step_kind() == StepKind::Verify && !context.prior_expected_paths.is_empty();
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "step_prompt_contract",
            "prompt_contract_version": 1,
            "step_id": step.id,
            "step_kind": step.kind,
            "has_overall_goal": prompt.contains("Overall goal:"),
            "has_required_final_artifacts": prompt.contains("Required final artifacts:"),
            "has_required_final_capabilities": prompt.contains("Required final capabilities:"),
            "has_required_final_evidence": prompt.contains("Required final evidence:"),
            "required_final_capabilities": context.final_required_capabilities.clone(),
            "required_final_evidence": context.final_required_evidence.clone(),
            "has_expected_paths": prompt.contains("Expected paths after this step:"),
            "has_verify_commands": prompt.contains("Verification commands for this step:"),
            "has_expected_result": prompt.contains("Expected verification result:"),
            "has_prior_artifact_context": !context.prior_expected_paths.is_empty()
                && prompt.contains("Artifacts available from previous steps:"),
            "prior_artifact_context_applicable": prior_context_applicable,
            "has_bounded_repair_policy": prompt.contains("bounded step-local repair"),
            "prompt_body_saved": false,
        }),
    );
}

#[allow(dead_code)]
fn prompt_with_required_paths(instruction: &str, paths: &[String]) -> String {
    if paths.is_empty() || instruction.contains("Required final artifacts:") {
        return instruction.to_string();
    }
    format!(
        "{}\n\nRequired final artifacts:\n{}",
        instruction,
        paths
            .iter()
            .map(|path| format!("- {path}"))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

fn ultra_phase_prompt(
    plan: &UltraPlan,
    phase: &UltraPhase,
    config: &Config,
    context: &UltraRunContext,
    fix_runtime: Option<&crate::planner::fix_runtime::FixRuntime>,
) -> String {
    let prompt = match config.prompt_layout {
        PromptLayout::Stable => ultra_phase_prompt_stable(plan, phase, config, context),
        PromptLayout::Legacy => ultra_phase_prompt_legacy(plan, phase, config, context),
    };
    let prompt = crate::planner::fix_runtime::phase_prompt(
        plan,
        phase,
        prompt,
        config.intent_override == Some(IntentId::Fix),
    );
    let prompt = crate::planner::fix_reproducer::attach_to_phase_prompt(
        plan,
        phase,
        config.eval_events_path.as_deref(),
        prompt,
    );
    let prompt = crate::planner::fix_diagnostics::attach_to_phase_prompt(
        phase,
        fix_runtime.and_then(|runtime| runtime.repair_diagnostic()),
        prompt,
    );
    let prompt = crate::planner::fix_contract_predicate::attach_to_phase_prompt(
        phase,
        fix_runtime.and_then(|runtime| runtime.contract_predicate()),
        prompt,
    );
    crate::planner::fix_runtime::attach_phase_policy_prompt(fix_runtime, phase, prompt)
}

fn ultra_phase_prompt_stable(
    plan: &UltraPlan,
    phase: &UltraPhase,
    config: &Config,
    context: &UltraRunContext,
) -> String {
    let runtime = resolve_profile_runtime(&plan.profile);
    let expected_paths = runtime.expected_scaffold_paths(&config.workspace_root, &plan.goal);
    let expectations = runtime.quality_expectations(&config.workspace_root, &plan.goal);
    let generation_rules = runtime.generation_rules(&plan.intent).unwrap_or("- none\n");
    let runtime_contract = runtime.runtime_contract(&plan.intent, &plan.goal);
    let phase_contract_text = format!("{}\n{}", plan.goal, phase.prompt);
    let required_capabilities = runtime.infer_required_capabilities(&phase_contract_text);
    let required_evidence =
        runtime_required_evidence(runtime, &phase_contract_text, &required_capabilities);
    let required = if expected_paths.is_empty() {
        String::new()
    } else {
        format!(
            "\n\nRequired final artifacts:\n{}",
            expected_paths
                .iter()
                .map(|path| format!("- {path}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };
    let capability_section = if required_capabilities.is_empty() {
        String::new()
    } else {
        format!(
            "\n\nRequired final capabilities:\n{}",
            required_capabilities
                .iter()
                .map(|capability| format!("- {capability}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };
    let evidence_section = if required_evidence.is_empty() {
        String::new()
    } else {
        format!(
            "\n\nRequired final evidence:\n{}",
            required_evidence
                .iter()
                .map(|evidence| format!("- {evidence}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };
    let preferred_verify = if expectations.preferred_verify.is_empty() {
        "- none".to_string()
    } else {
        expectations
            .preferred_verify
            .iter()
            .map(|command| format!("- {command}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let workspace_snapshot = compact_workspace_snapshot(&config.workspace_root);
    let prior_context = context.render_prompt_section();
    let unmet_final_requirements = context.render_unmet_final_requirements_section();
    let plan_adherence = plan_adherence_report(plan, &config.workspace_root);
    let requested_features = render_requested_features_not_detected_line(&plan_adherence.missing);
    let route_bound_constraint = runtime.route_bound_constraint();
    let scaffold_note = preprovisioned_scaffold_note(&config.workspace_root, runtime)
        .map(|note| format!("\nPre-provisioned scaffold note:\n- {note}\n"))
        .unwrap_or_default();
    let mut prompt = String::new();
    prompt.push_str("Profile generation rules:\n");
    prompt.push_str(generation_rules);
    prompt.push_str("\nProfile runtime contract:\n");
    prompt.push_str(&runtime_contract);
    prompt.push_str(route_bound_constraint);
    prompt.push_str(&scaffold_note);
    prompt.push_str("Deterministic verification preference:\n");
    prompt.push_str(&preferred_verify);
    prompt.push('\n');
    prompt.push_str(&required);
    prompt.push_str("\n\nOriginal ultra goal: ");
    prompt.push_str(&plan.goal);
    prompt.push_str("\nProfile: ");
    prompt.push_str(&plan.profile);
    prompt.push_str("\nStyle: ");
    prompt.push_str(&plan.style);
    prompt.push_str("\nIntent: ");
    prompt.push_str(&plan.intent);
    prompt.push_str("\nPhase id: ");
    prompt.push_str(&phase.id);
    prompt.push_str("\nPhase task: ");
    prompt.push_str(&phase.prompt);
    prompt.push_str("\n\nWorkspace snapshot:\n");
    prompt.push_str(&workspace_snapshot);
    prompt.push_str("\n\n");
    prompt.push_str(&prior_context);
    prompt.push_str("\n\n");
    prompt.push_str(&unmet_final_requirements);
    prompt.push_str("\n\n");
    prompt.push_str(&requested_features);
    prompt.push_str(&capability_section);
    prompt.push_str(&evidence_section);
    prompt
}

fn ultra_phase_prompt_legacy(
    plan: &UltraPlan,
    phase: &UltraPhase,
    config: &Config,
    context: &UltraRunContext,
) -> String {
    let runtime = resolve_profile_runtime(&plan.profile);
    let expected_paths = runtime.expected_scaffold_paths(&config.workspace_root, &plan.goal);
    let expectations = runtime.quality_expectations(&config.workspace_root, &plan.goal);
    let generation_rules = runtime.generation_rules(&plan.intent).unwrap_or("- none\n");
    let runtime_contract = runtime.runtime_contract(&plan.intent, &plan.goal);
    let phase_contract_text = format!("{}\n{}", plan.goal, phase.prompt);
    let required_capabilities = runtime.infer_required_capabilities(&phase_contract_text);
    let required_evidence =
        runtime_required_evidence(runtime, &phase_contract_text, &required_capabilities);
    let required = if expected_paths.is_empty() {
        String::new()
    } else {
        format!(
            "\n\nRequired final artifacts:\n{}",
            expected_paths
                .iter()
                .map(|path| format!("- {path}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };
    let capability_section = if required_capabilities.is_empty() {
        String::new()
    } else {
        format!(
            "\n\nRequired final capabilities:\n{}",
            required_capabilities
                .iter()
                .map(|capability| format!("- {capability}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };
    let evidence_section = if required_evidence.is_empty() {
        String::new()
    } else {
        format!(
            "\n\nRequired final evidence:\n{}",
            required_evidence
                .iter()
                .map(|evidence| format!("- {evidence}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };
    let preferred_verify = if expectations.preferred_verify.is_empty() {
        "- none".to_string()
    } else {
        expectations
            .preferred_verify
            .iter()
            .map(|command| format!("- {command}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let workspace_snapshot = compact_workspace_snapshot(&config.workspace_root);
    let prior_context = context.render_prompt_section();
    let unmet_final_requirements = context.render_unmet_final_requirements_section();
    let plan_adherence = plan_adherence_report(plan, &config.workspace_root);
    let requested_features = render_requested_features_not_detected_line(&plan_adherence.missing);
    let route_bound_constraint = runtime.route_bound_constraint();
    let scaffold_note = preprovisioned_scaffold_note(&config.workspace_root, runtime)
        .map(|note| format!("\nPre-provisioned scaffold note:\n- {note}\n"))
        .unwrap_or_default();
    let mut prompt = String::new();
    prompt.push_str("Original ultra goal: ");
    prompt.push_str(&plan.goal);
    prompt.push_str("\nProfile: ");
    prompt.push_str(&plan.profile);
    prompt.push_str("\nStyle: ");
    prompt.push_str(&plan.style);
    prompt.push_str("\nIntent: ");
    prompt.push_str(&plan.intent);
    prompt.push_str("\nPhase id: ");
    prompt.push_str(&phase.id);
    prompt.push_str("\nPhase task: ");
    prompt.push_str(&phase.prompt);
    prompt.push_str("\n\nWorkspace snapshot:\n");
    prompt.push_str(&workspace_snapshot);
    prompt.push_str("\n\n");
    prompt.push_str(&prior_context);
    prompt.push_str("\n\n");
    prompt.push_str(&unmet_final_requirements);
    prompt.push_str("\n\n");
    prompt.push_str(&requested_features);
    prompt.push_str("\n\nProfile generation rules:\n");
    prompt.push_str(generation_rules);
    prompt.push_str("\nProfile runtime contract:\n");
    prompt.push_str(&runtime_contract);
    prompt.push_str(route_bound_constraint);
    prompt.push_str(&scaffold_note);
    prompt.push_str("Deterministic verification preference:\n");
    prompt.push_str(&preferred_verify);
    prompt.push('\n');
    prompt.push_str(&required);
    prompt.push_str(&capability_section);
    prompt.push_str(&evidence_section);
    prompt
}

fn phase_goal_one_liner(prompt: &str) -> String {
    let mut line = prompt.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() > 180 {
        line = line.chars().take(180).collect::<String>();
        line.push_str("...");
    }
    if line.is_empty() {
        "restore the rolled-back phase intent".to_string()
    } else {
        line
    }
}

#[allow(dead_code)]
fn _format_report(report: &VerificationReport) -> String {
    format!("{:?}", report.status)
}

fn resolve_plan_file_path(root: &Path, path: &Path) -> anyhow::Result<PathBuf> {
    let root = root.canonicalize()?;
    let canonical = if path.is_absolute() {
        path.canonicalize()?
    } else {
        let raw = path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("plan path is not valid UTF-8"))?;
        crate::tools::path_guard::resolve_existing(&root, raw)?
    };
    if !canonical.starts_with(&root) {
        anyhow::bail!("plan path escapes workspace");
    }
    Ok(canonical)
}

#[cfg(test)]
#[path = "runner/tests/mod.rs"]
mod tests;
