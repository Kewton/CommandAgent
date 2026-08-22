use std::path::{Path, PathBuf};

use clap::Parser;
use commandagent::config::Provider;
use commandagent::planner::profile_manifest::ManifestStatus;
use commandagent::workflow::schema::{Workflow, WorkflowVersion};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/corpus/apps/issue253-workflow-v02/fixtures")
        .join(name)
}

fn parse_fixture(name: &str) -> Result<Workflow, String> {
    let source = std::fs::read_to_string(fixture(name)).unwrap();
    Workflow::parse(&source).map_err(|error| error.to_string())
}

#[test]
fn v02_fixture_accepts_registered_draft_and_paired_planner_pins() {
    let extension_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/corpus/apps/issue117-draft-profile/extension-root");
    commandagent::planner::extension_profiles::register(&extension_root).unwrap();

    let workflow = parse_fixture("v02-draft-planner.yaml").unwrap();
    assert_eq!(workflow.version, WorkflowVersion::V0_2);
    assert_eq!(
        commandagent::planner::extension_profiles::find("static-site")
            .unwrap()
            .status(),
        ManifestStatus::Draft
    );
    let node = &workflow.nodes["investigate"];
    assert_eq!(node.planner_model.as_deref(), Some("small-planner"));
    assert_eq!(node.planner_provider, Some(Provider::Gemini));

    let error = parse_fixture("negative-draft-v01.yaml").unwrap_err();
    assert!(error.contains("non-admitted profile"), "{error}");
}

#[test]
fn v02_negative_fixtures_keep_pairs_profiles_and_conditions_closed() {
    let cases = [
        (
            "negative-planner-half.yaml",
            "planner_model requires planner_provider",
        ),
        (
            "negative-planner-v01.yaml",
            "planner override requires workflow version 0.2",
        ),
        ("negative-unknown-condition.yaml", "unknown variant"),
        ("negative-unregistered-profile.yaml", "unregistered profile"),
    ];

    for (name, expected) in cases {
        let error = parse_fixture(name).unwrap_err();
        assert!(
            error.contains(expected),
            "fixture={name} expected={expected:?} error={error:?}"
        );
    }
}

#[test]
fn existing_v01_workflow_bytes_still_parse_without_v02_fields() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("workflows/recovery-circle-data.yaml");
    let source = std::fs::read_to_string(path).unwrap();
    let workflow = Workflow::parse(&source).unwrap();

    assert_eq!(workflow.version, WorkflowVersion::V0_1);
    for node in workflow.nodes.values() {
        assert!(node.planner_model.is_none());
        assert!(node.planner_provider.is_none());
    }
}

#[test]
fn verified_v02_draft_circle_is_capped_below_circle_full() {
    let extension_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/corpus/apps/issue117-draft-profile/extension-root");
    let origin = tempfile::tempdir().unwrap();
    let run_dir = origin.path().join(".anvil/runs/origin-run");
    let plans = origin.path().join(".anvil/plans");
    std::fs::create_dir_all(&run_dir).unwrap();
    std::fs::create_dir_all(&plans).unwrap();
    std::fs::write(
        run_dir.join("events.jsonl"),
        concat!(
            r#"{"event":"run_start","action":"Prompt(\"origin goal\")"}"#,
            "\n",
            r#"{"event":"verify_default_bound","bound_checks":["test -f marker"]}"#,
            "\n",
            r#"{"event":"run_stop","status":"failed"}"#,
            "\n"
        ),
    )
    .unwrap();
    std::fs::write(plans.join("recovery-origin.yaml"), "version: 1\n").unwrap();
    let cli = commandagent::cli::Cli::parse_from([
        "commandagent",
        "--cwd",
        origin.path().to_str().unwrap(),
        "--extension-root",
        extension_root.to_str().unwrap(),
        "--model",
        "global-model",
        "--provider",
        "ollama",
        "--ultra-plan-run",
        "goal",
    ]);
    let config = commandagent::config::Config::from_cli(cli).unwrap();

    commandagent::workflow::orchestrator::run_workflow(
        &config,
        &fixture("v02-draft-terminal-cap.yaml"),
        origin.path(),
    )
    .unwrap();

    let circle: serde_json::Value = serde_json::from_slice(
        &std::fs::read(origin.path().join("evidence/workflow-circle.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(circle["verdict"], "circle_failed");
    assert_eq!(circle["reason"], "profile_not_admitted");
    let events =
        std::fs::read_to_string(origin.path().join("evidence/workflow-events.jsonl")).unwrap();
    assert!(events.contains(r#""verdict":"circle_failed""#));
    assert!(events.contains(r#""reason":"profile_not_admitted""#));
    assert!(!events.contains(r#""verdict":"circle_full""#));
}
