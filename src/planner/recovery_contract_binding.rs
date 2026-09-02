//! Host-owned CompletionContract binding for isolated Recovery treatments.

use std::io::Write;
use std::path::Path;

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::config::Config;
use crate::minimal_loop::completion::CompletionContract;

const TREATMENT_CONTRACT_PATH: &str = ".commandagent/recovery-runtime/completion-contract.json";
const FIX_ORIGIN_PATH: &str = ".commandagent/recovery-runtime/fix-origin.json";
const FIX_ORIGIN_EVIDENCE_PATH: &str = ".commandagent/recovery-runtime/fix-origin-evidence.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RecoveryFixOrigin {
    pub(crate) schema_version: String,
    pub(crate) original_intent: String,
    pub(crate) contract_origin: String,
    pub(crate) contract_version: String,
    pub(crate) contract_ref: String,
    pub(crate) fix_run_id: String,
    pub(crate) evidence_path: String,
    pub(crate) evidence_sha256: String,
    pub(crate) reproducer_command: String,
}

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
    let contract = CompletionContract::load_for_config(&bound)
        .context("validate rebound Recovery treatment completion contract")?
        .context("rebound Recovery treatment completion contract is missing")?;
    if !inherit_fix_origin(config, &treatment, &contract)? {
        bind_fix_origin(config, &treatment, &contract)?;
    }
    Ok(bound)
}

pub(crate) fn load_fix_origin(config: &Config) -> anyhow::Result<Option<RecoveryFixOrigin>> {
    let path = config.workspace_root.join(FIX_ORIGIN_PATH);
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = std::fs::read(&path).context("read Recovery fix origin")?;
    let origin: RecoveryFixOrigin =
        serde_json::from_slice(&bytes).context("parse Recovery fix origin")?;
    if origin.schema_version != "1"
        || origin.original_intent != "fix"
        || origin.contract_origin != crate::planner::fix_runtime::FIX_CONTRACT_ORIGIN
        || origin.contract_version != crate::planner::adjudication::contract::FIX_CONTRACT_VERSION
        || origin.contract_ref != crate::planner::adjudication::contract::FIX_CONTRACT_REF
    {
        bail!("Recovery fix origin contract identity is invalid");
    }
    let evidence_path = config.workspace_root.join(&origin.evidence_path);
    let evidence = std::fs::read(&evidence_path).context("read Recovery fix origin evidence")?;
    let observed = format!("{:x}", Sha256::digest(&evidence));
    if observed != origin.evidence_sha256 {
        bail!("Recovery fix origin evidence hash changed");
    }
    Ok(Some(origin))
}

fn bind_fix_origin(
    config: &Config,
    treatment: &Path,
    contract: &CompletionContract,
) -> anyhow::Result<()> {
    let Some(event) = latest_final_acceptance(config)? else {
        return Ok(());
    };
    if event.get("intent").and_then(Value::as_str) != Some("fix") {
        return Ok(());
    }
    if event.get("ok").and_then(Value::as_bool) != Some(false) {
        bail!("successful fix acceptance must not enter automatic Recovery");
    }
    let fix_run_id = event
        .get("fix_run_id")
        .and_then(Value::as_str)
        .context("failed fix acceptance is missing fix_run_id")?;
    if fix_run_id.is_empty()
        || !fix_run_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        bail!("failed fix acceptance has an invalid fix_run_id");
    }
    let reproducer_command = contract
        .fix_reproducer_command
        .as_deref()
        .context("fix Recovery requires a bound fix_reproducer_command")?;
    let adjudication_path = format!("evidence/fix-{fix_run_id}-adjudication.json");
    let source = config.workspace_root.join(&adjudication_path);
    let evidence = std::fs::read(&source).context("read failed fix adjudication evidence")?;
    validate_fix_evidence(&evidence, fix_run_id, reproducer_command)?;
    let treatment_evidence = std::fs::read(treatment.join(&adjudication_path))
        .context("Recovery treatment is missing failed fix adjudication evidence")?;
    if treatment_evidence != evidence {
        bail!("Recovery treatment fix adjudication evidence changed during snapshot");
    }
    let origin = RecoveryFixOrigin {
        schema_version: "1".to_string(),
        original_intent: "fix".to_string(),
        contract_origin: crate::planner::fix_runtime::FIX_CONTRACT_ORIGIN.to_string(),
        contract_version: crate::planner::adjudication::contract::FIX_CONTRACT_VERSION.to_string(),
        contract_ref: crate::planner::adjudication::contract::FIX_CONTRACT_REF.to_string(),
        fix_run_id: fix_run_id.to_string(),
        evidence_path: FIX_ORIGIN_EVIDENCE_PATH.to_string(),
        evidence_sha256: format!("{:x}", Sha256::digest(&evidence)),
        reproducer_command: reproducer_command.to_string(),
    };
    write_read_only_bytes(
        &treatment.join(FIX_ORIGIN_EVIDENCE_PATH),
        &evidence,
        "Recovery fix origin evidence",
    )?;
    write_read_only_json(&treatment.join(FIX_ORIGIN_PATH), &origin)
}

fn inherit_fix_origin(
    config: &Config,
    treatment: &Path,
    contract: &CompletionContract,
) -> anyhow::Result<bool> {
    let Some(origin) = load_fix_origin(config)? else {
        return Ok(false);
    };
    if contract.fix_reproducer_command.as_deref() != Some(origin.reproducer_command.as_str()) {
        bail!("Recovery continuation changed the bound fix reproducer");
    }
    let evidence = std::fs::read(config.workspace_root.join(&origin.evidence_path))
        .context("read inherited Recovery fix origin evidence")?;
    validate_fix_evidence(&evidence, &origin.fix_run_id, &origin.reproducer_command)?;
    write_read_only_bytes(
        &treatment.join(&origin.evidence_path),
        &evidence,
        "inherited Recovery fix origin evidence",
    )?;
    write_read_only_json(&treatment.join(FIX_ORIGIN_PATH), &origin)?;
    Ok(true)
}

fn latest_final_acceptance(config: &Config) -> anyhow::Result<Option<Value>> {
    let Some(path) = config.eval_events_path.as_deref() else {
        return Ok(None);
    };
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("read Recovery source event stream"),
    };
    for line in text.lines().rev() {
        let event: Value = serde_json::from_str(line).context("parse Recovery source event")?;
        if event.get("event").and_then(Value::as_str) == Some("ultra_final_acceptance") {
            return Ok(Some(event));
        }
    }
    Ok(None)
}

fn validate_fix_evidence(
    bytes: &[u8],
    fix_run_id: &str,
    reproducer_command: &str,
) -> anyhow::Result<()> {
    let evidence: Value = serde_json::from_slice(bytes).context("parse failed fix evidence")?;
    if evidence.get("intent").and_then(Value::as_str) != Some("fix")
        || evidence.get("run_id").and_then(Value::as_str) != Some(fix_run_id)
        || evidence.get("contract_ref").and_then(Value::as_str)
            != Some(crate::planner::adjudication::contract::FIX_CONTRACT_REF)
        || evidence.get("contract_version").and_then(Value::as_str)
            != Some(crate::planner::adjudication::contract::FIX_CONTRACT_VERSION)
        || evidence
            .pointer("/adjudication/assurance")
            .and_then(Value::as_str)
            != Some("failed")
        || evidence
            .pointer("/adjudication/requirement_statuses/before_fails")
            .and_then(Value::as_str)
            != Some("passed")
        || evidence
            .pointer("/evidence/before/binding_id")
            .and_then(Value::as_str)
            != Some(reproducer_command)
        || evidence
            .pointer("/evidence/before/outcome")
            .and_then(Value::as_str)
            != Some("failure")
    {
        bail!("failed fix evidence does not match the Recovery contract");
    }
    Ok(())
}

fn write_read_only_json(path: &Path, value: &impl Serialize) -> anyhow::Result<()> {
    let bytes = serde_json::to_vec_pretty(value).context("serialize Recovery fix origin")?;
    let mut bytes = bytes;
    bytes.push(b'\n');
    write_read_only_bytes(path, &bytes, "Recovery fix origin")
}

fn write_read_only_bytes(path: &Path, bytes: &[u8], label: &str) -> anyhow::Result<()> {
    if path.exists() {
        bail!("{label} already exists");
    }
    let parent = path
        .parent()
        .context(format!("{label} parent is unavailable"))?;
    std::fs::create_dir_all(parent).with_context(|| format!("create {label} directory"))?;
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("create {label}"))?;
    output
        .write_all(bytes)
        .with_context(|| format!("write {label}"))?;
    output.sync_all().with_context(|| format!("sync {label}"))?;
    let mut permissions = output
        .metadata()
        .with_context(|| format!("read {label} permissions"))?
        .permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(path, permissions).with_context(|| format!("protect {label}"))?;
    Ok(())
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

    #[test]
    fn binds_failed_fix_origin_to_the_isolated_treatment() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join(".goal-verify-baseline/contract.json");
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        std::fs::write(
            &source,
            serde_json::to_vec(&serde_json::json!({
                "required_paths": ["cli.py"],
                "verify_commands": ["python3 cli.py 11"],
                "fix_reproducer_command": "python3 cli.py 11",
                "profile": "cli"
            }))
            .unwrap(),
        )
        .unwrap();
        let run_id = "01a-test-fix";
        let evidence_path = format!("evidence/fix-{run_id}-adjudication.json");
        std::fs::create_dir_all(root.path().join("evidence")).unwrap();
        let evidence = serde_json::to_vec(&serde_json::json!({
            "schema_version": "1",
            "intent": "fix",
            "contract_version": "v0",
            "contract_ref": "docs/fix-intent-contract.md",
            "run_id": run_id,
            "adjudication": {
                "assurance": "failed",
                "reason": "after_not_executed",
                "requirement_statuses": {
                    "before_fails": "passed",
                    "after_passes": "not_executed",
                    "no_regression": "not_executed"
                }
            },
            "evidence": {
                "before": {
                    "binding_id": "python3 cli.py 11",
                    "outcome": "failure"
                }
            }
        }))
        .unwrap();
        std::fs::write(root.path().join(&evidence_path), &evidence).unwrap();
        let events = root.path().join("events.jsonl");
        std::fs::write(
            &events,
            format!(
                "{}\n",
                serde_json::json!({
                    "event": "ultra_final_acceptance",
                    "intent": "fix",
                    "ok": false,
                    "fix_run_id": run_id
                })
            ),
        )
        .unwrap();
        let treatment = root.path().join("treatment");
        std::fs::create_dir_all(treatment.join("evidence")).unwrap();
        std::fs::write(treatment.join(&evidence_path), &evidence).unwrap();
        let mut source_config = config(root.path());
        source_config.completion_contract_path = Some(source);
        source_config.eval_events_path = Some(events);

        let bound = bind_config(&source_config, &treatment).unwrap();
        let origin = load_fix_origin(&bound).unwrap().unwrap();

        assert_eq!(origin.fix_run_id, run_id);
        assert_eq!(origin.reproducer_command, "python3 cli.py 11");
        assert_eq!(origin.evidence_path, FIX_ORIGIN_EVIDENCE_PATH);
        assert_eq!(
            std::fs::read(treatment.join(FIX_ORIGIN_EVIDENCE_PATH)).unwrap(),
            evidence
        );
        assert!(
            std::fs::metadata(treatment.join(FIX_ORIGIN_PATH))
                .unwrap()
                .permissions()
                .readonly()
        );
    }

    #[test]
    fn inherits_immutable_fix_origin_after_recovery_acceptance_succeeds() {
        let root = tempfile::tempdir().unwrap();
        let source_contract = root.path().join("completion-contract.json");
        std::fs::write(
            &source_contract,
            r#"{"required_paths":["cli.py"],"verify_commands":["python3 cli.py 11"],"fix_reproducer_command":"python3 cli.py 11","profile":"cli"}"#,
        )
        .unwrap();
        let run_id = "01a-continuation";
        let evidence = serde_json::to_vec(&serde_json::json!({
            "schema_version": "1",
            "intent": "fix",
            "contract_version": "v0",
            "contract_ref": "docs/fix-intent-contract.md",
            "run_id": run_id,
            "adjudication": {
                "assurance": "failed",
                "requirement_statuses": {"before_fails": "passed"}
            },
            "evidence": {
                "before": {
                    "binding_id": "python3 cli.py 11",
                    "outcome": "failure"
                }
            }
        }))
        .unwrap();
        std::fs::create_dir_all(root.path().join(".commandagent/recovery-runtime")).unwrap();
        std::fs::write(root.path().join(FIX_ORIGIN_EVIDENCE_PATH), &evidence).unwrap();
        let origin = RecoveryFixOrigin {
            schema_version: "1".to_string(),
            original_intent: "fix".to_string(),
            contract_origin: crate::planner::fix_runtime::FIX_CONTRACT_ORIGIN.to_string(),
            contract_version: crate::planner::adjudication::contract::FIX_CONTRACT_VERSION
                .to_string(),
            contract_ref: crate::planner::adjudication::contract::FIX_CONTRACT_REF.to_string(),
            fix_run_id: run_id.to_string(),
            evidence_path: FIX_ORIGIN_EVIDENCE_PATH.to_string(),
            evidence_sha256: format!("{:x}", Sha256::digest(&evidence)),
            reproducer_command: "python3 cli.py 11".to_string(),
        };
        std::fs::write(
            root.path().join(FIX_ORIGIN_PATH),
            serde_json::to_vec(&origin).unwrap(),
        )
        .unwrap();
        let events = root.path().join("events.jsonl");
        std::fs::write(
            &events,
            format!(
                "{}\n",
                serde_json::json!({
                    "event": "ultra_final_acceptance",
                    "intent": "fix",
                    "ok": true,
                    "fix_run_id": run_id
                })
            ),
        )
        .unwrap();
        let observation = root.path().join("observation");
        std::fs::create_dir(&observation).unwrap();
        let mut source_config = config(root.path());
        source_config.completion_contract_path = Some(source_contract);
        source_config.eval_events_path = Some(events);

        let bound = bind_config(&source_config, &observation).unwrap();
        let inherited = load_fix_origin(&bound).unwrap().unwrap();

        assert_eq!(inherited, origin);
        assert_eq!(
            std::fs::read(observation.join(FIX_ORIGIN_EVIDENCE_PATH)).unwrap(),
            evidence
        );
    }

    #[test]
    fn successful_acceptance_without_fix_origin_cannot_start_recovery() {
        let root = tempfile::tempdir().unwrap();
        let contract = root.path().join("completion-contract.json");
        std::fs::write(
            &contract,
            r#"{"required_paths":["cli.py"],"verify_commands":["python3 cli.py 11"],"fix_reproducer_command":"python3 cli.py 11","profile":"cli"}"#,
        )
        .unwrap();
        let events = root.path().join("events.jsonl");
        std::fs::write(
            &events,
            "{\"event\":\"ultra_final_acceptance\",\"intent\":\"fix\",\"ok\":true,\"fix_run_id\":\"01a-success\"}\n",
        )
        .unwrap();
        let treatment = root.path().join("treatment");
        std::fs::create_dir(&treatment).unwrap();
        let mut source_config = config(root.path());
        source_config.completion_contract_path = Some(contract);
        source_config.eval_events_path = Some(events);

        let error = bind_config(&source_config, &treatment).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("successful fix acceptance must not enter automatic Recovery")
        );
    }
}
