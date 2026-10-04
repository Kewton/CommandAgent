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
//! A heredoc body is dropped only when the heredoc's whole line — the same line
//! [`scan`] already produced, with comments and line continuations removed — is a
//! single command whose quote-removed argv exactly matches one of the data-reader
//! forms (see [`data_reader_argv`]), and the command defines no function or alias.
//! Every other body is kept, read both with and without comment elision, so a
//! shell, an interpreter, a function, a quote, or a pipeline that executes it is
//! still inspected. Text that cannot be read statically is reported as `None`
//! rather than guessed at, and the callers fail closed (H-02 §1, H-03 §5, H-04 §4).

/// Operation recorded for a command whose text the lexical guard cannot read
/// (an unterminated quote, an ambiguous arithmetic shift, or a heredoc whose
/// delimiter or body cannot be read). The caller fails closed on it.
pub(crate) const UNREADABLE_OPERATION: &str = "unreadable shell text";

/// A heredoc whose delimiter was seen but whose body has not been consumed yet.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Heredoc {
    delimiter: Vec<u8>,
    quoted: bool,
    dash: bool,
}

/// The elided text and the raw bodies of the heredocs it dropped or kept.
struct ScanOutput {
    text: String,
    bodies: Vec<String>,
}

/// One argument of a heredoc line: its quote-removed bytes, and whether any
/// quote or backslash produced it (so `c"at"` is not the literal `cat`).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Arg {
    bytes: Vec<u8>,
    quoted: bool,
}

/// Returns the command text with comments and data-reader heredoc bodies
/// removed, or `None` when the text cannot be read statically.
///
/// The result is byte-identical to the input when the input has no comment,
/// heredoc, or line continuation, so callers can detect "the shell rewrites this
/// text" by an equality check against the original.
pub(crate) fn strip_comments_and_heredocs(command: &str) -> Option<String> {
    scan(command, command).map(|output| output.text)
}

/// Returns the raw body of every heredoc in the command (dropped or kept,
/// including nested bodies), or `None` when the command cannot be read
/// statically. Used by the credential scan, which inspects each body's own words
/// (H-03 §5 N5, H-04 §4 C4).
pub(crate) fn heredoc_bodies(command: &str) -> Option<Vec<String>> {
    scan(command, command).map(|output| output.bodies)
}

/// Whether the elided text is byte-identical to the input, i.e. the command has
/// no comment, heredoc, or line continuation that the shell rewrites.
pub(crate) fn text_is_unchanged(command: &str) -> bool {
    strip_comments_and_heredocs(command).as_deref() == Some(command)
}

fn scan(command: &str, root_command: &str) -> Option<ScanOutput> {
    // An arithmetic shift and a heredoc both start with `<<`. When the command
    // also contains `((`, the two cannot be told apart portably, so fail closed.
    if command.contains("((") && command.contains("<<") {
        return None;
    }
    let bytes = command.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut bodies: Vec<String> = Vec::new();
    let mut index = 0usize;
    let mut single = false;
    let mut double = false;
    let mut backtick = false;
    let mut at_word_start = true;
    let mut out_line_start = 0usize;
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
                // The line the scan produced, with comments and continuations
                // already removed: the same text the guards will read.
                let elided_line = out[out_line_start..].to_vec();
                out.push(b'\n');
                index += 1;
                if !pending.is_empty() {
                    let (next, kept) = resolve_heredocs(
                        bytes,
                        index,
                        &pending,
                        &elided_line,
                        root_command,
                        &mut bodies,
                    )?;
                    out.extend_from_slice(&kept);
                    index = next;
                    pending.clear();
                }
                out_line_start = out.len();
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
    let text = String::from_utf8(out).ok()?;
    Some(ScanOutput { text, bodies })
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
    /// command is unreadable (B1, N3).
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
    // A carriage return in the delimiter is a character the shell keeps, so the
    // terminator line would differ; refuse rather than guess (N3).
    if delimiter.contains(&b'\r') {
        return Parsed::Unreadable;
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
/// bytes to keep. `bodies` receives the raw body of each heredoc. `None` when a
/// body cannot be read.
fn resolve_heredocs(
    bytes: &[u8],
    body_start: usize,
    pending: &[Heredoc],
    elided_line: &[u8],
    root_command: &str,
    bodies: &mut Vec<String>,
) -> Option<(usize, Vec<u8>)> {
    let drop_bodies = elided_line_is_data_reader(elided_line, root_command);
    let mut position = body_start;
    let mut kept: Vec<u8> = Vec::new();
    for heredoc in pending {
        let (body_end, after) = locate_body(bytes, position, heredoc)?;
        let body = &bytes[position..body_end];
        let Ok(body_text) = std::str::from_utf8(body) else {
            return None;
        };
        bodies.push(body_text.to_string());
        // An unquoted body is expanded by the outer shell, which runs a command
        // substitution in it. The scan must not re-read it as commands (B3).
        if !heredoc.quoted && body_contains_execution(body) {
            return None;
        }
        if !drop_bodies {
            // A kept body must be readable as commands (B2).
            if !body_is_readable(body) {
                return None;
            }
            // Read the kept body both with comments elided and with quoting
            // flattened, so neither a shell comment nor a quote can hide a path
            // the body runs (N4, H-04 C3).
            let cleaned = scan(body_text, root_command)?;
            kept.extend_from_slice(cleaned.text.as_bytes());
            bodies.extend(cleaned.bodies);
            kept.push(b' ');
            kept.extend_from_slice(&flatten_quotes(body));
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
/// `None` when an unquoted body line ends with `\` (B1) or when the terminator
/// line carries a carriage return (N3).
fn locate_body(bytes: &[u8], body_start: usize, heredoc: &Heredoc) -> Option<(usize, usize)> {
    let mut cursor = body_start;
    while cursor <= bytes.len() {
        let line_end = bytes[cursor..]
            .iter()
            .position(|ch| *ch == b'\n')
            .map(|offset| cursor + offset)
            .unwrap_or(bytes.len());
        let raw_line = &bytes[cursor..line_end];
        let line = if heredoc.dash {
            strip_leading_tabs(raw_line)
        } else {
            raw_line
        };
        if line == heredoc.delimiter.as_slice() {
            let after = if line_end < bytes.len() {
                line_end + 1
            } else {
                bytes.len()
            };
            return Some((cursor, after));
        }
        // The shell keeps a `\r` in the delimiter line, so a line that matches
        // only after dropping it is not the terminator (N3).
        if let Some(stripped) = line.strip_suffix(b"\r")
            && stripped == heredoc.delimiter.as_slice()
        {
            return None;
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

/// Whether the heredoc's elided line is a single command that reads its stdin as
/// data (H-03 §5 N1/N2, H-04 §4 C1/C2).
fn elided_line_is_data_reader(line: &[u8], root_command: &str) -> bool {
    // One command only: no pipelines, lists, subshells, groups, or substitutions.
    if line
        .iter()
        .any(|ch| matches!(ch, b';' | b'|' | b'&' | b'(' | b')' | b'{' | b'}' | b'`'))
    {
        return false;
    }
    let Some(argv) = line_argv(line) else {
        return false;
    };
    let Some(name) = argv.first() else {
        return false;
    };
    // A literal, unqualified command name: a quote or backslash in the name, or
    // a slash, disqualifies it.
    if name.quoted || name.bytes.contains(&b'/') {
        return false;
    }
    // A function or alias in the command may replace the program and execute the
    // body.
    if command_defines_function_or_alias(root_command) {
        return false;
    }
    data_reader_argv(&argv)
}

/// Whether a quote-removed argv exactly matches one of the data-reader forms.
fn data_reader_argv(argv: &[Arg]) -> bool {
    match argv[0].bytes.as_slice() {
        // `cat` with no arguments (redirects are already removed from argv).
        b"cat" => argv.len() == 1,
        b"git" => git_reads_stdin_as_data(argv),
        b"gh" => gh_reads_stdin_as_data(argv),
        _ => false,
    }
}

/// `git commit`/`git tag` immediately after `git`, with `-F -`, and no `-c`,
/// alias, or `--stdin*` form.
fn git_reads_stdin_as_data(argv: &[Arg]) -> bool {
    let Some(subcommand) = argv.get(1) else {
        return false;
    };
    if subcommand.bytes != b"commit" && subcommand.bytes != b"tag" {
        return false;
    }
    if argv.iter().any(|arg| {
        let word = arg.bytes.as_slice();
        word == b"-c"
            || (word.starts_with(b"-c") && word.len() > 2)
            || word.starts_with(b"--stdin")
            || contains_subslice(word, b"alias")
    }) {
        return false;
    }
    has_adjacent(argv, b"-F", b"-")
}

/// `gh issue|pr comment|create|edit` with `--body-file -`, and no shell alias.
fn gh_reads_stdin_as_data(argv: &[Arg]) -> bool {
    let (Some(group), Some(action)) = (argv.get(1), argv.get(2)) else {
        return false;
    };
    if group.bytes != b"issue" && group.bytes != b"pr" {
        return false;
    }
    if action.bytes != b"comment" && action.bytes != b"create" && action.bytes != b"edit" {
        return false;
    }
    if argv
        .iter()
        .any(|arg| contains_subslice(&arg.bytes, b"alias"))
    {
        return false;
    }
    has_adjacent(argv, b"--body-file", b"-")
}

fn has_adjacent(argv: &[Arg], option: &[u8], value: &[u8]) -> bool {
    argv.windows(2)
        .any(|pair| pair[0].bytes == option && pair[1].bytes == value)
}

fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

/// Reads a heredoc line into its quote-removed argv, dropping redirections and
/// their targets. `None` when the line cannot be tokenised.
fn line_argv(line: &[u8]) -> Option<Vec<Arg>> {
    let mut argv: Vec<Arg> = Vec::new();
    let mut word: Vec<u8> = Vec::new();
    let mut quoted = false;
    let mut started = false;
    let mut expect_target = false;
    let mut index = 0usize;
    while index < line.len() {
        match line[index] {
            b' ' | b'\t' => {
                flush_arg(
                    &mut word,
                    &mut quoted,
                    &mut started,
                    &mut expect_target,
                    &mut argv,
                );
                index += 1;
            }
            b'\'' => {
                quoted = true;
                started = true;
                index += 1;
                while index < line.len() && line[index] != b'\'' {
                    word.push(line[index]);
                    index += 1;
                }
                if index < line.len() {
                    index += 1;
                }
            }
            b'"' => {
                quoted = true;
                started = true;
                index += 1;
                while index < line.len() && line[index] != b'"' {
                    if line[index] == b'\\' && index + 1 < line.len() {
                        index += 1;
                    }
                    word.push(line[index]);
                    index += 1;
                }
                if index < line.len() {
                    index += 1;
                }
            }
            b'\\' => {
                quoted = true;
                started = true;
                index += 1;
                if index < line.len() {
                    word.push(line[index]);
                    index += 1;
                }
            }
            b'<' | b'>' => {
                if started {
                    if expect_target || word.iter().all(|ch| ch.is_ascii_digit()) {
                        word.clear();
                        quoted = false;
                        started = false;
                    } else {
                        argv.push(Arg {
                            bytes: std::mem::take(&mut word),
                            quoted,
                        });
                        quoted = false;
                        started = false;
                    }
                }
                index += 1;
                if index < line.len() && matches!(line[index], b'<' | b'>') {
                    index += 1;
                }
                if index < line.len() && matches!(line[index], b'-' | b'|' | b'&') {
                    index += 1;
                }
                expect_target = true;
            }
            ch => {
                word.push(ch);
                started = true;
                index += 1;
            }
        }
    }
    flush_arg(
        &mut word,
        &mut quoted,
        &mut started,
        &mut expect_target,
        &mut argv,
    );
    Some(argv)
}

fn flush_arg(
    word: &mut Vec<u8>,
    quoted: &mut bool,
    started: &mut bool,
    expect_target: &mut bool,
    argv: &mut Vec<Arg>,
) {
    if !*started {
        return;
    }
    if !*expect_target {
        argv.push(Arg {
            bytes: std::mem::take(word),
            quoted: *quoted,
        });
    }
    word.clear();
    *quoted = false;
    *started = false;
    *expect_target = false;
}

/// Whether the whole command defines a function or an alias: the word
/// `function`, the word `alias`, or a word followed by `(` … `)` across blanks
/// and line continuations (H-04 C1).
fn command_defines_function_or_alias(command: &str) -> bool {
    let bytes = command.as_bytes();
    if contains_shell_word(bytes, b"function") || contains_shell_word(bytes, b"alias") {
        return true;
    }
    (0..bytes.len()).any(|index| {
        bytes[index] == b'(' && name_before(bytes, index) && closes_after(bytes, index)
    })
}

fn contains_shell_word(haystack: &[u8], word: &[u8]) -> bool {
    (0..=haystack.len().saturating_sub(word.len())).any(|index| {
        &haystack[index..index + word.len()] == word
            && (index == 0 || !is_word_char(haystack[index - 1]))
            && (index + word.len() == haystack.len() || !is_word_char(haystack[index + word.len()]))
    })
}

fn is_word_char(ch: u8) -> bool {
    ch.is_ascii_alphanumeric() || ch == b'_'
}

/// Whether a word character precedes the `(` at `index`, across blanks and line
/// continuations.
fn name_before(bytes: &[u8], index: usize) -> bool {
    let mut cursor = index;
    loop {
        if cursor >= 2 && bytes[cursor - 1] == b'\n' && bytes[cursor - 2] == b'\\' {
            cursor -= 2;
        } else if cursor >= 1 && (bytes[cursor - 1] == b' ' || bytes[cursor - 1] == b'\t') {
            cursor -= 1;
        } else {
            break;
        }
    }
    cursor >= 1 && is_word_char(bytes[cursor - 1])
}

/// Whether `)` follows the `(` at `index`, across blanks and line continuations.
fn closes_after(bytes: &[u8], index: usize) -> bool {
    let mut cursor = index + 1;
    loop {
        if cursor < bytes.len() && (bytes[cursor] == b' ' || bytes[cursor] == b'\t') {
            cursor += 1;
        } else if cursor + 1 < bytes.len() && bytes[cursor] == b'\\' && bytes[cursor + 1] == b'\n' {
            cursor += 2;
        } else {
            break;
        }
    }
    cursor < bytes.len() && bytes[cursor] == b')'
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

/// A reading of a heredoc body with line continuations removed and every quote
/// or escape replaced by a blank, so quoting cannot hide a candidate the body
/// runs (H-04 C3).
fn flatten_quotes(body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(body.len());
    let mut index = 0usize;
    while index < body.len() {
        let ch = body[index];
        if ch == b'\\' {
            if index + 1 < body.len() && body[index + 1] == b'\n' {
                index += 2;
                continue;
            }
            out.push(b' ');
            index += 1;
            continue;
        }
        if matches!(ch, b'\'' | b'"' | b'`') {
            out.push(b' ');
            index += 1;
            continue;
        }
        out.push(ch);
        index += 1;
    }
    out
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
    fn shell_lexical_b4_carriage_return_and_form_feed_are_not_word_boundaries() {
        assert_eq!(strip("echo x\r#x"), "echo x\r#x");
        assert_eq!(strip("echo x\u{c}#x"), "echo x\u{c}#x");
    }

    #[test]
    fn shell_lexical_b5_a_line_continuation_keeps_the_word_start_state() {
        assert_eq!(strip("echo a\\\n#x\ntee f"), "echo a#x\ntee f");
        assert_eq!(strip("echo \\\n#x\ntee f"), "echo \ntee f");
        assert_eq!(strip("echo \"a\\\nb\""), "echo \"ab\"");
        assert!(!text_is_unchanged("echo \"a\\\nb\""));
    }

    #[test]
    fn shell_lexical_pins_the_output_of_escaped_quotes() {
        assert_eq!(strip("echo \"a\\b\""), "echo \"a\\b\"");
        assert_eq!(strip("echo \"a\\\\b\""), "echo \"a\\\\b\"");
        assert_eq!(strip("echo 'a\\b'"), "echo 'a\\b'");
        assert_eq!(strip("echo \"a\\"), "echo \"a\\");
        assert!(text_is_unchanged("echo \"a\\b\""));
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
    fn shell_lexical_n1_requires_a_single_data_reader_command() {
        for command in [
            "cat; sh <<'EOF'\ntee sub/link/f\nEOF",
            "cat | sh <<EOF\ntee sub/link/f\nEOF",
            "cat <<'EOF' | sh\ntee sub/link/f\nEOF",
            "tee >(sh) <<'EOF'\ntee sub/link/f\nEOF",
            "cat <<'EOF' > >(sh)\ntee sub/link/f\nEOF",
            "cat <<'EOF'; sh <<'END'\nx\nEOF\ntee sub/link/f\nEND",
            "cat && python3 <<'EOF'\nopen(\"sub/link/secret\").read()\nEOF",
            "cat; env sh <<'EOF'\ntee sub/link/f\nEOF",
        ] {
            let elided = strip(command);
            assert!(
                elided.contains("sub/link/f") || elided.contains("secret"),
                "body must be kept: {command:?} -> {elided:?}"
            );
        }
    }

    #[test]
    fn shell_lexical_c1_function_and_alias_definitions_keep_the_body() {
        for command in [
            "cat () { sh; }\ncat <<'EOF'\ntee sub/link/f\nEOF",
            "cat\t() { sh; }\ncat <<'EOF'\ntee sub/link/f\nEOF",
            "cat\\\n(){ sh; }\ncat <<'EOF'\ntee sub/link/f\nEOF",
            "function cat { sh; }\ncat <<'EOF'\ntee sub/link/f\nEOF",
            "alias cat=sh\ncat <<'EOF'\ntee sub/link/f\nEOF",
        ] {
            let elided = strip(command);
            assert!(
                elided.contains("tee sub/link/f"),
                "body must be kept: {command:?} -> {elided:?}"
            );
        }
    }

    #[test]
    fn shell_lexical_c2_git_and_gh_argv_forms() {
        assert_eq!(
            strip("git commit -F - <<'EOF'\nit's a fix\nEOF"),
            "git commit -F - <<'EOF'\n"
        );
        assert_eq!(
            strip("git tag -F - <<'EOF'\nit's a fix\nEOF"),
            "git tag -F - <<'EOF'\n"
        );
        assert_eq!(
            strip("gh issue comment 1 --body-file - <<'EOF'\nit's\nEOF"),
            "gh issue comment 1 --body-file - <<'EOF'\n"
        );
        assert_eq!(
            strip("gh pr create --body-file - <<'EOF'\nit's\nEOF"),
            "gh pr create --body-file - <<'EOF'\n"
        );
        for command in [
            "git '-c' 'al''ias.x=!sh #' x commit -F - <<'EOF'\ntee sub/link/f\nEOF",
            "git -c core.fake=1 commit -F - <<'EOF'\ntee sub/link/f\nEOF",
            "git -cX=Y commit -F - <<'EOF'\ntee sub/link/f\nEOF",
            "git --stdin-paths commit -F - <<'EOF'\ntee sub/link/f\nEOF",
            "git x -F - <<'EOF'\ntee sub/link/f\nEOF",
            "git commit x - <<'EOF'\ntee sub/link/f\nEOF",
            "git commit -F x <<'EOF'\ntee sub/link/f\nEOF",
            "gh x --body-file - <<'EOF'\ntee sub/link/f\nEOF",
            "openssl enc <<'EOF'\ntee sub/link/f\nEOF",
        ] {
            let elided = strip(command);
            assert!(
                elided.contains("tee sub/link/f"),
                "body must be kept: {command:?} -> {elided:?}"
            );
        }
    }

    #[test]
    fn shell_lexical_removed_simple_readers_keep_the_body() {
        // `tee`/`head` are no longer data readers, so an unreadable body rejects
        // (accepted false rejection) and a readable body is kept.
        assert_eq!(
            strip_comments_and_heredocs("tee x <<'EOF'\nit's\nEOF"),
            None
        );
        let elided = strip("wc -l <<'EOF'\nhello world\nEOF");
        assert!(elided.contains("hello world"), "{elided:?}");
        assert_eq!(strip_comments_and_heredocs("head <<'EOF'\nit's\nEOF"), None);
    }

    #[test]
    fn shell_lexical_n3_carriage_return_in_the_delimiter_is_unreadable() {
        assert_eq!(
            strip_comments_and_heredocs("cat <<EOF\r\nx\nEOF\r\ntee f"),
            None
        );
        assert_eq!(
            strip_comments_and_heredocs("cat <<EOF\nx\nEOF\r\ntee f"),
            None
        );
    }

    #[test]
    fn shell_lexical_n4_keeps_the_body_as_commands_and_drops_comments() {
        let elided = strip("sh <<'EOF'\n#'\ntee sub/link/f\n#'\nEOF");
        assert!(elided.contains("tee sub/link/f"), "{elided:?}");
        let elided = strip("bash <<'EOF'\necho x # it's\ntee sub/link/f #'\nEOF");
        assert!(elided.contains("tee sub/link/f"), "{elided:?}");
    }

    #[test]
    fn shell_lexical_c3_keeps_the_body_read_both_ways() {
        // Applying only the shell comment rule lets a Python inline comment's
        // quote hide the next line; the flattened reading keeps it visible (C3).
        let elided = strip("python3 <<'EOF'\npass#'\nopen(\"sub/link/f\",\"w\")#'\nEOF");
        assert!(elided.contains("sub/link/f"), "{elided:?}");
        let elided = strip("python3 <<'EOF'\npass#'\nopen(\"sub/link/secret\").read()#'\nEOF");
        assert!(elided.contains("sub/link/secret"), "{elided:?}");
        // A readable Python body with no path stays parseable.
        let elided = strip("python3 - <<'EOF'\nprint(\"it's\")\nEOF");
        assert!(elided.contains("print(\"it's\")"), "{elided:?}");
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
            "cat <<'E\\\nOF'\nx\nEOF\ntee sub/link/f",
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
        assert_eq!(
            strip("cat <<'EOF'\n$(tee sub/link/f)\nEOF"),
            "cat <<'EOF'\n"
        );
    }

    #[test]
    fn shell_lexical_body_is_readable_table() {
        for (body, expected) in [
            ("\"a\\", false),
            ("\"\\\"", false),
            ("`a\\", false),
            ("`\\`", false),
            ("`a", false),
            ("a\\\"\"", false),
            ("tee /tmp/f", true),
            ("print(\"it's\")", true),
        ] {
            assert_eq!(
                body_is_readable(body.as_bytes()),
                expected,
                "body: {body:?}"
            );
        }
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
        let elided = strip("python3 - <<'EOF'\nprint(\"it's\")\nEOF");
        assert!(elided.contains("print(\"it's\")"), "{elided:?}");
    }

    #[test]
    fn shell_lexical_treats_an_unterminated_heredoc_remainder_as_body() {
        assert_eq!(strip("cat <<EOF\nit's"), "cat <<EOF\n");
        let elided = strip("sh <<EOF\ntee /tmp/f");
        assert!(elided.contains("tee /tmp/f"), "{elided:?}");
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
    fn shell_lexical_does_not_hang_or_panic_on_short_heredoc_spellings() {
        assert_eq!(strip("cat <"), "cat <");
        assert_eq!(strip("cat <<x"), "cat <<x");
        assert_eq!(strip("echo a <<;"), "echo a <<;");
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
}
