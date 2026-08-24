//! Extension-root pack supply.
//!
//! `SupplyRoot` is the only write boundary below `--extension-root`. It stages,
//! verifies, pins, retires, and bundles packs and appends one journal line per
//! operation. gui_server and the CLI call these functions instead of writing
//! into the extension root themselves; `tests/protection_coverage_audit.rs`
//! registers that as the `extension_supply_writes` category.
//!
//! The lifecycle follows `docs/pack-institution-contract.md` section 7.2:
//! `stage` only writes an unpinned `id@version` through a temporary directory
//! plus atomic rename, `pin` never overwrites an existing pin, and `retire`
//! creates the `RETIRED` marker without deleting bytes, the pin, or history.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use thiserror::Error;

use super::catalog::{PACK_PIN_FILE, PackStatus, RETIRED_MARKER_FILE, status};
use super::{
    ASSIST_FILE, ConformanceReport, EVAL_FILE, MATERIALS_DIRECTORY, MAX_FILE_BYTES,
    MAX_MATERIAL_BYTES, MAX_TOTAL_MATERIAL_BYTES, PACKS_DIRECTORY,
};

/// Transient directory that holds a staged tree before its atomic rename.
const STAGING_DIRECTORY: &str = ".staging";
/// Upper bound on members accepted by one `stage` call.
pub const MAX_STAGED_FILES: usize = 64;

/// Who performed a supply operation. Contract v0.1 section 7.3 fixes the two
/// serialized values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Actor {
    Gui,
    Cli,
}

/// Journal action vocabulary. Closed by contract v0.1 section 7.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Stage,
    Verify,
    Pin,
    Retire,
}

/// Journal outcome vocabulary. Closed by contract v0.1 section 7.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Ok,
    Error,
}

/// Exact pack tuple recorded on every journal line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JournalPack {
    pub id: String,
    pub version: String,
    pub hash: String,
}

/// One append-only journal line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JournalEntry {
    pub ts: String,
    pub actor: Actor,
    pub action: Action,
    pub pack: JournalPack,
    pub result: Outcome,
    pub detail: String,
}

impl JournalEntry {
    /// Build one entry. `detail` is credential-scrubbed and bounded before it
    /// can reach the file, so a failure message can never publish a secret.
    pub fn new(
        actor: Actor,
        action: Action,
        pack: JournalPack,
        result: Outcome,
        detail: impl AsRef<str>,
    ) -> Self {
        Self {
            ts: now_rfc3339(),
            actor,
            action,
            pack,
            result,
            detail: journal::bounded_detail(detail.as_ref()),
        }
    }
}

/// One member submitted to or read back from a staged pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedFile {
    /// `assist.yaml`, `eval.yaml`, or `materials/<name>.md`.
    pub name: String,
    pub bytes: Vec<u8>,
}

/// Result of the credential scrub applied to every staged member.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScrubReport {
    pub status: &'static str,
    pub scanned: Vec<String>,
}

/// Verification result for one supplied pack.
#[derive(Debug, Serialize)]
pub struct StageReport {
    pub id: String,
    pub version: String,
    pub hash: String,
    pub status: PackStatus,
    pub conformance: ConformanceReport,
    pub scrub: ScrubReport,
    pub directory: PathBuf,
}

/// Listing row for one supplied `id@version`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SuppliedPack {
    pub id: String,
    pub version: String,
    pub status: PackStatus,
    pub hash: Option<String>,
    pub pin: Option<String>,
    pub conformance_ok: bool,
    pub has_assist: bool,
    pub has_eval: bool,
    pub materials: Vec<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Error)]
pub enum SupplyError {
    #[error("extension root `{path}` is unusable: {reason}")]
    Root { path: PathBuf, reason: String },
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Conflict(String),
    #[error("pack `{id}@{version}` is not supplied by the extension root")]
    NotFound { id: String, version: String },
    #[error("pack `{id}@{version}` failed verification: {reason}")]
    Verification {
        id: String,
        version: String,
        hash: Option<String>,
        reason: String,
    },
    #[error("extension supply io failed for `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

impl SupplyError {
    fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}

/// The `packs/` subtree of one operator-supplied extension root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupplyRoot {
    root: PathBuf,
}

impl SupplyRoot {
    /// Open an existing, owner-private, non-symlink extension root. The
    /// permission rule matches the `gui_server --check` preflight so a root the
    /// preflight accepted is the same root supply accepts.
    pub fn open(root: &Path) -> Result<Self, SupplyError> {
        let metadata = std::fs::symlink_metadata(root).map_err(|source| SupplyError::Root {
            path: root.to_path_buf(),
            reason: source.to_string(),
        })?;
        if metadata.file_type().is_symlink() {
            return Err(SupplyError::Root {
                path: root.to_path_buf(),
                reason: "extension root must not be a symlink".to_string(),
            });
        }
        if !metadata.file_type().is_dir() {
            return Err(SupplyError::Root {
                path: root.to_path_buf(),
                reason: "extension root must be a directory".to_string(),
            });
        }
        if let Some(reason) = permission_error(&metadata) {
            return Err(SupplyError::Root {
                path: root.to_path_buf(),
                reason,
            });
        }
        let canonical = root.canonicalize().map_err(|source| SupplyError::Root {
            path: root.to_path_buf(),
            reason: source.to_string(),
        })?;
        Ok(Self { root: canonical })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn packs_root(&self) -> PathBuf {
        self.root.join(PACKS_DIRECTORY)
    }

    /// Absolute directory of one supplied pack. Identifiers are validated
    /// first, so the join can never escape `packs/`.
    pub fn directory(&self, id: &str, version: &str) -> Result<PathBuf, SupplyError> {
        validate_identity(id, version)?;
        Ok(self.packs_root().join(id).join(version))
    }

    /// Enumerate every supplied `id@version` with its lifecycle status. Retired
    /// packs stay listed for audit; `status` reports them as never selectable.
    pub fn list(&self) -> Result<Vec<SuppliedPack>, SupplyError> {
        let packs_root = self.packs_root();
        if !packs_root.is_dir() {
            return Ok(Vec::new());
        }
        let mut rows = Vec::new();
        for id_entry in sorted_directories(&packs_root)? {
            let Some(id) = file_name(&id_entry) else {
                continue;
            };
            if id == STAGING_DIRECTORY {
                continue;
            }
            for version_entry in sorted_directories(&id_entry)? {
                let Some(version) = file_name(&version_entry) else {
                    continue;
                };
                rows.push(self.inspect(&version_entry, id.clone(), version));
            }
        }
        Ok(rows)
    }

    fn inspect(&self, directory: &Path, id: String, version: String) -> SuppliedPack {
        let loaded = super::load_directory(directory);
        let pack_status = status(directory);
        let pin = read_pin(directory);
        let identity_matches = loaded
            .as_ref()
            .is_ok_and(|pack| pack.id() == id && pack.identity.version == version);
        let conformance = loaded
            .as_ref()
            .ok()
            .filter(|_| identity_matches)
            .map(super::conform);
        let pin_matches = pack_status == PackStatus::Staged
            || pin.as_ref() == loaded.as_ref().ok().map(|pack| &pack.hash);
        let detail = match (&loaded, &conformance) {
            (Err(error), _) => Some(error.to_string()),
            (Ok(_), _) if !identity_matches => {
                Some("directory name and pack identity disagree".to_string())
            }
            (Ok(_), Some(Err(error))) => Some(error.to_string()),
            (Ok(_), Some(Ok(_))) if !pin_matches => {
                Some("pack pin does not match the observed exact-byte hash".to_string())
            }
            _ => None,
        };
        SuppliedPack {
            status: pack_status,
            hash: loaded.as_ref().ok().map(|pack| pack.hash.clone()),
            pin,
            conformance_ok: matches!(conformance, Some(Ok(_))) && pin_matches,
            has_assist: directory.join(ASSIST_FILE).is_file(),
            has_eval: directory.join(EVAL_FILE).is_file(),
            materials: loaded
                .as_ref()
                .map(|pack| pack.materials.keys().cloned().collect())
                .unwrap_or_default(),
            detail: detail.map(|detail| journal::bounded_detail(&detail)),
            id,
            version,
        }
    }

    /// Write a new or re-edited unpinned `id@version` and verify it.
    ///
    /// Members are validated and credential-scrubbed before any byte reaches
    /// the disk, the tree is built in a temporary directory and moved with one
    /// rename, and verification always runs afterwards. A verification failure
    /// leaves the staged bytes in place for re-editing but never pins them.
    pub fn stage(
        &self,
        id: &str,
        version: &str,
        files: &[StagedFile],
        actor: Actor,
    ) -> Result<StageReport, SupplyError> {
        let directory = self.directory(id, version)?;
        let members = validate_members(files)?;
        let submitted_hash = members.hash();
        self.validate_managed_path(id, &directory)?;
        match status(&directory) {
            PackStatus::Pinned => {
                self.record(
                    (actor, Action::Stage),
                    id,
                    version,
                    Some(submitted_hash),
                    Outcome::Error,
                    "pinned packs are immutable; supply a new version",
                )?;
                return Err(SupplyError::Conflict(format!(
                    "pack `{id}@{version}` is already pinned; supply a new version"
                )));
            }
            PackStatus::Retired => {
                self.record(
                    (actor, Action::Stage),
                    id,
                    version,
                    Some(submitted_hash),
                    Outcome::Error,
                    "retired packs are immutable; supply a new version",
                )?;
                return Err(SupplyError::Conflict(format!(
                    "pack `{id}@{version}` is retired; supply a new version"
                )));
            }
            PackStatus::Staged => {}
        }
        if let Err(error) = self.install(&directory, &members) {
            self.record(
                (actor, Action::Stage),
                id,
                version,
                Some(submitted_hash),
                Outcome::Error,
                error.to_string(),
            )?;
            return Err(error);
        }
        match self.verify(id, version) {
            Ok(report) => {
                self.record(
                    (actor, Action::Stage),
                    id,
                    version,
                    Some(report.hash.clone()),
                    Outcome::Ok,
                    format!("staged {} member(s)", members.count()),
                )?;
                Ok(report)
            }
            Err(error) => {
                self.record(
                    (actor, Action::Stage),
                    id,
                    version,
                    verification_hash(&error).or(Some(submitted_hash)),
                    Outcome::Error,
                    error.to_string(),
                )?;
                Err(error)
            }
        }
    }

    /// Re-read and re-check one supplied pack without journaling. `stage` and
    /// the read-only detail route use this; the authenticated verify route uses
    /// [`SupplyRoot::verify_recorded`] so the journal keeps one line per
    /// operator action.
    pub fn verify(&self, id: &str, version: &str) -> Result<StageReport, SupplyError> {
        let directory = self.directory(id, version)?;
        if !self.validate_managed_path(id, &directory)? {
            return Err(SupplyError::NotFound {
                id: id.to_string(),
                version: version.to_string(),
            });
        }
        let loaded =
            super::load_directory(&directory).map_err(|error| SupplyError::Verification {
                id: id.to_string(),
                version: version.to_string(),
                hash: None,
                reason: error.to_string(),
            })?;
        if loaded.id() != id || loaded.identity.version != version {
            return Err(SupplyError::Verification {
                id: id.to_string(),
                version: version.to_string(),
                hash: Some(loaded.hash.clone()),
                reason: format!(
                    "directory {id}@{version} declares {}@{}",
                    loaded.id(),
                    loaded.identity.version
                ),
            });
        }
        let mut scanned = Vec::new();
        for (name, bytes) in members_of(&directory, &loaded)? {
            scrub_member(&name, &bytes).map_err(|reason| SupplyError::Verification {
                id: id.to_string(),
                version: version.to_string(),
                hash: Some(loaded.hash.clone()),
                reason,
            })?;
            scanned.push(name);
        }
        let conformance = super::conform(&loaded).map_err(|error| SupplyError::Verification {
            id: id.to_string(),
            version: version.to_string(),
            hash: Some(loaded.hash.clone()),
            reason: error.to_string(),
        })?;
        if matches!(status(&directory), PackStatus::Pinned | PackStatus::Retired) {
            let pin = read_pin(&directory).ok_or_else(|| SupplyError::Verification {
                id: id.to_string(),
                version: version.to_string(),
                hash: Some(loaded.hash.clone()),
                reason: "pinned pack has no readable exact hash".to_string(),
            })?;
            if pin != loaded.hash {
                return Err(SupplyError::Verification {
                    id: id.to_string(),
                    version: version.to_string(),
                    hash: Some(loaded.hash.clone()),
                    reason: format!(
                        "pack pin `{pin}` does not match observed hash `{}`",
                        loaded.hash
                    ),
                });
            }
        }
        Ok(StageReport {
            id: id.to_string(),
            version: version.to_string(),
            hash: loaded.hash,
            status: status(&directory),
            conformance,
            scrub: ScrubReport {
                status: "clean",
                scanned,
            },
            directory,
        })
    }

    /// Verify and append one journal line for the operator action.
    pub fn verify_recorded(
        &self,
        id: &str,
        version: &str,
        actor: Actor,
    ) -> Result<StageReport, SupplyError> {
        match self.verify(id, version) {
            Ok(report) => {
                self.record(
                    (actor, Action::Verify),
                    id,
                    version,
                    Some(report.hash.clone()),
                    Outcome::Ok,
                    format!(
                        "conformant; {} member(s) scrubbed",
                        report.scrub.scanned.len()
                    ),
                )?;
                Ok(report)
            }
            Err(error) => {
                self.record(
                    (actor, Action::Verify),
                    id,
                    version,
                    verification_hash(&error).or_else(|| self.identity_hash(id, version)),
                    Outcome::Error,
                    error.to_string(),
                )?;
                Err(error)
            }
        }
    }

    /// Create `pack.sha256` after re-reading and re-hashing the members. An
    /// existing pin is never overwritten and a retired pack is never pinned.
    pub fn pin(
        &self,
        id: &str,
        version: &str,
        expected_hash: &str,
        actor: Actor,
    ) -> Result<(), SupplyError> {
        let directory = self.directory(id, version)?;
        if !self.validate_managed_path(id, &directory)? {
            return Err(SupplyError::NotFound {
                id: id.to_string(),
                version: version.to_string(),
            });
        }
        match status(&directory) {
            PackStatus::Pinned => {
                self.record(
                    (actor, Action::Pin),
                    id,
                    version,
                    self.identity_hash(id, version),
                    Outcome::Error,
                    "an existing pin is never overwritten",
                )?;
                return Err(SupplyError::Conflict(format!(
                    "pack `{id}@{version}` is already pinned"
                )));
            }
            PackStatus::Retired => {
                self.record(
                    (actor, Action::Pin),
                    id,
                    version,
                    self.identity_hash(id, version),
                    Outcome::Error,
                    "a retired pack is never pinned",
                )?;
                return Err(SupplyError::Conflict(format!(
                    "pack `{id}@{version}` is retired"
                )));
            }
            PackStatus::Staged => {}
        }
        let report = match self.verify(id, version) {
            Ok(report) => report,
            Err(error) => {
                self.record(
                    (actor, Action::Pin),
                    id,
                    version,
                    verification_hash(&error),
                    Outcome::Error,
                    error.to_string(),
                )?;
                return Err(error);
            }
        };
        if expected_hash.trim() != report.hash {
            let reason = format!(
                "pin hash mismatch: requested `{}`, observed `{}`",
                expected_hash.trim(),
                report.hash
            );
            self.record(
                (actor, Action::Pin),
                id,
                version,
                Some(report.hash.clone()),
                Outcome::Error,
                &reason,
            )?;
            return Err(SupplyError::Verification {
                id: id.to_string(),
                version: version.to_string(),
                hash: Some(report.hash),
                reason,
            });
        }
        let pin_path = directory.join(PACK_PIN_FILE);
        if let Err(error) = create_new_file(&pin_path, format!("{}\n", report.hash).as_bytes()) {
            self.record(
                (actor, Action::Pin),
                id,
                version,
                Some(report.hash.clone()),
                Outcome::Error,
                error.to_string(),
            )?;
            return Err(error);
        }
        self.record(
            (actor, Action::Pin),
            id,
            version,
            Some(report.hash),
            Outcome::Ok,
            "pinned",
        )?;
        Ok(())
    }

    /// Create the `RETIRED` marker. Nothing is deleted or rewritten, so the
    /// pack stays listable and bundle-readable for audit.
    pub fn retire(&self, id: &str, version: &str, actor: Actor) -> Result<(), SupplyError> {
        let directory = self.directory(id, version)?;
        if !self.validate_managed_path(id, &directory)? {
            return Err(SupplyError::NotFound {
                id: id.to_string(),
                version: version.to_string(),
            });
        }
        let hash = self.identity_hash(id, version);
        match status(&directory) {
            PackStatus::Retired => {
                self.record(
                    (actor, Action::Retire),
                    id,
                    version,
                    hash,
                    Outcome::Error,
                    "already retired",
                )?;
                return Err(SupplyError::Conflict(format!(
                    "pack `{id}@{version}` is already retired"
                )));
            }
            PackStatus::Staged => {
                self.record(
                    (actor, Action::Retire),
                    id,
                    version,
                    hash,
                    Outcome::Error,
                    "staged packs must be verified and pinned before retirement",
                )?;
                return Err(SupplyError::Conflict(format!(
                    "pack `{id}@{version}` must be pinned before retirement"
                )));
            }
            PackStatus::Pinned => {}
        }
        let Some(hash) = hash else {
            return Err(SupplyError::Verification {
                id: id.to_string(),
                version: version.to_string(),
                hash: None,
                reason: "retirement requires a readable pack hash or pin".to_string(),
            });
        };
        let marker = directory.join(RETIRED_MARKER_FILE);
        if let Err(error) = create_new_file(
            &marker,
            format!("retired-at: {}\n", now_rfc3339()).as_bytes(),
        ) {
            self.record(
                (actor, Action::Retire),
                id,
                version,
                Some(hash.clone()),
                Outcome::Error,
                error.to_string(),
            )?;
            return Err(error);
        }
        self.record(
            (actor, Action::Retire),
            id,
            version,
            Some(hash),
            Outcome::Ok,
            "retired; bytes, pin, and history are unchanged",
        )?;
        Ok(())
    }

    /// Read every pack member back for review or for a repository pull request.
    /// Retired packs remain bundle-readable on purpose.
    pub fn bundle(&self, id: &str, version: &str) -> Result<Vec<StagedFile>, SupplyError> {
        let directory = self.directory(id, version)?;
        if !self.validate_managed_path(id, &directory)? {
            return Err(SupplyError::NotFound {
                id: id.to_string(),
                version: version.to_string(),
            });
        }
        Ok(bundle_members(&directory)?
            .into_iter()
            .map(|(name, bytes)| StagedFile { name, bytes })
            .collect())
    }

    fn install(&self, directory: &Path, members: &Members) -> Result<(), SupplyError> {
        let nonce = uuid::Uuid::now_v7().to_string();
        let staging_root = self.packs_root().join(STAGING_DIRECTORY);
        let session = staging_root.join(&nonce);
        let pending = session.join("pack");
        self.create_directory(&pending)?;
        for (name, bytes) in members.iter() {
            let target = pending.join(&name);
            if let Some(parent) = target.parent() {
                self.create_directory(parent)?;
            }
            create_new_file(&target, &bytes)?;
        }
        if let Some(parent) = directory.parent() {
            self.create_directory(parent)?;
        }
        let replaced = session.join("replaced");
        let previous = directory.is_dir().then_some(replaced.as_path());
        if let Some(previous) = previous {
            std::fs::rename(directory, previous)
                .map_err(|source| SupplyError::io(directory, source))?;
        }
        if let Err(source) = std::fs::rename(&pending, directory) {
            if let Some(previous) = previous {
                let _ = std::fs::rename(previous, directory);
            }
            let _ = std::fs::remove_dir_all(&session);
            return Err(SupplyError::io(directory, source));
        }
        let _ = std::fs::remove_dir_all(&session);
        // Leave no residue: the staging parent is only meaningful while a
        // rename is in flight, and other roots below it belong to other packs.
        let _ = std::fs::remove_dir(staging_root);
        Ok(())
    }

    fn validate_managed_path(&self, id: &str, directory: &Path) -> Result<bool, SupplyError> {
        for path in [self.packs_root(), self.packs_root().join(id)] {
            validate_optional_private_directory(&path)?;
        }
        let exists = validate_optional_private_directory(directory)?;
        if exists {
            for marker in [PACK_PIN_FILE, RETIRED_MARKER_FILE] {
                validate_optional_regular_file(&directory.join(marker))?;
            }
        }
        Ok(exists)
    }

    fn create_directory(&self, path: &Path) -> Result<(), SupplyError> {
        let relative = path.strip_prefix(&self.root).map_err(|_| {
            SupplyError::Invalid(format!(
                "managed directory `{}` escapes the extension root",
                path.display()
            ))
        })?;
        let mut current = self.root.clone();
        for component in relative.components() {
            let std::path::Component::Normal(component) = component else {
                return Err(SupplyError::Invalid(format!(
                    "managed directory `{}` is not normalized",
                    path.display()
                )));
            };
            current.push(component);
            match std::fs::symlink_metadata(&current) {
                Ok(metadata) => validate_private_directory(&current, &metadata)?,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    std::fs::create_dir(&current)
                        .map_err(|source| SupplyError::io(&current, source))?;
                    make_private(&current)?;
                }
                Err(source) => return Err(SupplyError::io(&current, source)),
            }
        }
        Ok(())
    }

    fn identity_hash(&self, id: &str, version: &str) -> Option<String> {
        let directory = self.directory(id, version).ok()?;
        super::load_directory(&directory)
            .ok()
            .map(|pack| pack.hash)
            .or_else(|| read_pin(&directory).filter(|pin| is_pin_hash(pin)))
    }

    fn record(
        &self,
        operation: (Actor, Action),
        id: &str,
        version: &str,
        hash: Option<String>,
        result: Outcome,
        detail: impl AsRef<str>,
    ) -> Result<(), SupplyError> {
        // The schema requires an exact pack tuple, so an attempt whose hash
        // cannot be observed appends nothing rather than a fabricated line.
        let Some(hash) = hash else {
            return Ok(());
        };
        let entry = JournalEntry::new(
            operation.0,
            operation.1,
            JournalPack {
                id: id.to_string(),
                version: version.to_string(),
                hash,
            },
            result,
            detail,
        );
        journal::append(&self.root, &entry)
    }
}

/// Append-only `<extension-root>/journal.jsonl` writer.
pub mod journal {
    use std::io::Write;
    use std::path::Path;
    use std::sync::Mutex;

    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt;

    use serde::Serialize;

    use super::{JournalEntry, SupplyError};

    pub const JOURNAL_FILE: &str = "journal.jsonl";
    /// Contract v0.1 section 7.3 bounds `detail` after the credential scrub.
    pub const MAX_DETAIL_BYTES: usize = 4_096;
    static JOURNAL_LOCK: Mutex<()> = Mutex::new(());

    /// Append one journal line. Existing lines are never read, rewritten, or
    /// reordered; the file is opened for append only.
    pub fn append(root: &Path, entry: &JournalEntry) -> Result<(), SupplyError> {
        append_serializable(root, entry)
    }

    /// Append another extension-supply record without widening the GUI's
    /// filesystem authority. The caller owns the record's closed schema.
    pub(crate) fn append_serializable<T: Serialize>(
        root: &Path,
        entry: &T,
    ) -> Result<(), SupplyError> {
        if !super::validate_optional_private_directory(root)? {
            return Err(SupplyError::Invalid(format!(
                "extension journal root `{}` does not exist",
                root.display()
            )));
        }
        let path = root.join(JOURNAL_FILE);
        let line = serde_json::to_string(entry)
            .map_err(|error| SupplyError::Invalid(format!("serialize journal entry: {error}")))?;
        let _lock = JOURNAL_LOCK
            .lock()
            .map_err(|_| SupplyError::Invalid("extension journal lock is poisoned".to_string()))?;
        if let Ok(metadata) = std::fs::symlink_metadata(&path)
            && !metadata.file_type().is_file()
        {
            return Err(SupplyError::Invalid(format!(
                "extension journal `{}` must be a non-symlink regular file",
                path.display()
            )));
        }
        let mut options = std::fs::OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        options.custom_flags(libc::O_NOFOLLOW).mode(0o600);
        let mut file = options
            .open(&path)
            .map_err(|source| SupplyError::io(&path, source))?;
        writeln!(file, "{line}").map_err(|source| SupplyError::io(&path, source))
    }

    /// Credential-scrub and bound one `detail` value.
    pub fn bounded_detail(detail: &str) -> String {
        let redacted = super::super::material_document::redact_credentials(detail);
        let single_line = redacted.replace(['\n', '\r'], " ");
        truncate_utf8(single_line.trim(), MAX_DETAIL_BYTES)
    }

    fn truncate_utf8(text: &str, max_bytes: usize) -> String {
        if text.len() <= max_bytes {
            return text.to_string();
        }
        let mut end = max_bytes;
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }
        text[..end].to_string()
    }
}

/// Validated staged members, keyed by their pack-relative name.
struct Members {
    assist: Option<Vec<u8>>,
    eval: Option<Vec<u8>>,
    materials: BTreeMap<String, Vec<u8>>,
}

impl Members {
    fn count(&self) -> usize {
        usize::from(self.assist.is_some()) + usize::from(self.eval.is_some()) + self.materials.len()
    }

    fn hash(&self) -> String {
        super::exact_byte_hash_with_materials(
            self.assist.as_deref(),
            self.eval.as_deref(),
            &self.materials,
        )
    }

    fn iter(&self) -> Vec<(String, Vec<u8>)> {
        let mut members = Vec::new();
        if let Some(bytes) = &self.assist {
            members.push((ASSIST_FILE.to_string(), bytes.clone()));
        }
        if let Some(bytes) = &self.eval {
            members.push((EVAL_FILE.to_string(), bytes.clone()));
        }
        for (name, bytes) in &self.materials {
            members.push((format!("{MATERIALS_DIRECTORY}/{name}"), bytes.clone()));
        }
        members
    }
}

/// Read every member back as exact bytes. `load_directory` already bounded and
/// classified them, so this re-read only reproduces the pinned bytes.
fn members_of(
    directory: &Path,
    pack: &super::LoadedPack,
) -> Result<Vec<(String, Vec<u8>)>, SupplyError> {
    let mut members = Vec::new();
    for (present, name) in [
        (pack.assist.is_some(), ASSIST_FILE),
        (pack.eval.is_some(), EVAL_FILE),
    ] {
        if present {
            let path = directory.join(name);
            let bytes =
                std::fs::read(&path).map_err(|source| SupplyError::io(path.clone(), source))?;
            members.push((name.to_string(), bytes));
        }
    }
    for (name, bytes) in &pack.materials {
        members.push((format!("{MATERIALS_DIRECTORY}/{name}"), bytes.clone()));
    }
    Ok(members)
}

/// Bundle members without decoding YAML so a failed staged document remains
/// available for authenticated repair. Names, bounds, UTF-8, and symlink
/// rules are still enforced before any byte is returned.
fn bundle_members(directory: &Path) -> Result<Vec<(String, Vec<u8>)>, SupplyError> {
    let mut members = Vec::new();
    for name in [ASSIST_FILE, EVAL_FILE] {
        let path = directory.join(name);
        if let Some(bytes) = read_optional_member(&path, MAX_FILE_BYTES)? {
            members.push((name.to_string(), bytes));
        }
    }
    let materials = directory.join(MATERIALS_DIRECTORY);
    if !validate_optional_private_directory(&materials)? {
        return Ok(members);
    }
    let mut total = 0_u64;
    let mut paths = std::fs::read_dir(&materials)
        .map_err(|source| SupplyError::io(&materials, source))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| SupplyError::io(&materials, source))?;
    paths.sort();
    for path in paths {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| super::valid_material_name(name))
            .ok_or_else(|| {
                SupplyError::Invalid(format!(
                    "pack material `{}` has an invalid name",
                    path.display()
                ))
            })?;
        let bytes = read_optional_member(&path, MAX_MATERIAL_BYTES)?.ok_or_else(|| {
            SupplyError::Invalid(format!("pack material `{}` disappeared", path.display()))
        })?;
        total = total
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| SupplyError::Invalid("pack materials are too large".to_string()))?;
        if total > MAX_TOTAL_MATERIAL_BYTES {
            return Err(SupplyError::Invalid(format!(
                "materials exceed {MAX_TOTAL_MATERIAL_BYTES} bytes in aggregate"
            )));
        }
        members.push((format!("{MATERIALS_DIRECTORY}/{name}"), bytes));
    }
    Ok(members)
}

fn read_optional_member(path: &Path, max_bytes: u64) -> Result<Option<Vec<u8>>, SupplyError> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(SupplyError::io(path, source)),
    };
    if !metadata.file_type().is_file() {
        return Err(SupplyError::Invalid(format!(
            "pack member `{}` is not a non-symlink regular file",
            path.display()
        )));
    }
    if metadata.len() > max_bytes {
        return Err(SupplyError::Invalid(format!(
            "pack member `{}` exceeds {max_bytes} bytes",
            path.display()
        )));
    }
    let bytes = std::fs::read(path).map_err(|source| SupplyError::io(path, source))?;
    std::str::from_utf8(&bytes).map_err(|_| {
        SupplyError::Invalid(format!(
            "pack member `{}` is not valid UTF-8",
            path.display()
        ))
    })?;
    Ok(Some(bytes))
}

fn validate_members(files: &[StagedFile]) -> Result<Members, SupplyError> {
    if files.is_empty() {
        return Err(SupplyError::Invalid(
            "a staged pack must contain assist.yaml or eval.yaml".to_string(),
        ));
    }
    if files.len() > MAX_STAGED_FILES {
        return Err(SupplyError::Invalid(format!(
            "a staged pack accepts at most {MAX_STAGED_FILES} members"
        )));
    }
    let mut assist = None;
    let mut eval = None;
    let mut materials = BTreeMap::new();
    let mut material_total = 0_usize;
    for file in files {
        let name = file.name.as_str();
        if name.contains("..")
            || name.starts_with('/')
            || name.contains('\\')
            || name.contains('\0')
        {
            return Err(SupplyError::Invalid(format!(
                "member name `{name}` must be a relative pack path"
            )));
        }
        if file.bytes.len() as u64 > MAX_FILE_BYTES {
            return Err(SupplyError::Invalid(format!(
                "member `{name}` exceeds {MAX_FILE_BYTES} bytes"
            )));
        }
        std::str::from_utf8(&file.bytes)
            .map_err(|_| SupplyError::Invalid(format!("member `{name}` is not valid UTF-8")))?;
        scrub_member(name, &file.bytes).map_err(SupplyError::Invalid)?;
        match name {
            ASSIST_FILE if assist.is_none() => assist = Some(file.bytes.clone()),
            EVAL_FILE if eval.is_none() => eval = Some(file.bytes.clone()),
            ASSIST_FILE | EVAL_FILE => {
                return Err(SupplyError::Invalid(format!("member `{name}` is repeated")));
            }
            _ => {
                let material = name
                    .strip_prefix(MATERIALS_DIRECTORY)
                    .and_then(|rest| rest.strip_prefix('/'))
                    .filter(|rest| !rest.contains('/'))
                    .filter(|rest| super::valid_material_name(rest))
                    .ok_or_else(|| {
                        SupplyError::Invalid(format!(
                            "member `{name}` must be assist.yaml, eval.yaml, or materials/<name>.md"
                        ))
                    })?;
                if file.bytes.len() as u64 > MAX_MATERIAL_BYTES {
                    return Err(SupplyError::Invalid(format!(
                        "material `{name}` exceeds {MAX_MATERIAL_BYTES} bytes"
                    )));
                }
                material_total += file.bytes.len();
                if material_total as u64 > MAX_TOTAL_MATERIAL_BYTES {
                    return Err(SupplyError::Invalid(format!(
                        "materials exceed {MAX_TOTAL_MATERIAL_BYTES} bytes in aggregate"
                    )));
                }
                if materials
                    .insert(material.to_string(), file.bytes.clone())
                    .is_some()
                {
                    return Err(SupplyError::Invalid(format!("member `{name}` is repeated")));
                }
            }
        }
    }
    if assist.is_none() && eval.is_none() {
        return Err(SupplyError::Invalid(
            "a staged pack must contain assist.yaml or eval.yaml".to_string(),
        ));
    }
    Ok(Members {
        assist,
        eval,
        materials,
    })
}

/// Pack identifiers follow the contract regex, which also makes them safe path
/// segments: no separator, no dot segment, no absolute prefix.
fn validate_identity(id: &str, version: &str) -> Result<(), SupplyError> {
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
        return Err(SupplyError::Invalid(format!("invalid pack id `{id}`")));
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
        return Err(SupplyError::Invalid(format!(
            "pack version `{version}` must be SemVer core MAJOR.MINOR.PATCH"
        )));
    }
    Ok(())
}

fn scrub_member(name: &str, bytes: &[u8]) -> Result<(), String> {
    let text = std::str::from_utf8(bytes).map_err(|_| format!("`{name}` is not valid UTF-8"))?;
    super::material_document::reject_credentials(text)
        .map_err(|_| format!("`{name}` was rejected by the credential scrub"))
}

fn verification_hash(error: &SupplyError) -> Option<String> {
    match error {
        SupplyError::Verification { hash, .. } => hash.clone(),
        _ => None,
    }
}

fn is_pin_hash(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn read_pin(directory: &Path) -> Option<String> {
    let path = directory.join(PACK_PIN_FILE);
    let metadata = std::fs::symlink_metadata(&path).ok()?;
    if !metadata.file_type().is_file() || metadata.len() > 128 {
        return None;
    }
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn validate_optional_regular_file(path: &Path) -> Result<bool, SupplyError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => Ok(true),
        Ok(_) => Err(SupplyError::Invalid(format!(
            "managed marker `{}` must be a non-symlink regular file",
            path.display()
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(SupplyError::io(path, source)),
    }
}

fn create_new_file(path: &Path, bytes: &[u8]) -> Result<(), SupplyError> {
    use std::io::Write;

    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt;

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW);
    let mut file = options
        .open(path)
        .map_err(|source| SupplyError::io(path, source))?;
    file.write_all(bytes)
        .map_err(|source| SupplyError::io(path, source))
}

fn validate_optional_private_directory(path: &Path) -> Result<bool, SupplyError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            validate_private_directory(path, &metadata)?;
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(SupplyError::io(path, source)),
    }
}

fn validate_private_directory(
    path: &Path,
    metadata: &std::fs::Metadata,
) -> Result<(), SupplyError> {
    if !metadata.file_type().is_dir() {
        return Err(SupplyError::Invalid(format!(
            "managed path `{}` must be a non-symlink directory",
            path.display()
        )));
    }
    if let Some(reason) = permission_error(metadata) {
        return Err(SupplyError::Invalid(format!(
            "managed directory `{}` is unusable: {reason}",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(unix)]
fn make_private(path: &Path) -> Result<(), SupplyError> {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .map_err(|source| SupplyError::io(path, source))
}

#[cfg(not(unix))]
fn make_private(_: &Path) -> Result<(), SupplyError> {
    Ok(())
}

fn sorted_directories(root: &Path) -> Result<Vec<PathBuf>, SupplyError> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(root).map_err(|source| SupplyError::io(root, source))? {
        let entry = entry.map_err(|source| SupplyError::io(root, source))?;
        if entry
            .file_type()
            .map_err(|source| SupplyError::io(entry.path(), source))?
            .is_dir()
        {
            entries.push(entry.path());
        }
    }
    entries.sort();
    Ok(entries)
}

fn file_name(path: &Path) -> Option<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
}

pub(crate) fn now_rfc3339() -> String {
    crate::fetch_probe::time::rfc3339_utc(crate::fetch_probe::time::unix_epoch_ms())
}

#[cfg(unix)]
fn permission_error(metadata: &std::fs::Metadata) -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::fs::PermissionsExt;

    // SAFETY: `geteuid` has no preconditions and does not dereference memory.
    let effective_uid = unsafe { libc::geteuid() };
    if metadata.uid() != effective_uid {
        return Some(format!(
            "directory owner uid is {}, expected effective uid {effective_uid}",
            metadata.uid()
        ));
    }
    let mode = metadata.permissions().mode() & 0o777;
    if mode & 0o700 != 0o700 {
        return Some(format!(
            "owner permissions are {mode:03o}, expected rwx for extension supply"
        ));
    }
    if mode & 0o077 != 0 {
        return Some(format!(
            "permissions are {mode:03o}, expected private 700 for extension supply"
        ));
    }
    None
}

#[cfg(not(unix))]
fn permission_error(metadata: &std::fs::Metadata) -> Option<String> {
    metadata
        .permissions()
        .readonly()
        .then(|| "extension root is read-only".to_string())
}

#[cfg(test)]
mod tests;
