use super::*;
use crate::planner::step_plan::StepKind;

pub(super) fn begin(
    config: &Config,
    plan: &UltraPlan,
) -> anyhow::Result<(
    Vec<String>,
    Option<crate::planner::recovery_contract_authority::RunAuthorityGuard>,
)> {
    let authority = crate::planner::recovery_contract_authority::enter_run(config);
    let paths = resolve_profile_runtime(&plan.profile)
        .expected_scaffold_paths(&config.workspace_root, &plan.goal);
    initialize(config, plan, &paths)?;
    Ok((paths, authority))
}

pub(super) fn register_plan(config: &Config, plan: &StepPlan) -> anyhow::Result<()> {
    crate::planner::recovery_contract_authority::register_step_plan_commands(
        config,
        &plan
            .steps
            .iter()
            .filter(|step| {
                step.expected_result_kind() == crate::planner::step_plan::ExpectedResult::Pass
                    && matches!(step.step_kind(), StepKind::Implement | StepKind::Verify)
            })
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

#[cfg(test)]
#[path = "flow/recovery_authority_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "flow/issue435_tests.rs"]
mod issue435_tests;

#[cfg(test)]
#[path = "flow/issue439_tests.rs"]
mod issue439_tests;
