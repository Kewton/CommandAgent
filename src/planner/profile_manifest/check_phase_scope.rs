use std::collections::BTreeSet;

use super::{ManifestError, ManifestV0};

pub(super) fn validate(manifest: &ManifestV0) -> Result<(), ManifestError> {
    let declared = manifest
        .plan
        .phases
        .iter()
        .map(|phase| phase.id.as_str())
        .collect::<BTreeSet<_>>();
    for check in manifest.checks.values().flatten() {
        let Some(phases) = &check.phases else {
            continue;
        };
        if phases.is_empty() {
            return Err(invalid("must contain at least one phase id"));
        }
        let mut unique = BTreeSet::new();
        for phase in phases {
            if phase.trim().is_empty() {
                return Err(invalid("phase ids must not be empty"));
            }
            if !declared.contains(phase.as_str()) {
                return Err(invalid(format!("unknown phase id `{phase}`")));
            }
            if !unique.insert(phase.as_str()) {
                return Err(invalid(format!("duplicate phase id `{phase}`")));
            }
        }
    }
    Ok(())
}

pub(crate) fn check_ids_for_phase<'a>(manifest: &'a ManifestV0, phase_id: &str) -> Vec<&'a str> {
    let final_phase = manifest
        .plan
        .phases
        .last()
        .is_some_and(|phase| phase.id == phase_id);
    manifest
        .checks
        .values()
        .flatten()
        .filter(|check| {
            final_phase
                || check
                    .phases
                    .as_ref()
                    .is_some_and(|phases| phases.iter().any(|phase| phase == phase_id))
        })
        .map(|check| check.id.as_str())
        .collect()
}

fn invalid(reason: impl Into<String>) -> ManifestError {
    ManifestError::Invalid {
        field: "checks.<binding>[].phases",
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::profiles::data::manifest;

    #[test]
    fn omitted_scope_is_final_only_and_final_phase_collects_every_check() {
        let nextjs = crate::planner::profile_manifest::nextjs_manifest();
        assert!(check_ids_for_phase(nextjs, "project-setup").is_empty());
        assert_eq!(check_ids_for_phase(nextjs, "build-verification").len(), 7);

        let data = manifest::get();
        assert!(check_ids_for_phase(data, "data-inspection").is_empty());
        assert_eq!(check_ids_for_phase(data, "data-validation").len(), 5);
    }

    #[test]
    fn declared_scope_must_reference_a_known_phase() {
        let source = include_str!("../profiles/nextjs/manifest.toml").replacen(
            "id = \"package_json_port_script\"",
            "id = \"package_json_port_script\"\nphases = [\"unknown-phase\"]",
            1,
        );
        assert!(matches!(
            crate::planner::profile_manifest::ManifestV0::from_toml(&source),
            Err(ManifestError::Invalid {
                field: "checks.<binding>[].phases",
                ..
            })
        ));
    }
}
