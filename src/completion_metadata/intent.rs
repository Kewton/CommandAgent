use std::path::Path;

use serde_json::Value;

use crate::config::Config;
use crate::eval_events::{CompletionProjection, CompletionSnapshot};
use crate::planner::adjudication::contract::{EvidenceStage, ExpectedOutcome, IntentId};
use crate::planner::adjudication::investigate::InvestigationRunEvidence;

mod profile;

const INVESTIGATION_INCOMPLETE: &str = "investigation_incomplete";
const INVESTIGATION_PROBE_NOT_RUN: &str = "investigation_probe_not_run";

pub(super) fn apply_snapshot(config: &Config, snapshot: &mut CompletionSnapshot) -> bool {
    if config.intent_override == Some(IntentId::Investigate) {
        let (level, reason) = investigation_assurance(config);
        snapshot.assurance_level = level;
        snapshot.assurance_reason = reason;
        return true;
    }
    if snapshot.contract_origin == crate::planner::fix_runtime::FIX_CONTRACT_ORIGIN {
        return true;
    }
    profile::apply_snapshot(config, snapshot)
}

pub(super) fn apply_terminal_projection(config: &Config, projection: &mut CompletionProjection) {
    if config.intent_override == Some(IntentId::Investigate) {
        let (level, reason) = investigation_assurance(config);
        projection.assurance_level = level;
        projection.assurance_reason = reason;
    } else {
        profile::apply_terminal_projection(config, projection);
    }
}

fn investigation_assurance(config: &Config) -> (String, String) {
    latest_adjudication(config.eval_events_path.as_deref()).unwrap_or_else(|| {
        if i1_executed(&config.workspace_root) {
            ("failed".to_string(), INVESTIGATION_INCOMPLETE.to_string())
        } else {
            (
                "static".to_string(),
                INVESTIGATION_PROBE_NOT_RUN.to_string(),
            )
        }
    })
}

fn latest_adjudication(path: Option<&Path>) -> Option<(String, String)> {
    let text = std::fs::read_to_string(path?).ok()?;
    text.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .rev()
        .find(|event| {
            event.get("event").and_then(Value::as_str) == Some("investigation_adjudicated")
        })
        .and_then(|event| {
            Some((
                event.get("assurance_level")?.as_str()?.to_string(),
                event
                    .get("assurance_reason")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            ))
        })
}

fn i1_executed(root: &Path) -> bool {
    std::fs::read(root.join("evidence/investigation-run.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<InvestigationRunEvidence>(&bytes).ok())
        .is_some_and(|run| {
            run.intent == "investigate"
                && run.stage == EvidenceStage::Diagnosis
                && run.expected == ExpectedOutcome::Failure
                && run.executed
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use crate::eval_events::project_completion;
    use clap::Parser;

    const FIXTURE_ROOT: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/investigation_projection"
    );
    const DATA_ROOT: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/corpus/apps/test0715_data_b2j_terminal_projection/fixtures/",
        "data7_gemma31_profile_001"
    );

    #[test]
    fn inv1_measured_projection_shapes_follow_investigation_contract() {
        for (case, expected_level, expected_reason) in [
            ("run2-diagnosis-unbound", "failed", "diagnosis_unbound"),
            ("run1-incomplete", "failed", INVESTIGATION_INCOMPLETE),
            ("probe-not-run", "static", INVESTIGATION_PROBE_NOT_RUN),
        ] {
            let root = tempfile::tempdir().unwrap();
            let events = root.path().join("events.jsonl");
            let fixture = Path::new(FIXTURE_ROOT).join(case);
            if fixture.join("events.jsonl").is_file() {
                std::fs::copy(fixture.join("events.jsonl"), &events).unwrap();
            }
            if fixture.join("investigation-run.json").is_file() {
                std::fs::create_dir_all(root.path().join("evidence")).unwrap();
                std::fs::copy(
                    fixture.join("investigation-run.json"),
                    root.path().join("evidence/investigation-run.json"),
                )
                .unwrap();
            }
            let config = investigation_config(root.path(), events);
            let mut snapshot = data_snapshot();
            assert!(apply_snapshot(&config, &mut snapshot));
            let mut projection = project_completion(false, &snapshot);
            apply_terminal_projection(&config, &mut projection);

            assert_eq!(projection.assurance_level, expected_level, "{case}");
            assert_eq!(projection.assurance_reason, expected_reason, "{case}");
        }
    }

    #[test]
    fn create_data_snapshot_and_terminal_projection_remain_byte_compatible() {
        let root = Path::new(DATA_ROOT);
        let config = create_data_config(root);
        let mut expected_snapshot = data_snapshot();
        let mut actual_snapshot = expected_snapshot.clone();
        super::super::data::apply_snapshot(root, &mut expected_snapshot);
        assert!(apply_snapshot(&config, &mut actual_snapshot));
        assert_eq!(actual_snapshot, expected_snapshot);

        expected_snapshot.final_acceptance_status = "full_success".into();
        expected_snapshot.completion_contract_verification_enabled = true;
        let mut expected_projection = project_completion(true, &expected_snapshot);
        let mut actual_projection = expected_projection.clone();
        super::super::data::apply_terminal_projection(root, &mut expected_projection);
        apply_terminal_projection(&config, &mut actual_projection);
        assert_eq!(actual_projection, expected_projection);
    }

    fn investigation_config(root: &Path, events: std::path::PathBuf) -> Config {
        let mut config = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            root.to_str().unwrap(),
            "--intent",
            "investigate",
            "--profile",
            "data",
        ]))
        .unwrap();
        config.eval_events_path = Some(events);
        config
    }

    fn create_data_config(root: &Path) -> Config {
        Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            root.to_str().unwrap(),
            "--profile",
            "data",
        ]))
        .unwrap()
    }

    fn data_snapshot() -> CompletionSnapshot {
        let mut snapshot = CompletionSnapshot::empty();
        snapshot.profile = "data".into();
        snapshot.effective_profile = "data".into();
        snapshot.assurance_level = "static".into();
        snapshot.assurance_reason = "data_profile_probe_not_run".into();
        snapshot
    }
}
