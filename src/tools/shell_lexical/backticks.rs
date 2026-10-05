//! Backtick command substitution for the lexical Bash guards (Issue #581).
//!
//! The write-target inspector's lexical analysis did not know the backtick
//! command substitution (`` `...` ``): it kept the backquote as an ordinary
//! character, so the command inside was never inspected and a write (or a read)
//! through it escaped the workspace. The shell expands the substitution before
//! the program runs, and the expanded word cannot be proven to remain in the
//! Gate 1 workspace boundary, so every executed substitution is refused as an
//! unverifiable form — the same honest-failure shape as ANSI-C quoting (#575).
//!
//! [`in_executable_context`] is the one decision the lexer needs while it walks
//! the command: a backtick runs outside quotes and inside a double quote, and is
//! literal inside a single quote. [`opaque_expansion_with_backticks`] refuses a
//! backtick that sits behind a `$(`/`${` expansion, whose inner quoting the scan
//! does not model: a `` '`data`' `` inside `$(...)` is data in the shell, but the
//! form is refused rather than guessed at.
//!
//! The lexer elides comments and heredocs before it calls
//! [`opaque_expansion_with_backticks`], so that function reads the elided text
//! and does not track shell comments itself.

/// Operation recorded for a command that runs a backtick command substitution
/// whose expanded word cannot be verified to remain in the workspace.
pub(crate) const UNVERIFIABLE_OPERATION: &str = "unverifiable backtick command substitution";

/// The quoting context a backtick can sit in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Context {
    /// Outside every quote.
    Unquoted,
    /// Inside a double-quoted string, where command substitution still runs.
    DoubleQuoted,
    /// Inside a single-quoted string, where every character is literal.
    SingleQuoted,
}

/// Whether a backtick in `context` starts a command substitution the shell runs.
///
/// An unquoted or double-quoted backtick runs; a single-quoted backtick is a
/// literal character. Escaped backticks and backticks inside comments never
/// reach this check.
pub(crate) fn in_executable_context(context: Context) -> bool {
    matches!(context, Context::Unquoted | Context::DoubleQuoted)
}

/// Whether the elided `text` has an executed `$(`/`${` expansion with a backtick
/// behind it.
///
/// The expansion's own quoting is not analyzed — a single-quoted backtick inside
/// it is data in the shell — so the form is refused instead of guessed at. An
/// escaped backtick stays literal and is not counted.
pub(crate) fn opaque_expansion_with_backticks(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut index = 0usize;
    let mut single = false;
    let mut double = false;
    let mut opaque = false;
    while index < bytes.len() {
        let ch = bytes[index];
        if single {
            if opaque && ch == b'`' {
                return true;
            }
            if ch == b'\'' {
                single = false;
            }
            index += 1;
            continue;
        }
        if double {
            match ch {
                b'\\' => {
                    index += 2;
                    continue;
                }
                b'"' => {
                    double = false;
                    index += 1;
                    continue;
                }
                b'`' if opaque => return true,
                b'$' if matches!(bytes.get(index + 1).copied(), Some(b'(' | b'{')) => {
                    opaque = true;
                }
                _ => {}
            }
            index += 1;
            continue;
        }
        match ch {
            b'\\' => index += 2,
            b'\'' => {
                single = true;
                index += 1;
            }
            b'"' => {
                double = true;
                index += 1;
            }
            b'`' if opaque => return true,
            b'$' if matches!(bytes.get(index + 1).copied(), Some(b'(' | b'{')) => {
                opaque = true;
                index += 1;
            }
            _ => index += 1,
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backticks_context_is_executable_outside_single_quotes() {
        assert!(in_executable_context(Context::Unquoted));
        assert!(in_executable_context(Context::DoubleQuoted));
        assert!(!in_executable_context(Context::SingleQuoted));
    }

    #[test]
    fn backticks_only_refuses_an_execution_behind_an_opaque_expansion() {
        let cases: &[(&str, bool)] = &[
            // A bare backtick is the lexer's own concern, not this check.
            ("echo `x`", false),
            ("echo '`x`'", false),
            ("echo \"`x`\"", false),
            // A backtick behind `$(`/`${`, including a nested single quote.
            ("echo $(echo `x`)", true),
            ("echo \"$(echo `x`)\"", true),
            ("echo ${x}`y`", true),
            ("echo $(echo '`data`')", true),
            ("echo \"$(echo '`data`')\"", true),
            // An expansion with no backtick behind it is untouched.
            ("echo $(x)", false),
            ("echo ${x}", false),
            ("echo $(x)`y`", true),
            // A bare `$` is not an expansion, in any quoting context.
            ("echo $x `y`", false),
            ("echo \"$x `y`\"", false),
            // A backtick before the expansion, or in a single quote, is not.
            ("echo `x` $(y)", false),
            ("echo '$(x)`y`'", false),
            ("echo '$(x)' `y`", false),
            // An escaped introducer is not an expansion, and an escaped
            // backtick stays literal.
            ("echo \\$(x)`y`", false),
            ("echo $(x) \\`y\\`", false),
            ("cargo test", false),
            ("", false),
        ];
        for (text, expected) in cases {
            assert_eq!(
                opaque_expansion_with_backticks(text),
                *expected,
                "text: {text:?}"
            );
        }
    }
}
