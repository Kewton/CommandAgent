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

fn validate_artifact(artifact: &DirectiveArtifact) -> anyhow::Result<()> {
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
}
