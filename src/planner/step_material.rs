use serde_json::json;

use crate::config::Config;
use crate::planner::step_plan::PlanStep;

const INGEST_SELECTOR_STEP_ID: &str = "declare-ingest-inspection";
const INGEST_IMPLEMENT_STEP_ID: &str = "implement-ingest-delivery";

pub(crate) fn inject(config: &Config, step: &mut PlanStep) -> anyhow::Result<()> {
    if crate::planner::profile::domain_profile(&config.profile).id() != "ingest" {
        return Ok(());
    }
    match step.id.as_str() {
        INGEST_SELECTOR_STEP_ID => {
            let guidance = crate::planner::profiles::ingest::snapshot_structure::render(
                &config.workspace_root,
            )?;
            step.instruction.push_str("\n\n");
            step.instruction.push_str(&guidance.text);
            crate::eval_events::emit(
                config.eval_events_path.as_deref(),
                json!({
                    "event": "ingest_snapshot_structure_injected",
                    "profile": "ingest",
                    "step_id": step.id,
                    "files": &guidance.files,
                    "omitted_files": guidance.omitted_files,
                    "traversal_capped": guidance.traversal_capped,
                    "limits": crate::planner::profiles::ingest::snapshot_structure::limits(),
                }),
            );
        }
        INGEST_IMPLEMENT_STEP_ID => {
            let guidance = crate::planner::profiles::ingest::candidate_guidance::render(
                &config.workspace_root,
            )?;
            step.instruction.push_str("\n\n");
            step.instruction.push_str(&guidance.text);
            crate::eval_events::emit(
                config.eval_events_path.as_deref(),
                json!({
                    "event": "ingest_candidate_ids_injected",
                    "profile": "ingest",
                    "step_id": step.id,
                    "frozen_before_run": true,
                    "candidate_count": guidance.candidate_ids.len(),
                    "candidate_ids": guidance.candidate_ids,
                    "selector": {
                        "kind": guidance.selector_kind,
                        "value": guidance.selector_value,
                    },
                    "freeze_evidence_path":
                        crate::planner::profiles::ingest::accounting::FREEZE_EVIDENCE_PATH,
                }),
            );
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use clap::Parser;

    #[test]
    fn other_profile_and_step_ids_are_byte_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_string_lossy().to_string();
        let config = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--intent",
            "create",
            "--profile",
            "data",
            "--ultra-plan",
            "test",
        ]))
        .unwrap();
        let mut step = PlanStep {
            id: "declare-ingest-inspection".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: "original".to_string(),
            expected_paths: Vec::new(),
            verify: Vec::new(),
        };
        inject(&config, &mut step).unwrap();
        assert_eq!(step.instruction, "original");

        let mut config = config;
        config.profile = "ingest".to_string();
        let mut step = PlanStep {
            id: "unrelated-step".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: "original".to_string(),
            expected_paths: Vec::new(),
            verify: Vec::new(),
        };
        inject(&config, &mut step).unwrap();
        assert_eq!(step.instruction, "original");
    }
}
