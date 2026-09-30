use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, bail};

pub fn run(root: &Path, path: &Path, content: &str) -> anyhow::Result<String> {
    write_checked(root, path, content)?;
    Ok(format!("wrote {}", path.display()))
}

pub fn write_checked(root: &Path, path: &Path, content: &str) -> anyhow::Result<()> {
    ensure_mutation_allowed(root, path)?;
    #[cfg(unix)]
    {
        // The path-based pre-checks above keep their wording and classification;
        // the fd walk below is the authority, so a parent swapped for a symlink
        // after those checks is still refused instead of followed.
        super::dir_fd::fire_seam(path);
        let parent = super::dir_fd::open_parent(root, path, true)?;
        parent.write_leaf(content)?;
    }
    #[cfg(not(unix))]
    {
        let _ = (root, path, content);
        bail!("symlink_write_blocked: fd-relative writes are unavailable on this platform");
    }
    Ok(())
}

pub fn ensure_mutation_allowed(root: &Path, path: &Path) -> anyhow::Result<()> {
    super::sensitive_path::ensure_not_sensitive_path(root, path)?;
    reject_target_symlink(path)?;
    verify_existing_components_inside(root, path)
}

fn reject_target_symlink(path: &Path) -> anyhow::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            bail!(
                "symlink_write_blocked: refusing to write through {}",
                path.display()
            )
        }
        Ok(_) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => {
            Err(err).with_context(|| format!("failed to inspect target {}", path.display()))
        }
    }
}

fn verify_existing_components_inside(root: &Path, path: &Path) -> anyhow::Result<()> {
    let raw_root = root.to_path_buf();
    let root = root
        .canonicalize()
        .with_context(|| format!("workspace root is not accessible: {}", root.display()))?;
    let candidate = if path.is_absolute() && path.starts_with(&raw_root) {
        root.join(
            path.strip_prefix(&raw_root)
                .with_context(|| format!("path escapes workspace: {}", path.display()))?,
        )
    } else if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let relative = candidate
        .strip_prefix(&root)
        .with_context(|| format!("path escapes workspace: {}", candidate.display()))?;
    let mut current = PathBuf::from(&root);
    for component in relative.components() {
        let Component::Normal(part) = component else {
            bail!("path escapes workspace: {}", candidate.display());
        };
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(_) => {
                let canonical = current.canonicalize().with_context(|| {
                    format!("path component is not accessible: {}", current.display())
                })?;
                if !canonical.starts_with(&root) {
                    bail!(
                        "path escapes workspace through existing component {}; use workspace-relative paths",
                        current.display()
                    );
                }
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => break,
            Err(err) => {
                return Err(err).with_context(|| {
                    format!("failed to inspect path component {}", current.display())
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Deterministic parent-swap regression for Write (Issue #505). The hook in
    //! `dir_fd` replaces the inspected parent with a symlink to an "outside"
    //! directory after the pre-checks and before the fd walk. Synthetic values
    //! only.

    use std::fs;
    use std::path::{Path, PathBuf};

    use super::write_checked;
    use crate::tools::dir_fd::test_hook::{Reset, install};

    const PAYLOAD: &str = "PAYLOAD_505";
    const OUTSIDE_CANARY: &str = "OUTSIDE_CANARY_505";

    fn outside_of(root: &Path) -> PathBuf {
        let outside = root.parent().unwrap().join("outside_write_505");
        fs::create_dir_all(&outside).unwrap();
        outside
    }

    fn install_swap(hold: &Path, name: &str) -> Reset {
        let swapped = hold.join(name);
        let outside = hold.parent().unwrap().join("outside_write_505");
        fs::create_dir_all(&outside).unwrap();
        let name = name.to_string();
        install(move |component: &Path| {
            if component.file_name().and_then(|part| part.to_str()) == Some(name.as_str()) {
                let _ = fs::remove_dir_all(&swapped);
                std::os::unix::fs::symlink(&outside, &swapped).unwrap();
            }
        })
    }

    fn outside_entries(outside: &Path) -> Vec<std::ffi::OsString> {
        match fs::read_dir(outside) {
            Ok(entries) => entries.map(|entry| entry.unwrap().file_name()).collect(),
            Err(_) => Vec::new(),
        }
    }

    #[test]
    fn new_file_parent_swap_creates_nothing_outside() {
        let hold = tempfile::tempdir().unwrap();
        let root = hold.path().join("W");
        fs::create_dir_all(root.join("d")).unwrap();
        let _guard = install_swap(&root, "d");

        let err = write_checked(&root, &root.join("d/f.txt"), PAYLOAD).unwrap_err();
        assert!(err.to_string().contains("path escapes workspace"), "{err}");
        let outside = outside_of(&root);
        assert!(!outside.join("f.txt").exists());
        assert!(outside_entries(&outside).is_empty());
    }

    #[test]
    fn existing_file_parent_swap_does_not_truncate_outside() {
        let hold = tempfile::tempdir().unwrap();
        let root = hold.path().join("W");
        fs::create_dir_all(root.join("d")).unwrap();
        fs::write(root.join("d/f.txt"), "inside").unwrap();
        let outside = outside_of(&root);
        fs::write(outside.join("f.txt"), OUTSIDE_CANARY).unwrap();
        let _guard = install_swap(&root, "d");

        let err = write_checked(&root, &root.join("d/f.txt"), PAYLOAD).unwrap_err();
        assert!(err.to_string().contains("path escapes workspace"), "{err}");
        assert_eq!(
            fs::read_to_string(outside.join("f.txt")).unwrap(),
            OUTSIDE_CANARY
        );
    }

    #[test]
    fn nested_parent_swap_creates_nothing_outside() {
        let hold = tempfile::tempdir().unwrap();
        let root = hold.path().join("W");
        fs::create_dir_all(root.join("d")).unwrap();
        let _guard = install_swap(&root, "d");

        let err = write_checked(&root, &root.join("d/sub/f.txt"), PAYLOAD).unwrap_err();
        assert!(err.to_string().contains("path escapes workspace"), "{err}");
        let outside = outside_of(&root);
        assert!(!outside.join("sub").exists());
        assert!(outside_entries(&outside).is_empty());
    }

    #[test]
    fn ordinary_replace_keeps_the_inode() {
        use std::os::unix::fs::MetadataExt;

        let hold = tempfile::tempdir().unwrap();
        let root = hold.path().join("W");
        fs::create_dir_all(root.join("d")).unwrap();
        fs::write(root.join("d/f.txt"), "before").unwrap();
        let before = fs::metadata(root.join("d/f.txt")).unwrap().ino();

        write_checked(&root, &root.join("d/f.txt"), "after").unwrap();

        let after = fs::metadata(root.join("d/f.txt")).unwrap();
        assert_eq!(before, after.ino(), "in-place truncate must keep the inode");
        assert_eq!(fs::read_to_string(root.join("d/f.txt")).unwrap(), "after");
    }
}
