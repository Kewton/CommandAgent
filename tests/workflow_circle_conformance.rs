use commandagent::workflow::evidence::{
    EdgeCheck, EdgeChecks, EdgeRecord, NodeRunReference, OriginReference, WorkflowCircleEvidence,
};
use commandagent::workflow::runner::{EdgeEvidence, circle_adjudication, edge_earned};
use commandagent::workflow::schema::{Route, Verdict};
use std::fs;

fn route() -> Route {
    Route {
        from: "i".into(),
        on: Verdict::Full,
        when: None,
        to: "f".into(),
        carry: vec![],
    }
}
fn evidence() -> EdgeEvidence {
    EdgeEvidence {
        verdict: Verdict::Full,
        evidence: true,
        adjudicated: true,
        epoch: 2,
        previous_epoch: 1,
        carry_present: true,
    }
}

#[test]
fn earned_edge_positive() {
    assert!(edge_earned(&route(), "i_to_f", &evidence()).is_ok());
}
#[test]
fn label_only_rejected() {
    assert!(
        edge_earned(
            &route(),
            "i_to_f",
            &EdgeEvidence {
                evidence: false,
                ..evidence()
            }
        )
        .is_err()
    );
}
#[test]
fn lineage_or_carry_rejected() {
    assert!(
        edge_earned(
            &route(),
            "i_to_f",
            &EdgeEvidence {
                carry_present: false,
                ..evidence()
            }
        )
        .is_err()
    );
}
#[test]
fn epoch_reversal_rejected() {
    assert!(
        edge_earned(
            &route(),
            "i_to_f",
            &EdgeEvidence {
                epoch: 1,
                ..evidence()
            }
        )
        .is_err()
    );
}
#[test]
fn fix_full_does_not_close_circle() {
    assert_eq!(circle_adjudication(false, None).0, "circle_failed");
}
#[test]
fn verify_origin_closes_circle() {
    assert_eq!(circle_adjudication(true, None).0, "circle_full");
}

fn complete_circle_fixture() -> (tempfile::TempDir, WorkflowCircleEvidence) {
    let temp = tempfile::tempdir().unwrap();
    let origin = temp.path().join("origin");
    let origin_run = origin.join(".anvil/runs/origin-run");
    let plans = origin.join(".anvil/plans");
    fs::create_dir_all(&origin_run).unwrap();
    fs::create_dir_all(&plans).unwrap();
    let origin_events = origin_run.join("events.jsonl");
    let recovery = plans.join("recovery-origin.yaml");
    fs::write(
        &origin_events,
        "{\"event\":\"run_stop\",\"status\":\"failed\"}\n",
    )
    .unwrap();
    fs::write(&recovery, "version: 1\n").unwrap();
    let mut circle = WorkflowCircleEvidence::new(
        "recovery-circle-data",
        OriginReference {
            workspace_root: origin.clone(),
            run_id: "origin-run".into(),
            events_path: origin_events,
            recovery_yaml_paths: vec![recovery],
            goal: Some("origin goal".into()),
        },
    );
    circle.record_edge(EdgeRecord {
        edge: "create->investigate".into(),
        fired: true,
        checks: EdgeChecks {
            verdict: EdgeCheck::passed("failed origin verdict matches route"),
            evidence: EdgeCheck::passed("run_stop and recovery evidence exist"),
            epoch: EdgeCheck::passed("fresh target follows origin"),
            carry: EdgeCheck::passed("workspace and recovery YAML exist"),
        },
    });
    let run_id = uuid::Uuid::now_v7().to_string();
    let run_dir = origin.join(".anvil/runs").join(&run_id);
    fs::create_dir_all(&run_dir).unwrap();
    let events_path = run_dir.join("events.jsonl");
    fs::write(&events_path, "{\"event\":\"workflow_node_run_created\"}\n").unwrap();
    circle
        .record_node(
            "investigate",
            NodeRunReference {
                intent: "investigate".into(),
                run_id,
                run_dir,
                events_path,
            },
        )
        .unwrap();
    circle.adjudicate("circle_failed", Some("node_failed:investigate"));
    (temp, circle)
}

#[test]
fn circle_evidence_records_origin_edges_and_actual_node_mapping() {
    let (temp, circle) = complete_circle_fixture();
    let output = temp.path().join("workflow-circle.json");
    circle.write_to(&output).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();

    assert_eq!(value["origin"]["run_id"], "origin-run");
    assert!(value["edges"][0]["checks"]["E-A"]["passed"].as_bool() == Some(true));
    let node = &value["nodes"]["investigate"];
    assert_ne!(node["run_id"], "investigate");
    assert_eq!(
        node["run_dir"]
            .as_str()
            .and_then(|path| std::path::Path::new(path).file_name())
            .and_then(|name| name.to_str()),
        node["run_id"].as_str()
    );
}

#[test]
fn incomplete_circle_evidence_aborts_before_creating_json() {
    let (temp, mut circle) = complete_circle_fixture();
    circle.edges[0].checks.evidence.detail.clear();
    let output = temp.path().join("workflow-circle.json");

    assert_eq!(
        circle.write_to(&output).unwrap_err(),
        "incomplete workflow edge check details"
    );
    assert!(!output.exists());
}

#[test]
fn missing_origin_binding_or_actual_run_identity_is_rejected() {
    let (_temp, mut circle) = complete_circle_fixture();
    circle.origin.recovery_yaml_paths.clear();
    assert_eq!(
        circle.validate().unwrap_err(),
        "incomplete origin binding reference"
    );

    let (_temp, mut circle) = complete_circle_fixture();
    let node = circle.nodes.get_mut("investigate").unwrap();
    node.run_id = "investigate".into();
    assert_eq!(
        circle.validate().unwrap_err(),
        "incomplete workflow node/run mapping"
    );
}
