use std::path::{Path, PathBuf};

use commandagent::planner::profile::{ProfileRuntimeRegistry, verify_profile_final};
use commandagent::planner::profile_manifest::ManifestStatus;
use commandagent::planner::profile_manifest::source::load_extension_manifests;

const MANIFEST_HASH: &str =
    "sha256:ebe5c468d9ed2c030d53109a8891dd3351680cb6519758e7a7dff35c80c2ccb7";

fn run_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("workspace/management/runs/20260820-bp1-one-cell")
}

#[test]
fn measured_landing_page_cell_loads_and_verifies_as_draft() {
    let manifests = load_extension_manifests(&run_root().join("extension-root")).unwrap();
    assert_eq!(manifests.len(), 1);
    assert_eq!(manifests[0].id(), "landing-page");
    assert_eq!(manifests[0].status(), ManifestStatus::Draft);
    assert_eq!(manifests[0].hash(), Some(MANIFEST_HASH));
    assert!(manifests[0].warnings.is_empty());

    let descriptors = commandagent::planner::extension_profiles::extension_descriptors(
        &run_root().join("extension-root"),
    )
    .unwrap();
    assert_eq!(descriptors.len(), 1);
    let profile = commandagent::planner::extension_profiles::find("landing-page").unwrap();
    assert_eq!(profile.status(), ManifestStatus::Draft);
    assert_eq!(profile.assurance_ceiling(), "static");
    assert_eq!(profile.base_profile, None);
    assert!(ProfileRuntimeRegistry::registered().any(|id| id.as_str() == "landing-page"));

    let report = verify_profile_final(
        &run_root().join("workspace"),
        "landing-page",
        "Create a landing page",
    );
    assert!(report.is_pass(), "{}", report.primary_reason());
}
