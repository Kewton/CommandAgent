use std::collections::BTreeSet;
use std::sync::OnceLock;

use crate::planner::profile_manifest::{ManifestStatus, ManifestV1};
use crate::planner::ultra_plan::{UltraPhase, UltraPlan};

const SOURCE: &str = include_str!("manifest.toml");
const PHASE_IDS: [&str; 3] = [
    "ingest-inspection",
    "ingest-implementation",
    "ingest-validation",
];
const CHECK_IDS: [&str; 5] = [
    "pipeline_probe",
    "ingest_source_binding",
    "ingest_candidate_accounting",
    "ingest_format_schema",
    "ingest_rerun_consistency",
];

pub fn get() -> &'static ManifestV1 {
    static MANIFEST: OnceLock<ManifestV1> = OnceLock::new();
    MANIFEST.get_or_init(|| {
        let manifest =
            ManifestV1::from_toml(SOURCE).expect("embedded ingest manifest must parse and resolve");
        validate(&manifest).expect("embedded ingest manifest must satisfy the fixed contract");
        manifest
    })
}

pub fn preset_ultra_plan(goal: &str, style: &str, intent: &str) -> Option<UltraPlan> {
    let manifest = get();
    if style != manifest.plan.style || intent != manifest.plan.intent {
        return None;
    }
    Some(UltraPlan {
        goal: goal.to_string(),
        profile: manifest.plan.profile.clone(),
        style: style.to_string(),
        intent: intent.to_string(),
        phases: manifest
            .plan
            .phases
            .iter()
            .map(|phase| UltraPhase {
                id: phase.id.clone(),
                prompt: phase.prompt.replace("{goal}", goal),
            })
            .collect(),
    })
}

pub fn required_artifacts() -> Vec<String> {
    get().artifacts.preferred_paths()
}

pub fn guidance() -> String {
    get()
        .guidance
        .variants
        .values()
        .flat_map(|variant| variant.messages.values())
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn required_capability_ids() -> Vec<String> {
    CHECK_IDS.into_iter().map(str::to_string).collect()
}

pub fn is_manifest_check_id(id: &str) -> bool {
    CHECK_IDS.contains(&id)
}

fn validate(manifest: &ManifestV1) -> Result<(), String> {
    if manifest.metadata.id != "ingest"
        || manifest.plan.profile != "ingest"
        || manifest.metadata.status != ManifestStatus::Draft
    {
        return Err("ingest identity must remain draft until admission review".to_string());
    }
    let phases = manifest
        .plan
        .phases
        .iter()
        .map(|phase| phase.id.as_str())
        .collect::<Vec<_>>();
    if phases != PHASE_IDS {
        return Err(format!("ingest create phases are not fixed: {phases:?}"));
    }
    let checks = manifest
        .checks
        .values()
        .flatten()
        .map(|check| check.id.as_str())
        .collect::<BTreeSet<_>>();
    if checks != BTreeSet::from(CHECK_IDS) {
        return Err(format!("ingest N1-N5 bindings are incomplete: {checks:?}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::capability_catalog::{
        InternalCapability, ProbeCapability, ResolvedCapability, ingest::IngestInternalCheck,
    };

    #[test]
    fn manifest_is_draft_and_binds_all_five_checks() {
        assert_eq!(get().metadata.status, ManifestStatus::Draft);
        assert_eq!(get().plan.phases.len(), 3);
        let resolved = get().resolve().unwrap();
        let capabilities = resolved
            .values()
            .flatten()
            .map(|check| &check.capability)
            .collect::<Vec<_>>();
        for check in [
            IngestInternalCheck::SourceBinding,
            IngestInternalCheck::CandidateAccounting,
            IngestInternalCheck::FormatSchema,
        ] {
            assert!(capabilities.contains(&&ResolvedCapability::Internal(
                InternalCapability::Ingest(check)
            )));
        }
        assert!(capabilities.iter().any(|capability| matches!(
            capability,
            ResolvedCapability::Probe(ProbeCapability::Pipeline { .. })
        )));
        assert!(capabilities.iter().any(|capability| matches!(
            capability,
            ResolvedCapability::Probe(ProbeCapability::DataRerunConsistency { .. })
        )));
    }
}
