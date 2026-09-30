use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use serde_json::json;

use super::*;

/// Serializes the tests that touch the process-global scope registry, so a
/// `reset_scopes_for_tests` in one cannot clear another's scope (Issue #548:
/// "テストの分離は、同じファイル内の 1 つの lock で直列化する").
static REGISTRY_TEST_LOCK: Mutex<()> = Mutex::new(());

fn registry_guard() -> MutexGuard<'static, ()> {
    REGISTRY_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn catalog(values: &[&str]) -> Arc<SecretCatalog> {
    let mut catalog = SecretCatalog::new();
    for value in values {
        catalog.register(value);
    }
    Arc::new(catalog)
}

#[test]
fn arbitrary_format_canary_is_replaced_everywhere_in_a_nested_value() {
    let catalog = catalog(&["H01_CANARY_JwtStyle_NonPrefix_29486"]);
    let mut value = json!({
        "event": "provider_turn_duration",
        "nested": {"messages": ["prefix H01_CANARY_JwtStyle_NonPrefix_29486 suffix"]},
        "array": [{"tool_args": {"token": "H01_CANARY_JwtStyle_NonPrefix_29486"}}],
    });

    catalog.scrub_value_lenient(&mut value);

    assert!(
        !serde_json::to_string(&value)
            .unwrap()
            .contains("H01_CANARY")
    );
    assert!(!catalog.contains(&serde_json::to_string(&value).unwrap()));
}

#[test]
fn scrub_is_idempotent_and_longest_first() {
    let catalog = catalog(&["abcdefghij", "abcdefgh"]);
    let once = catalog.scrub("x abcdefghij abcdefgh y");
    let twice = catalog.scrub(&once);
    assert_eq!(once, twice);
    assert_eq!(once, "x <redacted> <redacted> y");
}

#[test]
fn unicode_and_duplicate_values_are_handled() {
    let catalog = catalog(&["秘密のトークン値", "秘密のトークン値", "портативное"]);
    assert_eq!(catalog.len(), 2);
    let scrubbed = catalog.scrub("a 秘密のトークン値 b портативное c");
    assert_eq!(scrubbed, "a <redacted> b <redacted> c");
}

#[test]
fn short_credential_values_are_refused_and_plain_short_values_are_not_secrets() {
    let mut catalog = SecretCatalog::new();
    // A credential value shorter than the minimum is refused.
    assert!(matches!(
        catalog.register_credential("VLLM_API_KEY", "short"),
        Err(RegistrationRefusal::TooShort { .. })
    ));
    // A non-credential short value is simply not a secret.
    assert!(catalog.register_plain("3000").is_ok());
    assert!(!catalog.contains("3000"));
    // A non-credential value that reaches the minimum is a secret.
    assert!(catalog.register_plain("abcdefgh").is_ok());
    assert!(catalog.contains("abcdefgh"));
    // A credential value that reaches the minimum is a secret.
    assert!(
        catalog
            .register_credential("OPENAI_API_KEY", "long-credential-value")
            .is_ok()
    );
    assert!(catalog.contains("long-credential-value"));
    // A credential value equal to a reserved marker is refused.
    assert!(matches!(
        catalog.register_credential("X", REDACTED),
        Err(RegistrationRefusal::Reserved { .. })
    ));
}

#[test]
fn debug_output_never_contains_a_value() {
    let catalog = catalog(&["super-secret-value-123"]);
    let debug = format!("{catalog:?}");
    assert!(!debug.contains("super-secret-value-123"), "{debug}");
    let context = RedactionContext::from_catalog(SecretCatalog::new());
    assert!(!format!("{context:?}").contains("super-secret-value-123"));
}

#[test]
fn strict_scrub_refuses_a_dynamic_key_without_leaking_it() {
    let catalog = catalog(&["leak-me-please"]);
    let mut value = json!({"safe": "ok", "leak-me-please": "value"});
    let error = catalog.scrub_value(&mut value).unwrap_err();
    let message = error.to_string();
    assert!(!message.contains("leak-me-please"), "{message}");
    assert!(message.contains("dynamic key"), "{message}");
}

#[test]
fn strict_scrub_passes_ordinary_keys() {
    let catalog = catalog(&["leak-me-please"]);
    let mut value = json!({"safe": "contains leak-me-please"});
    catalog.scrub_value(&mut value).unwrap();
    assert_eq!(value["safe"], "contains <redacted>");
}

#[test]
fn stream_carry_never_exposes_a_secret_split_across_chunks() {
    let catalog = catalog(&["H01_CANARY_JwtStyle_NonPrefix_29486"]);
    let secret = "H01_CANARY_JwtStyle_NonPrefix_29486";
    let text = format!("before {secret} after");
    let mut scrubber = StreamScrubber::new(Arc::clone(&catalog));

    let mut rendered = String::new();
    // Feed one byte at a time to maximise boundary pressure.
    for ch in text.chars() {
        let mut buffer = [0u8; 4];
        rendered.push_str(&scrubber.push(ch.encode_utf8(&mut buffer)));
    }
    rendered.push_str(&scrubber.finish());

    assert_eq!(rendered, "before <redacted> after");
}

#[test]
fn stream_carry_handles_multibyte_and_passes_through_when_empty() {
    let multibyte = catalog(&["秘密のトークン値です"]);
    let mut scrubber = StreamScrubber::new(Arc::clone(&multibyte));
    let mut rendered = String::new();
    for ch in "x 秘密のトークン値です y".chars() {
        let mut buffer = [0u8; 4];
        rendered.push_str(&scrubber.push(ch.encode_utf8(&mut buffer)));
    }
    rendered.push_str(&scrubber.finish());
    assert_eq!(rendered, "x <redacted> y");

    let mut passthrough = StreamScrubber::new(catalog(&[]));
    assert_eq!(passthrough.push("hello"), "hello");
    assert_eq!(passthrough.finish(), "");
}

#[test]
fn contexts_are_isolated_between_scopes() {
    let _guard = registry_guard();
    reset_scopes_for_tests();
    let mut first_catalog = SecretCatalog::new();
    first_catalog.register("alpha-secret-value");
    let mut second_catalog = SecretCatalog::new();
    second_catalog.register("beta-secret-value");
    install_scope(
        first_catalog,
        Some(Path::new("/tmp/workspace-a")),
        Some(Path::new("/tmp/workspace-a/events.jsonl")),
    );
    install_scope(
        second_catalog,
        Some(Path::new("/tmp/workspace-b")),
        Some(Path::new("/tmp/workspace-b/events.jsonl")),
    );

    let resolved_a = active_for(Some(Path::new("/tmp/workspace-a/events.jsonl"))).unwrap();
    let resolved_b = active_for(Some(Path::new("/tmp/workspace-b/events.jsonl"))).unwrap();

    assert!(resolved_a.contains("alpha-secret-value"));
    assert!(!resolved_a.contains("beta-secret-value"));
    assert!(resolved_b.contains("beta-secret-value"));
    assert!(!resolved_b.contains("alpha-secret-value"));
    reset_scopes_for_tests();
}

#[test]
fn active_for_falls_back_to_the_current_scope() {
    let _guard = registry_guard();
    reset_scopes_for_tests();
    let mut catalog = SecretCatalog::new();
    catalog.register("fallback-secret-value");
    install_scope(catalog, Some(Path::new("/tmp/only")), None);

    let resolved = active_for(Some(Path::new("/tmp/unregistered/events.jsonl"))).unwrap();
    assert!(resolved.contains("fallback-secret-value"));
    reset_scopes_for_tests();
}

#[test]
fn runnable_and_identity_refusals_name_the_field_not_the_value() {
    let catalog = catalog(&["H01_CANARY_JwtStyle_NonPrefix_29486"]);
    let yaml = "run: echo H01_CANARY_JwtStyle_NonPrefix_29486\n";
    let refusal = refuse_runnable(&catalog, "recovery.yaml", yaml).unwrap_err();
    assert_eq!(refusal.field, "recovery.yaml");
    assert!(!refusal.to_string().contains("H01_CANARY"));

    let identity = refuse_identity(
        &catalog,
        "confirmation",
        "H01_CANARY_JwtStyle_NonPrefix_29486",
    )
    .unwrap_err();
    assert_eq!(identity.kind, "identity");

    assert!(refuse_runnable(&catalog, "recovery.yaml", "run: echo safe\n").is_ok());
    assert!(refuse_identity(&catalog, "confirmation", "safe-identity").is_ok());
}

#[test]
fn yaml_scrub_rewrites_scalars_and_keys() {
    let catalog = catalog(&["H01_CANARY_JwtStyle_NonPrefix_29486"]);
    let mut value: serde_yaml::Value =
        serde_yaml::from_str("token: H01_CANARY_JwtStyle_NonPrefix_29486\nlist:\n  - a H01_CANARY_JwtStyle_NonPrefix_29486\n")
            .unwrap();
    let text = serde_yaml::to_string(&value).unwrap();
    assert!(runnable_yaml_contains_secret(&catalog, &text));
    catalog.scrub_yaml(&mut value);
    let scrubbed = serde_yaml::to_string(&value).unwrap();
    assert!(!scrubbed.contains("H01_CANARY"), "{scrubbed}");
}

#[test]
fn dotenv_collection_is_bounded_and_skips_symlinks() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".env"),
        "OPENAI_API_KEY=dotenv-secret-value\n",
    )
    .unwrap();
    std::fs::write(dir.path().join(".env.local"), "PORT=3000\nOTHER=abcdefgh\n").unwrap();
    std::fs::write(dir.path().join(".env.example"), "IGNORED=ignored-value\n").unwrap();
    std::fs::write(dir.path().join("notes.txt"), "not a dotenv\n").unwrap();
    std::os::unix::fs::symlink(dir.path().join(".env"), dir.path().join(".env.symlink")).unwrap();

    let collection = collect_scoped_dotenv(dir.path()).unwrap();

    assert!(collection.catalog.contains("dotenv-secret-value"));
    assert!(collection.catalog.contains("abcdefgh"));
    assert!(!collection.catalog.contains("3000"));
    assert!(!collection.catalog.contains("ignored-value"));
    assert_eq!(collection.skipped_symlinks, 1);
}

#[test]
fn dotenv_collection_reports_unreadable_regular_sources() {
    // A directory named like a dotenv source cannot be read as a file: the
    // collector reports the failure instead of silently dropping secrets.
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join(".env.production")).unwrap();
    let failure = collect_scoped_dotenv(dir.path()).unwrap_err();
    assert!(
        failure
            .refusals
            .iter()
            .any(|refusal| matches!(refusal, CollectionRefusal::Read { .. })),
        "{failure:?}"
    );
}

#[test]
fn dotenv_collection_refusal_never_shows_a_value() {
    let canary = "H06_REFUSAL_CANARY_4471";
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".env"),
        format!(
            "PRIVATE_TOKEN={canary}\n{}",
            "#".repeat(MAX_DOTENV_BYTES + 1)
        ),
    )
    .unwrap();

    let failure = collect_scoped_dotenv(dir.path()).unwrap_err();

    assert!(matches!(
        failure.refusals.first(),
        Some(CollectionRefusal::FileTooLarge { .. })
    ));
    assert!(!failure.to_string().contains(canary));
    assert!(!format!("{failure:?}").contains(canary));
    // The head of the file is still catalogued, so a later emit is scrubbed.
    assert!(failure.catalog.contains(canary));
}

#[test]
fn parse_dotenv_matches_the_loader_trim_and_quote_rules() {
    let parsed = parse_dotenv("A=plain\nB = \"quoted\"\nC='single'\n# comment\nD=\n");
    assert_eq!(
        parsed,
        vec![
            ("A".to_string(), "plain".to_string()),
            ("B".to_string(), "quoted".to_string()),
            ("C".to_string(), "single".to_string()),
            ("D".to_string(), String::new()),
        ]
    );
}

#[test]
fn marker_avoids_registered_values_and_stays_idempotent() {
    let mut catalog = SecretCatalog::new();
    catalog.register("redacted");
    catalog.register("H01_CANARY_JwtStyle_NonPrefix_29486");
    let once = catalog.scrub("H01_CANARY_JwtStyle_NonPrefix_29486");
    let twice = catalog.scrub(&once);
    assert_eq!(once, twice, "re-applying the scrub must be idempotent");
    assert!(
        !catalog.contains(&once),
        "marker still holds a value: {once}"
    );
    assert!(
        !once.contains("redacted"),
        "marker contains a value: {once}"
    );

    let mut exact = SecretCatalog::new();
    exact.register(REDACTED);
    assert!(
        !exact.is_empty(),
        "a value equal to the marker is registered"
    );
    let scrubbed = exact.scrub("prefix <redacted> suffix");
    assert!(
        !scrubbed.contains(REDACTED),
        "scrub must not leave the marker-equal value: {scrubbed}"
    );
}

#[test]
fn a_short_credential_value_is_refused_and_changes_nothing() {
    let mut catalog = SecretCatalog::new();
    assert!(matches!(
        catalog.register_credential("PASSWORD", "pass"),
        Err(RegistrationRefusal::TooShort { .. })
    ));
    assert!(catalog.is_empty());
    let original = json!({
        "event": "build_pass",
        "verdict": "pass",
        "schema_version": "commandagent.pass/v1",
        "type": "string",
        "message": "prefix pass suffix",
        "data": {"tool_name": "pass", "pw": 1, "dynamic_pw_key": 2},
    });
    let mut value = original.clone();
    catalog.scrub_value_lenient(&mut value);
    assert_eq!(value, original, "a refused value must not rewrite anything");
}

#[test]
fn a_long_value_that_matches_a_fixed_identifier_preserves_the_schema() {
    let catalog = catalog(&["build_pass", "commandagent.headless-summary/v1"]);
    let mut value = json!({
        "event": "build_pass",
        "schema_version": "commandagent.headless-summary/v1",
        "verdict": "pass",
        "message": "build_pass",
    });
    catalog.scrub_value_lenient(&mut value);
    assert_eq!(value["event"], "build_pass");
    assert_eq!(value["schema_version"], "commandagent.headless-summary/v1");
    assert_eq!(value["verdict"], "pass");
    assert_eq!(value["message"], "<redacted>");
}

#[test]
fn dynamic_keys_and_nested_free_values_are_scrubbed() {
    let catalog = catalog(&["H06_DYNAMIC_VALUE_2718"]);
    let mut value = json!({
        "event": "run_start",
        "data": {
            "message": "prefix H06_DYNAMIC_VALUE_2718 suffix",
            "H06_DYNAMIC_VALUE_2718": 1,
        },
    });
    catalog.scrub_value_lenient(&mut value);
    assert!(
        !serde_json::to_string(&value)
            .unwrap()
            .contains("H06_DYNAMIC_VALUE_2718")
    );
    assert_eq!(value["event"], "run_start");
}

#[test]
fn a_single_char_credential_value_is_refused() {
    let mut catalog = SecretCatalog::new();
    assert!(matches!(
        catalog.register_credential("PASSWORD", "a"),
        Err(RegistrationRefusal::TooShort { .. })
    ));
    assert!(catalog.is_empty());
    let mut value = json!({
        "event": "run_start",
        "schema_version": "v1",
        "data": {"value": "a"},
        "ok": true,
    });
    catalog.scrub_value_lenient(&mut value);
    assert_eq!(value["data"]["value"], "a");
    assert!(value.get("schema_version").is_some());
}

#[test]
fn dynamic_key_with_secret_keeps_element_count() {
    let catalog = catalog(&["H05_KEY_A_secret_8291", "H05_KEY_B_secret_8291"]);
    let mut value = json!({
        "event": "h05_collision",
        "nested": {"H05_KEY_A_secret_8291": 1, "H05_KEY_B_secret_8291": 2},
    });
    catalog.scrub_value_lenient(&mut value);
    assert_eq!(value["nested"].as_object().unwrap().len(), 2);
    assert!(!serde_json::to_string(&value).unwrap().contains("H05_KEY"));
}

#[test]
fn public_yaml_dynamic_keys_keep_element_count() {
    let catalog = catalog(&["H05_KEY_A_secret_8291", "H05_KEY_B_secret_8291"]);
    let mut value: serde_yaml::Value =
        serde_yaml::from_str("H05_KEY_A_secret_8291: 1\nH05_KEY_B_secret_8291: 2\n").unwrap();
    catalog.scrub_yaml(&mut value);
    assert_eq!(value.as_mapping().unwrap().len(), 2);
    assert!(!serde_yaml::to_string(&value).unwrap().contains("H05_KEY"));
}

#[test]
fn yaml_leaves_unrelated_repeated_keys_and_scrubs_composite_keys() {
    let catalog = catalog(&["H06_YAML_CANARY_5521"]);
    let mut value: serde_yaml::Value = serde_yaml::from_str(
        "first:\n  name: one\n  type: string\nsecond:\n  name: two\n  type: string\n",
    )
    .unwrap();
    let original = value.clone();
    catalog.scrub_yaml(&mut value);
    assert_eq!(value, original, "unrelated keys must not be renamed");

    let mut composite: serde_yaml::Value =
        serde_yaml::from_str("? [H06_YAML_CANARY_5521, ordinary]\n: ordinary\n").unwrap();
    catalog.scrub_yaml(&mut composite);
    assert!(
        !serde_yaml::to_string(&composite)
            .unwrap()
            .contains("H06_YAML_CANARY_5521")
    );
}

#[test]
fn stream_scrubber_handles_utf8_boundaries_and_overlaps() {
    let utf8 = catalog(&["éSECRET9"]);
    let mut scrubber = StreamScrubber::new(Arc::clone(&utf8));
    // Must not slice inside a multibyte character.
    let rendered = scrubber.push(&format!("éSECRET9{}", "z".repeat(30)));
    assert!(!rendered.contains("éSECRET9"), "{rendered}");

    let overlap = catalog(&["ABCDEFGHIJKL", "GHIJKLMNOPQRSTUV"]);
    let mut scrubber = StreamScrubber::new(Arc::clone(&overlap));
    let early = scrubber.push("ABCDEFGHIJKLMNOPQRSTUVZZZZ");
    let final_text = format!("{early}{}", scrubber.finish());
    assert!(
        !early.contains("ABCDEF"),
        "overlap leaked a prefix: {early}"
    );
    assert!(!final_text.contains("ABCDEF"), "{final_text}");
}

#[test]
fn dotenv_collection_refuses_over_cap_sources() {
    let large = tempfile::tempdir().unwrap();
    std::fs::write(
        large.path().join(".env"),
        format!("PRIVATE_TOKEN=x\n{}", "#".repeat(MAX_DOTENV_BYTES + 1)),
    )
    .unwrap();
    assert!(
        collect_scoped_dotenv(large.path())
            .unwrap_err()
            .refusals
            .iter()
            .any(|refusal| matches!(refusal, CollectionRefusal::FileTooLarge { .. }))
    );

    let many = tempfile::tempdir().unwrap();
    for index in 0..=MAX_DOTENV_FILES {
        std::fs::write(
            many.path().join(format!(".env.{index:03}")),
            "PRIVATE_TOKEN=x\n",
        )
        .unwrap();
    }
    assert!(
        collect_scoped_dotenv(many.path())
            .unwrap_err()
            .refusals
            .iter()
            .any(|refusal| matches!(refusal, CollectionRefusal::TooManyFiles { .. }))
    );

    let values = tempfile::tempdir().unwrap();
    let mut lines = String::new();
    for index in 0..=MAX_CATALOG_VALUES {
        lines.push_str(&format!("SECRET_{index}=value_{index:04}_long\n"));
    }
    std::fs::write(values.path().join(".env"), lines).unwrap();
    assert!(
        collect_scoped_dotenv(values.path())
            .unwrap_err()
            .refusals
            .iter()
            .any(|refusal| matches!(refusal, CollectionRefusal::TooManyValues { .. }))
    );
}

#[test]
fn combined_merge_refuses_past_the_catalog_cap() {
    let mut catalog = SecretCatalog::new();
    for index in 0..MAX_CATALOG_VALUES {
        catalog.register(&format!("H06_COMBINED_VALUE_{index:04}_only"));
    }
    assert_eq!(catalog.len(), MAX_CATALOG_VALUES);
    let mut extra = SecretCatalog::new();
    extra.register("H06_PROVIDER_EXTRA_4821");
    assert!(matches!(
        catalog.try_merge(&extra),
        Err(RegistrationRefusal::CatalogFull { .. })
    ));
}

#[test]
fn marker_terminal_fallback_contains_no_registered_value() {
    let ascii: String = (1u8..=127).map(char::from).collect();
    let mut fallback = SecretCatalog::new();
    for value in ["<redacted>", "[redacted]", "[hidden]", "«hidden»", &ascii] {
        fallback.register(value);
    }
    // Every listed candidate contains a registered value, so the terminal
    // fallback is chosen; it is still collision-free.
    assert_eq!(fallback.marker(), "\u{fffd}");
    assert!(!fallback.contains(fallback.marker()));

    // A credential value equal to the fallback marker is a short value and is
    // refused, so it can never become a collision.
    let mut refused = SecretCatalog::new();
    assert!(matches!(
        refused.register_credential("PASSWORD", "\u{fffd}"),
        Err(RegistrationRefusal::TooShort { .. })
    ));
    assert!(refused.is_empty());
    assert!(!refused.contains(fallback.marker()));

    // The terminal marker never contains a registered long value either.
    fallback.register("H06_FFFD_contains_\u{fffd}_marker_value");
    assert!(!fallback.contains(fallback.marker()));

    let canary = "H06_MARKER_CANARY_9931";
    fallback.register(canary);
    assert_eq!(
        fallback.scrub(&fallback.scrub(canary)),
        fallback.scrub(canary)
    );
}

#[test]
fn free_input_scrub_ignores_schema_key_names() {
    let canary = "H07_FREE_ARG_CANARY_7741";
    let catalog = catalog(&[canary]);
    let mut free = json!({
        "path": "ordinary.json",
        "status": canary,
        "type": canary,
        "action": canary,
        "tool_name": canary,
        "content": canary,
    });
    catalog.scrub_value_free(&mut free);
    assert!(
        !serde_json::to_string(&free).unwrap().contains(canary),
        "free arguments kept a secret: {free}"
    );

    // The same key names under the schema scrub are fixed identifiers, but a
    // fixed key no longer protects an arbitrary value: only a value that is
    // itself a fixed identifier is preserved (Issue #548 hole 2 / design 4).
    // This replaces the pre-#548 assertion that every value under these keys
    // passed through unchanged.
    let mut schema =
        json!({"status": canary, "type": canary, "action": canary, "tool_name": canary});
    catalog.scrub_value_lenient(&mut schema);
    assert_eq!(schema["status"], "<redacted>");
    assert_eq!(schema["type"], "<redacted>");
    assert_eq!(schema["action"], "<redacted>");
    assert_eq!(schema["tool_name"], "<redacted>");

    // A value that is a fixed identifier is still preserved.
    let mut fixed = json!({"status": "completed", "event": "run_start"});
    catalog.scrub_value_lenient(&mut fixed);
    assert_eq!(fixed["status"], "completed");
    assert_eq!(fixed["event"], "run_start");
}

#[test]
fn composite_yaml_keys_keep_both_elements() {
    let (a, b) = ("H07_YAML_A_SECRET_4412", "H07_YAML_B_SECRET_4412");
    let catalog = catalog(&[a, b]);
    let mut value: serde_yaml::Value = serde_yaml::from_str(&format!(
        "? [{a}, ordinary]\n: one\n? [{b}, ordinary]\n: two\n"
    ))
    .unwrap();

    catalog.scrub_yaml(&mut value);

    assert_eq!(value.as_mapping().unwrap().len(), 2);
    let text = serde_yaml::to_string(&value).unwrap();
    assert!(!text.contains(a), "{text}");
    assert!(!text.contains(b), "{text}");
}

#[test]
fn an_unrelated_marker_key_is_not_renamed_by_a_projection() {
    let secret = "!H07_DYNAMIC_SECRET_9631";
    let catalog = catalog(&[secret]);

    let mut object = serde_json::Map::new();
    object.insert(secret.to_string(), json!(1));
    object.insert("<redacted>".to_string(), json!(2));
    let mut value = serde_json::Value::Object(object);
    catalog.scrub_value_lenient(&mut value);
    let object = value.as_object().unwrap();
    assert_eq!(object.len(), 2);
    assert_eq!(object["<redacted>"], 2, "unrelated key changed: {object:?}");
    assert_eq!(object["<redacted>#1"], 1);
    assert!(!serde_json::to_string(&value).unwrap().contains(secret));

    let mut mapping = serde_yaml::Mapping::new();
    mapping.insert(
        serde_yaml::Value::from(secret),
        serde_yaml::Value::from(1i64),
    );
    mapping.insert(
        serde_yaml::Value::from("<redacted>"),
        serde_yaml::Value::from(2i64),
    );
    let mut value = serde_yaml::Value::Mapping(mapping);
    catalog.scrub_yaml(&mut value);
    let mapping = value.as_mapping().unwrap();
    assert_eq!(mapping.len(), 2);
    assert_eq!(
        mapping
            .get(serde_yaml::Value::from("<redacted>"))
            .and_then(serde_yaml::Value::as_i64),
        Some(2),
        "unrelated YAML key changed: {mapping:?}"
    );
}

#[test]
fn envelope_fixed_key_and_kind_are_preserved() {
    let catalog = catalog(&["evidence_", "ool_parse_fail"]);
    let mut value = json!({
        "capability_id": "tool_parse_failure",
        "evidence_envelope": {
            "envelope_version": 1,
            "family": "tool_parse",
            "kind": "tool_parse_failure",
        },
    });

    catalog.scrub_value_lenient(&mut value);

    let object = value.as_object().unwrap();
    assert!(
        object.contains_key("evidence_envelope"),
        "fixed key renamed: {object:?}"
    );
    assert_eq!(value["evidence_envelope"]["kind"], "tool_parse_failure");
    assert_eq!(value["evidence_envelope"]["family"], "tool_parse");
    assert_eq!(value["evidence_envelope"]["envelope_version"], 1);
}

// ---------------------------------------------------------------------------
// Issue #548 focused tests: the 31-row confirmation table.
//
// Every test is named `c<row>_...` after the row it covers. Each asserts both
// the refusal/redaction and what must survive (event name, key, element count,
// a FIXED value). No registry-touching path is used: the catalog is built
// directly, and the summary tests install a thread-local context with
// `set_current`, so these tests never race on the process-global registry.
// ---------------------------------------------------------------------------

const C1: &str = "H01_CANARY_JwtStyle_NonPrefix_29486";
const F1: [&str; 5] = [
    "completed",
    "incomplete",
    "build_pass",
    "run_start",
    "function",
];
const F3: [&str; 2] = ["ool_parse_fail", "evidence_"];
const M1: [&str; 2] = ["日本語", "秘密の値です"];
const M2: &str = "日本語の秘密の値です";
const S1: [&str; 3] = ["true", "8080", "dev"];
const N1: &str = "8675309012345";
const E1: &str = "H09_Q\"uo\\te_Canary_7731";
const K1A: &str = "H10_YAML_A_SECRET_4412";
const K1B: &str = "H10_YAML_B_SECRET_4412";

fn forced(values: &[&str]) -> Arc<SecretCatalog> {
    let mut catalog = SecretCatalog::new();
    for value in values {
        catalog.register(value);
    }
    Arc::new(catalog)
}

/// Run a body through the public summary writer with `catalog` stalled as the
/// thread-local run scope, and return the rendered summary document.
fn summary_document(catalog: SecretCatalog, body: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("run/events.jsonl");
    set_current(Some(RedactionContext::from_catalog(catalog)));
    crate::eval_events::write_run_summary(Some(&events), body);
    let text =
        std::fs::read_to_string(events.parent().unwrap().join("summary.md")).unwrap_or_default();
    set_current(None);
    text
}

fn append_summary_document(catalog: SecretCatalog, body: &str, appended: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("run/events.jsonl");
    set_current(Some(RedactionContext::from_catalog(catalog)));
    crate::eval_events::write_run_summary(Some(&events), body);
    crate::eval_events::append_run_summary(Some(&events), appended);
    let text =
        std::fs::read_to_string(events.parent().unwrap().join("summary.md")).unwrap_or_default();
    set_current(None);
    text
}

#[test]
fn c01_credential_equal_to_fixed_identifier_is_refused() {
    for value in F1 {
        let mut catalog = SecretCatalog::new();
        let error = catalog
            .register_credential("OPENAI_API_KEY", value)
            .unwrap_err();
        assert!(matches!(
            error,
            RegistrationRefusal::Reserved {
                kind: ReservedKind::FixedIdentifier,
                ..
            }
        ));
        assert!(catalog.is_empty(), "catalog not empty for {value}");
        assert!(!error.to_string().contains(value), "{error}");
        assert!(!format!("{error:?}").contains(value), "{error:?}");
    }
}

#[test]
fn c02_credential_substring_of_fixed_identifier_is_refused() {
    for value in F3 {
        assert!(collides_with_fixed_identifier(value), "{value} not refused");
        let mut catalog = SecretCatalog::new();
        let error = catalog
            .register_credential("PRIVATE_TOKEN", value)
            .unwrap_err();
        assert!(matches!(
            error,
            RegistrationRefusal::Reserved {
                kind: ReservedKind::FixedIdentifier,
                ..
            }
        ));
        assert!(catalog.is_empty());
        assert!(!error.to_string().contains(value), "{error}");
    }
}

#[test]
fn c03_schema_name_and_prefix_are_refused() {
    // The second value is an out-of-list schema; it is built without a source
    // literal so the FIXED completeness scan does not require it to be listed.
    let out_of_list = concat!("commandagent.", "foo/v9");
    for value in ["commandagent.headless-summary/v1", out_of_list] {
        let mut catalog = SecretCatalog::new();
        let error = catalog.register_credential("API_KEY", value).unwrap_err();
        assert!(matches!(
            error,
            RegistrationRefusal::Reserved {
                kind: ReservedKind::FixedIdentifier,
                ..
            }
        ));
        assert!(catalog.is_empty());
        assert!(!error.to_string().contains(value), "{error}");
    }
}

#[test]
fn c04_multibyte_length_counts_chars() {
    for value in M1 {
        assert!(value.len() >= MIN_GENERIC_SECRET_LEN, "not the byte case");
        assert!(value.chars().count() < MIN_GENERIC_SECRET_LEN);
        let mut catalog = SecretCatalog::new();
        let error = catalog
            .register_credential("GATEWAY_KEY", value)
            .unwrap_err();
        assert!(matches!(error, RegistrationRefusal::TooShort { .. }));
        assert!(catalog.is_empty());
    }
}

#[test]
fn c05_multibyte_eight_chars_is_registered() {
    assert!(M2.chars().count() >= MIN_GENERIC_SECRET_LEN);
    let mut catalog = SecretCatalog::new();
    catalog.register_credential("GATEWAY_KEY", M2).unwrap();
    assert!(catalog.contains(M2));
    assert_eq!(catalog.scrub(&format!("x {M2} y")), "x <redacted> y");
}

#[test]
fn c06_plain_value_colliding_with_fixed_is_not_registered() {
    for value in F1.iter().chain(F3.iter()) {
        let mut catalog = SecretCatalog::new();
        catalog.register_plain(value).unwrap();
        assert!(catalog.is_empty(), "{value} registered");
        assert!(!catalog.contains(value));
    }
    assert!(collides_with_fixed_identifier(concat!(
        "commandagent.",
        "foo/v9"
    )));
}

#[test]
fn c07_true_is_not_registered() {
    let mut catalog = SecretCatalog::new();
    for value in S1.iter().chain(M1.iter()) {
        catalog.register_plain(value).unwrap();
    }
    assert!(catalog.is_empty());
    for value in S1.iter().chain(M1.iter()) {
        assert!(!catalog.contains(value), "{value} registered");
    }
    assert!(!catalog.contains("true"));
    // The same ordinary value is a fixed identifier and would be refused as a
    // credential, but as a plain value it is simply not a secret.
    catalog.register_plain("completed").unwrap();
    assert!(catalog.is_empty());
}

#[test]
fn c12_gui_catalog_ignores_the_refusal_and_keeps_the_display_status() {
    // The GUI projection ignores a registration refusal, so a refused value is
    // not registered and a fixed display value is untouched.
    let mut catalog = SecretCatalog::new();
    assert!(
        catalog
            .register_credential("PRIVATE_TOKEN", "completed")
            .is_err()
    );
    assert!(catalog.is_empty());
    let mut display = json!({"status": "completed"});
    catalog.scrub_value_lenient(&mut display);
    assert_eq!(display["status"], "completed");
}

#[test]
fn c13_summary_fixed_status_is_preserved() {
    let mut catalog = SecretCatalog::new();
    catalog.register(C1);
    let summary = summary_document(catalog, &format!("Status: completed\nResult: {C1}\n"));
    assert!(summary.contains("Status: completed"), "{summary}");
    assert!(!summary.contains(C1), "{summary}");
}

#[test]
fn c14_result_and_prefixed_status_free_text_are_scrubbed() {
    let mut catalog = SecretCatalog::new();
    catalog.register(C1);
    let body = format!("Result: {C1}\nStatus: completed {C1}\n  Status: {C1}\nStatus: completed\n");
    let summary = summary_document(catalog, &body);
    assert!(!summary.contains(C1), "{summary}");
    assert!(summary.contains("<redacted>"), "{summary}");
    assert!(summary.contains("Status: completed"), "{summary}");
}

#[test]
fn c15_write_and_append_both_scrub_a_result_line() {
    let mut catalog = SecretCatalog::new();
    catalog.register(C1);
    let summary = append_summary_document(
        catalog,
        &format!("Result: {C1}\nStatus: completed\n"),
        &format!("Result: {C1}\n"),
    );
    assert!(!summary.contains(C1), "{summary}");
    assert!(summary.contains("<redacted>"), "{summary}");
}

#[test]
fn c16_many_fixed_status_lines_keep_their_values() {
    // Eleven fixed values reach placeholder index 10, which is the prefix
    // collision case (`...value1` inside `...value10`); a free line proves an
    // unmatched value is not protected.
    let fixed_values: Vec<&str> = FIXED_IDENTIFIERS
        .iter()
        .copied()
        .filter(|value| value.len() >= 8)
        .take(11)
        .collect();
    let mut body = String::new();
    for value in &fixed_values {
        body.push_str(&format!("Status: {value}\n"));
    }
    body.push_str("Status: s_free_marker\n");
    let mut catalog = SecretCatalog::new();
    catalog.register(C1);
    let summary = summary_document(catalog, &body);
    for value in &fixed_values {
        assert!(
            summary.contains(&format!("Status: {value}")),
            "fixed value {value} was rewritten: {summary}"
        );
    }
    assert!(!summary.contains("commandagentfixedvalue"), "{summary}");
    assert!(summary.contains("Status: s_free_marker"), "{summary}");
}

#[test]
fn c17_lenient_scrubs_free_text_in_fixed_key() {
    let catalog = forced(&[C1]);
    let mut value = json!({"status": format!("failed token={C1}"), "tool_name": C1});
    catalog.scrub_value_lenient(&mut value);
    assert_eq!(value["status"], "failed token=<redacted>");
    assert_eq!(value["tool_name"], "<redacted>");
    assert!(value.get("status").is_some());
    assert!(value.get("tool_name").is_some());
}

#[test]
fn c18_lenient_preserves_fixed_values_in_fixed_keys() {
    let catalog = forced(&F3);
    let mut value = json!({"event": "tool_parse_failure", "status": "completed"});
    catalog.scrub_value_lenient(&mut value);
    assert_eq!(value["event"], "tool_parse_failure");
    assert_eq!(value["status"], "completed");
}

#[test]
fn c19_envelope_fixed_key_and_kind_are_preserved() {
    let catalog = forced(&F3);
    let mut value = json!({
        "evidence_envelope": {
            "envelope_version": 1,
            "family": "tool_parse",
            "kind": "tool_parse_failure",
        },
    });
    catalog.scrub_value_lenient(&mut value);
    assert!(value.get("evidence_envelope").is_some());
    assert_eq!(value["evidence_envelope"]["kind"], "tool_parse_failure");
    assert_eq!(value["evidence_envelope"]["family"], "tool_parse");
}

#[test]
fn c20_strict_scrubs_free_text_in_fixed_key() {
    let catalog = forced(&[C1]);
    let mut value = json!({"status": format!("failed token={C1}"), "tool_name": C1});
    catalog.scrub_value(&mut value).unwrap();
    assert_eq!(value["status"], "failed token=<redacted>");
    assert_eq!(value["tool_name"], "<redacted>");

    // A dynamic key is still refused (not a fixed schema key).
    let mut keyed = json!({C1: "v"});
    let error = catalog.scrub_value(&mut keyed).unwrap_err();
    assert!(!error.to_string().contains(C1), "{error}");
}

#[test]
fn c21_numeric_yaml_secret_is_runnable() {
    let catalog = forced(&[N1]);
    assert!(runnable_yaml_contains_secret(
        &catalog,
        "args:\n  - --token\n  - 8675309012345\n"
    ));
    assert!(runnable_yaml_contains_secret(
        &catalog,
        "port: 8675309012345\n"
    ));
    let refusal = refuse_runnable(&catalog, "recovery.yaml", "port: 8675309012345\n").unwrap_err();
    assert_eq!(refusal.kind, "runnable_yaml");
    assert!(!refusal.to_string().contains(N1), "{refusal}");

    // A number that is not a secret is not a runnable refusal.
    assert!(!runnable_yaml_contains_secret(&catalog, "port: 8080\n"));
}

#[test]
fn c22_numeric_yaml_secret_is_scrubbed() {
    let catalog = forced(&[N1]);
    let mut secret: serde_yaml::Value = serde_yaml::from_str("port: 8675309012345\n").unwrap();
    catalog.scrub_yaml(&mut secret);
    let text = serde_yaml::to_string(&secret).unwrap();
    assert!(!text.contains(N1), "{text}");
    assert!(text.contains("<redacted>"), "{text}");

    let mut ordinary: serde_yaml::Value = serde_yaml::from_str("port: 8080\n").unwrap();
    catalog.scrub_yaml(&mut ordinary);
    let text = serde_yaml::to_string(&ordinary).unwrap();
    assert!(text.contains("port: 8080"), "{text}");
}

#[test]
fn c23_escaped_identity_is_refused() {
    let catalog = forced(&[E1]);
    let encoded = serde_json::to_string(E1).unwrap();
    assert!(encoded.contains("\\\""), "{encoded}");
    assert!(!catalog.contains(&encoded), "raw escaped form matched");
    let refusal = refuse_identity(&catalog, "confirmation", &encoded).unwrap_err();
    assert_eq!(refusal.kind, "identity");
    assert!(!refusal.to_string().contains(E1), "{refusal}");
}

#[test]
fn c24_unescaped_identity_is_refused_and_clean_passes() {
    let catalog = forced(&[C1]);
    assert!(refuse_identity(&catalog, "confirmation", C1).is_err());
    assert!(refuse_identity(&catalog, "confirmation", "safe-identity").is_ok());
}

// Issue #554 rows 17-21: an identity that serializes an object or an array must
// be refused when any key or string value still carries an escaped registered
// secret, and a secret-free serialized identity must pass. The refusal must not
// expose the secret through Display or Debug.
const S17_QUOTE: &str = "H01q\"uote_FAKE_secret_554";
const S17_BACKSLASH: &str = "H01_554_back\\slash_FAKE";
const S17_NEWLINE: &str = "H01_554_new\nline_FAKE";
const S17_PLAIN: &str = "H01_554_plain_FAKE_secret";

fn identity_catalog() -> Arc<SecretCatalog> {
    let mut catalog = SecretCatalog::new();
    for value in [S17_QUOTE, S17_BACKSLASH, S17_NEWLINE, S17_PLAIN] {
        catalog.register_credential("FAKE_KEY", value).unwrap();
    }
    Arc::new(catalog)
}

fn assert_identity_refused_without_leak(catalog: &SecretCatalog, encoded: &str) {
    let refusal = refuse_identity(catalog, "confirmation", encoded).unwrap_err();
    assert_eq!(refusal.kind, "identity");
    let display = refusal.to_string();
    let debug = format!("{refusal:?}");
    for secret in [S17_QUOTE, S17_BACKSLASH, S17_NEWLINE, S17_PLAIN] {
        assert!(
            !display.contains(secret),
            "display leaked {secret:?}: {display}"
        );
        assert!(!debug.contains(secret), "debug leaked {secret:?}: {debug}");
    }
}

#[test]
fn c30_serialized_object_and_array_identity_secrets_are_refused() {
    let catalog = identity_catalog();
    let cases = [
        serde_json::to_string(&json!({"token": S17_QUOTE})).unwrap(),
        serde_json::to_string(&json!(["a", S17_QUOTE])).unwrap(),
        serde_json::to_string(&json!({"a": {"b": [S17_BACKSLASH]}})).unwrap(),
    ];
    for encoded in &cases {
        assert!(
            !catalog.contains(encoded),
            "raw escaped form matched: {encoded}"
        );
        assert_identity_refused_without_leak(&catalog, encoded);
    }
}

#[test]
fn c31_serialized_control_char_and_pretty_identity_secrets_are_refused() {
    let catalog = identity_catalog();
    let control = serde_json::to_string(&json!({"k": S17_NEWLINE})).unwrap();
    assert!(
        !catalog.contains(&control),
        "raw control form matched: {control}"
    );
    assert_identity_refused_without_leak(&catalog, &control);

    let pretty = serde_json::to_string_pretty(&json!({"t": S17_QUOTE})).unwrap();
    assert_identity_refused_without_leak(&catalog, &pretty);
}

#[test]
fn c32_serialized_object_key_identity_secret_is_refused() {
    let catalog = identity_catalog();
    let mut map = serde_json::Map::new();
    map.insert(S17_QUOTE.to_string(), json!("v"));
    let encoded = serde_json::to_string(&serde_json::Value::Object(map)).unwrap();
    assert!(!catalog.contains(&encoded), "raw form matched: {encoded}");
    assert_identity_refused_without_leak(&catalog, &encoded);
}

#[test]
fn c33_unicode_escape_and_double_encoded_identity_secrets_are_refused() {
    let catalog = identity_catalog();
    let unicode_escaped = format!(r#"{{"t":"\u0048{}"}}"#, &S17_PLAIN[1..]);
    assert!(
        !catalog.contains(&unicode_escaped),
        "raw form matched: {unicode_escaped}"
    );
    assert_identity_refused_without_leak(&catalog, &unicode_escaped);

    let inner = serde_json::to_string(&json!({"t": S17_QUOTE})).unwrap();
    let double = serde_json::to_string(&inner).unwrap();
    assert!(
        !catalog.contains(&double),
        "raw double form matched: {double}"
    );
    assert_identity_refused_without_leak(&catalog, &double);
}

#[test]
fn c34_serialized_identity_without_a_secret_is_ok() {
    let catalog = identity_catalog();
    for text in [
        serde_json::to_string(&json!({"goal": "safe", "n": 1})).unwrap(),
        "safe-identity".to_string(),
        r#"{"a":1,"b":true,"c":null}"#.to_string(),
    ] {
        assert!(
            refuse_identity(&catalog, "confirmation", &text).is_ok(),
            "unexpected refusal for {text}"
        );
    }
}

#[test]
fn c25_public_predicates_match_the_constants() {
    assert!(is_fixed_schema_key("status"));
    assert!(!is_fixed_schema_key("not_a_schema_key"));
    assert_eq!(
        is_fixed_schema_key("status"),
        FIXED_SCHEMA_KEYS.contains(&"status")
    );
    assert!(is_fixed_identifier("completed"));
    assert!(!is_fixed_identifier("nope_not_fixed"));
    assert_eq!(
        is_fixed_identifier("completed"),
        FIXED_IDENTIFIERS.contains(&"completed")
    );
}

#[test]
fn c26_yaml_mapping_keys_keep_element_count() {
    let catalog = forced(&[K1A, K1B]);
    let mut value: serde_yaml::Value =
        serde_yaml::from_str(&format!("? {{a: {K1A}}}\n: 1\n? {{a: {K1B}}}\n: 2\n")).unwrap();
    catalog.scrub_yaml(&mut value);
    assert_eq!(value.as_mapping().unwrap().len(), 2);
    let text = serde_yaml::to_string(&value).unwrap();
    assert!(!text.contains(K1A), "{text}");
    assert!(!text.contains(K1B), "{text}");
}

#[test]
fn c27_yaml_tagged_keys_keep_element_count() {
    let catalog = forced(&[K1A, K1B]);
    let mut value: serde_yaml::Value =
        serde_yaml::from_str(&format!("!t {K1A}: 1\n!t {K1B}: 2\n!t '<redacted>': 3\n")).unwrap();
    catalog.scrub_yaml(&mut value);
    let mapping = value.as_mapping().unwrap();
    assert_eq!(mapping.len(), 3);
    let text = serde_yaml::to_string(&value).unwrap();
    assert!(!text.contains(K1A), "{text}");
    assert!(!text.contains(K1B), "{text}");
    // The unrelated tagged key keeps its value.
    let untouched = mapping.iter().any(|(key, item)| {
        matches!(
            key,
            serde_yaml::Value::Tagged(tagged)
                if matches!(&tagged.value, serde_yaml::Value::String(text) if text == "<redacted>")
        ) && matches!(item, serde_yaml::Value::Number(number) if number.as_i64() == Some(3))
    });
    assert!(untouched, "unrelated tagged key changed: {text}");
}

#[test]
fn c29_protocol_name_is_refused_and_builtin_tool_names_are_fixed() {
    let mut catalog = SecretCatalog::new();
    let error = catalog
        .register_credential("TOOL_KIND", "function")
        .unwrap_err();
    assert!(matches!(
        error,
        RegistrationRefusal::Reserved {
            kind: ReservedKind::FixedIdentifier,
            ..
        }
    ));
    assert!(catalog.is_empty());
    for name in [
        "function", "tool", "Bash", "Read", "Write", "Edit", "Glob", "Grep",
    ] {
        assert!(is_fixed_identifier(name), "{name} is not fixed");
    }
}

// --- Structural guards -----------------------------------------------------

fn collect_rust_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, out);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

#[test]
fn c28_fixed_identifiers_cover_every_source_literal() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_rust_files(&root, &mut files);
    assert!(!files.is_empty(), "no source files found");

    let event = regex::Regex::new(r#""event"\s*:\s*"([a-z0-9_]+)""#).unwrap();
    let schema = regex::Regex::new(r#""(commandagent\.[A-Za-z0-9._-]+/v[0-9]+)""#).unwrap();

    let mut missing = Vec::new();
    for path in &files {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        for capture in event.captures_iter(&text) {
            let name = &capture[1];
            if !is_fixed_identifier(name) {
                missing.push(format!("event {name} in {}", path.display()));
            }
        }
        for capture in schema.captures_iter(&text) {
            let name = &capture[1];
            if !is_fixed_identifier(name) {
                missing.push(format!("schema {name} in {}", path.display()));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "fixed identifiers are missing from FIXED_IDENTIFIERS: {missing:?}"
    );
}

#[test]
fn c28b_forced_registration_is_not_called_from_product_code() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_rust_files(&root, &mut files);
    let mut offenses = Vec::new();
    for path in &files {
        if path.file_name().and_then(|name| name.to_str()) == Some("tests.rs") {
            continue; // this unit-test module itself
        }
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        // The test-only section of a module starts at its `mod tests {`.
        let test_start = text.rfind("mod tests").unwrap_or(0);
        for (offset, _) in text.match_indices(".register(") {
            let line_start = text[..offset]
                .rfind('\n')
                .map(|index| index + 1)
                .unwrap_or(0);
            let line = &text[line_start..offset];
            let receiver_is_catalog = line.ends_with("catalog")
                || line.contains("catalog.")
                || line.ends_with("SecretCatalog");
            if !receiver_is_catalog {
                continue;
            }
            if offset < test_start {
                offenses.push(format!(
                    "{}:{}",
                    path.display(),
                    text[..offset].matches('\n').count() + 1
                ));
            }
        }
    }
    assert!(
        offenses.is_empty(),
        "forced SecretCatalog::register is called from product code: {offenses:?}"
    );
}
