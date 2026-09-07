//! Display-only normalization; this module grants no execution authority.
use std::path::{Component, Path};

use super::RecoveryHandoff;

const OMITTED_PATH: &str = "[external or unresolved path omitted; use workspace-relative paths]";

pub(super) fn handoff(root: Option<&Path>, source: &RecoveryHandoff) -> RecoveryHandoff {
    let text = |value: &str| display_text(root, value);
    let list = |values: &[String]| values.iter().map(|value| text(value)).collect();
    RecoveryHandoff {
        profile: text(&source.profile),
        original_goal: text(&source.original_goal),
        failed_phase: source.failed_phase.as_deref().map(text),
        failed_step: source.failed_step.as_deref().map(text),
        failure_kind: text(&source.failure_kind),
        failure_evidence: list(&source.failure_evidence),
        missing_paths: list(&source.missing_paths),
        missing_capabilities: list(&source.missing_capabilities),
        verify_commands: list(&source.verify_commands),
        changed_paths: list(&source.changed_paths),
        repair_targets: list(&source.repair_targets),
    }
}

pub(super) fn display_text(root: Option<&Path>, value: &str) -> String {
    let mut text = value.to_string();
    if let Some(root) = root {
        let mut roots = vec![root.to_path_buf()];
        if let Ok(canonical) = root.canonicalize() {
            roots.push(canonical);
        }
        let mut prefixes = Vec::new();
        for root in roots {
            let root = root.to_string_lossy().trim_end_matches('/').to_string();
            if root.is_empty() || !Path::new(&root).is_absolute() {
                continue;
            }
            let redacted = crate::eval_events::scrub_sensitive_text(&root);
            prefixes.extend([root, redacted.replace("<user>", "<redacted>"), redacted]);
        }
        prefixes.sort_by_key(|prefix| std::cmp::Reverse(prefix.len()));
        prefixes.dedup();
        for prefix in prefixes {
            text = relative_prefix(&text, &prefix);
        }
    }
    // Do not leave a redacted absolute/relative path that a model could copy
    // back into Bash. Standalone secret placeholders remain non-path text.
    text.split_inclusive(char::is_whitespace)
        .map(|token| {
            let whitespace = &token[token.trim_end().len()..];
            let token = crate::eval_events::scrub_sensitive_text(token.trim_end());
            if crate::tools::placeholder_path::detected(&token) && token.contains('/') {
                format!("{OMITTED_PATH}{whitespace}")
            } else {
                format!("{token}{whitespace}")
            }
        })
        .collect()
}

fn relative_prefix(text: &str, prefix: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0;
    for (start, _) in text.match_indices(prefix) {
        let end = start + prefix.len();
        let before = text[..start].chars().next_back();
        let after = text[end..].chars().next();
        let boundary = before.is_none_or(path_boundary)
            && after.is_none_or(|ch| ch == '/' || path_boundary(ch));
        let safe_suffix = decoded_word_suffix(text, start, end).is_some_and(|suffix| {
            (suffix.is_empty() || suffix.starts_with('/'))
                && !Path::new(&suffix)
                    .components()
                    .any(|component| component == Component::ParentDir)
        });
        if boundary && safe_suffix {
            out.push_str(&text[cursor..start]);
            out.push('.');
            cursor = end;
        }
    }
    out.push_str(&text[cursor..]);
    out
}

/// Inspect the entire shell word, including quoted/escaped whitespace and
/// concatenated quotes. Decoding is only for the check; output retains its
/// original spelling. Incomplete syntax and expansions are not normalized.
fn decoded_word_suffix(text: &str, start: usize, end: usize) -> Option<String> {
    let mut quote = None;
    let mut quote_start = 0;
    let mut escaped = false;
    for (offset, ch) in text[..end].char_indices() {
        if escaped {
            escaped = false;
        } else if ch == '\\' && quote != Some('\'') {
            escaped = true;
        } else if quote == Some(ch) {
            quote = None;
        } else if quote.is_none() && matches!(ch, '\'' | '"' | '`') {
            quote = Some(ch);
            quote_start = offset;
        }
    }
    if escaped
        || (quote.is_some()
            && (quote_start >= start
                || text[..quote_start].ends_with('$')
                || (quote != Some('\'') && text[quote_start..end].contains('$'))
                || text[quote_start + 1..start]
                    .chars()
                    .any(|ch| matches!(ch, '\'' | '"' | '`'))))
        || text[..start]
            .chars()
            .next_back()
            .is_some_and(|ch| matches!(ch, '\'' | '"' | '`') && quote != Some(ch))
    {
        return None;
    }
    let mut suffix = String::new();
    let mut chars = text[end..].chars();
    while let Some(ch) = chars.next() {
        match (quote, ch) {
            (Some(delimiter), ch) if ch == delimiter => quote = None,
            (Some('\''), ch) => suffix.push(ch),
            (_, '\\') => {
                let next = chars.next()?;
                if next != '\n' {
                    if quote == Some('"') && !matches!(next, '\\' | '$' | '`' | '"') {
                        suffix.push('\\');
                    }
                    suffix.push(next);
                }
            }
            (None, '\'' | '"') => quote = Some(ch),
            // Backticks can delimit a displayed path, but nested shell syntax
            // or expansions cannot establish a literal, complete path here.
            (Some('`'), '\'' | '"') | (_, '$' | '`') => return None,
            (None, ch)
                if ch.is_whitespace() || matches!(ch, '(' | ')' | ';' | '&' | '|' | '<' | '>') =>
            {
                break;
            }
            (_, ch) => suffix.push(ch),
        }
    }
    quote.is_none().then_some(suffix)
}

fn path_boundary(ch: char) -> bool {
    ch.is_whitespace()
        || matches!(
            ch,
            '\'' | '"' | '`' | '(' | ')' | ',' | ';' | ':' | '=' | '&' | '|' | '<' | '>'
        )
}

pub(super) fn shell_quote_path(path: &Path) -> String {
    let display = workspace_relative_handoff_path(path);
    if display
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/'))
    {
        display
    } else {
        format!("{display:?}")
    }
}

pub(crate) fn workspace_relative_handoff_path(path: &Path) -> String {
    let components = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    if let Some(index) = components
        .iter()
        .position(|part| matches!(part.as_str(), ".commandagent" | ".anvil"))
    {
        return components[index..].join("/");
    }
    path.to_string_lossy().replace('\\', "/")
}

pub(super) fn redacted_list(items: &[String]) -> Vec<String> {
    items
        .iter()
        .map(|item| display_text(None, &crate::eval_events::body_snippet(item)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issue441_review_checks_complete_path_suffixes() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/corpus/apps/issue441-placeholder/fixtures/recovery-suffixes.json"
        ))
        .unwrap();
        let root = Path::new(fixture["root"].as_str().unwrap());
        for case in fixture["cases"].as_array().unwrap() {
            let input = case["input"].as_str().unwrap();
            let expected = case["expected"]
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| display_text(None, input));
            assert_eq!(
                display_text(Some(root), input),
                expected,
                "{}: {input}",
                case["id"]
            );
        }
    }

    #[test]
    fn issue441_normalizes_only_known_root_before_redaction() {
        let root = Path::new("/Users/alice/work/project");
        for value in [
            "cat /Users/alice/work/project/src/a.ts",
            "cat /Users/<user>/work/project/src/a.ts",
            "cat /Users/<redacted>/work/project/src/a.ts",
        ] {
            assert_eq!(display_text(Some(root), value), "cat ./src/a.ts");
        }
        for value in [
            "cat /Users/alice/work/project-other/src/a.ts",
            "cat /Users/<user>/other/src/a.ts",
            "cat /Users/alice/work/project/../secret.txt",
        ] {
            let result = display_text(Some(root), value);
            assert!(!result.contains("<user>"), "{result}");
            assert!(!result.contains("alice"), "{result}");
            assert!(result.contains(OMITTED_PATH), "{result}");
        }
        assert_eq!(
            display_text(Some(root), "cd /Users/alice/work/project && pwd"),
            "cd . && pwd"
        );
        assert_eq!(
            display_text(Some(root), "cat</Users/alice/work/project/a.ts"),
            "cat<./a.ts"
        );
    }

    #[test]
    fn issue441_preserves_line_breaks_spaces_and_utf8() {
        let root = Path::new("/home/alice/my project");
        let value = "failed: '/home/alice/my project/src/日本語.ts'\nnext: './src/a.ts'";
        assert_eq!(
            display_text(Some(root), value),
            "failed: './src/日本語.ts'\nnext: './src/a.ts'"
        );
        assert_eq!(
            display_text(None, "api_key=sk-secret\n  next line"),
            "<redacted>\n  next line"
        );
    }
}
