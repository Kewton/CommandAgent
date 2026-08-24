use serde_json::json;

use crate::config::{Config, PlanPreset};
use crate::planner::adjudication::contract::IntentId;
use crate::planner::step_plan::{PlanStep, StepPlan};
use crate::planner::ultra_plan::{UltraPhase, UltraPlan};

mod guidance;
mod observed_failure;

const PHASE_IDS: [&str; 3] = ["reproduce-candidate", "diagnose", "bind-verify"];

pub(crate) fn resolve_phase_plan(
    config: &Config,
    plan: &UltraPlan,
    phase: &UltraPhase,
    fallback: impl FnOnce() -> anyhow::Result<StepPlan>,
) -> anyhow::Result<StepPlan> {
    if !applies(config, plan) {
        return fallback();
    }
    ensure_shape(plan)?;
    let suggestion = crate::planner::external_reproducer::resolve_candidate(
        config,
        investigation_reproducer_suggestion(plan),
    )
    .map_err(anyhow::Error::msg)?;
    let basis = suggestion
        .as_ref()
        .map(|suggestion| suggestion.0.as_str())
        .unwrap_or("model_required");
    if phase.id == "reproduce-candidate" {
        emit_synthesized(config, plan, basis);
    }
    let mut step_plan = match phase.id.as_str() {
        "reproduce-candidate" => {
            let Some((_, command)) = suggestion else {
                return fallback();
            };
            StepPlan {
                goal: plan.goal.clone(),
                steps: vec![PlanStep {
                    id: "reproduce-candidate".into(),
                    kind: "verify".into(),
                    expected_result: "fail".into(),
                    instruction: "Execute deterministic reproducer R at stage=diagnosis before diagnosis generation; one reproducer_defect rebuild is permitted before I1 is established.".into(),
                    expected_paths: Vec::new(),
                    verify: vec![command],
                }],
            }
        }
        "diagnose" => StepPlan {
            goal: plan.goal.clone(),
            steps: vec![PlanStep {
                id: "diagnose".into(),
                kind: "implement".into(),
                expected_result: "pass".into(),
                instruction: guidance::diagnose_instruction(config, &plan.goal),
                expected_paths: vec!["output/diagnosis.md".into()],
                verify: vec!["test -f output/diagnosis.md".into()],
            }],
        },
        "bind-verify" => {
            execute_binding_if_ready(config)?;
            StepPlan {
                goal: plan.goal.clone(),
                steps: vec![PlanStep {
                    id: "bind-verify".into(),
                    kind: "verify".into(),
                    expected_result: "pass".into(),
                    instruction: "Execute the deterministic I2 diagnosis binding verifier and write evidence/investigation-binding.json.".into(),
                    expected_paths: Vec::new(),
                    verify: vec!["test -f evidence/investigation-binding.json".into()],
                }],
            }
        }
        other => anyhow::bail!("unsupported synthesized investigation phase: {other}"),
    };
    let report = crate::planner::step_plan_finalize::finalize_step_plan_for_execution(
        &mut step_plan,
        config,
    );
    if !report.is_pass() {
        anyhow::bail!(
            "synthesized investigation phase '{}' failed lint: {}",
            phase.id,
            report.primary_message()
        );
    }
    Ok(step_plan)
}

fn execute_binding_if_ready(config: &Config) -> anyhow::Result<()> {
    let diagnosis_path = config.workspace_root.join("output/diagnosis.md");
    let run_path = config
        .workspace_root
        .join("evidence/investigation-run.json");
    if !diagnosis_path.is_file() || !run_path.is_file() {
        return Ok(());
    }
    let diagnosis = std::fs::read_to_string(diagnosis_path)?;
    let run: crate::planner::adjudication::investigate::InvestigationRunEvidence =
        serde_json::from_slice(&std::fs::read(run_path)?)?;
    let binding = crate::planner::investigation_binding::bind_diagnosis(
        &config.workspace_root,
        &diagnosis,
        &run,
    );
    crate::planner::adjudication::investigate::write_investigation_binding(
        &config.workspace_root,
        &binding,
    )?;
    Ok(())
}

fn applies(config: &Config, plan: &UltraPlan) -> bool {
    config.plan_preset == PlanPreset::Profile
        && config.intent_override == Some(IntentId::Investigate)
        && plan.intent == "investigate"
        && crate::planner::profile::resolve_profile_runtime(&plan.profile)
            .synthesizes_investigation_plan()
}

fn ensure_shape(plan: &UltraPlan) -> anyhow::Result<()> {
    let schema = crate::planner::intent_schema::load()?;
    // Schema supplies phase structure only; synthesis, normalization, checkpoints,
    // material injection, and adjudication intentionally remain Rust implementation.
    let _ = schema;
    let actual = plan
        .phases
        .iter()
        .map(|phase| phase.id.as_str())
        .collect::<Vec<_>>();
    if actual != PHASE_IDS {
        anyhow::bail!("investigation synthesis requires fixed three-phase order; got {actual:?}");
    }
    Ok(())
}

fn investigation_reproducer_suggestion(
    plan: &UltraPlan,
) -> Option<crate::planner::fix_reproducer::ReproducerSuggestion> {
    let mut proxy = plan.clone();
    proxy.intent = "fix".into();
    crate::planner::fix_reproducer::suggestion_for(&proxy)
}

fn emit_synthesized(config: &Config, plan: &UltraPlan, basis: &str) {
    crate::eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "investigation_plan_synthesized",
            "profile": plan.profile,
            "phase_count": plan.phases.len(),
            "r_basis": basis,
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use clap::Parser;

    fn config(root: &Path) -> Config {
        let cwd = root.to_string_lossy().to_string();
        Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--intent",
            "investigate",
            "--profile",
            "data",
        ]))
        .unwrap()
    }

    use std::path::Path;

    #[test]
    fn pipe_and_schema_goals_use_fixed_three_phase_synthesis() {
        let root = tempfile::tempdir().unwrap();
        for goal in [
            "pipeline/main.py execution fails for data/sales.csv",
            "output/results.json schema validation fails",
        ] {
            let config = config(root.path());
            let plan = crate::planner::intent::explicit_investigation_plan(goal, "data", "default");
            assert_eq!(
                plan.phases
                    .iter()
                    .map(|phase| phase.id.as_str())
                    .collect::<Vec<_>>(),
                PHASE_IDS
            );
            let generated =
                resolve_phase_plan(&config, &plan, &plan.phases[0], || panic!("model fallback"))
                    .unwrap();
            assert_eq!(generated.steps[0].id, "reproduce-candidate");
            assert_eq!(generated.steps[0].expected_result, "fail");
        }
    }
}
