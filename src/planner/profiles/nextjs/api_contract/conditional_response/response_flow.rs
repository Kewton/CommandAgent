//! Recognize a mutation response checked in the promise chain of the same
//! `fetch(...)` call. The first `.then` callback binds the Response and must
//! gate the success path on its `ok`: `(r) => r.ok ? value : null` followed by
//! a `.then` that returns or throws first on the sentinel, a leading
//! `if (!r.ok) return/throw`, or a body that is only `if (r.ok) { ... }`.
//! Every later link is followed to the end of the chain. A chain the scan
//! cannot prove is `Unconfirmed`, and the caller must not fall back to a looser
//! textual check for it.

use super::{
    ResponseCheck, Tokens, is_ident_byte, lex, skip_block_comment, skip_string, skip_template,
};

/// Where a failed Response leaves the chain after a link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Failure {
    /// The chain rejects, so later `.then` callbacks are skipped.
    Rejected,
    /// The chain resolves to `null` or `undefined`. `guarded` once no success
    /// handling has run on it so far.
    Sentinel { guarded: bool },
}

/// A one-argument arrow function passed to a chain link.
struct Arrow {
    param: Option<usize>,
    body: usize,
    last: usize,
}

/// Judge the `fetch(` mutation match at `fetch_start..fetch_end` when the call
/// is followed directly by `.then`. Every token that proves the check must end
/// inside `window_end`, the end of the existing response-check window.
pub(in super::super) fn check(
    content: &str,
    fetch_start: usize,
    fetch_end: usize,
    window_end: usize,
) -> ResponseCheck {
    let Some(rest) = content.get(fetch_end..) else {
        return ResponseCheck::NotApplicable;
    };
    if !rest.contains("then") {
        return ResponseCheck::NotApplicable;
    }
    // Without tokens, only a raw `.then` right after the call is in scope.
    let unprovable = || {
        if raw_then_follows(content, fetch_start) {
            ResponseCheck::Unconfirmed
        } else {
            ResponseCheck::NotApplicable
        }
    };
    let Some(list) = lex(content) else {
        return unprovable();
    };
    let tokens = Tokens { src: content, list };
    let Ok(fetch) = tokens.list.binary_search_by_key(&fetch_start, |t| t.start) else {
        return ResponseCheck::NotApplicable;
    };
    if !tokens.is(fetch, "fetch") || !tokens.is(fetch + 1, "(") {
        return ResponseCheck::NotApplicable;
    }
    let Some(close) = tokens.matching_close(fetch + 1) else {
        return unprovable();
    };
    let link = close + 1;
    if !(tokens.is(link, ".") || tokens.is(link, "?.")) || !tokens.is(link + 1, "then") {
        return ResponseCheck::NotApplicable;
    }
    if fetch_end <= tokens.list[close].end && prove(&tokens, fetch, link, window_end).is_some() {
        ResponseCheck::Confirmed
    } else {
        ResponseCheck::Unconfirmed
    }
}

/// Whether the raw text after the `fetch(...)` call at `fetch_start` starts
/// with `.then` or `?.then`, skipping strings, templates and comments inside
/// the call. `false` when the call does not close by that scan.
fn raw_then_follows(content: &str, fetch_start: usize) -> bool {
    let bytes = content.as_bytes();
    let Some(open) = content
        .get(fetch_start..)
        .and_then(|rest| rest.find('('))
        .map(|offset| fetch_start + offset)
    else {
        return false;
    };
    let Some(close) = raw_close(bytes, open) else {
        return false;
    };
    let mut index = skip_trivia(bytes, close + 1);
    if bytes.get(index) == Some(&b'?') {
        index += 1;
    }
    if bytes.get(index) != Some(&b'.') {
        return false;
    }
    index = skip_trivia(bytes, index + 1);
    bytes[index..].starts_with(b"then") && !bytes.get(index + 4).copied().is_some_and(is_ident_byte)
}

/// Index of the `)` that closes the `(` at `open` in raw text.
fn raw_close(bytes: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut index = open;
    while index < bytes.len() {
        match bytes[index] {
            b'"' | b'\'' => {
                index = skip_string(bytes, index)?;
                continue;
            }
            b'`' => {
                index = skip_template(bytes, index + 1)?;
                continue;
            }
            b'/' if matches!(bytes.get(index + 1), Some(b'/' | b'*')) => {
                index = skip_trivia(bytes, index);
                continue;
            }
            b'(' | b'[' | b'{' => depth += 1,
            byte @ (b')' | b']' | b'}') => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return (byte == b')').then_some(index);
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

/// Skip whitespace and comments from `from`; an unclosed block comment runs
/// to the end.
fn skip_trivia(bytes: &[u8], from: usize) -> usize {
    let mut index = from;
    loop {
        while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
            index += 1;
        }
        match (bytes.get(index), bytes.get(index + 1)) {
            (Some(b'/'), Some(b'/')) => {
                while bytes.get(index).is_some_and(|byte| *byte != b'\n') {
                    index += 1;
                }
            }
            (Some(b'/'), Some(b'*')) => {
                index = skip_block_comment(bytes, index + 2).unwrap_or(bytes.len());
            }
            _ => return index,
        }
    }
}

fn prove(tokens: &Tokens<'_>, fetch: usize, link: usize, window_end: usize) -> Option<()> {
    let (open, close) = call_link(tokens, link, "then")?;
    let (mut failure, mut proof) = response_callback(tokens, &arrow(tokens, open, close)?)?;
    let mut at = close + 1;
    while tokens.is(at, ".") {
        let name = tokens.text(at + 1);
        let (open, close) = call_link(tokens, at, name)?;
        match (name, failure) {
            // A rejected chain skips the fulfillment handler.
            ("then", Failure::Rejected) => {
                single_argument(tokens, open, close)?;
            }
            ("then", Failure::Sentinel { .. }) => {
                let (next, exit) = sentinel_callback(tokens, &arrow(tokens, open, close)?)?;
                failure = next;
                proof = proof.max(exit);
            }
            ("catch", _) => {
                if inert_catch(tokens, &arrow(tokens, open, close)?)? {
                    failure = match failure {
                        Failure::Rejected => Failure::Sentinel { guarded: true },
                        sentinel => sentinel,
                    };
                }
            }
            _ => return None,
        }
        at = close + 1;
    }
    let statement_ends = at >= tokens.list.len()
        || tokens.is(at, ";")
        || tokens.is(at, "}")
        || (tokens.list[at].newline_before && tokens.can_start_statement(at));
    if !statement_ends && ![")", "]", ","].iter().any(|end| tokens.is(at, end)) {
        return None;
    }
    // A sentinel only stays contained when nothing awaits or uses the chain.
    let discarded = tokens.is_statement_start(fetch)
        || (tokens.is(fetch - 1, "void") && tokens.is_statement_start(fetch - 1));
    // Statements after an un-awaited chain run before the response arrives,
    // so only an awaited or returned chain may have statements after it.
    let awaited = fetch
        .checked_sub(1)
        .is_some_and(|before| tokens.is(before, "await") || tokens.is(before, "return"));
    let next = if tokens.is(at, ";") { at + 1 } else { at };
    let last_statement = next >= tokens.list.len() || tokens.is(next, "}");
    // Elsewhere a `}` may close an object literal; only a concise arrow body
    // closed by `}` (as in a JSX attribute) has nothing after it.
    let arrow_body = fetch
        .checked_sub(1)
        .is_some_and(|before| tokens.is(before, "=>"));
    let ends = if discarded {
        statement_ends && last_statement
    } else {
        arrow_body && tokens.is(at, "}")
    };
    if !(awaited || ends) {
        return None;
    }
    let contained = match failure {
        Failure::Rejected => true,
        Failure::Sentinel { guarded } => guarded && discarded,
    };
    (contained && tokens.list[proof].end <= window_end).then_some(())
}

/// `.name(` at `at`; returns the open and close paren indexes.
fn call_link(tokens: &Tokens<'_>, at: usize, name: &str) -> Option<(usize, usize)> {
    if !tokens.is(at, ".") || !tokens.is(at + 1, name) || !tokens.is(at + 2, "(") {
        return None;
    }
    Some((at + 2, tokens.matching_close(at + 2)?))
}

/// The call has exactly one argument, with an optional trailing comma.
fn single_argument(tokens: &Tokens<'_>, open: usize, close: usize) -> Option<()> {
    let last = if tokens.is(close - 1, ",") {
        close - 2
    } else {
        close - 1
    };
    if last <= open {
        return None;
    }
    let mut depth = 0usize;
    for index in open + 1..=last {
        if tokens.opens(index) {
            depth += 1;
        } else if tokens.closes(index) {
            depth = depth.checked_sub(1)?;
        } else if depth == 0 && tokens.is(index, ",") {
            return None;
        }
    }
    Some(())
}

/// `[async] (p) => body`, `(p: Type) => body`, `p => body` or `() => body`
/// as the only argument of the call at `open..close`.
fn arrow(tokens: &Tokens<'_>, open: usize, close: usize) -> Option<Arrow> {
    single_argument(tokens, open, close)?;
    let last = if tokens.is(close - 1, ",") {
        close - 2
    } else {
        close - 1
    };
    let mut at = open + 1;
    if tokens.is(at, "async") && !tokens.is(at + 1, "=>") {
        at += 1;
    }
    let (param, arrow_at) = if tokens.is(at, "(") {
        let end = tokens.matching_close(at)?;
        let param = match end - at {
            1 => None,
            2 if tokens.is_ident(at + 1) => Some(at + 1),
            4 if tokens.is_ident(at + 1) && tokens.is(at + 2, ":") && tokens.is_ident(at + 3) => {
                Some(at + 1)
            }
            _ => return None,
        };
        (param, end + 1)
    } else if tokens.is_ident(at) {
        (Some(at), at + 1)
    } else {
        return None;
    };
    let body = arrow_at + 1;
    if !tokens.is(arrow_at, "=>") || body > last {
        return None;
    }
    if tokens.is(body, "{") && tokens.matching_close(body)? != last {
        return None;
    }
    Some(Arrow { param, body, last })
}

/// The first callback, bound to the Response. Returns where a failed Response
/// leaves the chain and the last token of the proof.
fn response_callback(tokens: &Tokens<'_>, arrow: &Arrow) -> Option<(Failure, usize)> {
    let name = tokens.text(arrow.param?);
    let body = arrow.body;
    if tokens.is(body, "{") {
        if !tokens.is(body + 1, "if") || !tokens.is(body + 2, "(") {
            return None;
        }
        let condition_close = tokens.matching_close(body + 2)?;
        if tokens.is(body + 3, "!") {
            // `if (!r.ok [|| ...]) return/throw` leads the callback.
            if !reads_ok(tokens, body + 4, name) {
                return None;
            }
            condition_rest(tokens, body + 7, condition_close)?;
            return exit(tokens, condition_close + 1);
        }
        // `if (r.ok) { ... }` is the whole callback.
        let open = condition_close + 1;
        if !reads_ok(tokens, body + 3, name)
            || condition_close != body + 6
            || !tokens.is(open, "{")
            || tokens.matching_close(open)? + 1 != arrow.last
        {
            return None;
        }
        return Some((Failure::Sentinel { guarded: true }, body + 5));
    }
    // `r.ok ? value : null`, optionally parenthesized.
    let (mut start, mut end) = (body, arrow.last);
    while tokens.is(start, "(") && tokens.matching_close(start) == Some(end) {
        start += 1;
        end = end.checked_sub(1)?;
    }
    if start + 6 > end || !reads_ok(tokens, start, name) || !tokens.is(start + 3, "?") {
        return None;
    }
    let colon = consequent_end(tokens, start + 4, end)?;
    if colon + 1 != end
        || !sentinel(tokens, end)
        || (colon == start + 5 && sentinel(tokens, start + 4))
    {
        return None;
    }
    Some((Failure::Sentinel { guarded: false }, end))
}

/// A later callback that receives the sentinel: `if (!p [|| ...])` exits
/// first.
fn sentinel_callback(tokens: &Tokens<'_>, arrow: &Arrow) -> Option<(Failure, usize)> {
    let name = tokens.text(arrow.param?);
    let body = arrow.body;
    if !tokens.is(body, "{")
        || !tokens.is(body + 1, "if")
        || !tokens.is(body + 2, "(")
        || !tokens.is(body + 3, "!")
        || !tokens.is_ident(body + 4)
        || tokens.text(body + 4) != name
    {
        return None;
    }
    let condition_close = tokens.matching_close(body + 2)?;
    condition_rest(tokens, body + 5, condition_close)?;
    exit(tokens, condition_close + 1)
}

/// `.catch` that only yields a sentinel (`true`) or rethrows (`false`).
fn inert_catch(tokens: &Tokens<'_>, arrow: &Arrow) -> Option<bool> {
    let body = arrow.body;
    if tokens.is(body, "{") {
        if arrow.last == body + 1 {
            return Some(true);
        }
        return tokens.is(body + 1, "throw").then_some(false);
    }
    (arrow.last == body && sentinel(tokens, body)).then_some(true)
}

fn reads_ok(tokens: &Tokens<'_>, at: usize, name: &str) -> bool {
    tokens.is_ident(at)
        && tokens.text(at) == name
        && tokens.is(at + 1, ".")
        && tokens.is(at + 2, "ok")
}

fn sentinel(tokens: &Tokens<'_>, at: usize) -> bool {
    tokens.is_ident(at) && matches!(tokens.text(at), "null" | "undefined")
}

/// The condition ends at `close`, or continues with `|| ...` that assigns
/// nothing.
fn condition_rest(tokens: &Tokens<'_>, at: usize, close: usize) -> Option<()> {
    if at == close {
        return Some(());
    }
    if !tokens.is(at, "||") || at + 1 >= close {
        return None;
    }
    (at + 1..close)
        .all(|index| !tokens.is(index, "=") && !tokens.is(index, "=>"))
        .then_some(())
}

/// Index of the `:` that ends a simple ternary consequent starting at `start`.
fn consequent_end(tokens: &Tokens<'_>, start: usize, end: usize) -> Option<usize> {
    let mut depth = 0usize;
    for index in start..end {
        if tokens.opens(index) {
            depth += 1;
        } else if tokens.closes(index) {
            depth = depth.checked_sub(1)?;
        } else if depth == 0 {
            if tokens.is(index, ":") {
                return (index > start).then_some(index);
            }
            if ["?", "=>", ",", ";"]
                .iter()
                .any(|text| tokens.is(index, text))
            {
                return None;
            }
        }
    }
    None
}

/// The unconditional exit of an `if` whose consequent starts at `at`.
fn exit(tokens: &Tokens<'_>, at: usize) -> Option<(Failure, usize)> {
    let at = if tokens.is(at, "{") {
        let end = tokens.matching_close(at)?;
        let mut depth = 0usize;
        (at + 1..end).find(|&index| {
            if tokens.opens(index) {
                depth += 1;
            } else if tokens.closes(index) {
                depth = depth.saturating_sub(1);
            } else if depth == 0
                && (tokens.is(index, "return") || tokens.is(index, "throw"))
                // A line break alone may end an `if (...)` or loop head.
                && ["{", ";", "}"].iter().any(|text| tokens.is(index - 1, text))
            {
                return true;
            }
            false
        })?
    } else {
        at
    };
    if tokens.is(at, "throw") {
        return Some((Failure::Rejected, at));
    }
    if !tokens.is(at, "return") {
        return None;
    }
    let value = at + 1;
    let ends = |index: usize| {
        tokens
            .list
            .get(index)
            .is_none_or(|token| token.newline_before)
            || tokens.is(index, ";")
            || tokens.is(index, "}")
    };
    if ends(value) {
        Some((Failure::Sentinel { guarded: true }, at))
    } else if sentinel(tokens, value) && ends(value + 1) {
        Some((Failure::Sentinel { guarded: true }, value))
    } else {
        None
    }
}

#[cfg(test)]
mod tests;
