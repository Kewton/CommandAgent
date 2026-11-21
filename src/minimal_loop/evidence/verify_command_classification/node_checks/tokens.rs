//! Lexing and bracket skipping for the Node
//! failure-check recognizer. Binding and statement rules stay in the caller.

use regex::Regex;
use std::sync::LazyLock;

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
