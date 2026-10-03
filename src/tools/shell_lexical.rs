//! Comment and heredoc elision for the lexical Bash guards (Issue #576).
//!
//! The lexical analyses in this crate do not understand two pieces of shell
//! syntax: a `#` at the start of a word runs to the end of the line, and a
//! heredoc body runs from the next unquoted newline to its delimiter. A `'` or
//! `"` inside either is read as the start of a quote, so every command after it
//! is skipped, the lexer returns `None`, the write-target set becomes empty,
//! and the command is allowed. The same blind spot makes the credential scan and
//! the second-stage path candidates miss (or falsely flag) the text.
//!
//! [`strip_comments_and_heredocs`] performs the elision once, so the four
//! lexical analyses (the write inspector, the working-directory candidates, the
//! second-stage path candidates, and the credential scan) read the elided text
//! at their entry points instead of each rewriting the quote state machine.
//!
//! Every skip is chosen so the elided text is never longer than what the shell
//! runs. A heredoc body is dropped only when the command the heredoc is attached
//! to reads its stdin purely as data (see [`DATA_READER_PROGRAMS`]); every other
//! body is kept verbatim, so a shell, an interpreter, or a function that executes
//! it is still inspected. Text that cannot be read statically is reported as
//! `None` rather than guessed at, and the callers fail closed (Merge-review
//! blockers B1-B3, H-02 §1).

/// Operation recorded for a command whose text the lexical guard cannot read
/// (an unterminated quote, an ambiguous arithmetic shift, or a heredoc whose
/// delimiter or body cannot be read). The caller fails closed on it.
pub(crate) const UNREADABLE_OPERATION: &str = "unreadable shell text";

/// Programs that read a heredoc body only as data, never as commands.
///
/// Dropping the body of a heredoc attached to one of these is the "shorter than
/// the shell" direction: the program cannot execute its stdin text. The list is
/// deliberately short and conservative (H-02 §4 B2). A command whose name word
/// carries a quote, `\`, `$`, or backtick is never treated as one of these,
/// because its name cannot be read literally. Every other command keeps its
/// body, so `sh`, `bash`, `python3`, `node`, `xargs`, `{ ...; }`, `exec N<<`,
/// and a function call are all inspected.
const DATA_READER_PROGRAMS: &[&str] = &[
    "cat",
    "tee",
    "git",
    "gh",
    "wc",
    "head",
    "tail",
    "sort",
    "uniq",
    "cut",
    "tr",
    "comm",
    "cmp",
    "diff",
    "fold",
    "nl",
    "rev",
    "tac",
    "base64",
    "jq",
    "sha256sum",
    "md5sum",
    "shasum",
    "openssl",
];

/// A heredoc whose delimiter was seen but whose body has not been consumed yet.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Heredoc {
    delimiter: Vec<u8>,
    quoted: bool,
    dash: bool,
}

/// How a heredoc body is treated once its extent is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BodyMode {
    /// Drop a data reader's body; keep every other body verbatim. Used by the
    /// write inspector, the working-directory candidates, and the second-stage
    /// path candidates.
    DropDataBodies,
    /// Keep every body verbatim, only eliding comments. Used by the credential
    /// scan, which must still see a `.env` named inside a heredoc body (H-02 §1
    /// B2補, §2).
    KeepAllBodies,
}

/// Returns the command text with comments and data-reader heredoc bodies
/// removed, or `None` when the text cannot be read statically.
///
/// The result is byte-identical to the input when the input has no comment,
/// heredoc, or line continuation, so callers can detect "the shell rewrites this
/// text" by an equality check against the original.
pub(crate) fn strip_comments_and_heredocs(command: &str) -> Option<String> {
    scan(command, BodyMode::DropDataBodies)
}

/// Returns the command text with comments removed and every heredoc body kept
/// verbatim, or `None` when the text cannot be read statically.
pub(crate) fn strip_comments_keeping_heredocs(command: &str) -> Option<String> {
    scan(command, BodyMode::KeepAllBodies)
}

/// Whether the elided text is byte-identical to the input, i.e. the command has
/// no comment, heredoc, or line continuation that the shell rewrites.
pub(crate) fn text_is_unchanged(command: &str) -> bool {
    strip_comments_and_heredocs(command).as_deref() == Some(command)
}

fn scan(command: &str, mode: BodyMode) -> Option<String> {
    // An arithmetic shift and a heredoc both start with `<<`. When the command
    // also contains `((`, the two cannot be told apart portably, so fail closed.
    if command.contains("((") && command.contains("<<") {
        return None;
    }
    let bytes = command.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0usize;
    let mut single = false;
    let mut double = false;
    let mut backtick = false;
    let mut at_word_start = true;
    let mut line_start = 0usize;
    let mut pending: Vec<Heredoc> = Vec::new();
    while index < bytes.len() {
        let ch = bytes[index];
        if single {
            out.push(ch);
            if ch == b'\'' {
                single = false;
                at_word_start = false;
            }
            index += 1;
            continue;
        }
        if double {
            if ch == b'\\' {
                if index + 1 < bytes.len() && bytes[index + 1] == b'\n' {
                    // A line continuation carries the word-start state across
                    // it (B5).
                    index += 2;
                    continue;
                }
                out.push(ch);
                if index + 1 < bytes.len() {
                    out.push(bytes[index + 1]);
                    index += 2;
                } else {
                    index += 1;
                }
                at_word_start = false;
                continue;
            }
            out.push(ch);
            if ch == b'"' {
                double = false;
                at_word_start = false;
            }
            index += 1;
            continue;
        }
        if backtick {
            out.push(ch);
            if ch == b'\\' {
                if index + 1 < bytes.len() {
                    out.push(bytes[index + 1]);
                    index += 2;
                } else {
                    index += 1;
                }
                continue;
            }
            if ch == b'`' {
                backtick = false;
                at_word_start = false;
            }
            index += 1;
            continue;
        }
        match ch {
            b'\'' => {
                out.push(ch);
                single = true;
                at_word_start = false;
                index += 1;
            }
            b'"' => {
                out.push(ch);
                double = true;
                at_word_start = false;
                index += 1;
            }
            b'`' => {
                out.push(ch);
                backtick = true;
                at_word_start = false;
                index += 1;
            }
            b'\\' => {
                if index + 1 < bytes.len() && bytes[index + 1] == b'\n' {
                    // A line continuation carries the word-start state across
                    // it, so `echo \<newline># x` still sees a comment (B5).
                    index += 2;
                } else if index + 1 < bytes.len() {
                    out.push(ch);
                    out.push(bytes[index + 1]);
                    index += 2;
                    at_word_start = false;
                } else {
                    out.push(ch);
                    index += 1;
                    at_word_start = false;
                }
            }
            b'#' if at_word_start => {
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
            }
            b'<' if index + 2 < bytes.len()
                && bytes[index + 1] == b'<'
                && bytes[index + 2] == b'<' =>
            {
                // `<<<` is a here-string, not a heredoc: copy it and let the
                // ordinary operand scan follow without queueing a body.
                out.extend_from_slice(&bytes[index..index + 3]);
                at_word_start = true;
                index += 3;
            }
            b'<' if index + 2 < bytes.len()
                && bytes[index + 1] == b'<'
                && bytes[index + 2] != b'<' =>
            {
                match parse_heredoc(bytes, index + 2) {
                    Parsed::Heredoc(heredoc, next) => {
                        out.extend_from_slice(&bytes[index..next]);
                        pending.push(heredoc);
                        index = next;
                        at_word_start = false;
                    }
                    Parsed::NotHeredoc => {
                        out.push(ch);
                        at_word_start = true;
                        index += 1;
                    }
                    Parsed::Unreadable => return None,
                }
            }
            b'\n' => {
                out.push(b'\n');
                index += 1;
                if !pending.is_empty() {
                    let line = &bytes[line_start..index - 1];
                    let (next, kept) = resolve_heredocs(bytes, index, &pending, line, mode)?;
                    out.extend_from_slice(&kept);
                    index = next;
                    pending.clear();
                }
                line_start = index;
                at_word_start = true;
            }
            ch if is_word_boundary(ch) => {
                out.push(ch);
                at_word_start = true;
                index += 1;
            }
            b' ' | b'\t' => {
                // Bash's only blank characters are the space and the tab. A
                // carriage return or form feed stays inside the word (B4).
                out.push(ch);
                at_word_start = true;
                index += 1;
            }
            _ => {
                out.push(ch);
                at_word_start = false;
                index += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

fn is_word_boundary(ch: u8) -> bool {
    matches!(ch, b';' | b'&' | b'|' | b'(' | b')' | b'<' | b'>')
}

/// The result of reading a heredoc delimiter word.
enum Parsed {
    /// A delimiter word and the index just past it.
    Heredoc(Heredoc, usize),
    /// No delimiter word follows, so the `<<` is ordinary shell text.
    NotHeredoc,
    /// The delimiter cannot be read the way the shell reads it, so the whole
    /// command is unreadable (B1).
    Unreadable,
}

/// Reads a heredoc delimiter starting at `start` (just past `<<`).
fn parse_heredoc(bytes: &[u8], start: usize) -> Parsed {
    let mut index = start;
    let dash = index < bytes.len() && bytes[index] == b'-';
    if dash {
        index += 1;
    }
    while index < bytes.len() && (bytes[index] == b' ' || bytes[index] == b'\t') {
        index += 1;
    }
    if index >= bytes.len() {
        return Parsed::NotHeredoc;
    }
    let mut delimiter: Vec<u8> = Vec::new();
    let mut quoted = false;
    while index < bytes.len() {
        match bytes[index] {
            b'\'' => {
                quoted = true;
                index += 1;
                while index < bytes.len() && bytes[index] != b'\'' {
                    // A line continuation inside the delimiter changes where the
                    // word ends, so the scan cannot trust it (B1).
                    if bytes[index] == b'\\' && index + 1 < bytes.len() && bytes[index + 1] == b'\n'
                    {
                        return Parsed::Unreadable;
                    }
                    delimiter.push(bytes[index]);
                    index += 1;
                }
                if index < bytes.len() {
                    index += 1;
                }
            }
            b'"' => {
                quoted = true;
                index += 1;
                while index < bytes.len() && bytes[index] != b'"' {
                    if bytes[index] == b'\\' {
                        let Some(&next) = bytes.get(index + 1) else {
                            return Parsed::Unreadable;
                        };
                        // The shell removes a backslash only before `$`, `` ` ``,
                        // `"`, `\`, or a newline. Anything else keeps it, so the
                        // delimiter differs from a quote-stripped read (B1).
                        if matches!(next, b'$' | b'`' | b'"' | b'\\') {
                            delimiter.push(next);
                            index += 2;
                            continue;
                        }
                        return Parsed::Unreadable;
                    }
                    delimiter.push(bytes[index]);
                    index += 1;
                }
                if index < bytes.len() {
                    index += 1;
                }
            }
            b'\\' => {
                let Some(&next) = bytes.get(index + 1) else {
                    return Parsed::Unreadable;
                };
                if next == b'\n' {
                    return Parsed::Unreadable;
                }
                quoted = true;
                delimiter.push(next);
                index += 2;
            }
            b' ' | b'\t' | b'\n' => break,
            ch if is_word_boundary(ch) => break,
            ch => {
                delimiter.push(ch);
                index += 1;
            }
        }
    }
    if delimiter.is_empty() {
        return Parsed::NotHeredoc;
    }
    Parsed::Heredoc(
        Heredoc {
            delimiter,
            quoted,
            dash,
        },
        index,
    )
}

/// Locates every pending heredoc body in order from `body_start` (the character
/// after the newline). Returns the index just past the last terminator and the
/// bytes to keep verbatim, or `None` when a body cannot be read (B1).
fn resolve_heredocs(
    bytes: &[u8],
    body_start: usize,
    pending: &[Heredoc],
    line: &[u8],
    mode: BodyMode,
) -> Option<(usize, Vec<u8>)> {
    let data_reader = command_is_data_reader(line);
    let mut position = body_start;
    let mut kept: Vec<u8> = Vec::new();
    for heredoc in pending {
        let (body_end, after) = locate_body(bytes, position, heredoc)?;
        let body = &bytes[position..body_end];
        // An unquoted body is expanded by the outer shell, which runs a command
        // substitution in it. The scan must not re-read it as commands, so it is
        // refused (B3).
        if mode == BodyMode::DropDataBodies && !heredoc.quoted && body_contains_execution(body) {
            return None;
        }
        let drop = mode == BodyMode::DropDataBodies && data_reader;
        if !drop {
            // A kept body must be readable as commands; an unbalanced quote means
            // it cannot be read (B2).
            if mode == BodyMode::DropDataBodies && !body_is_readable(body) {
                return None;
            }
            kept.extend_from_slice(body);
        }
        position = after;
        if position >= bytes.len() {
            break;
        }
    }
    Some((position, kept))
}

/// Returns the start of the terminator line (the body end) and the index just
/// past it. When no terminator line matches, the rest of the input is the body.
/// `None` when an unquoted body line ends with `\`, which the shell joins to the
/// next line (B1).
fn locate_body(bytes: &[u8], body_start: usize, heredoc: &Heredoc) -> Option<(usize, usize)> {
    let mut cursor = body_start;
    while cursor <= bytes.len() {
        let line_end = bytes[cursor..]
            .iter()
            .position(|ch| *ch == b'\n')
            .map(|offset| cursor + offset)
            .unwrap_or(bytes.len());
        let raw_line = &bytes[cursor..line_end];
        let line = raw_line.strip_suffix(b"\r").unwrap_or(raw_line);
        let line = if heredoc.dash {
            strip_leading_tabs(line)
        } else {
            line
        };
        if line == heredoc.delimiter.as_slice() {
            let after = if line_end < bytes.len() {
                line_end + 1
            } else {
                bytes.len()
            };
            return Some((cursor, after));
        }
        if !heredoc.quoted && line.ends_with(b"\\") {
            return None;
        }
        if line_end >= bytes.len() {
            break;
        }
        cursor = line_end + 1;
    }
    Some((bytes.len(), bytes.len()))
}

fn strip_leading_tabs(line: &[u8]) -> &[u8] {
    let mut start = 0usize;
    while start < line.len() && line[start] == b'\t' {
        start += 1;
    }
    &line[start..]
}

/// Whether the command word a heredoc is attached to reads its stdin as data.
fn command_is_data_reader(line: &[u8]) -> bool {
    let mut start = 0usize;
    while start < line.len() && (line[start] == b' ' || line[start] == b'\t') {
        start += 1;
    }
    let mut end = start;
    while end < line.len()
        && !matches!(
            line[end],
            b' ' | b'\t' | b'\n' | b';' | b'&' | b'|' | b'(' | b')' | b'<' | b'>'
        )
    {
        end += 1;
    }
    if end == start {
        return false;
    }
    let token = &line[start..end];
    if token
        .iter()
        .any(|ch| matches!(ch, b'\'' | b'"' | b'\\' | b'$' | b'`'))
    {
        return false;
    }
    let name = match token.iter().rposition(|ch| *ch == b'/') {
        Some(position) => &token[position + 1..],
        None => token,
    };
    let Ok(name) = std::str::from_utf8(name) else {
        return false;
    };
    DATA_READER_PROGRAMS.contains(&name)
}

/// Whether an unquoted heredoc body runs a command substitution.
fn body_contains_execution(body: &[u8]) -> bool {
    body.contains(&b'`') || body.windows(2).any(|window| window == b"$(")
}

/// Whether a kept heredoc body can be read as commands (its quotes are
/// balanced).
fn body_is_readable(body: &[u8]) -> bool {
    let mut single = false;
    let mut double = false;
    let mut backtick = false;
    let mut index = 0usize;
    while index < body.len() {
        let ch = body[index];
        if single {
            if ch == b'\'' {
                single = false;
            }
            index += 1;
            continue;
        }
        if double {
            if ch == b'\\' {
                index += 2;
                continue;
            }
            if ch == b'"' {
                double = false;
            }
            index += 1;
            continue;
        }
        if backtick {
            if ch == b'\\' {
                index += 2;
                continue;
            }
            if ch == b'`' {
                backtick = false;
            }
            index += 1;
            continue;
        }
        match ch {
            b'\'' => single = true,
            b'"' => double = true,
            b'`' => backtick = true,
            b'\\' => index += 1,
            _ => {}
        }
        index += 1;
    }
    !(single || double || backtick)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strip(command: &str) -> String {
        strip_comments_and_heredocs(command).expect("command must be readable")
    }

    fn strip_keep(command: &str) -> String {
        strip_comments_keeping_heredocs(command).expect("command must be readable")
    }

    #[test]
    fn shell_lexical_drops_word_start_comments_and_keeps_operand_hashes() {
        assert_eq!(strip("echo x # it's"), "echo x ");
        assert_eq!(strip("echo x # break\ncargo build"), "echo x \ncargo build");
        assert_eq!(strip("echo x;# c\ntee f"), "echo x;\ntee f");
        assert_eq!(strip("a &&# c\nb"), "a &&\nb");
        assert_eq!(strip("a |# c\nb"), "a |\nb");
        assert_eq!(strip("(# c\n)"), "(\n)");
        assert_eq!(strip("# lead\ntee f"), "\ntee f");
        assert_eq!(strip("echo a#b > out.txt"), "echo a#b > out.txt");
        assert_eq!(
            strip("echo $# ${#x} ${x#y} > out.txt"),
            "echo $# ${#x} ${x#y} > out.txt"
        );
        assert_eq!(strip("echo x # cat .env"), "echo x ");
    }

    #[test]
    fn shell_lexical_keeps_comments_inside_quotes_and_backticks() {
        assert_eq!(strip("echo 'a#b'"), "echo 'a#b'");
        assert_eq!(strip("echo \"a#b\""), "echo \"a#b\"");
        assert_eq!(strip("echo `echo x # it's`"), "echo `echo x # it's`");
        assert!(text_is_unchanged("echo `echo x # it's`"));
    }

    #[test]
    fn shell_lexical_b4_carriage_return_and_form_feed_are_not_word_boundaries() {
        // Bash's blank is space and tab only, so `x<CR>#x` is one word and the
        // `#` does not start a comment (B4).
        assert_eq!(strip("echo x\r#x"), "echo x\r#x");
        assert_eq!(strip("echo x\u{c}#x"), "echo x\u{c}#x");
    }

    #[test]
    fn shell_lexical_b5_a_line_continuation_keeps_the_word_start_state() {
        // `\⏎` is removed; `#` after `a` stays mid-word, `#` after the space is
        // still a comment (B5).
        assert_eq!(strip("echo a\\\n#x\ntee f"), "echo a#x\ntee f");
        assert_eq!(strip("echo \\\n#x\ntee f"), "echo \ntee f");
        assert_eq!(strip("echo \"a\\\nb\""), "echo \"ab\"");
        assert!(!text_is_unchanged("echo \"a\\\nb\""));
    }

    #[test]
    fn shell_lexical_drops_data_reader_heredoc_bodies_for_every_delimiter_form() {
        let cases: &[(&str, &str)] = &[
            ("cat <<EOF\nit's\nEOF", "cat <<EOF\n"),
            ("cat <<-EOF\n\tit's\n\tEOF", "cat <<-EOF\n"),
            ("cat <<'EOF'\nit's\nEOF", "cat <<'EOF'\n"),
            ("cat <<\"EOF\"\nit's\nEOF", "cat <<\"EOF\"\n"),
            ("cat <<\\EOF\nit's\nEOF", "cat <<\\EOF\n"),
            ("cat << EOF\nit's\nEOF", "cat << EOF\n"),
            ("cat <<A <<B\none\nA\ntwo\nB", "cat <<A <<B\n"),
            (
                "git commit -F - <<'EOF'\nfix it's\nEOF",
                "git commit -F - <<'EOF'\n",
            ),
        ];
        for (command, expected) in cases {
            assert_eq!(strip(command), *expected, "command: {command}");
        }
    }

    #[test]
    fn shell_lexical_keeps_heredoc_bodies_for_non_data_readers() {
        // The command name is quoted, expanded, an interpreter, a non-reader,
        // or an option/fd form, so the body stays visible to the guards.
        for command in [
            "sh <<EOF\ntee /tmp/f\nEOF",
            "bash -s <<EOF\ntee sub/link/f\nEOF",
            "/bin/zsh <<EOF\ntee /tmp/f\nEOF",
            "ba\"sh\" <<EOF\ntee sub/link/f\nEOF",
            "b\\ash <<EOF\ntee sub/link/f\nEOF",
            "s''h <<EOF\ntee sub/link/f\nEOF",
            "$SHELL <<EOF\ntee sub/link/f\nEOF",
            "mksh <<EOF\ntee sub/link/f\nEOF",
            "fish <<EOF\ntee sub/link/f\nEOF",
            "xargs tee <<EOF\nsub/link/f\nEOF",
            "exec 3<<EOF\ntee sub/link/f\nEOF",
            "} <<EOF\ntee sub/link/f\nEOF",
            "f <<EOF\ntee sub/link/f\nEOF",
            "python3 <<EOF\nopen(\"sub/link/f\",\"w\")\nEOF",
        ] {
            let elided = strip(command);
            assert!(
                elided.contains("tee /tmp/f") || elided.contains("sub/link/f"),
                "body must be kept: {command:?} -> {elided:?}"
            );
        }
    }

    #[test]
    fn shell_lexical_b1_unreadable_heredoc_delimiters() {
        for command in [
            "cat <<\"E\\OF\"\nx\nE\\OF\ntee sub/link/f\nEOF",
            "cat <<\"E\\aOF\"\nx\nE\\aOF\ntee sub/link/f\nEaOF",
            "cat <<- \"E\\OF\"\nx\nE\\OF\ntee sub/link/f\nEOF",
            "cat <<E\\\nOF\nx\nEOF\ntee sub/link/f",
            "cat <<\"E\\\nOF\"\nx\nEOF\ntee sub/link/f",
            "cat <<E'O'\\\nF\nx\nEOF\ntee sub/link/f",
            "cat <<\\\nEOF\nx\nEOF\ntee sub/link/f",
            "cat <<EOF\nx\nEO\\\nF\ntee sub/link/f\nEOF",
        ] {
            assert_eq!(
                strip_comments_and_heredocs(command),
                None,
                "expected unreadable: {command:?}"
            );
        }
    }

    #[test]
    fn shell_lexical_b1_readable_delimiter_forms_stay_parseable() {
        for (command, expected) in [
            ("cat <<'E\\OF'\nx\nE\\OF\ntee f", "cat <<'E\\OF'\ntee f"),
            ("cat <<E\\\\OF\nx\nE\\OF\ntee f", "cat <<E\\\\OF\ntee f"),
            (
                "cat <<\"E\\$OF\"\nx\nE$OF\ntee f",
                "cat <<\"E\\$OF\"\ntee f",
            ),
        ] {
            assert_eq!(strip(command), *expected, "command: {command}");
        }
    }

    #[test]
    fn shell_lexical_b3_unquoted_body_execution_is_unreadable() {
        for command in [
            "cat <<EOF\n# $(tee sub/link/f)\nEOF",
            "cat <<EOF\nx # $(tee sub/link/f)\nEOF",
            "sh <<EOF\n# $(tee sub/link/f)\nEOF",
            "cat <<EOF\n# $(cat .env)\nEOF",
            "cat <<EOF\n$(tee sub/link/f)\nEOF",
            "cat <<EOF\n`tee sub/link/f`\nEOF",
        ] {
            assert_eq!(
                strip_comments_and_heredocs(command),
                None,
                "expected unreadable: {command:?}"
            );
        }
        // A quoted delimiter disables expansion, so the body stays dropped.
        assert_eq!(
            strip("cat <<'EOF'\n$(tee sub/link/f)\nEOF"),
            "cat <<'EOF'\n"
        );
    }

    #[test]
    fn shell_lexical_b2_kept_body_must_be_readable() {
        assert_eq!(
            strip_comments_and_heredocs("sh <<'EOF'\necho it's\nEOF"),
            None
        );
        assert_eq!(
            strip_comments_and_heredocs("python3 <<'EOF'\n# it's\nEOF"),
            None
        );
        assert_eq!(
            strip("python3 - <<'EOF'\nprint(\"it's\")\nEOF"),
            "python3 - <<'EOF'\nprint(\"it's\")\n"
        );
    }

    #[test]
    fn shell_lexical_treats_an_unterminated_heredoc_remainder_as_body() {
        // No terminator line: the rest is the body. The shell reports an
        // unterminated here-document and runs nothing, so dropping is the
        // "shorter than the shell" direction for a data reader.
        assert_eq!(strip("cat <<EOF\nit's"), "cat <<EOF\n");
        // A non-reader keeps the body.
        assert_eq!(strip("sh <<EOF\ntee /tmp/f"), "sh <<EOF\ntee /tmp/f");
    }

    #[test]
    fn shell_lexical_treats_a_near_delimiter_line_as_body() {
        assert_eq!(
            strip("cat <<EOF\nEOF \n EOF\nEOF\ntee f"),
            "cat <<EOF\ntee f"
        );
    }

    #[test]
    fn shell_lexical_drops_line_continuations_and_ignores_here_strings() {
        assert_eq!(strip("cat .e\\\nnv"), "cat .env");
        assert!(text_is_unchanged("cat <<<\"it's\""));
        assert_eq!(strip("cat <<<\"it's\"\ntee f"), "cat <<<\"it's\"\ntee f");
    }

    #[test]
    fn shell_lexical_does_not_panic_on_short_heredoc_spellings() {
        assert_eq!(strip("cat <"), "cat <");
        assert_eq!(strip("cat <<x"), "cat <<x");
        assert_eq!(
            strip("cat << \"E\\\\OF\"\ndata\nEOF\ntee sub/link/f"),
            "cat << \"E\\\\OF\"\n"
        );
    }

    #[test]
    fn shell_lexical_fails_closed_on_an_ambiguous_arithmetic_heredoc() {
        assert_eq!(
            strip_comments_and_heredocs("echo $((1<<2))\ntee /tmp/f"),
            None
        );
        assert_eq!(
            strip_comments_and_heredocs("cat <<EOF\nbody\nEOF\n(( ))"),
            None
        );
    }

    #[test]
    fn shell_lexical_returns_the_command_unchanged_when_nothing_is_elided() {
        for command in [
            "cargo test",
            "cd frontend && npm test",
            "cd crates/x && cargo test 2>&1 | tee test.log",
            "printf 'a\\tb\\n' > out.txt",
            "echo \"$'x'\"",
            "echo \"\\q\"",
        ] {
            assert_eq!(strip(command), command, "command: {command}");
            assert!(text_is_unchanged(command), "command: {command}");
        }
        assert!(!text_is_unchanged("cargo test # comment"));
    }

    #[test]
    fn shell_lexical_keeps_heredocs_for_the_credential_scan() {
        // Comments are still dropped, but a body word stays visible.
        assert_eq!(strip_keep("echo x # cat .env"), "echo x ");
        assert_eq!(
            strip_keep("cat <<'EOF'\ncat .env\nEOF"),
            "cat <<'EOF'\ncat .env\n"
        );
        assert_eq!(
            strip_keep("cat <<EOF\n# $(cat .env)\nEOF"),
            "cat <<EOF\n# $(cat .env)\n"
        );
    }
}
