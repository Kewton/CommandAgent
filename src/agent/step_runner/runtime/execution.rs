use super::SlashRuntime;
use super::paths::{display_path, missing_paths};
use super::planning::{StepPlanCorrectionContext, planner_text};
use super::prompts::step_prompt;
use super::repair_loop::RepairStepState;
use crate::agent::minimal_loop::loop_run::{ChatClient, run_session};
use crate::agent::step_runner::plan_lint::lint_step_plan_with_workspace;
use crate::agent::step_runner::profiles::profile_contract_text;
use crate::agent::step_runner::ultra_plan::{UltraPlan, parse_ultra_plan_yaml};
use crate::agent::step_runner::ultra_run::phase_step_plan_prompt;
use crate::agent::step_runner::verify::{VerificationFailure, run_verifiers};
use crate::agent::step_runner::{
    ExpectedResult, StepKind, StepPlan, StepPlanStep, WorkIntent, parse_step_plan_yaml,
    save_step_plan,
};
use crate::safety::path_guard::PathGuard;
use std::fs;
use std::path::Path;

pub(super) fn execute_ultra_plan<E, P>(
    runtime: &mut SlashRuntime<'_, E, P>,
    plan: &UltraPlan,
) -> Result<String, String>
where
    E: ChatClient,
    P: ChatClient,
{
    let profile_contract = profile_contract_text(&plan.profile).map_err(|err| err.to_string())?;
    let snapshot = crate::agent::step_runner::ultra_run::workspace_snapshot(runtime.cwd);
    let mut lines = Vec::new();
    lines.push(format!("ultra plan: {} phases", plan.phases.len()));

    for (idx, phase) in plan.phases.iter().enumerate() {
        lines.push(format!(
            "phase {}/{} {}: planning",
            idx + 1,
            plan.phases.len(),
            phase.id
        ));
        let prompt = phase_step_plan_prompt(plan, phase, &snapshot, &profile_contract);
        let text = planner_text(runtime.planner, &runtime.planner_config, &prompt)?;
        let correction_context = StepPlanCorrectionContext {
            goal: &phase.goal,
            profile: &plan.profile,
            style: &plan.style,
            intent: WorkIntent::parse(&plan.intent).unwrap_or(WorkIntent::Unknown),
            required_artifacts: &plan.required_artifacts,
            save_kind: "phase-step-plan",
            prompt_kind: "phase step plan",
        };
        let step_plan =
            runtime.parse_generated_step_plan_with_corrections(text, correction_context)?;
        let path = save_step_plan(runtime.cwd, &step_plan).map_err(|err| err.to_string())?;
        lines.push(format!(
            "phase {}: step plan {}",
            phase.id,
            display_path(runtime.cwd, &path)
        ));
        let report = execute_step_plan(runtime, &step_plan)?;
        lines.push(format!("phase {}: ok\n{}", phase.id, report));
    }

    let missing = missing_paths(runtime.cwd, &plan.required_artifacts);
    if !missing.is_empty() {
        return Err(format!(
            "missing required final artifacts: {}",
            missing.join(", ")
        ));
    }

    Ok(lines.join("\n"))
}

pub(super) fn execute_step_plan<E, P>(
    runtime: &mut SlashRuntime<'_, E, P>,
    plan: &StepPlan,
) -> Result<String, String>
where
    E: ChatClient,
    P: ChatClient,
{
    let mut lines = Vec::new();
    lines.push(format!("step plan: {} steps", plan.steps.len()));
    for (idx, step) in plan.steps.iter().enumerate() {
        lines.push(format!(
            "step {}/{} {}: running",
            idx + 1,
            plan.steps.len(),
            step.id
        ));
        execute_step(runtime, plan, step)?;
        lines.push(format!("step {}: ok", step.id));
    }
    Ok(lines.join("\n"))
}

pub(super) fn execute_step<E, P>(
    runtime: &mut SlashRuntime<'_, E, P>,
    plan: &StepPlan,
    step: &StepPlanStep,
) -> Result<(), String>
where
    E: ChatClient,
    P: ChatClient,
{
    let mut config = runtime.loop_config.clone();
    if step.expected_result == ExpectedResult::Pass {
        config.expected_artifacts = step.expected_paths.clone();
    }
    if matches!(step.kind, StepKind::Verify) && !step.verify.is_empty() {
        let failures = verify_step(runtime.cwd, step)?;
        if failures.is_empty() || step_accepts_verifier_failure(step) {
            return Ok(());
        }
        return runtime.repair_step_with_state(
            plan,
            step,
            config,
            RepairStepState {
                failures,
                changed_files: Vec::new(),
                file_changing_attempts: 0,
                initial_turn_error: None,
            },
        );
    }
    let missing_expected_paths = missing_paths(runtime.cwd, &step.expected_paths);
    let prompt = step_prompt(plan, step, &missing_expected_paths)?;
    let result = match run_session(runtime.executor, runtime.cwd, &prompt, config.clone()) {
        Ok(result) => result,
        Err(err) => {
            let failures = verify_step(runtime.cwd, step)?;
            if failures.is_empty() || step_accepts_verifier_failure(step) {
                return Ok(());
            }
            return runtime.repair_step_after_turn_error(
                plan,
                step,
                config,
                err.to_string(),
                failures,
            );
        }
    };
    let failures = verify_step(runtime.cwd, step)?;
    if failures.is_empty() || step_accepts_verifier_failure(step) {
        return Ok(());
    }

    runtime.repair_step(plan, step, config, result, failures)
}

fn step_accepts_verifier_failure(step: &StepPlanStep) -> bool {
    matches!(
        step.expected_result,
        ExpectedResult::Fail | ExpectedResult::Unavailable
    )
}

pub(super) fn verify_step(
    cwd: &Path,
    step: &StepPlanStep,
) -> Result<Vec<VerificationFailure>, String> {
    let commands = if step.verify.is_empty() {
        Vec::new()
    } else {
        step.verify.clone()
    };
    let report = run_verifiers(cwd, &commands).map_err(|err| err.to_string())?;
    Ok(report.failures)
}

pub(super) fn load_step_plan(cwd: &Path, path: &str) -> Result<StepPlan, String> {
    let guard = PathGuard::new(cwd).map_err(|err| err.to_string())?;
    let path = guard.resolve(path).map_err(|err| err.to_string())?;
    let text = fs::read_to_string(&path).map_err(|err| format!("{}: {err}", path.display()))?;
    let plan = parse_step_plan_yaml(&text).map_err(|err| err.to_string())?;
    lint_step_plan_with_workspace(&plan, Some(cwd))
        .map_err(|err| format!("plan lint failed: {err}"))?;
    Ok(plan)
}

pub(super) fn load_ultra_plan(cwd: &Path, path: &str) -> Result<UltraPlan, String> {
    let guard = PathGuard::new(cwd).map_err(|err| err.to_string())?;
    let path = guard.resolve(path).map_err(|err| err.to_string())?;
    let text = fs::read_to_string(&path).map_err(|err| format!("{}: {err}", path.display()))?;
    parse_ultra_plan_yaml(&text).map_err(|err| err.to_string())
}
