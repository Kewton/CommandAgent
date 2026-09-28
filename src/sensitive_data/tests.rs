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
    let catalog = catalog(&["abcdef", "abc"]);
    let once = catalog.scrub("x abcdef abc y");
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
fn credential_named_sources_register_short_values_but_generic_short_values_do_not() {
    let mut catalog = SecretCatalog::new();
    catalog.register_named("VLLM_API_KEY", "short");
    catalog.register_named("PORT", "3000");
    catalog.register_named("DEBUG", "true");
    catalog.register_named("GENERIC", "abcdefgh");

    assert!(catalog.contains("short"), "credential-named short value");
    assert!(catalog.contains("abcdefgh"), "generic 8-char value");
    assert!(
        !catalog.contains("3000"),
        "ordinary short value is not a secret"
    );
    assert!(!catalog.contains("true"));
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
    let error = collect_scoped_dotenv(dir.path()).unwrap_err();
    assert!(matches!(error, CollectionRefusal::Read { .. }), "{error}");
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
