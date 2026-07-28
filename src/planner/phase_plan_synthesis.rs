use crate::config::Config;
use crate::planner::fix_runtime::FixRuntime;
use crate::planner::step_plan::StepPlan;
use crate::planner::ultra_plan::{UltraPhase, UltraPlan};

pub(crate) fn resolve(
    config: &Config,
    plan: &UltraPlan,
    phase: &UltraPhase,
    fix_runtime: Option<&FixRuntime>,
    fallback: impl FnOnce() -> anyhow::Result<StepPlan>,
) -> anyhow::Result<StepPlan> {
    crate::planner::ingest_plan_synthesis::resolve_phase_plan(config, plan, phase, || {
        crate::planner::investigation_plan_synthesis::resolve_phase_plan(
            config,
            plan,
            phase,
            || {
                crate::planner::fix_plan_synthesis::resolve_phase_plan(
                    config,
                    plan,
                    phase,
                    fix_runtime,
                    fallback,
                )
            },
        )
    })
}
