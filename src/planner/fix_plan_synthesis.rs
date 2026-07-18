use anyhow::Context;
use serde_json::json;

use crate::config::{Config, PlanPreset};
use crate::eval_events;
use crate::planner::adjudication::contract::{IntentId, is_fix_intent};
use crate::planner::fix_runtime::FixRuntime;
use crate::planner::profile::{ProfileFixRegressionAdapter, profile_expected_paths};
use crate::planner::repair_targeting::{
    RepairTargetPriority, RepairTargetResolutionInput, RepairTargetSelection,
    resolve_repair_targets,
};
use crate::planner::step_plan::{PlanStep, StepKind, StepPlan};
use crate::planner::ultra_plan::{UltraPhase, UltraPlan};

const CONTRACT_PHASE_IDS: [&str; 4] = [
    "reproduce-before",
    "isolate-cause",
    "repair",
    "verify-regressions",
];
const MODEL_REPRODUCER_BASIS: &str = "model_required";

pub(crate) enum PhasePlan {
    NotApplicable,
    ModelReproducer,
    Generated(StepPlan),
}

pub(crate) fn resolve_phase_plan(
    config: &Config,
    plan: &UltraPlan,
    phase: &UltraPhase,
    runtime: Option<&FixRuntime>,
    generate_with_model: impl FnOnce() -> anyhow::Result<StepPlan>,
) -> anyhow::Result<StepPlan> {
    match phase_plan(config, plan, phase, runtime)? {
        PhasePlan::Generated(plan) => Ok(plan),
        PhasePlan::ModelReproducer => {
            canonicalize_model_reproducer(config, plan, phase, generate_with_model()?)
        }
        PhasePlan::NotApplicable => generate_with_model(),
    }
}

pub(crate) fn phase_plan(
    config: &Config,
    plan: &UltraPlan,
    phase: &UltraPhase,
    runtime: Option<&FixRuntime>,
) -> anyhow::Result<PhasePlan> {
    if !applies(config, plan) {
        return Ok(PhasePlan::NotApplicable);
    }
    ensure_contract_shape(plan)?;
    match phase.id.as_str() {
        "reproduce-before" => reproduce_before(config, plan, phase),
        "isolate-cause" => generated(config, phase, isolate_cause(config, plan, runtime)?),
        "repair" => generated(config, phase, implement_fix(config, plan, runtime)?),
        "verify-regressions" => generated(config, phase, verify_after(config, plan, runtime)?),
        other => anyhow::bail!("unsupported synthesized data fix phase: {other}"),
    }
}

pub(crate) fn canonicalize_model_reproducer(
    config: &Config,
    plan: &UltraPlan,
    phase: &UltraPhase,
    model_plan: StepPlan,
) -> anyhow::Result<StepPlan> {
    if !applies(config, plan) || phase.id != "reproduce-before" {
        return Ok(model_plan);
    }
    let commands = model_plan
        .steps
        .iter()
        .filter(|step| step.step_kind() == StepKind::Verify)
        .flat_map(|step| step.verify.iter())
        .map(|command| command.trim())
        .filter(|command| !command.is_empty())
        .collect::<Vec<_>>();
    if commands.len() != 1 {
        anyhow::bail!(
            "model-derived data fix reproducer must contain exactly one verify command, got {}",
            commands.len()
        );
    }
    generated(
        config,
        phase,
        reproducer_plan(&plan.goal, commands[0].to_string()),
    )
    .map(|result| match result {
        PhasePlan::Generated(plan) => plan,
        _ => unreachable!("generated reproducer always returns a StepPlan"),
    })
}

fn applies(config: &Config, plan: &UltraPlan) -> bool {
    config.plan_preset == PlanPreset::Profile
        && config.intent_override == Some(IntentId::Fix)
        && is_fix_intent(&plan.intent)
        && crate::planner::profile::domain_profile(&plan.profile).id() == "data"
}

fn ensure_contract_shape(plan: &UltraPlan) -> anyhow::Result<()> {
    let phase_ids = plan
        .phases
        .iter()
        .map(|phase| phase.id.as_str())
        .collect::<Vec<_>>();
    if phase_ids != CONTRACT_PHASE_IDS {
        anyhow::bail!(
            "synthesized data fix requires fixed four-phase contract order; got {phase_ids:?}"
        );
    }
    Ok(())
}

fn reproduce_before(
    config: &Config,
    plan: &UltraPlan,
    phase: &UltraPhase,
) -> anyhow::Result<PhasePlan> {
    let suggestion = crate::planner::fix_reproducer::suggestion_for(plan);
    let basis = suggestion
        .as_ref()
        .map(|suggestion| suggestion.basis.as_str())
        .unwrap_or(MODEL_REPRODUCER_BASIS);
    emit_synthesized(config, plan, basis);
    let Some(command) = suggestion
        .as_ref()
        .and_then(|suggestion| primary_reproducer_command(&suggestion.suggestion))
    else {
        return Ok(PhasePlan::ModelReproducer);
    };
    generated(config, phase, reproducer_plan(&plan.goal, command))
}

fn primary_reproducer_command(suggestion: &str) -> Option<String> {
    let candidate = suggestion.split(" | ").next()?.trim();
    let (_, command) = candidate.split_once(" => ")?;
    let command = command.trim();
    if command.is_empty() || command.lines().count() != 1 {
        return None;
    }
    Some(command.to_string())
}

fn reproducer_plan(goal: &str, command: String) -> StepPlan {
    StepPlan {
        goal: goal.to_string(),
        steps: vec![PlanStep {
            id: "reproduce-before".to_string(),
            kind: "verify".to_string(),
            expected_result: "fail".to_string(),
            instruction: "Run the bound reproducer R to record the required F1 failing baseline before any workspace changes.".to_string(),
            expected_paths: Vec::new(),
            verify: vec![command],
        }],
    }
}

fn isolate_cause(
    config: &Config,
    plan: &UltraPlan,
    runtime: Option<&FixRuntime>,
) -> anyhow::Result<StepPlan> {
    let runtime = runtime.context("data fix synthesis requires an active fix runtime")?;
    let evidence_path = runtime
        .before_evidence_path()
        .context("data fix isolate phase requires confirmed F1 evidence")?;
    if !config.workspace_root.join(&evidence_path).is_file() {
        anyhow::bail!("data fix isolate phase F1 evidence is not present: {evidence_path}");
    }
    let mut present = crate::planner::profiles::data::manifest::source_paths();
    present.extend(profile_expected_paths(
        &config.workspace_root,
        &plan.profile,
        &plan.goal,
    ));
    if let Some(diagnostic) = runtime.repair_diagnostic() {
        present.insert(0, diagnostic.target_path.clone());
    }
    present.retain(|path| config.workspace_root.join(path).is_file());
    present.sort();
    present.dedup();
    let subjects = if present.is_empty() {
        "no additional workspace subject".to_string()
    } else {
        present
            .iter()
            .map(|path| format!("`{path}`"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    Ok(StepPlan {
        goal: plan.goal.clone(),
        steps: vec![PlanStep {
            id: "isolate-cause".to_string(),
            kind: "inspect".to_string(),
            expected_result: "pass".to_string(),
            instruction: format!(
                "Read only the executed runtime-bound F1 failure evidence and the existing subjects {subjects}; identify the narrow cause without changing the workspace."
            ),
            expected_paths: Vec::new(),
            verify: Vec::new(),
        }],
    })
}

fn implement_fix(
    config: &Config,
    plan: &UltraPlan,
    runtime: Option<&FixRuntime>,
) -> anyhow::Result<StepPlan> {
    let runtime = runtime.context("data fix synthesis requires an active fix runtime")?;
    let command = runtime
        .reproducer_command()
        .context("data fix repair phase requires the bound F1 reproducer")?;
    let selection = repair_target(config, plan, runtime)
        .context("data fix synthesis could not resolve an existing repair target")?;
    let target = selection
        .primary_target()
        .context("data fix repair target selection was empty")?;
    let selection_reason = selection.selection_reason.as_str();
    Ok(StepPlan {
        goal: plan.goal.clone(),
        steps: vec![PlanStep {
            id: "implement-fix".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: format!(
                "Repair the F1-diagnosed defect in `{target}` using the isolated cause and the shared target resolver ({selection_reason}); preserve the existing data contract and keep ownership of this path in this step."
            ),
            expected_paths: vec![target.to_string()],
            verify: vec![command.to_string()],
        }],
    })
}

fn repair_target(
    config: &Config,
    plan: &UltraPlan,
    runtime: &FixRuntime,
) -> Option<RepairTargetSelection> {
    let mapped = runtime
        .repair_diagnostic()
        .map(|diagnostic| RepairTargetSelection {
            selected_targets: vec![diagnostic.target_path.clone()],
            selection_reason: diagnostic.selection_reason,
        });
    let reproducer_evidence = runtime
        .reproducer_command()
        .and_then(crate::planner::profiles::data::step_policy::catalog_check_id)
        .map(str::to_string)
        .into_iter()
        .collect::<Vec<_>>();
    let fallback_paths = crate::planner::profiles::data::manifest::source_paths()
        .into_iter()
        .filter(|path| config.workspace_root.join(path).is_file())
        .collect::<Vec<_>>();
    let mut selection = resolve_repair_targets(RepairTargetResolutionInput {
        root: &config.workspace_root,
        profile: &plan.profile,
        pending_evidence: &reproducer_evidence,
        missing_capabilities: &[],
        contract_attribute_paths: &[],
        repair_changed_paths: &[],
        required_paths: &[],
        fallback_paths: &fallback_paths,
        mapped_selection: mapped.as_ref(),
        priority: RepairTargetPriority::FixIntent,
    })?;
    selection
        .selected_targets
        .retain(|path| config.workspace_root.join(path).is_file());
    (!selection.selected_targets.is_empty()).then_some(selection)
}

fn verify_after(
    _config: &Config,
    plan: &UltraPlan,
    runtime: Option<&FixRuntime>,
) -> anyhow::Result<StepPlan> {
    let runtime = runtime.context("data fix synthesis requires an active fix runtime")?;
    let reproducer = runtime
        .reproducer_command()
        .context("data fix verification phase requires the bound F1 reproducer")?;
    let regressions = runtime
        .regression_bindings()
        .iter()
        .map(|binding| match &binding.adapter {
            ProfileFixRegressionAdapter::DataManifestCheck => {
                Ok(crate::planner::profiles::data::step_policy::catalog_check_command(&binding.id))
            }
            other => anyhow::bail!(
                "data fix synthesis received non-data F3 adapter for {}: {other:?}",
                binding.id
            ),
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    if regressions.is_empty() {
        anyhow::bail!("data fix verification requires a non-empty frozen F3 set");
    }
    Ok(StepPlan {
        goal: plan.goal.clone(),
        steps: vec![
            PlanStep {
                id: "verify-after".to_string(),
                kind: "verify".to_string(),
                expected_result: "pass".to_string(),
                instruction:
                    "Run the exact F1 reproducer R after the repair to bind F2 to the same lineage."
                        .to_string(),
                expected_paths: Vec::new(),
                verify: vec![reproducer.to_string()],
            },
            PlanStep {
                id: "verify-regressions".to_string(),
                kind: "verify".to_string(),
                expected_result: "pass".to_string(),
                instruction: "Run every frozen data-profile regression check for F3 after F2."
                    .to_string(),
                expected_paths: Vec::new(),
                verify: regressions,
            },
        ],
    })
}

fn generated(config: &Config, phase: &UltraPhase, plan: StepPlan) -> anyhow::Result<PhasePlan> {
    let mut plan = plan;
    let report =
        crate::planner::step_plan_finalize::finalize_step_plan_for_execution(&mut plan, config);
    if !report.is_pass() {
        anyhow::bail!(
            "synthesized data fix phase `{}` failed lint: {}",
            phase.id,
            report.primary_message()
        );
    }
    crate::tui::presentation::emit_step_plan_block(&phase.id, &plan, None);
    Ok(PhasePlan::Generated(plan))
}

fn emit_synthesized(config: &Config, plan: &UltraPlan, basis: &str) {
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "fix_plan_synthesized",
            "profile": plan.profile,
            "phase_count": plan.phases.len(),
            "r_basis": basis,
        }),
    );
}

#[cfg(test)]
mod tests;
