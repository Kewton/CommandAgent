use crate::tui::progress::{sanitize_for_progress, truncate_chars};

const LOGO_ART: &[&str] = &[
    "  ____                          _   _                    _",
    " / ___|___  _ __ ___  _ __ ___ | | / \\   __ _  ___ _ __ | |_",
    "| |   / _ \\| '_ ` _ \\| '_ ` _ \\| |/ _ \\ / _` |/ _ \\ '_ \\| __|",
    "| |__| (_) | | | | | | | | | | | / ___ \\ (_| |  __/ | | | |_",
    " \\____\\___/|_| |_| |_|_| |_| |_|_/_/   \\_\\__, |\\___|_| |_|\\__|",
    "                                          |___/",
];
const LOGO_GRADIENT_256: &[u8] = &[51, 39, 45, 81, 87, 123];
const MAX_DYNAMIC_CHARS: usize = 56;
const RESET: &str = "\x1b[0m";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BannerStyle {
    ColorArt,
    MonoArt,
    Plain,
}

#[derive(Debug, Clone, Copy)]
pub struct StartupBanner<'a> {
    pub version: &'a str,
    pub cwd: &'a str,
    pub executor: &'a str,
    pub planner: &'a str,
    pub flags: &'a [&'a str],
    pub style: BannerStyle,
}

pub fn decide_banner_style(
    stderr_is_tty: bool,
    banner_enabled: bool,
    color_enabled: bool,
) -> BannerStyle {
    if !stderr_is_tty || !banner_enabled {
        BannerStyle::Plain
    } else if color_enabled {
        BannerStyle::ColorArt
    } else {
        BannerStyle::MonoArt
    }
}

pub fn render_startup_banner(input: &StartupBanner<'_>) -> String {
    let mut out = String::new();

    match input.style {
        BannerStyle::ColorArt => {
            for (index, line) in LOGO_ART.iter().enumerate() {
                let code = LOGO_GRADIENT_256[index % LOGO_GRADIENT_256.len()];
                out.push_str(&format!("\x1b[38;5;{code}m{line}{RESET}\n"));
            }
        }
        BannerStyle::MonoArt => {
            for line in LOGO_ART {
                out.push_str(line);
                out.push('\n');
            }
        }
        BannerStyle::Plain => {}
    }

    out.push_str(&context_lines(input));
    out
}

pub fn logo_art() -> &'static [&'static str] {
    LOGO_ART
}

fn context_lines(input: &StartupBanner<'_>) -> String {
    let version = bounded(input.version);
    let cwd = bounded(input.cwd);
    let executor = bounded(input.executor);
    let planner = bounded(input.planner);
    let flags = flags_suffix(input.flags);

    if input.style == BannerStyle::Plain {
        return format!(
            "CommandAgent {version} cwd={cwd} provider={executor} planner={planner}{flags}\n"
        );
    }

    format!("CommandAgent {version} cwd={cwd}{flags}\nprovider={executor}\nplanner={planner}\n")
}

fn flags_suffix(flags: &[&str]) -> String {
    if flags.is_empty() {
        String::new()
    } else {
        format!(
            " [{}]",
            flags
                .iter()
                .map(|flag| bounded(flag))
                .collect::<Vec<_>>()
                .join(",")
        )
    }
}

fn bounded(value: &str) -> String {
    truncate_chars(&sanitize_for_progress(value), MAX_DYNAMIC_CHARS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(style: BannerStyle) -> StartupBanner<'static> {
        StartupBanner {
            version: "0.1.0",
            cwd: "work",
            executor: "gemini:gemini-3.1-flash-lite",
            planner: "gemini:gemini-3.5-flash",
            flags: &["yes"],
            style,
        }
    }

    #[test]
    fn logo_art_is_narrow() {
        for line in logo_art() {
            assert!(line.chars().count() <= 64, "{line}");
        }
    }

    #[test]
    fn decide_banner_style_uses_tty_banner_and_color() {
        assert_eq!(decide_banner_style(false, true, true), BannerStyle::Plain);
        assert_eq!(decide_banner_style(true, false, true), BannerStyle::Plain);
        assert_eq!(decide_banner_style(true, true, false), BannerStyle::MonoArt);
        assert_eq!(decide_banner_style(true, true, true), BannerStyle::ColorArt);
    }

    #[test]
    fn plain_banner_keeps_compact_startup_context_without_art() {
        let out = render_startup_banner(&sample(BannerStyle::Plain));

        assert!(!out.contains("\x1b["));
        assert!(!out.contains("____"));
        assert!(out.contains("CommandAgent 0.1.0 cwd=work"));
        assert!(out.contains("provider=gemini:gemini-3.1-flash-lite"));
        assert!(out.contains("planner=gemini:gemini-3.5-flash"));
        assert!(out.contains("[yes]"));
    }

    #[test]
    fn mono_banner_contains_art_without_ansi() {
        let out = render_startup_banner(&sample(BannerStyle::MonoArt));

        assert!(!out.contains("\x1b["));
        assert!(out.contains("____"));
        assert!(out.contains("provider=gemini:gemini-3.1-flash-lite"));
    }

    #[test]
    fn color_banner_colors_only_fixed_art() {
        let out = render_startup_banner(&sample(BannerStyle::ColorArt));

        assert!(out.contains("\x1b[38;5;51m"));
        assert!(out.contains(RESET));
        assert!(out.contains("CommandAgent 0.1.0 cwd=work"));
    }

    #[test]
    fn dynamic_fields_are_sanitized_and_bounded() {
        let long = "x".repeat(200);
        let banner = StartupBanner {
            version: "0.1.0\nbad\x1b[31m",
            cwd: "cwd\nbad",
            executor: &long,
            planner: "planner\u{009b}bad",
            flags: &["yes\nbad"],
            style: BannerStyle::Plain,
        };

        let out = render_startup_banner(&banner);
        let lines = out.lines().collect::<Vec<_>>();

        assert_eq!(lines.len(), 1);
        assert!(!out.contains("\x1b[31m"));
        assert!(out.contains("xxx..."));
        assert!(out.contains("planner bad"));
        assert!(out.contains("[yes bad]"));
    }
}
