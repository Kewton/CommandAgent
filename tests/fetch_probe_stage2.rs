use std::fs;
use std::path::{Path, PathBuf};

use commandagent::fetch_probe::{FETCH_EVIDENCE_PATH, FETCH_FRESHNESS_PATH};
use commandagent::planner::profiles::ingest::{runtime, stage2};

const FIXTURE_WORKSPACE: &str = "tests/fixtures/fetch-probe/stage2-workspace";
const CONTRACT: &str = "tests/fixtures/fetch-probe/contracts/valid.toml";
const RECORDING: &str = "tests/fixtures/fetch-probe/recordings/success.json";

fn repository_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn prepare(root: &Path) {
    copy_tree(&repository_path(FIXTURE_WORKSPACE), root);
    fs::copy(repository_path(CONTRACT), root.join("fetch.toml")).unwrap();
}

fn run_one(root: &Path, run_id: &str) -> stage2::Stage2Summary {
    prepare(root);
    stage2::run_recorded(root, "fetch.toml", run_id, &repository_path(RECORDING)).unwrap()
}

#[test]
fn recorded_localhost_fixture_reaches_full_three_times() {
    let persistent = std::env::var_os("COMMANDAGENT_FETCH_ACCEPTANCE_ROOT").map(PathBuf::from);
    for index in 1..=3 {
        let temp;
        let root = if let Some(parent) = persistent.as_ref() {
            let root = parent.join(format!("run-{index}"));
            assert!(!root.exists(), "acceptance run already exists: {root:?}");
            fs::create_dir_all(&root).unwrap();
            root
        } else {
            temp = tempfile::tempdir().unwrap();
            temp.path().to_path_buf()
        };
        let summary = run_one(&root, &format!("localhost-recorded-{index:03}"));
        assert_eq!(summary.assurance, runtime::IngestAssurance::Full);
        for id in [
            runtime::N1,
            runtime::N2,
            runtime::N3,
            runtime::N4,
            runtime::N5,
            stage2::N6,
        ] {
            assert_eq!(summary.checks[id], runtime::CheckStatus::Pass, "{id}");
        }
        for path in [
            FETCH_EVIDENCE_PATH,
            FETCH_FRESHNESS_PATH,
            runtime::ASSURANCE_EVIDENCE_PATH,
            stage2::STAGE2_EVIDENCE_PATH,
        ] {
            assert!(root.join(path).is_file(), "missing {path}");
        }
        let fetch: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join(FETCH_EVIDENCE_PATH)).unwrap()).unwrap();
        let entry = &fetch["entries"][0];
        assert_eq!(entry["http_status"], 200);
        assert_eq!(entry["snapshot_path"], "data/snapshots/events.html");
        assert_eq!(
            entry["content_sha256"],
            "f5db7e08f869612e9d4136fc0a511128288ffc45ef35efce35df4d47f0b8399b"
        );
        assert!(entry["fetched_at_utc"].as_str().unwrap().ends_with('Z'));
    }
}

#[test]
fn existing_stage1_manifest_and_runtime_ids_remain_n1_through_n5() {
    assert_eq!(
        commandagent::planner::profiles::ingest::manifest::required_capability_ids(),
        [
            runtime::N1,
            runtime::N2,
            runtime::N3,
            runtime::N4,
            runtime::N5,
        ]
    );
    assert!(!commandagent::planner::profiles::ingest::manifest::is_manifest_check_id(stage2::N6));
}

#[test]
fn stage2_suite_is_fixture_only_and_pins_three_recorded_runs() {
    let path = repository_path("workspace/management/bench/suites/ingest-fetch-stage2.toml");
    let suite: toml::Value = toml::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(suite["suite"]["live_fetch"].as_bool(), Some(false));
    assert_eq!(
        suite["suite"]["public_site_selected"].as_bool(),
        Some(false)
    );
    assert_eq!(suite["acceptance"]["network_in_ci"].as_bool(), Some(false));
    assert_eq!(suite["acceptance"]["required_runs"].as_integer(), Some(3));
    assert_eq!(suite["fixture_runs"].as_array().unwrap().len(), 3);
    assert_eq!(
        suite["fetch_probe"]["stage_order"].as_array().unwrap(),
        &[
            toml::Value::String("fetch".to_string()),
            toml::Value::String("ingest_fetch_freshness".to_string()),
            toml::Value::String("existing_n1_through_n5".to_string()),
        ]
    );
}
