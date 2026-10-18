//! Lexing, bracket skipping and token-pattern screening for the Node
//! failure-check recognizer. Binding and statement rules stay in the caller.

use super::super::super::{LiteralScanMode, strip_c_family_comments_and_literals};
use regex::Regex;
use std::sync::LazyLock;

/// Strip the shebang and comments. `None` means the source is outside what the
/// recognizer can read: an unterminated literal or comment, or a template.
pub(super) fn scan(source: &str) -> Option<String> {
    let source = source.strip_prefix("#!").map_or(source, |rest| {
        rest.split_once('\n').map_or("", |(_, body)| body)
    });
    let scan = strip_c_family_comments_and_literals(source, LiteralScanMode::Keep)?;
    // Keep a literal as one opaque token: an assertion mentioned inside a string
    // is not a binding or a call. Template interpolation/regexp ambiguity is
    // deliberately unsupported in this small evidence recognizer.
    (!scan.contains('`')).then_some(scan)
}

pub(super) fn tokenize(scan: &str) -> Vec<&str> {
    static TOKENS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#""(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|[A-Za-z_$][A-Za-z0-9_$]*|[0-9]+|[^\s]"#)
            .unwrap()
    });
    TOKENS.find_iter(scan).map(|m| m.as_str()).collect()
}

pub(super) fn closing(tokens: &[&str], start: usize) -> Option<usize> {
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

pub(super) fn statement_end(tokens: &[&str], start: usize) -> usize {
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

pub(super) fn branch<'a>(tokens: &'a [&'a str], start: usize) -> Option<(&'a [&'a str], usize)> {
    if tokens.get(start) == Some(&"{") {
        let end = closing(tokens, start)?;
        Some((&tokens[start + 1..end], end + 1))
    } else {
        let end = statement_end(tokens, start);
        Some((tokens.get(start..end)?, end))
    }
}

/// Catch/finally and process mutation require control-flow/binding analysis
/// outside this bounded recognizer. Do not borrow evidence from other files.
pub(super) fn has_unsupported_construct(tokens: &[&str]) -> bool {
    has_opaque_token(tokens)
        || shadows_process_or_require(tokens)
        || mutates_process(tokens)
        || tokens.windows(2).any(|t| t == ["assert", "["])
}

fn has_opaque_token(tokens: &[&str]) -> bool {
    tokens.iter().any(|token| {
        matches!(
            *token,
            "catch"
                | "finally"
                | "eval"
                | "Function"
                | "constructor"
                | "global"
                | "globalThis"
                | "/"
                | "\\"
        )
    })
}

fn shadows_process_or_require(tokens: &[&str]) -> bool {
    tokens.windows(2).any(|t| {
        matches!(
            t[0],
            "const" | "let" | "var" | "function" | "class" | "import"
        ) && matches!(t[1], "process" | "require")
    })
}

fn mutates_process(tokens: &[&str]) -> bool {
    tokens
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
}
