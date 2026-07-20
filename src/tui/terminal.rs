use std::io::{self, IsTerminal};

pub fn stdin_is_tty() -> bool {
    io::stdin().is_terminal()
}

pub fn stdout_is_tty() -> bool {
    io::stdout().is_terminal()
}

pub fn stderr_is_tty() -> bool {
    io::stderr().is_terminal()
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
