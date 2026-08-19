use std::path::Path;

use crate::planner::profile_descriptor::{DATA_PROFILE_ID, PYTHON_CLI_PROFILE_ID};
use anyhow::{Context, bail};

use super::confirmation::PackSelection;

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

pub fn compatible(profile: &str, intent: &str) -> Vec<&'static AdmittedPack> {
    ADMITTED_PACKS
        .iter()
        .filter(|pack| pack.profile == profile && pack.intent == intent)
        .collect()
}

pub fn validate_selection(
    profile: &str,
    intent: &str,
    selection: &PackSelection,
) -> anyhow::Result<()> {
    let PackSelection::Pinned {
        id,
        version,
        hash,
        point,
    } = selection
    else {
        return Ok(());
    };
    if ADMITTED_PACKS.iter().any(|pack| {
        pack.id == id
            && pack.version == version
            && pack.profile == profile
            && pack.intent == intent
            && pack.hash == hash
            && pack.point == point
    }) {
        Ok(())
    } else {
        bail!("pack selection is not an admitted compatible exact-byte pin")
    }
}

pub fn observed_pin(
    repository_root: &Path,
    selection: &PackSelection,
) -> anyhow::Result<Option<String>> {
    let PackSelection::Pinned { id, version, .. } = selection else {
        return Ok(None);
    };
    let pack = ADMITTED_PACKS
        .iter()
        .find(|pack| pack.id == id && pack.version == version)
        .context("selected pack is absent from the admitted catalog")?;
    let directory = repository_root.join(pack.relative_directory);
    let loaded = crate::planner::pack::load_directory(&directory)
        .with_context(|| format!("load admitted pack {}", directory.display()))?;
    Ok(Some(loaded.hash))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_or_cross_profile_pack_is_not_selectable() {
        let wrong = PackSelection::Pinned {
            id: "cli-assist".to_string(),
            version: "1.1.0".to_string(),
            hash: ADMITTED_PACKS[1].hash.to_string(),
            point: "cli-validation".to_string(),
        };
        assert!(validate_selection("ingest", "create", &wrong).is_err());
        assert!(validate_selection(PYTHON_CLI_PROFILE_ID, "create", &wrong).is_ok());
    }

    #[test]
    fn catalog_hashes_match_the_exact_repository_bytes() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        for pack in ADMITTED_PACKS {
            let selection = PackSelection::Pinned {
                id: pack.id.to_string(),
                version: pack.version.to_string(),
                hash: pack.hash.to_string(),
                point: pack.point.to_string(),
            };
            assert_eq!(
                observed_pin(root, &selection).unwrap().as_deref(),
                Some(pack.hash)
            );
        }
    }
}
