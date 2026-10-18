//! Lexical pass for response_shape sources: text to tokens, no bracket structure.
use super::Token;

/// `None` means the text is not lexically closed (comment, string or regex).
pub(super) fn tokenize(text: &str) -> Option<Vec<Token>> {
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
    Some(tokens)
}
