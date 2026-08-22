use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::planner::pack::catalog::PackLocator;
use crate::planner::pack::catalog::PackSource;

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
        #[serde(default)]
        source: PackSource,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DraftManifestIdentity {
    pub source: String,
    pub path: String,
    pub hash: String,
    pub assurance_ceiling: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_profile: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_manifest: Option<DraftManifestIdentity>,
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
        super::pack_catalog::validate_selection(
            route.profile.as_str(),
            route.intent.as_str(),
            &pack,
        )?;
        Self::from_validated(request, workspace, route, band, pins, pack)
    }

    pub fn new_with_locator(
        request: String,
        workspace: &Path,
        route: &RouteCandidate,
        band: &BandValue,
        pins: ExecutionPins,
        pack: PackSelection,
        locator: &PackLocator,
    ) -> anyhow::Result<Self> {
        super::pack_catalog::validate_selection_with_locator(
            route.profile.as_str(),
            route.intent.as_str(),
            &pack,
            locator,
        )?;
        Self::from_validated(request, workspace, route, band, pins, pack)
    }

    fn from_validated(
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
            draft_manifest: None,
        })
    }

    pub fn new_draft(
        request: String,
        workspace: &Path,
        route: &RouteCandidate,
        pins: ExecutionPins,
        pack: PackSelection,
        profile: &crate::planner::extension_profiles::ExtensionProfile,
    ) -> anyhow::Result<Self> {
        if !matches!(pack, PackSelection::None) {
            bail!("draft profiles fix the pack selection to none");
        }
        Self::from_validated_draft(request, workspace, route, pins, pack, profile)
    }

    pub fn new_draft_with_locator(
        request: String,
        workspace: &Path,
        route: &RouteCandidate,
        pins: ExecutionPins,
        pack: PackSelection,
        profile: &crate::planner::extension_profiles::ExtensionProfile,
        locator: &PackLocator,
    ) -> anyhow::Result<Self> {
        super::pack_catalog::validate_selection_with_locator(
            route.profile.as_str(),
            route.intent.as_str(),
            &pack,
            locator,
        )?;
        Self::from_validated_draft(request, workspace, route, pins, pack, profile)
    }

    fn from_validated_draft(
        request: String,
        workspace: &Path,
        route: &RouteCandidate,
        pins: ExecutionPins,
        pack: PackSelection,
        profile: &crate::planner::extension_profiles::ExtensionProfile,
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
            contract_checks: profile.contract_checks.clone(),
            band_full: 0,
            band_denominator: 0,
            band_rate: "未計測".to_string(),
            band_arm: "draft / 未承認".to_string(),
            band_measurement: "未計測".to_string(),
            band_source: "未計測".to_string(),
            full_meaning:
                "manifest の全必須チェックに合格しても、未承認プロファイルの保証上限は static"
                    .to_string(),
            pins,
            pack,
            draft_manifest: Some(DraftManifestIdentity {
                source: profile.source.as_str().to_string(),
                path: profile.manifest_path.to_string(),
                hash: profile.manifest_hash.to_string(),
                assurance_ceiling: profile.assurance_ceiling().to_string(),
                base_profile: profile.base_profile.map(str::to_string),
            }),
        })
    }

    pub fn card_hash(&self) -> anyhow::Result<String> {
        // Preserve schema-v1 admitted-pin hashes. The source is still explicit
        // in new records and is fixed indirectly by exact catalog admission;
        // repository and local sources remain part of their new card hashes.
        if let Some(hash) = self.legacy_card_hash()? {
            return Ok(hash);
        }
        let bytes = serde_json::to_vec(self)?;
        Ok(sha256(&bytes))
    }

    fn legacy_card_hash(&self) -> anyhow::Result<Option<String>> {
        let PackSelection::Pinned {
            id,
            version,
            hash,
            point,
            source: PackSource::Admitted,
        } = &self.pack
        else {
            return Ok(None);
        };
        let legacy = LegacyConfirmationIdentity {
            request: &self.request,
            workspace: &self.workspace,
            profile: &self.profile,
            intent: &self.intent,
            task_family: &self.task_family,
            route_bases: &self.route_bases,
            contract_ref: &self.contract_ref,
            contract_checks: &self.contract_checks,
            band_full: self.band_full,
            band_denominator: self.band_denominator,
            band_rate: &self.band_rate,
            band_arm: &self.band_arm,
            band_measurement: &self.band_measurement,
            band_source: &self.band_source,
            full_meaning: &self.full_meaning,
            pins: &self.pins,
            pack: LegacyPackSelection::Pinned {
                id,
                version,
                hash,
                point,
            },
        };
        Ok(Some(sha256(&serde_json::to_vec(&legacy)?)))
    }
}

#[derive(Serialize)]
#[serde(tag = "selection", rename_all = "snake_case")]
enum LegacyPackSelection<'a> {
    Pinned {
        id: &'a str,
        version: &'a str,
        hash: &'a str,
        point: &'a str,
    },
}

#[derive(Serialize)]
struct LegacyConfirmationIdentity<'a> {
    request: &'a str,
    workspace: &'a str,
    profile: &'a str,
    intent: &'a str,
    task_family: &'a str,
    route_bases: &'a [String],
    contract_ref: &'a str,
    contract_checks: &'a [String],
    band_full: u16,
    band_denominator: u16,
    band_rate: &'a str,
    band_arm: &'a str,
    band_measurement: &'a str,
    band_source: &'a str,
    full_meaning: &'a str,
    pins: &'a ExecutionPins,
    pack: LegacyPackSelection<'a>,
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

pub fn load_latest_confirmation(root: &Path) -> anyhow::Result<Option<ConfirmedDispatch>> {
    if !root.is_dir() {
        return Ok(None);
    }
    let mut latest: Option<(u64, PathBuf, ConfirmationRecord)> = None;
    for entry in std::fs::read_dir(root)
        .with_context(|| format!("read confirmation directory {}", root.display()))?
    {
        let path = entry?.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let record = read_record(&path)?;
        if latest
            .as_ref()
            .is_none_or(|(epoch, _, _)| record.confirmed_at_epoch > *epoch)
        {
            latest = Some((record.confirmed_at_epoch, path, record));
        }
    }
    let Some((_, record_path, record)) = latest else {
        return Ok(None);
    };
    let confirmed = ConfirmedDispatch {
        record_path,
        card_hash: record.card_hash,
        identity: record.identity,
    };
    confirmed.validate()?;
    Ok(Some(confirmed))
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

    #[test]
    fn legacy_pinned_record_without_source_loads_as_admitted() {
        let dir = tempfile::tempdir().unwrap();
        let identity = ConfirmationIdentity {
            request: "create a CLI".to_string(),
            workspace: dir.path().display().to_string(),
            profile: "python-cli".to_string(),
            intent: "create".to_string(),
            task_family: "stats".to_string(),
            route_bases: vec!["fixture=stats".to_string()],
            contract_ref: "docs/cli-profile-contract.md".to_string(),
            contract_checks: vec!["C1".to_string()],
            band_full: 0,
            band_denominator: 3,
            band_rate: "0%".to_string(),
            band_arm: "fixture".to_string(),
            band_measurement: "2026-08-19".to_string(),
            band_source: "fixture.md".to_string(),
            full_meaning: "all checks pass".to_string(),
            pins: ExecutionPins {
                planner_provider: "ollama".to_string(),
                planner_model: "planner".to_string(),
                executor_provider: "ollama".to_string(),
                executor_model: "executor".to_string(),
                preset: "profile".to_string(),
            },
            pack: PackSelection::Pinned {
                id: "cli-assist".to_string(),
                version: "1.1.0".to_string(),
                hash: crate::planner::pack::catalog::ADMITTED_PACKS[1]
                    .hash
                    .to_string(),
                point: "cli-validation".to_string(),
                source: PackSource::Admitted,
            },
            draft_manifest: None,
        };
        assert!(crate::planner::pack::catalog::is_admitted(
            PackSource::Admitted,
            &identity.profile,
            &identity.intent,
            "cli-assist",
            "1.1.0",
            crate::planner::pack::catalog::ADMITTED_PACKS[1].hash,
            "cli-validation",
        ));
        let legacy_hash = identity.legacy_card_hash().unwrap().unwrap();
        assert_eq!(identity.card_hash().unwrap(), legacy_hash);
        let legacy_identity = match &identity.pack {
            PackSelection::Pinned {
                id,
                version,
                hash,
                point,
                ..
            } => LegacyConfirmationIdentity {
                request: &identity.request,
                workspace: &identity.workspace,
                profile: &identity.profile,
                intent: &identity.intent,
                task_family: &identity.task_family,
                route_bases: &identity.route_bases,
                contract_ref: &identity.contract_ref,
                contract_checks: &identity.contract_checks,
                band_full: identity.band_full,
                band_denominator: identity.band_denominator,
                band_rate: &identity.band_rate,
                band_arm: &identity.band_arm,
                band_measurement: &identity.band_measurement,
                band_source: &identity.band_source,
                full_meaning: &identity.full_meaning,
                pins: &identity.pins,
                pack: LegacyPackSelection::Pinned {
                    id,
                    version,
                    hash,
                    point,
                },
            },
            PackSelection::None => unreachable!(),
        };
        #[derive(Serialize)]
        struct LegacyRecord<'a> {
            schema_version: u8,
            card_hash: &'a str,
            confirmed_at_epoch: u64,
            identity: LegacyConfirmationIdentity<'a>,
        }
        let record = LegacyRecord {
            schema_version: CONFIRMATION_SCHEMA_VERSION,
            card_hash: &legacy_hash,
            confirmed_at_epoch: 1,
            identity: legacy_identity,
        };
        let records = dir.path().join("records");
        std::fs::create_dir_all(&records).unwrap();
        let record_bytes = serde_json::to_vec_pretty(&record).unwrap();
        assert!(!String::from_utf8_lossy(&record_bytes).contains("\"source\":"));
        std::fs::write(records.join("legacy.json"), record_bytes).unwrap();

        let loaded = load_latest_confirmation(&records).unwrap().unwrap();
        assert_eq!(loaded.identity(), &identity);
        assert_eq!(loaded.card_hash(), legacy_hash);
        loaded.validate().unwrap();
    }
}
