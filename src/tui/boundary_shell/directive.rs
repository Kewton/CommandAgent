use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, bail};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::planner::pack::AssistSource;

const DIRECTIVE_SCHEMA_VERSION: u8 = 1;
const MAX_DIRECTIVE_BYTES: usize = 8 * 1024;
pub const DEFAULT_MAX_RENDERED_BYTES: usize = 24_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectiveArtifact {
    schema_version: u8,
    pub raw: String,
    pub epoch: u64,
    pub target_run_id: String,
    pub round: u32,
    pub issued_gate: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedDirective {
    artifact: DirectiveArtifact,
    hash: String,
    path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectiveContinuation {
    pub plan_path: PathBuf,
    pub plan_workspace_path: String,
    pub target_run_id: String,
    pub directive_round: u32,
    pub directive_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DirectiveRunMetadata {
    schema_version: u8,
    directive_round: u32,
    directive_hash: String,
    target_run_id: String,
    continuation_plan_path: String,
    same_workspace: bool,
    ok: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DirectiveConfirmationRecord {
    schema_version: u8,
    directive_hash: String,
    confirmed_at_epoch: u64,
    target_run_id: String,
    round: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmedDirective {
    directive: PersistedDirective,
    record_path: PathBuf,
}

impl ConfirmedDirective {
    pub fn directive(&self) -> &PersistedDirective {
        &self.directive
    }

    pub fn record_path(&self) -> &Path {
        &self.record_path
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        self.directive.validate()?;
        let bytes = std::fs::read(&self.record_path).with_context(|| {
            format!(
                "read directive confirmation record {}",
                self.record_path.display()
            )
        })?;
        let record: DirectiveConfirmationRecord =
            serde_json::from_slice(&bytes).with_context(|| {
                format!(
                    "parse directive confirmation record {}",
                    self.record_path.display()
                )
            })?;
        let artifact = self.directive.artifact();
        if record.schema_version != DIRECTIVE_SCHEMA_VERSION
            || record.directive_hash != self.directive.hash()
            || record.target_run_id != artifact.target_run_id
            || record.round != artifact.round
        {
            bail!("directive confirmation record does not match the frozen artifact");
        }
        Ok(())
    }
}

impl PersistedDirective {
    pub fn artifact(&self) -> &DirectiveArtifact {
        &self.artifact
    }

    pub fn hash(&self) -> &str {
        &self.hash
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        let bytes = std::fs::read(&self.path)
            .with_context(|| format!("read directive artifact {}", self.path.display()))?;
        let actual_hash = sha256(&bytes);
        let artifact: DirectiveArtifact = serde_json::from_slice(&bytes)
            .with_context(|| format!("parse directive artifact {}", self.path.display()))?;
        validate_artifact(&artifact)?;
        if artifact != self.artifact || actual_hash != self.hash {
            bail!("persisted directive does not match its frozen artifact and hash");
        }
        Ok(())
    }
}

pub fn persist(
    root: &Path,
    raw: &str,
    target_run_id: &str,
    round: u32,
) -> anyhow::Result<PersistedDirective> {
    let epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system time before UNIX epoch")?
        .as_secs();
    persist_at_epoch(root, raw, target_run_id, round, epoch)
}

pub fn confirm(root: &Path, directive: &PersistedDirective) -> anyhow::Result<ConfirmedDirective> {
    let epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system time before UNIX epoch")?
        .as_secs();
    confirm_at_epoch(root, directive, epoch)
}

fn confirm_at_epoch(
    root: &Path,
    directive: &PersistedDirective,
    epoch: u64,
) -> anyhow::Result<ConfirmedDirective> {
    directive.validate()?;
    std::fs::create_dir_all(root)
        .with_context(|| format!("create directive confirmation directory {}", root.display()))?;
    let record_path = root.join(format!(
        "{}.json",
        directive.hash().trim_start_matches("sha256:")
    ));
    let artifact = directive.artifact();
    let record = DirectiveConfirmationRecord {
        schema_version: DIRECTIVE_SCHEMA_VERSION,
        directive_hash: directive.hash().to_string(),
        confirmed_at_epoch: epoch,
        target_run_id: artifact.target_run_id.clone(),
        round: artifact.round,
    };
    if record_path.exists() {
        let bytes = std::fs::read(&record_path).with_context(|| {
            format!(
                "read existing directive confirmation {}",
                record_path.display()
            )
        })?;
        let existing: DirectiveConfirmationRecord = serde_json::from_slice(&bytes)?;
        if existing.directive_hash != record.directive_hash
            || existing.target_run_id != record.target_run_id
            || existing.round != record.round
        {
            bail!("directive confirmation hash collision or stale record");
        }
    } else {
        let mut bytes = serde_json::to_vec_pretty(&record)?;
        bytes.push(b'\n');
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&record_path)
            .with_context(|| {
                format!(
                    "create directive confirmation record {}",
                    record_path.display()
                )
            })?;
        file.write_all(&bytes)?;
        file.sync_all()?;
    }
    let confirmed = ConfirmedDirective {
        directive: directive.clone(),
        record_path,
    };
    confirmed.validate()?;
    Ok(confirmed)
}

fn persist_at_epoch(
    root: &Path,
    raw: &str,
    target_run_id: &str,
    round: u32,
    epoch: u64,
) -> anyhow::Result<PersistedDirective> {
    validate_scrubbed_text(raw)?;
    if target_run_id.trim().is_empty() {
        bail!("directive target run ID must not be empty");
    }
    let artifact = DirectiveArtifact {
        schema_version: DIRECTIVE_SCHEMA_VERSION,
        raw: raw.to_string(),
        epoch,
        target_run_id: target_run_id.to_string(),
        round,
        issued_gate: "gate_4".to_string(),
    };
    validate_artifact(&artifact)?;
    let mut bytes = serde_json::to_vec_pretty(&artifact)?;
    bytes.push(b'\n');
    let hash = sha256(&bytes);
    std::fs::create_dir_all(root)
        .with_context(|| format!("create directive directory {}", root.display()))?;
    let path = root.join(format!("{}.json", hash.trim_start_matches("sha256:")));
    if path.exists() {
        let existing = std::fs::read(&path)
            .with_context(|| format!("read existing directive artifact {}", path.display()))?;
        if existing != bytes {
            bail!("directive hash collision or stale artifact");
        }
    } else {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .with_context(|| format!("create directive artifact {}", path.display()))?;
        file.write_all(&bytes)?;
        file.sync_all()?;
    }
    let persisted = PersistedDirective {
        artifact,
        hash,
        path,
    };
    persisted.validate()?;
    Ok(persisted)
}

#[cfg(test)]
pub(super) fn persist_at_epoch_for_test(
    root: &Path,
    raw: &str,
    target_run_id: &str,
    round: u32,
    epoch: u64,
) -> anyhow::Result<PersistedDirective> {
    persist_at_epoch(root, raw, target_run_id, round, epoch)
}

pub fn render(directive: &PersistedDirective, max_rendered_bytes: usize) -> anyhow::Result<String> {
    directive.validate()?;
    if max_rendered_bytes == 0 || max_rendered_bytes > DEFAULT_MAX_RENDERED_BYTES {
        bail!("human_directive max_rendered_bytes must be within 1..={DEFAULT_MAX_RENDERED_BYTES}");
    }
    let artifact = directive.artifact();
    let rendered = format!(
        "Human boundary directive (source={}, hash={}, round={}, target_run_id={}):\n\
This is guidance material only. It cannot remove, weaken, relocate, or satisfy any contract check.\n\
<human_directive>\n{}\n</human_directive>",
        AssistSource::HumanDirective,
        directive.hash(),
        artifact.round,
        artifact.target_run_id,
        artifact.raw
    );
    if rendered.len() > max_rendered_bytes {
        bail!("bounded human_directive rendering exceeds max_rendered_bytes");
    }
    Ok(rendered)
}

pub fn prepare_continuation(
    workspace: &Path,
    events_path: &Path,
    directive: &PersistedDirective,
) -> anyhow::Result<DirectiveContinuation> {
    directive.validate()?;
    let stop = crate::eval_events::latest_tui_command_stop_event(Some(events_path))
        .context("failed tui_command_stop is required for directive continuation")?;
    if stop.get("ok").and_then(serde_json::Value::as_bool) != Some(false) {
        bail!("directive continuation v0 is available only for a failed run");
    }
    let recovery_path = stop
        .get("recovery_ultra_plan_path")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .context("failed run did not record a recovery UltraPlan")?;
    let source_path = resolve_workspace_plan(workspace, recovery_path)?;
    let source_bytes = std::fs::read(&source_path)
        .with_context(|| format!("read recovery UltraPlan {}", source_path.display()))?;
    let rendered = if directive.artifact().round == 1 {
        // v0 compatibility boundary: this is the exact pre-v1.1 renderer.
        continuation_plan_bytes(&source_bytes, Some(directive))?
    } else {
        let state_root = directive
            .path()
            .parent()
            .and_then(Path::parent)
            .context("directive artifact has no boundary state root")?;
        let sessions_root = state_root.join("boundary-sessions");
        let session = super::directive_session::record_latest_result(
            &sessions_root,
            &directive.artifact().target_run_id,
            directive.artifact().round - 1,
            events_path,
        )?;
        let history = super::directive_session::render_history(
            session.session(),
            directive.artifact().round,
            super::directive_session::MAX_HISTORY_RENDERED_BYTES,
        )?;
        continuation_plan_bytes_with_history(&source_bytes, directive, &history)?
    };
    let directory = workspace.join(".anvil").join("plans");
    std::fs::create_dir_all(&directory)
        .with_context(|| format!("create continuation plan directory {}", directory.display()))?;
    let hash_token = directive.hash().trim_start_matches("sha256:");
    let path = directory.join(format!(
        "directive-round-{}-{}.yaml",
        directive.artifact().round,
        &hash_token[..12]
    ));
    if path.exists() {
        let existing = std::fs::read(&path)
            .with_context(|| format!("read existing continuation plan {}", path.display()))?;
        if existing != rendered {
            bail!("directive continuation plan collision or stale plan");
        }
    } else {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .with_context(|| format!("create directive continuation plan {}", path.display()))?;
        file.write_all(&rendered)?;
        file.sync_all()?;
    }
    let plan_workspace_path = crate::planner::repair::workspace_relative_handoff_path(&path);
    Ok(DirectiveContinuation {
        plan_path: path,
        plan_workspace_path,
        target_run_id: directive.artifact().target_run_id.clone(),
        directive_round: directive.artifact().round,
        directive_hash: directive.hash().to_string(),
    })
}

pub fn persist_run_metadata(
    root: &Path,
    continuation: &DirectiveContinuation,
    ok: bool,
) -> anyhow::Result<PathBuf> {
    std::fs::create_dir_all(root)
        .with_context(|| format!("create directive run metadata directory {}", root.display()))?;
    let path = root.join(format!(
        "{}.json",
        continuation.directive_hash.trim_start_matches("sha256:")
    ));
    let metadata = DirectiveRunMetadata {
        schema_version: DIRECTIVE_SCHEMA_VERSION,
        directive_round: continuation.directive_round,
        directive_hash: continuation.directive_hash.clone(),
        target_run_id: continuation.target_run_id.clone(),
        continuation_plan_path: continuation.plan_workspace_path.clone(),
        same_workspace: true,
        ok,
    };
    let mut bytes = serde_json::to_vec_pretty(&metadata)?;
    bytes.push(b'\n');
    if path.exists() {
        let existing = std::fs::read(&path)
            .with_context(|| format!("read directive run metadata {}", path.display()))?;
        if existing != bytes {
            bail!("directive run metadata already records a different terminal result");
        }
    } else {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .with_context(|| format!("create directive run metadata {}", path.display()))?;
        file.write_all(&bytes)?;
        file.sync_all()?;
    }
    Ok(path)
}

fn continuation_plan_bytes(
    source_bytes: &[u8],
    directive: Option<&PersistedDirective>,
) -> anyhow::Result<Vec<u8>> {
    let Some(directive) = directive else {
        return Ok(source_bytes.to_vec());
    };
    let source_text =
        std::str::from_utf8(source_bytes).context("recovery UltraPlan is not UTF-8")?;
    let mut plan = crate::planner::ultra_plan::parse_ultra_plan(source_text)?;
    let mut repair_phases = plan
        .phases
        .iter_mut()
        .filter(|phase| {
            phase.id == "repair" || phase.id == "implement-fix" || phase.id.starts_with("repair-")
        })
        .collect::<Vec<_>>();
    if repair_phases.len() != 1 {
        bail!(
            "directive continuation requires exactly one implement/repair phase; found {}",
            repair_phases.len()
        );
    }
    let rendered_directive = render(directive, DEFAULT_MAX_RENDERED_BYTES)?;
    repair_phases[0].prompt.push_str("\n\n");
    repair_phases[0].prompt.push_str(&rendered_directive);
    let artifact = directive.artifact();
    let mut rendered = format!(
        "# anvil-directive-continuation\n\
directive_schema_version: \"1\"\n\
directive_round: {}\n\
directive_hash: {}\n\
directive_target_run_id: {}\n",
        artifact.round,
        crate::planner::ultra_plan::quote_yaml_string(directive.hash()),
        crate::planner::ultra_plan::quote_yaml_string(&artifact.target_run_id),
    );
    rendered.push_str(&crate::planner::ultra_plan::render_ultra_plan(&plan));
    Ok(rendered.into_bytes())
}

fn continuation_plan_bytes_with_history(
    source_bytes: &[u8],
    directive: &PersistedDirective,
    history: &str,
) -> anyhow::Result<Vec<u8>> {
    let source_text =
        std::str::from_utf8(source_bytes).context("recovery UltraPlan is not UTF-8")?;
    let mut plan = crate::planner::ultra_plan::parse_ultra_plan(source_text)?;
    let mut repair_phases = plan
        .phases
        .iter_mut()
        .filter(|phase| {
            phase.id == "repair" || phase.id == "implement-fix" || phase.id.starts_with("repair-")
        })
        .collect::<Vec<_>>();
    if repair_phases.len() != 1 {
        bail!(
            "directive continuation requires exactly one implement/repair phase; found {}",
            repair_phases.len()
        );
    }
    let rendered_directive = render(directive, DEFAULT_MAX_RENDERED_BYTES)?;
    repair_phases[0].prompt.push_str("\n\n");
    repair_phases[0].prompt.push_str(history);
    repair_phases[0].prompt.push_str("\n\n");
    repair_phases[0].prompt.push_str(&rendered_directive);
    let artifact = directive.artifact();
    let mut rendered = format!(
        "# anvil-directive-continuation\n\
directive_schema_version: \"1\"\n\
directive_round: {}\n\
directive_hash: {}\n\
directive_target_run_id: {}\n",
        artifact.round,
        crate::planner::ultra_plan::quote_yaml_string(directive.hash()),
        crate::planner::ultra_plan::quote_yaml_string(&artifact.target_run_id),
    );
    rendered.push_str(&crate::planner::ultra_plan::render_ultra_plan(&plan));
    Ok(rendered.into_bytes())
}

fn resolve_workspace_plan(workspace: &Path, raw: &str) -> anyhow::Result<PathBuf> {
    let workspace = workspace
        .canonicalize()
        .context("workspace root is not accessible")?;
    let path = Path::new(raw);
    let resolved = if path.is_absolute() {
        path.canonicalize()
            .with_context(|| format!("resolve recovery UltraPlan {raw}"))?
    } else {
        crate::tools::path_guard::resolve_existing(&workspace, raw)?
    };
    if !resolved.starts_with(&workspace) {
        bail!("recovery UltraPlan escapes the continued workspace");
    }
    Ok(resolved)
}

pub(super) fn validate_artifact(artifact: &DirectiveArtifact) -> anyhow::Result<()> {
    if artifact.schema_version != DIRECTIVE_SCHEMA_VERSION {
        bail!("unsupported directive artifact schema version");
    }
    validate_scrubbed_text(&artifact.raw)?;
    if artifact.target_run_id.trim().is_empty() {
        bail!("directive target run ID must not be empty");
    }
    if artifact.round == 0 {
        bail!("directive round must be positive");
    }
    if artifact.issued_gate != "gate_4" {
        bail!("directive issued_gate must be gate_4");
    }
    Ok(())
}

fn validate_scrubbed_text(raw: &str) -> anyhow::Result<()> {
    if raw.trim().is_empty() {
        bail!("directive must not be empty");
    }
    if raw.len() > MAX_DIRECTIVE_BYTES {
        bail!("directive exceeds the {MAX_DIRECTIVE_BYTES}-byte bound");
    }
    for pattern in credential_patterns()? {
        if pattern.is_match(raw) {
            bail!("directive rejected by credential scrub; no artifact was written");
        }
    }
    Ok(())
}

fn credential_patterns() -> anyhow::Result<Vec<Regex>> {
    [
        r"sk-[A-Za-z0-9_-]{16,}",
        r"AIza[0-9A-Za-z_-]{35}",
        r"gh[pousr]_[A-Za-z0-9]{30,}",
        r"xox[a-z]-[A-Za-z0-9-]{16,}",
        r"AKIA[0-9A-Z]{16}",
        r"eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}",
        r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----",
        r"(?i)(?:api[_-]?key|secret|token|authorization)\s*[:=]\s*[^\s]{16,}",
    ]
    .into_iter()
    .map(Regex::new)
    .collect::<Result<Vec<_>, _>>()
    .context("compile directive credential scrub patterns")
}

fn sha256(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("sha256:{:x}", digest.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_is_exact_byte_hashed_and_rendered_verbatim() {
        let root = tempfile::tempdir().unwrap();
        let raw = "README.mdの出力例を実行結果に合わせて修正してください";
        let directive = persist_at_epoch(root.path(), raw, "run-001", 1, 1_722_400_000).unwrap();
        let bytes = std::fs::read(directive.path()).unwrap();
        assert_eq!(directive.hash(), sha256(&bytes));
        assert_eq!(directive.artifact().raw, raw);
        let rendered = render(&directive, DEFAULT_MAX_RENDERED_BYTES).unwrap();
        assert!(rendered.contains(&format!("source={}", AssistSource::HumanDirective)));
        assert!(rendered.contains(raw));
        assert!(rendered.contains("cannot remove, weaken, relocate, or satisfy"));
    }

    #[test]
    fn credential_scrub_rejects_before_writing() {
        let root = tempfile::tempdir().unwrap();
        let error = persist_at_epoch(
            root.path(),
            "use token=abcdefghijklmnopsecretvalue",
            "run-001",
            1,
            1,
        )
        .unwrap_err();
        assert!(error.to_string().contains("credential scrub"));
        assert!(std::fs::read_dir(root.path()).unwrap().next().is_none());
    }

    #[test]
    fn strict_artifact_rejects_unknown_fields_and_invalid_round() {
        let raw = br#"{
          "schema_version": 1,
          "raw": "repair README",
          "epoch": 1,
          "target_run_id": "run-001",
          "round": 0,
          "issued_gate": "gate_4",
          "unexpected": true
        }"#;
        assert!(serde_json::from_slice::<DirectiveArtifact>(raw).is_err());
        let root = tempfile::tempdir().unwrap();
        assert!(persist_at_epoch(root.path(), "repair README", "run-001", 0, 1).is_err());
    }

    #[test]
    fn persisted_bytes_are_immutable() {
        let root = tempfile::tempdir().unwrap();
        let first = persist_at_epoch(root.path(), "repair README", "run-001", 1, 1).unwrap();
        let second = persist_at_epoch(root.path(), "repair README", "run-001", 1, 1).unwrap();
        assert_eq!(first.hash(), second.hash());
        assert_eq!(first.path(), second.path());
    }

    #[test]
    fn failed_run_recovery_plan_gets_directive_only_in_repair_phase() {
        let root = tempfile::tempdir().unwrap();
        let plans = root.path().join(".anvil/plans");
        std::fs::create_dir_all(&plans).unwrap();
        let source_path = plans.join("recovery.yaml");
        let plan = crate::planner::ultra_plan::UltraPlan {
            goal: "repair the CLI".to_string(),
            profile: "python-cli".to_string(),
            style: "default".to_string(),
            intent: "recover".to_string(),
            phases: vec![
                crate::planner::ultra_plan::UltraPhase {
                    id: "inspect-current-state".to_string(),
                    prompt: "inspect".to_string(),
                },
                crate::planner::ultra_plan::UltraPhase {
                    id: "repair-final-acceptance".to_string(),
                    prompt: "repair".to_string(),
                },
                crate::planner::ultra_plan::UltraPhase {
                    id: "verify-recovery".to_string(),
                    prompt: "verify".to_string(),
                },
            ],
        };
        let source = crate::planner::ultra_plan::render_ultra_plan(&plan);
        std::fs::write(&source_path, &source).unwrap();
        let events = root.path().join("events.jsonl");
        crate::eval_events::emit(
            Some(&events),
            serde_json::json!({
                "event": "tui_command_stop",
                "ok": false,
                "recovery_ultra_plan_path": ".anvil/plans/recovery.yaml",
            }),
        );
        let directive = persist_at_epoch(
            &root.path().join("boundary-directives"),
            "repair README",
            "run-001",
            1,
            1,
        )
        .unwrap();

        let continuation = prepare_continuation(root.path(), &events, &directive).unwrap();
        assert_eq!(continuation.target_run_id, "run-001");
        assert_eq!(continuation.directive_round, 1);
        assert!(
            continuation
                .plan_workspace_path
                .starts_with(".anvil/plans/")
        );
        let derived_bytes = std::fs::read(&continuation.plan_path).unwrap();
        assert_eq!(
            derived_bytes,
            continuation_plan_bytes(source.as_bytes(), Some(&directive)).unwrap(),
            "the v0 round-1 continuation bytes are the compatibility fixture"
        );
        let derived = String::from_utf8(derived_bytes).unwrap();
        let parsed = crate::planner::ultra_plan::parse_ultra_plan(&derived).unwrap();
        assert_eq!(parsed.phases[0].prompt, "inspect");
        assert!(parsed.phases[1].prompt.contains("source=human_directive"));
        assert!(parsed.phases[1].prompt.contains("repair README"));
        assert_eq!(parsed.phases[2].prompt, "verify");
        assert_eq!(std::fs::read_to_string(source_path).unwrap(), source);
    }

    #[test]
    fn round_two_prompt_contains_round_one_directive_and_evidence_result() {
        let root = tempfile::tempdir().unwrap();
        let plans = root.path().join(".anvil/plans");
        let artifacts = root.path().join("boundary-directives");
        let sessions = root.path().join("boundary-sessions");
        std::fs::create_dir_all(&plans).unwrap();
        let source_path = plans.join("recovery.yaml");
        let plan = crate::planner::ultra_plan::UltraPlan {
            goal: "repair the CLI".to_string(),
            profile: "python-cli".to_string(),
            style: "default".to_string(),
            intent: "recover".to_string(),
            phases: vec![crate::planner::ultra_plan::UltraPhase {
                id: "repair-final-acceptance".to_string(),
                prompt: "repair".to_string(),
            }],
        };
        std::fs::write(
            &source_path,
            crate::planner::ultra_plan::render_ultra_plan(&plan),
        )
        .unwrap();
        let first = persist_at_epoch(
            &artifacts,
            "README outputを実行結果に合わせる",
            "run-001",
            1,
            10,
        )
        .unwrap();
        super::super::directive_session::record_directive(&sessions, &artifacts, &first).unwrap();
        let second =
            persist_at_epoch(&artifacts, "起動例をpython3へ戻す", "run-001", 2, 20).unwrap();
        super::super::directive_session::record_directive(&sessions, &artifacts, &second).unwrap();
        let events = root.path().join("events.jsonl");
        crate::eval_events::emit(
            Some(&events),
            serde_json::json!({
                "event": "tui_command_stop",
                "ok": false,
                "status": "failed",
                "stop_reason": "cli_readme_structure:cli_invocation_missing",
                "recovery_ultra_plan_path": ".anvil/plans/recovery.yaml",
            }),
        );

        let continuation = prepare_continuation(root.path(), &events, &second).unwrap();
        let derived = std::fs::read_to_string(continuation.plan_path).unwrap();
        let parsed = crate::planner::ultra_plan::parse_ultra_plan(&derived).unwrap();
        let prompt = &parsed.phases[0].prompt;
        assert!(prompt.contains("material=session_history"));
        assert!(prompt.contains("README outputを実行結果に合わせる"));
        assert!(prompt.contains("result_verdict: failed"));
        assert!(prompt.contains("cli_readme_structure:cli_invocation_missing"));
        assert!(prompt.contains("起動例をpython3へ戻す"));
    }

    #[test]
    fn directive_absence_preserves_recovery_plan_bytes_exactly() {
        let source = b"# existing header\r\ngoal: \"x\"\r\ncustom spacing\r\n";
        assert_eq!(continuation_plan_bytes(source, None).unwrap(), source);
    }

    #[test]
    fn full_run_and_plan_without_one_repair_phase_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        let plans = root.path().join(".anvil/plans");
        std::fs::create_dir_all(&plans).unwrap();
        let source = crate::planner::ultra_plan::UltraPlan::deterministic(
            "goal",
            "python-cli",
            "default",
            "create",
        );
        std::fs::write(
            plans.join("recovery.yaml"),
            crate::planner::ultra_plan::render_ultra_plan(&source),
        )
        .unwrap();
        let events = root.path().join("events.jsonl");
        crate::eval_events::emit(
            Some(&events),
            serde_json::json!({
                "event": "tui_command_stop",
                "ok": true,
                "recovery_ultra_plan_path": ".anvil/plans/recovery.yaml",
            }),
        );
        let directive = persist_at_epoch(
            &root.path().join("boundary-directives"),
            "repair README",
            "run-001",
            1,
            1,
        )
        .unwrap();
        assert!(prepare_continuation(root.path(), &events, &directive).is_err());

        crate::eval_events::emit(
            Some(&events),
            serde_json::json!({
                "event": "tui_command_stop",
                "ok": false,
                "recovery_ultra_plan_path": ".anvil/plans/recovery.yaml",
            }),
        );
        assert!(prepare_continuation(root.path(), &events, &directive).is_err());
    }

    #[test]
    fn directive_run_metadata_records_only_the_nonzero_round_configuration() {
        let root = tempfile::tempdir().unwrap();
        let continuation = DirectiveContinuation {
            plan_path: root.path().join("plan.yaml"),
            plan_workspace_path: ".anvil/plans/plan.yaml".to_string(),
            target_run_id: "run-001".to_string(),
            directive_round: 1,
            directive_hash: format!("sha256:{}", "a".repeat(64)),
        };
        let path = persist_run_metadata(root.path(), &continuation, false).unwrap();
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(value["directive_round"], 1);
        assert_eq!(value["directive_hash"], continuation.directive_hash);
        assert_eq!(value["same_workspace"], true);
        assert_eq!(value["ok"], false);
    }
}
