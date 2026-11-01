//! A verify command whose shape cannot execute as written is a lint error.
//!
//! Two narrow shapes are rejected: a line that reads as prose, and a `test` /
//! `[` unary file operator whose operand is missing or comes before it, or a
//! `[` without its closing `]`. Anything that may execute is left to the
//! existing gates.
//!
//! A line reads as prose only when its first word is no shell builtin, no
//! command on the planning PATH, no existing or declared workspace path, and
//! no `#` comment, and then one of two shapes holds:
//!
//! - the first word has no ASCII letter, digit, `.`, `/`, `_`, or `-`, so it
//!   cannot be a command name; or
//! - the line is a sentence: it has at least three words, every raw word (as
//!   written, before quotes are removed or fragments joined) is made of
//!   letters only, and the last word closes with `.`, `!`, `?`, `。`, `！`, or
//!   `？`. A quote, glob character, digit, flag, path, `~`, `=`, `$`,
//!   backslash, comma, or operator anywhere keeps a line out of that shape.
//!
//! Every other line is left to the runtime gate, which fails it there when it
//! cannot run; an unknown or not-yet-installed command name alone never makes
//! a line prose. Nothing is executed, installed, or fetched to decide.
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::path::Path;

use crate::planner::lint::PlanLintReport;
use crate::planner::recovery_contract_authority::verifier_obligations;
use crate::planner::step_plan::StepPlan;
use crate::planner::verify::{normalize_planner_verify_command, shell_words_with_spans};

const CATEGORY: &str = "verify_command_shape";

/// Shell builtins and reserved words that may start a simple command.
const SHELL_BUILTINS: &[&str] = &[
    "!", "(", "{", "}", ":", ".", "[", "[[", "alias", "bg", "break", "builtin", "cd", "command",
    "continue", "declare", "echo", "eval", "exec", "exit", "export", "false", "fg", "getopts",
    "hash", "jobs", "kill", "let", "local", "printf", "pwd", "read", "readonly", "return", "set",
    "shift", "source", "test", "times", "trap", "true", "type", "ulimit", "umask", "unalias",
    "unset", "wait",
];

const FILE_OPERATORS: &[&str] = &["-f", "-d", "-e", "-s", "-x", "-r", "-w"];

const BINARY_OPERATORS: &[&str] = &[
    "=", "==", "!=", "<", ">", "-eq", "-ne", "-lt", "-le", "-gt", "-ge", "-nt", "-ot", "-ef", "-a",
    "-o",
];

const EXECUTABLE_EXAMPLE: &str = "replace it with an executable command such as \
    `python3 <script> <args>`, `node <script>`, or `test -f <path>`";

/// Adds one `verify_command_shape` lint error per verify command that cannot
/// execute as written, with PATH resolved in the environment of the planning
/// process. Commands in `skip` are host or user obligations, not proposals.
pub(crate) fn push_lint_errors(
    plan: &StepPlan,
    work_root: Option<&Path>,
    skip: &[String],
    report: &mut PlanLintReport,
) {
    let path_var = std::env::var_os("PATH");
    for message in diagnose_with_path(plan, work_root, skip, path_var.as_deref()) {
        report.push(CATEGORY, message);
    }
}

/// One lint message per verify command that cannot execute as written.
pub(crate) fn diagnose_with_path(
    plan: &StepPlan,
    work_root: Option<&Path>,
    skip: &[String],
    path_var: Option<&OsStr>,
) -> Vec<String> {
    let mut offenses = Vec::new();
    let mut declared = BTreeSet::new();
    for step in &plan.steps {
        declared.extend(
            step.expected_paths
                .iter()
                .filter_map(|path| verifier_obligations::normalized(path).ok()),
        );
        for command in &step.verify {
            if skip.contains(command) || is_exempt(command) {
                continue;
            }
            // A command the shared normalizer rejects is already a verify_policy error.
            let Ok(normalized) = normalize_planner_verify_command(command) else {
                continue;
            };
            let context = Resolve {
                work_root,
                declared: &declared,
                path_var,
            };
            if let Some(reason) = normalized
                .iter()
                .find_map(|line| shape_defect(line, &context))
            {
                offenses.push(format!(
                    "verify command cannot execute as written ({reason}): step {} command \
                     `{command}`; {EXECUTABLE_EXAMPLE}",
                    step.id
                ));
            }
        }
    }
    offenses
}

fn is_exempt(command: &str) -> bool {
    crate::planner::profiles::data::step_policy::catalog_check_id(command).is_some()
        || crate::planner::verify::dependency_install_verify_segment(command).is_some()
}

struct Resolve<'a> {
    work_root: Option<&'a Path>,
    declared: &'a BTreeSet<String>,
    path_var: Option<&'a OsStr>,
}

fn shape_defect(command: &str, context: &Resolve<'_>) -> Option<String> {
    let words = shell_words_with_spans(command)?
        .into_iter()
        .map(|word| word.value)
        .collect::<Vec<_>>();
    let first = words.first()?;
    if reads_as_prose(command, first, context) {
        return Some(format!(
            "`{first}` is not a shell builtin, a command on PATH, or a workspace path"
        ));
    }
    test_operand_defect(&words)
}

/// `line` is the command as written; `first` is its first shell word.
fn reads_as_prose(line: &str, first: &str, context: &Resolve<'_>) -> bool {
    // A word starting with `#` begins a shell comment, which runs as a no-op.
    if first.starts_with('#')
        || SHELL_BUILTINS.contains(&first)
        || resolves_on_path(first, context.path_var)
        || is_workspace_path(first, context)
    {
        return false;
    }
    has_no_command_character(first) || reads_as_sentence(line)
}

/// No ASCII letter, digit, `.`, `/`, `_`, or `-`: no command name looks so.
fn has_no_command_character(word: &str) -> bool {
    !word
        .chars()
        .any(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '/' | '_' | '-'))
}

/// Sentence punctuation that may close the last word.
const SENTENCE_END: &[char] = &['.', '!', '?', '。', '！', '？'];
/// A sentence has at least this many words.
const MIN_SENTENCE_WORDS: usize = 3;

/// Reads the raw words as written, so quoted data, globs, and joined
/// fragments never pass as letters: every word is letters only, at least
/// three words, and the last one closes with sentence punctuation.
fn reads_as_sentence(line: &str) -> bool {
    let raw = line.split_whitespace().collect::<Vec<_>>();
    let Some((last, rest)) = raw.split_last() else {
        return false;
    };
    let Some(closed) = last
        .strip_suffix(SENTENCE_END)
        .filter(|_| raw.len() >= MIN_SENTENCE_WORDS)
    else {
        return false;
    };
    rest.iter().all(|word| is_letters(word)) && is_letters(closed)
}

fn is_letters(word: &str) -> bool {
    !word.is_empty() && word.chars().all(char::is_alphabetic)
}

fn resolves_on_path(word: &str, path_var: Option<&OsStr>) -> bool {
    let Some(path_var) = path_var else {
        return false;
    };
    std::env::split_paths(path_var).any(|dir| is_executable_file(&dir.join(word)))
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

fn is_workspace_path(word: &str, context: &Resolve<'_>) -> bool {
    let Ok(path) = verifier_obligations::normalized(word) else {
        return false;
    };
    context.declared.contains(&path)
        || context
            .work_root
            .is_some_and(|root| root.join(&path).exists())
}

/// Reads `test ...` / `[ ... ]` as primaries joined by `-a` / `-o`. Only the
/// listed unary file operators are judged; any other shape stops the scan
/// without a finding, so an unusual but valid expression is never rejected.
fn test_operand_defect(words: &[String]) -> Option<String> {
    let args = match words.first()?.as_str() {
        "test" => &words[1..],
        "[" => match words.split_last() {
            Some((last, rest)) if last == "]" => &rest[1..],
            _ => return Some("`[` has no closing `]`".to_string()),
        },
        _ => return None,
    };
    let mut index = 0;
    while index < args.len() {
        while matches!(args.get(index).map(String::as_str), Some("!" | "(")) {
            index += 1;
        }
        let word = args.get(index)?.as_str();
        let next = args.get(index + 1).map(String::as_str);
        if FILE_OPERATORS.contains(&word) {
            if next.is_none() {
                return Some(format!("file operator `{word}` has no operand"));
            }
            index += 2;
        } else if next.is_some_and(|next| BINARY_OPERATORS.contains(&next)) {
            args.get(index + 2)?;
            index += 3;
        } else if let Some(operator) = next.filter(|next| FILE_OPERATORS.contains(next)) {
            return Some(format!(
                "operand `{word}` comes before file operator `{operator}`"
            ));
        } else {
            index += 1;
        }
        while args.get(index).map(String::as_str) == Some(")") {
            index += 1;
        }
        match args.get(index).map(String::as_str) {
            None => return None,
            Some("-a" | "-o") => index += 1,
            Some(_) => return None,
        }
    }
    None
}

#[cfg(test)]
mod tests;
