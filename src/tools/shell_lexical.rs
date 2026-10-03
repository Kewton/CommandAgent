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
//! runs, and elision that could hide an executing body is refused: a heredoc
//! whose body the shell executes (`sh <<EOF`, or an unquoted body containing
//! `$(` or a backtick) is kept as commands. When a `<<` cannot be told from an
//! arithmetic shift (`((` is present too), the whole command is reported
//! unreadable (`None`) rather than guessed at.

/// Operation recorded for a command whose text the lexical guard cannot read
/// (an unterminated quote, or a `<<` that cannot be told from a shift). The
/// caller fails closed on it.
pub(crate) const UNREADABLE_OPERATION: &str = "unreadable shell text";

/// A heredoc whose delimiter was seen but whose body has not been consumed yet.
struct Heredoc {
    delimiter: Vec<u8>,
    quoted: bool,
    dash: bool,
}

/// Returns the command text with comments and heredoc bodies removed, or `None`
/// when the text cannot be read statically.
///
/// The result is byte-identical to the input when the input has no comment,
/// heredoc, or line continuation, so callers can detect "the shell rewrites this
/// text" by an equality check against the original.
pub(crate) fn strip_comments_and_heredocs(command: &str) -> Option<String> {
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
                    index += 2;
                    at_word_start = false;
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
                    index += 2;
                } else if index + 1 < bytes.len() {
                    out.push(ch);
                    out.push(bytes[index + 1]);
                    index += 2;
                } else {
                    out.push(ch);
                    index += 1;
                }
                at_word_start = false;
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
                if let Some((heredoc, next)) = parse_heredoc(bytes, index + 2) {
                    out.extend_from_slice(&bytes[index..next]);
                    pending.push(heredoc);
                    index = next;
                    at_word_start = false;
                } else {
                    out.push(ch);
                    at_word_start = true;
                    index += 1;
                }
            }
            b'\n' => {
                out.push(b'\n');
                index += 1;
                if !pending.is_empty() {
                    let line = &bytes[line_start..index - 1];
                    let (keep, next) = process_heredocs(bytes, index, &pending, line);
                    pending.clear();
                    if !keep {
                        index = next;
                    }
                }
                line_start = index;
                at_word_start = true;
            }
            ch if is_word_boundary(ch) => {
                out.push(ch);
                at_word_start = true;
                index += 1;
            }
            ch if ch.is_ascii_whitespace() => {
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

/// Whether the elided text is byte-identical to the input, i.e. the command has
/// no comment, heredoc, or line continuation that the shell rewrites.
pub(crate) fn text_is_unchanged(command: &str) -> bool {
    strip_comments_and_heredocs(command).as_deref() == Some(command)
}

fn is_word_boundary(ch: u8) -> bool {
    matches!(ch, b';' | b'&' | b'|' | b'(' | b')' | b'<' | b'>')
}

/// Reads a heredoc delimiter starting at `start` (just past `<<`), returning the
/// delimiter and the index just past it. `None` when no delimiter word follows.
fn parse_heredoc(bytes: &[u8], start: usize) -> Option<(Heredoc, usize)> {
    let mut index = start;
    let dash = index < bytes.len() && bytes[index] == b'-';
    if dash {
        index += 1;
    }
    while index < bytes.len() && (bytes[index] == b' ' || bytes[index] == b'\t') {
        index += 1;
    }
    if index >= bytes.len() {
        return None;
    }
    let mut delimiter: Vec<u8> = Vec::new();
    let mut quoted = false;
    while index < bytes.len() {
        match bytes[index] {
            b'\'' => {
                quoted = true;
                index += 1;
                while index < bytes.len() && bytes[index] != b'\'' {
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
                    if bytes[index] == b'\\' && index + 1 < bytes.len() {
                        index += 1;
                    }
                    delimiter.push(bytes[index]);
                    index += 1;
                }
                if index < bytes.len() {
                    index += 1;
                }
            }
            b'\\' => {
                quoted = true;
                index += 1;
                if index < bytes.len() {
                    delimiter.push(bytes[index]);
                    index += 1;
                }
            }
            ch if is_word_boundary(ch) || ch.is_ascii_whitespace() => break,
            ch => {
                delimiter.push(ch);
                index += 1;
            }
        }
    }
    if delimiter.is_empty() {
        return None;
    }
    Some((
        Heredoc {
            delimiter,
            quoted,
            dash,
        },
        index,
    ))
}

/// Locates the body of every pending heredoc in order, starting at `body_start`
/// (the character after the newline). Returns whether any body must be kept as
/// commands, and the index just past the last terminator (used only when no body
/// is kept).
fn process_heredocs(
    bytes: &[u8],
    body_start: usize,
    pending: &[Heredoc],
    line: &[u8],
) -> (bool, usize) {
    let mut keep = line_has_interpreter(line);
    let mut position = body_start;
    for heredoc in pending {
        let (body_end, after) = locate_body(bytes, position, heredoc);
        if !keep && !heredoc.quoted && body_contains_execution(&bytes[position..body_end]) {
            keep = true;
        }
        position = after;
        if position >= bytes.len() {
            break;
        }
    }
    (keep, position)
}

/// Returns the start of the terminator line (the body end) and the index just
/// past it. When no terminator line matches, the rest of the input is the body.
fn locate_body(bytes: &[u8], body_start: usize, heredoc: &Heredoc) -> (usize, usize) {
    let mut cursor = body_start;
    while cursor <= bytes.len() {
        let line_end = bytes[cursor..]
            .iter()
            .position(|ch| *ch == b'\n')
            .map(|offset| cursor + offset)
            .unwrap_or(bytes.len());
        let line = &bytes[cursor..line_end];
        let line = line.strip_suffix(b"\r").unwrap_or(line);
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
            return (cursor, after);
        }
        if line_end >= bytes.len() {
            break;
        }
        cursor = line_end + 1;
    }
    (bytes.len(), bytes.len())
}

fn strip_leading_tabs(line: &[u8]) -> &[u8] {
    let mut start = 0usize;
    while start < line.len() && line[start] == b'\t' {
        start += 1;
    }
    &line[start..]
}

/// Whether the heredoc's command line names a shell that executes the body.
fn line_has_interpreter(line: &[u8]) -> bool {
    let text = String::from_utf8_lossy(line);
    text.split(|ch: char| {
        ch.is_whitespace() || matches!(ch, ';' | '&' | '|' | '(' | ')' | '<' | '>')
    })
    .any(|token| {
        let name = token.rsplit('/').next().unwrap_or(token);
        let name = name.trim_matches(|ch| ch == '\'' || ch == '"');
        matches!(name, "sh" | "bash" | "zsh" | "dash" | "ksh")
    })
}

/// Whether an unquoted heredoc body runs a command substitution.
fn body_contains_execution(body: &[u8]) -> bool {
    body.contains(&b'`') || body.windows(2).any(|window| window == b"$(")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strip(command: &str) -> String {
        strip_comments_and_heredocs(command).expect("command must be readable")
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
    fn shell_lexical_drops_heredoc_bodies_for_every_delimiter_form() {
        let cases: &[(&str, &str)] = &[
            ("cat <<EOF\nit's\nEOF", "cat <<EOF\n"),
            ("cat <<-EOF\n\tit's\n\tEOF", "cat <<-EOF\n"),
            ("cat <<'EOF'\nit's\nEOF", "cat <<'EOF'\n"),
            ("cat <<\"EOF\"\nit's\nEOF", "cat <<\"EOF\"\n"),
            ("cat <<\\EOF\nit's\nEOF", "cat <<\\EOF\n"),
            ("cat << EOF\nit's\nEOF", "cat << EOF\n"),
            ("cat <<A <<B\none\nA\ntwo\nB", "cat <<A <<B\n"),
        ];
        for (command, expected) in cases {
            assert_eq!(strip(command), *expected, "command: {command}");
        }
    }

    #[test]
    fn shell_lexical_treats_an_unterminated_heredoc_remainder_as_body() {
        // No terminator line: the rest is the body. The shell reports an
        // unterminated here-document and runs nothing, so dropping is the
        // "shorter than the shell" direction.
        assert_eq!(strip("cat <<EOF\nit's"), "cat <<EOF\n");
        // A body the shell would execute is still kept.
        assert_eq!(strip("sh <<EOF\ntee /tmp/f"), "sh <<EOF\ntee /tmp/f");
    }

    #[test]
    fn shell_lexical_keeps_executed_heredoc_bodies_as_commands() {
        for command in [
            "sh <<EOF\ntee /tmp/f\nEOF",
            "bash -s <<EOF\ntee /tmp/f\nEOF",
            "/bin/zsh <<EOF\ntee /tmp/f\nEOF",
            "cat <<EOF\n$(tee /tmp/f)\nEOF",
            "cat <<EOF\n`tee /tmp/f`\nEOF",
        ] {
            let elided = strip(command);
            assert!(
                elided.contains("tee /tmp/f"),
                "executed body must be kept: {command:?} -> {elided:?}"
            );
        }
        // A quoted delimiter disables expansion, so the body stays dropped.
        assert_eq!(
            strip("cat <<'EOF'\n$(tee sub/link/f)\nEOF"),
            "cat <<'EOF'\n"
        );
    }

    #[test]
    fn shell_lexical_drops_line_continuations_and_ignores_here_strings() {
        assert_eq!(strip("cat .e\\\nnv"), "cat .env");
        assert!(text_is_unchanged("cat <<<\"it's\""));
        assert_eq!(strip("cat <<<\"it's\"\ntee f"), "cat <<<\"it's\"\ntee f");
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
        ] {
            assert_eq!(strip(command), command, "command: {command}");
            assert!(text_is_unchanged(command), "command: {command}");
        }
        assert!(!text_is_unchanged("cargo test # comment"));
    }
}
