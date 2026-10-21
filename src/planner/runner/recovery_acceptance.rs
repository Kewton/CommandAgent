use crate::config::Config;
use crate::minimal_loop::evidence::RuntimeAcceptanceReport;
use crate::planner::ultra_plan::UltraPlan;

pub(crate) fn runtime_acceptance_report(
    plan: &UltraPlan,
    config: &Config,
) -> anyhow::Result<RuntimeAcceptanceReport> {
    super::adjudication_create::ultra_contract_runtime_acceptance_report(plan, config)
}

#[cfg(test)]
pub(crate) fn final_acceptance_report(
    plan: &UltraPlan,
    config: &Config,
) -> anyhow::Result<crate::planner::verify::VerificationReport> {
    super::final_acceptance::ultra_final_acceptance_report(plan, config)
}
