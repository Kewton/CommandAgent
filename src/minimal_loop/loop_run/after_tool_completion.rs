//! Early completion of a run right after a tool batch (Issues #913, #921).
//!
//! A tool batch that leaves every required path in place may end the run with
//! `required_artifacts_satisfied_after_tool` before the model answers. For a
//! direct prompt the paths being present is not evidence that the request was
//! carried out, whether they existed at the start or the batch created them: a
//! successful `ls`, `cat`, or visible test run, an Edit that nobody looked at,
//! or a Write or `touch` that produced a file nobody ran would complete the
//! request. Such a run continues, and the model's next reply, its final answer,
//! and the existing checks decide. Plan steps, recovery, and profile or final
//! acceptance repair run as plan steps and keep the early completion.
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
/// direct prompt may not, whether its required paths existed at the start or
/// were created since.
pub(super) fn after_tool_stop_allowed(options: &RunSessionOptions) -> bool {
    !is_direct_prompt(options)
}

pub(super) struct AfterToolStop {
    final_text: String,
}

/// Emits the `required_artifacts_satisfied_after_tool` stop when the run may
/// end here; `None` keeps the loop going. Which required paths were missing at
/// the start no longer decides the stop.
pub(super) fn stop(
    eval_events_path: Option<&Path>,
    options: &RunSessionOptions,
    required_paths: &[String],
    _initially_missing_paths: &[String],
) -> Option<AfterToolStop> {
    if !after_tool_stop_allowed(options) {
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

/// What may follow `直して` without turning it into a refusal: `直しては`
/// (prohibition), `直してほしくない`, `直してもらう必要はない`, `直していらない`.
const NOT_A_REQUEST_AFTER_SHITE: [&str; 7] = [
    "は",
    "ほしくな",
    "欲しくな",
    "もらう必要はな",
    "もらわなくて",
    "くれなくて",
    "いらな",
];

/// `直す` ends a request when the sentence ends, or when `こと` / `ように`
/// follows without a prohibition; `直すのではなく`, `直す方法`, `直すべき`,
/// and `直すな` are not requests.
const REQUEST_END_AFTER_SU: [char; 16] = [
    '。', '、', '．', '，', '.', ',', '！', '？', '!', '?', ' ', '　', '\n', '）', ')', '」',
];
const REQUEST_SUFFIX_AFTER_SU: [&str; 2] = ["こと", "よう"];
const PROHIBITION_AFTER_SUFFIX: [&str; 8] = [
    "はしな",
    "はな",
    "は不要",
    "は禁止",
    "は駄目",
    "はだめ",
    "はいけな",
    "をしな",
];

fn japanese_repair_request(text: &str) -> bool {
    text.match_indices('直').any(|(at, matched)| {
        let before = text[..at].chars().next_back();
        let after = &text[at + matched.len()..];
        !before.is_some_and(|ch| NOT_REPAIR_BEFORE.contains(&ch))
            && (after.strip_prefix("して").is_some_and(|rest| {
                !NOT_A_REQUEST_AFTER_SHITE
                    .iter()
                    .any(|refusal| rest.starts_with(refusal))
            }) || after.strip_prefix('す').is_some_and(su_ends_a_request))
    })
}

fn su_ends_a_request(rest: &str) -> bool {
    match rest.chars().next() {
        None => true,
        Some(ch) if REQUEST_END_AFTER_SU.contains(&ch) => true,
        Some(_) => REQUEST_SUFFIX_AFTER_SU.iter().any(|suffix| {
            rest.strip_prefix(suffix).is_some_and(|tail| {
                !PROHIBITION_AFTER_SUFFIX
                    .iter()
                    .any(|prohibition| tail.starts_with(prohibition))
            })
        }),
    }
}

const NEGATIONS: [&str; 7] = ["not", "never", "don't", "dont", "without", "no", "nor"];
/// How many words before `repair` a negation of the same sentence reaches
/// (`do not try to repair`, `never attempt to repair`).
const NEGATION_REACH: usize = 4;

fn english_repair_request(text: &str) -> bool {
    let lower = text.to_lowercase().replace('’', "'");
    english_sentences(&lower).into_iter().any(|sentence| {
        let words = sentence
            .split(|ch: char| !(ch.is_alphanumeric() || "_-./\\'".contains(ch)))
            .map(|word| word.trim_end_matches(['.', '\'']))
            .filter(|word| !word.is_empty())
            .collect::<Vec<_>>();
        words.iter().enumerate().any(|(index, word)| {
            *word == "repair"
                && !words[index.saturating_sub(NEGATION_REACH)..index]
                    .iter()
                    .any(|previous| NEGATIONS.contains(previous))
        })
    })
}

/// Splits at `;`, `!`, `?`, a line break, or a `.` that ends a sentence (not
/// the one inside `dates.py`).
fn english_sentences(text: &str) -> Vec<&str> {
    let mut bounds = vec![0];
    let mut chars = text.char_indices().peekable();
    while let Some((at, ch)) = chars.next() {
        let next = chars.peek().map(|(_, next)| *next);
        if matches!(ch, ';' | '!' | '?' | '\n')
            || (ch == '.' && next.is_none_or(char::is_whitespace))
        {
            bounds.push(at + ch.len_utf8());
        }
    }
    bounds.push(text.len());
    bounds
        .windows(2)
        .map(|pair| &text[pair[0]..pair[1]])
        .collect()
}
