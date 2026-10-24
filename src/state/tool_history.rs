//! History policy for tool results.
//!
//! Every tool result is bounded before it is stored and again before it is
//! sent to the model. `Read` is the one tool whose result is the model's only
//! view of a file, so it keeps its line breaks and its content up to an
//! explicit byte budget instead of the 500-character flattened snippet.

const READ_TOOL_NAME: &str = "Read";

/// Upper bound, in UTF-8 bytes, of a stored `Read` result including the path
/// line, the body and the truncation notice.
pub(crate) const READ_HISTORY_MAX_BYTES: usize = 24_000;

const TRUNCATION_NOTICE: &str = "[commandagent: read result truncated in history at 24000 bytes]";

/// Bounded history content for a tool result produced by `name`.
///
/// `name` is `None` for a message that carries no tool name. Only a tool named
/// exactly `Read` gets the larger, line-preserving policy.
pub(crate) fn history_content(name: Option<&str>, content: &str) -> String {
    match name {
        Some(READ_TOOL_NAME) => read_history_content(content),
        _ => crate::eval_events::body_snippet_whole_tokens(content),
    }
}

/// Scrub the complete value first, then cut. A secret that straddles the cut
/// is already replaced, so no fragment of it can survive. Applying this to its
/// own output returns the same text.
fn read_history_content(content: &str) -> String {
    let scrubbed = crate::eval_events::scrub_sensitive_text(content);
    if scrubbed.len() <= READ_HISTORY_MAX_BYTES {
        return scrubbed;
    }
    let notice = crate::eval_events::scrub_sensitive_text(TRUNCATION_NOTICE);
    let mut head_budget = READ_HISTORY_MAX_BYTES.saturating_sub(notice.len() + 1);
    loop {
        let head = cut_head(&scrubbed, head_budget);
        // The cut text and the notice are scrubbed once more as a whole; the
        // total is re-checked because a replacement can change the length.
        let candidate = crate::eval_events::scrub_sensitive_text(&format!("{head}\n{notice}"));
        if candidate.len() <= READ_HISTORY_MAX_BYTES {
            return candidate;
        }
        let excess = candidate.len() - READ_HISTORY_MAX_BYTES;
        if head_budget == 0 {
            return notice;
        }
        head_budget = head_budget.saturating_sub(excess);
    }
}

/// The longest prefix of `text` within `budget` bytes that ends on a char
/// boundary, preferring the end of a whole line when one is near the end.
fn cut_head(text: &str, budget: usize) -> &str {
    let mut end = budget.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let head = &text[..end];
    match head.rfind('\n') {
        Some(newline) if newline >= end / 2 => &head[..newline],
        _ => head,
    }
}

#[cfg(test)]
mod tests {
    use super::{READ_HISTORY_MAX_BYTES, TRUNCATION_NOTICE, history_content};
    use crate::sensitive_data::{SecretCatalog, install_scope, set_current};

    fn read(content: &str) -> String {
        history_content(Some("Read"), content)
    }

    /// `src/a.ts\n` followed by `body_len - 9` filler bytes in short lines.
    fn read_of_len(total: usize) -> String {
        let mut out = String::from("src/a.ts\n");
        while out.len() < total {
            let remaining = total - out.len();
            let line = "abcdefghi\n";
            if remaining >= line.len() {
                out.push_str(line);
            } else {
                out.push_str(&"z".repeat(remaining));
            }
        }
        out
    }

    #[test]
    fn read_at_or_below_the_budget_is_kept_whole_with_newlines() {
        for total in [READ_HISTORY_MAX_BYTES - 1, READ_HISTORY_MAX_BYTES] {
            let input = read_of_len(total);
            let stored = read(&input);
            assert_eq!(stored, input, "total {total}");
            assert!(stored.contains('\n'));
        }
    }

    #[test]
    fn read_one_byte_over_the_budget_is_cut_with_the_notice_within_the_budget() {
        let input = read_of_len(READ_HISTORY_MAX_BYTES + 1);
        let stored = read(&input);
        assert!(stored.len() <= READ_HISTORY_MAX_BYTES);
        assert!(stored.ends_with(TRUNCATION_NOTICE));
        assert!(stored.starts_with("src/a.ts\nabcdefghi\n"));
        assert!(stored.matches(TRUNCATION_NOTICE).count() == 1);
    }

    #[test]
    fn read_single_long_line_is_cut_within_the_budget() {
        let input = format!("src/a.ts\n{}", "q".repeat(READ_HISTORY_MAX_BYTES * 2));
        let stored = read(&input);
        assert!(stored.len() <= READ_HISTORY_MAX_BYTES);
        assert!(stored.ends_with(TRUNCATION_NOTICE));
        assert!(stored.len() > READ_HISTORY_MAX_BYTES - 200);
    }

    #[test]
    fn read_multibyte_text_is_never_cut_inside_a_codepoint() {
        for prefix in 0..4 {
            let input = format!(
                "src/a.ts\n{}{}",
                "x".repeat(prefix),
                "日本語€😀".repeat(READ_HISTORY_MAX_BYTES / 4)
            );
            let stored = read(&input);
            assert!(stored.len() <= READ_HISTORY_MAX_BYTES, "prefix {prefix}");
            assert!(stored.ends_with(TRUNCATION_NOTICE));
            assert!(String::from_utf8(stored.clone().into_bytes()).is_ok());
            assert!(stored.contains('日'));
        }
    }

    #[test]
    fn read_with_a_head_tail_summary_shape_stays_within_the_budget() {
        let head = (1..=120)
            .map(|n| format!("{n}: {}", "h".repeat(300)))
            .collect::<Vec<_>>()
            .join("\n");
        let tail = (381..=460)
            .map(|n| format!("{n}: {}", "t".repeat(300)))
            .collect::<Vec<_>>()
            .join("\n");
        let input = format!(
            "src/big.ts\n{head}\n[commandagent: file summarized; omitted 260 middle lines]\n{tail}"
        );
        assert!(input.len() > READ_HISTORY_MAX_BYTES);
        let stored = read(&input);
        assert!(stored.len() <= READ_HISTORY_MAX_BYTES);
        assert!(stored.starts_with("src/big.ts\n1: "));
        assert!(stored.ends_with(TRUNCATION_NOTICE));
    }

    #[test]
    fn read_directory_listing_keeps_one_entry_per_line() {
        let listing = format!(
            "src\n{}",
            (0..2_000)
                .map(|n| format!("file-{n:05}.ts"))
                .collect::<Vec<_>>()
                .join("\n")
        );
        let stored = read(&listing);
        assert!(stored.len() <= READ_HISTORY_MAX_BYTES);
        assert!(stored.starts_with("src\nfile-00000.ts\nfile-00001.ts\n"));
        assert!(stored.ends_with(TRUNCATION_NOTICE));
    }

    #[test]
    fn read_policy_applied_twice_returns_the_same_text() {
        for total in [100, READ_HISTORY_MAX_BYTES, READ_HISTORY_MAX_BYTES * 3] {
            let once = read(&read_of_len(total));
            let twice = read(&once);
            assert_eq!(once, twice, "total {total}");
            assert_eq!(
                twice.matches(TRUNCATION_NOTICE).count(),
                usize::from(total > READ_HISTORY_MAX_BYTES)
            );
        }
    }

    #[test]
    fn other_tools_and_missing_names_keep_the_flattened_snippet() {
        let input = format!("line one\nline two {}", "w ".repeat(600));
        let expected = crate::eval_events::body_snippet_whole_tokens(&input);
        for name in [
            Some("Bash"),
            Some("Write"),
            Some("Edit"),
            Some("Grep"),
            Some("read"),
            Some("Read "),
            Some("Unknown"),
            None,
        ] {
            let stored = history_content(name, &input);
            assert_eq!(stored, expected, "{name:?}");
            assert!(stored.chars().count() <= 500);
            assert!(!stored.contains('\n'));
        }
    }

    #[test]
    fn read_secret_straddling_the_cut_leaves_no_fragment() {
        let mut catalog = SecretCatalog::new();
        let secret = "fake-straddle-secret-0123456789abcdef";
        catalog.register(secret);
        install_scope(catalog, None, None);
        let notice_len = TRUNCATION_NOTICE.len();
        // Place the secret so the byte budget lands inside it.
        let pad = READ_HISTORY_MAX_BYTES - notice_len - 1 - 10;
        let input = format!(
            "src/a.ts\n{}{secret} tail {}",
            "p".repeat(pad),
            "r".repeat(2_000)
        );
        assert!(input.len() > READ_HISTORY_MAX_BYTES);
        let stored = read(&input);
        set_current(None);
        assert!(stored.len() <= READ_HISTORY_MAX_BYTES);
        assert!(!stored.contains(secret));
        assert!(!stored.contains("fake-straddle"));
        assert!(!stored.contains("0123456789abcdef"));
    }
}
