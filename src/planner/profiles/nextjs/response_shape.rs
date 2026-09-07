//! Diagnose only response shapes established by local, literal source evidence.
//! Dynamic URLs, indirect helpers and unknown returns remain runtime obligations.
use std::collections::BTreeSet;
use std::path::Path;

use super::api_contract::{api_routes, route_matches, source_files};

mod syntax;
use syntax::Source;

#[derive(Debug, PartialEq, Eq)]
enum Shape {
    Array,
    Object(BTreeSet<String>),
}

pub(super) fn failure(root: &Path) -> Option<String> {
    let routes = api_routes(root);
    for path in source_files(root) {
        // Server request bodies are not client response bodies.
        if routes.iter().any(|(route, _)| root.join(route) == path) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Some(client) = Source::parse(&text) else {
            continue;
        };
        for i in 0..client.tokens.len() {
            let Some((response, method, url, fetch_end)) = fetch_binding(&client, i) else {
                continue;
            };
            let Some((route_path, route_text)) =
                routes.iter().find(|(p, _)| route_matches(p, &url))
            else {
                continue;
            };
            let Some(server) = Source::parse(route_text) else {
                continue;
            };
            let Some(shapes) = success_shapes(&server, &method) else {
                continue;
            };
            if let Some(expected) = incompatible_read(&client, fetch_end, response, &shapes) {
                return Some(format!(
                    "api_contract_failure: {method} {url} response shape mismatch: {} reads {expected}, but {} returns {shapes:?}; align the JSON body and check Response.ok",
                    path.strip_prefix(root).unwrap_or(&path).display(),
                    route_path.display()
                ));
            }
        }
    }
    None
}

fn fetch_binding(s: &Source, i: usize) -> Option<(&str, String, String, usize)> {
    // A const binding avoids merging mutable responses from different branches.
    if !s.is(i, "const") || !s.seq(i + 2, &["=", "await", "fetch", "("]) {
        return None;
    }
    let response = s.name(i + 1)?;
    let route = s.tokens.get(i + 6)?;
    if !route.quoted || !route.text.starts_with("/api/") || route.text.contains(['\\', '$']) {
        return None;
    }
    let end = s.pairs[i + 5]?;
    let args = s.items(i + 6, end);
    if args.first().copied() != Some((i + 6, i + 7)) || args.len() > 2 {
        return None;
    }
    let method = if let Some(&(start, end)) = args.get(1) {
        let fields = object_fields(s, start, end)?;
        if let Some((_, value, value_end)) = fields.iter().find(|(key, _, _)| key == "method") {
            let token = s.tokens.get(*value)?;
            if !token.quoted || value + 1 != *value_end {
                return None;
            }
            token.text.to_ascii_uppercase()
        } else {
            "GET".into()
        }
    } else {
        "GET".into()
    };
    let url = route
        .text
        .split(['?', '#'])
        .next()?
        .trim_end_matches('/')
        .to_string();
    Some((response, method, url, end))
}

fn incompatible_read(
    s: &Source,
    fetch_end: usize,
    response: &str,
    shapes: &[Shape],
) -> Option<String> {
    let end = s.scope_end(fetch_end);
    for i in fetch_end + 1..end {
        if s.scopes[i] != s.scopes[fetch_end]
            || !s.is(i, "const")
            || !s.seq(i + 2, &["=", "await", response, ".", "json", "(", ")"])
        {
            continue;
        }
        let body = s.name(i + 1)?;
        let start = i + 9;
        if !s.is(start, ";") {
            continue; // Chained .catch/.then or ASI flow is not inferred.
        }
        // Rebinding, shadowing or mutation makes local inference ambiguous.
        // The arrow operator is not an assignment to its parameter.
        if (start..end).any(|j| {
            (s.any(j, &["const", "let", "var"]) && s.is(j + 1, body))
                || (s.is(j, body) && s.is(j + 1, "=") && !s.any(j + 2, &["=", ">"]))
                || (s.seq(j, &[body, "."]) && s.is(j + 3, "=") && !s.is(j + 4, "="))
                || s.seq(j, &[body, "["])
        }) {
            continue;
        }
        let mut keys = BTreeSet::new();
        let mut array_read = false;
        let excluded = non_success_ranges(s, start, end, response, body);
        for j in start..end {
            if excluded.iter().any(|&(a, b)| (a..b).contains(&j)) {
                continue;
            }
            if s.seq(j, &["Array", ".", "isArray", "(", body, ")"]) {
                array_read = true;
            }
            let key = if s.seq(j, &[body, "."]) {
                s.name(j + 2)
            } else {
                None
            };
            let Some(key) = key else { continue };
            if optional_default(s, j) {
                continue;
            }
            if matches!(
                key,
                "error"
                    | "details"
                    | "hasOwnProperty"
                    | "propertyIsEnumerable"
                    | "isPrototypeOf"
                    | "constructor"
                    | "__proto__"
                    | "toString"
                    | "toLocaleString"
                    | "valueOf"
            ) {
                // Error envelopes and inherited Object members do not describe
                // the top-level keys of a successful JSON body.
                continue;
            }
            if matches!(
                key,
                "map"
                    | "filter"
                    | "forEach"
                    | "find"
                    | "reduce"
                    | "some"
                    | "every"
                    | "at"
                    | "concat"
                    | "entries"
                    | "flat"
                    | "flatMap"
                    | "includes"
                    | "indexOf"
                    | "join"
                    | "keys"
                    | "lastIndexOf"
                    | "reduceRight"
                    | "slice"
                    | "values"
                    | "findIndex"
                    | "findLast"
                    | "findLastIndex"
                    | "toReversed"
                    | "toSorted"
                    | "toSpliced"
                    | "with"
                    | "push"
                    | "pop"
                    | "shift"
                    | "unshift"
                    | "splice"
                    | "sort"
                    | "reverse"
                    | "fill"
                    | "copyWithin"
            ) {
                if s.is(j + 3, "(") {
                    array_read = true;
                } else if !shapes.iter().any(|shape| matches!(shape, Shape::Array)) {
                    // `entries`, `values`, etc. can be JSON object keys. Only
                    // a call establishes method use; arrays also inherit these
                    // members when they are passed as references.
                    keys.insert(key);
                }
            } else {
                keys.insert(key);
            }
        }
        // Array/object alternatives guarded by Array.isArray are intentionally
        // left to runtime verification, rather than rejecting either branch.
        if array_read && !keys.is_empty() {
            continue;
        }
        if array_read && shapes.iter().all(|shape| matches!(shape, Shape::Object(_))) {
            return Some(format!("an array from {body}"));
        }
        for key in keys {
            if shapes.iter().all(|shape| match shape {
                Shape::Object(keys) => !keys.contains(key),
                Shape::Array => key != "length",
            }) {
                return Some(format!("{body}.{key}"));
            }
        }
    }
    None
}

fn success_shapes(s: &Source, method: &str) -> Option<Vec<Shape>> {
    let (start, end) = method_body(s, method)?;
    let nested = callable_ranges(s, start + 1, end);
    let mut shapes = Vec::new();
    for i in start + 1..end {
        if !s.is(i, "return") || nested.iter().any(|&(a, b)| (a..b).contains(&i)) {
            continue;
        }
        let receiver = s.name(i + 1)?;
        if !matches!(receiver, "NextResponse" | "Response") || !s.seq(i + 2, &[".", "json", "("]) {
            return None;
        }
        let close = s.pairs[i + 4]?;
        let args = s.items(i + 5, close);
        if args.is_empty() || args.len() > 2 {
            return None;
        }
        let status = if let Some(&(a, b)) = args.get(1) {
            let fields = object_fields(s, a, b)?;
            if let Some((_, value, value_end)) = fields.iter().find(|(k, _, _)| k == "status") {
                if value + 1 != *value_end {
                    return None;
                }
                s.tokens[*value].text.parse::<u16>().ok()?
            } else {
                200
            }
        } else {
            200
        };
        if !(200..300).contains(&status) {
            continue;
        }
        let (a, b) = args[0];
        shapes.push(expression_shape(s, a, b, start)?);
    }
    (!shapes.is_empty()).then_some(shapes)
}

/// A scalar or unknown fallback permits an absent optional member. Preserve
/// collection-envelope checks such as E3's `json.expenses || []` and S3's
/// `json.data ?? []`: a missing collection would silently discard server data.
fn optional_default(s: &Source, read: usize) -> bool {
    let mut start = read;
    let mut after = read + 3;
    while start > 0 && s.is(after, ")") && s.pairs[after] == Some(start - 1) {
        start -= 1;
        after += 1;
    }
    if !s.seq(after, &["?", "?"]) && !s.seq(after, &["|", "|"]) {
        return false;
    }
    let mut fallback = after + 2;
    while s.is(fallback, "(") {
        fallback += 1;
    }
    !s.is(fallback, "[")
}

/// Calls nested in the handler are not returns from that handler. Unrecognized
/// function syntax remains unknown rather than contributing a response shape.
fn callable_ranges(s: &Source, start: usize, end: usize) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    for i in start..end {
        if s.seq(i, &["=", ">"]) {
            let params = i.checked_sub(1).map_or(i, |previous| {
                if s.is(previous, ")") {
                    s.pairs[previous].unwrap_or(previous)
                } else {
                    previous
                }
            });
            let body = i + 2;
            let close = if s.is(body, "{") {
                s.pairs[body].map_or(end, |close| close + 1)
            } else {
                s.expression_end(body, end)
            };
            ranges.push((params, close));
        }
        if !s.is(i, "{") {
            continue;
        }
        let function = if i > 0 && s.is(i - 1, ")") {
            s.pairs[i - 1].is_some_and(|open| {
                open > 0 && !s.any(open - 1, &["if", "for", "while", "switch", "catch", "with"])
            })
        } else {
            false
        };
        if function && let Some(close) = s.pairs[i] {
            ranges.push((i, close + 1));
        }
    }
    ranges
}

fn statement_end(s: &Source, start: usize, end: usize) -> usize {
    if s.is(start, "{") {
        return s.pairs[start].map_or(end, |i| i + 1);
    }
    let mut i = start;
    while i < end {
        if s.is(i, ";") {
            return i + 1;
        }
        if s.any(i, &["(", "[", "{"]) {
            i = s.pairs[i].unwrap_or(end);
        }
        i += 1;
    }
    end
}

fn non_success_ranges(
    s: &Source,
    start: usize,
    end: usize,
    response: &str,
    body: &str,
) -> Vec<(usize, usize)> {
    let callables = callable_ranges(s, start, end);
    let mut ranges = callables.clone();
    for i in start..end {
        if callables.iter().any(|&(a, b)| (a..b).contains(&i)) {
            continue;
        }
        if s.seq(i, &["if", "("])
            && let Some(close) = s.pairs[i + 1]
        {
            let branch_end = statement_end(s, close + 1, end);
            let error_branch = s.seq(i + 2, &["!", response, ".", "ok"]);
            let success_branch = s.seq(i + 2, &[response, ".", "ok", ")"]);
            let unknown_status = !success_branch && (i + 2..close).any(|j| s.is(j, response));
            let required_envelope = s.seq(i + 2, &["!", body, "."])
                && (close + 1..branch_end).any(|j| s.is(j, "throw"));
            let property_guard = !required_envelope && (i + 2..close).any(|j| s.is(j, body));
            if error_branch || unknown_status || property_guard {
                // The condition itself may inspect an optional key. Its branch
                // does not establish an unconditional success requirement.
                ranges.push((i, branch_end));
            }
            if success_branch && s.is(branch_end, "else") {
                ranges.push((branch_end, statement_end(s, branch_end + 1, end)));
            }
        }
        if s.is(i, response) && !s.is(i.saturating_sub(1), "if") {
            let statement = statement_end(s, i, end);
            if (i..statement).any(|j| s.is(j, "?") && !s.any(j + 1, &["?", "."])) {
                ranges.push((i, statement)); // Unknown status-dependent expression.
            }
        }
        if s.seq(i, &[body, "."]) {
            let after = i + 3;
            if s.seq(after, &["&", "&"]) || s.is(after, "?") && !s.is(after + 1, "?") {
                // Short-circuit/conditional feature checks are not proven
                // consumption, including reads in their guarded expression.
                ranges.push((i, statement_end(s, after, end)));
            }
        }
    }
    ranges
}

fn method_body(s: &Source, method: &str) -> Option<(usize, usize)> {
    for i in 0..s.tokens.len() {
        if !s.is(i, "export") || s.scopes[i].is_some() {
            continue;
        }
        let j = i + 1 + usize::from(s.is(i + 1, "async"));
        let params = if s.seq(j, &["function", method, "("]) {
            j + 2
        } else if s.seq(j, &["const", method, "="]) {
            j + 3 + usize::from(s.is(j + 3, "async"))
        } else {
            continue;
        };
        if !s.is(params, "(") {
            continue;
        }
        let after_params = s.pairs[params]? + 1;
        // Typed return declarations are unknown; don't mistake a type for a body.
        let body = after_params
            + if s.seq(after_params, &["=", ">"]) {
                2
            } else {
                0
            };
        if s.is(body, "{") {
            return Some((body, s.pairs[body]?));
        }
    }
    None
}

fn expression_shape(s: &Source, start: usize, end: usize, method_start: usize) -> Option<Shape> {
    if s.is(start, "[") && s.pairs[start] == Some(end - 1) {
        return Some(Shape::Array);
    }
    if s.is(start, "{") {
        return Some(Shape::Object(
            object_fields(s, start, end)?
                .into_iter()
                .map(|(key, _, _)| key)
                .collect(),
        ));
    }
    // Only immutable array/object literals, never infer from variable spelling.
    if start + 1 == end {
        let name = s.name(start)?;
        let declarations = (method_start..start)
            .filter(|&i| s.seq(i, &["const", name, "="]))
            .collect::<Vec<_>>();
        if declarations.len() != 1 {
            return None;
        }
        if s.scopes[declarations[0]] != s.scopes[start] {
            return None; // Never borrow a literal from an unrelated local scope.
        }
        let value = declarations[0] + 3;
        if !s.any(value, &["[", "{"]) {
            return None;
        }
        let close = s.pairs[value]?;
        if (close + 1..start).any(|i| s.is(i, name)) {
            return None;
        }
        return expression_shape(s, value, close + 1, method_start);
    }
    None
}

fn object_fields(s: &Source, start: usize, end: usize) -> Option<Vec<(String, usize, usize)>> {
    if !s.is(start, "{") || s.pairs[start] != Some(end - 1) {
        return None;
    }
    s.items(start + 1, end - 1)
        .into_iter()
        .map(|(a, b)| {
            let token = s.tokens.get(a)?;
            if !(token.quoted || s.name(a).is_some()) {
                return None;
            }
            if a + 1 == b {
                Some((token.text.clone(), a, b))
            } else if s.is(a + 1, ":") {
                Some((token.text.clone(), a + 2, b))
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
