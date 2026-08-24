//! Read-only projection of one extension root for the '--extensions' action.
//!
//! Runtime loading stays fail-fast. This diagnostic leaf deliberately inspects
//! each established profile and pack entry independently so one bad entry does
//! not hide the remaining reasons an operator needs to fix.

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use serde::Serialize;
use serde_json::Value;

use crate::cli::Cli;
use crate::planner::pack::catalog::{PACK_PIN_FILE, PackStatus, status};
use crate::planner::pack::{self, PACKS_DIRECTORY};
use crate::planner::profile_manifest::commands;
use crate::planner::profile_manifest::source::{
    EXTENSION_MANIFEST_FILE, EXTENSION_OVERLAY_FILE, EXTENSION_PROFILES_DIRECTORY, exact_byte_hash,
};

const SCHEMA_VERSION: &str = "commandagent.extensions/v1";
const MAX_PROJECTED_MANIFEST_BYTES: u64 = 256 * 1024;
const MAX_PIN_BYTES: u64 = 128;
const MAX_JOURNAL_RECORD_BYTES: usize = 16 * 1024;

#[derive(Debug, Serialize)]
pub(crate) struct Inventory {
    schema_version: &'static str,
    extension_root: String,
    profile_catalog_error: Option<String>,
    profiles: Vec<ProfileRow>,
    packs: Vec<PackRow>,
    journal: JournalProjection,
}

#[derive(Debug, Serialize)]
struct ProfileRow {
    id: String,
    kind: &'static str,
    path: String,
    hash: Option<String>,
    status: &'static str,
    base_profile: Option<String>,
    usable: bool,
    reason: Option<String>,
}

#[derive(Debug, Serialize)]
struct PackRow {
    id: String,
    version: String,
    path: String,
    source: &'static str,
    status: PackStatus,
    pin: Option<String>,
    observed_hash: Option<String>,
    pin_matches_hash: bool,
    conformance: &'static str,
    profile: Option<String>,
    intent: Option<String>,
    usable: bool,
    reason: Option<String>,
}

#[derive(Debug, Serialize)]
struct JournalProjection {
    status: &'static str,
    latest: Option<Value>,
    reason: Option<String>,
}

pub(crate) fn run_if_requested(cli: &Cli) -> anyhow::Result<bool> {
    if !cli.extensions {
        return Ok(false);
    }
    let root = resolve_root(cli)?;
    let inventory = inspect(&root)?;
    if cli.json {
        println!("{}", serde_json::to_string_pretty(&inventory)?);
    } else {
        print!("{}", inventory.render_text());
    }
    Ok(true)
}

fn resolve_root(cli: &Cli) -> anyhow::Result<PathBuf> {
    let workspace_root = cli
        .cwd
        .clone()
        .unwrap_or(std::env::current_dir().context("failed to read current directory")?)
        .canonicalize()
        .context("failed to canonicalize workspace root")?;
    cli.extension_root
        .clone()
        .or(crate::config::configured_extension_root(&workspace_root)?)
        .context("--extensions requires --extension-root or a configured top-level extension_root")
}

fn inspect(root: &Path) -> anyhow::Result<Inventory> {
    let metadata = fs::symlink_metadata(root)
        .with_context(|| format!("inspect extension root '{}'", root.display()))?;
    if !metadata.file_type().is_dir() {
        bail!(
            "extension root '{}' must be an existing, non-symlink directory",
            root.display()
        );
    }
    let canonical = root
        .canonicalize()
        .with_context(|| format!("canonicalize extension root '{}'", root.display()))?;
    let profiles = inspect_profiles(&canonical)?;
    let profile_catalog_error = crate::planner::extension_profiles::register(&canonical)
        .err()
        .map(|error| error.to_string());
    let packs = inspect_packs(&canonical)?;
    let journal = inspect_journal(&canonical);
    Ok(Inventory {
        schema_version: SCHEMA_VERSION,
        extension_root: canonical.display().to_string(),
        profile_catalog_error,
        profiles,
        packs,
        journal,
    })
}

fn inspect_profiles(root: &Path) -> anyhow::Result<Vec<ProfileRow>> {
    let profiles_root = root.join(EXTENSION_PROFILES_DIRECTORY);
    let Some(entries) = optional_sorted_entries(&profiles_root)? else {
        return Ok(Vec::new());
    };
    let mut rows = Vec::new();
    for entry in entries {
        let directory = entry.path();
        let fallback_id = entry.file_name().to_string_lossy().into_owned();
        let file_type = entry
            .file_type()
            .with_context(|| format!("inspect profile entry '{}'", directory.display()))?;
        if !file_type.is_dir() {
            rows.push(ProfileRow {
                id: fallback_id,
                kind: "profile",
                path: relative_path(root, &directory),
                hash: None,
                status: "invalid",
                base_profile: None,
                usable: false,
                reason: Some("profile entry must be a non-symlink directory".to_string()),
            });
            continue;
        }
        let manifest = directory.join(EXTENSION_MANIFEST_FILE);
        let overlay = directory.join(EXTENSION_OVERLAY_FILE);
        let mut found = false;
        for (path, kind, logical_name) in [
            (&manifest, "profile", EXTENSION_MANIFEST_FILE),
            (&overlay, "overlay", EXTENSION_OVERLAY_FILE),
        ] {
            if fs::symlink_metadata(path).is_err() {
                continue;
            }
            found = true;
            rows.push(inspect_profile_file(
                root,
                path,
                &fallback_id,
                kind,
                logical_name,
            ));
        }
        if !found {
            rows.push(ProfileRow {
                id: fallback_id,
                kind: "profile",
                path: relative_path(root, &directory),
                hash: None,
                status: "invalid",
                base_profile: None,
                usable: false,
                reason: Some(
                    "profile directory contains neither manifest.toml nor overlay.toml".to_string(),
                ),
            });
        }
    }
    rows.sort_by(|left, right| {
        (&left.id, left.kind, &left.path).cmp(&(&right.id, right.kind, &right.path))
    });
    Ok(rows)
}

fn inspect_profile_file(
    root: &Path,
    path: &Path,
    fallback_id: &str,
    kind: &'static str,
    logical_name: &str,
) -> ProfileRow {
    let bytes = bounded_regular_file(path, MAX_PROJECTED_MANIFEST_BYTES).ok();
    let hash = bytes
        .as_deref()
        .map(|bytes| exact_byte_hash(logical_name, bytes));
    let decoded = bytes.as_deref().and_then(|bytes| {
        std::str::from_utf8(bytes)
            .ok()
            .and_then(|text| toml::from_str::<toml::Value>(text).ok())
    });
    let id = decoded
        .as_ref()
        .and_then(|value| value.get("metadata"))
        .and_then(|metadata| metadata.get("id"))
        .and_then(toml::Value::as_str)
        .unwrap_or(fallback_id)
        .to_string();
    let base_profile = (kind == "overlay")
        .then(|| {
            decoded
                .as_ref()
                .and_then(|value| value.get("overlay"))
                .and_then(|overlay| overlay.get("base_profile"))
                .and_then(toml::Value::as_str)
                .map(str::to_string)
        })
        .flatten();
    let validation = commands::validate_file(path);
    ProfileRow {
        id,
        kind,
        path: relative_path(root, path),
        hash,
        status: if validation.is_ok() {
            "draft"
        } else {
            "invalid"
        },
        base_profile,
        usable: validation.is_ok(),
        reason: validation.err().map(|error| error.to_string()),
    }
}

fn inspect_packs(root: &Path) -> anyhow::Result<Vec<PackRow>> {
    let packs_root = root.join(PACKS_DIRECTORY);
    let Some(id_entries) = optional_sorted_entries(&packs_root)? else {
        return Ok(Vec::new());
    };
    let mut rows = Vec::new();
    for id_entry in id_entries {
        let id = id_entry.file_name().to_string_lossy().into_owned();
        let id_path = id_entry.path();
        if !id_entry
            .file_type()
            .with_context(|| format!("inspect pack entry '{}'", id_path.display()))?
            .is_dir()
        {
            rows.push(invalid_pack_row(
                root,
                &id_path,
                id,
                String::new(),
                "pack id entry must be a non-symlink directory",
            ));
            continue;
        }
        for version_entry in sorted_entries(&id_path)? {
            let version = version_entry.file_name().to_string_lossy().into_owned();
            let directory = version_entry.path();
            if !version_entry
                .file_type()
                .with_context(|| format!("inspect pack version '{}'", directory.display()))?
                .is_dir()
            {
                rows.push(invalid_pack_row(
                    root,
                    &directory,
                    id.clone(),
                    version,
                    "pack version entry must be a non-symlink directory",
                ));
                continue;
            }
            rows.push(inspect_pack_directory(
                root,
                &directory,
                id.clone(),
                version,
            ));
        }
    }
    rows.sort_by(|left, right| (&left.id, &left.version).cmp(&(&right.id, &right.version)));
    Ok(rows)
}

fn inspect_pack_directory(root: &Path, directory: &Path, id: String, version: String) -> PackRow {
    let pack_status = status(directory);
    let (pin, pin_error) = inspect_pin(directory);
    let loaded = pack::load_directory(directory);
    let observed_hash = loaded.as_ref().ok().map(|pack| pack.hash.clone());
    let profile = loaded
        .as_ref()
        .ok()
        .map(|pack| pack.identity.profile.as_str().to_string());
    let intent = loaded
        .as_ref()
        .ok()
        .map(|pack| pack.identity.intent.as_str().to_string());
    let identity_matches = loaded
        .as_ref()
        .is_ok_and(|pack| pack.id() == id && pack.identity.version == version);
    let conformance_result = loaded
        .as_ref()
        .ok()
        .filter(|_| identity_matches)
        .map(pack::conform);
    let conformance = match &conformance_result {
        Some(Ok(_)) => "passed",
        Some(Err(_)) => "failed",
        None => "not_run",
    };
    let pin_matches_hash = pin
        .as_ref()
        .zip(observed_hash.as_ref())
        .is_some_and(|(pin, observed)| pin == observed);
    let reason = if let Err(error) = &loaded {
        Some(error.to_string())
    } else if !identity_matches {
        Some("directory name and pack identity disagree".to_string())
    } else if let Some(Err(error)) = &conformance_result {
        Some(error.to_string())
    } else if pack_status == PackStatus::Retired {
        Some("pack is retired and cannot be selected".to_string())
    } else if let Some(error) = pin_error {
        Some(error)
    } else if pin.is_none() {
        Some("pack is not pinned: pack.sha256 is missing".to_string())
    } else if !pin_matches_hash {
        Some("pack pin does not match the observed exact-byte hash".to_string())
    } else {
        None
    };
    PackRow {
        id,
        version,
        path: relative_path(root, directory),
        source: "local",
        status: pack_status,
        pin,
        observed_hash,
        pin_matches_hash,
        conformance,
        profile,
        intent,
        usable: reason.is_none(),
        reason,
    }
}

fn invalid_pack_row(
    root: &Path,
    path: &Path,
    id: String,
    version: String,
    reason: &str,
) -> PackRow {
    PackRow {
        id,
        version,
        path: relative_path(root, path),
        source: "local",
        status: PackStatus::Staged,
        pin: None,
        observed_hash: None,
        pin_matches_hash: false,
        conformance: "not_run",
        profile: None,
        intent: None,
        usable: false,
        reason: Some(reason.to_string()),
    }
}

fn inspect_pin(directory: &Path) -> (Option<String>, Option<String>) {
    let path = directory.join(PACK_PIN_FILE);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return (None, None),
        Err(error) => {
            return (
                None,
                Some(format!(
                    "failed to inspect pack pin '{}': {error}",
                    path.display()
                )),
            );
        }
    };
    if !metadata.file_type().is_file() {
        return (
            None,
            Some(format!(
                "pack pin '{}' must be a non-symlink regular file",
                path.display()
            )),
        );
    }
    if metadata.len() > MAX_PIN_BYTES {
        return (
            None,
            Some(format!(
                "pack pin '{}' exceeds {MAX_PIN_BYTES} bytes",
                path.display()
            )),
        );
    }
    match fs::read_to_string(&path) {
        Ok(value) if !value.trim().is_empty() => (Some(value.trim().to_string()), None),
        Ok(_) => (None, Some("pack.sha256 is empty".to_string())),
        Err(error) => (
            None,
            Some(format!(
                "failed to read pack pin '{}': {error}",
                path.display()
            )),
        ),
    }
}

fn inspect_journal(root: &Path) -> JournalProjection {
    let path = root.join(pack::supply::journal::JOURNAL_FILE);
    match latest_journal_value(&path) {
        Ok(Some(latest)) => JournalProjection {
            status: "present",
            latest: Some(latest),
            reason: None,
        },
        Ok(None) => JournalProjection {
            status: "absent",
            latest: None,
            reason: None,
        },
        Err(reason) => JournalProjection {
            status: "invalid",
            latest: None,
            reason: Some(reason),
        },
    }
}

fn latest_journal_value(path: &Path) -> Result<Option<Value>, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("inspect '{}': {error}", path.display())),
    };
    if !metadata.file_type().is_file() {
        return Err(format!(
            "journal '{}' must be a non-symlink regular file",
            path.display()
        ));
    }
    if metadata.len() == 0 {
        return Ok(None);
    }
    let offset = metadata
        .len()
        .saturating_sub(MAX_JOURNAL_RECORD_BYTES as u64);
    let mut file =
        File::open(path).map_err(|error| format!("open '{}': {error}", path.display()))?;
    file.seek(SeekFrom::Start(offset))
        .map_err(|error| format!("seek '{}': {error}", path.display()))?;
    let mut tail = Vec::new();
    file.read_to_end(&mut tail)
        .map_err(|error| format!("read '{}': {error}", path.display()))?;
    let mut end = tail.len();
    while end > 0 && matches!(tail[end - 1], b'\n' | b'\r') {
        end -= 1;
    }
    if end == 0 {
        return Ok(None);
    }
    let start = tail[..end]
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |index| index + 1);
    if offset > 0 && start == 0 {
        return Err(format!(
            "final journal record exceeds {MAX_JOURNAL_RECORD_BYTES} bytes"
        ));
    }
    let line = std::str::from_utf8(&tail[start..end])
        .map_err(|_| "final journal record is not valid UTF-8".to_string())?;
    serde_json::from_str(line)
        .map(Some)
        .map_err(|error| format!("final journal record is invalid JSON: {error}"))
}

fn bounded_regular_file(path: &Path, max_bytes: u64) -> anyhow::Result<Vec<u8>> {
    let metadata =
        fs::symlink_metadata(path).with_context(|| format!("inspect '{}'", path.display()))?;
    if !metadata.file_type().is_file() {
        bail!("'{}' must be a non-symlink regular file", path.display());
    }
    if metadata.len() > max_bytes {
        bail!("'{}' exceeds {max_bytes} bytes", path.display());
    }
    fs::read(path).with_context(|| format!("read '{}'", path.display()))
}

fn optional_sorted_entries(directory: &Path) -> anyhow::Result<Option<Vec<fs::DirEntry>>> {
    match fs::symlink_metadata(directory) {
        Ok(metadata) if metadata.file_type().is_dir() => sorted_entries(directory).map(Some),
        Ok(_) => bail!(
            "extension namespace '{}' must be a non-symlink directory",
            directory.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("inspect '{}'", directory.display())),
    }
}

fn sorted_entries(directory: &Path) -> anyhow::Result<Vec<fs::DirEntry>> {
    let mut entries = fs::read_dir(directory)
        .with_context(|| format!("read '{}'", directory.display()))?
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| format!("read entries under '{}'", directory.display()))?;
    entries.sort_by_key(fs::DirEntry::file_name);
    Ok(entries)
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn text_cell(value: impl AsRef<str>) -> String {
    value.as_ref().replace(['\t', '\n', '\r'], " ")
}

impl Inventory {
    fn render_text(&self) -> String {
        let mut lines = vec![format!("ROOT\t{}", text_cell(&self.extension_root))];
        if let Some(error) = &self.profile_catalog_error {
            lines.push(format!("PROFILE_CATALOG\tunusable\t{}", text_cell(error)));
        } else {
            lines.push("PROFILE_CATALOG\tusable\t-".to_string());
        }
        for row in &self.profiles {
            lines.push(format!(
                "PROFILE\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                text_cell(&row.id),
                row.kind,
                row.status,
                text_cell(row.base_profile.as_deref().unwrap_or("-")),
                text_cell(row.hash.as_deref().unwrap_or("-")),
                if row.usable { "usable" } else { "unusable" },
                text_cell(row.reason.as_deref().unwrap_or("-")),
            ));
        }
        for row in &self.packs {
            lines.push(format!(
                "PACK\t{}@{}\t{}\t{}\t{}\t{}×{}\t{}\t{}",
                text_cell(&row.id),
                text_cell(&row.version),
                row.source,
                row.status,
                row.conformance,
                text_cell(row.profile.as_deref().unwrap_or("-")),
                text_cell(row.intent.as_deref().unwrap_or("-")),
                if row.usable { "usable" } else { "unusable" },
                text_cell(row.reason.as_deref().unwrap_or("-")),
            ));
        }
        let journal_detail = self
            .journal
            .latest
            .as_ref()
            .map(Value::to_string)
            .or_else(|| self.journal.reason.clone())
            .unwrap_or_else(|| "-".to_string());
        lines.push(format!(
            "JOURNAL\t{}\t{}",
            self.journal.status,
            text_cell(journal_detail)
        ));
        format!("{}\n", lines.join("\n"))
    }
}
