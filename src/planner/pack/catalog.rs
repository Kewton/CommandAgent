use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

use crate::planner::profile_descriptor::{DATA_PROFILE_ID, PYTHON_CLI_PROFILE_ID};

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum PackSource {
    #[default]
    Admitted,
    Repository,
    Local,
}

impl PackSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Admitted => "admitted",
            Self::Repository => "repository",
            Self::Local => "local",
        }
    }

    pub const fn japanese_label(self) -> &'static str {
        match self {
            Self::Admitted => "承認済み",
            Self::Repository => "リポジトリ（未承認）",
            Self::Local => "ローカル（未承認・帯域未計測）",
        }
    }
}

impl fmt::Display for PackSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdmittedPack {
    pub id: &'static str,
    pub version: &'static str,
    pub profile: &'static str,
    pub intent: &'static str,
    pub hash: &'static str,
    pub point: &'static str,
    pub relative_directory: &'static str,
}

pub const ADMITTED_PACKS: &[AdmittedPack] = &[
    AdmittedPack {
        id: "cli-assist",
        version: "1.0.0",
        profile: PYTHON_CLI_PROFILE_ID,
        intent: "create",
        hash: "sha256:b1dcee70c1a0536954c25639e2d67508d8029328e414aaff030368e7fac844fd",
        point: "cli-validation",
        relative_directory: "packs/cli-assist/1.0.0",
    },
    AdmittedPack {
        id: "cli-assist",
        version: "1.1.0",
        profile: PYTHON_CLI_PROFILE_ID,
        intent: "create",
        hash: "sha256:3d11e126d3afbcd8a53e23367d53859924c700aeaf5345fa366060d66c917c82",
        point: "cli-validation",
        relative_directory: "packs/cli-assist/1.1.0",
    },
    AdmittedPack {
        id: "data-assist",
        version: "1.0.0",
        profile: DATA_PROFILE_ID,
        intent: "create",
        hash: "sha256:58277d6a5bd999331f380ee9c68b56f2cc5b1743f615169d0fe0d131d353349e",
        point: "data-cleaning",
        relative_directory: "packs/data-assist/1.0.0",
    },
];

pub fn admitted_packs() -> &'static [AdmittedPack] {
    ADMITTED_PACKS
}

pub fn compatible(profile: &str, intent: &str) -> impl Iterator<Item = &'static AdmittedPack> {
    ADMITTED_PACKS
        .iter()
        .filter(move |pack| pack.profile == profile && pack.intent == intent)
}

pub fn is_admitted(
    source: PackSource,
    profile: &str,
    intent: &str,
    id: &str,
    version: &str,
    hash: &str,
    point: &str,
) -> bool {
    source == PackSource::Admitted
        && compatible(profile, intent).any(|pack| {
            pack.id == id && pack.version == version && pack.hash == hash && pack.point == point
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackLocator {
    repository_root: PathBuf,
}

impl PackLocator {
    pub fn new(repository_root: impl Into<PathBuf>) -> Self {
        Self {
            repository_root: repository_root.into(),
        }
    }

    pub fn repository_root(&self) -> &Path {
        &self.repository_root
    }

    pub fn locate(&self, source: PackSource, id: &str, version: &str) -> anyhow::Result<PathBuf> {
        if source != PackSource::Admitted {
            bail!("pack source `{source}` is not available from the admitted catalog")
        }
        let pack = ADMITTED_PACKS
            .iter()
            .find(|pack| pack.id == id && pack.version == version)
            .context("selected pack is absent from the admitted catalog")?;
        Ok(self.repository_root.join(pack.relative_directory))
    }

    pub fn observed_hash(
        &self,
        source: PackSource,
        id: &str,
        version: &str,
    ) -> anyhow::Result<String> {
        let directory = self.locate(source, id, version)?;
        let loaded = super::load_directory(&directory)
            .with_context(|| format!("load admitted pack {}", directory.display()))?;
        if loaded.id() != id || loaded.identity.version != version {
            bail!("resolved pack identity differs from the admitted catalog entry")
        }
        Ok(loaded.hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatible_enumeration_and_admission_use_the_exact_catalog_tuple() {
        let compatible = compatible(PYTHON_CLI_PROFILE_ID, "create").collect::<Vec<_>>();
        assert_eq!(compatible.len(), 2);
        let pack = compatible[1];
        assert!(is_admitted(
            PackSource::Admitted,
            pack.profile,
            pack.intent,
            pack.id,
            pack.version,
            pack.hash,
            pack.point,
        ));
        assert!(!is_admitted(
            PackSource::Repository,
            pack.profile,
            pack.intent,
            pack.id,
            pack.version,
            pack.hash,
            pack.point,
        ));
        assert!(!is_admitted(
            PackSource::Admitted,
            "ingest",
            pack.intent,
            pack.id,
            pack.version,
            pack.hash,
            pack.point,
        ));
    }

    #[test]
    fn pack_source_uses_closed_snake_case_values_and_japanese_labels() {
        for (source, serialized, label) in [
            (PackSource::Admitted, "\"admitted\"", "承認済み"),
            (
                PackSource::Repository,
                "\"repository\"",
                "リポジトリ（未承認）",
            ),
            (
                PackSource::Local,
                "\"local\"",
                "ローカル（未承認・帯域未計測）",
            ),
        ] {
            assert_eq!(serde_json::to_string(&source).unwrap(), serialized);
            assert_eq!(
                serde_json::from_str::<PackSource>(serialized).unwrap(),
                source
            );
            assert_eq!(source.japanese_label(), label);
        }
        assert!(serde_json::from_str::<PackSource>("\"remote\"").is_err());
    }

    #[test]
    fn locator_resolves_only_admitted_catalog_entries() {
        let locator = PackLocator::new("/repository");
        assert_eq!(
            locator
                .locate(PackSource::Admitted, "cli-assist", "1.1.0")
                .unwrap(),
            Path::new("/repository/packs/cli-assist/1.1.0")
        );
        assert!(
            locator
                .locate(PackSource::Repository, "cli-assist", "1.1.0")
                .is_err()
        );
        assert!(
            locator
                .locate(PackSource::Admitted, "unknown", "1.0.0")
                .is_err()
        );
    }

    #[test]
    fn catalog_hashes_match_the_exact_repository_bytes() {
        let locator = PackLocator::new(env!("CARGO_MANIFEST_DIR"));
        for pack in admitted_packs() {
            assert_eq!(
                locator
                    .observed_hash(PackSource::Admitted, pack.id, pack.version)
                    .unwrap(),
                pack.hash
            );
        }
    }
}
