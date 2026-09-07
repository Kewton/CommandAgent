//! A conservative recognizer for direct checks in a Node script, not a JS
//! interpreter. Opaque control flow cannot establish a failing execution path.

use super::super::{LiteralScanMode, strip_c_family_comments_and_literals};
use regex::Regex;
use std::sync::LazyLock;

pub(super) fn has_failure_check(source: &str) -> bool {
    let source = source.strip_prefix("#!").map_or(source, |rest| {
        rest.split_once('\n').map_or("", |(_, body)| body)
    });
    let Some(scan) = strip_c_family_comments_and_literals(source, LiteralScanMode::Keep) else {
        return false;
    };
    // Keep a literal as one opaque token: an assertion mentioned inside a string
    // is not a binding or a call. Template interpolation/regexp ambiguity is
    // deliberately unsupported in this small evidence recognizer.
    if scan.contains('`') {
        return false;
    }
    static TOKENS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#""(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|[A-Za-z_$][A-Za-z0-9_$]*|[0-9]+|[^\s]"#)
            .unwrap()
    });
    let tokens = TOKENS
        .find_iter(&scan)
        .map(|m| m.as_str())
        .collect::<Vec<_>>();
    // Catch/finally and process mutation require control-flow/binding analysis
    // outside this bounded recognizer. Do not borrow evidence from other files.
    if tokens.iter().any(|token| {
        matches!(
            *token,
            "catch" | "finally" | "eval" | "Function" | "constructor" | "global" | "globalThis" | "/" | "\\"
        )
    }) || tokens.windows(2).any(|t| {
        matches!(
            t[0],
            "const" | "let" | "var" | "function" | "class" | "import"
        ) && matches!(t[1], "process" | "require")
    }) || tokens
        .windows(4)
        .any(|t| t == ["process", ".", "exit", "="])
        || tokens
            .windows(3)
            .any(|t| t == ["process", ".", "on"] || t == ["process", ".", "once"])
        // Computed properties, aliases, destructuring and parameters can change
        // the identity of process/exit. Only direct exit calls are understood.
        || tokens.iter().enumerate().any(|(i, token)| {
            *token == "process" && !tokens[i + 1..].starts_with(&[".", "exit", "("])
        })
        || tokens.windows(2).any(|t| t == ["assert", "["])
    {
        return false;
    }
    let module = |token: &str| {
        matches!(
            token.trim_matches(['\'', '"']),
            "assert" | "node:assert" | "node:assert/strict"
        )
    };
    // A require inside a function/block/loop does not bind the identifier used
    // by a later top-level call. Strings are opaque tokens, so their braces do
    // not affect the lexical nesting recorded here.
    let mut nesting = 0usize;
    let mut direct_assert = false;
    for (i, token) in tokens.iter().enumerate() {
        let rest = &tokens[i..];
        if *token == "assert" {
            let previous = i.checked_sub(1).and_then(|i| tokens.get(i)).copied();
            let binding = match previous {
                Some("const") => {
                    nesting == 0
                        && rest.starts_with(&["assert", "=", "require", "("])
                        && rest.get(4).is_some_and(|token| module(token))
                        && rest.get(5) == Some(&")")
                        && rest.get(6).is_none_or(|token| *token == ";")
                }
                Some("import") => {
                    nesting == 0
                        && rest.get(1) == Some(&"from")
                        && rest.get(2).is_some_and(|token| module(token))
                }
                Some("let" | "var" | "function" | "class") => false,
                _ => {
                    // Passing or aliasing assert can permit indirect mutation.
                    // Only direct calls are understood outside its binding.
                    if rest.get(1) == Some(&"(")
                        || rest.get(1) == Some(&".") && rest.get(3) == Some(&"(")
                    {
                        continue;
                    }
                    return false;
                }
            };
            if !binding {
                return false;
            }
            direct_assert = true;
        }
        match *token {
            "(" | "{" | "[" => nesting += 1,
            ")" | "}" | "]" => nesting = nesting.saturating_sub(1),
            _ => {}
        }
        if nesting > 64 {
            return false;
        }
    }
    // Reject reassignment, property overrides and counterfeit assertion bindings.
    if tokens
        .windows(2)
        .enumerate()
        .any(|(i, t)| t == ["assert", "="] && (i == 0 || tokens[i - 1] != "const"))
        || tokens
            .windows(4)
            .any(|t| t[0] == "assert" && t[1] == "." && t[3] == "=")
    {
        return false;
    }
    checks_in_statements(&tokens, false, direct_assert)
}

fn closing(tokens: &[&str], start: usize) -> Option<usize> {
    let close = match *tokens.get(start)? {
        "(" => ")",
        "{" => "}",
        "[" => "]",
        _ => return None,
    };
    let mut i = start + 1;
    while i < tokens.len() {
        if tokens[i] == close {
            return Some(i);
        }
        if matches!(tokens[i], "(" | "{" | "[") {
            i = closing(tokens, i)?;
        }
        i += 1;
    }
    None
}

fn statement_end(tokens: &[&str], start: usize) -> usize {
    let mut i = start;
    while i < tokens.len() {
        if tokens[i] == ";" {
            return i + 1;
        }
        if matches!(tokens[i], "(" | "{" | "[") {
            let Some(end) = closing(tokens, i) else {
                return tokens.len();
            };
            i = end;
        }
        i += 1;
    }
    i
}

fn checks_in_statements(tokens: &[&str], conditional: bool, direct_assert: bool) -> bool {
    let mut i = 0;
    while i < tokens.len() {
        if tokens[i] == ";" {
            i += 1;
            continue;
        }
        if tokens[i] == "if" && tokens.get(i + 1) == Some(&"(") {
            let Some(end_condition) = closing(tokens, i + 1) else {
                return false;
            };
            let condition = &tokens[i + 2..end_condition];
            // Literal-only conditions do not constitute an inspection.
            let meaningful = condition.iter().any(|t| {
                t.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
                    && !matches!(*t, "true" | "false" | "null" | "undefined")
            });
            // Literal boolean control flow (for example false && actual.ok)
            // cannot establish a reachable inspection in this recognizer.
            if !meaningful || condition.iter().any(|t| matches!(*t, "true" | "false")) {
                return false;
            }
            i = end_condition + 1;
            let Some((body, next)) = branch(tokens, i) else {
                return false;
            };
            if meaningful && checks_in_statements(body, true, direct_assert) {
                return true;
            }
            i = next;
            if tokens.get(i) == Some(&"else") {
                let Some((body, next)) = branch(tokens, i + 1) else {
                    return false;
                };
                if meaningful && checks_in_statements(body, true, direct_assert) {
                    return true;
                }
                i = next;
            }
            continue;
        }
        let rest = &tokens[i..];
        if rest.starts_with(&["process", ".", "exit", "("]) {
            return conditional
                && rest
                    .get(4)
                    .and_then(|code| code.parse::<u8>().ok())
                    .is_some_and(|code| code != 0)
                && rest.get(5) == Some(&")");
        }
        if matches!(tokens[i], "return" | "throw") {
            return false;
        }
        if direct_assert
            && rest.first() == Some(&"assert")
            && (rest.get(1) == Some(&"(")
                || rest.get(1) == Some(&".")
                    && rest.get(3) == Some(&"(")
                    && rest.get(2).is_some_and(|method| {
                        matches!(
                            *method,
                            "ok" | "equal"
                                | "strictEqual"
                                | "deepEqual"
                                | "deepStrictEqual"
                                | "notEqual"
                                | "notStrictEqual"
                                | "notDeepEqual"
                                | "notDeepStrictEqual"
                                | "throws"
                                | "doesNotThrow"
                                | "ifError"
                                | "match"
                                | "doesNotMatch"
                                | "fail"
                        )
                    }))
        {
            return true;
        }
        // Skip function bodies, callbacks and compound expressions as a unit:
        // an assertion in an uncalled function cannot certify a plain Node run.
        i = statement_end(tokens, i);
    }
    false
}

fn branch<'a>(tokens: &'a [&'a str], start: usize) -> Option<(&'a [&'a str], usize)> {
    if tokens.get(start) == Some(&"{") {
        let end = closing(tokens, start)?;
        Some((&tokens[start + 1..end], end + 1))
    } else {
        let end = statement_end(tokens, start);
        Some((tokens.get(start..end)?, end))
    }
}
