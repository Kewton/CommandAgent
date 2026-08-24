use std::collections::BTreeMap;

use commandagent::planner::profiles::ingest::{manifest, runtime};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Fixture {
    fixture: String,
    checks: BTreeMap<String, runtime::CheckStatus>,
    expected: runtime::IngestAssurance,
    failure_kind: Option<String>,
}

const FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/corpus/apps/test0727_ingest_profile_conformance/fixtures/conformance.jsonl"
);

#[test]
fn fixed_contract_rejects_six_forgery_shapes_and_accepts_full_evidence() {
    let fixtures = std::fs::read_to_string(FIXTURES)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Fixture>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(fixtures.len(), 7);
    assert_eq!(
        fixtures
            .iter()
            .filter(|fixture| fixture.expected == runtime::IngestAssurance::Failed)
            .count(),
        5
    );
    assert_eq!(
        fixtures
            .iter()
            .filter(|fixture| fixture.expected == runtime::IngestAssurance::Static)
            .count(),
        1
    );
    assert_eq!(
        fixtures
            .iter()
            .filter(|fixture| fixture.expected == runtime::IngestAssurance::Full)
            .count(),
        1
    );

    for fixture in fixtures {
        let observed = runtime::classify(&runtime::IngestEvidenceState {
            checks: fixture.checks,
        });
        assert_eq!(observed, fixture.expected, "{}", fixture.fixture);
        if observed != runtime::IngestAssurance::Full {
            assert!(
                fixture
                    .failure_kind
                    .as_deref()
                    .is_some_and(|kind| !kind.trim().is_empty()),
                "{}",
                fixture.fixture
            );
        }
    }
}

#[test]
fn admitted_manifest_binds_exactly_n1_through_n5() {
    assert_eq!(
        manifest::required_capability_ids(),
        [
            runtime::N1,
            runtime::N2,
            runtime::N3,
            runtime::N4,
            runtime::N5,
        ]
    );
    assert_eq!(
        manifest::get().metadata.status,
        commandagent::planner::profile_manifest::ManifestStatus::Admitted
    );
}
