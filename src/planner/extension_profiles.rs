//! Dynamic registration of externally supplied draft profiles.
//!
//! `PROFILE_DESCRIPTORS` stays the compiled-in registry. This module adds the
//! bounded, process-lifetime entry point for profiles that arrive as files
//! under an extension root: it decodes them, forces them to draft, and exposes
//! them as descriptors so the shared runtime, CLI, and GUI resolve them exactly
//! like any other profile — except that they can never be admitted.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};

use crate::planner::adjudication::contract::IntentId;
use crate::planner::profile::ProfileId;
use crate::planner::profile_descriptor::ProfileDescriptor;
use crate::planner::profile_manifest::ManifestStatus;
use crate::planner::profile_manifest::overlay::{LoadedOverlay, load_extension_overlays};
use crate::planner::profile_manifest::source::{
    ExtensionManifestError, LoadedManifest, MAX_EXTENSION_PROFILES, ManifestSource,
    load_extension_manifests,
};
use crate::planner::profiles::manifest_driven::ManifestDrivenProfile;
use crate::tui::boundary_shell::family_catalog::TaskFamilyId;

/// A registration replaces the previous one. The bound keeps the leaked,
/// process-lifetime registry finite even under repeated registration.
const MAX_REGISTRATIONS: usize = 512;

/// Serializes registration so a process never observes a half-installed
/// registry.
pub(crate) static REGISTRATION_LOCK: Mutex<()> = Mutex::new(());

static REGISTERED: RwLock<Option<&'static [ExtensionProfile]>> = RwLock::new(None);
static REGISTERED_ROOT: RwLock<Option<PathBuf>> = RwLock::new(None);
static REGISTRATION_COUNT: Mutex<usize> = Mutex::new(0);

/// One externally supplied profile, ready for display and for execution.
pub struct ExtensionProfile {
    pub descriptor: ProfileDescriptor,
    pub id: &'static str,
    pub display_label: &'static str,
    pub description: &'static str,
    pub source: ManifestSource,
    pub manifest_hash: &'static str,
    pub manifest_path: &'static str,
    pub task_family: TaskFamilyId,
    pub intent: IntentId,
    pub contract_checks: Vec<String>,
    pub base_profile: Option<&'static str>,
    pub warnings: Vec<String>,
}

impl ExtensionProfile {
    /// Externally supplied profiles are draft by construction.
    pub const fn status(&self) -> ManifestStatus {
        ManifestStatus::Draft
    }

    /// The ceiling the shared admission gate applies to this profile.
    pub const fn assurance_ceiling(&self) -> &'static str {
        "static"
    }

    pub fn profile_id(&self) -> ProfileId {
        self.descriptor.id.clone()
    }

    pub fn is_overlay(&self) -> bool {
        self.base_profile.is_some()
    }
}

fn draft_admission() -> ManifestStatus {
    ManifestStatus::Draft
}

/// Decode every profile declared by `extension_root` without registering it.
pub fn load(extension_root: &Path) -> Result<Vec<ExtensionProfile>, ExtensionManifestError> {
    let manifests = load_extension_manifests(extension_root)?;
    let overlays = load_extension_overlays(extension_root)?;
    if manifests.len() + overlays.len() > MAX_EXTENSION_PROFILES {
        return Err(ExtensionManifestError::TooMany {
            limit: MAX_EXTENSION_PROFILES,
        });
    }
    let mut profiles = Vec::with_capacity(manifests.len() + overlays.len());
    for manifest in manifests {
        profiles.push(standalone_profile(manifest));
    }
    for overlay in overlays {
        if profiles.iter().any(|existing| existing.id == overlay.id()) {
            return Err(ExtensionManifestError::Invalid {
                path: overlay.path.clone(),
                reason: format!(
                    "overlay id `{}` collides with another extension profile",
                    overlay.id()
                ),
            });
        }
        profiles.push(overlay_profile(overlay)?);
    }
    profiles.sort_by(|left, right| left.id.cmp(right.id));
    Ok(profiles)
}

/// Install `extension_root`'s profiles for the rest of the process.
///
/// Re-registering the same root is a no-op that returns the installed set.
pub fn register(
    extension_root: &Path,
) -> Result<&'static [ExtensionProfile], ExtensionManifestError> {
    let _guard = REGISTRATION_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    register_locked(extension_root)
}

pub(crate) fn register_locked(
    extension_root: &Path,
) -> Result<&'static [ExtensionProfile], ExtensionManifestError> {
    let root = extension_root.to_path_buf();
    if registered_root().as_deref() == Some(root.as_path()) {
        return Ok(registered());
    }
    let profiles = load(extension_root)?;
    let mut count = REGISTRATION_COUNT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if *count >= MAX_REGISTRATIONS {
        return Err(ExtensionManifestError::Root {
            path: root,
            reason: format!("at most {MAX_REGISTRATIONS} extension roots may be registered"),
        });
    }
    *count += 1;
    let installed: &'static [ExtensionProfile] = Box::leak(profiles.into_boxed_slice());
    *REGISTERED
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(installed);
    *REGISTERED_ROOT
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(root);
    Ok(installed)
}

pub fn registered() -> &'static [ExtensionProfile] {
    REGISTERED
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .unwrap_or(&[])
}

pub fn registered_root() -> Option<PathBuf> {
    REGISTERED_ROOT
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

pub fn find(id: &str) -> Option<&'static ExtensionProfile> {
    let normalized = id.trim().to_ascii_lowercase();
    registered().iter().find(|profile| profile.id == normalized)
}

pub fn descriptor_for_name(name: &str) -> Option<&'static ProfileDescriptor> {
    find(name).map(|profile| &profile.descriptor)
}

pub fn descriptor(id: &ProfileId) -> Option<&'static ProfileDescriptor> {
    registered()
        .iter()
        .find(|profile| &profile.descriptor.id == id)
        .map(|profile| &profile.descriptor)
}

/// Load and register an extension root, then return its dynamic descriptors.
pub fn extension_descriptors(
    extension_root: &Path,
) -> Result<Vec<&'static ProfileDescriptor>, ExtensionManifestError> {
    register(extension_root)
        .map(|profiles| profiles.iter().map(|profile| &profile.descriptor).collect())
}

fn standalone_profile(loaded: LoadedManifest) -> ExtensionProfile {
    let id: &'static str = leak(loaded.id().to_string());
    let display_label: &'static str = leak(format!("{}（下書き）", loaded.display_name()));
    let description: &'static str = leak(
        "外部 manifest から読み込んだ下書きプロファイル（未承認・帯域未計測・保証上限 static）。"
            .to_string(),
    );
    let manifest_hash: &'static str = leak(loaded.hash().unwrap_or_default().to_string());
    let manifest_path: &'static str = leak(loaded.contract_ref());
    let source = loaded.source();
    let task_family = loaded.task_family;
    let intent = loaded.intent;
    let contract_checks = loaded
        .manifest
        .checks
        .values()
        .flatten()
        .map(|check| check.id.clone())
        .collect();
    let warnings = loaded.warnings.clone();
    let manifest: &'static crate::planner::profile_manifest::ManifestV1 =
        Box::leak(Box::new(loaded.manifest));
    let runtime: &'static ManifestDrivenProfile =
        Box::leak(Box::new(ManifestDrivenProfile::standalone(id, manifest)));
    ExtensionProfile {
        descriptor: ProfileDescriptor {
            id: ProfileId::Other(id.to_string()),
            canonical: id,
            aliases: &[],
            display_name_ja: display_label,
            description_ja: description,
            admission: draft_admission,
            runtime,
            domain: runtime,
            contract_ref: Some(manifest_path),
            band_key: None,
            pack_profile: None,
        },
        id,
        display_label,
        description,
        source,
        manifest_hash,
        manifest_path,
        task_family,
        intent,
        contract_checks,
        base_profile: None,
        warnings,
    }
}

fn overlay_profile(loaded: LoadedOverlay) -> Result<ExtensionProfile, ExtensionManifestError> {
    let base_profile = loaded.base_profile;
    let Some(base) = crate::planner::profile_descriptor::PROFILE_DESCRIPTORS
        .iter()
        .find(|descriptor| descriptor.canonical == base_profile)
    else {
        return Err(ExtensionManifestError::Invalid {
            path: loaded.path.clone(),
            reason: format!("overlay base `{base_profile}` is not registered"),
        });
    };
    let Some(task_family) = crate::tui::boundary_shell::family_catalog::TASK_FAMILY_CATALOG
        .iter()
        .find(|family| family.profile == base_profile)
        .map(|family| family.id)
    else {
        return Err(ExtensionManifestError::Invalid {
            path: loaded.path.clone(),
            reason: format!("overlay base `{base_profile}` has no registered task family"),
        });
    };
    let intent = match loaded.base.plan.intent.as_str() {
        "fix" => IntentId::Fix,
        "investigate" => IntentId::Investigate,
        _ => IntentId::Create,
    };
    let id: &'static str = leak(loaded.id().to_string());
    let display_label: &'static str = leak(format!("{}（下書き上乗せ）", loaded.display_name()));
    let description: &'static str = leak(format!(
        "{base_profile} に追加専用の上乗せを適用した下書きプロファイル（未承認・帯域未計測・保証上限 static）。"
    ));
    let manifest_hash: &'static str = leak(loaded.hash.clone());
    let manifest_path: &'static str = leak(loaded.contract_ref());
    let source = loaded.source;
    let mut contract_checks = loaded
        .base
        .checks
        .values()
        .flatten()
        .map(|check| check.id.clone())
        .collect::<Vec<_>>();
    contract_checks.extend(loaded.added_check_ids());
    let overlay: &'static LoadedOverlay = Box::leak(Box::new(loaded));
    let runtime: &'static ManifestDrivenProfile =
        Box::leak(Box::new(ManifestDrivenProfile::overlay(id, base, overlay)));
    Ok(ExtensionProfile {
        descriptor: ProfileDescriptor {
            id: ProfileId::Other(id.to_string()),
            canonical: id,
            aliases: &[],
            display_name_ja: display_label,
            description_ja: description,
            admission: draft_admission,
            runtime,
            domain: runtime,
            contract_ref: Some(manifest_path),
            band_key: None,
            pack_profile: None,
        },
        id,
        display_label,
        description,
        source,
        manifest_hash,
        manifest_path,
        task_family,
        intent,
        contract_checks,
        base_profile: Some(base_profile),
        warnings: Vec::new(),
    })
}

fn leak(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}
