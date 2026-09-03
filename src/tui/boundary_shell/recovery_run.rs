use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

const SCHEMA_VERSION: u8 = 1;
const PROPOSAL_DIRECTORY: &str = "boundary-recovery-run-proposals";
const CONFIRMATION_DIRECTORY: &str = "boundary-recovery-run-confirmations";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryRunProposal {
    schema_version: u8,
    pub target_run_id: String,
    pub recovery_round: u32,
    pub source_plan_path: String,
    pub frozen_plan_path: String,
    pub plan_hash: String,
    pub execution_phases: Vec<String>,
    pub permission_policy: String,
    pub automatic_run_budget: u8,
    pub identity_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedRecoveryRun {
    proposal: RecoveryRunProposal,
    confirmation_hash: String,
    artifact_path: PathBuf,
    frozen_plan: PathBuf,
}

#[derive(Clone, Copy, Debug)]
pub struct RecoveryRunBinding<'a> {
    pub target_run_id: &'a str,
    pub identity_hash: &'a str,
    pub permission_policy: &'a str,
    pub automatic_run_budget: u8,
}

impl PersistedRecoveryRun {
    pub fn proposal(&self) -> &RecoveryRunProposal {
        &self.proposal
    }

    pub fn confirmation_hash(&self) -> &str {
        &self.confirmation_hash
    }

    pub fn frozen_plan(&self) -> &Path {
        &self.frozen_plan
    }
}

#[derive(Debug, Error)]
pub enum RecoveryRunError {
    #[error("Recovery Plan drift rejected: {0}")]
    Drift(String),
    #[error("automatic Recovery treatment was rejected: {0}")]
    TreatmentRejected(String),
    #[error("automatic Recovery treatment is unresolved")]
    TreatmentPending,
    #[error("Recovery Run confirmation hash is stale: {0}")]
    Stale(String),
    #[error("Recovery Run confirmation was already used")]
    AlreadyConfirmed,
    #[error("Recovery Run is unavailable: {0}")]
    Invalid(String),
    #[error("Recovery Run state could not be stored: {0:#}")]
    Storage(#[source] anyhow::Error),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfirmationRecord {
    schema_version: u8,
    confirmation_hash: String,
    plan_hash: String,
    target_run_id: String,
    recovery_round: u32,
    confirmed_at_epoch: u64,
}

pub fn propose(
    state_root: &Path,
    workspace: &Path,
    events_path: &Path,
    target_run_id: &str,
    identity_hash: &str,
    permission_policy: &str,
    automatic_run_budget: u8,
) -> Result<PersistedRecoveryRun, RecoveryRunError> {
    validate_fixed_inputs(target_run_id, identity_hash, permission_policy)?;
    require_treatment_allows_run(events_path)?;
    let source = current_plan(workspace, events_path)?;
    let plan = std::str::from_utf8(&source.bytes)
        .map_err(|_| RecoveryRunError::Invalid("resolved Recovery Plan is not UTF-8".into()))
        .and_then(|text| {
            crate::planner::ultra_plan::parse_ultra_plan(text)
                .map_err(|error| RecoveryRunError::Invalid(error.to_string()))
        })?;
    let plan_hash = sha256(&source.bytes);
    let proposal_root = state_root.join(PROPOSAL_DIRECTORY);
    let recovery_round = next_round(&proposal_root)?;
    let frozen_plan_path = format!(
        ".commandagent/plans/recovery-run-{recovery_round}-{}.yaml",
        &plan_hash["sha256:".len()..][..12]
    );
    let frozen_plan = resolve_frozen_plan(workspace, &frozen_plan_path)?;
    persist_exact_bytes(&frozen_plan, &source.bytes, "frozen Recovery Plan")?;
    let proposal = RecoveryRunProposal {
        schema_version: SCHEMA_VERSION,
        target_run_id: target_run_id.to_string(),
        recovery_round,
        source_plan_path: source.display_path,
        frozen_plan_path,
        plan_hash,
        execution_phases: plan.phases.into_iter().map(|phase| phase.id).collect(),
        permission_policy: permission_policy.to_string(),
        automatic_run_budget,
        identity_hash: identity_hash.to_string(),
    };
    validate_proposal(&proposal)?;
    let bytes = artifact_bytes(&proposal)?;
    let confirmation_hash = sha256(&bytes);
    let artifact_path = proposal_root.join(format!(
        "{}.json",
        confirmation_hash.trim_start_matches("sha256:")
    ));
    persist_exact_bytes(&artifact_path, &bytes, "Recovery Run proposal")?;
    Ok(PersistedRecoveryRun {
        proposal,
        confirmation_hash,
        artifact_path,
        frozen_plan,
    })
}

pub fn load_current(
    state_root: &Path,
    workspace: &Path,
    events_path: &Path,
    confirmation_hash: &str,
    binding: RecoveryRunBinding<'_>,
) -> Result<PersistedRecoveryRun, RecoveryRunError> {
    validate_hash(confirmation_hash)?;
    let proposal_root = state_root.join(PROPOSAL_DIRECTORY);
    let artifact_path = proposal_root.join(format!(
        "{}.json",
        confirmation_hash.trim_start_matches("sha256:")
    ));
    let bytes = std::fs::read(&artifact_path)
        .map_err(|_| RecoveryRunError::Stale("proposal artifact was not found".into()))?;
    if sha256(&bytes) != confirmation_hash {
        return Err(RecoveryRunError::Stale(
            "proposal artifact bytes no longer match".into(),
        ));
    }
    let proposal: RecoveryRunProposal = serde_json::from_slice(&bytes)
        .map_err(|error| RecoveryRunError::Stale(format!("proposal cannot be parsed: {error}")))?;
    validate_proposal(&proposal)?;
    let latest_round = latest_round(&proposal_root)?;
    if proposal.recovery_round != latest_round {
        return Err(RecoveryRunError::Stale(
            "a newer Recovery Run proposal exists".into(),
        ));
    }
    if proposal.target_run_id != binding.target_run_id
        || proposal.identity_hash != binding.identity_hash
        || proposal.permission_policy != binding.permission_policy
        || proposal.automatic_run_budget != binding.automatic_run_budget
    {
        return Err(RecoveryRunError::Stale(
            "session identity or execution policy changed".into(),
        ));
    }
    require_treatment_allows_run(events_path)?;
    let source = current_plan(workspace, events_path)?;
    if source.display_path != proposal.source_plan_path {
        return Err(RecoveryRunError::Drift(
            "the resolved Recovery Plan path changed".into(),
        ));
    }
    if sha256(&source.bytes) != proposal.plan_hash {
        return Err(RecoveryRunError::Drift(
            "the resolved Recovery Plan bytes changed".into(),
        ));
    }
    let frozen_plan = resolve_frozen_plan(workspace, &proposal.frozen_plan_path)?;
    let frozen_bytes = std::fs::read(&frozen_plan)
        .map_err(|_| RecoveryRunError::Drift("the frozen Recovery Plan is missing".into()))?;
    if frozen_bytes != source.bytes || sha256(&frozen_bytes) != proposal.plan_hash {
        return Err(RecoveryRunError::Drift(
            "the frozen and resolved Recovery Plan bytes differ".into(),
        ));
    }
    Ok(PersistedRecoveryRun {
        proposal,
        confirmation_hash: confirmation_hash.to_string(),
        artifact_path,
        frozen_plan,
    })
}

pub fn confirm(
    state_root: &Path,
    recovery: &PersistedRecoveryRun,
) -> Result<PathBuf, RecoveryRunError> {
    let bytes = std::fs::read(&recovery.artifact_path)
        .map_err(|_| RecoveryRunError::Stale("proposal artifact was not found".into()))?;
    if sha256(&bytes) != recovery.confirmation_hash {
        return Err(RecoveryRunError::Stale(
            "proposal artifact changed before confirmation".into(),
        ));
    }
    let confirmation_root = state_root.join(CONFIRMATION_DIRECTORY);
    std::fs::create_dir_all(&confirmation_root).map_err(|error| storage(error.into()))?;
    let path = confirmation_root.join(format!(
        "{}.json",
        recovery.confirmation_hash.trim_start_matches("sha256:")
    ));
    let record = ConfirmationRecord {
        schema_version: SCHEMA_VERSION,
        confirmation_hash: recovery.confirmation_hash.clone(),
        plan_hash: recovery.proposal.plan_hash.clone(),
        target_run_id: recovery.proposal.target_run_id.clone(),
        recovery_round: recovery.proposal.recovery_round,
        confirmed_at_epoch: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| storage(anyhow::Error::new(error)))?
            .as_secs(),
    };
    let mut record_bytes =
        serde_json::to_vec_pretty(&record).map_err(|error| storage(error.into()))?;
    record_bytes.push(b'\n');
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                RecoveryRunError::AlreadyConfirmed
            } else {
                storage(error.into())
            }
        })?;
    file.write_all(&record_bytes)
        .map_err(|error| storage(error.into()))?;
    file.sync_all().map_err(|error| storage(error.into()))?;
    Ok(path)
}

struct CurrentPlan {
    display_path: String,
    bytes: Vec<u8>,
}

fn current_plan(workspace: &Path, events_path: &Path) -> Result<CurrentPlan, RecoveryRunError> {
    let terminal = crate::eval_events::latest_tui_command_stop_event(Some(events_path))
        .ok_or_else(|| RecoveryRunError::Invalid("failed terminal evidence is missing".into()))?;
    if terminal.get("ok").and_then(Value::as_bool) != Some(false) {
        return Err(RecoveryRunError::Invalid(
            "current terminal is not a failed Gate 4 execution".into(),
        ));
    }
    let raw = terminal
        .get("recovery_ultra_plan_path")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            RecoveryRunError::Invalid("resolved Recovery Plan path is missing".into())
        })?;
    let canonical_workspace = workspace
        .canonicalize()
        .map_err(|error| RecoveryRunError::Invalid(format!("workspace is unavailable: {error}")))?;
    let resolved = if Path::new(raw).is_absolute() {
        Path::new(raw)
            .canonicalize()
            .map_err(|error| RecoveryRunError::Invalid(format!("resolve Recovery Plan: {error}")))?
    } else {
        crate::tools::path_guard::resolve_existing(&canonical_workspace, raw)
            .map_err(|error| RecoveryRunError::Invalid(error.to_string()))?
    };
    if !resolved.starts_with(&canonical_workspace) {
        return Err(RecoveryRunError::Invalid(
            "resolved Recovery Plan escapes the session workspace".into(),
        ));
    }
    let display_path = resolved
        .strip_prefix(&canonical_workspace)
        .context("strip Recovery Plan workspace prefix")
        .map_err(storage)?
        .to_string_lossy()
        .replace('\\', "/");
    let bytes = std::fs::read(&resolved)
        .with_context(|| format!("read resolved Recovery Plan {}", resolved.display()))
        .map_err(storage)?;
    Ok(CurrentPlan {
        display_path,
        bytes,
    })
}

fn require_treatment_allows_run(events_path: &Path) -> Result<(), RecoveryRunError> {
    let text = std::fs::read_to_string(events_path)
        .with_context(|| format!("read recovery events {}", events_path.display()))
        .map_err(storage)?;
    let mut state = TreatmentState::Ready;
    for event in current_interval_events(&text)? {
        let name = event.get("event").and_then(Value::as_str);
        match name {
            Some("recovery_plan_auto_run_start") => state = TreatmentState::Pending,
            Some("recovery_treatment_promoted") => state = TreatmentState::Ready,
            Some("recovery_control_retained") => {
                state = TreatmentState::Rejected(event_reason(&event))
            }
            Some("recovery_promotion_decision") => {
                state = match event.get("decision").and_then(Value::as_str) {
                    Some("promoted") => TreatmentState::Ready,
                    Some("rejected") => TreatmentState::Rejected(event_reason(&event)),
                    _ => state,
                }
            }
            _ => {}
        }
    }
    match state {
        TreatmentState::Ready => Ok(()),
        TreatmentState::Pending => Err(RecoveryRunError::TreatmentPending),
        TreatmentState::Rejected(reason) => Err(RecoveryRunError::TreatmentRejected(reason)),
    }
}

fn current_interval_events(text: &str) -> Result<Vec<Value>, RecoveryRunError> {
    let mut current = Vec::new();
    let mut terminal_since_boundary = false;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let event: Value = serde_json::from_str(line)
            .map_err(|error| RecoveryRunError::Invalid(format!("invalid event stream: {error}")))?;
        let name = event.get("event").and_then(Value::as_str);
        let boundary = name == Some("human_directive_continuation_started")
            || (name == Some("tui_command_start") && terminal_since_boundary);
        if boundary {
            current.clear();
            terminal_since_boundary = false;
        }
        if matches!(
            name,
            Some("tui_command_stop" | "run_stop" | "gui_trial_stop_completed")
        ) {
            terminal_since_boundary = true;
        }
        current.push(event);
    }
    Ok(current)
}

enum TreatmentState {
    Ready,
    Pending,
    Rejected(String),
}

fn event_reason(event: &Value) -> String {
    event
        .get("reason")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("treatment promotion was rejected")
        .to_string()
}

fn next_round(root: &Path) -> Result<u32, RecoveryRunError> {
    latest_round(root)?
        .checked_add(1)
        .ok_or_else(|| RecoveryRunError::Invalid("Recovery Run round overflow".into()))
}

fn latest_round(root: &Path) -> Result<u32, RecoveryRunError> {
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(storage(error.into())),
    };
    let mut latest = 0;
    for entry in entries {
        let entry = entry.map_err(|error| storage(error.into()))?;
        if entry.path().extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let bytes = std::fs::read(entry.path()).map_err(|error| storage(error.into()))?;
        let proposal: RecoveryRunProposal = serde_json::from_slice(&bytes).map_err(|error| {
            RecoveryRunError::Stale(format!("stored proposal is invalid: {error}"))
        })?;
        latest = latest.max(proposal.recovery_round);
    }
    Ok(latest)
}

fn validate_fixed_inputs(
    target_run_id: &str,
    identity_hash: &str,
    permission_policy: &str,
) -> Result<(), RecoveryRunError> {
    if target_run_id.trim().is_empty() {
        return Err(RecoveryRunError::Invalid("target run ID is empty".into()));
    }
    validate_hash(identity_hash)?;
    if permission_policy.trim().is_empty() {
        return Err(RecoveryRunError::Invalid(
            "permission policy is empty".into(),
        ));
    }
    Ok(())
}

fn validate_proposal(proposal: &RecoveryRunProposal) -> Result<(), RecoveryRunError> {
    if proposal.schema_version != SCHEMA_VERSION
        || proposal.recovery_round == 0
        || proposal.source_plan_path.trim().is_empty()
        || proposal.frozen_plan_path.trim().is_empty()
        || proposal.execution_phases.is_empty()
        || proposal
            .execution_phases
            .iter()
            .any(|phase| phase.trim().is_empty())
    {
        return Err(RecoveryRunError::Stale(
            "proposal fields are incomplete".into(),
        ));
    }
    validate_fixed_inputs(
        &proposal.target_run_id,
        &proposal.identity_hash,
        &proposal.permission_policy,
    )?;
    crate::tools::path_guard::validate_workspace_relative(&proposal.source_plan_path)
        .map_err(|error| RecoveryRunError::Stale(format!("invalid source plan path: {error}")))?;
    crate::tools::path_guard::validate_workspace_relative(&proposal.frozen_plan_path)
        .map_err(|error| RecoveryRunError::Stale(format!("invalid frozen plan path: {error}")))?;
    if !proposal
        .frozen_plan_path
        .starts_with(".commandagent/plans/recovery-run-")
    {
        return Err(RecoveryRunError::Stale(
            "frozen plan path is outside the Recovery Run namespace".into(),
        ));
    }
    validate_hash(&proposal.plan_hash)
}

fn validate_hash(hash: &str) -> Result<(), RecoveryRunError> {
    let valid = hash.strip_prefix("sha256:").is_some_and(|value| {
        value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
    });
    if valid {
        Ok(())
    } else {
        Err(RecoveryRunError::Stale(
            "hash must be sha256 followed by 64 hexadecimal characters".into(),
        ))
    }
}

fn artifact_bytes(proposal: &RecoveryRunProposal) -> Result<Vec<u8>, RecoveryRunError> {
    let mut bytes = serde_json::to_vec_pretty(proposal).map_err(|error| storage(error.into()))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn persist_exact_bytes(path: &Path, bytes: &[u8], label: &str) -> Result<(), RecoveryRunError> {
    let parent = path
        .parent()
        .ok_or_else(|| RecoveryRunError::Invalid(format!("{label} path has no parent")))?;
    std::fs::create_dir_all(parent).map_err(|error| storage(error.into()))?;
    if path.exists() {
        let existing = std::fs::read(path).map_err(|error| storage(error.into()))?;
        if existing != bytes {
            return Err(RecoveryRunError::Drift(format!(
                "{label} path already contains different bytes"
            )));
        }
        return Ok(());
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| storage(error.into()))?;
    file.write_all(bytes)
        .map_err(|error| storage(error.into()))?;
    file.sync_all().map_err(|error| storage(error.into()))
}

fn resolve_frozen_plan(workspace: &Path, relative: &str) -> Result<PathBuf, RecoveryRunError> {
    let candidate = workspace.join(relative);
    match std::fs::symlink_metadata(&candidate) {
        Ok(metadata) if !metadata.file_type().is_file() => Err(RecoveryRunError::Drift(
            "the frozen Recovery Plan path is not a regular file".into(),
        )),
        Ok(_) => crate::tools::path_guard::resolve_existing(workspace, relative)
            .map_err(|error| RecoveryRunError::Drift(error.to_string())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            crate::tools::path_guard::resolve_for_create(workspace, relative)
                .map_err(|error| RecoveryRunError::Invalid(error.to_string()))
        }
        Err(error) => Err(storage(error.into())),
    }
}

fn storage(error: anyhow::Error) -> RecoveryRunError {
    RecoveryRunError::Storage(error)
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let workspace = root.path().join("workspace");
        let plan = workspace.join(".commandagent/plans/recovery.yaml");
        let events = root.path().join("events.jsonl");
        std::fs::create_dir_all(plan.parent().unwrap()).unwrap();
        std::fs::write(
            &plan,
            "goal: \"recover\"\nprofile: \"generic\"\nstyle: \"recovery\"\nintent: \"recover\"\nphases:\n  - id: \"repair\"\n    prompt: \"repair\"\n  - id: \"verify\"\n    prompt: \"verify\"\n",
        )
        .unwrap();
        std::fs::write(
            &events,
            "{\"event\":\"recovery_prompt_saved\",\"recovery_ultra_plan_path\":\".commandagent/plans/recovery.yaml\"}\n{\"event\":\"tui_command_stop\",\"ok\":false,\"status\":\"failed\",\"recovery_ultra_plan_path\":\".commandagent/plans/recovery.yaml\"}\n",
        )
        .unwrap();
        let state = root.path().join("state");
        (root, workspace, events, state)
    }

    fn identity_hash() -> String {
        format!("sha256:{}", "a".repeat(64))
    }

    #[test]
    fn proposal_freezes_exact_bytes_and_binds_visible_execution_fields() {
        let (_root, workspace, events, state) = setup();
        let source = std::fs::read(workspace.join(".commandagent/plans/recovery.yaml")).unwrap();

        let recovery = propose(
            &state,
            &workspace,
            &events,
            "run-415",
            &identity_hash(),
            "read,write,bash:verify",
            2,
        )
        .unwrap();

        assert_eq!(std::fs::read(recovery.frozen_plan()).unwrap(), source);
        assert_eq!(recovery.proposal.plan_hash, sha256(&source));
        assert_eq!(recovery.proposal.execution_phases, ["repair", "verify"]);
        assert_eq!(
            recovery.proposal.permission_policy,
            "read,write,bash:verify"
        );
        assert_eq!(recovery.proposal.automatic_run_budget, 2);
    }

    #[test]
    fn source_and_frozen_plan_drift_are_rejected() {
        let (_root, workspace, events, state) = setup();
        let recovery = propose(
            &state,
            &workspace,
            &events,
            "run-415",
            &identity_hash(),
            "read,write,bash:verify",
            0,
        )
        .unwrap();
        std::fs::write(
            workspace.join(".commandagent/plans/recovery.yaml"),
            "goal: changed\nphases:\n  - id: repair\n    prompt: changed\n",
        )
        .unwrap();

        assert!(matches!(
            load_current(
                &state,
                &workspace,
                &events,
                recovery.confirmation_hash(),
                RecoveryRunBinding {
                    target_run_id: "run-415",
                    identity_hash: &identity_hash(),
                    permission_policy: "read,write,bash:verify",
                    automatic_run_budget: 0,
                },
            ),
            Err(RecoveryRunError::Drift(_))
        ));
    }

    #[test]
    fn newer_proposal_makes_an_older_hash_stale() {
        let (_root, workspace, events, state) = setup();
        let first = propose(
            &state,
            &workspace,
            &events,
            "run-415",
            &identity_hash(),
            "read,write,bash:verify",
            0,
        )
        .unwrap();
        let _second = propose(
            &state,
            &workspace,
            &events,
            "run-415",
            &identity_hash(),
            "read,write,bash:verify",
            0,
        )
        .unwrap();

        assert!(matches!(
            load_current(
                &state,
                &workspace,
                &events,
                first.confirmation_hash(),
                RecoveryRunBinding {
                    target_run_id: "run-415",
                    identity_hash: &identity_hash(),
                    permission_policy: "read,write,bash:verify",
                    automatic_run_budget: 0,
                },
            ),
            Err(RecoveryRunError::Stale(_))
        ));
    }

    #[test]
    fn rejected_and_unresolved_treatments_are_reasoned_denials() {
        let (_root, workspace, events, state) = setup();
        std::fs::write(
            &events,
            "{\"event\":\"recovery_prompt_saved\",\"recovery_ultra_plan_path\":\".commandagent/plans/recovery.yaml\"}\n{\"event\":\"recovery_plan_auto_run_start\"}\n{\"event\":\"recovery_promotion_decision\",\"decision\":\"rejected\",\"reason\":\"verification failed\"}\n{\"event\":\"tui_command_stop\",\"ok\":false}\n",
        )
        .unwrap();
        assert!(matches!(
            propose(
                &state,
                &workspace,
                &events,
                "run-415",
                &identity_hash(),
                "read,write,bash:verify",
                0,
            ),
            Err(RecoveryRunError::TreatmentRejected(reason)) if reason == "verification failed"
        ));

        std::fs::write(
            &events,
            "{\"event\":\"recovery_prompt_saved\",\"recovery_ultra_plan_path\":\".commandagent/plans/recovery.yaml\"}\n{\"event\":\"recovery_plan_auto_run_start\"}\n{\"event\":\"tui_command_stop\",\"ok\":false}\n",
        )
        .unwrap();
        assert!(matches!(
            propose(
                &state,
                &workspace,
                &events,
                "run-415",
                &identity_hash(),
                "read,write,bash:verify",
                0,
            ),
            Err(RecoveryRunError::TreatmentPending)
        ));
    }

    #[test]
    fn confirmation_is_exact_hash_bound_and_one_shot() {
        let (_root, workspace, events, state) = setup();
        let recovery = propose(
            &state,
            &workspace,
            &events,
            "run-415",
            &identity_hash(),
            "read,write,bash:verify",
            0,
        )
        .unwrap();

        confirm(&state, &recovery).unwrap();
        assert!(matches!(
            confirm(&state, &recovery),
            Err(RecoveryRunError::AlreadyConfirmed)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn frozen_plan_symlink_is_rejected_even_when_its_bytes_match() {
        use std::os::unix::fs::symlink;

        let (root, workspace, events, state) = setup();
        let source = std::fs::read(workspace.join(".commandagent/plans/recovery.yaml")).unwrap();
        let outside = root.path().join("outside.yaml");
        std::fs::write(&outside, &source).unwrap();
        let expected_hash = sha256(&source);
        symlink(
            &outside,
            workspace.join(format!(
                ".commandagent/plans/recovery-run-1-{}.yaml",
                &expected_hash["sha256:".len()..][..12]
            )),
        )
        .unwrap();

        assert!(matches!(
            propose(
                &state,
                &workspace,
                &events,
                "run-415",
                &identity_hash(),
                "read,write,bash:verify",
                0,
            ),
            Err(RecoveryRunError::Drift(reason)) if reason.contains("regular file")
        ));
    }
}
