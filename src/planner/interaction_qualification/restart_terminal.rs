//! Release-gate branch for a restart hook that only exists in a terminal overlay.
//!
//! The behavioral evidence already treats an overlay-only restart as
//! `unverified:terminal_state_not_reached` (partial). The release gate must
//! agree: when only the restart hook is missing after start, and the
//! route-bound source statically carries a restart hook with a strong reset
//! handler, the gate is partial. It is never full, because the restart was not
//! exercised.

use std::collections::BTreeSet;
use std::ops::Range;
use std::path::{Path, PathBuf};

use super::InteractionFullQualification;
use crate::minimal_loop::evidence::verify_runtime_acceptance;
use crate::minimal_loop::import_scan::route_bound_closure;

pub(super) const TERMINAL_STATE_NOT_REACHED_REASON: &str =
    "interaction_unverified:terminal_state_not_reached";
const TERMINAL_STATE_NOT_REACHED_STATUS: &str = "unverified:terminal_state_not_reached";
const RESTART_MISSING_REASON: &str = "contract_instrumentation_missing:restart";
const RESTART_EVIDENCE: &str = "restart_or_recoverable_state_evidence";
const SOURCE_EXTENSIONS: [&str; 6] = ["tsx", "jsx", "ts", "js", "mjs", "cjs"];

/// Maps a qualification whose only missing piece is the in-play restart hook to
/// a partial one, or returns `None` when the legacy failure must stand.
pub(super) fn overlay_only_restart_partial(
    qualification: &InteractionFullQualification,
    root: &Path,
    profile: &str,
) -> Option<InteractionFullQualification> {
    if qualification.full_eligible
        || qualification.release_gate_reasons != [RESTART_MISSING_REASON.to_string()]
    {
        return None;
    }
    let closure = route_bound_closure(root, profile);
    let hook_files = restart_hook_files(root, &closure);
    if hook_files.is_empty() || !restart_handler_is_strong(root, &closure, &hook_files) {
        return None;
    }
    Some(InteractionFullQualification {
        full_eligible: false,
        evidence_status: TERMINAL_STATE_NOT_REACHED_STATUS.to_string(),
        release_gate_reasons: vec![TERMINAL_STATE_NOT_REACHED_REASON.to_string()],
    })
}

fn restart_hook_files(root: &Path, closure: &BTreeSet<PathBuf>) -> Vec<String> {
    closure
        .iter()
        .filter(|rel| is_source_file(rel))
        .filter(|rel| {
            std::fs::read_to_string(root.join(rel))
                .is_ok_and(|source| source_has_restart_hook(&source))
        })
        .map(|rel| rel.to_string_lossy().replace('\\', "/"))
        .collect()
}

fn is_source_file(rel: &Path) -> bool {
    rel.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| SOURCE_EXTENSIONS.contains(&ext))
}

/// Strong when the existing restart signal is strong for the route-bound
/// workspace, or when it becomes strong once the functions the restart handler
/// calls directly (one level, resolved only inside the route-bound closure) are
/// read next to the handler. The criterion itself is the existing one.
fn restart_handler_is_strong(
    root: &Path,
    closure: &BTreeSet<PathBuf>,
    hook_files: &[String],
) -> bool {
    restart_tier_is_strong(root, hook_files)
        || hook_files
            .iter()
            .any(|rel| linked_handler_is_strong(root, closure, rel))
}

fn restart_tier_is_strong(root: &Path, files: &[String]) -> bool {
    let report = verify_runtime_acceptance(
        root,
        files,
        &[],
        &[],
        &[RESTART_EVIDENCE.to_string()],
        &[],
        &[],
    );
    report
        .evidence_tiers
        .get(RESTART_EVIDENCE)
        .is_some_and(|tier| tier == "strong")
}

/// Counts a one-level factory only in this narrow form: the restart handler
/// replaces state with the value of a closure function (`setX(f())`) and `f`
/// returns an object literal carrying `0` / `[]` values. Those entries are
/// rewritten as assignments after the call, so the existing restart criterion
/// (its score and entity name tables, and its game-over transition requirement)
/// judges them. A discarded return value is never counted.
fn linked_handler_is_strong(root: &Path, closure: &BTreeSet<PathBuf>, rel: &str) -> bool {
    let Ok(source) = std::fs::read_to_string(root.join(rel)) else {
        return false;
    };
    let code = strip_comments(&source);
    for offset in restart_hook_offsets(&code) {
        let Some(span) = handler_span(&code, offset) else {
            continue;
        };
        for (name, call) in calls_in(&code, span) {
            let Some(insert_at) = state_replacing_call_end(&code, &call) else {
                continue;
            };
            let Some(definition) = callee_definition(root, closure, rel, &code, &name) else {
                continue;
            };
            for literal in returned_object_literals(&definition) {
                let resets = literal_reset_assignments(literal);
                if resets.is_empty() {
                    continue;
                }
                let synthesized =
                    format!("{}; {resets} {}", &code[..insert_at], &code[insert_at..]);
                if synthesized_is_strong(&synthesized) {
                    return true;
                }
            }
        }
    }
    false
}

fn synthesized_is_strong(page_source: &str) -> bool {
    let Ok(dir) = tempfile::tempdir() else {
        return false;
    };
    let page = "src/app/page.tsx";
    let written = std::fs::create_dir_all(dir.path().join("src/app"))
        .and_then(|()| std::fs::write(dir.path().join(page), page_source));
    written.is_ok() && restart_tier_is_strong(dir.path(), &[page.to_string()])
}

/// Byte range of the code run by the `onClick` of the restart-hook element at `offset`.
fn handler_span(code: &str, offset: usize) -> Option<Range<usize>> {
    let tag_start = code[..offset].rfind('<').unwrap_or(0);
    let bytes = code.as_bytes();
    let tag_end = (offset..bytes.len())
        .find(|&i| bytes[i] == b'>' && bytes[i - 1] != b'=')
        .unwrap_or(bytes.len());
    let tag = &code[tag_start..tag_end];
    let after = tag.find("onClick=")? + "onClick=".len();
    let rest = &tag[after..];
    let trimmed = rest.trim_start();
    let inner = trimmed.strip_prefix('{')?;
    let inner_start = tag_start + after + (rest.len() - inner.len());
    let close = matching_close(inner, '{', '}')?;
    let expression = inner[..close].trim();
    if is_identifier(expression) {
        definition_range(code, expression)
    } else {
        Some(inner_start..inner_start + close)
    }
}

/// Calls `name(...)` inside `span`, as (name, byte range of the whole call).
fn calls_in(code: &str, span: Range<usize>) -> Vec<(String, Range<usize>)> {
    const KEYWORDS: [&str; 8] = [
        "if", "for", "while", "switch", "catch", "function", "return", "typeof",
    ];
    let mut calls = Vec::new();
    let mut cursor = span.start;
    while cursor < span.end {
        let Some(c) = code[cursor..].chars().next() else {
            break;
        };
        if !is_identifier_char(c) {
            cursor += c.len_utf8();
            continue;
        }
        let start = cursor;
        while let Some(c) = code[cursor..]
            .chars()
            .next()
            .filter(|c| is_identifier_char(*c))
        {
            cursor += c.len_utf8();
        }
        let name = &code[start..cursor];
        let after_dot = code[..start].ends_with('.');
        let rest = code[cursor..].trim_start();
        if after_dot || !is_identifier(name) || KEYWORDS.contains(&name) {
            continue;
        }
        let Some(args) = rest.strip_prefix('(') else {
            continue;
        };
        let open = code.len() - args.len();
        if let Some(close) = matching_close(args, '(', ')') {
            calls.push((name.to_string(), start..open + close + 1));
        }
    }
    calls
}

/// Where to insert the rewritten assignments when the call's value replaces
/// state: right after `setX(f(..))`, or after `x = f(..)`. `None` otherwise
/// (for example a bare `f();` that discards the value).
fn state_replacing_call_end(code: &str, call: &Range<usize>) -> Option<usize> {
    let before = code[..call.start].trim_end();
    if let Some(opener) = before.strip_suffix('(') {
        let setter = opener
            .rsplit(|c: char| !is_identifier_char(c))
            .next()
            .unwrap_or("");
        let after = code[call.end..].trim_start();
        return (setter.starts_with("set") && setter.len() > 3 && after.starts_with(')'))
            .then(|| code.len() - after.len() + 1);
    }
    let assigned = before.strip_suffix('=')?;
    let statement = &assigned[assigned.rfind([';', '\n', '{', '}']).map_or(0, |at| at + 1)..];
    let declares_local = matches!(
        statement.split_whitespace().next(),
        Some("const" | "let" | "var")
    );
    let plain = !assigned.ends_with(['=', '!', '<', '>']);
    (plain && !declares_local).then_some(call.end)
}

fn callee_definition(
    root: &Path,
    closure: &BTreeSet<PathBuf>,
    rel: &str,
    code: &str,
    name: &str,
) -> Option<String> {
    imported_definition(root, closure, rel, code, name).or_else(|| definition_of(code, name))
}

/// Object literals a function returns directly: a depth-0 `return { .. }` in
/// its body, or the whole concise arrow body `=> ({ .. })`. Returns inside
/// nested blocks or nested functions are not counted.
fn returned_object_literals(definition: &str) -> Vec<&str> {
    let Some(parsed) = parse_declaration(definition) else {
        return Vec::new();
    };
    let body = &definition[parsed.body.clone()];
    if !parsed.block {
        let expression = body.trim();
        let inner = expression
            .strip_prefix('(')
            .and_then(|rest| rest.trim_start().strip_prefix('{'));
        return inner
            .and_then(|inner| {
                let close = matching_close(inner, '{', '}')?;
                inner[close + 1..]
                    .trim_start()
                    .trim_start_matches(')')
                    .trim()
                    .is_empty()
                    .then_some(&inner[..close])
            })
            .into_iter()
            .collect();
    }
    let mut literals = Vec::new();
    let mut depth = 0i32;
    for (index, c) in body.char_indices() {
        match c {
            '{' | '[' | '(' => depth += 1,
            '}' | ']' | ')' => depth -= 1,
            'r' if depth == 0 && body[index..].starts_with("return") => {
                let before_ok = body[..index]
                    .chars()
                    .next_back()
                    .is_none_or(|c| !is_identifier_char(c));
                let rest = &body[index + "return".len()..];
                if before_ok && rest.chars().next().is_none_or(|c| !is_identifier_char(c)) {
                    let rest = rest.trim_start();
                    let rest = rest.strip_prefix('(').map_or(rest, str::trim_start);
                    if let Some(inner) = rest.strip_prefix('{')
                        && let Some(close) = matching_close(inner, '{', '}')
                    {
                        literals.push(&inner[..close]);
                    }
                }
            }
            _ => {}
        }
    }
    literals
}

/// `key = 0;` / `key = [];` for each top-level `key: 0` / `key: []` entry.
fn literal_reset_assignments(literal: &str) -> String {
    let mut entries = Vec::new();
    let (mut depth, mut start) = (0i32, 0usize);
    for (index, c) in literal.char_indices() {
        match c {
            '{' | '[' | '(' => depth += 1,
            '}' | ']' | ')' => depth -= 1,
            ',' if depth == 0 => {
                entries.push(&literal[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    entries.push(&literal[start..]);
    entries
        .into_iter()
        .filter_map(|entry| {
            let (key, value) = entry.split_once(':')?;
            let key = key.trim();
            let value = value.split_whitespace().collect::<String>();
            (is_identifier(key) && matches!(value.as_str(), "0" | "[]"))
                .then(|| format!("{key} = {value};"))
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Byte index of the delimiter closing a block whose opener was just consumed.
fn matching_close(text: &str, open: char, close: char) -> Option<usize> {
    let mut depth = 1usize;
    for (index, c) in text.char_indices() {
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

fn is_identifier(text: &str) -> bool {
    !text.is_empty()
        && !text.starts_with(|c: char| c.is_ascii_digit())
        && text.chars().all(is_identifier_char)
}

fn is_identifier_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

/// The single declaration of `name` in `code` (`function` or `const` form),
/// from its keyword through the end of its body. `None` when absent or ambiguous.
fn definition_of(code: &str, name: &str) -> Option<String> {
    definition_range(code, name).map(|range| code[range].to_string())
}

fn definition_range(code: &str, name: &str) -> Option<Range<usize>> {
    let mut found = None;
    for keyword in ["function", "const", "let", "var"] {
        let mut from = 0;
        while let Some(at) = code[from..].find(keyword) {
            let start = from + at;
            from = start + keyword.len();
            let boundary_before = code[..start]
                .chars()
                .next_back()
                .is_none_or(|c| !is_identifier_char(c));
            let rest = code[from..].trim_start();
            let Some(after_name) = rest.strip_prefix(name) else {
                continue;
            };
            let declares = after_name
                .chars()
                .next()
                .is_none_or(|c| !is_identifier_char(c));
            let after_name = after_name.trim_start();
            let shaped = if keyword == "function" {
                after_name.starts_with('(') || after_name.starts_with('<')
            } else {
                after_name.starts_with('=') || after_name.starts_with(':')
            };
            if boundary_before && declares && shaped {
                if found.is_some() {
                    return None;
                }
                found = Some(start);
            }
        }
    }
    let start = found?;
    let tail = &code[start..];
    let end = parse_declaration(tail)?.end;
    Some(start..start + end)
}

struct ParsedDeclaration {
    /// End of the whole declaration, relative to its start.
    end: usize,
    /// The function body: the inside of `{ .. }`, or the concise arrow expression.
    body: Range<usize>,
    block: bool,
}

/// Parses `function f(..) { .. }`, or `const|let|var f [: T] = <function expression |
/// arrow>`. Any other initializer (a plain value, an alias, ..) yields `None`.
fn parse_declaration(declaration: &str) -> Option<ParsedDeclaration> {
    if declaration.starts_with("function") {
        return parse_function(declaration, 0);
    }
    let eq = initializer_equals(declaration)?;
    let init_start =
        eq + 1 + (declaration[eq + 1..].len() - declaration[eq + 1..].trim_start().len());
    let mut start = init_start;
    let init = &declaration[start..];
    if let Some(rest) = init.strip_prefix("async")
        && rest.starts_with(|c: char| c.is_whitespace() || c == '(')
    {
        start += init.len() - rest.trim_start().len();
    }
    let init = &declaration[start..];
    if init.starts_with("function") {
        return parse_function(declaration, start);
    }
    parse_arrow(declaration, start)
}

fn parse_function(declaration: &str, start: usize) -> Option<ParsedDeclaration> {
    let open = start + declaration[start..].find('(')?;
    let close = open + 1 + matching_close(&declaration[open + 1..], '(', ')')?;
    let brace = close + declaration[close..].find('{')?;
    if declaration[close..brace].contains(';') {
        return None;
    }
    block_body(declaration, brace)
}

fn parse_arrow(declaration: &str, start: usize) -> Option<ParsedDeclaration> {
    let init = &declaration[start..];
    let after_params = if let Some(inner) = init.strip_prefix('(') {
        start + 1 + matching_close(inner, '(', ')')? + 1
    } else {
        let len = init
            .find(|c: char| !is_identifier_char(c))
            .unwrap_or(init.len());
        if len == 0 {
            return None;
        }
        start + len
    };
    let rest = &declaration[after_params..];
    let arrow = if rest.trim_start().starts_with("=>") {
        after_params + (rest.len() - rest.trim_start().len())
    } else if rest.trim_start().starts_with(':') {
        let at = rest.find("=>")?;
        if rest[..at].contains(';') {
            return None;
        }
        after_params + at
    } else {
        return None;
    };
    let after_arrow = &declaration[arrow + 2..];
    let body_start = arrow + 2 + (after_arrow.len() - after_arrow.trim_start().len());
    let body = &declaration[body_start..];
    if body.starts_with('{') {
        return block_body(declaration, body_start);
    }
    let end = if let Some(inner) = body.strip_prefix('(') {
        body_start + 1 + matching_close(inner, '(', ')')? + 1
    } else {
        body.find([';', '\n'])
            .map_or(declaration.len(), |at| body_start + at)
    };
    Some(ParsedDeclaration {
        end,
        body: body_start..end,
        block: false,
    })
}

fn block_body(declaration: &str, brace: usize) -> Option<ParsedDeclaration> {
    let close = brace + 1 + matching_close(&declaration[brace + 1..], '{', '}')?;
    Some(ParsedDeclaration {
        end: close + 1,
        body: brace + 1..close,
        block: true,
    })
}

/// Index of the `=` that starts the initializer of a `const|let|var` declaration.
fn initializer_equals(declaration: &str) -> Option<usize> {
    let bytes = declaration.as_bytes();
    let mut depth = 0i32;
    for (index, &byte) in bytes.iter().enumerate() {
        match byte {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b';' => return None,
            b'=' if depth == 0 => {
                let next = bytes.get(index + 1).copied();
                let prev = index.checked_sub(1).map(|at| bytes[at]);
                if next != Some(b'>') && !matches!(prev, Some(b'=' | b'!' | b'<' | b'>')) {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

/// Definition of `name` when it is a named import of `rel` that resolves to a
/// file inside the route-bound closure; same-name ambiguity yields `None`.
fn imported_definition(
    root: &Path,
    closure: &BTreeSet<PathBuf>,
    rel: &str,
    code: &str,
    name: &str,
) -> Option<String> {
    let specifier = named_import_specifier(code, name)?;
    let target = resolve_in_closure(closure, rel, &specifier)?;
    let target_code = strip_comments(&std::fs::read_to_string(root.join(target)).ok()?);
    definition_of(&target_code, name)
}

fn named_import_specifier(code: &str, name: &str) -> Option<String> {
    let mut specifiers = Vec::new();
    for statement in code.split("import ").skip(1) {
        let Some((head, tail)) = statement.split_once(" from ") else {
            continue;
        };
        let Some(names) = head
            .trim()
            .strip_prefix('{')
            .and_then(|h| h.split_once('}'))
        else {
            continue;
        };
        if !names.0.split(',').any(|item| item.trim() == name) {
            continue;
        }
        let tail = tail.trim_start();
        let quote = tail.chars().next().filter(|c| matches!(c, '"' | '\''))?;
        let specifier = tail[1..].split(quote).next()?;
        specifiers.push(specifier.to_string());
    }
    (specifiers.len() == 1).then(|| specifiers.remove(0))
}

fn resolve_in_closure(
    closure: &BTreeSet<PathBuf>,
    importer: &str,
    specifier: &str,
) -> Option<PathBuf> {
    let base = if let Some(rest) = specifier.strip_prefix("@/") {
        format!("src/{rest}")
    } else if specifier.starts_with('.') {
        let dir = importer.rsplit_once('/').map_or("", |(dir, _)| dir);
        format!("{dir}/{specifier}")
    } else {
        return None;
    };
    let mut parts: Vec<&str> = Vec::new();
    for part in base.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other),
        }
    }
    let base = parts.join("/");
    let mut matches = SOURCE_EXTENSIONS
        .iter()
        .flat_map(|ext| [format!("{base}.{ext}"), format!("{base}/index.{ext}")])
        .map(PathBuf::from)
        .filter(|candidate| closure.contains(candidate));
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn source_has_restart_hook(source: &str) -> bool {
    !restart_hook_offsets(&strip_comments(source)).is_empty()
}

/// Offsets of each `data-anvil-action` attribute whose value is `restart`.
fn restart_hook_offsets(code: &str) -> Vec<usize> {
    const ATTRIBUTE: &str = "data-anvil-action";
    code.match_indices(ATTRIBUTE)
        .filter(|(at, _)| attribute_value_is_restart(&code[at + ATTRIBUTE.len()..]))
        .map(|(at, _)| at)
        .collect()
}

/// Accepts `="restart"`, `='restart'`, `={"restart"}` and `={`restart`}`.
fn attribute_value_is_restart(after_name: &str) -> bool {
    let Some(value) = after_name.trim_start().strip_prefix('=') else {
        return false;
    };
    let value = value.trim_start();
    let (value, braced) = match value.strip_prefix('{') {
        Some(inner) => (inner.trim_start(), true),
        None => (value, false),
    };
    let mut chars = value.chars();
    let Some(quote) = chars.next().filter(|c| matches!(c, '"' | '\'' | '`')) else {
        return false;
    };
    let Some(tail) = chars.as_str().strip_prefix("restart") else {
        return false;
    };
    let Some(tail) = tail.strip_prefix(quote) else {
        return false;
    };
    !braced || tail.trim_start().starts_with('}')
}

fn strip_comments(source: &str) -> String {
    #[derive(Clone, Copy, PartialEq)]
    enum State {
        Code,
        Line,
        Block,
        Quoted(char),
    }
    let mut out = String::with_capacity(source.len());
    let mut state = State::Code;
    let mut chars = source.chars().peekable();
    while let Some(c) = chars.next() {
        match state {
            State::Code => match (c, chars.peek().copied()) {
                ('/', Some('/')) => {
                    chars.next();
                    state = State::Line;
                }
                ('/', Some('*')) => {
                    chars.next();
                    state = State::Block;
                }
                ('"' | '\'' | '`', _) => {
                    out.push(c);
                    state = State::Quoted(c);
                }
                _ => out.push(c),
            },
            State::Line => {
                if c == '\n' {
                    out.push(c);
                    state = State::Code;
                }
            }
            State::Block => {
                if c == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    state = State::Code;
                }
            }
            State::Quoted(quote) => {
                out.push(c);
                if c == '\\' {
                    out.extend(chars.next());
                } else if c == quote || (c == '\n' && quote != '`') {
                    state = State::Code;
                }
            }
        }
    }
    out
}

#[cfg(test)]
#[path = "restart_terminal_tests.rs"]
mod tests;
