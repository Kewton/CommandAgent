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
                    verification_failures: state.failures.clone(),
                    missing_expected_paths: missing_expected_paths.clone(),
                    changed_files: state.changed_files.clone(),
                };
                build_repair_prompt(&context)
            }
        };
        let mut repair_config = config.clone();
        repair_config.action_requirement = ActionRequirement::Required;
        repair_config.step_tool_policy = StepToolPolicy::FileMutationAllowed;
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
    let context = RepairContext {
        step_id: step.id.clone(),
        original_goal: plan.goal.clone(),
        profile: plan.profile.clone(),
        style: plan.style.clone(),
        step_instruction: step.instruction.clone(),
        active_profile_contract_facts: active_contract_facts(runtime, plan, contract_seed),
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
}
