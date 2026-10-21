//! Closed package-value observation/refinement grammar. These are executed
//! structural checks, never business assertions. No JavaScript equivalence guess.
use super::{import_check, single_command_words};

pub(crate) fn identifier(value: &str) -> bool {
    value.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !matches!(value, "constructor" | "prototype" | "__proto__")
}

pub(crate) fn literal(value: &str) -> bool {
    !value.is_empty()
        && value.trim() == value
        && !value.contains("  ")
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b" _./:=@+-".contains(&b))
}

pub(crate) fn printed_script(command: &str) -> Option<String> {
    let words = single_command_words(command)?;
    let [program, flag, source] = words.as_slice() else {
        return None;
    };
    let script = source.strip_prefix("require('./package.json').scripts.")?;
    (program == "node" && matches!(flag.as_str(), "-p" | "--print") && identifier(script))
        .then(|| script.into())
}

pub(crate) fn comparison(command: &str) -> Option<(String, String)> {
    let source = import_check::source(command)?;
    let rest = source.strip_prefix("const actual=require('./package.json').scripts.")?;
    let (script, rest) = rest.split_once(";require('node:assert/strict').strictEqual(actual,")?;
    let expected: String =
        serde_json::from_str(rest.strip_suffix(");console.log(actual)")?).ok()?;
    (identifier(script) && literal(&expected)).then(|| (script.into(), expected))
}

pub(crate) fn formed_command(script: &str, expected: &str) -> String {
    // Both inputs have already passed the bounded identifier/literal grammar;
    // JSON quotes are escaped for the outer shell double quotes.
    let value = serde_json::to_string(expected)
        .expect("string serialization")
        .replace('"', "\\\"");
    format!(
        "node -e \"const actual=require('./package.json').scripts.{script};require('node:assert/strict').strictEqual(actual,{value});console.log(actual)\""
    )
}
