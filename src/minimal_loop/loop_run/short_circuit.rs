use super::*;

pub(super) struct ShortCircuitContext<'a> {
    pub(super) verify_attempts: &'a mut usize,
    pub(super) at: StepShortCircuitAt,
    pub(super) write_or_edit_seen: bool,
    pub(super) implementation: &'a ImplementationCompletion,
    pub(super) changed_paths: &'a [String],
}

pub(super) fn maybe_short_circuit_satisfied_step(
    config: &Config,
    options: &RunSessionOptions,
    user_prompt: &str,
    required_paths: &[String],
    contract: Option<&CompletionContract>,
    context: ShortCircuitContext<'_>,
) -> anyhow::Result<Option<RunSessionOutcome>> {
    let ShortCircuitContext {
        verify_attempts,
        at,
        write_or_edit_seen,
        implementation,
        changed_paths,
    } = context;
    if at == StepShortCircuitAt::Start && options.step_kind == Some(RunSessionStepKind::Implement) {
        return Ok(None);
    }
    if implementation
        .feedback(config, options, write_or_edit_seen)
        .is_some()
    {
        return Ok(None);
    }
    let has_contract_gate = contract.is_some_and(CompletionContract::has_verify);
    if required_paths.is_empty() {
        return Ok(None);
    }
    let missing = missing_paths(&config.workspace_root, required_paths);
    if !missing.is_empty() {
        return Ok(None);
    }
    if options.require_mutation_before_contract_short_circuit && !write_or_edit_seen {
        return Ok(None);
    }
    if let Some(contract) = contract.filter(|_| has_contract_gate) {
        let attempts_before_probe = *verify_attempts;
        match verify_completion_contract_with_enforcement(
            &config.workspace_root,
            config.eval_events_path.as_deref(),
            contract,
            user_prompt,
            verify_attempts,
            None,
            None,
            &[],
            &[],
            &[],
            false,
            options.dependency_setup_authority,
            config.offline,
            options,
        )? {
            ContractVerificationOutcome::Satisfied => {
                emit_step_short_circuited(
                    config,
                    options,
                    required_paths,
                    *verify_attempts,
                    at,
                    implementation,
                    write_or_edit_seen,
                    Some(contract),
                );
                return Ok(Some(RunSessionOutcome {
                    final_text: format!("step short-circuited: {}", required_paths.join(", ")),
                    stop_reason: RunStopReason::CompletionContractSatisfied,
                    changed_paths: implementation.changes(&config.workspace_root, changed_paths),
                    iterations: 0,
                    tool_calls: 0,
                    missing_required_paths: Vec::new(),
                    missing_capabilities: Vec::new(),
                    missing_evidence: Vec::new(),
                    missing_obligations: Vec::new(),
                    verify_attempts: *verify_attempts,
                    last_blocking_reason: None,
                    last_provider_error: None,
                }));
            }
            ContractVerificationOutcome::NeedsRepair(_)
            | ContractVerificationOutcome::ObservationIncomplete(_) => {
                *verify_attempts = attempts_before_probe;
                return Ok(None);
            }
        }
    }
    if !setup_short_circuit_allowed(options, user_prompt) {
        return Ok(None);
    }
    emit_step_short_circuited(
        config,
        options,
        required_paths,
        *verify_attempts,
        at,
        implementation,
        write_or_edit_seen,
        None,
    );
    Ok(Some(RunSessionOutcome {
        final_text: format!("step short-circuited: {}", required_paths.join(", ")),
        stop_reason: RunStopReason::RequiredArtifactsSatisfiedAfterTool,
        changed_paths: implementation.changes(&config.workspace_root, changed_paths),
        iterations: 0,
        tool_calls: 0,
        missing_required_paths: Vec::new(),
        missing_capabilities: Vec::new(),
        missing_evidence: Vec::new(),
        missing_obligations: Vec::new(),
        verify_attempts: *verify_attempts,
        last_blocking_reason: None,
        last_provider_error: None,
    }))
}

#[allow(clippy::too_many_arguments)]
fn emit_step_short_circuited(
    config: &Config,
    options: &RunSessionOptions,
    required_paths: &[String],
    verify_attempts: usize,
    at: StepShortCircuitAt,
    implementation: &ImplementationCompletion,
    write_or_edit_seen: bool,
    contract: Option<&CompletionContract>,
) {
    let mut event = implementation.event(config, options, write_or_edit_seen);
    event["event"] = json!("step_short_circuited");
    event["at"] = json!(at.as_str());
    event["required_paths"] = json!(required_paths);
    event["verify_attempts"] = json!(verify_attempts);
    event["session_scope"] = json!(options.scope.as_str());
    event["step_kind"] = json!(
        options
            .step_kind
            .map(RunSessionStepKind::as_str)
            .unwrap_or("")
    );
    event["phase_scope"] = json!(options.phase_scope.as_deref().unwrap_or(""));
    if contract.is_none() {
        event["satisfaction_basis"] = json!("setup_artifacts_present");
    }
    event["completion_requirements"] = json!(contract.map(|contract| json!({
        "required_paths": contract.required_paths,
        "verify_commands": contract.verify_commands,
        "required_capabilities": contract.required_capabilities,
        "required_evidence": contract.required_evidence,
        "required_obligations": contract.required_obligations,
    })));
    eval_events::emit(config.eval_events_path.as_deref(), event);
}

fn setup_short_circuit_allowed(options: &RunSessionOptions, user_prompt: &str) -> bool {
    if options.step_kind != Some(RunSessionStepKind::Setup) {
        return false;
    }
    if setup_step_policy::prompt_mentions_setup(user_prompt) {
        return true;
    }
    let phase_scope = options
        .phase_scope
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase();
    phase_scope.contains("setup") || phase_scope.contains("scaffold")
}
