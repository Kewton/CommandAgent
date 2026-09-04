//! Resolve the run-level completion contract before automatic Recovery begins.

use anyhow::Context;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::config::Config;
use crate::minimal_loop::completion::CompletionContract;

const ULTRA_RUN_CONTRACT: &str = "completion-contract-ultra-plan-run.json";

pub(crate) fn bind_for_recovery(config: &Config) -> anyhow::Result<Config> {
    let mut bound = config.clone();
    let (path, source) = match CompletionContract::configured_path_for_config(config)? {
        Some(path) => (path, "configured"),
        None => {
            let generated = crate::planner::completion_contract_path::generated_path(
                &config.workspace_root,
                config.eval_events_path.as_deref(),
                ULTRA_RUN_CONTRACT,
            );
            if !generated.is_file() {
                return Ok(bound);
            }
            (generated, "generated_ultra_plan_run")
        }
    };
    let path = path.canonicalize().with_context(|| {
        format!(
            "canonicalize Recovery completion contract {}",
            path.display()
        )
    })?;
    let bytes = std::fs::read(&path)
        .with_context(|| format!("read Recovery completion contract {}", path.display()))?;
    bound.completion_contract_path = Some(path.clone());
    CompletionContract::load_for_config(&bound)?
        .context("Recovery completion contract binding resolved no contract")?;
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
            "external_oracle_used": false,
        }),
    );
    Ok(bound)
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

        let bound = bind_for_recovery(&config).unwrap();

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

        let bound = bind_for_recovery(&config).unwrap();

        assert!(bound.completion_contract_path.is_none());
    }
}
