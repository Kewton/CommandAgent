#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CodeLanguage {
    JavaScript,
    TypeScript,
    Tsx,
    Python,
    Rust,
    Bash,
    Json,
}

impl CodeLanguage {
    pub(super) fn from_fence(fence: &str) -> Option<Self> {
        let tag = fence
            .trim_start()
            .strip_prefix("```")?
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        match tag.as_str() {
            "js" | "javascript" => Some(Self::JavaScript),
            "ts" | "typescript" => Some(Self::TypeScript),
            "tsx" => Some(Self::Tsx),
            "python" | "py" => Some(Self::Python),
            "rust" | "rs" => Some(Self::Rust),
            "bash" | "sh" | "shell" => Some(Self::Bash),
            "json" => Some(Self::Json),
            _ => None,
        }
    }

    fn has_slash_comments(self) -> bool {
        matches!(
            self,
            Self::JavaScript | Self::TypeScript | Self::Tsx | Self::Rust
        )
    }

    fn has_hash_comments(self) -> bool {
        matches!(self, Self::Python | Self::Bash)
    }

    fn supports_backticks(self) -> bool {
        matches!(self, Self::JavaScript | Self::TypeScript | Self::Tsx)
    }

    fn is_keyword(self, word: &str) -> bool {
        match self {
            Self::JavaScript | Self::TypeScript | Self::Tsx => matches!(
                word,
                "as" | "async"
                    | "await"
                    | "break"
                    | "case"
                    | "catch"
                    | "class"
                    | "const"
                    | "continue"
                    | "default"
                    | "delete"
                    | "do"
                    | "else"
                    | "enum"
                    | "export"
                    | "extends"
                    | "false"
                    | "finally"
                    | "for"
                    | "from"
                    | "function"
                    | "if"
                    | "implements"
                    | "import"
                    | "in"
                    | "instanceof"
                    | "interface"
                    | "let"
                    | "new"
                    | "null"
                    | "of"
                    | "return"
                    | "static"
                    | "super"
                    | "switch"
                    | "this"
                    | "throw"
                    | "true"
                    | "try"
                    | "type"
                    | "typeof"
                    | "undefined"
                    | "var"
                    | "void"
                    | "while"
                    | "yield"
            ),
            Self::Python => matches!(
                word,
                "and"
                    | "as"
                    | "assert"
                    | "async"
                    | "await"
                    | "break"
                    | "class"
                    | "continue"
                    | "def"
                    | "del"
                    | "elif"
                    | "else"
                    | "except"
                    | "False"
                    | "finally"
                    | "for"
                    | "from"
                    | "global"
                    | "if"
                    | "import"
                    | "in"
                    | "is"
                    | "lambda"
                    | "None"
                    | "nonlocal"
                    | "not"
                    | "or"
                    | "pass"
                    | "raise"
                    | "return"
                    | "True"
                    | "try"
                    | "while"
                    | "with"
                    | "yield"
            ),
            Self::Rust => matches!(
                word,
                "as" | "async"
                    | "await"
                    | "break"
                    | "const"
                    | "continue"
                    | "crate"
                    | "dyn"
                    | "else"
                    | "enum"
                    | "extern"
                    | "false"
                    | "fn"
                    | "for"
                    | "if"
                    | "impl"
                    | "in"
                    | "let"
                    | "loop"
                    | "match"
                    | "mod"
                    | "move"
                    | "mut"
                    | "pub"
                    | "ref"
                    | "return"
                    | "self"
                    | "Self"
                    | "static"
                    | "struct"
                    | "super"
                    | "trait"
                    | "true"
                    | "type"
                    | "unsafe"
                    | "use"
                    | "where"
                    | "while"
            ),
            Self::Bash => matches!(
                word,
                "case"
                    | "do"
                    | "done"
                    | "elif"
                    | "else"
                    | "esac"
                    | "fi"
                    | "for"
                    | "function"
                    | "if"
                    | "in"
                    | "select"
                    | "then"
                    | "until"
                    | "while"
            ),
            Self::Json => matches!(word, "false" | "null" | "true"),
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct HighlightState {
    in_block_comment: bool,
}

impl HighlightState {
    pub(super) fn reset(&mut self) {
        self.in_block_comment = false;
    }
}

const KEYWORD_COLOR: &str = "\x1b[35m";
const STRING_COLOR: &str = "\x1b[33m";
const COMMENT_COLOR: &str = "\x1b[2m\x1b[34m";

pub(super) fn render(line: &str, language: CodeLanguage, state: &mut HighlightState) -> String {
    let mut out = String::with_capacity(line.len());
    let bytes = line.as_bytes();
    let mut cursor = 0usize;

    while cursor < bytes.len() {
        if state.in_block_comment {
            let end = find_bytes(&bytes[cursor..], b"*/")
                .map(|offset| cursor + offset + 2)
                .unwrap_or(bytes.len());
            push_colored(&mut out, &line[cursor..end], COMMENT_COLOR);
            state.in_block_comment = end == bytes.len() && !line[cursor..end].ends_with("*/");
            cursor = end;
            continue;
        }

        if language.has_slash_comments() && bytes[cursor..].starts_with(b"//") {
            push_colored(&mut out, &line[cursor..], COMMENT_COLOR);
            break;
        }
        if language.has_slash_comments() && bytes[cursor..].starts_with(b"/*") {
            let end = find_bytes(&bytes[cursor + 2..], b"*/")
                .map(|offset| cursor + 2 + offset + 2)
                .unwrap_or(bytes.len());
            push_colored(&mut out, &line[cursor..end], COMMENT_COLOR);
            state.in_block_comment = end == bytes.len() && !line[cursor..end].ends_with("*/");
            cursor = end;
            continue;
        }
        if language.has_hash_comments() && bytes[cursor] == b'#' {
            push_colored(&mut out, &line[cursor..], COMMENT_COLOR);
            break;
        }

        let quote = bytes[cursor];
        if (quote == b'\'' || quote == b'"' || (quote == b'`' && language.supports_backticks()))
            && let Some(end) = string_end(bytes, cursor, quote)
        {
            push_colored(&mut out, &line[cursor..end], STRING_COLOR);
            cursor = end;
            continue;
        }

        if is_identifier_start(bytes[cursor]) {
            let mut end = cursor + 1;
            while end < bytes.len() && is_identifier_continue(bytes[end]) {
                end += 1;
            }
            let word = &line[cursor..end];
            if language.is_keyword(word) {
                push_colored(&mut out, word, KEYWORD_COLOR);
            } else {
                out.push_str(word);
            }
            cursor = end;
            continue;
        }

        let ch = line[cursor..].chars().next().expect("cursor is in bounds");
        out.push(ch);
        cursor += ch.len_utf8();
    }

    out
}

fn push_colored(out: &mut String, text: &str, color: &str) {
    out.push_str(super::MD_RESET);
    out.push_str(color);
    out.push_str(text);
    out.push_str(super::MD_RESET);
    out.push_str(super::MD_CODE_FENCE_COLOR);
}

fn string_end(bytes: &[u8], start: usize, quote: u8) -> Option<usize> {
    let mut cursor = start + 1;
    let mut escaped = false;
    while cursor < bytes.len() {
        let byte = bytes[cursor];
        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == quote {
            return Some(cursor + 1);
        }
        cursor += 1;
    }
    None
}

fn is_identifier_start(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphabetic()
}

fn is_identifier_continue(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphanumeric()
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    (0..=haystack.len() - needle.len())
        .find(|&start| &haystack[start..start + needle.len()] == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_fence_tags_map_to_lightweight_lexers() {
        for (tag, expected) in [
            ("js", CodeLanguage::JavaScript),
            ("ts", CodeLanguage::TypeScript),
            ("tsx", CodeLanguage::Tsx),
            ("python", CodeLanguage::Python),
            ("rust", CodeLanguage::Rust),
            ("bash", CodeLanguage::Bash),
            ("json", CodeLanguage::Json),
        ] {
            assert_eq!(
                CodeLanguage::from_fence(&format!("```{tag}")),
                Some(expected)
            );
        }
        assert_eq!(CodeLanguage::from_fence("```unknown"), None);
        assert_eq!(CodeLanguage::from_fence("```"), None);
    }

    #[test]
    fn rust_block_comments_continue_across_lines() {
        let mut state = HighlightState::default();
        let first = render("let x = 1; /* open", CodeLanguage::Rust, &mut state);
        let second = render("closed */ let y = 2;", CodeLanguage::Rust, &mut state);
        assert!(first.contains(KEYWORD_COLOR));
        assert!(first.contains(COMMENT_COLOR));
        assert!(second.contains(COMMENT_COLOR));
        assert!(second.contains(KEYWORD_COLOR));
        assert!(!state.in_block_comment);
    }
}
