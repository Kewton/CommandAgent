use super::*;

pub(super) fn register_plan(config: &Config, plan: &StepPlan) -> anyhow::Result<()> {
    crate::planner::recovery_contract_authority::register_step_plan_commands(
        config,
        &plan
            .steps
            .iter()
            .flat_map(|step| step.verify.iter().cloned())
            .collect::<Vec<_>>(),
    )
}

pub(super) fn initialize(
    config: &Config,
    plan: &UltraPlan,
    paths: &[String],
) -> anyhow::Result<()> {
    if plan.intent != "create" {
        return Ok(());
    }
    let profile = ProfileId::parse(&plan.profile);
    let runtime = ProfileRuntimeRegistry::resolve(&profile);
    let capabilities = runtime.required_capabilities(&plan.goal);
    super::super::bind_completion_contract_for_acceptance(
        config,
        "ultra-plan-run",
        &plan.profile,
        &plan.goal,
        paths,
        &capabilities,
        &runtime.required_evidence(&plan.goal, &capabilities),
        &runtime.required_obligations(&profile, &plan.goal, &capabilities),
    )?;
    Ok(())
}
