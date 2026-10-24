//! A conservative recognizer for direct checks in a Node script, not a JS
//! interpreter. Opaque control flow cannot establish a failing execution path.

use rejection::{scan, unsupported_construct};
use std::ops::ControlFlow;
use tokens::{branch, closing, statement_end, tokenize};

#[path = "node_checks/rejection.rs"]
mod rejection;
#[path = "node_checks/tokens.rs"]
mod tokens;

pub(super) use rejection::repair_guidance;

const ASSERT_METHODS: [&str; 15] = [
    "ok",
    "equal",
    "strictEqual",
    "deepEqual",
    "deepStrictEqual",
    "notEqual",
    "notStrictEqual",
    "notDeepEqual",
    "notDeepStrictEqual",
    "throws",
    "doesNotThrow",
    "ifError",
    "match",
    "doesNotMatch",
    "fail",
];

#[cfg(test)]
pub(super) fn has_failure_check(source: &str) -> bool {
    assertion_rejection(source).is_none()
}

/// The first reason the script is not accepted as assertion evidence, in the
/// order the recognizer applies its screens; `None` when it is accepted.
pub(super) fn assertion_rejection(source: &str) -> Option<String> {
    let scan = match scan(source) {
        Ok(scan) => scan,
        Err(detail) => return Some(detail),
    };
    let tokens = tokenize(&scan);
    if let Some(detail) = unsupported_construct(&tokens) {
        return Some(detail);
    }
    let Some(direct_assert) = assert_binding(&tokens) else {
        return Some("assert_binding_unsupported".into());
    };
    if overrides_assert(&tokens) {
        return Some("assert_overridden".into());
    }
    if checks_in_statements(&tokens, false, direct_assert) {
        return None;
    }
    Some(
        if direct_assert {
            "no_reachable_top_level_check"
        } else {
            "no_assert_binding"
        }
        .into(),
    )
}

fn is_assert_module(token: &str) -> bool {
    matches!(
        token.trim_matches(['\'', '"']),
        "assert" | "node:assert" | "node:assert/strict"
    )
}

/// `Some(true)` when the script binds `assert` directly, `Some(false)` when it
/// only calls it, `None` when `assert` is bound, aliased or passed in a way this
/// recognizer cannot trust.
fn assert_binding(tokens: &[&str]) -> Option<bool> {
    // A require inside a function/block/loop does not bind the identifier used
    // by a later top-level call. Strings are opaque tokens, so their braces do
    // not affect the lexical nesting recorded here.
    let mut nesting = 0usize;
    let mut direct_assert = false;
    for (i, token) in tokens.iter().enumerate() {
        if *token == "assert" {
            let previous = i.checked_sub(1).and_then(|i| tokens.get(i)).copied();
            direct_assert |= assert_use(previous, &tokens[i..], nesting)?;
        }
        nesting = nesting_after(nesting, token);
        if nesting > 64 {
            return None;
        }
    }
    Some(direct_assert)
}

fn nesting_after(nesting: usize, token: &str) -> usize {
    match token {
        "(" | "{" | "[" => nesting + 1,
        ")" | "}" | "]" => nesting.saturating_sub(1),
        _ => nesting,
    }
}

fn assert_use(previous: Option<&str>, rest: &[&str], nesting: usize) -> Option<bool> {
    match previous {
        Some("const") => (nesting == 0 && is_require_binding(rest)).then_some(true),
        Some("import") => (nesting == 0 && is_import_binding(rest)).then_some(true),
        Some("let" | "var" | "function" | "class") => None,
        // Passing or aliasing assert can permit indirect mutation.
        // Only direct calls are understood outside its binding.
        _ => is_direct_call(rest).then_some(false),
    }
}

fn is_require_binding(rest: &[&str]) -> bool {
    rest.starts_with(&["assert", "=", "require", "("])
        && rest.get(4).is_some_and(|token| is_assert_module(token))
        && rest.get(5) == Some(&")")
        && rest.get(6).is_none_or(|token| *token == ";")
}

fn is_import_binding(rest: &[&str]) -> bool {
    rest.get(1) == Some(&"from") && rest.get(2).is_some_and(|token| is_assert_module(token))
}

fn is_direct_call(rest: &[&str]) -> bool {
    rest.get(1) == Some(&"(") || rest.get(1) == Some(&".") && rest.get(3) == Some(&"(")
}

/// Reject reassignment, property overrides and counterfeit assertion bindings.
fn overrides_assert(tokens: &[&str]) -> bool {
    tokens
        .windows(2)
        .enumerate()
        .any(|(i, t)| t == ["assert", "="] && (i == 0 || tokens[i - 1] != "const"))
        || tokens
            .windows(4)
            .any(|t| t[0] == "assert" && t[1] == "." && t[3] == "=")
}

fn checks_in_statements(tokens: &[&str], conditional: bool, direct_assert: bool) -> bool {
    let mut i = 0;
    while i < tokens.len() {
        if tokens[i] == ";" {
            i += 1;
            continue;
        }
        if tokens[i] == "if" && tokens.get(i + 1) == Some(&"(") {
            match if_statement(tokens, i, direct_assert) {
                ControlFlow::Break(verdict) => return verdict,
                ControlFlow::Continue(next) => i = next,
            }
            continue;
        }
        let rest = &tokens[i..];
        if rest.starts_with(&["process", ".", "exit", "("]) {
            return conditional && is_nonzero_exit(rest);
        }
        if matches!(tokens[i], "return" | "throw") {
            return false;
        }
        if direct_assert && is_assert_check(rest) {
            return true;
        }
        // Skip function bodies, callbacks and compound expressions as a unit:
        // an assertion in an uncalled function cannot certify a plain Node run.
        i = statement_end(tokens, i);
    }
    false
}

/// Break with the verdict, or continue with the index after the statement.
fn if_statement(tokens: &[&str], start: usize, direct_assert: bool) -> ControlFlow<bool, usize> {
    let Some(end_condition) = closing(tokens, start + 1) else {
        return ControlFlow::Break(false);
    };
    if !inspects_something(&tokens[start + 2..end_condition]) {
        return ControlFlow::Break(false);
    }
    let mut next = branch_checks(tokens, end_condition + 1, direct_assert)?;
    if tokens.get(next) == Some(&"else") {
        next = branch_checks(tokens, next + 1, direct_assert)?;
    }
    ControlFlow::Continue(next)
}

fn inspects_something(condition: &[&str]) -> bool {
    // Literal-only conditions do not constitute an inspection.
    let meaningful = condition.iter().any(|t| {
        t.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
            && !matches!(*t, "true" | "false" | "null" | "undefined")
    });
    // Literal boolean control flow (for example false && actual.ok)
    // cannot establish a reachable inspection in this recognizer.
    meaningful && !condition.iter().any(|t| matches!(*t, "true" | "false"))
}

/// Break(true) when the branch holds a check, Break(false) when it cannot be
/// read, otherwise continue with the index after it.
fn branch_checks(tokens: &[&str], start: usize, direct_assert: bool) -> ControlFlow<bool, usize> {
    let Some((body, next)) = branch(tokens, start) else {
        return ControlFlow::Break(false);
    };
    if checks_in_statements(body, true, direct_assert) {
        return ControlFlow::Break(true);
    }
    ControlFlow::Continue(next)
}

fn is_nonzero_exit(rest: &[&str]) -> bool {
    rest.get(4)
        .and_then(|code| code.parse::<u8>().ok())
        .is_some_and(|code| code != 0)
        && rest.get(5) == Some(&")")
}

fn is_assert_check(rest: &[&str]) -> bool {
    rest.first() == Some(&"assert")
        && (rest.get(1) == Some(&"(")
            || rest.get(1) == Some(&".")
                && rest.get(3) == Some(&"(")
                && rest
                    .get(2)
                    .is_some_and(|method| ASSERT_METHODS.contains(method)))
}

#[cfg(test)]
#[path = "node_checks/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "node_checks/rejection_tests.rs"]
mod rejection_tests;
