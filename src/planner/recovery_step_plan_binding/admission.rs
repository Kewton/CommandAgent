//! Formation and Recovery binding use the existing planner attempt budget.
use super::formation_scope::{FormationScope, ProfileAddition};
use super::*;
use crate::planner::recovery_contract_authority::verifier_obligations as scope;
use crate::planner::recovery_inspection::verifier_obligations::{
    BindingFailure, FailureClass, failure,
};

#[derive(Default)]
pub(crate) struct Admission {
    original: Option<StepPlan>,
    sources: Option<FormationScope>,
    profile_addition: Option<ProfileAddition>,
    replacements: Vec<super::verifier_formation::Replacement>,
    pending_sources: Option<super::package_script_formation::CapturedSources>,
    package_scripts: Option<super::package_script_formation::PackageScripts>,
    awaiting_check: bool,
    duplicate_strengthen: bool,
    marker_checks: Vec<super::literal_marker_formation::Obligation>,
}

pub(crate) enum Decision {
    Ready(bool),
    Retry(String),
}

impl Admission {
    pub(crate) fn strengthen(&mut self, config: &Config, plan: &mut StepPlan) {
        if self.awaiting_check {
            self.duplicate_strengthen = true;
            return;
        }
        self.awaiting_check = true;
        self.profile_addition =
            super::profile_augmentation::strengthen_step_plan_for_profile(plan, config);
        #[cfg(test)]
        crate::planner::setup_step_policy::issue492_tests::record("augmented", plan);
        if self.original.is_none() {
            self.pending_sources = Some(super::package_script_formation::CapturedSources::new(
                config,
                FormationScope::capture(plan, self.profile_addition.as_ref()),
                "profile_augmentation_before_sanitization",
            ));
        }
    }

    fn preserve(&self, plan: &StepPlan) -> anyhow::Result<()> {
        super::verifier_formation::preserve_registered(&self.replacements, plan)?;
        if let Some(original) = &self.original {
            let (projected, _) = super::verifier_formation::project(
                original,
                plan,
                self.package_scripts.as_ref(),
                &self.marker_checks,
            );
            if let Some(sources) = &self.sources {
                sources.preserve(original, &projected)?;
            } else {
                preserve(original, &projected)?;
            }
        }
        Ok(())
    }

    fn retain(&mut self, config: &Config, plan: &StepPlan, contract: &CompletionContract) {
        if self.original.is_none() {
            self.marker_checks = super::literal_marker_formation::capture(config, contract, plan);
            self.sources = Some(FormationScope::capture(
                plan,
                self.profile_addition.as_ref(),
            ));
            let sources = self.pending_sources.take().unwrap_or_else(|| {
                super::package_script_formation::CapturedSources::new(
                    config,
                    self.sources.as_ref().expect("captured sources").clone(),
                    "admission_before_first_retry",
                )
            });
            self.package_scripts = Some(super::package_script_formation::PackageScripts::capture(
                config, plan, sources, contract,
            ));
            self.original = Some(plan.clone());
        }
    }

    pub(crate) fn finish(
        &self,
        config: &Config,
        phase: Option<&str>,
        plan: StepPlan,
    ) -> anyhow::Result<StepPlan> {
        self.require_capture_boundary()?;
        if let Some(contract) =
            crate::planner::recovery_contract_authority::load_for_handoff(config)?
            && crate::planner::recovery_contract_authority::inline_admission::eligible(
                config, &contract,
            )?
        {
            let admitted = scope::admitted_commands(config, &contract, &plan)?;
            crate::planner::recovery_contract_authority::inline_admission::validate(
                config, &contract, &admitted,
            )?;
        }
        if let Some(original) = &self.original
            && let Err(error) = self.preserve(&plan)
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
            check_model_ownership(config, phase, &plan)?;
            let report = super::lint(config, &plan, phase);
            anyhow::ensure!(
                report.is_pass(),
                "cannot return unbound Recovery plan: {}",
                report.primary_message()
            );
        } else if self.original.is_some() {
            super::verifier_formation::require_formed(config, &scope::commands(&plan))?;
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
        self.awaiting_check = false;
        let host = plan.clone();
        let recovery = crate::planner::recovery_inspection::has_origin(config)?;
        let result = if let Err(error) = self.require_capture_boundary() {
            Err(error)
        } else if recovery {
            check_model_ownership(config, phase, model)
                .and_then(|()| bind_generated(config, phase, plan))
        } else {
            self.package_scripts
                .as_ref()
                .map_or(Ok(()), |p| p.preserve_raw_commands(model))
                .and_then(|()| {
                    super::literal_marker_formation::preserve_raw_commands(
                        &self.marker_checks,
                        model,
                    )
                })
                .and_then(|()| self.form(config, plan, attempt))
                .map(|()| false)
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
                "original_scope":self.original, "original_obligation_sources":self.sources,
                "package_script_formation":self.package_scripts,
                "planner_attempt":attempt,
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
            "Goal: {}\nCorrect this verifier plan admission failure (attempt {attempt}/3): {error}\nReturn a complete StepPlan. Assign verifier creation to one dedicated Implement/pass per original producer; retain application/configuration work in separate owners. Preserve every original requirement verbatim within the responsible instructions, every required output, expected result and check; do not erase mixed duties. Host augmentation targets the last implementation step, so place the application/configuration owner last when needed. Keep model and host duties on their respective executable owners; do not duplicate the combined instruction.\nOriginal scope to preserve:\n{}\nOriginal obligation sources (model and host separately):\n{}\nHost-augmented proposal:\n{}",
            plan.goal,
            serde_json::to_string(self.original.as_ref().unwrap_or(&host))?,
            serde_json::to_string(&self.sources)?,
            serde_json::to_string(&host)?
        )))
    }

    fn require_capture_boundary(&self) -> anyhow::Result<()> {
        if self.duplicate_strengthen {
            return Err(failure(
                FailureClass::Unsafe,
                "source acquisition requires one strengthen followed by check per proposal; duplicate strengthen cannot identify the original source",
            ));
        }
        Ok(())
    }

    fn form(&mut self, config: &Config, plan: &StepPlan, _attempt: usize) -> anyhow::Result<()> {
        if !matches!(
            config.profile.as_str(),
            crate::planner::profile_descriptor::GENERIC_PROFILE_ID
                | crate::planner::profile_descriptor::NEXTJS_PROFILE_ID
        ) || CompletionContract::configured_path_for_config(config)?.is_some()
        {
            return Ok(());
        }
        self.preserve(plan)?;
        if let Some(original) = &self.original {
            let (_, replacements) = super::verifier_formation::project(
                original,
                plan,
                self.package_scripts.as_ref(),
                &self.marker_checks,
            );
            for replacement in &replacements {
                if !self.replacements.contains(replacement) {
                    self.replacements.push(replacement.clone());
                }
            }
            #[cfg(test)]
            super::issue466_tests::issue494::record(
                "form_preserve_ok",
                _attempt,
                original,
                plan,
                &serde_json::to_value(&replacements).expect("serialize formation trace"),
            );
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
        let registered_contract = contract.clone();
        contract.verify_commands.extend(admitted);
        if let Err(error) =
            super::verifier_formation::require_formed(config, &scope::commands(plan))
        {
            self.retain(config, plan, &registered_contract);
            let mut guidance = self
                .package_scripts
                .as_ref()
                .map(|p| p.guidance())
                .unwrap_or_default();
            guidance.push_str(&super::literal_marker_formation::guidance(
                &self.marker_checks,
            ));
            return if guidance.is_empty() {
                Err(error)
            } else {
                let message = format!("{error}\n{guidance}");
                Err(error.context(message))
            };
        }
        #[cfg(test)]
        if let Some(original) = &self.original {
            super::issue466_tests::issue494::record(
                "require_formed_result=ok",
                _attempt,
                original,
                plan,
                &serde_json::to_value(&self.replacements).expect("serialize formation trace"),
            );
        }
        if let Some(original) = &self.original {
            let (_, replacements) = super::verifier_formation::project(
                original,
                plan,
                self.package_scripts.as_ref(),
                &self.marker_checks,
            );
            crate::eval_events::emit(
                config.eval_events_path.as_deref(),
                json!({
                    "event":"preclosure_verifier_replacements_validated",
                    "replacements":replacements, "scope_preserved":true,
                    "formed_verify_commands":scope::commands(plan),
                    "package_script_formation":self.package_scripts,
                    "formed_plan":plan,
                }),
            );
        }
        let scripts = scope::scripts(&contract.verify_commands);
        if scripts.is_empty() {
            return Ok(());
        }
        if let Some(addition) = &self.profile_addition
            && let Err(error) = addition.preserve(plan)
        {
            self.retain(config, plan, &registered_contract);
            return Err(error);
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
                self.retain(config, plan, &registered_contract);
                return Err(failure(FailureClass::ProposalRepairable, error.to_string()));
            }
            if claimed.iter().any(|old| scope::overlaps(old, step)) {
                self.retain(config, plan, &registered_contract);
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
                    "original_obligation_sources":self.sources,
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
        super::verifier_formation::require_repairable(config, &contract)?;
    }
    Ok(())
}

pub(super) fn preserve(original: &StepPlan, proposed: &StepPlan) -> anyhow::Result<()> {
    super::reader_obligations::preserve(original, proposed)?;
    for step in &original.steps {
        if super::reader_obligations::supported(step) {
            continue;
        }
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
