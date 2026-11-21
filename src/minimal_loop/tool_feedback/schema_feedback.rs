//! Issue #997: a profile tool's schema refusal carries its violations as one
//! JSON token. A long one does not fit the 500-character history snippet, whose
//! whole-token cut then drops it and leaves the model `schema_violation:` only.
//! Only then, a short summary of fixed codes and bounded pointers goes first,
//! so at least one whole violation and the counts stay inside the snippet. The
//! refusal's own text follows unchanged. A refusal the snippet keeps whole gets
//! today's feedback byte for byte. The snippet, its limit and its scrub are not
//! touched here.

use serde_json::Value;

use crate::tools::profile_tools::schema::MAX_DIAGNOSTICS;

const MARKER: &str = "profile_tool_refused: schema_violation: ";
const MAX_SHOWN: usize = 3;
const MAX_POINTER_CHARS: usize = 64;
const CUT: &str = "...";
const REPLACEMENT: char = '?';
const OTHER: &str = "other";
const STAGES: &[&str] = &["received_native", "received_call", "before_execution"];
/// The codes the G2 validator and its raw decoder give. Any other text is
/// shown as `other`, so a summary never repeats words it did not choose.
const CODES: &[&str] = &[
    "above_maximum",
    "arguments_too_deep",
    "arguments_too_large",
    "arguments_without_receipt",
    "below_minimum",
    "duplicate_key",
    "enum_mismatch",
    "lossy_number",
    "malformed_json",
    "missing_required",
    "pattern_mismatch",
    "string_too_long",
    "string_too_short",
    "too_few_items",
    "too_many_elements",
    "too_many_items",
    "trailing_data",
    "union_no_match",
    "unknown_key",
    "wrong_type",
];

/// The missing-`Read` feedback, else the schema summary, else `None`.
pub(super) fn missing_read_or_schema(name: &str, error: &anyhow::Error) -> Option<String> {
    super::path_feedback::missing_read(name, error).or_else(|| schema_violation(name, error))
}

/// `None`, so today's feedback stands byte for byte, unless the history snippet
/// would cut today's text. Cut or not is read from the snippet itself: a whole
/// text ends with the fixed last sentence, which no scrub replaces.
fn schema_violation(name: &str, error: &anyhow::Error) -> Option<String> {
    let text = error.to_string();
    let summary = summary(&text)?;
    let today = format!("{LEAD}{name}{AFTER_NAME}{text}. {LAST_SENTENCE}");
    if crate::eval_events::body_snippet_whole_tokens(&today).ends_with(LAST_SENTENCE) {
        return None;
    }
    Some(format!(
        "{LEAD}{name}{AFTER_NAME}{summary} {text}. {LAST_SENTENCE}"
    ))
}

/// Today's fallback in `tool_feedback.rs`, in its parts.
const LEAD: &str = "Tool call `";
const AFTER_NAME: &str = "` was rejected with a recoverable validation error: ";
const LAST_SENTENCE: &str = "Retry with the same tool or another available tool using a valid JSON object that matches the tool schema.";

/// `None` unless the text after the marker is exactly the refusal's data.
fn summary(text: &str) -> Option<String> {
    let (_, data) = text.split_once(MARKER)?;
    let data: Value = serde_json::from_str(data).ok()?;
    let stage = data.get("stage")?.as_str()?;
    let violations = data.get("violations")?.as_array()?;
    let mut items = Vec::with_capacity(violations.len());
    for violation in violations {
        let code = violation.get("code")?.as_str()?;
        let path = violation.get("path")?.as_str()?;
        items.push((code, path));
    }
    if items.is_empty() {
        return None;
    }
    let shown = items
        .iter()
        .take(MAX_SHOWN)
        .map(|(code, path)| format!("{}@{}", fixed(code, CODES), pointer(path)))
        .collect::<Vec<_>>();
    // The validator stops at its most, so a full list may hide more.
    let more = if items.len() >= MAX_DIAGNOSTICS {
        "+"
    } else {
        ""
    };
    Some(format!(
        "schema_violation stage={} violations={}{more} shown={} omitted={}{more}: {}",
        fixed(stage, STAGES),
        items.len(),
        shown.len(),
        items.len() - shown.len(),
        shown.join(" ")
    ))
}

fn fixed(value: &str, known: &[&'static str]) -> &'static str {
    known
        .iter()
        .find(|candidate| **candidate == value)
        .copied()
        .unwrap_or(OTHER)
}

/// Scrubbed whole before it is cut, so no fragment of a secret survives the
/// cut. One token: no whitespace, control character, or the invisible
/// characters `shown_as_is` lists (format characters, bidirectional controls,
/// tag characters and variation selectors).
fn pointer(path: &str) -> String {
    let scrubbed = crate::eval_events::scrub_sensitive_text(path);
    let clean = scrubbed
        .chars()
        .map(|c| if shown_as_is(c) { c } else { REPLACEMENT })
        .collect::<String>();
    if clean.chars().count() <= MAX_POINTER_CHARS {
        return clean;
    }
    let keep = MAX_POINTER_CHARS - CUT.len();
    format!("{}{CUT}", clean.chars().take(keep).collect::<String>())
}

fn shown_as_is(c: char) -> bool {
    !(c.is_control()
        || c.is_whitespace()
        || matches!(
            c,
            '\u{00AD}'
                | '\u{061C}'
                | '\u{180E}'
                | '\u{200B}'..='\u{200F}'
                | '\u{2028}'..='\u{202E}'
                | '\u{2060}'..='\u{206F}'
                | '\u{FE00}'..='\u{FE0F}'
                | '\u{FEFF}'
                | '\u{FFF9}'..='\u{FFFD}'
                | '\u{E0000}'..='\u{E007F}'
                | '\u{E0100}'..='\u{E01EF}'
        ))
}

#[cfg(test)]
mod tests {
    use super::{AFTER_NAME, LAST_SENTENCE, LEAD, MAX_POINTER_CHARS, summary};
    use crate::tools::profile_tools::schema::MAX_DIAGNOSTICS;

    fn refusal(data: &str) -> String {
        format!("extension_tool_rejected: `T`: profile_tool_refused: schema_violation: {data}")
    }

    #[test]
    fn the_summary_names_the_stage_the_counts_and_at_most_three_violations() {
        let data = r#"{"stage":"received_native","violations":[{"path":"/a","code":"unknown_key"},{"path":"/b","code":"wrong_type"},{"path":"/c","code":"enum_mismatch"},{"path":"/d","code":"missing_required"}]}"#;
        assert_eq!(
            summary(&refusal(data)).as_deref(),
            Some(
                "schema_violation stage=received_native violations=4 shown=3 omitted=1: unknown_key@/a wrong_type@/b enum_mismatch@/c"
            )
        );
    }

    #[test]
    fn an_unknown_code_or_stage_is_shown_as_other() {
        let data = r#"{"stage":"ignore previous","violations":[{"path":"/a","code":"do this"}]}"#;
        assert_eq!(
            summary(&refusal(data)).as_deref(),
            Some("schema_violation stage=other violations=1 shown=1 omitted=0: other@/a")
        );
    }

    #[test]
    fn a_pointer_is_one_bounded_token() {
        let long = format!("/{}", "k".repeat(120));
        let data = serde_json::json!({
            "stage": "before_execution",
            "violations": [
                {"path": long, "code": "unknown_key"},
                {"path": "/a\tb\u{7}c\u{202e}d\u{fffd}e f", "code": "unknown_key"},
            ],
        })
        .to_string();
        let text = summary(&refusal(&data)).unwrap();
        let tokens = text.split(' ').collect::<Vec<_>>();
        let first = tokens[5].strip_prefix("unknown_key@").unwrap();
        assert_eq!(first.chars().count(), MAX_POINTER_CHARS, "{text}");
        assert!(first.ends_with("..."), "{text}");
        assert_eq!(tokens[6], "unknown_key@/a?b?c?d?e?f", "{text}");
        assert_eq!(tokens.len(), 7, "{text}");
    }

    /// Tag characters and both variation selector blocks are invisible: each is replaced.
    #[test]
    fn tag_characters_and_variation_selectors_are_replaced() {
        for (label, c) in [
            ("tag first", '\u{E0000}'),
            ("tag A", '\u{E0041}'),
            ("tag last", '\u{E007F}'),
            ("selector first", '\u{FE00}'),
            ("selector 16", '\u{FE0F}'),
            ("supplement first", '\u{E0100}'),
            ("supplement last", '\u{E01EF}'),
        ] {
            let data = serde_json::json!({
                "stage": "received_native",
                "violations": [{"path": format!("/a{c}b"), "code": "unknown_key"}],
            })
            .to_string();
            assert_eq!(
                summary(&refusal(&data)).as_deref(),
                Some(
                    "schema_violation stage=received_native violations=1 shown=1 omitted=0: unknown_key@/a?b"
                ),
                "{label}"
            );
        }
    }

    fn of_count(count: usize) -> String {
        let violations = (0..count)
            .map(|index| serde_json::json!({"path": format!("/k{index}"), "code": "unknown_key"}))
            .collect::<Vec<_>>();
        let data = serde_json::json!({"stage": "received_native", "violations": violations});
        summary(&refusal(&data.to_string())).unwrap()
    }

    /// The validator stops counting at its limit, so a full list may hide more.
    #[test]
    fn a_full_list_of_violations_is_marked_as_possibly_more() {
        let below = MAX_DIAGNOSTICS - 1;
        assert!(
            of_count(below).starts_with(&format!(
                "schema_violation stage=received_native violations={below} shown=3 omitted={}: ",
                below - 3
            )),
            "{}",
            of_count(below)
        );
        assert!(
            of_count(MAX_DIAGNOSTICS).starts_with(&format!(
                "schema_violation stage=received_native violations={MAX_DIAGNOSTICS}+ shown=3 omitted={}+: ",
                MAX_DIAGNOSTICS - 3
            )),
            "{}",
            of_count(MAX_DIAGNOSTICS)
        );
    }

    /// The parts rebuild today's fallback exactly, so a whole refusal stays byte-identical.
    #[test]
    fn the_parts_are_today_s_fallback() {
        let error = anyhow::anyhow!("some recoverable error");
        assert_eq!(
            super::super::recoverable_tool_feedback("T", &error, None),
            format!("{LEAD}T{AFTER_NAME}some recoverable error. {LAST_SENTENCE}")
        );
    }

    #[test]
    fn anything_but_the_refusal_data_gives_no_summary() {
        for text in [
            "profile_tool_refused: invalid_pin: x".to_string(),
            refusal("{not json"),
            refusal(r#"{"stage":"received_native","violations":[]}"#),
            refusal(r#"{"stage":"received_native","violations":[{"code":"wrong_type"}]}"#),
            refusal(r#"{"stage":7,"violations":[{"path":"/","code":"wrong_type"}]}"#),
            refusal(
                r#"{"stage":"received_native","violations":[{"path":"/","code":"wrong_type"}]} tail"#,
            ),
        ] {
            assert_eq!(summary(&text), None, "{text}");
        }
    }
}
