//! Additive overlay manifests (Issue #105 decision, implemented by #117).
//!
//! An overlay adds artifact, guidance, check, and evidence-target obligations
//! to an admitted embedded base profile. It never replaces, relocates, removes,
//! or weakens a base obligation, and it never inherits the base's admission:
//! selecting an overlay produces a distinct draft effective profile whose
//! assurance stays capped at `static`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::source::{
    EXTENSION_OVERLAY_FILE, ExtensionManifestError, ManifestSource, exact_byte_hash,
    profile_directories, read_optional, reject_fixture_vocabulary, reject_registered_identity,
};
use super::{
    ArtifactRequirements, CheckBinding, EvidenceTargetsReference, ManifestGuidance, ManifestStatus,
    ManifestV1, SchemaVersion,
};

/// Root sections an overlay document may declare. `plan`, `step_templates`,
/// and `vocabulary` are forbidden and are rejected as unknown fields.
pub const OVERLAY_V1_SECTIONS: &[&str] = &[
    "metadata",
    "overlay",
    "artifacts",
    "guidance",
    "checks",
    "evidence_targets",
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OverlayV1 {
    pub metadata: OverlayMetadata,
    pub overlay: OverlayBinding,
    #[serde(default)]
    pub artifacts: Option<ArtifactRequirements>,
    #[serde(default)]
    pub guidance: Option<ManifestGuidance>,
    #[serde(default)]
    pub checks: Option<BTreeMap<String, Vec<CheckBinding>>>,
    #[serde(default)]
    pub evidence_targets: Option<EvidenceTargetsReference>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OverlayMetadata {
    pub id: String,
    pub display_name: String,
    pub schema_version: SchemaVersion,
    pub status: ManifestStatus,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OverlayBinding {
    pub base_profile: String,
    pub mode: OverlayMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverlayMode {
    Additive,
}

impl OverlayMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Additive => "additive",
        }
    }
}

/// One decoded overlay bound to its admitted embedded base.
#[derive(Debug)]
pub struct LoadedOverlay {
    pub overlay: OverlayV1,
    pub base: &'static ManifestV1,
    pub base_profile: &'static str,
    pub path: PathBuf,
    pub hash: String,
    pub source: ManifestSource,
}

impl LoadedOverlay {
    pub fn id(&self) -> &str {
        &self.overlay.metadata.id
    }

    pub fn display_name(&self) -> &str {
        &self.overlay.metadata.display_name
    }

    /// The effective profile is always a draft, regardless of the base.
    pub const fn status(&self) -> ManifestStatus {
        ManifestStatus::Draft
    }

    pub fn contract_ref(&self) -> String {
        self.path.display().to_string()
    }

    /// Paths the overlay adds on top of the base contract.
    pub fn added_artifacts(&self) -> Vec<String> {
        self.overlay
            .artifacts
            .as_ref()
            .map(ArtifactRequirements::preferred_paths)
            .unwrap_or_default()
    }

    /// Guidance the overlay always injects.
    pub fn always_guidance(&self) -> Vec<String> {
        let Some(guidance) = self.overlay.guidance.as_ref() else {
            return Vec::new();
        };
        guidance
            .variants
            .values()
            .filter(|variant| {
                variant
                    .triggers
                    .iter()
                    .any(|trigger| trigger.condition == super::GuidanceTriggerCondition::Always)
            })
            .flat_map(|variant| variant.messages.values().cloned())
            .collect()
    }

    /// Capability ids the overlay adds, in binding order.
    pub fn added_check_ids(&self) -> Vec<String> {
        self.overlay
            .checks
            .as_ref()
            .map(|checks| {
                checks
                    .values()
                    .flatten()
                    .map(|check| check.id.clone())
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Read every `profiles/<dir>/overlay.toml` under `extension_root`.
///
/// `<dir>` names the admitted base profile. The effective overlay id lives in
/// metadata and remains a separate draft identity.
pub fn load_extension_overlays(
    extension_root: &Path,
) -> Result<Vec<LoadedOverlay>, ExtensionManifestError> {
    let mut loaded: Vec<LoadedOverlay> = Vec::new();
    for directory in profile_directories(extension_root)? {
        let path = directory.join(EXTENSION_OVERLAY_FILE);
        let Some(bytes) = read_optional(&path)? else {
            continue;
        };
        let entry = decode(&directory, &path, &bytes, ManifestSource::Local)?;
        if loaded.iter().any(|existing| existing.id() == entry.id()) {
            return Err(ExtensionManifestError::Invalid {
                path,
                reason: format!(
                    "duplicate overlay id `{}` in this extension root",
                    entry.id()
                ),
            });
        }
        loaded.push(entry);
    }
    Ok(loaded)
}

/// Admitted embedded profiles that may act as an overlay base.
fn embedded_base(profile: &str) -> Option<(&'static str, &'static ManifestV1)> {
    let descriptor = crate::planner::profile_descriptor::PROFILE_DESCRIPTORS
        .iter()
        .find(|descriptor| descriptor.canonical == profile)?;
    if (descriptor.admission)() != ManifestStatus::Admitted {
        return None;
    }
    let manifest = match descriptor.canonical {
        crate::planner::profile_descriptor::NEXTJS_PROFILE_ID => super::nextjs_manifest(),
        crate::planner::profile_descriptor::PYTHON_CLI_PROFILE_ID => {
            crate::planner::profiles::python_cli::manifest::get()
        }
        crate::planner::profile_descriptor::DATA_PROFILE_ID => {
            crate::planner::profiles::data::manifest::get()
        }
        crate::planner::profile_descriptor::INGEST_PROFILE_ID => {
            crate::planner::profiles::ingest::manifest::get()
        }
        _ => return None,
    };
    Some((descriptor.canonical, manifest))
}

fn decode(
    directory: &Path,
    path: &Path,
    bytes: &[u8],
    source: ManifestSource,
) -> Result<LoadedOverlay, ExtensionManifestError> {
    let invalid = |reason: String| ExtensionManifestError::Invalid {
        path: path.to_path_buf(),
        reason,
    };
    let text = std::str::from_utf8(bytes).map_err(|_| ExtensionManifestError::NotUtf8 {
        path: path.to_path_buf(),
    })?;
    let overlay = toml::from_str::<OverlayV1>(text)
        .map_err(|error| invalid(format!("overlay TOML is invalid: {error}")))?;
    if !source.is_external() {
        return Err(invalid(
            "an overlay must be supplied by the repository or an extension root".to_string(),
        ));
    }
    if overlay.metadata.status != ManifestStatus::Draft {
        return Err(invalid(
            "metadata.status must be exactly `draft` for an overlay".to_string(),
        ));
    }
    if overlay.metadata.display_name.trim().is_empty() {
        return Err(invalid(
            "metadata.display_name must not be empty".to_string(),
        ));
    }
    reject_registered_identity(&overlay.metadata.id).map_err(&invalid)?;
    reject_fixture_vocabulary(text).map_err(&invalid)?;

    let directory_name = directory
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if directory_name != overlay.overlay.base_profile {
        return Err(invalid(format!(
            "directory `{directory_name}` must name the admitted base profile `{}`",
            overlay.overlay.base_profile
        )));
    }
    let Some((base_profile, base)) = embedded_base(&overlay.overlay.base_profile) else {
        return Err(invalid(format!(
            "overlay.base_profile `{}` must be the canonical id of an admitted, manifest-backed embedded profile",
            overlay.overlay.base_profile
        )));
    };
    validate_additions(&overlay, base).map_err(&invalid)?;
    Ok(LoadedOverlay {
        overlay,
        base,
        base_profile,
        path: path.to_path_buf(),
        hash: exact_byte_hash(EXTENSION_OVERLAY_FILE, bytes),
        source,
    })
}

fn validate_additions(overlay: &OverlayV1, base: &ManifestV1) -> Result<(), String> {
    let mut additions = 0usize;

    if let Some(artifacts) = overlay.artifacts.as_ref() {
        let base_paths = base
            .artifacts
            .required
            .iter()
            .chain(base.artifacts.groups.iter().flat_map(|group| &group.paths))
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let base_group_ids = base
            .artifacts
            .groups
            .iter()
            .map(|group| group.id.as_str())
            .collect::<BTreeSet<_>>();
        let mut declared = BTreeSet::new();
        let mut declared_group_ids = BTreeSet::new();
        for path in &artifacts.required {
            validate_path(path)?;
            reject_collision(&base_paths, path, "artifact path")?;
            if !declared.insert(path.as_str()) {
                return Err(format!("duplicate artifact path `{path}`"));
            }
            additions += 1;
        }
        for group in &artifacts.groups {
            if group.id.trim().is_empty() {
                return Err("artifacts.groups[].id must not be empty".to_string());
            }
            reject_collision(&base_group_ids, &group.id, "artifact group id")?;
            if !declared_group_ids.insert(group.id.as_str()) {
                return Err(format!("duplicate artifact group id `{}`", group.id));
            }
            if group.paths.len() < 2 {
                return Err(format!(
                    "artifact group `{}` must contain at least two paths",
                    group.id
                ));
            }
            for path in &group.paths {
                validate_path(path)?;
                reject_collision(&base_paths, path, "artifact path")?;
                if !declared.insert(path.as_str()) {
                    return Err(format!("duplicate artifact path `{path}`"));
                }
            }
            if !group.paths.contains(&group.preferred) {
                return Err(format!(
                    "artifact group `{}` preferred path must be a member of the group",
                    group.id
                ));
            }
            additions += 1;
        }
    }

    if let Some(guidance) = overlay.guidance.as_ref() {
        if guidance.variants.is_empty() {
            return Err("guidance.variants must not be empty when declared".to_string());
        }
        for (name, variant) in &guidance.variants {
            if name.trim().is_empty() {
                return Err("guidance variant names must not be empty".to_string());
            }
            if base.guidance.variants.contains_key(name) {
                return Err(format!(
                    "guidance variant `{name}` already exists in the base profile; an overlay may not replace it"
                ));
            }
            if variant.triggers.is_empty() || variant.messages.is_empty() {
                return Err(format!(
                    "guidance variant `{name}` needs at least one trigger and one message"
                ));
            }
            for trigger in &variant.triggers {
                let expects_values = trigger.condition != super::GuidanceTriggerCondition::Always;
                if expects_values == trigger.values.is_empty() {
                    return Err(format!(
                        "guidance variant `{name}` trigger values must be empty only for `always`"
                    ));
                }
            }
            for (message_name, text) in &variant.messages {
                if message_name.trim().is_empty() {
                    return Err(format!(
                        "guidance variant `{name}` has an empty message name"
                    ));
                }
                if text.trim().is_empty() {
                    return Err(format!("guidance variant `{name}` has an empty message"));
                }
            }
            additions += 1;
        }
    }

    let mut overlay_check_ids = BTreeSet::new();
    if let Some(checks) = overlay.checks.as_ref() {
        let base_ids = base
            .checks
            .values()
            .flatten()
            .map(|check| check.id.as_str())
            .collect::<BTreeSet<_>>();
        let base_phases = base
            .plan
            .phases
            .iter()
            .map(|phase| phase.id.as_str())
            .collect::<BTreeSet<_>>();
        for (binding, entries) in checks {
            if binding.trim().is_empty() {
                return Err("checks binding names may not be empty".to_string());
            }
            if base.checks.contains_key(binding) {
                return Err(format!(
                    "check binding `{binding}` already exists in the base profile; an overlay may not replace it"
                ));
            }
            if entries.is_empty() {
                return Err(format!("check binding `{binding}` must declare a check"));
            }
            for check in entries {
                reject_collision(&base_ids, &check.id, "check capability id")?;
                if !overlay_check_ids.insert(check.id.clone()) {
                    return Err(format!("duplicate overlay check capability `{}`", check.id));
                }
                crate::planner::capability_catalog::resolve(&check.id, &check.params)
                    .map_err(|error| format!("check `{}` is invalid: {error}", check.id))?;
                if let Some(phases) = check.phases.as_ref() {
                    if phases.is_empty() {
                        return Err(format!(
                            "check `{}` declares an empty phase list; omit `phases` for final acceptance",
                            check.id
                        ));
                    }
                    for phase in phases {
                        if !base_phases.contains(phase.as_str()) {
                            return Err(format!(
                                "check `{}` names phase `{phase}`, which the base profile does not declare",
                                check.id
                            ));
                        }
                    }
                }
                additions += 1;
            }
        }
    }

    if let Some(targets) = overlay.evidence_targets.as_ref() {
        if targets.source.is_some() || targets.section.is_some() {
            return Err(
                "evidence_targets in an overlay must be a local mapping, not a shared reference"
                    .to_string(),
            );
        }
        if targets.mappings.is_empty() {
            return Err("evidence_targets.mappings must not be empty when declared".to_string());
        }
        for (evidence, paths) in &targets.mappings {
            if !overlay_check_ids.contains(evidence) {
                return Err(format!(
                    "evidence_targets mapping `{evidence}` must name a check added by this overlay"
                ));
            }
            if paths.is_empty() {
                return Err(format!("evidence_targets mapping `{evidence}` has no path"));
            }
            for path in paths {
                validate_path(path)?;
            }
        }
    }
    if !overlay_check_ids.is_empty() {
        let mapped = overlay
            .evidence_targets
            .as_ref()
            .map(|targets| targets.mappings.keys().cloned().collect::<BTreeSet<_>>())
            .unwrap_or_default();
        let missing = overlay_check_ids
            .difference(&mapped)
            .cloned()
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(format!(
                "evidence_targets must map every added check; missing: {}",
                missing.join(", ")
            ));
        }
    }

    if additions == 0 {
        return Err(
            "an overlay must add at least one artifact, guidance variant, or check".to_string(),
        );
    }
    Ok(())
}

fn validate_path(path: &str) -> Result<(), String> {
    crate::tools::path_guard::validate_workspace_relative(path).map_err(|error| error.to_string())
}

fn reject_collision(existing: &BTreeSet<&str>, value: &str, kind: &str) -> Result<(), String> {
    if existing.contains(value) {
        return Err(format!(
            "{kind} `{value}` already belongs to the base profile; v1 rejects replacement instead of proving it is stricter"
        ));
    }
    Ok(())
}
