use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use commandagent::planner::profiles::data::checks;
use commandagent::planner::profiles::data::runtime::{
    DataAssurance, assurance_from_evidence, run_manifest_checks,
};
use serde::Deserialize;

const FIXTURE_ROOT: &str = "tests/corpus/apps/test0713_data_profile_contract_v0/fixtures";
const E2_FIXTURE_ROOT: &str = "tests/corpus/apps/test0715_data_b2g_e2_calibration/fixtures";
const E2_ARTIFACT_ROOT: &str = "workspace/management/runs/uat-test0715-ff1-002/artifacts";
const DATA11_FIXTURE_ROOT: &str =
    "tests/corpus/apps/test0715_data11_final_scope/fixtures/data5_qwen35_none_001";
const DATA11_ARTIFACT_ROOT: &str =
    "workspace/management/runs/uat-test0715-data-005/artifacts/data5_qwen35_none_001";

#[derive(Debug, Deserialize)]
struct DataFixture {
    case: String,
    expected_assurance: String,
    expected_failed_check: Option<String>,
    files: BTreeMap<String, String>,
}

#[test]
fn full_fixture_runs_end_to_end_and_emits_full_evidence() {
    let (dir, fixture) = materialize("full.jsonl");

    let summary = run_manifest_checks(dir.path()).unwrap();

    assert_eq!(fixture.case, "full");
    assert_eq!(fixture.expected_assurance, "full");
    assert_eq!(summary.assurance, DataAssurance::Full, "{summary:?}");
    assert!(summary.checks.values().all(|passed| *passed));
    for path in [
        "evidence/pipeline-run.json",
        checks::INSPECTION_SCHEMA_EVIDENCE_PATH,
        checks::RESULTS_SCHEMA_EVIDENCE_PATH,
        checks::RECONCILIATION_EVIDENCE_PATH,
        checks::CLAIMS_BINDING_EVIDENCE_PATH,
        checks::RERUN_CONSISTENCY_EVIDENCE_PATH,
        "evidence/data-assurance.json",
    ] {
        assert!(dir.path().join(path).is_file(), "missing {path}");
    }
}

#[test]
fn fabricated_number_fails_e2_and_cannot_earn_full() {
    let (dir, fixture) = materialize("fabricated-claim.jsonl");

    let summary = run_manifest_checks(dir.path()).unwrap();

    assert_eq!(summary.assurance.as_str(), fixture.expected_assurance);
    assert!(!summary.checks[fixture.expected_failed_check.as_deref().unwrap()]);
    assert!(summary.checks["data_inspection_schema"]);
    assert!(summary.checks["data_reconciliation"]);
    assert!(summary.checks["data_rerun_consistency"]);
    let evidence =
        std::fs::read_to_string(dir.path().join(checks::CLAIMS_BINDING_EVIDENCE_PATH)).unwrap();
    assert!(evidence.contains("claims_binding_violation"));
}

#[test]
fn measured_uat_run1_and_run4_pass_e2_after_calibration() {
    for (case, expected_claims, expected_date_labels, expected_reconciliation) in [
        ("data4_qwen35_profile_001", 43, 24, 6),
        ("data4_qwen35_none_002", 29, 12, 7),
    ] {
        let dir = materialize_e2_output(case);
        let evidence = checks::check_claims_binding(dir.path()).unwrap();

        assert!(evidence.ok, "{case}: {evidence:?}");
        assert_eq!(evidence.status, "pass");
        assert_eq!(evidence.claims.len(), expected_claims);
        assert_eq!(
            evidence
                .claims
                .iter()
                .filter(|claim| claim.claim_kind == "date_label")
                .count(),
            expected_date_labels
        );
        assert_eq!(
            evidence
                .claims
                .iter()
                .filter(|claim| claim
                    .matched_key
                    .as_deref()
                    .is_some_and(|key| key.starts_with("reconciliation.")))
                .count(),
            expected_reconciliation
        );
        assert!(evidence.failure_kinds.is_empty());
    }
}

#[test]
fn measured_e2_fixtures_are_byte_identical_to_archived_uat_artifacts() {
    for case in ["data4_qwen35_profile_001", "data4_qwen35_none_002"] {
        for name in ["report.md", "results.json"] {
            let fixture = Path::new(E2_FIXTURE_ROOT)
                .join(case)
                .join("output")
                .join(name);
            let artifact = Path::new(E2_ARTIFACT_ROOT)
                .join(case)
                .join("output")
                .join(name);
            assert_eq!(
                std::fs::read(&fixture).unwrap(),
                std::fs::read(&artifact).unwrap(),
                "fixture drift: {}",
                fixture.display()
            );
        }
    }
}

#[test]
fn reconciliation_row_mismatch_remains_a_claims_binding_violation() {
    let dir = materialize_e2_output("reconciliation-row-mismatch");

    let evidence = checks::check_claims_binding(dir.path()).unwrap();

    assert!(!evidence.ok);
    assert_eq!(evidence.failure_kinds.len(), 1);
    assert!(evidence.failure_kinds[0].starts_with("claims_binding_violation"));
    let claim = evidence
        .claims
        .iter()
        .find(|claim| claim.raw == "61")
        .unwrap();
    assert!(!claim.ok);
    assert!(claim.matched_key.is_none());
    let nearest = claim.nearest_miss.as_ref().unwrap();
    assert_eq!(nearest.key, "reconciliation.input_rows");
    assert_eq!(nearest.result_value, 60.0);
    assert_eq!(nearest.absolute_difference, 1.0);
}

#[test]
fn measured_run3_earns_full_without_final_inspection_schema_gating() {
    let dir = tempfile::tempdir().unwrap();
    for relative in [
        "data/sales.csv",
        "pipeline/main.py",
        "output/inspection.json",
        "output/results.json",
        "output/report.md",
    ] {
        let fixture = Path::new(DATA11_FIXTURE_ROOT).join(relative);
        let artifact = Path::new(DATA11_ARTIFACT_ROOT).join(relative);
        let bytes = std::fs::read(&fixture).unwrap();
        assert_eq!(
            bytes,
            std::fs::read(artifact).unwrap(),
            "fixture drift: {relative}"
        );
        let target = dir.path().join(relative);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, bytes).unwrap();
    }
    let summary = run_manifest_checks(dir.path()).unwrap();

    assert_eq!(summary.assurance, DataAssurance::Full, "{summary:?}");
    for id in [
        "pipeline_probe",
        "data_results_schema",
        "data_reconciliation",
        "data_claims_binding",
        "data_rerun_consistency",
    ] {
        assert!(summary.checks[id], "{id}: {summary:?}");
    }
    assert!(!summary.checks["data_inspection_schema"]);
}

#[test]
fn silent_exclusion_fails_e1() {
    let (dir, fixture) = materialize("unaccounted-exclusion.jsonl");

    let summary = run_manifest_checks(dir.path()).unwrap();

    assert_eq!(summary.assurance.as_str(), fixture.expected_assurance);
    assert!(!summary.checks[fixture.expected_failed_check.as_deref().unwrap()]);
    assert!(summary.checks["data_inspection_schema"]);
    let evidence =
        std::fs::read_to_string(dir.path().join(checks::RECONCILIATION_EVIDENCE_PATH)).unwrap();
    assert!(evidence.contains("reconciliation_violation"));
}

#[test]
fn time_dependent_pipeline_fails_e3() {
    let (dir, fixture) = materialize("time-dependent.jsonl");

    let summary = run_manifest_checks(dir.path()).unwrap();

    assert_eq!(summary.assurance.as_str(), fixture.expected_assurance);
    assert!(!summary.checks[fixture.expected_failed_check.as_deref().unwrap()]);
    assert!(summary.checks["data_inspection_schema"]);
    let evidence =
        std::fs::read_to_string(dir.path().join(checks::RERUN_CONSISTENCY_EVIDENCE_PATH)).unwrap();
    assert!(evidence.contains("results_changed"));
}

#[test]
fn unexecuted_probe_is_static_and_never_projects_full() {
    let (dir, _) = materialize("full.jsonl");

    assert_eq!(assurance_from_evidence(dir.path()), DataAssurance::Static);
    assert!(!dir.path().join("evidence/pipeline-run.json").exists());
}

#[test]
fn partial_or_full_requires_both_e1_and_e3_pass_evidence() {
    for missing_evidence in [
        checks::RECONCILIATION_EVIDENCE_PATH,
        checks::RERUN_CONSISTENCY_EVIDENCE_PATH,
    ] {
        let (dir, _) = materialize("fabricated-claim.jsonl");
        let summary = run_manifest_checks(dir.path()).unwrap();
        assert_eq!(summary.assurance, DataAssurance::Partial);

        std::fs::remove_file(dir.path().join(missing_evidence)).unwrap();

        assert_eq!(
            assurance_from_evidence(dir.path()),
            DataAssurance::Failed,
            "missing {missing_evidence} must not project partial or full"
        );
    }
}

fn materialize(name: &str) -> (tempfile::TempDir, DataFixture) {
    let fixture = load_fixture(name);
    let dir = tempfile::tempdir().unwrap();
    for (relative, content) in &fixture.files {
        commandagent::tools::path_guard::validate_workspace_relative(relative).unwrap();
        let path = dir.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
    (dir, fixture)
}

fn materialize_e2_output(case: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("output")).unwrap();
    for name in ["report.md", "results.json"] {
        let source = Path::new(E2_FIXTURE_ROOT)
            .join(case)
            .join("output")
            .join(name);
        std::fs::write(
            dir.path().join("output").join(name),
            std::fs::read(source).unwrap(),
        )
        .unwrap();
    }
    dir
}

fn load_fixture(name: &str) -> DataFixture {
    let path = PathBuf::from(FIXTURE_ROOT).join(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    let line = text.lines().find(|line| !line.trim().is_empty()).unwrap();
    serde_json::from_str(line).unwrap_or_else(|error| {
        panic!(
            "failed to parse {} as JSONL: {error}",
            Path::new(&path).display()
        )
    })
}
