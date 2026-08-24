use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    planner: &mut dyn ChatClient,
    fix_runtime: Option<&mut crate::planner::fix_runtime::FixRuntime>,
    investigation_runtime: Option<&mut crate::planner::investigation_runtime::InvestigationRuntime>,
    phase_prompt: &str,
    step_plan: StepPlan,
    config: &Config,
    plan: &UltraPlan,
    phase: &UltraPhase,
    index: usize,
    ui: &dyn InteractionUi,
    preset_plan: bool,
    final_phase: bool,
) -> anyhow::Result<Option<StepPlan>> {
    if let Some(runtime) = fix_runtime
        && runtime.is_before_phase(index)
    {
        super::fix_before::run(
            planner,
            runtime,
            phase_prompt,
            step_plan,
            config,
            plan,
            phase,
            index,
            ui,
            preset_plan,
            final_phase,
        )?;
        return Ok(None);
    }
    if let Some(runtime) = investigation_runtime
        && runtime.is_reproducer_phase(index)
    {
        super::investigation_before::run(
            planner,
            runtime,
            phase_prompt,
            &step_plan,
            config,
            plan,
            phase,
            index,
            ui,
            preset_plan,
            final_phase,
        )?;
        return Ok(None);
    }
    Ok(Some(step_plan))
}
