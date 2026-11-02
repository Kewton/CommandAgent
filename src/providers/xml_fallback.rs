use std::ops::Range;

use serde_json::Value;

use crate::state::ToolCall;
use crate::tools::args_recovery::recover_tool_arguments;

/// The text the parser reads, which it closes up as it removes `<think>` blocks and calls,
/// with where each remaining part lay in the original.
struct RemainingText {
    text: String,
    /// In order: the original offset and the length of each contiguous part.
    parts: Vec<(usize, usize)>,
}

impl RemainingText {
    fn new(input: &str) -> Self {
        Self {
            text: input.to_string(),
            parts: vec![(0, input.len())],
        }
    }

    fn strip_think_tags(&mut self) {
        while let Some(start) = self.text.find("<think>") {
            let Some(end_rel) = self.text[start..].find("</think>") else {
                break;
            };
            self.remove(start..start + end_rel + "</think>".len());
        }
    }

    fn remove(&mut self, range: Range<usize>) {
        self.text.replace_range(range.clone(), "");
        let mut parts = Vec::with_capacity(self.parts.len() + 1);
        let mut at = 0;
        for &(original, length) in &self.parts {
            let kept_before = range.start.saturating_sub(at).min(length);
            if kept_before > 0 {
                parts.push((original, kept_before));
            }
            let dropped = range.end.saturating_sub(at).min(length);
            if dropped < length {
                parts.push((original + dropped, length - dropped));
            }
            at += length;
        }
        self.parts = parts;
    }

    /// Where `range` of the current text came from, and whether something between its ends
    /// has been removed.
    fn locate(&self, range: &Range<usize>) -> (Range<usize>, bool) {
        let (first_part, start) = self.original_of(range.start);
        let (last_part, last) = self.original_of(range.end.saturating_sub(1).max(range.start));
        (start..last + 1, first_part != last_part)
    }

    fn original_of(&self, offset: usize) -> (usize, usize) {
        let mut at = 0;
        for (index, &(original, length)) in self.parts.iter().enumerate() {
            if offset < at + length {
                return (index, original + offset - at);
            }
            at += length;
        }
        (self.parts.len(), offset)
    }

    /// Takes one parsed call out of the text.
    fn take(&mut self, call: ToolCall, range: Range<usize>, calls: &mut Vec<LocatedToolCall>) {
        let (span, spliced) = self.locate(&range);
        calls.push(LocatedToolCall {
            call,
            span,
            spliced,
        });
        self.remove(range);
    }
}

/// One call and the tag it was read from.
#[derive(Debug, Clone, PartialEq)]
pub struct LocatedToolCall {
    pub call: ToolCall,
    /// From the start of the tag to its end, in the original text.
    pub span: Range<usize>,
    /// Whether the parser removed something inside `span` before it read this call. The body
    /// it read is then not consecutive text of the original.
    pub spliced: bool,
}

pub fn strip_think_tags(input: &str) -> String {
    let mut remaining = RemainingText::new(input);
    remaining.strip_think_tags();
    remaining.text
}

pub fn extract_tool_calls(
    input: &str,
    allowed_tools: &[String],
) -> anyhow::Result<(Vec<ToolCall>, String)> {
    let (located, content) = extract_located(input, allowed_tools)?;
    let calls = located.into_iter().map(|located| located.call).collect();
    Ok((calls, content))
}

/// [`extract_tool_calls`], with where each call was read from.
pub fn extract_located(
    input: &str,
    allowed_tools: &[String],
) -> anyhow::Result<(Vec<LocatedToolCall>, String)> {
    let mut remaining = RemainingText::new(input);
    remaining.strip_think_tags();
    let mut calls = Vec::new();
    for tag in ["anvil_tool_call", "tool_call", "function_call"] {
        extract_plain_tag_calls(&mut remaining, tag, allowed_tools, &mut calls)?;
    }
    for tag in ["anvil_tool_call", "function"] {
        extract_named_tag_calls(&mut remaining, tag, allowed_tools, &mut calls)?;
    }
    extract_function_equals_calls(&mut remaining, allowed_tools, &mut calls)?;
    Ok((calls, remaining.text.trim().to_string()))
}

fn extract_plain_tag_calls(
    remaining: &mut RemainingText,
    tag: &str,
    allowed_tools: &[String],
    calls: &mut Vec<LocatedToolCall>,
) -> anyhow::Result<()> {
    loop {
        let open = format!("<{tag}>");
        let close = format!("</{tag}>");
        let Some(start) = remaining.text.find(&open) else {
            break;
        };
        let json_start = start + open.len();
        let Some(end_rel) = remaining.text[json_start..].find(&close) else {
            let raw = remaining.text[json_start..].trim();
            if json_looks_closed(raw) {
                let call = parse_tool_call(raw, None, allowed_tools)?;
                let end = remaining.text.len();
                remaining.take(call, start..end, calls);
                continue;
            }
            return Err(anyhow::anyhow!("malformed XML tool call"));
        };
        let json_end = json_start + end_rel;
        let raw = remaining.text[json_start..json_end].trim();
        let call = parse_tool_call(raw, None, allowed_tools)?;
        remaining.take(call, start..json_end + close.len(), calls);
    }
    Ok(())
}

fn extract_named_tag_calls(
    remaining: &mut RemainingText,
    tag: &str,
    allowed_tools: &[String],
    calls: &mut Vec<LocatedToolCall>,
) -> anyhow::Result<()> {
    let open_prefix = format!("<{tag} name=\"");
    let close = format!("</{tag}>");
    while let Some(start) = remaining.text.find(&open_prefix) {
        let text = &remaining.text;
        let name_start = start + open_prefix.len();
        let Some(name_end_rel) = text[name_start..].find('"') else {
            return Err(anyhow::anyhow!("malformed XML tool call"));
        };
        let name_end = name_start + name_end_rel;
        let name = text[name_start..name_end].to_string();
        let after_name = name_end + 1;
        let Some(gt_rel) = text[after_name..].find('>') else {
            return Err(anyhow::anyhow!("malformed XML tool call"));
        };
        let body_start = after_name + gt_rel + 1;
        let Some(end_rel) = text[body_start..].find(&close) else {
            return Err(anyhow::anyhow!("malformed XML tool call"));
        };
        let body_end = body_start + end_rel;
        let raw = text[body_start..body_end].trim();
        let call = parse_tool_call(raw, Some(&name), allowed_tools)?;
        remaining.take(call, start..body_end + close.len(), calls);
    }
    Ok(())
}

fn extract_function_equals_calls(
    remaining: &mut RemainingText,
    allowed_tools: &[String],
    calls: &mut Vec<LocatedToolCall>,
) -> anyhow::Result<()> {
    let open = "<function=";
    let close = "</function>";
    while let Some(start) = remaining.text.find(open) {
        let text = &remaining.text;
        let name_start = start + open.len();
        let Some(gt_rel) = text[name_start..].find('>') else {
            return Err(anyhow::anyhow!("malformed XML tool call"));
        };
        let name_end = name_start + gt_rel;
        let name = text[name_start..name_end]
            .trim()
            .trim_matches('"')
            .to_string();
        let body_start = name_end + 1;
        let Some(end_rel) = text[body_start..].find(close) else {
            return Err(anyhow::anyhow!("malformed XML tool call"));
        };
        let body_end = body_start + end_rel;
        let raw = text[body_start..body_end].trim();
        let call = parse_tool_call(raw, Some(&name), allowed_tools)?;
        remaining.take(call, start..body_end + close.len(), calls);
    }
    Ok(())
}

pub(super) fn parse_tool_call(
    raw: &str,
    default_name: Option<&str>,
    allowed_tools: &[String],
) -> anyhow::Result<ToolCall> {
    if let Some(call) =
        crate::tools::profile_tools::schema::invocation::receive_xml(raw, default_name)
    {
        return Ok(call);
    }
    let value = parse_json_relaxed(raw)?;
    let object = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("XML tool call payload must be an object"))?;
    let nested_arguments = object
        .get("arguments")
        .or_else(|| object.get("args"))
        .and_then(Value::as_object);
    let raw_name = default_name
        .or_else(|| {
            object
                .get("name")
                .or_else(|| object.get("tool"))
                .and_then(Value::as_str)
        })
        .or_else(|| {
            nested_arguments
                .and_then(|inner| inner.get("name").or_else(|| inner.get("tool")))
                .and_then(Value::as_str)
        })
        .map(str::to_string)
        .or_else(|| infer_tool_name_from_arguments(object, nested_arguments, allowed_tools))
        .ok_or_else(|| anyhow::anyhow!("tool call missing name"))?;
    let name = normalize_allowed_tool_name(&raw_name, allowed_tools)?;
    if let Some(call) = crate::tools::profile_tools::schema::invocation::refuse_unstrict_xml(&name)
    {
        return Ok(call);
    }
    let arguments = if default_name.is_some()
        && object.get("arguments").is_none()
        && object.get("args").is_none()
        && object.get("name").is_none()
        && object.get("tool").is_none()
    {
        Value::Object(object.clone())
    } else {
        object
            .get("arguments")
            .or_else(|| object.get("args"))
            .cloned()
            .unwrap_or_else(|| {
                let mut remaining = object.clone();
                remaining.remove("name");
                remaining.remove("tool");
                remaining.remove("arguments");
                remaining.remove("args");
                Value::Object(remaining)
            })
    };
    let arguments = recover_tool_arguments(&name, arguments).arguments;
    Ok(ToolCall::new(name, arguments))
}

fn normalize_allowed_tool_name(name: &str, allowed_tools: &[String]) -> anyhow::Result<String> {
    allowed_tools
        .iter()
        .find(|allowed| allowed.eq_ignore_ascii_case(name))
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("unknown tool in XML fallback: {name}"))
}

fn infer_tool_name_from_arguments(
    object: &serde_json::Map<String, Value>,
    nested_arguments: Option<&serde_json::Map<String, Value>>,
    allowed_tools: &[String],
) -> Option<String> {
    let args = nested_arguments.unwrap_or(object);
    let mut candidates = Vec::new();
    if has_any_key(args, &["command", "cmd"]) {
        maybe_push_allowed_tool(&mut candidates, "Bash", allowed_tools);
    }
    if has_any_key(args, &["path", "file", "file_path", "filepath", "filename"]) {
        if has_any_key(args, &["old_string", "old", "old_text", "oldText", "find"])
            && has_any_key(
                args,
                &[
                    "new_string",
                    "new",
                    "new_text",
                    "newText",
                    "replacement",
                    "replace_with",
                ],
            )
        {
            maybe_push_allowed_tool(&mut candidates, "Edit", allowed_tools);
        } else if has_any_key(args, &["content", "contents", "body", "text"]) {
            maybe_push_allowed_tool(&mut candidates, "Write", allowed_tools);
        } else {
            maybe_push_allowed_tool(&mut candidates, "Read", allowed_tools);
        }
    }
    if has_any_key(args, &["pattern", "query", "glob"]) {
        maybe_push_allowed_tool(&mut candidates, "Grep", allowed_tools);
        maybe_push_allowed_tool(&mut candidates, "Glob", allowed_tools);
    }
    candidates.dedup();
    (candidates.len() == 1).then(|| candidates.remove(0))
}

fn has_any_key(map: &serde_json::Map<String, Value>, keys: &[&str]) -> bool {
    keys.iter().any(|key| map.contains_key(*key))
}

fn maybe_push_allowed_tool(candidates: &mut Vec<String>, name: &str, allowed_tools: &[String]) {
    if let Some(allowed) = allowed_tools
        .iter()
        .find(|candidate| candidate.eq_ignore_ascii_case(name))
    {
        candidates.push(allowed.clone());
    }
}

fn parse_json_relaxed(raw: &str) -> anyhow::Result<Value> {
    let trimmed = raw.trim();
    for candidate in [trimmed.to_string(), strip_markdown_fence(trimmed)] {
        if let Ok(parsed) = serde_json::from_str(&candidate) {
            return Ok(parsed);
        }
        if let Some(parsed) = repair_json_candidate(&candidate) {
            return Ok(parsed);
        }
        let balanced = balance_braces(&candidate);
        if balanced != candidate {
            if let Ok(parsed) = serde_json::from_str(&balanced) {
                return Ok(parsed);
            }
            if let Some(parsed) = repair_json_candidate(&balanced) {
                return Ok(parsed);
            }
        }
    }
    serde_json::from_str(trimmed).map_err(Into::into)
}

fn repair_json_candidate(raw: &str) -> Option<Value> {
    let mut fixed = raw.trim().to_string();
    fixed = fixed.replace('\'', "\"");
    fixed = strip_trailing_commas(&fixed);
    fixed = quote_bare_keys(&fixed);
    serde_json::from_str(&fixed).ok()
}

fn strip_trailing_commas(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == ',' {
            let mut lookahead = chars.clone();
            while matches!(lookahead.peek(), Some(next) if next.is_whitespace()) {
                lookahead.next();
            }
            if matches!(lookahead.peek(), Some('}' | ']')) {
                continue;
            }
        }
        out.push(ch);
    }
    out
}

fn quote_bare_keys(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    let mut expecting_key = false;
    let mut in_string = false;
    let mut escaped = false;
    while let Some(ch) = chars.next() {
        if escaped {
            out.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_string => {
                out.push(ch);
                escaped = true;
            }
            '"' => {
                out.push(ch);
                in_string = !in_string;
                expecting_key = false;
            }
            '{' | ',' if !in_string => {
                out.push(ch);
                expecting_key = true;
            }
            ':' if !in_string => {
                out.push(ch);
                expecting_key = false;
            }
            ch if !in_string && expecting_key && (ch.is_ascii_alphabetic() || ch == '_') => {
                let mut key = String::new();
                key.push(ch);
                while let Some(next) = chars.peek() {
                    if next.is_ascii_alphanumeric() || matches!(next, '_' | '-') {
                        key.push(*next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                out.push('"');
                out.push_str(&key);
                out.push('"');
                expecting_key = false;
            }
            ch => out.push(ch),
        }
    }
    out
}

fn strip_markdown_fence(raw: &str) -> String {
    let trimmed = raw.trim();
    if !trimmed.starts_with("```") {
        return trimmed.to_string();
    }
    let mut lines = trimmed.lines();
    let _ = lines.next();
    let mut body = lines.collect::<Vec<_>>();
    if matches!(body.last(), Some(last) if last.trim_start().starts_with("```")) {
        body.pop();
    }
    body.join("\n")
}

fn balance_braces(raw: &str) -> String {
    let mut curly: i32 = 0;
    let mut square: i32 = 0;
    let mut in_string = false;
    let mut escaped = false;
    for ch in raw.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '{' if !in_string => curly += 1,
            '}' if !in_string => curly -= 1,
            '[' if !in_string => square += 1,
            ']' if !in_string => square -= 1,
            _ => {}
        }
    }
    let mut fixed = raw.to_string();
    while square > 0 {
        fixed.push(']');
        square -= 1;
    }
    while curly > 0 {
        fixed.push('}');
        curly -= 1;
    }
    fixed
}

fn json_looks_closed(raw: &str) -> bool {
    let trimmed = raw.trim();
    !trimmed.is_empty() && balance_braces(trimmed) == trimmed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn located(text: &str) -> Vec<(String, Range<usize>, bool)> {
        extract_located(text, &["Read".to_string()])
            .unwrap()
            .0
            .into_iter()
            .map(|at| (at.call.arguments.to_string(), at.span, at.spliced))
            .collect()
    }

    const READ_A: &str =
        "<anvil_tool_call>{\"name\":\"Read\",\"arguments\":{\"path\":\"a\"}}</anvil_tool_call>";

    #[test]
    fn a_call_read_from_consecutive_text_reports_where_it_was() {
        let text = format!("hi {READ_A} there");
        assert_eq!(
            located(&text),
            vec![(r#"{"path":"a"}"#.to_string(), 3..3 + READ_A.len(), false)]
        );
        // Its position in the original, also after earlier removals outside it.
        let text = format!("<think>x</think>{READ_A}{READ_A}<think>y</think>");
        let found = located(&text);
        assert_eq!(found[0].1, 16..16 + READ_A.len());
        assert_eq!(found[1].1, 16 + READ_A.len()..16 + 2 * READ_A.len());
        assert!(found.iter().all(|(_, _, spliced)| !spliced));
    }

    #[test]
    fn a_call_with_something_removed_inside_its_tag_reports_that() {
        // A call taken out of the body.
        let inner = format!(
            "<tool_call>{{\"name\":\"Read\",\"arguments\":{{\"path\":\"b{READ_A}c\"}}}}</tool_call>"
        );
        let found = located(&inner);
        assert_eq!(found.len(), 2);
        assert!(!found[0].2, "the inner call");
        assert_eq!(found[1].0, r#"{"path":"bc"}"#);
        assert!(found[1].2, "the outer call");
        assert_eq!(found[1].1, 0..inner.len());
        // A call taken out of the opening tag, which only then exists.
        let split = format!(
            "<to{READ_A}ol_call>{{\"name\":\"Read\",\"arguments\":{{\"path\":\"b\"}}}}</tool_call>"
        );
        let found = located(&split);
        assert!(found[1].2);
        assert_eq!(found[1].1, 0..split.len());
        // A think block removed inside the tag.
        let think = "<tool_call>{\"name\":\"Read\",\"arguments\":{\"path\":\"b<think>x</think>c\"}}</tool_call>";
        let found = located(think);
        assert_eq!(found[0].0, r#"{"path":"bc"}"#);
        assert!(found[0].2);
    }

    #[test]
    fn a_tag_in_a_body_read_first_changes_nothing_and_reports_nothing() {
        let text = "<anvil_tool_call>{\"name\":\"Read\",\"arguments\":{\"path\":\"<tool_call>x</tool_call>\"}}</anvil_tool_call>";
        let found = located(text);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, r#"{"path":"<tool_call>x</tool_call>"}"#);
        assert!(!found[0].2);
    }

    #[test]
    fn the_wrapper_keeps_what_the_located_parse_finds() {
        let text = format!(" <think>x</think>a{READ_A}b ");
        let (calls, content) = extract_tool_calls(&text, &["Read".to_string()]).unwrap();
        let (at, located_content) = extract_located(&text, &["Read".to_string()]).unwrap();
        assert_eq!(content, "ab");
        assert_eq!(content, located_content);
        assert_eq!(calls[0].arguments, at[0].call.arguments);
        assert_eq!(strip_think_tags("a<think>b</think>c<think>d"), "ac<think>d");
        assert_eq!(strip_think_tags("<thi<think>x</think>nk>y</think>z"), "z");
    }

    #[test]
    fn extracts_tool_call() {
        let (calls, text) = extract_tool_calls(
            r#"hi <anvil_tool_call>{"name":"Read","arguments":{"path":"a"}}</anvil_tool_call>"#,
            &["Read".to_string()],
        )
        .unwrap();
        assert_eq!(calls[0].name, "Read");
        assert_eq!(text, "hi");
    }

    #[test]
    fn extracts_source_style_function_call_tag() {
        let (calls, text) = extract_tool_calls(
            r#"hi <function_call>{"name":"Write","arguments":{"path":"provider-probe.txt","content":"ok"}}</function_call>"#,
            &["Write".to_string()],
        )
        .unwrap();
        assert_eq!(calls[0].name, "Write");
        assert_eq!(calls[0].arguments["path"], "provider-probe.txt");
        assert_eq!(text, "hi");
    }

    #[test]
    fn recovers_tool_like_argument_aliases() {
        let (calls, _) = extract_tool_calls(
            r#"<function_call>{"name":"Write","arguments":{"file":"provider-probe.txt","body":"ok"}}</function_call>"#,
            &["Write".to_string()],
        )
        .unwrap();
        assert_eq!(calls[0].arguments["path"], "provider-probe.txt");
        assert_eq!(calls[0].arguments["content"], "ok");
    }

    #[test]
    fn extracts_source_style_named_anvil_tool_call() {
        let (calls, text) = extract_tool_calls(
            r#"hi <anvil_tool_call name="Write">{"file":"provider-probe.txt","body":"ok"}</anvil_tool_call>"#,
            &["Write".to_string()],
        )
        .unwrap();
        assert_eq!(calls[0].name, "Write");
        assert_eq!(calls[0].arguments["path"], "provider-probe.txt");
        assert_eq!(calls[0].arguments["content"], "ok");
        assert_eq!(text, "hi");
    }

    #[test]
    fn extracts_source_style_function_equals_tag() {
        let (calls, _) = extract_tool_calls(
            r#"<function=Write>{"file":"provider-probe.txt","body":"ok"}</function>"#,
            &["Write".to_string()],
        )
        .unwrap();
        assert_eq!(calls[0].name, "Write");
        assert_eq!(calls[0].arguments["path"], "provider-probe.txt");
        assert_eq!(calls[0].arguments["content"], "ok");
    }

    #[test]
    fn infers_unambiguous_tool_name_from_arguments() {
        let (calls, _) = extract_tool_calls(
            r#"<function_call>{"arguments":{"file":"provider-probe.txt","body":"ok"}}</function_call>"#,
            &["Write".to_string(), "Read".to_string()],
        )
        .unwrap();
        assert_eq!(calls[0].name, "Write");
        assert_eq!(calls[0].arguments["path"], "provider-probe.txt");
        assert_eq!(calls[0].arguments["content"], "ok");
    }

    #[test]
    fn recovers_unterminated_closed_tool_call() {
        let (calls, text) = extract_tool_calls(
            r#"before <function_call>{"name":"Read","arguments":{"path":"README.md"}}"#,
            &["Read".to_string()],
        )
        .unwrap();
        assert_eq!(calls[0].name, "Read");
        assert_eq!(calls[0].arguments["path"], "README.md");
        assert_eq!(text, "before");
    }

    #[test]
    fn repairs_relaxed_json_payload() {
        let (calls, _) = extract_tool_calls(
            r#"<function_call>{name:'Write', arguments:{file:'provider-probe.txt', body:'ok',},}</function_call>"#,
            &["Write".to_string()],
        )
        .unwrap();
        assert_eq!(calls[0].name, "Write");
        assert_eq!(calls[0].arguments["path"], "provider-probe.txt");
        assert_eq!(calls[0].arguments["content"], "ok");
    }

    #[test]
    fn rejects_ambiguous_inferred_tool_name() {
        let err = extract_tool_calls(
            r#"<function_call>{"arguments":{"query":"TODO"}}</function_call>"#,
            &["Grep".to_string(), "Glob".to_string()],
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("tool call missing name"));
    }
}
