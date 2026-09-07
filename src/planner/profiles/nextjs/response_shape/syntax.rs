//! Small lexical view for proven local contracts, not a TypeScript type checker.
#[derive(Clone, Debug)]
pub(super) struct Token {
    pub(super) text: String,
    pub(super) quoted: bool,
}

pub(super) struct Source {
    pub(super) tokens: Vec<Token>,
    pub(super) pairs: Vec<Option<usize>>,
    pub(super) scopes: Vec<Option<usize>>,
}

impl Source {
    pub(super) fn parse(text: &str) -> Option<Self> {
        let mut chars = text.chars().peekable();
        let mut tokens = Vec::new();
        while let Some(ch) = chars.next() {
            if ch.is_whitespace() {
                continue;
            }
            if ch == '/' && chars.peek() == Some(&'/') {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
                continue;
            }
            if ch == '/' && chars.peek() == Some(&'*') {
                chars.next();
                loop {
                    if chars.next()? == '*' && chars.peek() == Some(&'/') {
                        chars.next();
                        break;
                    }
                }
                continue;
            }
            if ch == '/'
                && tokens.last().is_none_or(|t: &Token| {
                    !t.quoted
                        && matches!(
                            t.text.as_str(),
                            "=" | "(" | ":" | "," | "[" | "!" | "return"
                        )
                })
            {
                let mut class = false;
                loop {
                    match chars.next()? {
                        '\\' => {
                            chars.next()?;
                        }
                        '[' => class = true,
                        ']' => class = false,
                        '/' if !class => break,
                        '\n' | '\r' => return None,
                        _ => {}
                    }
                }
                while chars.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
                    chars.next();
                }
                tokens.push(Token {
                    text: String::new(),
                    quoted: true,
                });
                continue;
            }
            let quoted = matches!(ch, '\'' | '"' | '`');
            let mut value = String::new();
            if quoted {
                loop {
                    let c = chars.next()?;
                    if c == ch {
                        break;
                    }
                    if c == '\\' {
                        // Escaped or interpolated paths are not literal routes.
                        value.push(c);
                        value.push(chars.next()?);
                    } else {
                        value.push(c);
                    }
                }
            } else {
                value.push(ch);
                if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$') {
                    while chars
                        .peek()
                        .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '$'))
                    {
                        value.push(chars.next()?);
                    }
                }
            }
            tokens.push(Token {
                text: value,
                quoted,
            });
        }
        let mut source = Self {
            pairs: vec![None; tokens.len()],
            scopes: vec![None; tokens.len()],
            tokens,
        };
        let mut stack = Vec::new();
        let mut braces = Vec::new();
        for i in 0..source.tokens.len() {
            source.scopes[i] = braces.last().copied();
            if source.any(i, &["(", "[", "{"]) {
                stack.push(i);
                if source.is(i, "{") {
                    braces.push(i);
                }
            } else if source.any(i, &[")", "]", "}"]) {
                let start = stack.pop()?;
                let expected = match source.tokens[start].text.as_str() {
                    "(" => ")",
                    "[" => "]",
                    _ => "}",
                };
                if !source.is(i, expected) {
                    return None;
                }
                source.pairs[start] = Some(i);
                source.pairs[i] = Some(start);
                if source.is(i, "}") {
                    braces.pop();
                }
            }
        }
        stack.is_empty().then_some(source)
    }

    pub(super) fn is(&self, i: usize, text: &str) -> bool {
        self.tokens
            .get(i)
            .is_some_and(|t| !t.quoted && t.text == text)
    }

    pub(super) fn any(&self, i: usize, texts: &[&str]) -> bool {
        texts.iter().any(|text| self.is(i, text))
    }

    pub(super) fn seq(&self, i: usize, texts: &[&str]) -> bool {
        texts.iter().enumerate().all(|(j, t)| self.is(i + j, t))
    }

    pub(super) fn name(&self, i: usize) -> Option<&str> {
        let token = self.tokens.get(i)?;
        (!token.quoted
            && token
                .text
                .starts_with(|c: char| c.is_ascii_alphabetic() || c == '_' || c == '$'))
        .then_some(token.text.as_str())
    }

    pub(super) fn scope_end(&self, i: usize) -> usize {
        self.scopes[i]
            .and_then(|start| self.pairs[start])
            .unwrap_or(self.tokens.len())
    }

    /// Bound an arrow's assignment expression without borrowing a caller's
    /// next argument, enclosing conditional branch, or subsequent statement.
    pub(super) fn expression_end(&self, start: usize, end: usize) -> usize {
        let mut i = start;
        let mut conditionals = 0usize;
        while i < end {
            if self.any(i, &[",", ";", ")", "]", "}"]) || self.is(i, ":") && conditionals == 0 {
                return i;
            }
            if self.any(i, &["(", "[", "{"]) {
                i = self.pairs[i].unwrap_or(end);
            } else if self.is(i, "?")
                && !self.any(i + 1, &["?", "."])
                && (i == start || !self.is(i - 1, "?"))
            {
                conditionals += 1;
            } else if self.is(i, ":") {
                conditionals = conditionals.saturating_sub(1);
            }
            i += 1;
        }
        end
    }

    /// Split a list without borrowing commas from nested expressions.
    pub(super) fn items(&self, start: usize, end: usize) -> Vec<(usize, usize)> {
        let mut result = Vec::new();
        let mut begin = start;
        let mut i = start;
        while i < end {
            if self.is(i, ",") {
                result.push((begin, i));
                begin = i + 1;
            } else if self.any(i, &["(", "[", "{"]) {
                i = self.pairs[i].unwrap_or(end);
            }
            i += 1;
        }
        if begin < end {
            result.push((begin, end));
        }
        result
    }
}
