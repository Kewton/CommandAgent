//! Backtick command substitution for the lexical Bash guards (Issue #581).
//!
//! The write-target inspector's lexical analysis did not know the backtick
//! command substitution (`` `...` ``): it kept the backquote as an ordinary
//! character, so the command inside was never inspected and a write (or a read)
//! through it escaped the workspace. The shell expands the substitution before
//! the program runs, and the expanded word cannot be proven to remain in the
//! Gate 1 workspace boundary, so every executed substitution is refused as an
//! unverifiable form — the same honest-failure shape as ANSI-C quoting (#575).
//!
//! [`in_executable_context`] is the one decision the lexer needs while it walks
//! the command: a backtick runs outside quotes and inside a double quote, and is
//! literal inside a single quote. [`opaque_expansion_with_backticks`] refuses a
//! backtick that sits behind a `$(`/`${` expansion, whose inner quoting the scan
//! does not model: a `` '`data`' `` inside `$(...)` is data in the shell, but the
//! form is refused rather than guessed at.
//!
//! The lexer elides comments and heredocs before it calls
//! [`opaque_expansion_with_backticks`], so that function reads the elided text
//! and does not track shell comments itself.

/// Operation recorded for a command that runs a backtick command substitution
/// whose expanded word cannot be verified to remain in the workspace.
pub(crate) const UNVERIFIABLE_OPERATION: &str = "unverifiable backtick command substitution";

/// The quoting context a backtick can sit in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Context {
    /// Outside every quote.
    Unquoted,
    /// Inside a double-quoted string, where command substitution still runs.
    DoubleQuoted,
    /// Inside a single-quoted string, where every character is literal.
    SingleQuoted,
}

/// Whether a backtick in `context` starts a command substitution the shell runs.
///
/// An unquoted or double-quoted backtick runs; a single-quoted backtick is a
/// literal character. Escaped backticks and backticks inside comments never
/// reach this check.
pub(crate) fn in_executable_context(context: Context) -> bool {
    matches!(context, Context::Unquoted | Context::DoubleQuoted)
}

/// Whether the elided `text` has an executed `$(`/`${` expansion with a backtick
/// behind it.
///
/// The expansion's own quoting is not analyzed — a single-quoted backtick inside
/// it is data in the shell — so the form is refused instead of guessed at. An
/// escaped backtick stays literal and is not counted.
pub(crate) fn opaque_expansion_with_backticks(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut index = 0usize;
    let mut single = false;
    let mut double = false;
    let mut opaque = false;
    while index < bytes.len() {
        let ch = bytes[index];
        if single {
            if opaque && ch == b'`' {
                return true;
            }
            if ch == b'\'' {
                single = false;
            }
            index += 1;
            continue;
        }
        if double {
            match ch {
                b'\\' => {
                    index += 2;
                    continue;
                }
                b'"' => {
                    double = false;
                    index += 1;
                    continue;
                }
                b'`' if opaque => return true,
                b'$' if matches!(bytes.get(index + 1).copied(), Some(b'(' | b'{')) => {
                    opaque = true;
                }
                _ => {}
            }
            index += 1;
            continue;
        }
        match ch {
            b'\\' => index += 2,
            b'\'' => {
                single = true;
                index += 1;
            }
            b'"' => {
                double = true;
                index += 1;
            }
            b'`' if opaque => return true,
            b'$' if matches!(bytes.get(index + 1).copied(), Some(b'(' | b'{')) => {
                opaque = true;
                index += 1;
            }
            _ => index += 1,
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backticks_context_is_executable_outside_single_quotes() {
        assert!(in_executable_context(Context::Unquoted));
        assert!(in_executable_context(Context::DoubleQuoted));
        assert!(!in_executable_context(Context::SingleQuoted));
    }

    #[test]
    fn backticks_only_refuses_an_execution_behind_an_opaque_expansion() {
        let cases: &[(&str, bool)] = &[
            // A bare backtick is the lexer's own concern, not this check.
            ("echo `x`", false),
            ("echo '`x`'", false),
            ("echo \"`x`\"", false),
            // A backtick behind `$(`/`${`, including a nested single quote.
            ("echo $(echo `x`)", true),
            ("echo \"$(echo `x`)\"", true),
            ("echo ${x}`y`", true),
            ("echo $(echo '`data`')", true),
            ("echo \"$(echo '`data`')\"", true),
            // An expansion with no backtick behind it is untouched.
            ("echo $(x)", false),
            ("echo ${x}", false),
            ("echo $(x)`y`", true),
            // A bare `$` is not an expansion, in any quoting context.
            ("echo $x `y`", false),
            ("echo \"$x `y`\"", false),
            // A backtick before the expansion, or in a single quote, is not.
            ("echo `x` $(y)", false),
            ("echo '$(x)`y`'", false),
            ("echo '$(x)' `y`", false),
            // An escaped introducer is not an expansion, and an escaped
            // backtick stays literal.
            ("echo \\$(x)`y`", false),
            ("echo $(x) \\`y\\`", false),
            ("cargo test", false),
            ("", false),
        ];
        for (text, expected) in cases {
            assert_eq!(
                opaque_expansion_with_backticks(text),
                *expected,
                "text: {text:?}"
            );
        }
    }

    // The tables below cover the branches of `opaque_expansion_with_backticks`
    // that the leader's mutation run left surviving or untested (Issue #581).
    // Each row's note names the line, the branch, and the mutant it kills. A row
    // that cannot kill a mutant is documented as equivalent or as the stricter
    // side (a mutation that refuses *more*, never fewer).

    /// Single-quoted branch: lines 61 (`opaque && ch == '`'`), 64 (`ch == '\''`),
    /// and 67 (`index += 1`).
    #[test]
    fn backticks_single_quote_branch_pins_the_result() {
        let cases: &[(&str, bool, &str)] = &[
            (
                "echo $(echo '`data`')",
                true,
                "line 61 true arm: opaque + single quote + backtick",
            ),
            (
                "echo $(echo 'x')",
                false,
                "line 61: `x` is not a backtick, so the `opaque &&` guard must hold",
            ),
            (
                "echo '`x`'",
                false,
                "line 61: opaque is false, so dropping the guard returns true wrongly",
            ),
            (
                "echo 'a$(y)`z`'",
                false,
                "line 64: single closes only on `'`; `==`->`!=` closes on `a`, lets `$(` set opaque, and returns true",
            ),
            (
                "echo 'a' $(x)`y`",
                true,
                "line 64: dropping the closing arm never re-enters the unquoted state, so `$(` stays data",
            ),
            (
                "echo '`x` $(y)`z`'",
                false,
                "line 67: one byte per step keeps every backtick inside the single quote",
            ),
        ];
        for (text, expected, note) in cases {
            assert_eq!(
                opaque_expansion_with_backticks(text),
                *expected,
                "{note}: {text:?}"
            );
        }
    }

    /// Double-quoted branch: lines 72/73 (`b'\\'` arm), 76/78 (`b'"'` arm),
    /// 81 (`b'`' if opaque`), 82 (`$(`/`${`), and 87 (`index += 1`). The issue's
    /// example `echo "a\"'" $(x)`y`` is the first row.
    #[test]
    fn backticks_double_quote_branch_pins_the_result() {
        let cases: &[(&str, bool, &str)] = &[
            (
                r#"echo "a\"'" $(x)`y`"#,
                true,
                "line 72/73: `\"` must stay inside the double quote; removing the arm or `+= 2`->`+= 1` closes it early and loses the `$(`+backtick refusal",
            ),
            (
                r#"echo "a\\" $(x)`y`"#,
                true,
                "line 72/73: `\\` skips the second backslash, the quote still closes cleanly",
            ),
            (
                r#"echo "a\"$(x)`y`""#,
                true,
                "line 72/82: `$(` after an escaped quote sets opaque",
            ),
            (
                r#"echo "a\"'`y`""#,
                false,
                "line 81: a backtick with no opaque expansion stays false even after `\"`",
            ),
            (
                r#"echo "a\"'$(x)`y`""#,
                true,
                "line 72/82: escaped quote, single quote, `$(` then backtick",
            ),
            (
                r#"echo "a\"${x}`y`""#,
                true,
                "line 82: `${` after an escaped quote sets opaque",
            ),
            (
                r#"echo "a\\${x}`y`""#,
                true,
                "line 72/82: `\\` then `${` then backtick",
            ),
            (
                r#"echo "a" '$(x)`y`'"#,
                false,
                "line 76/78: the closing `\"` must end the double quote before `'` starts a single quote; dropping the arm or `+= 1`->`-= 1` keeps opaque and returns true",
            ),
            (
                r#"echo "$(x)`y`""#,
                true,
                "line 81/82/87: `$(` and backtick inside double quotes",
            ),
            (
                r#"echo "${x}`y`""#,
                true,
                "line 82: `${` inside double quotes",
            ),
            (
                r#"echo "$(x)""#,
                false,
                "no backtick stays false even with an escaped-free expansion",
            ),
        ];
        for (text, expected, note) in cases {
            assert_eq!(
                opaque_expansion_with_backticks(text),
                *expected,
                "{note}: {text:?}"
            );
        }
    }

    /// Unquoted branch: lines 91 (`b'\\'`), 92/94 (`b'\''`), 96/98 (`b'"'`),
    /// 100 (`b'`' if opaque`), 101/103 (`$(`/`${`), and 105 (`_ => index += 1`).
    #[test]
    fn backticks_unquoted_branch_pins_the_result() {
        let cases: &[(&str, bool, &str)] = &[
            (
                "echo \\$(x)`y`",
                false,
                "line 91: `+= 2` skips the escaped `$`; `+= 1` or dropping the arm would read `$(` and return true",
            ),
            (
                "echo $(x)`y`",
                true,
                "line 100/101/105: `$(` sets opaque and the backtick refuses",
            ),
            ("echo ${x}`y`", true, "line 101: `${` sets opaque"),
            (
                "echo `x` $(y)",
                false,
                "line 100: a backtick before the expansion is not refused",
            ),
            (
                "echo 'a' $(x)`y`",
                true,
                "line 92/94: the single quote closes before `$(`",
            ),
            (
                r#"echo " '$(x)`y`'"#,
                true,
                "line 96/98: `\"` opens a double quote; dropping the arm starts a single quote and `$(` stays data",
            ),
            (
                "echo \\`x\\` $(y)",
                false,
                "line 91: escaped backticks stay literal and set no opaque",
            ),
            (
                "echo $x `y`",
                false,
                "line 101/105: a bare `$` is not an expansion",
            ),
            (
                "echo \"$x `y`\"",
                false,
                "line 82/105: a bare `$` inside double quotes is not an expansion",
            ),
        ];
        for (text, expected, note) in cases {
            assert_eq!(
                opaque_expansion_with_backticks(text),
                *expected,
                "{note}: {text:?}"
            );
        }
    }

    /// Forms with no backtick substitution must stay allowed (false), including
    /// the item-2 examples `$(echo 'x')`, `${x}`, and `"$(echo "a")"`.
    #[test]
    fn backticks_without_a_substitution_stay_allowed() {
        let cases: &[&str] = &[
            "$(echo 'x')",
            "${x}",
            r#"echo "$(echo "a")""#,
            r#"echo "$(echo \"a\")""#,
            "echo $(x)",
            "echo ${x}",
            r#"echo "$(x)""#,
            "echo $x",
            "echo $x `y`",
            "cargo test",
            "",
        ];
        for text in cases {
            assert!(
                !opaque_expansion_with_backticks(text),
                "no backtick substitution: {text:?}"
            );
        }
    }

    /// `in_executable_context` is a three-value decision; pin every value.
    #[test]
    fn backticks_context_decision_is_pinned() {
        assert!(in_executable_context(Context::Unquoted));
        assert!(in_executable_context(Context::DoubleQuoted));
        assert!(!in_executable_context(Context::SingleQuoted));
    }

    /// The unquoted `b'\\' => index += 2` arm (line 91). The surviving mutant
    /// `index *= 2` equals `+= 2` only at index 2; for a backslash at index >= 3
    /// it jumps to `2 * index`, skips the `$(` behind the escape, and misses the
    /// executing backtick. Each row moves the backslash so the jump lands on the
    /// `(` past the `$`, or further into the expansion, and the unmutated answer
    /// (true) flips. The last two rows pin the escape itself: one lands before
    /// the `$` (it does not kill `*= 2`, but `-= 2` underflows and panics at
    /// index 1 — the `\` is at index 1 there), and one escapes the `$` so no
    /// expansion is seen at all.
    #[test]
    fn backticks_unquoted_backslash_skip_pins_the_result() {
        let cases: &[(&str, bool, &str)] = &[
            (
                "abc\\x$(echo `y`)",
                true,
                "backslash at 3: `*= 2` lands on `(` past the `$`, opaque stays false",
            ),
            (
                "abcd\\x$(echo `y`)",
                true,
                "backslash at 4: `*= 2` lands inside `echo`, past `$(`",
            ),
            (
                "abcde\\x$(echo `y`)",
                true,
                "backslash at 5: `*= 2` lands past `$(`",
            ),
            (
                "abcdef\\x$(echo `y`)",
                true,
                "backslash at 6: `*= 2` lands past `$(`",
            ),
            (
                "abcdefg\\x$(echo `y`)",
                true,
                "backslash at 7: `*= 2` lands past `$(`",
            ),
            (
                "echo \\x $(echo `y`)",
                true,
                "backslash at 5 with a separator: `*= 2` lands past `$(`",
            ),
            (
                "echo ab\\c $(echo '`d`')",
                true,
                "backslash at 7 before a single-quoted backtick under `$(`: `*= 2` skips opaque",
            ),
            (
                "a\\x$(echo `y`)",
                true,
                "backslash at 1: `*= 2` lands before `$` (no kill here), but `-= 2` underflows and panics",
            ),
            (
                "abc\\$(x)`y`",
                false,
                "the `+= 2` must skip the escaped `$`; `+= 1` recognises `$(` and returns true",
            ),
        ];
        for (text, expected, note) in cases {
            assert_eq!(
                opaque_expansion_with_backticks(text),
                *expected,
                "{note}: {text:?}"
            );
        }
    }
}
