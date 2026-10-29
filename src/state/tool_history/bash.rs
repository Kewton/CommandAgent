//! History policy for `Bash` results (Issue #916).
//!
//! A Bash result is the model's only view of a command's diagnostics: the
//! last exception of a traceback, stderr printed after a long stdout, or the
//! indentation of a file printed with `cat`. The result keeps its line breaks
//! and its whitespace up to [`BASH_HISTORY_MAX_BYTES`], truncation notice
//! included.
//!
//! The policy only decides which text the model sees. It never adds a line,
//! never changes the outcome, status or stream labels the formatter wrote, and
//! is not consulted by any status, verification or evidence decision; those
//! read the raw tool result before it reaches the history.

use crate::sensitive_data::{active_marker, scrub_active};

/// Upper bound, in UTF-8 bytes, of a stored `Bash` result including the
/// truncation notice.
pub(crate) const BASH_HISTORY_MAX_BYTES: usize = 4_000;

/// Upper bound, in UTF-8 bytes, of the formatter's metadata lines plus the
/// truncation notice when a structured result is cut.
const METADATA_MAX_BYTES: usize = 768;

pub(crate) const BASH_TRUNCATION_NOTICE: &str =
    "[commandagent: bash result truncated in history at 4000 bytes]";

/// The metadata lines `tools::bash::format_outcome` writes, in order, before
/// the `stdout:` heading.
const METADATA_PREFIXES: [&str; 4] = ["outcome: ", "status: ", "elapsed_ms: ", "summary: "];
const STDOUT_HEADING: &str = "stdout:\n";
const STDERR_SEPARATOR: &str = "\nstderr:\n";

/// Bounded, line-preserving history content for a `Bash` result.
///
/// The complete value is scrubbed first and only then cut, so a secret that
/// straddles a cut is already replaced. The cut text is scrubbed once more as
/// a whole and the budget re-checked. Applying this to its own output returns
/// the same text.
pub(crate) fn bash_history_content(content: &str) -> String {
    let scrubbed = scrub_preserving_layout(content);
    if scrubbed.len() <= BASH_HISTORY_MAX_BYTES {
        return scrubbed;
    }
    let notice = scrub_preserving_layout(BASH_TRUNCATION_NOTICE);
    let sections = formatted_sections(&scrubbed);
    let mut budget = BASH_HISTORY_MAX_BYTES;
    loop {
        let cut = match &sections {
            Some(sections) => cut_sections(sections, &notice, budget),
            None => cut_whole(&scrubbed, &notice, budget),
        };
        let candidate = scrub_preserving_layout(&cut);
        if candidate.len() <= BASH_HISTORY_MAX_BYTES {
            return candidate;
        }
        // A replacement in the second scrub changed the length; shrink and
        // cut again. The budget strictly decreases, so this ends.
        let excess = candidate.len() - BASH_HISTORY_MAX_BYTES;
        if budget <= notice.len() + excess {
            return notice;
        }
        budget -= excess;
    }
}

/// The same scrub as `eval_events::scrub_sensitive_text` (active secrets, then
/// secret-like tokens, then home paths) except that whitespace is kept as is,
/// so indentation and blank lines survive. A secret-like token is replaced
/// whole by the run's marker.
fn scrub_preserving_layout(value: &str) -> String {
    let scrubbed = scrub_active(value);
    let marker = active_marker();
    let mut out = String::with_capacity(scrubbed.len());
    let mut token_start: Option<usize> = None;
    for (index, ch) in scrubbed.char_indices() {
        if ch.is_whitespace() {
            if let Some(start) = token_start.take() {
                push_token(&mut out, &scrubbed[start..index], &marker);
            }
            out.push(ch);
        } else if token_start.is_none() {
            token_start = Some(index);
        }
    }
    if let Some(start) = token_start {
        push_token(&mut out, &scrubbed[start..], &marker);
    }
    redact_home_paths(&out)
}

fn push_token(out: &mut String, token: &str, marker: &str) {
    if token.starts_with("sk-")
        || token.starts_with("AIza")
        || token.to_ascii_lowercase().contains("api_key")
    {
        out.push_str(marker);
    } else {
        out.push_str(token);
    }
}

/// The user name in `/Users/<name>/` and `/home/<name>/`, as event snippets
/// redact it.
fn redact_home_paths(value: &str) -> String {
    let mut out = value.to_string();
    for prefix in ["/Users/", "/home/"] {
        let mut search_from = 0usize;
        while let Some(relative_start) = out[search_from..].find(prefix) {
            let name_start = search_from + relative_start + prefix.len();
            let Some(rest_end) = out[name_start..].find('/') else {
                break;
            };
            let name_end = name_start + rest_end;
            if &out[name_start..name_end] != "<user>" {
                out.replace_range(name_start..name_end, "<user>");
            }
            search_from = name_start + "<user>".len();
        }
    }
    out
}

/// The parts of a result written by `tools::bash::format_outcome`.
#[derive(Debug, PartialEq, Eq)]
struct Sections<'a> {
    metadata: &'a str,
    stdout: &'a str,
    stderr: &'a str,
}

/// Splits a result only when it has exactly the formatter's shape: the four
/// metadata lines in order, the `stdout:` heading, and one `stderr:` heading.
/// Anything else (a split run's combined result, an error feedback, a
/// flattened legacy snippet, or a stream that itself prints a `stderr:`
/// heading) is not guessed at and returns `None`.
fn formatted_sections(text: &str) -> Option<Sections<'_>> {
    let mut end = 0usize;
    for prefix in METADATA_PREFIXES {
        let rest = &text[end..];
        if !rest.starts_with(prefix) {
            return None;
        }
        end += rest.find('\n')? + 1;
    }
    let metadata = &text[..end - 1];
    let body = text[end..].strip_prefix(STDOUT_HEADING)?;
    let separator = body.find(STDERR_SEPARATOR)?;
    // A second heading, overlapping or not, makes the boundary ambiguous.
    if body[separator + 1..].contains(STDERR_SEPARATOR) {
        return None;
    }
    Some(Sections {
        metadata,
        stdout: &body[..separator],
        stderr: &body[separator + STDERR_SEPARATOR.len()..],
    })
}

/// Metadata (bounded), then each stream as head and tail, then the notice.
/// The streams share the rest equally; a stream that needs less leaves the
/// remainder to the other.
fn cut_sections(sections: &Sections<'_>, notice: &str, budget: usize) -> String {
    let metadata_cap = METADATA_MAX_BYTES.saturating_sub(notice.len() + 1);
    let metadata = cut_head(sections.metadata, metadata_cap);
    let fixed =
        metadata.len() + 1 + STDOUT_HEADING.len() + STDERR_SEPARATOR.len() + 1 + notice.len();
    let available = budget.saturating_sub(fixed);
    let half = available / 2;
    let (stdout_share, stderr_share) = if sections.stdout.len() <= half {
        let stdout = sections.stdout.len();
        (stdout, available - stdout)
    } else if sections.stderr.len() <= available - half {
        let stderr = sections.stderr.len();
        (available - stderr, stderr)
    } else {
        (half, available - half)
    };
    format!(
        "{metadata}\n{STDOUT_HEADING}{}{STDERR_SEPARATOR}{}\n{notice}",
        cut_stream(sections.stdout, stdout_share, "stdout"),
        cut_stream(sections.stderr, stderr_share, "stderr"),
    )
}

/// Head and tail of a text whose shape is not recognised, then the notice.
fn cut_whole(text: &str, notice: &str, budget: usize) -> String {
    let available = budget.saturating_sub(notice.len() + 1);
    format!("{}\n{notice}", cut_stream(text, available, "output"))
}

/// `text` whole when it fits in `share` bytes; otherwise its first third and
/// last two thirds around an omission line. The tail gets every byte the head
/// leaves, so the final lines are kept first.
fn cut_stream(text: &str, share: usize, label: &str) -> String {
    if text.len() <= share {
        return text.to_string();
    }
    let reserved = omission_line(text.len(), label).len() + 2;
    if share <= reserved {
        return cut_tail(text, share).to_string();
    }
    let inner = share - reserved;
    let head = cut_head(text, inner / 3);
    let tail = cut_tail(&text[head.len()..], inner - head.len());
    let omitted = text.len() - head.len() - tail.len();
    format!("{head}\n{}\n{tail}", omission_line(omitted, label))
}

fn omission_line(bytes: usize, label: &str) -> String {
    format!("[commandagent: {bytes} bytes of {label} omitted in history]")
}

/// The longest prefix of `text` within `budget` bytes that ends on a char
/// boundary, preferring the end of a whole line when one is near the end.
fn cut_head(text: &str, budget: usize) -> &str {
    let mut end = budget.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let head = &text[..end];
    if end == text.len() {
        return head;
    }
    match head.rfind('\n') {
        Some(newline) if newline >= end / 2 => &head[..newline],
        _ => head,
    }
}

/// The longest suffix of `text` within `budget` bytes that starts on a char
/// boundary, preferring the start of a whole line when one is near the start.
fn cut_tail(text: &str, budget: usize) -> &str {
    let mut start = text.len() - budget.min(text.len());
    while !text.is_char_boundary(start) {
        start += 1;
    }
    let tail = &text[start..];
    if start == 0 {
        return tail;
    }
    match tail.find('\n') {
        Some(newline) if newline < tail.len() / 2 => &tail[newline + 1..],
        _ => tail,
    }
}

#[cfg(test)]
mod tests;
