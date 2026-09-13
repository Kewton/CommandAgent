//! Closed source-read/includes/throw grammar. The whole program must match:
//! additional reads, catches, aliases, imports and opaque control flow refuse
//! attribution. Token offsets bind the runtime frame to the executed throw.
use regex::Regex;
use std::sync::LazyLock;

/// A complete single read/branch program: with empty output, its only nonzero
/// exit is the missing predicate. Read/runtime exceptions instead produce stderr.
pub(super) fn single_exit(command: &str) -> Option<(String, String)> {
    let rest = command.strip_prefix("node -p 'String(require(\"fs\").readFileSync(\"")?;
    let (path, rest) = rest.split_once("\")).includes(")?;
    crate::tools::path_guard::validate_workspace_relative(path).ok()?;
    if !path
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b"_./-".contains(&b))
        || !matches!(
            std::path::Path::new(path).extension()?.to_str()?,
            "js" | "jsx" | "ts" | "tsx"
        )
    {
        return None;
    }
    let literal = rest.strip_suffix(") ? true : process.exit(1)'")?;
    let attribute: String = serde_json::from_str(literal).ok()?;
    supported(&attribute).then(|| (attribute, path.trim_start_matches("./").into()))
}

pub(super) fn attribute(
    source: &str,
    error: &str,
    line: usize,
    column: usize,
) -> Option<(String, String)> {
    let mut parser = Parser::new(source)?;
    parser.take(&["const", "fs", "=", "require", "("])?;
    if !matches!(parser.literal()?.as_str(), "fs" | "node:fs") {
        return None;
    }
    parser.take(&[")", ";", "const"])?;
    let binding = parser.next()?.0;
    if !matches!(binding, "s" | "p" | "source") {
        return None;
    }
    parser.take(&["=", "fs", ".", "readFileSync", "("])?;
    let path = parser.literal()?;
    crate::tools::path_guard::validate_workspace_relative(&path).ok()?;
    if !matches!(
        std::path::Path::new(&path).extension()?.to_str()?,
        "tsx" | "jsx" | "ts" | "js"
    ) {
        return None;
    }
    parser.take(&[","])?;
    if parser.literal()? != "utf8" {
        return None;
    }
    parser.take(&[")", ";"])?;
    let mut selected = None;
    let mut checks = 0;
    while parser.peek() == Some("if") {
        parser.take(&["if", "(", "!", binding, ".", "includes", "("])?;
        let attribute = parser.literal()?;
        parser.take(&[")", ")", "throw"])?;
        let (_, offset) = parser.next().filter(|(token, _)| *token == "new")?;
        parser.take(&["Error", "("])?;
        let message = parser.literal()?;
        parser.take(&[")"])?;
        if parser.peek() == Some(";") {
            parser.next();
        } else if parser.peek().is_some() {
            return None;
        }
        checks += 1;
        let before = &source[..offset];
        let actual_line = before.bytes().filter(|b| *b == b'\n').count() + 1;
        let actual_column = before.rsplit('\n').next()?.encode_utf16().count() + 1;
        if (line, column) == (actual_line, actual_column)
            && error == format!("Error: {message}")
            && supported(&attribute)
        {
            selected = Some((attribute, path.trim_start_matches("./").to_owned()));
        }
    }
    if parser.peek() == Some("console") {
        parser.take(&["console", ".", "log", "("])?;
        if parser.literal()? != "ok" {
            return None;
        }
        parser.take(&[")"])?;
        if parser.peek() == Some(";") {
            parser.next();
        }
    }
    (checks > 0 && parser.peek().is_none()).then_some(selected)?
}

fn supported(attribute: &str) -> bool {
    attribute == "data-anvil-state"
        || ["primary", "input", "restart", "search", "submit"]
            .iter()
            .any(|action| attribute == format!("data-anvil-action=\"{action}\""))
}

struct Parser<'a> {
    tokens: std::vec::IntoIter<(&'a str, usize)>,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Option<Self> {
        static TOKENS: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(r#"(?s)\s+|/\*.*?\*/|//[^\n]*|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|[A-Za-z_$][A-Za-z0-9_$]*|."#).unwrap()
        });
        // Parsing work stays bounded independently of captured process output.
        if source.len() > 128_000 {
            return None;
        }
        let tokens = TOKENS
            .find_iter(source)
            .filter_map(|m| {
                let text = m.as_str();
                (!text.starts_with(char::is_whitespace)
                    && !text.starts_with("/*")
                    && !text.starts_with("//"))
                .then_some((text, m.start()))
            })
            .collect::<Vec<_>>();
        Some(Self {
            tokens: tokens.into_iter(),
        })
    }

    fn next(&mut self) -> Option<(&'a str, usize)> {
        self.tokens.next()
    }

    fn peek(&self) -> Option<&'a str> {
        self.tokens.as_slice().first().map(|(text, _)| *text)
    }

    fn take(&mut self, expected: &[&str]) -> Option<()> {
        for token in expected {
            if self.next()?.0 != *token {
                return None;
            }
        }
        Some(())
    }

    fn literal(&mut self) -> Option<String> {
        let text = self.next()?.0;
        let quote = text.chars().next()?;
        if !matches!(quote, '\'' | '"') || !text.ends_with(quote) {
            return None;
        }
        let mut out = String::new();
        let mut chars = text[1..text.len() - 1].chars();
        while let Some(ch) = chars.next() {
            out.push(if ch == '\\' {
                match chars.next()? {
                    ch @ ('\'' | '"' | '\\') => ch,
                    _ => return None,
                }
            } else if ch.is_control() {
                return None;
            } else {
                ch
            });
        }
        Some(out)
    }
}
