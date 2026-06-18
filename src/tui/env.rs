use std::io::IsTerminal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalEnv {
    pub stdout_is_tty: bool,
    pub stderr_is_tty: bool,
    pub stdout_color_enabled: bool,
    pub stderr_color_enabled: bool,
    pub color_enabled: bool,
    pub progress_enabled: bool,
    pub spinner_enabled: bool,
    pub banner_enabled: bool,
    pub markdown_enabled: bool,
    pub emoji_enabled: bool,
    pub utf8_locale: bool,
}

pub fn detect() -> TerminalEnv {
    detect_with(
        |key| std::env::var(key).ok(),
        std::io::stdout().is_terminal(),
        std::io::stderr().is_terminal(),
    )
}

pub fn detect_with(
    get_env: impl Fn(&str) -> Option<String>,
    stdout_is_tty: bool,
    stderr_is_tty: bool,
) -> TerminalEnv {
    let no_color = env_flag(&get_env, "NO_COLOR");
    let no_spinner = env_flag(&get_env, "COMMANDAGENT_NO_SPINNER");
    let no_banner = env_flag(&get_env, "COMMANDAGENT_NO_BANNER");
    let no_markdown = env_flag(&get_env, "COMMANDAGENT_NO_MARKDOWN");
    let no_emoji = env_flag(&get_env, "COMMANDAGENT_NO_EMOJI");
    let utf8_locale = ["LC_ALL", "LC_CTYPE", "LANG"]
        .iter()
        .filter_map(|key| get_env(key))
        .any(|value| is_utf8_locale(&value));

    TerminalEnv {
        stdout_is_tty,
        stderr_is_tty,
        stdout_color_enabled: stdout_is_tty && !no_color,
        stderr_color_enabled: stderr_is_tty && !no_color,
        color_enabled: stdout_is_tty && !no_color,
        progress_enabled: stderr_is_tty,
        spinner_enabled: stderr_is_tty && !no_spinner,
        banner_enabled: stderr_is_tty && !no_banner,
        markdown_enabled: stdout_is_tty && !no_markdown,
        emoji_enabled: utf8_locale && !no_emoji,
        utf8_locale,
    }
}

pub fn env_flag(get_env: &impl Fn(&str) -> Option<String>, key: &str) -> bool {
    get_env(key).is_some_and(|value| !value.is_empty())
}

pub fn is_utf8_locale(value: &str) -> bool {
    value
        .to_ascii_lowercase()
        .split(['.', '_', '@', ';', ',', ' '])
        .any(|part| part == "utf-8" || part == "utf8")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn detects_interactive_defaults() {
        let env = HashMap::from([("LANG", "ja_JP.UTF-8")]);
        let detected = detect_with(|key| env.get(key).map(|v| v.to_string()), true, true);

        assert!(detected.color_enabled);
        assert!(detected.stdout_color_enabled);
        assert!(detected.stderr_color_enabled);
        assert!(detected.progress_enabled);
        assert!(detected.spinner_enabled);
        assert!(detected.banner_enabled);
        assert!(detected.markdown_enabled);
        assert!(detected.emoji_enabled);
        assert!(detected.utf8_locale);
    }

    #[test]
    fn respects_disable_flags() {
        let env = HashMap::from([
            ("LANG", "C.UTF-8"),
            ("NO_COLOR", "1"),
            ("COMMANDAGENT_NO_SPINNER", "1"),
            ("COMMANDAGENT_NO_BANNER", "1"),
            ("COMMANDAGENT_NO_MARKDOWN", "1"),
            ("COMMANDAGENT_NO_EMOJI", "1"),
        ]);
        let detected = detect_with(|key| env.get(key).map(|v| v.to_string()), true, true);

        assert!(!detected.color_enabled);
        assert!(!detected.stdout_color_enabled);
        assert!(!detected.stderr_color_enabled);
        assert!(detected.progress_enabled);
        assert!(!detected.spinner_enabled);
        assert!(!detected.banner_enabled);
        assert!(!detected.markdown_enabled);
        assert!(!detected.emoji_enabled);
    }

    #[test]
    fn non_tty_disables_terminal_features() {
        let env = HashMap::from([("LANG", "C.UTF-8")]);
        let detected = detect_with(|key| env.get(key).map(|v| v.to_string()), false, false);

        assert!(!detected.color_enabled);
        assert!(!detected.stdout_color_enabled);
        assert!(!detected.stderr_color_enabled);
        assert!(!detected.progress_enabled);
        assert!(!detected.spinner_enabled);
        assert!(!detected.banner_enabled);
        assert!(!detected.markdown_enabled);
        assert!(detected.emoji_enabled);
    }

    #[test]
    fn color_and_feature_gates_are_stream_aware() {
        let env = HashMap::from([("LANG", "C.UTF-8")]);

        let stderr_only = detect_with(|key| env.get(key).map(|v| v.to_string()), false, true);
        assert!(!stderr_only.stdout_color_enabled);
        assert!(stderr_only.stderr_color_enabled);
        assert!(stderr_only.progress_enabled);
        assert!(stderr_only.spinner_enabled);
        assert!(stderr_only.banner_enabled);
        assert!(!stderr_only.markdown_enabled);

        let stdout_only = detect_with(|key| env.get(key).map(|v| v.to_string()), true, false);
        assert!(stdout_only.stdout_color_enabled);
        assert!(!stdout_only.stderr_color_enabled);
        assert!(!stdout_only.progress_enabled);
        assert!(!stdout_only.spinner_enabled);
        assert!(!stdout_only.banner_enabled);
        assert!(stdout_only.markdown_enabled);
    }

    #[test]
    fn empty_disable_flags_do_not_disable_features() {
        let env = HashMap::from([
            ("LANG", "C.UTF-8"),
            ("NO_COLOR", ""),
            ("COMMANDAGENT_NO_SPINNER", ""),
            ("COMMANDAGENT_NO_BANNER", ""),
        ]);
        let detected = detect_with(|key| env.get(key).map(|v| v.to_string()), true, true);

        assert!(detected.stdout_color_enabled);
        assert!(detected.stderr_color_enabled);
        assert!(detected.spinner_enabled);
        assert!(detected.banner_enabled);
    }

    #[test]
    fn utf8_locale_table() {
        for value in ["en_US.UTF-8", "C.UTF8", "ja_JP.UTF-8@variant"] {
            assert!(is_utf8_locale(value), "{value}");
        }
        for value in ["", "C", "POSIX", "en_US.utf-800", "utf8x"] {
            assert!(!is_utf8_locale(value), "{value}");
        }
    }
}
