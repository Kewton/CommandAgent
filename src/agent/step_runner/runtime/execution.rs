use super::SlashRuntime;
use super::paths::{display_path, missing_paths};
use super::planning::{StepPlanCorrectionContext, planner_text};
use super::prompts::step_prompt;
use super::repair_loop::RepairStepState;
use crate::agent::events::{
    ArtifactScope, ArtifactStatus, PlanKind, RuntimeEvent, RuntimeObserver, bounded_event_text,
};
use crate::agent::minimal_loop::loop_run::ChatClient;
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
    observer: &mut dyn RuntimeObserver,
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
        observer.on_event(RuntimeEvent::UltraPhaseStarted {
            index: idx + 1,
            total: plan.phases.len(),
            phase_id: bounded_event_text(&phase.id),
        });
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
        observer.on_event(RuntimeEvent::PlanSaved {
            kind: PlanKind::PhaseStepPlan,
            path: display_path(runtime.cwd, &path),
            item_ids: step_plan.steps.iter().map(|step| step.id.clone()).collect(),
        });
        lines.push(format!(
            "phase {}: step plan {}",
            phase.id,
            display_path(runtime.cwd, &path)
        ));
        let report = match execute_step_plan(runtime, &step_plan, observer) {
            Ok(report) => report,
            Err(err) => {
                observer.on_event(RuntimeEvent::UltraPhaseFailed {
                    index: idx + 1,
                    total: plan.phases.len(),
                    phase_id: bounded_event_text(&phase.id),
                    error: bounded_event_text(&err),
                });
                return Err(err);
            }
        };
        observer.on_event(RuntimeEvent::UltraPhaseFinished {
            index: idx + 1,
            total: plan.phases.len(),
            phase_id: bounded_event_text(&phase.id),
        });
        lines.push(format!("phase {}: ok\n{}", phase.id, report));
    }

    let missing = missing_paths(runtime.cwd, &plan.required_artifacts);
    emit_final_artifact_status(observer, &plan.required_artifacts, &missing);
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
    observer: &mut dyn RuntimeObserver,
) -> Result<String, String>
where
    E: ChatClient,
    P: ChatClient,
{
    let mut lines = Vec::new();
    lines.push(format!("step plan: {} steps", plan.steps.len()));
    for (idx, step) in plan.steps.iter().enumerate() {
        observer.on_event(RuntimeEvent::StepStarted {
            index: idx + 1,
            total: plan.steps.len(),
            step_id: bounded_event_text(&step.id),
        });
        lines.push(format!(
            "step {}/{} {}: running",
            idx + 1,
            plan.steps.len(),
            step.id
        ));
        if let Err(err) = execute_step(runtime, plan, step, observer) {
            observer.on_event(RuntimeEvent::StepFailed {
                index: idx + 1,
                total: plan.steps.len(),
                step_id: bounded_event_text(&step.id),
                error: bounded_event_text(&err),
                missing_expected_paths: missing_paths(runtime.cwd, &step.expected_paths),
            });
            return Err(err);
        }
        observer.on_event(RuntimeEvent::StepFinished {
            index: idx + 1,
            total: plan.steps.len(),
            step_id: bounded_event_text(&step.id),
        });
        lines.push(format!("step {}: ok", step.id));
    }
    Ok(lines.join("\n"))
}

pub(super) fn execute_step<E, P>(
    runtime: &mut SlashRuntime<'_, E, P>,
    plan: &StepPlan,
    step: &StepPlanStep,
    observer: &mut dyn RuntimeObserver,
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
        let failures = verify_step_with_observer(runtime.cwd, step, observer)?;
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
            observer,
        );
    }
    let missing_expected_paths = missing_paths(runtime.cwd, &step.expected_paths);
    let prompt = step_prompt(plan, step, &missing_expected_paths)?;
    let result = match crate::agent::minimal_loop::loop_run::run_session_with_observer(
        runtime.executor,
        runtime.cwd,
        &prompt,
        config.clone(),
        observer,
    ) {
        Ok(result) => result,
        Err(err) => {
            let failures = verify_step_with_observer(runtime.cwd, step, observer)?;
            if failures.is_empty() || step_accepts_verifier_failure(step) {
                return Ok(());
            }
            return runtime.repair_step_after_turn_error(
                plan,
                step,
                config,
                err.to_string(),
                failures,
                observer,
            );
        }
    };
    let failures = verify_step_with_observer(runtime.cwd, step, observer)?;
    if failures.is_empty() || step_accepts_verifier_failure(step) {
        return Ok(());
    }

    runtime.repair_step(plan, step, config, result, failures, observer)
}

fn step_accepts_verifier_failure(step: &StepPlanStep) -> bool {
    matches!(
        step.expected_result,
        ExpectedResult::Fail | ExpectedResult::Unavailable
    )
}

pub(super) fn verify_step_with_observer(
    cwd: &Path,
    step: &StepPlanStep,
    observer: &mut dyn RuntimeObserver,
) -> Result<Vec<VerificationFailure>, String> {
    let commands = if step.verify.is_empty() {
        Vec::new()
    } else {
        step.verify.clone()
    };
    for command in &commands {
        observer.on_event(RuntimeEvent::VerifierStarted {
            step_id: bounded_event_text(&step.id),
            command: bounded_event_text(command),
        });
    }
    let report = run_verifiers(cwd, &commands).map_err(|err| err.to_string())?;
    for command in &commands {
        observer.on_event(RuntimeEvent::VerifierFinished {
            step_id: bounded_event_text(&step.id),
            command: bounded_event_text(command),
            ok: report.failures.is_empty(),
            failure_count: report.failures.len(),
        });
    }
    Ok(report.failures)
}

fn emit_final_artifact_status(
    observer: &mut dyn RuntimeObserver,
    required_artifacts: &[String],
    missing: &[String],
) {
    for path in required_artifacts {
        let status = if missing.iter().any(|missing_path| missing_path == path) {
            ArtifactStatus::Missing
        } else {
            ArtifactStatus::Ok
        };
        observer.on_event(RuntimeEvent::ArtifactStatus {
            scope: ArtifactScope::FinalRequiredArtifact,
            path: bounded_event_text(path),
            status,
        });
    }
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
