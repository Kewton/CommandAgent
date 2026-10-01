//! Shell brace and glob expansion for Bash write-target confinement.
//!
//! The literal path proof in [`super`] cannot see where a Bash *write target*
//! actually lands when the word carries shell expansion syntax. A glob (`*`,
//! `?`, `[...]`) only matches names that already exist, and a brace
//! (`{a,b}`, `{1..9}`) is rewritten before the command runs, so the
//! pre-expansion spelling does not exist on disk. The literal proof then falls
//! back to the nearest existing parent -- the workspace root -- and reports the
//! target as inside the workspace. At run time the shell expands the word and
//! can reach an intermediate symlink that leaves the workspace, or a `..`
//! produced by a brace such as `{.,}./f`.
//!
//! This module expands the word the way the shell would and re-runs the
//! literal proof on every result. It never reads a directory outside the
//! workspace root and it stops with an honest failure once an expansion limit
//! is exceeded, so a symlink loop or an unbounded glob cannot hang the guard.

use std::collections::VecDeque;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, bail};
use globset::{Glob, GlobMatcher};

/// Upper bound on the number of strings a single brace word may expand into.
pub(super) const MAX_BRACE_EXPANSIONS: usize = 256;
/// Upper bound on the number of names a single word's globs may match.
pub(super) const MAX_GLOB_MATCHES: usize = 4096;

/// Whether the word carries any shell expansion syntax this module understands.
pub(super) fn contains_expansion(raw: &str) -> bool {
    raw.chars().any(|ch| matches!(ch, '*' | '?' | '[' | '{'))
}

/// Expand `raw` and prove every expansion with the literal write-target proof.
pub(super) fn ensure_expanded_write_target(root: &Path, raw: &str) -> anyhow::Result<()> {
    for expanded in expand_braces(raw)? {
        if has_glob_meta(&expanded) {
            match expand_glob(root, &expanded)? {
                Some(matches) => {
                    for matched in matches {
                        super::ensure_single_target(root, &matched)?;
                    }
                }
                None => super::ensure_single_target(root, &expanded)?,
            }
        } else {
            super::ensure_single_target(root, &expanded)?;
        }
    }
    Ok(())
}

fn has_glob_meta(value: &str) -> bool {
    value.chars().any(|ch| matches!(ch, '*' | '?' | '['))
}

fn expand_braces(raw: &str) -> anyhow::Result<Vec<String>> {
    let mut pending = VecDeque::from([raw.to_string()]);
    let mut results = Vec::new();
    while let Some(item) = pending.pop_front() {
        match first_expandable_brace(&item) {
            Some(expansion) => {
                for alternative in expansion.alternatives {
                    let mut next = String::with_capacity(item.len() + alternative.len());
                    next.push_str(&item[..expansion.start]);
                    next.push_str(&alternative);
                    next.push_str(&item[expansion.end..]);
                    pending.push_back(next);
                }
            }
            None => {
                results.push(item);
                if results.len() > MAX_BRACE_EXPANSIONS {
                    bail!(
                        "Bash write target brace expansion exceeds the {MAX_BRACE_EXPANSIONS}-expansion limit; the target cannot be proven to remain in the workspace"
                    );
                }
            }
        }
    }
    Ok(results)
}

struct BraceExpansion {
    start: usize,
    end: usize,
    alternatives: Vec<String>,
}

fn first_expandable_brace(input: &str) -> Option<BraceExpansion> {
    let bytes = input.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'{' {
            index += 1;
            continue;
        }
        let start = index;
        let mut depth = 0usize;
        let mut end = None;
        let mut cursor = start;
        while cursor < bytes.len() {
            match bytes[cursor] {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(cursor);
                        break;
                    }
                }
                _ => {}
            }
            cursor += 1;
        }
        let end = end?;
        if let Some(alternatives) = brace_alternatives(&input[start + 1..end]) {
            return Some(BraceExpansion {
                start,
                end: end + 1,
                alternatives,
            });
        }
        // This brace is literal; a nested brace may still be expandable, so
        // resume just after its opening brace rather than skipping it whole.
        index = start + 1;
    }
    None
}

fn brace_alternatives(content: &str) -> Option<Vec<String>> {
    split_top_level_commas(content).or_else(|| range_alternatives(content))
}

fn split_top_level_commas(content: &str) -> Option<Vec<String>> {
    let mut depth = 0i32;
    let mut current = String::new();
    let mut parts = Vec::new();
    let mut saw_comma = false;
    for ch in content.chars() {
        match ch {
            '{' => {
                depth += 1;
                current.push(ch);
            }
            '}' => {
                depth -= 1;
                current.push(ch);
            }
            ',' if depth == 0 => {
                saw_comma = true;
                parts.push(std::mem::take(&mut current));
            }
            _ => current.push(ch),
        }
    }
    if !saw_comma {
        return None;
    }
    parts.push(current);
    Some(parts)
}

fn range_alternatives(content: &str) -> Option<Vec<String>> {
    let parts: Vec<&str> = content.split("..").collect();
    match parts.as_slice() {
        [from, to] => range_values(from, to, 1),
        [from, to, step] => {
            let step = step.parse::<i64>().ok()?;
            range_values(from, to, step)
        }
        _ => None,
    }
}

fn range_values(from: &str, to: &str, step: i64) -> Option<Vec<String>> {
    let step = step.abs();
    if step == 0 {
        return None;
    }
    if let (Ok(start), Ok(end)) = (from.parse::<i64>(), to.parse::<i64>()) {
        return Some(
            integer_range(start, end, step)
                .into_iter()
                .map(|value| value.to_string())
                .collect(),
        );
    }
    let start = single_ascii_char(from)?;
    let end = single_ascii_char(to)?;
    let values = integer_range(i64::from(start), i64::from(end), step)
        .into_iter()
        .map(|value| char::from(value as u8).to_string())
        .collect();
    Some(values)
}

fn integer_range(start: i64, end: i64, step: i64) -> Vec<i64> {
    let direction = if end >= start { 1 } else { -1 };
    let mut values = Vec::new();
    let mut value = start;
    while (direction > 0 && value <= end) || (direction < 0 && value >= end) {
        values.push(value);
        value += direction * step;
    }
    values
}

fn single_ascii_char(value: &str) -> Option<u8> {
    let mut chars = value.chars();
    let ch = chars.next()?;
    if chars.next().is_some() || !ch.is_ascii() {
        return None;
    }
    Some(ch as u8)
}

/// Expand a word that still contains glob metacharacters.
///
/// Returns `Some(paths)` when the glob matched at least one existing name and
/// `None` when it matched nothing, so the caller can fall back to treating the
/// word as the shell would: an unmatched glob is passed through literally.
fn expand_glob(root: &Path, pattern: &str) -> anyhow::Result<Option<Vec<String>>> {
    let root_canonical = root
        .canonicalize()
        .context("workspace root is not accessible")?;
    let path = Path::new(pattern);
    let remainder = if path.is_absolute() {
        match path
            .strip_prefix(&root_canonical)
            .or_else(|_| path.strip_prefix(root))
        {
            Ok(remainder) => remainder,
            // An absolute word outside the root cannot be enumerated here; the
            // literal proof in the caller rejects it.
            Err(_) => return Ok(None),
        }
    } else {
        path
    };

    let mut current = vec![root_canonical.clone()];
    let mut matched_total = 0usize;
    let components: Vec<Component> = remainder.components().collect();
    for (index, component) in components.iter().enumerate() {
        let is_last = index + 1 == components.len();
        let name = match component {
            Component::Normal(name) => name.to_string_lossy().to_string(),
            Component::CurDir => ".".to_string(),
            Component::ParentDir => "..".to_string(),
            Component::RootDir | Component::Prefix(_) => continue,
        };
        if !has_glob_meta(&name) {
            current = current.into_iter().map(|base| base.join(&name)).collect();
            continue;
        }
        let (next, total, matched) =
            enumerate_component(&root_canonical, &current, &name, matched_total, !is_last)?;
        if !matched {
            return Ok(None);
        }
        matched_total = total;
        current = next;
    }
    Ok(Some(
        current
            .into_iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect(),
    ))
}

fn enumerate_component(
    root_canonical: &Path,
    current: &[PathBuf],
    name: &str,
    mut matched_total: usize,
    intermediate: bool,
) -> anyhow::Result<(Vec<PathBuf>, usize, bool)> {
    let Some(matcher) = compile_component_matcher(name) else {
        // The component is not a valid glob; the shell treats it literally.
        let next = current.iter().map(|base| base.join(name)).collect();
        return Ok((next, matched_total, true));
    };
    let dot_leading = name.starts_with('.');
    let mut next = Vec::new();
    for base in current {
        let Ok(base_canonical) = base.canonicalize() else {
            continue;
        };
        if !base_canonical.starts_with(root_canonical) {
            bail!(
                "Bash write target glob expands outside the workspace root at `{}`",
                base.display()
            );
        }
        let Ok(entries) = std::fs::read_dir(&base_canonical) else {
            continue;
        };
        let mut candidates = Vec::new();
        for entry in entries.flatten() {
            let entry_name = entry.file_name().to_string_lossy().to_string();
            if !dot_leading && entry_name.starts_with('.') {
                continue;
            }
            candidates.push(entry_name);
        }
        if dot_leading {
            candidates.push(".".to_string());
            candidates.push("..".to_string());
        }
        for candidate in candidates {
            if !matcher.is_match(&candidate) {
                continue;
            }
            if candidate == ".." {
                bail!(
                    "Bash write target glob matches `..`, which cannot be proven to remain in the workspace"
                );
            }
            let child = base_canonical.join(&candidate);
            let child_canonical = child.canonicalize().ok();
            if let Some(canonical) = &child_canonical
                && !canonical.starts_with(root_canonical)
            {
                bail!(
                    "Bash write target glob match `{candidate}` resolves outside the workspace root"
                );
            }
            if intermediate {
                // A glob used as a directory component only expands to
                // directories the shell can descend into. A regular file (or a
                // symlink the shell cannot traverse) drops the whole branch.
                match &child_canonical {
                    Some(canonical) if canonical.is_dir() => {}
                    _ => continue,
                }
            } else if child_canonical.is_none() {
                bail!(
                    "Bash write target glob match `{candidate}` cannot be resolved inside the workspace root"
                );
            }
            matched_total += 1;
            if matched_total > MAX_GLOB_MATCHES {
                bail!(
                    "Bash write target glob exceeds the {MAX_GLOB_MATCHES}-match limit; the target cannot be proven to remain in the workspace"
                );
            }
            next.push(child);
        }
    }
    let matched = !next.is_empty();
    Ok((next, matched_total, matched))
}

fn compile_component_matcher(name: &str) -> Option<GlobMatcher> {
    let collapsed = collapse_stars(name);
    Glob::new(&collapsed)
        .ok()
        .map(|glob| glob.compile_matcher())
}

/// Collapse runs of `*` within one path component. Inside a single component
/// the shell's `**` behaves like `*`, and globset rejects `**` when it is not
/// the whole component.
fn collapse_stars(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut previous_star = false;
    for ch in name.chars() {
        if ch == '*' {
            if previous_star {
                continue;
            }
            previous_star = true;
        } else {
            previous_star = false;
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_expansion_table() {
        assert!(contains_expansion("src/[id]"));
        assert!(contains_expansion("lin*/secret"));
        assert!(contains_expansion("a?b"));
        assert!(contains_expansion("{a,b}"));
        assert!(!contains_expansion("plain/path.txt"));
    }

    #[test]
    fn collapse_stars_collapses_runs() {
        assert_eq!(collapse_stars("a**b"), "a*b");
        assert_eq!(collapse_stars("**"), "*");
        assert_eq!(collapse_stars("a*b"), "a*b");
    }

    #[test]
    fn expand_braces_table() {
        assert_eq!(expand_braces("plain").unwrap(), ["plain"]);
        assert_eq!(expand_braces("{a,b}").unwrap(), ["a", "b"]);
        assert_eq!(
            expand_braces("pre{a,b}post").unwrap(),
            ["preapost", "prebpost"]
        );
        assert_eq!(expand_braces("{a,{b,c}}").unwrap(), ["a", "b", "c"]);
        assert_eq!(expand_braces("{a{b,c}}").unwrap(), ["{ab}", "{ac}"]);
        assert_eq!(expand_braces("{k..m}").unwrap(), ["k", "l", "m"]);
        assert_eq!(expand_braces("{1..3}").unwrap(), ["1", "2", "3"]);
        assert_eq!(expand_braces("{3..1}").unwrap(), ["3", "2", "1"]);
        assert_eq!(expand_braces("{1..5..2}").unwrap(), ["1", "3", "5"]);
        assert_eq!(expand_braces("{a}").unwrap(), ["{a}"]);
        assert_eq!(expand_braces("{x}y{z}").unwrap(), ["{x}y{z}"]);
    }

    #[test]
    fn expand_braces_rejects_over_the_limit() {
        let word = "{a,b}".repeat(12);
        let error = expand_braces(&word).unwrap_err();
        assert!(error.to_string().contains("256-expansion limit"), "{error}");
    }

    #[cfg(unix)]
    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ws");
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("sub/f"), "x").unwrap();
        std::fs::create_dir_all(dir.path().join("outside")).unwrap();
        std::fs::write(dir.path().join("outside/secret"), "x").unwrap();
        std::os::unix::fs::symlink(dir.path().join("outside"), root.join("linked-outside"))
            .unwrap();
        std::os::unix::fs::symlink(dir.path().join("outside"), root.join(".hidden-out")).unwrap();
        dir
    }

    #[cfg(unix)]
    #[test]
    fn glob_expansion_matches_inside_and_refuses_escapes() {
        let dir = fixture();
        let root = dir.path().join("ws");

        assert!(expand_glob(&root, "lin*/secret").is_err());
        assert!(expand_glob(&root, "lin*/f").is_err());
        assert!(expand_glob(&root, ".*").is_err());

        assert_eq!(expand_glob(&root, "nomatch*/f").unwrap(), None);

        let matched = expand_glob(&root, "sub/*").unwrap().expect("sub/f match");
        assert_eq!(matched.len(), 1, "{matched:?}");
        assert!(matched[0].ends_with("sub/f"), "{matched:?}");
    }
}
