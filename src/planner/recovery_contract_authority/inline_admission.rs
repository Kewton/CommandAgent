//! Preclosure candidate validation. No command removal, replacement or writes.
use super::*;

pub(crate) fn eligible(config: &Config, contract: &CompletionContract) -> anyhow::Result<bool> {
    Ok(crate::planner::profile::is_nextjs_profile(&config.profile)
        && config.resolved_run_intent() == crate::config::IntentId::Create
        && contract
            .profile
            .as_deref()
            .is_some_and(crate::planner::profile::is_nextjs_profile)
        && CompletionContract::configured_path_for_config(config)?.is_none()
        && provenance::owns_run_contract(&generated_path(config, ULTRA_RUN_CONTRACT)))
}

pub(crate) fn refusal(
    config: &Config,
    contract: &CompletionContract,
    command: &str,
) -> anyhow::Result<Option<String>> {
    if !eligible(config, contract)? || contract.verify_commands.iter().any(|c| c == command) {
        return Ok(None);
    }
    Ok(
        crate::minimal_loop::evidence::command_diagnosis::source_independent_inline_failure(
            contract, command,
        ),
    )
}

/// Call with the complete normalized, policy-checked, profile-filtered batch
/// before persisting either contract bytes or verifier producer obligations.
pub(crate) fn validate(
    config: &Config,
    contract: &CompletionContract,
    commands: &[String],
) -> anyhow::Result<()> {
    for command in commands {
        if let Some(reason) = refusal(config, contract, command)? {
            anyhow::bail!(
                "preclosure weak inline verification requires formation: {command}: {reason}; the current classifier cannot recognize this failure path; re-propose with all original requirements preserved before registration"
            );
        }
    }
    Ok(())
}
