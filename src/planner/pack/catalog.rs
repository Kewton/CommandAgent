use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

use crate::planner::profile_descriptor::{DATA_PROFILE_ID, PYTHON_CLI_PROFILE_ID};

/// Exact-byte pin file written next to a pack's members.
pub const PACK_PIN_FILE: &str = "pack.sha256";
/// Retirement marker. Its presence removes a pack from new selection without
/// deleting bytes, the pin, or journal history (contract v0.1 section 7.2).
pub const RETIRED_MARKER_FILE: &str = "RETIRED";

/// Supply lifecycle state of one `<id>/<version>` pack directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackStatus {
    /// Present but not pinned; not selectable.
    Staged,
    /// Pinned by `pack.sha256`; selectable when conformance and identity agree.
    Pinned,
    /// Carries the `RETIRED` marker; listable and bundle-readable, never selectable.
    Retired,
}

impl PackStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Staged => "staged",
            Self::Pinned => "pinned",
            Self::Retired => "retired",
        }
    }

    pub const fn is_selectable(self) -> bool {
        matches!(self, Self::Pinned)
    }
}

impl fmt::Display for PackStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Retirement is a marker file, so a retired pack keeps its bytes and pin.
pub fn is_retired(directory: &Path) -> bool {
    is_regular_file(&directory.join(RETIRED_MARKER_FILE))
}

/// Classify one pack directory. Retirement outranks pinning so a retired pin
/// can never be reported as selectable.
pub fn status(directory: &Path) -> PackStatus {
    if is_retired(directory) {
        PackStatus::Retired
    } else if is_regular_file(&directory.join(PACK_PIN_FILE)) {
        PackStatus::Pinned
    } else {
        PackStatus::Staged
    }
}

fn is_regular_file(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_file())
}

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
    extension_root: Option<PathBuf>,
}

impl PackLocator {
    pub fn new(repository_root: impl Into<PathBuf>) -> Self {
        Self {
            repository_root: repository_root.into(),
            extension_root: None,
        }
    }

    pub fn with_extension_root(
        repository_root: impl Into<PathBuf>,
        extension_root: Option<PathBuf>,
    ) -> Self {
        Self {
            repository_root: repository_root.into(),
            extension_root,
        }
    }

    pub fn repository_root(&self) -> &Path {
        &self.repository_root
    }

    pub fn extension_root(&self) -> Option<&Path> {
        self.extension_root.as_deref()
    }

    pub fn locate(&self, source: PackSource, id: &str, version: &str) -> anyhow::Result<PathBuf> {
        validate_identity(id, version)?;
        let directory = match source {
            PackSource::Admitted => {
                let pack = ADMITTED_PACKS
                    .iter()
                    .find(|pack| pack.id == id && pack.version == version)
                    .context("selected pack is absent from the admitted catalog")?;
                self.repository_root.join(pack.relative_directory)
            }
            PackSource::Repository => self.repository_root.join("packs").join(id).join(version),
            PackSource::Local => self
                .extension_root
                .as_deref()
                .context("local pack selection requires an extension root")?
                .join("packs")
                .join(id)
                .join(version),
        };
        if is_retired(&directory) {
            bail!("pack `{id}@{version}` is retired and cannot be selected")
        }
        Ok(directory)
    }

    /// Resolve the extension-root-first catalog entry and require an exact,
    /// conformant pin. A local directory shadows the repository even when the
    /// local bytes are invalid, so an invalid local pack can never fall back to
    /// a same-named repository pack.
    pub fn locate_pinned(
        &self,
        id: &str,
        version: &str,
        expected_hash: Option<&str>,
    ) -> anyhow::Result<LocatedPack> {
        validate_identity(id, version)?;
        let source = if self.extension_root.as_deref().is_some_and(|root| {
            std::fs::symlink_metadata(root.join("packs").join(id).join(version)).is_ok()
        }) {
            PackSource::Local
        } else {
            PackSource::Repository
        };
        self.locate_pinned_from(source, id, version, expected_hash)
    }

    /// Require a pin from one already-confirmed supply source. This is used at
    /// dispatch time so a newly introduced local shadow cannot change a frozen
    /// repository identity.
    pub fn locate_pinned_from(
        &self,
        source: PackSource,
        id: &str,
        version: &str,
        expected_hash: Option<&str>,
    ) -> anyhow::Result<LocatedPack> {
        let directory = self.locate(source, id, version)?;
        validate_directory_chain(
            match source {
                PackSource::Local => self
                    .extension_root
                    .as_deref()
                    .context("local pack selection requires an extension root")?,
                PackSource::Admitted | PackSource::Repository => &self.repository_root,
            },
            id,
            version,
        )
        .with_context(|| format!("locate {source} pack `{id}@{version}`"))?;
        let loaded = super::load_directory(&directory)
            .with_context(|| format!("load {source} pack {}", directory.display()))?;
        if loaded.id() != id || loaded.identity.version != version {
            bail!(
                "resolved pack identity is {}@{}, not {id}@{version}",
                loaded.id(),
                loaded.identity.version
            )
        }
        let hash = loaded.hash.clone();
        let pin_path = directory.join(PACK_PIN_FILE);
        let pin_metadata = std::fs::symlink_metadata(&pin_path)
            .with_context(|| format!("inspect pack pin {}", pin_path.display()))?;
        if !pin_metadata.file_type().is_file() {
            bail!(
                "pack pin `{}` is not a non-symlink file",
                pin_path.display()
            )
        }
        let pin = std::fs::read_to_string(&pin_path)
            .with_context(|| format!("read pack pin {}", pin_path.display()))?;
        if pin.trim() != hash {
            bail!(
                "pack `{id}@{version}` pin mismatch: expected `{}`, observed `{hash}`",
                pin.trim()
            )
        }
        if let Some(expected_hash) = expected_hash
            && expected_hash.trim() != hash
        {
            bail!(
                "pack `{id}@{version}` hash mismatch: expected `{}`, observed `{hash}`",
                expected_hash.trim()
            )
        }
        super::conform(&loaded).context("selected pack conformance failed")?;
        let point = pack_point(&loaded);
        let admitted = source != PackSource::Local
            && point.as_deref().is_some_and(|point| {
                is_admitted(
                    PackSource::Admitted,
                    loaded.identity.profile.as_str(),
                    loaded.identity.intent.as_str(),
                    id,
                    version,
                    &hash,
                    point,
                )
            });
        if source == PackSource::Admitted && !admitted {
            bail!("pack `{id}@{version}` no longer matches the admitted catalog tuple")
        }
        let resolved_source = if admitted {
            PackSource::Admitted
        } else {
            source
        };
        Ok(LocatedPack {
            id: id.to_string(),
            version: version.to_string(),
            hash,
            profile: loaded.identity.profile.as_str().to_string(),
            intent: loaded.identity.intent.as_str().to_string(),
            point,
            directory,
            source: resolved_source,
            shadowed_repository: source == PackSource::Local
                && self
                    .repository_root
                    .join("packs")
                    .join(id)
                    .join(version)
                    .is_dir(),
        })
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

/// A selected, exact-byte pinned pack returned by [`PackLocator`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocatedPack {
    pub id: String,
    pub version: String,
    pub hash: String,
    pub profile: String,
    pub intent: String,
    pub point: Option<String>,
    pub directory: PathBuf,
    pub source: PackSource,
    pub shadowed_repository: bool,
}

fn pack_point(pack: &super::LoadedPack) -> Option<String> {
    pack.assist.as_ref().and_then(|assist| {
        assist
            .inject
            .first()
            .map(|injection| injection.point.as_str().to_string())
            .or_else(|| {
                assist
                    .vocabulary
                    .first()
                    .map(|vocabulary| vocabulary.point.as_str().to_string())
            })
    })
}

fn validate_identity(id: &str, version: &str) -> anyhow::Result<()> {
    if id.is_empty()
        || id.len() > 64
        || !id.as_bytes()[0].is_ascii_lowercase()
        || id.split('-').any(|segment| {
            segment.is_empty()
                || !segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
    {
        bail!("invalid pack id `{id}`")
    }
    let parts = version.split('.').collect::<Vec<_>>();
    if parts.len() != 3
        || parts.iter().any(|part| {
            part.is_empty()
                || !part.bytes().all(|byte| byte.is_ascii_digit())
                || (part.len() > 1 && part.starts_with('0'))
                || part.parse::<u64>().is_err()
        })
    {
        bail!("pack version `{version}` must be SemVer core MAJOR.MINOR.PATCH")
    }
    Ok(())
}

fn validate_directory_chain(root: &Path, id: &str, version: &str) -> anyhow::Result<()> {
    let mut current = root.to_path_buf();
    for component in ["packs", id, version] {
        current.push(component);
        let metadata = std::fs::symlink_metadata(&current)
            .with_context(|| format!("inspect pack directory {}", current.display()))?;
        if !metadata.file_type().is_dir() {
            bail!(
                "pack path component `{}` is not a non-symlink directory",
                current.display()
            )
        }
    }
    Ok(())
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
    fn locator_resolves_each_explicit_supply_source_without_crossing_roots() {
        let locator = PackLocator::new("/repository");
        assert_eq!(
            locator
                .locate(PackSource::Admitted, "cli-assist", "1.1.0")
                .unwrap(),
            Path::new("/repository/packs/cli-assist/1.1.0")
        );
        assert_eq!(
            locator
                .locate(PackSource::Repository, "cli-assist", "1.1.0")
                .unwrap(),
            Path::new("/repository/packs/cli-assist/1.1.0")
        );
        assert!(
            locator
                .locate(PackSource::Local, "cli-assist", "1.1.0")
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
