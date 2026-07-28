use std::path::Path;

use crate::eval_events::{CompletionProjection, CompletionSnapshot};
use crate::planner::fix_runtime::FIX_CONTRACT_ORIGIN;
use crate::planner::profile::canonical_profile_name;
use crate::planner::profiles::ingest::runtime::{
    ASSURANCE_EVIDENCE_PATH, IngestAssurance, IngestCheckSummary, classify,
};

const PROBE_NOT_RUN: &str = "ingest_probe_not_run";
const CLAIMS_ABSENT: &str = "ingest_claims_absent";
const ASSURANCE_FAILED: &str = "ingest_assurance_failed";

pub(super) fn apply_snapshot(root: &Path, snapshot: &mut CompletionSnapshot) -> bool {
    if canonical_profile_name(&snapshot.effective_profile) != "ingest" {
        return false;
    }
    let (assurance, reason) = completion_assurance(root);
    snapshot.assurance_level = assurance.as_str().to_string();
    snapshot.assurance_reason = reason.to_string();
    true
}

pub(super) fn apply_terminal_projection(root: &Path, projection: &mut CompletionProjection) {
    if projection.contract_origin == FIX_CONTRACT_ORIGIN
        || canonical_profile_name(&projection.effective_profile) != "ingest"
    {
        return;
    }
    let (assurance, reason) = completion_assurance(root);
    projection.assurance_level = assurance.as_str().to_string();
    projection.assurance_reason = reason.to_string();
}

fn completion_assurance(root: &Path) -> (IngestAssurance, &'static str) {
    let assurance = std::fs::read(root.join(ASSURANCE_EVIDENCE_PATH))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<IngestCheckSummary>(&bytes).ok())
        .map(|summary| classify(&summary.evidence))
        .unwrap_or(IngestAssurance::Static);
    let reason = match assurance {
        IngestAssurance::Full => "",
        IngestAssurance::Partial => CLAIMS_ABSENT,
        IngestAssurance::Static => PROBE_NOT_RUN,
        IngestAssurance::Failed => ASSURANCE_FAILED,
    };
    (assurance, reason)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use crate::config::Config;
    use crate::eval_events::project_completion;
    use clap::Parser;
    use serde_json::Value;

    const FIXTURE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/corpus/apps/test0727_ingest_completion_projection/fixtures/assurance.jsonl"
    );

    #[test]
    fn runtime_shaped_fixtures_preserve_contract_assurance_after_admission() {
        for line in std::fs::read_to_string(FIXTURE).unwrap().lines() {
            let fixture: Value = serde_json::from_str(line).unwrap();
            let root = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(root.path().join("evidence")).unwrap();
            std::fs::write(
                root.path().join(ASSURANCE_EVIDENCE_PATH),
                serde_json::to_vec_pretty(&fixture["summary"]).unwrap(),
            )
            .unwrap();
            let mut snapshot = ingest_snapshot();
            assert!(apply_snapshot(root.path(), &mut snapshot));
            assert_eq!(
                snapshot.assurance_level, fixture["expected_earned"],
                "{}",
                fixture["fixture"]
            );

            let mut projection = project_completion(true, &snapshot);
            let config = ingest_config(root.path());
            crate::completion_metadata::apply_config_completion_projection(
                &config,
                &mut projection,
            );
            let earned = fixture["expected_earned"].as_str().unwrap();
            assert_eq!(projection.assurance_level, earned);
            assert_ne!(projection.assurance_reason, "profile_not_admitted");
        }
    }

    #[test]
    fn non_ingest_projection_remains_byte_compatible() {
        let root = tempfile::tempdir().unwrap();
        let mut snapshot = CompletionSnapshot::empty();
        snapshot.effective_profile = "data".into();
        let mut projection = project_completion(false, &snapshot);
        let expected = projection.clone();
        apply_terminal_projection(root.path(), &mut projection);
        assert_eq!(projection, expected);
    }

    fn ingest_config(root: &Path) -> Config {
        Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            root.to_str().unwrap(),
            "--profile",
            "ingest",
        ]))
        .unwrap()
    }

    fn ingest_snapshot() -> CompletionSnapshot {
        let mut snapshot = CompletionSnapshot::empty();
        snapshot.profile = "ingest".into();
        snapshot.effective_profile = "ingest".into();
        snapshot.final_acceptance_status = "full_success".into();
        snapshot.release_gate_status = "pass".into();
        snapshot.completion_contract_verification_enabled = true;
        snapshot
    }
}
