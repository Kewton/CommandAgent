//! Admission of the bounded schema subset. The vocabulary is closed: a keyword
//! outside it, a keyword on a type it does not apply to, and any combination this
//! file does not name are refused, never ignored.

use std::collections::{BTreeMap, BTreeSet};

use regex::RegexBuilder;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use super::super::{ProfileToolRefusal, RefusalKind};
use super::raw_json::{self, DecodeLimits};
use super::{
    Limits, MAX_ANY_OF_BRANCHES, MAX_ENUM_VALUE_BYTES, MAX_PATTERN_BYTES, MAX_REGEX_BYTES,
    MAX_SCHEMA_JSON_DEPTH, MAX_UNION_PATHS,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScalarKind {
    String,
    Integer {
        minimum: Option<i128>,
        maximum: Option<i128>,
    },
    Number {
        minimum: Option<OrderedBound>,
        maximum: Option<OrderedBound>,
    },
    Boolean,
    Null,
}

/// A finite bound, kept as bits so the schema stays comparable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OrderedBound(u64);

impl OrderedBound {
    fn new(value: f64) -> Self {
        Self(value.to_bits())
    }

    fn get(self) -> f64 {
        f64::from_bits(self.0)
    }
}

fn integer_value(value: &Value) -> Option<i128> {
    value
        .as_i64()
        .map(i128::from)
        .or_else(|| value.as_u64().map(i128::from))
}

impl ScalarKind {
    pub(super) fn accepts(self, value: &Value) -> bool {
        match self {
            Self::String => value.is_string(),
            Self::Integer { .. } => integer_value(value).is_some(),
            Self::Number { .. } => value.is_number(),
            Self::Boolean => value.is_boolean(),
            Self::Null => value.is_null(),
        }
    }

    pub(super) fn bound_violation(self, value: &Value) -> Option<&'static str> {
        match self {
            Self::Integer { minimum, maximum } => {
                let number = integer_value(value)?;
                bound_code(
                    minimum.is_some_and(|minimum| number < minimum),
                    maximum.is_some_and(|maximum| number > maximum),
                )
            }
            Self::Number { minimum, maximum } => {
                let number = value.as_f64()?;
                bound_code(
                    minimum.is_some_and(|minimum| number < minimum.get()),
                    maximum.is_some_and(|maximum| number > maximum.get()),
                )
            }
            Self::String | Self::Boolean | Self::Null => None,
        }
    }
}

fn bound_code(below: bool, above: bool) -> Option<&'static str> {
    if below {
        Some("below_minimum")
    } else if above {
        Some("above_maximum")
    } else {
        None
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Node {
    Object {
        properties: BTreeMap<String, Node>,
        required: Vec<String>,
    },
    Array {
        items: Box<Node>,
        min_items: Option<usize>,
        max_items: Option<usize>,
    },
    AnyOf(Vec<Node>),
    Scalar {
        kind: ScalarKind,
        enum_values: Option<Vec<Value>>,
        min_length: Option<usize>,
        max_length: Option<usize>,
        pattern: Option<regex::Regex>,
    },
}

/// A schema that passed admission, with the exact value sent and its digest.
#[derive(Debug, Clone)]
pub(crate) struct CompiledSchema {
    root: Node,
    source: Value,
    sha256: String,
    limits: Limits,
}

impl CompiledSchema {
    pub(super) fn root(&self) -> &Node {
        &self.root
    }

    pub(crate) fn source(&self) -> &Value {
        &self.source
    }

    pub(crate) fn sha256(&self) -> &str {
        &self.sha256
    }

    pub(crate) fn limits(&self) -> Limits {
        self.limits
    }
}

fn unsupported(reason: String) -> ProfileToolRefusal {
    ProfileToolRefusal::new(RefusalKind::SchemaUnsupported, reason)
}

fn over_limit(reason: String) -> ProfileToolRefusal {
    ProfileToolRefusal::new(RefusalKind::SchemaLimitExceeded, reason)
}

/// Compile one root schema. The root must be a closed object.
pub(crate) fn compile(root: &Value, limits: Limits) -> Result<CompiledSchema, ProfileToolRefusal> {
    let mut walker = Walker {
        limits,
        elements: 0,
    };
    let node = walker.node(root, 0, 0, "")?;
    if !matches!(node, Node::Object { .. }) {
        return Err(unsupported("the root must be `type: object`".to_string()));
    }
    let paths = union_paths(&node);
    if paths > MAX_UNION_PATHS {
        return Err(over_limit(format!(
            "nested anyOf reach {paths} paths; at most {MAX_UNION_PATHS} are allowed"
        )));
    }
    let bytes = serde_json::to_vec(root).map_err(|error| unsupported(error.to_string()))?;
    if bytes.len() > limits.schema_bytes {
        return Err(over_limit(format!(
            "the schema is {} bytes; at most {} are allowed",
            bytes.len(),
            limits.schema_bytes
        )));
    }
    Ok(CompiledSchema {
        root: node,
        source: root.clone(),
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        limits,
    })
}

/// How many times one argument value can be tried: the product of the anyOf
/// branches along a path, saturating.
fn union_paths(node: &Node) -> usize {
    match node {
        Node::Object { properties, .. } => properties.values().map(union_paths).max().unwrap_or(1),
        Node::Array { items, .. } => union_paths(items),
        Node::AnyOf(branches) => branches
            .len()
            .saturating_mul(branches.iter().map(union_paths).max().unwrap_or(1)),
        Node::Scalar { .. } => 1,
    }
}

struct Walker {
    limits: Limits,
    elements: usize,
}

impl Walker {
    fn count(&mut self, added: usize) -> Result<(), ProfileToolRefusal> {
        self.elements = self.elements.saturating_add(added);
        if self.elements > self.limits.elements {
            return Err(over_limit(format!(
                "the schema has more than {} elements",
                self.limits.elements
            )));
        }
        Ok(())
    }

    /// `depth` is how many containers the argument value is inside here. A scalar
    /// or an anyOf adds none, so it measures the same thing a document's depth
    /// does. `json_depth` bounds the recursion over the schema's own nesting.
    fn node(
        &mut self,
        value: &Value,
        depth: usize,
        json_depth: usize,
        path: &str,
    ) -> Result<Node, ProfileToolRefusal> {
        if json_depth > MAX_SCHEMA_JSON_DEPTH {
            return Err(over_limit(format!(
                "the schema nests deeper than {MAX_SCHEMA_JSON_DEPTH} levels"
            )));
        }
        self.count(1)?;
        let shown = shown_path(path);
        let map = value
            .as_object()
            .ok_or_else(|| unsupported(format!("schema `{shown}` must be an object")))?;
        if map.contains_key("anyOf") {
            return self.any_of(map, depth, json_depth, path);
        }
        let kind = map
            .get("type")
            .ok_or_else(|| unsupported(format!("schema `{shown}` must declare `type`")))?
            .as_str()
            .ok_or_else(|| {
                unsupported(format!("schema `{shown}`: `type` must be one type name"))
            })?;
        let allowed: &[&str] = match kind {
            "object" => &["type", "properties", "required", "additionalProperties"],
            "array" => &["type", "items", "minItems", "maxItems"],
            "string" => &["type", "enum", "minLength", "maxLength", "pattern"],
            "integer" | "number" => &["type", "enum", "minimum", "maximum"],
            "boolean" => &["type", "enum"],
            "null" => &["type"],
            other => {
                return Err(unsupported(format!(
                    "schema `{shown}` has an unsupported type `{}`",
                    bounded(other)
                )));
            }
        };
        if let Some(keyword) = map.keys().find(|key| !allowed.contains(&key.as_str())) {
            return Err(unsupported(format!(
                "schema `{shown}`: keyword `{}` is not supported on type `{kind}`",
                bounded(keyword)
            )));
        }
        match kind {
            "object" => self.object(map, depth, json_depth, path),
            "array" => self.array(map, depth, json_depth, path),
            _ => self.scalar(map, kind, path),
        }
    }

    fn any_of(
        &mut self,
        map: &Map<String, Value>,
        depth: usize,
        json_depth: usize,
        path: &str,
    ) -> Result<Node, ProfileToolRefusal> {
        let shown = shown_path(path);
        if map.len() != 1 {
            return Err(unsupported(format!(
                "schema `{shown}`: `anyOf` cannot be combined with another keyword"
            )));
        }
        let branches = map["anyOf"]
            .as_array()
            .filter(|branches| !branches.is_empty());
        let branches = branches.ok_or_else(|| {
            unsupported(format!(
                "schema `{shown}`: `anyOf` must be a non-empty array"
            ))
        })?;
        if branches.len() > MAX_ANY_OF_BRANCHES {
            return Err(over_limit(format!(
                "schema `{shown}`: `anyOf` has {} branches; at most {MAX_ANY_OF_BRANCHES} are allowed",
                branches.len()
            )));
        }
        branches
            .iter()
            .map(|branch| self.node(branch, depth, json_depth + 2, path))
            .collect::<Result<Vec<_>, _>>()
            .map(Node::AnyOf)
    }

    fn enter(&self, depth: usize, path: &str) -> Result<usize, ProfileToolRefusal> {
        if depth + 1 > self.limits.depth {
            return Err(over_limit(format!(
                "schema `{}` describes arguments deeper than {}",
                shown_path(path),
                self.limits.depth
            )));
        }
        Ok(depth + 1)
    }

    fn object(
        &mut self,
        map: &Map<String, Value>,
        depth: usize,
        json_depth: usize,
        path: &str,
    ) -> Result<Node, ProfileToolRefusal> {
        let shown = shown_path(path);
        let depth = self.enter(depth, path)?;
        if map.get("additionalProperties") != Some(&Value::Bool(false)) {
            return Err(unsupported(format!(
                "schema `{shown}`: an object must declare `additionalProperties: false`"
            )));
        }
        let declared = map
            .get("properties")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                unsupported(format!(
                    "schema `{shown}`: an object must declare `properties`"
                ))
            })?;
        let mut properties = BTreeMap::new();
        for (name, property) in declared {
            let child = format!("{path}/{name}");
            properties.insert(
                name.clone(),
                self.node(property, depth, json_depth + 2, &child)?,
            );
        }
        let required = match map.get("required") {
            None => Vec::new(),
            Some(value) => required_names(value, &properties, &shown)?,
        };
        self.count(required.len())?;
        Ok(Node::Object {
            properties,
            required,
        })
    }

    fn array(
        &mut self,
        map: &Map<String, Value>,
        depth: usize,
        json_depth: usize,
        path: &str,
    ) -> Result<Node, ProfileToolRefusal> {
        let shown = shown_path(path);
        let depth = self.enter(depth, path)?;
        let items = map.get("items").ok_or_else(|| {
            unsupported(format!("schema `{shown}`: an array must declare `items`"))
        })?;
        let (min_items, max_items) = size_bounds(map, "minItems", "maxItems", &shown)?;
        let child = format!("{path}/items");
        Ok(Node::Array {
            items: Box::new(self.node(items, depth, json_depth + 1, &child)?),
            min_items,
            max_items,
        })
    }

    fn scalar(
        &mut self,
        map: &Map<String, Value>,
        kind: &str,
        path: &str,
    ) -> Result<Node, ProfileToolRefusal> {
        let shown = shown_path(path);
        let scalar = match kind {
            "string" => ScalarKind::String,
            "integer" => integer_kind(map, &shown)?,
            "number" => number_kind(map, &shown)?,
            "boolean" => ScalarKind::Boolean,
            _ => ScalarKind::Null,
        };
        let (min_length, max_length) = size_bounds(map, "minLength", "maxLength", &shown)?;
        let pattern = map
            .get("pattern")
            .map(|pattern| compile_pattern(pattern, &shown))
            .transpose()?;
        let enum_values = map
            .get("enum")
            .map(|values| enum_values(values, scalar, map, &shown))
            .transpose()?;
        self.count(enum_values.as_ref().map_or(0, Vec::len))?;
        Ok(Node::Scalar {
            kind: scalar,
            enum_values,
            min_length,
            max_length,
            pattern,
        })
    }
}

fn required_names(
    value: &Value,
    properties: &BTreeMap<String, Node>,
    shown: &str,
) -> Result<Vec<String>, ProfileToolRefusal> {
    let invalid = |reason: &str| unsupported(format!("schema `{shown}`: `required` {reason}"));
    let names = value
        .as_array()
        .ok_or_else(|| invalid("must be an array of names"))?;
    let mut seen = BTreeSet::new();
    let mut required = Vec::with_capacity(names.len());
    for name in names {
        let name = name
            .as_str()
            .ok_or_else(|| invalid("must be an array of names"))?;
        if !properties.contains_key(name) {
            return Err(invalid("names a key `properties` does not declare"));
        }
        if !seen.insert(name) {
            return Err(invalid("repeats a name"));
        }
        required.push(name.to_string());
    }
    Ok(required)
}

fn size_bounds(
    map: &Map<String, Value>,
    min_key: &str,
    max_key: &str,
    shown: &str,
) -> Result<(Option<usize>, Option<usize>), ProfileToolRefusal> {
    let read = |key: &str| {
        map.get(key)
            .map(|value| {
                value
                    .as_u64()
                    .and_then(|value| usize::try_from(value).ok())
                    .ok_or_else(|| {
                        unsupported(format!(
                            "schema `{shown}`: `{key}` must be a non-negative integer"
                        ))
                    })
            })
            .transpose()
    };
    let (minimum, maximum) = (read(min_key)?, read(max_key)?);
    if let (Some(minimum), Some(maximum)) = (minimum, maximum)
        && minimum > maximum
    {
        return Err(unsupported(format!(
            "schema `{shown}`: `{min_key}` exceeds `{max_key}`"
        )));
    }
    Ok((minimum, maximum))
}

fn integer_kind(map: &Map<String, Value>, shown: &str) -> Result<ScalarKind, ProfileToolRefusal> {
    let read = |key: &str| {
        map.get(key)
            .map(|value| {
                integer_value(value).ok_or_else(|| {
                    unsupported(format!("schema `{shown}`: `{key}` must be an integer"))
                })
            })
            .transpose()
    };
    let (minimum, maximum) = (read("minimum")?, read("maximum")?);
    if let (Some(minimum), Some(maximum)) = (minimum, maximum)
        && minimum > maximum
    {
        return Err(unsupported(format!(
            "schema `{shown}`: `minimum` exceeds `maximum`"
        )));
    }
    Ok(ScalarKind::Integer { minimum, maximum })
}

fn number_kind(map: &Map<String, Value>, shown: &str) -> Result<ScalarKind, ProfileToolRefusal> {
    let read = |key: &str| {
        map.get(key)
            .map(|value| {
                value.as_f64().ok_or_else(|| {
                    unsupported(format!("schema `{shown}`: `{key}` must be a number"))
                })
            })
            .transpose()
    };
    let (minimum, maximum) = (read("minimum")?, read("maximum")?);
    if let (Some(minimum), Some(maximum)) = (minimum, maximum)
        && minimum > maximum
    {
        return Err(unsupported(format!(
            "schema `{shown}`: `minimum` exceeds `maximum`"
        )));
    }
    Ok(ScalarKind::Number {
        minimum: minimum.map(OrderedBound::new),
        maximum: maximum.map(OrderedBound::new),
    })
}

/// A pattern keeps one meaning on the host and on a provider's engine only
/// without a class or flag whose reading differs between them. The `regex` crate
/// also has no backtracking, so its time is linear in the text.
fn compile_pattern(pattern: &Value, shown: &str) -> Result<regex::Regex, ProfileToolRefusal> {
    let source = pattern
        .as_str()
        .ok_or_else(|| unsupported(format!("schema `{shown}`: `pattern` must be a string")))?;
    if source.len() > MAX_PATTERN_BYTES {
        return Err(over_limit(format!(
            "schema `{shown}`: `pattern` exceeds {MAX_PATTERN_BYTES} bytes"
        )));
    }
    if !pattern_has_one_meaning(source) {
        return Err(unsupported(format!(
            "schema `{shown}`: `pattern` uses a class, flag, or group outside the supported subset"
        )));
    }
    RegexBuilder::new(source)
        .size_limit(MAX_REGEX_BYTES)
        .dfa_size_limit(MAX_REGEX_BYTES)
        .build()
        .map_err(|error| match error {
            regex::Error::CompiledTooBig(_) => over_limit(format!(
                "schema `{shown}`: `pattern` exceeds the compiled size limit"
            )),
            _ => unsupported(format!(
                "schema `{shown}`: `pattern` does not compile in the supported subset"
            )),
        })
}

fn pattern_has_one_meaning(source: &str) -> bool {
    let mut characters = source.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '\\' => match characters.next() {
                Some(escaped) if !escaped.is_ascii_alphanumeric() => {}
                Some('n' | 't' | 'r') => {}
                _ => return false,
            },
            '(' if characters.peek() == Some(&'?') => {
                characters.next();
                if characters.next() != Some(':') {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

fn enum_values(
    values: &Value,
    kind: ScalarKind,
    map: &Map<String, Value>,
    shown: &str,
) -> Result<Vec<Value>, ProfileToolRefusal> {
    let invalid = |reason: &str| unsupported(format!("schema `{shown}`: `enum` {reason}"));
    if map.len() != 2 {
        return Err(invalid(
            "cannot be combined with a length, bound, or pattern",
        ));
    }
    let values = values
        .as_array()
        .filter(|values| !values.is_empty())
        .ok_or_else(|| invalid("must be a non-empty array"))?;
    for (index, value) in values.iter().enumerate() {
        if !kind.accepts(value) {
            return Err(invalid("holds a value of another type"));
        }
        if values[..index].contains(value) {
            return Err(invalid("repeats a value"));
        }
        if value
            .as_str()
            .is_some_and(|text| text.len() > MAX_ENUM_VALUE_BYTES)
        {
            return Err(over_limit(format!(
                "schema `{shown}`: an `enum` value exceeds {MAX_ENUM_VALUE_BYTES} bytes"
            )));
        }
    }
    Ok(values.clone())
}

/// A reason can quote the schema, which a compiler may have derived from an input,
/// so what it quotes is bounded.
fn bounded(text: &str) -> String {
    text.chars().take(64).collect()
}

fn shown_path(path: &str) -> String {
    if path.is_empty() {
        "/".to_string()
    } else {
        bounded(path)
    }
}

/// How a schema-bearing descriptor is told apart before its v1 depth check: the
/// G2 document is deeper than v1 allows, and only this bounded decode reads it.
pub(crate) fn descriptor_carries_schema(bytes: &[u8], max_bytes: usize) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    decode_descriptor(text, max_bytes)
        .ok()
        .is_some_and(|value| value.get("schema").is_some())
}

/// A repeated key anywhere in a schema-bearing descriptor is refused: `serde_json`
/// would keep the last one, so the pinned text and the value could disagree.
pub(crate) fn decode_descriptor(
    text: &str,
    max_bytes: usize,
) -> Result<Value, raw_json::DecodeError> {
    raw_json::decode(
        text,
        DecodeLimits {
            bytes: max_bytes,
            depth: MAX_SCHEMA_JSON_DEPTH + 4,
            elements: usize::MAX,
            lossless_numbers: true,
        },
    )
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::super::super::RefusalKind;
    use super::super::{Limits, MAX_ANY_OF_BRANCHES};
    use super::compile;

    fn object(property: Value) -> Value {
        json!({"type": "object", "additionalProperties": false, "properties": {"a": property}})
    }

    fn kind(root: Value) -> RefusalKind {
        compile(&root, Limits::OPENING).unwrap_err().kind
    }

    /// `depth` containers, the innermost one empty.
    fn nested(depth: usize) -> Value {
        let mut schema = json!({"type": "object", "additionalProperties": false, "properties": {}});
        for _ in 1..depth {
            schema = object(schema);
        }
        schema
    }

    #[test]
    fn a_keyword_outside_the_closed_vocabulary_is_refused() {
        for keyword in [
            "$ref",
            "$schema",
            "$id",
            "$defs",
            "oneOf",
            "allOf",
            "not",
            "const",
            "format",
            "default",
            "description",
            "title",
            "exclusiveMinimum",
            "multipleOf",
            "uniqueItems",
            "patternProperties",
            "unevaluatedProperties",
            "dependentSchemas",
        ] {
            let mut property = json!({"type": "string"});
            property[keyword] = json!("#/x");
            assert_eq!(
                kind(object(property)),
                RefusalKind::SchemaUnsupported,
                "{keyword}"
            );
        }
    }

    #[test]
    fn a_keyword_on_a_type_it_does_not_apply_to_is_refused() {
        for property in [
            json!({"type": "string", "minimum": 1}),
            json!({"type": "integer", "minLength": 1}),
            json!({"type": "integer", "pattern": "a"}),
            json!({"type": "number", "items": {"type": "string"}}),
            json!({"type": "boolean", "maxItems": 1}),
            json!({"type": "null", "enum": [null]}),
            json!({"type": "array", "items": {"type": "string"}, "properties": {}}),
            json!({"type": "array", "items": {"type": "string"}, "enum": [1]}),
            json!({"type": "object", "additionalProperties": false, "properties": {}, "minItems": 1}),
        ] {
            assert_eq!(
                kind(object(property.clone())),
                RefusalKind::SchemaUnsupported,
                "{property}"
            );
        }
    }

    #[test]
    fn a_shape_the_subset_does_not_define_is_refused() {
        for property in [
            json!({}),
            json!(true),
            json!({"type": ["string", "null"]}),
            json!({"type": "date"}),
            json!({"type": "string", "enum": []}),
            json!({"type": "string", "enum": [1]}),
            json!({"type": "string", "enum": ["a", "a"]}),
            json!({"type": "string", "enum": ["a"], "minLength": 1}),
            json!({"type": "string", "minLength": 2, "maxLength": 1}),
            json!({"type": "string", "minLength": -1}),
            json!({"type": "integer", "minimum": 1.5}),
            json!({"type": "integer", "minimum": 2, "maximum": 1}),
            json!({"type": "number", "minimum": 2.5, "maximum": 1.5}),
            json!({"type": "number", "minimum": "1"}),
            json!({"type": "array"}),
            json!({"type": "array", "items": {"type": "string"}, "minItems": 2, "maxItems": 1}),
            json!({"type": "object", "properties": {}}),
            json!({"type": "object", "additionalProperties": true, "properties": {}}),
            json!({"type": "object", "additionalProperties": {"type": "string"}, "properties": {}}),
            json!({"type": "object", "additionalProperties": false}),
            json!({"type": "object", "additionalProperties": false, "properties": {}, "required": ["x"]}),
            json!({"type": "object", "additionalProperties": false, "properties": {"x": {"type": "null"}}, "required": ["x", "x"]}),
            json!({"anyOf": []}),
            json!({"anyOf": {"type": "string"}}),
            json!({"anyOf": [{"type": "string"}], "type": "string"}),
        ] {
            assert_eq!(
                kind(object(property.clone())),
                RefusalKind::SchemaUnsupported,
                "{property}"
            );
        }
    }

    #[test]
    fn the_root_must_be_a_closed_object() {
        for root in [
            json!({"type": "string"}),
            json!({"anyOf": [{"type": "object", "additionalProperties": false, "properties": {}}]}),
        ] {
            assert_eq!(kind(root), RefusalKind::SchemaUnsupported);
        }
    }

    #[test]
    fn a_pattern_outside_the_one_meaning_subset_is_refused() {
        for pattern in [
            r"\d+", r"\w", r"\s", r"\bx", r"(?i)x", r"(?=x)", r"(a)\1", "(", "[",
        ] {
            assert_eq!(
                kind(object(json!({"type": "string", "pattern": pattern}))),
                RefusalKind::SchemaUnsupported,
                "{pattern}"
            );
        }
        for pattern in [r"^[a-z]{1,3}$", r"\.json$", r"(?:a|b)+", r"\n"] {
            assert!(
                compile(
                    &object(json!({"type": "string", "pattern": pattern})),
                    Limits::OPENING
                )
                .is_ok(),
                "{pattern}"
            );
        }
    }

    #[test]
    fn the_depth_limit_holds_at_its_edge() {
        assert!(compile(&nested(32), Limits::OPENING).is_ok());
        assert_eq!(kind(nested(33)), RefusalKind::SchemaLimitExceeded);
    }

    #[test]
    fn the_element_limit_holds_at_its_edge() {
        let properties = |count: usize| {
            let members = (0..count)
                .map(|index| (format!("k{index}"), json!({"type": "null"})))
                .collect::<serde_json::Map<_, _>>();
            json!({"type": "object", "additionalProperties": false, "properties": members})
        };
        assert!(compile(&properties(511), Limits::OPENING).is_ok());
        assert_eq!(kind(properties(512)), RefusalKind::SchemaLimitExceeded);
    }

    #[test]
    fn the_schema_byte_limit_holds_at_its_edge() {
        let root = object(json!({"type": "string", "pattern": "a"}));
        let size = serde_json::to_vec(&root).unwrap().len();
        let exact = Limits {
            schema_bytes: size,
            ..Limits::OPENING
        };
        assert!(compile(&root, exact).is_ok());
        let under = Limits {
            schema_bytes: size - 1,
            ..Limits::OPENING
        };
        assert_eq!(
            compile(&root, under).unwrap_err().kind,
            RefusalKind::SchemaLimitExceeded
        );
    }

    /// The opening limits themselves, which an operator can only narrow.
    #[test]
    fn the_opening_limits_are_the_protocol_values() {
        assert_eq!(Limits::OPENING.argument_bytes, 65_536);
        assert_eq!(Limits::OPENING.depth, 32);
        assert_eq!(Limits::OPENING.elements, 512);
        assert_eq!(Limits::OPENING.schema_bytes, 262_144);
        let with_name = |name: String| {
            json!({
                "type": "object", "additionalProperties": false,
                "properties": {name: {"type": "null"}},
            })
        };
        let overhead = serde_json::to_vec(&with_name(String::new())).unwrap().len();
        let exact = with_name("k".repeat(262_144 - overhead));
        assert_eq!(serde_json::to_vec(&exact).unwrap().len(), 262_144);
        assert!(compile(&exact, Limits::OPENING).is_ok());
        assert_eq!(
            kind(with_name("k".repeat(262_144 - overhead + 1))),
            RefusalKind::SchemaLimitExceeded
        );
    }

    #[test]
    fn the_computation_limits_are_refused_before_the_start() {
        let branches = |count: usize| {
            let branches = (0..count)
                .map(|_| json!({"type": "null"}))
                .collect::<Vec<_>>();
            json!({"anyOf": branches})
        };
        assert!(compile(&object(branches(MAX_ANY_OF_BRANCHES)), Limits::OPENING).is_ok());
        assert_eq!(
            kind(object(branches(MAX_ANY_OF_BRANCHES + 1))),
            RefusalKind::SchemaLimitExceeded
        );
        let mut product = json!({"type": "null"});
        for _ in 0..5 {
            product = json!({"anyOf": [product, {"type": "null"}, {"type": "null"}, {"type": "null"},
                {"type": "null"}, {"type": "null"}, {"type": "null"}, {"type": "null"}]});
        }
        assert_eq!(kind(object(product)), RefusalKind::SchemaLimitExceeded);
        let long = "a".repeat(513);
        assert_eq!(
            kind(object(json!({"type": "string", "pattern": long}))),
            RefusalKind::SchemaLimitExceeded
        );
        let huge = "(?:a{1,1000}){1,1000}";
        assert_eq!(
            kind(object(json!({"type": "string", "pattern": huge}))),
            RefusalKind::SchemaLimitExceeded
        );
    }

    #[test]
    fn nested_any_of_cannot_recurse_without_a_bound() {
        let mut schema = json!({"type": "null"});
        for _ in 0..60 {
            schema = json!({"anyOf": [schema]});
        }
        assert_eq!(kind(object(schema)), RefusalKind::SchemaLimitExceeded);
    }

    #[test]
    fn the_digest_is_of_the_exact_compact_text() {
        use sha2::{Digest, Sha256};
        let text = r#"{"additionalProperties":false,"properties":{},"type":"object"}"#;
        let root: Value = serde_json::from_str(text).unwrap();
        assert_eq!(
            compile(&root, Limits::OPENING).unwrap().sha256(),
            format!("{:x}", Sha256::digest(text.as_bytes()))
        );
    }
}
