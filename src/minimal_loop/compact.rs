use std::ops::Range;

use crate::state::ConversationMessage;

pub fn compact_if_needed(messages: &mut Vec<ConversationMessage>, context_budget: usize) {
    let approx_chars = context_budget.saturating_mul(4);
    let mut total: usize = messages.iter().map(|m| m.content.len()).sum();
    while total > approx_chars && messages.len() > 4 {
        let protected = protected_indices(messages);
        let groups = message_groups(messages, &protected);
        // Issue #1088: a reply's tool calls and the outputs that answer them are removed
        // together or not at all when the reply has several calls, or when one of its outputs
        // is protected evidence. Removing a part alone leaves an output with no call, or a call
        // with no output, and `check_correlation` refuses such a history before the next request
        // leaves. A single call with unprotected outputs keeps the existing per-message rule, so
        // what is protected and the budget arithmetic are unchanged.
        let Some(group) = groups
            .iter()
            .find(|group| group_is_removable(group, &protected))
            .cloned()
        else {
            break;
        };
        let removed_chars: usize = messages[group.clone()]
            .iter()
            .map(|message| message.content.len())
            .sum();
        for index in group.rev() {
            messages.remove(index);
        }
        total = total.saturating_sub(removed_chars);
    }
}

/// The message runs that must be kept or removed together. An assistant message whose calls must
/// stay with their outputs forms one run with the contiguous tool outputs that answer them:
/// a reply with two or more calls, or a reply with at least one protected output. Every other
/// message is its own run, so a single call with unprotected outputs keeps the per-message rule.
/// A run that starts at the head or holds a protected message cannot be removed, so a protected
/// tool output never loses the call it answers.
fn message_groups(messages: &[ConversationMessage], protected: &[usize]) -> Vec<Range<usize>> {
    let mut groups = Vec::new();
    let mut start = 0;
    while start < messages.len() {
        let mut end = start + 1;
        if !messages[start].tool_calls.is_empty() {
            while end < messages.len() && messages[end].role == "tool" {
                end += 1;
            }
            let paired = messages[start].tool_calls.len() >= 2
                || (start..end).any(|index| protected.contains(&index));
            if !paired {
                end = start + 1;
            }
        }
        groups.push(start..end);
        start = end;
    }
    groups
}

fn group_is_removable(group: &Range<usize>, protected: &[usize]) -> bool {
    group.start >= 1 && !(group.start..group.end).any(|index| protected.contains(&index))
}

fn protected_indices(messages: &[ConversationMessage]) -> Vec<usize> {
    let mut indices = Vec::new();
    if messages
        .first()
        .is_some_and(|message| message.role == "system")
    {
        indices.push(0);
    }
    if let Some(index) = messages.iter().rposition(|message| message.role == "user") {
        indices.push(index);
    }
    for (index, message) in messages.iter().enumerate().rev() {
        if is_evidence_message(message) {
            indices.push(index);
            if indices.len() >= 6 {
                break;
            }
        }
    }
    indices.sort_unstable();
    indices.dedup();
    indices
}

fn is_evidence_message(message: &ConversationMessage) -> bool {
    if message.role != "tool" {
        return false;
    }
    match message.name.as_deref() {
        Some("Read") => true,
        Some("Edit") => {
            let lower = message.content.to_ascii_lowercase();
            lower.contains("error")
                || lower.contains("edit_")
                || lower.contains("recoverable")
                || lower.contains("anchor")
        }
        Some("Bash") => {
            let lower = message.content.to_ascii_lowercase();
            lower.contains("outcome: commandfailed")
                || lower.contains("outcome: timeout")
                || lower.contains("outcome: cancelled")
                || lower.contains("command failed")
                || lower.contains("error")
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ToolCall;
    use serde_json::json;

    #[test]
    fn compaction_preserves_latest_user_message() {
        let mut messages = vec![
            ConversationMessage::system("system"),
            ConversationMessage::user("old user".repeat(200)),
            ConversationMessage::assistant("old assistant".repeat(200), Vec::new()),
            ConversationMessage::user("latest user"),
            ConversationMessage::assistant("tail".repeat(200), Vec::new()),
        ];
        compact_if_needed(&mut messages, 16);
        assert!(
            messages
                .iter()
                .any(|message| message.content == "latest user")
        );
    }

    #[test]
    fn compaction_preserves_recent_read_evidence() {
        let mut messages = vec![
            ConversationMessage::system("system"),
            ConversationMessage::user("old user".repeat(200)),
            ConversationMessage::assistant("old assistant".repeat(200), Vec::new()),
            ConversationMessage::tool("Read", "important anchor"),
            ConversationMessage::user("latest user"),
            ConversationMessage::assistant("tail".repeat(200), Vec::new()),
        ];
        compact_if_needed(&mut messages, 16);
        assert!(
            messages
                .iter()
                .any(|message| message.name.as_deref() == Some("Read")
                    && message.content == "important anchor")
        );
    }

    #[test]
    fn compaction_preserves_recent_edit_error() {
        let mut messages = vec![
            ConversationMessage::system("system"),
            ConversationMessage::user("old user".repeat(200)),
            ConversationMessage::assistant("old assistant".repeat(200), Vec::new()),
            ConversationMessage::tool("Edit", "edit_anchor_not_found: missing"),
            ConversationMessage::user("latest user"),
            ConversationMessage::assistant("tail".repeat(200), Vec::new()),
        ];
        compact_if_needed(&mut messages, 16);
        assert!(
            messages
                .iter()
                .any(|message| message.name.as_deref() == Some("Edit")
                    && message.content.contains("edit_anchor_not_found"))
        );
    }

    #[test]
    fn compaction_removes_a_multi_call_group_whole() {
        let first = ToolCall {
            id: "call-a".to_string(),
            name: "Bash".to_string(),
            arguments: json!({"command": "true"}),
        };
        let second = ToolCall {
            id: "call-b".to_string(),
            name: "Bash".to_string(),
            arguments: json!({"command": "true"}),
        };
        let mut messages = vec![
            ConversationMessage::system("system"),
            ConversationMessage::assistant("".to_string(), vec![first.clone(), second.clone()]),
            ConversationMessage::tool_result(
                "Bash",
                Some("call-a"),
                "outcome: Success\n".repeat(100),
            ),
            ConversationMessage::tool_result(
                "Bash",
                Some("call-b"),
                "outcome: Success\n".repeat(100),
            ),
            ConversationMessage::user("latest user"),
        ];
        compact_if_needed(&mut messages, 1);
        assert!(messages.iter().all(|message| message.tool_calls.is_empty()));
        assert!(messages.iter().all(|message| message.role != "tool"));
    }

    #[test]
    fn compaction_keeps_a_single_call_with_its_protected_output() {
        let call = ToolCall {
            id: "call-r".to_string(),
            name: "Read".to_string(),
            arguments: json!({"path": "src/a.ts"}),
        };
        let mut messages = vec![
            ConversationMessage::system("system"),
            ConversationMessage::assistant("".to_string(), vec![call]),
            ConversationMessage::tool_result("Read", Some("call-r"), "src/a.ts\nanchor"),
            ConversationMessage::user("latest user"),
            ConversationMessage::assistant("tail".repeat(400), Vec::new()),
        ];
        compact_if_needed(&mut messages, 1);
        // The protected Read output cannot be dropped, and neither can the call it answers.
        assert!(
            messages
                .iter()
                .any(|message| message.content == "src/a.ts\nanchor")
        );
        assert!(
            messages
                .iter()
                .any(|message| message.tool_calls.iter().any(|call| call.id == "call-r"))
        );
    }
}
