use std::path::Path;

use serde::Deserialize;

use super::{FinalAcceptanceRepairTargetInput, FinalAcceptanceRepairTargets};
use crate::planner::verify::VerificationReport;

const CLI_C3_FAILURE: &str = "cli_output_claims:observed_stdout_mismatch";
const CLI_C3_EVIDENCE_PATH: &str = "evidence/cli-probe.json";

#[derive(Deserialize)]
struct CliTestimonyEvidence {
    output_claims: Vec<CliTestimonyClaim>,
}

#[derive(Deserialize)]
struct CliTestimonyClaim {
    matched: bool,
    source: Option<String>,
}

pub(crate) fn final_acceptance_testimony_artifact_paths(
    root: &Path,
    profile: &str,
    report: &VerificationReport,
) -> Vec<String> {
    if crate::planner::profile::canonical_profile_name(profile) != "cli"
        || !testimony_is_primary_or_only(report)
    {
        return Vec::new();
    }
    let Ok(path) = crate::tools::path_guard::resolve_existing(root, CLI_C3_EVIDENCE_PATH) else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(evidence) = serde_json::from_str::<CliTestimonyEvidence>(&text) else {
        return Vec::new();
    };
    let mut paths = Vec::new();
    for source in evidence
        .output_claims
        .iter()
        .filter(|claim| !claim.matched)
        .filter_map(|claim| claim.source.as_deref())
    {
        let artifact = source.split_once(':').map_or(source, |(path, _)| path);
        if crate::tools::path_guard::resolve_existing(root, artifact).is_ok()
            && !paths.iter().any(|path| path == artifact)
        {
            paths.push(artifact.to_string());
        }
    }
    paths
}

pub(crate) fn resolve_final_acceptance_repair_targets_with_testimony(
    input: FinalAcceptanceRepairTargetInput<'_>,
    testimony_artifact_paths: &[String],
) -> FinalAcceptanceRepairTargets {
    if testimony_artifact_paths.is_empty() {
        return super::resolve_final_acceptance_repair_targets(input);
    }
    FinalAcceptanceRepairTargets {
        selected_targets: testimony_artifact_paths.to_vec(),
        selection_reason:
            crate::planner::repair_targeting::RepairTargetSelectionReason::TestimonyArtifactMapped
                .as_str()
                .to_string(),
    }
}

fn testimony_is_primary_or_only(report: &VerificationReport) -> bool {
    if report.primary_reason().contains(CLI_C3_FAILURE) {
        return true;
    }
    if !report.missing_paths.is_empty()
        || !report.dependency_missing.is_empty()
        || !report.command_failures.is_empty()
        || !report.verifier_command_false_negatives.is_empty()
        || !report.compile_errors.is_empty()
    {
        return false;
    }
    let mut observed = false;
    for failure in &report.profile_failures {
        if failure.contains(CLI_C3_FAILURE) {
            observed = true;
        } else if !failure.starts_with("profile behavior evidence:") {
            return false;
        }
    }
    observed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::repair_targeting::FinalAcceptanceRepairTargetInput;

    const PACK_002_C3: &str = include_str!(
        "../../../tests/corpus/apps/test0725_cli_elev_004/fixtures/uat-test0730-cli-pack-002/filter_cloud_001/evidence/cli-probe-c3.json"
    );

    #[test]
    fn pack_002_c3_failure_targets_the_measured_readme_source() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("README.md"), "# measured README\n").unwrap();
        std::fs::create_dir_all(root.path().join("cli")).unwrap();
        std::fs::write(root.path().join("cli/main.py"), "print('ok')\n").unwrap();
        std::fs::create_dir_all(root.path().join("evidence")).unwrap();
        std::fs::write(root.path().join(CLI_C3_EVIDENCE_PATH), PACK_002_C3).unwrap();
        let mut report = VerificationReport::profile_failed(CLI_C3_FAILURE);
        report.push_profile_failure(
            "profile behavior evidence: evidence/cli-assurance.json".to_string(),
        );
        report.push_profile_failure(
            "release gate failed: profile_behavior_probe_failed:cli_output_claims:observed_stdout_mismatch; profile_behavior_probe_evidence:evidence/cli-assurance.json".to_string(),
        );

        let testimony = final_acceptance_testimony_artifact_paths(root.path(), "cli", &report);
        let selection = resolve_final_acceptance_repair_targets_with_testimony(
            FinalAcceptanceRepairTargetInput {
                root: root.path(),
                profile: "cli",
                pending_evidence: &[],
                contract_attribute_paths: &[],
                repair_changed_paths: &[],
                required_paths: &["cli/main.py".to_string(), "README.md".to_string()],
                diagnosis_path: None,
            },
            &testimony,
        );

        assert_eq!(testimony, ["README.md"]);
        assert_eq!(selection.selected_targets, ["README.md"]);
        assert_eq!(selection.selection_reason, "testimony_artifact_mapped");
    }

    #[test]
    fn non_testimony_failure_keeps_the_existing_code_target_chain() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("README.md"), "# measured README\n").unwrap();
        std::fs::create_dir_all(root.path().join("evidence")).unwrap();
        std::fs::write(root.path().join(CLI_C3_EVIDENCE_PATH), PACK_002_C3).unwrap();
        let report =
            VerificationReport::command_failed("python3 cli/main.py", "implementation failure");

        assert!(final_acceptance_testimony_artifact_paths(root.path(), "cli", &report).is_empty());
    }
}
