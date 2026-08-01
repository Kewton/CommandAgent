use crate::config::ToolProtocol;
use crate::providers::AssistantReply;
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
) -> anyhow::Result<()> {
    let allowed_tools = tools
        .iter()
        .map(|tool| tool.function.name.clone())
        .collect::<Vec<_>>();
    let (tool_calls, content) =
        crate::providers::xml_fallback::extract_tool_calls(&reply.content, &allowed_tools)?;
    reply.content = content;
    reply.tool_calls = tool_calls;
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

        normalize_text_reply(&mut reply, ToolRegistry::default().specs()).unwrap();

        assert_eq!(reply.content, "working");
        assert_eq!(reply.tool_calls.len(), 1);
        assert_eq!(reply.tool_calls[0].name, "Read");
        assert_eq!(reply.tool_calls[0].arguments["path"], "README.md");
    }
}
