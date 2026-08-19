use anyhow::bail;

use crate::planner::pack::catalog::{self, AdmittedPack, PackLocator};

use super::confirmation::PackSelection;

pub fn compatible(profile: &str, intent: &str) -> Vec<&'static AdmittedPack> {
    catalog::compatible(profile, intent).collect()
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
}
