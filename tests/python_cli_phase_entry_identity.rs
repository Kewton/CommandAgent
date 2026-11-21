//! #896 (IC-42): replay the recorded and synthetic phase plans of
//! `tests/corpus/apps/python-cli-phase-entry-identity` through the phase
//! admission check, and pin the unchanged final single-entry gate.
use std::path::Path;

use commandagent::planner::profile::verify_profile_final;
use commandagent::planner::profiles::python_cli::entry_identity::{EntryIdentity, assess};
use commandagent::planner::step_plan::{PlanStep, StepPlan};
use serde_json::Value;

const CORPUS: &str = "tests/corpus/apps/python-cli-phase-entry-identity/fixtures/cases.json";
const ISSUE811: &str =
    "tests/corpus/apps/issue811-python-cli-single-entrypoint/fixtures/cases.json";

fn cases(path: &str) -> Value {
    let raw = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap();
    serde_json::from_str(&raw).unwrap()
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item.as_str().unwrap().to_owned())
        .collect()
}

/// The phase prompt shape the host passes as the StepPlan goal.
fn phase_prompt(case: &Value) -> String {
    let changed = strings(&case["changed"])
        .iter()
        .map(|path| format!("  - {path}\n"))
        .collect::<String>();
    format!(
        "Original ultra goal: {}\nProfile: python-cli\nStyle: default\nIntent: create\nPhase id: {}\nPhase task: {}\n\nPrior ultra context:\n- Created or changed paths:\n{changed}",
        case["goal"].as_str().unwrap(),
        case["phase_id"].as_str().unwrap(),
        case["phase_task"].as_str().unwrap(),
    )
}

fn write_main(root: &Path, rel: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "print('ok')\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

fn write_pyproject(root: &Path, scripts: &serde_json::Map<String, Value>) {
    let (name, _) = scripts.iter().next().unwrap();
    let lines = scripts
        .iter()
        .map(|(script, target)| format!("{script} = \"{}\"\n", target.as_str().unwrap()))
        .collect::<String>();
    std::fs::write(
        root.join("pyproject.toml"),
        format!("[project]\nname = \"{name}\"\nversion = \"0.1.0\"\n\n[project.scripts]\n{lines}"),
    )
    .unwrap();
}

#[test]
fn issue896_corpus_rows_match_phase_admission() {
    let corpus = cases(CORPUS);
    let rows = corpus["cases"].as_array().unwrap();
    for origin in ["recorded", "synthetic"] {
        assert!(
            rows.iter().any(|row| row["origin"] == origin),
            "corpus keeps {origin} rows"
        );
    }
    for row in rows {
        let id = row["id"].as_str().unwrap();
        let root = tempfile::tempdir().unwrap();
        for entry in strings(&row["existing"]) {
            write_main(root.path(), &entry);
        }
        let plan = StepPlan {
            goal: phase_prompt(row),
            steps: row["steps"]
                .as_array()
                .unwrap()
                .iter()
                .map(|step| PlanStep {
                    id: step["id"].as_str().unwrap().to_owned(),
                    kind: step["kind"].as_str().unwrap().to_owned(),
                    expected_result: "pass".to_owned(),
                    instruction: step["instruction"].as_str().unwrap().to_owned(),
                    expected_paths: strings(&step["expected_paths"]),
                    verify: strings(&step["verify"]),
                })
                .collect(),
        };
        let outcome = assess(root.path(), &plan);
        let reason = match (row["expect"].as_str().unwrap(), &outcome) {
            ("admitted", EntryIdentity::Admitted) => String::new(),
            ("repairable", EntryIdentity::Repairable(reason)) => reason.clone(),
            (expect, outcome) => panic!("{id}: expected {expect}, got {outcome:?}"),
        };
        for needle in strings(&row["reason_contains"]) {
            assert!(reason.contains(&needle), "{id}: {needle:?} not in {reason}");
        }
    }
}

#[test]
fn issue896_final_single_entry_gate_is_unchanged() {
    let corpus = cases(CORPUS);
    let issue811 = cases(ISSUE811);
    let mut rows = corpus["final"].as_array().unwrap().clone();
    // The issue811 rows list every file; only the main.py files are entries.
    for case in issue811["cases"].as_array().unwrap() {
        let mut row = case.clone();
        row["mains"] = Value::Array(
            strings(&case["files"])
                .into_iter()
                .filter(|file| file.ends_with("main.py"))
                .map(Value::String)
                .collect(),
        );
        rows.push(row);
    }
    for row in rows {
        let id = row["id"].as_str().unwrap();
        let root = tempfile::tempdir().unwrap();
        for main in strings(&row["mains"]) {
            write_main(root.path(), &main);
        }
        write_pyproject(root.path(), row["scripts"].as_object().unwrap());
        let report = verify_profile_final(root.path(), "python-cli", row["goal"].as_str().unwrap());
        let reasons = report.profile_failures;
        match row["expect"].as_str().unwrap() {
            "pass" => assert!(reasons.is_empty(), "{id}: {reasons:?}"),
            _ => {
                assert_eq!(reasons.len(), 1, "{id}: {reasons:?}");
                for needle in strings(&row["reason_contains"]) {
                    assert!(reasons[0].contains(&needle), "{id}: {reasons:?}");
                }
            }
        }
    }
}
