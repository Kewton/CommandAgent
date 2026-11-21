//! Issue #928: the cached input token count a Chat Completions response
//! reports in `usage.prompt_tokens_details.cached_tokens` reaches
//! `ProviderResponseMetadata.cached_input_tokens`. An explicit zero stays
//! `Some(0)`; a missing, malformed, or inconsistent count stays unreported.

use serde_json::{Value, json};

use super::parse_response;

/// A response with one native tool call and the given `usage` (omitted when
/// `None`).
fn body(usage: Option<Value>) -> String {
    let mut value = json!({
        "id": "chatcmpl-928",
        "model": "compatible-model",
        "created": 1_790_000_000,
        "system_fingerprint": "fp_928",
        "service_tier": "default",
        "choices": [{"message": {
            "content": "reading",
            "tool_calls": [{
                "id": "call-928",
                "type": "function",
                "function": {"name": "Read", "arguments": "{\"path\":\"README.md\"}"}
            }]
        }}]
    });
    if let Some(usage) = usage {
        value["usage"] = usage;
    }
    value.to_string()
}

fn usage_with_cached(prompt_tokens: Value, cached: Value) -> Value {
    json!({
        "prompt_tokens": prompt_tokens,
        "completion_tokens": 2,
        "total_tokens": 5363,
        "prompt_tokens_details": {"cached_tokens": cached}
    })
}

fn cached(usage: Option<Value>) -> Option<u64> {
    parse_response(&body(usage))
        .expect("a malformed cached count must not fail the parse")
        .1
        .cached_input_tokens
}

#[test]
fn reported_counts_within_the_prompt_total_are_kept() {
    for (prompt_tokens, count) in [(5361, 0), (5361, 5344), (5362, 2688), (5361, 5361)] {
        assert_eq!(
            cached(Some(usage_with_cached(json!(prompt_tokens), json!(count)))),
            Some(count),
            "cached {count} of {prompt_tokens}"
        );
    }
    assert_eq!(
        cached(Some(usage_with_cached(json!(u64::MAX), json!(u64::MAX)))),
        Some(u64::MAX)
    );
}

#[test]
fn explicit_zero_is_reported_as_zero_not_unknown() {
    let (reply, metadata) =
        parse_response(&body(Some(usage_with_cached(json!(5361), json!(0))))).unwrap();
    assert_eq!(metadata.cached_input_tokens, Some(0));
    assert_eq!(reply.prompt_tokens, Some(5361));
}

#[test]
fn missing_counts_stay_unreported() {
    let legacy = json!({"prompt_tokens": 5361, "completion_tokens": 2, "total_tokens": 5363});
    let empty_details = json!({
        "prompt_tokens": 5361,
        "completion_tokens": 2,
        "prompt_tokens_details": {}
    });
    let other_details = json!({
        "prompt_tokens": 5361,
        "completion_tokens": 2,
        "prompt_tokens_details": {"audio_tokens": 0}
    });
    for (label, usage) in [
        ("no usage", None),
        ("legacy usage", Some(legacy)),
        ("empty details", Some(empty_details)),
        ("details without cached_tokens", Some(other_details)),
        ("usage is null", Some(Value::Null)),
    ] {
        assert_eq!(cached(usage), None, "{label}");
    }
}

#[test]
fn malformed_counts_stay_unreported() {
    let out_of_range: Value = serde_json::from_str("18446744073709551616").unwrap();
    for (label, count) in [
        ("null", Value::Null),
        ("negative", json!(-1)),
        ("fraction", json!(1.5)),
        ("string", json!("5344")),
        ("boolean", json!(true)),
        ("array", json!([5344])),
        ("object", json!({"value": 5344})),
        ("above u64", out_of_range),
    ] {
        assert_eq!(
            cached(Some(usage_with_cached(json!(5361), count))),
            None,
            "{label}"
        );
    }
    for details in [json!(5344), json!("5344"), json!([{"cached_tokens": 5344}])] {
        let usage = json!({
            "prompt_tokens": 5361,
            "completion_tokens": 2,
            "prompt_tokens_details": details
        });
        assert_eq!(cached(Some(usage)), None, "details {details}");
    }
}

#[test]
fn count_above_the_known_prompt_total_stays_unreported() {
    assert_eq!(
        cached(Some(usage_with_cached(json!(5361), json!(5362)))),
        None
    );
    assert_eq!(cached(Some(usage_with_cached(json!(0), json!(1)))), None);
}

#[test]
fn count_without_a_prompt_total_is_kept_without_inventing_a_total() {
    for prompt_tokens in [Value::Null, json!("5361"), json!(-5)] {
        let (reply, metadata) = parse_response(&body(Some(usage_with_cached(
            prompt_tokens.clone(),
            json!(5344),
        ))))
        .unwrap();
        assert_eq!(metadata.cached_input_tokens, Some(5344), "{prompt_tokens}");
        assert_eq!(reply.prompt_tokens, None, "{prompt_tokens}");
        assert_eq!(metadata.total_tokens, None, "{prompt_tokens}");
    }
    let usage = json!({"prompt_tokens_details": {"cached_tokens": 7}});
    let (reply, metadata) = parse_response(&body(Some(usage))).unwrap();
    assert_eq!(metadata.cached_input_tokens, Some(7));
    assert_eq!(reply.prompt_tokens, None);
    assert_eq!(reply.completion_tokens, None);
}

#[test]
fn reply_and_other_metadata_are_unchanged_by_the_cached_count() {
    let legacy = json!({"prompt_tokens": 5361, "completion_tokens": 2, "total_tokens": 5363});
    let (legacy_reply, legacy_metadata) = parse_response(&body(Some(legacy))).unwrap();
    assert_eq!(legacy_metadata.cached_input_tokens, None);
    assert_eq!(legacy_reply.content, "reading");
    assert_eq!(legacy_reply.tool_calls.len(), 1);
    assert_eq!(legacy_reply.tool_calls[0].id, "call-928");
    assert_eq!(legacy_reply.tool_calls[0].name, "Read");
    assert_eq!(legacy_reply.tool_calls[0].arguments["path"], "README.md");
    assert_eq!(legacy_reply.prompt_tokens, Some(5361));
    assert_eq!(legacy_reply.completion_tokens, Some(2));
    assert_eq!(legacy_metadata.response_id.as_deref(), Some("chatcmpl-928"));
    assert_eq!(
        legacy_metadata.model_id.as_deref(),
        Some("compatible-model")
    );
    assert_eq!(
        legacy_metadata.system_fingerprint.as_deref(),
        Some("fp_928")
    );
    assert_eq!(legacy_metadata.created_epoch, Some(1_790_000_000));
    assert_eq!(legacy_metadata.service_tier.as_deref(), Some("default"));
    assert_eq!(legacy_metadata.reasoning_tokens, None);
    assert_eq!(legacy_metadata.total_tokens, None);

    for (count, expected) in [
        (json!(5344), Some(5344)),
        (json!(0), Some(0)),
        (json!("bad"), None),
        (json!(9999), None),
    ] {
        let (reply, metadata) =
            parse_response(&body(Some(usage_with_cached(json!(5361), count.clone())))).unwrap();
        assert_eq!(reply, legacy_reply, "cached {count}");
        assert_eq!(metadata.cached_input_tokens, expected, "cached {count}");
        let mut without_cached = metadata.clone();
        without_cached.cached_input_tokens = None;
        assert_eq!(without_cached, legacy_metadata, "cached {count}");
    }
}

#[test]
fn text_reply_without_usage_still_parses() {
    let (reply, metadata) =
        parse_response(r#"{"choices":[{"message":{"content":"done"}}]}"#).unwrap();
    assert_eq!(reply.content, "done");
    assert!(reply.tool_calls.is_empty());
    assert_eq!(reply.prompt_tokens, None);
    assert_eq!(reply.completion_tokens, None);
    assert_eq!(metadata.cached_input_tokens, None);
}
