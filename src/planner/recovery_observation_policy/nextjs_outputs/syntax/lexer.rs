//! A bounded lexer, not a JS parser. Opaque expressions never grant authority.
use std::{collections::BTreeSet, iter::Peekable, str::Chars};

use super::Token;

mod template_reads;

fn dynamic_evaluation(tokens: &[Token]) -> bool {
    tokens.iter().any(|token| {
        matches!(token, Token::Word(name) | Token::String(name)
        if matches!(name.as_str(), "eval" | "Function" | "constructor"))
    })
}

type Lexed = (Vec<Token>, BTreeSet<String>, BTreeSet<String>);

pub(super) fn lex(text: &str) -> Option<Lexed> {
    let result = scan(&mut text.chars().peekable(), false, 0)?;
    (!dynamic_evaluation(&result.0)).then_some(result)
}

fn scan(chars: &mut Peekable<Chars<'_>>, interpolation: bool, depth: u8) -> Option<Lexed> {
    if depth > 64 {
        return None;
    }
    let mut tokens = Vec::new();
    let mut unsafe_names = BTreeSet::new();
    let mut basename_receivers = BTreeSet::new();
    let mut braces = 0usize;
    let mut separated_by_newline = false;
    while let Some(ch) = chars.next() {
        match ch {
            ch if ch.is_whitespace() => {
                separated_by_newline |= matches!(ch, '\n' | '\r');
                continue;
            }
            '/' if chars.peek() == Some(&'/') => {
                for ch in chars.by_ref() {
                    if ch == '\n' {
                        separated_by_newline = true;
                        break;
                    }
                }
                continue;
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                loop {
                    let ch = chars.next()?;
                    separated_by_newline |= matches!(ch, '\n' | '\r');
                    if ch == '*' && chars.peek() == Some(&'/') {
                        chars.next();
                        break;
                    }
                }
                continue;
            }
            '/' => {
                // Automatic semicolon insertion can turn this into a new
                // regexp statement. Without line-aware parsing, never guess.
                if separated_by_newline {
                    return None;
                }
                if regex_position(&tokens)? {
                    let text = regex(chars)?;
                    // Conservative invalidation also prevents a lexical ambiguity
                    // from concealing executable assignments to trusted names.
                    unsafe_names.extend(words(&text));
                    tokens.push(Token::Opaque);
                } else {
                    tokens.push(Token::Symbol('/'));
                }
            }
            '<' if chars.peek().is_some_and(|ch| matches!(ch, '/' | '>'))
                || (chars.peek().is_some_and(|ch| ch.is_ascii_alphabetic())
                    && !matches!(tokens.last(), Some(Token::Word(word)) if !matches!(word.as_str(), "return" | "yield" | "await"))) =>
            {
                return None;
            } // JSX/type-assertion ambiguity is outside this recognizer.
            '\\' => return None, // Escaped executable identifiers are unsupported.
            '`' => {
                loop {
                    match chars.next()? {
                        '\\' => {
                            chars.next()?;
                        }
                        '`' => break,
                        '$' if chars.peek() == Some(&'{') => {
                            chars.next();
                            let (inner, names, receivers) = scan(chars, true, depth + 1)?;
                            // String arguments to dynamic evaluators can mutate
                            // identities absent from the visible Word tokens.
                            if dynamic_evaluation(&inner) {
                                return None;
                            }
                            unsafe_names.extend(names);
                            basename_receivers.extend(receivers);
                            if template_reads::is_process_id_read(&inner) {
                                continue;
                            }
                            if let Some(receiver) = template_reads::basename_receiver(&inner) {
                                basename_receivers.insert(receiver);
                                continue;
                            }
                            unsafe_names.extend(inner.iter().enumerate().filter_map(
                                |(i, token)| {
                                    // A property name such as ALLOWED_ROLES.join is
                                    // not a reference to the imported join binding.
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
                                },
                            ));
                        }
                        _ => {}
                    }
                }
                tokens.push(Token::Opaque);
            }
            '\'' | '"' => {
                let mut value = String::new();
                let mut escaped = false;
                loop {
                    match chars.next()? {
                        next if next == ch => break,
                        '\\' => {
                            escaped = true;
                            chars.next()?;
                        }
                        '\n' | '\r' => return None,
                        next => value.push(next),
                    }
                }
                tokens.push(if escaped {
                    Token::Opaque
                } else {
                    Token::String(value)
                });
            }
            ch if ch.is_ascii_alphabetic() || matches!(ch, '_' | '$') => {
                let mut word = ch.to_string();
                while chars
                    .peek()
                    .is_some_and(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$'))
                {
                    word.push(chars.next()?);
                }
                tokens.push(Token::Word(word));
            }
            '{' => {
                braces += 1;
                tokens.push(Token::Symbol(ch));
            }
            '}' if interpolation && braces == 0 => {
                return Some((tokens, unsafe_names, basename_receivers));
            }
            '}' => {
                braces = braces.checked_sub(1)?;
                tokens.push(Token::Symbol(ch));
            }
            ch => tokens.push(Token::Symbol(ch)),
        }
        separated_by_newline = false;
    }
    (!interpolation && braces == 0).then_some((tokens, unsafe_names, basename_receivers))
}

fn words(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|ch: char| !ch.is_ascii_alphanumeric() && !matches!(ch, '_' | '$'))
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
}

fn regex(chars: &mut Peekable<Chars<'_>>) -> Option<String> {
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

fn regex_position(tokens: &[Token]) -> Option<bool> {
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
        Token::Symbol(')') => {
            let mut nesting = 0;
            let mut control = false;
            for i in (0..tokens.len()).rev() {
                match &tokens[i] {
                    Token::Symbol(')') => nesting += 1,
                    Token::Symbol('(') => {
                        nesting -= 1;
                        if nesting == 0 {
                            control = matches!(i.checked_sub(1).and_then(|j| tokens.get(j)),
                                Some(Token::Word(word)) if matches!(word.as_str(), "if" | "while" | "for" | "with" | "switch" | "catch"));
                            control |= i >= 2
                                && matches!(&tokens[i - 1], Token::Word(word) if word == "await")
                                && matches!(&tokens[i - 2], Token::Word(word) if word == "for");
                            if i >= 2 && matches!(tokens[i - 2], Token::Symbol('.')) {
                                control = false;
                            }
                            break;
                        }
                    }
                    _ => {}
                }
            }
            control
        }
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
