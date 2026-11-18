//! Issue #966: the heading line of a `Read` result.
//!
//! The target path reaches `Read` already canonicalized (`resolve_existing`),
//! while the workspace root keeps the caller's spelling, which may pass
//! through a symlink (`/var` → `/private/var` on macOS, or a symlinked
//! workspace). The shared `path_guard::relative_display` strips only the root
//! as spelled, so such a root printed an absolute heading. This helper retries
//! with the canonical root, the same way `sensitive_path::strip_root` does.
//!
//! It is display only: it never decides what is read or refused. A path that
//! is outside the root in both spellings, or a root that cannot be
//! canonicalized, keeps the shared helper's output unchanged.

use std::path::{Component, Path};

use crate::tools::path_guard::relative_display;

pub(super) fn heading(root: &Path, path: &Path) -> String {
    if path.strip_prefix(root).is_err()
        && !path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        && let Ok(canonical_root) = root.canonicalize()
        && path.strip_prefix(&canonical_root).is_ok()
    {
        return relative_display(&canonical_root, path);
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
            assert_eq!(heading(&root, &path), relative_display(&root, &path));
        }
        assert_eq!(heading(&root, &root.join("空白 x/ノ.txt")), "空白 x/ノ.txt");
    }

    #[test]
    fn symlinked_root_heading_is_workspace_relative() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir_all(real.join("sub")).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let canonical = real.canonicalize().unwrap();

        assert_eq!(heading(&link, &canonical.join("sub/f.txt")), "sub/f.txt");
        assert_eq!(heading(&link, &canonical), "");
        // The shared helper is not changed by this issue.
        assert_eq!(
            relative_display(&link, &canonical.join("sub/f.txt")),
            canonical.join("sub/f.txt").to_string_lossy()
        );
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
            assert_eq!(heading(&link, path), path.to_string_lossy(), "{path:?}");
        }
    }

    #[test]
    fn uncanonicalizable_root_keeps_the_shared_helper_output() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing");
        let dangling = dir.path().join("dangling");
        std::os::unix::fs::symlink(&missing, &dangling).unwrap();
        let target = dir.path().canonicalize().unwrap().join("missing/f.txt");

        for root in [&missing, &dangling] {
            assert_eq!(heading(root, &target), relative_display(root, &target));
        }
    }
}
