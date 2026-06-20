const COLOR_RESET: &str = "\x1b[0m";

pub fn truncate_chars(input: &str, max_chars: usize) -> String {
    match input.char_indices().nth(max_chars) {
        Some((idx, _)) => {
            let mut out = String::with_capacity(idx + 3);
            out.push_str(&input[..idx]);
            out.push_str("...");
            out
        }
        None => input.to_string(),
    }
}

pub fn sanitize_for_progress(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        let cp = ch as u32;
        if cp < 0x20 || cp == 0x7f || (0x80..=0x9f).contains(&cp) {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    out.trim_end().to_string()
}

pub fn tool_color(tool_name: &str) -> &'static str {
    match tool_name {
        "Read" => "\x1b[38;5;87m",
        "Write" => "\x1b[38;5;198m",
        "Edit" => "\x1b[38;5;208m",
        "Bash" => "\x1b[38;5;226m",
        "Glob" => "\x1b[38;5;51m",
        "Grep" => "\x1b[38;5;39m",
        _ => "\x1b[38;5;245m",
    }
}

pub fn tool_label(tool_name: &str, emoji_enabled: bool) -> &'static str {
    if emoji_enabled {
        return match tool_name {
            "Read" => "READ",
            "Write" => "WRITE",
            "Edit" => "EDIT",
            "Bash" => "RUN",
            "Glob" => "GLOB",
            "Grep" => "GREP",
            _ => "TOOL",
        };
    }

    match tool_name {
        "Read" => "R",
        "Write" => "W",
        "Edit" => "E",
        "Bash" => "$",
        "Glob" => "G",
        "Grep" => "S",
        _ => "?",
    }
}

pub fn paint(input: &str, color: &str, color_enabled: bool) -> String {
    if color_enabled && !color.is_empty() {
        format!("{color}{input}{COLOR_RESET}")
    } else {
        input.to_string()
    }
}

pub fn progress_detail_budget(cols: Option<u16>, prefix: &str) -> usize {
    cols.map(usize::from)
        .unwrap_or(96)
        .saturating_sub(prefix.chars().count())
        .max(24)
}

pub fn format_progress_field(prefix: &str, value: &str, cols: Option<u16>) -> String {
    let budget = progress_detail_budget(cols, prefix);
    format!(
        "{prefix}{}",
        truncate_chars(&sanitize_for_progress(value), budget)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncates_without_splitting_multibyte() {
        assert_eq!(truncate_chars("abcdef", 3), "abc...");
        assert_eq!(truncate_chars("abc", 3), "abc");
        assert_eq!(truncate_chars("あいうえお", 2), "あい...");
    }

    #[test]
    fn sanitizes_terminal_controls() {
        assert_eq!(sanitize_for_progress("hello\nworld"), "hello world");
        assert_eq!(sanitize_for_progress("red\x1b[31m"), "red [31m");
        assert_eq!(sanitize_for_progress("x\u{009b}31m"), "x 31m");
    }

    #[test]
    fn maps_tool_styles_and_ascii_fallbacks() {
        assert_eq!(tool_color("Write"), "\x1b[38;5;198m");
        assert_eq!(tool_label("Write", false), "W");
        assert_eq!(tool_label("Bash", false), "$");
        assert_eq!(tool_label("Bash", true), "RUN");
    }

    #[test]
    fn paint_uses_sgr_reset_only_when_enabled() {
        assert_eq!(paint("Bash", tool_color("Bash"), false), "Bash");
        assert_eq!(
            paint("Bash", tool_color("Bash"), true),
            "\x1b[38;5;226mBash\x1b[0m"
        );
    }

    #[test]
    fn formats_width_aware_field() {
        assert_eq!(
            format_progress_field("tool: ", "abcdef", Some(20)),
            "tool: abcdef"
        );
        assert_eq!(
            format_progress_field("tool: ", "abcdefghijklmnopqrstuvwxyz", Some(10)),
            "tool: abcdefghijklmnopqrstuvwx..."
        );
    }
}
