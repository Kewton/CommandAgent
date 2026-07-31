use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::band_catalog::BandValue;
use super::route::RouteCandidate;

const CONFIRMATION_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionPins {
    pub planner_provider: String,
    pub planner_model: String,
    pub executor_provider: String,
    pub executor_model: String,
    pub preset: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "selection", rename_all = "snake_case", deny_unknown_fields)]
pub enum PackSelection {
    None,
    Pinned {
        id: String,
        version: String,
        hash: String,
        point: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfirmationIdentity {
    pub request: String,
    pub workspace: String,
    pub profile: String,
    pub intent: String,
    pub task_family: String,
    pub route_bases: Vec<String>,
    pub contract_ref: String,
    pub contract_checks: Vec<String>,
    pub band_full: u16,
    pub band_denominator: u16,
    pub band_rate: String,
    pub band_arm: String,
    pub band_measurement: String,
    pub band_source: String,
    pub full_meaning: String,
    pub pins: ExecutionPins,
    pub pack: PackSelection,
}

impl ConfirmationIdentity {
    pub fn new(
        request: String,
        workspace: &Path,
        route: &RouteCandidate,
        band: &BandValue,
        pins: ExecutionPins,
        pack: PackSelection,
    ) -> anyhow::Result<Self> {
        let workspace = workspace
            .canonicalize()
            .with_context(|| format!("canonicalize workspace {}", workspace.display()))?;
        Ok(Self {
            request,
            workspace: workspace.to_string_lossy().into_owned(),
            profile: route.profile.to_string(),
            intent: route.intent.as_str().to_string(),
            task_family: route.family.to_string(),
            route_bases: route
                .bases
                .iter()
                .map(|basis| format!("{}={}", basis.rule, basis.observation))
                .collect(),
            contract_ref: route.contract_ref.to_string(),
            contract_checks: contract_checks(route),
            band_full: band.full,
            band_denominator: band.denominator,
            band_rate: band.display_rate.to_string(),
            band_arm: band.arm.to_string(),
            band_measurement: band.measurement.to_string(),
            band_source: band.source.to_string(),
            full_meaning: band.full_meaning.to_string(),
            pins,
            pack,
        })
    }

    pub fn card_hash(&self) -> anyhow::Result<String> {
        let bytes = serde_json::to_vec(self)?;
        Ok(sha256(&bytes))
    }
}

fn contract_checks(route: &RouteCandidate) -> Vec<String> {
    if route.intent == crate::planner::adjudication::contract::IntentId::Create {
        return match route.profile.as_str() {
            "nextjs" => vec![
                "build".to_string(),
                "browser_route".to_string(),
                "interaction_state".to_string(),
                "T1_testimony".to_string(),
            ],
            "data" => vec!["E1".into(), "E2".into(), "E3".into(), "E4".into()],
            "python-cli" => vec!["C1".into(), "C2".into(), "C3".into(), "C4".into()],
            "ingest" => vec![
                "N1".into(),
                "N2".into(),
                "N3".into(),
                "N4".into(),
                "N5".into(),
            ],
            _ => vec!["create_acceptance".to_string()],
        };
    }
    crate::planner::adjudication::contract::intent_contract(route.intent.as_str())
        .map(|contract| {
            contract
                .requirements
                .iter()
                .map(|requirement| requirement.id.to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfirmationRecord {
    schema_version: u8,
    card_hash: String,
    confirmed_at_epoch: u64,
    identity: ConfirmationIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmedDispatch {
    record_path: PathBuf,
    card_hash: String,
    identity: ConfirmationIdentity,
}

impl ConfirmedDispatch {
    pub fn record_path(&self) -> &Path {
        &self.record_path
    }

    pub fn card_hash(&self) -> &str {
        &self.card_hash
    }

    pub fn identity(&self) -> &ConfirmationIdentity {
        &self.identity
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        let record = read_record(&self.record_path)?;
        if record.schema_version != CONFIRMATION_SCHEMA_VERSION
            || record.card_hash != self.card_hash
            || record.identity != self.identity
            || record.identity.card_hash()? != self.card_hash
        {
            bail!("persisted confirmation record does not match the frozen Gate 1 card");
        }
        Ok(())
    }
}

pub fn persist_confirmation(
    root: &Path,
    identity: &ConfirmationIdentity,
    expected_hash: &str,
) -> anyhow::Result<ConfirmedDispatch> {
    let actual_hash = identity.card_hash()?;
    if actual_hash != expected_hash {
        bail!("Gate 1 card changed before confirmation");
    }
    std::fs::create_dir_all(root)
        .with_context(|| format!("create confirmation directory {}", root.display()))?;
    let record_path = root.join(format!(
        "{}.json",
        expected_hash.trim_start_matches("sha256:")
    ));
    let record = ConfirmationRecord {
        schema_version: CONFIRMATION_SCHEMA_VERSION,
        card_hash: expected_hash.to_string(),
        confirmed_at_epoch: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system time before UNIX epoch")?
            .as_secs(),
        identity: identity.clone(),
    };
    if record_path.exists() {
        let existing = read_record(&record_path)?;
        if existing.card_hash != record.card_hash || existing.identity != record.identity {
            bail!("confirmation hash collision or stale confirmation record");
        }
    } else {
        let bytes = serde_json::to_vec_pretty(&record)?;
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&record_path)
            .with_context(|| format!("create confirmation record {}", record_path.display()))?;
        file.write_all(&bytes)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
    }
    let confirmed = ConfirmedDispatch {
        record_path,
        card_hash: expected_hash.to_string(),
        identity: identity.clone(),
    };
    confirmed.validate()?;
    Ok(confirmed)
}

fn read_record(path: &Path) -> anyhow::Result<ConfirmationRecord> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("read confirmation record {}", path.display()))?;
    serde_json::from_slice(&bytes)
        .with_context(|| format!("parse confirmation record {}", path.display()))
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
    fn strict_record_rejects_unknown_fields() {
        let fixture = br#"{
          "schema_version": 1,
          "card_hash": "sha256:x",
          "confirmed_at_epoch": 1,
          "identity": {},
          "unexpected": true
        }"#;
        assert!(serde_json::from_slice::<ConfirmationRecord>(fixture).is_err());
    }
}
