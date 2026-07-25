#[cfg(test)]
mod moved {
    use super::super::*;
    use crate::planner::profiles::python_cli::{argv_probe, help_binding, runtime};

    fn write_fixture(root: &Path, script: &str) {
        std::fs::create_dir_all(root.join("cli")).unwrap();
        std::fs::write(root.join("cli/main.py"), script).unwrap();
        std::fs::write(
            root.join("README.md"),
            "## Usage\n\n```console\n$ python3 cli/main.py sample.csv\nvalue=7\n```\n",
        )
        .unwrap();
    }

    fn cli_plan() -> UltraPlan {
        UltraPlan {
            goal: "Create a deterministic command line tool".to_string(),
            profile: "cli".to_string(),
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
        cfg.profile = "cli".to_string();
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
        assert!(event_text.contains("\"profile\":\"cli\""));
        assert!(event_text.contains("\"status\":\"pass\""));
        assert!(event_text.contains("\"evidence_path\":\"evidence/cli-assurance.json\""));
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
}
