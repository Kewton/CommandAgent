use serde_json::{Value, json};

use crate::providers::{AssistantReply, ProviderResponseMetadata};
use crate::state::{ConversationMessage, ToolCall};
use crate::tools::args_recovery::recover_tool_arguments;
use crate::tools::registry::ToolSpec;

use super::parsing::sanitized_tool_schema;

/// Provider-owned conversation state for Responses API reasoning items.
///
/// K3's state-management lesson applies at this boundary: reasoning output is
/// harness state, not display text. The Responses API requires every reasoning
/// item returned with a function call to be replayed with the subsequent tool
/// output, so we retain the exact output items and splice them back into the
/// matching assistant turn. This state is shared across the clones created by
/// `provider_call`, keeping timeout/cancellation/event policy at that chokepoint.
#[derive(Clone, Debug, Default)]
pub(crate) struct ConversationState {
    turns: Vec<StoredTurn>,
}

#[derive(Clone, Debug)]
struct StoredTurn {
    reply: AssistantReply,
    output: Vec<Value>,
}

impl ConversationState {
    pub(crate) fn record(&mut self, parsed: &ParsedResponse) {
        if !parsed.output.is_empty() {
            self.turns.push(StoredTurn {
                reply: parsed.reply.clone(),
                output: parsed.output.clone(),
            });
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ParsedResponse {
    pub(crate) reply: AssistantReply,
    pub(crate) metadata: ProviderResponseMetadata,
    output: Vec<Value>,
}

pub(crate) fn build_request(
    model: &str,
    messages: &[ConversationMessage],
    tools: &[ToolSpec],
    native_tools_enabled: bool,
    max_predict: usize,
    reasoning_effort: Option<&str>,
    state: &ConversationState,
) -> Value {
    let mut body = json!({
        "model": model,
        "input": responses_input(messages, state),
        "max_output_tokens": max_predict,
        "store": false,
        "include": ["reasoning.encrypted_content"],
    });
    if native_tools_enabled && !tools.is_empty() {
        body["tools"] = Value::Array(tools.iter().map(sanitized_tool_schema).collect());
    }
    if let Some(effort) = reasoning_effort {
        body["reasoning"] = json!({"effort": effort});
    }
    body
}

fn responses_input(messages: &[ConversationMessage], state: &ConversationState) -> Vec<Value> {
    let mut input = Vec::new();
    let mut used_turns = vec![false; state.turns.len()];
    for message in messages {
        match message.role.as_str() {
            "assistant" => {
                if let Some((index, turn)) =
                    state.turns.iter().enumerate().find(|(index, turn)| {
                        !used_turns[*index] && same_reply(message, &turn.reply)
                    })
                {
                    used_turns[index] = true;
                    input.extend(turn.output.iter().cloned());
                } else {
                    push_assistant_fallback(&mut input, message);
                }
            }
            "tool" => {
                if let Some(call_id) = message.tool_call_id.as_deref() {
                    input.push(json!({
                        "type": "function_call_output",
                        "call_id": call_id,
                        "output": message.content,
                    }));
                } else {
                    input.push(message_item("user", "input_text", &message.content));
                }
            }
            role => input.push(message_item(role, "input_text", &message.content)),
        }
    }
    input
}

fn same_reply(message: &ConversationMessage, reply: &AssistantReply) -> bool {
    message.content == reply.content && message.tool_calls == reply.tool_calls
}

fn push_assistant_fallback(input: &mut Vec<Value>, message: &ConversationMessage) {
    if !message.content.is_empty() {
        input.push(message_item("assistant", "output_text", &message.content));
    }
    input.extend(message.tool_calls.iter().map(|call| {
        json!({
            "type": "function_call",
            "call_id": call.id,
            "name": call.name,
            "arguments": call.arguments.to_string(),
        })
    }));
}

fn message_item(role: &str, content_type: &str, content: &str) -> Value {
    json!({
        "role": role,
        "content": [{"type": content_type, "text": content}],
    })
}

pub(crate) fn parse_response(body: &str) -> anyhow::Result<ParsedResponse> {
    let value: Value = serde_json::from_str(body)?;
    let output = value
        .get("output")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut content = String::new();
    let mut tool_calls = Vec::new();
    for item in &output {
        match item.get("type").and_then(Value::as_str).unwrap_or_default() {
            "function_call" => tool_calls.push(parse_function_call(item)?),
            "message" => append_message_text(item, &mut content),
            // Reasoning items are deliberately opaque. They are retained in
            // `output` and replayed byte-for-value, never rendered as prose.
            "reasoning" => {}
            _ => append_message_text(item, &mut content),
        }
    }
    if content.is_empty() {
        content = value
            .get("output_text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
    }
    let usage = value.get("usage");
    let input_tokens = usage
        .and_then(|usage| usage.get("input_tokens"))
        .and_then(Value::as_u64);
    let output_tokens = usage
        .and_then(|usage| usage.get("output_tokens"))
        .and_then(Value::as_u64);
    let metadata = ProviderResponseMetadata {
        response_id: optional_string(&value, "id"),
        model_id: optional_string(&value, "model"),
        system_fingerprint: optional_string(&value, "system_fingerprint"),
        created_epoch: epoch_value(value.get("created_at")),
        service_tier: optional_string(&value, "service_tier"),
        cached_input_tokens: usage
            .and_then(|usage| usage.get("input_tokens_details"))
            .and_then(|details| details.get("cached_tokens"))
            .and_then(Value::as_u64),
        reasoning_tokens: usage
            .and_then(|usage| usage.get("output_tokens_details"))
            .and_then(|details| details.get("reasoning_tokens"))
            .and_then(Value::as_u64),
        total_tokens: usage
            .and_then(|usage| usage.get("total_tokens"))
            .and_then(Value::as_u64),
    };
    Ok(ParsedResponse {
        reply: AssistantReply {
            content,
            tool_calls,
            prompt_tokens: input_tokens,
            completion_tokens: output_tokens,
        },
        metadata,
        output,
    })
}

fn append_message_text(item: &Value, content: &mut String) {
    let Some(parts) = item.get("content").and_then(Value::as_array) else {
        return;
    };
    for part in parts {
        if matches!(
            part.get("type").and_then(Value::as_str),
            Some("output_text" | "text")
        ) && let Some(text) = part.get("text").and_then(Value::as_str)
        {
            content.push_str(text);
        }
    }
}

fn parse_function_call(item: &Value) -> anyhow::Result<ToolCall> {
    let name = item
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("OpenAI function_call missing name"))?
        .to_string();
    let arguments = normalize_function_arguments(item.get("arguments").cloned())?;
    Ok(ToolCall {
        id: item
            .get("call_id")
            .or_else(|| item.get("id"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| uuid::Uuid::now_v7().to_string()),
        name: name.clone(),
        arguments: recover_tool_arguments(&name, arguments).arguments,
    })
}

fn normalize_function_arguments(value: Option<Value>) -> anyhow::Result<Value> {
    match value {
        Some(object @ Value::Object(_)) => Ok(object),
        Some(Value::String(raw)) => {
            let decoded: Value = serde_json::from_str(&raw).map_err(|error| {
                anyhow::anyhow!("OpenAI function_call arguments are not valid JSON: {error}")
            })?;
            if decoded.is_object() {
                Ok(decoded)
            } else {
                anyhow::bail!(
                    "OpenAI function_call arguments must decode to object, got {}",
                    json_type(&decoded)
                )
            }
        }
        Some(Value::Null) | None => {
            anyhow::bail!("OpenAI function_call missing arguments")
        }
        Some(other) => anyhow::bail!(
            "OpenAI function_call arguments must be object or JSON string, got {}",
            json_type(&other)
        ),
    }
}

fn optional_string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

fn epoch_value(value: Option<&Value>) -> Option<i64> {
    value.and_then(Value::as_i64).or_else(|| {
        value
            .and_then(Value::as_f64)
            .map(|epoch| epoch.floor() as i64)
    })
}

fn json_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::registry::ToolRegistry;

    const RESPONSE: &str = r#"{
      "id":"resp_f0b",
      "model":"gpt-5.6-luna-2026-08-01",
      "created_at":1785542400.5,
      "service_tier":"default",
      "output":[
        {"type":"reasoning","id":"rs_1","encrypted_content":"opaque"},
        {"type":"function_call","id":"fc_1","call_id":"call_1","name":"Read","arguments":"{\"path\":\"README.md\"}"}
      ],
      "usage":{"input_tokens":11,"input_tokens_details":{"cached_tokens":3},"output_tokens":7,"output_tokens_details":{"reasoning_tokens":5},"total_tokens":18}
    }"#;

    #[test]
    fn responses_request_uses_native_tool_shape() {
        let body = build_request(
            "gpt-5.6-luna",
            &[ConversationMessage::user("inspect")],
            ToolRegistry::default().specs(),
            true,
            128,
            None,
            &ConversationState::default(),
        );

        assert_eq!(body["model"], "gpt-5.6-luna");
        assert_eq!(body["input"][0]["content"][0]["type"], "input_text");
        assert_eq!(body["tools"][0]["type"], "function");
        assert!(body["tools"][0].get("function").is_none());
        assert_eq!(body["store"], false);
        assert_eq!(body["include"][0], "reasoning.encrypted_content");
    }

    #[test]
    fn response_maps_items_and_reasoning_usage() {
        let parsed = parse_response(RESPONSE).unwrap();

        assert_eq!(parsed.reply.tool_calls[0].id, "call_1");
        assert_eq!(parsed.reply.tool_calls[0].arguments["path"], "README.md");
        assert_eq!(parsed.reply.prompt_tokens, Some(11));
        assert_eq!(parsed.reply.completion_tokens, Some(7));
        assert_eq!(parsed.metadata.response_id.as_deref(), Some("resp_f0b"));
        assert_eq!(parsed.metadata.reasoning_tokens, Some(5));
        assert_eq!(parsed.metadata.cached_input_tokens, Some(3));
        assert_eq!(parsed.metadata.total_tokens, Some(18));
    }

    #[test]
    fn reasoning_item_is_replayed_before_function_output() {
        let parsed = parse_response(RESPONSE).unwrap();
        let mut state = ConversationState::default();
        state.record(&parsed);
        let messages = vec![
            ConversationMessage::user("inspect"),
            ConversationMessage::assistant(
                parsed.reply.content.clone(),
                parsed.reply.tool_calls.clone(),
            ),
            ConversationMessage::tool_result("Read", Some("call_1"), "contents"),
        ];
        let body = build_request(
            "gpt-5.6-luna",
            &messages,
            ToolRegistry::default().specs(),
            true,
            128,
            Some("high"),
            &state,
        );

        assert_eq!(body["input"][1]["type"], "reasoning");
        assert_eq!(body["input"][1]["encrypted_content"], "opaque");
        assert_eq!(body["input"][2]["type"], "function_call");
        assert_eq!(body["input"][3]["type"], "function_call_output");
        assert_eq!(body["input"][3]["call_id"], "call_1");
        assert_eq!(body["reasoning"]["effort"], "high");
    }

    #[test]
    fn message_and_output_text_fallback_do_not_duplicate_content() {
        let with_message = parse_response(
            r#"{"output_text":"hello","output":[{"type":"message","content":[{"type":"output_text","text":"hello"}]}]}"#,
        )
        .unwrap();
        let fallback = parse_response(r#"{"output_text":"fallback"}"#).unwrap();

        assert_eq!(with_message.reply.content, "hello");
        assert_eq!(fallback.reply.content, "fallback");
    }
}
