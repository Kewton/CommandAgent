//! The stage tools' history rule (Issue #1004).
//!
//! A stage tool's answer is the model's only view of what its operator's checks said, as `Read`
//! is of a file and `Bash` of a command. The process may send what the protocol's diagnostic
//! limits allow, so the history keeps up to a budget derived from those limits instead of the
//! 500-character snippet. Everything else is the snippet's rule, in the same order: the active
//! scrub, line breaks to spaces, the legacy heuristics, then a cut at whole tokens.
//!
//! Which names are stage tools is ambient on the session's thread: admission installs the
//! function names of the tools it joined to a stage set, and dropping the guard puts the
//! previous set back. Another thread sees none of them.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::sync::Arc;

use crate::tools::profile_tools::protocol::{
    MAX_DIAGNOSTIC_CODE_BYTES, MAX_DIAGNOSTIC_MESSAGE_BYTES, MAX_DIAGNOSTICS,
};

/// Upper bound, in UTF-8 bytes, of a stored stage tool answer: every diagnostic the protocol
/// accepts at its longest, plus 512 bytes for the rest of the answer's JSON (its fixed keys, the
/// syntax of each item, and the attempt counts).
pub(crate) const STAGE_ANSWER_MAX_BYTES: usize =
    512 + MAX_DIAGNOSTICS * (MAX_DIAGNOSTIC_CODE_BYTES + MAX_DIAGNOSTIC_MESSAGE_BYTES);

type StageNames = Arc<BTreeSet<String>>;

thread_local! {
    static CURRENT: RefCell<Option<StageNames>> = const { RefCell::new(None) };
}

/// Holds a session's stage tool names on its thread, and puts the previous set back.
#[derive(Debug)]
pub(crate) struct StageNamesGuard {
    previous: Option<StageNames>,
}

impl Drop for StageNamesGuard {
    fn drop(&mut self) {
        let previous = self.previous.take();
        CURRENT.with(|current| *current.borrow_mut() = previous);
    }
}

/// Installs `names` on this thread until the guard is dropped.
pub(crate) fn enter_stage_names(names: StageNames) -> StageNamesGuard {
    StageNamesGuard {
        previous: CURRENT.with(|current| current.borrow_mut().replace(names)),
    }
}

/// Whether `name` is a stage tool of the session on this thread.
pub(super) fn is_stage_tool(name: &str) -> bool {
    CURRENT.with(|current| {
        current
            .borrow()
            .as_deref()
            .is_some_and(|names| names.contains(name))
    })
}

/// The stage rule itself, whatever is installed on this thread.
pub(crate) fn stage_history_content(content: &str) -> String {
    flatten_and_cut(content, Measure::Bytes, STAGE_ANSWER_MAX_BYTES)
}

/// How a cut measures text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Measure {
    /// The snippet's measure. Only the test that holds this copy to the snippet's rule uses it.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the snippet's measure exists to prove the copy is the snippet's rule"
        )
    )]
    Chars,
    Bytes,
}

impl Measure {
    fn of(self, text: &str) -> usize {
        match self {
            Self::Chars => text.chars().count(),
            Self::Bytes => text.len(),
        }
    }

    /// The longest head of `text` within `limit`, ending on a char boundary.
    fn head(self, text: &str, limit: usize) -> String {
        match self {
            Self::Chars => text.chars().take(limit).collect(),
            Self::Bytes => {
                let mut end = limit.min(text.len());
                while !text.is_char_boundary(end) {
                    end -= 1;
                }
                text[..end].to_string()
            }
        }
    }
}

/// The steps of `eval_events::body_snippet_whole_tokens`, with the measure and the limit as
/// arguments. The whole value is scrubbed before anything is cut.
fn flatten_and_cut(body: &str, measure: Measure, limit: usize) -> String {
    let scrubbed = crate::sensitive_data::scrub_active(body);
    let mut clean = scrubbed.replace('\n', " ");
    clean = clean.replace('\r', " ");
    clean = crate::eval_events::scrub_legacy_heuristics(&clean);
    truncate_whole_tokens(&clean, measure, limit)
}

/// A copy of `eval_events::truncate_whole_tokens`, measuring with `measure`.
fn truncate_whole_tokens(value: &str, measure: Measure, limit: usize) -> String {
    if measure.of(value) <= limit {
        return value.to_string();
    }
    let mut out = String::new();
    let mut len = 0usize;
    for token in value.split_whitespace() {
        let token_len = measure.of(token);
        let next_len = len + usize::from(!out.is_empty()) + token_len;
        if next_len > limit {
            break;
        }
        if !out.is_empty() {
            out.push(' ');
            len += 1;
        }
        out.push_str(token);
        len += token_len;
    }
    if out.is_empty() {
        measure.head(value, limit)
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use super::{
        Measure, STAGE_ANSWER_MAX_BYTES, enter_stage_names, flatten_and_cut, is_stage_tool,
        stage_history_content,
    };
    use crate::eval_events::body_snippet_whole_tokens;
    use crate::sensitive_data::{SecretCatalog, install_scope, set_current};
    use crate::state::tool_history::history_content;

    const NAME: &str = "ProposeUpper";

    fn names(list: &[&str]) -> Arc<BTreeSet<String>> {
        Arc::new(list.iter().map(|name| (*name).to_string()).collect())
    }

    fn snippet_rule(text: &str) -> String {
        flatten_and_cut(text, Measure::Chars, 500)
    }

    /// Words of 9 characters, as many as fill `bytes`.
    fn words(bytes: usize) -> String {
        "abcdefgh ".repeat(bytes / 9 + 1)
    }

    #[test]
    fn the_stage_budget_is_derived_from_the_protocol_limits() {
        assert_eq!(STAGE_ANSWER_MAX_BYTES, 5_120);
        assert_eq!(
            STAGE_ANSWER_MAX_BYTES,
            512 + crate::tools::profile_tools::protocol::MAX_DIAGNOSTICS
                * (crate::tools::profile_tools::protocol::MAX_DIAGNOSTIC_CODE_BYTES
                    + crate::tools::profile_tools::protocol::MAX_DIAGNOSTIC_MESSAGE_BYTES)
        );
    }

    /// The copy with the snippet's measure and limit is the snippet, for every input that
    /// exercises a step: a carriage return, a registered secret over lines, a home path, a
    /// credential-like word, a word across the edge, and multibyte text.
    #[test]
    fn the_copied_rule_with_the_snippets_measure_is_the_snippet() {
        let secret = "fake-stage-history-secret-line-one\nline-two-of-the-fake-secret";
        let mut catalog = SecretCatalog::new();
        catalog.register(secret);
        install_scope(catalog, None, None);
        let across = format!("{}{}", "e".repeat(495), " crossingtheedgeword tail");
        let inputs = [
            "a\rb\r\nc".to_string(),
            format!("before {secret} after"),
            format!("{} {secret} {}", "w ".repeat(100), "z ".repeat(300)),
            "see /Users/someone/project/file.rs and /home/other/x".to_string(),
            "token sk-abcdefghijklmnopqrstuvwxyz0123456789 here".to_string(),
            across,
            "日本語の文 ".repeat(200),
            "日".repeat(800),
            "x".repeat(800),
            String::new(),
        ];
        let outcomes = inputs
            .iter()
            .map(|input| (snippet_rule(input), body_snippet_whole_tokens(input)))
            .collect::<Vec<_>>();
        set_current(None);
        for (index, (copied, snippet)) in outcomes.iter().enumerate() {
            assert_eq!(copied, snippet, "input {index}");
        }
        assert!(!outcomes[1].0.contains("line-two-of-the-fake-secret"));
        assert!(!outcomes[3].0.contains("someone"));
        assert!(!outcomes[4].0.contains("sk-abcdefghijklmnop"));
    }

    #[test]
    fn the_stage_rule_applied_twice_returns_the_same_text() {
        for input in [
            words(100),
            words(STAGE_ANSWER_MAX_BYTES * 2),
            "日".repeat(STAGE_ANSWER_MAX_BYTES),
            format!("a\nb\rc {}", "/home/ab/ ".repeat(800)),
        ] {
            let once = stage_history_content(&input);
            assert!(once.len() <= STAGE_ANSWER_MAX_BYTES);
            assert_eq!(stage_history_content(&once), once);
        }
    }

    /// The whole value is scrubbed first: a secret past where the snippet would have cut is
    /// still replaced, and so is a credential-like word.
    #[test]
    fn a_secret_past_the_snippets_edge_is_still_replaced() {
        let secret = "fake-stage-late-secret-0123456789";
        let mut catalog = SecretCatalog::new();
        catalog.register(secret);
        install_scope(catalog, None, None);
        let input = format!(
            "{} {secret} sk-abcdefghijklmnopqrstuvwxyz0123456789 end",
            "q".repeat(600)
        );
        let kept = stage_history_content(&input);
        set_current(None);
        assert!(input.chars().count() > 600);
        assert!(!kept.contains(secret));
        assert!(!kept.contains("fake-stage-late"));
        assert!(!kept.contains("sk-abcdefghijklmnop"));
        assert!(kept.ends_with("end"), "{kept}");
    }

    /// No word fits: one word longer than the budget, or multibyte characters only. The head is
    /// cut within the budget, on a char boundary.
    #[test]
    fn a_text_with_no_word_that_fits_is_cut_on_a_char_boundary() {
        for prefix in 0..4 {
            for body in ["q", "日", "€", "😀"] {
                let input = format!(
                    "{}{}",
                    "x".repeat(prefix),
                    body.repeat(STAGE_ANSWER_MAX_BYTES)
                );
                let kept = stage_history_content(&input);
                assert!(kept.len() <= STAGE_ANSWER_MAX_BYTES, "{prefix} {body}");
                assert!(kept.len() > STAGE_ANSWER_MAX_BYTES - 4, "{prefix} {body}");
                assert!(input.starts_with(&kept));
            }
        }
    }

    #[test]
    fn a_guard_makes_the_name_a_stage_tool_until_it_is_dropped() {
        let long = words(2_000);
        assert!(!is_stage_tool(NAME));
        assert_eq!(
            history_content(Some(NAME), &long),
            body_snippet_whole_tokens(&long)
        );
        let guard = enter_stage_names(names(&[NAME]));
        assert!(is_stage_tool(NAME));
        let kept = history_content(Some(NAME), &long);
        assert_eq!(kept, stage_history_content(&long));
        assert!(kept.len() > 500 && kept.len() <= STAGE_ANSWER_MAX_BYTES);
        // A name not in the set, and no name, keep the snippet.
        for name in [Some("ProposeOther"), Some("stage-upper-tool"), None] {
            assert_eq!(
                history_content(name, &long),
                body_snippet_whole_tokens(&long)
            );
        }
        drop(guard);
        assert!(!is_stage_tool(NAME));
        assert_eq!(
            history_content(Some(NAME), &long),
            body_snippet_whole_tokens(&long)
        );
    }

    #[test]
    fn another_thread_does_not_see_the_names() {
        let _guard = enter_stage_names(names(&[NAME]));
        assert!(is_stage_tool(NAME));
        let long = words(2_000);
        let (seen, kept) =
            std::thread::spawn(move || (is_stage_tool(NAME), history_content(Some(NAME), &long)))
                .join()
                .unwrap();
        assert!(!seen);
        assert_eq!(kept, body_snippet_whole_tokens(&words(2_000)));
    }

    #[test]
    fn a_nested_guard_puts_the_outer_set_back_when_dropped() {
        let outer = enter_stage_names(names(&["Outer"]));
        {
            let _inner = enter_stage_names(names(&["Inner"]));
            assert!(is_stage_tool("Inner"));
            assert!(!is_stage_tool("Outer"));
        }
        assert!(is_stage_tool("Outer"));
        assert!(!is_stage_tool("Inner"));
        drop(outer);
        assert!(!is_stage_tool("Outer"));
    }
}
