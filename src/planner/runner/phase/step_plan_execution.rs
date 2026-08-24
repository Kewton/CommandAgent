use super::*;

#[allow(clippy::result_large_err, clippy::too_many_arguments)]
pub(in crate::planner::runner) fn run_step_plan_with_session_with_ui(
    client: &mut dyn ChatClient,
    session: &mut SessionSnapshot,
    plan: &StepPlan,
    config: &Config,
    ui: &dyn InteractionUi,
    verify_final_contract: bool,
    mode: &'static str,
    phase_scope: Option<&str>,
    overall_goal_override: Option<&str>,
) -> Result<StepPlanRunOutcome, StepPlanRunError> {
    run_step_plan_with_session_with_ui_and_run_authority(
        client,
        session,
        plan,
        config,
        ui,
        verify_final_contract,
        mode,
        phase_scope,
        overall_goal_override,
        None,
    )
}
#[allow(clippy::result_large_err, clippy::too_many_arguments)]
pub(in crate::planner::runner) fn run_step_plan_with_session_with_ui_and_run_authority(
    client: &mut dyn ChatClient,
    session: &mut SessionSnapshot,
    plan: &StepPlan,
    config: &Config,
    ui: &dyn InteractionUi,
    verify_final_contract: bool,
    mode: &'static str,
    phase_scope: Option<&str>,
    overall_goal_override: Option<&str>,
    mut run_setup_authority: Option<&mut UltraRunSetupAuthorityState>,
) -> Result<StepPlanRunOutcome, StepPlanRunError> {
    let mut outcome = StepPlanRunOutcome::for_plan(plan);
    let overall_goal = overall_goal_override.unwrap_or(&plan.goal);
    let report = crate::planner::lint::lint_plan_for_execution(plan, Some(&config.workspace_root));
    if !report.is_pass() {
        emit_planner_error_for_lint(config, "plan-file", &config.planner_model, &report, 0);
        if !lint_report_is_runtime_repairable_verifier_command(&report) {
            return Err(StepPlanRunError::from_error(
                report.primary_message(),
                outcome,
            ));
        }
    }
    let required_final_artifacts = required_final_artifacts(plan, &config.workspace_root);
    let profile_id = ProfileId::parse(&config.profile);
    let runtime = ProfileRuntimeRegistry::resolve(&profile_id);
    let mut final_required_capabilities = runtime.required_capabilities(&plan.goal);
    let final_required_obligations =
        runtime.required_obligations(&profile_id, &plan.goal, &final_required_capabilities);
    let initial_required_evidence =
        runtime.required_evidence(&plan.goal, &final_required_capabilities);
    let bound_contract = bind_completion_contract_for_acceptance(
        config,
        "plan-run",
        &config.profile,
        &plan.goal,
        &required_final_artifacts,
        &final_required_capabilities,
        &initial_required_evidence,
        &final_required_obligations,
    )
    .map_err(|err| StepPlanRunError::from_error(err.to_string(), outcome.clone()))?;
    if let Some(contract) = bound_contract.as_ref().map(|bound| &bound.contract) {
        merge_unique_strings(
            &mut final_required_capabilities,
            &contract.required_capabilities,
        );
    }
    let mut final_required_evidence =
        runtime.required_evidence(&plan.goal, &final_required_capabilities);
    if let Some(contract) = bound_contract.as_ref().map(|bound| &bound.contract) {
        merge_unique_strings(
            &mut final_required_capabilities,
            &contract.required_capabilities,
        );
        merge_unique_strings(&mut final_required_evidence, &contract.required_evidence);
    }
    let contract_enforcement = if verify_final_contract {
        ContractEnforcement::Enforce
    } else {
        ContractEnforcement::Observe
    };
    let mut prior_expected_paths = Vec::new();
    let events = PlanStepEvents::new(plan, config, session, mode, phase_scope);
    for (index, step) in plan.steps.iter().enumerate() {
        let task = events.start(step, index);
        if ui.interrupted() {
            task.interrupted();
            return Err(StepPlanRunError::from_error("interrupted by user", outcome));
        }
        let prompt_context = StepPromptContext {
            overall_goal: overall_goal.to_string(),
            required_final_artifacts: required_final_artifacts.clone(),
            prior_expected_paths: prior_expected_paths.clone(),
            final_required_capabilities: final_required_capabilities.clone(),
            final_required_evidence: final_required_evidence.clone(),
            completion_contract_path: bound_contract
                .as_ref()
                .and_then(|bound| bound.fs_path.clone()),
        };
        let result = run_step(
            client,
            session,
            plan,
            step,
            &prompt_context,
            config,
            ui,
            mode,
            contract_enforcement,
            phase_scope,
            run_setup_authority.as_deref_mut(),
        );
        task.finish(&result);
        match result {
            Ok(step_outcome) => {
                outcome.completed_steps += 1;
                outcome.merge_step(&step_outcome);
            }
            Err(err) => {
                outcome.merge_step(&err.outcome);
                return Err(StepPlanRunError::from_error(err.message, outcome));
            }
        }
        if let Some(state) = run_setup_authority.as_deref_mut()
            && step_carries_setup_authority(plan, step, phase_scope)
        {
            state.grant("phase_setup_step");
        }
        merge_unique_strings(&mut prior_expected_paths, &step.expected_paths);
    }
    if verify_final_contract
        && let Err(err) = verify_plan_final_contract(
            plan,
            &required_final_artifacts,
            config,
            bound_contract.as_ref(),
        )
    {
        return Err(StepPlanRunError::from_error(err.to_string(), outcome));
    }
    outcome.summary = format!("plan-run complete: {} steps", plan.steps.len());
    Ok(outcome)
}

pub(super) fn lint_report_is_runtime_repairable_verifier_command(report: &PlanLintReport) -> bool {
    !report.errors.is_empty()
        && report.errors.iter().all(|error| {
            error.category == "verify_policy"
                && (error
                    .message
                    .contains("grep pattern begins with '-' but command lacks `--` or `-e`")
                    || error.message.contains(
                        "grep package.json script assertion replaced with JSON parser check",
                    ))
        })
}
