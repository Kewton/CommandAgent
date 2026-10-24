//! Recognize one narrow shape of mutation response check: a single `const`
//! declaration whose initializer is a simple ternary with a direct
//! `await fetch(...)` in each branch, followed by a sibling
//! `if (!name.ok ...)` guard that returns or throws. Anything the scan cannot
//! prove is `Unconfirmed`; it never falls back to a looser textual check.

const WINDOW_BYTES: usize = 4_096;

/// What the scan concluded about one `fetch(...)` mutation match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ResponseCheck {
    /// Not a branch of a `const name = ... ? await fetch : await fetch` form.
    NotApplicable,
    /// The ternary form was proven to be checked by a failure guard.
    Confirmed,
    /// The ternary form was recognized but the guard could not be proven.
    Unconfirmed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Ident,
    Opaque,
    Punct,
}

#[derive(Debug, Clone, Copy)]
struct Token {
    kind: Kind,
    start: usize,
    end: usize,
    newline_before: bool,
}

struct Tokens<'a> {
    src: &'a str,
    list: Vec<Token>,
}

impl Tokens<'_> {
    fn text(&self, index: usize) -> &str {
        self.list
            .get(index)
            .and_then(|token| self.src.get(token.start..token.end))
            .unwrap_or("")
    }

    fn is(&self, index: usize, text: &str) -> bool {
        self.list
            .get(index)
            .is_some_and(|token| token.kind != Kind::Opaque)
            && self.text(index) == text
    }

    fn is_ident(&self, index: usize) -> bool {
        self.list
            .get(index)
            .is_some_and(|token| token.kind == Kind::Ident)
    }

    fn opens(&self, index: usize) -> bool {
        ["(", "[", "{"].iter().any(|open| self.is(index, open))
    }

    fn closes(&self, index: usize) -> bool {
        [")", "]", "}"].iter().any(|close| self.is(index, close))
    }

    /// Whether the token ends an expression, so a following line may start a
    /// new statement.
    fn ends_expression(&self, index: usize) -> bool {
        let Some(token) = self.list.get(index) else {
            return false;
        };
        match token.kind {
            Kind::Opaque => true,
            Kind::Ident => !CONTINUING.contains(&self.text(index)),
            Kind::Punct => self.closes(index),
        }
    }

    fn can_start_statement(&self, index: usize) -> bool {
        self.is_ident(index) && !CONTINUATION_STARTS.contains(&self.text(index))
    }

    fn is_statement_start(&self, index: usize) -> bool {
        index == 0
            || self.is(index - 1, ";")
            || self.is(index - 1, "{")
            || self.is(index - 1, "}")
            || (self.list[index].newline_before
                && self.ends_expression(index - 1)
                && self.can_start_statement(index))
    }

    /// Index of the bracket that closes the one at `open`.
    fn matching_close(&self, open: usize) -> Option<usize> {
        let mut depth = 0usize;
        for index in open..self.list.len() {
            if self.opens(index) {
                depth += 1;
            } else if self.closes(index) {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    let pair = [("(", ")"), ("[", "]"), ("{", "}")]
                        .iter()
                        .any(|(left, right)| self.is(open, left) && self.is(index, right));
                    return pair.then_some(index);
                }
            }
        }
        None
    }
}

/// Words after which a new line continues the expression.
const CONTINUING: &[&str] = &[
    "as",
    "satisfies",
    "typeof",
    "in",
    "instanceof",
    "new",
    "await",
    "void",
    "delete",
    "of",
    "else",
    "do",
    "return",
    "throw",
    "case",
    "extends",
    "keyof",
];
/// Words that continue the previous statement instead of starting one.
const CONTINUATION_STARTS: &[&str] = &[
    "as",
    "satisfies",
    "in",
    "instanceof",
    "of",
    "else",
    "catch",
    "finally",
    "extends",
];
/// Words that mean the scan left the expression it was looking for.
const STATEMENT_WORDS: &[&str] = &[
    "return", "throw", "if", "else", "for", "while", "do", "switch", "case", "default", "try",
    "catch", "finally", "break", "continue", "const", "let", "var", "function", "class", "import",
    "export",
];
/// Words after which a `/` starts a regular expression.
const REGEX_AFTER: &[&str] = &[
    "return", "typeof", "case", "in", "of", "delete", "void", "throw", "new", "await", "yield",
    "else", "do",
];

/// Judge the `fetch(` mutation match at `fetch_start..fetch_end`. `window_end`
/// is the end of the existing response-check window (4,096 bytes after the
/// match, on a char boundary); the guard must end inside it.
pub(super) fn check(
    content: &str,
    fetch_start: usize,
    fetch_end: usize,
    window_end: usize,
) -> ResponseCheck {
    if !follows_branch_marker(content, fetch_start) {
        return ResponseCheck::NotApplicable;
    }
    let Some(list) = lex(content) else {
        return ResponseCheck::NotApplicable;
    };
    let tokens = Tokens { src: content, list };
    let Ok(fetch) = tokens.list.binary_search_by_key(&fetch_start, |t| t.start) else {
        return ResponseCheck::NotApplicable;
    };
    let Some((equals, name)) = find_declaration(&tokens, fetch) else {
        return ResponseCheck::NotApplicable;
    };
    if confirm(&tokens, equals, name, fetch, fetch_end, window_end).is_some() {
        ResponseCheck::Confirmed
    } else {
        ResponseCheck::Unconfirmed
    }
}

/// Cheap pre-filter: the text before `fetch` ends with `? await` or `: await`.
fn follows_branch_marker(content: &str, fetch_start: usize) -> bool {
    let Some(before) = content.get(..fetch_start) else {
        return false;
    };
    let Some(before) = before.trim_end().strip_suffix("await") else {
        return false;
    };
    let before = before.trim_end();
    before.ends_with('?') || before.ends_with(':')
}

/// Walk back from the `fetch` token to `const name =` at the same depth.
fn find_declaration(tokens: &Tokens<'_>, fetch: usize) -> Option<(usize, usize)> {
    if !tokens.is(fetch.checked_sub(1)?, "await")
        || !(tokens.is(fetch.checked_sub(2)?, "?") || tokens.is(fetch.checked_sub(2)?, ":"))
    {
        return None;
    }
    let mut depth = 0usize;
    let mut index = fetch;
    while index > 0 {
        index -= 1;
        if tokens.closes(index) {
            depth += 1;
        } else if tokens.opens(index) {
            depth = depth.checked_sub(1)?;
        } else if depth > 0 {
            continue;
        } else if tokens.is(index, ";") {
            return None;
        } else if tokens.is(index, "=") {
            let name = index.checked_sub(1)?;
            let keyword = index.checked_sub(2)?;
            return (tokens.is_ident(name)
                && tokens.is_ident(keyword)
                && tokens.is(keyword, "const")
                && tokens.is_statement_start(keyword))
            .then_some((index, name));
        } else if tokens.is_ident(index) && STATEMENT_WORDS.contains(&tokens.text(index)) {
            return None;
        }
    }
    None
}

fn confirm(
    tokens: &Tokens<'_>,
    equals: usize,
    name: usize,
    fetch: usize,
    fetch_end: usize,
    window_end: usize,
) -> Option<()> {
    let condition_end = condition_end(tokens, equals + 1)?;
    let (consequent_fetch, consequent_close) = await_fetch(tokens, condition_end + 1)?;
    let colon = consequent_close + 1;
    if !tokens.is(colon, ":") {
        return None;
    }
    let (alternate_fetch, alternate_close) = await_fetch(tokens, colon + 1)?;
    let close = if fetch == consequent_fetch {
        consequent_close
    } else if fetch == alternate_fetch {
        alternate_close
    } else {
        return None;
    };
    // The mutation match must stay inside its own branch's call.
    if fetch_end > tokens.list[close].end
        || tokens.list[alternate_close].end - tokens.list[equals].start > WINDOW_BYTES
    {
        return None;
    }
    let first = match alternate_close + 1 {
        next if tokens.is(next, ";") => next + 1,
        next if tokens.list.get(next)?.newline_before && tokens.can_start_statement(next) => next,
        _ => return None,
    };
    guard_follows(tokens, first, tokens.text(name), window_end)
}

/// Index of the `?` that ends a simple, single-expression condition.
fn condition_end(tokens: &Tokens<'_>, start: usize) -> Option<usize> {
    // A line break right after `=` is a separate, unsupported layout.
    if tokens.list.get(start)?.newline_before {
        return None;
    }
    let mut depth = 0usize;
    let mut index = start;
    loop {
        if index >= tokens.list.len() {
            return None;
        }
        if tokens.is(index, "?") {
            return (depth == 0 && index > start).then_some(index);
        }
        if tokens.opens(index) {
            depth += 1;
        } else if tokens.closes(index) {
            depth = depth.checked_sub(1)?;
        } else if (depth == 0 && tokens.is(index, ":"))
            || tokens.is(index, ";")
            || tokens.is(index, "=")
            || tokens.is(index, "=>")
            || (tokens.is_ident(index)
                && (STATEMENT_WORDS.contains(&tokens.text(index))
                    || matches!(tokens.text(index), "await" | "fetch" | "yield")))
            || (index > start && depth == 0 && tokens.is_statement_start(index))
        {
            return None;
        }
        index += 1;
    }
}

/// `await fetch ( ... )` at `start`; returns the `fetch` and `)` token indexes.
fn await_fetch(tokens: &Tokens<'_>, start: usize) -> Option<(usize, usize)> {
    if !tokens.is(start, "await") || !tokens.is(start + 1, "fetch") || !tokens.is(start + 2, "(") {
        return None;
    }
    let close = tokens.matching_close(start + 2)?;
    Some((start + 1, close))
}

/// Find the failure guard among the sibling statements that follow. Only
/// declarations that read the response may sit in between.
fn guard_follows(tokens: &Tokens<'_>, first: usize, name: &str, window_end: usize) -> Option<()> {
    let mut start = first;
    loop {
        if tokens.is(start, "if") {
            return guard(tokens, start, name, window_end);
        }
        if !(tokens.is(start, "const") || tokens.is(start, "let")) {
            return None;
        }
        let end = statement_end(tokens, start)?;
        let mut mentions = false;
        let mut binding = true;
        let mut depth = 0usize;
        for index in start + 1..end {
            if tokens.opens(index) {
                depth += 1;
            } else if tokens.closes(index) {
                depth = depth.checked_sub(1)?;
            }
            if binding && depth == 0 && tokens.is(index, "=") {
                binding = false;
            }
            if tokens.is_ident(index) && tokens.text(index) == name {
                if binding {
                    return None;
                }
                mentions = true;
            }
        }
        if !mentions {
            return None;
        }
        start = end;
    }
}

/// Index of the first token of the statement after the one starting at `start`.
fn statement_end(tokens: &Tokens<'_>, start: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut index = start;
    loop {
        if tokens.opens(index) {
            depth += 1;
        } else if tokens.closes(index) {
            depth = depth.checked_sub(1)?;
        } else if depth == 0 && tokens.is(index, ";") {
            return Some(index + 1);
        }
        index += 1;
        let next = tokens.list.get(index)?;
        if depth == 0
            && next.newline_before
            && tokens.ends_expression(index - 1)
            && tokens.can_start_statement(index)
        {
            return Some(index);
        }
    }
}

/// `if (!name.ok) throw ...`, or `if (!name.ok [|| ...]) { ...; return/throw }`.
fn guard(tokens: &Tokens<'_>, at: usize, name: &str, window_end: usize) -> Option<()> {
    if !tokens.is(at + 1, "(") {
        return None;
    }
    let close = tokens.matching_close(at + 1)?;
    let negated_ok = tokens.is(at + 2, "!")
        && tokens.is_ident(at + 3)
        && tokens.text(at + 3) == name
        && tokens.is(at + 4, ".")
        && tokens.is(at + 5, "ok");
    if !negated_ok || tokens.list[at + 5].end > window_end {
        return None;
    }
    let plain = close == at + 6;
    if !plain && !tokens.is(at + 6, "||") {
        return None;
    }
    let body = close + 1;
    if tokens.is(body, "throw") {
        return plain.then_some(());
    }
    if !tokens.is(body, "{") {
        return None;
    }
    let end = tokens.matching_close(body)?;
    let mut depth = 0usize;
    for index in body + 1..end {
        if tokens.opens(index) {
            depth += 1;
        } else if tokens.closes(index) {
            depth = depth.checked_sub(1)?;
        } else if depth == 0
            && (tokens.is(index, "return") || tokens.is(index, "throw"))
            && (tokens.is(index - 1, "{") || tokens.is(index - 1, ";") || tokens.is(index - 1, "}"))
        {
            return Some(());
        }
    }
    None
}

fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$') || byte >= 0x80
}

/// Split JS/TS source into tokens, dropping comments and keeping strings,
/// templates (with their `${}` bodies) and regular expressions opaque. `None`
/// only for a block comment or template that never closes.
fn lex(src: &str) -> Option<Vec<Token>> {
    let bytes = src.as_bytes();
    let mut out: Vec<Token> = Vec::new();
    let mut index = 0;
    let mut newline = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'\n' {
            newline = true;
            index += 1;
            continue;
        }
        if byte.is_ascii_whitespace() {
            index += 1;
            continue;
        }
        if byte == b'/' && bytes.get(index + 1) == Some(&b'/') {
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            continue;
        }
        if byte == b'/' && bytes.get(index + 1) == Some(&b'*') {
            let end = skip_block_comment(bytes, index + 2)?;
            newline |= bytes[index..end].contains(&b'\n');
            index = end;
            continue;
        }
        let start = index;
        let kind = if byte == b'"' || byte == b'\'' {
            // A lone quote is JSX text, not a string.
            match skip_string(bytes, index) {
                Some(end) => {
                    index = end;
                    Kind::Opaque
                }
                None => {
                    index += 1;
                    Kind::Punct
                }
            }
        } else if byte == b'`' {
            index = skip_template(bytes, index + 1)?;
            Kind::Opaque
        } else if byte == b'/' && regex_allowed(src, &out) {
            match skip_regex(bytes, index) {
                Some(end) => {
                    index = end;
                    Kind::Opaque
                }
                None => {
                    index += 1;
                    Kind::Punct
                }
            }
        } else if is_ident_byte(byte) {
            while index < bytes.len() && is_ident_byte(bytes[index]) {
                index += 1;
            }
            Kind::Ident
        } else {
            index += punct_len(&bytes[index..]);
            Kind::Punct
        };
        out.push(Token {
            kind,
            start,
            end: index,
            newline_before: newline,
        });
        newline = false;
    }
    Some(out)
}

fn punct_len(rest: &[u8]) -> usize {
    [
        "===", "!==", "...", "==", "!=", "<=", ">=", "=>", "&&", "||", "??", "?.",
    ]
    .iter()
    .find(|operator| rest.starts_with(operator.as_bytes()))
    .map_or(1, |operator| operator.len())
}

fn skip_block_comment(bytes: &[u8], from: usize) -> Option<usize> {
    bytes[from..]
        .windows(2)
        .position(|pair| pair == b"*/")
        .map(|offset| from + offset + 2)
}

/// End of the string that opens at `open`, or `None` when it does not close on
/// the same line.
fn skip_string(bytes: &[u8], open: usize) -> Option<usize> {
    let quote = bytes[open];
    let mut index = open + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'\n' => return None,
            byte if byte == quote => return Some(index + 1),
            _ => index += 1,
        }
    }
    None
}

/// End of the template whose body starts at `from`.
fn skip_template(bytes: &[u8], from: usize) -> Option<usize> {
    let mut index = from;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'`' => return Some(index + 1),
            b'$' if bytes.get(index + 1) == Some(&b'{') => {
                index = skip_braces(bytes, index + 2)?;
            }
            _ => index += 1,
        }
    }
    None
}

/// End of the `${ ... }` expression whose body starts at `from`.
fn skip_braces(bytes: &[u8], from: usize) -> Option<usize> {
    let mut depth = 1usize;
    let mut index = from;
    while index < bytes.len() {
        match bytes[index] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index + 1);
                }
            }
            b'"' | b'\'' => {
                index = skip_string(bytes, index).unwrap_or(index + 1);
                continue;
            }
            b'`' => {
                index = skip_template(bytes, index + 1)?;
                continue;
            }
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
                continue;
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                index = skip_block_comment(bytes, index + 2)?;
                continue;
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn regex_allowed(src: &str, before: &[Token]) -> bool {
    let Some(previous) = before.last() else {
        return true;
    };
    let text = src.get(previous.start..previous.end).unwrap_or("");
    match previous.kind {
        Kind::Opaque => false,
        Kind::Ident => REGEX_AFTER.contains(&text),
        // `</div>` is a closing tag, not a regular expression.
        Kind::Punct => !matches!(text, ")" | "]" | "}" | "<"),
    }
}

fn skip_regex(bytes: &[u8], open: usize) -> Option<usize> {
    let mut index = open + 1;
    let mut in_class = false;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'\n' => return None,
            b'[' => {
                in_class = true;
                index += 1;
            }
            b']' => {
                in_class = false;
                index += 1;
            }
            b'/' if !in_class => {
                index += 1;
                while index < bytes.len() && bytes[index].is_ascii_alphabetic() {
                    index += 1;
                }
                return Some(index);
            }
            _ => index += 1,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{ResponseCheck, check};

    const POST: &str = r#"fetch("/api/notes", { method: "POST""#;
    const PUT: &str = r#"fetch(`/api/notes/${id}`, { method: "PUT""#;
    const LIVE: &str = "const res = isNew\n  ? await fetch(\"/api/notes\", { method: \"POST\", body: JSON.stringify({ title, content }) })\n  : await fetch(`/api/notes/${id}`, { method: \"PUT\", body: JSON.stringify({ title, content }) });\n";
    const GUARD: &str = "if (!res.ok || !body) {\n  setError(\"x\");\n  return;\n}\n";

    /// Verdicts for the POST and PUT fetch of `source`, with the full window.
    fn verdicts(source: &str) -> [ResponseCheck; 2] {
        [POST, PUT].map(|needle| {
            source
                .find(needle)
                .map_or(ResponseCheck::NotApplicable, |start| {
                    check(source, start, start + needle.len(), source.len())
                })
        })
    }

    fn both(source: &str, expected: ResponseCheck) {
        assert_eq!(verdicts(source), [expected, expected], "{source}");
    }

    #[test]
    fn confirms_both_branches_of_the_live_ternary() {
        let body = "const body = (await res.json().catch(() => null)) as\n  | { note?: string }\n  | null;\n";
        both(&format!("{LIVE}{body}{GUARD}"), ResponseCheck::Confirmed);
        both(&format!("{LIVE}{GUARD}"), ResponseCheck::Confirmed);
        both(
            &format!("{LIVE}if (!res.ok) throw new Error(\"x\");\n"),
            ResponseCheck::Confirmed,
        );
        both(
            &format!("try {{\n{LIVE}{GUARD}}} catch (e) {{}}\n"),
            ResponseCheck::Confirmed,
        );
        both(
            &format!("{}{GUARD}", LIVE.replace(";\n", "\n")),
            ResponseCheck::Confirmed,
        );
    }

    #[test]
    fn leaves_other_shapes_not_applicable() {
        both(
            "const res = await fetch(\"/api/notes\", { method: \"POST\" });\n",
            ResponseCheck::NotApplicable,
        );
        // The PUT branch is not preceded by `? await` or `: await` here.
        let source = "return isNew ? fetch(\"/api/notes\", { method: \"POST\" }) : fetch(`/api/notes/${id}`, { method: \"PUT\" });";
        both(source, ResponseCheck::NotApplicable);
    }

    #[test]
    fn rejects_guards_that_do_not_prove_the_check() {
        for (name, guard) in [
            ("no guard", ""),
            ("and condition", "if (!res.ok && !body) { return; }\n"),
            ("bare ok", "res.ok;\n"),
            ("other name", "if (!other.ok) { return; }\n"),
            ("ok-like name", "if (!res.okay) { return; }\n"),
            ("optional chain", "if (!res?.ok) { return; }\n"),
            ("or before ok", "if (x || !res.ok) { return; }\n"),
            ("no exit", "if (!res.ok) { setError(\"x\"); }\n"),
            ("conditional exit", "if (!res.ok) { if (x) return; }\n"),
            ("bare return", "if (!res.ok) return;\n"),
            ("or then throw", "if (!res.ok || x) throw new Error();\n"),
            ("comment", "// if (!res.ok) { return; }\n"),
            ("block comment", "/* if (!res.ok) { return; } */\n"),
            ("string", "const s = \"if (!res.ok) { return; }\";\n"),
            (
                "template",
                "const s = `${\"x\"} if (!res.ok) { return; }`;\n",
            ),
            (
                "success first",
                "setSaved(true);\nif (!res.ok) { return; }\n",
            ),
            ("rebind", "res = await other();\nif (!res.ok) { return; }\n"),
            (
                "shadow decl",
                "const { res } = other;\nif (!res.ok) { return; }\n",
            ),
            ("unrelated decl", "const x = 1;\nif (!res.ok) { return; }\n"),
            (
                "other scope",
                "{\n  const res = other;\n  if (!res.ok) { return; }\n}\n",
            ),
            ("closure", "const f = () => { if (!res.ok) { return; } };\n"),
            ("block ends", "}\nif (!res.ok) { return; }\n"),
        ] {
            assert_eq!(
                verdicts(&format!("{LIVE}{guard}")),
                [ResponseCheck::Unconfirmed; 2],
                "{name}"
            );
        }
    }

    #[test]
    fn rejects_ternaries_that_are_not_simple() {
        let guard = "if (!res.ok) { return; }\n";
        let post = "fetch(\"/api/notes\", { method: \"POST\" })";
        let put = "fetch(`/api/notes/${id}`, { method: \"PUT\" })";
        for (name, decl) in [
            (
                "branch without await",
                format!("const res = c ? await {post} : {put};\n"),
            ),
            (
                "plain branch",
                format!("const res = c ? await {post} : null;\n"),
            ),
            (
                "nested",
                format!("const res = a ? b ? await {post} : await {put} : await {put};\n"),
            ),
            (
                "nested condition",
                format!("const res = (a ? b : c) ? await {post} : await {put};\n"),
            ),
            (
                "let",
                format!("let res = c ? await {post} : await {put};\n"),
            ),
            (
                "two statements",
                format!("const res = c ? await {post} : await {put}, x = 1;\n"),
            ),
            (
                "trailing chain",
                format!("const res = c ? await {post} : await {put}.ok;\n"),
            ),
            (
                "newline after equals",
                format!("const res =\n  c ? await {post} : await {put};\n"),
            ),
            (
                "await condition",
                format!("const res = (await c()) ? await {post} : await {put};\n"),
            ),
        ] {
            let source = format!("{decl}{guard}");
            let [post_verdict, put_verdict] = verdicts(&source);
            assert_ne!(post_verdict, ResponseCheck::Confirmed, "{name}");
            assert_ne!(put_verdict, ResponseCheck::Confirmed, "{name}");
        }
        // The two arms of `await (c ? fetch : fetch)` are not direct awaits.
        both(
            &format!("const res = await (c ? {post} : {put});\n{guard}"),
            ResponseCheck::NotApplicable,
        );
    }

    #[test]
    fn keeps_quotes_templates_and_brackets_in_their_place() {
        let tricky = "const res = isNew\n  ? await fetch(\"/api/notes\", { method: \"POST\", body: JSON.stringify({ t: \"a)b\" }) })\n  : await fetch(`/api/notes/${id}`, { method: \"PUT\", body: JSON.stringify({ t: `${\"}\"})` }) });\n";
        both(&format!("{tricky}{GUARD}"), ResponseCheck::Confirmed);
        both(
            &format!("// 日本語 😀 ─ it's\n{tricky}{GUARD}"),
            ResponseCheck::Confirmed,
        );
        both(
            &format!("{tricky}/* unclosed {GUARD}"),
            ResponseCheck::NotApplicable,
        );
    }

    #[test]
    fn requires_the_guard_inside_the_window() {
        let source = format!("{LIVE}{GUARD}");
        let start = source.find(POST).unwrap();
        let end = start + POST.len();
        let guard_end = source.find("res.ok").unwrap() + "res.ok".len();
        assert_eq!(
            check(&source, start, end, guard_end),
            ResponseCheck::Confirmed
        );
        assert_eq!(
            check(&source, start, end, guard_end - 1),
            ResponseCheck::Unconfirmed
        );
    }
}
