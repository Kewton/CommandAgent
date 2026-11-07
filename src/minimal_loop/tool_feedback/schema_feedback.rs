//! Issue #997: a profile tool's schema refusal carries its violations as one
//! JSON token. A long one does not fit the 500-character history snippet, whose
//! whole-token cut then drops it and leaves the model `schema_violation:` only.
//! A short summary of fixed codes and bounded pointers goes first, so at least
//! one whole violation and the counts stay inside the snippet. The refusal's
//! own text follows unchanged; the snippet, its limit and its scrub are not
//! touched here.

use serde_json::Value;

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

fn schema_violation(name: &str, error: &anyhow::Error) -> Option<String> {
    let text = error.to_string();
    let summary = summary(&text)?;
    Some(format!(
        "Tool call `{name}` was rejected with a recoverable validation error: {summary} {text}. Retry with the same tool or another available tool using a valid JSON object that matches the tool schema."
    ))
}

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
    Some(format!(
        "schema_violation stage={} violations={} shown={} omitted={}: {}",
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
/// cut. One token: no whitespace, control or invisible format character.
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
                | '\u{FEFF}'
                | '\u{FFF9}'..='\u{FFFD}'
        ))
}

#[cfg(test)]
mod tests {
    use super::{MAX_POINTER_CHARS, summary};

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
        let first = tokens[6].strip_prefix("unknown_key@").unwrap();
        assert_eq!(first.chars().count(), MAX_POINTER_CHARS, "{text}");
        assert!(first.ends_with("..."), "{text}");
        assert_eq!(tokens[7], "unknown_key@/a?b?c?d?e?f", "{text}");
        assert_eq!(tokens.len(), 8, "{text}");
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
