//! Host-side judgement of the hook attribute, package script `includes` and source
//! assertion checks the host assembles (Issue #881). They used to be inline `node -p`
//! programs, which a Verify step cannot repeat once its runtime Bash refuses inline
//! interpreters. They are now `anvil-nextjs-check:` tokens that the host decides in
//! Rust and never hands to a shell. The meaning is the former JS predicate or
//! stricter. Unlike the package script tokens of Issue #879 these are not bound to
//! the nextjs profile: the grep rewrite they replace never looked at the profile.
use std::path::Path;

use regex::Regex;
use serde_json::Value;

use super::package_script_check::CHECK_PREFIX;
use crate::planner::source_assertion::{
    SOURCE_ASSERTION_MARKER, safe_identifier, source_api_token, source_path,
};
use crate::planner::verify::{hook_source_path, safe_hook_action_value};

const HOOK_ACTION: &str = "hook_action";
const HOOK_STATE: &str = "hook_state";
const SCRIPT_INCLUDES: &str = "script_includes";
const SOURCE_KEYDOWN: &str = "source_keydown";
const SOURCE_TOKEN: &str = "source_token";
const FAMILIES: [&str; 5] = [
    HOOK_ACTION,
    HOOK_STATE,
    SCRIPT_INCLUDES,
    SOURCE_KEYDOWN,
    SOURCE_TOKEN,
];

/// The JS `\s` set, spelled out: it holds U+FEFF and not U+0085, unlike Rust's.
const JS_SPACE: &str = r"[\t\n\x0B\x0C\r \x{A0}\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}\x{FEFF}]*";

/// The scripts `script_includes` can name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Script {
    Dev,
    Start,
    Build,
}

impl Script {
    fn name(self) -> &'static str {
        match self {
            Self::Dev => "dev",
            Self::Start => "start",
            Self::Build => "build",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Check {
    HookAction { value: String, path: String },
    HookState { path: String },
    ScriptIncludes { script: Script, pattern: String },
    SourceKeydown { path: String },
    SourceToken { token: String, path: String },
}

/// Everything outside `[A-Za-z0-9_./-]` becomes `%XX` (upper-case hex), so a token
/// holds no space, quote, `$`, `;`, `&` or `:` for a shell or a splitter to read.
fn encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'/' | b'-') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Only the canonical spelling decodes: re-encoding must give `text` back.
fn decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = text.get(index + 1..index + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    let decoded = String::from_utf8(out).ok()?;
    (encode(&decoded) == text).then_some(decoded)
}

fn token(family: &str, args: &[&str]) -> String {
    let args = args.iter().map(|arg| encode(arg)).collect::<Vec<_>>();
    format!("{CHECK_PREFIX}{family}:{}", args.join(":"))
}

pub(crate) fn hook_action_command(value: &str, path: &str) -> String {
    token(HOOK_ACTION, &[value, path])
}

pub(crate) fn hook_state_command(path: &str) -> String {
    token(HOOK_STATE, &[path])
}

pub(crate) fn source_keydown_command(path: &str) -> String {
    token(SOURCE_KEYDOWN, &[path])
}

pub(crate) fn source_token_command(api_token: &str, path: &str) -> String {
    token(SOURCE_TOKEN, &[api_token, path])
}

/// Which script a `grep` pattern on package.json is about, by the rule the inline
/// builder used.
fn script_for_pattern(pattern: &str) -> Option<Script> {
    let lower = pattern.to_ascii_lowercase();
    if lower.contains("next build") || lower.contains("scripts.build") {
        Some(Script::Build)
    } else if lower.contains("next dev")
        || lower.contains("scripts.dev")
        || lower.contains("\"dev\"")
        || lower.contains("'dev'")
    {
        Some(Script::Dev)
    } else if lower.contains("next start")
        || lower.contains("scripts.start")
        || lower.contains("\"start\"")
        || lower.contains("'start'")
    {
        Some(Script::Start)
    } else {
        None
    }
}

/// A pattern with a quote or a backslash is left as the grep it was.
fn pattern_is_tokenizable(pattern: &str) -> bool {
    !pattern.is_empty() && !pattern.contains(['\'', '"', '\\'])
}

pub(crate) fn script_includes_command(pattern: &str) -> Option<String> {
    let script = script_for_pattern(pattern)?;
    pattern_is_tokenizable(pattern).then(|| token(SCRIPT_INCLUDES, &[script.name(), pattern]))
}

fn api_token_is_valid(api_token: &str) -> bool {
    source_api_token(api_token).as_deref() == Some(api_token) || safe_identifier(api_token)
}

/// The exact token grammar. Any altered form (unknown check, extra or missing
/// argument, non-canonical encoding, a value or path the builders would refuse) is
/// `None`.
pub(crate) fn parse(command: &str) -> Option<Check> {
    let rest = command.trim().strip_prefix(CHECK_PREFIX)?;
    let mut parts = rest.split(':');
    let family = parts.next()?;
    let args = parts.map(decode).collect::<Option<Vec<_>>>()?;
    let hook_path = |path: &String| hook_source_path(path).as_deref() == Some(path.as_str());
    let src_path = |path: &String| source_path(path).as_deref() == Some(path.as_str());
    match (family, args.as_slice()) {
        (HOOK_ACTION, [value, path]) if safe_hook_action_value(value) && hook_path(path) => {
            Some(Check::HookAction {
                value: value.clone(),
                path: path.clone(),
            })
        }
        (HOOK_STATE, [path]) if hook_path(path) => Some(Check::HookState { path: path.clone() }),
        (SCRIPT_INCLUDES, [script, pattern]) if pattern_is_tokenizable(pattern) => {
            let script =
                script_for_pattern(pattern).filter(|found| found.name() == script.as_str())?;
            Some(Check::ScriptIncludes {
                script,
                pattern: pattern.clone(),
            })
        }
        (SOURCE_KEYDOWN, [path]) if src_path(path) => {
            Some(Check::SourceKeydown { path: path.clone() })
        }
        (SOURCE_TOKEN, [api_token, path]) if api_token_is_valid(api_token) && src_path(path) => {
            Some(Check::SourceToken {
                token: api_token.clone(),
                path: path.clone(),
            })
        }
        _ => None,
    }
}

/// Whether the command is in one of this leaf's families, valid or not. An altered
/// token is therefore never handed to a shell and is judged (and failed) by the host.
pub(crate) fn is_source_check(command: &str) -> bool {
    command
        .trim()
        .strip_prefix(CHECK_PREFIX)
        .and_then(|rest| rest.split(':').next())
        .is_some_and(|family| FAMILIES.contains(&family))
}

/// Whether the token is a valid source assertion (the kind a failed run may demote
/// to an advisory).
pub(crate) fn is_source_assertion(command: &str) -> bool {
    matches!(
        parse(command),
        Some(Check::SourceKeydown { .. } | Check::SourceToken { .. })
    )
}

/// Whether the token asserts the contract or the build, the way the inline text did
/// by naming `data-anvil-` or `package.json`.
pub(crate) fn is_contract_or_build_check(command: &str) -> bool {
    matches!(
        parse(command),
        Some(Check::HookAction { .. } | Check::HookState { .. } | Check::ScriptIncludes { .. })
    ) || super::package_script_check::parse(command).is_some()
}

/// Judge one token against the workspace. `Err` carries the reason.
pub(crate) fn evaluate(root: &Path, command: &str) -> Result<(), String> {
    let check = parse(command).ok_or_else(|| {
        format!(
            "unknown or malformed nextjs host check `{}`; use the declared token unchanged",
            command.trim()
        )
    })?;
    match check {
        Check::HookAction { value, path } => {
            let source = read_source(root, &path)?;
            let (escaped, space) = (regex::escape(&value), JS_SPACE);
            let pattern = format!(
                "data-anvil-action{space}={space}(?:\"{escaped}\"|'{escaped}'|[{{]{space}(?:\"{escaped}\"|'{escaped}'|`{escaped}`){space}[}}])"
            );
            require(&source, &pattern, || {
                format!("{path} has no data-anvil-action=\"{value}\"")
            })
        }
        Check::HookState { path } => {
            let source = read_source(root, &path)?;
            let pattern = format!("data-anvil-state(?:{JS_SPACE}=|(?-u:\\b))");
            require(&source, &pattern, || {
                format!("{path} has no data-anvil-state")
            })
        }
        Check::ScriptIncludes { script, pattern } => script_includes(root, script, &pattern),
        Check::SourceKeydown { path } => {
            let source = read_source(root, &path)?;
            let keydown = source.contains("onKeyDown")
                || source.contains("keys[")
                || (source.contains("addEventListener") && source.contains("keydown"));
            keydown.then_some(()).ok_or_else(|| {
                format!("{path} has no keydown input handler (onKeyDown, keys[ or addEventListener keydown)")
            })
        }
        Check::SourceToken { token, path } => {
            let source = read_source(root, &path)?;
            source
                .contains(token.as_str())
                .then_some(())
                .ok_or_else(|| format!("{path} does not contain `{token}`"))
        }
    }
}

fn require(source: &str, pattern: &str, reason: impl FnOnce() -> String) -> Result<(), String> {
    let found = Regex::new(pattern).is_ok_and(|regex| regex.is_match(source));
    found.then_some(()).ok_or_else(reason)
}

/// The file as `String(readFileSync(path))` read it: bytes, invalid UTF-8 replaced.
fn read_source(root: &Path, path: &str) -> Result<String, String> {
    std::fs::read(root.join(path))
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .map_err(|error| format!("{path} is not readable: {error}"))
}

fn script_includes(root: &Path, script: Script, pattern: &str) -> Result<(), String> {
    let raw = std::fs::read_to_string(root.join("package.json"))
        .map_err(|error| format!("package.json is not readable: {error}"))?;
    let manifest: Value = serde_json::from_str(&raw)
        .map_err(|error| format!("package.json is not valid JSON: {error}"))?;
    let name = script.name();
    match manifest
        .get("scripts")
        .and_then(|scripts| scripts.get(name))
    {
        Some(Value::String(text)) if text.contains(pattern) => Ok(()),
        Some(Value::String(text)) => Err(format!(
            "scripts.{name} must contain `{pattern}`; found `{text}`"
        )),
        _ => Err(format!(
            "scripts.{name} must be a string containing `{pattern}`"
        )),
    }
}

/// The inline `node -p` text the host assembled before Issue #881. The host no
/// longer emits it, but saved plans and recorded runs still carry it, so the
/// comparisons and classifiers keep recognizing these exact texts. Recognition is
/// only classification: a model-typed copy is still an inline interpreter and stays
/// refused in a Verify or Report step's runtime Bash.
pub(crate) fn legacy_inline_command(check: &Check) -> String {
    match check {
        Check::HookAction { value, path } => legacy_hook_action(value, path),
        Check::HookState { path } => legacy_hook_state(path),
        Check::ScriptIncludes { script, pattern } => format!(
            "node -p \"String(require('./package.json').scripts.{}).includes('{pattern}') ? true : process.exit(1)\"",
            script.name()
        ),
        Check::SourceKeydown { path } => legacy_source_keydown(path),
        Check::SourceToken { token, path } => legacy_source_token(token, path),
    }
}

fn legacy_hook_action(value: &str, source_path: &str) -> String {
    format!(
        concat!(
            "node -p '",
            "(function(s,w,d,q,b){{return [",
            "new RegExp(\"data-anvil-action\"+w+\"=\"+w+d+\"{value}\"+d),",
            "new RegExp(\"data-anvil-action\"+w+\"=\"+w+q+\"{value}\"+q),",
            "new RegExp(\"data-anvil-action\"+w+\"=\"+w+\"[{{]\"+w+d+\"{value}\"+d+w+\"[}}]\"),",
            "new RegExp(\"data-anvil-action\"+w+\"=\"+w+\"[{{]\"+w+q+\"{value}\"+q+w+\"[}}]\"),",
            "new RegExp(\"data-anvil-action\"+w+\"=\"+w+\"[{{]\"+w+b+\"{value}\"+b+w+\"[}}]\")",
            "].some(function(r){{return r.test(s)}})?true:process.exit(1)}})",
            "(String(require(\"fs\").readFileSync(\"{source_path}\")),",
            "String.fromCharCode(92)+\"s*\",String.fromCharCode(34),",
            "String.fromCharCode(39),String.fromCharCode(96))",
            "'"
        ),
        value = value,
        source_path = source_path
    )
}

fn legacy_hook_state(source_path: &str) -> String {
    format!(
        concat!(
            "node -p '",
            "(function(s,w,b){{return [",
            "new RegExp(\"data-anvil-state\"+w+\"=\"),",
            "new RegExp(\"data-anvil-state\"+b)",
            "].some(function(r){{return r.test(s)}})?true:process.exit(1)}})",
            "(String(require(\"fs\").readFileSync(\"{source_path}\")),",
            "String.fromCharCode(92)+\"s*\",String.fromCharCode(92)+\"b\")",
            "'"
        ),
        source_path = source_path
    )
}

fn legacy_source_keydown(source_path: &str) -> String {
    format!(
        concat!(
            "node -p '",
            "(function(s,m){{return [",
            "s.includes(\"onKeyDown\"),",
            "s.includes(\"keys[\"),",
            "s.includes(\"addEventListener\")?s.includes(\"keydown\"):false",
            "].some(function(x){{return x}})?true:process.exit(1)}})",
            "(String(require(\"fs\").readFileSync(\"{source_path}\")),",
            "\"{marker}\")",
            "'"
        ),
        source_path = source_path,
        marker = SOURCE_ASSERTION_MARKER,
    )
}

fn legacy_source_token(api_token: &str, source_path: &str) -> String {
    format!(
        concat!(
            "node -p '",
            "(function(s,m){{return s.includes(\"{token}\")?true:process.exit(1)}})",
            "(String(require(\"fs\").readFileSync(\"{source_path}\")),",
            "\"{marker}\")",
            "'"
        ),
        token = api_token,
        source_path = source_path,
        marker = SOURCE_ASSERTION_MARKER,
    )
}

/// The inline text a valid token stood for, if `command` is one.
pub(crate) fn legacy_for_token(command: &str) -> Option<String> {
    parse(command).map(|check| legacy_inline_command(&check))
}

/// Whitespace outside string literals does not change a `node -p '…'` program.
fn legacy_key(command: &str) -> Option<String> {
    let script = command
        .trim()
        .strip_prefix("node -p '")?
        .strip_suffix('\'')?;
    let mut key = String::with_capacity(script.len());
    let mut quote = None;
    let mut escaped = false;
    for ch in script.chars() {
        if let Some(active_quote) = quote {
            key.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
        } else if matches!(ch, '"' | '\'' | '`') {
            quote = Some(ch);
            key.push(ch);
        } else if !ch.is_ascii_whitespace() {
            key.push(ch);
        }
    }
    quote.is_none().then_some(key)
}

/// Whether two commands name the same hook check: equal tokens, or the legacy inline
/// texts compared by [`legacy_key`], or a token and the legacy text it replaced. It is
/// for comparing a measured command with a host-derived one only; it never admits,
/// runs or emits anything, and a one-character difference is not equivalent.
pub(crate) fn same_hook_check(left: &str, right: &str) -> bool {
    let identity = |command: &str| {
        let text = legacy_for_token(command).unwrap_or_else(|| command.to_string());
        legacy_key(&text)
    };
    identity(left).is_some_and(|left| Some(left) == identity(right))
}

/// Whether `command` is a generated hook check: a token for a hook assertion the host
/// derives from a grep of the contract values, or the exact inline text it once was.
pub(crate) fn is_generated_hook_check(command: &str) -> bool {
    const ACTIONS: [&str; 5] = ["primary", "restart", "input", "search", "submit"];
    match parse(command) {
        Some(Check::HookAction { value, .. }) => ACTIONS.contains(&value.as_str()),
        Some(Check::HookState { .. }) => true,
        Some(_) => false,
        None => legacy_generated_hook_check(command, &ACTIONS),
    }
}

fn legacy_generated_hook_check(command: &str, actions: &[&str]) -> bool {
    let Some((_, rest)) = command.split_once("readFileSync(\"") else {
        return false;
    };
    let Some((path, _)) = rest.split_once('"') else {
        return false;
    };
    if hook_source_path(path).as_deref() != Some(path) {
        return false;
    }
    command == legacy_hook_state(path)
        || actions
            .iter()
            .any(|value| command == legacy_hook_action(value, path))
}

#[cfg(test)]
#[path = "source_checks_tests.rs"]
mod tests;
