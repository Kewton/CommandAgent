use super::PlanLintError;
use std::path::Path;

pub(super) fn lint_verifier_command(step_id: &str, command: &str) -> Result<(), PlanLintError> {
    let trimmed = command.trim();
    if contains_unquoted_shell_control(trimmed) {
        return Err(PlanLintError::InvalidVerifierCommand {
            step_id: step_id.to_string(),
            command: command.to_string(),
            reason: "verifier commands must be one simple local check; split shell chaining into separate commands".to_string(),
        });
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower == "true" {
        return Err(PlanLintError::InvalidVerifierCommand {
            step_id: step_id.to_string(),
            command: command.to_string(),
            reason: "no-op verifier is not allowed; use an empty verify list for report-only steps"
                .to_string(),
        });
    }
    if is_source_grep_verifier(trimmed) {
        return Err(PlanLintError::InvalidVerifierCommand {
            step_id: step_id.to_string(),
            command: command.to_string(),
            reason: "source-code behavior must be verified with build/test/check commands; reserve grep -q for literal docs, data, or content checks"
                .to_string(),
        });
    }
    if is_wrong_language_py_compile(trimmed) {
        return Err(PlanLintError::InvalidVerifierCommand {
            step_id: step_id.to_string(),
            command: command.to_string(),
            reason: "python -m py_compile only verifies Python files; use the profile's build/test/check command for other source files"
                .to_string(),
        });
    }
    if starts_with_any(
        &lower,
        &[
            "npm install",
            "npm ci",
            "pnpm install",
            "pip install",
            "python -m pip",
            "cargo add",
            "cargo update",
        ],
    ) {
        return Err(PlanLintError::InvalidVerifierCommand {
            step_id: step_id.to_string(),
            command: command.to_string(),
            reason: "verifier commands must not install dependencies or mutate project state"
                .to_string(),
        });
    }
    if requires_dependency_cache(trimmed) {
        return Err(PlanLintError::InvalidVerifierCommand {
            step_id: step_id.to_string(),
            command: command.to_string(),
            reason: "verifier commands must not require generated dependency caches; report dependency_missing when local dependencies are unavailable".to_string(),
        });
    }
    Ok(())
}

fn requires_dependency_cache(command: &str) -> bool {
    let lower = command.trim().to_ascii_lowercase();
    if !(lower.starts_with("test -f ") || lower.starts_with("test -d ")) {
        return false;
    }
    lower.contains("node_modules/")
        || lower.ends_with(" node_modules")
        || lower.contains(" .venv/")
        || lower.ends_with(" .venv")
}

fn contains_unquoted_shell_control(command: &str) -> bool {
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;
    let mut chars = command.chars().peekable();

    while let Some(ch) = chars.next() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && in_double {
            escaped = true;
            continue;
        }
        match ch {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            ';' if !in_single && !in_double => return true,
            '&' if !in_single && !in_double && chars.peek() == Some(&'&') => return true,
            '|' if !in_single && !in_double && chars.peek() == Some(&'|') => return true,
            _ => {}
        }
    }

    false
}

fn is_wrong_language_py_compile(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    if !(lower.starts_with("python -m py_compile ") || lower.starts_with("python3 -m py_compile "))
    {
        return false;
    }
    command
        .split_whitespace()
        .skip_while(|part| *part != "py_compile")
        .skip(1)
        .filter(|part| !part.starts_with('-'))
        .map(|part| part.trim_matches(|ch| ch == '\'' || ch == '"'))
        .any(|path| !path.ends_with(".py"))
}

fn is_source_grep_verifier(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    if !lower.starts_with("grep -q ") {
        return false;
    }
    let Some(path) = command.split_whitespace().last() else {
        return false;
    };
    let path = path.trim_matches(|ch| ch == '\'' || ch == '"');
    is_source_file_path(path)
}

fn is_source_file_path(path: &str) -> bool {
    matches!(
        Path::new(path).extension().and_then(|ext| ext.to_str()),
        Some(
            "c" | "cc"
                | "cpp"
                | "cs"
                | "go"
                | "h"
                | "hpp"
                | "java"
                | "js"
                | "jsx"
                | "kt"
                | "php"
                | "py"
                | "rb"
                | "rs"
                | "swift"
                | "ts"
                | "tsx"
        )
    )
}

pub(super) fn verifier_runs_build_test(commands: &[String]) -> bool {
    commands.iter().any(|command| {
        let lower = command.trim().to_ascii_lowercase();
        lower == "npm run build"
            || lower == "cargo check"
            || lower == "cargo test"
            || lower == "cargo build"
            || lower.starts_with("python -m pytest")
            || lower.starts_with("python3 -m pytest")
            || lower == "pytest"
            || lower.starts_with("pytest ")
    })
}

fn starts_with_any(value: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|prefix| value.starts_with(prefix))
}
