// Driver, initialization, and prompt construction extracted from the runner facade.
// Keep observable strings and event order byte-compatible with the pre-split runner.
#[allow(unused_imports)]
use super::{
    AssistantReply, BTreeSet, ChatClient, CompletionContract, Config, Duration,
    GeneratedStepPlanFieldDefault, IntentId, InteractionUi, MissingImport, NOOP_UI, Path, PathBuf,
    PlanLintReport, PlanQualityContext, PlanQualityReport, PlanStep, ProfileRuntime,
    ProfileSnapshot, PromptLayout, ProviderCallScope, RepairContext, RepairReachability,
    RunSessionError, SanitizerReport, SessionSnapshot, StepKind, StepPlan, StepPromptContext,
    StepRunOutcome, UiStatus, UltraPhase, UltraPlan, UltraRunContext, VerificationReport,
    capability_evidence_remedy_lines, capability_evidence_unresolved_reason,
    compact_workspace_snapshot, dedup_strings, eval_events, extract_json_object,
    format_missing_import_findings, hook_snapshot, json, model_for,
    parse_generated_step_plan_json_with_report, parse_step_plan, parse_ultra_plan,
    plan_adherence_report, provider_call, reachability_failure_kind, render_prompt_bullets,
    render_requested_features_not_detected_line, render_step_plan, render_ultra_plan,
    repair_generated_step_plan_contract, repair_targeting, resolve_existing,
    resolve_profile_runtime, run_step_plan_with_session_with_ui, runtime_required_evidence,
    sanitize_step_plan_against_policy, scan_relative_imports, signals, step_plan_quality_report,
    step_plan_quality_warnings, ultra_plan_phase_signal_text, workspace_relative_handoff_path,
};

pub(super) const STEP_TURN_MAX_ITERATIONS: usize = 8;
pub(super) const STEP_REPAIR_MAX_ITERATIONS: usize = 6;
pub(super) const STEP_REPAIR_MAX_TURNS: usize = 4;
pub(super) const STEP_REPAIR_IDENTICAL_NO_CHANGE_LIMIT: usize = 2;
pub(super) const PLANNER_PROVIDER_REQUEST_ATTEMPTS: usize = 2;
pub(super) const PLANNER_PROVIDER_REQUEST_RETRY_DELAY: Duration = Duration::from_millis(80);
pub(super) const ULTRA_PLAN_GENERATION_ATTEMPTS: usize = 3;
pub(super) const NEXTJS_DEV_SERVER_DEFAULT_PORT: u16 = 3011;
pub(super) const NEXTJS_DEV_SERVER_READY_TIMEOUT: Duration = Duration::from_secs(8);
pub(super) const NEXTJS_DEV_SERVER_CONNECT_TIMEOUT: Duration = Duration::from_millis(500);
pub(super) const NEXTJS_DEV_SERVER_WAIT_INTERVAL: Duration = Duration::from_millis(250);
pub(super) const DEV_SERVER_CLEANUP_TERM_TIMEOUT: Duration = Duration::from_secs(5);
pub(super) const DEV_SERVER_CLEANUP_KILL_TIMEOUT: Duration = Duration::from_secs(1);
pub(super) const DEV_SERVER_LOG_EXCERPT_BYTES: usize = 24_000;
pub(super) const DEV_SERVER_ROUTE: &str = "/";
pub(super) const DEV_SERVER_LIFECYCLE_STAGES: [&str; 4] = ["start", "wait", "probe", "cleanup"];
pub(super) const PROFILE_REPAIR_FILE_EXCERPT_MAX_CHARS: usize = 2_400;
pub(super) const TEXT_ECHO_REPAIR_REQUIREMENT: &str = "token never rendered; render the input's content reactively (no manual rebuild) - the typed text must appear in the preview/list";
pub(super) const TEXT_ECHO_AFTER_RELOAD_REPAIR_REQUIREMENT: &str =
    "preview renders only after reload - make it reactive to input";
pub(super) const RESTART_PARTIAL_REPAIR_GUIDANCE: &str = "either expose an in-play restart control, or accept the partial classification (the restart exists but cannot be behaviorally verified by the generic probe)";
pub(super) const APP_BEHAVIOR_PROBE_FAILURE_KINDS: [&str; 15] = [
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
pub(super) enum PlannerSessionMode {
    Standard,
    CompactRetry,
    FreshCompact,
}

impl PlannerSessionMode {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::CompactRetry => "compact_retry",
            Self::FreshCompact => "fresh_compact",
        }
    }
}

pub(super) const GENERIC_INTERACTIVE_EVIDENCE_KEYS: [&str; 3] = [
    "user_input_handler_evidence",
    "stateful_update_evidence",
    "visible_interactive_surface_evidence",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RepairSessionMode {
    Appended,
    Compact,
}

impl RepairSessionMode {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Appended => "appended",
            Self::Compact => "compact",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EffectiveRequestedPort {
    pub(super) port: u16,
    pub(super) telemetry: String,
}

pub(super) fn effective_requested_port(
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
pub(super) struct RecoveryArtifactValidation {
    pub(super) prompt_exists: bool,
    pub(super) prompt_parse_ok: bool,
    pub(super) prompt_parse_error: Option<String>,
    pub(super) yaml_exists: bool,
    pub(super) yaml_parse_ok: bool,
    pub(super) yaml_parse_error: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct ReleaseRecoveryHandoffSummary {
    pub(super) recovery_handoff_kind: String,
    pub(super) acceptance_layer: String,
    pub(super) recovery_prompt_path: String,
    pub(super) recovery_ultra_plan_path: String,
    pub(super) suggested_recovery_command: String,
    pub(super) suggested_recovery_yaml_command: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct PlanAdherenceReport {
    pub(super) present: Vec<String>,
    pub(super) missing: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct DepthProfile {
    pub(super) route_bound_source_line_count: usize,
    pub(super) state_dimensions_count: usize,
    pub(super) data_anvil_action_kind_count: usize,
    pub(super) input_types_with_observed_state_change_count: usize,
    pub(super) summary: String,
}

impl ReleaseRecoveryHandoffSummary {
    pub(super) fn has_artifact(&self) -> bool {
        !self.recovery_prompt_path.is_empty() || !self.recovery_ultra_plan_path.is_empty()
    }
}

#[derive(Debug, Clone)]
pub(super) struct BoundCompletionContract {
    pub(super) contract: CompletionContract,
    pub(super) path: String,
    pub(super) fs_path: Option<PathBuf>,
    pub(super) generated: bool,
    pub(super) required: bool,
}

impl RecoveryArtifactValidation {
    pub(super) fn prompt_command_available(&self) -> bool {
        self.prompt_exists && self.prompt_parse_ok
    }

    pub(super) fn yaml_command_available(&self) -> bool {
        self.yaml_exists && self.yaml_parse_ok
    }

    pub(super) fn command_targets_valid(&self) -> bool {
        self.prompt_command_available() && self.yaml_command_available()
    }
}

pub(super) fn validate_recovery_artifacts(
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

pub(super) fn validate_recovery_prompt(path: &Path) -> Result<(), String> {
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

pub(crate) fn validate_recovery_yaml(path: &Path) -> Result<(), String> {
    crate::planner::recovery_validation::validate(path)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

pub(super) fn recovery_artifact_check_summary(validation: &RecoveryArtifactValidation) -> String {
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

pub(super) fn planner_chat_with_request_retry(
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

pub(super) fn planner_chat_for_step_plan_attempt(
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

pub(super) fn generate_step_plan_with_ui_for_phase(
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
                let python_cli_canonicalized =
                    crate::planner::python_cli_plan_synthesis::canonicalize_implementation_plan(
                        &mut plan,
                        &config.workspace_root,
                        &config.profile,
                        config.resolved_run_intent() == IntentId::Create,
                        config.eval_events_path.as_deref(),
                    );
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
                    || preset_converted > 0
                    || python_cli_canonicalized > 0;
                emit_planner_plan_sanitized(
                    config,
                    client.label(),
                    model,
                    attempt,
                    &sanitizer_report,
                );
                let mut lint_report = crate::planner::lint::lint_template_contract(
                    &plan,
                    Some(&config.workspace_root),
                );
                sanitizer_report.append_policy_errors(&mut lint_report);
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
                        if crate::planner::profile::community_quality_retry_is_terminal(
                            &config.profile,
                        ) {
                            anyhow::bail!("planner_quality_exhausted");
                        }
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

pub(super) fn deterministic_step_plan_for_phase(
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
    let mut lint_report =
        crate::planner::lint::lint_template_contract(&plan, Some(&config.workspace_root));
    sanitizer_report.append_policy_errors(&mut lint_report);
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

pub(super) fn emit_deterministic_step_plan_used(
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

pub(super) fn emit_step_plan_presentation(
    phase_label: Option<&str>,
    plan: &StepPlan,
    sanitizer_report: Option<&SanitizerReport>,
) {
    let phase = phase_label.unwrap_or(&plan.goal);
    crate::tui::presentation::emit_step_plan_block(phase, plan, sanitizer_report);
}

pub fn save_step_plan(root: &Path, plan: &StepPlan) -> anyhow::Result<PathBuf> {
    let dir = crate::runtime_paths::plans_dir(root);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("plan-{}.yaml", uuid::Uuid::now_v7()));
    std::fs::write(&path, crate::planner::plan::render_editable_step_plan(plan))?;
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
    let text = std::fs::read_to_string(&path).map_err(|error| {
        anyhow::anyhow!("failed to read plan file `{}`: {error}", path.display())
    })?;
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

pub(super) fn missing_final_artifacts(root: &Path, required_paths: &[String]) -> Vec<String> {
    required_paths
        .iter()
        .filter(|path| resolve_existing(root, path).is_err())
        .cloned()
        .collect()
}

#[derive(Debug, Clone)]
pub(super) struct ProfileInvariantFailureEvidence {
    pub(super) report: VerificationReport,
    pub(super) missing_paths: Vec<String>,
    pub(super) failure_evidence: Vec<String>,
}

pub(super) fn fresh_profile_invariant_failure_evidence(
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

pub(super) fn verify_invariant_with_hooks(
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

pub(super) fn profile_missing_relative_imports(root: &Path, profile: &str) -> Vec<MissingImport> {
    let paths = resolve_profile_runtime(profile).source_paths(root);
    if paths.is_empty() {
        return Vec::new();
    }
    scan_relative_imports(root, &paths).unwrap_or_default()
}

pub(super) fn merge_unique_strings(out: &mut Vec<String>, incoming: &[String]) {
    for item in incoming {
        if !out.contains(item) {
            out.push(item.clone());
        }
    }
}

pub(super) fn step_run_outcome_from_session_error(
    err: &anyhow::Error,
    stop_reason: &str,
) -> StepRunOutcome {
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

pub(super) fn apply_session_error_observations(
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

pub(super) fn exhaustion_reason_with_pending_contract_state(
    message: &str,
    pending_keys: &[String],
) -> String {
    if !is_exhaustion_message(message) {
        return message.to_string();
    }
    capability_evidence_unresolved_reason(pending_keys).unwrap_or_else(|| message.to_string())
}

pub(super) fn is_exhaustion_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("loop_progress_exhausted")
        || lower.contains("progress_exhausted")
        || lower.contains("iteration")
        || lower.contains("exhausted")
}

pub(super) fn capability_evidence_failure_evidence(
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

pub(super) fn restart_hook_attachment_guidance(root: &Path, profile: &str) -> Vec<String> {
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

pub(super) fn restart_hook_scan_candidate(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("tsx" | "jsx" | "ts" | "js")
    )
}

pub(super) fn restart_attachment_candidate_line(line: &str) -> bool {
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

pub(super) fn has_restart_action_attribute(line: &str) -> bool {
    line.contains("data-anvil-action=\"restart\"")
        || line.contains("data-anvil-action='restart'")
        || line.contains("data-anvil-action={`restart`}")
}

pub(super) fn restart_attachment_line_number(lines: &[&str], index: usize) -> usize {
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

pub(super) fn restart_attachment_block(lines: &[&str], index: usize) -> String {
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

pub(super) fn restart_block_is_initial_primary(block: &str) -> bool {
    let lower = block.to_ascii_lowercase();
    lower.contains("data-anvil-action=\"primary\"")
        || lower.contains("data-anvil-action='primary'")
        || lower.contains("start game")
        || lower.contains(">start<")
}

pub(super) fn restart_attachment_label(text: &str) -> &'static str {
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

pub(super) fn verification_missing_signals(report: &VerificationReport) -> Vec<String> {
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

pub(super) fn missing_signals_from_text(text: &str) -> Vec<String> {
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

pub(super) fn missing_signal_values_after_prefix(text: &str, prefix: &str) -> Vec<String> {
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

pub(super) fn handoff_path(path: &Path) -> String {
    workspace_relative_handoff_path(path)
}

pub(super) fn optional_handoff_path(path: Option<&PathBuf>) -> String {
    path.map(|path| handoff_path(path.as_path()))
        .unwrap_or_default()
}

pub(super) fn command_failure_summaries(report: &VerificationReport) -> Vec<String> {
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

pub(super) fn reachability_action_labels(reachability: &RepairReachability) -> Vec<&'static str> {
    reachability
        .viable_actions
        .iter()
        .map(|action| action.as_str())
        .collect()
}

pub(super) fn reachability_blocked_evidence(blocked_requirements: &[String]) -> Vec<String> {
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

pub(super) fn emit_repair_unreachable(
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

pub(super) fn verification_report_signature(report: &VerificationReport) -> Vec<String> {
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

pub(super) fn normalize_report_reason_for_signature(reason: &str) -> String {
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

pub(super) fn push_context_items_capped(
    out: &mut Vec<String>,
    incoming: &[String],
    cap: usize,
    truncated: &mut bool,
) {
    for item in incoming {
        push_context_unique_capped(out, item.clone(), cap, truncated);
    }
}

pub(super) fn push_context_unique_capped(
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

pub(super) fn append_context_list(lines: &mut Vec<String>, label: &str, values: &[String]) {
    if values.is_empty() {
        return;
    }
    lines.push(format!("- {label}:"));
    for value in values {
        lines.push(format!("  - {value}"));
    }
}

pub(super) fn pending_capability_context_items(keys: &[String]) -> Vec<String> {
    let mut out = keys.to_vec();
    for remedy in capability_evidence_remedy_lines(keys) {
        let line = format!("remedy: {remedy}");
        if !out.contains(&line) {
            out.push(line);
        }
    }
    out
}

pub(super) fn render_bounded_prompt_section(
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

pub(super) fn merge_changed_files(context: &mut RepairContext, incoming: &[String]) {
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

pub(super) fn emit_ultra_context_initialized(
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

pub(super) fn emit_ultra_phase_context_attached(
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

pub(super) fn emit_ultra_phase_context_updated(
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

pub(super) fn emit_planner_error_for_lint(
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

pub(super) fn emit_planner_error(
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

pub(super) fn emit_planner_raw_output_shape(
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

pub(super) fn emit_planner_schema_field_defaults(
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

pub(super) fn collect_step_verify_commands(plan: &StepPlan) -> Vec<String> {
    plan.steps
        .iter()
        .flat_map(|step| step.verify.iter().cloned())
        .collect()
}

pub(super) fn emit_planner_verify_command_normalized(
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

pub(super) fn emit_planner_plan_sanitized(
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
            "semantic_change_rejections": report.semantic_change_rejections.iter().map(|record| json!({
                "kind": &record.kind,
                "step_id": &record.step_id,
                "value": eval_events::body_snippet(&record.value),
                "reason": eval_events::body_snippet(&record.reason),
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

pub(super) fn emit_planner_fallback_plan(
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

pub(super) fn stable_command_list_hash(commands: &[String]) -> String {
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

pub(super) fn command_list_summary(commands: &[String]) -> String {
    eval_events::body_snippet(&commands.join(" | "))
}

pub(super) fn fallback_step_plan_for_setup_phase(goal: &str, config: &Config) -> Option<StepPlan> {
    let plan = resolve_profile_runtime(&config.profile)
        .fallback_setup_plan(&config.workspace_root, goal)?;
    crate::planner::lint::lint_template_contract(&plan, Some(&config.workspace_root))
        .is_pass()
        .then_some(plan)
}

pub(super) fn phase_id_and_task_text(goal: &str) -> Option<String> {
    let mut out = Vec::new();
    for line in goal.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("Phase id:") || trimmed.starts_with("Phase task:") {
            out.push(trimmed.to_string());
        }
    }
    (!out.is_empty()).then(|| out.join("\n"))
}

pub(super) fn compact_single_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(super) fn emit_ultra_plan_raw_output_shape(
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

pub(super) fn emit_ultra_plan_generation_attempt(
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

pub(super) fn emit_ultra_plan_generation_retry(
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

pub(super) fn emit_ultra_plan_generation_failed(
    config: &Config,
    provider: &str,
    model: &str,
    message: &str,
) {
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

pub(super) fn emit_ultra_plan_generation_succeeded(
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

pub(super) fn emit_ultra_plan_generation_tool_call_rejected(
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

pub(super) fn emit_ultra_plan_generation_metadata_normalized(
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

pub(super) fn emit_planner_quality_warnings(
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

pub(super) fn emit_planner_quality_issues(
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

pub(super) fn emit_planner_quality_retry(
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

pub(super) fn emit_planner_quality_retry_degraded(
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

pub(super) fn emit_planner_quality_retry_exhausted(
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
            "stop_class": "planner_quality_exhausted",
        }),
    );
}

pub(super) fn planner_stage_and_kind_for_lint(
    report: &PlanLintReport,
) -> (&'static str, &'static str) {
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

pub(super) fn build_schema_retry_prompt(goal: &str, error: &str, attempt: usize) -> String {
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

pub(super) fn schema_retry_issue_hints(error: &str) -> Vec<&'static str> {
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

pub(super) fn build_empty_step_plan_compact_prompt(goal: &str, attempt: usize) -> String {
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

pub(super) fn build_lint_retry_prompt(
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

pub(super) fn is_only_goal_length_lint(report: &PlanLintReport) -> bool {
    report.errors.len() == 1
        && report.errors[0].category == "contract"
        && report.errors[0].message == "StepPlan goal is too long"
}

pub(super) fn build_quality_retry_prompt(
    goal: &str,
    report: &PlanQualityReport,
    attempt: usize,
) -> String {
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

pub(super) fn lint_retry_hard_constraints(
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

pub(super) fn step_plan_messages(prompt: &str) -> Vec<crate::state::ConversationMessage> {
    vec![
        crate::state::ConversationMessage::system(plan_generation_system_prompt()),
        crate::state::ConversationMessage::user(prompt.to_string()),
    ]
}

pub(super) fn ultra_plan_generation_messages(
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

pub(super) fn ultra_plan_generation_system_prompt(
    profile: &str,
    style: &str,
    intent: &str,
) -> String {
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
    let phase_count_rule = if profile == crate::planner::profiles::community_mini_app::PROFILE_ID {
        "- Return one L2 phase by default; add phases only when a valid promotion procedure requires L3/L4, never more than 8.\n"
    } else {
        "- Return 2 to 6 phases for most tasks, never more than 8.\n"
    };
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
{phase_count_rule}\
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

pub(super) fn ultra_plan_generation_user_prompt(
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

pub(super) fn build_ultra_plan_schema_retry_prompt(
    goal: &str,
    profile: &str,
    error: &str,
    attempt: usize,
) -> String {
    let phase_count_rule = if profile == crate::planner::profiles::community_mini_app::PROFILE_ID {
        "- Include top-level goal and 1-8 phases; use one L2 phase by default."
    } else {
        "- Include top-level goal and 2-8 phases."
    };
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
{phase_count_rule}\n\
- Each phase must have id and prompt.\n\
- Phase prompts must be natural-language tasks, not shell commands.\n\n\
Goal: {goal}"
    )
}

pub(super) fn build_ultra_plan_lint_retry_prompt(
    goal: &str,
    profile: &str,
    report: &PlanLintReport,
    attempt: usize,
) -> String {
    let phase_count_rule = if profile == crate::planner::profiles::community_mini_app::PROFILE_ID {
        "- Keep 1-8 phases; use one L2 phase by default."
    } else {
        "- Keep 2-8 phases."
    };
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
{phase_count_rule}\n\
- Use unique kebab-case phase ids.\n\
- Phase prompts must be natural-language /plan-run goals, not shell commands or REPL commands.\n\
- Keep concrete outcomes and verification expectations in phase prompts.\n\
- Preserve any Required final artifacts from the user goal.\n\n\
Goal: {goal}"
    )
}

pub(super) fn build_ultra_plan_tool_call_retry_prompt(goal: &str, attempt: usize) -> String {
    format!(
        "Your previous UltraPlan generation attempted to emit tool calls on attempt {attempt}/{ULTRA_PLAN_GENERATION_ATTEMPTS}.\n\
Do not call tools. Return corrected UltraPlan YAML only.\n\
Use natural-language phase prompts for later /plan-run execution.\n\n\
Goal: {goal}"
    )
}

pub(super) fn normalize_ultra_plan_metadata(
    plan: &mut UltraPlan,
    goal: &str,
    config: &Config,
) -> Vec<String> {
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

pub(super) fn tool_call_names(tool_calls: &[crate::state::ToolCall]) -> String {
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

pub(super) fn plan_generation_system_prompt() -> String {
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

pub(super) fn build_step_plan_user_prompt(goal: &str, config: &Config) -> String {
    match config.prompt_layout {
        PromptLayout::Stable => build_step_plan_user_prompt_stable(goal, config),
        PromptLayout::Legacy => build_step_plan_user_prompt_legacy(goal, config),
    }
}

pub(super) fn build_step_plan_user_prompt_stable(goal: &str, config: &Config) -> String {
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

pub(super) fn build_step_plan_user_prompt_legacy(goal: &str, config: &Config) -> String {
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

pub(super) fn preprovisioned_scaffold_note(
    root: &Path,
    runtime: &dyn ProfileRuntime,
) -> Option<String> {
    let scaffold_paths = runtime.setup_scaffold_paths(root);
    (!scaffold_paths.is_empty()).then(|| {
        "Required scaffold files are authored before phase 1 when absent; verify or extend the scaffold rather than re-planning file creation.".to_string()
    })
}

pub(super) fn is_ultra_phase_step_goal(goal: &str) -> bool {
    goal.contains("Original ultra goal:")
        && goal.contains("Phase id:")
        && goal.contains("Phase task:")
}

pub(super) fn plan_quality_context(config: &Config, goal: &str) -> PlanQualityContext {
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
pub(super) struct WorkspaceQualitySnapshot {
    pub(super) context_known: bool,
    pub(super) snapshot_class: String,
    pub(super) has_user_seed_files: bool,
    pub(super) has_only_agent_metadata: bool,
}

pub(super) fn workspace_quality_snapshot(root: &Path) -> WorkspaceQualitySnapshot {
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

pub(super) fn is_agent_metadata_entry(name: &str) -> bool {
    matches!(
        name,
        ".git" | ".commandagent" | ".anvil" | ".codex" | ".agents" | "target" | ".DS_Store"
    ) || name.starts_with("commandagent-eval-")
}

pub(super) fn strengthen_step_plan_for_profile(plan: &mut StepPlan, config: &Config) {
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

pub(super) fn build_step_prompt(
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

pub(super) fn build_step_prompt_stable(
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

pub(super) fn build_step_prompt_legacy(
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

pub(super) fn append_bullets_or_none(prompt: &mut String, items: &[String]) {
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

pub(super) fn step_expected_result(step: &PlanStep) -> &str {
    let trimmed = step.expected_result.trim();
    if trimmed.is_empty() { "pass" } else { trimmed }
}

pub(super) fn emit_step_prompt_contract(
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
            "prompt_body_saved": crate::run_trace::enabled(),
        }),
    );
}

#[allow(dead_code)]
pub(super) fn prompt_with_required_paths(instruction: &str, paths: &[String]) -> String {
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

pub(super) fn ultra_phase_prompt(
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

pub(super) fn ultra_phase_prompt_stable(
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

pub(super) fn ultra_phase_prompt_legacy(
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

pub(super) fn phase_goal_one_liner(prompt: &str) -> String {
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
pub(super) fn _format_report(report: &VerificationReport) -> String {
    format!("{:?}", report.status)
}

pub(super) fn resolve_plan_file_path(root: &Path, path: &Path) -> anyhow::Result<PathBuf> {
    let root = root.canonicalize()?;
    let canonical = if path.is_absolute() {
        path.canonicalize().map_err(|error| {
            anyhow::anyhow!("failed to resolve plan file `{}`: {error}", path.display())
        })?
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
