//! Limited `$(...)` / `${...}` recognition inside double quotes (Issue #585).
//!
//! The comment/heredoc elision in [`super::scan`] models single quotes, double
//! quotes, and backticks, but it does not model `$(...)` and `${...}`. Inside a
//! double-quoted string it therefore reads a `"` that belongs to an expansion's
//! own quoting — for example `echo "$(echo "x")"` — as the end of the
//! surrounding double quote. The desynchronized quote state makes a following
//! word-start `#` look like a comment, so the scan drops the rest of the line
//! and a write named after it escapes the workspace (the hole #581's review
//! found; it comes from #576 and is present on the base commit too).
//!
//! Rather than build a general recursive parser (adopted design 2), the guards
//! recognize only the expansions whose interior is known not to change the
//! surrounding quoting, and refuse every other expansion inside double quotes:
//!
//! * `${NAME}` where `NAME` is ASCII `[A-Za-z_][A-Za-z0-9_]*` only.
//! * `$(TEXT)` where `TEXT` is ASCII alphanumerics, `_`, space, tab, `.`, `/`,
//!   `:`, `=`, `,`, `+`, `@`, `%`, and `-` only (empty `TEXT` allowed). The
//!   first byte outside the set, or the first `)` reached, decides.
//! * A backslash-newline in the introducer and in the body is skipped the way
//!   the shell removes a line continuation before it expands the word.
//!
//! Everything else — an inner quote, nesting, an ordinary escape, a newline, a
//! backtick, a `${...}` operator or subscript, `$((...))`, or an unterminated
//! expansion — is *ambiguous* and makes the command fail closed. An escaped `$`
//! never reaches this check, and a lone `$` is not an introducer. The check runs
//! only inside double quotes: an unquoted expansion keeps its existing handling,
//! and single quotes, true comments, and quoted-delimiter heredoc data are never
//! classified.

/// The classification of a `$` seen in a double-quoted context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Expansion {
    /// Not a `$(` / `${` introducer; the caller keeps its ordinary byte access.
    Bare,
    /// A limited simple expansion, ending just past its closing delimiter.
    Simple {
        /// The byte index just past the closing `)` / `}`.
        end: usize,
    },
    /// An expansion whose interior cannot be proven not to change the quoting.
    Ambiguous,
}

/// Classifies the `$` at `start` in a double-quoted context.
///
/// The caller has already handled escapes, so `bytes[start]` is an unescaped
/// `$`. A `$` not followed by `(`/`{` (after any line continuation) is
/// [`Expansion::Bare`].
pub(crate) fn classify(bytes: &[u8], start: usize) -> Expansion {
    let index = skip_line_continuations(bytes, start + 1);
    match bytes.get(index) {
        Some(&b'(') => classify_command_substitution(bytes, index + 1),
        Some(&b'{') => classify_parameter_expansion(bytes, index + 1),
        _ => Expansion::Bare,
    }
}

/// Reads a `$(TEXT)` expansion from just past its `(`.
fn classify_command_substitution(bytes: &[u8], mut index: usize) -> Expansion {
    loop {
        index = skip_line_continuations(bytes, index);
        match bytes.get(index) {
            Some(&b')') => return Expansion::Simple { end: index + 1 },
            Some(&byte) if is_simple_body_byte(byte) => index += 1,
            _ => return Expansion::Ambiguous,
        }
    }
}

/// Reads a `${NAME}` expansion from just past its `{`.
fn classify_parameter_expansion(bytes: &[u8], mut index: usize) -> Expansion {
    index = skip_line_continuations(bytes, index);
    match bytes.get(index) {
        Some(&byte) if is_name_start(byte) => index += 1,
        _ => return Expansion::Ambiguous,
    }
    loop {
        index = skip_line_continuations(bytes, index);
        match bytes.get(index) {
            Some(&byte) if is_name_byte(byte) => index += 1,
            Some(&b'}') => return Expansion::Simple { end: index + 1 },
            _ => return Expansion::Ambiguous,
        }
    }
}

/// Skips the `\` + newline pairs the shell removes as a line continuation.
fn skip_line_continuations(bytes: &[u8], mut index: usize) -> usize {
    while bytes.get(index) == Some(&b'\\') && bytes.get(index + 1) == Some(&b'\n') {
        index += 2;
    }
    index
}

/// The bytes `$(TEXT)` allows before its first `)`.
fn is_simple_body_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'_' | b' ' | b'\t' | b'.' | b'/' | b':' | b'=' | b',' | b'+' | b'@' | b'%' | b'-'
        )
}

fn is_name_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classify_at(text: &str) -> Expansion {
        let bytes = text.as_bytes();
        classify(bytes, 0)
    }

    #[test]
    fn quoted_expansion_simple_command_substitution_is_recognized() {
        for text in [
            "$(x)",
            "$()",
            "$(git rev-parse --show-toplevel)",
            "$(cat path/to/file.txt)",
            "$(printf %s a,b+c@d)",
            "$(x=1,y:2)",
        ] {
            assert!(
                matches!(classify_at(text), Expansion::Simple { end } if end == text.len()),
                "{text:?} must be simple"
            );
        }
    }

    #[test]
    fn quoted_expansion_simple_parameter_is_recognized() {
        for text in ["${x}", "${HOME}", "${_a1}"] {
            assert!(
                matches!(classify_at(text), Expansion::Simple { end } if end == text.len()),
                "{text:?} must be simple"
            );
        }
    }

    #[test]
    fn quoted_expansion_ambiguous_interiors_are_refused() {
        for text in [
            // An inner quote (the hole this issue closes).
            "$(echo \"x\")",
            "$(echo 'x')",
            // A newline, backtick, escape, or nested/non-simple byte.
            "$(echo\nx)",
            "$(echo `x`)",
            "$(a\\b)",
            "$(a(b))",
            // Arithmetic, subscripts, and parameter operators.
            "$((1+2))",
            "${x:-y}",
            "${x#y}",
            "${x%y}",
            "${x/y/z}",
            "${x[0]}",
            "${#x}",
            "${}",
            "${1}",
            // Unterminated.
            "$(echo",
            "${x",
        ] {
            assert_eq!(classify_at(text), Expansion::Ambiguous, "{text:?}");
        }
    }

    #[test]
    fn quoted_expansion_non_introducers_are_left_alone() {
        for text in ["$", "$x", "$'x'", "$1"] {
            assert_eq!(classify_at(text), Expansion::Bare, "{text:?}");
        }
        // Trailing text after the closing delimiter is not part of the expansion.
        assert!(
            matches!(classify_at("$(x)trailing"), Expansion::Simple { end } if end == 4),
            "trailing text"
        );
    }

    #[test]
    fn quoted_expansion_skips_line_continuations_in_introducer_and_body() {
        let introducer = "$\\\n(x)";
        assert!(
            matches!(classify_at(introducer), Expansion::Simple { end } if end == introducer.len()),
            "{introducer:?}"
        );
        let body = "$(ec\\\nho)";
        assert!(
            matches!(classify_at(body), Expansion::Simple { end } if end == body.len()),
            "{body:?}"
        );
        let name = "${\\\nNAME}";
        assert!(
            matches!(classify_at(name), Expansion::Simple { end } if end == name.len()),
            "{name:?}"
        );
        // A continuation alone is not an introducer.
        assert_eq!(classify_at("$\\\nx"), Expansion::Bare);
    }

    #[test]
    fn quoted_expansion_end_points_past_the_delimiter() {
        let text = "$(x) after";
        assert!(matches!(classify_at(text), Expansion::Simple { end } if end == 4));
        let text = "${name} after";
        assert!(matches!(classify_at(text), Expansion::Simple { end } if end == 7));
    }
}
