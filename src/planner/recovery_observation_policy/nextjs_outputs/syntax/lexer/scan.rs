//! Pure helpers for the lexer's ambiguous constructs: comments, strings,
//! regexps, template interpolations, JSX and slash disambiguation. Each helper
//! only reads its input; `None` means the construct is refused, never granted.
use std::{iter::Peekable, str::Chars};

use super::Token;

/// Deepest template interpolation nesting the lexer will follow.
pub(super) const MAX_DEPTH: u8 = 64;

/// Consumes a `//` comment and reports whether it ended at a newline.
pub(super) fn skip_line_comment(chars: &mut Peekable<Chars<'_>>) -> bool {
    chars.by_ref().any(|ch| ch == '\n')
}

/// Consumes a block comment after its opening `/*` and reports whether it
/// spanned a line break. An unterminated comment is refused.
pub(super) fn skip_block_comment(chars: &mut Peekable<Chars<'_>>) -> Option<bool> {
    let mut newline = false;
    loop {
        let ch = chars.next()?;
        newline |= matches!(ch, '\n' | '\r');
        if ch == '*' && chars.peek() == Some(&'/') {
            chars.next();
            return Some(newline);
        }
    }
}

/// JSX and type assertions are outside this recognizer, so a `<` that could
/// open one is refused rather than read as a comparison.
pub(super) fn jsx_ambiguity(next: Option<char>, tokens: &[Token]) -> bool {
    next.is_some_and(|ch| matches!(ch, '/' | '>'))
        || (next.is_some_and(|ch| ch.is_ascii_alphabetic())
            && !matches!(tokens.last(), Some(Token::Word(word)) if !matches!(word.as_str(), "return" | "yield" | "await")))
}

/// Reads a quoted string after its opening quote. An escape makes the value
/// opaque; a line break or missing close quote is refused.
pub(super) fn string(chars: &mut Peekable<Chars<'_>>, quote: char) -> Option<Token> {
    let mut value = String::new();
    let mut escaped = false;
    loop {
        match chars.next()? {
            next if next == quote => break,
            '\\' => {
                escaped = true;
                chars.next()?;
            }
            '\n' | '\r' => return None,
            next => value.push(next),
        }
    }
    Some(if escaped {
        Token::Opaque
    } else {
        Token::String(value)
    })
}

/// Reads an identifier whose first character has already been consumed.
pub(super) fn word(chars: &mut Peekable<Chars<'_>>, first: char) -> Token {
    let mut word = first.to_string();
    while let Some(ch) = chars.next_if(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$')) {
        word.push(ch);
    }
    Token::Word(word)
}

pub(super) fn words(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|ch: char| !ch.is_ascii_alphanumeric() && !matches!(ch, '_' | '$'))
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
}

/// Names an interpolation references as bindings. A property name such as
/// `ALLOWED_ROLES.join` is not a reference to the imported `join` binding.
pub(super) fn referenced_names(inner: &[Token]) -> impl Iterator<Item = String> + '_ {
    inner.iter().enumerate().filter_map(|(i, token)| {
        let property = matches!(
            i.checked_sub(1).and_then(|j| inner.get(j)),
            Some(Token::Symbol('.'))
        ) && !matches!(
            i.checked_sub(2).and_then(|j| inner.get(j)),
            Some(Token::Symbol('.'))
        );
        match token {
            Token::Word(name) if !property => Some(name.clone()),
            _ => None,
        }
    })
}

/// Reads a regexp body and flags after the opening `/`.
pub(super) fn regex(chars: &mut Peekable<Chars<'_>>) -> Option<String> {
    let mut class = false;
    let mut text = String::new();
    loop {
        let ch = chars.next()?;
        match ch {
            '\n' | '\r' => return None,
            '\\' => {
                text.push(ch);
                text.push(chars.next()?);
            }
            '[' => {
                class = true;
                text.push(ch);
            }
            ']' => {
                class = false;
                text.push(ch);
            }
            '/' if !class => break,
            _ => text.push(ch),
        }
    }
    while chars.peek().is_some_and(|ch| ch.is_ascii_alphabetic()) {
        text.push(chars.next()?);
    }
    Some(text)
}

/// Whether the `)` ending `tokens` closes the head of a control statement,
/// after which a `/` starts a regexp rather than dividing.
fn closes_control_head(tokens: &[Token]) -> bool {
    let mut nesting = 0;
    for i in (0..tokens.len()).rev() {
        match &tokens[i] {
            Token::Symbol(')') => nesting += 1,
            Token::Symbol('(') => {
                nesting -= 1;
                if nesting == 0 {
                    let mut control = matches!(i.checked_sub(1).and_then(|j| tokens.get(j)),
                        Some(Token::Word(word)) if matches!(word.as_str(), "if" | "while" | "for" | "with" | "switch" | "catch"));
                    control |= i >= 2
                        && matches!(&tokens[i - 1], Token::Word(word) if word == "await")
                        && matches!(&tokens[i - 2], Token::Word(word) if word == "for");
                    if i >= 2 && matches!(tokens[i - 2], Token::Symbol('.')) {
                        control = false;
                    }
                    return control;
                }
            }
            _ => {}
        }
    }
    false
}

/// Decides whether a `/` after `tokens` starts a regexp. `None` refuses the
/// ambiguous positions instead of guessing.
pub(super) fn regex_position(tokens: &[Token]) -> Option<bool> {
    let Some(last) = tokens.last() else {
        return Some(true);
    };
    Some(match last {
        Token::Word(word) if matches!(word.as_str(), "yield" | "await") => return None,
        Token::Word(word) => {
            !matches!(
                tokens.get(tokens.len().wrapping_sub(2)),
                Some(Token::Symbol('.'))
            ) && matches!(
                word.as_str(),
                "return"
                    | "throw"
                    | "case"
                    | "delete"
                    | "void"
                    | "typeof"
                    | "new"
                    | "in"
                    | "instanceof"
                    | "else"
                    | "do"
            )
        }
        Token::String(_) | Token::Opaque => false,
        // Object literals and blocks have different slash semantics. Refuse
        // either here rather than guess and conceal executable code.
        Token::Symbol('}' | '!') => return None,
        Token::Symbol('>')
            if !matches!(
                tokens.get(tokens.len().wrapping_sub(2)),
                Some(Token::Symbol('='))
            ) =>
        {
            return None;
        }
        Token::Symbol(')') => closes_control_head(tokens),
        Token::Symbol(']') => false,
        Token::Symbol(ch) if ch.is_ascii_digit() => false,
        Token::Symbol(ch @ ('+' | '-')) => {
            if matches!(tokens.get(tokens.len().wrapping_sub(2)), Some(Token::Symbol(previous)) if previous == ch)
            {
                return None;
            }
            true
        }
        Token::Symbol('.') => return None,
        Token::Symbol(_) => true,
    })
}
