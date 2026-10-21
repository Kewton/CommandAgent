//! Bounded execution diagnostics, separate from display excerpts. Never derive
//! a causal exception from Node's echoed eval source or from stdout.
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::LazyLock;

mod predicate;

const PREFIX: &str = "node_failure: ";
const RECORD_LIMIT: usize = 400;

#[derive(Serialize, Deserialize)]
struct Failure {
    command_sha256: String,
    error: String,
    line: usize,
    column: usize,
}

fn digest(command: &str) -> String {
    format!("{:x}", Sha256::digest(command.as_bytes()))
}

/// Called only for a nonzero completed process, before stream truncation.
pub(crate) fn summary(command: &str, stdout: &str, stderr: &str) -> Option<String> {
    if stdout.is_empty() && stderr.is_empty() && predicate::single_exit(command).is_some() {
        return encode(Failure {
            command_sha256: digest(command),
            error: "source predicate exited nonzero".into(),
            line: 0,
            column: 0,
        });
    }
    crate::minimal_loop::evidence::import_check::source(command)?;
    static EXCEPTION: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^((?:[A-Za-z]*Error)(?: \[[A-Z_0-9]+\])?: [^\r\n]+)\r?\n    at \[eval\]:(\d+):(\d+)\r?$").unwrap()
    });
    let mut matches = EXCEPTION.captures_iter(stderr);
    let found = matches.next()?;
    // Several diagnostics cannot establish one executed predicate.
    if matches.next().is_some() {
        return None;
    }
    let failure = Failure {
        command_sha256: digest(command),
        error: crate::eval_events::scrub_sensitive_text(&found[1]),
        line: found[2].parse().ok()?,
        column: found[3].parse().ok()?,
    };
    encode(failure)
}

fn encode(failure: Failure) -> Option<String> {
    let record = format!("{PREFIX}{}", serde_json::to_string(&failure).ok()?);
    // Never truncate a structured record into apparently valid evidence.
    (record.len() <= RECORD_LIMIT).then_some(record)
}

fn record<'a>(command: &str, text: &'a str) -> Option<(&'a str, Failure)> {
    let json = text.strip_prefix(PREFIX)?;
    let mut stream = serde_json::Deserializer::from_str(json).into_iter::<Failure>();
    let failure = stream.next()?.ok()?;
    let end = PREFIX.len() + stream.byte_offset();
    (end <= RECORD_LIMIT && failure.command_sha256 == digest(command))
        .then_some((&text[..end], failure))
}

fn observation<'a>(command: &str, output: &'a str) -> Option<(&'a str, Failure)> {
    let output = output
        .strip_prefix("build_verify_failed: ")
        .or_else(|| output.strip_prefix("dependency_setup_lifecycle_failed: "))
        .unwrap_or(output);
    if let Some(record) = record(command, output) {
        return Some(record);
    }
    // Only the executor's summary header, before stdout/stderr payloads, may
    // carry this record. An echoed/printed marker is not an observation.
    static HEADER: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?:^|\n)outcome: CommandFailed\nstatus: exit status: [1-9][0-9]*\nelapsed_ms: [0-9]+\nsummary: (node_failure: [^\n]+)\nstdout:\n").unwrap()
    });
    let header = HEADER.captures(output)?;
    record(command, header.get(1)?.as_str())
}

/// Keep the complete redacted causal record at the front of the existing
/// 500-character reason budget; display excerpts retain their own semantics.
pub(crate) fn reason(command: &str, output: &str) -> String {
    match observation(command, output) {
        Some((record, _)) => {
            let display = output
                .split_once("\noutcome: ")
                .map(|(_, display)| display)
                .unwrap_or(output.strip_prefix(record).unwrap_or(output));
            crate::eval_events::body_snippet(&format!("{record}\n{display}"))
        }
        None => crate::eval_events::body_snippet(output),
    }
}

pub(crate) fn attribute(command: &str, reason: &str) -> Option<(String, String)> {
    let (_, failure) = observation(command, reason)?;
    if failure.line == 0
        && failure.column == 0
        && failure.error == "source predicate exited nonzero"
    {
        return predicate::single_exit(command);
    }
    let source = crate::minimal_loop::evidence::import_check::source(command)?;
    predicate::attribute(&source, &failure.error, failure.line, failure.column)
}

#[cfg(test)]
pub(crate) mod tests;
