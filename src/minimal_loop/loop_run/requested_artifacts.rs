//! Extracts the artifact paths a prompt asks for.
//!
//! Two sources feed the extraction. A `Required final artifacts` block is an
//! explicit declaration: its entries keep the long-standing parsing, special
//! characters included. Backticked spans in free text are only an inference,
//! so a span that reads as code or quoted data — whitespace, quotes,
//! brackets, shell or expression symbols, or an all-digit `03/04/2026` — is
//! not promoted to an obligation. Brackets are accepted only as a whole route
//! segment such as `[id]`, `[...slug]`, `[[...opt]]`, or `(shop)`. Both
//! sources still pass the same
//! workspace-relative, metadata, escape, and symlink checks.

use std::collections::BTreeSet;
use std::path::Path;

use crate::tools::path_guard::{resolve_optional_existing, validate_workspace_relative};

#[cfg(test)]
mod tests;

pub(crate) fn extract_requested_artifact_paths(root: &Path, prompt: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let mut seen = BTreeSet::new();
    let mut in_required_block = false;
    for line in prompt.lines() {
        let trimmed = line.trim();
        if trimmed
            .to_ascii_lowercase()
            .starts_with("required final artifacts")
        {
            in_required_block = true;
            continue;
        }
        if in_required_block {
            if trimmed.is_empty() {
                continue;
            }
            if !is_artifact_list_line(trimmed) && looks_like_section_boundary(trimmed) {
                in_required_block = false;
            } else if let Some(candidate) = artifact_candidate_from_line(trimmed)
                && requested_artifact_path_allowed(root, &candidate)
                && seen.insert(candidate.clone())
            {
                paths.push(candidate);
                continue;
            }
        }
        for candidate in backticked_candidates(trimmed) {
            if looks_like_artifact_path(&candidate)
                && !reads_as_code_or_quoted_data(&candidate)
                && requested_artifact_path_allowed(root, &candidate)
                && seen.insert(candidate.clone())
            {
                paths.push(candidate);
            }
        }
    }
    paths
}

fn requested_artifact_path_allowed(root: &Path, raw: &str) -> bool {
    if validate_workspace_relative(raw).is_err() {
        return false;
    }
    let path = Path::new(raw);
    let blocked = [
        ".commandagent",
        ".anvil",
        ".git",
        "target",
        "node_modules",
        ".next",
    ];
    if path.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .is_some_and(|part| blocked.contains(&part))
    }) {
        return false;
    }
    resolve_optional_existing(root, raw).is_ok()
}

/// Whether a backticked free-text span reads as code or quoted data rather
/// than a file path. Only the inference from free text applies this; an
/// explicit declaration is never filtered by it.
fn reads_as_code_or_quoted_data(value: &str) -> bool {
    const CODE_OR_DATA_CHARS: &[char] = &[
        ' ', '\t', '"', '\'', '`', '(', ')', '[', ']', '{', '}', '=', '<', '>', ';', '|', '&', '$',
        '*', '?', ',',
    ];
    value
        .split('/')
        .filter(|segment| !is_route_segment(segment))
        .any(|segment| segment.contains(CODE_OR_DATA_CHARS))
        || all_segments_are_digits(value)
}

/// A whole path segment that names a route parameter or group:
/// `[name]`, `[...name]`, `[[...name]]`, or `(name)`, where `name` is a
/// non-empty run of ASCII letters, digits, `_`, or `-`. Only such a segment
/// may carry brackets; `x[0]` or `f(a)` still reads as code.
fn is_route_segment(segment: &str) -> bool {
    let name = segment
        .strip_prefix("[[...")
        .and_then(|rest| rest.strip_suffix("]]"))
        .or_else(|| {
            segment
                .strip_prefix("[...")
                .and_then(|rest| rest.strip_suffix(']'))
        })
        .or_else(|| {
            segment
                .strip_prefix('[')
                .and_then(|rest| rest.strip_suffix(']'))
        })
        .or_else(|| {
            segment
                .strip_prefix('(')
                .and_then(|rest| rest.strip_suffix(')'))
        });
    name.is_some_and(|name| {
        !name.is_empty()
            && name
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    })
}

/// `03/04/2026` and `1/2` are dates and fractions, not paths.
fn all_segments_are_digits(value: &str) -> bool {
    value.contains('/')
        && value
            .split('/')
            .all(|segment| !segment.is_empty() && segment.chars().all(|ch| ch.is_ascii_digit()))
}

fn is_artifact_list_line(line: &str) -> bool {
    let line = line.trim_start();
    line.starts_with("- ")
        || line.starts_with("* ")
        || line.chars().next().is_some_and(|ch| ch.is_ascii_digit())
}

fn looks_like_section_boundary(line: &str) -> bool {
    line.ends_with(':') || line.starts_with('#')
}

fn artifact_candidate_from_line(line: &str) -> Option<String> {
    let mut value = line.trim();
    value = value
        .trim_start_matches("- ")
        .trim_start_matches("* ")
        .trim_start();
    if let Some((head, tail)) = value.split_once(". ")
        && head.chars().all(|ch| ch.is_ascii_digit())
    {
        value = tail.trim_start();
    }
    let first = value.split_whitespace().next().unwrap_or_default();
    let candidate = first
        .trim_matches('`')
        .trim_matches('"')
        .trim_matches('\'')
        .trim_end_matches([',', ';']);
    if looks_like_artifact_path(candidate) {
        Some(candidate.to_string())
    } else {
        None
    }
}

fn backticked_candidates(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find('`') {
        let after_start = &rest[start + 1..];
        let Some(end) = after_start.find('`') else {
            break;
        };
        out.push(after_start[..end].trim().to_string());
        rest = &after_start[end + 1..];
    }
    out
}

fn looks_like_artifact_path(value: &str) -> bool {
    if value.is_empty() || value.starts_with("http://") || value.starts_with("https://") {
        return false;
    }
    if value.contains('/') {
        return true;
    }
    matches!(
        value,
        "Cargo.toml"
            | "README.md"
            | "package.json"
            | "tsconfig.json"
            | "index.html"
            | "pyproject.toml"
    ) || Path::new(value).extension().is_some_and(|ext| {
        matches!(
            ext.to_str().unwrap_or_default(),
            "js" | "jsx"
                | "ts"
                | "tsx"
                | "rs"
                | "py"
                | "md"
                | "txt"
                | "csv"
                | "json"
                | "toml"
                | "yaml"
                | "yml"
                | "html"
                | "css"
        )
    })
}
