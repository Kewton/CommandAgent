use super::SlashRuntime;
use super::execution;
use super::paths::{changed_file_markers, missing_paths, result_changed_files};
use crate::agent::events::{RuntimeEvent, RuntimeObserver, bounded_event_text};
use crate::agent::minimal_loop::loop_run::{ChatClient, MinimalLoopConfig, RunResult};
use crate::agent::step_runner::repair::{
    RepairBudget, RepairContext, build_repair_prompt, save_repair_prompt,
};
use crate::agent::step_runner::verify::VerificationFailure;
use crate::agent::step_runner::{StepPlan, StepPlanStep};

const MAX_REPAIR_TURNS: usize = 3;

pub(super) struct RepairStepState {
    pub(super) failures: Vec<VerificationFailure>,
    pub(super) changed_files: Vec<String>,
    pub(super) file_changing_attempts: usize,
    pub(super) initial_turn_error: Option<String>,
}

pub(super) fn turn_error_failure(command: &str, error: String) -> VerificationFailure {
    let (reason, diagnostic_excerpt) = turn_error_reason_and_diagnostic(&error);
    VerificationFailure {
        command: command.to_string(),
        reason: reason.to_string(),
        stdout_excerpt: String::new(),
        stderr_excerpt: String::new(),
        diagnostic_excerpt,
        source_excerpt: None,
    }
}

fn turn_error_reason_and_diagnostic(error: &str) -> (&'static str, String) {
    if is_edit_target_not_found(error) {
        return (
            "edit_target_not_found",
            format!(
                "Edit target was not found. The file state is stale for this Edit attempt. Read or Glob the current file first, then use Edit only with exact current target text from this repair turn, or Write when full replacement/creation is safer.\nOriginal error: {error}"
            ),
        );
    }
    ("turn_error", error.to_string())
}

pub(super) fn recoverable_repair_turn_error(error: &str) -> bool {
    error.contains("assistant violated final answer contract")
        || error.contains("missing expected artifacts")
        || is_edit_target_not_found(error)
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
    plan: &StepPlan,
    step: &StepPlanStep,
    config: MinimalLoopConfig,
    turn_error: String,
    failures: Vec<VerificationFailure>,
    observer: &mut dyn RuntimeObserver,
) -> Result<(), String>
where
    E: ChatClient,
    P: ChatClient,
{
    let mut failures = failures;
    failures.insert(0, turn_error_failure("initial turn", turn_error.clone()));
    repair_step_with_state(
        runtime,
        plan,
        step,
        config,
        RepairStepState {
            failures,
            changed_files: Vec::new(),
            file_changing_attempts: 0,
            initial_turn_error: Some(turn_error),
        },
        observer,
    )
}

pub(super) fn repair_step<E, P>(
    runtime: &mut SlashRuntime<'_, E, P>,
    plan: &StepPlan,
    step: &StepPlanStep,
    config: MinimalLoopConfig,
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
        plan,
        step,
        config,
        RepairStepState {
            failures,
            changed_files: changed_file_markers(&first_result),
            file_changing_attempts: usize::from(result_changed_files(&first_result)),
            initial_turn_error: None,
        },
        observer,
    )
}

pub(super) fn repair_step_with_state<E, P>(
    runtime: &mut SlashRuntime<'_, E, P>,
    plan: &StepPlan,
    step: &StepPlanStep,
    config: MinimalLoopConfig,
    mut state: RepairStepState,
    observer: &mut dyn RuntimeObserver,
) -> Result<(), String>
where
    E: ChatClient,
    P: ChatClient,
{
    let budget = RepairBudget::default();
    let mut repair_turns = 0usize;
    let mut missing_artifact_no_tool_guard_sent = false;
    let initial_missing_expected_paths = missing_paths(runtime.cwd, &step.expected_paths);
    if let Some(message) =
        dependency_missing_blocker_message(step, &state.failures, &initial_missing_expected_paths)
    {
        return Err(message);
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
        let context = RepairContext {
            step_id: step.id.clone(),
            original_goal: plan.goal.clone(),
            profile: plan.profile.clone(),
            style: plan.style.clone(),
            step_instruction: step.instruction.clone(),
            verification_failures: state.failures.clone(),
            missing_expected_paths: missing_expected_paths.clone(),
            changed_files: state.changed_files.clone(),
        };
        let prompt = build_repair_prompt(&context);
        let result = match crate::agent::minimal_loop::loop_run::run_session_with_observer(
            runtime.executor,
            runtime.cwd,
            &prompt,
            config.clone(),
            observer,
        ) {
            Ok(result) => result,
            Err(err) => {
                let error = err.to_string();
                state
                    .failures
                    .push(turn_error_failure("repair turn", error.clone()));
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
                if recoverable_repair_turn_error(&error)
                    && budget.allows_next_attempt(state.file_changing_attempts)
                    && repair_turns < MAX_REPAIR_TURNS
                {
                    continue;
                }
                break;
            }
        };
        if result_changed_files(&result) {
            state.file_changing_attempts += 1;
        }
        state.changed_files.extend(changed_file_markers(&result));
        state.failures = execution::verify_step_with_observer(runtime.cwd, step, observer)?;
        if state.failures.is_empty() {
            return Ok(());
        }
        let missing_expected_paths = missing_paths(runtime.cwd, &step.expected_paths);
        if let Some(message) =
            dependency_missing_blocker_message(step, &state.failures, &missing_expected_paths)
        {
            return Err(message);
        }
    }

    let missing_expected_paths = missing_paths(runtime.cwd, &step.expected_paths);
    if let Some(message) =
        dependency_missing_blocker_message(step, &state.failures, &missing_expected_paths)
    {
        return Err(message);
    }
    let context = RepairContext {
        step_id: step.id.clone(),
        original_goal: plan.goal.clone(),
        profile: plan.profile.clone(),
        style: plan.style.clone(),
        step_instruction: step.instruction.clone(),
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

fn dependency_missing_blocker_message(
    step: &StepPlanStep,
    failures: &[VerificationFailure],
    missing_expected_paths: &[String],
) -> Option<String> {
    if failures.is_empty()
        || !missing_expected_paths.is_empty()
        || !failures
            .iter()
            .all(|failure| failure.reason == "dependency_missing")
    {
        return None;
    }

    let mut commands = Vec::new();
    let mut diagnostics = Vec::new();
    for failure in failures {
        if !commands.contains(&failure.command) {
            commands.push(failure.command.clone());
        }
        if !failure.diagnostic_excerpt.trim().is_empty() {
            diagnostics.push(failure.diagnostic_excerpt.trim().to_string());
        }
    }

    let mut message = format!(
        "dependency_missing: step {} cannot be repaired by editing files.\n\n\
This is an environment/setup blocker, not a code repair failure. Install project dependencies explicitly when allowed, then rerun the verifier.",
        step.id
    );
    if !diagnostics.is_empty() {
        message.push_str("\n\nVerifier evidence:\n");
        message.push_str(&diagnostics.join("\n"));
    }
    message.push_str("\n\nRun dependency setup manually, for example:\n  npm install");
    if commands.is_empty() {
        message.push_str("\n\nThen rerun the original verifier.");
    } else {
        message.push_str("\n\nThen rerun:\n");
        for command in commands {
            message.push_str("  ");
            message.push_str(&command);
            message.push('\n');
        }
        if message.ends_with('\n') {
            message.pop();
        }
    }
    message
        .push_str("\n\nCommandAgent did not create a repair prompt because this blocker requires explicit setup.");
    Some(message)
}
