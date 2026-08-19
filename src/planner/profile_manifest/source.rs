//! `ManifestSource`: where a profile manifest came from, and how an externally
//! supplied manifest is admitted.
//!
//! Embedded manifests keep their declared admission status. A manifest read
//! from an extension root is always a draft, regardless of what it declares,
//! and is identified by the exact bytes that were read.

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::planner::adjudication::contract::IntentId;
use crate::tui::boundary_shell::family_catalog::TaskFamilyId;

use super::{ManifestError, ManifestStatus, ManifestV1};

/// Extension-root layout fixed by Issue #103.
pub const EXTENSION_PROFILES_DIRECTORY: &str = "profiles";
pub const EXTENSION_MANIFEST_FILE: &str = "manifest.toml";
pub const EXTENSION_OVERLAY_FILE: &str = "overlay.toml";

const HASH_DOMAIN: &[u8] = b"commandagent-profile-manifest-v1\0";
const MAX_MANIFEST_BYTES: u64 = 256 * 1024;

/// Bounded because every admitted extension profile is registered for the
/// lifetime of the process.
pub const MAX_EXTENSION_PROFILES: usize = 64;

/// The measured-fixture vocabulary tripwire applied to repository manifests by
/// `tests/generality_guardrails.rs`. External manifests run the same scan at
/// load time so an extension root cannot smuggle scenario leakage past it.
pub const FIXTURE_VOCABULARY: &[&str] = &["sales", "売上", "東京", "大阪", "名古屋"];
pub const SCANNED_SECTIONS: &[&str] = &["plan", "step_templates", "checks"];

/// Closed supply set. `Embedded` is compiled into the binary; `Repository` is
/// supplied by the checked-out repository; `Local` is supplied by an extension
/// root. Only `Repository` and `Local` are external.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ManifestSource {
    Embedded,
    Repository,
    Local,
}

impl ManifestSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Embedded => "embedded",
            Self::Repository => "repository",
            Self::Local => "local",
        }
    }

    pub const fn japanese_label(self) -> &'static str {
        match self {
            Self::Embedded => "埋め込み",
            Self::Repository => "リポジトリ（未承認）",
            Self::Local => "ローカル（未承認・帯域未計測）",
        }
    }

    pub const fn is_external(self) -> bool {
        matches!(self, Self::Repository | Self::Local)
    }
}

impl fmt::Display for ManifestSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestOrigin {
    Embedded,
    Extension { path: PathBuf, hash: String },
}

impl ManifestOrigin {
    pub fn source(&self) -> ManifestSource {
        match self {
            Self::Embedded => ManifestSource::Embedded,
            Self::Extension { .. } => ManifestSource::Local,
        }
    }

    pub fn hash(&self) -> Option<&str> {
        match self {
            Self::Embedded => None,
            Self::Extension { hash, .. } => Some(hash),
        }
    }

    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Embedded => None,
            Self::Extension { path, .. } => Some(path),
        }
    }
}

/// One decoded profile manifest together with the supply it came from.
#[derive(Debug)]
pub struct LoadedManifest {
    pub manifest: ManifestV1,
    pub origin: ManifestOrigin,
    pub task_family: TaskFamilyId,
    pub intent: IntentId,
    pub warnings: Vec<String>,
}

impl LoadedManifest {
    pub fn id(&self) -> &str {
        &self.manifest.metadata.id
    }

    pub fn display_name(&self) -> &str {
        &self.manifest.metadata.display_name
    }

    pub fn status(&self) -> ManifestStatus {
        self.manifest.metadata.status
    }

    pub fn source(&self) -> ManifestSource {
        self.origin.source()
    }

    pub fn hash(&self) -> Option<&str> {
        self.origin.hash()
    }

    /// A draft profile's contract reference is the manifest that declares it.
    pub fn contract_ref(&self) -> String {
        self.origin
            .path()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| format!("embedded:{}", self.id()))
    }
}

#[derive(Debug)]
pub enum ExtensionManifestError {
    Root {
        path: PathBuf,
        reason: String,
    },
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    TooLarge {
        path: PathBuf,
    },
    NotUtf8 {
        path: PathBuf,
    },
    TooMany {
        limit: usize,
    },
    Manifest {
        path: PathBuf,
        source: Box<ManifestError>,
    },
    Invalid {
        path: PathBuf,
        reason: String,
    },
}

impl fmt::Display for ExtensionManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Root { path, reason } => {
                write!(
                    f,
                    "extension root `{}` is unusable: {reason}",
                    path.display()
                )
            }
            Self::Io { path, source } => {
                write!(f, "failed to read `{}`: {source}", path.display())
            }
            Self::TooLarge { path } => write!(
                f,
                "external manifest `{}` exceeds {MAX_MANIFEST_BYTES} bytes",
                path.display()
            ),
            Self::NotUtf8 { path } => {
                write!(
                    f,
                    "external manifest `{}` is not valid UTF-8",
                    path.display()
                )
            }
            Self::TooMany { limit } => {
                write!(f, "an extension root may declare at most {limit} profiles")
            }
            Self::Manifest { path, source } => {
                write!(
                    f,
                    "external manifest `{}` is invalid: {source}",
                    path.display()
                )
            }
            Self::Invalid { path, reason } => {
                write!(
                    f,
                    "external manifest `{}` is rejected: {reason}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for ExtensionManifestError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Manifest { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}

/// Exact-byte identity, using the same framing discipline as pack hashes.
pub fn exact_byte_hash(name: &str, bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(HASH_DOMAIN);
    digest.update((name.len() as u64).to_be_bytes());
    digest.update(name.as_bytes());
    digest.update((bytes.len() as u64).to_be_bytes());
    digest.update(bytes);
    format!("sha256:{:x}", digest.finalize())
}

/// Read every `profiles/<id>/manifest.toml` under `extension_root`.
///
/// Every returned manifest is a draft: a manifest that declares
/// `status = "admitted"` is accepted as a draft with a recorded warning rather
/// than being rejected, and never gains admitted assurance.
pub fn load_extension_manifests(
    extension_root: &Path,
) -> Result<Vec<LoadedManifest>, ExtensionManifestError> {
    let mut loaded = Vec::new();
    let mut ids = BTreeSet::new();
    for directory in profile_directories(extension_root)? {
        let path = directory.join(EXTENSION_MANIFEST_FILE);
        let Some(bytes) = read_optional(&path)? else {
            continue;
        };
        if loaded.len() == MAX_EXTENSION_PROFILES {
            return Err(ExtensionManifestError::TooMany {
                limit: MAX_EXTENSION_PROFILES,
            });
        }
        let entry = decode(&directory, &path, &bytes)?;
        if !ids.insert(entry.id().to_string()) {
            return Err(ExtensionManifestError::Invalid {
                path,
                reason: format!(
                    "duplicate profile id `{}` in this extension root",
                    entry.id()
                ),
            });
        }
        loaded.push(entry);
    }
    Ok(loaded)
}

pub(super) fn profile_directories(
    extension_root: &Path,
) -> Result<Vec<PathBuf>, ExtensionManifestError> {
    let extension_metadata =
        std::fs::symlink_metadata(extension_root).map_err(|source| ExtensionManifestError::Io {
            path: extension_root.to_path_buf(),
            source,
        })?;
    if !extension_metadata.file_type().is_dir() {
        return Err(ExtensionManifestError::Root {
            path: extension_root.to_path_buf(),
            reason: "must be an existing, non-symlink directory".to_string(),
        });
    }
    let root = extension_root.join(EXTENSION_PROFILES_DIRECTORY);
    let root_metadata = match std::fs::symlink_metadata(&root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(ExtensionManifestError::Io { path: root, source }),
    };
    if !root_metadata.file_type().is_dir() {
        return Err(ExtensionManifestError::Root {
            path: root,
            reason: "profiles must be a non-symlink directory".to_string(),
        });
    }
    let mut entries = std::fs::read_dir(&root)
        .map_err(|source| ExtensionManifestError::Io {
            path: root.clone(),
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| ExtensionManifestError::Io { path: root, source })?;
    entries.retain(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()));
    entries.sort_by_key(std::fs::DirEntry::file_name);
    Ok(entries.into_iter().map(|entry| entry.path()).collect())
}

pub(super) fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, ExtensionManifestError> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(ExtensionManifestError::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    if !metadata.file_type().is_file() {
        return Err(ExtensionManifestError::Invalid {
            path: path.to_path_buf(),
            reason: "must be a regular file".to_string(),
        });
    }
    if metadata.len() > MAX_MANIFEST_BYTES {
        return Err(ExtensionManifestError::TooLarge {
            path: path.to_path_buf(),
        });
    }
    std::fs::read(path)
        .map(Some)
        .map_err(|source| ExtensionManifestError::Io {
            path: path.to_path_buf(),
            source,
        })
}

fn decode(
    directory: &Path,
    path: &Path,
    bytes: &[u8],
) -> Result<LoadedManifest, ExtensionManifestError> {
    let text = std::str::from_utf8(bytes).map_err(|_| ExtensionManifestError::NotUtf8 {
        path: path.to_path_buf(),
    })?;
    let mut manifest =
        ManifestV1::from_toml(text).map_err(|source| ExtensionManifestError::Manifest {
            path: path.to_path_buf(),
            source: Box::new(source),
        })?;
    let invalid = |reason: String| ExtensionManifestError::Invalid {
        path: path.to_path_buf(),
        reason,
    };
    let expected = directory
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if manifest.metadata.id != expected {
        return Err(invalid(format!(
            "metadata.id `{}` must match the directory name `{expected}`",
            manifest.metadata.id
        )));
    }
    reject_registered_identity(&manifest.metadata.id).map_err(&invalid)?;
    let task_family = required_task_family(&manifest).map_err(&invalid)?;
    let intent = required_intent(&manifest).map_err(&invalid)?;
    reject_fixture_vocabulary(text).map_err(&invalid)?;

    let mut warnings = Vec::new();
    if manifest.metadata.status == ManifestStatus::Admitted {
        warnings.push(format!(
            "external manifest `{}` declares status `admitted`; an externally supplied profile is always a draft",
            path.display()
        ));
        manifest.metadata.status = ManifestStatus::Draft;
    }
    Ok(LoadedManifest {
        manifest,
        origin: ManifestOrigin::Extension {
            path: path.to_path_buf(),
            hash: exact_byte_hash(EXTENSION_MANIFEST_FILE, bytes),
        },
        task_family,
        intent,
        warnings,
    })
}

/// Externally supplied ids may never shadow a compiled-in profile identity.
pub(super) fn reject_registered_identity(id: &str) -> Result<(), String> {
    let normalized = id.trim().to_ascii_lowercase();
    if normalized != id {
        return Err(format!(
            "profile id `{id}` must already be trimmed and lowercase"
        ));
    }
    if normalized.is_empty()
        || normalized.len() > 64
        || !normalized
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        || normalized.starts_with('-')
        || normalized.ends_with('-')
    {
        return Err(format!(
            "profile id `{id}` must contain 1..=64 lowercase ASCII letters, digits, or interior hyphens"
        ));
    }
    for descriptor in crate::planner::profile_descriptor::PROFILE_DESCRIPTORS {
        if descriptor.canonical == normalized || descriptor.aliases.contains(&normalized.as_str()) {
            return Err(format!(
                "profile id `{id}` collides with the registered profile `{}`",
                descriptor.canonical
            ));
        }
    }
    if crate::planner::profile::ProfileId::parse(&normalized)
        != crate::planner::profile::ProfileId::Other(normalized.clone())
    {
        return Err(format!("profile id `{id}` is a reserved runtime identity"));
    }
    Ok(())
}

fn required_intent(manifest: &ManifestV1) -> Result<IntentId, String> {
    match manifest.plan.intent.trim() {
        "create" => Ok(IntentId::Create),
        "fix" => Ok(IntentId::Fix),
        "investigate" => Ok(IntentId::Investigate),
        value => Err(format!(
            "plan.intent `{value}` must be one of the registered intents: create, fix, investigate"
        )),
    }
}

fn required_task_family(manifest: &ManifestV1) -> Result<TaskFamilyId, String> {
    let declared = manifest
        .metadata
        .task_family
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            "metadata.task_family is required for an externally supplied profile".to_string()
        })?;
    TaskFamilyId::ALL
        .into_iter()
        .find(|family| family.as_str().eq_ignore_ascii_case(declared))
        .ok_or_else(|| {
            format!(
                "metadata.task_family `{declared}` must be one of the registered families: {}",
                TaskFamilyId::ALL
                    .iter()
                    .map(|family| family.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
}

/// The repository tripwire from `tests/generality_guardrails.rs`, executed at
/// load time so an external manifest cannot bypass it.
pub(super) fn reject_fixture_vocabulary(text: &str) -> Result<(), String> {
    let document = text
        .parse::<toml::Value>()
        .map_err(|error| format!("manifest TOML is invalid: {error}"))?;
    for section in SCANNED_SECTIONS {
        let Some(value) = document.get(section) else {
            continue;
        };
        for token in FIXTURE_VOCABULARY {
            if toml_value_contains(value, token) {
                return Err(format!(
                    "measured-fixture vocabulary {token:?} is not allowed in the `{section}` section"
                ));
            }
        }
    }
    Ok(())
}

fn toml_value_contains(value: &toml::Value, needle: &str) -> bool {
    match value {
        toml::Value::String(text) => text.contains(needle),
        toml::Value::Array(items) => items.iter().any(|item| toml_value_contains(item, needle)),
        toml::Value::Table(table) => table
            .iter()
            .any(|(key, item)| key.contains(needle) || toml_value_contains(item, needle)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_byte_hash_is_lowercase_sha256_over_the_exact_bytes() {
        let hash = exact_byte_hash(EXTENSION_MANIFEST_FILE, b"a");
        assert!(hash.starts_with("sha256:"), "{hash}");
        assert_eq!(hash.len(), "sha256:".len() + 64);
        assert!(hash.chars().skip(7).all(|ch| ch.is_ascii_hexdigit()));
        assert_ne!(hash, exact_byte_hash(EXTENSION_MANIFEST_FILE, b"b"));
        assert_ne!(hash, exact_byte_hash(EXTENSION_OVERLAY_FILE, b"a"));
    }

    #[test]
    fn registered_and_reserved_identities_are_rejected() {
        for id in ["nextjs", "cli", "python", "data-analysis", "generic"] {
            assert!(reject_registered_identity(id).is_err(), "{id}");
        }
        assert!(reject_registered_identity("static-site").is_ok());
        assert!(reject_registered_identity("Static-Site").is_err());
    }

    #[test]
    fn fixture_vocabulary_is_rejected_only_inside_execution_sections() {
        let leaking = "[plan]\nintent = \"売上を集計する\"\n";
        assert!(reject_fixture_vocabulary(leaking).is_err());
        let vocabulary_only = "[vocabulary]\nterms = [\"売上\"]\n";
        assert!(reject_fixture_vocabulary(vocabulary_only).is_ok());
    }

    #[test]
    fn manifest_sources_are_labelled_and_only_external_ones_are_supplied() {
        assert!(!ManifestSource::Embedded.is_external());
        assert!(ManifestSource::Repository.is_external());
        assert!(ManifestSource::Local.is_external());
        assert_eq!(ManifestSource::Local.as_str(), "local");
        assert_eq!(ManifestOrigin::Embedded.source(), ManifestSource::Embedded);
        assert_eq!(ManifestOrigin::Embedded.hash(), None);
    }
}
