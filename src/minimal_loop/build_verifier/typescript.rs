//! Supplement a first-error Next build with the installed whole-project checker.
//! This is diagnostic evidence only: the failed build remains failed.
use std::path::Path;

use crate::planner::verify::NormalizedVerifyCommand;

use super::{BuildVerifierRequirement, CompileError};

const COMMAND: &str =
    "node node_modules/typescript/bin/tsc --noEmit --incremental false --pretty false";
const MARKER: &str = "Supplementary TypeScript diagnostics (bounded output; not acceptance):";

pub(super) fn supplement(
    root: &Path,
    requirement: &BuildVerifierRequirement,
    failure: String,
) -> String {
    if !requirement
        .profile
        .as_deref()
        .is_some_and(crate::planner::profile::is_nextjs_profile)
        || !failure.contains("Type error:")
        || !root.join("tsconfig.json").is_file()
        || !root.join("node_modules/typescript/bin/tsc").is_file()
    {
        return failure;
    }
    let command: NormalizedVerifyCommand =
        crate::planner::verify::normalize_verify_command(COMMAND)
            .expect("fixed diagnostic command");
    let output = match crate::minimal_loop::verifier_env::run_checked(&command, root, false) {
        Ok(output) => {
            format!("Checker exited successfully; original build must still be rerun.\n{output}")
        }
        Err(error) => error.to_string(),
    };
    format!("{failure}\n\n{MARKER}\n{output}")
}

pub(super) fn append_diagnostics(output: &str, errors: &mut Vec<CompileError>) {
    let Some((_, diagnostic)) = output.split_once(MARKER) else {
        return;
    };
    // The executor summary repeats a diagnostic with a `summary:` prefix.
    // Parse the actual tsc stdout, never that summary as a source path.
    let diagnostic = diagnostic
        .split_once("\nstdout:\n")
        .map_or(diagnostic, |(_, stdout)| {
            stdout
                .split_once("\nstderr:\n")
                .map_or(stdout, |(stdout, _)| stdout)
        });
    let pattern = regex::Regex::new(r"^(.+\.(?:ts|tsx))\((\d+),(\d+)\): error (TS\d+: .+)$")
        .expect("tsc plain diagnostic format");
    for line in diagnostic.lines() {
        let Some(c) = pattern.captures(line) else {
            continue;
        };
        let Some(path) = super::normalize_compile_error_path(&c[1]) else {
            continue;
        };
        let error = CompileError {
            path,
            line: c[2].parse().unwrap_or_default(),
            column: c[3].parse().unwrap_or_default(),
            message: c[4].to_string(),
            excerpt: String::new(),
            symbol: None,
            route_bound: None,
        };
        if !errors.iter().any(|e| {
            e.path == error.path
                && e.line == error.line
                && e.column == error.column
                && (e.message == error.message
                    || e.message.strip_prefix("Type error: ")
                        == error.message.split_once(": ").map(|(_, message)| message))
        }) {
            errors.push(error);
        }
    }
}

#[cfg(test)]
mod tests;
