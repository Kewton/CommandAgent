use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    planner: &mut dyn ChatClient,
    runtime: &mut crate::planner::investigation_runtime::InvestigationRuntime,
    phase_prompt: &str,
    step_plan: &StepPlan,
    config: &Config,
    plan: &UltraPlan,
    phase: &UltraPhase,
    index: usize,
    ui: &dyn InteractionUi,
    preset_plan: bool,
    final_phase: bool,
) -> anyhow::Result<()> {
    let outcome = runtime.run_reproducer_phase(step_plan, config, plan, phase, index)?;
    let crate::planner::investigation_runtime::InvestigationBeforeOutcome::RebuildRequired {
        feedback,
    } = outcome
    else {
        return Ok(());
    };
    let retry_prompt = format!(
        "{phase_prompt}\n\nInvestigation contract deterministic reproducer rebuild feedback:\n{feedback}"
    );
    let mut retry_plan = generate_step_plan_with_ui_for_phase(
        planner,
        &retry_prompt,
        config,
        ui,
        Some(&phase.id),
        preset_plan,
        final_phase,
    )?;
    let report = crate::planner::step_plan_finalize::finalize_step_plan_for_execution(
        &mut retry_plan,
        config,
    );
    if !report.is_pass() {
        anyhow::bail!(
            "investigation reproducer rebuild failed lint: {}",
            report.primary_message()
        );
    }
    save_step_plan(&config.workspace_root, &retry_plan)?;
    if matches!(
        runtime.run_reproducer_phase(&retry_plan, config, plan, phase, index)?,
        crate::planner::investigation_runtime::InvestigationBeforeOutcome::RebuildRequired { .. }
    ) {
        anyhow::bail!("investigation reproducer rebuild exhausted");
    }
    Ok(())
}
