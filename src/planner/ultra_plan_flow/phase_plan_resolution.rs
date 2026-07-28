use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve(
    planner: &mut dyn ChatClient,
    phase_prompt: &str,
    config: &Config,
    ui: &dyn InteractionUi,
    phase: &UltraPhase,
    plan: &UltraPlan,
    fix_runtime: Option<&crate::planner::fix_runtime::FixRuntime>,
    preset_plan: bool,
    final_phase: bool,
) -> anyhow::Result<StepPlan> {
    crate::planner::phase_plan_synthesis::resolve(config, plan, phase, fix_runtime, || {
        generate_step_plan_with_ui_for_phase(
            planner,
            phase_prompt,
            config,
            ui,
            Some(&phase.id),
            preset_plan,
            final_phase,
        )
    })
}
