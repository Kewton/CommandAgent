use crate::config::ToolProtocol;
use crate::providers::AssistantReply;
use crate::providers::xml_repair::ToolCallRepair;
use crate::tools::registry::ToolSpec;

/// Resolve the declared capability without inspecting a model name. Absence
/// deliberately preserves the provider's established capability negotiation,
/// which keeps every pre-declaration request byte-compatible.
pub(crate) fn native_tools_enabled(
    declared: Option<ToolProtocol>,
    provider_supports_native: bool,
    session_disabled: bool,
) -> bool {
    if session_disabled {
        return false;
    }
    match declared {
        Some(ToolProtocol::Text) => false,
        Some(ToolProtocol::Native) | None => provider_supports_native,
    }
}

/// Explicit text mode reuses the established XML/text parser used by local
/// providers. It only translates the provider reply into the existing typed
/// tool-call shape; execution and repair remain on the normal loop path.
pub(crate) fn normalize_text_reply(
    reply: &mut AssistantReply,
    tools: &[ToolSpec],
) -> anyhow::Result<Vec<ToolCallRepair>> {
    let allowed_tools = tools
        .iter()
        .map(|tool| tool.function.name.clone())
        .collect::<Vec<_>>();
    let original = reply.content.clone();
    let strict = crate::providers::xml_fallback::extract_tool_calls(&original, &allowed_tools);
    if let Some(candidate) = crate::providers::xml_repair::candidate(&original, &allowed_tools)
        && let Ok((tool_calls, content)) =
            crate::providers::xml_fallback::extract_tool_calls(&candidate.response, &allowed_tools)
        && validate_tool_calls(&tool_calls, tools).is_ok()
    {
        reply.content = content;
        reply.tool_calls = tool_calls;
        return Ok(vec![candidate.repair]);
    }
    let (tool_calls, content) = strict?;
    reply.content = content;
    reply.tool_calls = tool_calls;
    Ok(Vec::new())
}

fn validate_tool_calls(calls: &[crate::state::ToolCall], tools: &[ToolSpec]) -> anyhow::Result<()> {
    for call in calls {
        let spec = tools
            .iter()
            .find(|spec| spec.function.name == call.name)
            .ok_or_else(|| anyhow::anyhow!("unknown tool in XML fallback: {}", call.name))?;
        let arguments = call
            .arguments
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("XML tool call arguments must be an object"))?;
        if let Some(required) = spec.function.parameters["required"].as_array() {
            for key in required.iter().filter_map(serde_json::Value::as_str) {
                if !arguments.contains_key(key) {
                    anyhow::bail!("repaired tool call missing required argument `{key}`");
                }
            }
        }
        if let Some(properties) = spec.function.parameters["properties"].as_object() {
            for (key, value) in arguments {
                let Some(expected) = properties
                    .get(key)
                    .and_then(|property| property["type"].as_str())
                else {
                    continue;
                };
                let matches = match expected {
                    "string" => value.is_string(),
                    "integer" => value.is_i64() || value.is_u64(),
                    "boolean" => value.is_boolean(),
                    "object" => value.is_object(),
                    "array" => value.is_array(),
                    _ => true,
                };
                if !matches {
                    anyhow::bail!("repaired tool call argument `{key}` must be {expected}");
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::registry::ToolRegistry;

    #[test]
    fn declaration_controls_capability_without_changing_absent_default() {
        assert!(native_tools_enabled(None, true, false));
        assert!(!native_tools_enabled(None, false, false));
        assert!(native_tools_enabled(
            Some(ToolProtocol::Native),
            true,
            false
        ));
        assert!(!native_tools_enabled(Some(ToolProtocol::Text), true, false));
        assert!(!native_tools_enabled(
            Some(ToolProtocol::Native),
            true,
            true
        ));
    }

    #[test]
    fn explicit_text_reuses_xml_tool_parser() {
        let mut reply = AssistantReply {
            content: concat!(
                "working\n",
                "<anvil_tool_call name=\"Read\">",
                r#"{"path":"README.md"}"#,
                "</anvil_tool_call>"
            )
            .to_string(),
            tool_calls: Vec::new(),
            prompt_tokens: Some(10),
            completion_tokens: Some(4),
        };

        let repairs = normalize_text_reply(&mut reply, ToolRegistry::default().specs()).unwrap();

        assert!(repairs.is_empty());
        assert_eq!(reply.content, "working");
        assert_eq!(reply.tool_calls.len(), 1);
        assert_eq!(reply.tool_calls[0].name, "Read");
        assert_eq!(reply.tool_calls[0].arguments["path"], "README.md");
    }

    #[test]
    fn repair_is_rejected_when_required_arguments_do_not_validate() {
        let mut reply = AssistantReply::text(concat!(
            r#"<anvil_tool_call name="Write">{"path":"README.md"}"#,
            "}",
            "</anvil_tool_call>"
        ));

        assert!(normalize_text_reply(&mut reply, ToolRegistry::default().specs()).is_err());
    }
}
