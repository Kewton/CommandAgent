//! Early completion of a run right after a tool batch (Issue #913).
//!
//! A tool batch that leaves every required path in place may end the run with
//! `required_artifacts_satisfied_after_tool` before the model answers. For a
//! direct prompt whose required paths all existed when the run started, the
//! paths being present is not evidence that the request was carried out: a
//! successful `ls`, `cat`, or visible test run, or an Edit that nobody looked
//! at, would complete a request to repair those files. Such a run continues,
//! and the model's final answer and the existing checks decide. Plan steps,
//! recovery, and profile or final acceptance repair run as plan steps and keep
//! the early completion, as does a direct prompt that created a missing path.
//!
//! The action-prompt vocabulary lives here too. A direct prompt also counts a
//! request to repair (`直して`, `直す`, `repair`) as an action; plan steps keep
//! the vocabulary they had.

use std::path::Path;

use serde_json::json;

use crate::eval_events;
use crate::tools::path_guard::resolve_existing;

use super::{RunSessionOptions, RunSessionOutcome, RunSessionScope, RunStopReason, missing_paths};

#[cfg(test)]
mod tests;

pub(super) fn required_paths_satisfied_after_tool(
    root: &Path,
    required_paths: &[String],
    initially_missing_paths: &[String],
    progress_seen: bool,
) -> bool {
    if required_paths.is_empty() || !missing_paths(root, required_paths).is_empty() {
        return false;
    }
    progress_seen
        || initially_missing_paths
            .iter()
            .any(|path| resolve_existing(root, path).is_ok())
}

/// A direct prompt: the minimal loop outside any plan step.
fn is_direct_prompt(options: &RunSessionOptions) -> bool {
    options.scope == RunSessionScope::MinimalLoop && options.step_kind.is_none()
}

/// Whether satisfied required paths may end the run after a tool batch. A
/// direct prompt whose required paths all existed at the start may not.
pub(super) fn after_tool_stop_allowed(
    options: &RunSessionOptions,
    required_paths: &[String],
    initially_missing_paths: &[String],
) -> bool {
    !(is_direct_prompt(options) && !required_paths.is_empty() && initially_missing_paths.is_empty())
}

pub(super) struct AfterToolStop {
    final_text: String,
}

/// Emits the `required_artifacts_satisfied_after_tool` stop when the run may
/// end here; `None` keeps the loop going.
pub(super) fn stop(
    eval_events_path: Option<&Path>,
    options: &RunSessionOptions,
    required_paths: &[String],
    initially_missing_paths: &[String],
) -> Option<AfterToolStop> {
    if !after_tool_stop_allowed(options, required_paths, initially_missing_paths) {
        return None;
    }
    eval_events::emit(
        eval_events_path,
        json!({
            "event": "loop_stop",
            "reason": "required_artifacts_satisfied_after_tool",
            "required_paths": required_paths,
        }),
    );
    Some(AfterToolStop {
        final_text: format!(
            "required artifacts satisfied: {}",
            required_paths.join(", ")
        ),
    })
}

impl AfterToolStop {
    pub(super) fn into_outcome(
        self,
        changed_paths: Vec<String>,
        iterations: usize,
        tool_calls: usize,
        verify_attempts: usize,
        last_blocking_reason: Option<String>,
        last_provider_error: Option<String>,
    ) -> RunSessionOutcome {
        RunSessionOutcome {
            final_text: self.final_text,
            stop_reason: RunStopReason::RequiredArtifactsSatisfiedAfterTool,
            changed_paths,
            iterations,
            tool_calls,
            missing_required_paths: Vec::new(),
            missing_capabilities: Vec::new(),
            missing_evidence: Vec::new(),
            missing_obligations: Vec::new(),
            verify_attempts,
            last_blocking_reason,
            last_provider_error,
        }
    }
}

pub(super) fn looks_like_action_prompt(options: &RunSessionOptions, content: &str) -> bool {
    uses_action_vocabulary(content) || (is_direct_prompt(options) && requests_repair(content))
}

/// The vocabulary every scope shares.
fn uses_action_vocabulary(content: &str) -> bool {
    let lower = content.to_ascii_lowercase();
    lower.contains("create")
        || lower.contains("write")
        || lower.contains("edit")
        || lower.contains("fix")
        || lower.contains("implement")
        || lower.contains("add ")
        || lower.contains("build")
        || lower.contains("作成")
        || lower.contains("実装")
        || lower.contains("修正")
        || lower.contains("追加")
}

/// A request to repair, outside quoted text, code spans, and paths. `見直し`
/// (review), a prohibition (`直すな`, `do not repair`), and words that merely
/// contain the letters (`repair_notes.md`) are not requests.
fn requests_repair(content: &str) -> bool {
    let text = without_quoted_spans(content);
    japanese_repair_request(&text) || english_repair_request(&text)
}

const QUOTE_PAIRS: [(char, char); 5] = [
    ('`', '`'),
    ('"', '"'),
    ('“', '”'),
    ('「', '」'),
    ('『', '』'),
];

fn without_quoted_spans(content: &str) -> String {
    let chars = content.chars().collect::<Vec<_>>();
    let mut out = String::new();
    let mut index = 0;
    while index < chars.len() {
        let close = QUOTE_PAIRS
            .iter()
            .find(|(open, _)| *open == chars[index])
            .and_then(|(_, close)| {
                chars[index + 1..]
                    .iter()
                    .position(|ch| ch == close)
                    .map(|offset| index + 1 + offset)
            });
        match close {
            Some(close) => {
                out.push(' ');
                index = close + 1;
            }
            None => {
                out.push(chars[index]);
                index += 1;
            }
        }
    }
    out
}

/// Characters that make `直` part of another word (`見直す` is to review,
/// `正直` is honest).
const NOT_REPAIR_BEFORE: [char; 5] = ['見', '正', '素', '率', '垂'];

fn japanese_repair_request(text: &str) -> bool {
    text.match_indices('直').any(|(at, matched)| {
        let before = text[..at].chars().next_back();
        let after = &text[at + matched.len()..];
        !before.is_some_and(|ch| NOT_REPAIR_BEFORE.contains(&ch))
            && ((after.starts_with("して") && !after.starts_with("しては"))
                || (after.starts_with('す') && !after.starts_with("すな")))
    })
}

const NEGATIONS: [&str; 7] = ["not", "never", "don't", "dont", "without", "no", "nor"];

fn english_repair_request(text: &str) -> bool {
    let lower = text.to_lowercase().replace('’', "'");
    let words = lower
        .split(|ch: char| !(ch.is_alphanumeric() || "_-./\\'".contains(ch)))
        .map(|word| word.trim_end_matches(['.', '\'']))
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    words.iter().enumerate().any(|(index, word)| {
        *word == "repair" && (index == 0 || !NEGATIONS.contains(&words[index - 1]))
    })
}
