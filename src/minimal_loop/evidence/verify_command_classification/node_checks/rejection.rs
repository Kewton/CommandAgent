//! Why the Node failure-check recognizer refused a script, and the shape it
//! accepts instead. The screens are the ones `has_failure_check` always ran;
//! they now name the first refusal rather than only returning `false`.

use super::super::super::{LiteralScanMode, strip_c_family_comments_and_literals};

/// Strip the shebang and comments. `Err` carries the refusal detail when the
/// source is outside what the recognizer can read: an unterminated literal or
/// comment, or a template. A backtick in a comment of an unterminated source is
/// reported too, since nothing can be stripped from it.
pub(super) fn scan(source: &str) -> Result<String, String> {
    let source = source.strip_prefix("#!").map_or(source, |rest| {
        rest.split_once('\n').map_or("", |(_, body)| body)
    });
    // Keep a literal as one opaque token: an assertion mentioned inside a string
    // is not a binding or a call. Template interpolation/regexp ambiguity is
    // deliberately unsupported in this small evidence recognizer.
    let stripped = strip_c_family_comments_and_literals(source, LiteralScanMode::Keep);
    let template = stripped.as_deref().unwrap_or(source).contains('`');
    match stripped {
        Some(scan) if !template => Ok(scan),
        Some(_) => Err("template_literal".into()),
        None if template => Err("unterminated_quote_or_comment,template_literal".into()),
        None => Err("unterminated_quote_or_comment".into()),
    }
}

/// Catch/finally and process mutation require control-flow/binding analysis
/// outside this bounded recognizer. Do not borrow evidence from other files.
pub(super) fn unsupported_construct(tokens: &[&str]) -> Option<String> {
    if let Some(token) = tokens.iter().find(|token| is_opaque_token(token)) {
        return Some(format!("opaque_token:{token}"));
    }
    if shadows_process_or_require(tokens) {
        return Some("process_or_require_shadowed".into());
    }
    if mutates_process(tokens) {
        return Some("process_use".into());
    }
    tokens
        .windows(2)
        .any(|t| t == ["assert", "["])
        .then(|| "assert_computed_member".into())
}

fn is_opaque_token(token: &str) -> bool {
    matches!(
        token,
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

const ACCEPTED_FORM: &str = "(1) bind assert at top level with `const assert = require(\"node:assert/strict\");` (\"assert\" and \"node:assert\" also work, as does `import assert from \"node:assert/strict\"`); not inside a block or function, not with let/var, destructuring or aliasing, not passed as a value, not reassigned, no `assert.x = ...`; (2) directly after that line write top-level statements ending in `;` of the form `assert(...)` or `assert.<ok|equal|strictEqual|deepEqual|deepStrictEqual|notEqual|notStrictEqual|notDeepEqual|notDeepStrictEqual|throws|doesNotThrow|ifError|match|doesNotMatch|fail>(...)`; asserts inside functions, loops or callbacks do not count, and they must come before any top-level return, throw or process.exit; (3) avoid anywhere in the code: backticks (template literals, even inside strings), regex literals and the `/` operator (use includes, indexOf, startsWith or new RegExp(\"...\")), a quote that leaves a '...' or \"...\" string open at the end of a line, backslashes outside strings, try/catch/finally, eval, Function, constructor, global, globalThis, any use of `process` other than a direct `process.exit(<code>)` call, and `assert[...]`.";

/// One repair line for a refused Node verify command, without a prefix.
pub(in super::super) fn repair_guidance(command: &str, detail: &str) -> String {
    format!(
        "`{command}` ran, but the script it runs is not accepted as assertion evidence (node_smoke_without_assertion; first rejection: {detail}). Keep the verify command and rewrite the script into the accepted shape: {ACCEPTED_FORM}"
    )
}
