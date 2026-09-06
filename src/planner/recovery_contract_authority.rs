//! Resolve the run-level completion contract before automatic Recovery begins.

use anyhow::Context;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::config::Config;
use crate::minimal_loop::completion::CompletionContract;

const ULTRA_RUN_CONTRACT: &str = "completion-contract-ultra-plan-run.json";

fn generated_path(config: &Config, filename: &str) -> std::path::PathBuf {
    crate::planner::completion_contract_path::generated_path(
        &config.workspace_root,
        config.eval_events_path.as_deref(),
        filename,
    )
}

fn admits_generated_commands(profile: &str) -> bool {
    matches!(
        profile,
        crate::planner::profile_descriptor::NEXTJS_PROFILE_ID
            | crate::planner::profile_descriptor::GENERIC_PROFILE_ID
    )
}

fn profile_commands(config: &Config, profile: &str, goal: &str) -> Vec<String> {
    if !admits_generated_commands(profile) {
        return Vec::new();
    }
    crate::planner::profile::resolve_profile_runtime(profile)
        .quality_expectations(&config.workspace_root, goal)
        .preferred_verify
}

/// Register product-owned profile checks at generation, preserving admitted
/// plan checks when final acceptance refreshes the same run contract.
pub(crate) fn generated_verify_commands(
    config: &Config,
    scope: &str,
    profile: &str,
    goal: &str,
) -> anyhow::Result<Vec<String>> {
    if scope != "ultra-plan-run" || !admits_generated_commands(profile) {
        return Ok(Vec::new());
    }
    let mut commands = profile_commands(config, profile, goal);
    let path = generated_path(config, ULTRA_RUN_CONTRACT);
    if path.is_file() {
        let mut bound = config.clone();
        bound.completion_contract_path = Some(path);
        if let Some(contract) = CompletionContract::load_for_config(&bound)?
            && contract.profile.as_deref() == Some(profile)
            && contract.goal.as_deref() == Some(goal)
        {
            commands.extend(contract.verify_commands);
        }
    }
    Ok(commands)
}

/// Called after StepPlan admission and before execution, never from model
/// Recovery proposals. Configured and data contracts remain closed registries.
pub(crate) fn register_step_plan_commands(
    config: &Config,
    commands: &[String],
) -> anyhow::Result<()> {
    if commands.is_empty() || CompletionContract::configured_path_for_config(config)?.is_some() {
        return Ok(());
    }
    let path = generated_path(config, ULTRA_RUN_CONTRACT);
    if !path.is_file() {
        return Ok(());
    }
    let mut bound = config.clone();
    bound.completion_contract_path = Some(path.clone());
    let mut contract =
        CompletionContract::load_for_config(&bound)?.context("run contract missing")?;
    if !admits_generated_commands(contract.profile.as_deref().unwrap_or_default()) {
        return Ok(());
    }
    let original = contract.verify_commands.clone();
    contract.verify_commands.extend_from_slice(commands);
    let contract = contract.validate(&config.workspace_root)?;
    if contract.verify_commands != original {
        persist_generated_commands(config, &path, &contract, "admitted_step_plan")?;
    }
    Ok(())
}

/// Implement steps receive a narrow generated plan contract. Recovery must
/// instead retain the run goal and checks, without changing step acceptance.
pub(crate) fn load_for_handoff(config: &Config) -> anyhow::Result<Option<CompletionContract>> {
    let configured = CompletionContract::configured_path_for_config(config)?;
    let step_path = generated_path(config, "completion-contract-plan-run.json");
    let generated_step = configured.as_ref().is_some_and(|path| {
        path.canonicalize()
            .ok()
            .zip(step_path.canonicalize().ok())
            .is_some_and(|(configured, step)| configured == step)
    });
    if configured.is_some() && !generated_step {
        return CompletionContract::load_for_config(config);
    }
    let path = generated_path(config, ULTRA_RUN_CONTRACT);
    if !path.is_file() {
        return CompletionContract::load_for_config(config);
    }
    let mut bound = config.clone();
    bound.completion_contract_path = Some(path);
    CompletionContract::load_for_config(&bound)
}

pub(crate) fn handoff_commands(config: &Config, existing: &[String]) -> Vec<String> {
    // Preserve the bounded-repair checks already carried by the failed step.
    if !existing.is_empty() {
        return existing.to_vec();
    }
    match load_for_handoff(config) {
        Ok(contract) => contract
            .map(|contract| contract.verify_commands)
            .unwrap_or_default(),
        Err(error) => {
            crate::eval_events::emit(
                config.eval_events_path.as_deref(),
                json!({
                    "event": "recovery_handoff_fidelity_failed",
                    "reason": format!("completion contract authority unavailable: {error}"),
                    "status": "incomplete",
                }),
            );
            Vec::new()
        }
    }
}

pub(crate) fn bind_for_recovery(
    config: &Config,
    failed_plan_verify_commands: &[String],
) -> anyhow::Result<Config> {
    let mut bound = config.clone();
    let (path, source, generated) = match CompletionContract::configured_path_for_config(config)? {
        Some(path) => (path, "configured", false),
        None => {
            let generated = crate::planner::completion_contract_path::generated_path(
                &config.workspace_root,
                config.eval_events_path.as_deref(),
                ULTRA_RUN_CONTRACT,
            );
            if !generated.is_file() {
                return Ok(bound);
            }
            (generated, "generated_ultra_plan_run", true)
        }
    };
    let path = path.canonicalize().with_context(|| {
        format!(
            "canonicalize Recovery completion contract {}",
            path.display()
        )
    })?;
    bound.completion_contract_path = Some(path.clone());
    let contract = CompletionContract::load_for_config(&bound)?
        .context("Recovery completion contract binding resolved no contract")?;
    let registered_from_handoff = if generated {
        complete_generated_verify_commands(config, &path, contract, failed_plan_verify_commands)?
    } else {
        0
    };
    let bytes = std::fs::read(&path)
        .with_context(|| format!("read Recovery completion contract {}", path.display()))?;
    let sha256 = format!("{:x}", Sha256::digest(&bytes));
    let display_path = path
        .strip_prefix(&config.workspace_root)
        .unwrap_or(&path)
        .to_string_lossy()
        .replace('\\', "/");
    crate::eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "recovery_completion_contract_bound",
            "source": source,
            "completion_contract_path": display_path,
            "completion_contract_sha256": sha256,
            "registered_verify_commands_from_failed_plan": registered_from_handoff,
            "external_oracle_used": false,
        }),
    );
    Ok(bound)
}

fn complete_generated_verify_commands(
    config: &Config,
    path: &std::path::Path,
    mut contract: CompletionContract,
    failed_plan_verify_commands: &[String],
) -> anyhow::Result<usize> {
    let profile = contract.profile.clone().unwrap_or_default();
    if !contract.verify_commands.is_empty() || !admits_generated_commands(&profile) {
        return Ok(0);
    }

    let source = if failed_plan_verify_commands.is_empty() {
        contract.verify_commands = profile_commands(
            config,
            &profile,
            contract.goal.as_deref().unwrap_or_default(),
        );
        "profile_runtime"
    } else {
        contract.verify_commands = failed_plan_verify_commands.to_vec();
        "failed_plan_handoff"
    };
    let contract = contract
        .validate(&config.workspace_root)
        .context("validate failed-plan commands for generated Recovery completion contract")?;
    if contract.verify_commands.is_empty() {
        return Ok(0);
    }
    persist_generated_commands(config, path, &contract, source)?;
    Ok(if source == "failed_plan_handoff" {
        contract.verify_commands.len()
    } else {
        0
    })
}

fn persist_generated_commands(
    config: &Config,
    path: &std::path::Path,
    contract: &CompletionContract,
    source: &str,
) -> anyhow::Result<()> {
    let mut bytes = serde_json::to_vec_pretty(contract)
        .context("serialize completed generated Recovery completion contract")?;
    bytes.push(b'\n');
    std::fs::write(path, &bytes).with_context(|| {
        format!(
            "complete generated Recovery completion contract {}",
            path.display()
        )
    })?;
    crate::eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "recovery_generated_completion_contract_completed",
            "source": source,
            "profile": contract.profile,
            "completion_contract_path": path
                .strip_prefix(&config.workspace_root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/"),
            "registered_verify_command_count": contract.verify_commands.len(),
            "external_oracle_used": false,
        }),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn config(root: &std::path::Path) -> Config {
        let mut config =
            Config::from_cli(crate::cli::Cli::parse_from(["commandagent", "--ux-demo"])).unwrap();
        config.workspace_root = root.to_path_buf();
        config.eval_events_path = Some(root.join(".commandagent/runs/session/events.jsonl"));
        std::fs::create_dir_all(
            config
                .eval_events_path
                .as_ref()
                .and_then(|path| path.parent())
                .unwrap(),
        )
        .unwrap();
        config
    }

    fn contract_json() -> String {
        serde_json::to_string(&CompletionContract {
            required_paths: Vec::new(),
            protected_paths: Vec::new(),
            verify_commands: Vec::new(),
            fix_reproducer_command: None,
            profile: Some("nextjs".to_string()),
            goal: Some("create a todo app".to_string()),
            required_capabilities: vec!["persistence".to_string()],
            deterministic_oracles: Vec::new(),
            required_evidence: vec!["persistence_evidence".to_string()],
            evidence_hint_tokens: Vec::new(),
            required_obligations: Vec::new(),
            deferred_verify_requirements: Vec::new(),
            verify_repair_cap: 1,
        })
        .unwrap()
    }

    #[test]
    fn binds_generated_ultra_run_contract_when_config_path_is_missing() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path());
        let contract = crate::planner::completion_contract_path::generated_path(
            root.path(),
            config.eval_events_path.as_deref(),
            ULTRA_RUN_CONTRACT,
        );
        std::fs::create_dir_all(contract.parent().unwrap()).unwrap();
        std::fs::write(&contract, contract_json()).unwrap();

        let bound = bind_for_recovery(&config, &[]).unwrap();

        assert_eq!(
            bound.completion_contract_path,
            Some(contract.canonicalize().unwrap())
        );
        let events = std::fs::read_to_string(config.eval_events_path.unwrap()).unwrap();
        assert!(events.contains("recovery_completion_contract_bound"));
        assert!(events.contains("completion_contract_sha256"));
    }

    #[test]
    fn missing_generated_contract_remains_unconfigured() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path());

        let bound = bind_for_recovery(&config, &[]).unwrap();

        assert!(bound.completion_contract_path.is_none());
    }

    #[test]
    fn generated_nextjs_contract_registers_failed_plan_verify_commands() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path());
        let contract_path = crate::planner::completion_contract_path::generated_path(
            root.path(),
            config.eval_events_path.as_deref(),
            ULTRA_RUN_CONTRACT,
        );
        std::fs::create_dir_all(contract_path.parent().unwrap()).unwrap();
        std::fs::write(&contract_path, contract_json()).unwrap();

        let bound = bind_for_recovery(&config, &["npm run build".to_string()]).unwrap();
        let contract = CompletionContract::load_for_config(&bound)
            .unwrap()
            .unwrap();

        assert_eq!(contract.verify_commands, ["npm run build"]);
        let events = std::fs::read_to_string(config.eval_events_path.unwrap()).unwrap();
        assert!(events.contains("recovery_generated_completion_contract_completed"));
        assert!(events.contains("\"source\":\"failed_plan_handoff\""));
        assert!(events.contains("\"registered_verify_command_count\":1"));
    }

    #[test]
    fn generated_generic_contract_registers_failed_plan_verify_commands() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path());
        let contract_path = crate::planner::completion_contract_path::generated_path(
            root.path(),
            config.eval_events_path.as_deref(),
            ULTRA_RUN_CONTRACT,
        );
        std::fs::create_dir_all(contract_path.parent().unwrap()).unwrap();
        let mut contract: CompletionContract = serde_json::from_str(&contract_json()).unwrap();
        contract.profile = Some("generic".to_string());
        std::fs::write(&contract_path, serde_json::to_vec(&contract).unwrap()).unwrap();

        let bound = bind_for_recovery(&config, &["cargo test".to_string()]).unwrap();
        let contract = CompletionContract::load_for_config(&bound)
            .unwrap()
            .unwrap();

        assert_eq!(contract.verify_commands, ["cargo test"]);
    }

    #[test]
    fn configured_and_data_contracts_do_not_register_handoff_commands() {
        let root = tempfile::tempdir().unwrap();
        let mut configured = config(root.path());
        let configured_path = root.path().join("configured-contract.json");
        std::fs::write(&configured_path, contract_json()).unwrap();
        configured.completion_contract_path = Some(configured_path);

        let bound = bind_for_recovery(&configured, &["npm run build".to_string()]).unwrap();
        assert!(
            CompletionContract::load_for_config(&bound)
                .unwrap()
                .unwrap()
                .verify_commands
                .is_empty()
        );

        let generated = config(root.path());
        let generated_path = crate::planner::completion_contract_path::generated_path(
            root.path(),
            generated.eval_events_path.as_deref(),
            ULTRA_RUN_CONTRACT,
        );
        let mut data_contract: CompletionContract = serde_json::from_str(&contract_json()).unwrap();
        data_contract.profile = Some("data".to_string());
        std::fs::write(&generated_path, serde_json::to_vec(&data_contract).unwrap()).unwrap();

        let bound = bind_for_recovery(
            &generated,
            &["python3 pipeline/main.py data/input.csv".to_string()],
        )
        .unwrap();
        assert!(
            CompletionContract::load_for_config(&bound)
                .unwrap()
                .unwrap()
                .verify_commands
                .is_empty()
        );
    }
}
