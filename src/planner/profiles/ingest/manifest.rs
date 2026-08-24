use std::collections::BTreeSet;
use std::sync::OnceLock;

use crate::planner::profile_manifest::{ManifestStatus, ManifestV1};
use crate::planner::ultra_plan::{UltraPhase, UltraPlan};

const SOURCE: &str = include_str!("manifest.toml");
pub(crate) const PHASE_IDS: [&str; 3] =
    ["ingest-implement", "ingest-run", "ingest-structural-gate"];
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
        || manifest.metadata.status != ManifestStatus::Admitted
    {
        return Err("ingest identity must be admitted after E-4d".to_string());
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
    let implementation_prompt = &manifest.plan.phases[0].prompt;
    let guidance_text = manifest
        .guidance
        .variants
        .values()
        .flat_map(|variant| variant.messages.values())
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    for marker in [
        crate::planner::profiles::ingest::guidance::SELECTOR_LITERAL,
        crate::planner::profiles::ingest::guidance::INSPECTION_LITERAL,
        crate::planner::profiles::ingest::guidance::RECORDS_LITERAL,
        "examples only",
        "actual snapshots",
        "never copy example values as fixed data",
    ] {
        if !implementation_prompt.contains(marker) {
            return Err(format!(
                "ingest implementation prompt lacks canonical literal guidance: {marker}"
            ));
        }
    }
    for marker in [
        "only the model-authored files pipeline/main.py and output/inspection.json",
        "following run phase executes it",
        "do not hand-author those runtime outputs",
    ] {
        if !implementation_prompt.contains(marker) {
            return Err(format!(
                "ingest implementation prompt crosses the run-output ownership boundary: {marker}"
            ));
        }
    }
    let run_prompt = &manifest.plan.phases[1].prompt;
    for marker in [
        "python3 -B pipeline/main.py",
        "generated output/records.json and output/report.md",
    ] {
        if !run_prompt.contains(marker) {
            return Err(format!(
                "ingest run prompt lacks ordered execution postcondition: {marker}"
            ));
        }
    }
    for kind in crate::planner::profiles::ingest::guidance::SELECTOR_KINDS {
        if !implementation_prompt.contains(kind) || !guidance_text.contains(kind) {
            return Err(format!(
                "ingest selector vocabulary is not published before validation: {kind}"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::capability_catalog::{
        InternalCapability, ProbeCapability, ResolvedCapability, ingest::IngestInternalCheck,
    };
    use crate::planner::profiles::ingest::{accounting::CandidateSelector, guidance};

    #[test]
    fn manifest_is_admitted_and_binds_all_five_checks() {
        assert_eq!(get().metadata.status, ManifestStatus::Admitted);
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

    #[test]
    fn preset_and_repair_guidance_publish_every_canonical_literal_before_the_gate() {
        let preset = preset_ultra_plan("Extract actual events.", "default", "create").unwrap();
        let implementation = &preset.phases[0].prompt;
        let run = &preset.phases[1].prompt;
        let repair_guidance = guidance();
        for marker in [
            guidance::SELECTOR_LITERAL,
            guidance::INSPECTION_LITERAL,
            guidance::RECORDS_LITERAL,
            "css, html_tag, and line_prefix",
            "pipeline/main.py",
            "output/report.md",
            "examples only",
            "actual snapshots",
            "never copy example values as fixed data",
        ] {
            assert!(implementation.contains(marker), "preset lacks {marker}");
            assert!(
                guidance::GENERATION_RULES.contains(marker),
                "synthesis guidance lacks {marker}"
            );
        }
        for marker in [
            guidance::SELECTOR_LITERAL,
            "candidate_accounting.accepted",
            "candidate_accounting.excluded",
            "record_format.fields",
            "output/records.json",
            "examples only",
            "actual observed snapshots",
        ] {
            assert!(repair_guidance.contains(marker), "repair lacks {marker}");
        }
        for marker in [
            "only the model-authored files pipeline/main.py and output/inspection.json",
            "following run phase executes it",
            "do not hand-author those runtime outputs",
        ] {
            assert!(implementation.contains(marker), "implement lacks {marker}");
        }
        for marker in [
            "python3 -B pipeline/main.py",
            "generated output/records.json and output/report.md",
        ] {
            assert!(run.contains(marker), "run lacks {marker}");
        }
    }

    #[test]
    fn published_selector_vocabulary_is_exactly_deserializable() {
        for kind in guidance::SELECTOR_KINDS {
            let selector = serde_json::from_value::<CandidateSelector>(serde_json::json!({
                "kind": kind,
                "value": if kind == "css" { "ul.events > li" } else { "article" }
            }))
            .unwrap();
            assert!(!selector.value.is_empty());
        }
        assert!(
            serde_json::from_value::<CandidateSelector>(serde_json::json!({
                "kind": "xpath",
                "value": "//article"
            }))
            .is_err()
        );
    }
}
