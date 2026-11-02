//! The one validator of the G2 protocol. Native and XML receipt, and the check
//! right before an execution, all call [`validate`]. It only answers: it never
//! changes, fills in, or removes a value.

use serde_json::Value;

use super::MAX_DIAGNOSTICS;
use super::admission::{CompiledSchema, Node, ScalarKind};

const MAX_PATH_BYTES: usize = 128;

/// One reason an argument was refused. A path can hold a key the model sent, so
/// it is bounded data and is never read as an instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Violation {
    pub(crate) path: String,
    pub(crate) code: &'static str,
}

/// At most [`MAX_DIAGNOSTICS`] violations, in the order the walk met them.
pub(crate) fn validate(schema: &CompiledSchema, value: &Value) -> Result<(), Vec<Violation>> {
    let mut violations = Vec::new();
    if let Some(code) = over_limit(schema, value) {
        violations.push(Violation {
            path: "/".to_string(),
            code,
        });
        return Err(violations);
    }
    walk(schema.root(), value, "", &mut violations);
    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}

/// The limits apply to a value that came without raw text as well, such as an
/// argument given directly to the public registry.
fn over_limit(schema: &CompiledSchema, value: &Value) -> Option<&'static str> {
    let limits = schema.limits();
    let (depth, elements) = measure(value);
    if depth > limits.depth {
        return Some("arguments_too_deep");
    }
    if elements > limits.elements {
        return Some("too_many_elements");
    }
    let bytes = serde_json::to_vec(value).map_or(usize::MAX, |bytes| bytes.len());
    (bytes > limits.argument_bytes).then_some("arguments_too_large")
}

fn measure(value: &Value) -> (usize, usize) {
    match value {
        Value::Object(members) => measure_children(members.values()),
        Value::Array(items) => measure_children(items.iter()),
        _ => (0, 1),
    }
}

fn measure_children<'a>(children: impl Iterator<Item = &'a Value>) -> (usize, usize) {
    let (mut depth, mut elements) = (0, 1);
    for child in children {
        let (child_depth, child_elements) = measure(child);
        depth = depth.max(child_depth);
        elements += child_elements;
    }
    (depth + 1, elements)
}

fn walk(node: &Node, value: &Value, path: &str, out: &mut Vec<Violation>) {
    match node {
        Node::Object {
            properties,
            required,
        } => {
            let Some(members) = value.as_object() else {
                return push(out, path, "wrong_type");
            };
            for name in required {
                if !members.contains_key(name) {
                    push(out, &child(path, name), "missing_required");
                }
            }
            for (name, member) in members {
                match properties.get(name) {
                    Some(property) => walk(property, member, &child(path, name), out),
                    None => push(out, &child(path, name), "unknown_key"),
                }
            }
        }
        Node::Array {
            items,
            min_items,
            max_items,
        } => {
            let Some(values) = value.as_array() else {
                return push(out, path, "wrong_type");
            };
            if min_items.is_some_and(|min| values.len() < min) {
                push(out, path, "too_few_items");
            }
            if max_items.is_some_and(|max| values.len() > max) {
                push(out, path, "too_many_items");
            }
            for (index, item) in values.iter().enumerate() {
                walk(items, item, &child(path, &index.to_string()), out);
            }
        }
        Node::AnyOf(branches) => {
            let matched = branches.iter().any(|branch| {
                let mut trial = Vec::new();
                walk(branch, value, path, &mut trial);
                trial.is_empty()
            });
            if !matched {
                push(out, path, "union_no_match");
            }
        }
        Node::Scalar {
            kind,
            enum_values,
            min_length,
            max_length,
            pattern,
        } => {
            let Some(code) = scalar_violation(
                *kind,
                enum_values.as_deref(),
                (*min_length, *max_length),
                pattern.as_ref(),
                value,
            ) else {
                return;
            };
            push(out, path, code);
        }
    }
}

fn scalar_violation(
    kind: ScalarKind,
    enum_values: Option<&[Value]>,
    lengths: (Option<usize>, Option<usize>),
    pattern: Option<&regex::Regex>,
    value: &Value,
) -> Option<&'static str> {
    if !kind.accepts(value) {
        return Some("wrong_type");
    }
    if enum_values.is_some_and(|values| !values.contains(value)) {
        return Some("enum_mismatch");
    }
    if let Some(text) = value.as_str() {
        let length = text.chars().count();
        if lengths.0.is_some_and(|min| length < min) {
            return Some("string_too_short");
        }
        if lengths.1.is_some_and(|max| length > max) {
            return Some("string_too_long");
        }
        if pattern.is_some_and(|pattern| !pattern.is_match(text)) {
            return Some("pattern_mismatch");
        }
    }
    kind.bound_violation(value)
}

fn child(path: &str, name: &str) -> String {
    let mut joined = String::with_capacity(path.len() + name.len() + 1);
    joined.push_str(path);
    joined.push('/');
    joined.push_str(name);
    joined
}

fn push(out: &mut Vec<Violation>, path: &str, code: &'static str) {
    if out.len() >= MAX_DIAGNOSTICS {
        return;
    }
    let path = if path.is_empty() { "/" } else { path };
    out.push(Violation {
        path: truncate(path).to_string(),
        code,
    });
}

fn truncate(path: &str) -> &str {
    let mut end = path.len().min(MAX_PATH_BYTES);
    while !path.is_char_boundary(end) {
        end -= 1;
    }
    &path[..end]
}

/// The data a refusal carries: a stage and at most [`MAX_DIAGNOSTICS`] bounded
/// violations.
pub(crate) fn render(stage: &str, violations: &[Violation]) -> String {
    let violations = violations
        .iter()
        .map(|violation| serde_json::json!({"path": violation.path, "code": violation.code}))
        .collect::<Vec<_>>();
    serde_json::json!({"stage": stage, "violations": violations}).to_string()
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::super::admission::{CompiledSchema, compile};
    use super::super::{Limits, MAX_DIAGNOSTICS};
    use super::{render, validate};

    fn schema(root: Value) -> CompiledSchema {
        compile(&root, Limits::OPENING).unwrap()
    }

    fn codes(schema: &CompiledSchema, value: Value) -> Vec<&'static str> {
        validate(schema, &value)
            .err()
            .unwrap_or_default()
            .iter()
            .map(|violation| violation.code)
            .collect()
    }

    fn settings() -> CompiledSchema {
        schema(json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["mode", "retries", "tags", "target"],
            "properties": {
                "mode": {"type": "string", "enum": ["fast", "safe"]},
                "retries": {"type": "integer", "minimum": 1, "maximum": 3},
                "ratio": {"type": "number", "minimum": 0.5, "maximum": 1.5},
                "name": {"type": "string", "minLength": 2, "maxLength": 3, "pattern": "^[a-z]+$"},
                "enabled": {"type": "boolean"},
                "note": {"type": "null"},
                "tags": {"type": "array", "minItems": 1, "maxItems": 2, "items": {"type": "string"}},
                "target": {"anyOf": [
                    {"type": "object", "additionalProperties": false, "required": ["host"],
                     "properties": {"host": {"type": "string"}}},
                    {"type": "integer"},
                ]},
            },
        }))
    }

    fn valid() -> Value {
        json!({"mode": "fast", "retries": 1, "tags": ["a"], "target": {"host": "h"}})
    }

    fn with(key: &str, value: Value) -> Value {
        let mut base = valid();
        base[key] = value;
        base
    }

    #[test]
    fn a_nested_object_array_and_union_pass() {
        let schema = settings();
        assert_eq!(codes(&schema, valid()), Vec::<&str>::new());
        assert_eq!(codes(&schema, with("target", json!(7))), Vec::<&str>::new());
    }

    #[test]
    fn each_constraint_refuses_on_its_own() {
        let schema = settings();
        let cases = [
            (with("mode", json!("slow")), "enum_mismatch"),
            (with("mode", json!(1)), "wrong_type"),
            (with("mode", Value::Null), "wrong_type"),
            (with("retries", json!(0)), "below_minimum"),
            (with("retries", json!(4)), "above_maximum"),
            (with("retries", json!(1.0)), "wrong_type"),
            (with("retries", json!("1")), "wrong_type"),
            (with("ratio", json!(0.49)), "below_minimum"),
            (with("ratio", json!(1.51)), "above_maximum"),
            (with("name", json!("a")), "string_too_short"),
            (with("name", json!("abcd")), "string_too_long"),
            (with("name", json!("A1")), "pattern_mismatch"),
            (with("enabled", json!("true")), "wrong_type"),
            (with("note", json!(0)), "wrong_type"),
            (with("tags", json!([])), "too_few_items"),
            (with("tags", json!(["a", "b", "c"])), "too_many_items"),
            (with("tags", json!([1])), "wrong_type"),
            (with("target", json!("h")), "union_no_match"),
            (with("target", json!({"host": 1})), "union_no_match"),
            (
                with("target", json!({"host": "h", "x": 1})),
                "union_no_match",
            ),
            (with("extra", json!(1)), "unknown_key"),
        ];
        for (value, code) in cases {
            assert_eq!(codes(&schema, value.clone()), vec![code], "{value}");
        }
        for key in ["mode", "retries", "tags", "target"] {
            let mut value = valid();
            value.as_object_mut().unwrap().remove(key);
            assert_eq!(codes(&schema, value), vec!["missing_required"], "{key}");
        }
    }

    #[test]
    fn each_bound_admits_its_edge() {
        let schema = settings();
        for value in [
            with("retries", json!(3)),
            with("ratio", json!(0.5)),
            with("ratio", json!(1.5)),
            with("name", json!("ab")),
            with("name", json!("abc")),
            with("tags", json!(["a", "b"])),
        ] {
            assert_eq!(codes(&schema, value.clone()), Vec::<&str>::new(), "{value}");
        }
    }

    #[test]
    fn a_nested_unknown_or_missing_key_is_named_by_its_path() {
        let schema = schema(json!({
            "type": "object", "additionalProperties": false, "required": ["a"],
            "properties": {"a": {
                "type": "object", "additionalProperties": false, "required": ["b"],
                "properties": {"b": {"type": "string"}},
            }},
        }));
        let refused = validate(&schema, &json!({"a": {"c": 1}})).unwrap_err();
        let paths = refused
            .iter()
            .map(|violation| (violation.path.as_str(), violation.code))
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec![("/a/b", "missing_required"), ("/a/c", "unknown_key")]
        );
    }

    #[test]
    fn the_diagnostics_are_bounded_in_count_and_length() {
        let schema = schema(json!({
            "type": "object", "additionalProperties": false, "properties": {},
        }));
        let mut members = serde_json::Map::new();
        for index in 0..32 {
            members.insert(format!("{index}-{}", "é".repeat(200)), json!(1));
        }
        let refused = validate(&schema, &Value::Object(members)).unwrap_err();
        assert_eq!(refused.len(), MAX_DIAGNOSTICS);
        assert!(refused.iter().all(|violation| violation.path.len() <= 128));
        assert!(render("received", &refused).len() <= 2048);
    }

    #[test]
    fn the_value_limits_hold_without_raw_text() {
        let limits = Limits {
            argument_bytes: 16,
            depth: 2,
            elements: 3,
            schema_bytes: Limits::OPENING.schema_bytes,
        };
        let schema = compile(
            &json!({
                "type": "object", "additionalProperties": false,
                "properties": {"s": {"type": "string"}},
            }),
            limits,
        )
        .unwrap();
        assert_eq!(codes(&schema, json!({"s": "a"})), Vec::<&str>::new());
        assert_eq!(
            codes(&schema, json!({"a": [[]]})),
            vec!["arguments_too_deep"]
        );
        assert_eq!(
            codes(&schema, json!({"a": 1, "b": 2, "c": 3})),
            vec!["too_many_elements"]
        );
        assert_eq!(
            codes(&schema, json!({"s": "0123456789"})),
            vec!["arguments_too_large"]
        );
    }
}
