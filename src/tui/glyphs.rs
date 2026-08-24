pub fn for_locale(text: &str, use_utf8: bool) -> String {
    if use_utf8 {
        return text.to_string();
    }

    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '→' => out.push_str("->"),
            '✓' => out.push_str("ok"),
            '✗' => out.push('x'),
            '↻' => out.push('~'),
            '…' => out.push_str("..."),
            '·' => out.push('|'),
            '▶' => out.push('>'),
            '○' => out.push('o'),
            '─' => out.push('-'),
            '╭' | '╮' | '╰' | '╯' => out.push('+'),
            '│' => out.push('|'),
            _ => out.push(ch),
        }
    }
    out
}

pub fn for_current_locale(text: &str) -> String {
    for_locale(text, crate::tui::terminal::utf8_locale())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_locale_replaces_presentation_glyphs_only() {
        let text = "╭─╮ → ✓ ✗ ↻ … · ▶ ○ 日本語 │ ╰─╯";

        assert_eq!(
            for_locale(text, false),
            "+-+ -> ok x ~ ... | > o 日本語 | +-+"
        );
        assert_eq!(for_locale(text, true), text);
    }
}
