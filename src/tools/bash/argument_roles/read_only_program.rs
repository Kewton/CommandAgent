//! Whole-program reading of the inline `sed` and `awk` programs whose words the
//! argument-role leaf may treat as data (Issue #930).
//!
//! Each reader parses the complete program text and answers one of three
//! verdicts. [`Verdict::ReadOnly`] means every command is in the small read-only
//! subset (sed: numeric, `$`, and `/regex/` addresses, a two-address range, `p`,
//! `d`, and `s` with the `g`/`p`/`I` flags; awk: `/regex/` or flag-variable
//! patterns, actions of integer flag assignments, `print`, and `next`).
//! [`Verdict::Effect`] means the parse found file I/O or command execution
//! (sed `w`/`W`/`r`/`R`/`e` and the `s` flags `w`/`e`; awk `getline`, `system`,
//! a pipe, an output redirect of `print`/`printf`, and `@` directives).
//! Everything else is [`Verdict::Unsupported`]. Only the first verdict lets a
//! word leave the path proof; a parse failure is never read as read-only, and
//! the I/O search is a parse, not a character search. An effect program keeps
//! the existing candidate reading of its word, and the files it names (sed
//! `w`/`W`/`r`/`R` and `s///w`, awk `> "file"`, `>> "file"`, and
//! `getline < "file"`) are returned as [`Targets`] for the existing proof.
//!
//! A regex is split at its first unescaped delimiter, as GNU sed does. A
//! bracket expression left open by that split, or one holding `[` or `\`, is
//! unsupported, so an implementation that reads the delimiter inside brackets
//! differently cannot see another command.

/// The reading of one inline program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Verdict {
    ReadOnly,
    Effect,
    Unsupported,
}

/// The files a program names for its own I/O. The caller adds them to the
/// existing path proof: reads as read candidates, writes as write targets.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct Targets {
    pub reads: Vec<String>,
    pub writes: Vec<String>,
}

/// Reads a `sed` script, with the files its `r`/`R` commands read and its
/// `w`/`W` commands and `s///w` flag write.
pub(super) fn sed(script: &str) -> (Verdict, Targets) {
    let mut parser = Sed {
        chars: script.chars().collect(),
        at: 0,
        subset: true,
        effect: false,
        commands: 0,
        targets: Targets::default(),
    };
    let parsed = parser.script();
    let verdict = if parser.effect {
        Verdict::Effect
    } else if parsed.is_ok() && parser.subset && parser.commands > 0 {
        Verdict::ReadOnly
    } else {
        Verdict::Unsupported
    };
    (verdict, parser.targets)
}

struct Sed {
    chars: Vec<char>,
    at: usize,
    /// Every command so far is in the read-only subset.
    subset: bool,
    /// A command or flag so far does file I/O or runs a command.
    effect: bool,
    commands: usize,
    targets: Targets,
}

impl Sed {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }

    fn next(&mut self) -> Result<char, Verdict> {
        let ch = self.peek().ok_or(Verdict::Unsupported)?;
        self.at += 1;
        Ok(ch)
    }

    fn skip_blanks(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t')) {
            self.at += 1;
        }
    }

    fn skip_digits(&mut self) -> bool {
        let start = self.at;
        while self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
            self.at += 1;
        }
        self.at > start
    }

    /// Skips to the end of the line (a newline or `;` when `semicolon`).
    fn skip_argument(&mut self, semicolon: bool) {
        while let Some(ch) = self.peek() {
            if ch == '\n' || (semicolon && ch == ';') {
                break;
            }
            self.at += 1;
        }
    }

    fn script(&mut self) -> Result<(), Verdict> {
        loop {
            while matches!(self.peek(), Some(' ' | '\t' | '\n' | ';')) {
                self.at += 1;
            }
            let Some(ch) = self.peek() else {
                return Ok(());
            };
            if ch == '#' {
                self.subset = false;
                self.skip_argument(false);
                continue;
            }
            if ch == '}' {
                self.subset = false;
                self.at += 1;
                self.end_of_command()?;
                continue;
            }
            self.address()?;
            self.skip_blanks();
            while self.peek() == Some('!') {
                self.subset = false;
                self.at += 1;
                self.skip_blanks();
            }
            self.command()?;
        }
    }

    /// An optional address or two-address range.
    fn address(&mut self) -> Result<(), Verdict> {
        if !self.single_address()? {
            return Ok(());
        }
        if self.peek() != Some(',') {
            return Ok(());
        }
        self.at += 1;
        self.skip_blanks();
        if matches!(self.peek(), Some('+' | '~')) {
            self.subset = false;
            self.at += 1;
            if !self.skip_digits() {
                return Err(Verdict::Unsupported);
            }
            return Ok(());
        }
        if !self.single_address()? {
            return Err(Verdict::Unsupported);
        }
        Ok(())
    }

    fn single_address(&mut self) -> Result<bool, Verdict> {
        match self.peek() {
            Some(ch) if ch.is_ascii_digit() => {
                self.skip_digits();
                if self.peek() == Some('~') {
                    self.subset = false;
                    self.at += 1;
                    if !self.skip_digits() {
                        return Err(Verdict::Unsupported);
                    }
                }
            }
            Some('$') => self.at += 1,
            Some('/') => {
                self.at += 1;
                self.regex('/')?;
                self.address_flags();
            }
            Some('\\') => {
                self.subset = false;
                self.at += 1;
                let delimiter = self.next()?;
                if matches!(delimiter, '\n' | '\\') {
                    return Err(Verdict::Unsupported);
                }
                self.regex(delimiter)?;
                self.address_flags();
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn address_flags(&mut self) {
        while matches!(self.peek(), Some('I' | 'M')) {
            self.subset = false;
            self.at += 1;
        }
    }

    /// Reads a regex up to its first unescaped `delimiter`.
    fn regex(&mut self, delimiter: char) -> Result<(), Verdict> {
        let mut content = String::new();
        loop {
            let ch = self.next()?;
            if ch == '\\' {
                let escaped = self.next()?;
                if escaped == '\n' {
                    return Err(Verdict::Unsupported);
                }
                content.push('\\');
                content.push(escaped);
                continue;
            }
            if ch == '\n' {
                return Err(Verdict::Unsupported);
            }
            if ch == delimiter {
                break;
            }
            content.push(ch);
        }
        if brackets_closed(&content) {
            Ok(())
        } else {
            Err(Verdict::Unsupported)
        }
    }

    /// Reads an `s` replacement up to its first unescaped `delimiter`.
    fn replacement(&mut self, delimiter: char) -> Result<(), Verdict> {
        loop {
            let ch = self.next()?;
            if ch == '\\' {
                self.next()?;
                continue;
            }
            if ch == '\n' {
                return Err(Verdict::Unsupported);
            }
            if ch == delimiter {
                return Ok(());
            }
        }
    }

    fn command(&mut self) -> Result<(), Verdict> {
        let ch = self.next()?;
        match ch {
            'p' | 'd' => {}
            's' => self.substitute()?,
            'y' => {
                self.subset = false;
                let delimiter = self.next()?;
                if matches!(delimiter, '\n' | '\\') {
                    return Err(Verdict::Unsupported);
                }
                self.replacement(delimiter)?;
                self.replacement(delimiter)?;
            }
            // The file name, or the command `e` runs, is the rest of the line.
            'w' | 'W' | 'r' | 'R' | 'e' => {
                self.effect = true;
                let argument = self.rest_of_line();
                match ch {
                    'w' | 'W' => self.targets.writes.extend(argument),
                    'r' | 'R' => self.targets.reads.extend(argument),
                    _ => {}
                }
                self.commands += 1;
                return Ok(());
            }
            // The text runs to the end of the line, so nothing after it on the
            // line is a command.
            'a' | 'i' | 'c' => {
                self.subset = false;
                while let Some(ch) = self.peek() {
                    self.at += 1;
                    if ch == '\\' {
                        self.at += 1;
                    } else if ch == '\n' {
                        break;
                    }
                }
                self.commands += 1;
                return Ok(());
            }
            ':' | 'b' | 't' | 'T' | 'v' => {
                self.subset = false;
                self.skip_blanks();
                self.skip_argument(true);
            }
            'q' | 'Q' | 'l' | 'L' => {
                self.subset = false;
                self.skip_blanks();
                self.skip_digits();
            }
            '=' | 'n' | 'N' | 'P' | 'D' | 'g' | 'G' | 'h' | 'H' | 'x' | 'z' | 'F' => {
                self.subset = false;
            }
            '{' => {
                self.subset = false;
                self.commands += 1;
                return Ok(());
            }
            _ => return Err(Verdict::Unsupported),
        }
        self.commands += 1;
        self.end_of_command()
    }

    fn end_of_command(&mut self) -> Result<(), Verdict> {
        self.skip_blanks();
        match self.peek() {
            None | Some(';' | '\n' | '}' | '#') => Ok(()),
            Some(_) => Err(Verdict::Unsupported),
        }
    }

    fn substitute(&mut self) -> Result<(), Verdict> {
        let delimiter = self.next()?;
        if matches!(delimiter, '\n' | '\\') {
            return Err(Verdict::Unsupported);
        }
        if !delimiter.is_ascii_punctuation() {
            self.subset = false;
        }
        self.regex(delimiter)?;
        self.replacement(delimiter)?;
        loop {
            match self.peek() {
                Some('g' | 'p' | 'I') => self.at += 1,
                Some('i' | 'm' | 'M') => {
                    self.subset = false;
                    self.at += 1;
                }
                Some(ch) if ch.is_ascii_digit() => {
                    self.subset = false;
                    self.at += 1;
                }
                Some('e') => {
                    self.effect = true;
                    self.at += 1;
                }
                Some('w') => {
                    self.effect = true;
                    self.at += 1;
                    let file = self.rest_of_line();
                    self.targets.writes.extend(file);
                    return Ok(());
                }
                _ => return Ok(()),
            }
        }
    }

    /// The rest of the line after leading blanks, consumed, or `None` when it
    /// is empty.
    fn rest_of_line(&mut self) -> Option<String> {
        self.skip_blanks();
        let start = self.at;
        self.skip_argument(false);
        let argument: String = self.chars[start..self.at].iter().collect();
        (!argument.is_empty()).then_some(argument)
    }
}

/// Whether every bracket expression in a regex closes, and none holds `[` or a
/// backslash, whose reading differs between implementations.
fn brackets_closed(regex: &str) -> bool {
    let chars: Vec<char> = regex.chars().collect();
    let mut at = 0usize;
    while at < chars.len() {
        match chars[at] {
            '\\' => at += 2,
            '[' => {
                at += 1;
                if chars.get(at) == Some(&'^') {
                    at += 1;
                }
                if chars.get(at) == Some(&']') {
                    at += 1;
                }
                loop {
                    match chars.get(at) {
                        None | Some('[' | '\\') => return false,
                        Some(']') => {
                            at += 1;
                            break;
                        }
                        Some(_) => at += 1,
                    }
                }
            }
            _ => at += 1,
        }
    }
    true
}

/// One awk token.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Regex,
    Str(String),
    Ident(String),
    Number(String),
    Dollar,
    Punct(&'static str),
    Newline,
}

/// Words awk reads as keywords, built-in functions, or reserved names. None of
/// them is a flag variable.
const AWK_RESERVED: &[&str] = &[
    "BEGIN",
    "END",
    "BEGINFILE",
    "ENDFILE",
    "function",
    "func",
    "getline",
    "print",
    "printf",
    "next",
    "nextfile",
    "exit",
    "return",
    "delete",
    "if",
    "else",
    "while",
    "for",
    "do",
    "in",
    "break",
    "continue",
    "switch",
    "case",
    "default",
    "system",
    "close",
    "fflush",
    "length",
    "substr",
    "index",
    "split",
    "sub",
    "gsub",
    "gensub",
    "match",
    "sprintf",
    "sin",
    "cos",
    "atan2",
    "exp",
    "log",
    "sqrt",
    "int",
    "rand",
    "srand",
    "tolower",
    "toupper",
    "strftime",
    "systime",
    "mktime",
    "and",
    "or",
    "xor",
    "lshift",
    "rshift",
    "compl",
    "ENVIRON",
    "PROCINFO",
    "ARGV",
    "ARGC",
];

/// Two-character awk operators, read before the one-character ones.
const AWK_PAIRS: &[&str] = &[
    ">>", ">=", "<=", "==", "!=", "&&", "||", "|&", "++", "--", "+=", "-=", "*=", "/=", "%=", "^=",
    "!~", "**",
];

/// One-character awk operators.
const AWK_SINGLES: &[&str] = &[
    "{", "}", "(", ")", "[", "]", ";", ",", "+", "-", "*", "/", "%", "^", "!", ">", "<", "|", "?",
    ":", "~", "=",
];

/// Reads an `awk` program.
pub(super) fn awk(program: &str) -> (Verdict, Targets) {
    let tokens = match awk_tokens(program) {
        Ok(tokens) => tokens,
        Err(verdict) => return (verdict, Targets::default()),
    };
    let (effect, targets) = awk_effect(&tokens);
    let verdict = if effect {
        Verdict::Effect
    } else if awk_subset(&tokens) {
        Verdict::ReadOnly
    } else {
        Verdict::Unsupported
    };
    (verdict, targets)
}

/// Whether a `/` after `previous` starts a regex rather than a division.
fn regex_allowed(previous: Option<&Token>) -> bool {
    match previous {
        None | Some(Token::Newline) => true,
        Some(Token::Punct(punct)) => !matches!(*punct, ")" | "]" | "++" | "--"),
        Some(Token::Ident(name)) => matches!(name.as_str(), "print" | "printf" | "return"),
        Some(_) => false,
    }
}

fn awk_tokens(program: &str) -> Result<Vec<Token>, Verdict> {
    let chars: Vec<char> = program.chars().collect();
    let mut tokens: Vec<Token> = Vec::new();
    let mut at = 0usize;
    while at < chars.len() {
        let ch = chars[at];
        match ch {
            ' ' | '\t' => at += 1,
            '\\' if chars.get(at + 1) == Some(&'\n') => at += 2,
            '\n' => {
                tokens.push(Token::Newline);
                at += 1;
            }
            '#' => {
                while at < chars.len() && chars[at] != '\n' {
                    at += 1;
                }
            }
            '@' => return Err(Verdict::Effect),
            '"' => {
                at += 1;
                let mut text = String::new();
                loop {
                    match chars.get(at) {
                        None | Some('\n') => return Err(Verdict::Unsupported),
                        Some('"') => {
                            at += 1;
                            break;
                        }
                        Some('\\') => {
                            let escaped = chars.get(at + 1).ok_or(Verdict::Unsupported)?;
                            text.push('\\');
                            text.push(*escaped);
                            at += 2;
                        }
                        Some(other) => {
                            text.push(*other);
                            at += 1;
                        }
                    }
                }
                tokens.push(Token::Str(text));
            }
            '/' if regex_allowed(tokens.last()) => {
                at += 1;
                let mut content = String::new();
                loop {
                    match chars.get(at) {
                        None | Some('\n') => return Err(Verdict::Unsupported),
                        Some('/') => {
                            at += 1;
                            break;
                        }
                        Some('\\') => {
                            let escaped = chars.get(at + 1).ok_or(Verdict::Unsupported)?;
                            content.push('\\');
                            content.push(*escaped);
                            at += 2;
                        }
                        Some(other) => {
                            content.push(*other);
                            at += 1;
                        }
                    }
                }
                if !brackets_closed(&content) {
                    return Err(Verdict::Unsupported);
                }
                tokens.push(Token::Regex);
            }
            '$' => {
                tokens.push(Token::Dollar);
                at += 1;
            }
            _ if ch.is_ascii_digit()
                || (ch == '.' && chars.get(at + 1).is_some_and(char::is_ascii_digit)) =>
            {
                let start = at;
                while at < chars.len() && (chars[at].is_ascii_digit() || chars[at] == '.') {
                    at += 1;
                }
                tokens.push(Token::Number(chars[start..at].iter().collect()));
            }
            _ if ch == '_' || ch.is_ascii_alphabetic() => {
                let start = at;
                while at < chars.len() && (chars[at] == '_' || chars[at].is_ascii_alphanumeric()) {
                    at += 1;
                }
                tokens.push(Token::Ident(chars[start..at].iter().collect()));
            }
            _ => {
                let pair: String = chars[at..(at + 2).min(chars.len())].iter().collect();
                if let Some(punct) = AWK_PAIRS.iter().find(|candidate| **candidate == pair) {
                    tokens.push(Token::Punct(punct));
                    at += 2;
                } else if let Some(punct) = AWK_SINGLES
                    .iter()
                    .find(|candidate| candidate.starts_with(ch) && candidate.len() == ch.len_utf8())
                {
                    tokens.push(Token::Punct(punct));
                    at += 1;
                } else {
                    return Err(Verdict::Unsupported);
                }
            }
        }
    }
    Ok(tokens)
}

/// Whether the tokens hold file I/O or command execution: `getline`,
/// `system`, a pipe, or an output redirect of `print`/`printf` (a `>` or `>>`
/// outside parentheses in the statement). A literal string after such a `>`
/// or `>>` is a write target, and one after the `<` of a `getline` is a read
/// target.
fn awk_effect(tokens: &[Token]) -> (bool, Targets) {
    let mut effect = false;
    let mut targets = Targets::default();
    let mut in_print = false;
    let mut in_getline = false;
    let mut depth = 0usize;
    for (at, token) in tokens.iter().enumerate() {
        let literal = match tokens.get(at + 1) {
            Some(Token::Str(text)) if !text.contains('\\') => Some(text.clone()),
            _ => None,
        };
        match token {
            Token::Ident(name) if name == "getline" => {
                effect = true;
                in_getline = true;
            }
            Token::Ident(name) if name == "system" => effect = true,
            Token::Punct("|" | "|&") => effect = true,
            Token::Ident(name) if matches!(name.as_str(), "print" | "printf") => {
                in_print = true;
                depth = 0;
            }
            Token::Punct("(" | "[") if in_print => depth += 1,
            Token::Punct(")" | "]") if in_print => depth = depth.saturating_sub(1),
            Token::Punct(">" | ">>") if in_print && depth == 0 => {
                effect = true;
                targets.writes.extend(literal);
            }
            Token::Punct("<") if in_getline => targets.reads.extend(literal),
            Token::Punct(";" | "{" | "}") | Token::Newline => {
                in_print = false;
                in_getline = false;
            }
            _ => {}
        }
    }
    (effect, targets)
}

fn is_flag_variable(token: &Token) -> bool {
    matches!(token, Token::Ident(name) if !AWK_RESERVED.contains(&name.as_str()))
}

fn ends_statement(token: Option<&Token>) -> bool {
    matches!(token, None | Some(Token::Newline | Token::Punct(";" | "}")))
}

/// Whether the tokens form the read-only subset: rules of a `/regex/` or flag
/// variable pattern with an optional action, or an action alone, where an
/// action holds `next`, `print` of fields/variables/strings/numbers, and
/// integer flag assignments.
fn awk_subset(tokens: &[Token]) -> bool {
    let mut at = 0usize;
    let mut rules = 0usize;
    loop {
        while matches!(tokens.get(at), Some(Token::Newline | Token::Punct(";"))) {
            at += 1;
        }
        let Some(token) = tokens.get(at) else {
            return rules > 0;
        };
        rules += 1;
        let pattern = matches!(token, Token::Regex) || is_flag_variable(token);
        if pattern {
            at += 1;
            if tokens.get(at) != Some(&Token::Punct("{")) {
                if !matches!(
                    tokens.get(at),
                    None | Some(Token::Newline | Token::Punct(";"))
                ) {
                    return false;
                }
                continue;
            }
        } else if token != &Token::Punct("{") {
            return false;
        }
        // The action, from `{` to its `}`.
        at += 1;
        loop {
            while matches!(tokens.get(at), Some(Token::Newline | Token::Punct(";"))) {
                at += 1;
            }
            match tokens.get(at) {
                Some(Token::Punct("}")) => {
                    at += 1;
                    break;
                }
                Some(Token::Ident(name)) if name == "next" => at += 1,
                Some(Token::Ident(name)) if name == "print" => {
                    at += 1;
                    if !ends_statement(tokens.get(at)) {
                        loop {
                            let Some(next) = print_argument(tokens, at) else {
                                return false;
                            };
                            at = next;
                            if tokens.get(at) == Some(&Token::Punct(",")) {
                                at += 1;
                            } else {
                                break;
                            }
                        }
                    }
                }
                Some(token) if is_flag_variable(token) => {
                    let assigns_integer = tokens.get(at + 1) == Some(&Token::Punct("="))
                        && matches!(
                            tokens.get(at + 2),
                            Some(Token::Number(number)) if number.chars().all(|ch| ch.is_ascii_digit())
                        );
                    if !assigns_integer {
                        return false;
                    }
                    at += 3;
                }
                _ => return false,
            }
            if !ends_statement(tokens.get(at)) {
                return false;
            }
        }
    }
}

/// One `print` argument: a field (`$N`, `$NF`), a variable, a string without
/// escapes, or a number. Returns the index after it.
fn print_argument(tokens: &[Token], at: usize) -> Option<usize> {
    match tokens.get(at)? {
        Token::Dollar => match tokens.get(at + 1)? {
            Token::Number(number) if number.chars().all(|ch| ch.is_ascii_digit()) => Some(at + 2),
            Token::Ident(name) if name == "NF" => Some(at + 2),
            _ => None,
        },
        Token::Str(text) if !text.contains('\\') => Some(at + 1),
        Token::Number(_) => Some(at + 1),
        token if is_flag_variable(token) => Some(at + 1),
        _ => None,
    }
}
