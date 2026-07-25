use std::path::Path;

use crate::planner::profile::{ProfileBehaviorProbeReport, canonical_profile_name, domain_profile};
use crate::planner::profiles::python_cli::runtime;

pub(crate) fn run(
    root: &Path,
    profile: &str,
    goal: &str,
    required_capabilities: &[String],
    offline: bool,
) -> anyhow::Result<ProfileBehaviorProbeReport> {
    if canonical_profile_name(profile) == "cli" {
        let summary = runtime::run_manifest_checks(root)?;
        return Ok(ProfileBehaviorProbeReport {
            status: summary.assurance.behavior_status(),
            reasons: summary.reasons,
            evidence_path: Some(runtime::EVIDENCE_PATH.to_string()),
        });
    }
    domain_profile(profile).behavior_probe(root, goal, required_capabilities, offline)
}
