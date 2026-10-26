//! Host-side judgement of the package scripts the nextjs profile assembles
//! (Issue #879). The checks used to be inline `node -p` programs that a Verify
//! step cannot repeat once its runtime Bash refuses inline interpreters. They
//! are now `anvil-nextjs-check:` tokens: the host decides them in Rust and never
//! hands a token to a shell. The meaning is the former JS predicate or stricter.
use std::path::Path;

use serde_json::Value;

use crate::planner::step_plan::PlanStep;
use crate::planner::verify::VerificationReport;

pub(crate) const CHECK_PREFIX: &str = "anvil-nextjs-check:";
const PORT_SCRIPT_CHECK: &str = "package_script_port";
const BUILD_SCRIPT_CHECK: &str = "build_script";

/// The scripts whose port the host judges. `dev` is required, `start` optional.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PortScript {
    Dev,
    Start,
}

impl PortScript {
    fn name(self) -> &'static str {
        match self {
            Self::Dev => "dev",
            Self::Start => "start",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Check {
    PortScript { script: PortScript, port: u16 },
    BuildScript,
}

pub(crate) fn port_check_command(script: PortScript, port: u16) -> String {
    format!("{CHECK_PREFIX}{PORT_SCRIPT_CHECK}:{}:{port}", script.name())
}

/// A port that is not a `u16` can never be satisfied: the token keeps the text so
/// the host check fails closed instead of the builder inventing a different gate.
pub(crate) fn dev_port_check_command_for(port: &str) -> String {
    match port.parse::<u16>() {
        Ok(port) => port_check_command(PortScript::Dev, port),
        Err(_) => format!("{CHECK_PREFIX}{PORT_SCRIPT_CHECK}:dev:{port}"),
    }
}

pub(crate) fn build_check_command() -> String {
    format!("{CHECK_PREFIX}{BUILD_SCRIPT_CHECK}")
}

/// Whether the host owns this command: anything in the token namespace, valid or
/// not. An altered token is therefore never handed to a shell.
pub(crate) fn is_check_command(command: &str) -> bool {
    command.trim().starts_with(CHECK_PREFIX)
}

/// The exact token grammar. Any altered form (unknown check, extra or missing
/// argument, non-canonical port, surrounding text) is `None`.
pub(crate) fn parse(command: &str) -> Option<Check> {
    let rest = command.trim().strip_prefix(CHECK_PREFIX)?;
    if rest == BUILD_SCRIPT_CHECK {
        return Some(Check::BuildScript);
    }
    let mut parts = rest.split(':');
    if parts.next()? != PORT_SCRIPT_CHECK {
        return None;
    }
    let script = match parts.next()? {
        "dev" => PortScript::Dev,
        "start" => PortScript::Start,
        _ => return None,
    };
    let text = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let port = text.parse::<u16>().ok()?;
    (port.to_string() == text).then_some(Check::PortScript { script, port })
}

/// Judge one token against `root/package.json`. `Err` carries the reason.
pub(crate) fn evaluate(root: &Path, command: &str) -> Result<(), String> {
    let check = parse(command).ok_or_else(|| {
        format!(
            "unknown or malformed nextjs host check `{}`; use the declared token unchanged",
            command.trim()
        )
    })?;
    let raw = std::fs::read_to_string(root.join("package.json"))
        .map_err(|error| format!("package.json is not readable: {error}"))?;
    let manifest: Value = serde_json::from_str(&raw)
        .map_err(|error| format!("package.json is not valid JSON: {error}"))?;
    let scripts = manifest.get("scripts").and_then(Value::as_object);
    let script = |name: &str| scripts.and_then(|scripts| scripts.get(name));
    match check {
        Check::BuildScript => match script("build") {
            Some(Value::String(build)) if build == "next build" => Ok(()),
            _ => Err("scripts.build must be exactly `next build`".to_string()),
        },
        Check::PortScript { script: name, port } => match (script(name.name()), name) {
            (Some(Value::String(text)), _) => port_script_error(name, text, port),
            (None | Some(Value::Null), PortScript::Start) => Ok(()),
            (Some(value), PortScript::Start) if is_js_falsy(value) => Ok(()),
            (Some(_), _) => Err(format!(
                "scripts.{} must be a string that runs `next {}` on port {port}",
                name.name(),
                name.name()
            )),
            (None, PortScript::Dev) => Err(format!(
                "scripts.{} is missing; it must run `next dev` on port {port}",
                name.name()
            )),
        },
    }
}

/// An optional script is skipped when the former JS read it as false.
fn is_js_falsy(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::Bool(flag) => !flag,
        Value::Number(number) => number.as_f64().is_some_and(|number| number == 0.0),
        Value::String(text) => text.is_empty(),
        _ => false,
    }
}

fn port_script_error(script: PortScript, text: &str, port: u16) -> Result<(), String> {
    if text.is_empty() && script == PortScript::Start {
        return Ok(());
    }
    let words = text.split(' ').collect::<Vec<_>>();
    let name = script.name();
    let runs_script = words
        .iter()
        .enumerate()
        .any(|(index, word)| *word == "next" && next_word(&words, index) == Some(name));
    if !runs_script {
        return Err(format!(
            "scripts.{name} must run `next {name}`; found `{text}`"
        ));
    }
    let port_text = port.to_string();
    let has_port = words.iter().enumerate().any(|(index, word)| {
        *word == format!("--port={port_text}")
            || *word == format!("-p{port_text}")
            || (matches!(*word, "-p" | "--port")
                && next_word(&words, index) == Some(port_text.as_str()))
    });
    if has_port {
        Ok(())
    } else {
        Err(format!(
            "scripts.{name} must pass port {port} (`--port={port}`, `-p {port}`, `-p{port}` or `--port {port}`); found `{text}`"
        ))
    }
}

/// The first non-empty word after `index`, as the former `find` read it.
fn next_word<'a>(words: &[&'a str], index: usize) -> Option<&'a str> {
    words[index + 1..]
        .iter()
        .copied()
        .find(|word| !word.is_empty())
}

/// The inline `node -p` forms the host assembled before Issue #879. The host no
/// longer emits them, but saved plans and recorded corpora still carry them, so the
/// classifiers keep recognizing these exact texts as the package checks they were.
/// Recognition is only classification: a model-typed copy is still an inline
/// interpreter and stays refused in a Verify or Report step's runtime Bash.
pub(crate) fn legacy_inline_command(check: Check) -> String {
    match check {
        Check::BuildScript => LEGACY_BUILD_COMMAND.to_string(),
        Check::PortScript { script, port } => {
            let (script, port) = (script.name(), port);
            let (guard, tail) = match script {
                "dev" => ("", ""),
                _ => ("Object(require('./package.json').scripts)[k] ? ", " : true"),
            };
            format!(
                concat!(
                    "node -p \"",
                    "['{script}'].every(function(k){{",
                    "return {guard}",
                    "String(Object(require('./package.json').scripts)[k]).split(' ')",
                    ".some(function(t,i,a){{return t=='next' ? a.slice(i+1).find(function(x){{return x}})==k : false}}) ? ",
                    "String(Object(require('./package.json').scripts)[k]).split(' ')",
                    ".some(function(t,i,a){{",
                    "return t=='--port={port}' ? true : ",
                    "t=='-p' ? a.slice(i+1).find(function(x){{return x}})=='{port}' : ",
                    "t=='-p{port}' ? true : ",
                    "t=='--port' ? a.slice(i+1).find(function(x){{return x}})=='{port}' : ",
                    "false",
                    "}}) : false{tail}",
                    "}}) ? true : process.exit(1)",
                    "\""
                ),
                script = script,
                port = port,
                guard = guard,
                tail = tail
            )
        }
    }
}

const LEGACY_BUILD_COMMAND: &str = "node -p \"String(require('./package.json').scripts.build)=='next build' ? true : process.exit(1)\"";

/// The port a package check pins, for a token or a legacy inline text alike.
pub(crate) fn checked_port(command: &str) -> Option<u16> {
    if let Some(check) = parse(command) {
        return match check {
            Check::PortScript { port, .. } => Some(port),
            Check::BuildScript => None,
        };
    }
    let (_, rest) = command.split_once("--port=")?;
    let (port, _) = rest.split_once('\'')?;
    port.parse().ok()
}

/// Whether `command` is exactly one of the legacy inline package checks.
pub(crate) fn is_legacy_inline_command(command: &str) -> bool {
    command == LEGACY_BUILD_COMMAND
        || checked_port(command).is_some_and(|port| {
            [PortScript::Dev, PortScript::Start]
                .into_iter()
                .any(|script| command == legacy_inline_command(Check::PortScript { script, port }))
        })
}

/// Whether two commands name the same declared check: equal, or one is a token and
/// the other the legacy inline text the host once assembled for it. It is for
/// comparing a saved declaration with a host-derived one only: it never admits, runs
/// or emits anything, and a one-character difference is not equivalent.
pub(crate) fn same_declared_check(left: &str, right: &str) -> bool {
    let legacy_of = |token: &str, text: &str| {
        parse(token).is_some_and(|check| text == legacy_inline_command(check))
    };
    left == right || legacy_of(left, right) || legacy_of(right, left)
}

/// Whether `list` carries `command` or an equivalent declared check (see
/// [`same_declared_check`]); for comparing saved declarations only.
pub(crate) fn list_has_declared_check(list: &[String], command: &str) -> bool {
    list.iter().any(|item| same_declared_check(item, command))
}

/// Whether `command` is a legacy inline check for exactly this requested port.
pub(crate) fn legacy_inline_covers(command: &str, port: u16) -> bool {
    command == LEGACY_BUILD_COMMAND
        || [PortScript::Dev, PortScript::Start]
            .into_iter()
            .any(|script| command == legacy_inline_command(Check::PortScript { script, port }))
}

/// Whether a plan-quality lint reads  as a verify of package.json: only an
/// exact declared token, and only in the nextjs profile. The inline check it replaced
/// named package.json in its text; an unknown or malformed token, or one in another
/// profile, is not coupled to any artifact and is refused as before.
pub(crate) fn coupled(profile_is_nextjs: bool, command: &str) -> bool {
    profile_is_nextjs && parse(command).is_some()
}

/// Step-level host judgement, run next to the other internal checks. Outside the
/// nextjs profile a token fails (the data catalog's shape).
pub(crate) fn run_step_check(
    root: &Path,
    profile: Option<&str>,
    step: &PlanStep,
    report: &mut VerificationReport,
) {
    for command in step
        .verify
        .iter()
        .filter(|command| is_check_command(command))
    {
        if let Err(reason) = evaluate_in_profile(root, profile, command) {
            report.push_command_failure(command.clone(), reason);
        }
    }
}

/// The one judgement shared by the step verifier and the runtime Bash segment.
pub(crate) fn evaluate_in_profile(
    root: &Path,
    profile: Option<&str>,
    command: &str,
) -> Result<(), String> {
    if !profile.is_some_and(is_nextjs_profile) {
        return Err("nextjs host check is invalid outside the active nextjs profile".to_string());
    }
    evaluate(root, command)
}

fn is_nextjs_profile(profile: &str) -> bool {
    crate::planner::profile::domain_profile(profile).id() == super::PROFILE_ID
}

#[cfg(test)]
#[path = "package_script_check_tests.rs"]
mod tests;
