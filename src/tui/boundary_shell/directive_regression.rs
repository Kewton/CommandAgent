use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::directive::{
    DEFAULT_MAX_RENDERED_BYTES, DirectiveContinuation, PersistedDirective, render,
};

const REGRESSION_FREEZE_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegressionFreezeArtifact {
    schema_version: u8,
    pub target_run_id: String,
    pub profile: String,
    pub intent: String,
    pub frozen_at_epoch: u64,
    pub source_event: String,
    pub check_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedRegressionFreeze {
    artifact: RegressionFreezeArtifact,
    hash: String,
    path: PathBuf,
}

impl PersistedRegressionFreeze {
    pub fn artifact(&self) -> &RegressionFreezeArtifact {
        &self.artifact
    }

    pub fn hash(&self) -> &str {
        &self.hash
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        let bytes = std::fs::read(&self.path)
            .with_context(|| format!("read regression freeze {}", self.path.display()))?;
        let artifact: RegressionFreezeArtifact = serde_json::from_slice(&bytes)
            .with_context(|| format!("parse regression freeze {}", self.path.display()))?;
        validate_artifact(&artifact)?;
        if artifact != self.artifact || sha256(&bytes) != self.hash {
            bail!("persisted regression freeze changed after confirmation");
        }
        Ok(())
    }
}

pub fn freeze_from_full(
    root: &Path,
    events_path: &Path,
    target_run_id: &str,
    profile: &str,
    intent: &str,
    check_ids: &[String],
) -> anyhow::Result<PersistedRegressionFreeze> {
    let event = latest_stop_event(events_path)?;
    if !is_full(&event) {
        bail!("post-full directive requires an immediately preceding full terminal");
    }
    let event_profile = event
        .get("effective_profile")
        .or_else(|| event.get("profile"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    if canonical_profile(event_profile) != canonical_profile(profile) {
        bail!("regression freeze profile differs from the confirmed profile");
    }
    if check_ids.is_empty() {
        bail!("post-full directive cannot start without a frozen contract check set");
    }
    let mut unique = check_ids.to_vec();
    unique.dedup();
    if unique.len() != check_ids.len() || unique.iter().any(|check| check.trim().is_empty()) {
        bail!("regression freeze check IDs must be non-empty and unique");
    }
    let artifact = RegressionFreezeArtifact {
        schema_version: REGRESSION_FREEZE_SCHEMA_VERSION,
        target_run_id: target_run_id.to_string(),
        profile: profile.to_string(),
        intent: intent.to_string(),
        frozen_at_epoch: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system time before UNIX epoch")?
            .as_secs(),
        source_event: events_path.display().to_string(),
        check_ids: check_ids.to_vec(),
    };
    validate_artifact(&artifact)?;
    let mut bytes = serde_json::to_vec_pretty(&artifact)?;
    bytes.push(b'\n');
    let hash = sha256(&bytes);
    std::fs::create_dir_all(root)
        .with_context(|| format!("create regression freeze directory {}", root.display()))?;
    let path = root.join(format!("{}.json", hash.trim_start_matches("sha256:")));
    if path.is_file() {
        if std::fs::read(&path)? != bytes {
            bail!("regression freeze hash collision or stale artifact");
        }
    } else {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .with_context(|| format!("create regression freeze {}", path.display()))?;
        file.write_all(&bytes)?;
        file.sync_all()?;
    }
    let persisted = PersistedRegressionFreeze {
        artifact,
        hash,
        path,
    };
    persisted.validate()?;
    Ok(persisted)
}

pub fn prepare_modification_continuation(
    workspace: &Path,
    directive: &PersistedDirective,
    freeze: PersistedRegressionFreeze,
    history: Option<&str>,
) -> anyhow::Result<DirectiveContinuation> {
    directive.validate()?;
    freeze.validate()?;
    if directive.artifact().issued_gate != "gate_3" {
        bail!("post-full modification requires a Gate 3 directive");
    }
    if directive.artifact().target_run_id != freeze.artifact.target_run_id {
        bail!("directive and regression freeze target different lineages");
    }
    let rendered_directive = render(directive, DEFAULT_MAX_RENDERED_BYTES)?;
    let mut prompt = format!(
        "Apply the confirmed post-full modification in the existing workspace. Preserve every frozen contract check.\n\
Regression freeze: {}\n\
Frozen checks: {}",
        freeze.hash,
        freeze.artifact.check_ids.join(", ")
    );
    if let Some(history) = history {
        prompt.push_str("\n\n");
        prompt.push_str(history);
    }
    prompt.push_str("\n\n");
    prompt.push_str(&rendered_directive);
    let plan = crate::planner::ultra_plan::UltraPlan {
        goal: "Apply a confirmed post-full boundary directive without regressing acceptance"
            .to_string(),
        profile: freeze.artifact.profile.clone(),
        style: "default".to_string(),
        intent: freeze.artifact.intent.clone(),
        phases: vec![crate::planner::ultra_plan::UltraPhase {
            id: "implement-fix".to_string(),
            prompt,
        }],
    };
    let mut rendered = format!(
        "# anvil-directive-modification\n\
directive_schema_version: \"1\"\n\
directive_round: {}\n\
directive_hash: {}\n\
directive_target_run_id: {}\n\
regression_freeze_hash: {}\n\
regression_check_ids: {}\n",
        directive.artifact().round,
        crate::planner::ultra_plan::quote_yaml_string(directive.hash()),
        crate::planner::ultra_plan::quote_yaml_string(&directive.artifact().target_run_id),
        crate::planner::ultra_plan::quote_yaml_string(freeze.hash()),
        crate::planner::ultra_plan::quote_yaml_string(&freeze.artifact.check_ids.join(",")),
    );
    rendered.push_str(&crate::planner::ultra_plan::render_ultra_plan(&plan));
    let directory = workspace.join(".anvil").join("plans");
    std::fs::create_dir_all(&directory)
        .with_context(|| format!("create modification plan directory {}", directory.display()))?;
    let path = directory.join(format!(
        "directive-round-{}-{}.yaml",
        directive.artifact().round,
        &directive.hash().trim_start_matches("sha256:")[..12]
    ));
    write_immutable(&path, rendered.as_bytes())?;
    Ok(DirectiveContinuation {
        plan_workspace_path: crate::planner::repair::workspace_relative_handoff_path(&path),
        plan_path: path,
        target_run_id: directive.artifact().target_run_id.clone(),
        directive_round: directive.artifact().round,
        directive_hash: directive.hash().to_string(),
        regression_freeze: Some(freeze),
    })
}

pub fn verify_preserved_full(
    freeze: &PersistedRegressionFreeze,
    events_path: &Path,
) -> anyhow::Result<()> {
    freeze.validate()?;
    let event = latest_stop_event(events_path)?;
    if !is_full(&event) {
        bail!(
            "post-full modification regressed frozen checks: {}",
            freeze.artifact.check_ids.join(", ")
        );
    }
    let event_profile = event
        .get("effective_profile")
        .or_else(|| event.get("profile"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    if canonical_profile(event_profile) != canonical_profile(&freeze.artifact.profile) {
        bail!("post-full modification changed the frozen profile");
    }
    Ok(())
}

fn latest_stop_event(path: &Path) -> anyhow::Result<serde_json::Value> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("open regression evidence {}", path.display()))?;
    let mut latest = None;
    for line in BufReader::new(file).lines() {
        let line = line?;
        let value: serde_json::Value = serde_json::from_str(&line)
            .with_context(|| format!("parse regression evidence {}", path.display()))?;
        if value.get("event").and_then(serde_json::Value::as_str) == Some("tui_command_stop") {
            latest = Some(value);
        }
    }
    latest.context("regression freeze requires tui_command_stop evidence")
}

fn is_full(event: &serde_json::Value) -> bool {
    event.get("ok").and_then(serde_json::Value::as_bool) == Some(true)
        && event.get("status").and_then(serde_json::Value::as_str) == Some("completed")
        && event
            .get("assurance_level")
            .and_then(serde_json::Value::as_str)
            == Some("full")
        && matches!(
            event
                .get("final_acceptance_status")
                .and_then(serde_json::Value::as_str),
            Some("full" | "full_success" | "completed")
        )
}

fn validate_artifact(artifact: &RegressionFreezeArtifact) -> anyhow::Result<()> {
    if artifact.schema_version != REGRESSION_FREEZE_SCHEMA_VERSION
        || artifact.target_run_id.trim().is_empty()
        || artifact.profile.trim().is_empty()
        || artifact.intent.trim().is_empty()
        || artifact.check_ids.is_empty()
    {
        bail!("regression freeze artifact is incomplete");
    }
    Ok(())
}

fn write_immutable(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    if path.is_file() {
        if std::fs::read(path)? != bytes {
            bail!("directive modification plan collision or stale plan");
        }
        return Ok(());
    }
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .with_context(|| format!("create directive modification plan {}", path.display()))?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn canonical_profile(profile: &str) -> &str {
    match profile {
        "cli" => "python-cli",
        other => other,
    }
}

fn sha256(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("sha256:{:x}", digest.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_event(path: &Path) {
        std::fs::write(
            path,
            "{\"event\":\"tui_command_stop\",\"ok\":true,\"status\":\"completed\",\"assurance_level\":\"full\",\"final_acceptance_status\":\"full_success\",\"effective_profile\":\"python-cli\"}\n",
        )
        .unwrap();
    }

    #[test]
    fn full_check_set_is_frozen_and_bound_to_modification_plan() {
        let root = tempfile::tempdir().unwrap();
        let events = root.path().join("events.jsonl");
        full_event(&events);
        let checks = vec!["C1".into(), "C2".into(), "C3".into(), "C4".into()];
        let freeze = freeze_from_full(
            &root.path().join("freezes"),
            &events,
            "run-001",
            "python-cli",
            "create",
            &checks,
        )
        .unwrap();
        let directive = super::super::directive::persist_at_epoch_for_gate_for_test(
            &root.path().join("boundary-directives"),
            "change README",
            "run-001",
            1,
            10,
            "gate_3",
        )
        .unwrap();
        let continuation =
            prepare_modification_continuation(root.path(), &directive, freeze, None).unwrap();
        let plan = std::fs::read_to_string(continuation.plan_path).unwrap();
        assert!(plan.contains("regression_check_ids: \"C1,C2,C3,C4\""));
        assert!(continuation.regression_freeze.is_some());
    }

    #[test]
    fn modification_cannot_start_without_a_full_terminal_or_checks() {
        let root = tempfile::tempdir().unwrap();
        let events = root.path().join("events.jsonl");
        std::fs::write(
            &events,
            "{\"event\":\"tui_command_stop\",\"ok\":false,\"status\":\"failed\"}\n",
        )
        .unwrap();
        assert!(
            freeze_from_full(
                root.path(),
                &events,
                "run-001",
                "python-cli",
                "create",
                &["C1".into()]
            )
            .is_err()
        );
        full_event(&events);
        assert!(
            freeze_from_full(root.path(), &events, "run-001", "python-cli", "create", &[]).is_err()
        );
    }
}
