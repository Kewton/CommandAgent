use super::SlashRuntime;
use super::paths::{display_path, missing_paths};
use super::planning::{StepPlanCorrectionContext, planner_text};
use super::prompts::step_prompt;
use super::repair_loop::{RepairStepRequest, RepairStepState};
use crate::agent::events::{
    ArtifactScope, ArtifactStatus, PlanKind, RuntimeEvent, RuntimeObserver, bounded_event_text,
};
use crate::agent::minimal_loop::config::{ActionRequirement, StepToolPolicy};
use crate::agent::minimal_loop::loop_run::ChatClient;
use crate::agent::step_runner::plan_lint::lint_step_plan_with_workspace;
use crate::agent::step_runner::profiles::{
    ProfileVerificationContext, profile_contract_text, profile_fact_summary, verify_profile,
};
use crate::agent::step_runner::repair::{ProfileRepairContext, save_profile_repair_prompt};
use crate::agent::step_runner::runtime::phase_contract::{
    ActiveStepContract, PhaseWorkspaceContract, current_profile_facts,
};
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
        let snapshot = crate::agent::step_runner::ultra_run::workspace_snapshot(runtime.cwd);
        let phase_contract = PhaseWorkspaceContract::collect_with_goal(
            runtime.cwd,
            &plan.profile,
            &plan.required_artifacts,
            &format!("{} {}", plan.goal, phase.goal),
        );
        let active_contract_seed =
            ActiveStepContract::from_phase_contract(&plan.profile, &phase_contract, Vec::new());
        let phase_contract_rendered = phase_contract.render();
        let prompt = phase_step_plan_prompt(
            plan,
            phase,
            &snapshot,
            &phase_contract_rendered,
            &profile_contract,
        );
        let text = planner_text(runtime.planner, &runtime.planner_config, &prompt)?;
        let correction_context = StepPlanCorrectionContext {
            goal: &phase.goal,
            profile: &plan.profile,
            style: &plan.style,
            intent: WorkIntent::parse(&plan.intent).unwrap_or(WorkIntent::Unknown),
            required_artifacts: &plan.required_artifacts,
            profile_obligations: &phase_contract.profile_obligations,
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
        let report = match execute_step_plan(runtime, &step_plan, &active_contract_seed, observer) {
            Ok(report) => report,
            Err(err) => {
                let err = match verify_phase_profile(
                    runtime.cwd,
                    plan,
                    phase,
                    &step_plan,
                    &phase_contract,
                    observer,
                ) {
                    Ok(()) => err,
                    Err(profile_err) => format!("{err}\n{profile_err}"),
                };
                observer.on_event(RuntimeEvent::UltraPhaseFailed {
                    index: idx + 1,
                    total: plan.phases.len(),
                    phase_id: bounded_event_text(&phase.id),
                    error: bounded_event_text(&err),
                });
                return Err(err);
            }
        };
        if let Err(err) = verify_phase_profile(
            runtime.cwd,
            plan,
            phase,
            &step_plan,
            &phase_contract,
            observer,
        ) {
            observer.on_event(RuntimeEvent::UltraPhaseFailed {
                index: idx + 1,
                total: plan.phases.len(),
                phase_id: bounded_event_text(&phase.id),
                error: bounded_event_text(&err),
            });
            return Err(err);
        }
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

fn verify_phase_profile(
    cwd: &Path,
    plan: &UltraPlan,
    phase: &crate::agent::step_runner::ultra_plan::UltraPhase,
    step_plan: &StepPlan,
    phase_contract: &PhaseWorkspaceContract,
    observer: &mut dyn RuntimeObserver,
) -> Result<(), String> {
    let profile_facts = profile_fact_summary(&plan.profile, cwd)
        .map_err(|err| err.to_string())?
        .lines;
    let expected_paths = step_plan
        .steps
        .iter()
        .flat_map(|step| step.expected_paths.iter().cloned())
        .collect::<Vec<_>>();
    let context = ProfileVerificationContext {
        goal_excerpt: bounded_event_text(format!("{} {}", plan.goal, phase.goal)),
        required_artifacts: plan.required_artifacts.clone(),
        expected_paths: expected_paths.clone(),
        phase_contract_facts: phase_contract.fact_lines(),
        profile_facts: profile_facts.clone(),
    };
    let failures = verify_profile(&plan.profile, cwd, &context).map_err(|err| err.to_string())?;
    if failures.is_empty() {
        return Ok(());
    }

    let rendered = failures
        .iter()
        .map(|failure| bounded_event_text(failure.render()))
        .collect::<Vec<_>>();
    observer.on_event(RuntimeEvent::ProfileVerificationFailed {
        profile: bounded_event_text(&plan.profile),
        failures: rendered.clone(),
    });
    let saved = save_profile_repair_prompt(
        cwd,
        &ProfileRepairContext {
            phase_id: phase.id.clone(),
            original_goal: plan.goal.clone(),
            phase_goal: phase.goal.clone(),
            profile: plan.profile.clone(),
            style: plan.style.clone(),
            profile_failures: failures,
            phase_contract_facts: phase_contract.fact_lines(),
            profile_facts,
            expected_paths,
        },
    )
    .map_err(|err| err.to_string())?;
    Err(format!(
        "profile verification failed for {} after phase {}: {}.\nprofile repair prompt saved: {}\nsuggested command: {}\nRun an explicit repair or replan command; the runtime did not continue automatically.",
        plan.profile,
        phase.id,
        rendered.join("; "),
        saved.relative_path,
        saved.suggested_command
    ))
}

pub(super) fn execute_step_plan<E, P>(
    runtime: &mut SlashRuntime<'_, E, P>,
    plan: &StepPlan,
    contract_seed: &ActiveStepContract,
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
        if let Err(err) = execute_step(runtime, plan, step, contract_seed, observer) {
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
    contract_seed: &ActiveStepContract,
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
    config.step_tool_policy = step_tool_policy_for_step(step);
    let missing_expected_paths = missing_paths(runtime.cwd, &step.expected_paths);
    config.action_requirement = action_requirement_for_step(step, &missing_expected_paths);
    if matches!(step.kind, StepKind::Verify) && !step.verify.is_empty() {
        let failures = verify_step_with_observer(runtime.cwd, step, observer)?;
        if failures.is_empty() || step_accepts_verifier_failure(step) {
            return Ok(());
        }
        return runtime.repair_step_with_state(
            RepairStepRequest {
                plan,
                step,
                config,
                contract_seed,
            },
            RepairStepState {
                failures,
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
            },
            observer,
        );
    }
    let active_contract =
        contract_seed.with_current_profile_facts(current_profile_facts(&plan.profile, runtime.cwd));
    let prompt = step_prompt(plan, step, &missing_expected_paths, &active_contract)?;
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
            if fatal_turn_error_for_step(&err) {
                return runtime.repair_step_after_turn_error(
                    RepairStepRequest {
                        plan,
                        step,
                        config,
                        contract_seed,
                    },
                    err,
                    failures,
                    observer,
                );
            }
            if failures.is_empty() || step_accepts_verifier_failure(step) {
                return Ok(());
            }
            return runtime.repair_step_after_turn_error(
                RepairStepRequest {
                    plan,
                    step,
                    config,
                    contract_seed,
                },
                err,
                failures,
                observer,
            );
        }
    };
    let failures = verify_step_with_observer(runtime.cwd, step, observer)?;
    if failures.is_empty() || step_accepts_verifier_failure(step) {
        return Ok(());
    }

    runtime.repair_step(
        RepairStepRequest {
            plan,
            step,
            config,
            contract_seed,
        },
        result,
        failures,
        observer,
    )
}

fn step_accepts_verifier_failure(step: &StepPlanStep) -> bool {
    matches!(
        step.expected_result,
        ExpectedResult::Fail | ExpectedResult::Unavailable
    )
}

fn action_requirement_for_step(
    step: &StepPlanStep,
    missing_expected_paths: &[String],
) -> ActionRequirement {
    match step.kind {
        StepKind::Create | StepKind::Edit | StepKind::Repair => ActionRequirement::Required,
        StepKind::Setup
            if !step.expected_paths.is_empty() || !missing_expected_paths.is_empty() =>
        {
            ActionRequirement::Required
        }
        _ => ActionRequirement::Optional,
    }
}

fn step_tool_policy_for_step(step: &StepPlanStep) -> StepToolPolicy {
    match step.kind {
        StepKind::Inspect | StepKind::Report => StepToolPolicy::ReadOnly,
        StepKind::Verify => StepToolPolicy::NoMutation,
        StepKind::Setup => StepToolPolicy::SetupMutationOnly,
        StepKind::Create | StepKind::Edit | StepKind::Repair => StepToolPolicy::FileMutationAllowed,
    }
}

fn fatal_turn_error_for_step(error: &crate::agent::minimal_loop::result::MinimalLoopError) -> bool {
    use crate::agent::minimal_loop::result::MinimalLoopError;

    match error {
        MinimalLoopError::MaxIterations
        | MinimalLoopError::ToolArgs(_)
        | MinimalLoopError::Tool(_) => true,
        MinimalLoopError::Model(_)
        | MinimalLoopError::FinalAnswerContract(_)
        | MinimalLoopError::ActionRequiredNoEvidence(_)
        | MinimalLoopError::MissingArtifacts(_) => false,
    }
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
