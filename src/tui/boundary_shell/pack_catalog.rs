use anyhow::{Context, bail};

use crate::planner::pack::PackProfile;
use crate::planner::pack::catalog::{self, AdmittedPack, PackLocator, PackSource};

use super::confirmation::PackSelection;

pub fn compatible(profile: &str, intent: &str) -> Vec<&'static AdmittedPack> {
    catalog::compatible(profile, intent).collect()
}

pub fn select(profile: &str, intent: &str, selector: &str) -> anyhow::Result<PackSelection> {
    let Some((id, version)) = selector.trim().split_once('@') else {
        bail!("pack selector `{selector}` must pin id@MAJOR.MINOR.PATCH")
    };
    let pack = catalog::compatible(profile, intent)
        .find(|pack| pack.id == id && pack.version == version)
        .ok_or_else(|| {
            anyhow::anyhow!("pack `{selector}` is not an admitted pack for {profile} × {intent}")
        })?;
    Ok(PackSelection::Pinned {
        id: pack.id.to_string(),
        version: pack.version.to_string(),
        hash: pack.hash.to_string(),
        point: pack.point.to_string(),
        source: PackSource::Admitted,
    })
}

/// Resolve an admitted, repository, or local selector through the configured
/// roots. The returned selection freezes the source and exact hash that were
/// verified before Gate 1.
pub fn select_with_locator(
    profile: &str,
    intent: &str,
    selector: &str,
    locator: &PackLocator,
) -> anyhow::Result<PackSelection> {
    let Some((id, version)) = selector.trim().split_once('@') else {
        bail!("pack selector `{selector}` must pin id@MAJOR.MINOR.PATCH")
    };
    if id.is_empty() || version.is_empty() || version.contains('@') {
        bail!("pack selector `{selector}` must pin one id@MAJOR.MINOR.PATCH")
    }
    let located = locator.locate_pinned(id, version, None)?;
    let located_profile = PackProfile::parse(&located.profile)
        .context("selected pack profile is no longer registered")?;
    if !catalog::profile_is_compatible(located.source, profile, located_profile)
        || located.intent != intent
    {
        bail!(
            "pack `{selector}` is for {} × {}, not {profile} × {intent}",
            located.profile,
            located.intent
        )
    }
    let point = located
        .point
        .context("selected pack has no registered injection point")?;
    Ok(PackSelection::Pinned {
        id: located.id,
        version: located.version,
        hash: located.hash,
        point,
        source: located.source,
    })
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
        source,
    } = selection
    else {
        return Ok(());
    };
    if catalog::is_admitted(*source, profile, intent, id, version, hash, point) {
        Ok(())
    } else {
        bail!("pack selection is not an admitted compatible exact-byte pin")
    }
}

/// Validate every supply source against the exact bytes visible through the
/// configured locator. The admitted-only validator remains the default for
/// call sites that have no operator root and therefore cannot honestly admit
/// repository or local bytes.
pub fn validate_selection_with_locator(
    profile: &str,
    intent: &str,
    selection: &PackSelection,
    locator: &PackLocator,
) -> anyhow::Result<()> {
    let PackSelection::Pinned {
        id,
        version,
        hash,
        point,
        source,
    } = selection
    else {
        return Ok(());
    };
    if *source == PackSource::Admitted {
        return validate_selection(profile, intent, selection);
    }
    let located = locator.locate_pinned_from(*source, id, version, Some(hash))?;
    if located.source != *source {
        bail!(
            "pack `{id}@{version}` resolved as {}, not confirmed source {source}",
            located.source
        )
    }
    let located_profile = PackProfile::parse(&located.profile)
        .context("selected pack profile is no longer registered")?;
    if !catalog::profile_is_compatible(located.source, profile, located_profile)
        || located.intent != intent
    {
        bail!(
            "pack `{id}@{version}` is for {} × {}, not {profile} × {intent}",
            located.profile,
            located.intent
        )
    }
    if located.point.as_deref() != Some(point) {
        bail!("pack `{id}@{version}` injection point changed before confirmation")
    }
    Ok(())
}

pub fn observed_pin(
    locator: &PackLocator,
    selection: &PackSelection,
) -> anyhow::Result<Option<String>> {
    let PackSelection::Pinned {
        id,
        version,
        source,
        ..
    } = selection
    else {
        return Ok(None);
    };
    locator.observed_hash(*source, id, version).map(Some)
}

#[cfg(test)]
mod tests {
    use crate::planner::pack::catalog::{ADMITTED_PACKS, PackSource};
    use crate::planner::profile_descriptor::PYTHON_CLI_PROFILE_ID;

    use super::*;

    #[test]
    fn unknown_cross_profile_or_unapproved_pack_is_not_selectable() {
        let wrong = PackSelection::Pinned {
            id: "cli-assist".to_string(),
            version: "1.1.0".to_string(),
            hash: ADMITTED_PACKS[1].hash.to_string(),
            point: "cli-validation".to_string(),
            source: PackSource::Admitted,
        };
        assert!(validate_selection("ingest", "create", &wrong).is_err());
        assert!(validate_selection(PYTHON_CLI_PROFILE_ID, "create", &wrong).is_ok());

        let unapproved = PackSelection::Pinned {
            id: "cli-assist".to_string(),
            version: "1.1.0".to_string(),
            hash: ADMITTED_PACKS[1].hash.to_string(),
            point: "cli-validation".to_string(),
            source: PackSource::Repository,
        };
        assert!(validate_selection(PYTHON_CLI_PROFILE_ID, "create", &unapproved).is_err());
    }

    #[test]
    fn selector_resolves_only_an_exact_compatible_admitted_pack() {
        assert_eq!(
            select(PYTHON_CLI_PROFILE_ID, "create", "cli-assist@1.1.0").unwrap(),
            PackSelection::Pinned {
                id: "cli-assist".to_string(),
                version: "1.1.0".to_string(),
                hash: ADMITTED_PACKS[1].hash.to_string(),
                point: "cli-validation".to_string(),
                source: PackSource::Admitted,
            }
        );
        assert!(select(PYTHON_CLI_PROFILE_ID, "fix", "cli-assist@1.1.0").is_err());
        assert!(select(PYTHON_CLI_PROFILE_ID, "create", "cli-assist@1.1").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn locator_selects_an_exact_local_pin_and_rejects_it_after_retirement() {
        use std::os::unix::fs::PermissionsExt;

        use crate::planner::pack::{Actor, StagedFile, SupplyRoot};

        let extension = tempfile::tempdir().unwrap();
        std::fs::set_permissions(extension.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let supply = SupplyRoot::open(extension.path()).unwrap();
        let assist = std::fs::read_to_string("packs/cli-assist/1.0.0/assist.yaml")
            .unwrap()
            .replace("id: cli-assist", "id: local-supply");
        let staged = supply
            .stage(
                "local-supply",
                "1.0.0",
                &[StagedFile {
                    name: "assist.yaml".to_string(),
                    bytes: assist.into_bytes(),
                }],
                Actor::Gui,
            )
            .unwrap();
        supply
            .pin("local-supply", "1.0.0", &staged.hash, Actor::Gui)
            .unwrap();
        let locator = PackLocator::with_extension_root(
            env!("CARGO_MANIFEST_DIR"),
            Some(extension.path().to_path_buf()),
        );
        let selection = select_with_locator(
            PYTHON_CLI_PROFILE_ID,
            "create",
            "local-supply@1.0.0",
            &locator,
        )
        .unwrap();
        assert!(matches!(
            selection,
            PackSelection::Pinned {
                source: PackSource::Local,
                ..
            }
        ));
        validate_selection_with_locator(PYTHON_CLI_PROFILE_ID, "create", &selection, &locator)
            .unwrap();

        supply.retire("local-supply", "1.0.0", Actor::Gui).unwrap();
        assert!(
            select_with_locator(
                PYTHON_CLI_PROFILE_ID,
                "create",
                "local-supply@1.0.0",
                &locator,
            )
            .is_err()
        );
    }
}
