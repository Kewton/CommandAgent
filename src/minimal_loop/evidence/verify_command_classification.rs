//! Command-local evidence. Classification never substitutes for executing a verifier.

use super::{VerifyCommandKind, WorkspaceEvidence, has_test_artifact};

#[path = "verify_command_classification/node_checks.rs"]
mod node_checks;

pub(super) fn node_command_kind(
    command: &str,
    workspace: &WorkspaceEvidence,
) -> Option<VerifyCommandKind> {
    let program = command.split_whitespace().next()?;
    if !matches!(program, "node" | "npm" | "pnpm" | "yarn") {
        return None;
    }
    let weak = || VerifyCommandKind::Weak("node_smoke_without_assertion".into());
    let Some(words) = single_command_words(command) else {
        return Some(weak());
    };
    let args = &words[1..];
    let test_runner = if program == "node" {
        args.first().is_some_and(|arg| arg == "--test")
    } else {
        args.first().is_some_and(|arg| arg == "test")
            || args.first().is_some_and(|arg| arg == "run")
                && args.get(1).is_some_and(|arg| arg == "test")
    };
    if test_runner {
        return Some(if has_test_artifact(workspace) {
            VerifyCommandKind::Test
        } else {
            VerifyCommandKind::Weak("node_test_without_test_artifact".into())
        });
    }
    if program != "node" {
        return None;
    }
    // These exact generated predicates still execute in the completion contract.
    // Treating them as syntax checks does not let them supply business evidence.
    if crate::planner::profiles::nextjs::recovery_authority::is_generated_hook_check(command) {
        return Some(VerifyCommandKind::StaticSyntax);
    }
    let Some(first) = args.first() else {
        return Some(weak());
    };
    let checked = match first.as_str() {
        "-e" | "--eval" | "-p" | "--print" => args
            .get(1)
            .is_some_and(|source| node_checks::has_failure_check(source)),
        option if option.starts_with('-') => false,
        path => {
            let path = std::path::Path::new(path);
            let mut normalized = std::path::PathBuf::new();
            for component in path.components() {
                match component {
                    std::path::Component::CurDir => {}
                    std::path::Component::Normal(part) => normalized.push(part),
                    _ => return Some(weak()),
                }
            }
            workspace
                .source_files
                .iter()
                .chain(&workspace.test_files)
                .find(|file| std::path::Path::new(&file.rel) == normalized)
                .is_some_and(|file| node_checks::has_failure_check(&file.content))
        }
    };
    Some(if checked {
        VerifyCommandKind::Test
    } else {
        weak()
    })
}

/// Deliberately accept only literal words in a single foreground command. Shell
/// control/expansion must never turn a failing check into successful evidence.
pub(crate) fn single_command_words(command: &str) -> Option<Vec<String>> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut started = false;
    let mut chars = command.chars();
    while let Some(ch) = chars.next() {
        if quote == Some('\'') {
            if ch == '\'' {
                quote = None;
            } else {
                word.push(ch);
            }
        } else if ch == '$' || ch == '`' {
            return None;
        } else if ch == '\\' {
            let escaped = chars.next()?;
            if quote == Some('"') && !matches!(escaped, '"' | '\\') {
                word.push('\\');
            }
            word.push(escaped);
            started = true;
        } else if quote == Some('"') {
            if ch == '"' {
                quote = None;
            } else {
                word.push(ch);
            }
        } else if matches!(ch, '\'' | '"') {
            quote = Some(ch);
            started = true;
        } else if matches!(ch, ';' | '|' | '&' | '>' | '<' | '\n' | '\r' | '(' | ')') {
            return None;
        } else if ch.is_whitespace() {
            if started {
                words.push(std::mem::take(&mut word));
                started = false;
            }
        } else {
            word.push(ch);
            started = true;
        }
    }
    if quote.is_some() {
        return None;
    }
    if started {
        words.push(word);
    }
    (!words.is_empty()).then_some(words)
}

#[cfg(test)]
#[path = "verify_command_classification/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "verify_command_classification/issue474_tests.rs"]
mod issue474_tests;
