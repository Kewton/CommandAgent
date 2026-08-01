use serde_json::{Value, json};

use crate::providers::{AssistantReply, ProviderResponseMetadata};
use crate::state::{ConversationMessage, ToolCall};
use crate::tools::args_recovery::recover_tool_arguments;
use crate::tools::registry::ToolSpec;

use super::parsing::sanitize_schema;

pub(crate) const LUNA_MODEL: &str = "gpt-5.6-luna";

pub(crate) fn uses_chat_completions(model: &str) -> bool {
    model == LUNA_MODEL || model.starts_with("gpt-5.6-luna-")
}

pub(crate) fn build_request(
    model: &str,
    messages: &[ConversationMessage],
    tools: &[ToolSpec],
    native_tools_enabled: bool,
    max_predict: usize,
    reasoning_effort: Option<&str>,
) -> Value {
    let mut body = json!({
        "model": model,
        "messages": messages.iter().map(chat_message).collect::<Vec<_>>(),
        "max_completion_tokens": max_predict,
    });
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

fn chat_message(message: &ConversationMessage) -> Value {
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
    };
    Ok((reply, metadata))
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
mod tests {
    use super::*;
    use crate::tools::registry::ToolRegistry;

    #[test]
    fn luna_and_snapshot_ids_use_chat_completions() {
        assert!(uses_chat_completions("gpt-5.6-luna"));
        assert!(uses_chat_completions("gpt-5.6-luna-2026-07-31"));
        assert!(!uses_chat_completions("gpt-5.6"));
        assert!(!uses_chat_completions("gpt-5.4-mini"));
    }

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
