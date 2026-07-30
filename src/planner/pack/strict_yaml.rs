use std::collections::BTreeMap;
use std::fmt;

use serde::Deserialize;
use serde::de::{DeserializeOwned, Error as _, MapAccess, SeqAccess, Visitor};

use super::{MAX_FILE_BYTES, PackError};

const MAX_DEPTH: usize = 16;
const MAX_SEQUENCE: usize = 256;
const MAX_SCALAR_BYTES: usize = 64 * 1024;

pub(super) fn decode<T: DeserializeOwned>(
    file: &'static str,
    bytes: &[u8],
) -> Result<T, PackError> {
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(PackError::Invalid {
            file,
            reason: format!("file exceeds {MAX_FILE_BYTES} bytes"),
        });
    }
    let text = std::str::from_utf8(bytes).map_err(|error| PackError::Invalid {
        file,
        reason: format!("not UTF-8: {error}"),
    })?;
    reject_yaml_extensions(text).map_err(|reason| PackError::Invalid { file, reason })?;
    let node = serde_yaml::from_str::<StrictNode>(text).map_err(|error| PackError::Invalid {
        file,
        reason: error.to_string(),
    })?;
    node.validate(1)
        .map_err(|reason| PackError::Invalid { file, reason })?;
    serde_yaml::from_str(text).map_err(|error| PackError::Invalid {
        file,
        reason: error.to_string(),
    })
}

#[derive(Debug)]
enum StrictNode {
    Scalar(usize),
    Sequence(Vec<Self>),
    Mapping(BTreeMap<String, Self>),
}

impl StrictNode {
    fn validate(&self, depth: usize) -> Result<(), String> {
        if depth > MAX_DEPTH {
            return Err(format!("YAML nesting exceeds {MAX_DEPTH}"));
        }
        match self {
            Self::Scalar(bytes) if *bytes > MAX_SCALAR_BYTES => {
                Err(format!("YAML scalar exceeds {MAX_SCALAR_BYTES} bytes"))
            }
            Self::Scalar(_) => Ok(()),
            Self::Sequence(values) => {
                if values.len() > MAX_SEQUENCE {
                    return Err(format!("YAML sequence exceeds {MAX_SEQUENCE} entries"));
                }
                values
                    .iter()
                    .try_for_each(|value| value.validate(depth + 1))
            }
            Self::Mapping(values) => values.iter().try_for_each(|(key, value)| {
                if key.len() > MAX_SCALAR_BYTES {
                    return Err(format!("YAML key exceeds {MAX_SCALAR_BYTES} bytes"));
                }
                value.validate(depth + 1)
            }),
        }
    }
}

impl<'de> Deserialize<'de> for StrictNode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(StrictNodeVisitor)
    }
}

struct StrictNodeVisitor;

impl<'de> Visitor<'de> for StrictNodeVisitor {
    type Value = StrictNode;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a strict YAML scalar, sequence, or string-keyed mapping")
    }

    fn visit_bool<E>(self, _value: bool) -> Result<Self::Value, E> {
        Ok(StrictNode::Scalar(0))
    }

    fn visit_i64<E>(self, _value: i64) -> Result<Self::Value, E> {
        Ok(StrictNode::Scalar(0))
    }

    fn visit_u64<E>(self, _value: u64) -> Result<Self::Value, E> {
        Ok(StrictNode::Scalar(0))
    }

    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E> {
        Ok(StrictNode::Scalar(0))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(StrictNode::Scalar(value.len()))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(StrictNode::Scalar(value.len()))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(StrictNode::Scalar(0))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(StrictNode::Scalar(0))
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element()? {
            if values.len() == MAX_SEQUENCE {
                return Err(A::Error::custom(format!(
                    "YAML sequence exceeds {MAX_SEQUENCE} entries"
                )));
            }
            values.push(value);
        }
        Ok(StrictNode::Sequence(values))
    }

    fn visit_map<A>(self, mut mapping: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = BTreeMap::new();
        while let Some(key) = mapping.next_key::<String>()? {
            let value = mapping.next_value::<StrictNode>()?;
            if values.insert(key.clone(), value).is_some() {
                return Err(A::Error::custom(format!("duplicate YAML key `{key}`")));
            }
        }
        Ok(StrictNode::Mapping(values))
    }
}

fn reject_yaml_extensions(text: &str) -> Result<(), String> {
    let mut block_indent = None;
    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        let indent = line.bytes().take_while(|byte| *byte == b' ').count();
        if let Some(parent) = block_indent {
            if line.trim().is_empty() || indent > parent {
                continue;
            }
            block_indent = None;
        }
        let structural = structural_text(line);
        let trimmed = structural.trim();
        if trimmed.contains("<<:") {
            return Err(format!("YAML merge keys are forbidden at line {number}"));
        }
        for (offset, byte) in structural.bytes().enumerate() {
            if matches!(byte, b'&' | b'*' | b'!')
                && token_boundary(structural.as_bytes().get(offset.wrapping_sub(1)).copied())
            {
                let feature = match byte {
                    b'&' => "anchors",
                    b'*' => "aliases",
                    _ => "tags",
                };
                return Err(format!("YAML {feature} are forbidden at line {number}"));
            }
        }
        if block_scalar_header(trimmed) {
            block_indent = Some(indent);
        }
    }
    Ok(())
}

fn structural_text(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut single = false;
    let mut double = false;
    let mut escaped = false;
    let chars = line.chars().peekable();
    for character in chars {
        if double {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                double = false;
            }
            out.push(' ');
            continue;
        }
        if single {
            if character == '\'' {
                single = false;
            }
            out.push(' ');
            continue;
        }
        match character {
            '#' => break,
            '\'' => {
                single = true;
                out.push(' ');
            }
            '"' => {
                double = true;
                out.push(' ');
            }
            other => out.push(other),
        }
    }
    out
}

fn token_boundary(previous: Option<u8>) -> bool {
    previous.is_none_or(|byte| {
        byte.is_ascii_whitespace() || matches!(byte, b'[' | b'{' | b':' | b',' | b'-' | b'?')
    })
}

fn block_scalar_header(line: &str) -> bool {
    line.rsplit_once(':')
        .is_some_and(|(_, suffix)| matches!(suffix.trim(), "|" | "|-" | "|+" | ">" | ">-" | ">+"))
}
