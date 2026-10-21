//! Conservative local JSON-wrapper and request-key checks. No domain vocabulary.
use super::*;

pub(super) fn failure(root: &Path) -> Option<String> {
    inspect(root, false)
}

pub(super) fn advisory(root: &Path) -> Option<String> {
    inspect(root, true)
}

fn inspect(root: &Path, advisory: bool) -> Option<String> {
    let routes = api_routes(root);
    for path in source_files(root) {
        if routes.iter().any(|(p, _)| root.join(p) == path) {
            continue;
        }
        let text = std::fs::read_to_string(&path).ok()?;
        let Some(client) = Source::parse(&text) else {
            continue;
        };
        for helper in json_helpers(&client) {
            for i in 0..client.tokens.len() {
                if !client.seq(i, &["await", &helper, "("]) {
                    continue;
                }
                let Some(close) = client.pairs[i + 2] else {
                    continue;
                };
                let args = client.items(i + 3, close);
                let Some(&(a, b)) = args.first() else {
                    continue;
                };
                if a + 1 != b {
                    continue;
                }
                let token = &client.tokens[a];
                if !token.quoted || !token.text.starts_with("/api/") || token.text.contains('\\') {
                    continue;
                }
                // Interpolation is supported only as a complete path segment.
                let url = token.text.split('?').next().unwrap_or(&token.text);
                if url
                    .split('/')
                    .any(|p| p.contains('$') && !(p.starts_with("${") && p.ends_with('}')))
                {
                    continue;
                }
                let fields = match args.get(1) {
                    Some(&(a, b)) => match object_fields(&client, a, b) {
                        Some(f) => f,
                        None => continue,
                    },
                    None => Vec::new(),
                };
                if args.len() > 2 {
                    continue;
                }
                let method = match fields.iter().find(|(k, _, _)| k == "method") {
                    Some((_, a, b)) if a + 1 == *b && client.tokens[*a].quoted => {
                        client.tokens[*a].text.to_uppercase()
                    }
                    Some(_) => continue,
                    None => "GET".into(),
                };
                let Some((route, server_text)) = routes.iter().find(|(p, _)| route_matches(p, url))
                else {
                    continue;
                };
                let Some(server) = Source::parse(server_text) else {
                    continue;
                };
                let source = path.strip_prefix(root).unwrap_or(&path).display();
                if advisory
                    && method == "GET"
                    && i >= 3
                    && client.seq(i - 3, &["const"])
                    && client.is(i - 1, "=")
                    && let Some(body) = client.name(i - 2)
                    && let Some(shapes) = success_shapes(&server, &method)
                    && let Some(key) = consumed_collection(&client, close + 1, body, &shapes)
                {
                    return Some(format!(
                        "Optional collection supply to inspect: {method} {url}: {source} reads {body}.{key}, but {} does not supply it. A local default or another source may supply the collection; establish the goal's directory source and verify selection and reload. This is not a proven failure.",
                        route.display()
                    ));
                }
                if !advisory
                    && matches!(method.as_str(), "POST" | "PUT" | "PATCH")
                    && let Some((_, a, b)) = fields.iter().find(|(k, _, _)| k == "body")
                    && client.seq(*a, &["JSON", ".", "stringify", "("])
                    && client.pairs[*a + 3] == Some(b - 1)
                    && let Some(payload) = object_fields(&client, a + 4, b - 1)
                    && let Some(reads) = request_keys(&server, &method)
                {
                    let sent: BTreeSet<_> = payload.into_iter().map(|(k, _, _)| k).collect();
                    if !sent.is_empty() && !reads.is_empty() && sent.is_disjoint(&reads) {
                        return Some(format!(
                            "api_contract_failure: {method} {url} request field mismatch: {source} sends {sent:?}, but {} reads {reads:?}; align request, shared types and stored value, then verify the committed result after reload",
                            route.display()
                        ));
                    }
                }
            }
        }
    }
    None
}

/// Only a single top-level helper forwarding its two parameters, with an
/// immutable fetch binding and a final response.json() return. Do not borrow
/// same-named helpers, reassigned responses, transforms or alternate returns.
fn json_helpers(s: &Source) -> Vec<String> {
    let mut helpers = Vec::new();
    for i in 0..s.tokens.len() {
        if s.scopes[i].is_some() || !s.seq(i, &["async", "function"]) {
            continue;
        }
        let Some(name) = s.name(i + 2) else { continue };
        if !s.is(i + 3, "(") {
            continue;
        }
        let Some(params_end) = s.pairs[i + 3] else {
            continue;
        };
        let params = s.items(i + 4, params_end);
        if params.len() != 2 {
            continue;
        }
        let (Some(url), Some(init)) = (s.name(params[0].0), s.name(params[1].0)) else {
            continue;
        };
        let start = params_end + 1;
        if !s.is(start, "{") {
            continue;
        }
        let Some(end) = s.pairs[start] else { continue };
        let fetch = start + 1;
        let Some(response) = s.name(fetch + 1) else {
            continue;
        };
        if !s.is(fetch, "const")
            || !s.seq(
                fetch + 2,
                &["=", "await", "fetch", "(", url, ",", init, ")", ";"],
            )
        {
            continue;
        }
        let tail = end.saturating_sub(7);
        if !s.seq(tail, &["return", response, ".", "json", "(", ")", ";"]) {
            continue;
        }
        let middle = fetch + 11;
        // The only intervening top-level statement may be a throwing !ok guard.
        if !s.seq(middle, &["if", "(", "!", response, ".", "ok", ")", "{"]) {
            continue;
        }
        if s.pairs[middle + 7] != Some(tail - 1) {
            continue;
        }
        if !(middle + 8..tail).any(|j| s.is(j, "throw"))
            || (middle + 8..tail).any(|j| s.is(j, "return") || s.seq(j, &[response, "="]))
        {
            continue;
        }
        // Shadowing/rebinding anywhere in this file makes call resolution unknown.
        if (0..s.tokens.len()).any(|j| {
            s.is(j, name) && j != i + 2 && !(s.is(j.saturating_sub(1), "await") && s.is(j + 1, "("))
        }) {
            continue;
        }
        helpers.push(name.to_string());
    }
    helpers
}

fn consumed_collection(s: &Source, start: usize, body: &str, shapes: &[Shape]) -> Option<String> {
    let end = s.scope_end(start);
    if (start..end).any(|j| {
        s.seq(j, &[body, "="]) || (s.any(j, &["const", "let", "var"]) && s.is(j + 1, body))
    }) {
        return None;
    }
    for j in start..end {
        // The exact guarded collection-to-state pattern in the saved source.
        if !s.seq(j, &["if", "(", "Array", ".", "isArray", "(", body, "."]) {
            continue;
        }
        let Some(key) = s.name(j + 8) else { continue };
        if !s.seq(j + 9, &[")", ")"]) {
            continue;
        }
        let call = j + 11;
        let Some(setter) = s.name(call) else { continue };
        if !s.seq(call + 1, &["(", body, ".", key, ")", ";"]) {
            continue;
        }
        // Establish useState's setter, rather than arbitrary optional logging.
        let state_setter = (0..j).any(|k| {
            s.seq(k, &["const", "["]) && s.seq(k + 3, &[",", setter, "]", "=", "useState"])
        });
        if state_setter
            && shapes
                .iter()
                .all(|shape| matches!(shape, Shape::Object(keys) if !keys.contains(key)))
        {
            return Some(key.to_string());
        }
    }
    None
}

fn request_keys(s: &Source, method: &str) -> Option<BTreeSet<String>> {
    let (start, end) = method_body(s, method)?;
    let mut keys = BTreeSet::new();
    let mut locals = BTreeSet::new();
    // Recognize `const body: unknown = await request.json();` then literal
    // destructuring from that body. Helpers, aliases and computed keys are unknown.
    for i in start..end {
        if !s.is(i, "const") {
            continue;
        }
        let Some(body) = s.name(i + 1) else { continue };
        let equal = i
            + 2
            + if s.seq(i + 2, &[":", "unknown"]) {
                2
            } else {
                0
            };
        if !s.seq(equal, &["=", "await"]) || !s.seq(equal + 3, &[".", "json", "(", ")", ";"]) {
            continue;
        }
        // Verify that the receiver is the route's first Request parameter.
        let params = (0..start)
            .rev()
            .find(|&j| s.is(j, "(") && s.pairs[j] == Some(start - 1))?;
        if s.name(params + 1) != s.name(equal + 2) {
            continue;
        }
        for j in equal + 8..end {
            // Any use other than the recognized destructuring or a simple
            // null/type guard can consume additional keys. Do not infer that
            // the destructured set is exhaustive in that case.
            if s.is(j, body)
                && !(s.is(j.saturating_sub(1), "=") && s.is(j.saturating_sub(2), "}"))
                && (!s.any(j.saturating_sub(1), &["!", "typeof"]) || s.any(j + 1, &[".", "[", "?"]))
            {
                return None;
            }
            if !s.seq(j, &["const", "{"]) {
                continue;
            }
            let close = s.pairs[j + 1]?;
            if !s.seq(close + 1, &["=", body]) {
                continue;
            }
            for (a, b) in s.items(j + 2, close) {
                if a + 1 != b && !(a + 3 == b && s.is(a + 1, ":") && s.name(a + 2).is_some()) {
                    return None;
                }
                keys.insert(s.name(a)?.to_string());
                locals.insert(s.name(if a + 1 == b { a } else { a + 2 })?.to_string());
            }
        }
    }
    if keys.is_empty() || !success_requires_key(s, start, end, &locals) {
        return None;
    }
    Some(keys)
}

// Only reject disjoint keys when every observed 2xx return is behind one of
// the saved handler's `if (key !== undefined)` guards. Optional/default-only
// handlers and indirect return helpers do not establish required input keys.
fn success_requires_key(s: &Source, start: usize, end: usize, keys: &BTreeSet<String>) -> bool {
    let guards = (start..end)
        .filter_map(|i| {
            let key = s.name(i + 2)?;
            if !keys.contains(key)
                || !s.seq(i, &["if", "(", key, "!", "=", "=", "undefined", ")", "{"])
            {
                return None;
            }
            Some((i + 8, s.pairs[i + 8]?))
        })
        .collect::<Vec<_>>();
    let mut successes = 0;
    for i in start..end {
        if !s.is(i, "return") {
            continue;
        }
        if !s.any(i + 1, &["NextResponse", "Response"]) || !s.seq(i + 2, &[".", "json", "("]) {
            return false;
        }
        let Some(close) = s.pairs[i + 4] else {
            return false;
        };
        let args = s.items(i + 5, close);
        let status = match args.get(1) {
            None => 200,
            Some(&(a, b)) => {
                let Some(fields) = object_fields(s, a, b) else {
                    return false;
                };
                let Some((_, a, b)) = fields.iter().find(|(k, _, _)| k == "status") else {
                    return false;
                };
                if a + 1 != *b {
                    return false;
                }
                let Ok(status) = s.tokens[*a].text.parse::<u16>() else {
                    return false;
                };
                status
            }
        };
        if (200..300).contains(&status) {
            if !guards.iter().any(|&(a, b)| (a..b).contains(&i)) {
                return false;
            }
            successes += 1;
        }
    }
    successes > 0
}

#[cfg(test)]
mod tests;
