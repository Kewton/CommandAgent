use std::sync::OnceLock;

use crate::planner::profile_manifest::{ManifestStatus, ManifestV1};
use crate::planner::ultra_plan::{UltraPhase, UltraPlan};

const SOURCE: &str = include_str!("manifest.toml");
const PHASE_IDS: [&str; 3] = ["cli-scaffold", "cli-implementation", "cli-validation"];
const CHECK_IDS: [&str; 4] = [
    "cli_probe",
    "help_binding",
    "cli_output_claims",
    "cli_rerun_consistency",
];

pub fn get() -> &'static ManifestV1 {
    static MANIFEST: OnceLock<ManifestV1> = OnceLock::new();
    MANIFEST.get_or_init(|| {
        let manifest =
            ManifestV1::from_toml(SOURCE).expect("embedded CLI manifest must parse and resolve");
        validate(&manifest).expect("embedded CLI manifest must satisfy the fixed contract");
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

fn validate(manifest: &ManifestV1) -> Result<(), String> {
    if manifest.metadata.id != "python-cli"
        || manifest.plan.profile != "python-cli"
        || manifest.metadata.status != ManifestStatus::Draft
    {
        return Err("CLI identity must be python-cli with draft admission".to_string());
    }
    let phases = manifest
        .plan
        .phases
        .iter()
        .map(|phase| phase.id.as_str())
        .collect::<Vec<_>>();
    if phases != PHASE_IDS {
        return Err(format!("CLI create phases are not fixed: {phases:?}"));
    }
    let checks = manifest
        .checks
        .values()
        .flatten()
        .map(|check| check.id.as_str())
        .collect::<Vec<_>>();
    if checks.len() != CHECK_IDS.len() || CHECK_IDS.iter().any(|id| !checks.contains(id)) {
        return Err(format!("CLI C1-C4 bindings are incomplete: {checks:?}"));
    }
    Ok(())
}
