use super::*;

const VALID_ASSIST: &[u8] = br#"schema_version: commandagent.pack.assist/v0
pack:
  id: ingest-default
  version: 1.0.0
  profile: ingest
  intent: create
inject:
  - point: declare-ingest-inspection
    source: ingest_snapshot_structure_injected
    required: true
    params:
      max_files: 8
literals:
  - gate: ingest_source_binding
    example:
      format: json
      value: '{"date":"2026-08-03"}'
vocabulary:
  - point: implement-ingest-delivery
    source: ingest_candidate_ids_injected
    mode: verbatim
    required: true
    params: {}
"#;

const VALID_EVAL: &[u8] = br#"schema_version: commandagent.pack.eval/v0
pack:
  id: ingest-default
  version: 1.0.0
  profile: ingest
  intent: create
checks:
  - id: ingest_source_binding
    at:
      kind: final_acceptance
    extraction:
      - source_binding.source_values
    normalizers:
      - identity
    params: {}
schemas:
  - artifact: output/records.json
    format: json
    root: array
    fields:
      - name: name
        type: string
        required: true
    additional_fields: false
"#;

#[test]
fn strict_loader_accepts_registered_closed_vocabulary() {
    let pack = parse_bytes(Some(VALID_ASSIST), Some(VALID_EVAL)).unwrap();
    assert_eq!(pack.id(), "ingest-default");
    assert_eq!(pack.identity.profile, PackProfile::Ingest);
    assert_eq!(pack.hash.len(), "sha256:".len() + 64);
    assert_eq!(pack.assist.as_ref().unwrap().inject.len(), 1);
    assert_eq!(pack.eval.as_ref().unwrap().checks.len(), 1);
}

#[test]
fn exact_byte_hash_changes_for_formatting_and_pins_absence() {
    let first = exact_byte_hash(Some(VALID_ASSIST), None);
    let mut changed = VALID_ASSIST.to_vec();
    changed.push(b'\n');
    assert_ne!(first, exact_byte_hash(Some(&changed), None));
    assert_eq!(first, exact_byte_hash(Some(VALID_ASSIST), Some(&[])));
}

#[test]
fn decoder_rejects_unknown_duplicate_and_yaml_extension_keys() {
    for (needle, replacement) in [
        ("inject:", "invented: true\ninject:"),
        (
            "  id: ingest-default",
            "  id: ingest-default\n  id: duplicate",
        ),
        (
            "  id: ingest-default",
            "  id: &identity ingest-default\ncopy: *identity",
        ),
        ("inject:", "<<: {inject: []}\ninject:"),
        ("inject:", "!custom\ninject:"),
    ] {
        let raw =
            String::from_utf8(VALID_ASSIST.to_vec())
                .unwrap()
                .replacen(needle, replacement, 1);
        assert!(parse_bytes(Some(raw.as_bytes()), None).is_err(), "{raw}");
    }
}

#[test]
fn decoder_rejects_invented_ids_and_invalid_source_point_pairs() {
    let unknown = String::from_utf8(VALID_ASSIST.to_vec())
        .unwrap()
        .replace("ingest_source_binding", "invented_gate");
    assert!(parse_bytes(Some(unknown.as_bytes()), None).is_err());

    let wrong_point = String::from_utf8(VALID_ASSIST.to_vec())
        .unwrap()
        .replace("declare-ingest-inspection", "cli-implementation");
    assert!(parse_bytes(Some(wrong_point.as_bytes()), None).is_err());

    let wrong_owner = String::from_utf8(VALID_ASSIST.to_vec())
        .unwrap()
        .replace("profile: ingest", "profile: python-cli");
    assert!(parse_bytes(Some(wrong_owner.as_bytes()), None).is_err());
}

#[test]
fn eval_check_ids_resolve_through_capability_or_intent_registries() {
    let fix = br#"schema_version: commandagent.pack.eval/v0
pack:
  id: fix-floor
  version: 1.0.0
  profile: data
  intent: fix
checks:
  - id: before_fails
    at:
      kind: stage
      id: before
    params: {}
"#;
    assert!(parse_bytes(None, Some(fix)).is_ok());

    let invented = String::from_utf8(fix.to_vec())
        .unwrap()
        .replace("before_fails", "invented_requirement");
    assert!(parse_bytes(None, Some(invented.as_bytes())).is_err());
}

#[test]
fn load_directory_rejects_empty_and_non_regular_pack_files() {
    let root = tempfile::tempdir().unwrap();
    assert!(matches!(load_directory(root.path()), Err(PackError::Empty)));

    std::fs::create_dir(root.path().join(ASSIST_FILE)).unwrap();
    assert!(matches!(
        load_directory(root.path()),
        Err(PackError::NotRegularFile { .. })
    ));
}

#[test]
fn strict_decoder_rejects_multiple_documents_and_non_string_keys() {
    let multiple = [VALID_ASSIST, b"\n---\n{}\n"].concat();
    assert!(parse_bytes(Some(&multiple), None).is_err());

    let non_string = String::from_utf8(VALID_ASSIST.to_vec())
        .unwrap()
        .replace("params:\n      max_files: 8", "params:\n      7: 8");
    assert!(parse_bytes(Some(non_string.as_bytes()), None).is_err());
}
