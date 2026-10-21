use std::path::Path;

use anyhow::bail;

pub fn run(
    root: &Path,
    path: &Path,
    old: &str,
    new: &str,
    replace_all: bool,
) -> anyhow::Result<String> {
    crate::tools::write::ensure_mutation_allowed(root, path)?;
    #[cfg(unix)]
    {
        // One parent fd for the whole read-modify-write, so a parent swapped for
        // a symlink after the pre-checks and after the read cannot redirect the
        // write. The path-based checks above keep their wording and class.
        super::dir_fd::fire_seam(path);
        let parent = super::dir_fd::open_parent(root, path, false)?;
        run_with_parent(&parent, path, old, new, replace_all)
    }
    #[cfg(not(unix))]
    {
        let _ = (root, path, old, new, replace_all);
        bail!("edit refused: fd-relative writes are unavailable on this platform");
    }
}

#[cfg(unix)]
fn run_with_parent(
    parent: &super::dir_fd::ParentDir,
    path: &Path,
    old: &str,
    new: &str,
    replace_all: bool,
) -> anyhow::Result<String> {
    let content = parent.read_leaf_to_string()?;
    if old == new {
        bail!("edit_noop: old_string and new_string are identical");
    }
    if !content.contains(old) && !new.is_empty() && content.contains(new) {
        return Ok(format!("edit_already_applied {}", path.display()));
    }
    let occurrences = content.matches(old).count();
    if occurrences > 1 && !replace_all {
        bail!(
            "edit_ambiguous_anchor: old_string appears {occurrences} times; Read the file and provide a more specific anchor"
        );
    }
    if !content.contains(old) {
        if let Some(salvage) = normalized_anchor_salvage(&content, old, new) {
            parent.write_leaf(&salvage.edited)?;
            return Ok(format!(
                "edited {} via edit_anchor_salvaged at line {}",
                path.display(),
                salvage.line
            ));
        }
        let best = best_match_region(&content, old)
            .map(|region| {
                format!(
                    " Deterministic best-match region: {}:{}\n{}\nRe-anchor mandate: retry using the exact current excerpt above, or use Write with the complete corrected file content if anchor failures repeat.",
                    path.display(),
                    region.line,
                    line_numbered_excerpt(&region)
                )
            })
            .unwrap_or_else(|| {
                " Re-anchor mandate: Read the file again and retry with an exact current excerpt, or use Write with the complete corrected file content if anchor failures repeat.".to_string()
            });
        bail!(
            "edit_anchor_not_found: exact anchor mismatch; whitespace-normalized anchor did not identify one unique region.{best}"
        );
    }
    let edited = if replace_all {
        content.replace(old, new)
    } else {
        content.replacen(old, new, 1)
    };
    parent.write_leaf(&edited)?;
    Ok(format!("edited {}", path.display()))
}

#[derive(Debug, Clone)]
struct AnchorSalvage {
    edited: String,
    line: usize,
}

#[derive(Debug, Clone)]
struct AnchorRegion {
    start: usize,
    end: usize,
    line: usize,
    excerpt: String,
}

fn normalized_anchor_salvage(content: &str, old: &str, new: &str) -> Option<AnchorSalvage> {
    let matches = normalized_anchor_matches(content, old);
    if matches.len() != 1 {
        return None;
    }
    let region = matches.into_iter().next()?;
    let mut edited = String::new();
    edited.push_str(&content[..region.start]);
    edited.push_str(new);
    edited.push_str(&content[region.end..]);
    Some(AnchorSalvage {
        edited,
        line: region.line,
    })
}

fn normalized_anchor_matches(content: &str, old: &str) -> Vec<AnchorRegion> {
    let old_normalized = normalize_ws(old);
    if old_normalized.is_empty() {
        return Vec::new();
    }
    let old_line_count = old.lines().count().max(1);
    let lines = line_regions(content);
    if lines.len() < old_line_count {
        return Vec::new();
    }
    let mut matches = Vec::new();
    for window in lines.windows(old_line_count) {
        let start = window.first().map(|region| region.start).unwrap_or(0);
        let end = window.last().map(|region| region.end).unwrap_or(start);
        let excerpt = content[start..end].to_string();
        if normalize_ws(&excerpt) == old_normalized {
            matches.push(AnchorRegion {
                start,
                end,
                line: window.first().map(|region| region.line).unwrap_or(1),
                excerpt,
            });
        }
    }
    matches
}

fn best_match_region(content: &str, old: &str) -> Option<AnchorRegion> {
    let old_tokens = old.split_whitespace().collect::<Vec<_>>();
    if old_tokens.is_empty() {
        return None;
    }
    let old_line_count = old.lines().count().max(1);
    let lines = line_regions(content);
    if lines.is_empty() {
        return None;
    }
    let window_size = old_line_count.min(lines.len()).max(1);
    lines
        .windows(window_size)
        .map(|window| {
            let start = window.first().map(|region| region.start).unwrap_or(0);
            let end = window.last().map(|region| region.end).unwrap_or(start);
            let excerpt = content[start..end].to_string();
            let score = old_tokens
                .iter()
                .filter(|token| excerpt.contains(**token))
                .count();
            (
                score,
                AnchorRegion {
                    start,
                    end,
                    line: window.first().map(|region| region.line).unwrap_or(1),
                    excerpt,
                },
            )
        })
        .max_by_key(|(score, _)| *score)
        .and_then(|(score, region)| (score > 0).then_some(region))
}

fn line_regions(content: &str) -> Vec<AnchorRegion> {
    let mut out = Vec::new();
    let mut start = 0;
    for (idx, line) in content.split_inclusive('\n').enumerate() {
        let end = start + line.trim_end_matches('\n').len();
        out.push(AnchorRegion {
            start,
            end,
            line: idx + 1,
            excerpt: content[start..end].to_string(),
        });
        start += line.len();
    }
    if start < content.len() || content.is_empty() {
        out.push(AnchorRegion {
            start,
            end: content.len(),
            line: out.len() + 1,
            excerpt: content[start..].to_string(),
        });
    }
    out
}

fn line_numbered_excerpt(region: &AnchorRegion) -> String {
    let lines = region.excerpt.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        return format!("{:>4} | ", region.line);
    }
    lines
        .into_iter()
        .enumerate()
        .map(|(offset, line)| format!("{:>4} | {}", region.line + offset, line))
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalize_ws(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_already_applied_is_success() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        std::fs::write(&path, "new").unwrap();
        let output = run(dir.path(), &path, "old", "new", false).unwrap();
        assert!(output.contains("already_applied"));
    }

    #[test]
    fn edit_noop_returns_recoverable_feedback() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        std::fs::write(&path, "same").unwrap();
        let err = run(dir.path(), &path, "same", "same", false)
            .unwrap_err()
            .to_string();
        assert!(err.contains("edit_noop"));
    }

    #[test]
    fn edit_normalized_line_fallback_applies_change() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        std::fs::write(&path, "const  x = 1;\n").unwrap();
        let output = run(dir.path(), &path, "const x = 1;", "const x = 2;", false).unwrap();
        assert!(output.contains("edit_anchor_salvaged"));
        assert_eq!(std::fs::read_to_string(path).unwrap(), "const x = 2;\n");
    }

    #[test]
    fn edit_normalized_multiline_fallback_applies_change() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        std::fs::write(&path, "function run() {\n  return   1;\n}\n").unwrap();
        let output = run(
            dir.path(),
            &path,
            "function run() {\nreturn 1;\n}",
            "function run() {\n  return 2;\n}",
            false,
        )
        .unwrap();
        assert!(output.contains("edit_anchor_salvaged"));
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "function run() {\n  return 2;\n}\n"
        );
    }

    #[test]
    fn edit_token_anchor_fallback_no_longer_applies_without_normalized_match() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        std::fs::write(&path, "alpha middle omega").unwrap();
        let err = run(dir.path(), &path, "alpha changed omega", "done", false)
            .unwrap_err()
            .to_string();
        assert!(err.contains("edit_anchor_not_found"));
        assert!(err.contains("Deterministic best-match region"));
        assert!(err.contains("1 | alpha middle omega"));
        assert!(err.contains("alpha middle omega"));
        assert!(err.contains("Re-anchor mandate"));
        assert_eq!(std::fs::read_to_string(path).unwrap(), "alpha middle omega");
    }

    #[test]
    fn edit_fallback_does_not_apply_when_multiple_candidates_exist() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        std::fs::write(&path, "alpha one omega\nalpha two omega\n").unwrap();
        let err = run(dir.path(), &path, "alpha changed omega", "done", false)
            .unwrap_err()
            .to_string();
        assert!(err.contains("edit_anchor_not_found"));
        assert!(err.contains("Re-anchor mandate"));
    }

    const OUTSIDE_CANARY: &str = "OUTSIDE_CANARY_505";

    fn install_swap(root: &Path, name: &str) -> crate::tools::dir_fd::test_hook::Reset {
        let swapped = root.join(name);
        let outside = root.parent().unwrap().join("outside_edit_505");
        std::fs::create_dir_all(&outside).unwrap();
        let name = name.to_string();
        crate::tools::dir_fd::test_hook::install(move |component: &Path| {
            if component.file_name().and_then(|part| part.to_str()) == Some(name.as_str()) {
                let _ = std::fs::remove_dir_all(&swapped);
                std::os::unix::fs::symlink(&outside, &swapped).unwrap();
            }
        })
    }

    #[test]
    fn edit_parent_swap_between_check_and_read_does_not_touch_outside() {
        let hold = tempfile::tempdir().unwrap();
        let root = hold.path().join("W");
        std::fs::create_dir_all(root.join("d")).unwrap();
        std::fs::write(root.join("d/f.txt"), "anchor_505\n").unwrap();
        let outside = root.parent().unwrap().join("outside_edit_505");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("f.txt"), "outside-unchanged").unwrap();
        let _guard = install_swap(&root, "d");

        let err = run(
            &root,
            &root.join("d/f.txt"),
            "anchor_505",
            "replaced",
            false,
        )
        .unwrap_err();
        assert!(err.to_string().contains("path escapes workspace"), "{err}");
        assert_eq!(
            std::fs::read_to_string(outside.join("f.txt")).unwrap(),
            "outside-unchanged"
        );
    }

    #[test]
    fn edit_parent_swap_never_leaks_outside_canary_in_error() {
        let hold = tempfile::tempdir().unwrap();
        let root = hold.path().join("W");
        std::fs::create_dir_all(root.join("d")).unwrap();
        std::fs::write(root.join("d/f.txt"), "inside anchor\n").unwrap();
        let outside = root.parent().unwrap().join("outside_edit_505");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("f.txt"), format!("{OUTSIDE_CANARY}\n")).unwrap();

        // The same simil-shaped anchor does mismatch the inside content, so the
        // refusal below is the fs guard, not an anchor-only shortcut.
        let mismatch = run(&root, &root.join("d/f.txt"), "similar anchor", "x", false).unwrap_err();
        assert!(
            mismatch.to_string().contains("edit_anchor_not_found"),
            "{mismatch}"
        );

        let _guard = install_swap(&root, "d");
        let err = run(&root, &root.join("d/f.txt"), "similar anchor", "x", false).unwrap_err();
        assert!(err.to_string().contains("path escapes workspace"), "{err}");
        assert!(!err.to_string().contains(OUTSIDE_CANARY), "{err}");
        assert!(!format!("{err:?}").contains(OUTSIDE_CANARY), "{err:?}");
        assert_eq!(
            std::fs::read_to_string(outside.join("f.txt")).unwrap(),
            format!("{OUTSIDE_CANARY}\n")
        );
    }
}
