//! Host-owned CompletionContract binding for isolated Recovery treatments.

use std::io::Write;
use std::path::Path;

use anyhow::{Context, bail};

use crate::config::Config;
use crate::minimal_loop::completion::CompletionContract;

const TREATMENT_CONTRACT_PATH: &str = ".commandagent/recovery-runtime/completion-contract.json";

pub(crate) fn bind_config(config: &Config, treatment: &Path) -> anyhow::Result<Config> {
    let treatment = treatment
        .canonicalize()
        .context("Recovery treatment workspace is unavailable for contract binding")?;
    let mut bound = config.clone();
    bound.workspace_root = treatment.clone();
    let Some(source) = CompletionContract::configured_path_for_config(config)? else {
        bound.completion_contract_path = None;
        return Ok(bound);
    };
    let bytes = std::fs::read(&source).with_context(|| {
        format!(
            "read Recovery treatment completion contract {}",
            source.display()
        )
    })?;
    let destination = treatment.join(TREATMENT_CONTRACT_PATH);
    if destination.exists() {
        bail!("Recovery treatment completion contract already exists");
    }
    let parent = destination
        .parent()
        .context("Recovery treatment completion contract parent is unavailable")?;
    std::fs::create_dir_all(parent)
        .context("create host-owned Recovery treatment completion contract directory")?;
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destination)
        .context("create host-owned Recovery treatment completion contract")?;
    output
        .write_all(&bytes)
        .context("copy Recovery treatment completion contract bytes")?;
    output
        .sync_all()
        .context("sync Recovery treatment completion contract")?;
    let mut permissions = output
        .metadata()
        .context("read Recovery treatment completion contract permissions")?
        .permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&destination, permissions)
        .context("protect Recovery treatment completion contract")?;
    let canonical = destination
        .canonicalize()
        .context("canonicalize Recovery treatment completion contract")?;
    if !canonical.starts_with(&treatment) {
        bail!("Recovery treatment completion contract escaped treatment workspace");
    }
    bound.completion_contract_path = Some(canonical);
    CompletionContract::load_for_config(&bound)
        .context("validate rebound Recovery treatment completion contract")?
        .context("rebound Recovery treatment completion contract is missing")?;
    Ok(bound)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn config(root: &Path) -> Config {
        let mut config =
            Config::from_cli(crate::cli::Cli::parse_from(["commandagent", "--ux-demo"])).unwrap();
        config.workspace_root = root.to_path_buf();
        config
    }

    #[test]
    fn copies_exact_contract_bytes_and_rebinds_only_the_treatment() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join(".goal-verify-baseline/contract.json");
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        let bytes = br#"{"required_paths":[],"verify_commands":["true"],"profile":"generic"}"#;
        std::fs::write(&source, bytes).unwrap();
        let treatment = root.path().join("treatment");
        std::fs::create_dir(&treatment).unwrap();
        let mut source_config = config(root.path());
        source_config.completion_contract_path = Some(source.clone());

        let bound = bind_config(&source_config, &treatment).unwrap();
        let destination = bound.completion_contract_path.unwrap();

        assert_eq!(bound.workspace_root, treatment.canonicalize().unwrap());
        assert!(destination.starts_with(treatment.canonicalize().unwrap()));
        assert_eq!(std::fs::read(&destination).unwrap(), bytes);
        assert!(
            std::fs::metadata(&destination)
                .unwrap()
                .permissions()
                .readonly()
        );
        assert_eq!(source_config.completion_contract_path, Some(source));
    }

    #[test]
    fn missing_contract_rejects_treatment_binding() {
        let root = tempfile::tempdir().unwrap();
        let treatment = root.path().join("treatment");
        std::fs::create_dir(&treatment).unwrap();
        let mut source_config = config(root.path());
        source_config.completion_contract_path = Some(root.path().join("missing.json"));

        assert!(bind_config(&source_config, &treatment).is_err());
        assert!(!treatment.join(TREATMENT_CONTRACT_PATH).exists());
    }
}
