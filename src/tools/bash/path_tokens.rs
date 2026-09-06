//! Shell word boundaries for confinement, without executing shell expansions.

pub(super) fn path_candidates(command: &str) -> Vec<String> {
    let Some(words) = literal_shell_words(command) else {
        // Expansion syntax and malformed input must not disable the old,
        // conservative inspection of paths embedded in executable text.
        return embedded_candidates(command);
    };
    words
        .into_iter()
        .flat_map(|word| {
            let mut candidates = Vec::new();
            if word.starts_with('/') {
                // A quoted operator or bracket is part of the actual filename.
                // Always inspect the complete absolute word before any fallback.
                candidates.push(word.clone());
            }
            if is_literal_path(&word) {
                if !word.starts_with('/') {
                    candidates.push(word.clone());
                }
            } else {
                candidates.extend(embedded_candidates(&word));
            }
            candidates.dedup();
            candidates
        })
        .collect()
}

fn embedded_candidates(text: &str) -> Vec<String> {
    super::absolute_path_candidates(text)
        .map(str::to_owned)
        .collect()
}

fn is_literal_path(word: &str) -> bool {
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

fn literal_shell_words(command: &str) -> Option<Vec<String>> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut chars = command.chars().peekable();
    while let Some(ch) = chars.next() {
        match (quote, ch) {
            (Some('\''), '\'') | (Some('"'), '"') => quote = None,
            (Some('\''), _) => word.push(ch),
            (_, '$' | '`') => return None,
            // Unquoted glob patterns can select different paths (including
            // symlinks) from their literal spelling. Keep prior inspection.
            (None, '*' | '?' | '[' | ']') => return None,
            (_, '\\') => {
                let next = chars.next()?;
                if next != '\n' {
                    // Inside double quotes, Bash only removes backslashes
                    // before $, `, ", \, or newline.
                    if quote == Some('"') && !matches!(next, '$' | '`' | '"' | '\\') {
                        word.push('\\');
                    }
                    word.push(next);
                }
            }
            (None, '\'' | '"') => quote = Some(ch),
            (None, ';' | '|' | '&' | '<' | '>' | '(' | ')') => {
                words.push(std::mem::take(&mut word));
            }
            (None, ch) if ch.is_whitespace() => {
                words.push(std::mem::take(&mut word));
            }
            _ => word.push(ch),
        }
    }
    if quote.is_some() {
        return None;
    }
    words.push(word);
    Some(words)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_relative_routes_as_single_words() {
        for command in [
            r#"cat "src/app/api/expenses/[id]/route.ts""#,
            r"cat src/app/api/expenses/\[id\]/route.ts",
            r#"cat src/app/api/expenses/'[id]'"/approve/route.ts""#,
            r#"cat 'src/app/api/expenses/[id]/reject/route.ts'"#,
        ] {
            let candidates = path_candidates(command);
            assert!(!candidates.is_empty(), "{command}");
            assert!(candidates[0].starts_with("src/"), "{command}");
            assert!(candidates[0].ends_with("/route.ts"), "{command}");
            assert!(
                candidates.iter().all(|path| !path.starts_with('/')),
                "{command}"
            );
        }
    }

    #[test]
    fn splits_only_unquoted_unescaped_shell_operators() {
        for operator in [";", "&&", "||", "|", "&", "\n", "\r\n"] {
            assert_eq!(
                path_candidates(&format!("ls 2>/dev/null{operator}cat /outside/file")),
                ["/dev/null", "/outside/file"],
            );
        }
        assert_eq!(
            literal_shell_words(r#"cat '/dev/null;' /dev/null\; "/outside/[id]/file""#).unwrap(),
            ["cat", "/dev/null;", "/dev/null;", "/outside/[id]/file"]
        );
        assert_eq!(
            path_candidates(r#"cat '/dev/null;' /dev/null\;"#),
            ["/dev/null;", "/dev/null;"]
        );
    }

    #[test]
    fn preserves_quote_concatenation_and_shell_escape_rules() {
        assert_eq!(
            path_candidates("cat /ou\"tside\"/file /out\\side/other /out\\\nside/last"),
            ["/outside/file", "/outside/other", "/outside/last"]
        );
        assert_eq!(
            literal_shell_words(r#"cat "src/\[id\]/file" 'src/\[id\]/file'"#).unwrap(),
            ["cat", r"src/\[id\]/file", r"src/\[id\]/file"]
        );
    }

    #[test]
    fn retains_embedded_absolute_path_detection() {
        for command in [
            r#"sh -c 'cat /outside/file'"#,
            r#"sh -c '/bin/cat /outside/file'"#,
            r#"sh -c ' /outside/file'"#,
            r#"python -c "open('/outside/file')""#,
            r#"python -c "paths=['/outside/file']""#,
            r#"printf '%s' "$(cat /outside/file)""#,
            "cat `cat /outside/file`",
            "cat --input=/outside/file",
            "FILE=/outside/file cat file",
            "cat host:/outside/file",
            "cat [/outside/file]",
            "cat ]/outside/file",
            "cat '/outside/file",
            "sh -c 'cat src/[id]/outside/file'",
        ] {
            assert!(
                path_candidates(command)
                    .iter()
                    .any(|path| path == "/outside/file"),
                "{command}: {:?}",
                path_candidates(command)
            );
        }
    }
}
