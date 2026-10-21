//! ANSI-C (`$'...'`) and locale (`$"..."`) quoting outside shell quotes
//! (Issue #575).
//!
//! The write-target inspector's lexical analysis in the parent module does not
//! know `$'...'` and `$"..."`: it keeps the `$` as an ordinary character and
//! strips the following quotes as ordinary quotes, so `$'tee'` becomes the word
//! `$tee` and the program, its prefixes, and its options are read wrong. The
//! shell expands those forms before the program runs, and the expansion cannot
//! be verified portably: the escape table differs between shell versions and
//! `$"..."` depends on the locale. Every `$'` or `$"` that sits outside a shell
//! quote is therefore refused as an unverifiable form.
//!
//! Quote handling mirrors the parent module's lexer (outside, `'...'`, `"..."`,
//! and `\` escaping), so a `$'` or `$"` inside a quote, after a backslash, or a
//! lone `$` stays allowed. The scan runs over the whole command before the
//! lexer so a mis-read `$'\''` cannot hide the commands that follow it.

/// Operation recorded for a command that spells `$'` or `$"` outside a shell
/// quote, whose expansion cannot be verified.
pub(super) const OPERATION: &str = "ANSI-C / locale quoting";

/// The quoting introducer found outside a shell quote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    /// ANSI-C quoting, `$'...'`.
    AnsiC,
    /// Locale translation quoting, `$"..."`.
    Locale,
}

impl Kind {
    /// The two-character introducer as it appears in the command.
    pub(super) fn introducer(self) -> &'static str {
        match self {
            Kind::AnsiC => "$'",
            Kind::Locale => "$\"",
        }
    }
}

/// Returns the first `$'` / `$"` introducer that sits outside a shell quote.
pub(super) fn outside_quotes(command: &str) -> Option<Kind> {
    let mut chars = command.chars().peekable();
    let mut single_quoted = false;
    let mut double_quoted = false;
    while let Some(ch) = chars.next() {
        if single_quoted {
            if ch == '\'' {
                single_quoted = false;
            }
            continue;
        }
        if double_quoted {
            match ch {
                '"' => double_quoted = false,
                '\\' => {
                    chars.next();
                }
                _ => {}
            }
            continue;
        }
        match ch {
            '\'' => single_quoted = true,
            '"' => double_quoted = true,
            '\\' => {
                chars.next();
            }
            '$' => match chars.peek() {
                Some('\'') => return Some(Kind::AnsiC),
                Some('"') => return Some(Kind::Locale),
                _ => {}
            },
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_c_quoting_outside_quotes_detection_table() {
        let cases: &[(&str, Option<Kind>)] = &[
            ("$'tee' /tmp/f", Some(Kind::AnsiC)),
            ("env $'tee' /tmp/f", Some(Kind::AnsiC)),
            ("env $'-S' 'tee /tmp/f'", Some(Kind::AnsiC)),
            ("$'\\x74ee' /tmp/f", Some(Kind::AnsiC)),
            ("$'t\\145e' /tmp/f", Some(Kind::AnsiC)),
            ("x$'tee' /tmp/f", Some(Kind::AnsiC)),
            ("echo $'\\'' ; tee /tmp/f", Some(Kind::AnsiC)),
            ("echo $'\\''\ntee sub/link/f", Some(Kind::AnsiC)),
            ("$'\\cX' /tmp/f", Some(Kind::AnsiC)),
            ("$''tee /tmp/f", Some(Kind::AnsiC)),
            ("printf $'a\\'b' > sub/link/f", Some(Kind::AnsiC)),
            ("X=$'tee'", Some(Kind::AnsiC)),
            ("cat $'.env'", Some(Kind::AnsiC)),
            ("$\"tee\" /tmp/f", Some(Kind::Locale)),
            ("$\"\"tee /tmp/f", Some(Kind::Locale)),
            // A closed quote must be seen as closed, so the `$'` after it is
            // outside; an escaped `"` inside a double quote must not close it.
            ("echo 'a' $'tee'", Some(Kind::AnsiC)),
            ("echo \"x\" $'tee'", Some(Kind::AnsiC)),
            (r#"echo "a\"" $'tee'"#, Some(Kind::AnsiC)),
            ("echo \"$'x'\"", None),
            ("echo \"$\\'x'\"", None),
            ("echo '$'", None),
            ("echo 'a$'", None),
            ("echo 'a' 'b'", None),
            ("echo \\$'x'", None),
            ("echo \"$\"", None),
            ("printf '%s\\n' x", None),
            ("printf 'a\\tb\\n'", None),
            ("cargo test", None),
            ("$", None),
            ("", None),
        ];
        for (command, expected) in cases {
            assert_eq!(outside_quotes(command), *expected, "command: {command}");
        }
    }

    #[test]
    fn ansi_c_quoting_kind_reports_its_introducer() {
        assert_eq!(Kind::AnsiC.introducer(), "$'");
        assert_eq!(Kind::Locale.introducer(), "$\"");
    }
}
