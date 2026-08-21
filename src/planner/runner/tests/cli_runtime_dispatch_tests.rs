#[cfg(test)]
mod moved {
    use super::super::*;
    use crate::minimal_loop::pipeline_probe;
    use crate::planner::profiles::ingest::{
        accounting as ingest_accounting, runtime as ingest_runtime,
        source_binding as ingest_source_binding,
    };
    use crate::planner::profiles::python_cli::{self, argv_probe, help_binding, runtime};

    fn write_fixture(root: &Path, script: &str) {
        std::fs::create_dir_all(root.join("cli")).unwrap();
        std::fs::write(root.join("cli/main.py"), script).unwrap();
        std::fs::write(
            root.join("README.md"),
            "## Usage\n\n```console\n$ python3 cli/main.py sample.csv\nvalue=7\n```\n",
        )
        .unwrap();
        let scaffold = ["pyproject.toml".into(), "src/anvil_app/main.py".into()];
        python_cli::complete_scaffold(root, &scaffold).unwrap();
    }

    fn cli_plan() -> UltraPlan {
        UltraPlan {
            goal: "Create a deterministic command line tool".to_string(),
            profile: "python-cli".to_string(),
            style: "default".to_string(),
            intent: "create".to_string(),
            phases: Vec::new(),
        }
    }

    #[test]
    fn cli_final_acceptance_production_path_executes_manifest_c1_through_c4() {
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join("events.jsonl");
        write_fixture(
            dir.path(),
            "import argparse\n\
p = argparse.ArgumentParser()\n\
p.add_argument('input', nargs='?')\n\
p.parse_args()\n\
print('value=7')\n",
        );
        let mut cfg = config(dir.path().to_path_buf());
        cfg.profile = "python-cli".to_string();
        cfg.eval_events_path = Some(events.clone());

        let report = ultra_final_acceptance_report(&cli_plan(), &cfg).unwrap();

        assert!(report.is_pass(), "{report:?}");
        for evidence in [
            argv_probe::CASE_BINDING_PATH,
            argv_probe::EVIDENCE_PATH,
            help_binding::EVIDENCE_PATH,
            runtime::EVIDENCE_PATH,
        ] {
            assert!(dir.path().join(evidence).is_file(), "{evidence}");
        }
        let probe: argv_probe::Report = serde_json::from_slice(
            &std::fs::read(dir.path().join(argv_probe::EVIDENCE_PATH)).unwrap(),
        )
        .unwrap();
        assert_eq!(probe.observations.len(), 3);
        assert_eq!(probe.observations[0].exit_code, Some(0));
        assert!(
            probe.observations[1]
                .exit_code
                .is_some_and(|code| code != 0)
        );
        let summary: runtime::CliCheckSummary = serde_json::from_slice(
            &std::fs::read(dir.path().join(runtime::EVIDENCE_PATH)).unwrap(),
        )
        .unwrap();
        assert_eq!(summary.assurance, runtime::CliAssurance::Full);
        assert!(
            summary
                .evidence
                .checks
                .values()
                .all(|status| *status == runtime::CheckStatus::Pass)
        );
        let event_text = std::fs::read_to_string(events).unwrap();
        assert!(event_text.contains("\"event\":\"profile_behavior_probe\""));
        assert!(event_text.contains("\"profile\":\"python-cli\""));
        assert!(event_text.contains("\"status\":\"pass\""));
        assert!(event_text.contains("\"evidence_path\":\"evidence/cli-assurance.json\""));
        assert!(event_text.contains("\"assurance_level\":\"full\""));
        assert!(!event_text.contains("\"assurance_reason\":\"cli_probe_not_run\""));
        let final_acceptance = latest_event(
            cfg.eval_events_path.as_ref().unwrap(),
            "ultra_final_acceptance",
        );
        assert_eq!(
            final_acceptance
                .get("profile_behavior_probe_status")
                .and_then(Value::as_str),
            Some("pass")
        );
    }

    #[test]
    fn cli_final_acceptance_failure_still_persists_c_evidence() {
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join("events.jsonl");
        write_fixture(dir.path(), "print('value=7')\n");
        let mut cfg = config(dir.path().to_path_buf());
        cfg.profile = "cli".to_string();
        cfg.eval_events_path = Some(events);

        let report = ultra_final_acceptance_report(&cli_plan(), &cfg).unwrap();

        assert!(!report.is_pass(), "{report:?}");
        let probe: argv_probe::Report = serde_json::from_slice(
            &std::fs::read(dir.path().join(argv_probe::EVIDENCE_PATH)).unwrap(),
        )
        .unwrap();
        assert_eq!(probe.observations.len(), 3);
        assert_eq!(probe.observations[0].exit_code, Some(0));
        assert_eq!(probe.observations[1].exit_code, Some(0));
        let summary: runtime::CliCheckSummary = serde_json::from_slice(
            &std::fs::read(dir.path().join(runtime::EVIDENCE_PATH)).unwrap(),
        )
        .unwrap();
        assert_eq!(summary.assurance, runtime::CliAssurance::Failed);
        for evidence in [
            argv_probe::CASE_BINDING_PATH,
            argv_probe::EVIDENCE_PATH,
            help_binding::EVIDENCE_PATH,
            runtime::EVIDENCE_PATH,
        ] {
            assert!(dir.path().join(evidence).is_file(), "{evidence}");
        }
    }

    #[test]
    fn ingest_final_acceptance_production_path_executes_n1_through_n5() {
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join("events.jsonl");
        write_ingest_fixture(dir.path());
        let mut cfg = config(dir.path().to_path_buf());
        cfg.profile = "ingest".to_string();
        cfg.eval_events_path = Some(events.clone());
        let plan = UltraPlan {
            goal: "Extract snapshot events into declared records".to_string(),
            profile: "ingest".to_string(),
            style: "default".to_string(),
            intent: "create".to_string(),
            phases: Vec::new(),
        };

        let report = ultra_final_acceptance_report(&plan, &cfg).unwrap();

        assert!(report.is_pass(), "{report:?}");
        for evidence in [
            ingest_accounting::FREEZE_EVIDENCE_PATH,
            pipeline_probe::PIPELINE_RUN_EVIDENCE_PATH,
            ingest_runtime::INGEST_PROBE_EVIDENCE_PATH,
            ingest_source_binding::EVIDENCE_PATH,
            ingest_accounting::ACCOUNTING_EVIDENCE_PATH,
            ingest_runtime::FORMAT_SCHEMA_EVIDENCE_PATH,
            ingest_runtime::RERUN_EVIDENCE_PATH,
            ingest_runtime::ASSURANCE_EVIDENCE_PATH,
        ] {
            assert!(dir.path().join(evidence).is_file(), "missing {evidence}");
        }
        let summary: ingest_runtime::IngestCheckSummary = serde_json::from_slice(
            &std::fs::read(dir.path().join(ingest_runtime::ASSURANCE_EVIDENCE_PATH)).unwrap(),
        )
        .unwrap();
        assert_eq!(summary.assurance, ingest_runtime::IngestAssurance::Full);
        assert!(
            summary
                .evidence
                .checks
                .values()
                .all(|status| { *status == ingest_runtime::CheckStatus::Pass })
        );
        let probe: ingest_runtime::IngestProbeEvidence = serde_json::from_slice(
            &std::fs::read(dir.path().join(ingest_runtime::INGEST_PROBE_EVIDENCE_PATH)).unwrap(),
        )
        .unwrap();
        assert!(probe.ok);
        assert!(!probe.snapshot_ids.is_empty());
        assert!(probe.required_artifacts.values().all(|present| *present));
        assert!(probe.execution.is_some());
        let event_text = std::fs::read_to_string(events).unwrap();
        assert!(event_text.contains("\"event\":\"profile_behavior_probe\""));
        assert!(event_text.contains("\"profile\":\"ingest\""));
        assert!(event_text.contains("\"status\":\"pass\""));
        assert!(event_text.contains("\"evidence_path\":\"evidence/ingest-assurance.json\""));
    }

    fn write_ingest_fixture(root: &Path) {
        std::fs::create_dir_all(root.join("data/snapshots")).unwrap();
        std::fs::create_dir_all(root.join("pipeline")).unwrap();
        std::fs::create_dir_all(root.join("output")).unwrap();
        std::fs::write(
            root.join("data/snapshots/events.html"),
            "<article><time>令和7年7月25日</time><span>市民会館</span></article>",
        )
        .unwrap();
        std::fs::write(
            root.join(ingest_accounting::INSPECTION_PATH),
            r#"{
  "candidate_selector": {"kind": "html_tag", "value": "article"},
  "candidate_accounting": {
    "accepted": [{"candidate_id": "data/snapshots/events.html#0", "record_index": 0}],
    "excluded": []
  },
  "record_format": {"fields": [
    {"name": "date", "type": "string", "normalizations": ["japanese_date_to_iso"]},
    {"name": "venue", "type": "string", "normalizations": ["identity"]}
  ]}
}
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("pipeline/main.py"),
            r##"import json
from pathlib import Path
Path("output").mkdir(exist_ok=True)
records = [{"date": "2025-07-25", "venue": "市民会館"}]
Path("output/records.json").write_text(json.dumps(records, ensure_ascii=False, sort_keys=True) + "\n")
Path("output/report.md").write_text("# Extracted records\n\n- 2025-07-25 市民会館\n")
"##,
        )
        .unwrap();
        std::fs::write(root.join("output/records.json"), "[]\n").unwrap();
        std::fs::write(root.join("output/report.md"), "# pending\n").unwrap();
    }
}
