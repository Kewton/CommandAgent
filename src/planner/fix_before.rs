use super::*;
use crate::planner::fix_reproducer_defect::BeforePhaseOutcome;

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    planner: &mut dyn ChatClient,
    runtime: &mut crate::planner::fix_runtime::FixRuntime,
    phase_prompt: &str,
    step_plan: StepPlan,
    config: &Config,
    plan: &UltraPlan,
    phase: &UltraPhase,
    index: usize,
    ui: &dyn InteractionUi,
    preset_plan: bool,
    final_phase: bool,
) -> anyhow::Result<()> {
    let BeforePhaseOutcome::RebuildRequired { feedback } =
        runtime.run_before_phase(&step_plan, config, plan, phase, index)?
    else {
        return Ok(());
    };
    let retry_prompt =
        crate::planner::fix_reproducer_defect::rebuild_prompt(phase_prompt, &feedback);
    let retry_plan = generate_step_plan_with_ui_for_phase(
        planner,
        &retry_prompt,
        config,
        ui,
        Some(&phase.id),
        preset_plan,
        final_phase,
    )?;
    let mut retry_plan = crate::planner::fix_plan_synthesis::canonicalize_model_reproducer(
        config, plan, phase, retry_plan,
    )?;
    crate::planner::fix_runtime::bind_step_plan(Some(runtime), phase, &mut retry_plan);
    save_step_plan(&config.workspace_root, &retry_plan)?;
    match runtime.run_before_phase(&retry_plan, config, plan, phase, index)? {
        BeforePhaseOutcome::Confirmed => Ok(()),
        BeforePhaseOutcome::RebuildRequired { feedback } => {
            anyhow::bail!("fix reproducer rebuild exhausted: {feedback}")
        }
    }
}
