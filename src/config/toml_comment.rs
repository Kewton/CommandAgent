//! Comment removal for one line of a layered `config.toml` (Issue #941).
//!
//! The config readers are line based. A `#` starts a comment only outside a
//! TOML string: a basic string (`"..."`, with `\` escapes) or a literal string
//! (`'...'`, no escapes, so a `"` inside it is an ordinary character).
//!
//! A line this reader cannot interpret, because it opens a multi-line string
//! (`"""` or `'''`) or leaves a string unclosed, is never cut: comment removal
//! returns it unchanged, and [`reject_uninterpretable`] and
//! [`validate_config_files`] refuse it with [`UNINTERPRETABLE_LINE`], which
//! names the file and line only, never the key, the value, or the line.

use std::path::Path;

use anyhow::bail;

/// The fixed diagnostic for a line the line-based reader cannot interpret.
pub(super) const UNINTERPRETABLE_LINE: &str = "has a multi-line or unclosed string that the config reader does not support (the key and value are not shown)";

/// What one line holds outside comments.
#[derive(Debug, PartialEq, Eq)]
enum Scan<'a> {
    /// The line up to its comment, or the whole line when it has none.
    Code(&'a str),
    /// A multi-line string opens, or a string is left unclosed.
    Uninterpretable,
}

fn scan(line: &str) -> Scan<'_> {
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in line.char_indices() {
        match quote {
            Some('"') if escaped => escaped = false,
            Some('"') if character == '\\' => escaped = true,
            Some(open) if character == open => quote = None,
            Some(_) => {}
            None if character == '#' => return Scan::Code(&line[..index]),
            None if character == '"' || character == '\'' => {
                let delimiter = if character == '"' { "\"\"\"" } else { "'''" };
                if line[index..].starts_with(delimiter) {
                    return Scan::Uninterpretable;
                }
                quote = Some(character);
            }
            None => {}
        }
    }
    match quote {
        Some(_) => Scan::Uninterpretable,
        None => Scan::Code(line),
    }
}

/// `line` without its trailing comment. A line that opens a multi-line string
/// or leaves a string unclosed is returned unchanged, never cut.
pub(crate) fn strip_toml_comment(line: &str) -> &str {
    match scan(line) {
        Scan::Code(code) => code,
        Scan::Uninterpretable => line,
    }
}

/// Refuses a line the line-based reader cannot interpret.
pub(super) fn reject_uninterpretable(
    path: &Path,
    line_no: usize,
    line: &str,
) -> anyhow::Result<()> {
    if scan(line) == Scan::Uninterpretable {
        bail!("{}:{} {UNINTERPRETABLE_LINE}", path.display(), line_no);
    }
    Ok(())
}

/// Refuses, before any key or preset is read, an uninterpretable line in any
/// existing config layer. A file that is not valid TOML, or cannot be read, is
/// left to the existing startup validation and its diagnostics.
pub(super) fn validate_config_files(root: &Path) -> anyhow::Result<()> {
    for path in super::config_paths(root) {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        if text.parse::<toml::Table>().is_err() {
            continue;
        }
        for (index, line) in text.lines().enumerate() {
            reject_uninterpretable(&path, index + 1, line)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{UNINTERPRETABLE_LINE, reject_uninterpretable, strip_toml_comment};

    /// The comment removal before Issue #941, which tracked basic strings only.
    fn basic_only(line: &str) -> &str {
        let mut quoted = false;
        let mut escaped = false;
        for (index, ch) in line.char_indices() {
            if escaped {
                escaped = false;
                continue;
            }
            match ch {
                '\\' if quoted => escaped = true,
                '"' => quoted = !quoted,
                '#' if !quoted => return &line[..index],
                _ => {}
            }
        }
        line
    }

    #[test]
    fn a_hash_inside_a_literal_string_is_not_a_comment() {
        assert_eq!(strip_toml_comment("x = 'a#b'"), "x = 'a#b'");
        assert_eq!(strip_toml_comment("x = 'a#b' # c"), "x = 'a#b' ");
        assert_eq!(
            strip_toml_comment("x = { only = ['deep#seek'] } # c"),
            "x = { only = ['deep#seek'] } "
        );
    }

    #[test]
    fn a_double_quote_inside_a_literal_string_does_not_open_a_basic_string() {
        assert_eq!(strip_toml_comment("x = 'a\"b' # c"), "x = 'a\"b' ");
        assert_eq!(strip_toml_comment("'a\"b' # c"), "'a\"b' ");
    }

    #[test]
    fn a_single_quote_inside_a_basic_string_does_not_open_a_literal_string() {
        assert_eq!(strip_toml_comment("x = \"a'b\" # c"), "x = \"a'b\" ");
        assert_eq!(strip_toml_comment("\"a'b\" # c"), "\"a'b\" ");
    }

    #[test]
    fn a_hash_and_an_equals_sign_inside_a_quoted_key_are_kept() {
        assert_eq!(
            strip_toml_comment("openrouter_provider.'MARKER#=0' = [\"a\"] # c"),
            "openrouter_provider.'MARKER#=0' = [\"a\"] "
        );
        assert_eq!(
            strip_toml_comment("\"k#=1\" = 'v#2' # c"),
            "\"k#=1\" = 'v#2' "
        );
        assert_eq!(
            strip_toml_comment("[preset.p.openrouter_provider.'MARKER#=0'] # c"),
            "[preset.p.openrouter_provider.'MARKER#=0'] "
        );
    }

    #[test]
    fn a_backslash_escapes_only_inside_a_basic_string() {
        assert_eq!(
            strip_toml_comment("x = \"a\\\"#b\" # c"),
            "x = \"a\\\"#b\" "
        );
        assert_eq!(strip_toml_comment("x = \"a\\\\\" # c"), "x = \"a\\\\\" ");
        // A literal string has no escapes: its `\` is a character and the
        // next `'` closes it.
        assert_eq!(strip_toml_comment("x = 'a\\' # c"), "x = 'a\\' ");
    }

    #[test]
    fn real_comments_blank_lines_and_comment_only_lines_are_unchanged() {
        for line in [
            "",
            "   ",
            "# only a comment",
            "   # indented comment with 'quote and \"quote",
            "x = 1 # trailing",
            "x = \"v\" # it's \"here\"",
            "x = \"a#b\" # c",
            "x = \"\" # empty",
            "x = '' # empty",
            "[preset.p] # c",
            "no comment at all",
            "x = \"unclosed",
        ] {
            assert_eq!(strip_toml_comment(line), basic_only(line), "{line:?}");
        }
    }

    #[test]
    fn a_multi_line_or_unclosed_string_line_is_never_cut() {
        for line in [
            "x = \"\"\"",
            "x = '''",
            "x = \"\"\"a # b\"\"\"",
            "x = '''a # b'''",
            "openrouter_provider.'MARKER#=0' = \"\"\"",
            "x = 'unclosed # c",
            "x = \"unclosed # c",
        ] {
            assert_eq!(strip_toml_comment(line), line, "{line:?}");
        }
    }

    #[test]
    fn an_uninterpretable_line_fails_with_only_the_location() {
        let path = Path::new("cfg/config.toml");
        for line in [
            "PRIVATE_KEY_941 = \"\"\"PRIVATE_VALUE_941",
            "PRIVATE_KEY_941 = '''PRIVATE_VALUE_941",
            "PRIVATE_KEY_941 = 'PRIVATE_VALUE_941 # c",
            "PRIVATE_KEY_941 = \"PRIVATE_VALUE_941 # c",
        ] {
            let error = reject_uninterpretable(path, 7, line).expect_err(line);
            assert_eq!(
                format!("{error:#}"),
                format!("cfg/config.toml:7 {UNINTERPRETABLE_LINE}"),
                "{line:?}"
            );
        }
    }

    #[test]
    fn an_interpretable_line_is_accepted() {
        let path = Path::new("cfg/config.toml");
        for line in [
            "",
            "# c",
            "x = 'a\"b' # c",
            "x = \"a'b\" # c",
            "x = \"a\\\"b\"",
            "x = \"\"",
            "x = ''",
            "x = 1 # it's '''",
            "x = 1 # \"\"\"",
        ] {
            assert!(reject_uninterpretable(path, 1, line).is_ok(), "{line:?}");
        }
    }

    #[test]
    fn the_fixed_diagnostic_carries_no_placeholder() {
        assert!(!UNINTERPRETABLE_LINE.contains('{'));
        assert!(UNINTERPRETABLE_LINE.contains("not shown"));
    }
}
