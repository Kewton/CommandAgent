use std::ops::Range;

use serde_json::json;

use crate::config::Config;
use crate::state::ConversationMessage;

/// What one compaction pass left behind, so the over-budget fallback is observable as well as
/// reported. Not `#[must_use]`: the loop ahead of the request calls compaction for its effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompactionOutcome {
    /// The history's remaining character count after the pass.
    pub remaining_chars: usize,
    /// The character budget the pass aimed at (`context_budget * 4`).
    pub budget_chars: usize,
    /// `true` when nothing removable was left and the history is still over budget.
    pub over_budget: bool,
}

/// Compacts `messages` toward `context_budget`. When nothing removable is left and the history
/// is still over budget, the overage is reported on stderr here, before the caller builds the
/// request, so it is never sent silently.
pub fn compact_if_needed(
    messages: &mut Vec<ConversationMessage>,
    context_budget: usize,
) -> CompactionOutcome {
    let approx_chars = context_budget.saturating_mul(4);
    let mut total: usize = messages.iter().map(|m| m.content.len()).sum();
    while total > approx_chars {
        let protected = protected_indices(messages);
        let pinned = pinned_indices(messages);
        let groups = message_groups(messages);
        // Issue #1088: a reply's tool calls and the outputs that answer them are removed as one
        // run, so a part never outlives the whole and `check_correlation` never sees a call
        // without its output or an output that names no call. Issue #1097: this holds for a
        // single-call reply too, so a budget that falls inside the reply's text never keeps the
        // output while dropping the call. A run that holds protected evidence is spared while an
        // unprotected run can go; when only protected runs are left and the history is still over
        // budget, the oldest protected run is removed whole too, so the budget is served first.
        // The head system message and the latest user message never go.
        //
        // Issue #1096: the old count floor of four messages stopped here while a removable
        // protected run was still present. A whole call-and-output run that holds neither pinned
        // message is now removed at the floor too, so the budget is served before the floor; the
        // floor only spares a lone message that has no counterpart to leave with it. A run is a
        // paired call and output, so removing it never orphans the other half, and the latest
        // user message and head system message stay. When nothing removable is left the overage
        // is reported rather than sent silently.
        let group = if messages.len() > 4 {
            groups
                .iter()
                .find(|group| group.start >= 1 && !group_holds(group, &protected))
                .or_else(|| {
                    groups
                        .iter()
                        .find(|group| group.start >= 1 && !group_holds(group, &pinned))
                })
                .cloned()
        } else {
            groups
                .iter()
                .find(|group| {
                    group.start >= 1 && !group_holds(group, &pinned) && is_call_run(messages, group)
                })
                .cloned()
        };
        let Some(group) = group else {
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
    let over_budget = total > approx_chars;
    if over_budget {
        eprintln!(
            "warning: conversation history is over its compaction budget: {total} characters > \
             {approx_chars} (context budget {context_budget} tokens); no removable group is left, \
             so the request is sent as it is"
        );
    }
    CompactionOutcome {
        remaining_chars: total,
        budget_chars: approx_chars,
        over_budget,
    }
}

/// The event recorded when a history cannot be compacted under budget (Issue #1096). It is a
/// fixed identifier so the secret scrub never renames it.
pub const OVER_BUDGET_EVENT: &str = "compaction_over_budget";

/// Issue #1096: compacts through [`compact_if_needed`], then, when the history is still over
/// budget, records that state as one structured event through the existing `eval_events::emit`
/// path *before* the caller builds the request. A GUI that keeps only the event file (and drops
/// stderr) then still sees the overage rather than sending it silently. The event carries only
/// numbers, never message text. The stderr warning stays.
pub fn compact_and_record(
    messages: &mut Vec<ConversationMessage>,
    config: &Config,
) -> CompactionOutcome {
    let outcome = compact_if_needed(messages, config.context_budget);
    if outcome.over_budget {
        crate::eval_events::emit(
            config.eval_events_path.as_deref(),
            json!({
                "event": OVER_BUDGET_EVENT,
                "remaining_chars": outcome.remaining_chars,
                "budget_chars": outcome.budget_chars,
                "context_budget": config.context_budget,
            }),
        );
    }
    outcome
}

/// Whether a group is a call-and-output run: an assistant message with calls plus at least one
/// output answered by it. A lone assistant call with no output, or a lone tool output with no
/// call, is not a run, so removing one never orphans the other.
fn is_call_run(messages: &[ConversationMessage], group: &Range<usize>) -> bool {
    group.end - group.start >= 2
        && (group.start..group.end).any(|index| !messages[index].tool_calls.is_empty())
}

/// The message runs that must be kept or removed together. An assistant message whose calls must
/// stay with their outputs forms one run with the contiguous tool outputs that answer it, whether
/// it has one call or several (Issue #1097). An output belongs to the run only when its
/// `tool_call_id` is one of that assistant's calls, so a later reply's output is never mistaken
/// for this one's. Every other message is its own run.
fn message_groups(messages: &[ConversationMessage]) -> Vec<Range<usize>> {
    let mut groups = Vec::new();
    let mut start = 0;
    while start < messages.len() {
        let mut end = start + 1;
        if !messages[start].tool_calls.is_empty() {
            let calls = &messages[start].tool_calls;
            while end < messages.len()
                && messages[end].role == "tool"
                && messages[end]
                    .tool_call_id
                    .as_deref()
                    .is_some_and(|id| calls.iter().any(|call| call.id == id))
            {
                end += 1;
            }
        }
        groups.push(start..end);
        start = end;
    }
    groups
}

fn group_holds(group: &Range<usize>, indices: &[usize]) -> bool {
    (group.start..group.end).any(|index| indices.contains(&index))
}

/// The messages a compaction never removes: the head system prompt and the latest user message.
fn pinned_indices(messages: &[ConversationMessage]) -> Vec<usize> {
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
    indices
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
        // The only uncompactable run is the protected one; the message-count floor stops before
        // it, and when it does go its call goes with it, so no output is left without its call.
        let call_present = messages
            .iter()
            .any(|message| message.tool_calls.iter().any(|call| call.id == "call-r"));
        let output_present = messages
            .iter()
            .any(|message| message.tool_call_id.as_deref() == Some("call-r"));
        assert_eq!(call_present, output_present);
    }
}
