use std::path::Path;

use crate::state::ConversationMessage;
use crate::tools::registry::ToolSpec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolPromptMode {
    Native,
    XmlFallback,
}

/// The optional system prompt blocks: the profile guidance and, for a direct
/// prompt run that adopted a request pattern (Issue #918), the request guidance.
#[derive(Debug, Clone, Copy, Default)]
pub struct PromptGuidance<'a> {
    pub profile: Option<&'a str>,
    pub request: Option<&'a str>,
}

pub fn build_request_messages(
    history: &[ConversationMessage],
    tools: &[ToolSpec],
    root: &Path,
    pending_feedback: Option<&str>,
    profile_guidance: Option<&str>,
    mode: ToolPromptMode,
) -> Vec<ConversationMessage> {
    build_guided_request_messages(
        history,
        tools,
        root,
        pending_feedback,
        PromptGuidance {
            profile: profile_guidance,
            request: None,
        },
        mode,
    )
}

pub fn build_guided_request_messages(
    history: &[ConversationMessage],
    tools: &[ToolSpec],
    root: &Path,
    pending_feedback: Option<&str>,
    guidance: PromptGuidance<'_>,
    mode: ToolPromptMode,
) -> Vec<ConversationMessage> {
    let mut messages = Vec::new();
    messages.push(ConversationMessage::system(system_prompt(
        tools, root, guidance, mode,
    )));
    messages.extend(history.iter().map(sanitize_history_message_for_request));
    if let Some(feedback) = pending_feedback {
        messages.push(ConversationMessage::user(feedback.to_string()));
    }
    messages
}

fn system_prompt(
    tools: &[ToolSpec],
    root: &Path,
    guidance: PromptGuidance<'_>,
    mode: ToolPromptMode,
) -> String {
    let mut prompt = format!(
        "You are commandagent, a local coding agent. Work only inside workspace `{}`.\n\
Use tools for file changes, repository facts, file inspection, build/test checks, and any action that changes the workspace.\n\
Do not end with planned future work. Do not claim files, tests, or builds succeeded unless you observed them with tools.\n\
Use workspace-relative paths only, and do not read or write outside the workspace.\n\
Do not claim completion until requested files and evidence are present.",
        root.display(),
    );
    // A native turn sends each description in the function schema, so only a
    // turn without schemas lists the tools here (Issue #936).
    if mode == ToolPromptMode::XmlFallback {
        let tools = tools
            .iter()
            .map(|tool| format!("- {}: {}", tool.function.name, tool.function.description))
            .collect::<Vec<_>>()
            .join("\n");
        prompt.push_str("\nTools:\n");
        prompt.push_str(&tools);
    }
    if let Some(guidance) = guidance.profile {
        prompt.push_str("\n\nProfile guidance:\n");
        prompt.push_str(guidance);
    }
    if let Some(guidance) = guidance.request {
        prompt.push_str("\n\nRequest guidance:\n");
        prompt.push_str(guidance);
    }
    if mode == ToolPromptMode::XmlFallback {
        prompt.push_str(
            "\n\nNative tool calls are unavailable for this provider turn. Use XML fallback tool calls exactly like:\n\
<anvil_tool_call name=\"Read\">{\"path\":\"src/main.rs\"}</anvil_tool_call>\n\
The XML body must be one JSON object matching the tool schema.",
        );
    }
    prompt
}

fn sanitize_history_message_for_request(message: &ConversationMessage) -> ConversationMessage {
    if message.role == "assistant" && !message.tool_calls.is_empty() {
        let mut cloned = message.clone();
        cloned.content.clear();
        return cloned;
    }
    if message.role == "tool" {
        let mut cloned = message.clone();
        cloned.content =
            crate::state::tool_history::history_content(cloned.name.as_deref(), &cloned.content);
        return cloned;
    }
    message.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ToolCall;
    use crate::tools::registry::ToolRegistry;
    use serde_json::json;

    #[test]
    fn system_prompt_contains_source_safety_rules() {
        let messages = build_request_messages(
            &[],
            ToolRegistry::default().specs(),
            Path::new("/tmp/work"),
            None,
            None,
            ToolPromptMode::Native,
        );
        let prompt = &messages[0].content;
        assert!(prompt.contains("Do not end with planned future work"));
        assert!(prompt.contains("Do not claim files, tests, or builds succeeded"));
        assert!(prompt.contains("workspace-relative paths"));
    }

    #[test]
    fn xml_fallback_prompt_contains_tool_call_example() {
        let messages = build_request_messages(
            &[],
            ToolRegistry::default().specs(),
            Path::new("/tmp/work"),
            None,
            None,
            ToolPromptMode::XmlFallback,
        );
        let prompt = &messages[0].content;
        assert!(prompt.contains("<anvil_tool_call"));
        assert!(prompt.contains("one JSON object"));
        assert!(prompt.contains("Do not end with planned future work"));
    }

    #[test]
    fn only_the_xml_fallback_prompt_lists_the_tools() {
        let registry = ToolRegistry::default();
        let specs = registry.specs();
        let prompt = |mode| {
            build_request_messages(&[], specs, Path::new("/tmp/work"), None, None, mode)[0]
                .content
                .clone()
        };
        let native = prompt(ToolPromptMode::Native);
        let xml = prompt(ToolPromptMode::XmlFallback);
        assert!(!native.contains("Tools:"));
        assert!(xml.contains("Tools:\n- Bash: "));
        for spec in specs {
            assert!(!native.contains(&spec.function.description));
            assert!(xml.contains(&spec.function.description));
        }
        assert!(xml.starts_with(&native));
    }

    #[test]
    fn tool_call_assistant_preamble_is_not_reprompted() {
        let history = vec![ConversationMessage::assistant(
            "I will create it now.",
            vec![ToolCall::new(
                "Write",
                json!({"path":"a.txt","content":"ok"}),
            )],
        )];
        let messages = build_request_messages(
            &history,
            ToolRegistry::default().specs(),
            Path::new("/tmp/work"),
            None,
            Some(
                "For the nextjs profile, create a runnable Next.js app, not only package metadata.",
            ),
            ToolPromptMode::Native,
        );
        assert_eq!(messages[1].role, "assistant");
        assert!(messages[1].content.is_empty());
        assert_eq!(history[0].content, "I will create it now.");
        assert!(messages[0].content.contains("Profile guidance"));
    }

    #[test]
    fn tool_history_messages_are_bounded_before_requesting() {
        let history = vec![ConversationMessage {
            role: "tool".to_string(),
            content: format!("{}{}", "x".repeat(1200), "y".repeat(1200)),
            name: Some("Write".to_string()),
            tool_call_id: Some("tool-call-1".to_string()),
            tool_calls: Vec::new(),
        }];
        let messages = build_request_messages(
            &history,
            ToolRegistry::default().specs(),
            Path::new("/tmp/work"),
            None,
            None,
            ToolPromptMode::Native,
        );
        assert!(messages[1].content.chars().count() <= 500);
        assert!(messages[1].content.ends_with('x') || messages[1].content.ends_with('y'));
        assert!(history[0].content.chars().count() > messages[1].content.chars().count());
    }
}
