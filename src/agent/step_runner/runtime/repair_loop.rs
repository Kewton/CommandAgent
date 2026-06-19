use super::SlashRuntime;
use super::execution;
use super::paths::{changed_file_markers, missing_paths, result_changed_files};
use super::setup::{
    DependencySetupDisposition, DependencySetupRunner, SetupRunStatus, ShellDependencySetupRunner,
    dependency_missing_blocker_message, dependency_setup_disposition, setup_failed_blocker_message,
};
use crate::agent::events::{RuntimeEvent, RuntimeObserver, bounded_event_text};
use crate::agent::minimal_loop::config::{ActionRequirement, StepToolPolicy};
use crate::agent::minimal_loop::loop_run::{ChatClient, MinimalLoopConfig, RunResult};
use crate::agent::minimal_loop::result::{MinimalLoopError, ToolArgError};
use crate::agent::step_runner::correction_evidence::{ContractEvidence, failure_signature};
use crate::agent::step_runner::recovery_task::{
    RecoveryExecutionEnvelope, recovery_execution_envelope,
};
use crate::agent::step_runner::repair::{
    RepairBudget, RepairContext, ToolProtocolCorrectionContext, build_repair_prompt,
    build_tool_protocol_correction_prompt, save_repair_prompt,
};
use crate::agent::step_runner::runtime::phase_contract::{
    ActiveStepContract, current_profile_facts,
};
use crate::agent::step_runner::verify::VerificationFailure;
use crate::agent::step_runner::{StepKind, StepPlan, StepPlanStep};
use std::time::Instant;

const MAX_REPAIR_TURNS: usize = 3;

pub(super) struct RepairStepState {
    pub(super) failures: Vec<VerificationFailure>,
    pub(super) changed_files: Vec<String>,
    pub(super) file_changing_attempts: usize,
    pub(super) initial_turn_error: Option<String>,
    pub(super) dependency_setup_attempted: bool,
    pub(super) dependency_setup_note: Option<String>,
    pub(super) contract_evidence: Vec<ContractEvidence>,
    pub(super) repair_attempt_ledger: Vec<String>,
    pub(super) tool_arg_schema_correction_spent: bool,
    pub(super) pending_tool_arg_error: Option<ToolArgError>,
    pub(super) pending_tool_arg_error_source: Option<ToolProtocolCorrectionSource>,
}

pub(super) struct RepairStepRequest<'a> {
    pub(super) plan: &'a StepPlan,
    pub(super) step: &'a StepPlanStep,
    pub(super) config: MinimalLoopConfig,
    pub(super) contract_seed: &'a ActiveStepContract,
}

pub(super) fn turn_error_failure(command: &str, error: &MinimalLoopError) -> VerificationFailure {
    let (reason, diagnostic_excerpt) = turn_error_reason_and_diagnostic(error);
    VerificationFailure {
        command: command.to_string(),
        reason: reason.to_string(),
        stdout_excerpt: String::new(),
        stderr_excerpt: String::new(),
        diagnostic_excerpt,
        source_excerpt: None,
    }
}

fn turn_error_reason_and_diagnostic(error: &MinimalLoopError) -> (&'static str, String) {
    if let MinimalLoopError::ToolArgs(arg_error) = error {
        return (
            arg_error.reason_code(),
            format!(
                "{}\nOriginal error: {}",
                arg_error.diagnostic_excerpt(),
                error
            ),
        );
    }
    let rendered = error.to_string();
    if is_edit_target_not_found(&rendered) {
        return (
            "edit_target_not_found",
            format!(
                "Edit target was not found. The file state is stale for this Edit attempt. Read or Glob the current file first, then use Edit only with exact current target text from this repair turn, or Write when full replacement/creation is safer.\nOriginal error: {rendered}"
            ),
        );
    }
    ("turn_error", rendered)
}

pub(super) fn recoverable_repair_turn_error(error: &MinimalLoopError) -> bool {
    let rendered = error.to_string();
    matches!(
        error,
        MinimalLoopError::FinalAnswerContract(_)
            | MinimalLoopError::ActionRequiredNoEvidence(_)
            | MinimalLoopError::MissingArtifacts(_)
    ) || is_edit_target_not_found(&rendered)
}

fn is_tool_arg_schema_failure(error: &MinimalLoopError) -> bool {
    matches!(error, MinimalLoopError::ToolArgs(_))
}

fn tool_arg_error(error: &MinimalLoopError) -> Option<ToolArgError> {
    match error {
        MinimalLoopError::ToolArgs(arg_error) => Some(arg_error.clone()),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ToolProtocolCorrectionDecision {
    CorrectOnce(ToolProtocolCorrectionContext),
    Terminal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ToolProtocolCorrectionSource {
    InitialTurn,
    RepairTurn,
}

fn tool_protocol_correction_decision(
    step: &StepPlanStep,
    arg_error: Option<&ToolArgError>,
    target_path: Option<String>,
    correction_spent: bool,
    source: Option<ToolProtocolCorrectionSource>,
) -> ToolProtocolCorrectionDecision {
    if correction_spent || !step_allows_tool_protocol_correction(step.kind, source) {
        return ToolProtocolCorrectionDecision::Terminal;
    }
    let Some(arg_error) = arg_error else {
        return ToolProtocolCorrectionDecision::Terminal;
    };
    ToolProtocolCorrectionDecision::CorrectOnce(ToolProtocolCorrectionContext {
        tool: arg_error.tool_name().to_string(),
        reason_code: arg_error.reason_code().to_string(),
        missing_field: arg_error.missing_field().map(str::to_string),
        required_fields: arg_error.required_fields().to_vec(),
        target_path,
        diagnostic: arg_error.diagnostic_excerpt(),
    })
}

fn tool_protocol_correction_target_path(
    step: &StepPlanStep,
    missing_expected_paths: &[String],
) -> Option<String> {
    missing_expected_paths
        .first()
        .cloned()
        .or_else(|| match step.expected_paths.as_slice() {
            [single] => Some(single.clone()),
            _ => None,
        })
}

fn step_allows_tool_protocol_correction(
    kind: StepKind,
    source: Option<ToolProtocolCorrectionSource>,
) -> bool {
    match source {
        Some(ToolProtocolCorrectionSource::InitialTurn) => matches!(
            kind,
            StepKind::Create | StepKind::Edit | StepKind::Setup | StepKind::Repair
        ),
        Some(ToolProtocolCorrectionSource::RepairTurn) => matches!(
            kind,
            StepKind::Create
                | StepKind::Edit
                | StepKind::Setup
                | StepKind::Repair
                | StepKind::Verify
        ),
        None => false,
    }
}

fn is_edit_target_not_found(error: &str) -> bool {
    error.contains("edit target was not found")
}

pub(super) fn should_send_missing_artifact_no_tool_guard(
    error: &str,
    missing_expected_paths: &[String],
    already_sent: bool,
) -> bool {
    !already_sent
        && !missing_expected_paths.is_empty()
        && (error.contains("assistant violated final answer contract")
            || error.contains("missing expected artifacts"))
}

pub(super) fn missing_artifact_no_tool_guard_failure(
    missing_expected_paths: &[String],
) -> VerificationFailure {
    VerificationFailure {
        command: "repair missing-artifact guard".to_string(),
        reason: "missing_artifact_no_tool".to_string(),
        stdout_excerpt: String::new(),
        stderr_excerpt: String::new(),
        diagnostic_excerpt: format!(
            "The required path is still missing: {}. Do not describe the next action. Call Read, Glob, Write, or Edit in this response. If creating the missing file is required, call Write now.",
            missing_expected_paths.join(", ")
        ),
        source_excerpt: None,
    }
}

pub(super) fn repair_step_after_turn_error<E, P>(
    runtime: &mut SlashRuntime<'_, E, P>,
    request: RepairStepRequest<'_>,
    turn_error: MinimalLoopError,
    failures: Vec<VerificationFailure>,
    observer: &mut dyn RuntimeObserver,
) -> Result<(), String>
where
    E: ChatClient,
    P: ChatClient,
{
    let mut failures = failures;
    let turn_error_text = turn_error.to_string();
    let pending_tool_arg_error = tool_arg_error(&turn_error);
    let pending_tool_arg_error_source = pending_tool_arg_error
        .as_ref()
        .map(|_| ToolProtocolCorrectionSource::InitialTurn);
    let mut contract_evidence = Vec::new();
    if let Some(evidence) = read_only_mutation_contract_evidence(request.step, &turn_error) {
        push_contract_evidence_once(&mut contract_evidence, evidence);
    }
    failures.insert(0, turn_error_failure("initial turn", &turn_error));
    repair_step_with_state(
        runtime,
        request,
        RepairStepState {
            failures,
            changed_files: Vec::new(),
            file_changing_attempts: 0,
            initial_turn_error: Some(turn_error_text),
            dependency_setup_attempted: false,
            dependency_setup_note: None,
            contract_evidence,
            repair_attempt_ledger: Vec::new(),
            tool_arg_schema_correction_spent: false,
            pending_tool_arg_error,
            pending_tool_arg_error_source,
        },
        observer,
    )
}

pub(super) fn repair_step<E, P>(
    runtime: &mut SlashRuntime<'_, E, P>,
    request: RepairStepRequest<'_>,
    first_result: RunResult,
    failures: Vec<VerificationFailure>,
    observer: &mut dyn RuntimeObserver,
) -> Result<(), String>
where
    E: ChatClient,
    P: ChatClient,
{
    repair_step_with_state(
        runtime,
        request,
        RepairStepState {
            failures,
            changed_files: changed_file_markers(&first_result),
            file_changing_attempts: usize::from(result_changed_files(&first_result)),
            initial_turn_error: None,
            dependency_setup_attempted: false,
            dependency_setup_note: None,
            contract_evidence: Vec::new(),
            repair_attempt_ledger: Vec::new(),
            tool_arg_schema_correction_spent: false,
            pending_tool_arg_error: None,
            pending_tool_arg_error_source: None,
        },
        observer,
    )
}

pub(super) fn repair_step_with_state<E, P>(
    runtime: &mut SlashRuntime<'_, E, P>,
    request: RepairStepRequest<'_>,
    mut state: RepairStepState,
    observer: &mut dyn RuntimeObserver,
) -> Result<(), String>
where
    E: ChatClient,
    P: ChatClient,
{
    let plan = request.plan;
    let step = request.step;
    let config = request.config;
    let contract_seed = request.contract_seed;
    let budget = RepairBudget::default();
    let mut repair_turns = 0usize;
    let mut missing_artifact_no_tool_guard_sent = false;
    let initial_missing_expected_paths = missing_paths(runtime.cwd, &step.expected_paths);
    if let Some(arg_error) = state.pending_tool_arg_error.clone() {
        push_tool_arg_contract_evidence(
            &mut state,
            step,
            &arg_error,
            tool_protocol_correction_target_path(step, &initial_missing_expected_paths),
        );
    }
    match try_dependency_setup_recovery(
        runtime,
        step,
        &config,
        &mut state,
        &initial_missing_expected_paths,
        observer,
    )? {
        DependencyRecoveryResult::Recovered => return Ok(()),
        DependencyRecoveryResult::Blocked(message) => return Err(message),
        DependencyRecoveryResult::ContinueRepair | DependencyRecoveryResult::NotApplicable => {}
    }

    while budget.allows_next_attempt(state.file_changing_attempts)
        && repair_turns < MAX_REPAIR_TURNS
    {
        repair_turns += 1;
        let missing_expected_paths = missing_paths(runtime.cwd, &step.expected_paths);
        observer.on_event(RuntimeEvent::RepairAttemptStarted {
            step_id: bounded_event_text(&step.id),
            attempt: repair_turns,
            max_attempts: MAX_REPAIR_TURNS,
            missing_expected_paths: missing_expected_paths.clone(),
        });
        let current_contract_evidence = contract_evidence_for_state(step, &state);
        let selected_envelope = recovery_execution_envelope(&current_contract_evidence);
        let correction_decision = tool_protocol_correction_decision(
            step,
            state.pending_tool_arg_error.as_ref(),
            tool_protocol_correction_target_path(step, &missing_expected_paths),
            state.tool_arg_schema_correction_spent,
            state.pending_tool_arg_error_source,
        );
        let prompt = match correction_decision {
            ToolProtocolCorrectionDecision::CorrectOnce(context) => {
                state.tool_arg_schema_correction_spent = true;
                state.pending_tool_arg_error = None;
                state.pending_tool_arg_error_source = None;
                build_tool_protocol_correction_prompt(&context)
            }
            ToolProtocolCorrectionDecision::Terminal if state.pending_tool_arg_error.is_some() => {
                break;
            }
            ToolProtocolCorrectionDecision::Terminal => {
                let context = RepairContext {
                    step_id: step.id.clone(),
                    original_goal: plan.goal.clone(),
                    profile: plan.profile.clone(),
                    style: plan.style.clone(),
                    step_instruction: step.instruction.clone(),
                    active_profile_contract_facts: active_contract_facts(
                        runtime,
                        plan,
                        contract_seed,
                    ),
                    contract_evidence: current_contract_evidence.clone(),
                    verification_failures: state.failures.clone(),
                    missing_expected_paths: missing_expected_paths.clone(),
                    changed_files: state.changed_files.clone(),
                };
                build_repair_prompt(&context)
            }
        };
        let mut repair_config = config.clone();
        apply_repair_execution_envelope(&mut repair_config, selected_envelope);
        let result = match crate::agent::minimal_loop::loop_run::run_session_with_observer(
            runtime.executor,
            runtime.cwd,
            &prompt,
            repair_config,
            observer,
        ) {
            Ok(result) => result,
            Err(err) => {
                let error = err.to_string();
                let is_schema_failure = is_tool_arg_schema_failure(&err);
                if let Some(arg_error) = tool_arg_error(&err) {
                    push_tool_arg_contract_evidence(
                        &mut state,
                        step,
                        &arg_error,
                        tool_protocol_correction_target_path(step, &missing_expected_paths),
                    );
                }
                if let Some(evidence) = read_only_mutation_contract_evidence(step, &err) {
                    push_contract_evidence_once(&mut state.contract_evidence, evidence);
                }
                let mut failure = turn_error_failure("repair turn", &err);
                if is_schema_failure && state.tool_arg_schema_correction_spent {
                    failure.diagnostic_excerpt = format!(
                        "Tool protocol correction was attempted once for this step, but the next tool call still violated the schema.\n{}",
                        failure.diagnostic_excerpt
                    );
                }
                state.failures.push(failure);
                state.pending_tool_arg_error = tool_arg_error(&err);
                state.pending_tool_arg_error_source = state
                    .pending_tool_arg_error
                    .as_ref()
                    .map(|_| ToolProtocolCorrectionSource::RepairTurn);
                if should_send_missing_artifact_no_tool_guard(
                    &error,
                    &missing_expected_paths,
                    missing_artifact_no_tool_guard_sent,
                ) {
                    missing_artifact_no_tool_guard_sent = true;
                    state.failures.push(missing_artifact_no_tool_guard_failure(
                        &missing_expected_paths,
                    ));
                }
                if is_schema_failure {
                    if !state.tool_arg_schema_correction_spent
                        && budget.allows_next_attempt(state.file_changing_attempts)
                        && repair_turns < MAX_REPAIR_TURNS
                    {
                        continue;
                    }
                    break;
                }
                state.pending_tool_arg_error = None;
                state.pending_tool_arg_error_source = None;
                if recoverable_repair_turn_error(&err)
                    && budget.allows_next_attempt(state.file_changing_attempts)
                    && repair_turns < MAX_REPAIR_TURNS
                {
                    continue;
                }
                break;
            }
        };
        state.pending_tool_arg_error = None;
        state.pending_tool_arg_error_source = None;
        if result_changed_files(&result) {
            state.file_changing_attempts += 1;
        }
        state.changed_files.extend(changed_file_markers(&result));
        state.failures = execution::verify_step_with_observer(runtime.cwd, step, observer)?;
        if state.failures.is_empty() {
            return Ok(());
        }
        record_repair_attempt_ledger(&mut state, step, repair_turns);
        let missing_expected_paths = missing_paths(runtime.cwd, &step.expected_paths);
        match try_dependency_setup_recovery(
            runtime,
            step,
            &config,
            &mut state,
            &missing_expected_paths,
            observer,
        )? {
            DependencyRecoveryResult::Recovered => return Ok(()),
            DependencyRecoveryResult::Blocked(message) => return Err(message),
            DependencyRecoveryResult::ContinueRepair | DependencyRecoveryResult::NotApplicable => {}
        }
    }

    let missing_expected_paths = missing_paths(runtime.cwd, &step.expected_paths);
    match try_dependency_setup_recovery(
        runtime,
        step,
        &config,
        &mut state,
        &missing_expected_paths,
        observer,
    )? {
        DependencyRecoveryResult::Recovered => return Ok(()),
        DependencyRecoveryResult::Blocked(message) => return Err(message),
        DependencyRecoveryResult::ContinueRepair | DependencyRecoveryResult::NotApplicable => {}
    }
    let contract_evidence = contract_evidence_for_state(step, &state);
    let context = RepairContext {
        step_id: step.id.clone(),
        original_goal: plan.goal.clone(),
        profile: plan.profile.clone(),
        style: plan.style.clone(),
        step_instruction: step.instruction.clone(),
        active_profile_contract_facts: active_contract_facts(runtime, plan, contract_seed),
        contract_evidence,
        verification_failures: state.failures,
        missing_expected_paths,
        changed_files: state.changed_files,
    };
    let saved = save_repair_prompt(runtime.cwd, &context).map_err(|err| err.to_string())?;
    observer.on_event(RuntimeEvent::RepairExhausted {
        step_id: bounded_event_text(&step.id),
        repair_path: saved.relative_path.clone(),
        suggested_command: saved.suggested_command.clone(),
        missing_expected_paths: context.missing_expected_paths.clone(),
    });
    let initial = state
        .initial_turn_error
        .map(|err| format!("initial turn error: {err}\n"))
        .unwrap_or_default();
    Err(format!(
        "{initial}step {} failed verification; repair prompt saved: {}\nsuggested command: {}",
        step.id, saved.relative_path, saved.suggested_command
    ))
}

fn apply_repair_execution_envelope(
    config: &mut MinimalLoopConfig,
    envelope: Option<RecoveryExecutionEnvelope>,
) {
    match envelope {
        Some(RecoveryExecutionEnvelope::ReadOnlyEvidence) => {
            config.action_requirement = ActionRequirement::RepositoryEvidenceRequired;
            config.step_tool_policy = StepToolPolicy::ReadOnly;
        }
        Some(
            RecoveryExecutionEnvelope::FileMutationRepair
            | RecoveryExecutionEnvelope::ToolProtocolCorrection,
        )
        | None => {
            config.action_requirement = ActionRequirement::Required;
            config.step_tool_policy = StepToolPolicy::FileMutationAllowed;
        }
    }
}

fn push_tool_arg_contract_evidence(
    state: &mut RepairStepState,
    step: &StepPlanStep,
    arg_error: &ToolArgError,
    target_path: Option<String>,
) {
    let required_fields = if arg_error.required_fields().is_empty() {
        "the required fields".to_string()
    } else {
        arg_error.required_fields().join(", ")
    };
    let mut evidence = ContractEvidence::new("tool_protocol")
        .with_failed_step(step.id.clone())
        .with_violated_contract(arg_error.reason_code())
        .with_reason_code(arg_error.reason_code())
        .with_tool(arg_error.tool_name())
        .with_required_action(format!(
            "emit exactly one valid {} tool call with required fields: {required_fields}",
            arg_error.tool_name()
        ))
        .with_diagnostic(arg_error.diagnostic_excerpt());
    if let Some(field) = arg_error.missing_field() {
        evidence = evidence.with_target_field(field);
    }
    if !arg_error.required_fields().is_empty() {
        evidence = evidence.with_required_fields(arg_error.required_fields().iter().cloned());
    }
    if let Some(path) = target_path {
        evidence = evidence.with_target_path(path);
    }
    push_contract_evidence_once(&mut state.contract_evidence, evidence);
}

fn read_only_mutation_contract_evidence(
    step: &StepPlanStep,
    error: &MinimalLoopError,
) -> Option<ContractEvidence> {
    let MinimalLoopError::Tool(message) = error else {
        return None;
    };
    let tool = read_only_policy_tool(message)?;
    Some(
        ContractEvidence::new("step_policy")
            .with_failed_step(step.id.clone())
            .with_violated_contract("read_only_step_mutation")
            .with_reason_code("read_only_step_mutation")
            .with_tool(tool)
            .with_required_action(
                "use only read-only tools in inspect/report steps; move mutation into create/edit/repair steps",
            )
            .with_diagnostic(message.clone()),
    )
}

fn read_only_policy_tool(message: &str) -> Option<String> {
    let detail = message
        .strip_prefix("tool_policy_violation: ")
        .unwrap_or(message);
    if detail.contains("not allowed in a read-only step") {
        return detail.split_whitespace().next().map(str::to_string);
    }
    if detail.starts_with("Bash command is not read-only for this step") {
        return Some("Bash".to_string());
    }
    None
}

fn contract_evidence_for_state(
    step: &StepPlanStep,
    state: &RepairStepState,
) -> Vec<ContractEvidence> {
    let mut evidence = state.contract_evidence.clone();
    for failure in &state.failures {
        if let Some(verifier_evidence) = verifier_contract_evidence(
            step,
            failure,
            state.dependency_setup_note.as_deref(),
            &state.repair_attempt_ledger,
        ) {
            push_contract_evidence_once(&mut evidence, verifier_evidence);
        }
    }
    evidence
}

fn verifier_contract_evidence(
    step: &StepPlanStep,
    failure: &VerificationFailure,
    dependency_setup_note: Option<&str>,
    repair_attempt_ledger: &[String],
) -> Option<ContractEvidence> {
    if !is_verifier_failure(failure) {
        return None;
    }
    let candidate_artifacts = verifier_candidate_artifacts(failure);
    let repair_target = single_candidate(&candidate_artifacts);
    let diagnostic_code = failure.reason.clone();
    let signature = failure_signature([
        "verifier",
        step.id.as_str(),
        failure.command.as_str(),
        diagnostic_code.as_str(),
        repair_target.as_deref().unwrap_or(""),
    ]);
    let mut evidence = ContractEvidence::new("verifier")
        .with_failed_step(step.id.clone())
        .with_violated_contract(failure.reason.clone())
        .with_reason_code(failure.reason.clone())
        .with_failure_kind(verifier_failure_kind(failure).to_string())
        .with_diagnostic_code(diagnostic_code)
        .with_failure_signature(signature)
        .with_command(failure.command.clone())
        .with_candidate_artifacts(candidate_artifacts)
        .with_required_action("fix the reported verifier failure before adding feature work");
    if let Some(target) = repair_target {
        evidence = evidence
            .with_target_path(target.clone())
            .with_repair_target(target)
            .with_repair_focus(
                "fix the verifier error in the repair target before adding feature work",
            );
    } else {
        evidence = evidence.with_repair_focus(
            "fix the reported verifier failure before expanding implementation scope",
        );
    }
    if let Some(source) = &failure.source_excerpt
        && !ignored_repair_candidate_path(&source.path)
    {
        evidence = evidence.with_related_source_excerpt(format!(
            "{}:{}\n{}",
            source.path, source.line, source.excerpt
        ));
    }
    if !repair_attempt_ledger.is_empty() {
        evidence = evidence
            .with_prior_attempts(repair_attempt_ledger.iter().cloned())
            .with_repair_attempt_ledger(repair_attempt_ledger.iter().cloned());
    }
    let diagnostic = verifier_diagnostic(failure, dependency_setup_note);
    if !diagnostic.trim().is_empty() {
        evidence = evidence.with_diagnostic(diagnostic);
    }
    Some(evidence)
}

fn verifier_failure_kind(failure: &VerificationFailure) -> &'static str {
    if failure.reason == "dependency_missing" {
        "dependency_missing"
    } else if failure.reason.starts_with("blocked:") {
        "verifier_command_blocked"
    } else if failure.reason.starts_with("command_failed:") {
        "verifier_command_failed"
    } else {
        "verifier_failure"
    }
}

fn verifier_candidate_artifacts(failure: &VerificationFailure) -> Vec<String> {
    let mut artifacts = Vec::new();
    if let Some(source) = &failure.source_excerpt {
        push_repairable_candidate(&mut artifacts, source.path.clone());
    }
    if tailwind_postcss_plugin_diagnostic(failure) {
        push_repairable_candidate(&mut artifacts, "package.json".to_string());
        push_repairable_candidate(&mut artifacts, "postcss.config.js".to_string());
    }
    for text in [
        failure.diagnostic_excerpt.as_str(),
        failure.stderr_excerpt.as_str(),
        failure.stdout_excerpt.as_str(),
    ] {
        for candidate in source_like_paths(text) {
            push_unique_value(&mut artifacts, candidate);
        }
    }
    artifacts.truncate(8);
    artifacts
}

fn tailwind_postcss_plugin_diagnostic(failure: &VerificationFailure) -> bool {
    let combined = format!(
        "{}\n{}\n{}",
        failure.diagnostic_excerpt, failure.stderr_excerpt, failure.stdout_excerpt
    )
    .to_ascii_lowercase();
    combined.contains("tailwindcss")
        && combined.contains("postcss")
        && combined.contains("@tailwindcss/postcss")
}

fn source_like_paths(text: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for raw in text.split_whitespace() {
        let value = raw
            .trim_matches(|ch: char| {
                matches!(
                    ch,
                    '.' | ',' | ';' | ':' | '(' | ')' | '[' | ']' | '{' | '}' | '\'' | '"' | '`'
                )
            })
            .trim_start_matches("./")
            .to_string();
        if is_source_like_path(&value) {
            push_repairable_candidate(&mut paths, value);
        }
    }
    paths
}

fn is_source_like_path(value: &str) -> bool {
    !ignored_repair_candidate_path(value)
        && value.contains('/')
        && matches!(
            value.rsplit('.').next(),
            Some("ts" | "tsx" | "js" | "jsx" | "rs" | "py")
        )
}

fn ignored_repair_candidate_path(value: &str) -> bool {
    value.starts_with('/')
        || value.starts_with("node_modules/")
        || value.contains("/node_modules/")
        || value.starts_with(".next/")
        || value.contains("/.next/")
}

fn single_candidate(values: &[String]) -> Option<String> {
    match values {
        [single] => Some(single.clone()),
        _ => None,
    }
}

fn record_repair_attempt_ledger(state: &mut RepairStepState, step: &StepPlanStep, attempt: usize) {
    let failures = state.failures.clone();
    for failure in failures {
        if let Some(evidence) =
            verifier_contract_evidence(step, &failure, state.dependency_setup_note.as_deref(), &[])
            && let Some(signature) = evidence.failure_signature
        {
            push_repair_attempt_ledger(
                &mut state.repair_attempt_ledger,
                format!("attempt {attempt}: {signature}"),
            );
        }
    }
}

fn push_repair_attempt_ledger(ledger: &mut Vec<String>, entry: String) {
    if ledger.last().is_some_and(|last| last == &entry) {
        return;
    }
    ledger.push(entry);
    const MAX_LEDGER_ENTRIES: usize = 8;
    if ledger.len() > MAX_LEDGER_ENTRIES {
        let drop_count = ledger.len() - MAX_LEDGER_ENTRIES;
        ledger.drain(0..drop_count);
    }
}

fn push_unique_value(values: &mut Vec<String>, value: String) {
    if !value.trim().is_empty() && !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn push_repairable_candidate(values: &mut Vec<String>, value: String) {
    if !ignored_repair_candidate_path(&value) {
        push_unique_value(values, value);
    }
}

fn is_verifier_failure(failure: &VerificationFailure) -> bool {
    !failure.command.trim().is_empty()
        && !matches!(
            failure.command.as_str(),
            "initial turn" | "repair turn" | "repair missing-artifact guard"
        )
}

fn verifier_diagnostic(
    failure: &VerificationFailure,
    dependency_setup_note: Option<&str>,
) -> String {
    let mut parts = Vec::new();
    push_non_empty(&mut parts, &failure.diagnostic_excerpt);
    push_non_empty(&mut parts, &failure.stderr_excerpt);
    push_non_empty(&mut parts, &failure.stdout_excerpt);
    if let Some(source) = &failure.source_excerpt {
        parts.push(format!(
            "source_excerpt={}:\n{}",
            source.path, source.excerpt
        ));
    }
    if let Some(note) = dependency_setup_note {
        push_non_empty(&mut parts, note);
    }
    parts.join("\n")
}

fn push_non_empty(values: &mut Vec<String>, value: &str) {
    if !value.trim().is_empty() {
        values.push(value.to_string());
    }
}

fn push_contract_evidence_once(evidence: &mut Vec<ContractEvidence>, candidate: ContractEvidence) {
    if !evidence.contains(&candidate) {
        evidence.push(candidate);
    }
}

fn active_contract_facts<E, P>(
    runtime: &SlashRuntime<'_, E, P>,
    plan: &StepPlan,
    contract_seed: &ActiveStepContract,
) -> Vec<String>
where
    E: ChatClient,
    P: ChatClient,
{
    contract_seed
        .with_current_profile_facts(current_profile_facts(&plan.profile, runtime.cwd))
        .rendered_lines()
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DependencyRecoveryResult {
    NotApplicable,
    Recovered,
    ContinueRepair,
    Blocked(String),
}

fn try_dependency_setup_recovery<E, P>(
    runtime: &mut SlashRuntime<'_, E, P>,
    step: &StepPlanStep,
    config: &MinimalLoopConfig,
    state: &mut RepairStepState,
    missing_expected_paths: &[String],
    observer: &mut dyn RuntimeObserver,
) -> Result<DependencyRecoveryResult, String>
where
    E: ChatClient,
    P: ChatClient,
{
    try_dependency_setup_recovery_with_runner(
        runtime,
        step,
        config,
        state,
        missing_expected_paths,
        observer,
        &ShellDependencySetupRunner,
    )
}

fn try_dependency_setup_recovery_with_runner<E, P, R>(
    runtime: &mut SlashRuntime<'_, E, P>,
    step: &StepPlanStep,
    config: &MinimalLoopConfig,
    state: &mut RepairStepState,
    missing_expected_paths: &[String],
    observer: &mut dyn RuntimeObserver,
    runner: &R,
) -> Result<DependencyRecoveryResult, String>
where
    E: ChatClient,
    P: ChatClient,
    R: DependencySetupRunner,
{
    let disposition = dependency_setup_disposition(
        runtime.cwd,
        &step.id,
        &state.failures,
        missing_expected_paths,
        config.dependency_setup_policy,
        state.dependency_setup_attempted,
    );

    let command = match disposition {
        DependencySetupDisposition::NotApplicable => {
            return Ok(DependencyRecoveryResult::NotApplicable);
        }
        DependencySetupDisposition::Blocked(message) => {
            return Ok(DependencyRecoveryResult::Blocked(message));
        }
        DependencySetupDisposition::Attempt(command) => command,
    };

    state.dependency_setup_attempted = true;
    observer.on_event(RuntimeEvent::DependencySetupStarted {
        step_id: bounded_event_text(&step.id),
        command: bounded_event_text(command.as_shell_command()),
    });
    let started = Instant::now();
    let status = runner.run_setup(runtime.cwd, command, config.dependency_setup_policy);
    observer.on_event(RuntimeEvent::DependencySetupFinished {
        step_id: bounded_event_text(&step.id),
        command: bounded_event_text(command.as_shell_command()),
        ok: status.ok(),
        elapsed_ms: started.elapsed().as_millis(),
        status: bounded_event_text(status.label()),
    });

    if !matches!(status, SetupRunStatus::Success) {
        return Ok(DependencyRecoveryResult::Blocked(
            setup_failed_blocker_message(&step.id, command, &status),
        ));
    }

    state.failures = execution::verify_step_with_observer(runtime.cwd, step, observer)?;
    if state.failures.is_empty() {
        return Ok(DependencyRecoveryResult::Recovered);
    }
    state.dependency_setup_note = Some(format!(
        "dependency_setup_attempted=true; dependency_setup_command={}; dependency_setup_result=success; verifier_rerun_result=failed",
        command.as_shell_command()
    ));

    if state
        .failures
        .iter()
        .all(|failure| failure.reason == "dependency_missing")
    {
        return Ok(DependencyRecoveryResult::Blocked(
            dependency_missing_blocker_message(
                &step.id,
                &state.failures,
                "Dependency setup completed once, but the verifier still reports missing dependencies.",
            ),
        ));
    }

    Ok(DependencyRecoveryResult::ContinueRepair)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::step_runner::verify::SourceExcerpt;
    use crate::agent::step_runner::{ExpectedResult, StepKind, StepPlanStep};

    #[test]
    fn missing_write_path_gets_protocol_correction_with_target() {
        let err = missing_write_path_error();
        let decision = tool_protocol_correction_decision(
            &step(StepKind::Create),
            Some(&err),
            Some("README.md".to_string()),
            false,
            Some(ToolProtocolCorrectionSource::InitialTurn),
        );

        let ToolProtocolCorrectionDecision::CorrectOnce(context) = decision else {
            panic!("expected CorrectOnce");
        };
        assert_eq!(context.tool, "Write");
        assert_eq!(context.reason_code, "tool_args_missing_required_field");
        assert_eq!(context.missing_field.as_deref(), Some("path"));
        assert_eq!(context.required_fields, vec!["path", "content"]);
        assert_eq!(context.target_path.as_deref(), Some("README.md"));
    }

    #[test]
    fn correction_spent_makes_protocol_failure_terminal() {
        let err = missing_write_path_error();
        let decision = tool_protocol_correction_decision(
            &step(StepKind::Create),
            Some(&err),
            None,
            true,
            Some(ToolProtocolCorrectionSource::InitialTurn),
        );

        assert_eq!(decision, ToolProtocolCorrectionDecision::Terminal);
    }

    #[test]
    fn invalid_json_gets_protocol_correction() {
        let err = ToolArgError::InvalidJson {
            tool: "Write".to_string(),
            message: "expected value".to_string(),
        };
        let decision = tool_protocol_correction_decision(
            &step(StepKind::Repair),
            Some(&err),
            None,
            false,
            Some(ToolProtocolCorrectionSource::InitialTurn),
        );

        let ToolProtocolCorrectionDecision::CorrectOnce(context) = decision else {
            panic!("expected CorrectOnce");
        };
        assert_eq!(context.reason_code, "tool_args_invalid_json");
        assert_eq!(context.tool, "Write");
        assert!(context.missing_field.is_none());
        assert!(context.target_path.is_none());
    }

    #[test]
    fn non_mutating_step_does_not_get_protocol_correction() {
        let err = missing_write_path_error();
        for kind in [StepKind::Inspect, StepKind::Verify, StepKind::Report] {
            let decision = tool_protocol_correction_decision(
                &step(kind),
                Some(&err),
                None,
                false,
                Some(ToolProtocolCorrectionSource::InitialTurn),
            );
            assert_eq!(
                decision,
                ToolProtocolCorrectionDecision::Terminal,
                "kind {kind:?}"
            );
        }
    }

    #[test]
    fn verify_repair_turn_gets_protocol_correction() {
        let err = missing_write_path_error();
        let decision = tool_protocol_correction_decision(
            &step(StepKind::Verify),
            Some(&err),
            None,
            false,
            Some(ToolProtocolCorrectionSource::RepairTurn),
        );

        let ToolProtocolCorrectionDecision::CorrectOnce(context) = decision else {
            panic!("expected CorrectOnce");
        };
        assert_eq!(context.tool, "Write");
        assert_eq!(context.reason_code, "tool_args_missing_required_field");
    }

    #[test]
    fn inspect_and_report_repair_turns_do_not_get_protocol_correction() {
        let err = missing_write_path_error();
        for kind in [StepKind::Inspect, StepKind::Report] {
            let decision = tool_protocol_correction_decision(
                &step(kind),
                Some(&err),
                None,
                false,
                Some(ToolProtocolCorrectionSource::RepairTurn),
            );
            assert_eq!(
                decision,
                ToolProtocolCorrectionDecision::Terminal,
                "kind {kind:?}"
            );
        }
    }

    #[test]
    fn absent_tool_arg_error_is_terminal() {
        let decision = tool_protocol_correction_decision(
            &step(StepKind::Create),
            None,
            Some("README.md".to_string()),
            false,
            Some(ToolProtocolCorrectionSource::InitialTurn),
        );

        assert_eq!(decision, ToolProtocolCorrectionDecision::Terminal);
    }

    #[test]
    fn target_path_prefers_missing_then_single_expected_path() {
        let mut step = step(StepKind::Create);
        step.expected_paths = vec!["app/page.tsx".to_string()];

        assert_eq!(
            tool_protocol_correction_target_path(&step, &["package.json".to_string()]).as_deref(),
            Some("package.json")
        );
        assert_eq!(
            tool_protocol_correction_target_path(&step, &[]).as_deref(),
            Some("app/page.tsx")
        );

        step.expected_paths.push("app/layout.tsx".to_string());
        assert!(tool_protocol_correction_target_path(&step, &[]).is_none());
    }

    #[test]
    fn tool_arg_contract_evidence_includes_target_path() {
        let mut state = empty_state();
        push_tool_arg_contract_evidence(
            &mut state,
            &step(StepKind::Create),
            &missing_write_path_error(),
            Some("src/components/GameCanvas.tsx".to_string()),
        );

        let rendered = state.contract_evidence[0].render().unwrap();

        assert!(rendered.contains("guard: tool_protocol"));
        assert!(rendered.contains("violated_contract: tool_args_missing_required_field"));
        assert!(rendered.contains("tool: Write"));
        assert!(rendered.contains("target_field: path"));
        assert!(rendered.contains("target_path: src/components/GameCanvas.tsx"));
        assert!(rendered.contains("required_fields: path, content"));
        assert!(rendered.contains("required_action: emit exactly one valid Write tool call"));
    }

    #[test]
    fn read_only_mutation_contract_evidence_classifies_step_policy() {
        let evidence = read_only_mutation_contract_evidence(
            &step(StepKind::Inspect),
            &MinimalLoopError::Tool(
                "tool_policy_violation: Write is not allowed in a read-only step".to_string(),
            ),
        )
        .unwrap();

        let rendered = evidence.render().unwrap();

        assert!(rendered.contains("guard: step_policy"));
        assert!(rendered.contains("violated_contract: read_only_step_mutation"));
        assert!(rendered.contains("tool: Write"));
        assert!(rendered.contains("move mutation into create/edit/repair steps"));
    }

    #[test]
    fn verifier_contract_evidence_renders_command_and_diagnostic() {
        let failure = VerificationFailure {
            command: "npm run build".to_string(),
            reason: "command_failed:1".to_string(),
            stdout_excerpt: String::new(),
            stderr_excerpt: "Failed to compile".to_string(),
            diagnostic_excerpt: "Type error: mismatch".to_string(),
            source_excerpt: None,
        };

        let evidence =
            verifier_contract_evidence(&step(StepKind::Verify), &failure, None, &[]).unwrap();
        let rendered = evidence.render().unwrap();

        assert!(rendered.contains("guard: verifier"));
        assert!(rendered.contains("violated_contract: command_failed:1"));
        assert!(rendered.contains("failure_kind: verifier_command_failed"));
        assert!(rendered.contains("diagnostic_code: command_failed:1"));
        assert!(
            rendered.contains("failure_signature: verifier|step|npm run build|command_failed:1")
        );
        assert!(rendered.contains("command: npm run build"));
        assert!(rendered.contains("Type error: mismatch"));
        assert!(rendered.contains("Failed to compile"));
    }

    #[test]
    fn verifier_contract_evidence_names_source_target_and_ledger() {
        let failure = VerificationFailure {
            command: "npm run build".to_string(),
            reason: "command_failed:1".to_string(),
            stdout_excerpt: String::new(),
            stderr_excerpt: "Failed to compile".to_string(),
            diagnostic_excerpt: "Type error: mismatch".to_string(),
            source_excerpt: Some(SourceExcerpt {
                path: "app/hooks/useGameLoop.ts".to_string(),
                line: 12,
                excerpt: " 11: const ref = useRef<number>()\n>12: ref.current = now".to_string(),
            }),
        };

        let evidence = verifier_contract_evidence(
            &step(StepKind::Verify),
            &failure,
            None,
            &[
                "attempt 1: verifier|step|npm run build|command_failed:1|app/hooks/useGameLoop.ts"
                    .to_string(),
            ],
        )
        .unwrap();
        let rendered = evidence.render().unwrap();

        assert!(rendered.contains("candidate_artifacts: app/hooks/useGameLoop.ts"));
        assert!(rendered.contains("repair_target: app/hooks/useGameLoop.ts"));
        assert!(rendered.contains("related_source_excerpt: app/hooks/useGameLoop.ts:12"));
        assert!(rendered.contains("repair_attempt_ledger: attempt 1: verifier|step"));
        assert!(rendered.contains("fix the verifier error in the repair target"));
    }

    #[test]
    fn verifier_contract_evidence_filters_dependency_paths_and_names_config_candidates() {
        let failure = VerificationFailure {
            command: "npm run build".to_string(),
            reason: "command_failed:1".to_string(),
            stdout_excerpt: String::new(),
            stderr_excerpt: String::new(),
            diagnostic_excerpt: "Error: It looks like you're trying to use `tailwindcss` directly as a PostCSS plugin. Install `@tailwindcss/postcss` and update your PostCSS configuration."
                .to_string(),
            source_excerpt: Some(SourceExcerpt {
                path: "node_modules/tailwindcss/dist/lib.js".to_string(),
                line: 38,
                excerpt: ">38: throw new Error('plugin')".to_string(),
            }),
        };

        let evidence =
            verifier_contract_evidence(&step(StepKind::Verify), &failure, None, &[]).unwrap();
        let rendered = evidence.render().unwrap();

        assert!(rendered.contains("candidate_artifacts: package.json, postcss.config.js"));
        assert!(!rendered.contains("repair_target: node_modules"));
        assert!(!rendered.contains("related_source_excerpt: node_modules"));
    }

    #[test]
    fn dependency_setup_note_is_verifier_diagnostic_context() {
        let failure = VerificationFailure {
            command: "npm run build".to_string(),
            reason: "command_failed:1".to_string(),
            stdout_excerpt: String::new(),
            stderr_excerpt: String::new(),
            diagnostic_excerpt: "Build still fails".to_string(),
            source_excerpt: None,
        };

        let evidence = verifier_contract_evidence(
            &step(StepKind::Verify),
            &failure,
            Some("dependency_setup_attempted=true; dependency_setup_command=npm install; dependency_setup_result=success; verifier_rerun_result=failed"),
            &[],
        )
        .unwrap();
        let rendered = evidence.render().unwrap();

        assert!(rendered.contains("guard: verifier"));
        assert!(rendered.contains("dependency_setup_attempted=true"));
        assert!(rendered.contains("dependency_setup_command=npm install"));
        assert!(!rendered.contains("guard: setup"));
    }

    #[test]
    fn read_only_recovery_envelope_selects_read_only_repair_config() {
        let mut config = MinimalLoopConfig::default();

        apply_repair_execution_envelope(
            &mut config,
            Some(RecoveryExecutionEnvelope::ReadOnlyEvidence),
        );

        assert_eq!(
            config.action_requirement,
            ActionRequirement::RepositoryEvidenceRequired
        );
        assert_eq!(config.step_tool_policy, StepToolPolicy::ReadOnly);
    }

    #[test]
    fn file_repair_envelope_keeps_file_mutation_repair_config() {
        let mut config = MinimalLoopConfig::default();

        apply_repair_execution_envelope(
            &mut config,
            Some(RecoveryExecutionEnvelope::FileMutationRepair),
        );

        assert_eq!(config.action_requirement, ActionRequirement::Required);
        assert_eq!(config.step_tool_policy, StepToolPolicy::FileMutationAllowed);
    }

    #[test]
    fn turn_error_failures_do_not_become_verifier_contract_evidence() {
        let failure = VerificationFailure {
            command: "repair turn".to_string(),
            reason: "tool_args_missing_required_field".to_string(),
            stdout_excerpt: String::new(),
            stderr_excerpt: String::new(),
            diagnostic_excerpt: "Write missing path".to_string(),
            source_excerpt: None,
        };

        assert!(verifier_contract_evidence(&step(StepKind::Repair), &failure, None, &[]).is_none());
    }

    fn missing_write_path_error() -> ToolArgError {
        ToolArgError::MissingRequiredStringField {
            tool: "Write".to_string(),
            field: "path".to_string(),
            required_fields: vec!["path".to_string(), "content".to_string()],
        }
    }

    fn step(kind: StepKind) -> StepPlanStep {
        StepPlanStep {
            id: "step".to_string(),
            kind,
            instruction: "Do the step.".to_string(),
            expected_result: ExpectedResult::Pass,
            expected_paths: Vec::new(),
            verify: Vec::new(),
        }
    }

    fn empty_state() -> RepairStepState {
        RepairStepState {
            failures: Vec::new(),
            changed_files: Vec::new(),
            file_changing_attempts: 0,
            initial_turn_error: None,
            dependency_setup_attempted: false,
            dependency_setup_note: None,
            contract_evidence: Vec::new(),
            repair_attempt_ledger: Vec::new(),
            tool_arg_schema_correction_spent: false,
            pending_tool_arg_error: None,
            pending_tool_arg_error_source: None,
        }
    }
}
