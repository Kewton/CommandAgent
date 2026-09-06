use std::collections::BTreeSet;

use super::Value;

#[derive(Debug)]
pub(super) enum Token {
    Word(String),
    String(String),
    Symbol(char),
    Opaque,
}

pub(super) struct Source {
    pub(super) tokens: Vec<Token>,
    pub(super) scopes: Vec<usize>,
    pub(super) parents: Vec<Option<usize>>,
    pub(super) unsafe_names: BTreeSet<String>,
}

impl Source {
    pub(super) fn parse(text: &str) -> Option<Self> {
        let tokens = lex(text)?;
        let mut source = Self {
            tokens,
            scopes: Vec::new(),
            parents: vec![None],
            unsafe_names: BTreeSet::new(),
        };
        let mut scope = 0;
        for i in 0..source.tokens.len() {
            source.scopes.push(scope);
            if source.is(i, "{") {
                source.parents.push(Some(scope));
                scope = source.parents.len() - 1;
            } else if source.is(i, "}") {
                scope = source.parents[scope]?;
            }
        }
        if scope != 0 {
            return None;
        }
        source.reject_ambiguous_bindings();
        Some(source)
    }

    pub(super) fn is(&self, i: usize, text: &str) -> bool {
        match self.tokens.get(i) {
            Some(Token::Word(word)) => word == text,
            Some(Token::Symbol(symbol)) => text.len() == 1 && text.starts_with(*symbol),
            _ => false,
        }
    }

    pub(super) fn sequence(&self, start: usize, values: &[&str]) -> bool {
        values
            .iter()
            .enumerate()
            .all(|(offset, value)| self.is(start + offset, value))
    }

    pub(super) fn identifier(&self, i: usize) -> Option<&str> {
        if let Some(Token::Word(name)) = self.tokens.get(i) {
            Some(name)
        } else {
            None
        }
    }

    fn import_statements(&self) -> Vec<(usize, usize, &str)> {
        let mut imports = Vec::new();
        for i in 0..self.tokens.len() {
            if !self.is(i, "import") || self.scopes[i] != 0 || self.is(i + 1, "(") {
                continue;
            }
            for end in i + 1..self.tokens.len() {
                if self.is(end, ";") {
                    break;
                }
                if let Token::String(specifier) = &self.tokens[end] {
                    if self.is(end - 1, "from") {
                        imports.push((i, end, specifier.as_str()));
                    }
                    break;
                }
            }
        }
        imports
    }

    pub(super) fn imports(&self) -> Vec<&str> {
        let mut imports = self
            .import_statements()
            .into_iter()
            .map(|(_, _, specifier)| specifier)
            .collect::<Vec<_>>();
        for i in 0..self.tokens.len() {
            if self.sequence(i, &["import", "("])
                && let Some(Token::String(specifier)) = self.tokens.get(i + 2)
                && self.is(i + 3, ")")
            {
                imports.push(specifier);
            }
        }
        imports
    }

    pub(super) fn builtin_bindings(&self) -> Vec<(String, Value, usize)> {
        let mut bindings = Vec::new();
        for (start, end, specifier) in self.import_statements() {
            let namespace = match specifier.strip_prefix("node:").unwrap_or(specifier) {
                "fs" => Value::Fs,
                "fs/promises" => Value::Promises,
                "path" => Value::PathModule,
                _ => continue,
            };
            if let Some(name) = self.identifier(start + 1) {
                if self.is(start + 2, "from") {
                    bindings.push((name.to_string(), namespace, start));
                }
            } else if self.sequence(start + 1, &["*", "as"]) {
                if let Some(name) = self.identifier(start + 3) {
                    bindings.push((name.to_string(), namespace, start));
                }
            } else if self.is(start + 1, "{") {
                let mut i = start + 2;
                while i < end && !self.is(i, "}") {
                    let Some(imported) = self.identifier(i) else {
                        break;
                    };
                    let value = match (&namespace, imported) {
                        (Value::Fs, "promises") => Some(Value::Promises),
                        (Value::Fs, "writeFile" | "writeFileSync")
                        | (Value::Promises, "writeFile") => Some(Value::Writer),
                        (Value::PathModule, "join") => Some(Value::Join),
                        _ => None,
                    };
                    let local = if self.is(i + 1, "as") { i + 2 } else { i };
                    if let (Some(name), Some(value)) = (self.identifier(local), value) {
                        bindings.push((name.to_string(), value, start));
                    }
                    i = local + 1;
                    if self.is(i, ",") {
                        i += 1;
                    } else {
                        break;
                    }
                }
            }
        }
        bindings
    }

    fn reject_ambiguous_bindings(&mut self) {
        let mut unsafe_names = BTreeSet::new();
        // Unknown imports can shadow process or a recognized Node import.
        let builtins = self.builtin_bindings();
        for (start, end, _) in self.import_statements() {
            for i in start + 1..end - 1 {
                if let Some(name) = self.identifier(i)
                    && !builtins
                        .iter()
                        .any(|(local, _, position)| local == name && *position == start)
                {
                    unsafe_names.insert(name.to_string());
                }
            }
        }
        let mut parens = Vec::new();
        for i in 0..self.tokens.len() {
            if self.is(i, "(") {
                parens.push(i);
            }
            if self.is(i, ")")
                && let Some(start) = parens.pop()
            {
                // Parameters (including typed function returns, methods, catch,
                // and arrows) are conservatively unknown throughout the file.
                if self.is(i + 1, "{") || self.is(i + 1, ":") || self.sequence(i + 1, &["=", ">"]) {
                    for j in start + 1..i {
                        if let Some(name) = self.identifier(j) {
                            unsafe_names.insert(name.to_string());
                        }
                    }
                }
            }
            if self.sequence(i + 1, &["=", ">"])
                && let Some(name) = self.identifier(i)
            {
                unsafe_names.insert(name.to_string());
            }
            if matches!(
                self.identifier(i),
                Some("let" | "var" | "function" | "class")
            ) && let Some(name) = self.identifier(i + 1)
            {
                unsafe_names.insert(name.to_string());
            }
            if matches!(self.identifier(i), Some("const" | "let" | "var"))
                && (self.is(i + 1, "{") || self.is(i + 1, "["))
            {
                for j in i + 2..self.tokens.len() {
                    if self.is(j, "=") || self.is(j, ";") {
                        break;
                    }
                    if let Some(name) = self.identifier(j) {
                        unsafe_names.insert(name.to_string());
                    }
                }
            }
            let Some(name) = self.identifier(i) else {
                continue;
            };
            if i > 0 && self.is(i - 1, "const") {
                continue;
            }
            let mut end = i + 1;
            while self.is(end, ".") && self.identifier(end + 1).is_some() {
                end += 2;
            }
            let assignment = self.is(end, "=") && !self.is(end + 1, "=") && !self.is(end + 1, ">");
            if assignment
                || self.is(end, "[")
                || self.sequence(end, &["+", "+"])
                || self.sequence(end, &["-", "-"])
                || ["+", "-", "*", "/", "?", "|", "&"]
                    .iter()
                    .any(|op| self.sequence(end, &[op, "="]))
            {
                unsafe_names.insert(name.to_string());
            }
        }
        self.unsafe_names = unsafe_names;
    }
}

fn lex(text: &str) -> Option<Vec<Token>> {
    let mut chars = text.chars().peekable();
    let mut tokens = Vec::new();
    while let Some(ch) = chars.next() {
        match ch {
            ch if ch.is_whitespace() => {}
            '/' if chars.peek() == Some(&'/') => {
                for ch in chars.by_ref() {
                    if ch == '\n' {
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                loop {
                    let ch = chars.next()?;
                    if ch == '*' && chars.peek() == Some(&'/') {
                        chars.next();
                        break;
                    }
                }
            }
            // Regex/division and templates are outside this recognizer. Refuse
            // the file rather than tokenize executable-looking literal text.
            '/' | '`' => return None,
            '\'' | '"' => {
                let mut value = String::new();
                let mut escaped = false;
                loop {
                    let next = chars.next()?;
                    if next == ch {
                        break;
                    }
                    if next == '\\' {
                        escaped = true;
                        chars.next()?;
                    } else {
                        value.push(next);
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
            ch => tokens.push(Token::Symbol(ch)),
        }
    }
    Some(tokens)
}
