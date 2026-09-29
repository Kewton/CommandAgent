use std::path::Path;
use std::sync::Arc;

use serde_json::json;

use super::*;

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

    // The same key names under the schema scrub are fixed identifiers.
    let mut schema =
        json!({"status": canary, "type": canary, "action": canary, "tool_name": canary});
    catalog.scrub_value_lenient(&mut schema);
    assert_eq!(schema["status"], canary);
    assert_eq!(schema["type"], canary);
    assert_eq!(schema["action"], canary);
    assert_eq!(schema["tool_name"], canary);
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
