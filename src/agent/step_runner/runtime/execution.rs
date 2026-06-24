use super::SlashRuntime;
use super::dev_server::{requested_dev_port, verify_nextjs_dev_server_smoke};
use super::paths::{display_path, missing_paths};
use super::planning::{StepPlanCorrectionContext, planner_text};
use super::prompts::step_prompt;
use super::repair_loop::{RepairStepRequest, RepairStepState};
use crate::agent::events::{
    ArtifactScope, ArtifactStatus, PlanKind, RuntimeEvent, RuntimeObserver, bounded_event_text,
};
use crate::agent::minimal_loop::config::{ActionRequirement, MinimalLoopConfig, StepToolPolicy};
use crate::agent::minimal_loop::loop_run::ChatClient;
use crate::agent::minimal_loop::result::ToolExecutionRecord;
use crate::agent::step_runner::artifact_graph::{ArtifactGraph, ArtifactLifecycle};
use crate::agent::step_runner::artifact_ledger::ArtifactLedgerSummary;
use crate::agent::step_runner::evidence_authority::{
    CompletionAuthorityResult, evaluate_completion_authority,
};
use crate::agent::step_runner::evidence_producer::{
    EvidenceProducerInput, produce_completion_evidence,
};
use crate::agent::step_runner::plan_lint::lint_step_plan_with_workspace;
use crate::agent::step_runner::profiles::{
    ProfileVerificationContext, profile_contract_text, profile_fact_summary, verify_profile,
};
use crate::agent::step_runner::repair::{
    ProfileRepairContext, build_profile_replan_packet, save_profile_repair_prompt,
};
use crate::agent::step_runner::runtime::phase_contract::{
    ActiveStepContract, PhaseWorkspaceContract, current_profile_facts,
};
use crate::agent::step_runner::task_contract::TaskContract;
use crate::agent::step_runner::ultra_plan::{UltraPlan, parse_ultra_plan_yaml};
use crate::agent::step_runner::ultra_run::{phase_owned_artifacts, phase_step_plan_prompt};
use crate::agent::step_runner::verify::{VerificationFailure, run_verifiers};
use crate::agent::step_runner::workspace_scope::WorkspaceScope;
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
    let mut nextjs_build_verifier_seen = false;

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
        let phase_artifacts = phase_owned_artifacts(plan, phase);
        let phase_contract = PhaseWorkspaceContract::collect_with_scope(
            runtime.cwd,
            &plan.profile,
            &plan.required_artifacts,
            &phase_artifacts,
            &phase.preserve_artifacts,
            &phase.verify_only_artifacts,
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
        let phase_task_contract = TaskContract::from_goal(
            &plan.profile,
            &phase.goal,
            WorkIntent::parse(&plan.intent).unwrap_or(WorkIntent::Unknown),
            &phase_artifacts,
            &phase_contract.profile_obligations,
        );
        let correction_context = StepPlanCorrectionContext {
            goal: &phase.goal,
            profile: &plan.profile,
            style: &plan.style,
            intent: WorkIntent::parse(&plan.intent).unwrap_or(WorkIntent::Unknown),
            required_artifacts: &phase_artifacts,
            profile_obligations: &phase_contract.profile_obligations,
            task_contract: Some(&phase_task_contract),
            save_kind: "phase-step-plan",
            prompt_kind: "phase step plan",
        };
        let mut step_plan =
            runtime.parse_generated_step_plan_with_corrections(text, correction_context)?;
        step_plan = prune_out_of_phase_mutation_steps(step_plan, &phase_artifacts);
        if step_plan_has_nextjs_build_verifier(&step_plan) {
            nextjs_build_verifier_seen = true;
        }
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
                    runtime,
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
        if let Err(err) =
            verify_phase_profile(runtime, plan, phase, &step_plan, &phase_contract, observer)
        {
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

    let authority = final_required_artifact_authority(runtime.cwd, &plan.required_artifacts);
    let missing = authority.missing_deliverables.clone();
    emit_final_artifact_status(observer, &plan.required_artifacts, &missing);
    if !authority.success_eligible() {
        if !missing.is_empty() {
            return Err(format!(
                "missing required final artifacts: {}\nContract completion evidence:\n{}",
                missing.join(", "),
                authority.render_contract_lines().join("\n")
            ));
        }
        return Err(format!(
            "final completion evidence failed: {}\nContract completion evidence:\n{}",
            authority.terminal_state(),
            authority.render_contract_lines().join("\n")
        ));
    }
    if let Some(smoke_report) = verify_requested_dev_server_contract(
        runtime.cwd,
        &plan.profile,
        &ultra_plan_requested_text(plan),
        nextjs_build_verifier_seen,
    )? {
        lines.push(smoke_report);
    }

    Ok(lines.join("\n"))
}

fn prune_out_of_phase_mutation_steps(mut plan: StepPlan, phase_artifacts: &[String]) -> StepPlan {
    if phase_artifacts.is_empty() {
        return plan;
    }
    plan.required_artifacts = phase_artifacts.to_vec();
    plan.steps.retain(|step| {
        if !matches!(
            step.kind,
            StepKind::Create | StepKind::Edit | StepKind::Repair | StepKind::Setup
        ) {
            return true;
        }
        step.expected_paths.is_empty()
            || step
                .expected_paths
                .iter()
                .any(|path| phase_artifacts.iter().any(|artifact| artifact == path))
    });
    plan
}

pub(super) fn step_plan_has_nextjs_build_verifier(plan: &StepPlan) -> bool {
    plan.steps
        .iter()
        .flat_map(|step| step.verify.iter())
        .any(|command| command.trim().eq_ignore_ascii_case("npm run build"))
}

pub(super) fn verify_requested_dev_server_contract(
    cwd: &Path,
    profile: &str,
    requested_text: &str,
    build_verifier_seen: bool,
) -> Result<Option<String>, String> {
    if !build_verifier_seen {
        return Ok(None);
    }
    let Some(requested_port) = requested_dev_port(profile, requested_text) else {
        return Ok(None);
    };
    let report = verify_nextjs_dev_server_smoke(cwd, requested_port);
    let rendered = report.render_lines().join("\n");
    if report.is_ok() {
        Ok(Some(format!("dev-server smoke: ok\n{rendered}")))
    } else {
        Err(format!("dev-server smoke failed:\n{rendered}"))
    }
}

fn ultra_plan_requested_text(plan: &UltraPlan) -> String {
    let mut text = String::new();
    text.push_str(&plan.goal);
    for artifact in &plan.required_artifacts {
        text.push(' ');
        text.push_str(artifact);
    }
    for phase in &plan.phases {
        text.push(' ');
        text.push_str(&phase.goal);
    }
    text
}

fn final_required_artifact_authority(
    cwd: &Path,
    required_artifacts: &[String],
) -> CompletionAuthorityResult {
    let mut graph = ArtifactGraph::new();
    for path in required_artifacts {
        let lifecycle = if cwd.join(path).exists() {
            ArtifactLifecycle::Existing
        } else {
            ArtifactLifecycle::Required
        };
        graph.add_path(path, lifecycle, "ultra_plan.required_artifacts");
    }
    let scope = WorkspaceScope::from_graph(&graph);
    let mut ledger = ArtifactLedgerSummary::from_tool_records(&[], &graph, &scope);
    for path in required_artifacts {
        if cwd.join(path).exists() {
            ledger.record_workspace_observation(path, &graph, &scope);
        }
    }
    let produced = produce_completion_evidence(&EvidenceProducerInput {
        step_id: "final-required-artifacts",
        profile: "final",
        required_paths: required_artifacts,
        verifier_commands: &[],
        verifier_failures: &[],
        ledger: &ledger,
        observed_completion_facts: &[],
        observed_bindings: &[],
    });
    evaluate_completion_authority(
        required_artifacts,
        &ledger,
        &produced.completion_evidence,
        &produced.evidence_bindings,
    )
}

fn verify_phase_profile<E, P>(
    runtime: &mut SlashRuntime<'_, E, P>,
    plan: &UltraPlan,
    phase: &crate::agent::step_runner::ultra_plan::UltraPhase,
    step_plan: &StepPlan,
    phase_contract: &PhaseWorkspaceContract,
    observer: &mut dyn RuntimeObserver,
) -> Result<(), String>
where
    E: ChatClient,
    P: ChatClient,
{
    let cwd = runtime.cwd;
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
    let repair_context = ProfileRepairContext {
        phase_id: phase.id.clone(),
        original_goal: plan.goal.clone(),
        phase_goal: phase.goal.clone(),
        profile: plan.profile.clone(),
        style: plan.style.clone(),
        profile_failures: failures.clone(),
        phase_contract_facts: phase_contract.fact_lines(),
        profile_facts,
        expected_paths,
    };
    if let Some(targets) = profile_auto_repair_targets(cwd, &failures) {
        observer.on_event(RuntimeEvent::RepairAttemptStarted {
            step_id: bounded_event_text(&phase.id),
            attempt: 1,
            max_attempts: 1,
            missing_expected_paths: missing_paths(cwd, &targets),
        });
        observer.on_event(RuntimeEvent::RecoveryTaskStarted {
            step_id: bounded_event_text(&phase.id),
            attempt: 1,
            active_job: "profile_contract_repair".to_string(),
            dispatch_status: "selected".to_string(),
            execution_envelope: Some("file_mutation_repair".to_string()),
            target_path: targets.first().map(bounded_event_text),
        });
        let mut repair_config = profile_repair_loop_config(cwd, &runtime.loop_config, &targets);
        repair_config.max_iterations = repair_config.max_iterations.min(6);
        let prompt = build_profile_replan_packet(&repair_context);
        if let Err(err) = crate::agent::minimal_loop::loop_run::run_session_with_observer(
            runtime.executor,
            cwd,
            &prompt,
            repair_config,
            observer,
        ) {
            return save_profile_repair_stop(
                cwd,
                &repair_context,
                &rendered,
                Some(err.to_string()),
            );
        }
        let after_failures =
            verify_profile(&plan.profile, cwd, &context).map_err(|err| err.to_string())?;
        if after_failures.is_empty() {
            return Ok(());
        }
        let after_rendered = after_failures
            .iter()
            .map(|failure| bounded_event_text(failure.render()))
            .collect::<Vec<_>>();
        observer.on_event(RuntimeEvent::ProfileVerificationFailed {
            profile: bounded_event_text(&plan.profile),
            failures: after_rendered.clone(),
        });
        let repeated =
            profile_failure_signature(&failures) == profile_failure_signature(&after_failures);
        let reason = if repeated {
            "profile verification repair stopped after one bounded attempt because the same failure signature remained"
        } else {
            "profile verification repair was attempted once but profile verification still failed"
        };
        let after_context = ProfileRepairContext {
            profile_failures: after_failures,
            ..repair_context
        };
        return save_profile_repair_stop(cwd, &after_context, &after_rendered, Some(reason.into()));
    }
    save_profile_repair_stop(
        cwd,
        &repair_context,
        &rendered,
        Some("profile verification failure was not safe for automatic repair".to_string()),
    )
}

fn profile_repair_loop_config(
    cwd: &Path,
    base: &MinimalLoopConfig,
    targets: &[String],
) -> MinimalLoopConfig {
    let mut config = base.clone();
    config.expected_artifacts = targets.to_vec();
    config.action_requirement = ActionRequirement::Required;
    config.step_tool_policy = if targets.iter().all(|target| cwd.join(target).exists()) {
        StepToolPolicy::EditExistingArtifactOnly
    } else {
        StepToolPolicy::CreateMissingArtifactOnly
    };
    config
}

fn profile_auto_repair_targets(
    cwd: &Path,
    failures: &[crate::agent::step_runner::profiles::ProfileVerificationFailure],
) -> Option<Vec<String>> {
    if failures.len() != 1 {
        return None;
    }
    let target = failures.first()?.paths.first()?.clone();
    if target.trim().is_empty()
        || target.split('/').any(|part| {
            matches!(
                part,
                ".git" | "node_modules" | "target" | ".next" | "dist" | "build"
            )
        })
    {
        return None;
    }
    PathGuard::new(cwd).ok()?.resolve(&target).ok()?;
    Some(vec![target])
}

fn profile_failure_signature(
    failures: &[crate::agent::step_runner::profiles::ProfileVerificationFailure],
) -> String {
    failures
        .iter()
        .map(|failure| format!("{}:{}", failure.code, failure.paths.join("|")))
        .collect::<Vec<_>>()
        .join(";")
}

fn save_profile_repair_stop(
    cwd: &Path,
    context: &ProfileRepairContext,
    rendered: &[String],
    reason: Option<String>,
) -> Result<(), String> {
    let saved = save_profile_repair_prompt(cwd, context).map_err(|err| err.to_string())?;
    let reason = reason
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("\nstop reason: {value}"))
        .unwrap_or_default();
    Err(format!(
        "profile verification failed for {} after phase {}: {}.{reason}\nprofile repair prompt saved: {}\nsuggested command: {}\nRun an explicit repair or replan command before continuing the ultra plan.",
        context.profile,
        context.phase_id,
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
    let plan_artifacts = if plan.required_artifacts.is_empty() {
        plan.steps
            .iter()
            .flat_map(|step| step.expected_paths.clone())
            .collect::<Vec<_>>()
    } else {
        plan.required_artifacts.clone()
    };
    let plan_initial_missing_artifacts = missing_paths(runtime.cwd, &plan_artifacts);
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
        if let Err(err) = execute_step(
            runtime,
            plan,
            step,
            contract_seed,
            &plan_initial_missing_artifacts,
            observer,
        ) {
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
    plan_initial_missing_artifacts: &[String],
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
    config.initial_missing_artifacts = plan_initial_missing_artifacts.to_vec();
    config.action_requirement = action_requirement_for_step(step, &missing_expected_paths);
    if matches!(step.kind, StepKind::Verify) && !step.verify.is_empty() {
        let failures = verify_step_with_observer(runtime.cwd, step, observer)?;
        if failures.is_empty() {
            ensure_step_completion_authority(runtime.cwd, plan, step, &[], &failures)?;
            return Ok(());
        }
        if step_accepts_verifier_failure(step) {
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
                dependency_setup_attempt_keys: Vec::new(),
                dependency_setup_note: None,
                setup_job_state: Vec::new(),
                tool_records: Vec::new(),
                contract_evidence: Vec::new(),
                repair_attempt_ledger: Vec::new(),
                repair_job_state: crate::agent::step_runner::repair_job::RepairJobState::new(
                    "unknown",
                )
                .with_step_id(step.id.clone()),
                tool_arg_schema_correction_spent: false,
                pending_tool_protocol_failure: None,
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
                if completion_probe_after_blocked_bash(runtime.cwd, step, &err, observer)? {
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
            if failures.is_empty() || step_accepts_verifier_failure(step) {
                if failures.is_empty() {
                    ensure_step_completion_authority(runtime.cwd, plan, step, &[], &failures)?;
                }
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
    if failures.is_empty() {
        ensure_step_completion_authority(runtime.cwd, plan, step, &result.tool_results, &failures)?;
        return Ok(());
    }
    if step_accepts_verifier_failure(step) {
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

fn ensure_step_completion_authority(
    cwd: &Path,
    plan: &StepPlan,
    step: &StepPlanStep,
    tool_records: &[ToolExecutionRecord],
    verifier_failures: &[VerificationFailure],
) -> Result<(), String> {
    let authority = step_completion_authority(cwd, plan, step, tool_records, verifier_failures);
    if authority.success_eligible() {
        Ok(())
    } else {
        Err(completion_authority_error(step, &authority))
    }
}

fn step_completion_authority(
    cwd: &Path,
    plan: &StepPlan,
    step: &StepPlanStep,
    tool_records: &[ToolExecutionRecord],
    verifier_failures: &[VerificationFailure],
) -> CompletionAuthorityResult {
    let mut graph = ArtifactGraph::new();
    for path in &step.expected_paths {
        let lifecycle = if cwd.join(path).exists() {
            ArtifactLifecycle::Existing
        } else {
            ArtifactLifecycle::Required
        };
        graph.add_path(path, lifecycle, &step.id);
    }
    let scope = WorkspaceScope::from_graph(&graph);
    let mut ledger = ArtifactLedgerSummary::from_tool_records(tool_records, &graph, &scope);
    for path in &step.expected_paths {
        if cwd.join(path).exists() {
            ledger.record_workspace_observation(path, &graph, &scope);
        }
    }
    let produced = produce_completion_evidence(&EvidenceProducerInput {
        step_id: &step.id,
        profile: &plan.profile,
        required_paths: &step.expected_paths,
        verifier_commands: &step.verify,
        verifier_failures,
        ledger: &ledger,
        observed_completion_facts: &[],
        observed_bindings: &[],
    });
    evaluate_completion_authority(
        &step.expected_paths,
        &ledger,
        &produced.completion_evidence,
        &produced.evidence_bindings,
    )
}

fn completion_authority_error(
    step: &StepPlanStep,
    authority: &CompletionAuthorityResult,
) -> String {
    format!(
        "step completion authority rejected {}: {}\nContract completion evidence:\n{}",
        step.id,
        authority.terminal_state(),
        authority.render_contract_lines().join("\n")
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
        StepKind::Create => StepToolPolicy::CreateMissingArtifactOnly,
        StepKind::Edit => StepToolPolicy::EditExistingArtifactOnly,
        StepKind::Repair => StepToolPolicy::FileMutationWithReadOnlyBash,
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
        | MinimalLoopError::MissingArtifacts(_)
        | MinimalLoopError::ProgressBudgetExhausted(_) => false,
    }
}

fn completion_probe_after_blocked_bash(
    cwd: &Path,
    step: &StepPlanStep,
    error: &crate::agent::minimal_loop::result::MinimalLoopError,
    observer: &mut dyn RuntimeObserver,
) -> Result<bool, String> {
    if worker_control_signal_after_turn_error(cwd, step, error)
        != WorkerControlSignal::RequestVerifierTransition
    {
        return Ok(false);
    }
    let failures = verify_step_with_observer(cwd, step, observer)?;
    Ok(failures.is_empty() || step_accepts_verifier_failure(step))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkerControlSignal {
    ContinueExecution,
    RequestVerifierTransition,
    ReportConcreteBlocker,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BlockedBashTurn {
    command_class: String,
    command: Option<String>,
}

fn worker_control_signal_after_turn_error(
    cwd: &Path,
    step: &StepPlanStep,
    error: &crate::agent::minimal_loop::result::MinimalLoopError,
) -> WorkerControlSignal {
    let Some(blocked) = blocked_bash_turn_error(error) else {
        return WorkerControlSignal::ContinueExecution;
    };
    if !missing_paths(cwd, &step.expected_paths).is_empty() {
        return WorkerControlSignal::ReportConcreteBlocker;
    }
    if blocked.is_canonical_verifier_request(step) {
        WorkerControlSignal::RequestVerifierTransition
    } else {
        WorkerControlSignal::ReportConcreteBlocker
    }
}

fn blocked_bash_turn_error(
    error: &crate::agent::minimal_loop::result::MinimalLoopError,
) -> Option<BlockedBashTurn> {
    use crate::agent::minimal_loop::result::MinimalLoopError;

    let MinimalLoopError::Tool(message) = error else {
        return None;
    };
    let detail = message
        .strip_prefix("tool_policy_violation: ")
        .unwrap_or(message);
    if let Some(rest) = detail.strip_prefix("Bash command is not read-only for this step: ") {
        let command_class = value_after(rest, "class=")
            .and_then(|value| value.split(';').next())
            .unwrap_or("unknown");
        return Some(BlockedBashTurn {
            command_class: normalize_command_class(command_class),
            command: value_after(rest, "command=").map(normalize_shell_command),
        });
    }
    let rest = detail.strip_prefix("bash command blocked as ")?;
    let (command_class, tail) = rest.split_once(':')?;
    Some(BlockedBashTurn {
        command_class: normalize_command_class(command_class),
        command: value_after(tail, "command=").map(normalize_shell_command),
    })
}

impl BlockedBashTurn {
    fn is_canonical_verifier_request(&self, step: &StepPlanStep) -> bool {
        if !matches!(self.command_class.as_str(), "build_test" | "script_run") {
            return false;
        }
        let Some(command) = &self.command else {
            return false;
        };
        step.verify
            .iter()
            .any(|verify| normalize_shell_command(verify) == *command)
    }
}

fn value_after<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.split_once(key).map(|(_, value)| value.trim())
}

fn normalize_shell_command(command: &str) -> String {
    command.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_command_class(value: &str) -> String {
    match value.trim() {
        "ReadOnly" => "read_only".to_string(),
        "ScriptRun" => "script_run".to_string(),
        "BuildTest" => "build_test".to_string(),
        "DirectoryCreation" => "directory_creation".to_string(),
        "EnvSetup" => "env_setup".to_string(),
        other => other.to_ascii_lowercase(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::minimal_loop::result::MinimalLoopError;
    use crate::agent::step_runner::ExpectedResult;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn worker_build_test_before_artifact_reports_concrete_blocker() {
        let root = temp_workspace("worker-build-before-artifact");
        let step = step(&["README.md"], &["cargo test"]);
        let error = read_only_bash_error(
            "BuildTest",
            "build/test Bash is verifier-owned",
            "cargo test",
        );

        let signal = worker_control_signal_after_turn_error(&root, &step, &error);

        assert_eq!(signal, WorkerControlSignal::ReportConcreteBlocker);
    }

    #[test]
    fn worker_build_test_after_artifact_requests_verifier_transition() {
        let root = temp_workspace("worker-build-after-artifact");
        fs::write(root.join("README.md"), "ok\n").unwrap();
        let step = step(&["README.md"], &["cargo test"]);
        let error = read_only_bash_error(
            "BuildTest",
            "build/test Bash is verifier-owned",
            "cargo test",
        );

        let signal = worker_control_signal_after_turn_error(&root, &step, &error);

        assert_eq!(signal, WorkerControlSignal::RequestVerifierTransition);
    }

    #[test]
    fn compound_read_check_does_not_request_verifier_transition() {
        let root = temp_workspace("worker-compound-read-check");
        fs::write(root.join("README.md"), "ok\n").unwrap();
        let step = step(&["README.md"], &["cat README.md"]);
        let error = read_only_bash_error(
            "Unknown",
            "compound shell commands, pipes, redirects, and shell substitutions are blocked",
            "test -f README.md && echo exists",
        );

        let signal = worker_control_signal_after_turn_error(&root, &step, &error);

        assert_eq!(signal, WorkerControlSignal::ReportConcreteBlocker);
    }

    #[test]
    fn noncanonical_build_test_does_not_request_verifier_transition() {
        let root = temp_workspace("worker-noncanonical-build");
        fs::write(root.join("README.md"), "ok\n").unwrap();
        let step = step(&["README.md"], &["cargo test --all"]);
        let error = read_only_bash_error(
            "BuildTest",
            "build/test Bash is verifier-owned",
            "cargo test",
        );

        let signal = worker_control_signal_after_turn_error(&root, &step, &error);

        assert_eq!(signal, WorkerControlSignal::ReportConcreteBlocker);
    }

    fn step(expected_paths: &[&str], verify: &[&str]) -> StepPlanStep {
        StepPlanStep {
            id: "step".to_string(),
            kind: StepKind::Create,
            instruction: "create artifact".to_string(),
            expected_result: ExpectedResult::Pass,
            expected_paths: expected_paths
                .iter()
                .map(|path| (*path).to_string())
                .collect(),
            verify: verify
                .iter()
                .map(|command| (*command).to_string())
                .collect(),
        }
    }

    fn read_only_bash_error(class: &str, reason: &str, command: &str) -> MinimalLoopError {
        MinimalLoopError::Tool(format!(
            "tool_policy_violation: Bash command is not read-only for this step: class={class}; reason={reason}; command={command}"
        ))
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("commandagent-{name}-{stamp}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
