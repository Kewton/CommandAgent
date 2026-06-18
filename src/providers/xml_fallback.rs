use crate::providers::{ToolCall, ToolCallMode};
use serde_json::Value;

pub const COMMANDAGENT_TOOL_CALL_TAG: &str = "commandagent_tool_call";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlToolCallExtraction {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XmlFallbackError {
    UnclosedToolCall { tag: String },
    InvalidJson { message: String },
    MissingToolName,
    InvalidArguments,
}

impl std::fmt::Display for XmlFallbackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnclosedToolCall { tag } => write!(f, "unclosed <{}> block", tag),
            Self::InvalidJson { message } => write!(f, "invalid tool call JSON: {}", message),
            Self::MissingToolName => write!(f, "tool call is missing a tool name"),
            Self::InvalidArguments => write!(f, "tool call arguments must be a JSON object"),
        }
    }
}

impl std::error::Error for XmlFallbackError {}

pub fn strip_think_tags(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut remaining = input;

    while let Some(start) = remaining.find("<think>") {
        output.push_str(&remaining[..start]);
        let after_start = &remaining[start + "<think>".len()..];
        let Some(end) = after_start.find("</think>") else {
            break;
        };
        remaining = &after_start[end + "</think>".len()..];
    }

    output.push_str(remaining);
    output
}

pub fn extract_tool_calls(input: &str) -> Result<Vec<ToolCall>, XmlFallbackError> {
    Ok(extract_tool_calls_with_content(input)?.tool_calls)
}

pub fn render_tool_calls(tool_calls: &[ToolCall]) -> String {
    tool_calls
        .iter()
        .map(render_tool_call)
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn extract_tool_calls_with_content(
    input: &str,
) -> Result<XmlToolCallExtraction, XmlFallbackError> {
    let stripped = strip_think_tags(input);
    let mut content = String::with_capacity(stripped.len());
    let mut calls = Vec::new();
    let mut remaining = stripped.as_str();

    while let Some((start, tag)) = next_open_tag(remaining) {
        let open = format!("<{}>", tag);
        let close = format!("</{}>", tag);
        content.push_str(&remaining[..start]);

        let after_open = &remaining[start + open.len()..];
        let Some(end) = after_open.find(&close) else {
            return Err(XmlFallbackError::UnclosedToolCall {
                tag: tag.to_string(),
            });
        };
        let payload = after_open[..end].trim();
        calls.push(parse_tool_call_payload(payload)?);
        remaining = &after_open[end + close.len()..];
    }

    content.push_str(remaining);

    Ok(XmlToolCallExtraction {
        content: content.trim().to_string(),
        tool_calls: calls,
    })
}

pub fn mode_after_parse_failure(current: ToolCallMode) -> ToolCallMode {
    match current {
        ToolCallMode::Native => ToolCallMode::XmlFallback,
        ToolCallMode::XmlFallback => ToolCallMode::XmlFallback,
    }
}

fn parse_tool_call_payload(payload: &str) -> Result<ToolCall, XmlFallbackError> {
    let value: Value =
        serde_json::from_str(payload).map_err(|err| XmlFallbackError::InvalidJson {
            message: err.to_string(),
        })?;
    let object = value.as_object().ok_or(XmlFallbackError::InvalidJson {
        message: "payload must be a JSON object".to_string(),
    })?;

    let name = first_string(object, &["name", "tool", "tool_name"])
        .ok_or(XmlFallbackError::MissingToolName)?;
    let arguments = object
        .get("args")
        .or_else(|| object.get("arguments"))
        .cloned()
        .unwrap_or_else(|| Value::Object(Default::default()));
    if !arguments.is_object() {
        return Err(XmlFallbackError::InvalidArguments);
    }

    Ok(ToolCall {
        name,
        args_json: serde_json::to_string(&arguments).unwrap_or_else(|_| "{}".to_string()),
    })
}

fn render_tool_call(tool_call: &ToolCall) -> String {
    let name = serde_json::to_string(&tool_call.name).unwrap_or_else(|_| "\"\"".to_string());
    format!(
        "<{tag}>{{\"name\":{name},\"args\":{args}}}</{tag}>",
        tag = COMMANDAGENT_TOOL_CALL_TAG,
        name = name,
        args = tool_call.args_json
    )
}

fn first_string(object: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .filter_map(|key| object.get(*key))
        .find_map(|value| value.as_str().map(ToString::to_string))
}

fn supported_tags() -> [&'static str; 2] {
    [COMMANDAGENT_TOOL_CALL_TAG, legacy_tool_call_tag()]
}

fn legacy_tool_call_tag() -> &'static str {
    concat!("an", "vil_tool_call")
}

fn next_open_tag(input: &str) -> Option<(usize, &'static str)> {
    supported_tags()
        .into_iter()
        .filter_map(|tag| input.find(&format!("<{}>", tag)).map(|idx| (idx, tag)))
        .min_by_key(|(idx, _)| *idx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_think_tags() {
        assert_eq!(strip_think_tags("a<think>hidden</think>b"), "ab");
    }

    #[test]
    fn extracts_commandagent_tool_call() {
        let calls = extract_tool_calls(
            r#"<commandagent_tool_call>{"name":"Read","args":{"path":"Cargo.toml"}}</commandagent_tool_call>"#,
        )
        .unwrap();

        assert_eq!(
            calls,
            vec![ToolCall {
                name: "Read".to_string(),
                args_json: r#"{"path":"Cargo.toml"}"#.to_string(),
            }]
        );
    }

    #[test]
    fn extracts_calls_and_removes_xml_blocks_from_content() {
        let extraction = extract_tool_calls_with_content(
            r#"Before
<commandagent_tool_call>{"name":"Read","args":{"path":"Cargo.toml"}}</commandagent_tool_call>
After"#,
        )
        .unwrap();

        assert_eq!(extraction.content, "Before\n\nAfter");
        assert_eq!(extraction.tool_calls.len(), 1);
        assert_eq!(extraction.tool_calls[0].name, "Read");
    }

    #[test]
    fn preserves_tool_call_order_across_supported_tags() {
        let legacy = legacy_tool_call_tag();
        let input = format!(
            r#"<commandagent_tool_call>{{"name":"Read","args":{{"path":"a"}}}}</commandagent_tool_call><{}>{{"name":"Write","arguments":{{"path":"b","content":"c"}}}}</{}>"#,
            legacy, legacy
        );
        let calls = extract_tool_calls(&input).unwrap();

        assert_eq!(calls[0].name, "Read");
        assert_eq!(calls[1].name, "Write");
    }

    #[test]
    fn renders_tool_calls_as_canonical_commandagent_xml() {
        let rendered = render_tool_calls(&[ToolCall {
            name: "Write".to_string(),
            args_json: r#"{"path":"hello.txt","content":"ok"}"#.to_string(),
        }]);

        assert_eq!(
            rendered,
            r#"<commandagent_tool_call>{"name":"Write","args":{"path":"hello.txt","content":"ok"}}</commandagent_tool_call>"#
        );
    }

    #[test]
    fn accepts_legacy_tool_call_tag_for_migration() {
        let input = format!(
            r#"<{}>{{"tool_name":"Write","arguments":{{"path":"a.txt","content":"x"}}}}</{}>"#,
            legacy_tool_call_tag(),
            legacy_tool_call_tag()
        );
        let calls = extract_tool_calls(&input).unwrap();

        assert_eq!(calls[0].name, "Write");
    }

    #[test]
    fn detects_unclosed_tool_call() {
        let err = extract_tool_calls(r#"<commandagent_tool_call>{"name":"Read"}"#).unwrap_err();

        assert!(matches!(err, XmlFallbackError::UnclosedToolCall { .. }));
    }

    #[test]
    fn rejects_invalid_json() {
        let err = extract_tool_calls(
            r#"<commandagent_tool_call>{"name":"Read",</commandagent_tool_call>"#,
        )
        .unwrap_err();

        assert!(matches!(err, XmlFallbackError::InvalidJson { .. }));
    }

    #[test]
    fn native_parse_failure_downgrades_to_xml_fallback() {
        assert_eq!(
            mode_after_parse_failure(ToolCallMode::Native),
            ToolCallMode::XmlFallback
        );
        assert_eq!(
            mode_after_parse_failure(ToolCallMode::XmlFallback),
            ToolCallMode::XmlFallback
        );
    }
}
