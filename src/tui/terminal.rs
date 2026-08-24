use std::io::{self, IsTerminal, Write};

pub fn stdin_is_tty() -> bool {
    io::stdin().is_terminal()
}

pub fn stdout_is_tty() -> bool {
    io::stdout().is_terminal()
}

pub fn stderr_is_tty() -> bool {
    io::stderr().is_terminal()
}

/// Writes LF-based logical text without relying on cooked-mode newline expansion.
pub(crate) fn write_stdout_text(mut writer: impl Write, text: &str) -> io::Result<()> {
    let raw_tty = stdout_is_tty() && crossterm::terminal::is_raw_mode_enabled().unwrap_or(false);
    write_text_for_mode(&mut writer, text, raw_tty)
}

fn write_text_for_mode(mut writer: impl Write, text: &str, raw_mode: bool) -> io::Result<()> {
    if !raw_mode {
        return writer.write_all(text.as_bytes());
    }
    let bytes = text.as_bytes();
    let mut start = 0;
    for (index, byte) in bytes.iter().copied().enumerate() {
        if byte == b'\n' && (index == 0 || bytes[index - 1] != b'\r') {
            writer.write_all(&bytes[start..index])?;
            writer.write_all(b"\r\n")?;
            start = index + 1;
        }
    }
    writer.write_all(&bytes[start..])
}

pub fn env_non_empty(name: &str) -> bool {
    crate::env_compat::var_os(name).is_some_and(|value| !value.is_empty())
}

pub fn no_color() -> bool {
    env_non_empty("NO_COLOR")
}

pub fn utf8_locale() -> bool {
    utf8_locale_with(|key| std::env::var(key).ok())
}

pub fn utf8_locale_with(get_env: impl Fn(&str) -> Option<String>) -> bool {
    let value = get_env("LC_ALL")
        .or_else(|| get_env("LANG"))
        .unwrap_or_default()
        .to_ascii_uppercase();
    value.contains("UTF-8") || value.contains("UTF8")
}

pub fn env_non_empty_with(get_env: impl Fn(&str) -> Option<String>, name: &str) -> bool {
    crate::env_compat::var_with(name, get_env).is_some_and(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_mode_text_uses_crlf_without_changing_existing_crlf_or_utf8() {
        let mut output = Vec::new();

        write_text_for_mode(&mut output, "first\nsecond\r\n日本語\n", true).unwrap();

        assert_eq!(output, "first\r\nsecond\r\n日本語\r\n".as_bytes());
    }

    #[test]
    fn cooked_mode_text_preserves_lf_bytes() {
        let mut output = Vec::new();

        write_text_for_mode(&mut output, "first\n日本語\n", false).unwrap();

        assert_eq!(output, "first\n日本語\n".as_bytes());
    }

    #[test]
    fn utf8_locale_prefers_lc_all_and_recognizes_common_spellings() {
        assert!(utf8_locale_with(|key| {
            (key == "LANG").then(|| "ja_JP.UTF-8".to_string())
        }));
        assert!(utf8_locale_with(|key| {
            (key == "LC_ALL").then(|| "en_US.UTF8".to_string())
        }));
        assert!(!utf8_locale_with(|key| match key {
            "LC_ALL" => Some("C".to_string()),
            "LANG" => Some("ja_JP.UTF-8".to_string()),
            _ => None,
        }));
    }
}
