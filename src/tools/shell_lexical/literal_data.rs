//! Literal-data bodies of plain `<<` heredocs read by a lone `cat` (Issue #924).
//!
//! The comment and heredoc elision drops a data-reader body only behind a
//! quoted delimiter, because the shell expands nothing there. A plain `<<`
//! body is expanded, so the elision keeps it and re-reads it as shell text, and
//! a JSON fragment such as `"/b"` or a quote in prose is then read as a path or
//! an unterminated quote.
//!
//! Bash changes an unquoted-delimiter body in three ways only: `$` starts a
//! parameter, arithmetic, or command expansion, a backtick starts a command
//! substitution, and a backslash quotes `$`, a backtick, `\`, or a newline.
//! A body with none of those three bytes therefore reaches `cat` as exactly the
//! bytes the quoted-delimiter spelling passes, and dropping it is the same
//! decision the quoted path already makes. [`unquoted_cat_body_is_literal`]
//! proves that much and nothing more; anything it cannot prove falls back to
//! the kept-body reading.
//!
//! The caller has already found the terminator with the shared scanner and
//! checked that the command before the body is a single data-reader command, so
//! this module reads neither quotes nor terminators itself.

use super::{Arg, Heredoc};

/// Whether a heredoc body may be dropped as literal `cat` input although its
/// delimiter is unquoted.
///
/// `argv` is the quote-removed argv of the single data-reader command that owns
/// the body, `pending` the heredocs that command opened, `body` the raw bytes
/// before the terminator line, and `terminated` whether that terminator line
/// exists. Every condition must hold:
///
/// - exactly one heredoc, a plain `<<` (not `<<-`) with an unquoted delimiter
///   that is an ASCII identifier;
/// - the terminator line exists;
/// - the command is a literal `cat` with no argument;
/// - the body is UTF-8 with no `$`, backtick, or backslash.
pub(super) fn unquoted_cat_body_is_literal(
    argv: &[Arg],
    pending: &[Heredoc],
    body: &[u8],
    terminated: bool,
) -> bool {
    let [heredoc] = pending else {
        return false;
    };
    if heredoc.quoted || heredoc.dash || !terminated {
        return false;
    }
    if !is_ascii_identifier(&heredoc.delimiter) {
        return false;
    }
    let [name] = argv else {
        return false;
    };
    if name.quoted || name.bytes != b"cat" {
        return false;
    }
    body_is_inert(body)
}

/// `[A-Za-z_][A-Za-z0-9_]*`.
fn is_ascii_identifier(word: &[u8]) -> bool {
    match word.split_first() {
        Some((first, rest)) => {
            (first.is_ascii_alphabetic() || *first == b'_')
                && rest
                    .iter()
                    .all(|ch| ch.is_ascii_alphanumeric() || *ch == b'_')
        }
        None => false,
    }
}

/// Whether the shell leaves an unquoted-delimiter body byte-identical: UTF-8
/// with no byte that starts an expansion or a backslash quote.
fn body_is_inert(body: &[u8]) -> bool {
    std::str::from_utf8(body).is_ok() && !body.iter().any(|ch| matches!(ch, b'$' | b'`' | b'\\'))
}

#[cfg(test)]
mod tests;
