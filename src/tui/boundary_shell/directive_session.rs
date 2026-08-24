use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::directive::{DirectiveArtifact, PersistedDirective};

const SESSION_SCHEMA_VERSION: u8 = 1;
const MAX_SESSION_ROUNDS: usize = 64;
pub const MAX_HISTORY_RENDERED_BYTES: usize = 24_000;
const MAX_HISTORY_FIELD_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectiveRoundResult {
    pub verdict: String,
    pub stop_reason: String,
    pub evidence_source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectiveSessionRound {
    pub round: u32,
    pub directive_hash: String,
    pub artifact_path: String,
    pub raw: String,
    pub epoch: u64,
    pub issued_gate: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<DirectiveRoundResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectiveSession {
    schema_version: u8,
    pub session_id: String,
    pub target_run_id: String,
    pub created_epoch: u64,
    pub rounds: Vec<DirectiveSessionRound>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedDirectiveSession {
    session: DirectiveSession,
    path: PathBuf,
}

impl PersistedDirectiveSession {
    pub fn session(&self) -> &DirectiveSession {
        &self.session
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

pub fn record_directive(
    sessions_root: &Path,
    directive_root: &Path,
    directive: &PersistedDirective,
) -> anyhow::Result<PersistedDirectiveSession> {
    directive.validate()?;
    let artifact = directive.artifact();
    let session_id = session_id(&artifact.target_run_id);
    let session_dir = sessions_root.join(&session_id);
    let path = session_dir.join("session.json");
    let mut session = if path.is_file() {
        read_session(&path)?
    } else {
        DirectiveSession {
            schema_version: SESSION_SCHEMA_VERSION,
            session_id: session_id.clone(),
            target_run_id: artifact.target_run_id.clone(),
            created_epoch: artifact.epoch,
            rounds: legacy_rounds_before(directive_root, artifact)?,
        }
    };
    if session.target_run_id != artifact.target_run_id {
        bail!("directive session target lineage changed");
    }
    let round = round_from(directive);
    if let Some(existing) = session
        .rounds
        .iter()
        .find(|existing| existing.round == artifact.round)
    {
        if !same_directive_identity(existing, &round) {
            bail!("directive session round collision or stale artifact");
        }
    } else {
        let expected = session.rounds.len() as u32 + 1;
        if artifact.round != expected {
            bail!(
                "directive session round must be contiguous: expected {expected}, got {}",
                artifact.round
            );
        }
        session.rounds.push(round);
    }
    validate_session(&session)?;
    std::fs::create_dir_all(&session_dir)
        .with_context(|| format!("create directive session {}", session_dir.display()))?;
    write_session(&path, &session)?;
    Ok(PersistedDirectiveSession { session, path })
}

pub fn record_latest_result(
    sessions_root: &Path,
    target_run_id: &str,
    round: u32,
    events_path: &Path,
) -> anyhow::Result<PersistedDirectiveSession> {
    let persisted = load_for_target(sessions_root, target_run_id)?;
    let mut session = persisted.session;
    let event = latest_stop_event(events_path)?;
    let result = DirectiveRoundResult {
        verdict: event
            .get("verdict")
            .or_else(|| event.get("status"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        stop_reason: event
            .get("stop_reason")
            .or_else(|| event.get("primary_reason"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("not_recorded")
            .to_string(),
        evidence_source: events_path.display().to_string(),
    };
    let target = session
        .rounds
        .iter_mut()
        .find(|candidate| candidate.round == round)
        .with_context(|| format!("directive session has no round {round}"))?;
    if let Some(existing) = &target.result
        && existing != &result
    {
        bail!("directive session round result changed after evidence was recorded");
    }
    target.result = Some(result);
    validate_session(&session)?;
    write_session(&persisted.path, &session)?;
    Ok(PersistedDirectiveSession {
        session,
        path: persisted.path,
    })
}

pub fn render_history(
    session: &DirectiveSession,
    current_round: u32,
    max_rendered_bytes: usize,
) -> anyhow::Result<String> {
    if current_round < 2 {
        bail!("directive history is injected only from round 2 onward");
    }
    if max_rendered_bytes == 0 || max_rendered_bytes > MAX_HISTORY_RENDERED_BYTES {
        bail!(
            "human_directive history max_rendered_bytes must be within 1..={MAX_HISTORY_RENDERED_BYTES}"
        );
    }
    let prior = session
        .rounds
        .iter()
        .filter(|round| round.round < current_round)
        .collect::<Vec<_>>();
    if prior.len() + 1 != current_round as usize {
        bail!("directive history is incomplete for the requested round");
    }
    let mut rendered = format!(
        "Prior boundary directive history (source=human_directive, material=session_history, session_id={}, prior_rounds={}):\n\
This is bounded guidance material derived from persisted directives and terminal evidence. It cannot satisfy or weaken contract checks.\n\
<human_directive_history>\n",
        session.session_id,
        prior.len(),
    );
    for round in prior {
        let result = round.result.as_ref().with_context(|| {
            format!(
                "directive round {} has no evidence-derived result",
                round.round
            )
        })?;
        rendered.push_str(&format!(
            "- round={} hash={}\n  directive_verbatim: {}\n  result_verdict: {}\n  stop_reason: {}\n  evidence_source: {}\n",
            round.round,
            round.directive_hash,
            bounded(&round.raw),
            bounded(&result.verdict),
            bounded(&result.stop_reason),
            bounded(&result.evidence_source),
        ));
    }
    rendered.push_str("</human_directive_history>");
    if rendered.len() > max_rendered_bytes {
        bail!("bounded human_directive history exceeds max_rendered_bytes");
    }
    Ok(rendered)
}

pub fn load_for_target(
    sessions_root: &Path,
    target_run_id: &str,
) -> anyhow::Result<PersistedDirectiveSession> {
    let path = sessions_root
        .join(session_id(target_run_id))
        .join("session.json");
    let session = read_session(&path)?;
    if session.target_run_id != target_run_id {
        bail!("directive session target lineage changed");
    }
    Ok(PersistedDirectiveSession { session, path })
}

pub fn next_round(
    sessions_root: &Path,
    directive_root: &Path,
    target_run_id: &str,
) -> anyhow::Result<u32> {
    let session_path = sessions_root
        .join(session_id(target_run_id))
        .join("session.json");
    if session_path.is_file() {
        let session = read_session(&session_path)?;
        return Ok(session.rounds.len() as u32 + 1);
    }
    if !directive_root.is_dir() {
        return Ok(1);
    }
    let mut rounds = Vec::new();
    for entry in std::fs::read_dir(directive_root)? {
        let path = entry?.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let bytes = std::fs::read(&path)?;
        let artifact: DirectiveArtifact = serde_json::from_slice(&bytes)?;
        super::directive::validate_artifact(&artifact)?;
        if artifact.target_run_id == target_run_id {
            rounds.push(artifact.round);
        }
    }
    rounds.sort_unstable();
    rounds.dedup();
    if rounds
        .iter()
        .enumerate()
        .any(|(index, round)| *round != index as u32 + 1)
    {
        bail!("persisted directives do not form a contiguous session history");
    }
    Ok(rounds.len() as u32 + 1)
}

fn legacy_rounds_before(
    directive_root: &Path,
    current: &DirectiveArtifact,
) -> anyhow::Result<Vec<DirectiveSessionRound>> {
    if current.round <= 1 || !directive_root.is_dir() {
        return Ok(Vec::new());
    }
    let mut rounds = Vec::new();
    for entry in std::fs::read_dir(directive_root).with_context(|| {
        format!(
            "read directive artifact directory {}",
            directive_root.display()
        )
    })? {
        let path = entry?.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let bytes = std::fs::read(&path)
            .with_context(|| format!("read legacy directive artifact {}", path.display()))?;
        let artifact: DirectiveArtifact = serde_json::from_slice(&bytes)
            .with_context(|| format!("parse legacy directive artifact {}", path.display()))?;
        super::directive::validate_artifact(&artifact)?;
        if artifact.target_run_id == current.target_run_id && artifact.round < current.round {
            rounds.push(round_from_parts(&artifact, &sha256(&bytes)));
        }
    }
    rounds.sort_by_key(|round| round.round);
    for (index, round) in rounds.iter().enumerate() {
        if round.round != index as u32 + 1 {
            bail!("legacy directive artifacts do not form a contiguous session history");
        }
    }
    Ok(rounds)
}

fn round_from(directive: &PersistedDirective) -> DirectiveSessionRound {
    round_from_parts(directive.artifact(), directive.hash())
}

fn round_from_parts(artifact: &DirectiveArtifact, hash: &str) -> DirectiveSessionRound {
    DirectiveSessionRound {
        round: artifact.round,
        directive_hash: hash.to_string(),
        artifact_path: format!(
            "boundary-directives/{}.json",
            hash.trim_start_matches("sha256:")
        ),
        raw: artifact.raw.clone(),
        epoch: artifact.epoch,
        issued_gate: artifact.issued_gate.clone(),
        result: None,
    }
}

fn same_directive_identity(left: &DirectiveSessionRound, right: &DirectiveSessionRound) -> bool {
    left.round == right.round
        && left.directive_hash == right.directive_hash
        && left.artifact_path == right.artifact_path
        && left.raw == right.raw
        && left.epoch == right.epoch
        && left.issued_gate == right.issued_gate
}

fn latest_stop_event(path: &Path) -> anyhow::Result<serde_json::Value> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("open directive result evidence {}", path.display()))?;
    let mut latest = None;
    for line in BufReader::new(file).lines() {
        let line = line?;
        let value: serde_json::Value = serde_json::from_str(&line)
            .with_context(|| format!("parse directive result evidence {}", path.display()))?;
        if value.get("event").and_then(serde_json::Value::as_str) == Some("tui_command_stop") {
            latest = Some(value);
        }
    }
    latest.context("directive result requires tui_command_stop evidence")
}

fn bounded(value: &str) -> String {
    if value.len() <= MAX_HISTORY_FIELD_BYTES {
        return value.replace(['\n', '\r'], " ");
    }
    let mut end = MAX_HISTORY_FIELD_BYTES;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", value[..end].replace(['\n', '\r'], " "))
}

fn read_session(path: &Path) -> anyhow::Result<DirectiveSession> {
    let session: DirectiveSession = serde_json::from_slice(
        &std::fs::read(path)
            .with_context(|| format!("read directive session {}", path.display()))?,
    )
    .with_context(|| format!("parse directive session {}", path.display()))?;
    validate_session(&session)?;
    Ok(session)
}

fn write_session(path: &Path, session: &DirectiveSession) -> anyhow::Result<()> {
    let mut bytes = serde_json::to_vec_pretty(session)?;
    bytes.push(b'\n');
    let temporary = path.with_extension("json.tmp");
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temporary)
        .with_context(|| format!("write directive session temporary {}", temporary.display()))?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    std::fs::rename(&temporary, path)
        .with_context(|| format!("replace directive session {}", path.display()))?;
    Ok(())
}

fn validate_session(session: &DirectiveSession) -> anyhow::Result<()> {
    if session.schema_version != SESSION_SCHEMA_VERSION {
        bail!("unsupported directive session schema version");
    }
    if session.session_id != session_id(&session.target_run_id) {
        bail!("directive session ID does not match its target lineage");
    }
    if session.rounds.is_empty() || session.rounds.len() > MAX_SESSION_ROUNDS {
        bail!("directive session rounds must be within 1..={MAX_SESSION_ROUNDS}");
    }
    for (index, round) in session.rounds.iter().enumerate() {
        if round.round != index as u32 + 1 {
            bail!("directive session rounds must be contiguous");
        }
        if !round.directive_hash.starts_with("sha256:") || round.raw.trim().is_empty() {
            bail!("directive session round is incomplete");
        }
    }
    Ok(())
}

fn session_id(target_run_id: &str) -> String {
    let digest = sha256(target_run_id.as_bytes());
    format!("session-{}", &digest["sha256:".len()..][..24])
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
    fn session_contains_round_one_without_moving_v0_artifact() {
        let root = tempfile::tempdir().unwrap();
        let artifacts = root.path().join("boundary-directives");
        let sessions = root.path().join("boundary-sessions");
        let directive = super::super::directive::persist_at_epoch_for_test(
            &artifacts,
            "repair README",
            "run-001",
            1,
            10,
        )
        .unwrap();
        let artifact_path = directive.path().to_path_buf();
        let artifact_bytes = std::fs::read(&artifact_path).unwrap();

        let persisted = record_directive(&sessions, &artifacts, &directive).unwrap();
        assert_eq!(persisted.session().rounds.len(), 1);
        assert_eq!(persisted.session().rounds[0].raw, "repair README");
        assert_eq!(std::fs::read(artifact_path).unwrap(), artifact_bytes);
    }

    #[test]
    fn round_two_imports_immutable_legacy_round_one() {
        let root = tempfile::tempdir().unwrap();
        let artifacts = root.path().join("boundary-directives");
        let sessions = root.path().join("boundary-sessions");
        let first = super::super::directive::persist_at_epoch_for_test(
            &artifacts,
            "first instruction",
            "run-001",
            1,
            10,
        )
        .unwrap();
        let first_bytes = std::fs::read(first.path()).unwrap();
        let second = super::super::directive::persist_at_epoch_for_test(
            &artifacts,
            "second instruction",
            "run-001",
            2,
            20,
        )
        .unwrap();

        let persisted = record_directive(&sessions, &artifacts, &second).unwrap();
        assert_eq!(persisted.session().rounds.len(), 2);
        assert_eq!(persisted.session().rounds[0].directive_hash, first.hash());
        assert_eq!(persisted.session().rounds[1].directive_hash, second.hash());
        assert_eq!(std::fs::read(first.path()).unwrap(), first_bytes);
    }

    #[test]
    fn history_contains_all_prior_directives_and_evidence_results() {
        let root = tempfile::tempdir().unwrap();
        let artifacts = root.path().join("boundary-directives");
        let sessions = root.path().join("boundary-sessions");
        let first = super::super::directive::persist_at_epoch_for_test(
            &artifacts,
            "first instruction",
            "run-001",
            1,
            10,
        )
        .unwrap();
        record_directive(&sessions, &artifacts, &first).unwrap();
        let second = super::super::directive::persist_at_epoch_for_test(
            &artifacts,
            "second instruction",
            "run-001",
            2,
            20,
        )
        .unwrap();
        record_directive(&sessions, &artifacts, &second).unwrap();
        let events = root.path().join("events.jsonl");
        std::fs::write(
            &events,
            "{\"event\":\"tui_command_stop\",\"status\":\"failed\",\"stop_reason\":\"structural gate failed\"}\n",
        )
        .unwrap();
        let persisted = record_latest_result(&sessions, "run-001", 1, &events).unwrap();
        let history = render_history(persisted.session(), 2, MAX_HISTORY_RENDERED_BYTES).unwrap();
        assert!(history.contains("first instruction"));
        assert!(history.contains("result_verdict: failed"));
        assert!(history.contains("stop_reason: structural gate failed"));
        assert!(!history.contains("second instruction"));
    }
}
