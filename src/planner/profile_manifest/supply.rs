//! Bounded extension-root supply for external draft profile documents.
//!
//! This module is the only write boundary below `<extension-root>/profiles`.
//! It validates bytes before creating managed directories, installs one fully
//! synced regular file without replacement, and records the outcome without
//! journaling submitted document text.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

use crate::planner::pack::{Actor, SupplyError as PackSupplyError, SupplyRoot};

use super::overlay::{self, LoadedOverlay};
use super::source::{
    self, EXTENSION_MANIFEST_FILE, EXTENSION_OVERLAY_FILE, EXTENSION_PROFILES_DIRECTORY,
    ExtensionManifestError, LoadedManifest, MAX_MANIFEST_BYTES, ManifestSource,
};

pub const ASSURANCE_CEILING: &str = "static";
pub const MAX_PROFILE_BODY_BYTES: usize = MAX_MANIFEST_BYTES as usize + 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileDocumentKind {
    Manifest,
    Overlay,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProfilePreview {
    pub id: String,
    pub display_name: String,
    pub kind: ProfileDocumentKind,
    pub path: String,
    pub hash: String,
    pub source: &'static str,
    pub status: &'static str,
    pub assurance_ceiling: &'static str,
    pub base_profile: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProfileRegistrationReport {
    pub profile: ProfilePreview,
    pub idempotent: bool,
    pub saved: bool,
    pub restart_required: bool,
    pub restart_instruction: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProfileCatalogEntry {
    #[serde(flatten)]
    pub profile: ProfilePreview,
    pub available: bool,
    pub restart_required: bool,
}

#[derive(Debug, Error)]
pub enum ProfileSupplyError {
    #[error("extension root is unusable: {0}")]
    Root(String),
    #[error("profile supply path is rejected: {0}")]
    InvalidPath(String),
    #[error("profile document exceeds {limit} bytes")]
    TooLarge { limit: u64 },
    #[error("profile validation failed: {0}")]
    Validation(String),
    #[error("profile supply conflict: {0}")]
    Conflict(String),
    #[error("profile supply I/O failed: {0}")]
    Io(String),
}

#[derive(Debug, Clone)]
pub struct ProfileSupplyRoot {
    root: PathBuf,
}

#[derive(Debug)]
struct Destination {
    relative: PathBuf,
    directory_name: String,
    kind: ProfileDocumentKind,
}

#[derive(Debug, Serialize)]
struct ProfileJournalEntry {
    ts: String,
    actor: Actor,
    action: &'static str,
    profile: JournalProfile,
    result: &'static str,
    detail: String,
}

#[derive(Debug, Serialize)]
struct JournalProfile {
    id: String,
    path: String,
    hash: String,
}

impl ProfileSupplyRoot {
    pub fn open(root: &Path) -> Result<Self, ProfileSupplyError> {
        let supply = SupplyRoot::open(root).map_err(map_root_error)?;
        Ok(Self {
            root: supply.root().to_path_buf(),
        })
    }

    pub fn preview(
        &self,
        relative_path: &str,
        bytes: &[u8],
    ) -> Result<ProfilePreview, ProfileSupplyError> {
        let destination = Destination::parse(relative_path)?;
        self.reject_managed_symlink(&destination)?;
        if bytes.len() as u64 > MAX_MANIFEST_BYTES {
            return Err(ProfileSupplyError::TooLarge {
                limit: MAX_MANIFEST_BYTES,
            });
        }
        let preview = decode(&destination, bytes)?;
        self.reject_identity_collision(&preview, &destination)?;
        Ok(preview)
    }

    pub fn register(
        &self,
        relative_path: &str,
        bytes: &[u8],
        expected_hash: &str,
        actor: Actor,
    ) -> Result<ProfileRegistrationReport, ProfileSupplyError> {
        let attempted_hash = source::exact_byte_hash("profile-document", bytes);
        let outcome = self.register_inner(relative_path, bytes, expected_hash);
        let (journal_profile, result, detail) = match &outcome {
            Ok(report) => (
                JournalProfile {
                    id: report.profile.id.clone(),
                    path: report.profile.path.clone(),
                    hash: report.profile.hash.clone(),
                },
                "ok",
                if report.idempotent {
                    "同一内容を確認しました。既存ファイルは変更していません。".to_string()
                } else {
                    "draft profile を保存しました。runtime 反映には再起動が必要です。".to_string()
                },
            ),
            Err(error) => (
                JournalProfile {
                    id: "unvalidated".to_string(),
                    path: Destination::parse(relative_path)
                        .map(|destination| destination.display())
                        .unwrap_or_else(|_| "rejected".to_string()),
                    hash: attempted_hash,
                },
                "error",
                error.to_string(),
            ),
        };
        let entry = ProfileJournalEntry {
            ts: crate::planner::pack::supply::now_rfc3339(),
            actor,
            action: "profile_register",
            profile: journal_profile,
            result,
            detail: crate::planner::pack::supply::journal::bounded_detail(&detail),
        };
        crate::planner::pack::supply::journal::append_serializable(&self.root, &entry)
            .map_err(map_journal_error)?;
        outcome
    }

    pub fn catalog(&self) -> Result<Vec<ProfileCatalogEntry>, ProfileSupplyError> {
        let mut rows = Vec::new();
        for manifest in source::load_extension_manifests(&self.root).map_err(validation_error)? {
            rows.push(catalog_manifest(&self.root, manifest)?);
        }
        for loaded in overlay::load_extension_overlays(&self.root).map_err(validation_error)? {
            rows.push(catalog_overlay(&self.root, loaded)?);
        }
        rows.sort_by(|left, right| left.profile.id.cmp(&right.profile.id));
        Ok(rows)
    }

    fn register_inner(
        &self,
        relative_path: &str,
        bytes: &[u8],
        expected_hash: &str,
    ) -> Result<ProfileRegistrationReport, ProfileSupplyError> {
        let preview = self.preview(relative_path, bytes)?;
        if preview.hash != expected_hash {
            return Err(ProfileSupplyError::Conflict(
                "preview の exact hash と保存要求が一致しません。再検証してください。".to_string(),
            ));
        }
        let destination = Destination::parse(relative_path)?;
        let target = self.root.join(&destination.relative);
        if let Some(existing) = read_existing(&target, &destination)? {
            if existing == bytes {
                return Ok(registration_report(&self.root, preview, true));
            }
            return Err(ProfileSupplyError::Conflict(format!(
                "{} は異なる内容で既に存在します。既存ファイルを上書きしません。",
                destination.display()
            )));
        }

        let parent = target.parent().expect("validated destination has a parent");
        create_private_directory(&self.root.join(EXTENSION_PROFILES_DIRECTORY))?;
        create_private_directory(parent)?;
        self.reject_managed_symlink(&destination)?;

        match install_no_replace(parent, &target, bytes) {
            Ok(()) => Ok(registration_report(&self.root, preview, false)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let existing = read_existing(&target, &destination)?.ok_or_else(|| {
                    ProfileSupplyError::Io(format!(
                        "{} の同時作成結果を確認できません。",
                        destination.display()
                    ))
                })?;
                if existing == bytes {
                    Ok(registration_report(&self.root, preview, true))
                } else {
                    Err(ProfileSupplyError::Conflict(format!(
                        "{} は別の要求により異なる内容で作成されました。",
                        destination.display()
                    )))
                }
            }
            Err(error) => Err(ProfileSupplyError::Io(format!(
                "{} を atomic install できません: {error}",
                destination.display()
            ))),
        }
    }

    fn reject_managed_symlink(&self, destination: &Destination) -> Result<(), ProfileSupplyError> {
        let profiles = self.root.join(EXTENSION_PROFILES_DIRECTORY);
        require_optional_directory(&profiles, "profiles")?;
        let directory = self.root.join(
            destination
                .relative
                .parent()
                .expect("validated destination has a parent"),
        );
        require_optional_directory(
            &directory,
            &format!("profiles/{}", destination.directory_name),
        )?;
        let target = self.root.join(&destination.relative);
        match fs::symlink_metadata(&target) {
            Ok(metadata) if metadata.file_type().is_file() => Ok(()),
            Ok(_) => Err(ProfileSupplyError::InvalidPath(format!(
                "{} must be a non-symlink regular file",
                destination.display()
            ))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(ProfileSupplyError::Io(format!(
                "{} を確認できません: {error}",
                destination.display()
            ))),
        }
    }

    fn reject_identity_collision(
        &self,
        candidate: &ProfilePreview,
        destination: &Destination,
    ) -> Result<(), ProfileSupplyError> {
        let target = self.root.join(&destination.relative);
        for manifest in source::load_extension_manifests(&self.root).map_err(validation_error)? {
            if manifest.id() == candidate.id && manifest.origin.path() != Some(target.as_path()) {
                return Err(ProfileSupplyError::Conflict(format!(
                    "profile id `{}` は別の外部 manifest で既に使用されています。",
                    candidate.id
                )));
            }
        }
        for loaded in overlay::load_extension_overlays(&self.root).map_err(validation_error)? {
            if loaded.id() == candidate.id && loaded.path != target {
                return Err(ProfileSupplyError::Conflict(format!(
                    "profile id `{}` は別の外部 overlay で既に使用されています。",
                    candidate.id
                )));
            }
        }
        Ok(())
    }
}

impl Destination {
    fn parse(value: &str) -> Result<Self, ProfileSupplyError> {
        if value.is_empty() || value.starts_with('/') || value.contains('\\') {
            return Err(invalid_path());
        }
        let parts = value.split('/').collect::<Vec<_>>();
        if parts.len() != 3
            || parts[0] != EXTENSION_PROFILES_DIRECTORY
            || parts
                .iter()
                .any(|part| part.is_empty() || matches!(*part, "." | ".."))
        {
            return Err(invalid_path());
        }
        let kind = match parts[2] {
            EXTENSION_MANIFEST_FILE => ProfileDocumentKind::Manifest,
            EXTENSION_OVERLAY_FILE => ProfileDocumentKind::Overlay,
            _ => return Err(invalid_path()),
        };
        Ok(Self {
            relative: PathBuf::from(value),
            directory_name: parts[1].to_string(),
            kind,
        })
    }

    fn display(&self) -> String {
        self.relative.to_string_lossy().replace('\\', "/")
    }
}

fn invalid_path() -> ProfileSupplyError {
    ProfileSupplyError::InvalidPath(
        "`profiles/<id>/manifest.toml` または `profiles/<admitted-base>/overlay.toml` を指定してください。"
            .to_string(),
    )
}

fn decode(destination: &Destination, bytes: &[u8]) -> Result<ProfilePreview, ProfileSupplyError> {
    let path = &destination.relative;
    let directory = path.parent().expect("validated destination has a parent");
    match destination.kind {
        ProfileDocumentKind::Manifest => {
            let loaded = source::decode(directory, path, bytes).map_err(validation_error)?;
            Ok(preview_manifest(loaded))
        }
        ProfileDocumentKind::Overlay => {
            let loaded = overlay::decode(directory, path, bytes, ManifestSource::Local)
                .map_err(validation_error)?;
            Ok(preview_overlay(loaded))
        }
    }
}

fn preview_manifest(loaded: LoadedManifest) -> ProfilePreview {
    ProfilePreview {
        id: loaded.id().to_string(),
        display_name: loaded.display_name().to_string(),
        kind: ProfileDocumentKind::Manifest,
        path: loaded.contract_ref(),
        hash: loaded
            .hash()
            .expect("extension manifest has a hash")
            .to_string(),
        source: loaded.source().as_str(),
        status: "draft",
        assurance_ceiling: ASSURANCE_CEILING,
        base_profile: None,
        warnings: loaded.warnings,
    }
}

fn preview_overlay(loaded: LoadedOverlay) -> ProfilePreview {
    ProfilePreview {
        id: loaded.id().to_string(),
        display_name: loaded.display_name().to_string(),
        kind: ProfileDocumentKind::Overlay,
        path: loaded.path.to_string_lossy().replace('\\', "/"),
        hash: loaded.hash,
        source: loaded.source.as_str(),
        status: "draft",
        assurance_ceiling: ASSURANCE_CEILING,
        base_profile: Some(loaded.base_profile.to_string()),
        warnings: Vec::new(),
    }
}

fn catalog_manifest(
    root: &Path,
    loaded: LoadedManifest,
) -> Result<ProfileCatalogEntry, ProfileSupplyError> {
    let mut profile = preview_manifest(loaded);
    profile.path = catalog_relative_path(root, &profile.path)?;
    Ok(catalog_entry(root, profile))
}

fn catalog_overlay(
    root: &Path,
    loaded: LoadedOverlay,
) -> Result<ProfileCatalogEntry, ProfileSupplyError> {
    let mut profile = preview_overlay(loaded);
    profile.path = catalog_relative_path(root, &profile.path)?;
    Ok(catalog_entry(root, profile))
}

fn catalog_relative_path(root: &Path, value: &str) -> Result<String, ProfileSupplyError> {
    let path = Path::new(value);
    let relative = if path.is_absolute() {
        path.strip_prefix(root).map_err(|_| {
            ProfileSupplyError::InvalidPath(
                "catalog profile path is outside the configured extension root".to_string(),
            )
        })?
    } else {
        path
    };
    let normalized = relative.to_string_lossy().replace('\\', "/");
    Destination::parse(&normalized).map(|destination| destination.display())
}

fn catalog_entry(root: &Path, profile: ProfilePreview) -> ProfileCatalogEntry {
    let available = registered_exact_hash(root, &profile);
    ProfileCatalogEntry {
        profile,
        available,
        restart_required: !available,
    }
}

fn registration_report(
    root: &Path,
    profile: ProfilePreview,
    idempotent: bool,
) -> ProfileRegistrationReport {
    let available = registered_exact_hash(root, &profile);
    ProfileRegistrationReport {
        profile,
        idempotent,
        saved: true,
        restart_required: !available,
        restart_instruction: "GUI サーバーを同じ --extension-root で再起動し、Layer 2 と Trial 候補の exact hash を確認してください。",
    }
}

fn registered_exact_hash(root: &Path, profile: &ProfilePreview) -> bool {
    crate::planner::extension_profiles::registered_root().as_deref() == Some(root)
        && crate::planner::extension_profiles::registered()
            .iter()
            .any(|registered| {
                registered.id == profile.id && registered.manifest_hash == profile.hash
            })
}

fn read_existing(
    target: &Path,
    destination: &Destination,
) -> Result<Option<Vec<u8>>, ProfileSupplyError> {
    let metadata = match fs::symlink_metadata(target) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(ProfileSupplyError::Io(format!(
                "{} を確認できません: {error}",
                destination.display()
            )));
        }
    };
    if !metadata.file_type().is_file() {
        return Err(ProfileSupplyError::InvalidPath(format!(
            "{} must be a non-symlink regular file",
            destination.display()
        )));
    }
    if metadata.len() > MAX_MANIFEST_BYTES {
        return Err(ProfileSupplyError::TooLarge {
            limit: MAX_MANIFEST_BYTES,
        });
    }
    fs::read(target).map(Some).map_err(|error| {
        ProfileSupplyError::Io(format!(
            "{} を読み取れません: {error}",
            destination.display()
        ))
    })
}

fn require_optional_directory(path: &Path, display: &str) -> Result<(), ProfileSupplyError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => Ok(()),
        Ok(_) => Err(ProfileSupplyError::InvalidPath(format!(
            "{display} must be a non-symlink directory"
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(ProfileSupplyError::Io(format!(
            "{display} を確認できません: {error}"
        ))),
    }
}

fn create_private_directory(path: &Path) -> Result<(), ProfileSupplyError> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    match builder.create(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            require_optional_directory(path, "managed profile directory")
        }
        Err(error) => Err(ProfileSupplyError::Io(format!(
            "managed profile directory を作成できません: {error}"
        ))),
    }
}

fn install_no_replace(parent: &Path, target: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let file_name = target
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("profile.toml");
    let temporary = parent.join(format!(".{file_name}.pending-{}", Uuid::now_v7()));
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW).mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::hard_link(&temporary, target)?;
        fs::remove_file(&temporary)?;
        sync_directory(parent)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> std::io::Result<()> {
    fs::File::open(path)?.sync_all()
}

#[cfg(not(unix))]
fn sync_directory(_: &Path) -> std::io::Result<()> {
    Ok(())
}

fn validation_error(error: ExtensionManifestError) -> ProfileSupplyError {
    ProfileSupplyError::Validation(error.to_string())
}

fn map_root_error(error: PackSupplyError) -> ProfileSupplyError {
    match error {
        PackSupplyError::Root { reason, .. } => ProfileSupplyError::Root(reason),
        other => ProfileSupplyError::Root(other.to_string()),
    }
}

fn map_journal_error(error: PackSupplyError) -> ProfileSupplyError {
    ProfileSupplyError::Io(format!("journal に記録できません: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
        }
        root
    }

    fn manifest(id: &str) -> String {
        super::super::commands::template(id)
    }

    fn overlay(id: &str) -> String {
        format!(
            "[metadata]\nid = \"{id}\"\ndisplay_name = \"Overlay\"\nschema_version = \"v1\"\nstatus = \"draft\"\n[overlay]\nbase_profile = \"nextjs\"\nmode = \"additive\"\n[artifacts]\nrequired = [\"SECURITY.md\"]\n"
        )
    }

    #[test]
    fn preview_rejects_unbounded_paths_sizes_and_vocabulary() {
        let root = root();
        let supply = ProfileSupplyRoot::open(root.path()).unwrap();
        let body = manifest("neutral-profile");
        for path in [
            "/profiles/neutral-profile/manifest.toml",
            "profiles/../neutral-profile/manifest.toml",
            "profiles/neutral-profile/../../outside.toml",
            "profiles\\neutral-profile\\manifest.toml",
            "profiles/neutral-profile/other.toml",
        ] {
            assert!(
                matches!(
                    supply.preview(path, body.as_bytes()),
                    Err(ProfileSupplyError::InvalidPath(_))
                ),
                "{path}"
            );
        }
        assert!(matches!(
            supply.preview(
                "profiles/neutral-profile/manifest.toml",
                &vec![b'x'; MAX_MANIFEST_BYTES as usize + 1]
            ),
            Err(ProfileSupplyError::TooLarge { .. })
        ));
        let unknown = body.replace("scaffold_files_present", "not_registered");
        assert!(matches!(
            supply.preview("profiles/neutral-profile/manifest.toml", unknown.as_bytes()),
            Err(ProfileSupplyError::Validation(_))
        ));
    }

    #[test]
    fn preview_uses_existing_manifest_and_overlay_validators() {
        let root = root();
        let supply = ProfileSupplyRoot::open(root.path()).unwrap();
        let preview = supply
            .preview(
                "profiles/neutral-profile/manifest.toml",
                manifest("neutral-profile").as_bytes(),
            )
            .unwrap();
        assert_eq!(preview.id, "neutral-profile");
        assert_eq!(preview.path, "profiles/neutral-profile/manifest.toml");
        assert_eq!(preview.status, "draft");
        assert_eq!(preview.assurance_ceiling, "static");

        let preview = supply
            .preview(
                "profiles/nextjs/overlay.toml",
                overlay("nextjs-extra").as_bytes(),
            )
            .unwrap();
        assert_eq!(preview.id, "nextjs-extra");
        assert_eq!(preview.base_profile.as_deref(), Some("nextjs"));

        let invalid = overlay("nextjs-extra").replace("additive", "replace");
        assert!(matches!(
            supply.preview("profiles/nextjs/overlay.toml", invalid.as_bytes()),
            Err(ProfileSupplyError::Validation(_))
        ));
        assert!(matches!(
            supply.preview(
                "profiles/nextjs/manifest.toml",
                manifest("nextjs").as_bytes()
            ),
            Err(ProfileSupplyError::Validation(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn managed_symlinks_never_redirect_profile_writes() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let profiles_root = root();
        let outside = tempfile::tempdir().unwrap();
        fs::set_permissions(outside.path(), fs::Permissions::from_mode(0o700)).unwrap();
        symlink(outside.path(), profiles_root.path().join("profiles")).unwrap();
        let supply = ProfileSupplyRoot::open(profiles_root.path()).unwrap();
        let result = supply.preview(
            "profiles/neutral-profile/manifest.toml",
            manifest("neutral-profile").as_bytes(),
        );
        assert!(matches!(result, Err(ProfileSupplyError::InvalidPath(_))));
        assert!(
            !outside
                .path()
                .join("neutral-profile/manifest.toml")
                .exists()
        );

        let directory_root = root();
        fs::create_dir(directory_root.path().join("profiles")).unwrap();
        symlink(
            outside.path(),
            directory_root.path().join("profiles/neutral-profile"),
        )
        .unwrap();
        let supply = ProfileSupplyRoot::open(directory_root.path()).unwrap();
        assert!(matches!(
            supply.preview(
                "profiles/neutral-profile/manifest.toml",
                manifest("neutral-profile").as_bytes(),
            ),
            Err(ProfileSupplyError::InvalidPath(_))
        ));

        let target_root = root();
        fs::create_dir(target_root.path().join("profiles")).unwrap();
        fs::create_dir(target_root.path().join("profiles/neutral-profile")).unwrap();
        symlink(
            outside.path().join("manifest.toml"),
            target_root
                .path()
                .join("profiles/neutral-profile/manifest.toml"),
        )
        .unwrap();
        let supply = ProfileSupplyRoot::open(target_root.path()).unwrap();
        assert!(matches!(
            supply.preview(
                "profiles/neutral-profile/manifest.toml",
                manifest("neutral-profile").as_bytes(),
            ),
            Err(ProfileSupplyError::InvalidPath(_))
        ));
    }

    #[test]
    fn register_is_atomic_idempotent_and_never_overwrites() {
        let root = root();
        let supply = ProfileSupplyRoot::open(root.path()).unwrap();
        let path = "profiles/neutral-profile/manifest.toml";
        let body = manifest("neutral-profile");
        let preview = supply.preview(path, body.as_bytes()).unwrap();
        let first = supply
            .register(path, body.as_bytes(), &preview.hash, Actor::Gui)
            .unwrap();
        assert!(!first.idempotent);
        assert!(first.restart_required);
        assert_eq!(fs::read(root.path().join(path)).unwrap(), body.as_bytes());
        assert!(
            fs::read_dir(root.path().join("profiles/neutral-profile"))
                .unwrap()
                .all(|entry| !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .contains("pending"))
        );

        let second = supply
            .register(path, body.as_bytes(), &preview.hash, Actor::Gui)
            .unwrap();
        assert!(second.idempotent);
        let catalog = supply.catalog().unwrap();
        assert_eq!(catalog[0].profile.path, path);
        assert!(
            !catalog[0]
                .profile
                .path
                .contains(root.path().to_string_lossy().as_ref())
        );

        let changed = body.replace(
            "display_name = \"neutral-profile\"",
            "display_name = \"Changed\"",
        );
        let changed_preview = supply.preview(path, changed.as_bytes()).unwrap();
        assert!(matches!(
            supply.register(path, changed.as_bytes(), &changed_preview.hash, Actor::Gui),
            Err(ProfileSupplyError::Conflict(_))
        ));
        assert_eq!(fs::read(root.path().join(path)).unwrap(), body.as_bytes());

        let parent = root.path().join("profiles/neutral-profile");
        let direct_target = parent.join("occupied.toml");
        fs::write(&direct_target, b"original").unwrap();
        let error = install_no_replace(&parent, &direct_target, b"replacement").unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&direct_target).unwrap(), b"original");
        assert!(fs::read_dir(&parent).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains("pending")
        }));
    }

    #[test]
    fn external_identity_collisions_and_stale_confirmation_fail_closed() {
        let root = root();
        let supply = ProfileSupplyRoot::open(root.path()).unwrap();
        let path = "profiles/neutral-profile/manifest.toml";
        let body = manifest("neutral-profile");
        let preview = supply.preview(path, body.as_bytes()).unwrap();
        assert!(matches!(
            supply.register(path, body.as_bytes(), "sha256:stale", Actor::Gui),
            Err(ProfileSupplyError::Conflict(_))
        ));
        supply
            .register(path, body.as_bytes(), &preview.hash, Actor::Gui)
            .unwrap();

        assert!(matches!(
            supply.preview(
                "profiles/nextjs/overlay.toml",
                overlay("neutral-profile").as_bytes()
            ),
            Err(ProfileSupplyError::Conflict(_))
        ));
    }

    #[test]
    fn profile_journal_records_success_and_failure_without_document_text() {
        let root = root();
        let supply = ProfileSupplyRoot::open(root.path()).unwrap();
        let path = "profiles/neutral-profile/manifest.toml";
        let body = manifest("neutral-profile");
        let preview = supply.preview(path, body.as_bytes()).unwrap();
        supply
            .register(path, body.as_bytes(), "sha256:stale", Actor::Gui)
            .unwrap_err();
        supply
            .register(path, body.as_bytes(), &preview.hash, Actor::Gui)
            .unwrap();

        let journal = fs::read_to_string(root.path().join("journal.jsonl")).unwrap();
        assert_eq!(journal.lines().count(), 2, "{journal}");
        assert!(journal.contains("\"action\":\"profile_register\""));
        assert!(journal.contains("\"result\":\"error\""));
        assert!(journal.contains("\"result\":\"ok\""));
        assert!(!journal.contains("display_name"), "{journal}");
        assert!(
            !journal.contains("Complete the requested work"),
            "{journal}"
        );
    }
}
