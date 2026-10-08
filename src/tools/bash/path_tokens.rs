//! Shell word boundaries for confinement, without executing shell expansions.
//!
//! [`read_words`] splits command text into words the way the shell lexes them,
//! keeping quotes, escapes, and adjacent word pieces so a candidate's literal
//! spelling survives. It records, per word, whether an executed expansion
//! (`$…`, `` `…` ``) or a glob/brace metacharacter is present, without running
//! any of them. The read-path judge in [`super::read_guard`] consumes the
//! metadata: a static word is a candidate, a supported glob/brace is expanded by
//! the existing write-side proof, and a word whose value cannot be determined is
//! refused. This module never returns a *result*; it only classifies.

/// One shell word with the metadata the read-path judge needs.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct Word {
    /// The word's literal text with quotes removed, escapes resolved, and
    /// adjacent quoted/unquoted pieces concatenated, matching the shell's word
    /// construction for the parts it does not expand.
    pub text: String,
    /// An executed expansion is present: an unquoted or double-quoted `$…` or
    /// `` `…` `` that the shell expands before running the command.
    pub dynamic: bool,
    /// Every executed expansion in the word is a simple `$NAME` / `${NAME}`
    /// reference. Command substitution, arithmetic, parameter operators, and
    /// special parameters clear it.
    pub simple_variable: bool,
    /// An unquoted glob or brace metacharacter (`* ? [ { }`) is present, so the
    /// shell may expand the word to a different set of names.
    pub glob: bool,
    /// A glob or brace metacharacter (`* ? [ ] { } ,`) is present but was
    /// quoted or escaped, so the shell keeps it literal. When such a word also
    /// carries an executed glob or brace, the two cannot be told apart after
    /// `text` drops the quoting, and the expansion proof would read a different
    /// set than the shell (Issue #582 review blocker).
    pub quoted_glob: bool,
}

impl Word {
    fn start() -> Self {
        Word {
            text: String::new(),
            dynamic: false,
            simple_variable: true,
            glob: false,
            quoted_glob: false,
        }
    }
}

/// Split a command into segments of words, tracking shell expansions without
/// running them. Returns `None` when the quoting cannot be read (an unterminated
/// quote), so the caller fails closed.
pub(super) fn read_words(command: &str) -> Option<Vec<Vec<Word>>> {
    let chars: Vec<char> = command.chars().collect();
    let mut segments: Vec<Vec<Word>> = Vec::new();
    let mut words: Vec<Word> = Vec::new();
    let mut word = Word::start();
    let mut has_word = false;
    let mut quote: Option<char> = None;
    let mut index = 0usize;

    while index < chars.len() {
        let ch = chars[index];
        if let Some(open) = quote {
            match (open, ch) {
                ('\'', '\'') => {
                    quote = None;
                    has_word = true;
                    index += 1;
                }
                ('\'', _) => {
                    if is_glob_brace_meta(ch) {
                        word.quoted_glob = true;
                    }
                    word.text.push(ch);
                    index += 1;
                }
                ('"', '"') => {
                    quote = None;
                    has_word = true;
                    index += 1;
                }
                ('"', '\\') => {
                    // Inside double quotes Bash only removes a backslash before
                    // `$`, `` ` ``, `"`, `\`, or newline.
                    if let Some(next) = chars.get(index + 1).copied() {
                        if next == '\n' {
                            index += 2;
                        } else {
                            if is_glob_brace_meta(next) {
                                word.quoted_glob = true;
                            }
                            if matches!(next, '$' | '`' | '"' | '\\') {
                                word.text.push(next);
                            } else {
                                word.text.push('\\');
                                word.text.push(next);
                            }
                            index += 2;
                        }
                    } else {
                        word.text.push('\\');
                        index += 1;
                    }
                }
                ('"', '$') | ('"', '`') => {
                    index = consume_expansion(&chars, index, &mut word);
                    has_word = true;
                }
                ('"', _) => {
                    if is_glob_brace_meta(ch) {
                        word.quoted_glob = true;
                    }
                    word.text.push(ch);
                    index += 1;
                }
                _ => unreachable!("quote state is single or double only"),
            }
            continue;
        }
        match ch {
            '\'' => {
                quote = Some('\'');
                has_word = true;
                index += 1;
            }
            '"' => {
                quote = Some('"');
                has_word = true;
                index += 1;
            }
            '\\' => {
                if let Some(next) = chars.get(index + 1).copied() {
                    if next != '\n' {
                        if is_glob_brace_meta(next) {
                            word.quoted_glob = true;
                        }
                        word.text.push(next);
                    }
                    index += 2;
                } else {
                    word.text.push('\\');
                    index += 1;
                }
                has_word = true;
            }
            '$' | '`' => {
                index = consume_expansion(&chars, index, &mut word);
                has_word = true;
            }
            '*' | '?' | '[' | '{' | '}' => {
                word.glob = true;
                word.text.push(ch);
                has_word = true;
                index += 1;
            }
            ';' | '|' | '&' | '<' | '>' | '(' | ')' | '\n' | '\r' => {
                flush(&mut words, &mut word, &mut has_word);
                if matches!(ch, ';' | '|' | '&') && chars.get(index + 1) == Some(&ch) {
                    index += 1;
                }
                segments.push(std::mem::take(&mut words));
                index += 1;
            }
            // Only the shell's default word separators end a word: a space and a
            // tab (the newline and carriage return are handled above). Every
            // other Unicode whitespace — an ideographic space, a no-break space —
            // is an ordinary character of the word, which is what the shell
            // reads (Issue #603 design 2).
            ' ' | '\t' => {
                flush(&mut words, &mut word, &mut has_word);
                index += 1;
            }
            other => {
                word.text.push(other);
                has_word = true;
                index += 1;
            }
        }
    }
    if quote.is_some() {
        return None;
    }
    flush(&mut words, &mut word, &mut has_word);
    segments.push(words);
    Some(segments)
}

fn flush(words: &mut Vec<Word>, word: &mut Word, has_word: &mut bool) {
    if *has_word {
        words.push(std::mem::take(word));
        *word = Word::start();
        *has_word = false;
    }
}

/// Consume one executed expansion starting at `index` (a `$` or a backtick),
/// appending its raw spelling to `word.text` and returning the index past it.
/// A `$` that does not introduce an expansion is a literal character.
fn consume_expansion(chars: &[char], index: usize, word: &mut Word) -> usize {
    let ch = chars[index];
    if ch == '`' {
        word.dynamic = true;
        word.simple_variable = false;
        let mut cursor = index + 1;
        while cursor < chars.len() {
            if chars[cursor] == '\\' && cursor + 1 < chars.len() {
                cursor += 2;
                continue;
            }
            if chars[cursor] == '`' {
                return cursor + 1;
            }
            cursor += 1;
        }
        return cursor;
    }
    // ch == '$'
    let Some(next) = chars.get(index + 1).copied() else {
        word.text.push('$');
        return index + 1;
    };
    match next {
        '{' => {
            let close = matching_brace(chars, index + 1).unwrap_or(chars.len() - 1);
            let content: String = chars[index + 2..close].iter().collect();
            word.dynamic = true;
            if !is_identifier(&content) {
                word.simple_variable = false;
            }
            for ch in &chars[index..=close.min(chars.len() - 1)] {
                word.text.push(*ch);
            }
            close + 1
        }
        '(' => {
            word.dynamic = true;
            word.simple_variable = false;
            let close = matching_paren(chars, index + 1).unwrap_or(chars.len() - 1);
            for ch in &chars[index..=close] {
                word.text.push(*ch);
            }
            close + 1
        }
        c if is_identifier_start(c) => {
            let mut cursor = index + 1;
            while cursor < chars.len() && is_identifier_continue(chars[cursor]) {
                cursor += 1;
            }
            word.dynamic = true;
            for ch in &chars[index..cursor] {
                word.text.push(*ch);
            }
            cursor
        }
        '?' => {
            // `$?` is the exit status. A simple echo may display it, so keep the
            // word "simple"; every other special parameter and arithmetic
            // expansion still clears the flag (Issue #582 review, false
            // rejection of `echo "EXIT_CODE=$?"`).
            word.dynamic = true;
            for ch in &chars[index..=index + 1] {
                word.text.push(*ch);
            }
            index + 2
        }
        c if c.is_ascii_digit() || matches!(c, '@' | '*' | '#' | '$' | '!' | '-') => {
            word.dynamic = true;
            word.simple_variable = false;
            for ch in &chars[index..=index + 1] {
                word.text.push(*ch);
            }
            index + 2
        }
        _ => {
            // A `$` that does not start an expansion is an ordinary character.
            word.text.push('$');
            index + 1
        }
    }
}

fn matching_brace(chars: &[char], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut cursor = open;
    while cursor < chars.len() {
        match chars[cursor] {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(cursor);
                }
            }
            _ => {}
        }
        cursor += 1;
    }
    None
}

fn matching_paren(chars: &[char], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut cursor = open;
    let mut quote: Option<char> = None;
    while cursor < chars.len() {
        let ch = chars[cursor];
        if let Some(open_quote) = quote {
            if ch == '\\' && open_quote == '"' {
                cursor += 2;
                continue;
            }
            if ch == open_quote {
                quote = None;
            }
            cursor += 1;
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(cursor);
                }
            }
            _ => {}
        }
        cursor += 1;
    }
    None
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    is_identifier_start(first) && chars.all(is_identifier_continue)
}

fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

/// Whether a character is a glob or brace metacharacter whose quoting the word
/// text cannot preserve.
fn is_glob_brace_meta(ch: char) -> bool {
    matches!(ch, '*' | '?' | '[' | ']' | '{' | '}' | ',')
}

/// Whether a static word names a literal path: it holds a `/` and no character
/// the shell reads as an operator, expansion, or quote.
pub(super) fn is_literal_path(word: &str) -> bool {
    word.contains('/')
        && !word.chars().any(|ch| {
            // Multiword arguments may themselves be executable programs, such
            // as sh -c payloads. Keep their conservative embedded inspection.
            ch.is_whitespace() || matches!(
                ch,
                '\'' | '"' | '`' | '$' | ';' | '|' | '&' | '<' | '>' | '(' | ')' | '=' | ':'
                    | '\\' | '\n' | '\r'
            )
        })
        // Brackets within a path component are ordinary filename characters.
        // A slash *inside* brackets may instead be an embedded absolute path,
        // such as an interpreter's list expression; retain that inspection.
        && word.split('/').all(|component| {
            component.chars().filter(|ch| *ch == '[').count()
                == component.chars().filter(|ch| *ch == ']').count()
        })
}

/// Whether a word names a static literal path once its interior whitespace is
/// removed (Issue #603). A word that still carries whitespace after
/// [`read_words`] ran was quoted or escaped into a single word, so its literal
/// spelling — not the split of its pieces — is the path. The same condition
/// judges the right-hand side of a `=` argument. The existing
/// [`is_literal_path`] is left unchanged, so the conservative multiword-argument
/// inspection still applies where it did.
pub(super) fn is_literal_path_allowing_whitespace(value: &str) -> bool {
    let stripped: String = value.chars().filter(|ch| !ch.is_whitespace()).collect();
    is_literal_path(&stripped)
}

/// Whether the word carries a glob or brace metacharacter the shell may expand.
pub(super) fn contains_glob_or_brace(value: &str) -> bool {
    value.chars().any(|ch| matches!(ch, '*' | '?' | '[' | '{'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(command: &str) -> Vec<String> {
        read_words(command)
            .unwrap_or_else(|| panic!("expected readable words: {command}"))
            .into_iter()
            .flatten()
            .map(|word| word.text)
            .collect()
    }

    #[test]
    fn keeps_relative_routes_as_single_words() {
        for command in [
            r#"cat "src/app/api/expenses/[id]/route.ts""#,
            r"cat src/app/api/expenses/\[id\]/route.ts",
            r#"cat src/app/api/expenses/'[id]'"/approve/route.ts""#,
            r#"cat 'src/app/api/expenses/[id]/reject/route.ts'"#,
        ] {
            let parsed = words(command);
            let route = parsed
                .get(1)
                .unwrap_or_else(|| panic!("{command}: {parsed:?}"));
            assert!(route.starts_with("src/"), "{command}: {parsed:?}");
            assert!(route.ends_with("/route.ts"), "{command}: {parsed:?}");
            assert!(!route.starts_with('/'), "{command}: {parsed:?}");
        }
    }

    #[test]
    fn splits_only_unquoted_unescaped_shell_operators() {
        assert_eq!(
            read_words(r#"cat '/dev/null;' /dev/null\; "/outside/[id]/file""#)
                .unwrap()
                .into_iter()
                .flatten()
                .map(|word| word.text)
                .collect::<Vec<_>>(),
            ["cat", "/dev/null;", "/dev/null;", "/outside/[id]/file"]
        );
    }

    #[test]
    fn preserves_quote_concatenation_and_shell_escape_rules() {
        assert_eq!(
            words("cat /ou\"tside\"/file /out\\side/other /out\\\nside/last"),
            ["cat", "/outside/file", "/outside/other", "/outside/last"]
        );
        assert_eq!(
            words(r#"cat "src/\[id\]/file" 'src/\[id\]/file'"#),
            ["cat", r"src/\[id\]/file", r"src/\[id\]/file"]
        );
    }

    #[test]
    fn marks_executed_expansions_and_literal_dollars() {
        let simple = &read_words("echo $HOME").unwrap()[0][1];
        assert!(simple.dynamic && simple.simple_variable);

        let braced = &read_words("echo ${HOME}").unwrap()[0][1];
        assert!(braced.dynamic && braced.simple_variable);

        let operator = &read_words("echo ${x:-y}").unwrap()[0][1];
        assert!(operator.dynamic && !operator.simple_variable);

        let arithmetic = &read_words("echo $((1+2))").unwrap()[0][1];
        assert!(arithmetic.dynamic && !arithmetic.simple_variable);

        let substitution = &read_words("echo $(x)").unwrap()[0][1];
        assert!(substitution.dynamic && !substitution.simple_variable);

        let backtick = &read_words("echo `x`").unwrap()[0][1];
        assert!(backtick.dynamic && !backtick.simple_variable);

        // A `$` that does not start an expansion is a literal character, so the
        // ANSI-C introducer inside double quotes stays a plain word.
        let literal = &read_words(r#"echo "$'x'""#).unwrap()[0][1];
        assert!(!literal.dynamic);
        assert_eq!(literal.text, "$'x'");

        let escaped = &read_words(r#"echo "\$(echo x)""#).unwrap()[0][1];
        assert!(!escaped.dynamic);
    }

    #[test]
    fn marks_only_unquoted_glob_and_brace_metacharacters() {
        assert!(read_words("cat lin*/secret").unwrap()[0][1].glob);
        assert!(read_words("cat *.txt").unwrap()[0][1].glob);
        assert!(read_words("cat {a,b}.rs").unwrap()[0][1].glob);
        // Brackets inside quotes are ordinary filename characters.
        assert!(!read_words(r#"cat "src/[id]/route.ts""#).unwrap()[0][1].glob);
        assert!(read_words("cat src/[id]/route.ts").unwrap()[0][1].glob);
        // An escaped metacharacter is literal.
        assert!(!read_words(r"cat src/\[id\]/route.ts").unwrap()[0][1].glob);
    }

    #[test]
    fn rejects_unreadable_quoting() {
        assert!(read_words("cat 'unterminated").is_none());
        assert!(read_words("cat \"unterminated").is_none());
    }

    #[test]
    fn literal_path_table() {
        assert!(is_literal_path("src/app/[id]/route.ts"));
        assert!(is_literal_path("sub/link/secret"));
        assert!(!is_literal_path("secret"));
        assert!(!is_literal_path("FILE=/outside/file"));
        assert!(!is_literal_path("sh -c /outside/file"));
    }

    /// The shell separates words on a space, a tab, and a newline only. An
    /// ideographic space and a no-break space stay inside the word, so a name
    /// Bash reads as one word is judged as one word (Issue #603 design 2).
    #[test]
    fn splits_only_space_tab_and_newline() {
        assert_eq!(
            words("cat sub/wide\u{3000}link/secret"),
            ["cat", "sub/wide\u{3000}link/secret"]
        );
        assert_eq!(
            words("cat sub/nb\u{a0}link/secret"),
            ["cat", "sub/nb\u{a0}link/secret"]
        );
        assert_eq!(words("cat a\tb"), ["cat", "a", "b"]);
        assert_eq!(words("cat a\nb"), ["cat", "a", "b"]);
    }

    /// The whitespace-allowing form accepts a quoted or escaped single word and a
    /// `=` right-hand side, but still refuses a multiword program, an option, and
    /// an assignment (Issue #603 designs 1 and 3).
    #[test]
    fn literal_path_allowing_whitespace_table() {
        assert!(is_literal_path_allowing_whitespace("sub/space link/secret"));
        assert!(is_literal_path_allowing_whitespace(
            "sub/wide\u{3000}link/secret"
        ));
        assert!(is_literal_path_allowing_whitespace(
            "sub/nb\u{a0}link/secret"
        ));
        assert!(is_literal_path_allowing_whitespace("sub/link/secret"));
        assert!(!is_literal_path_allowing_whitespace("cat a.txt"));
        assert!(!is_literal_path_allowing_whitespace("--features=gui"));
        assert!(!is_literal_path_allowing_whitespace("FILE=/outside/file"));
    }

    /// Exact tokenization of the shapes whose read refusal depends on one line
    /// of the splitter. Removing the double-quoted dynamic arm, mis-advancing a
    /// backslash index, or dropping a character after a separator all change
    /// these word lists, so each mutation turns one assertion red even when a
    /// desynced quote would still refuse the whole command downstream.
    #[test]
    fn keeps_expansions_and_escapes_whole() {
        // A `$`/backtick inside double quotes is an executed expansion.
        let quoted = read_words(r#"cat "$FILE""#).unwrap();
        assert_eq!(quoted[0][0].text, "cat");
        assert_eq!(quoted[0][1].text, "$FILE");
        assert!(quoted[0][1].dynamic && quoted[0][1].simple_variable);

        let substitution = read_words(r#"cat "a$(x)b""#).unwrap();
        assert_eq!(substitution[0][1].text, "a$(x)b");
        assert!(substitution[0][1].dynamic && !substitution[0][1].simple_variable);

        let operator = read_words(r#"cat "${x:-y}""#).unwrap();
        assert!(operator[0][1].dynamic && !operator[0][1].simple_variable);

        // A backslash inside double quotes (`\$`, `\"`, `\\`, `\`+newline).
        assert_eq!(
            words(r#"cat "a\$b" sub/link/secret"#),
            ["cat", "a$b", "sub/link/secret"]
        );
        assert_eq!(
            words(r#"cat "a\"b" sub/link/secret"#),
            ["cat", "a\"b", "sub/link/secret"]
        );
        assert_eq!(
            words("cat \"a\\\nb\" sub/link/secret"),
            ["cat", "ab", "sub/link/secret"]
        );
        assert_eq!(
            words(r#"cat "a\\b" sub/link/secret"#),
            ["cat", r"a\b", "sub/link/secret"]
        );

        // A backslash outside quotes.
        assert_eq!(words(r"cat sub/li\nk/secret"), ["cat", "sub/link/secret"]);
        assert_eq!(
            words(r"cat a\b sub/link/secret"),
            ["cat", "ab", "sub/link/secret"]
        );
        assert_eq!(words(r"cat sub/link\/secret"), ["cat", "sub/link/secret"]);

        // A separator with no following space must not drop a character.
        assert_eq!(words("true;../secret"), ["true", "../secret"]);
        assert_eq!(words("true&&../secret"), ["true", "../secret"]);
        assert_eq!(words("true||../secret"), ["true", "../secret"]);
        assert_eq!(words("true;/outside/x"), ["true", "/outside/x"]);
    }
}
