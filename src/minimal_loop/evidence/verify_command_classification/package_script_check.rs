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

const PREDICATE_METHODS: [&str; 3] = ["includes", "startsWith", "endsWith"];
const PREDICATE_SOURCE: &str = "const actual=require('./package.json').scripts.";
const PREDICATE_TYPE_CHECK: &str = ";require('node:assert/strict').strictEqual(typeof actual,'string');require('node:assert/strict').ok(actual.";

/// The weak printed predicate `node -p "require('./package.json').scripts.<id>.<m>('<lit>')"`.
/// It prints true or false and exits zero either way, so it is only ever replaced.
pub(crate) fn weak_predicate(command: &str) -> Option<(String, String, String)> {
    let words = single_command_words(command)?;
    let [program, flag, source] = words.as_slice() else {
        return None;
    };
    let rest = source.strip_prefix("require('./package.json').scripts.")?;
    let (script, rest) = rest.split_once('.')?;
    let (method, rest) = rest.split_once("('")?;
    let value = rest.strip_suffix("')")?;
    (program == "node"
        && matches!(flag.as_str(), "-p" | "--print")
        && identifier(script)
        && PREDICATE_METHODS.contains(&method)
        && literal(value))
    .then(|| (script.into(), method.into(), value.into()))
}

/// The strict replacement: a string-typed script value that must satisfy the
/// predicate, printing the same `true` on success.
pub(crate) fn predicate(command: &str) -> Option<(String, String, String)> {
    let source = import_check::source(command)?;
    let rest = source.strip_prefix(PREDICATE_SOURCE)?;
    let (script, rest) = rest.split_once(PREDICATE_TYPE_CHECK)?;
    let (method, rest) = rest.split_once('(')?;
    let value: String = serde_json::from_str(rest.strip_suffix("));console.log(true)")?).ok()?;
    (identifier(script) && PREDICATE_METHODS.contains(&method) && literal(&value))
        .then(|| (script.into(), method.into(), value))
}

pub(crate) fn formed_predicate_command(script: &str, method: &str, value: &str) -> String {
    // Inputs have already passed the bounded identifier/method/literal grammar;
    // JSON quotes are escaped for the outer shell double quotes.
    let value = serde_json::to_string(value)
        .expect("string serialization")
        .replace('"', "\\\"");
    format!(
        "node -e \"{PREDICATE_SOURCE}{script}{PREDICATE_TYPE_CHECK}{method}({value}));console.log(true)\""
    )
}
