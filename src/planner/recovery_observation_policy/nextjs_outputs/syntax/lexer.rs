//! A bounded lexer, not a JS parser. Opaque expressions never grant authority.
use std::{collections::BTreeSet, iter::Peekable, str::Chars};

use super::Token;

mod scan;
mod template_reads;
#[cfg(test)]
mod tests;

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
    if depth > scan::MAX_DEPTH {
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
                separated_by_newline |= scan::skip_line_comment(chars);
                continue;
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                separated_by_newline |= scan::skip_block_comment(chars)?;
                continue;
            }
            '/' => {
                // Automatic semicolon insertion can turn this into a new
                // regexp statement. Without line-aware parsing, never guess.
                if separated_by_newline {
                    return None;
                }
                if scan::regex_position(&tokens)? {
                    let text = scan::regex(chars)?;
                    // Conservative invalidation also prevents a lexical ambiguity
                    // from concealing executable assignments to trusted names.
                    unsafe_names.extend(scan::words(&text));
                    tokens.push(Token::Opaque);
                } else {
                    tokens.push(Token::Symbol('/'));
                }
            }
            '<' if scan::jsx_ambiguity(chars.peek().copied(), &tokens) => {
                return None;
            } // JSX/type-assertion ambiguity is outside this recognizer.
            '\\' => return None, // Escaped executable identifiers are unsupported.
            '`' => {
                template(chars, depth, &mut unsafe_names, &mut basename_receivers)?;
                tokens.push(Token::Opaque);
            }
            '\'' | '"' => tokens.push(scan::string(chars, ch)?),
            ch if ch.is_ascii_alphabetic() || matches!(ch, '_' | '$') => {
                tokens.push(scan::word(chars, ch));
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

/// Consumes a template literal after its opening backtick, folding each
/// interpolation's names into the caller's sets.
fn template(
    chars: &mut Peekable<Chars<'_>>,
    depth: u8,
    unsafe_names: &mut BTreeSet<String>,
    basename_receivers: &mut BTreeSet<String>,
) -> Option<()> {
    loop {
        match chars.next()? {
            '\\' => {
                chars.next()?;
            }
            '`' => return Some(()),
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
                unsafe_names.extend(scan::referenced_names(&inner));
            }
            _ => {}
        }
    }
}
