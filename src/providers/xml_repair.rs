use serde_json::Value;

use crate::state::ToolCall;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolCallRepairKind {
    FirstJsonValue,
    ClosingTagCompleted,
}

impl ToolCallRepairKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::FirstJsonValue => "first_json_value",
            Self::ClosingTagCompleted => "closing_tag_completed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ToolCallRepair {
    pub(crate) kind: ToolCallRepairKind,
    pub(crate) operation: &'static str,
    pub(crate) change: String,
}

pub(crate) struct RepairCandidate {
    pub(crate) response: String,
    pub(crate) repair: ToolCallRepair,
}

#[derive(Debug)]
struct TagSpan<'a> {
    start: usize,
    body_start: usize,
    close: &'static str,
    default_name: Option<&'a str>,
}

/// Produce at most one bounded dialect repair candidate. Callers must run the
/// normal parser and the complete ToolSpec validation before accepting it.
/// These are protocol-shape rules; no provider or model name participates.
pub(crate) fn candidate(input: &str, allowed_tools: &[String]) -> Option<RepairCandidate> {
    complete_missing_closing_tag(input, allowed_tools)
        .or_else(|| retain_first_json_value(input, allowed_tools))
}

fn complete_missing_closing_tag(input: &str, allowed_tools: &[String]) -> Option<RepairCandidate> {
    for span in tag_spans(input) {
        if input[span.body_start..].contains(span.close) {
            continue;
        }
        let raw = input[span.body_start..].trim();
        if !matches!(serde_json::from_str::<Value>(raw), Ok(Value::Object(_))) {
            continue;
        }
        let Ok(call) = super::xml_fallback::parse_tool_call(raw, span.default_name, allowed_tools)
        else {
            continue;
        };
        if !arguments_are_an_object(&call) {
            continue;
        }
        let mut response = input.to_string();
        response.push_str(span.close);
        return Some(RepairCandidate {
            response,
            repair: ToolCallRepair {
                kind: ToolCallRepairKind::ClosingTagCompleted,
                operation: "appended",
                change: span.close.to_string(),
            },
        });
    }
    None
}

fn retain_first_json_value(input: &str, allowed_tools: &[String]) -> Option<RepairCandidate> {
    for span in tag_spans(input) {
        let Some(close_offset) = input[span.body_start..].find(span.close) else {
            continue;
        };
        let body_end = span.body_start + close_offset;
        let body = &input[span.body_start..body_end];
        let leading = body.len() - body.trim_start().len();
        let raw = body.trim();
        let mut values = serde_json::Deserializer::from_str(raw).into_iter::<Value>();
        let Some(Ok(value)) = values.next() else {
            continue;
        };
        let first_end = values.byte_offset();
        if !value.is_object() {
            continue;
        }
        let first = raw[..first_end].trim_end();
        let discarded = raw[first_end..].trim();
        if discarded.is_empty() {
            continue;
        }
        let Ok(call) =
            super::xml_fallback::parse_tool_call(first, span.default_name, allowed_tools)
        else {
            continue;
        };
        if !arguments_are_an_object(&call) {
            continue;
        }
        let raw_start = span.body_start + leading;
        let mut response = input.to_string();
        response.replace_range(raw_start..body_end, first);
        return Some(RepairCandidate {
            response,
            repair: ToolCallRepair {
                kind: ToolCallRepairKind::FirstJsonValue,
                operation: "discarded",
                change: discarded.to_string(),
            },
        });
    }
    None
}

fn arguments_are_an_object(call: &ToolCall) -> bool {
    call.arguments.is_object()
}

fn tag_spans(input: &str) -> Vec<TagSpan<'_>> {
    let mut spans = Vec::new();
    for (open, close) in [
        ("<anvil_tool_call>", "</anvil_tool_call>"),
        ("<tool_call>", "</tool_call>"),
        ("<function_call>", "</function_call>"),
    ] {
        if let Some(start) = input.find(open) {
            spans.push(TagSpan {
                start,
                body_start: start + open.len(),
                close,
                default_name: None,
            });
        }
    }
    for (prefix, close) in [
        ("<anvil_tool_call name=\"", "</anvil_tool_call>"),
        ("<function name=\"", "</function>"),
    ] {
        let Some(start) = input.find(prefix) else {
            continue;
        };
        let name_start = start + prefix.len();
        let Some(name_end_rel) = input[name_start..].find('\"') else {
            continue;
        };
        let name_end = name_start + name_end_rel;
        let after_name = name_end + 1;
        let Some(gt_rel) = input[after_name..].find('>') else {
            continue;
        };
        spans.push(TagSpan {
            start,
            body_start: after_name + gt_rel + 1,
            close,
            default_name: Some(&input[name_start..name_end]),
        });
    }
    if let Some(start) = input.find("<function=") {
        let name_start = start + "<function=".len();
        if let Some(gt_rel) = input[name_start..].find('>') {
            let name_end = name_start + gt_rel;
            spans.push(TagSpan {
                start,
                body_start: name_end + 1,
                close: "</function>",
                default_name: Some(input[name_start..name_end].trim().trim_matches('\"')),
            });
        }
    }
    spans.sort_by_key(|span| span.start);
    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    const WRITE: &[&str] = &["Write"];

    fn allowed() -> Vec<String> {
        WRITE.iter().map(|name| (*name).to_string()).collect()
    }

    #[test]
    fn luna_004_first_json_value_discards_the_measured_extra_brace() {
        let input = concat!(
            r#"<anvil_tool_call name="Write">{"path":"data/sample.csv","content":"id,name,amount\n1,Alice,120.50\n2,Bob,75.25\n3,Charlie,204.00\n"}"#,
            "}",
            "</anvil_tool_call>"
        );
        let repaired = candidate(input, &allowed()).unwrap();
        assert_eq!(repaired.repair.kind, ToolCallRepairKind::FirstJsonValue);
        assert_eq!(repaired.repair.change, "}");
        let (calls, _) =
            super::super::xml_fallback::extract_tool_calls(&repaired.response, &allowed()).unwrap();
        assert_eq!(calls[0].name, "Write");
        assert_eq!(calls[0].arguments["path"], "data/sample.csv");
    }

    #[test]
    fn first_json_value_is_not_used_when_the_tool_is_not_allowed() {
        let input = concat!(
            r#"<anvil_tool_call name="Invent">{"path":"a","content":"b"}"#,
            "}",
            "</anvil_tool_call>"
        );
        assert!(candidate(input, &allowed()).is_none());
    }

    #[test]
    fn closing_tag_completion_requires_complete_json() {
        let valid = r#"<anvil_tool_call name="Write">{"path":"README.md","content":"ok"}"#;
        let repaired = candidate(valid, &allowed()).unwrap();
        assert_eq!(
            repaired.repair.kind,
            ToolCallRepairKind::ClosingTagCompleted
        );
        assert_eq!(repaired.repair.change, "</anvil_tool_call>");

        let incomplete = r#"<anvil_tool_call name="Write">{"path":"README.md","content":"ok""#;
        assert!(candidate(incomplete, &allowed()).is_none());
    }

    #[derive(Deserialize)]
    struct MeasuredExcerpt {
        run: String,
        raw_response_bytes: usize,
        start_byte: usize,
        text: String,
    }

    #[test]
    fn closing_tag_completion_carries_all_three_luna_004_measured_excerpts() {
        // F-2a-5 deliberately retained only bounded failure-point excerpts.
        // This fixture preserves those bytes verbatim; the wrapper restores
        // only the opening structural bytes omitted by that evidence bound.
        let measured: Vec<MeasuredExcerpt> = serde_json::from_str(include_str!(
            "../../tests/corpus/apps/test0802_text_tool_repair/fixtures/luna004-malformed-excerpts.json"
        ))
        .unwrap();
        assert_eq!(measured.len(), 3);
        let report =
            include_str!("../../workspace/management/runs/uat-test0801-cli-luna-004/uat-report.md");
        for item in &measured {
            let encoded = serde_json::to_string(&item.text).unwrap();
            assert!(
                report.contains(&format!("\"text\":{encoded}")),
                "{} excerpt drifted from the measured report",
                item.run
            );
        }
        assert_eq!(
            measured
                .iter()
                .map(|item| (item.run.as_str(), item.raw_response_bytes, item.start_byte))
                .collect::<Vec<_>>(),
            vec![
                ("stats_luna_003", 622, 111),
                ("filter_luna_002", 1412, 900),
                ("filter_luna_003", 1367, 855),
            ]
        );
        for item in measured {
            let payload = serde_json::json!({
                "path": format!("{}.txt", item.run),
                "content": item.text,
            });
            let input = format!("<anvil_tool_call name=\"Write\">{payload}");
            let repaired = candidate(&input, &allowed()).unwrap();
            assert_eq!(
                repaired.repair.kind,
                ToolCallRepairKind::ClosingTagCompleted
            );
            let (calls, _) =
                super::super::xml_fallback::extract_tool_calls(&repaired.response, &allowed())
                    .unwrap();
            assert_eq!(calls[0].arguments["content"], payload["content"]);
        }
    }
}
