//! Issue #966: the heading line of a `Read` result.
//!
//! The target reaches `Read` canonicalized by `resolve_existing_checked`,
//! while the workspace root keeps the caller's spelling, which may pass
//! through a symlink (`/var` → `/private/var` on macOS, or a symlinked
//! workspace). The shared `path_guard::relative_display` strips only the root
//! as spelled, so such a root printed an absolute heading.
//!
//! The heading is made from the pair the containment check used: the checked
//! canonical root and the target. The root is never resolved again here, so a
//! root alias that changes after the check cannot relabel the target. Without
//! a checked root, or for a target outside it, the shared helper's output is
//! kept unchanged. This is display only: it never decides what is read or
//! refused.

use std::path::{Component, Path};

use crate::tools::path_guard::relative_display;

pub(super) fn heading(root: &Path, checked_root: Option<&Path>, path: &Path) -> String {
    if let Some(checked_root) = checked_root
        && !path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        && path.strip_prefix(checked_root).is_ok()
    {
        return relative_display(checked_root, path);
    }
    relative_display(root, path)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn canonical_root_heading_is_the_shared_helper_output() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        for path in [
            root.join("a/b.txt"),
            root.clone(),
            Path::new("/outside/x.txt").to_path_buf(),
        ] {
            for checked in [None, Some(root.as_path())] {
                assert_eq!(
                    heading(&root, checked, &path),
                    relative_display(&root, &path)
                );
            }
        }
        assert_eq!(
            heading(&root, Some(&root), &root.join("空白 x/ノ.txt")),
            "空白 x/ノ.txt"
        );
    }

    #[test]
    fn checked_root_makes_a_symlinked_root_heading_relative() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir_all(real.join("sub")).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let canonical = real.canonicalize().unwrap();
        let target = canonical.join("sub/f.txt");

        assert_eq!(heading(&link, Some(&canonical), &target), "sub/f.txt");
        assert_eq!(heading(&link, Some(&canonical), &canonical), "");
        // Without the checked root nothing is resolved: the shared helper's
        // absolute output, which this issue does not change.
        assert_eq!(heading(&link, None, &target), target.to_string_lossy());
        assert_eq!(relative_display(&link, &target), target.to_string_lossy());
    }

    #[test]
    fn the_root_is_not_resolved_again_for_the_heading() {
        let dir = tempfile::tempdir().unwrap();
        let parent = dir.path().canonicalize().unwrap().join("parent");
        let checked = parent.join("ws");
        std::fs::create_dir_all(&checked).unwrap();
        let link = dir.path().join("link");
        // The alias now names the parent, not the root that was checked.
        std::os::unix::fs::symlink(&parent, &link).unwrap();
        let target = checked.join("file.txt");

        assert_eq!(heading(&link, Some(&checked), &target), "file.txt");
        assert_eq!(heading(&link, None, &target), target.to_string_lossy());
    }

    #[test]
    fn outside_paths_keep_their_absolute_heading() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir_all(&real).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let canonical = real.canonicalize().unwrap();
        let sibling = canonical.parent().unwrap().join("real-sibling/f.txt");
        let parent_escape = canonical.join("../outside/f.txt");

        for path in [&sibling, &parent_escape] {
            assert_eq!(
                heading(&link, Some(&canonical), path),
                path.to_string_lossy(),
                "{path:?}"
            );
        }
    }
}
