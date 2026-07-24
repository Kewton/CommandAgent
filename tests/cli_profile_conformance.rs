use std::path::Path;

use commandagent::planner::profile_manifest::ManifestStatus;
use commandagent::planner::profiles::python_cli::{manifest, runtime};
use serde::Deserialize;

const FIXTURE: &str = "tests/corpus/apps/test0725_cli_profile_contract/fixtures/conformance.jsonl";

#[derive(Debug, Deserialize)]
struct Case {
    case: String,
    expected_assurance: String,
    probe_attempted: bool,
    binding_intact: bool,
    checks: std::collections::BTreeMap<String, runtime::CheckStatus>,
}

#[test]
fn conformance_rejects_six_negatives_and_accepts_full_evidence() {
    let text = std::fs::read_to_string(FIXTURE).unwrap();
    let cases = text
        .lines()
        .map(|line| serde_json::from_str::<Case>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(cases.len(), 7);
    for case in &cases {
        let evidence = runtime::EvidenceState {
            probe_attempted: case.probe_attempted,
            binding_intact: case.binding_intact,
            checks: case.checks.clone(),
        };
        assert_eq!(
            runtime::classify(&evidence).as_str(),
            case.expected_assurance,
            "{}",
            case.case
        );
    }
    assert_eq!(
        cases
            .iter()
            .filter(|case| case.expected_assurance != "full")
            .count(),
        6
    );
}

#[test]
fn manifest_is_draft_and_binds_create_phases_and_c1_through_c4() {
    let cli = manifest::get();
    assert_eq!(cli.metadata.status, ManifestStatus::Draft);
    assert_eq!(
        cli.plan
            .phases
            .iter()
            .map(|phase| phase.id.as_str())
            .collect::<Vec<_>>(),
        ["cli-scaffold", "cli-implementation", "cli-validation"]
    );
    assert_eq!(
        cli.checks
            .values()
            .flatten()
            .map(|check| check.id.as_str())
            .collect::<std::collections::BTreeSet<_>>(),
        [
            "cli_probe",
            "help_binding",
            "cli_output_claims",
            "cli_rerun_consistency"
        ]
        .into_iter()
        .collect()
    );
    let plan = manifest::preset_ultra_plan("Build a converter", "default", "create").unwrap();
    assert_eq!(plan.phases.len(), 3);
    assert!(plan.phases[0].prompt.contains("Build a converter"));
    assert!(manifest::guidance().contains("argparse"));
    assert!(manifest::guidance().contains("randomness"));
}

#[test]
fn full_fixture_executes_two_components_and_emits_four_checks() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("cli")).unwrap();
    std::fs::write(
        dir.path().join("cli/main.py"),
        "import argparse\np=argparse.ArgumentParser()\np.add_argument('input', nargs='?')\na=p.parse_args()\nprint('value=7')\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("README.md"),
        "## Usage\n```console\n$ python3 cli/main.py sample.csv\nvalue=7\n```\n",
    )
    .unwrap();

    let summary = runtime::run_manifest_checks(dir.path()).unwrap();

    assert_eq!(summary.assurance, runtime::CliAssurance::Full);
    assert!(
        summary
            .evidence
            .checks
            .values()
            .all(|status| *status == runtime::CheckStatus::Pass)
    );
    for evidence in [
        "evidence/cli-case-binding.json",
        "evidence/cli-probe.json",
        "evidence/help-binding.json",
        "evidence/cli-assurance.json",
    ] {
        assert!(dir.path().join(evidence).is_file(), "{evidence}");
    }
    assert!(Path::new(FIXTURE).is_file());
}
