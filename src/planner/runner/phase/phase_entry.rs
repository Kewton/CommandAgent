//! Resolve, persist and register a phase plan before executing any steps.
use super::*;

pub(super) fn start(
    config: &Config,
    plan: &UltraPlan,
    context: &mut UltraRunContext,
) -> anyhow::Result<pipeline::PhaseRun> {
    let machine = pipeline::PhaseRun::start()?;
    #[cfg(test)]
    let machine = {
        let mut machine = machine;
        issue490_tests::restore(config, plan, context, &mut machine)?;
        machine
    };
    let _ = (config, plan, context);
    Ok(machine)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn prepare(
    planner: &mut dyn ChatClient,
    config: &Config,
    plan: &UltraPlan,
    phase: &UltraPhase,
    index: usize,
    ui: &dyn InteractionUi,
    ultra_context: &UltraRunContext,
    ultra_session: &SessionSnapshot,
    phase_machine: &mut pipeline::PhaseRun,
    mut fix_runtime: Option<&mut crate::planner::fix_runtime::FixRuntime>,
    investigation_runtime: Option<&mut crate::planner::investigation_runtime::InvestigationRuntime>,
    final_expected_paths: &[String],
    preset_plan: bool,
) -> anyhow::Result<Option<(StepPlan, crate::planner::profile::ProfileSnapshot)>> {
    #[cfg(test)]
    if issue490_tests::skip_setup(index) {
        return Ok(None);
    }
    if ui.interrupted() {
        phase_machine.interrupt("phase_start")?;
        anyhow::bail!("interrupted by user");
    }
    emit_ultra_phase_event(
        config,
        "ultra_phase_start",
        plan,
        phase,
        index,
        "start",
        None,
        None,
        None,
    );
    let profile_snapshot = profile_before_plan(&config.workspace_root, plan)?;
    ultra_context.emit_attached(config, plan, phase, index, ultra_session);
    phase_machine.phase_started()?;
    let final_phase = index + 1 == plan.phases.len();
    let phase_prompt =
        ultra_phase_prompt(plan, phase, config, ultra_context, fix_runtime.as_deref());
    let phase_prompt = crate::planner::pack::runtime::append_phase_material_from_environment(
        phase_prompt,
        &config.workspace_root,
        &plan.profile,
        &plan.intent,
        &phase.id,
    )?;
    let step_plan_result = phase_machine.resolve(
        planner,
        &phase_prompt,
        config,
        ui,
        phase,
        plan,
        fix_runtime.as_deref(),
        preset_plan,
        final_phase,
    );
    let mut step_plan = step_plan_result.map_err(|err| {
        let rejected_verify_commands =
            crate::planner::lint_rejection::rejected_commands_from_error(&err);
        let message = err.to_string();
        emit_ultra_phase_event(
            config,
            "ultra_phase_failed",
            plan,
            phase,
            index,
            "scaffold",
            Some(false),
            Some(&message),
            None,
        );
        emit_planner_error(
            config,
            planner.label(),
            &config.planner_model,
            "scaffold",
            "phase_scaffold_error",
            &format!("phase scaffold failed: {}", message),
            index + 1,
        );
        let handoff = save_ultra_phase_recovery_handoff(
            config,
            plan,
            phase,
            UltraPhaseRecoveryRequest {
                failure_kind: "phase_scaffold_error",
                reason: &message,
                missing_paths: &missing_final_artifacts(
                    &config.workspace_root,
                    final_expected_paths,
                ),
                missing_signals: &[],
                repair_targets: &["phase_scaffold".to_string()],
                verify_commands: &rejected_verify_commands,
            },
        );
        anyhow::anyhow!(
            "{}",
            render_failure_stop_reason(format!("phase scaffold failed: {message}"), handoff,)
        )
    })?;
    phase_machine.plan_resolved()?;
    crate::planner::fix_runtime::bind_step_plan(fix_runtime.as_deref_mut(), phase, &mut step_plan);
    emit_ultra_phase_event(
        config,
        "ultra_phase_scaffold_complete",
        plan,
        phase,
        index,
        "scaffold",
        Some(true),
        None,
        Some(step_plan.steps.len()),
    );
    emit_ultra_phase_event(
        config,
        "ultra_phase_plan_validated",
        plan,
        phase,
        index,
        "lint",
        Some(true),
        None,
        Some(step_plan.steps.len()),
    );
    save_step_plan(&config.workspace_root, &step_plan)?;
    #[cfg(test)]
    issue490_tests::returned_and_saved(config, &step_plan);
    let fix_before = fix_runtime
        .as_deref()
        .is_some_and(|runtime| runtime.is_before_phase(index));
    let investigation_before = investigation_runtime
        .as_deref()
        .is_some_and(|runtime| runtime.is_reproducer_phase(index));
    phase_machine.plan_persisted(fix_before, investigation_before)?;
    let Some(step_plan) = before_phase::run(
        planner,
        fix_runtime,
        investigation_runtime,
        &phase_prompt,
        step_plan,
        config,
        plan,
        phase,
        index,
        ui,
        preset_plan,
        final_phase,
    )?
    else {
        phase_machine.before_phase_completed(true, final_phase)?;
        return Ok(None);
    };
    phase_machine.before_phase_completed(false, final_phase)?;
    recovery_authority::register_plan(config, &step_plan)?;
    #[cfg(test)]
    if issue490_tests::registered(config, &step_plan) {
        anyhow::bail!("issue490: registered before execution");
    }
    Ok(Some((step_plan, profile_snapshot)))
}

#[cfg(test)]
#[path = "issue490_tests.rs"]
mod issue490_tests;
