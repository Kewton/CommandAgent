use commandagent::verification_spec::{
    PROMPT_VERSION, SCHEMA_VERSION, ShadowFailureKind, ShadowGeneration, VerificationIntent,
    observe_shadow, parse_provider_spec, provider_failure, write_shadow_artifact,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const CREATE: &str = include_str!("fixtures/verification_spec_v0/create.json");
const FIX: &str = include_str!("fixtures/verification_spec_v0/fix.json");
const INVESTIGATE: &str = include_str!("fixtures/verification_spec_v0/investigate.json");
const UNKNOWN: &str = include_str!("fixtures/verification_spec_v0/unknown.json");
const SHADOW_PROVIDER_UNAVAILABLE: &str =
    include_str!("fixtures/verification_spec_v0/shadow-provider-unavailable.json");
const SCHEMA: &str = include_str!("../eval/goal_verify/v0/verification-spec.schema.json");
const PROMPT: &str = include_str!("../eval/goal_verify/v0/verification-spec.prompt.txt");

#[test]
fn create_golden_preserves_original_goal_and_tracks_provider_provenance() {
    let goal = "Create a CLI that prints hello.";
    let spec = parse_provider_spec(goal, VerificationIntent::Create, CREATE).unwrap();
    assert_eq!(spec.goal, goal);
    assert_eq!(
        spec.goal_sha256,
        "5925fc67f0c9a3ad14ef8d0b75e16e30ad9dc0e6fd6a7e02044ff85f322327aa"
    );
    assert_eq!(
        spec.provenance.provider_goal_sha256,
        "8174cbde165b981d371b1c7bdab54379629daef3947ac18996a58e5b9c39035a"
    );
    assert!(!spec.provenance.provider_goal_matched);
    assert_eq!(spec.provenance.source, "caller_original_goal");
}

#[test]
fn fix_and_investigate_goldens_are_valid() {
    let fix = parse_provider_spec(
        "Fix parser crash for fixtures/a.json.",
        VerificationIntent::Fix,
        FIX,
    )
    .unwrap();
    assert_eq!(fix.claims[0].id, "same-reproducer");
    let investigate = parse_provider_spec(
        "Investigate timeout in src/worker.rs.",
        VerificationIntent::Investigate,
        INVESTIGATE,
    )
    .unwrap();
    assert_eq!(investigate.oracles[0].id, "source-snapshot");
}

#[test]
fn unknown_golden_is_rejected_instead_of_coerced_to_create() {
    let error = parse_provider_spec(
        "Do something ambiguous.",
        VerificationIntent::Create,
        UNKNOWN,
    )
    .unwrap_err();
    assert!(error.codes[0].starts_with("schema_invalid:"));
}

#[test]
fn schema_and_prompt_snapshots_pin_v0_contract() {
    let schema: Value = serde_json::from_str(SCHEMA).unwrap();
    assert_eq!(schema["$id"], SCHEMA_VERSION);
    assert_eq!(
        schema["properties"]["prompt_version"]["const"],
        PROMPT_VERSION
    );
    assert_eq!(
        schema["properties"]["intent"]["enum"],
        json!(["create", "fix", "investigate"])
    );
    assert_eq!(schema["properties"]["claims"]["maxItems"], 64);
    assert_eq!(schema["properties"]["oracles"]["maxItems"], 64);
    assert!(PROMPT.contains("Never reinterpret or replace the goal."));
    assert!(PROMPT.contains("Reject unknown or composite intent."));
    assert_eq!(
        hex_sha256(SCHEMA.as_bytes()),
        "c08838d8bdf9d2d1caa6426b79c3ccfbcb2234b4ecab0a1ab6e01a126a4b188a"
    );
    assert_eq!(
        hex_sha256(PROMPT.as_bytes()),
        "5097c22466b43adf3326080c2856248f4c774530782f88f6f2a9c0c2784cba47"
    );
}

fn hex_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn duplicate_missing_orphan_and_unbound_errors_are_sorted_and_deterministic() {
    let mut raw: Value = serde_json::from_str(CREATE).unwrap();
    raw["goal"] = json!("Create a CLI that prints hello.");
    let mut claim = raw["claims"][0].clone();
    claim["id"] = json!("dup");
    claim["oracle_ids"] = json!(["missing", "missing"]);
    let mut unbound = claim.clone();
    unbound["oracle_ids"] = json!([]);
    raw["claims"] = json!([claim, unbound]);
    let mut orphan = raw["oracles"][0].clone();
    orphan["id"] = json!("orphan");
    orphan["claim_id"] = json!("dup");
    raw["oracles"] = json!([orphan.clone(), orphan]);
    let error = parse_provider_spec(
        "Create a CLI that prints hello.",
        VerificationIntent::Create,
        &raw.to_string(),
    )
    .unwrap_err();
    assert_eq!(
        error.codes,
        vec![
            "claim_binding_duplicate:dup:missing",
            "claim_id_duplicate:dup",
            "claim_unbound:dup",
            "oracle_claim_binding_unmatched:orphan:dup",
            "oracle_id_duplicate:orphan",
            "oracle_orphan:orphan",
            "oracle_reference_missing:dup:missing",
        ]
    );
}

#[test]
fn unsafe_paths_and_argv_are_rejected() {
    let mut raw: Value = serde_json::from_str(CREATE).unwrap();
    raw["goal"] = json!("Create a CLI that prints hello.");
    raw["oracles"][0]["setup"]["argv"] = json!(["cargo", "bad\narg"]);
    raw["oracles"][0]["setup"]["cwd"] = json!("../escape");
    let error = parse_provider_spec(
        "Create a CLI that prints hello.",
        VerificationIntent::Create,
        &raw.to_string(),
    )
    .unwrap_err();
    assert_eq!(
        error.codes,
        vec!["oracle_argv_unsafe:run-cli:1", "oracle_path_unsafe:run-cli"]
    );
}

#[test]
fn schema_version_counts_and_input_size_are_enforced() {
    let mut no_claims: Value = serde_json::from_str(CREATE).unwrap();
    no_claims["schema_version"] = json!("commandagent.verification_spec.v1");
    no_claims["goal"] = json!("Create a CLI that prints hello.");
    no_claims["claims"] = json!([]);
    no_claims["oracles"] = json!([]);
    let error = parse_provider_spec(
        "Create a CLI that prints hello.",
        VerificationIntent::Create,
        &no_claims.to_string(),
    )
    .unwrap_err();
    assert_eq!(
        error.codes,
        vec![
            "claim_count_invalid",
            "oracle_count_invalid",
            "unsupported_schema_version:commandagent.verification_spec.v1"
        ]
    );
    let oversized = " ".repeat(commandagent::verification_spec::MAX_INPUT_BYTES + 1);
    assert_eq!(
        parse_provider_spec("g", VerificationIntent::Create, &oversized)
            .unwrap_err()
            .codes,
        vec!["input_too_large"]
    );
}

#[test]
fn shadow_generation_failure_cannot_change_authoritative_verdict() {
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Verdict {
        status: &'static str,
        assurance: &'static str,
    }
    let verdict = Verdict {
        status: "complete",
        assurance: "full",
    };
    let observed = observe_shadow(&verdict, "g", VerificationIntent::Create, "not json");
    assert_eq!(observed.authoritative, verdict);
    let ShadowGeneration::Rejected(failure) = observed.generation else {
        panic!("malformed provider response unexpectedly generated a spec");
    };
    assert_eq!(failure.kind, ShadowFailureKind::SchemaInvalid);

    let dir = tempfile::tempdir().unwrap();
    let unavailable = provider_failure(ShadowFailureKind::ProviderUnavailable, "offline");
    let path = write_shadow_artifact(dir.path(), &unavailable).unwrap();
    assert_eq!(path.file_name().unwrap(), "verification-spec-shadow.json");
    let artifact: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(
        artifact,
        serde_json::from_str::<Value>(SHADOW_PROVIDER_UNAVAILABLE).unwrap()
    );
}
