use unicode_width::UnicodeWidthChar;

pub fn floor_char_boundary(value: &str, max: usize) -> usize {
    let mut end = max.min(value.len());
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    end
}

pub fn truncate_at_char_boundary(value: &str, max: usize) -> &str {
    &value[..floor_char_boundary(value, max)]
}

pub fn excerpt_with_marker(value: &str, max: usize, marker: &str) -> String {
    if value.len() <= max {
        return value.to_string();
    }
    let mut out = truncate_at_char_boundary(value, max).to_string();
    out.push_str(marker);
    out
}

pub fn excerpt_with_newline_marker(value: &str, max: usize, marker: &str) -> String {
    if value.len() <= max {
        return value.to_string();
    }
    let mut out = truncate_at_char_boundary(value, max).to_string();
    out.push('\n');
    out.push_str(marker);
    out
}

pub fn char_display_width(ch: char) -> usize {
    UnicodeWidthChar::width(ch).unwrap_or(0)
}

pub fn display_width(value: &str) -> usize {
    value.chars().map(char_display_width).sum()
}

pub fn display_width_ansi(value: &str) -> usize {
    let mut width = 0usize;
    let mut index = 0usize;
    while index < value.len() {
        if let Some(end) = ansi_csi_end(value, index) {
            index = end;
            continue;
        }
        let ch = value[index..]
            .chars()
            .next()
            .expect("index is before the end of a valid UTF-8 string");
        width = width.saturating_add(char_display_width(ch));
        index += ch.len_utf8();
    }
    width
}

/// Fits the visible prefix of `value` to `cols` and appends `marker` on truncation.
///
/// `cols` is the content budget; the marker is outside that budget. This matches
/// the historical ASCII behavior of `excerpt_with_marker` while measuring
/// terminal columns instead of UTF-8 bytes. ANSI CSI sequences count as zero
/// columns and are never split.
pub fn fit_display_width(value: &str, cols: usize, marker: &str) -> String {
    if display_width_ansi(value) <= cols {
        return value.to_string();
    }

    let mut out = String::new();
    let mut width = 0usize;
    let mut index = 0usize;
    let mut has_sgr = false;
    while index < value.len() {
        if let Some(end) = ansi_csi_end(value, index) {
            has_sgr |= value.as_bytes().get(end.saturating_sub(1)) == Some(&b'm');
            out.push_str(&value[index..end]);
            index = end;
            continue;
        }
        let ch = value[index..]
            .chars()
            .next()
            .expect("index is before the end of a valid UTF-8 string");
        let next_width = width.saturating_add(char_display_width(ch));
        if next_width > cols {
            break;
        }
        out.push(ch);
        width = next_width;
        index += ch.len_utf8();
    }
    if has_sgr {
        out.push_str("\x1b[0m");
    }
    out.push_str(marker);
    out
}

fn ansi_csi_end(value: &str, start: usize) -> Option<usize> {
    let bytes = value.as_bytes();
    if bytes.get(start) != Some(&0x1b) || bytes.get(start + 1) != Some(&b'[') {
        return None;
    }
    value[start + 2..].char_indices().find_map(|(offset, ch)| {
        ('@'..='~')
            .contains(&ch)
            .then_some(start + 2 + offset + ch.len_utf8())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_at_char_boundary_never_panics_on_multibyte_lengths() {
        let fixture = "abc日本語def除外🙂かなカナ";
        for max in 0..=fixture.len() + 8 {
            let truncated = truncate_at_char_boundary(fixture, max);
            assert!(truncated.len() <= max.min(fixture.len()));
            assert!(fixture.starts_with(truncated));
        }
    }

    #[test]
    fn excerpt_with_marker_preserves_valid_utf8() {
        let fixture = "prefix日本語除外suffix";
        let excerpt = excerpt_with_marker(fixture, 10, "...[truncated]");
        assert!(excerpt.starts_with("prefix日"));
        assert!(excerpt.ends_with("...[truncated]"));
    }

    #[test]
    fn display_width_counts_unicode_columns_and_ignores_ansi_csi() {
        assert_eq!(display_width("ascii"), 5);
        assert_eq!(display_width("日本"), 4);
        assert_eq!(display_width("🙂"), 2);
        assert_eq!(display_width("e\u{301}"), 1);
        assert_eq!(display_width_ansi("\x1b[31m日本\x1b[0m"), 4);
    }

    #[test]
    fn fit_display_width_preserves_ascii_budget_and_expands_cjk_budget() {
        let ascii = "a".repeat(121);
        assert_eq!(
            fit_display_width(&ascii, 120, "..."),
            format!("{}...", "a".repeat(120))
        );

        let japanese = "日".repeat(61);
        assert_eq!(
            fit_display_width(&japanese, 120, "..."),
            format!("{}...", "日".repeat(60))
        );
    }

    #[test]
    fn fit_display_width_keeps_character_combining_and_ansi_boundaries() {
        assert_eq!(fit_display_width("e\u{301}x", 1, "..."), "e\u{301}...");
        assert_eq!(fit_display_width("🙂🙂", 2, "..."), "🙂...");
        assert_eq!(fit_display_width("日本", 0, "..."), "...");
        assert_eq!(fit_display_width("日本", 1, "..."), "...");
        assert_eq!(fit_display_width("日本", 2, "..."), "日...");
        assert_eq!(
            fit_display_width("\x1b[31m日本語\x1b[0m", 4, "..."),
            "\x1b[31m日本\x1b[0m..."
        );
        assert_eq!(fit_display_width("\x1b[31", 2, "..."), "\x1b[3...");
    }
}
