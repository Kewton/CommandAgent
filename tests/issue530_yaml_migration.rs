//! Issue #530 — the YAML parser migration compatibility corpus.
//!
//! `serde_yaml` was migrated from the unmaintained 0.9 (which pulled in
//! `unsafe-libyaml` and RUSTSEC-2024-0449) to the maintained `yaml_serde`
//! 0.10.7 (`libyaml-rs`). Because the crate name stays `serde_yaml`, no
//! production source changed. This integration test drives the public API over
//! the pre-migration measurements recorded in `fixtures/compatibility.json` and
//! requires the same accept/reject decisions, types, tags, mapping element
//! counts, and diagnostic line/column.
//!
//! The corpus fixture lives under `tests/corpus/apps/` so the generated-app
//! corpus regression discovers it; `expectations.toml` pins the fixture tokens.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

use commandagent::planner::pack::parse_bytes;
use commandagent::planner::plan::validate_plan_file;
use commandagent::sensitive_data::{REDACTED, SecretCatalog};
use commandagent::workflow::schema::Workflow;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Compatibility {
    pack_cases: Vec<PackCase>,
    plan_cases: Vec<PlanCase>,
    workflow_versions: Vec<WorkflowCase>,
    scrub_cases: Vec<ScrubCase>,
    boundary: Boundary,
}

#[derive(Debug, Deserialize)]
struct PackCase {
    id: String,
    document: String,
    parser_accepts: bool,
    schema_accepts: bool,
    #[serde(default)]
    schema_error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PlanCase {
    id: String,
    document: String,
    accepts: bool,
    #[serde(default)]
    error_contains: Option<String>,
    #[serde(default)]
    line: Option<usize>,
    #[serde(default)]
    column: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct WorkflowCase {
    id: String,
    document: String,
    accepts: bool,
    #[serde(default)]
    error_contains: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ScrubCase {
    id: String,
    register_plain: Vec<String>,
    document: String,
    mapping_len: usize,
    #[serde(default)]
    absent: Vec<String>,
    #[serde(default)]
    serialized_contains: Vec<String>,
    #[serde(default)]
    string_value: BTreeMap<String, String>,
    #[serde(default)]
    number_value: BTreeMap<String, i64>,
}

#[derive(Debug, Deserialize)]
struct Boundary {
    max_file_bytes: usize,
    file_error: String,
    max_depth: usize,
    depth_error: String,
    max_sequence: usize,
    sequence_error: String,
    max_scalar_bytes: usize,
    scalar_error: String,
}

fn fixture() -> Compatibility {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/corpus/apps/issue530-yaml-migration/fixtures/compatibility.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("invalid {}: {error}", path.display()))
}

/// The business schema must keep rejecting exactly what the YAML parser itself
/// may (or may not) accept; the two decisions are recorded separately and must
/// not collapse into one.
#[test]
fn pack_acceptance_keeps_parser_and_schema_decisions_separate() {
    let compatibility = fixture();
    assert!(
        !compatibility.pack_cases.is_empty(),
        "no pack cases recorded"
    );
    for case in &compatibility.pack_cases {
        let parser = serde_yaml::from_str::<serde_yaml::Value>(&case.document);
        assert_eq!(
            parser.is_ok(),
            case.parser_accepts,
            "{}: YAML parser accept/reject changed; error={:?}",
            case.id,
            parser.err().map(|error| error.to_string())
        );

        let schema = parse_bytes(Some(case.document.as_bytes()), None);
        assert_eq!(
            schema.is_ok(),
            case.schema_accepts,
            "{}: pack schema accept/reject changed; error={:?}",
            case.id,
            schema.as_ref().err().map(ToString::to_string)
        );

        if let Some(expected) = &case.schema_error {
            let error = schema.expect_err("negative case must reject").to_string();
            assert!(error.contains(expected), "{}: got {error}", case.id);
        }
    }
}

#[test]
fn plan_diagnostics_keep_reason_and_position() {
    let compatibility = fixture();
    for case in &compatibility.plan_cases {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("plan.yaml");
        std::fs::File::create(&path)
            .and_then(|mut file| file.write_all(case.document.as_bytes()))
            .unwrap();

        let result = validate_plan_file(&path, workspace.path());
        assert_eq!(
            result.is_ok(),
            case.accepts,
            "{}: plan accept/reject changed",
            case.id
        );
        let Err(error) = result else {
            continue;
        };
        let error = error.to_string();
        assert!(
            error.contains(&format!(
                ":{}:{}:",
                case.line.expect("rejected case must record a line"),
                case.column.expect("rejected case must record a column")
            )),
            "{}: diagnostic position lost; {error}",
            case.id
        );
        let expected = case
            .error_contains
            .as_deref()
            .expect("rejected case reason");
        assert!(error.contains(expected), "{}: got {error}", case.id);
    }
}

#[test]
fn workflow_versions_keep_the_numeric_value_contract() {
    let compatibility = fixture();
    for case in &compatibility.workflow_versions {
        let result = Workflow::parse(&case.document);
        assert_eq!(result.is_ok(), case.accepts, "{}", case.id);
        if let Some(expected) = &case.error_contains {
            let error = result.expect_err("negative case must reject").to_string();
            assert!(error.contains(expected), "{}: got {error}", case.id);
        }
    }
}

#[test]
fn scrub_contract_keeps_type_and_mapping_element_count() {
    let compatibility = fixture();
    assert!(
        !compatibility.scrub_cases.is_empty(),
        "no scrub cases recorded"
    );
    for case in &compatibility.scrub_cases {
        let mut catalog = SecretCatalog::new();
        for value in &case.register_plain {
            catalog.register_plain(value).unwrap();
        }
        let mut value: serde_yaml::Value = serde_yaml::from_str(&case.document)
            .unwrap_or_else(|error| panic!("{}: {error}", case.id));
        catalog.scrub_yaml(&mut value);

        let mapping = value.as_mapping().unwrap();
        assert_eq!(
            mapping.len(),
            case.mapping_len,
            "{}: element count",
            case.id
        );

        let serialized = serde_yaml::to_string(&value).unwrap();
        for absent in &case.absent {
            assert!(!serialized.contains(absent), "{}: leaked {absent}", case.id);
        }
        for expected in &case.serialized_contains {
            assert!(
                serialized.contains(expected),
                "{}: missing {expected}; serialized={serialized}",
                case.id
            );
        }
        for (key, expected) in &case.string_value {
            assert_eq!(
                value[key.as_str()].as_str(),
                Some(expected.as_str()),
                "{}: string value {key}",
                case.id
            );
        }
        for (key, expected) in &case.number_value {
            assert_eq!(
                value[key.as_str()].as_i64(),
                Some(*expected),
                "{}: number value {key}",
                case.id
            );
        }
    }
    assert_eq!(REDACTED, "<redacted>");
}

#[test]
fn boundary_limits_keep_their_recorded_errors() {
    let boundary = fixture().boundary;

    let oversized = format!("#{}\n", "x".repeat(boundary.max_file_bytes + 64));
    let error = parse_bytes(Some(oversized.as_bytes()), None)
        .unwrap_err()
        .to_string();
    assert!(error.contains(&boundary.file_error), "{error}");

    let mut deep = String::from("a:\n");
    for level in 1..=boundary.max_depth + 2 {
        deep.push_str(&" ".repeat(level * 2));
        deep.push_str("a:\n");
    }
    let error = parse_bytes(Some(deep.as_bytes()), None)
        .unwrap_err()
        .to_string();
    assert!(error.contains(&boundary.depth_error), "{error}");

    let mut sequence = String::from("a:\n");
    for _ in 0..boundary.max_sequence + 8 {
        sequence.push_str("  - x\n");
    }
    let error = parse_bytes(Some(sequence.as_bytes()), None)
        .unwrap_err()
        .to_string();
    assert!(error.contains(&boundary.sequence_error), "{error}");

    let scalar = format!("a: {}\n", "x".repeat(boundary.max_scalar_bytes + 16));
    let error = parse_bytes(Some(scalar.as_bytes()), None)
        .unwrap_err()
        .to_string();
    assert!(error.contains(&boundary.scalar_error), "{error}");
}
