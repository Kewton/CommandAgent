use commandagent::planner::profile_manifest::{
    ArtifactCardinality, MANIFEST_V1_SECTIONS, ManifestStatus, ManifestV1, nextjs_manifest,
};

#[test]
fn embedded_nextjs_manifest_loads_and_resolves_every_check() {
    let manifest = nextjs_manifest();

    assert_eq!(manifest.metadata.id, "nextjs");
    assert_eq!(manifest.metadata.display_name, "Next.js");
    assert_eq!(manifest.metadata.schema_version.as_str(), "v1");
    assert_eq!(manifest.metadata.status, ManifestStatus::Admitted);
    assert_eq!(
        manifest
            .plan
            .phases
            .iter()
            .map(|phase| phase.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "project-setup",
            "core-implementation",
            "contract-wiring",
            "build-verification",
        ]
    );

    let resolved = manifest.resolve().expect("all check bindings must resolve");
    assert_eq!(resolved.len(), 3);
    assert_eq!(resolved.values().map(Vec::len).sum::<usize>(), 7);
}

#[test]
fn manifest_v1_root_sections_match_golden() {
    let actual = format!("{}\n", MANIFEST_V1_SECTIONS.join("\n"));
    assert_eq!(
        actual,
        include_str!("golden/profile_manifest_v1_sections.txt")
    );
}

#[test]
fn direct_v1_parse_error_exposes_the_toml_source() {
    let error = ManifestV1::from_toml("[metadata]\n[metadata]\n").unwrap_err();
    let source = std::error::Error::source(&error).expect("TOML parse source must be preserved");

    assert!(source.downcast_ref::<toml::de::Error>().is_some());
    assert!(source.to_string().contains("duplicate key `metadata`"));
}

#[test]
fn v1_represents_both_artifact_group_cardinalities_and_rejects_v0() {
    let nextjs = nextjs_manifest();
    assert_eq!(
        nextjs.artifacts.groups[0].cardinality,
        ArtifactCardinality::EitherOf
    );
    let data = commandagent::planner::profiles::data::manifest::get();
    assert_eq!(
        data.artifacts.groups[0].cardinality,
        ArtifactCardinality::ExactlyOneOf
    );
    let v0 = include_str!("../src/planner/profiles/data/manifest.toml").replacen(
        "schema_version = \"v1\"",
        "schema_version = \"v0\"",
        1,
    );
    assert!(ManifestV1::from_toml(&v0).is_err());
}

#[test]
fn v1_rejects_invalid_artifact_groups_and_guidance_triggers() {
    let data = include_str!("../src/planner/profiles/data/manifest.toml");
    let bad_preferred = data.replacen(
        "preferred = \"output/report.md\"",
        "preferred = \"output/report.txt\"",
        1,
    );
    assert!(ManifestV1::from_toml(&bad_preferred).is_err());

    let bad_always = data.replacen(
        "triggers = [{ condition = \"always\" }]",
        "triggers = [{ condition = \"always\", values = [\"unexpected\"] }]",
        1,
    );
    assert!(ManifestV1::from_toml(&bad_always).is_err());
}
