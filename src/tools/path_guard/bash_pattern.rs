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
//! literal proof on every result. It counts a brace word's expansion before
//! expanding it, never builds an over-limit range, never reads a directory
//! outside the workspace root, and fails closed once an expansion limit is
//! exceeded, so a symlink loop, an unbounded glob, or an overflowing range
//! cannot hang the guard.
//!
//! Known limitations: shell options this guard does not model, so a word that
//! behaves differently under them may be mis-proven and fall back to allowing
//! the literal spelling. They are off in the default non-interactive shell this
//! guard targets: `extglob`, `GLOBIGNORE`, `nocaseglob`, and `dotglob`
//! (`shopt -s dotglob` would let `*` match leading-dot names the guard skips).

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
    if count_expansions(raw, MAX_BRACE_EXPANSIONS).is_none() {
        bail!(
            "Bash write target brace expansion exceeds the {MAX_BRACE_EXPANSIONS}-expansion limit; the target cannot be proven to remain in the workspace"
        );
    }
    let mut expanded = Vec::new();
    expand_into(raw, &mut expanded);
    Ok(expanded)
}

fn expand_into(input: &str, out: &mut Vec<String>) {
    match find_brace(input) {
        Some(span) => {
            for alternative in span.shape_values() {
                let next = format!(
                    "{}{alternative}{}",
                    &input[..span.start],
                    &input[span.end..]
                );
                expand_into(&next, out);
            }
        }
        None => out.push(input.to_string()),
    }
}

/// Count a word's full expansion without materializing it.
///
/// Returns `None` when the count exceeds `limit` or the arithmetic overflows,
/// so the caller rejects before any queue or range vector can grow.
fn count_expansions(input: &str, limit: usize) -> Option<usize> {
    let Some(span) = find_brace(input) else {
        return Some(1);
    };
    if span.shape_count(limit)? > limit {
        return None;
    }
    // The prefix before the first brace holds no brace, so it contributes
    // nothing to the count: only the alternative and the suffix matter.
    let suffix = &input[span.end..];
    let mut total = 0usize;
    for alternative in span.shape_values() {
        let sub = count_expansions(&format!("{alternative}{suffix}"), limit)?;
        total = total.checked_add(sub)?;
        if total > limit {
            return None;
        }
    }
    Some(total)
}

struct BraceSpan {
    start: usize,
    end: usize,
    shape: BraceShape,
}

impl BraceSpan {
    /// Immediate alternative count, capped at `limit + 1` so a huge range is
    /// never built. `None` marks an invalid shape (for example a zero step).
    fn shape_count(&self, limit: usize) -> Option<usize> {
        match &self.shape {
            BraceShape::List(alternatives) => Some(alternatives.len().min(limit.saturating_add(1))),
            BraceShape::Range(range) => range.bounded_count(limit),
        }
    }

    fn shape_values(&self) -> Vec<String> {
        match &self.shape {
            BraceShape::List(alternatives) => alternatives.clone(),
            BraceShape::Range(range) => range.values(),
        }
    }
}

enum BraceShape {
    List(Vec<String>),
    Range(RangeSpec),
}

struct RangeSpec {
    start: i64,
    end: i64,
    step: i64,
    /// Zero-pad width when an endpoint carries a leading zero, else 0.
    width: usize,
    /// Character range (`{a..z}`) renders each value as a character.
    is_char: bool,
}

impl RangeSpec {
    /// Number of expanded strings, capped at `limit + 1`. A padded range emits
    /// both the zero-padded and the zero-stripped form, so it doubles.
    fn bounded_count(&self, limit: usize) -> Option<usize> {
        if self.step == 0 {
            return None;
        }
        let span = (self.end as i128 - self.start as i128).unsigned_abs();
        let size = span / self.step as u128 + 1;
        let forms = if self.width > 0 { 2u128 } else { 1u128 };
        let count = size * forms;
        Some(if count > limit as u128 + 1 {
            limit.saturating_add(1)
        } else {
            count as usize
        })
    }

    fn values(&self) -> Vec<String> {
        let direction: i64 = if self.end >= self.start { 1 } else { -1 };
        let mut values = Vec::new();
        let mut value = self.start;
        loop {
            if (direction > 0 && value > self.end) || (direction < 0 && value < self.end) {
                break;
            }
            let plain = value.to_string();
            if self.is_char {
                values.push(char::from(value as u8).to_string());
            } else if self.width > 0 {
                values.push(format!("{value:0>width$}", width = self.width));
                values.push(plain);
            } else {
                values.push(plain);
            }
            let Some(next) = direction
                .checked_mul(self.step)
                .and_then(|delta| value.checked_add(delta))
            else {
                break;
            };
            value = next;
        }
        values
    }
}

fn find_brace(input: &str) -> Option<BraceSpan> {
    let bytes = input.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'{' {
            index += 1;
            continue;
        }
        let start = index;
        let mut depth = 0usize;
        let mut close = None;
        let mut cursor = start;
        while cursor < bytes.len() {
            match bytes[cursor] {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(cursor);
                        break;
                    }
                }
                _ => {}
            }
            cursor += 1;
        }
        // An unterminated brace is literal, not an error: the word is proven
        // as written, matching the shell, and never indexes past the input.
        let close = close?;
        if let Some(shape) = parse_brace_shape(&input[start + 1..close]) {
            return Some(BraceSpan {
                start,
                end: close + 1,
                shape,
            });
        }
        // This brace is literal; a nested brace may still be expandable, so
        // resume just after its opening brace rather than skipping it whole.
        index = start + 1;
    }
    None
}

fn parse_brace_shape(content: &str) -> Option<BraceShape> {
    if let Some(parts) = split_top_level_commas(content) {
        return Some(BraceShape::List(parts));
    }
    range_spec(content).map(BraceShape::Range)
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

fn range_spec(content: &str) -> Option<RangeSpec> {
    let parts: Vec<&str> = content.split("..").collect();
    let (from, to, step) = match parts.as_slice() {
        [from, to] => (*from, *to, 1i64),
        [from, to, step] => (*from, *to, step.parse::<i64>().ok()?.abs()),
        _ => return None,
    };
    if step == 0 {
        return None;
    }
    if let (Ok(start), Ok(end)) = (from.parse::<i64>(), to.parse::<i64>()) {
        return Some(RangeSpec {
            start,
            end,
            step,
            width: numeric_pad_width(from, to),
            is_char: false,
        });
    }
    let start = single_ascii_char(from)?;
    let end = single_ascii_char(to)?;
    Some(RangeSpec {
        start: i64::from(start),
        end: i64::from(end),
        step,
        width: 0,
        is_char: true,
    })
}

fn numeric_pad_width(from: &str, to: &str) -> usize {
    if has_leading_zero(from) || has_leading_zero(to) {
        from.len().max(to.len())
    } else {
        0
    }
}

fn has_leading_zero(value: &str) -> bool {
    let digits = value.strip_prefix('-').unwrap_or(value);
    digits.len() > 1 && digits.starts_with('0')
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
    // globset does not implement POSIX bracket expressions (`[[:class:]]`,
    // `[[=c=]]`, `[[.coll.]]`); it would read them as an ordinary class and
    // silently match a different set than the shell. Refuse rather than fall
    // back to the literal spelling and allow a word the shell expands.
    if name.contains("[:") || name.contains("[=") || name.contains("[.") {
        bail!(
            "Bash write target glob uses a POSIX bracket expression (`[:`/`[=`/`[.`) that cannot be proven to remain in the workspace"
        );
    }
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
    let pattern = globset_pattern(name);
    Glob::new(&pattern).ok().map(|glob| glob.compile_matcher())
}

/// Translate one path component into globset's syntax.
///
/// The shell reads a leftover `{`, `}`, `,`, or `\` as an ordinary character,
/// while globset gives all four a special meaning (alternation and escaping).
/// Rewrite them into character-class literals (`[{]`, `[}]`, `[,]`, `[\\]`) so
/// both engines match the same names, and collapse `*` runs because `**` inside
/// one component behaves like `*` in the shell and globset rejects it there.
fn globset_pattern(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut previous_star = false;
    for ch in name.chars() {
        match ch {
            '*' => {
                if previous_star {
                    continue;
                }
                previous_star = true;
                out.push('*');
            }
            '{' => {
                previous_star = false;
                out.push_str("[{]");
            }
            '}' => {
                previous_star = false;
                out.push_str("[}]");
            }
            ',' => {
                previous_star = false;
                out.push_str("[,]");
            }
            '\\' => {
                previous_star = false;
                out.push_str("[\\\\]");
            }
            _ => {
                previous_star = false;
                out.push(ch);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn contains_expansion_table() {
        assert!(contains_expansion("src/[id]"));
        assert!(contains_expansion("lin*/secret"));
        assert!(contains_expansion("a?b"));
        assert!(contains_expansion("{a,b}"));
        assert!(!contains_expansion("plain/path.txt"));
    }

    #[test]
    fn globset_pattern_renders_shell_literals() {
        assert_eq!(globset_pattern("plain"), "plain");
        assert_eq!(globset_pattern("a**b"), "a*b");
        assert_eq!(globset_pattern("**"), "*");
        assert_eq!(globset_pattern("{x}*"), "[{]x[}]*");
        assert_eq!(globset_pattern("{a,b}"), "[{]a[,]b[}]");
        assert_eq!(globset_pattern("a\\b"), "a[\\\\]b");
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
        assert_eq!(expand_braces("{{a,b},y}").unwrap(), ["a", "b", "y"]);
        assert_eq!(expand_braces("{a{b,c}}").unwrap(), ["{ab}", "{ac}"]);
        assert_eq!(expand_braces("{k..m}").unwrap(), ["k", "l", "m"]);
        assert_eq!(expand_braces("{1..3}").unwrap(), ["1", "2", "3"]);
        assert_eq!(expand_braces("{3..1}").unwrap(), ["3", "2", "1"]);
        assert_eq!(expand_braces("{1..5..2}").unwrap(), ["1", "3", "5"]);
        assert_eq!(expand_braces("{01..02}").unwrap(), ["01", "1", "02", "2"]);
        assert_eq!(expand_braces("{01..2}").unwrap(), ["01", "1", "02", "2"]);
        assert_eq!(expand_braces("{1..02}").unwrap(), ["01", "1", "02", "2"]);
        assert_eq!(expand_braces("{a}").unwrap(), ["{a}"]);
        assert_eq!(expand_braces("{x}y{z}").unwrap(), ["{x}y{z}"]);
    }

    #[test]
    fn unterminated_and_non_range_braces_stay_literal() {
        assert_eq!(expand_braces("{a,b").unwrap(), ["{a,b"]);
        assert_eq!(expand_braces("{a").unwrap(), ["{a"]);
        assert_eq!(expand_braces("{ab..d}").unwrap(), ["{ab..d}"]);
        assert_eq!(expand_braces("{a..}").unwrap(), ["{a..}"]);
    }

    #[test]
    fn over_limit_braces_are_counted_and_rejected_without_expanding() {
        let start = Instant::now();
        for word in [
            "{a,b}".repeat(12),
            "{a,b}".repeat(30),
            "{1..100000000}".to_string(),
            "{1..100000000}{1..100000000}".to_string(),
            "{-9223372036854775808..9223372036854775807}".to_string(),
        ] {
            let error = expand_braces(&word).unwrap_err();
            assert!(error.to_string().contains("256-expansion limit"), "{word}");
        }
        assert!(
            start.elapsed().as_secs() < 5,
            "over-limit braces must be counted before expanding"
        );
    }

    #[test]
    fn extreme_range_endpoints_do_not_panic() {
        assert_eq!(
            expand_braces("{9223372036854775806..9223372036854775807}").unwrap(),
            ["9223372036854775806", "9223372036854775807"]
        );
        assert_eq!(
            expand_braces("{9223372036854775807..9223372036854775807}").unwrap(),
            ["9223372036854775807"]
        );
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
        assert!(expand_glob(&root, "[[:alpha:]]inked-outside").is_err());

        assert_eq!(expand_glob(&root, "nomatch*/f").unwrap(), None);

        let matched = expand_glob(&root, "sub/*").unwrap().expect("sub/f match");
        assert_eq!(matched.len(), 1, "{matched:?}");
        assert!(matched[0].ends_with("sub/f"), "{matched:?}");
    }
}
