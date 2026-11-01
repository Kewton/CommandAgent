use serde_json::{Value, json};

use crate::providers::{AssistantReply, BilledCostUsd, ProviderResponseMetadata};
use crate::state::{ConversationMessage, ToolCall};
use crate::tools::args_recovery::recover_tool_arguments;
use crate::tools::registry::ToolSpec;

use super::parsing::sanitize_schema;

#[cfg(test)]
const LUNA_MODEL: &str = "gpt-5.6-luna";

pub(crate) fn build_request(
    model: &str,
    messages: &[ConversationMessage],
    tools: &[ToolSpec],
    native_tools_enabled: bool,
    max_predict: usize,
    reasoning_effort: Option<&str>,
) -> Value {
    build_request_with_token_field(
        model,
        messages,
        tools,
        native_tools_enabled,
        max_predict,
        reasoning_effort,
        "max_completion_tokens",
    )
}

pub(crate) fn build_lm_studio_request(
    model: &str,
    messages: &[ConversationMessage],
    tools: &[ToolSpec],
    native_tools_enabled: bool,
    max_predict: usize,
) -> Value {
    build_request_with_token_field(
        model,
        messages,
        tools,
        native_tools_enabled,
        max_predict,
        None,
        "max_tokens",
    )
}

/// The compatible request with an explicitly declared reasoning effort as the
/// top-level `reasoning_effort`; the token field stays `max_tokens`.
pub(crate) fn build_lm_studio_request_with_effort(
    model: &str,
    messages: &[ConversationMessage],
    tools: &[ToolSpec],
    native_tools_enabled: bool,
    max_predict: usize,
    reasoning_effort: &str,
) -> Value {
    build_request_with_token_field(
        model,
        messages,
        tools,
        native_tools_enabled,
        max_predict,
        Some(reasoning_effort),
        "max_tokens",
    )
}

fn build_request_with_token_field(
    model: &str,
    messages: &[ConversationMessage],
    tools: &[ToolSpec],
    native_tools_enabled: bool,
    max_predict: usize,
    reasoning_effort: Option<&str>,
    token_field: &str,
) -> Value {
    let mut body = json!({
        "model": model,
        "messages": messages
            .iter()
            .map(|message| chat_message(message, native_tools_enabled))
            .collect::<Vec<_>>(),
    });
    body[token_field] = json!(max_predict);
    if native_tools_enabled && !tools.is_empty() {
        body["tools"] = Value::Array(
            tools
                .iter()
                .map(|tool| {
                    json!({
                        "type": "function",
                        "function": {
                            "name": tool.function.name,
                            "description": tool.function.description,
                            "parameters": sanitize_schema(&tool.function.parameters),
                        }
                    })
                })
                .collect(),
        );
    }
    // Optional provider generation controls are declaration-only: omitting a
    // setting must not silently materialize a client-side provider default.
    if let Some(reasoning_effort) = reasoning_effort {
        body["reasoning_effort"] = Value::String(reasoning_effort.to_string());
    }
    body
}

fn chat_message(message: &ConversationMessage, native_tools_enabled: bool) -> Value {
    if !native_tools_enabled {
        let role = if message.role == "tool" {
            "user"
        } else {
            message.role.as_str()
        };
        return json!({
            "role": role,
            "content": message.content,
        });
    }
    match message.role.as_str() {
        "assistant" if !message.tool_calls.is_empty() => json!({
            "role": "assistant",
            "content": message.content,
            "tool_calls": message.tool_calls.iter().map(|call| json!({
                "id": call.id,
                "type": "function",
                "function": {
                    "name": call.name,
                    "arguments": call.arguments.to_string(),
                }
            })).collect::<Vec<_>>(),
        }),
        "tool" => json!({
            "role": "tool",
            "tool_call_id": message.tool_call_id,
            "content": message.content,
        }),
        _ => json!({
            "role": message.role,
            "content": message.content,
        }),
    }
}

pub(crate) fn parse_response(
    body: &str,
) -> anyhow::Result<(AssistantReply, ProviderResponseMetadata)> {
    let value: Value = serde_json::from_str(body)?;
    let message = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .ok_or_else(|| anyhow::anyhow!("OpenAI chat completion missing choices[0].message"))?;
    let content = message
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let tool_calls = message
        .get("tool_calls")
        .and_then(Value::as_array)
        .map(|calls| calls.iter().map(parse_tool_call).collect())
        .transpose()?
        .unwrap_or_default();
    let usage = value.get("usage");
    let reply = AssistantReply {
        content,
        tool_calls,
        prompt_tokens: usage
            .and_then(|usage| usage.get("prompt_tokens"))
            .and_then(Value::as_u64),
        completion_tokens: usage
            .and_then(|usage| usage.get("completion_tokens"))
            .and_then(Value::as_u64),
    };
    let metadata = ProviderResponseMetadata {
        response_id: optional_string(&value, "id"),
        model_id: optional_string(&value, "model"),
        system_fingerprint: optional_string(&value, "system_fingerprint"),
        created_epoch: value.get("created").and_then(Value::as_i64),
        service_tier: optional_string(&value, "service_tier"),
        cached_input_tokens: cached_input_tokens(usage, reply.prompt_tokens),
        reasoning_tokens: None,
        total_tokens: None,
        billed_cost_usd: billed_cost_usd(body),
    };
    Ok((reply, metadata))
}

/// The raw members of a JSON object, borrowed before any number in them
/// becomes an `f64`. Only an object deserializes into it (an array does not),
/// and a repeated key keeps its last value, as `serde_json::Value` does.
type RawMembers<'a> = std::collections::HashMap<String, &'a serde_json::value::RawValue>;

/// The provider-reported billed amount in `usage.cost` (Issue #947), kept as
/// optional provider metadata only; no amount is derived from token counts,
/// model, or provider. It is read from the raw number token of the body that
/// already parsed as a whole, so `1e-400` is not underflowed to zero and
/// `1.00000000000000001` is not rounded. Anything but an unsigned, in-range
/// number token is unreported (`None`) and changes no other field; an explicit
/// zero stays a value.
fn billed_cost_usd(body: &str) -> Option<BilledCostUsd> {
    let response: RawMembers<'_> = serde_json::from_str(body).ok()?;
    let usage: RawMembers<'_> = serde_json::from_str(response.get("usage")?.get()).ok()?;
    BilledCostUsd::from_json_token(usage.get("cost")?.get())
}

/// The provider-reported cached share of the prompt, already included in
/// `prompt_tokens`. A missing or malformed count, or one above a known prompt
/// total, is unreported (`None`); an explicit zero stays `Some(0)`.
fn cached_input_tokens(usage: Option<&Value>, prompt_tokens: Option<u64>) -> Option<u64> {
    let cached = usage?
        .get("prompt_tokens_details")?
        .get("cached_tokens")?
        .as_u64()?;
    match prompt_tokens {
        Some(total) if cached > total => None,
        _ => Some(cached),
    }
}

fn optional_string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

fn parse_tool_call(value: &Value) -> anyhow::Result<ToolCall> {
    let function = value
        .get("function")
        .ok_or_else(|| anyhow::anyhow!("OpenAI chat tool_call missing function"))?;
    let name = function
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("OpenAI chat tool_call missing function.name"))?
        .to_string();
    let raw_arguments = function
        .get("arguments")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("OpenAI chat tool_call missing function.arguments"))?;
    let arguments: Value = serde_json::from_str(raw_arguments).map_err(|error| {
        anyhow::anyhow!("OpenAI chat tool_call arguments are not valid JSON: {error}")
    })?;
    if !arguments.is_object() {
        anyhow::bail!("OpenAI chat tool_call arguments must decode to object");
    }
    Ok(ToolCall {
        id: value
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| uuid::Uuid::now_v7().to_string()),
        name: name.clone(),
        arguments: recover_tool_arguments(&name, arguments).arguments,
    })
}

#[cfg(test)]
mod cache_usage_tests;

/// Issue #947: `usage.cost` reaches `ProviderResponseMetadata.billed_cost_usd`
/// as the exact decimal text of the number token in the raw body. An explicit
/// zero is a value; a missing, null, non-number, negative, or out-of-range
/// amount is unreported. Every body here is raw text, never a `Value`, so a
/// lost digit cannot hide behind an `f64` on both sides.
#[cfg(test)]
mod billed_cost_tests {
    use super::parse_response;
    use crate::providers::{AssistantReply, BilledCostUsd, ProviderResponseMetadata};

    const USAGE_TOKENS: &str = r#""prompt_tokens":5361,"completion_tokens":2,"total_tokens":5363,"prompt_tokens_details":{"cached_tokens":5344}"#;

    /// A raw response body; `usage` is the raw text of the usage member, or
    /// `None` for no usage at all.
    fn body(usage: Option<&str>) -> String {
        let usage = usage.map_or_else(String::new, |usage| format!(r#","usage":{usage}"#));
        format!(
            r#"{{"id":"chatcmpl-947","model":"compatible-model","created":1790000000,"system_fingerprint":"fp_947","service_tier":"default","choices":[{{"message":{{"content":"reading","tool_calls":[{{"id":"call-947","type":"function","function":{{"name":"Read","arguments":"{{\"path\":\"README.md\"}}"}}}}]}}}}]{usage}}}"#
        )
    }

    /// The raw usage member with `cost` set to the raw token `cost`.
    fn usage_with_cost(cost: &str) -> String {
        format!(r#"{{{USAGE_TOKENS},"cost":{cost}}}"#)
    }

    fn parse(usage: Option<&str>) -> (AssistantReply, ProviderResponseMetadata) {
        parse_response(&body(usage)).expect("a malformed amount must not fail the parse")
    }

    fn cost(usage: Option<&str>) -> Option<BilledCostUsd> {
        parse(usage).1.billed_cost_usd
    }

    /// The kept decimal text for the raw cost token `sent`.
    fn cost_text(sent: &str) -> Option<String> {
        cost(Some(&usage_with_cost(sent))).map(|cost| cost.as_decimal_str().to_string())
    }

    #[test]
    fn positive_amounts_are_kept_as_sent_without_rounding() {
        for sent in [
            "0.012345",
            "0.000123",
            "1.5e-7",
            "12.5",
            "2",
            "1E+2",
            "1.00000000000000001",
            "0.123456789012345678901234567890",
            "123456.000000000000000000000001",
        ] {
            assert_eq!(cost_text(sent).as_deref(), Some(sent), "{sent}");
        }
        let kept = cost(Some(&usage_with_cost("0.012345"))).unwrap();
        assert_eq!(kept.as_decimal_str(), "0.012345");
        assert_eq!(kept.to_event_value().to_string(), r#""0.012345""#);
    }

    #[test]
    fn a_tiny_positive_amount_is_not_underflowed_to_zero() {
        for sent in ["1e-400", "5E-324", "0.1e-1000", "1e-99999999999999999999"] {
            assert_eq!(cost_text(sent).as_deref(), Some(sent), "{sent}");
        }
        let kept = cost(Some(&usage_with_cost("1e-400"))).unwrap();
        assert_eq!(kept.to_event_value().to_string(), r#""1e-400""#);
    }

    #[test]
    fn explicit_zero_is_a_value_not_unknown() {
        for sent in ["0", "0.0", "0e5", "0.000E-3"] {
            assert_eq!(cost_text(sent).as_deref(), Some(sent), "{sent}");
        }
        let zero = cost(Some(&usage_with_cost("0"))).unwrap();
        assert_eq!(zero.to_event_value().to_string(), r#""0""#);
    }

    #[test]
    fn surrounding_whitespace_is_not_part_of_the_amount() {
        assert_eq!(cost_text(" 0.012345 ").as_deref(), Some("0.012345"));
        assert_eq!(cost_text("\n1e-400\t").as_deref(), Some("1e-400"));
    }

    #[test]
    fn missing_amounts_stay_unreported() {
        let without_cost = format!("{{{USAGE_TOKENS}}}");
        for (label, usage) in [
            ("no usage", None),
            ("usage without cost", Some(without_cost.as_str())),
            ("usage is null", Some("null")),
            ("usage is a number", Some("1")),
            ("usage is an array", Some("[0.012345]")),
        ] {
            assert_eq!(cost(usage), None, "{label}");
        }
        assert_eq!(cost_text("null"), None, "cost null");
    }

    #[test]
    fn a_repeated_key_keeps_its_last_value_as_the_other_fields_do() {
        let last_null = format!(r#"{{{USAGE_TOKENS},"cost":0.5,"cost":null}}"#);
        let last_amount = format!(r#"{{{USAGE_TOKENS},"cost":null,"cost":0.5}}"#);
        assert_eq!(cost(Some(&last_null)), None);
        assert_eq!(
            cost(Some(&last_amount)).map(|cost| cost.as_decimal_str().to_string()),
            Some("0.5".to_string())
        );
        let repeated_usage = body(Some(&usage_with_cost("0.25"))).replacen(
            r#""id":"chatcmpl-947""#,
            r#""usage":{"cost":7},"id":"chatcmpl-947""#,
            1,
        );
        let (_, metadata) = parse_response(&repeated_usage).unwrap();
        assert_eq!(metadata.billed_cost_usd.unwrap().as_decimal_str(), "0.25");
        assert_eq!(metadata.cached_input_tokens, Some(5344));
    }

    #[test]
    fn non_number_amounts_stay_unreported() {
        for sent in [
            r#""0.012345""#,
            r#""""#,
            "true",
            "false",
            "[0.012345]",
            r#"{"usd":0.012345}"#,
        ] {
            assert_eq!(cost_text(sent), None, "{sent}");
        }
    }

    #[test]
    fn negative_amounts_stay_unreported() {
        for sent in ["-0.012345", "-1", "-0", "-0.0", "-0e5", "-1e-400"] {
            assert_eq!(cost_text(sent), None, "{sent}");
        }
    }

    #[test]
    fn the_upper_bound_is_exact_in_decimal() {
        for sent in [
            "9007199254740991",
            "9007199254740991.0",
            "9.007199254740991e15",
            "900719925474099.1E1",
            "0.9007199254740991e16",
            "9007199254740990.99999999999999999999",
        ] {
            assert_eq!(cost_text(sent).as_deref(), Some(sent), "{sent}");
        }
        for sent in [
            "9007199254740992",
            "9007199254740991.000000000000000001",
            "9.007199254740992e15",
            "9.0071992547409910001E15",
            "1e16",
            "18446744073709551616",
            "1e300",
            "1.7976931348623157e308",
        ] {
            assert_eq!(cost_text(sent), None, "{sent}");
        }
    }

    #[test]
    fn a_number_beyond_f64_still_fails_the_whole_body_as_at_base() {
        // serde_json rejects `1e400` (number out of range) while parsing the
        // whole body, before the amount is read; the base commit fails the same
        // way, and this change keeps that honest failure.
        for sent in ["1e400", "1e99999999999999999999"] {
            let error = parse_response(&body(Some(&usage_with_cost(sent))))
                .expect_err("the whole body fails to parse");
            assert!(
                error.to_string().contains("out of range"),
                "{sent}: {error}"
            );
        }
        for broken in ["{", "", r#"{"choices":[]}"#] {
            assert!(parse_response(broken).is_err(), "{broken}");
        }
    }

    #[test]
    fn reply_tokens_and_cached_count_are_unchanged_by_the_amount() {
        let legacy = format!("{{{USAGE_TOKENS}}}");
        let (legacy_reply, legacy_metadata) = parse(Some(&legacy));
        assert_eq!(legacy_metadata.billed_cost_usd, None);
        assert_eq!(legacy_metadata.cached_input_tokens, Some(5344));
        assert_eq!(legacy_metadata.total_tokens, None);
        assert_eq!(legacy_metadata.reasoning_tokens, None);
        assert_eq!(legacy_reply.prompt_tokens, Some(5361));
        assert_eq!(legacy_reply.completion_tokens, Some(2));
        for (sent, reported) in [
            ("0.012345", true),
            ("0", true),
            ("1e-400", true),
            ("null", false),
            (r#""0.012345""#, false),
            ("-1", false),
            ("-0", false),
            ("1e300", false),
        ] {
            let (reply, metadata) = parse(Some(&usage_with_cost(sent)));
            assert_eq!(reply, legacy_reply, "cost {sent}");
            assert_eq!(metadata.billed_cost_usd.is_some(), reported, "cost {sent}");
            let mut without_cost = metadata.clone();
            without_cost.billed_cost_usd = None;
            assert_eq!(without_cost, legacy_metadata, "cost {sent}");
        }
    }

    #[test]
    fn metadata_equality_compares_the_amount() {
        let first = parse(Some(&usage_with_cost("0.012345"))).1;
        let same = parse(Some(&usage_with_cost("0.012345"))).1;
        let other = parse(Some(&usage_with_cost("0.012346"))).1;
        let beyond_f64 = parse(Some(&usage_with_cost("1.00000000000000001"))).1;
        let one = parse(Some(&usage_with_cost("1"))).1;
        assert_eq!(first, same);
        assert_ne!(first, other);
        assert_ne!(beyond_f64, one);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::registry::ToolRegistry;

    #[test]
    fn request_uses_chat_completions_message_and_tool_shape() {
        let body = build_request(
            LUNA_MODEL,
            &[ConversationMessage::user("hello")],
            ToolRegistry::default().specs(),
            true,
            128,
            None,
        );

        assert_eq!(body["model"], LUNA_MODEL);
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["max_completion_tokens"], 128);
        assert_eq!(body["tools"][0]["type"], "function");
        assert!(body["tools"][0].get("function").is_some());
        assert!(body.get("reasoning_effort").is_none());
    }

    #[test]
    fn luna_text_request_omits_tools_and_native_history_shape() {
        let messages = [
            ConversationMessage::assistant(
                "",
                vec![ToolCall {
                    id: "call-1".to_string(),
                    name: "Read".to_string(),
                    arguments: json!({"path": "README.md"}),
                }],
            ),
            ConversationMessage::tool("call-1", "contents"),
        ];
        let body = build_request(
            LUNA_MODEL,
            &messages,
            ToolRegistry::default().specs(),
            false,
            128,
            None,
        );

        assert!(body.get("tools").is_none());
        assert!(body["messages"][0].get("tool_calls").is_none());
        assert_eq!(body["messages"][1]["role"], "user");
        assert!(body["messages"][1].get("tool_call_id").is_none());
    }

    #[test]
    fn reasoning_effort_is_present_only_when_explicitly_configured() {
        let unconfigured = build_request(
            LUNA_MODEL,
            &[ConversationMessage::user("hello")],
            &[],
            false,
            128,
            None,
        );
        let configured = build_request(
            LUNA_MODEL,
            &[ConversationMessage::user("hello")],
            ToolRegistry::default().specs(),
            true,
            128,
            Some("none"),
        );

        assert_eq!(
            unconfigured,
            json!({
                "model": LUNA_MODEL,
                "messages": [{"role": "user", "content": "hello"}],
                "max_completion_tokens": 128,
            })
        );
        assert_eq!(configured["reasoning_effort"], "none");
    }

    #[test]
    fn response_preserves_drift_metadata_and_tool_calls() {
        let (reply, metadata) = parse_response(
            r#"{
              "id":"chatcmpl-live-1",
              "model":"gpt-5.6-luna-2026-07-31",
              "created":1785456000,
              "system_fingerprint":"fp_luna_01",
              "service_tier":"default",
              "choices":[{"message":{"content":"ok","tool_calls":[{
                "id":"call-1","type":"function","function":{
                  "name":"Read","arguments":"{\"path\":\"README.md\"}"
                }}]}}],
              "usage":{"prompt_tokens":4,"completion_tokens":2}
            }"#,
        )
        .unwrap();

        assert_eq!(reply.content, "ok");
        assert_eq!(reply.tool_calls[0].arguments["path"], "README.md");
        assert_eq!(metadata.response_id.as_deref(), Some("chatcmpl-live-1"));
        assert_eq!(
            metadata.model_id.as_deref(),
            Some("gpt-5.6-luna-2026-07-31")
        );
        assert_eq!(metadata.system_fingerprint.as_deref(), Some("fp_luna_01"));
        assert_eq!(metadata.created_epoch, Some(1_785_456_000));
    }
}
