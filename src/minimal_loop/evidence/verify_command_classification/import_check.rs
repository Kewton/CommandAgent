//! A closed strengthening grammar. Identical import, then a target-dependent
//! assertion; no arbitrary equivalence declarations or swallowed rejections.
use super::single_command_words;
use serde_json::Value;

pub(crate) fn source(command: &str) -> Option<String> {
    let words = single_command_words(command)?;
    let [program, flag, ..] = words.as_slice() else {
        return None;
    };
    let eval = matches!(flag.as_str(), "-e" | "--eval") && words.len() == 3
        || (flag.starts_with("--eval=") || flag.starts_with("-e")) && words.len() == 2;
    let inline = super::inline_argument(&words[1..])?;
    (program == "node" && eval && inline.executable).then(|| inline.source.to_owned())
}

pub(super) fn local_path(path: &str) -> bool {
    let path = path.strip_prefix("./").unwrap_or(path);
    !path.is_empty()
        && path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_./-[]".contains(&b))
        && !path.split('/').any(|p| matches!(p, "" | "." | ".."))
        && matches!(
            std::path::Path::new(path)
                .extension()
                .and_then(|s| s.to_str()),
            Some("ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs")
        )
}

fn import(source: &str) -> Option<(&str, &str)> {
    let rest = source.strip_prefix("import('")?;
    let (target, rest) = rest.split_once("')")?;
    (target.starts_with("./") && local_path(target)).then_some((target, rest))
}

pub(crate) fn pure_target(command: &str) -> Option<String> {
    let source = source(command)?;
    let (target, rest) = import(&source)?;
    matches!(rest, "" | ";").then(|| target.into())
}

fn json_prefix(text: &str) -> Option<(Value, &str)> {
    let mut stream = serde_json::Deserializer::from_str(text).into_iter::<Value>();
    let value = stream.next()?.ok()?;
    Some((value, &text[stream.byte_offset()..]))
}

pub(crate) fn strengthened_target(command: &str) -> Option<String> {
    let source = source(command)?;
    let (target, rest) = import(&source)?;
    let rest =
        rest.strip_prefix(".then(actual=>{require('node:assert/strict').deepStrictEqual(actual.")?;
    let end = rest.find(|c: char| !c.is_ascii_alphanumeric() && c != '_')?;
    let (export, mut rest) = rest.split_at(end);
    if export.is_empty()
        || !export.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        || matches!(export, "constructor" | "__proto__" | "prototype" | "then")
    {
        return None;
    }
    // Literal arguments cannot introduce a callback, override assert or replace
    // the import. A plain export value is also a target-dependent observation.
    if let Some(args) = rest.strip_prefix("(...") {
        let (value, suffix) = json_prefix(args)?;
        if !value.is_array() {
            return None;
        }
        rest = suffix.strip_prefix(')')?;
    }
    serde_json::from_str::<Value>(rest.strip_prefix(',')?.strip_suffix(")})")?).ok()?;
    Some(target.into())
}

/// The proposal explicitly supplies the runtime export boundary. An empty
/// boundary can describe type-only modules, without claiming type validation.
pub(crate) fn export_set_target(command: &str) -> Option<String> {
    let source = source(command)?;
    let (target, rest) = import(&source)?;
    let (expected, rest) = json_prefix(rest.strip_prefix(
        ".then(actual=>{require('node:assert/strict').deepStrictEqual(Object.keys(actual).sort(),",
    )?)?;
    let names = expected
        .as_array()?
        .iter()
        .map(Value::as_str)
        .collect::<Option<Vec<_>>>()?;
    if names.iter().any(|name| name.is_empty()) || names.windows(2).any(|w| w[0] >= w[1]) {
        return None;
    }
    (rest == ")})").then(|| target.into())
}
