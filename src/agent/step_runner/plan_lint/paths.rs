use super::PlanLintError;
use std::path::Path;

pub(super) fn lint_expected_path(path: &str) -> Result<(), PlanLintError> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(PlanLintError::InvalidExpectedPath {
            path: path.to_string(),
            reason: "path is empty".to_string(),
        });
    }
    let path_obj = Path::new(trimmed);
    if path_obj.is_absolute() || trimmed.contains("..") {
        return Err(PlanLintError::InvalidExpectedPath {
            path: path.to_string(),
            reason: "path must be repository-relative and cannot contain parent traversal"
                .to_string(),
        });
    }
    if trimmed.contains(':') || trimmed.starts_with("$.") {
        return Err(PlanLintError::InvalidExpectedPath {
            path: path.to_string(),
            reason: "expected_paths must be file paths, not JSON/property selectors".to_string(),
        });
    }
    if trimmed.contains(" or ") || trimmed.contains("||") {
        return Err(PlanLintError::InvalidExpectedPath {
            path: path.to_string(),
            reason: "expected_paths must be one concrete file, not alternatives".to_string(),
        });
    }
    if trimmed.contains('*') || trimmed.contains('?') || trimmed.contains('{') {
        return Err(PlanLintError::InvalidExpectedPath {
            path: path.to_string(),
            reason: "expected_paths must be concrete files, not glob patterns".to_string(),
        });
    }
    if is_dependency_cache_path(trimmed) {
        return Err(PlanLintError::InvalidExpectedPath {
            path: path.to_string(),
            reason: "expected_paths must not name generated dependency caches".to_string(),
        });
    }
    if looks_like_version(trimmed) {
        return Err(PlanLintError::InvalidExpectedPath {
            path: path.to_string(),
            reason: "expected_paths must be file paths, not version strings".to_string(),
        });
    }
    if !looks_like_file_path(trimmed) {
        return Err(PlanLintError::InvalidExpectedPath {
            path: path.to_string(),
            reason: "expected_paths must name concrete files".to_string(),
        });
    }
    Ok(())
}

fn is_dependency_cache_path(path: &str) -> bool {
    path == "node_modules"
        || path.starts_with("node_modules/")
        || path == ".next"
        || path.starts_with(".next/")
        || path == ".venv"
        || path.starts_with(".venv/")
        || path == "target"
        || path.starts_with("target/")
        || path == ".git"
        || path.starts_with(".git/")
}

fn looks_like_file_path(path: &str) -> bool {
    if path.contains('/') {
        return !path.ends_with('/');
    }
    if matches!(
        path,
        "Dockerfile" | "Makefile" | "README" | "LICENSE" | "Cargo.lock"
    ) {
        return true;
    }
    if is_dotfile_path(path) {
        return true;
    }
    let Some(extension) = Path::new(path).extension().and_then(|ext| ext.to_str()) else {
        return false;
    };
    matches!(
        extension,
        "cjs"
            | "css"
            | "go"
            | "html"
            | "js"
            | "json"
            | "jsx"
            | "lock"
            | "md"
            | "mjs"
            | "py"
            | "rs"
            | "toml"
            | "ts"
            | "tsx"
            | "txt"
            | "yaml"
            | "yml"
    )
}

fn is_dotfile_path(path: &str) -> bool {
    let Some(rest) = path.strip_prefix('.') else {
        return false;
    };
    !rest.is_empty()
        && !rest.ends_with('.')
        && rest
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
}

fn looks_like_version(value: &str) -> bool {
    let mut saw_dot = false;
    let mut saw_digit = false;
    for ch in value.chars() {
        if ch == '.' {
            saw_dot = true;
        } else if ch.is_ascii_digit() {
            saw_digit = true;
        } else {
            return false;
        }
    }
    saw_dot && saw_digit
}
