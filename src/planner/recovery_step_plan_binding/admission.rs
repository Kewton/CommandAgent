//! Formation and Recovery binding use the existing planner attempt budget.
use super::*;
use crate::planner::recovery_contract_authority::verifier_obligations as scope;
use crate::planner::recovery_inspection::verifier_obligations::{
    BindingFailure, FailureClass, failure,
};

#[derive(Default)]
pub(crate) struct Admission {
    original: Option<StepPlan>,
}

pub(crate) enum Decision {
    Ready(bool),
    Retry(String),
}

impl Admission {
    pub(crate) fn finish(
        &self,
        config: &Config,
        phase: Option<&str>,
        plan: StepPlan,
    ) -> anyhow::Result<StepPlan> {
        if let Some(original) = &self.original
            && let Err(error) = preserve(original, &plan)
        {
            crate::eval_events::emit(
                config.eval_events_path.as_deref(),
                json!({
                    "event":"recovery_verifier_plan_return_rejected", "original_scope":original,
                    "returned_scope":plan, "reason":error.to_string(), "remaining_planner_attempts":0,
                }),
            );
            let message = format!(
                "cannot return fallback after verifier formation exhausted planner budget: {error}"
            );
            return Err(error.context(message));
        }
        if crate::planner::recovery_inspection::has_origin(config)? {
            let report = super::lint(config, &plan, phase);
            anyhow::ensure!(
                report.is_pass(),
                "cannot return unbound Recovery plan: {}",
                report.primary_message()
            );
        }
        Ok(plan)
    }

    pub(crate) fn check(
        &mut self,
        config: &Config,
        phase: Option<&str>,
        model: &StepPlan,
        plan: &mut StepPlan,
        attempt: usize,
    ) -> anyhow::Result<Decision> {
        let host = plan.clone();
        let recovery = crate::planner::recovery_inspection::has_origin(config)?;
        let result = if recovery {
            check_model_ownership(config, phase, model)
                .and_then(|()| bind_generated(config, phase, plan))
        } else {
            self.form(config, plan).map(|()| false)
        };
        let Err(error) = result else {
            return Ok(Decision::Ready(result.unwrap()));
        };
        let class = error
            .downcast_ref::<BindingFailure>()
            .map(|e| e.class)
            .unwrap_or(FailureClass::Unsafe);
        let retry = class == FailureClass::ProposalRepairable && attempt < 3;
        crate::eval_events::emit(
            config.eval_events_path.as_deref(),
            json!({
                "event":"recovery_verifier_plan_admission", "phase_id":phase,
                "stage":if recovery {"closed_binding"} else {"preclosure_formation"},
                "classification":class, "reason":error.to_string(),
                "model_proposal":model, "host_augmented_proposal":host,
                "original_scope":self.original, "planner_attempt":attempt,
                "remaining_planner_attempts":3usize.saturating_sub(attempt),
                "recovery_budget_changed":false,
                "status":if retry {"retry"} else if class == FailureClass::ProposalRepairable {"exhausted"} else {"stopped"},
            }),
        );
        if !retry {
            let reason = format!(
                "{}: {error}",
                if class == FailureClass::ProposalRepairable {
                    "verifier plan admission exhausted existing 3-attempt planner budget"
                } else {
                    "verifier plan admission cannot be corrected by proposal retry"
                }
            );
            return Err(error.context(reason));
        }
        Ok(Decision::Retry(format!(
            "Goal: {}\nCorrect this verifier plan admission failure (attempt {attempt}/3): {error}\nReturn a complete StepPlan. Assign verifier creation to one dedicated Implement/pass per original producer; retain application/configuration work in separate owners. Preserve every original requirement verbatim within the responsible instructions, every required output, expected result and check; do not erase mixed duties. Host augmentation targets the last implementation step, so place the application/configuration owner last when needed.\nOriginal scope to preserve:\n{}\nHost-augmented proposal:\n{}",
            plan.goal,
            serde_json::to_string(self.original.as_ref().unwrap_or(&host))?,
            serde_json::to_string(&host)?
        )))
    }

    fn form(&mut self, config: &Config, plan: &StepPlan) -> anyhow::Result<()> {
        if !matches!(
            config.profile.as_str(),
            crate::planner::profile_descriptor::GENERIC_PROFILE_ID
                | crate::planner::profile_descriptor::NEXTJS_PROFILE_ID
        ) || CompletionContract::configured_path_for_config(config)?.is_some()
        {
            return Ok(());
        }
        if let Some(original) = &self.original {
            preserve(original, plan)?;
        }
        let mut contract =
            match crate::planner::recovery_contract_authority::load_for_handoff(config)? {
                Some(contract) => contract,
                None => serde_json::from_value(json!({}))?,
            };
        // Invalid command syntax/policy remains owned by the existing lint
        // retry path. It cannot close a plan, and must not erase retained scope
        // or bypass the established lint diagnostics/last-valid-plan behavior.
        let Ok(admitted) = scope::admitted_commands(config, &contract, plan) else {
            return Ok(());
        };
        contract.verify_commands.extend(admitted);
        let scripts = scope::scripts(&contract.verify_commands);
        if scripts.is_empty() {
            return Ok(());
        }
        let mut claimed = Vec::<&PlanStep>::new();
        for step in &plan.steps {
            if step.step_kind() == StepKind::Verify
                || !step
                    .expected_paths
                    .iter()
                    .any(|p| scope::normalized(p).is_ok_and(|p| scripts.contains(&p)))
            {
                continue;
            }
            scope::checked_paths(config, &contract, step)
                .map_err(|e| failure(FailureClass::Unsafe, e.to_string()))?;
            if let Err(error) = scope::validate_producer(config, &contract, step) {
                self.original.get_or_insert_with(|| plan.clone());
                return Err(failure(FailureClass::ProposalRepairable, error.to_string()));
            }
            if claimed.iter().any(|old| scope::overlaps(old, step)) {
                self.original.get_or_insert_with(|| plan.clone());
                return Err(failure(
                    FailureClass::ProposalRepairable,
                    format!(
                        "multiple verifier owners include {}; keep every original requirement under one owner",
                        step.id
                    ),
                ));
            }
            claimed.push(step);
        }
        if let Some(original) = &self.original {
            crate::eval_events::emit(
                config.eval_events_path.as_deref(),
                json!({
                    "event":"recovery_verifier_obligations_formed", "original_scope":original,
                    "formed_scope":plan, "scope_preserved":true,
                    "producer_ids":claimed.iter().map(|s| &s.id).collect::<Vec<_>>(),
                }),
            );
        }
        Ok(())
    }
}

fn check_model_ownership(
    config: &Config,
    phase: Option<&str>,
    model: &StepPlan,
) -> anyhow::Result<()> {
    if phase.is_none() || phase == Some(INSPECTION_PHASE_ID) {
        return Ok(());
    }
    if let Some(contract) = CompletionContract::load_for_config(config)? {
        let mut model = model.clone();
        crate::planner::recovery_inspection::verifier_obligations::bind(
            config, &contract, &mut model,
        )?;
    }
    Ok(())
}

fn preserve(original: &StepPlan, proposed: &StepPlan) -> anyhow::Result<()> {
    for step in &original.steps {
        let mut owner_indices = Vec::new();
        for path in &step.expected_paths {
            let expected = scope::normalized(path)?;
            let owners: Vec<_> = proposed
                .steps
                .iter()
                .enumerate()
                .filter(|(_, s)| {
                    s.step_kind() == step.step_kind()
                        && s.expected_paths
                            .iter()
                            .any(|p| scope::normalized(p).ok().as_ref() == Some(&expected))
                })
                .collect();
            if owners.is_empty() {
                return Err(failure(
                    FailureClass::ProposalRepairable,
                    format!("formation dropped original required output owner {path}"),
                ));
            }
            for (index, owner) in owners {
                if !owner.instruction.contains(&step.instruction)
                    || owner.expected_result != step.expected_result
                {
                    return Err(failure(
                        FailureClass::ProposalRepairable,
                        format!(
                            "owner {} of {path} lost original requirements/expected result from {}; unrelated owners cannot discharge this duty",
                            owner.id, step.id
                        ),
                    ));
                }
                owner_indices.push(index);
            }
        }
        if !proposed.steps.iter().any(|s| {
            s.step_kind() == step.step_kind()
                && s.instruction.contains(&step.instruction)
                && s.expected_result == step.expected_result
        }) {
            return Err(failure(
                FailureClass::ProposalRepairable,
                format!(
                    "formation dropped original instruction/expected result for {}: {:?}",
                    step.id, step.instruction
                ),
            ));
        }
        for command in &step.verify {
            if !proposed.steps.iter().enumerate().any(|(index, s)| {
                s.expected_result == step.expected_result
                    && s.verify.contains(command)
                    && (owner_indices.iter().all(|i| *i == index)
                        && s.instruction.contains(&step.instruction)
                        || s.step_kind() == StepKind::Verify
                            && step.verify.iter().all(|c| s.verify.contains(c))
                            && owner_indices.iter().all(|i| *i < index))
            }) {
                return Err(failure(
                    FailureClass::ProposalRepairable,
                    format!(
                        "formation lost original check/expected result or its complete scope and boundary: {command}; keep it on the complete owner or a Verify after all corresponding output owners"
                    ),
                ));
            }
        }
    }
    Ok(())
}
