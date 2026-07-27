use std::collections::BTreeMap;
use std::sync::OnceLock;

use anyhow::{Context, bail};
use regex::Regex;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Compound {
    tag: Option<String>,
    id: Option<String>,
    classes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Selector {
    parent: Option<Compound>,
    target: Compound,
}

#[derive(Debug, Clone)]
struct Frame {
    tag: String,
    attributes: BTreeMap<String, String>,
    byte_start: usize,
}

pub(super) fn validate(value: &str) -> anyhow::Result<()> {
    Selector::parse(value).map(|_| ())
}

pub(super) fn enumerate(text: &str, value: &str) -> anyhow::Result<Vec<(usize, usize, String)>> {
    let selector = Selector::parse(value)?;
    let mut stack = Vec::<Frame>::new();
    let mut found = Vec::new();

    for token in tag_regex().captures_iter(text) {
        let whole = token
            .get(0)
            .context("candidate_set_violation:css_selector_token")?;
        let closing = token.get(1).is_some_and(|value| value.as_str() == "/");
        let tag = token
            .get(2)
            .context("candidate_set_violation:css_selector_tag")?
            .as_str()
            .to_ascii_lowercase();
        if closing {
            let Some(position) = stack.iter().rposition(|frame| frame.tag == tag) else {
                continue;
            };
            let frame = stack[position].clone();
            let parent = position.checked_sub(1).and_then(|index| stack.get(index));
            if selector.target.matches(&frame)
                && selector
                    .parent
                    .as_ref()
                    .is_none_or(|expected| parent.is_some_and(|actual| expected.matches(actual)))
            {
                found.push((
                    frame.byte_start,
                    whole.end(),
                    text[frame.byte_start..whole.end()].to_string(),
                ));
            }
            stack.truncate(position);
            continue;
        }

        let raw_attributes = token.get(3).map_or("", |value| value.as_str());
        if raw_attributes.trim_end().ends_with('/') || is_void_tag(&tag) {
            continue;
        }
        stack.push(Frame {
            tag,
            attributes: parse_attributes(raw_attributes),
            byte_start: whole.start(),
        });
    }

    found.sort_by_key(|(start, _, _)| *start);
    Ok(found)
}

impl Selector {
    fn parse(value: &str) -> anyhow::Result<Self> {
        let parts = value.split('>').map(str::trim).collect::<Vec<_>>();
        match parts.as_slice() {
            [target] => Ok(Self {
                parent: None,
                target: Compound::parse(target)?,
            }),
            [parent, target] => Ok(Self {
                parent: Some(Compound::parse(parent)?),
                target: Compound::parse(target)?,
            }),
            _ => bail!("candidate_set_violation:css_selector_unsupported"),
        }
    }
}

impl Compound {
    fn parse(value: &str) -> anyhow::Result<Self> {
        let value = value.trim();
        if value.is_empty() || !value.is_ascii() || value.chars().any(char::is_whitespace) {
            bail!("candidate_set_violation:css_selector_compound");
        }
        let bytes = value.as_bytes();
        let mut cursor = 0usize;
        let tag = if bytes[0] == b'*' {
            cursor = 1;
            None
        } else if matches!(bytes[0], b'.' | b'#') {
            None
        } else {
            let end = component_end(bytes, cursor);
            let value = component(&value[cursor..end])?;
            if !value.as_bytes()[0].is_ascii_alphabetic() {
                bail!("candidate_set_violation:css_selector_tag");
            }
            cursor = end;
            Some(value.to_ascii_lowercase())
        };
        let mut id = None;
        let mut classes = Vec::new();
        while cursor < bytes.len() {
            let marker = bytes[cursor];
            if !matches!(marker, b'.' | b'#') {
                bail!("candidate_set_violation:css_selector_compound");
            }
            cursor += 1;
            let end = component_end(bytes, cursor);
            let value = component(&value[cursor..end])?.to_string();
            cursor = end;
            if marker == b'#' {
                if id.replace(value).is_some() {
                    bail!("candidate_set_violation:css_selector_duplicate_id");
                }
            } else {
                classes.push(value);
            }
        }
        if tag.is_none() && id.is_none() && classes.is_empty() {
            bail!("candidate_set_violation:css_selector_compound");
        }
        Ok(Self { tag, id, classes })
    }

    fn matches(&self, frame: &Frame) -> bool {
        if self.tag.as_ref().is_some_and(|tag| tag != &frame.tag) {
            return false;
        }
        if self
            .id
            .as_ref()
            .is_some_and(|id| frame.attributes.get("id").is_none_or(|actual| actual != id))
        {
            return false;
        }
        let actual_classes = frame
            .attributes
            .get("class")
            .map(|value| value.split_ascii_whitespace().collect::<Vec<_>>())
            .unwrap_or_default();
        self.classes
            .iter()
            .all(|class| actual_classes.contains(&class.as_str()))
    }
}

fn component_end(bytes: &[u8], start: usize) -> usize {
    bytes[start..]
        .iter()
        .position(|byte| matches!(byte, b'.' | b'#'))
        .map_or(bytes.len(), |offset| start + offset)
}

fn component(value: &str) -> anyhow::Result<&str> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        bail!("candidate_set_violation:css_selector_component");
    }
    Ok(value)
}

fn parse_attributes(raw: &str) -> BTreeMap<String, String> {
    attribute_regex()
        .captures_iter(raw)
        .filter_map(|capture| {
            let name = capture.get(1)?.as_str().to_ascii_lowercase();
            let value = (2..=4)
                .find_map(|index| capture.get(index))
                .map(|value| value.as_str().to_string())?;
            Some((name, value))
        })
        .collect()
}

fn tag_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?is)<\s*(/?)\s*([a-z][a-z0-9-]*)\b([^>]*)>").expect("static HTML tag regex")
    })
}

fn attribute_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r#"(?is)(?:^|\s)([a-z_:][a-z0-9_:.-]*)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'=<>`]+))"#,
        )
        .expect("static HTML attribute regex")
    })
}

fn is_void_tag(tag: &str) -> bool {
    matches!(
        tag,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_direct_child_example_selects_only_matching_children() {
        let html = r#"
<ul class="events"><li>A</li><li>B</li><div><li>nested</li></div></ul>
<ul><li>other</li></ul>
"#;

        let found = enumerate(html, "ul.events > li").unwrap();

        assert_eq!(found.len(), 2);
        assert_eq!(found[0].2, "<li>A</li>");
        assert_eq!(found[1].2, "<li>B</li>");
    }

    #[test]
    fn compound_tag_and_class_selects_whole_blocks() {
        let html = r#"<article class="event featured">A</article><article>B</article>"#;

        let found = enumerate(html, "article.event").unwrap();

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].2, r#"<article class="event featured">A</article>"#);
    }

    #[test]
    fn unsupported_descendant_or_multi_child_selector_is_rejected() {
        assert!(validate("ul.events li").is_err());
        assert!(validate("main > ul > li").is_err());
    }
}
