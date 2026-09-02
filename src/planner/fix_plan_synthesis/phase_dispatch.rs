use anyhow::Context;

use super::PhasePlan;
use crate::config::Config;
use crate::planner::fix_runtime::FixRuntime;
use crate::planner::ultra_plan::{UltraPhase, UltraPlan};

pub(crate) fn phase_plan(
    config: &Config,
    plan: &UltraPlan,
    phase: &UltraPhase,
    runtime: Option<&FixRuntime>,
) -> anyhow::Result<PhasePlan> {
    if super::is_fix_intent(&plan.intent)
        && plan
            .phases
            .first()
            .is_some_and(|first| first.id == phase.id)
        && let Some(suggestion) =
            crate::planner::fix_reproducer::completion_contract_suggestion(config)?
    {
        super::ensure_contract_shape(plan)?;
        let candidate =
            crate::planner::external_reproducer::resolve_candidate(config, Some(suggestion))
                .map_err(anyhow::Error::msg)?
                .context("completion contract reproducer candidate was empty")?;
        super::emit_synthesized(config, plan, &candidate.0);
        return super::generated(
            config,
            phase,
            super::reproducer_plan(&plan.goal, candidate.1),
        );
    }
    if !super::applies(config, plan) {
        return Ok(PhasePlan::NotApplicable);
    }
    super::ensure_contract_shape(plan)?;
    match phase.id.as_str() {
        "reproduce-before" => super::reproduce_before(config, plan, phase),
        "isolate-cause" => {
            super::generated(config, phase, super::isolate_cause(config, plan, runtime)?)
        }
        "repair" => super::generated(config, phase, super::implement_fix(config, plan, runtime)?),
        "verify-regressions" => {
            super::generated(config, phase, super::verify_after(config, plan, runtime)?)
        }
        other => anyhow::bail!("unsupported synthesized data fix phase: {other}"),
    }
}
